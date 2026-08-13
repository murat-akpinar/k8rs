//! The rule engine — the bottom of the pyramid. Pure functions over a snapshot:
//! no network, no terminal, no globals, no `Result`, no clock call — the snapshot
//! carries `now`, so a fixture cannot expire (CLAUDE.md invariant 5).
//!
//! The contract `rules.rs`, `views.rs` and the `--once` printer meet on. The
//! rules, the snapshot types and the timestamp are later boxes of Phase 3.

// `expect` rather than `allow` because it expires by itself — and the box that constructs the
// last item in this file is the one that deletes this attribute, pre-authorised and not a freeze
// violation. Its module-wide blind spot is accepted (NOTES § D38).
#![expect(dead_code, reason = "the rules that fill these in are the next boxes")]

use k8s_openapi::api::apps::v1::{
    DaemonSet, DaemonSetCondition, Deployment, DeploymentCondition, ReplicaSet,
    ReplicaSetCondition, StatefulSet, StatefulSetCondition,
};
use k8s_openapi::api::core::v1::{
    ContainerState as ApiContainerState, ContainerStateTerminated, ContainerStatus, Node,
    NodeCondition, Pod, PodCondition, PodSpec, ResourceRequirements,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ObjectMeta, Time};
use k8s_openapi::jiff::SignedDuration;
use std::collections::BTreeMap;

/// How bad it is. **Declaration order is severity order** — the derived `Ord` sorts the
/// Alerts list and `--once`, and a test asserts it (NOTES § D35).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Broken now: something is not doing its job and someone has to answer it.
    Critical,
    /// Wrong now, broken soon. It still needs an answer, just not this minute.
    Warn,
    /// Worth knowing; nothing here is broken. No rule reaching the Alerts list produces one
    /// (NOTES § D2) — but a rule can live in this file and still be `Info` (N4's kubelet
    /// skew → the Versions report). Both files share this scale.
    Info,
}

/// The kind of Kubernetes object a finding names (NOTES § D36).
///
/// **An `ownerReference` of kind `Node` is discarded, never carried into `owner`** — a mirror
/// pod files under itself (`owner == object`, kind `Pod`), and `ObjectKind::Node` appears in
/// the `owner` role only when the finding is about the node itself, N1–N3 (NOTES § D39). Why the
/// set runs past D3's four kinds — `CronJob`, `ReplicaSet`, `Other` — is D36's.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Deployment,
    StatefulSet,
    DaemonSet,
    Job,
    CronJob,
    ReplicaSet,
    /// A node. Cluster-scoped, so its `ObjectId` carries no namespace; in the `owner`
    /// role only for N1–N3 (see the type doc).
    Node,
    /// A pod. In the `owner` role it additionally means nothing controls it, or its
    /// controller is a Node and was discarded (see the type doc).
    Pod,
    /// The kind as the API reported it, **qualified by its API group when it has one** —
    /// `Other("Rollout.argoproj.io")` — or, for rule C1, `Other("kubeconfig")` with the
    /// kubeconfig context name and no uid (NOTES § D39, § D51).
    ///
    /// **`Kind.group` is not how `kubectl` spells a resource**, only what its RESTMapper
    /// accepts. The consequence is Phase 7's: **a `kubectl_cmd` built from an `Other(_)` must
    /// lowercase it**, or invariant 4's teaching device prints a form no documentation shows.
    Other(String),
}

/// One Kubernetes object, identified the way a human would identify it. Every `Finding`
/// carries two: the one it is filed under and the one it is about.
///
/// Two questions, two mechanisms (NOTES § D38): the derived `Eq` over all four fields answers
/// *are these the same object?*, [`ObjectId::group_key`] answers *which card is this?*.
///
/// `Hash` is **deliberately not derived**, so keying a map on the whole identity stops
/// compiling. **The error arrives one line later than you expect, with bad advice**: the bound
/// sits on `insert`/`get`/`entry`, not `HashMap::new`, and rustc suggests deriving `Hash` —
/// which is the two-cards bug. Add `group_key()` to the call instead, except when the call is
/// *counting*, where the answer is a `Vec` (see [`Finding::object`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectId {
    pub kind: ObjectKind,
    /// `payments` in `payments/web`. `None` means cluster-scoped — an `Option` because `""`
    /// draws as `/node-3` and builds `-n ""`, a command that does not work, printed in the
    /// record invariant 4 says may not lie (NOTES § D36).
    pub namespace: Option<String>,
    /// The name, read per role. In `owner`: the controller's, resolved up to the Deployment
    /// where there is one and stopping at the ReplicaSet where the chain does. In `object`:
    /// the object's own — W1's object is a ReplicaSet. Resolving one to the other is
    /// `k8s.rs`'s job (Phase 5, NOTES § D28).
    pub name: String,
    /// The object's UID, so a confirmation cannot act on the object that replaced the one the
    /// user selected (NOTES § D22, § D38). A workload owner always carries one; rule C1's
    /// kubeconfig certificate is the only `None`. **Group members agree on the *owner's* uid,
    /// and where a card holds two the dialog refuses and offers a re-read** (NOTES § D39,
    /// Phase 7/9). `resourceVersion` is deliberately not a field (NOTES § D36).
    pub uid: Option<String>,
}

impl ObjectId {
    /// The identity findings are grouped by: kind, namespace, name — **not** the uid. One card
    /// per owner (NOTES § D3), decided here so `views.rs` cannot hold a second copy that
    /// drifts.
    pub fn group_key(&self) -> (&ObjectKind, Option<&str>, &str) {
        (&self.kind, self.namespace.as_deref(), &self.name)
    }
}

/// One thing that is wrong, in three parts: what happened · what it means · what to do.
///
/// **Every string reachable from here is untrusted, identities included** (invariant 9).
/// Nothing here or downstream strips control characters yet; where the guard goes is the
/// decision of this phase's last box, the temporary `main.rs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// How bad it is, and therefore where it lands in the list.
    pub severity: Severity,
    /// **What happened**, translated: "Containers exceeded their memory limit and were killed
    /// by the kernel", not `OOMKilled` printed and left (invariant 14). The raw reason may
    /// follow in brackets; it never replaces the sentence.
    pub title: String,
    /// **What it means** — the fields and numbers that prove the title:
    /// `limit 256Mi · exit 137 · 47 restarts`. A controller's own status message is quoted
    /// **verbatim** (NOTES § D37); what is absolute is what k8rs *fetches* — never Secret
    /// data, never an environment variable value. The type cannot enforce that; rule authors
    /// do.
    ///
    /// **This can be empty, and an empty one is drawn by leaving the line out** — not by
    /// drawing a blank one, which is a hole in the middle of a card. [`no_node_accepted_it`]
    /// is the first rule that can produce it, and the renderers (Phase 9, Phase 11) owe it the
    /// same answer as a missing age.
    pub evidence: String,
    /// **What to do** about it, in one line the reader can act on.
    pub action: String,
    /// The `kubectl` command that shows the same thing, as the user would have typed it — the
    /// teaching device (invariant 4). Display text only: k8rs never executes it and never
    /// feeds it back into a process. `None` means **no such command exists** — never "the rule
    /// author had not got round to it" (NOTES § D36).
    pub kubectl_cmd: Option<String>,
    /// **The grouping key** — what this finding is filed under. One card per owner, never per
    /// pod (NOTES § D3). `rules.rs` decides the identity; `views.rs` does the grouping. Equals
    /// `object` whenever nothing controls the subject **or its controller is a Node and is
    /// discarded** (NOTES § D39).
    pub owner: ObjectId,
    /// **What the finding is actually about** — whatever the rule looked at, which is not
    /// always a pod: a ReplicaSet for W1, a Deployment for W2, a node for N1–N3.
    ///
    /// **The numerator of D3's "3 of 40 pods" is the number of distinct `object`s in the group
    /// whose `kind` is `Pod`, and a group with none of those has no `n of m` at all**
    /// (NOTES § D39) — this is the whole spec `views.rs` (Phase 9) gets. **Distinct is the
    /// whole `object`, uid included**: a `Vec` and a linear `contains`, not a `HashSet` of
    /// `group_key()`, which answers *which card* rather than *what is counted on it*.
    ///
    /// The denominator is the group's total pod count, from the snapshot. This is also the
    /// only source for the detail view's first act (`screens/detail.md`).
    pub object: ObjectId,
    /// **When the event this card is about happened — the moment, never the phrase.** A renderer
    /// calls [`Finding::age`], never the free [`age`] on this field (NOTES § D18, § D69).
    ///
    /// **A rule may fill this only from a field that records the event itself.** The wrong one is
    /// never missing — it is three lines away and it draws (NOTES § D69):
    ///
    /// | rule | the field | the one it is not |
    /// |---|---|---|
    /// | 1, 2, 6 | [`Terminated::finished_at`] on [`ContainerSnapshot::last_terminated`] | `started_at`, one line above it |
    /// | 7 | the **later** of [`PodSnapshot::ready`]'s `last_transition` and the container's own `started_at` — a floor, since `Ready` is pod-scoped (NOTES § D71) | [`PodSnapshot::scheduled`]'s |
    /// | 8 | **`None`** — a standing property, not an event (NOTES § D69) | `metadata.creationTimestamp` |
    /// | 12 | `deletionTimestamp − grace` (NOTES § D46) | the `deletionTimestamp` itself, the deadline |
    /// | 14 | `metadata.creationTimestamp` — the one rule whose event never happened (NOTES § D74) | — |
    /// | N2 | the cordon taint's [`Taint::added_at`], which dates the taint and not the cordon (NOTES § D65) | — |
    /// | N3 | *that* condition's `last_transition` | `Ready`'s, off the same flat `Vec` |
    /// | N6 | the pod's `scheduled` `last_transition` | the blocking node's taint `added_at` |
    ///
    /// A rule not in the table owes the same answer, and owes it in a test.
    ///
    /// **`None` is the empty right edge** — no field to read, or a moment [`age`] refuses. An
    /// `Option` and not a zero: the epoch dates as *20678 days ago* against the pin the tests use.
    ///
    /// **This field says how a finding *renders*, and nothing about how it sorts.**
    /// `screens/alerts.md` wants ageless cards **last** in their band while `Option`'s derived
    /// `Ord` puts `None` **first**, so Phase 9's reflex produces the reverse (NOTES § D69).
    pub timestamp: Option<Time>,
}

impl Finding {
    /// **How long ago this finding's event happened, or nothing** — **the call a renderer makes
    /// for a finding**, so the Alerts view and `--once` cannot disagree, and so [`age`]'s two
    /// same-typed arguments cannot be swapped on the path that matters (NOTES § D69).
    ///
    /// `None` means **draw no age at all**: no timestamp, or one [`age`] itself refuses.
    pub fn age(&self, now: &Time) -> Option<String> {
        self.timestamp.as_ref().and_then(|t| age(now, t))
    }
}

/// **How long ago it happened, in the words the screens already print** — the one place those
/// words are spelled, so two renderers cannot disagree about the same moment (NOTES § D68).
///
/// **For a finding, a renderer calls [`Finding::age`] and not this**; what comes here directly is
/// the age that hangs off no `Finding`, the header's stale-vitals age. So **`now` is the
/// *caller's* moment** — the snapshot's for a finding, a freshly read clock for that age, which
/// has to keep advancing while the snapshot does not (NOTES § D69). The subtraction is
/// `now − event`, that way round (invariant 5, NOTES § D18).
///
/// | age | text | where the spelling comes from |
/// |---|---|---|
/// | ahead by more than [`SKEW_ALLOWANCE`] | **`None`** — draw nothing | `screens/alerts.md` § *No number we cannot produce* |
/// | ahead by less, or under one whole second | `just now` | NOTES § D68 |
/// | under a minute | `40s ago` | `screens/states.md`, the header's stale-vitals age |
/// | under an hour | `4 min ago` | `screens/alerts.md`, `screens/once.md` |
/// | under a day | `2 hours ago`, `1 hour ago` | nothing draws one yet; it follows the days rung |
/// | a day or more | `6 days ago`, `1 day ago` | `screens/alerts.md` |
///
/// Every rung truncates, and **`min` stays abbreviated and unpluralised** because that is how
/// both screens spell it (NOTES § D68).
///
/// **The `None` rung is a wrong-field guard, not a clock feature**, and the other half of the
/// skew is deliberately not clamped (NOTES § D55, § D69). **The arithmetic is
/// `Timestamp::duration_since`, never `-`** (NOTES § D54).
pub fn age(now: &Time, event: &Time) -> Option<String> {
    let elapsed = now.0.duration_since(event.0);
    if elapsed < -SKEW_ALLOWANCE {
        return None;
    }
    Some(if elapsed.as_secs() <= 0 {
        "just now".to_string()
    } else if elapsed.as_mins() < 1 {
        format!("{}s ago", elapsed.as_secs())
    } else if elapsed.as_hours() < 1 {
        format!("{} min ago", elapsed.as_mins())
    } else if elapsed.as_hours() < 24 {
        format!("{} ago", counted(elapsed.as_hours(), "hour"))
    } else {
        format!("{} ago", counted(elapsed.as_hours() / 24, "day"))
    })
}

/// **How far into the future a timestamp may sit and still be read as a wrong clock rather
/// than a wrong field** — five minutes, and past it [`age`] draws nothing.
///
/// Not tuned: it is the clock-skew tolerance the ecosystem already settled on, and it covers
/// an unsynced laptop without covering a certificate expiring next year (NOTES § D69).
const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(5);

/// `1 hour` / `2 hours` — the rungs whose unit is a word the reader pluralises. Not the
/// minutes rung: both screens spell that `4 min` (NOTES § D68).
///
/// **The ` ago` is the caller's**, because two callers need the same length in two tenses:
/// [`age`] says when something happened and appends it, [`lasted`] says how long something
/// took and does not.
fn counted(n: i64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

/// **How long one container run lasted** — `2s`, `40 min`, `3 hours`, `6 days`. Rules 1, 5 and
/// 6 all show it, because it is the first fork of every crashloop triage and
/// `kubectl describe` leaves the subtraction to a human (NOTES § D51).
///
/// **Not [`age`] with the suffix taken off.** A span is not a moment, so both of `age`'s
/// special rungs are wrong here: a run that lasted no measurable time is an ordinary instant
/// crash, and *"under a second"* is the fact rather than a refusal to answer. The rungs and
/// the pluralisation are still shared, through [`counted`].
///
/// `None` when either end is missing, and when the run ended before it began.
fn lasted(run: &Terminated) -> Option<String> {
    let elapsed = run
        .finished_at
        .as_ref()?
        .0
        .duration_since(run.started_at.as_ref()?.0);
    if elapsed < SignedDuration::ZERO {
        return None;
    }
    Some(if elapsed.as_secs() < 1 {
        "under a second".to_string()
    } else if elapsed.as_mins() < 1 {
        format!("{}s", elapsed.as_secs())
    } else if elapsed.as_hours() < 1 {
        format!("{} min", elapsed.as_mins())
    } else if elapsed.as_hours() < 24 {
        counted(elapsed.as_hours(), "hour")
    } else {
        counted(elapsed.as_hours() / 24, "day")
    })
}

// --- SNAPSHOT TYPES START ---
//
// What a rule is allowed to look at, and the single decode that fills it. Reduced structs, not
// wrapped API objects (`docs/architecture.md` § Performance). Every field below names the rule
// that reads it; a rule with no field here cannot be written.
//
// The decode lives here rather than in `k8s.rs` because it is the one place a fixture and a live
// watch event meet. A missing field means `None` or empty — never a panic and never a `Result`
// (invariant 5), which `From` not being able to fail is the mechanical guarantee of.
//
// **This decode deliberately does not strip control characters (invariant 9) or bound lengths;
// both belong to `k8s.rs` at ingest (Phase 5), on the way *into* these impls** — and the fields
// that carry untrusted text are wider than the security gate's "names, messages, annotations,
// log lines": also `ownerReferences[].kind` and `.apiVersion`, `metadata.finalizers`, and
// `status.conditions[].message`, which rule 10 renders whole by design (NOTES § D37).

/// One `status.conditions[]` entry, from whichever object carries it.
///
/// Read by rule 10 (`PodScheduled`), N1 (`Ready` plus its `last_transition`), N3 (the
/// three pressure types), W1 (`ReplicaFailure`) and W2 (`Progressing`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Condition {
    pub type_: String,
    /// `"True"`, `"False"` or `"Unknown"` — the API's own tri-state, kept as it arrived.
    pub status: String,
    pub reason: Option<String>,
    /// The controller's own sentence, carried verbatim (NOTES § D37) — rule 10 shows the
    /// scheduler's, W1 the quota's.
    pub message: Option<String>,
    pub last_transition: Option<Time>,
}

// Six condition types across core/v1 and apps/v1, field-for-field identical, with no
// trait in common upstream. The macro is what keeps that mapping written once.
macro_rules! condition_from {
    ($($t:ty),+ $(,)?) => { $(
        impl From<$t> for Condition {
            fn from(c: $t) -> Self {
                Self {
                    type_: c.type_,
                    status: c.status,
                    reason: c.reason,
                    message: c.message,
                    last_transition: c.last_transition_time,
                }
            }
        }
    )+ };
}
condition_from!(
    PodCondition,
    NodeCondition,
    DeploymentCondition,
    ReplicaSetCondition,
    StatefulSetCondition,
    DaemonSetCondition,
);

fn conditions<T: Into<Condition>>(src: Option<Vec<T>>) -> Vec<Condition> {
    src.into_iter().flatten().map(Into::into).collect()
}

/// A `Quantity` is a string upstream (`"64Mi"`, `"500m"`, `"4009164Ki"`) and stays one
/// here: parsing it is judgement, and the rule that needs a number (N5) owns that.
fn quantity(map: &Option<BTreeMap<String, Quantity>>, key: &str) -> Option<String> {
    Some(map.as_ref()?.get(key)?.0.clone())
}

/// What the container was actually **given**, falling back to what it asked for:
/// `status.resources` is what was *enacted*, `spec` is what was *asked for*, and in-place
/// resize makes the two disagree, so a spec-first read names a limit the container was never
/// given (NOTES § D51). `status.allocatedResources` is deliberately not consulted.
///
/// **The fallback is per key, and upstream computes it the same way** — a key present on
/// either side falls through rather than the whole side reading as "nothing was enacted"
/// (NOTES § D53).
///
/// One case is knowingly left wrong: a resize rejected as `Infeasible`, where upstream drops
/// the spec entirely and [`PodSnapshot`] carries no `PodResizePending` to notice
/// (NOTES § D53).
fn effective(
    enacted: Option<&ResourceRequirements>,
    declared: Option<&ResourceRequirements>,
    of: impl Fn(&ResourceRequirements) -> &Option<BTreeMap<String, Quantity>>,
    key: &str,
) -> Option<String> {
    enacted
        .and_then(|r| quantity(of(r), key))
        .or_else(|| declared.and_then(|r| quantity(of(r), key)))
}

/// How a container stopped. Read by rule 2 (`OOMKilled`) and rule 6 (the exit-code table);
/// `finished_at` is when it last died, which is the timestamp rules 1, 2 and 6 show an age
/// from.
///
/// `signal` is deliberately left out — 137 already carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terminated {
    pub reason: Option<String>,
    pub exit_code: i32,
    /// When this run *began*, so a finding can say **how long it lasted** — the first fork of
    /// every crashloop triage, which `kubectl describe` leaves to a human (NOTES § D51).
    pub started_at: Option<Time>,
    pub finished_at: Option<Time>,
    /// The kubelet's own last word on the run, carried verbatim like every other controller
    /// message (NOTES § D37). Usually absent — but under `terminationMessagePolicy:
    /// FallbackToLogsOnError` it holds the tail of the container's log, which turns rule 6's
    /// action from "check the logs" into the log line (NOTES § D51).
    pub message: Option<String>,
}

impl From<ContainerStateTerminated> for Terminated {
    fn from(t: ContainerStateTerminated) -> Self {
        Self {
            reason: t.reason,
            exit_code: t.exit_code,
            started_at: t.started_at,
            finished_at: t.finished_at,
            message: t.message,
        }
    }
}

/// What the container is doing *now* — an enum because upstream sets exactly one of the three,
/// and rule 7 is only distinguishable from rule 1 by which one it is. Three `Option`s would
/// let a rule read a waiting reason off a terminated container (NOTES § D45).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContainerState {
    /// Rules 1, 3, 4: the reason is `CrashLoopBackOff` / `ImagePullBackOff` /
    /// `CreateContainerConfigError`, and the message is the runtime's own sentence.
    Waiting {
        reason: Option<String>,
        message: Option<String>,
    },
    /// Rule 7's state, and `started_at` is **when the current run began** — the other half of
    /// rules 1, 5 and 6's evidence, *"it came back up forty seconds later"*.
    ///
    /// **It is not rule 7's "since when".** A start time says when the process began, not
    /// whether it ever became ready; the only source for *not ready since* is
    /// [`PodSnapshot::ready`]'s `last_transition` (NOTES § D51).
    Running { started_at: Option<Time> },
    /// An init container that failed and is not being retried sits here — `Init:Error`,
    /// which NOTES § D27 lists beside `Init:CrashLoopBackOff`.
    Terminated(Terminated),
}

impl From<ApiContainerState> for ContainerState {
    fn from(s: ApiContainerState) -> Self {
        // Exactly one is set upstream; the order only decides a case the API does not
        // produce, and waiting is the one carrying a reason a rule can name.
        if let Some(w) = s.waiting {
            Self::Waiting {
                reason: w.reason,
                message: w.message,
            }
        } else if let Some(t) = s.terminated {
            Self::Terminated(t.into())
        } else if let Some(r) = s.running {
            Self::Running {
                started_at: r.started_at,
            }
        } else {
            // Not a fourth state: upstream's own doc says an empty state *is* a waiting one
            // with nothing said about why, and rules 1, 3 and 4 match on a named reason, so
            // this fires nothing (NOTES § D45).
            Self::Waiting {
                reason: None,
                message: None,
            }
        }
    }
}

/// What kind of container this is — **three states, not a boolean**, because a native sidecar
/// is an init container that never finishes and the two arithmetics are opposite: in the
/// scheduler's effective pod request a [`Sidecar`](ContainerRole::Sidecar) is **additive** and
/// an [`Init`](ContainerRole::Init) is not (NOTES § D46, § D51). It is also invariant 14 —
/// "the init container `istio-proxy` is crashlooping" is wrong, not merely unclear.
///
/// **That formula is an approximation, deliberately.** Upstream walks the init list *in
/// order*, carrying the sidecar total forward; the order-free version is the only implementable
/// one here, because [`PodSnapshot::containers`] promises no order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerRole {
    /// `spec.containers[]` — the workload itself.
    Regular,
    /// `spec.initContainers[]`, runs to completion before the regular containers start.
    Init,
    /// `spec.initContainers[]` with `restartPolicy: Always` — the native sidecar, GA since 1.29
    /// and how Istio, Linkerd and the Vault agent run. It is charged like a regular container
    /// and described like one.
    Sidecar,
}

/// One container of a pod, init and regular in the same list.
///
/// **One list, not two.** Rules 1–6 read `initContainerStatuses` as well as `containerStatuses`
/// (NOTES § D27, § D75): two fields would let a rule iterate one and forget the other, and the
/// role is what lets the finding say *which* container — the whole diagnosis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerSnapshot {
    pub name: String,
    /// `status.image` — what the runtime actually resolved. Rule 3's action needs the name, and
    /// it otherwise reaches the user only inside the runtime's own sentence, which containerd
    /// and CRI-O word differently (NOTES § D46).
    pub image: String,
    /// Which of the three this is (NOTES § D27 for why both arrays are read at all).
    pub role: ContainerRole,
    /// Rule 7: running but not passing its readiness probe, so the Service dropped it.
    pub ready: bool,
    /// `status.started` — true once the container has passed its **startup probe** and run its
    /// `postStart` hook; a null value is treated the same as false (upstream).
    ///
    /// **A boot signal only where a `startupProbe` is declared, which most workloads do not
    /// do** — and no container in any committed fixture declares one. **Rule 7's "since when"
    /// is [`PodSnapshot::ready`]'s `last_transition`, never this field** (NOTES § D51); read
    /// the other way round, as a *suppressor*, it says something the trigger reading cannot,
    /// which is rule 7's own note (NOTES § D71).
    pub started: bool,
    /// Rule 5, thresholds ≥3 and ≥10.
    pub restarts: i32,
    pub state: ContainerState,
    /// `lastState.terminated` — how the *previous* run ended. Rules 2 and 6.
    pub last_terminated: Option<Terminated>,
    /// N5 sums these per node against the node's allocatable — unless the pod declares
    /// [`PodSnapshot::cpu_request`] / [`PodSnapshot::memory_request`], which replace the sum
    /// rather than adding to it. All three read **what the kubelet enacted first and the spec
    /// second** ([`effective`]).
    pub cpu_request: Option<String>,
    pub memory_request: Option<String>,
    /// Rule 2's evidence: "exceeded its 64Mi limit" — the limit it was actually running
    /// under, never the one a pending resize asked for ([`effective`]).
    pub memory_limit: Option<String>,
}

/// A hostPath volume as one container actually mounts it. Rule 8 decides which of these is bad
/// and the Phase 4 posture report lists the rest, so what is stored is the fact, not the
/// verdict (NOTES § D46).
///
/// `read_only` belongs to the *mount*, not the volume: the same hostPath can be mounted
/// read-only by one container and writable by another, and only the second is rule 8's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPathMount {
    /// The volume's `hostPath.path` — where on the node it starts.
    pub path: String,
    /// The mount's `subPath` — what of it the container actually gets. **Rule 8 reads `path`
    /// joined with this, never `path` alone**, or its docker.sock escalator never sees the
    /// socket it is looking for (NOTES § D46). `None` is the whole path.
    pub sub_path: Option<String>,
    /// The mount's `subPathExpr` — the same narrowing, written with environment variables in it
    /// (`$(POD_NAME)`), and **carried deliberately unresolved**: the values sit in
    /// `spec.containers[].env` and, through `valueFrom`, in objects k8rs does not read and the
    /// security gate does not let it read (NOTES § D71).
    ///
    /// So the only fact it carries is the one that matters — **something narrows this mount and
    /// we cannot say what** — and [`mounted_path`] joins it like a `subPath`, which drops the
    /// `/` escalator rather than announcing the node's whole filesystem. The mount can still be
    /// reported writable. The cost is a miss the other way, uncloseable without the env values.
    ///
    /// Upstream forbids `subPath` and `subPathExpr` on the same mount, so at most one is set.
    pub sub_path_expr: Option<String>,
    pub read_only: bool,
    /// Which container mounts it. Without it the finding cannot say *who* has the node's root,
    /// and two containers mounting one volume produce two entries the rule cannot tell apart
    /// (NOTES § D46).
    pub container: String,
}

/// What the pod will put up with — N6 answers *which* taint is blocking it, and it can only say
/// "untolerated" by holding these. `tolerationSeconds` is left out: it times an eviction after
/// the fact, it does not decide whether the pod can be scheduled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toleration {
    pub key: Option<String>,
    pub operator: Option<String>,
    pub value: Option<String>,
    pub effect: Option<String>,
}

/// One pod, reduced to what rules 1–8, 10 and 12–14 read, plus the pod half of the N5 and
/// N6 joins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodSnapshot {
    pub id: ObjectId,
    /// The card this pod's findings file under (NOTES § D3) — itself when nothing controls it,
    /// and itself when a Node does (NOTES § D39). Phase 5 resolves the ReplicaSet named here up
    /// to its Deployment; this layer records what the object said.
    pub owner: ObjectId,
    /// A static pod — the kubelet runs it off a file on the node and mirrors it into the API.
    /// **The bit is kept even though the Node identity behind it is discarded** (NOTES § D39):
    /// N2 counts only the pods a drain would actually move, and a drain never evicts a mirror
    /// or a DaemonSet pod, so without it N2 fires on every node that *was* drained properly
    /// (NOTES § D46). Rule 8's node-agent exemption reads it too.
    ///
    /// **Sourced from the `ownerReference` of kind `Node`, not from the
    /// `kubernetes.io/config.mirror` annotation**, which the fixture sanitizer strips — an
    /// annotation-sourced bit would decode `false` in every capture (NOTES § D46).
    pub mirror: bool,
    /// `spec.nodeName` — the join N5 and N6 are, and empty while the pod is unscheduled.
    pub node: Option<String>,
    /// Rule 7 is about a pod that is *Running*, and N5 cannot sum without it: a `Succeeded` Job
    /// pod keeps its `nodeName` for as long as nobody collects it, and its requests are charged
    /// to nobody.
    pub phase: Option<String>,
    /// **Driven by `status`, not by `spec`:** every container rule reads a status field, so a
    /// container the kubelet has not reported on cannot produce a finding, and inventing one
    /// would hand rule 7 a `ready: false` for every container that has not started yet. The
    /// cost is that an unscheduled pod contributes no requests, which is right for N5.
    ///
    /// `ephemeralContainerStatuses` is left out: a container someone attached with
    /// `kubectl debug` is not a workload (NOTES § D46).
    ///
    /// **The order is not a contract** — nothing may read this list by index or assume the init
    /// ones come first: find a container by name, and order a screen by
    /// [`ContainerSnapshot::role`], which deliberately has no `Ord` (NOTES § D46).
    pub containers: Vec<ContainerSnapshot>,
    /// **The pod's own request** (`spec.resources.requests`, KEP-2837), and **when it is set it
    /// replaces the container sum for N5, it does not add to it**: a pod declaring only
    /// `spec.resources.requests` decodes with all-`None` containers, so an N5 that sums
    /// containers reports the node healthy while four committed CPUs sit invisible
    /// (NOTES § D51).
    ///
    /// Pod-level *limits* are not carried, and that is **a known gap, not a clean boundary** —
    /// under the same KEP the limit that killed a container can sit on the pod while the
    /// container declares none, and rule 2 would then say "exceeded its memory limit" with no
    /// figure. The field can wait for Phase 4 under NOTES § D42.
    pub cpu_request: Option<String>,
    pub memory_request: Option<String>,
    /// `metadata.creationTimestamp` — **rule 14's clock, and the only age of an object any v1
    /// rule reads** (NOTES § D74). Rule 14 is about an event that never happened, so the only
    /// moment it can measure from is when the pod arrived and the waiting started.
    ///
    /// **`None` fires nothing**: the two minutes *are* the gate. The API server sets it on every
    /// accepted create, so the producer that matters is a prune that drops it — which is why
    /// this field is named in the fields `k8s.rs` must keep (invariant 6).
    pub creation_timestamp: Option<Time>,
    /// `conditions[PodScheduled]` — rule 10's whole input (NOTES § D27). **Its absence is rule
    /// 14's whole input**, which is why that rule cannot be a branch of rule 10: the two are
    /// mutually exclusive by construction (NOTES § D74).
    pub scheduled: Option<Condition>,
    /// `status.nominatedNodeName` — **the field that makes rule 10's verdict false**, and the
    /// reason it is on this struct rather than left out as one nobody reads. When preemption
    /// picks a node, kube-scheduler writes it in the *same* status patch that sets
    /// `PodScheduled: False / Unschedulable`, and the pair stays that way for the whole
    /// graceful termination of the victims (NOTES § D73). Rule 10 stays silent on it; the card
    /// that would describe it is a new rule and was refused (NOTES § D74, invariant 13).
    ///
    /// **Written by the scheduler today, and nothing here assumes it stays that way** — this
    /// layer records what the object said, and the rule reads only whether a machine has been
    /// named, never who named it.
    pub nominated_node_name: Option<String>,
    /// `conditions[Ready]`, kept whole beside `scheduled` for its `last_transition`. **It is the
    /// only source of "not ready since" there is** — no container status carries such a field
    /// anywhere, and rule 7 without a since-when also describes every rolling update, node
    /// reboot and scale-up, on the one screen whose promise is *only what is broken*
    /// (NOTES § D46, § D51).
    ///
    /// `None` for a pod the kubelet has not reached — `pending.json` carries `PodScheduled` and
    /// nothing else.
    pub ready: Option<Condition>,
    /// `conditions[PodReadyToStartContainers]` — **rule 13's evidence line, and never its
    /// gate**. KEP-3085's renamed `PodHasNetwork`: `True` once the kubelet has created the pod's
    /// sandbox *and* configured its network. Volume work happens *before* the sandbox, so
    /// `False` covers storage as much as network and a rule gated on it would be silent for most
    /// of its own class — [`placed_but_never_started`] gates on the residual and reads this only
    /// to say which side of the sandbox the block is on (NOTES § D72, § D76).
    ///
    /// **`None` is not a third case, it is the second one.** The condition is written only once
    /// the kubelet has looked at the pod, and it did not exist before 1.28; an old server and a
    /// silent kubelet both read as "not `False`".
    pub ready_to_start_containers: Option<Condition>,
    /// Rule 12. **Not the moment the delete was accepted: it is request time plus the grace
    /// period**, so the pod is overdue once `now` passes this field itself, and a rule reading
    /// it as the request time doubles its own threshold and reports an age one grace period
    /// short, forever (NOTES § D46). Cleared never — the pod object goes away instead.
    ///
    /// The subtraction is always the *metadata* grace, never the spec fallback below: the API
    /// server writes both fields in the same accepted delete.
    pub deletion_timestamp: Option<Time>,
    /// Rule 12's threshold, and it is the pod's own, never a constant. Reads
    /// `metadata.deletionGracePeriodSeconds` first — the grace this *delete* was granted
    /// — and falls back to `spec.terminationGracePeriodSeconds`, which is what the pod
    /// asked for. They differ exactly when someone passed `--grace-period`, and using
    /// the spec value there would keep a force-deleted pod quiet for 30 seconds it was
    /// never given.
    pub grace_period_seconds: Option<i64>,
    /// `metadata.finalizers` — who still has to sign off before the object can go. Rule 12's two
    /// causes have completely different actions, so without the list the finding is a coin flip;
    /// `kubectl describe pod` does not print finalizers at all, which makes this one of the few
    /// places k8rs says strictly *more* than describe (NOTES § D46).
    pub finalizers: Vec<String>,
    /// Rule 8.
    pub host_path_mounts: Vec<HostPathMount>,
    /// N6, the pod side. **`spec.affinity` is deliberately not here** — NOTES § Node rules names
    /// `nodeSelector`, and node affinity is a term tree that no v1 rule reads. N6 explains a
    /// `nodeSelector` and stays silent about affinity rather than guessing.
    pub node_selector: BTreeMap<String, String>,
    /// N6, the other half of "which taint is blocking it".
    pub tolerations: Vec<Toleration>,
}

/// A node taint, N6's other half.
///
/// **`added_at` is `Option` because of *who wrote the taint*, not which effect it carries**: the
/// node lifecycle controller stamps `timeAdded` on every taint it adds, and `kubectl taint` is
/// client-side and stamps none — `nodes.json` carries both halves (NOTES § D65).
///
/// **What it dates is the taint, not the cordon.** Anything that rewrites `node.spec.taints`
/// wholesale makes the controller re-stamp it while `spec.unschedulable` never moved, so the
/// stamp is a *floor*: a card may say *"cordoned about 2 hours ago"* and may not build an
/// argument on it (NOTES § D69).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taint {
    pub key: String,
    pub value: Option<String>,
    pub effect: String,
    pub added_at: Option<Time>,
}

/// One node, reduced to what N1–N6 read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeSnapshot {
    /// Cluster-scoped, so `namespace` is `None`. N1–N3 file their findings under it in both
    /// roles — `owner == object` (NOTES § D39), which is why there is no separate owner field.
    pub id: ObjectId,
    /// N1 reads `Ready` and how long ago it changed; N3 reads DiskPressure,
    /// MemoryPressure and PIDPressure.
    pub conditions: Vec<Condition>,
    /// N2: cordoned, and possibly forgotten.
    pub unschedulable: bool,
    /// N6.
    pub taints: Vec<Taint>,
    /// N6 matches a pod's `nodeSelector` against these.
    pub labels: BTreeMap<String, String>,
    /// N4, against the control plane's version in [`ClusterSnapshot::server_version`].
    pub kubelet_version: Option<String>,
    /// N5: what the sum of pod requests is measured against.
    pub allocatable_cpu: Option<String>,
    pub allocatable_memory: Option<String>,
}

/// One Deployment, StatefulSet, DaemonSet or ReplicaSet — the objects that know a pod was
/// *supposed* to exist. **The blind spot this closes:** when the pods were never created there
/// is nothing for a pod rule to iterate, and k8rs reported a healthy cluster (NOTES § D28).
///
/// **Four kinds decode into one type** because all four produce the same three facts — desired,
/// ready, conditions — so a second type would carry no extra field, and a missing decode would
/// mean `k8s.rs` reaching back into this file after it freezes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadSnapshot {
    pub id: ObjectId,
    /// A ReplicaSet's Deployment, so W1's finding files under the name the user deployed
    /// rather than under a hashed one. Itself when nothing controls it.
    pub owner: ObjectId,
    /// **How many the controller was told to run** — the top half of the shortfall W2 measures.
    /// `spec.replicas` for a Deployment, StatefulSet or ReplicaSet; a DaemonSet has no such
    /// field and answers with `status.desiredNumberScheduled`, which is always `Some`.
    ///
    /// **`None` is not zero here** — the opposite of [`ready`](WorkloadSnapshot::ready) below.
    /// The API server defaults an absent `spec.replicas` to **1**, never to 0, so
    /// `desired.unwrap_or(0)` says the workload wants nothing where the API says it wants one:
    /// the two `Option`s cannot share a habit (NOTES § D53).
    pub desired: Option<i32>,
    /// **How many of them are passing their probes — and `None` means zero, not "unknown".**
    /// `readyReplicas` carries `omitempty`, so the API server omits it *exactly* when it is 0 —
    /// the state W1 and W2 exist for — while a DaemonSet's required `numberReady` decodes
    /// `Some(0)` for the same fact. So this reads as `ready.unwrap_or(0)`, and a W2 written
    /// `if let (Some(d), Some(r))` goes silent on **total** outage (NOTES § D28, § D53).
    pub ready: Option<i32>,
    /// W1: `ReplicaFailure`, message verbatim. W2: `Progressing` with reason
    /// `ProgressDeadlineExceeded` — which fires only when the two counters above show a
    /// shortfall and no pod-level finding already explains it.
    pub conditions: Vec<Condition>,
}

/// Everything a rule may read, at one instant.
///
/// Assembled by `k8s.rs` from the watch streams (Phase 5), never decoded from a single API
/// object — there is none. **Deliberately no `Default`, and the type now enforces that**: `Time`
/// has no upstream `Default`, so a hand-written one would have to invent a moment — the epoch,
/// handed to every rule as the current time, which is the failure invariant 5 prevents.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterSnapshot {
    /// **What time it is — the one clock a rule may read, and it reads it as a field**, because
    /// `analyze(&Snapshot) -> Vec<Finding>` is pure and a clock call is the impurity that hides
    /// (invariant 5, NOTES § D18). **Captured once per analysis pass**, by `k8s.rs` (Phase 5),
    /// never once per rule. **`Time`, not a bare `jiff::Timestamp`**, and **not an `Option`**
    /// (NOTES § D54).
    ///
    /// **The arithmetic gets none of that, and it has three traps** (NOTES § D54, § D56). Every
    /// *duration* site needs `.0` on both sides. `a - b` on two timestamps yields a
    /// **seconds-only `Span`** whose `.get_minutes()` is `0` over a 43-minute gap; the call that
    /// behaves is `Timestamp::duration_since`. And taking a grace period back off a deadline is
    /// `checked_sub`, never `-`, or anyone with `create` and `delete` on pods can panic a
    /// function invariant 5 says cannot fail.
    ///
    /// **Clock skew is real, and its two halves are not symmetric**; neither is clamped here
    /// (NOTES § D55).
    pub now: Time,
    pub pods: Vec<PodSnapshot>,
    pub nodes: Vec<NodeSnapshot>,
    pub workloads: Vec<WorkloadSnapshot>,
    /// The control plane's version, for N4's skew comparison. `k8s.rs` reads it with
    /// `apiserver_version`; `None` means it could not be read, and N4 says so instead of
    /// comparing against a guess.
    pub server_version: Option<String>,
    /// **Rule C1's first input, and the reason a kubeconfig is anywhere near this struct.** C1's
    /// input has to arrive here like every other rule's, because a second entry point taking PEM
    /// bytes would be an amendment to invariant 5 — a stop, not a convenience (NOTES § D51).
    ///
    /// The kubeconfig **context name** is what the user calls this cluster, and it is C1's
    /// `ObjectId` name. `None` when the kubeconfig names no current context.
    pub context: Option<String>,
    /// The kubeconfig's client **certificate**, PEM bytes as they sit on disk. "Your access to
    /// this cluster expires in 24 days" is a thing only k8rs tells the user — no `kubectl`
    /// command shows it, which is why C1's `kubectl_cmd` is `None`.
    ///
    /// **The certificate and nothing else off the kubeconfig** — never the private key, never a
    /// token, never an exec plugin's output: a key or a token copied into our own types is one
    /// `Debug` away from a backtrace (invariant 8, NOTES § D51). `None` whenever the user
    /// authenticates any other way, and C1 says nothing rather than guessing.
    pub client_certificate: Option<Vec<u8>>,
    /// **How much of the cluster [`pods`](ClusterSnapshot::pods) covers.** `None` = every
    /// namespace; `Some(ns)` = that one only. Set by `--namespace` **and** by the 403 fallback,
    /// because to a rule the two are the same fact.
    ///
    /// N2 and N5 both join every pod on a node, so both are disabled under a namespace scope and
    /// say so rather than computing a partial answer. Without this field a small cluster and a
    /// namespace-scoped view of a big one decode identically — a **silent miss**
    /// (NOTES § D43, § D46).
    pub namespace_scope: Option<String>,
}

fn object_id(kind: ObjectKind, meta: &ObjectMeta) -> ObjectId {
    ObjectId {
        kind,
        namespace: meta.namespace.clone(),
        // An object without a name cannot exist in the API — only in a create request,
        // which this tool never reads back. Empty is the answer that does not panic.
        name: meta.name.clone().unwrap_or_default(),
        uid: meta.uid.clone(),
    }
}

/// The controller that owns this object, or the object itself when there is none — and whether
/// the controller that was discarded was a **Node**, which is what makes a pod a mirror pod
/// ([`PodSnapshot::mirror`]).
///
/// One traversal returns both, so the bit and the discarded reference cannot come to disagree
/// (NOTES § D46). An `ownerReference` carries no namespace, so the object's own is used.
fn owner_of(meta: &ObjectMeta, own: &ObjectId) -> (ObjectId, bool) {
    let controller = meta
        .owner_references
        .iter()
        .flatten()
        .find(|o| o.controller == Some(true));
    // Only a *controlling* reference decides anything (NOTES § D46). `find` is the whole search
    // because there is at most one to find: `ValidateOwnerReferences` rejects a second.
    let Some(o) = controller else {
        return (own.clone(), false);
    };
    // The decision reads off the *resolved* kind rather than off the string: `Node` in
    // somebody's CRD group is an ordinary owner, not the kubelet (NOTES § D51).
    let kind = ObjectKind::from_api(&o.api_version, &o.kind);
    // A Node owner is discarded and the object files under itself (NOTES § D39).
    if kind == ObjectKind::Node {
        return (own.clone(), true);
    }
    (
        ObjectId {
            kind,
            namespace: own.namespace.clone(),
            name: o.name.clone(),
            uid: Some(o.uid.clone()),
        },
        false,
    )
}

impl ObjectKind {
    /// The `kind` string **read together with its `apiVersion`**, because a kind string on its
    /// own does not name a kind: OpenKruise's Advanced StatefulSet is
    /// `apps.kruise.io/v1beta1, Kind: StatefulSet`, and matched on the kind alone it becomes the
    /// built-in variant — which points Phase 7's `scale` at a different object (NOTES § D51).
    ///
    /// **The group decides, not the whole `apiVersion`.** Anything this project has no branch
    /// for stays as qualified text; inventing a variant would be per-kind code (invariant 12).
    ///
    /// **Both arguments are unvalidated free text when they come off an `ownerReference`**, and
    /// the `Other` arms carry them into a string that reaches a card — so Phase 5's ingest strip
    /// has to cover `ownerReferences[].kind` and `.apiVersion` as well as the names
    /// (invariant 9).
    fn from_api(api_version: &str, kind: &str) -> Self {
        // `apps/v1` -> `apps`; the core group is written `v1`, with no group at all.
        let group = api_version.split_once('/').map_or("", |(g, _)| g);
        match (group, kind) {
            ("apps", "Deployment") => Self::Deployment,
            ("apps", "StatefulSet") => Self::StatefulSet,
            ("apps", "DaemonSet") => Self::DaemonSet,
            ("apps", "ReplicaSet") => Self::ReplicaSet,
            ("batch", "Job") => Self::Job,
            ("batch", "CronJob") => Self::CronJob,
            ("", "Node") => Self::Node,
            ("", "Pod") => Self::Pod,
            ("", _) => Self::Other(kind.to_string()),
            _ => Self::Other(format!("{kind}.{group}")),
        }
    }
}

fn container_snapshots(
    spec: &PodSpec,
    init: Option<Vec<ContainerStatus>>,
    main: Option<Vec<ContainerStatus>>,
) -> Vec<ContainerSnapshot> {
    let init = init.into_iter().flatten().map(|s| (true, s));
    let main = main.into_iter().flatten().map(|s| (false, s));
    init.chain(main)
        .map(|(is_init, s)| {
            // Container names are unique across both arrays — Kubernetes enforces it — so one
            // scan finds the declaration this status belongs to, and a scan beats a map per pod
            // for a handful of containers.
            //
            // **The miss has no test because the API cannot produce the object**: both container
            // lists are immutable after create, and the one list that grows is
            // `ephemeralContainers`, whose statuses are deliberately not read (NOTES § D46).
            let declared = spec
                .init_containers
                .iter()
                .flatten()
                .chain(spec.containers.iter())
                .find(|c| c.name == s.name);
            let requested = declared.and_then(|c| c.resources.as_ref());
            // What the node actually enacted, which is not always what the spec asks for.
            let enacted = s.resources;
            // `restartPolicy: Always` on an *init* container is the native sidecar. The regular
            // list is not asked because a regular container answers `Regular` either way, which
            // is a statement about our own behaviour rather than a bet on upstream's
            // (NOTES § D46).
            let restartable = declared.and_then(|c| c.restart_policy.as_deref()) == Some("Always");
            let role = match (is_init, restartable) {
                (true, true) => ContainerRole::Sidecar,
                (true, false) => ContainerRole::Init,
                (false, _) => ContainerRole::Regular,
            };
            ContainerSnapshot {
                name: s.name,
                image: s.image,
                role,
                ready: s.ready,
                // Upstream: "The null value must be treated the same as false."
                started: s.started.unwrap_or(false),
                restarts: s.restart_count,
                // A status with no `state` at all takes the same road as one whose state is set
                // but empty: `unwrap_or_default` hands the `From` impl the waiting the API says
                // that means. One construction of that case, not two (NOTES § D45).
                state: ContainerState::from(s.state.unwrap_or_default()),
                last_terminated: s
                    .last_state
                    .and_then(|l| l.terminated)
                    .map(Terminated::from),
                cpu_request: effective(enacted.as_ref(), requested, |r| &r.requests, "cpu"),
                memory_request: effective(enacted.as_ref(), requested, |r| &r.requests, "memory"),
                memory_limit: effective(enacted.as_ref(), requested, |r| &r.limits, "memory"),
            }
        })
        .collect()
}

fn host_path_mounts(spec: &PodSpec) -> Vec<HostPathMount> {
    let mut mounts = Vec::new();
    for volume in spec.volumes.iter().flatten() {
        let Some(host_path) = &volume.host_path else {
            continue;
        };
        // A hostPath volume no container mounts exposes nothing, so it is not a mount.
        // `spec.ephemeralContainers` is not walked, for the same reason their statuses are not
        // read (see `PodSnapshot::containers`).
        for container in spec
            .init_containers
            .iter()
            .flatten()
            .chain(spec.containers.iter())
        {
            for mount in container
                .volume_mounts
                .iter()
                .flatten()
                .filter(|m| m.name == volume.name)
            {
                mounts.push(HostPathMount {
                    path: host_path.path.clone(),
                    sub_path: mount.sub_path.clone(),
                    sub_path_expr: mount.sub_path_expr.clone(),
                    read_only: mount.read_only.unwrap_or(false),
                    container: container.name.clone(),
                });
            }
        }
    }
    mounts
}

impl From<Pod> for PodSnapshot {
    fn from(pod: Pod) -> Self {
        let Pod {
            metadata,
            spec,
            status,
        } = pod;
        let id = object_id(ObjectKind::Pod, &metadata);
        let (owner, mirror) = owner_of(&metadata, &id);
        let spec = spec.unwrap_or_default();
        let status = status.unwrap_or_default();

        let containers = container_snapshots(
            &spec,
            status.init_container_statuses,
            status.container_statuses,
        );
        let host_path_mounts = host_path_mounts(&spec);
        let pod_resources = spec.resources.as_ref();
        let cpu_request = pod_resources.and_then(|r| quantity(&r.requests, "cpu"));
        let memory_request = pod_resources.and_then(|r| quantity(&r.requests, "memory"));
        // All three are picked by name off the same array. A pod carries five conditions
        // and `PodScheduled` is the last of them, so none can be "the first one".
        let conditions = status.conditions.unwrap_or_default();
        let condition = |type_: &str| {
            conditions
                .iter()
                .find(|c| c.type_ == type_)
                .cloned()
                .map(Condition::from)
        };

        Self {
            id,
            owner,
            mirror,
            node: spec.node_name,
            phase: status.phase,
            containers,
            cpu_request,
            memory_request,
            creation_timestamp: metadata.creation_timestamp,
            scheduled: condition("PodScheduled"),
            nominated_node_name: status.nominated_node_name,
            ready: condition("Ready"),
            ready_to_start_containers: condition("PodReadyToStartContainers"),
            deletion_timestamp: metadata.deletion_timestamp,
            grace_period_seconds: metadata
                .deletion_grace_period_seconds
                .or(spec.termination_grace_period_seconds),
            finalizers: metadata.finalizers.unwrap_or_default(),
            host_path_mounts,
            node_selector: spec.node_selector.unwrap_or_default(),
            tolerations: spec
                .tolerations
                .into_iter()
                .flatten()
                .map(|t| Toleration {
                    key: t.key,
                    operator: t.operator,
                    value: t.value,
                    effect: t.effect,
                })
                .collect(),
        }
    }
}

impl From<Node> for NodeSnapshot {
    fn from(node: Node) -> Self {
        let Node {
            metadata,
            spec,
            status,
        } = node;
        let id = object_id(ObjectKind::Node, &metadata);
        let spec = spec.unwrap_or_default();
        let status = status.unwrap_or_default();
        Self {
            id,
            conditions: conditions(status.conditions),
            unschedulable: spec.unschedulable.unwrap_or(false),
            taints: spec
                .taints
                .into_iter()
                .flatten()
                .map(|t| Taint {
                    key: t.key,
                    value: t.value,
                    effect: t.effect,
                    added_at: t.time_added,
                })
                .collect(),
            labels: metadata.labels.unwrap_or_default(),
            kubelet_version: status.node_info.map(|i| i.kubelet_version),
            allocatable_cpu: quantity(&status.allocatable, "cpu"),
            allocatable_memory: quantity(&status.allocatable, "memory"),
        }
    }
}

fn workload(
    kind: ObjectKind,
    metadata: ObjectMeta,
    desired: Option<i32>,
    ready: Option<i32>,
    conditions: Vec<Condition>,
) -> WorkloadSnapshot {
    let id = object_id(kind, &metadata);
    // A Node does not control a workload, so the mirror bit has nothing to say here.
    let (owner, _mirror) = owner_of(&metadata, &id);
    WorkloadSnapshot {
        id,
        owner,
        desired,
        ready,
        conditions,
    }
}

impl From<Deployment> for WorkloadSnapshot {
    fn from(d: Deployment) -> Self {
        let status = d.status.unwrap_or_default();
        workload(
            ObjectKind::Deployment,
            d.metadata,
            d.spec.and_then(|s| s.replicas),
            status.ready_replicas,
            conditions(status.conditions),
        )
    }
}

/// **No test covers this impl, and none can yet**: `tests/fixtures/statefulsets.json` is an
/// empty list, and synthesizing a whole StatefulSet would be the hand-written JSON CLAUDE.md
/// forbids. The impl stays because `k8s.rs` watches the kind (NOTES § D28) and this file freezes
/// at the end of Phase 3; the open Phase 2 capture trip owns closing the gap (NOTES § D40).
impl From<StatefulSet> for WorkloadSnapshot {
    fn from(s: StatefulSet) -> Self {
        let status = s.status.unwrap_or_default();
        workload(
            ObjectKind::StatefulSet,
            s.metadata,
            s.spec.and_then(|s| s.replicas),
            status.ready_replicas,
            conditions(status.conditions),
        )
    }
}

impl From<ReplicaSet> for WorkloadSnapshot {
    fn from(r: ReplicaSet) -> Self {
        let status = r.status.unwrap_or_default();
        workload(
            ObjectKind::ReplicaSet,
            r.metadata,
            r.spec.and_then(|s| s.replicas),
            status.ready_replicas,
            conditions(status.conditions),
        )
    }
}

impl From<DaemonSet> for WorkloadSnapshot {
    fn from(d: DaemonSet) -> Self {
        let status = d.status.unwrap_or_default();
        // A DaemonSet has no `spec.replicas`: how many it wants is however many nodes it
        // matches, which only the controller knows, so both numbers come from `status`.
        workload(
            ObjectKind::DaemonSet,
            d.metadata,
            Some(status.desired_number_scheduled),
            Some(status.number_ready),
            conditions(status.conditions),
        )
    }
}

// --- SNAPSHOT TYPES END ---

// --- THE POD RULES START ---
//
// One function per rule of NOTES § v1 rule set, each one pure and each one returning what it
// found rather than reporting how it failed: a missing field is `None` and no finding, never a
// `Result` (invariant 5). The clock arrives as [`ClusterSnapshot::now`].
//
// **What is in here is D2's line — broken *now*.** Rule 9 and the plain read-only hostPath
// belong to the Analysis reports; rule 11 needs an Events watch this project does not open.
//
// **Every string below is written for someone in their first month** (invariant 14): the jargon
// is explained in a sentence and then named in brackets.

/// The evidence line's separator, spelled once — `screens/alerts.md` draws
/// `limit 256Mi · exit 137 · 47 restarts`, and two rules picking different glue is a
/// screen that looks assembled from two products.
const FACTS: &str = " · ";

/// Rule 5's two bands (REQUIREMENTS: restarts ≥3 warn, ≥10 critical).
const RESTARTS_WARN: i32 = 3;
const RESTARTS_CRITICAL: i32 = 10;

/// **How long something may be misbehaving before it counts as a failure** — ten minutes, and
/// the number is borrowed rather than tuned: it is `progressDeadlineSeconds`' default,
/// Kubernetes' own answer to *"how long may a pod take to become ready before that counts as a
/// failure"* (NOTES § D46, § D51, § D72).
///
/// **Read by rules 2, 7, 10 and 13** — one threshold for one question, so changing it moves all
/// four.
const NOT_READY_GRACE: SignedDuration = SignedDuration::from_mins(10);

/// **How long a pod may sit with nothing having judged it at all** — two minutes, anchored at
/// eight times kube-scheduler's leader-election failover rather than picked (NOTES § D74).
///
/// **Deliberately not [`NOT_READY_GRACE`]**, which answers *how long may something take to
/// become ready* — a question about work in progress. Nothing is in progress here: what is being
/// waited on is a handover between schedulers, and that has its own default to borrow.
const NEVER_JUDGED_GRACE: SignedDuration = SignedDuration::from_mins(2);

/// **The margin on rule 12's deadline** — sixty seconds, flat, covering kubelet observation,
/// watch latency and ordinary skew, none of which scales with a grace the deadline already
/// spent. It costs nothing: a pod actually held by a finalizer is held for minutes or forever
/// (NOTES § D55, § D71).
const OVERDUE_MARGIN: SignedDuration = SignedDuration::from_secs(60);

/// The namespace whose CNI, kube-proxy and control-plane pods mount the node on purpose —
/// see [`escalated_host_path`].
const NODE_NAMESPACE: &str = "kube-system";

/// **Every control socket that is the machine.** A process that can talk to one of these can
/// start a privileged container on the node, so a **read-only** bind of it is still full root —
/// which is why rule 8 escalates on the path and not on the mode.
///
/// **One spelling each, written under `/run`, and [`is_runtime_socket`] reaches the rest** — the
/// `/var/run/…` name every systemd distribution also has for the same file, and any directory an
/// entry sits under. So a socket added here is matched under both names and through its
/// directories without a second line, which is the part that has to be mechanical: carrying
/// spellings by hand is what left `/run/crio/crio.sock` matching nothing (NOTES § D77, § D78).
/// **Every entry being under `/run` is what makes the fold safe** — it is the one property of
/// this list the function relies on, and the sweep asserts it rather than re-checking it here.
///
/// **The list is not complete and no list of paths can be**: a kubelet's
/// `--container-runtime-endpoint` puts the socket wherever the operator says. These are the
/// defaults a 2026 node ships — Docker (which NOTES § v1 rule set names, in its `/var/run` form),
/// containerd, CRI-O, the containerd k3s and RKE2 embed, and cri-dockerd, which is what a cluster
/// that kept Docker past 1.24 runs.
const RUNTIME_SOCKETS: [&str; 5] = [
    "/run/docker.sock",
    "/run/containerd/containerd.sock",
    "/run/crio/crio.sock",
    "/run/k3s/containerd/containerd.sock",
    "/run/cri-dockerd.sock",
];

/// **Every rule in this file, over one snapshot** — the signature invariant 5 names, and the only
/// entry point `k8s.rs` and the `--once` printer are given.
///
/// Rules 1–8, 10 and 12–14. The N-series, W-series and C1 are later boxes of this phase and are
/// deliberately not wired here: a half-built rule is worse than an absent one, because the screen
/// looks complete either way.
///
/// **Rules 1–6 read every container the pod has**, in either status array and whichever of
/// [`ContainerRole`] it is (NOTES § D27, § D75). **Rule 7 is the one exception and reads regular
/// containers only.** **Rules 8 and 10 are not container rules at all** — rule 10 reads a pod
/// condition, which is what lets it fire on a pod that has no containers — and **rule 13 is a
/// third shape**: one card about the *pod*, reached by walking its containers.
///
/// **A pod that finished is not broken now**, so rules 1–8, 10, 13 and 14 skip `Succeeded` and
/// `Failed`, whose restart counts and last exits belong to the **Waste** report (NOTES § D2,
/// § D71). Rule 12 is deliberately outside the skip: a `Succeeded` pod that will not go away is
/// still stuck. **No committed capture is in either state** — todo.md's capture-trip box.
pub fn analyze(snapshot: &ClusterSnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();
    for pod in &snapshot.pods {
        findings.extend(stuck_terminating(&snapshot.now, pod));
        if matches!(pod.phase.as_deref(), Some("Succeeded" | "Failed")) {
            continue;
        }
        findings.extend(escalated_host_path(pod));
        findings.extend(no_node_accepted_it(&snapshot.now, pod));
        findings.extend(placed_but_never_started(&snapshot.now, pod));
        findings.extend(nothing_has_looked_at_it(&snapshot.now, pod));
        for c in &pod.containers {
            findings.extend(crash_looping(pod, c));
            findings.extend(out_of_memory(&snapshot.now, pod, c));
            findings.extend(image_not_pulled(pod, c));
            findings.extend(container_config_missing(pod, c));
            findings.extend(restarting_repeatedly(pod, c));
            findings.extend(previous_run_failed(pod, c));
            findings.extend(running_but_not_ready(&snapshot.now, pod, c));
        }
    }
    findings
}

/// `kubectl describe pod …` — the one command that shows a container's current state, how its
/// last run ended, its restart count, the limits it is running under and its mounts. That is
/// what rules 1–8 claim, checked per card (NOTES § D71).
///
/// **Rule 13 is here for a different reason**: what finishes its diagnosis is an Event, which
/// `describe` prints and `get -o yaml` does not. **Rule 12 does not use it**: `describe` prints
/// no finalizers at all (NOTES § D46), and a teaching command that does not show what the card
/// says is worse than none.
fn describe(id: &ObjectId) -> Option<String> {
    Some(format!(
        "kubectl describe pod {}{}",
        id.name,
        in_namespace(id)
    ))
}

/// `kubectl get pod … -o yaml` — for the three cards whose evidence is a field `describe` does
/// not print at all: rule 12's `metadata.finalizers`, and rules 3 and 4's
/// `state.waiting.message`, which kubectl's `describeStatus` never renders and which is the
/// entire evidence line of both cards (NOTES § D46, § D71). A teaching command that does not
/// show what the card says is worse than none (invariant 4).
fn get_yaml(id: &ObjectId) -> Option<String> {
    Some(format!(
        "kubectl get pod {}{} -o yaml",
        id.name,
        in_namespace(id)
    ))
}

/// ` -n <namespace>`, or nothing at all when there is none. The flag is appended rather than
/// always written because `-n ""` is a command that does not work, printed in the record
/// invariant 4 says may not lie ([`ObjectId::namespace`]).
fn in_namespace(id: &ObjectId) -> String {
    id.namespace
        .as_deref()
        .map_or_else(String::new, |ns| format!(" -n {ns}"))
}

/// The reason and the runtime's own sentence, for a container that is waiting — and `None` for
/// one that is running or has stopped, which is what keeps rules 1, 3 and 4 from reading a
/// waiting reason off a container in another state ([`ContainerState`]).
///
/// Also `None` for a waiting container that has been given no reason, which the decode produces
/// for an empty `state` (NOTES § D45); every caller matches on a named reason, so the two
/// collapse to the same answer.
fn waiting(c: &ContainerSnapshot) -> Option<(&str, Option<&str>)> {
    match &c.state {
        ContainerState::Waiting { reason, message } => {
            Some((reason.as_deref()?, message.as_deref()))
        }
        _ => None,
    }
}

/// **Which container this is, in words that also say what kind of container it is** — the first
/// fact of every card rules 1–6 draw, carried in the evidence rather than in six titles
/// (NOTES § D27, § D75).
///
/// Each role brings its own sentence, and each is a **property of that kind of container, never
/// a claim about this pod** — rules 5 and 6 also reach an init container that finished long ago
/// inside a pod that is serving happily. **A regular container gets no gloss**: it *is* the
/// application, and a clause on every card teaches the reader to skip the line.
fn container_fact(c: &ContainerSnapshot) -> String {
    match c.role {
        ContainerRole::Regular => format!("container {}", c.name),
        ContainerRole::Init => format!(
            "init container {} (the app starts only after this one finishes)",
            c.name
        ),
        ContainerRole::Sidecar => format!(
            "sidecar container {} (it runs beside the app the whole time)",
            c.name
        ),
    }
}

/// **Is this container doing the job it was given, right now?** — the suppressor
/// [`restarting_repeatedly`], [`previous_run_failed`] and [`out_of_memory`] share, and the one
/// place the answer depends on [`ContainerRole`].
///
/// Running and ready for a [`Regular`](ContainerRole::Regular) or a
/// [`Sidecar`](ContainerRole::Sidecar); **`exit 0` for an [`Init`](ContainerRole::Init)**, because
/// "serving" means nothing about a container that runs once and finishes and the other expression
/// answers *no* for every init container that ever succeeded (NOTES § D75). **A failed init
/// container is deliberately not settled by this.**
///
/// **No committed capture reaches the init branch with anything to suppress**; it is exercised on
/// a decoded copy (NOTES § D53), and todo.md's capture-trip box owns closing it.
fn doing_its_job(c: &ContainerSnapshot) -> bool {
    match (&c.state, c.role) {
        (ContainerState::Running { .. }, _) => c.ready,
        (ContainerState::Terminated(run), ContainerRole::Init) => run.exit_code == 0,
        _ => false,
    }
}

/// **What an exit code means, in the words a beginner needs** — NOTES § v1 rule set's
/// translation table, and nothing invented beside it. `None` is a code with no accepted meaning,
/// where the number alone is the honest answer.
///
/// 143 is the one entry that says *nothing is wrong*, which is why [`previous_run_failed`]
/// refuses to fire on it. It stays here because rule 1 does print it.
///
/// **137 needs the `reason` beside it, and NOTES' own table is corrected here** (NOTES § D71):
/// with [`Terminated::reason`] `OOMKilled` the kernel took the container for using too much
/// memory; **without it**, 137 is the kubelet's own SIGKILL after an unanswered SIGTERM — a
/// failing `livenessProbe`, or a shutdown that hangs.
fn exit_meaning(code: i32, reason: Option<&str>) -> Option<&'static str> {
    Some(match code {
        137 if reason == Some("OOMKilled") => {
            "killed by the kernel for using more memory than it was allowed"
        }
        137 => {
            "killed because it did not stop when it was asked to — a failing liveness probe, \
             or a shutdown that hangs"
        }
        143 => "stopped with SIGTERM, which is an ordinary shutdown and not an error",
        1 | 2 => "the application's own error",
        126 => "the command was found but could not be run",
        127 => "the command was not found",
        _ => return None,
    })
}

/// `exit 137 (killed with SIGKILL, …)` — the number first, because that is what the reader will
/// search for, and the sentence in brackets like every other piece of jargon on these cards.
/// Takes the whole [`Terminated`], because [`exit_meaning`] needs the reason that sits beside
/// the code.
fn exit_fact(run: &Terminated) -> String {
    match exit_meaning(run.exit_code, run.reason.as_deref()) {
        Some(meaning) => format!("exit {} ({meaning})", run.exit_code),
        None => format!("exit {}", run.exit_code),
    }
}

/// **The last thing the container actually said**, out of the kubelet's termination message —
/// `None` when it left none, which is the usual case ([`Terminated::message`]).
///
/// **The last non-empty line, not the first.** Under `terminationMessagePolicy:
/// FallbackToLogsOnError` this field is the *tail* of the container's log, so the first line is
/// whatever the process printed on the way up — `tests/fixtures/crashloop.json` starts its with
/// `starting` and ends it with the panic that killed it.
///
/// **One line, and this is where that is decided.** A `Finding`'s fields are one card line each
/// (`screens/widgets.md` § 2). It is not truncation — § 7 forbids k8rs shortening a string
/// itself, and bounding a huge value is `k8s.rs`'s job at ingest.
fn last_log_line(run: &Terminated) -> Option<&str> {
    run.message
        .as_deref()?
        .lines()
        .map(str::trim_end)
        .rfind(|l| !l.is_empty())
}

/// **Rule 1 — the container keeps crashing and Kubernetes has started waiting between
/// restarts.** `state.waiting.reason == CrashLoopBackOff`, CRITICAL: this container is not doing
/// its job right now.
///
/// The age is [`Terminated::finished_at`] on the previous run ([`Finding::timestamp`]).
fn crash_looping(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let (reason, _) = waiting(c)?;
    if reason != "CrashLoopBackOff" {
        return None;
    }
    let mut facts = vec![container_fact(c)];
    if c.restarts > 0 {
        facts.push(format!("{} restarts", c.restarts));
    }
    if let Some(run) = &c.last_terminated {
        if let Some(d) = lasted(run) {
            facts.push(format!("the last run lasted {d}"));
        }
        facts.push(exit_fact(run));
    }
    Some(Finding {
        severity: Severity::Critical,
        title: "Container keeps crashing, and each restart waits longer (CrashLoopBackOff)"
            .to_string(),
        evidence: facts.join(FACTS),
        action: "read the previous run's logs — that is where it says why it exits".to_string(),
        kubectl_cmd: describe(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: c
            .last_terminated
            .as_ref()
            .and_then(|t| t.finished_at.clone()),
    })
}

/// **Rule 2 — the kernel killed the container for using more memory than it was allowed.**
/// `lastState.terminated.reason == OOMKilled`, CRITICAL.
///
/// **The limit named is the one that was enacted**, never the one a pending in-place resize asked
/// for ([`ContainerSnapshot::memory_limit`], NOTES § D51). When no limit is readable the term is
/// left out rather than guessed ([`PodSnapshot::cpu_request`]).
///
/// **Quiet on an old kill the container has been fine since — and on nothing weaker**, because
/// `lastState.terminated` never expires: both halves are required, doing its job **and** the kill
/// older than [`NOT_READY_GRACE`]. **An undated kill is never suppressed** — the exemption has to
/// be *proved*, which also drops a future-dated kill back into the firing branch — and this is
/// **stricter than [`previous_run_failed`]'s suppressor, which needs no clock** (NOTES § D75).
///
/// **No committed capture carries an OOM kill on a serving container** — todo.md's capture-trip
/// box.
fn out_of_memory(now: &Time, pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let run = c.last_terminated.as_ref()?;
    if run.reason.as_deref() != Some("OOMKilled") {
        return None;
    }
    if doing_its_job(c)
        && run
            .finished_at
            .as_ref()
            .is_some_and(|t| now.0.duration_since(t.0) > NOT_READY_GRACE)
    {
        return None;
    }
    let mut facts = vec![container_fact(c)];
    if let Some(limit) = &c.memory_limit {
        facts.push(format!("limit {limit}"));
    }
    facts.push(format!("exit {}", run.exit_code));
    if c.restarts > 0 {
        facts.push(format!("{} restarts", c.restarts));
    }
    Some(Finding {
        severity: Severity::Critical,
        title: "Container used more memory than it was allowed and the kernel killed it \
                (OOMKilled)"
            .to_string(),
        evidence: facts.join(FACTS),
        action: "raise the container's memory limit, or find what is using the memory".to_string(),
        kubectl_cmd: describe(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: run.finished_at.clone(),
    })
}

/// **Every way the kubelet says "this container is not getting its image"** — rule 3's trigger
/// and, through [`stuck_at_the_starting_line`], rule 13's largest exclusion. **One list read by
/// two rules**, so the pair cannot drift because there is no pair.
///
/// The five past the first two are `pkg/kubelet/images/types.go`'s error set and mean one thing
/// to the reader — *this image will never become available* — whatever the cause, and each
/// carries the kubelet's own sentence, which is the diagnosis (NOTES § D76).
const UNUSABLE_IMAGE: [&str; 7] = [
    "ErrImagePull",
    "ImagePullBackOff",
    "InvalidImageName",
    "ErrImageNeverPull",
    "ImageInspectError",
    "RegistryUnavailable",
    "SignatureValidationFailed",
];

/// **Rule 3 — the container cannot get its image, so it never started.**
/// `state.waiting.reason` in [`UNUSABLE_IMAGE`]. CRITICAL.
///
/// **The title does not say "download"** — that word is wrong about `InvalidImageName`, where
/// the name is not a reference, and about `ErrImageNeverPull`, where the policy forbids
/// downloading at all. The reason in brackets and the kubelet's sentence below it tell the
/// reader which of the seven they have (invariant 14).
///
/// The runtime's own sentence is quoted verbatim (NOTES § D37) because it is the only place the
/// actual failure appears; the resolved image name comes from [`ContainerSnapshot::image`]
/// rather than being dug out of that sentence (NOTES § D46).
///
/// No age: nothing in the container status records when the first attempt was made.
fn image_not_pulled(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let (reason, message) = waiting(c)?;
    if !UNUSABLE_IMAGE.contains(&reason) {
        return None;
    }
    let mut facts = vec![container_fact(c), format!("image {}", c.image)];
    facts.extend(message.map(str::to_string));
    Some(Finding {
        severity: Severity::Critical,
        title: format!("Container image is not usable, so the container never started ({reason})"),
        evidence: facts.join(FACTS),
        action: "check the image name and tag, whether this namespace has a pull secret for \
                 that registry, and whether the pull policy lets the node fetch it at all"
            .to_string(),
        kubectl_cmd: get_yaml(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: None,
    })
}

/// **Rule 4 — the container refers to a ConfigMap or Secret that is not there.**
/// `state.waiting.reason == CreateContainerConfigError`, CRITICAL.
///
/// The kubelet's message names the missing object (`configmap "…" not found`), and that name is
/// the whole of what the reader has to go and create or correct, so it is quoted verbatim rather
/// than summarised (NOTES § D37).
fn container_config_missing(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let (reason, message) = waiting(c)?;
    if reason != "CreateContainerConfigError" {
        return None;
    }
    let mut facts = vec![container_fact(c)];
    facts.extend(message.map(str::to_string));
    Some(Finding {
        severity: Severity::Critical,
        title: "Container needs a ConfigMap or Secret that does not exist \
                (CreateContainerConfigError)"
            .to_string(),
        evidence: facts.join(FACTS),
        action: "create the missing object, or correct the name the pod refers to".to_string(),
        kubectl_cmd: get_yaml(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: None,
    })
}

/// **Rule 5 — the container has been restarted enough times that something is wrong even if it
/// looks fine now.** `restartCount`, WARN at [`RESTARTS_WARN`] and CRITICAL at
/// [`RESTARTS_CRITICAL`].
///
/// **Quiet on a container rule 1 is already describing** — one incident, one card — and **quiet on
/// an init container that has already finished successfully**, whose count can never rise again
/// ([`doing_its_job`], NOTES § D71, § D75). **The title changes with the state**, because NOTES'
/// wording would be a lie about a container that has stopped.
///
/// **Severity is WARN whenever the container is serving, whatever the count**: a lifetime counter
/// carries no *rate*, and REQUIREMENTS marks the two numbers *(suggestion)* (NOTES § D71). The age
/// is when the counter last went up.
///
/// **The CRITICAL branch and the `&& !serving` half have no capture behind them** — todo.md's
/// capture-trip box.
fn restarting_repeatedly(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    // An init container that has finished successfully is out of this rule's subject altogether,
    // not merely a milder case of it: its count is frozen for the life of the pod, and every
    // sentence below is about a container something is *still* killing (NOTES § D75).
    if c.role == ContainerRole::Init && doing_its_job(c) {
        return None;
    }
    if c.restarts < RESTARTS_WARN || waiting(c).map(|(r, _)| r) == Some("CrashLoopBackOff") {
        return None;
    }
    // Every container that reaches here is judged by the expression this rule always used, and
    // the one init case that differs returned above — so the title below cannot say "it is
    // serving now" about a container that has stopped.
    let serving = doing_its_job(c);
    Some(Finding {
        severity: if c.restarts >= RESTARTS_CRITICAL && !serving {
            Severity::Critical
        } else {
            Severity::Warn
        },
        title: if serving {
            format!(
                "Container has been restarted {} times — it is serving now, but something \
                 keeps killing it",
                c.restarts
            )
        } else {
            format!("Container has been restarted {} times", c.restarts)
        },
        evidence: [container_fact(c), c.image.clone()].join(FACTS),
        action: "check the memory limit and the liveness probe — those are what restart a \
                 container that otherwise runs"
            .to_string(),
        kubectl_cmd: describe(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: c
            .last_terminated
            .as_ref()
            .and_then(|t| t.finished_at.clone()),
    })
}

/// **Rule 6 — the container's previous run ended badly, and here is what the code means.**
/// `lastState.terminated.exitCode`, WARN: the run that failed is over, and where the container is
/// *currently* broken rules 1 to 4 say so as CRITICAL beside this.
///
/// **Two exits are not findings** — `0` and `143`, every rolling update and every scale-down
/// (NOTES § v1 rule set) — and **`OOMKilled` belongs to rule 2**: one event, one card.
///
/// **And quiet on a container that is serving now, because this field never expires** — the
/// largest false-positive volume in this box, needing no unusual manifest, only uptime
/// (NOTES § D71); that history belongs to [`restarting_repeatedly`], which has a threshold under
/// it. **"Serving" is the wrong word for an init container, and [`doing_its_job`] is where that is
/// decided.**
///
/// **When the kubelet kept the container's last words, they replace the advice**
/// ([`Terminated::message`]). **Neither exit exemption has a capture behind it, and nor do two of
/// the three actions** — todo.md's capture-trip box.
fn previous_run_failed(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let run = c.last_terminated.as_ref()?;
    if run.exit_code == 0
        || run.exit_code == 143
        || run.reason.as_deref() == Some("OOMKilled")
        || doing_its_job(c)
    {
        return None;
    }
    // The kubelet's `reason` for a non-zero exit is the bare word `Error`, which says nothing
    // the title has not already said in a sentence (invariant 14).
    let mut facts = vec![container_fact(c)];
    if let Some(d) = lasted(run) {
        facts.push(format!("ran for {d}"));
    }
    Some(Finding {
        severity: Severity::Warn,
        title: format!("The container's previous run failed — {}", exit_fact(run)),
        evidence: facts.join(FACTS),
        action: match (last_log_line(run), run.exit_code) {
            (Some(line), _) => format!("the last thing it logged was: {line}"),
            (None, 126 | 127) => {
                "check the container's command and arguments — what they name is not in the \
                 image"
                    .to_string()
            }
            (None, _) => {
                "read the logs of that run to find the application's own error".to_string()
            }
        },
        kubectl_cmd: describe(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: run.finished_at.clone(),
    })
}

/// **Rule 7 — the container is up but its readiness check is failing, so the Service has stopped
/// sending it traffic.** WARN, and the hardest rule in this file to keep quiet.
///
/// Four conditions, each load-bearing: the pod is `Running`; **the container is in
/// [`ContainerState::Running`]**, which is what tells this rule apart from rule 1; it is not
/// ready; and it has been that way for longer than [`NOT_READY_GRACE`].
///
/// **The since-when is [`PodSnapshot::ready`]'s `last_transition` and nothing else** —
/// specifically not [`ContainerSnapshot::started`] (NOTES § D51) — and **it is floored at the
/// container's own run start**, because `Ready` is a condition of the *pod* while this rule fires
/// per container (NOTES § D71, [`Finding::timestamp`]).
///
/// **`started` is read here as a suppressor, and that is not what D51 rejected.**
/// `Running && !started` is reachable **only** where a `startupProbe` is declared and has not
/// passed, and until it does the kubelet does not run the readiness probe at all — so
/// `ready: false` means *not asked yet* (NOTES § D71).
///
/// **No condition, no finding.** **Regular containers only — the one rule of the seven that is**;
/// what a not-ready sidecar does to the pod's own readiness is a rule of its own (invariant 13).
///
/// **The state check and the `started` suppressor are unproven and mutually redundant on the
/// committed captures** — todo.md's capture-trip box.
fn running_but_not_ready(now: &Time, pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    if c.role != ContainerRole::Regular {
        return None;
    }
    let ContainerState::Running { started_at } = &c.state else {
        return None;
    };
    if pod.phase.as_deref() != Some("Running") || c.ready || !c.started {
        return None;
    }
    let unready_since = pod.ready.as_ref()?.last_transition.as_ref()?;
    let since = match started_at {
        Some(began) if began.0 > unready_since.0 => began,
        _ => unready_since,
    };
    if now.0.duration_since(since.0) <= NOT_READY_GRACE {
        return None;
    }
    Some(Finding {
        severity: Severity::Warn,
        // `screens/alerts.md` and `screens/once.md` both draw this card, word for word. The
        // screen spec and the rule that fills it must not be a third place they can drift.
        title: "Running, but not receiving traffic — the readiness check is failing".to_string(),
        evidence: [container_fact(c), c.image.clone()].join(FACTS),
        action: "check the readiness probe: the path, the port, and whether the application \
                 answers it yet"
            .to_string(),
        kubectl_cmd: describe(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: Some(since.clone()),
    })
}

/// **Rule 8 — a mount that hands the container the machine, not a directory.** CRITICAL, and only
/// the escalated case: `/`, the runtime socket, or a writable host directory (NOTES § v1 rule set,
/// *Severity escalators*). The plain read-only hostPath belongs to the Analysis posture rows
/// (NOTES § D2).
///
/// **What the container gets is `path` joined with the mount's `subPath`**, never `path` alone,
/// and the join cuts both ways, which is correct (NOTES § D46, [`mounted_path`]).
///
/// **The socket escalator matches a runtime socket or any directory one sits under**
/// ([`is_runtime_socket`]), because what is inside a mounted directory is mounted too. Its action
/// carries the legitimate holder — an nvidia toolkit installer, a node security agent — since the
/// most severe card on the screen must not talk a newcomer into breaking one (NOTES § D78).
///
/// **Node infrastructure in `kube-system` is silent on the writable escalator alone** —
/// DaemonSet-owned **or** a mirror pod, since `etcd` and `kube-apiserver` are the latter
/// ([`PodSnapshot::mirror`]). **The other two escalators fire straight through that silence.**
/// **The `kube-system` narrowing is a known limit**: a CSI driver in `longhorn-system` gets a card
/// it has not earned (NOTES § D70).
///
/// No age ([`Finding::timestamp`]). **One captured shape reaches the socket escalator** —
/// `hostpath.json`'s `nosy` is handed `/run/containerd`. The named sockets themselves are planted
/// into decoded copies and stay that way: the fixtures' cluster runs containerd, so no capture off
/// it can carry a Docker or CRI-O socket at all (NOTES § D40, § D78).
fn escalated_host_path(pod: &PodSnapshot) -> Vec<Finding> {
    let node_agent = pod.id.namespace.as_deref() == Some(NODE_NAMESPACE)
        && (pod.mirror || pod.owner.kind == ObjectKind::DaemonSet);
    pod.host_path_mounts
        .iter()
        .filter_map(|m| {
            let path = mounted_path(m);
            // The three escalators, and the order matters: the two that are about *what* is
            // mounted are asked first, so they answer for a node agent that the writable one
            // stays quiet about.
            let (title, action) = if path == "/" {
                (
                    "A container has the whole filesystem of the machine it runs on mounted \
                     inside it",
                    "mount only the directory the container actually needs, not the root",
                )
            } else if is_runtime_socket(&path) {
                (
                    "A container can drive the container runtime, which is full control of \
                     that machine",
                    "remove the mount, unless this pod's job is to manage or watch the \
                     containers on the node — if it is, it already has full control of every \
                     node it runs on",
                )
            } else if !m.read_only && !node_agent {
                (
                    "A container can change files on the machine it runs on",
                    "mount it read-only if the container only needs to read it",
                )
            } else {
                return None;
            };
            Some(Finding {
                severity: Severity::Critical,
                title: title.to_string(),
                evidence: [
                    format!("container {}", m.container),
                    format!("{path} on the node"),
                    if m.read_only { "read-only" } else { "writable" }.to_string(),
                ]
                .join(FACTS),
                action: action.to_string(),
                kubectl_cmd: describe(&pod.id),
                owner: pod.owner.clone(),
                object: pod.id.clone(),
                timestamp: None,
            })
        })
        .collect()
}

/// The path the container actually receives: the volume's `hostPath.path` narrowed by the
/// mount's `subPath` — or by its [`sub_path_expr`](HostPathMount::sub_path_expr), which joins
/// the same way and stays unresolved on purpose. Upstream forbids both at once, so the `or`
/// picks whichever exists.
///
/// **The result is normalised, and rule 8's string compares only mean what they read if it
/// is**: `hostPath: {path: "//"}` passes upstream validation and resolves to `/` on the node,
/// and `/.` is the same trick (NOTES § D71). So: repeated separators collapsed, `.` elements
/// dropped, trailing separator gone.
///
/// `..` is deliberately **not** resolved — upstream rejects it in both a hostPath and a subPath,
/// and if one ever arrived, leaving it in the string matches no escalator and lands in the
/// writable branch, the safe direction.
fn mounted_path(m: &HostPathMount) -> String {
    let narrowing = m
        .sub_path
        .as_deref()
        .or(m.sub_path_expr.as_deref())
        .filter(|s| !s.is_empty());
    let joined = match narrowing {
        None => m.path.clone(),
        Some(sub) => format!("{}/{sub}", m.path),
    };
    let kept = joined
        .split('/')
        .filter(|e| !e.is_empty() && *e != ".")
        .collect::<Vec<_>>()
        .join("/");
    // An absolute path keeps its leading separator and a root that emptied out is `/`. A relative
    // one does reach here — upstream validation rejects `..` and nothing else — and is returned as
    // it arrived rather than being given a root it never had: `run/crio` is resolved against the
    // pod's bundle directory on the node and is not the node's `/run/crio`, so matching no
    // escalator is the safe direction (NOTES § D79).
    if joined.starts_with('/') {
        format!("/{kept}")
    } else {
        kept
    }
}

/// Whether what the container gets is one of [`RUNTIME_SOCKETS`] **or a directory one of them
/// sits under** — a container handed `/run/containerd` opens the socket inside it, which is the
/// same machine as being handed the socket. `/var/run/…` is folded onto `/run/…` first, so either
/// of the two names a systemd distribution has for the file answers the same (NOTES § D78).
///
/// **The rewrite is the comparison's and never the card's** — [`escalated_host_path`] prints the
/// path the manifest wrote, which is the string the reader searches their own YAML for. A path
/// that reaches no socket matches nothing and lands in the writable branch under its own name.
///
/// **`/var` and the empty string are not ancestors**, and the emptiness guard is what says so:
/// every prefix test calls `""` a prefix of everything, and `/var` — which does arrive off the
/// wire — folds to `""`. A mount of `/var` genuinely does not hand over the socket: a bind mount
/// carries `/var/run` as the symlink it is — `/run` on some hosts, `../run` on others — and
/// either form resolves inside the container's own mount namespace. The fold is `/var`-only and
/// one-directional, not a symlink resolver (NOTES § D78, § D79).
fn is_runtime_socket(path: &str) -> bool {
    let path = path.strip_prefix("/var").unwrap_or(path);
    !path.is_empty()
        && RUNTIME_SOCKETS.iter().any(|socket| {
            socket
                .strip_prefix(path)
                .is_some_and(|below| below.is_empty() || below.starts_with('/'))
        })
}

/// **Rule 10 — no machine in the cluster will take this pod.** `conditions[PodScheduled]` at
/// `False` with reason `Unschedulable`, and the scheduler's own sentence is the finding
/// (NOTES § D27, § D37). **It needs no Events watch**, which is the whole reason it ships in v1.
///
/// **Both halves of the condition are tested, never its presence.** The condition flips to `True`
/// rather than going away, and `reason` is asked because `SchedulingGated` — how Kueue, Volcano
/// and every quota-manager queue work — and `SchedulerError` are also `PodScheduled: False`.
/// Cutting either half leaves a green suite green, so both are proven on a planted field
/// (NOTES § D40, § D73).
///
/// **The severity is a ladder on the condition's own age, not a constant** — WARN below
/// [`NOT_READY_GRACE`], CRITICAL above it or with no stamp to measure — because a flat CRITICAL is
/// false on three routine paths that resolve without a human. **That age is when the condition
/// last changed *status*, which is not always when the pod became unplaceable**, so a released
/// `SchedulingGated` pod reaches CRITICAL immediately: a known imprecision accepted for want of a
/// better field (NOTES § D73). **Unlike rule 7, a missing stamp does not silence the rule**
/// ([`Finding::timestamp`]).
///
/// **`get -o yaml` and not `describe`**: `describePodConditions` prints a Type/Status table with
/// no reason or message (NOTES § D71).
///
/// **Rule 10 is silent on a Pending pod that has no `PodScheduled` condition** — that is
/// [`nothing_has_looked_at_it`]'s subject (NOTES § D74). **It can emit an empty `evidence`**, and
/// the renderers owe it what [`Finding::evidence`] asks. **Nothing here touches a
/// container**: an unschedulable pod has no `containerStatuses` at all, so a rule shaped like
/// rules 1–7 would go silent on its own fixture.
fn no_node_accepted_it(now: &Time, pod: &PodSnapshot) -> Option<Finding> {
    let scheduled = pod.scheduled.as_ref()?;
    if scheduled.status != "False" || scheduled.reason.as_deref() != Some("Unschedulable") {
        return None;
    }
    // Preemption has already chosen a machine and is clearing it: the pod is unschedulable and
    // the card's sentence is still false, which is the one shape where those two come apart
    // ([`PodSnapshot::nominated_node_name`], NOTES § D73).
    if pod.nominated_node_name.is_some() {
        return None;
    }
    // Somebody has asked for this pod to go away, so where it could have run is no longer a
    // question anyone can act on — the only move left is finding what holds the delete, which is
    // rule 12's card (NOTES § D73). For the first sixty seconds such a pod draws nothing at all,
    // until rule 12's margin opens, which is right.
    if pod.deletion_timestamp.is_some() {
        return None;
    }
    let since = scheduled.last_transition.as_ref();
    // No stamp is not "recent": a pod that cannot be shown to have just become
    // unplaceable is read as one that has been that way, which is the safe direction.
    let resolving = since.is_some_and(|t| now.0.duration_since(t.0) <= NOT_READY_GRACE);
    Some(Finding {
        severity: if resolving {
            Severity::Warn
        } else {
            Severity::Critical
        },
        // **The parenthetical is true only because the guard above already left.** `printPod`
        // overrides the column to **Terminating** on `deletionTimestamp != nil` for any
        // non-terminal phase while `phase` stays `Pending` underneath, so the two lines are one
        // decision written in two places (NOTES § D73).
        title: format!(
            "No machine in the cluster will take this pod, so it has never started{}",
            if pod.phase.as_deref() == Some("Pending") {
                " (it shows as Pending)"
            } else {
                ""
            }
        ),
        // The scheduler's sentence, verbatim and framed (NOTES § D37). The prefix does two
        // invariant-14 jobs: it says a machine wrote this, and it glosses the one word that
        // would otherwise split the card into two vocabularies — the title says *machine*, the
        // scheduler says *node*.
        evidence: scheduled.message.as_deref().map_or_else(String::new, |m| {
            format!("the scheduler's own words (a node is one machine): {m}")
        }),
        // **Only the half the command can answer.** The node half needs
        // `kubectl get nodes --show-labels`, which this card does not print, and it is N6's;
        // asking for work the command beside it cannot start points invariant 4's teaching device
        // away from itself. No reference to the line above either — it is empty whenever the
        // message is missing.
        action: "check what this pod asks for: the node labels it selects, which machines \
                 it says it can run on, and how much cpu and memory it requests"
            .to_string(),
        kubectl_cmd: get_yaml(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: scheduled.last_transition.clone(),
    })
}

/// **Every waiting reason another rule in this file already has a card for** — rule 13's
/// exclusion list, and the reason it is a *residual* rather than a twelfth opinion
/// (NOTES § D72). A reason that gains a rule of its own is added here in the same change, or two
/// cards describe one incident. Rule 3's seven are excluded through [`UNUSABLE_IMAGE`] itself
/// rather than copied here, which meets that requirement structurally (NOTES § D76).
const EXPLAINED_ELSEWHERE: [&str; 2] = [
    "CrashLoopBackOff", // rule 1 — and it has run, which rule 13 also excludes
    "CreateContainerConfigError", // rule 4
];

/// **The kubelet's `defaultWaitingState`, which is not a diagnosis and is not always a pointer
/// either** — and the difference between those two readings is most of rule 13.
///
/// It is written into **both** status arrays for every container of a pod that declares an init
/// container, so reading it as a block fires on every slow init container while reading it as a
/// pointer silences the rule on most production pods — the worse half, and the one that shipped
/// first (NOTES § D2, § D76).
///
/// **So it is a pointer only when there is something to point at**, which is what
/// [`nothing_else_to_point_at`] decides.
const WAITING_ON_A_SIBLING: &str = "PodInitializing";

/// **Is `PodInitializing` the only thing this pod has to say?** — the pod-level half of
/// [`WAITING_ON_A_SIBLING`], and the reason rule 13 takes the whole pod.
///
/// A container that is `Running` is something to wait for; a container carrying a reason of its
/// own is something to point at, whoever owns that reason, and the card for it is that rule's
/// (NOTES § D76).
fn nothing_else_to_point_at(pod: &PodSnapshot) -> bool {
    !pod.containers
        .iter()
        .any(|c| is_running(c) || waiting(c).is_some_and(|(r, _)| r != WAITING_ON_A_SIBLING))
}

/// **Is this container up right now?** — [`ContainerState::Running`] and nothing about readiness,
/// which is [`doing_its_job`]'s question and a different one. Rule 13 asks it twice: as something
/// for a `PodInitializing` sibling to be waiting on, and as what makes *"it has not been able to
/// start"* false about the pod.
fn is_running(c: &ContainerSnapshot) -> bool {
    matches!(c.state, ContainerState::Running { .. })
}

/// **A container that has never run and is not waiting for a reason somebody else owns** — rule
/// 13's per-container half, returning the kubelet's reason and sentence for the card. `bare`
/// carries [`nothing_else_to_point_at`]'s answer, because whether [`WAITING_ON_A_SIBLING`] counts
/// is a fact about the pod and cannot be decided from one container.
///
/// **"Never run" is [`ContainerSnapshot::last_terminated`] and not the state alone**, or rule
/// 13's card claims the pod never started when it did.
///
/// **What that leaves uncovered is wider than "rule 5 has it".** A container SIGTERMed by a node
/// reboot and then unable to be recreated reaches neither [`restarting_repeatedly`]'s threshold
/// nor [`previous_run_failed`]'s exits, so it draws nothing from any rule here. Still the right
/// trade, but it is a hole and not a hand-off.
fn stuck_at_the_starting_line(c: &ContainerSnapshot, bare: bool) -> Option<(&str, Option<&str>)> {
    if c.last_terminated.is_some() {
        return None;
    }
    let (reason, message) = waiting(c)?;
    if (reason == WAITING_ON_A_SIBLING && !bare)
        || EXPLAINED_ELSEWHERE.contains(&reason)
        || UNUSABLE_IMAGE.contains(&reason)
    {
        return None;
    }
    Some((reason, message))
}

/// **Rule 13 — it was given a machine to run on, and it has not been able to start.** The
/// `ContainerCreating` wedge: WARN, on a pod whose `PodScheduled` condition has read `True` for
/// more than [`NOT_READY_GRACE`] while nothing in it is running (NOTES § D72).
///
/// **It fires on the residual, and that is the design** — the reason list grows upstream without
/// asking, so a positive match would cover one case and go silent on the rest. What it must not do
/// is repeat a card another rule drew: [`EXPLAINED_ELSEWHERE`] and [`UNUSABLE_IMAGE`]'s job, and
/// **the image-error family is on the other side of that line** (NOTES § D76). **Rule 10 does not
/// see this pod, which is why the rule exists** — such a pod *is* scheduled.
///
/// **Silent on a pod that has no container statuses at all, and that gap is the N-series'** —
/// firing on the absence would name the pod when the fault is the node; N1 owns that.
///
/// **Ten minutes, from `scheduled.last_transition`** — rule 7's `progressDeadlineSeconds` borrow,
/// because pulling a large image onto a cold node legitimately takes minutes. **An unstamped
/// condition fires nothing**, the opposite direction from rule 10: here the ten minutes *is* the
/// gate. **WARN, not CRITICAL** (NOTES § D2, § D72), and **silent on a pod that is being
/// deleted**, for rule 10's reason.
///
/// **Silent the moment anything in the pod is running, and the title is why.** **It costs a real
/// case:** a sidecar up and one regular container stuck on a `CreateContainerError` draws nothing
/// from this file — a named hole, kept because a confident sentence that is false about the pod in
/// front of you is the more expensive failure (NOTES § D76).
///
/// **One card per pod, not per container**, and **which container it names is whichever the decode
/// put first** — alphabetical, since the kubelet sorts each status array by name — so the others
/// are **counted only when they share the reason** and **named with their own otherwise**
/// (NOTES § D76).
///
/// **`describe` and not `get -o yaml`, the opposite of rules 3, 4 and 10**: this card quotes a
/// reason that usually carries no message at all, and the sentence that finishes the diagnosis
/// exists only as an Event — re-emitted continuously for a wedged pod, so the `--event-ttl`
/// argument does not reach this rule.
///
/// **It ships with a negative side only** — todo.md's capture-trip box.
fn placed_but_never_started(now: &Time, pod: &PodSnapshot) -> Option<Finding> {
    let scheduled = pod.scheduled.as_ref()?;
    if scheduled.status != "True" {
        return None;
    }
    let since = scheduled.last_transition.as_ref()?;
    if now.0.duration_since(since.0) <= NOT_READY_GRACE {
        return None;
    }
    if pod.deletion_timestamp.is_some() {
        return None;
    }
    // Anything serving makes the title false, whatever else is wrong with the pod.
    if pod.containers.iter().any(is_running) {
        return None;
    }
    let bare = nothing_else_to_point_at(pod);
    let stuck: Vec<_> = pod
        .containers
        .iter()
        .filter_map(|c| stuck_at_the_starting_line(c, bare).map(|(r, m)| (c, r, m)))
        .collect();
    let &(named, reason, message) = stuck.first()?;

    let mut facts = vec![container_fact(named)];
    let rest = &stuck[1..];
    if !rest.is_empty() {
        facts.push(if rest.iter().all(|&(_, r, _)| r == reason) {
            format!(
                "{} in the same state",
                counted(rest.len() as i64, "other container")
            )
        } else {
            // Two failures needing two different fixes, so both are named: "in the same state"
            // would be the card inventing an agreement the kubelet never made (NOTES § D76).
            format!(
                "also: {}",
                rest.iter()
                    .map(|(c, r, _)| format!("{} ({r})", c.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
    }
    // The machine's own word, framed as a quote rather than translated: the reasons are an open
    // set and the frame has to fit all of them. **`PodInitializing` may not be framed that way at
    // all** — it is the kubelet's default waiting state, not a step
    // ([`WAITING_ON_A_SIBLING`], NOTES § D76).
    facts.push(if reason == WAITING_ON_A_SIBLING {
        "the machine has not said which step it is on — it still reports every container as \
         starting up (PodInitializing)"
            .to_string()
    } else {
        format!("the machine's own word for where it is stuck: {reason}")
    });
    facts.extend(message.map(str::to_string));
    facts.push(
        // **The order of the kubelet's own work decides which sentence is which**:
        // `kubelet.SyncPod` calls `volumeManager.WaitForAttachAndMount` *before*
        // `containerRuntime.SyncPod` creates the sandbox, so the condition is `False` for a
        // volume failure as much as for a network one. Inverted, this pair sends a reader whose
        // ConfigMap is missing to look at the CNI (NOTES § D76).
        if pod
            .ready_to_start_containers
            .as_ref()
            .is_some_and(|c| c.status == "False")
        {
            "the machine has not been able to give this pod its storage or its network yet — \
             it has not got as far as creating the container"
                .to_string()
        } else {
            "this pod has its storage and its network, so the block is later — the image is \
             still downloading, or the container could not be created"
                .to_string()
        },
    );
    Some(Finding {
        severity: Severity::Warn,
        title: "This pod was given a machine to run on, but it has not been able to start"
            .to_string(),
        evidence: facts.join(FACTS),
        action: "read the Events at the bottom of the describe output — that is where the \
                 machine says what it is still waiting for"
            .to_string(),
        kubectl_cmd: describe(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        // The same binding the grace was measured from rather than a second lookup of the same
        // field: the card's age and the rule's threshold answer one question and must never come
        // apart.
        timestamp: Some(since.clone()),
    })
}

/// **Rule 14 — nothing has even looked at this pod.** CRITICAL, on a pod that is `Pending` with
/// **no `PodScheduled` condition at all**, more than [`NEVER_JUDGED_GRACE`] after
/// `metadata.creationTimestamp` (NOTES § D74).
///
/// **The absence is the whole signal, and it is a residual like rule 13's**: whatever picks
/// machines writes that condition either way, so a pod carrying neither has not been judged by
/// anything — kube-scheduler is down, or `spec.schedulerName` names one that is not installed or
/// lacks RBAC. **The card names both and claims neither**, because `schedulerName` is not on
/// [`PodSnapshot`] (NOTES § D74).
///
/// **CRITICAL, where rule 13 is WARN**: nothing healthy looks like this. **An absent
/// `creationTimestamp` fires nothing** — the grace *is* the gate.
///
/// **Silent on a pod that is being deleted**, for rules 10 and 13's reason and one of its own:
/// `printPod` prints **Terminating** while `phase` stays `Pending` underneath, so without this
/// guard the card would say *it shows as Pending* beside rule 12 saying *it shows as Terminating*
/// (NOTES § D73). **The guard and the parenthetical below are one decision written in two
/// places.**
///
/// **`get -o yaml` and not `describe`**: the evidence is the *absence* of a field, and `describe`
/// prints `Events: <none>` — the dead end a beginner has already reached.
///
/// **Known and deliberately unsolved:** if the scheduler really is down this fires for every owner
/// in the cluster and buries the screen (NOTES § D74).
///
/// **One shape it names imprecisely, kept rather than guarded.** A pod created with
/// `spec.nodeName` already set skips the scheduler, so if that node's kubelet never reports, the
/// card blames a scheduler that was never in the story — not a false *finding*, only an action
/// pointed one component away, and narrowing on `pod.node.is_none()` would trade it for silence on
/// a broken pod. The node half is N1's.
///
/// **Its positive side has no capture** — todo.md's capture-trip box.
fn nothing_has_looked_at_it(now: &Time, pod: &PodSnapshot) -> Option<Finding> {
    if pod.phase.as_deref() != Some("Pending") || pod.scheduled.is_some() {
        return None;
    }
    let created = pod.creation_timestamp.as_ref()?;
    if now.0.duration_since(created.0) <= NEVER_JUDGED_GRACE {
        return None;
    }
    if pod.deletion_timestamp.is_some() {
        return None;
    }
    Some(Finding {
        severity: Severity::Critical,
        // True only because the deletion guard above already left — read the deletion note in
        // this function's doc before touching either.
        title: "Nothing has even looked at this pod yet, so it has never started (it shows as \
                Pending)"
            .to_string(),
        // **The field is explained by what carries it rather than translated**, because there is
        // no plainer name for a line that is not there: naming the two states that both write it
        // is what makes the absence mean something to someone meeting `PodScheduled` for the
        // first time (invariant 14).
        evidence: "nothing has written a scheduling decision on it: a pod that was given a \
                   machine and a pod that was refused one both carry a PodScheduled line in \
                   their status, and this one has no such line at all"
            .to_string(),
        // Both causes, as checks rather than claims, and in the order they cost to check.
        action: "check that something is actually scheduling — on most clusters kube-scheduler \
                 is a pod in the kube-system namespace — and that this pod is not asking for a \
                 different one by name (spec.schedulerName)"
            .to_string(),
        kubectl_cmd: get_yaml(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        // The same moment the grace was measured from: when the pod arrived and the waiting
        // started. There is no event of its own to date it by — that is the finding.
        timestamp: Some(created.clone()),
    })
}

/// **Rule 12 — the pod was asked to shut down and is still here.** WARN: nothing is down, but an
/// operation somebody started has not finished, and until it does the replacement pod does not
/// start and the node does not drain.
///
/// **`deletionTimestamp` is a deadline, not a moment** — request time plus the grace period — so
/// the pod is overdue once `now` passes the field itself (NOTES § D46). **The trigger carries a
/// flat [`OVERDUE_MARGIN`]** (NOTES § D55, § D71).
///
/// **The age is the moment the user asked**, `deletionTimestamp − grace`, and the subtraction is
/// `checked_sub`: a `terminationGracePeriodSeconds` of `i64::MAX` is a value the live API server
/// accepted, and a plain `-` panics on it (NOTES § D56). It answers `None` there rather than a
/// wrong moment.
///
/// **The finalizers are the whole diagnosis** — two causes with unrelated fixes — and
/// `kubectl describe pod` does not print them at all, which is why the command beside this card
/// is `get -o yaml` (NOTES § D46).
fn stuck_terminating(now: &Time, pod: &PodSnapshot) -> Option<Finding> {
    let deadline = pod.deletion_timestamp.as_ref()?;
    let grace = pod.grace_period_seconds.map(SignedDuration::from_secs);
    if now.0.duration_since(deadline.0) <= OVERDUE_MARGIN {
        return None;
    }
    let mut facts = Vec::new();
    if let Some(node) = &pod.node {
        facts.push(format!("on node {node}"));
    }
    let held = !pod.finalizers.is_empty();
    if held {
        facts.push(format!(
            "held by {}: {}",
            if pod.finalizers.len() == 1 {
                "a finalizer"
            } else {
                "finalizers"
            },
            pod.finalizers.join(", ")
        ));
    }
    Some(Finding {
        severity: Severity::Warn,
        title: "This pod was asked to shut down and is still here (it shows as Terminating)"
            .to_string(),
        evidence: facts.join(FACTS),
        action: if held {
            "nothing can delete this pod while that list has anything in it — find what put \
             it there"
                .to_string()
        } else {
            "nothing is holding the pod, so check the kubelet on that machine".to_string()
        },
        kubectl_cmd: get_yaml(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: grace.and_then(|g| deadline.0.checked_sub(g).ok()).map(Time),
    })
}

// --- THE POD RULES END ---

#[cfg(test)]
mod tests {
    use super::*;
    // `ContainerStateRunning` is imported here and not beside the decode's own types: no
    // product code in this file constructs one, and the top-level list is what `rules.rs`
    // reads off the API.
    use k8s_openapi::api::core::v1::{
        ContainerStateRunning, ContainerStateWaiting, HostPathVolumeSource, Taint as ApiTaint,
        Toleration as ApiToleration, Volume, VolumeMount,
    };
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
    use std::collections::{BTreeSet, HashSet};

    /// The Alerts list is sorted by severity, and that order is nothing but the order
    /// the variants are declared in — so it is asserted, not assumed.
    #[test]
    fn severity_sorts_most_severe_first() {
        let mut got = vec![Severity::Info, Severity::Critical, Severity::Warn];
        got.sort();
        println!("sorted severities: {got:?}");
        assert_eq!(
            got,
            vec![Severity::Critical, Severity::Warn, Severity::Info],
            "sorting by severity must put the most severe first"
        );
        assert!(Severity::Critical < Severity::Warn);
        assert!(Severity::Warn < Severity::Info);
    }

    fn deployment(uid: &str) -> ObjectId {
        ObjectId {
            kind: ObjectKind::Deployment,
            namespace: Some("payments".to_string()),
            name: "web".to_string(),
            uid: Some(uid.to_string()),
        }
    }

    /// One Deployment deleted and recreated under the same name — an Argo
    /// prune-and-recreate: the old generation's pods still terminate under uid-A while
    /// the new ones run under uid-B. D3 says that is **one** card.
    #[test]
    fn the_uid_is_not_part_of_the_grouping_key() {
        let old = deployment("9f2c-aaaa");
        let new = deployment("9f2c-bbbb");

        println!("old generation: {old:?}\nnew generation: {new:?}");
        println!("group keys: {:?} vs {:?}", old.group_key(), new.group_key());

        assert_eq!(
            old.group_key(),
            new.group_key(),
            "two ObjectIds differing only in uid must group onto one card (D3)"
        );

        // The same fact the way `views.rs` will actually meet it: as map keys.
        let keys: HashSet<_> = [old.group_key(), new.group_key()].into_iter().collect();
        println!("distinct group keys: {}", keys.len());
        assert_eq!(keys.len(), 1, "one Deployment must not produce two cards");
    }

    /// The negative half: a `group_key` dropping the namespace, or the name, or
    /// returning a constant satisfies the test above perfectly, and over-grouping is the
    /// worse bug — one card over two Deployments hides an outage. The last case is a
    /// Node, the shape a cluster-scoped `ObjectId` actually arrives in.
    #[test]
    fn objects_that_are_not_the_same_object_do_not_share_a_group_key() {
        let base = deployment("9f2c-aaaa");
        let mut other_namespace = base.clone();
        other_namespace.namespace = Some("staging".to_string());
        let mut other_name = base.clone();
        other_name.name = "api".to_string();
        let mut other_kind = base.clone();
        other_kind.kind = ObjectKind::StatefulSet;
        let node = ObjectId {
            kind: ObjectKind::Node,
            namespace: None,
            name: "k8rs-worker2".to_string(),
            uid: Some("7b1e-cccc".to_string()),
        };

        for (label, other) in [
            ("different namespace", &other_namespace),
            ("different name", &other_name),
            ("different kind", &other_kind),
            ("cluster-scoped node vs namespaced workload", &node),
        ] {
            println!("{label}: {:?} vs {:?}", base.group_key(), other.group_key());
            assert_ne!(
                base.group_key(),
                other.group_key(),
                "{label}: these are different objects and must not share a card"
            );
        }
    }

    /// Equality still sees the uid, because it answers D22's question — is this still
    /// the object the operator inspected. A `group_key` that grouped correctly by
    /// weakening `Eq` would trade a drawing bug for a mutation bug.
    #[test]
    fn equality_still_sees_the_uid() {
        assert_ne!(
            deployment("9f2c-aaaa"),
            deployment("9f2c-bbbb"),
            "equality must distinguish an object from the one that replaced it (D22)"
        );
        assert_eq!(
            deployment("9f2c-aaaa"),
            deployment("9f2c-aaaa"),
            "the same object read twice must still compare equal"
        );
    }

    // --- THE AGE AT THE RIGHT EDGE ---
    //
    // Every case here hands a **duration** in and compares the answer against the string a
    // screen draws. The ladder goes through [`age`] because the rungs are what it is
    // testing; the card goes through [`Finding::age`], because that is the call a renderer
    // makes for a finding. Nothing parses English back into a number: a test that read
    // "4" out of "4 min ago" would agree with an implementation that printed the minutes
    // of the wall clock, which is the class of bug the whole "timestamps, not phrases"
    // contract exists to stop.

    /// A moment `secs` seconds before the pinned [`now`]. Negative puts the event in the
    /// future: D55's *slow* laptop while it is inside [`SKEW_ALLOWANCE`], and past that a
    /// rule reading a field that was never an event time.
    ///
    /// `checked_sub`, because every subtraction in this file that can leave the
    /// representable range is checked (NOTES § D56); here the failure would be a
    /// mistyped case rather than a hostile pod, and it names itself either way.
    fn ago(secs: i64) -> Time {
        Time(
            now()
                .0
                .checked_sub(SignedDuration::from_secs(secs))
                .unwrap_or_else(|e| panic!("{secs}s before the pinned now is not a moment: {e}")),
        )
    }

    /// **The ladder, at both sides of every boundary it has.** The rungs are not a
    /// choice: each string below is one a `screens/` file already prints, and the
    /// boundaries are where one stops being the truth and the next starts.
    ///
    /// **43 minutes is the case that is here for the arithmetic and not for the
    /// wording** — `now.0 - event.0` yields a seconds-only `Span`, so a formatter written
    /// with `.get_minutes()` reads "0 min ago" for it, and for every gap under an hour.
    /// The value comes from NOTES § D54, which names the trap and the length it hides.
    ///
    /// **The two cases at the top are the [`SKEW_ALLOWANCE`] boundary**, and the far one
    /// is not a clock story: 25 hours ahead is what a rule pointed at a certificate's
    /// `notAfter` or at a raw `deletionTimestamp` produces, and the requirement is that it
    /// draws *nothing* rather than a sentence that reads fine.
    #[test]
    fn the_age_ladder_is_the_words_the_screens_print() {
        for (secs, want) in [
            (-90_000, None),
            (-301, None),
            (-300, Some("just now")),
            (-1, Some("just now")),
            (0, Some("just now")),
            (1, Some("1s ago")),
            (40, Some("40s ago")),
            (59, Some("59s ago")),
            (60, Some("1 min ago")),
            (60 * 4, Some("4 min ago")),
            (60 * 43, Some("43 min ago")),
            (60 * 60 - 1, Some("59 min ago")),
            (60 * 60, Some("1 hour ago")),
            (60 * 60 * 2, Some("2 hours ago")),
            (60 * 60 * 24 - 1, Some("23 hours ago")),
            (60 * 60 * 24, Some("1 day ago")),
            (60 * 60 * 24 * 2, Some("2 days ago")),
            (60 * 60 * 24 * 6, Some("6 days ago")),
        ] {
            let got = age(&now(), &ago(secs));
            println!("{secs:>9}s -> {got:?}");
            assert_eq!(
                got.as_deref(),
                want,
                "an event {secs}s before now has to read {want:?} — the strings are the \
                 ones screens/ draws, and the boundaries are where they stop being true"
            );
        }

        // The rung the table cannot reach, because its cases are whole seconds: an event
        // 400ms old is inside the first one. `0s ago` is a string no screen draws and it
        // reads as a stopped clock, so the sub-second gap says "just now" with the
        // negative ages — the one place this branch is not about a wrong laptop.
        let sub_second = age(&now(), &time("2026-08-12T23:59:59.600Z"));
        println!("     0.4s -> {sub_second:?}");
        assert_eq!(
            sub_second.as_deref(),
            Some("just now"),
            "an event 400ms old is \"just now\", never \"0s ago\""
        );
    }

    /// **One event, four laptops** — the framing NOTES § D55 corrects, and the two things
    /// the guard does not do.
    ///
    /// A laptop a little behind the cluster produces a negative age and draws "just now":
    /// under-reporting, which harms nobody. Far enough behind and the timestamp stops
    /// being distinguishable from a rule reading a field that is future-dated by design,
    /// so [`age`] draws **nothing** and leaves the explaining to the header banner, which
    /// is its own box. A laptop *ahead* of the cluster inflates the same event into a
    /// ten-minute-old one, and **that is left visible on purpose** — clamping it would
    /// hide a wrong clock rather than survive one, and it is the half that manufactures
    /// findings on a healthy cluster.
    ///
    /// A formatter that took `.abs()` of the difference, or clamped both ends, passes the
    /// ladder test above and fails here.
    #[test]
    fn a_laptop_a_little_behind_says_just_now_far_behind_says_nothing_and_ahead_is_not_hidden() {
        let event = time("2026-08-12T12:00:00Z");
        let behind = |mins: i64| {
            Time(
                event
                    .0
                    .checked_sub(SignedDuration::from_mins(mins))
                    .expect("a moment"),
            )
        };

        for (label, laptop, want) in [
            ("2 min behind the cluster", behind(2), Some("just now")),
            ("agreeing with the cluster", behind(0), Some("just now")),
            ("10 min behind the cluster", behind(10), None),
            (
                "10 min ahead of the cluster",
                behind(-10),
                Some("10 min ago"),
            ),
        ] {
            let got = age(&laptop, &event);
            println!("event {:?}, laptop {label}: {got:?}", event.0);
            assert_eq!(got.as_deref(), want, "a laptop {label} must draw {want:?}");
        }
    }

    /// **The `Option`, on the two taints the capture actually carries.** N2's card is
    /// where the field's two states are one keystroke apart: `break-nodes` cordons
    /// `k8rs-worker`, the node lifecycle controller mirrors that boolean into a taint and
    /// stamps `timeAdded` on it — so the card can say when — while the operator's own
    /// `dedicated=gpu:NoExecute` on `k8rs-worker2` was written by `kubectl taint`, which
    /// is client-side and stamps nothing (NOTES § D64, § D65).
    ///
    /// What is asserted is the whole render decision — [`Finding::age`], the one call
    /// both renderers make: a phrase for the card that has a moment, **nothing at all**
    /// for the one that has not, which is `screens/alerts.md`'s blank right edge and
    /// `screens/once.md`'s bare title line.
    ///
    /// **And why the field is not a plain `Time`:** the value a non-optional field would
    /// hold is the epoch, which this formatter dates honestly and uselessly. That
    /// assertion is deliberately loose about the count — it is 1970 that is being shown,
    /// not a number worth pinning.
    #[test]
    fn the_captured_cordon_dates_itself_and_the_hand_applied_taint_leaves_the_age_blank() {
        let nodes: Vec<NodeSnapshot> = items::<Node>("nodes").into_iter().map(Into::into).collect();
        let taint = |node: &str, key: &str| {
            nodes
                .iter()
                .find(|n| n.id.name == node)
                .unwrap_or_else(|| panic!("the capture has no {node}"))
                .taints
                .iter()
                .find(|t| t.key == key)
                .unwrap_or_else(|| panic!("{node} carries no {key} taint"))
                .clone()
        };
        let cordon = taint("k8rs-worker", "node.kubernetes.io/unschedulable");
        let by_hand = taint("k8rs-worker2", "dedicated");

        // The card N2 files, with the moment the capture gives it and without one. Both
        // identities are the node itself — `owner == object` for N1–N3 (D39).
        let node = ObjectId {
            kind: ObjectKind::Node,
            namespace: None,
            name: "k8rs-worker".to_string(),
            uid: None,
        };
        let card = |t: Option<Time>| Finding {
            severity: Severity::Warn,
            title: "This node refuses new pods (cordoned)".to_string(),
            evidence: "2 pods here would still have to move".to_string(),
            action: "allow new pods once the work is done".to_string(),
            kubectl_cmd: Some("kubectl describe node k8rs-worker".to_string()),
            owner: node.clone(),
            object: node.clone(),
            timestamp: t,
        };
        let dated = card(cordon.added_at.clone());
        let undated = card(by_hand.added_at.clone());
        println!(
            "cordon taint {:?}\n  {} · {:?}\nhand-applied taint {:?}\n  {} · {:?}",
            cordon.added_at,
            dated.title,
            dated.age(&now()),
            by_hand.added_at,
            undated.title,
            undated.age(&now()),
        );

        // The property the fixture has to keep, and it is asserted at the precision of the
        // string below rather than looser: the pin is the midnight after the capture day
        // (D57), so this cordon is two-something hours old. A band of `[1h, 24h)` would let
        // a recapture at 1h50m past this line and fail on the phrase instead, with a
        // message about cards saying when — which is the confusion the check exists to
        // prevent, not to cause.
        let stamped = cordon.added_at.clone().expect(
            "the controller stamps timeAdded on the taint it mirrors from spec.unschedulable \
             — a capture without it is D64's premise back again",
        );
        let elapsed = now().0.duration_since(stamped.0);
        assert_eq!(
            elapsed.as_hours(),
            2,
            "the cordon is {elapsed:?} before the pinned now, and the phrase below says two \
             hours — if `just fixtures` was re-run, repin `fn now()` (see the note there for \
             what moves with it) and move both together"
        );
        assert_eq!(
            dated.age(&now()).as_deref(),
            Some("2 hours ago"),
            "a cordon the controller stamped has a moment, and the card says when"
        );
        assert_eq!(
            undated.age(&now()),
            None,
            "`kubectl taint` stamps no time, so the card has no age to draw and draws \
             none — never a nearby timestamp that answers a different question"
        );

        // **The third state, which no capture can hold.** Every committed timestamp is
        // before the pin by construction — the sweep guarantees it — so the card that was
        // filled from a field which is future-dated *by design* has to be synthesised, the
        // same licence D40 gives the taint that carries a value and a stamp at once. The
        // moment here is C1's shape: `notAfter` on the healthy committed certificate,
        // which `certs-test.sh` reports as 364 days out. `Finding::age` flattens it to the
        // same blank the missing field draws — `.map` in place of `.and_then` would print
        // it, and `Option<Time>` alone cannot tell the two cases apart because the field
        // is present and perfectly valid.
        let wrong_field = card(Some(time("2027-08-12T00:00:00Z")));
        println!(
            "a rule that filled the timestamp from a certificate's notAfter: {:?}",
            wrong_field.age(&now())
        );
        assert_eq!(
            wrong_field.age(&now()),
            None,
            "a moment a year ahead is a rule reading the wrong field, not a wrong clock, \
             and it draws nothing rather than a sentence that reads fine"
        );

        let epoch = age(&now(), &time("1970-01-01T00:00:00Z")).expect("1970 is in the past");
        println!("what a zero would have drawn: {epoch}");
        assert!(
            epoch.ends_with(" days ago") && epoch != "just now",
            "a zero timestamp draws as 1970 and not as silence — which is why the field is \
             an Option, and it read {epoch:?}"
        );
    }

    // --- THE DECODE, AGAINST THE COMMITTED CAPTURES ---
    //
    // Every fixture below came off the kind cluster `scripts/cluster.sh` builds, was
    // verified to have reached the state its rule is about, and was sanitized on the way
    // out (`just fixtures`). What is asserted is the value a later rule fires on — not
    // that the JSON parsed, which is `serde`'s claim and not ours.

    fn fixture(name: &str) -> serde_json::Value {
        let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("fixture {path} could not be read: {e}"));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {path} is not JSON: {e}"))
    }

    /// C1's input is a PEM file, not JSON — `scripts/make-certs.sh` writes the three with
    /// pinned dates, so a certificate fixture cannot expire out from under a test.
    fn certificate(name: &str) -> Vec<u8> {
        let path = format!(
            "{}/tests/fixtures/certs/{name}.crt.pem",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(&path).unwrap_or_else(|e| panic!("certificate {path} could not be read: {e}"))
    }

    fn pod(name: &str) -> PodSnapshot {
        let pod: Pod = serde_json::from_value(fixture(name))
            .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
        PodSnapshot::from(pod)
    }

    /// `kubectl get -A` answers with `kind: List`, which `k8s_openapi::List<T>` refuses
    /// — it wants `NodeList` — so the items come out by hand here. JSON plumbing in a
    /// test, not a second decode of the snapshot types.
    fn items<T: k8s_openapi::serde::de::DeserializeOwned>(name: &str) -> Vec<T> {
        fixture(name)["items"]
            .as_array()
            .unwrap_or_else(|| panic!("{name}.json has no items array"))
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone())
                    .unwrap_or_else(|e| panic!("{name}.json item does not decode: {e}"))
            })
            .collect()
    }

    fn time(s: &str) -> Time {
        Time(
            s.parse()
                .unwrap_or_else(|e| panic!("{s} is not a time: {e}")),
        )
    }

    // --- WHAT THE CAPTURE ITSELF SAYS START ---
    //
    // **An expectation read out of the capture, for the fields whose value belongs to the
    // cluster rather than to the requirement.** A uid, a timestamp, a restart counter, a
    // node name and a scheduler's sentence are all new on every `just fixtures`: a literal
    // transcribed from one capture asserts the cluster that produced it, and the next trip
    // reddens a test whose requirement never moved (which is exactly what happened here —
    // the four-node capture of 2026-08-12 turned seventeen of these red at once).
    //
    // **It is not a tautology, because the decode is not what it is read from.** These are
    // decode assertions and what they exist to catch is a field dropped on the way
    // through, filled from its neighbour, or rewritten — comparing the snapshot against
    // the *JSON the decode was handed* catches all three, and names the path it must have
    // come from while doing it (`initContainerStatuses` is not `containerStatuses`, and
    // `startedAt` is not `finishedAt`).
    //
    // **What it cannot check is that the capture still has the shape the test exists
    // for** — a fixture whose restart count fell to zero would satisfy every derived
    // assertion above and prove nothing about rule 5. So each use is paired with the
    // property the fixture has to keep: a counter inside its rule's band, two moments that
    // differ, a message that still names the object it is evidence about.
    //
    // Absence panics rather than comparing `None` against `None`: a path that stops
    // matching the capture is the one failure this technique could otherwise hide.

    fn at<'a>(value: &'a serde_json::Value, path: &[&str]) -> &'a serde_json::Value {
        let mut at = value;
        for key in path {
            at = &at[key];
        }
        at
    }

    fn captured_str<'a>(value: &'a serde_json::Value, path: &[&str]) -> &'a str {
        at(value, path).as_str().unwrap_or_else(|| {
            panic!(
                "the capture carries no string at {path:?}, so nothing here is compared against it"
            )
        })
    }

    /// `i32` because that is what the API declares a restart count and every replica
    /// counter as, and what the snapshot types carry.
    fn captured_i32(value: &serde_json::Value, path: &[&str]) -> i32 {
        let n = at(value, path).as_i64().unwrap_or_else(|| {
            panic!(
                "the capture carries no number at {path:?}, so nothing here is compared against it"
            )
        });
        i32::try_from(n)
            .unwrap_or_else(|_| panic!("{path:?} is {n}, which is not the i32 the API declares"))
    }

    fn captured_time(value: &serde_json::Value, path: &[&str]) -> Time {
        time(captured_str(value, path))
    }

    /// One container's captured status, out of the array it has to have come from.
    /// **Which array is part of the assertion** — a number read back from
    /// `containerStatuses` is not evidence about the init one, and reading both lists is
    /// the whole of D27.
    fn captured_status<'a>(
        pod: &'a serde_json::Value,
        array: &str,
        name: &str,
    ) -> &'a serde_json::Value {
        pod["status"][array]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["name"] == name)
            .unwrap_or_else(|| panic!("the capture has no {name} in status.{array}"))
    }

    /// One condition of a captured object, by type — the array [`PodSnapshot::ready`],
    /// [`PodSnapshot::scheduled`] and every `NodeSnapshot` condition are picked out of by
    /// name, and picking by index instead is the defect the healthy pod exists to catch.
    fn captured_condition<'a>(object: &'a serde_json::Value, type_: &str) -> &'a serde_json::Value {
        object["status"]["conditions"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|c| c["type"] == type_)
            .unwrap_or_else(|| panic!("the capture carries no {type_} condition"))
    }

    /// One item of a `kubectl get -A` capture, by name — the List counterpart of
    /// [`captured_status`].
    fn captured_item<'a>(list: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
        list["items"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|i| i["metadata"]["name"] == name)
            .unwrap_or_else(|| panic!("the capture has no {name} in its items"))
    }
    // --- WHAT THE CAPTURE ITSELF SAYS END ---

    /// **The moment every snapshot in this file is read at.** One helper rather than a
    /// literal at each construction site: the pin is a single fact about the committed
    /// captures, and a copy of a fact is a copy that drifts.
    ///
    /// **The value is not free.** `scripts/certs-test.sh` extracts this literal out of this
    /// function, refuses to disagree with it, and asserts the committed certificates against
    /// it on every `just check`. **Moving it moves `scripts/certs-test.sh` and
    /// `scripts/make-certs.sh` in the same change** — the pin is one fact spelled in four
    /// places across two ownership rows (NOTES § D57).
    ///
    /// It also lands after every `Time` the snapshot types *expose*, which
    /// `the_pinned_now_is_not_before_the_captures_it_is_read_against` asserts rather than
    /// leaves to trust.
    ///
    /// **The shape of the value is the midnight after the capture day** — near enough that a
    /// fixture's age is one an operator would recognise, and round enough to be repeated in
    /// three other files without transcription error.
    fn now() -> Time {
        time("2026-08-13T00:00:00Z")
    }

    fn container<'a>(pod: &'a PodSnapshot, name: &str) -> &'a ContainerSnapshot {
        pod.containers
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("{} has no container {name}", pod.id.name))
    }

    /// Rules 1, 5 and 6 all fire on this one pod, so it is the fixture that proves the
    /// three fields they read arrive together and separately.
    #[test]
    fn crashloop_pod_decodes_what_rules_1_5_and_6_read() {
        let raw = fixture("crashloop");
        let p = pod("crashloop");
        println!("{:?}\n  containers: {:?}", p.id, p.containers);

        assert_eq!(p.id.kind, ObjectKind::Pod);
        assert_eq!(p.id.namespace.as_deref(), Some("default"));
        assert_eq!(p.id.name, "broken-crashloop");
        assert_eq!(
            p.id.uid.as_deref(),
            Some(captured_str(&raw, &["metadata", "uid"])),
            "D22 asks whether this is still the object the operator inspected, and this \
             field is the whole of the answer — the apiserver's own uid, minted fresh for \
             every pod of every capture"
        );
        assert_eq!(
            p.owner, p.id,
            "nothing controls this pod, so it files under itself (D3)"
        );
        assert_eq!(
            p.node.as_deref(),
            Some(captured_str(&raw, &["spec", "nodeName"])),
            "the key N5 and N6 join on; which worker the scheduler picked is the \
             cluster's business, and it picked a different one this capture"
        );
        assert_eq!(p.phase.as_deref(), Some("Running"));

        let status = captured_status(&raw, "containerStatuses", "quitter");
        let c = container(&p, "quitter");
        assert_eq!(c.role, ContainerRole::Regular);
        assert!(!c.ready);
        assert!(
            !c.started,
            "it is between crashes, so it is not started either — and rule 7 must not \
             read that as a readiness failure"
        );
        assert_eq!(
            c.restarts,
            captured_i32(status, &["restartCount"]),
            "rule 5 counts restarts, and it counts this container's own"
        );
        assert!(
            c.restarts >= 3,
            "a container the kubelet has put in CrashLoopBackOff has died several times \
             by definition, so a count below rule 5's own WARN threshold means the \
             counter is wrong and not the pod: got {}",
            c.restarts
        );
        match &c.state {
            ContainerState::Waiting { reason, message } => {
                assert_eq!(reason.as_deref(), Some("CrashLoopBackOff"), "rule 1");
                assert!(
                    message.as_deref().unwrap_or_default().contains("back-off"),
                    "rule 1 shows the kubelet's own sentence, got {message:?}"
                );
            }
            other => panic!("a crashlooping container must decode as waiting, got {other:?}"),
        }
        // The two moments come from their own keys, and the gap between them is the whole
        // point of carrying both: "it runs for about two seconds and then exits 1" is a
        // different incident from "it ran for forty minutes and then exited 1", and a
        // decode that filled `started_at` from `finished_at` — or dropped it — tells the
        // operator the same sentence for both. The exit code and the reason stay literal:
        // they are what `scripts/broken.yaml`'s container *does*, not when it was
        // photographed doing it.
        let last = at(status, &["lastState", "terminated"]);
        assert_eq!(
            c.last_terminated,
            Some(Terminated {
                reason: Some("Error".to_string()),
                exit_code: 1,
                started_at: Some(captured_time(last, &["startedAt"])),
                finished_at: Some(captured_time(last, &["finishedAt"])),
                // D51's field, and the capture carries it now: `terminationMessagePolicy:
                // FallbackToLogsOnError` makes the kubelet copy the tail of the
                // container's log in here, which is what turns rule 6's action from
                // "check the logs" into the log line itself.
                message: Some(captured_str(last, &["message"]).to_string()),
            }),
            "rule 6 translates the exit code, and the finding is aged from finished_at"
        );
        let decoded = c.last_terminated.as_ref().expect("asserted just above");
        assert!(
            decoded.started_at < decoded.finished_at,
            "the run has to have started before it ended, or one of the two was filled \
             from the other: {decoded:?}"
        );
        assert!(
            decoded
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("connection refused"),
            "rule 6 shows the application's own last line, and the log tail is what makes \
             it worth showing: {:?}",
            decoded.message
        );
        assert_eq!(
            c.memory_limit, None,
            "this container sets no limit, and rule 2 may not report exceeding one"
        );
    }

    /// Rule 2 needs two fields from two different places: the kill reason from the
    /// status and the limit it broke from the spec.
    #[test]
    fn oom_pod_decodes_the_exit_code_and_the_limit_rule_2_reads() {
        let p = pod("oom");
        let c = container(&p, "hog");
        println!(
            "{}: {:?} limit={:?}",
            c.name, c.last_terminated, c.memory_limit
        );

        let last = c
            .last_terminated
            .as_ref()
            .expect("the container was killed once");
        assert_eq!(last.reason.as_deref(), Some("OOMKilled"));
        assert_eq!(
            last.exit_code, 137,
            "137 is SIGKILL, almost always the OOM killer"
        );
        assert_eq!(
            c.memory_limit.as_deref(),
            Some("64Mi"),
            "rule 2's evidence is the limit that was exceeded"
        );
        assert_eq!(c.memory_request.as_deref(), Some("64Mi"));
        assert_eq!(c.cpu_request, None, "this pod sets no cpu request");
    }

    /// Rules 3 and 4 have nothing to say beyond the runtime's own message, so losing it
    /// in the decode would leave the finding with no evidence at all.
    #[test]
    fn image_and_config_failures_keep_the_runtimes_own_message() {
        let raw = fixture("image");
        let image = pod("image");
        let c = container(&image, "nope");
        println!("image: {:?}", c.state);
        let ContainerState::Waiting { reason, message } = &c.state else {
            panic!("an image that cannot be pulled leaves the container waiting")
        };
        // **The reason is one of two and the kubelet alternates between them**, which is
        // why neither is written here as a literal. A failed pull reports `ErrImagePull`,
        // the kubelet then backs off and reports `ImagePullBackOff` until it retries, and
        // a capture lands on whichever half of that cycle was running — this one caught
        // the first, the capture before it caught the second, and nothing about rule 3
        // changed in between. NOTES § v1 rule set row 3 names both for exactly this
        // reason, so both are the requirement and only the pair is asserted.
        let waiting = at(
            captured_status(&raw, "containerStatuses", "nope"),
            &["state", "waiting"],
        );
        assert_eq!(reason.as_deref(), Some(captured_str(waiting, &["reason"])));
        assert!(
            matches!(reason.as_deref(), Some("ImagePullBackOff" | "ErrImagePull")),
            "rule 3 fires on those two and nothing else — a fixture in a third state is \
             not a fixture for it: {reason:?}"
        );
        // Verbatim against the capture's own bytes, for the reason the two condition
        // messages are: rule 3 has no evidence but this sentence, and a decode that
        // appended to it or cut it short passes any looser assertion.
        assert_eq!(
            message.as_deref(),
            Some(captured_str(waiting, &["message"]))
        );
        assert!(
            message
                .as_deref()
                .unwrap_or_default()
                .contains("registry.invalid/does-not-exist:v9"),
            "and only the message names the image, which is the whole of what rule 3 \
             tells the user to go and check: {message:?}"
        );
        assert_eq!(c.restarts, 0, "it never started, so it never restarted");
        assert_eq!(c.last_terminated, None);

        let config = pod("config");
        let c = container(&config, "app");
        println!("config: {:?}", c.state);
        let ContainerState::Waiting { reason, message } = &c.state else {
            panic!("a missing ConfigMap leaves the container waiting")
        };
        assert_eq!(reason.as_deref(), Some("CreateContainerConfigError"));
        assert_eq!(
            message.as_deref(),
            Some("configmap \"this-configmap-does-not-exist\" not found"),
            "rule 4 names the object that is missing, and the message is where it is"
        );
    }

    /// D27's blind spot, at the decode: this pod's app container is fine and the init one is
    /// dead, and a snapshot built from `containerStatuses` alone would hand the rules nothing
    /// to fire on. The rules do read both arrays now ([`analyze`]); what this test holds is
    /// the list they read it off.
    #[test]
    fn the_init_container_is_in_the_list_and_marked_as_one() {
        let raw = fixture("init");
        let p = pod("init");
        println!(
            "{:?}",
            p.containers
                .iter()
                .map(|c| (&c.name, c.role))
                .collect::<Vec<_>>()
        );

        assert_eq!(p.containers.len(), 2, "both arrays are read, not just one");

        let migrate = container(&p, "migrate");
        assert_eq!(
            migrate.role,
            ContainerRole::Init,
            "the finding has to be able to say which one is the init one — and this one \
             declares no restartPolicy, so it is an init container and not a sidecar"
        );
        // Out of `initContainerStatuses`, which is the point of the test: the same
        // counter sits in `containerStatuses` for `app`, at 0, so a decode that read the
        // regular list for both containers answers a plausible number here.
        assert_eq!(
            migrate.restarts,
            captured_i32(
                captured_status(&raw, "initContainerStatuses", "migrate"),
                &["restartCount"]
            ),
            "rule 5's counter for an init container comes out of the init array"
        );
        assert!(
            migrate.restarts > 0,
            "an init container that has never restarted is not the D27 blind spot this \
             fixture exists for: got {}",
            migrate.restarts
        );
        assert!(
            matches!(&migrate.state, ContainerState::Waiting { reason, .. }
                if reason.as_deref() == Some("CrashLoopBackOff")),
            "Init:CrashLoopBackOff, got {:?}",
            migrate.state
        );
        assert_eq!(
            migrate.last_terminated.as_ref().map(|t| t.exit_code),
            Some(1)
        );

        let app = container(&p, "app");
        assert_eq!(app.role, ContainerRole::Regular);
        assert!(
            matches!(&app.state, ContainerState::Waiting { reason, .. }
                if reason.as_deref() == Some("PodInitializing")),
            "the app container is waiting on the init one, not broken itself: {:?}",
            app.state
        );
        assert_eq!(
            app.restarts, 0,
            "it is still waiting on the init container, so it has never started and \
             never restarted — and the init container's count above is not this one"
        );
        // The two containers of this pod carry *different* images — the app one was never
        // pulled, so the kubelet echoed the spec's `busybox` back instead of the resolved
        // reference. A decode reading the image off the pod, or off the first status, or
        // off the spec, answers the same string twice here.
        assert_eq!(
            migrate.image, "docker.io/library/busybox:latest",
            "rule 3's action is 'check the image name or the pull secret', so the name is \
             per container and comes from the status"
        );
        assert_eq!(app.image, "busybox");
    }

    /// Rule 10 exists because the scheduler writes both the verdict and the sentence onto
    /// the pod, so no Events watch is needed (D27). Losing the message would leave the
    /// most common beginner question unanswered.
    #[test]
    fn pending_pod_carries_the_schedulers_sentence_and_no_containers() {
        let raw = fixture("pending");
        let p = pod("pending");
        println!("{:?}\n  scheduled: {:?}", p.id, p.scheduled);

        let c = p
            .scheduled
            .as_ref()
            .expect("an unschedulable pod has a PodScheduled condition");
        assert_eq!(c.type_, "PodScheduled");
        assert_eq!(c.status, "False");
        assert_eq!(c.reason.as_deref(), Some("Unschedulable"));
        // Equality against the capture's own bytes, not `starts_with`: D37 says a
        // controller's message is shown verbatim, and a decode that appended or truncated
        // one would pass any looser assertion. The sentence itself is the scheduler's and
        // counts the cluster it ran against — "0/3 … 2 Insufficient cpu" when this fixture
        // was three nodes and a cpu request, "0/4 … didn't match Pod's node
        // affinity/selector" now that it is four and a nodeSelector — so what is pinned
        // here is that it arrives whole, and what rule 10 needs of it is asserted below.
        assert_eq!(
            c.message.as_deref(),
            Some(captured_str(
                captured_condition(&raw, "PodScheduled"),
                &["message"]
            )),
            "the scheduler's own sentence is the finding, word for word"
        );
        assert!(
            c.message
                .as_deref()
                .unwrap_or_default()
                .contains("nodes are available"),
            "rule 10's entire evidence is this sentence — a message that no longer counts \
             the nodes that refused the pod is a fixture rule 10 cannot be written \
             against: {:?}",
            c.message
        );

        assert_eq!(
            p.phase.as_deref(),
            Some("Pending"),
            "a pod no node accepted is Pending, and N5 must not charge its requests to one"
        );
        assert_eq!(p.node, None, "it was never scheduled onto anything");
        assert!(
            p.containers.is_empty(),
            "the kubelet never reported on it, so no container rule can fire: {:?}",
            p.containers
        );
    }

    /// Rule 5's whole point: this pod is Running and Ready and something is still wrong.
    #[test]
    fn a_pod_that_looks_healthy_still_carries_its_restart_count() {
        let raw = fixture("restarts");
        let status = captured_status(&raw, "containerStatuses", "flaky");
        let p = pod("restarts");
        let c = container(&p, "flaky");
        println!(
            "{}: ready={} restarts={} state={:?}",
            c.name, c.ready, c.restarts, c.state
        );

        assert!(c.ready, "it is passing its probes right now");
        // The restart is what makes this pod interesting, and the run that followed it is
        // what this state carries. Rule 5's "restarted 3 times" is aged from
        // `last_terminated`, but "it came back up and has been up since" is only readable
        // here — so the two are asserted against each other below rather than against a
        // pair of literals that were true of one afternoon's cluster.
        assert_eq!(
            c.state,
            ContainerState::Running {
                started_at: Some(captured_time(status, &["state", "running", "startedAt"])),
            }
        );
        assert_eq!(
            c.restarts,
            captured_i32(status, &["restartCount"]),
            "rule 5's whole evidence on a pod that looks healthy"
        );
        assert!(
            (3..10).contains(&c.restarts),
            "this is rule 5's WARN fixture (REQUIREMENTS: ≥3 warns, ≥10 is critical), and \
             a capture that drifted out of that band is a fixture for the other severity \
             or for neither: got {}",
            c.restarts
        );
        let last = c
            .last_terminated
            .as_ref()
            .expect("a pod that restarted has a previous run");
        assert_eq!(
            last.exit_code, 1,
            "and it died with the application's own error code"
        );
        assert!(
            matches!(&c.state, ContainerState::Running { started_at }
                if started_at.as_ref() >= last.finished_at.as_ref()),
            "the run that is up now began after the one that died, or the two states were \
             filled from one another: {:?} vs {:?}",
            c.state,
            last.finished_at
        );
    }

    /// Rule 12 compares the deletion timestamp against the pod's own grace period, never
    /// a constant.
    #[test]
    fn the_terminating_pod_carries_its_deletion_timestamp_and_its_own_grace_period() {
        let raw = fixture("stuck");
        let stuck = pod("stuck");
        println!(
            "stuck: deleted at {:?}, grace {:?}",
            stuck.deletion_timestamp, stuck.grace_period_seconds
        );
        assert_eq!(
            stuck.deletion_timestamp,
            Some(captured_time(&raw, &["metadata", "deletionTimestamp"])),
            "the deadline the apiserver wrote when it accepted the delete — a different \
             instant on every capture, and rule 12 subtracts the grace below from it"
        );
        assert_eq!(
            stuck.grace_period_seconds,
            Some(5),
            "`scripts/broken.yaml` asks for five seconds and the delete granted them; \
             this is the manifest's number, not the cluster's"
        );
        // Rule 12 promises "a finalizer, *or* the kubelet is holding it" — two causes
        // whose actions are nothing alike, and this is the field that decides which.
        // `scripts/broken.yaml` puts this one on the pod on purpose and says the fix is
        // patching it out.
        assert_eq!(
            stuck.finalizers,
            vec!["k8rs.test/never-removed".to_string()],
            "the finding names who is holding the object, and `kubectl describe pod` \
             does not print this at all"
        );

        let healthy = pod("healthy");
        assert_eq!(
            healthy.deletion_timestamp, None,
            "a pod nobody deleted must not look like it is shutting down"
        );
        assert!(
            healthy.finalizers.is_empty(),
            "and an ordinary pod has none, so rule 12 blames the kubelet instead: {:?}",
            healthy.finalizers
        );
        assert_eq!(
            healthy.grace_period_seconds,
            Some(30),
            "no delete has happened, so the value falls back to what the spec asked for"
        );
    }

    /// The two grace fields agree in every capture, so precedence is asserted by taking
    /// the real object and giving it the one thing a cluster can produce and this
    /// capture did not: a `kubectl delete --grace-period=0`.
    #[test]
    fn a_forced_delete_beats_the_grace_period_the_spec_asked_for() {
        let mut object: Pod =
            serde_json::from_value(fixture("stuck")).expect("stuck.json is a Pod");
        object.metadata.deletion_grace_period_seconds = Some(0);
        object
            .spec
            .as_mut()
            .expect("the captured pod has a spec")
            .termination_grace_period_seconds = Some(30);

        let p = PodSnapshot::from(object);
        println!("forced delete: grace {:?}", p.grace_period_seconds);
        assert_eq!(
            p.grace_period_seconds,
            Some(0),
            "rule 12 must not stay quiet for 30 seconds the pod was never granted"
        );
    }

    /// Rule 8 fires on `/`, on a runtime socket or a directory one sits under, or on a
    /// writable mount (NOTES § D78), and the Phase 4 posture report lists the read-only
    /// ones — so the decode carries the fact and not a verdict.
    ///
    /// **One volume, two mounts, and every field of the pair is asserted here**, because
    /// this is the object the three discriminations below are read off: one hostPath
    /// volume mounted twice, narrowed by a `subPath` in one container and read-only in the
    /// other. The values are `scripts/broken.yaml`'s own — a re-capture of the same
    /// manifest produces them again — which is why they are literals where the uid and
    /// the timestamps beside them are not.
    #[test]
    fn the_hostpath_mount_keeps_the_path_and_the_writable_flag() {
        let p = pod("hostpath");
        println!("{:?}", p.host_path_mounts);
        assert_eq!(
            p.host_path_mounts,
            vec![
                HostPathMount {
                    path: "/".to_string(),
                    sub_path: Some("run/containerd".to_string()),
                    sub_path_expr: None,
                    read_only: false,
                    container: "nosy".to_string(),
                },
                HostPathMount {
                    path: "/".to_string(),
                    sub_path: None,
                    sub_path_expr: None,
                    read_only: true,
                    container: "shipper".to_string(),
                },
            ],
            "the node's whole filesystem, writable — both of rule 8's escalators, and \
             the finding has to name the container that has it"
        );

        let healthy = pod("healthy");
        assert!(
            healthy.host_path_mounts.is_empty(),
            "the projected service-account volume is not a hostPath: {:?}",
            healthy.host_path_mounts
        );
    }

    /// Rule 7 is "running but not receiving traffic". A crashlooping container is also
    /// not ready, and it is rule 1's, not rule 7's — the state is what separates them.
    ///
    /// It is also the pod that carries the one field rule 7 needs to *not* fire on a pod
    /// that is merely still booting: the pod's `Ready` condition, and the moment it last
    /// changed. `started` is **not** that field, and this pod is the proof — nothing in
    /// it declares a `startupProbe`, so `started` is true because the container is
    /// running and for no other reason (NOTES § D51).
    #[test]
    fn running_but_not_ready_is_distinguishable_from_waiting() {
        let raw = fixture("readiness");
        let readiness = pod("readiness");
        let c = container(&readiness, "app");
        println!(
            "readiness: ready={} started={} state={:?}\n  pod Ready: {:?}",
            c.ready, c.started, c.state, readiness.ready
        );
        assert_eq!(
            c.state,
            ContainerState::Running {
                started_at: Some(captured_time(
                    captured_status(&raw, "containerStatuses", "app"),
                    &["state", "running", "startedAt"]
                )),
            }
        );
        assert!(
            !c.ready,
            "the readiness probe is failing, so the Service dropped it"
        );
        assert!(
            c.started,
            "no `startupProbe` is declared here, so this is upstream's \"always true when \
             no startupProbe is defined and container is running\" case — it says nothing \
             about whether the container ever served, and a rule 7 built on it would fire \
             on every rolling update. The discrimination is the Ready condition below"
        );

        // The only "not ready since" there is: no container status carries one.
        let ready = readiness
            .ready
            .as_ref()
            .expect("a pod the kubelet has reached carries a Ready condition");
        assert_eq!(ready.type_, "Ready");
        assert_eq!(ready.status, "False");
        assert_eq!(ready.reason.as_deref(), Some("ContainersNotReady"));
        assert_eq!(
            ready.message.as_deref(),
            Some("containers with unready status: [app]")
        );
        assert_eq!(
            ready.last_transition,
            Some(captured_time(
                captured_condition(&raw, "Ready"),
                &["lastTransitionTime"]
            )),
            "without this, rule 7 also describes every rolling update inside its \
             initialDelaySeconds"
        );

        let crashloop = pod("crashloop");
        let c = container(&crashloop, "quitter");
        assert!(!c.ready);
        assert!(
            !matches!(c.state, ContainerState::Running { .. }),
            "rule 7 must not also fire on the crashlooping pod rule 1 already explains: \
             {:?}",
            c.state
        );
    }

    /// The `Ready` condition is picked by name off the same five-entry array `scheduled`
    /// comes from, so the two must not be able to collapse onto one another. The healthy
    /// pod is the discriminator: both of its conditions are `True`, and only the `type_`
    /// and the transition time tell them apart. `broken-pending` is the third shape —
    /// the kubelet never saw it, so `Ready` is absent while `PodScheduled` is present.
    #[test]
    fn ready_and_scheduled_are_two_different_conditions() {
        let raw = fixture("healthy");
        let p = pod("healthy");
        let ready = p.ready.as_ref().expect("a running pod reports Ready");
        let scheduled = p.scheduled.as_ref().expect("and it reports PodScheduled");
        println!("healthy ready={ready:?}\n  scheduled={scheduled:?}");

        assert_eq!(ready.type_, "Ready");
        assert_eq!(scheduled.type_, "PodScheduled");
        assert_eq!(ready.status, "True", "nothing is wrong with this pod");
        assert_ne!(
            ready, scheduled,
            "`Ready` is third in this pod's condition array and `PodScheduled` is fifth; \
             a decode taking the first, or the same one twice, reads identically here — \
             and the first is `PodReadyToStartContainers`, which any substring match on \
             \"Ready\" lands on"
        );
        assert_eq!(
            ready.last_transition,
            Some(captured_time(
                captured_condition(&raw, "Ready"),
                &["lastTransitionTime"]
            )),
            "the moment it started serving — rule 7's clock when it stops"
        );
        assert_eq!(
            scheduled.last_transition,
            Some(captured_time(
                captured_condition(&raw, "PodScheduled"),
                &["lastTransitionTime"]
            )),
            "and the moment a node accepted it, which is a different moment out of a \
             different entry of the same array"
        );

        let pending = pod("pending");
        println!("pending ready={:?}", pending.ready);
        assert_eq!(
            pending.ready, None,
            "no node accepted it, so the kubelet never wrote a Ready condition — and a \
             rule that read a missing condition as `False` would file rule 7 on top of \
             rule 10's answer"
        );
        assert!(
            pending.scheduled.is_some(),
            "while PodScheduled, the one rule 10 reads, is there"
        );
    }

    /// The negative side, and the one that catches false positives: nothing in here may
    /// look like anything a rule fires on.
    #[test]
    fn the_healthy_pod_offers_no_rule_anything_to_fire_on() {
        let raw = fixture("healthy");
        let p = pod("healthy");
        println!("{:?}", p);

        assert_eq!(
            p.containers.len(),
            2,
            "an init container that succeeded, and the app"
        );
        let migrate = container(&p, "migrate");
        assert_eq!(migrate.role, ContainerRole::Init);
        let migrate_status = captured_status(&raw, "initContainerStatuses", "migrate");
        assert_eq!(
            migrate.state,
            ContainerState::Terminated(Terminated {
                reason: Some("Completed".to_string()),
                exit_code: 0,
                started_at: Some(captured_time(
                    migrate_status,
                    &["state", "terminated", "startedAt"]
                )),
                finished_at: Some(captured_time(
                    migrate_status,
                    &["state", "terminated", "finishedAt"]
                )),
                // This container writes nothing and its policy is the default `File`, so
                // there is no termination message to carry — the crashlooping container in
                // `crashloop.json` is where the populated case is asserted.
                message: None,
            }),
            "an init container that finished is terminated with exit 0, not a finding"
        );
        // The pair that kills a hardwired `started`: same pod, same decode, two answers.
        // An init container that ran to completion is not "started" — it is done.
        assert!(!migrate.started);

        let app = container(&p, "app");
        assert!(app.ready);
        assert!(app.started, "and the app container is");
        assert_eq!(
            app.state,
            ContainerState::Running {
                started_at: Some(captured_time(
                    captured_status(&raw, "containerStatuses", "app"),
                    &["state", "running", "startedAt"]
                )),
            }
        );
        assert_eq!(app.restarts, 0);
        assert_eq!(app.last_terminated, None, "it has never died");
        assert_eq!(app.memory_limit.as_deref(), Some("64Mi"));
        assert_eq!(app.cpu_request.as_deref(), Some("10m"));
        // The only container in any capture whose request and limit differ, so it is the
        // only place a request read out of `limits` can be caught. `broken-oom` sets both
        // to 64Mi, which is what a burstable-looking pod that is really guaranteed does.
        assert_eq!(
            app.memory_request.as_deref(),
            Some("16Mi"),
            "N5 sums what was reserved, not what was capped — they are different numbers"
        );
        // The condition does not go away once the pod is scheduled, it flips to True — so
        // rule 10 has to test `status` and `reason`, never "is it there".
        let scheduled = p
            .scheduled
            .as_ref()
            .expect("a scheduled pod keeps the condition, it does not drop it");
        // **This pod is the one that proves the condition was picked by name.** Its five
        // conditions lead with `PodReadyToStartContainers` and end with `PodScheduled`,
        // so a decode that simply took the first would land on the wrong one here — and
        // on `broken-pending`, which carries only `PodScheduled`, it never could.
        assert_eq!(scheduled.type_, "PodScheduled");
        assert_eq!(scheduled.status, "True");
        assert_eq!(
            scheduled.reason, None,
            "nothing refused it, so there is no reason to give"
        );

        assert!(p.node_selector.is_empty());
        assert!(
            !p.mirror,
            "an ordinary pod is not a static one, and N2 counts it as drainable"
        );
        // Not "no tolerations": the admission controller adds these two to every pod in
        // the cluster, so N6 must not read a pod that tolerates nothing as one that
        // tolerates something. Asserted whole — dropping `operator` or `effect` would
        // leave N6 unable to say whether a taint is tolerated, which is its only job.
        assert_eq!(
            p.tolerations,
            vec![
                Toleration {
                    key: Some("node.kubernetes.io/not-ready".to_string()),
                    operator: Some("Exists".to_string()),
                    value: None,
                    effect: Some("NoExecute".to_string()),
                },
                Toleration {
                    key: Some("node.kubernetes.io/unreachable".to_string()),
                    operator: Some("Exists".to_string()),
                    value: None,
                    effect: Some("NoExecute".to_string()),
                },
            ]
        );
    }

    /// N1–N6 all read this object, and N5/N6 join it against the pods.
    ///
    /// **The capture is no longer of a healthy cluster, and that is the point of it.**
    /// `scripts/cluster.sh break-nodes` hands each worker exactly one broken state —
    /// cordoned (N2), `NoExecute`-tainted (N6), kubelet stopped (N1) — one per node so
    /// that no fixture wears two, which is why this reads four nodes where it once read
    /// three and why the "nothing is wrong anywhere" loop it used to end with has been
    /// replaced by a comparison against what the capture actually says.
    #[test]
    fn nodes_decode_with_their_conditions_taints_and_versions() {
        let raw = fixture("nodes");
        let nodes: Vec<NodeSnapshot> = items::<Node>("nodes").into_iter().map(Into::into).collect();
        println!(
            "{:?}",
            nodes
                .iter()
                .map(|n| (&n.id.name, n.unschedulable, &n.kubelet_version))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            nodes.len(),
            raw["items"].as_array().map_or(0, Vec::len),
            "every node in the capture decodes into one snapshot — N2 and N5 are joins \
             over this list, and a node quietly dropped from it is a join that closes \
             over the wrong denominator"
        );
        assert!(
            nodes.len() >= 4,
            "`break-nodes` gives each worker one of the three broken states and needs \
             three workers to do it, so a control plane and three workers is the floor \
             this fixture is captured at: got {}",
            nodes.len()
        );

        let cp = nodes
            .iter()
            .find(|n| n.id.name == "k8rs-control-plane")
            .expect("the control plane is in the capture");
        assert_eq!(cp.id.kind, ObjectKind::Node);
        assert_eq!(
            cp.id.namespace, None,
            "a node is cluster-scoped, and `\"\"` is a lie"
        );
        let cp_raw = captured_item(&raw, "k8rs-control-plane");
        assert_eq!(
            cp.kubelet_version.as_deref(),
            Some(captured_str(
                cp_raw,
                &["status", "nodeInfo", "kubeletVersion"]
            )),
            "N4 compares this"
        );
        assert_eq!(
            cp.kubelet_version.as_deref(),
            Some(
                std::fs::read_to_string(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/K8S_VERSION"
                ))
                .expect("the capture stamps the version it came from")
                .trim()
            ),
            "N4 is 'this kubelet is behind the API server', and on this cluster nothing \
             is — a fixture where they already disagree is N4's positive case, and it \
             would want saying out loud rather than arriving by accident"
        );
        // Allocatable is the machine the capture ran on and moves with it — the first
        // capture came off a 4-CPU box and this one off the dev machine.
        assert_eq!(
            cp.allocatable_cpu.as_deref(),
            Some(captured_str(cp_raw, &["status", "allocatable", "cpu"])),
            "N5 measures against this"
        );
        assert_eq!(
            cp.allocatable_memory.as_deref(),
            Some(captured_str(cp_raw, &["status", "allocatable", "memory"]))
        );
        assert_eq!(
            cp.taints,
            vec![Taint {
                key: "node-role.kubernetes.io/control-plane".to_string(),
                value: None,
                effect: "NoSchedule".to_string(),
                added_at: None,
            }],
            "N6 explains a Pending pod with this"
        );
        assert_eq!(
            cp.labels.get("kubernetes.io/hostname").map(String::as_str),
            Some("k8rs-control-plane"),
            "N6 matches a pod's nodeSelector against the labels"
        );

        let ready = cp
            .conditions
            .iter()
            .find(|c| c.type_ == "Ready")
            .expect("N1 reads the Ready condition");
        assert_eq!(ready.status, "True");
        assert_eq!(
            ready.last_transition,
            Some(captured_time(
                captured_condition(cp_raw, "Ready"),
                &["lastTransitionTime"]
            )),
            "N1 needs how long it has been that way, not just that it is"
        );

        // **What the decode says about every node is what the capture says**, condition by
        // condition and cordon by cordon. This replaces an assertion that the whole
        // cluster was healthy: that was true of the first capture and is deliberately
        // false of this one, but the defect it was written against — a decode that
        // invented a pressure, dropped a condition or answered one node's for another's —
        // is caught by the comparison and not by the optimism.
        let decoded: Vec<(&str, &str, &str)> = nodes
            .iter()
            .flat_map(|n| {
                n.conditions
                    .iter()
                    .map(|c| (n.id.name.as_str(), c.type_.as_str(), c.status.as_str()))
            })
            .collect();
        let captured: Vec<(&str, &str, &str)> = raw["items"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|n| {
                n["status"]["conditions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| {
                        (
                            captured_str(n, &["metadata", "name"]),
                            captured_str(c, &["type"]),
                            captured_str(c, &["status"]),
                        )
                    })
            })
            .collect();
        println!("{decoded:?}");
        assert_eq!(
            decoded, captured,
            "N1 and N3 read these, and the decode may neither invent one nor lose one"
        );

        let cordoned: Vec<&str> = nodes
            .iter()
            .filter(|n| n.unschedulable)
            .map(|n| n.id.name.as_str())
            .collect();
        let captured_cordoned: Vec<&str> = raw["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|n| n["spec"]["unschedulable"] == true)
            .map(|n| captured_str(n, &["metadata", "name"]))
            .collect();
        println!("cordoned: {cordoned:?}");
        assert_eq!(
            cordoned, captured_cordoned,
            "N2 is the whole of 'cordoned and forgotten', and this field is all of it — \
             an absent key is a schedulable node, never a cordoned one"
        );

        // The three states `break-nodes` puts on the cluster, asserted as three because a
        // capture that produced only two of them is a fixture the N-series cannot be
        // written against — and every one of them reads as "healthy" to a decode that
        // dropped the field it lives in.
        assert_eq!(
            cordoned.len(),
            1,
            "one worker is cordoned and still carrying pods (N2's positive): {cordoned:?}"
        );
        let not_ready: Vec<&str> = nodes
            .iter()
            .filter(|n| {
                n.conditions
                    .iter()
                    .any(|c| c.type_ == "Ready" && c.status != "True")
            })
            .map(|n| n.id.name.as_str())
            .collect();
        println!("not ready: {not_ready:?}");
        assert_eq!(
            not_ready.len(),
            1,
            "and one worker's kubelet has stopped posting (N1's positive): {not_ready:?}"
        );
        let gone = nodes
            .iter()
            .find(|n| n.id.name == not_ready[0])
            .expect("just found by name");
        assert_eq!(
            gone.conditions
                .iter()
                .find(|c| c.type_ == "Ready")
                .map(|c| c.status.as_str()),
            Some("Unknown"),
            "`Unknown` is a kubelet that went quiet and `False` is one that answered and \
             said no — N1 must be able to tell the operator which of the two happened"
        );
        let tainted: Vec<&Taint> = nodes
            .iter()
            .flat_map(|n| &n.taints)
            .filter(|t| t.effect == "NoExecute" && t.value.is_some())
            .collect();
        println!("valued NoExecute taints: {tainted:?}");
        // Asserted whole, because N6's job is to name the taint that is blocking a pod and
        // `dedicated=gpu:NoExecute` is three fields, not one. This is also the **only
        // captured proof that `value` survives at all**: every other taint in the capture
        // was written by the node controller, which gives its taints no value, so the
        // control plane's `value: None` above is an absence and this is the presence.
        // `cluster.sh break-nodes` applies it, so the string is the script's, not the
        // cluster's.
        let operators = Taint {
            key: "dedicated".to_string(),
            value: Some("gpu".to_string()),
            effect: "NoExecute".to_string(),
            // Absent, and that is the split: a hand-applied taint carries no `timeAdded`
            // (see [`Taint::added_at`]), which is why the pair of fields together can only
            // be asserted on a synthesis.
            added_at: None,
        };
        assert_eq!(
            tainted,
            vec![&operators],
            "and one worker carries an operator's own `dedicated=gpu:NoExecute` (N6's \
             positive, and the only taint in the capture with a value)"
        );

        // N3's positive is `True`, and this capture has none: the unreachable node's
        // pressures are `Unknown`, which is N1's answer and not N3's. A decode that read
        // an absent or unknown condition as a pressure would file evictions-are-coming on
        // a node nobody can reach.
        for n in &nodes {
            for c in &n.conditions {
                let pressured = matches!(
                    c.type_.as_str(),
                    "MemoryPressure" | "DiskPressure" | "PIDPressure"
                ) && c.status == "True";
                assert!(
                    !pressured,
                    "{} reported {} = {} (N3)",
                    n.id.name, c.type_, c.status
                );
            }
        }
    }

    /// W1: the pods were never created, so the only object that knows anything is the
    /// ReplicaSet. It is also the one capture in the repo with a real `ownerReference`,
    /// so it is what proves the owner decode reads one at all.
    #[test]
    fn the_failed_replicaset_carries_the_quota_message_and_files_under_its_deployment() {
        let rs: Vec<WorkloadSnapshot> = items::<ReplicaSet>("quota-replicasets")
            .into_iter()
            .map(Into::into)
            .collect();
        let rs = rs.first().expect("the quota namespace has one ReplicaSet");
        println!(
            "{:?}\n  owner: {:?}\n  {:?}",
            rs.id, rs.owner, rs.conditions
        );

        assert_eq!(rs.id.kind, ObjectKind::ReplicaSet);
        assert_eq!(rs.id.name, "broken-quota-59654c756");
        assert_eq!(rs.id.namespace.as_deref(), Some("k8rs-quota"));

        assert_eq!(
            rs.owner.kind,
            ObjectKind::Deployment,
            "the chain goes up one step"
        );
        assert_eq!(
            rs.owner.name, "broken-quota",
            "the card reads the name the user deployed, not the hashed one (D28)"
        );
        let raw = fixture("quota-replicasets");
        let controller = raw["items"][0]["metadata"]["ownerReferences"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|o| o["controller"] == true)
            .expect("the capture's ReplicaSet is controlled by its Deployment");
        assert_eq!(
            rs.owner.uid.as_deref(),
            Some(captured_str(controller, &["uid"]))
        );
        assert_ne!(
            rs.owner.uid, rs.id.uid,
            "the owner's own uid and never the object's — the group agrees on the \
             owner's (D39), and the two are one field apart in the JSON"
        );
        assert_eq!(
            rs.owner.namespace, rs.id.namespace,
            "an ownerReference carries no namespace; the owner is in the object's own"
        );

        assert_eq!(rs.desired, Some(1));
        assert_eq!(
            rs.ready, None,
            "not one pod exists — this is the blind spot D28 closed"
        );
        let failure = rs
            .conditions
            .iter()
            .find(|c| c.type_ == "ReplicaFailure")
            .expect("W1 reads ReplicaFailure");
        assert_eq!(failure.status, "True");
        assert_eq!(failure.reason.as_deref(), Some("FailedCreate"));
        // Verbatim means verbatim (D37) — `contains` would pass on a message the decode
        // had appended to or cut short, and this one is the whole of W1's evidence. The
        // sentence names the pod the ReplicaSet tried to create, whose generated suffix is
        // new on every capture, so the comparison is against the capture's own bytes and
        // what W1 needs of them is asserted beside it.
        assert_eq!(
            failure.message.as_deref(),
            Some(captured_str(
                captured_condition(&raw["items"][0], "ReplicaFailure"),
                &["message"]
            )),
            "W1 shows the API server's own refusal, word for word"
        );
        assert!(
            failure
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("exceeded quota: deny-all-pods"),
            "and the refusal has to still name the quota that refused it, or W1's finding \
             sends the operator looking for a broken image: {:?}",
            failure.message
        );

        // The negative side: a ReplicaSet that worked has no such condition at all.
        let healthy: Vec<WorkloadSnapshot> = items::<ReplicaSet>("healthy-replicasets")
            .into_iter()
            .map(Into::into)
            .collect();
        let healthy = healthy
            .first()
            .expect("the healthy Deployment has one ReplicaSet");
        assert_eq!(healthy.owner.name, "healthy-deploy");
        assert_eq!((healthy.desired, healthy.ready), (Some(2), Some(2)));
        assert!(
            healthy.conditions.is_empty(),
            "no failure means no condition, not a condition saying False: {:?}",
            healthy.conditions
        );
    }

    /// W2 reads a Deployment's `Progressing`; a DaemonSet counts its pods somewhere else
    /// entirely, which is the whole reason four kinds get four decodes.
    #[test]
    fn deployments_and_daemonsets_decode_their_own_desired_and_ready() {
        let deployments: Vec<WorkloadSnapshot> = items::<Deployment>("deployments")
            .into_iter()
            .map(Into::into)
            .collect();
        println!(
            "{:?}",
            deployments
                .iter()
                .map(|w| (&w.id.name, w.desired, w.ready))
                .collect::<Vec<_>>()
        );

        let broken = deployments
            .iter()
            .find(|w| w.id.name == "broken-quota")
            .expect("the quota Deployment is in the capture");
        assert_eq!(broken.id.kind, ObjectKind::Deployment);
        assert_eq!(
            broken.owner, broken.id,
            "nothing controls a Deployment, so it is its own card"
        );
        assert_eq!((broken.desired, broken.ready), (Some(1), None));
        // **That `None` is a zero, and the capture is what says so.** `readyReplicas` is
        // `omitempty` on a plain `int32`, so it is absent exactly when it is 0 — provable
        // here from the counter the API server *did* write: one replica wanted, one
        // unavailable, therefore none ready. Asserted rather than described, because a
        // W2 reading `None` as "no number" would skip precisely this Deployment.
        let raw = fixture("deployments");
        let status = &raw["items"]
            .as_array()
            .expect("deployments.json has an items array")
            .iter()
            .find(|w| w["metadata"]["name"] == "broken-quota")
            .expect("the quota Deployment is in the capture")["status"];
        let (ready_key, unavailable) =
            (status.get("readyReplicas"), &status["unavailableReplicas"]);
        println!("broken-quota: readyReplicas={ready_key:?} unavailableReplicas={unavailable}");
        assert!(
            ready_key.is_none() && *unavailable == 1,
            "the shape `ready: None` means zero rests on: no readyReplicas key, and one \
             of one replica unavailable — got {ready_key:?} and {unavailable}"
        );
        let progressing = broken
            .conditions
            .iter()
            .find(|c| c.type_ == "Progressing")
            .expect("W2 reads Progressing");
        assert_eq!(progressing.status, "False");
        assert_eq!(
            progressing.reason.as_deref(),
            Some("ProgressDeadlineExceeded"),
            "W2 fires on this reason and no other"
        );

        // The negative side of W2, from the same capture.
        let healthy = deployments
            .iter()
            .find(|w| w.id.name == "healthy-deploy")
            .expect("the healthy Deployment is in the capture");
        assert_eq!((healthy.desired, healthy.ready), (Some(2), Some(2)));
        assert_eq!(
            healthy
                .conditions
                .iter()
                .find(|c| c.type_ == "Progressing")
                .and_then(|c| c.reason.as_deref()),
            Some("NewReplicaSetAvailable"),
            "a rollout that finished must not look like one that gave up"
        );

        let daemonsets: Vec<WorkloadSnapshot> = items::<DaemonSet>("daemonsets")
            .into_iter()
            .map(Into::into)
            .collect();
        let kindnet = daemonsets
            .iter()
            .find(|w| w.id.name == "kindnet")
            .expect("kind runs kindnet on every node");
        println!(
            "kindnet: desired={:?} ready={:?}",
            kindnet.desired, kindnet.ready
        );
        assert_eq!(kindnet.id.kind, ObjectKind::DaemonSet);
        assert_eq!(kindnet.id.namespace.as_deref(), Some("kube-system"));
        // **Both numbers out of the DaemonSet's own status keys.** A DaemonSet has no
        // `spec.replicas` to read and wants one pod per matching node, so the pair is the
        // size of the cluster and moves with it — it was `(3, 3)` while the fixture
        // cluster had two workers and is `(4, 4)` now that `break-nodes` needs three. What
        // must not move is which field each comes out of, and that is what is asserted;
        // that neither is a *neighbouring* counter is
        // `desired_and_ready_are_read_from_their_own_fields_and_not_a_neighbour`.
        let raw = fixture("daemonsets");
        let kindnet_raw = captured_item(&raw, "kindnet");
        assert_eq!(
            (kindnet.desired, kindnet.ready),
            (
                Some(captured_i32(
                    kindnet_raw,
                    &["status", "desiredNumberScheduled"]
                )),
                Some(captured_i32(kindnet_raw, &["status", "numberReady"])),
            ),
            "a DaemonSet has no spec.replicas: both numbers come from its status"
        );
    }

    /// Every pod capture in the repository. Named once because two tests read the same
    /// set — the join below and the pin guard — and a second copy is a second list to
    /// keep in step with `tests/fixtures`.
    const CAPTURED_PODS: [&str; 12] = [
        "crashloop",
        "oom",
        "image",
        "config",
        "pending",
        "hostpath",
        "readiness",
        "restarts",
        "nolimits",
        "stuck",
        "init",
        "healthy",
    ];

    /// The snapshot a rule would be handed if it ran over the whole committed capture at
    /// once: every pod, every node, the Deployments, and the pinned [`now`](now).
    fn fixture_snapshot() -> ClusterSnapshot {
        ClusterSnapshot {
            now: now(),
            pods: CAPTURED_PODS.iter().map(|n| pod(n)).collect(),
            nodes: items::<Node>("nodes").into_iter().map(Into::into).collect(),
            workloads: items::<Deployment>("deployments")
                .into_iter()
                .map(Into::into)
                .collect(),
            server_version: Some(
                std::fs::read_to_string(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/K8S_VERSION"
                ))
                .expect("the capture stamps the version it came from")
                .trim()
                .to_string(),
            ),
            context: Some("kind-k8rs".to_string()),
            client_certificate: Some(certificate("healthy-client")),
            // `just fixtures` captures `-A`, so this snapshot covers the whole cluster
            // and N2 and N5 are allowed to run over it.
            namespace_scope: None,
        }
    }

    /// The snapshot is what a rule is handed, and N5 and N6 are joins across it — so the
    /// join has to close. `scripts/sanitize.jq` refuses to rewrite node names for exactly
    /// this reason, and this is the assertion that would notice if it ever did.
    #[test]
    fn a_cluster_snapshot_joins_its_pods_to_the_nodes_they_run_on() {
        let snapshot = fixture_snapshot();
        println!(
            "{} pods, {} nodes, {} workloads, server {:?}",
            snapshot.pods.len(),
            snapshot.nodes.len(),
            snapshot.workloads.len(),
            snapshot.server_version
        );

        assert_eq!(snapshot.pods.len(), CAPTURED_PODS.len());
        assert_eq!(
            snapshot.server_version.as_deref(),
            Some("v1.36.1"),
            "N4 compares the kubelets against this"
        );

        let mut scheduled = 0;
        for p in &snapshot.pods {
            let Some(node) = &p.node else { continue };
            scheduled += 1;
            assert!(
                snapshot.nodes.iter().any(|n| &n.id.name == node),
                "{} says it runs on {node}, which is in no NodeSnapshot — N5 and N6 cannot join",
                p.id.name
            );
        }
        assert_eq!(
            scheduled,
            CAPTURED_PODS.len() - 1,
            "every captured pod but broken-pending was scheduled onto a node"
        );
    }

    /// One swept timestamp: the field it came from, the value, and the grace that has to
    /// come back off it before it names a moment.
    ///
    /// **The grace is `Some` for exactly one field, and that is the whole point of the third
    /// slot.** Every other label is a moment that has already happened, so the value *is*
    /// the moment. [`PodSnapshot::deletion_timestamp`] is a **deadline** — request time
    /// *plus* grace — so it legitimately points at the future for a pod inside its grace
    /// period, and comparing it against `now` would reject that pod and blame the clock.
    type Swept<'a> = (&'static str, &'a Time, Option<i64>);

    /// One entry per `Some`, labelled with the field it came from, and no grace — every
    /// caller of this is a field whose value is already a moment. A `None` contributes
    /// nothing at all, which is exactly why the labels are asserted separately below.
    fn labelled<'a>(out: &mut Vec<Swept<'a>>, label: &'static str, t: Option<&'a Time>) {
        out.extend(t.map(|t| (label, t, None)));
    }

    /// `Terminated` hangs in two places — a container's current state, and the run before
    /// this one — and **the label pair comes from the caller so those two walks are named
    /// apart**. Sharing one pair made either walk satisfy the set on its own: deleting the
    /// `ContainerState::Terminated` arm lost 2 timestamps and deleting the
    /// `last_terminated` walk lost 8, both silently green, because the other walk kept
    /// filling the same two labels.
    fn terminated_times<'a>(
        out: &mut Vec<Swept<'a>>,
        started: &'static str,
        finished: &'static str,
        t: &'a Terminated,
    ) {
        labelled(out, started, t.started_at.as_ref());
        labelled(out, finished, t.finished_at.as_ref());
    }

    /// Every `Time` a [`ClusterSnapshot`] exposes, each carrying the name of the field it
    /// was read out of.
    fn snapshot_times(s: &ClusterSnapshot) -> Vec<Swept<'_>> {
        let mut out = Vec::new();
        for p in &s.pods {
            // Both or neither. The apiserver writes `deletionTimestamp` and the grace it
            // granted in the same accepted delete, so a deadline with no grace beside it
            // is not a shape it produces — and if one ever arrives, the label goes
            // unreached and the coverage assertion names it, which is louder than
            // guessing a grace of zero and asserting the deadline itself.
            if let (Some(dt), Some(grace)) = (&p.deletion_timestamp, p.grace_period_seconds) {
                out.push(("pod.deletion_timestamp", dt, Some(grace)));
            }
            labelled(
                &mut out,
                "pod.creation_timestamp",
                p.creation_timestamp.as_ref(),
            );
            for c in [&p.scheduled, &p.ready, &p.ready_to_start_containers]
                .into_iter()
                .flatten()
            {
                labelled(
                    &mut out,
                    "pod.condition.last_transition",
                    c.last_transition.as_ref(),
                );
            }
            for c in &p.containers {
                match &c.state {
                    ContainerState::Running { started_at } => labelled(
                        &mut out,
                        "container.state.running.started_at",
                        started_at.as_ref(),
                    ),
                    ContainerState::Terminated(t) => terminated_times(
                        &mut out,
                        "container.state.terminated.started_at",
                        "container.state.terminated.finished_at",
                        t,
                    ),
                    // A waiting container has no time of its own: it is not running, and
                    // the run that ended is `last_terminated` below.
                    ContainerState::Waiting { .. } => {}
                }
                if let Some(t) = &c.last_terminated {
                    terminated_times(
                        &mut out,
                        "container.last_terminated.started_at",
                        "container.last_terminated.finished_at",
                        t,
                    );
                }
            }
        }
        for n in &s.nodes {
            for c in &n.conditions {
                labelled(
                    &mut out,
                    "node.condition.last_transition",
                    c.last_transition.as_ref(),
                );
            }
            for t in &n.taints {
                labelled(&mut out, "node.taint.added_at", t.added_at.as_ref());
            }
        }
        for w in &s.workloads {
            for c in &w.conditions {
                labelled(
                    &mut out,
                    "workload.condition.last_transition",
                    c.last_transition.as_ref(),
                );
            }
        }
        out
    }

    /// **A pin behind the timestamps the snapshot exposes makes every duration in the suite
    /// run backwards, and nothing else here would notice.** `now` is the user's laptop and
    /// the fixture timestamps are the API server's, so an early pin enters D55's *slow* half
    /// permanently and by construction: rule 12 computes "asked to shut down in 43 minutes",
    /// and the renderer draws the whole suite as "just now". Every other assertion in this
    /// file reads a field rather than subtracting two times, so this is the only place the
    /// pin can be wrong out loud.
    ///
    /// **The sweep is labelled, not counted**, because a bare total cannot tell every field
    /// walked once from one field walked ninety-six times, and a sweep that reached nothing
    /// prints the same green line as one with nothing to reach (CLAUDE.md — a derived list
    /// asserts it found something). Each walk is named separately: deleting any one turns
    /// this red.
    ///
    /// **What it does *not* buy is a guard against a new field.** A new *variant* is caught
    /// by the compiler — the sweep's `match &c.state` is exhaustive — but a new **field** is
    /// caught by nothing: adding a `Time` to [`PodSnapshot`] and decoding it leaves this
    /// green on the labels it already had. **A box that adds a `Time` to these types adds
    /// its walk here in the same change**, and no mechanism will remind it.
    ///
    /// **This is a guard over the contract, not over the captures**, and four fields sit in
    /// the gap: `metadata.creationTimestamp` on every object that is not a Pod, a pod's
    /// `status.startTime`, and the two [`Condition`] keeps no room for —
    /// `NodeCondition.lastHeartbeatTime` and `DeploymentCondition.lastUpdateTime`. All four
    /// sit before the pin today and nothing asserts that they do; NOTES § D42 lets Phase 4
    /// add any of them.
    #[test]
    fn the_pinned_now_is_not_before_the_captures_it_is_read_against() {
        let snapshot = fixture_snapshot();
        let times = snapshot_times(&snapshot);
        let reached: BTreeSet<&str> = times.iter().map(|&(label, _, _)| label).collect();
        println!(
            "now {:?}\n  {} timestamps, newest {:?}\n  fields reached: {reached:?}",
            snapshot.now,
            times.len(),
            times.iter().map(|&(_, t, _)| t).max(),
        );

        // A superset, not an equality: reaching *more* than this is a new walk over a field
        // the captures started filling, which is right and must not be a red build.
        // Reaching *less* is the defect, and that is all this asks about.
        let expected = BTreeSet::from([
            "container.last_terminated.finished_at",
            "container.last_terminated.started_at",
            "container.state.running.started_at",
            "container.state.terminated.finished_at",
            "container.state.terminated.started_at",
            "node.condition.last_transition",
            "node.taint.added_at",
            "pod.condition.last_transition",
            "pod.creation_timestamp",
            "pod.deletion_timestamp",
            "workload.condition.last_transition",
        ]);
        assert!(
            reached.is_superset(&expected),
            "the sweep no longer reaches {:?} — every Time field the captures fill has to \
             be walked, because a sweep that reached nothing prints the same green line as \
             one with nothing to reach, and the loop below is just as quiet either way",
            expected.difference(&reached).collect::<Vec<_>>()
        );

        for &(label, t, grace) in &times {
            // What has to be in the past is the moment the thing *happened*. For ten of
            // the eleven labels that is the value; for the deadline it is the value minus
            // the grace it was granted — D46's `asked_at`.
            let moment = match grace {
                None => t.0,
                // Checked, and the failure is named rather than skipped: a grace this
                // subtraction cannot represent is reachable from the cluster, not
                // theoretical — v1.36.1 accepted `terminationGracePeriodSeconds:
                // 9223372036854775807` in a server-side dry-run (NOTES § D56).
                Some(g) => {
                    t.0.checked_sub(SignedDuration::from_secs(g))
                        .unwrap_or_else(|e| {
                            panic!(
                                "{label} is {t:?} with a grace of {g}s, and taking the grace \
                             back off it cannot be represented: {e}\n  \
                             `grep -rl {g} tests/fixtures` names the capture, if one carries it."
                            )
                        })
                }
            };
            assert!(
                moment <= snapshot.now.0,
                "{label} puts its moment at {moment}, after the pinned now {:?}.\n  \
                 If `just fixtures` was just re-run, the pin moved out from under this \
                 and the captures are simply newer than it — repin `fn now()` (see the \
                 note there for what moves with it).\n  \
                 Otherwise it is what it looks like: a clock behind the cluster's, whose \
                 negative ages D18 renders as \"just now\".",
                snapshot.now
            );
        }
    }

    /// **The two snapshots that decode identically and mean opposite things** (D43). Without
    /// this field "a small cluster" and "one namespace of a big one" are the same value, and
    /// N2 counts zero on a cordoned node whose 40 pods are all outside the scope — a missing
    /// finding with nothing on the screen to show it happened. One value, two producers:
    /// `--namespace` and the 403 fallback, which are the same fact to a rule.
    #[test]
    fn the_snapshot_says_whether_its_pod_list_covers_the_whole_cluster() {
        let nodes: Vec<NodeSnapshot> = items::<Node>("nodes").into_iter().map(Into::into).collect();
        let one_namespace: Vec<PodSnapshot> =
            ["crashloop", "healthy"].iter().map(|n| pod(n)).collect();

        // Same pods, same nodes; every other field of the two snapshots is equal.
        let whole_cluster = ClusterSnapshot {
            now: now(),
            pods: one_namespace.clone(),
            nodes: nodes.clone(),
            workloads: Vec::new(),
            server_version: None,
            context: None,
            client_certificate: None,
            namespace_scope: None,
        };
        let scoped = ClusterSnapshot {
            namespace_scope: Some("default".to_string()),
            ..whole_cluster.clone()
        };
        println!(
            "{:?} vs {:?}",
            whole_cluster.namespace_scope, scoped.namespace_scope
        );

        assert_ne!(
            whole_cluster, scoped,
            "a two-pod cluster and a two-pod view of a large one must not be one value"
        );
        assert_eq!(
            scoped.namespace_scope.as_deref(),
            Some("default"),
            "N2 and N5 name the namespace they were limited to when they switch off"
        );
        assert_eq!(
            whole_cluster.namespace_scope, None,
            "`None` is the whole cluster — never an empty string, which is a namespace \
             nothing is in"
        );
    }

    /// **C1's input arrives on the snapshot like every other rule's.** `analyze(&Snapshot)
    /// -> Vec<Finding>` is the whole signature [invariant 5](CLAUDE.md) describes, so
    /// "PEM bytes in, finding out" would have been a second entry point — an amendment to
    /// a hard invariant, which is a stop rather than a convenience (D51). C1 is the one
    /// finding with no API object behind it, so its identity is built here to show the
    /// context name is enough to make one.
    ///
    /// The certificate assertion is the security half and it is not decoration: this
    /// field carries the certificate and **nothing else off the kubeconfig**, and a
    /// private key arriving in it is a credential copied into our own types, one `Debug`
    /// away from a backtrace.
    #[test]
    fn the_snapshot_carries_c1s_certificate_and_the_context_name_it_files_under() {
        let snapshot = ClusterSnapshot {
            now: now(),
            pods: Vec::new(),
            nodes: Vec::new(),
            workloads: Vec::new(),
            server_version: None,
            context: Some("kind-k8rs".to_string()),
            client_certificate: Some(certificate("expiring-client")),
            namespace_scope: None,
        };

        let pem = snapshot
            .client_certificate
            .as_deref()
            .expect("the kubeconfig authenticates with a client certificate");
        let text = String::from_utf8(pem.to_vec()).expect("a PEM file is text");
        println!(
            "context {:?}, {} bytes of PEM starting {:?}",
            snapshot.context,
            pem.len(),
            text.lines().next()
        );
        assert!(
            text.starts_with("-----BEGIN CERTIFICATE-----"),
            "C1 parses PEM, so the bytes arrive as they sit on disk"
        );
        assert!(
            !text.contains("PRIVATE KEY"),
            "the certificate and nothing else: a key in this field is a credential in \
             our own struct, and `scripts/make-certs.sh` deletes them for that reason"
        );

        // The identity C1 files under — the one `None` uid in the product, and a kind
        // that names what the thing is because there is no API object to name.
        let id = ObjectId {
            kind: ObjectKind::Other("kubeconfig".to_string()),
            namespace: None,
            name: snapshot
                .context
                .clone()
                .expect("the kubeconfig names a current context"),
            uid: None,
        };
        println!("C1 files under {:?}", id.group_key());
        assert_eq!(
            id.group_key(),
            (
                &ObjectKind::Other("kubeconfig".to_string()),
                None,
                "kind-k8rs"
            ),
            "the card is named after the context the user recognises, not after a file path"
        );

        // The other authentication paths — a token, an exec plugin, OIDC — leave nothing
        // to parse, and C1 says nothing rather than guessing.
        let token_auth = ClusterSnapshot {
            client_certificate: None,
            ..snapshot.clone()
        };
        assert_ne!(
            token_auth, snapshot,
            "a cluster reached with a token is not the same snapshot as one reached with \
             a certificate — C1 fires on exactly one of them"
        );
    }

    /// A lookup table, so it is tested as one. Eight of the nine branches are unreachable
    /// from any committed capture — every fixture object is a Pod, a Node, a Deployment,
    /// a ReplicaSet or a DaemonSet — and an unreachable branch is one nobody would notice
    /// going wrong.
    #[test]
    fn every_kind_this_file_has_a_branch_for_maps_to_it() {
        for (api_version, kind, expected) in [
            ("apps/v1", "Deployment", ObjectKind::Deployment),
            ("apps/v1", "StatefulSet", ObjectKind::StatefulSet),
            ("apps/v1", "DaemonSet", ObjectKind::DaemonSet),
            ("apps/v1", "ReplicaSet", ObjectKind::ReplicaSet),
            ("batch/v1", "Job", ObjectKind::Job),
            ("batch/v1", "CronJob", ObjectKind::CronJob),
            ("v1", "Node", ObjectKind::Node),
            ("v1", "Pod", ObjectKind::Pod),
        ] {
            let got = ObjectKind::from_api(api_version, kind);
            println!("{api_version} {kind} -> {got:?}");
            assert_eq!(got, expected);
        }

        // The version is not the identity: a type is named by its group and its kind, and
        // `apps/v1beta1` is the same StatefulSet as `apps/v1`.
        assert_eq!(
            ObjectKind::from_api("apps/v1beta1", "StatefulSet"),
            ObjectKind::StatefulSet,
            "an older serialisation of a built-in kind is still that kind"
        );

        // The owner chain genuinely stops at kinds this project has no branch for, and
        // the kind has to survive as text rather than collapse onto a wrong variant.
        assert_eq!(
            ObjectKind::from_api("argoproj.io/v1alpha1", "Rollout"),
            ObjectKind::Other("Rollout.argoproj.io".to_string()),
            "an Argo Rollout is a real owner, and inventing a variant for it is per-kind code"
        );
        assert_ne!(
            ObjectKind::from_api("apps/v1", "deployment"),
            ObjectKind::Deployment,
            "the API sends TitleCase kinds; matching loosely would map an unknown kind onto a real one"
        );

        // The core group is written `v1` and has no group *name*, so a core kind with no
        // branch here has nothing to qualify it with and must stay bare. A
        // ReplicationController is a real, still-supported `controller: true` pod owner,
        // and `ReplicationController.` on a card is a dangling dot in front of a user
        // (invariant 14). Phase 4's drain report files findings about
        // PersistentVolumeClaims and PodDisruptionBudgets, which take this same arm.
        assert_eq!(
            ObjectKind::from_api("v1", "ReplicationController"),
            ObjectKind::Other("ReplicationController".to_string()),
            "a core-group kind keeps its bare name — there is no group to append"
        );
    }

    /// **The kind string alone does not name a kind, and the write path is what pays for
    /// that.** OpenKruise is deliberately drop-in: its Advanced StatefulSet is
    /// `apps.kruise.io/v1beta1, Kind: StatefulSet`, its Advanced DaemonSet
    /// `apps.kruise.io/v1alpha1, Kind: DaemonSet`, and Volcano's Job is
    /// `batch.volcano.sh/v1alpha1` — each one decodes as the built-in variant if only the
    /// kind is read. The card lying is the small half; the large half is Phase 7 aiming
    /// `scale` at `apps/v1 statefulsets/<name>`, which is a 404 or, worse, a different
    /// object that happens to share the name.
    ///
    /// The last case is the same question from the other side: `Node` in somebody's CRD
    /// group is an ordinary owner, not the kubelet's mirror-pod reference, so the group
    /// has to gate the *discard* as well as the variant.
    /// **Capture trip:** none — installing OpenKruise on the fixture cluster to
    /// photograph one owner reference is a cluster change, not a fixture.
    #[test]
    fn the_api_group_decides_which_kind_an_owner_reference_names() {
        let owned_by = |api_version: &str, kind: &str| {
            let mut object: Pod =
                serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
            object.metadata.owner_references = Some(vec![OwnerReference {
                api_version: api_version.to_string(),
                kind: kind.to_string(),
                name: "web".to_string(),
                uid: "5c8a1f37-2b6d-4a90-8e11-77c2d4b0a913".to_string(),
                controller: Some(true),
                block_owner_deletion: Some(true),
            }]);
            PodSnapshot::from(object)
        };

        let kruise = owned_by("apps.kruise.io/v1beta1", "StatefulSet");
        println!("kruise owner: {:?} mirror: {}", kruise.owner, kruise.mirror);
        assert_ne!(
            kruise.owner.kind,
            ObjectKind::StatefulSet,
            "an Advanced StatefulSet is not an apps/v1 one — Phase 7 would scale the \
             wrong object, or nothing at all"
        );
        assert_eq!(
            kruise.owner.kind,
            ObjectKind::Other("StatefulSet.apps.kruise.io".to_string()),
            "and the group survives, so a card can say which StatefulSet this is"
        );
        assert_eq!(kruise.owner.name, "web", "it is still a real owner");

        let volcano = owned_by("batch.volcano.sh/v1alpha1", "Job");
        println!("volcano owner: {:?}", volcano.owner);
        assert_eq!(
            volcano.owner.kind,
            ObjectKind::Other("Job.batch.volcano.sh".to_string()),
            "a Volcano Job is not a batch/v1 Job either"
        );

        let crd_node = owned_by("example.com/v1", "Node");
        println!(
            "crd node owner: {:?} mirror: {}",
            crd_node.owner, crd_node.mirror
        );
        assert!(
            !crd_node.mirror,
            "only the core `v1` Node is the kubelet's own reference; a CRD that happens \
             to be called Node does not make this a static pod, and N2 must still count \
             a pod a drain would move"
        );
        assert_eq!(
            crd_node.owner.kind,
            ObjectKind::Other("Node.example.com".to_string()),
            "so it is kept as an ordinary owner instead of being discarded"
        );
    }

    // --- ONE FIELD CHANGED ON A REAL CAPTURE ---
    //
    // Everything below starts from a committed capture and changes exactly one field, or
    // one group of fields that a single `kubectl` action changes together, to a value the
    // API demonstrably produces and the kind cluster did not happen to be in when it was
    // photographed. Each test says which value, why the API produces it, and which object
    // the open Phase 2 capture trip should bring back to retire the synthesis.
    //
    // These are **decode** tests. A rule's positive fixture stays a real capture — this
    // technique never becomes one.

    /// D3's whole premise — one card per owner — and not one of the twelve pod captures
    /// carries an `ownerReference`, because `scripts/broken.yaml` creates bare pods.
    /// A pod created by a Deployment carries exactly this: its ReplicaSet, `controller:
    /// true`. **Capture trip:** the owned broken pod already listed in the open Phase 2
    /// box retires this.
    #[test]
    fn a_pod_with_a_controller_files_under_it_and_not_under_itself() {
        let mut object: Pod =
            serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
        object.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "apps/v1".to_string(),
            kind: "ReplicaSet".to_string(),
            name: "web-7d4f5c6b8".to_string(),
            uid: "3f1c9a20-0d2e-4c19-9a71-5a4b1f6e8d10".to_string(),
            controller: Some(true),
            block_owner_deletion: Some(true),
        }]);

        let p = PodSnapshot::from(object);
        println!("owner: {:?}\nobject: {:?}", p.owner, p.id);

        assert_ne!(p.owner, p.id, "a controlled pod does not file under itself");
        assert_eq!(p.owner.kind, ObjectKind::ReplicaSet);
        assert_eq!(p.owner.name, "web-7d4f5c6b8");
        assert_eq!(
            p.owner.uid.as_deref(),
            Some("3f1c9a20-0d2e-4c19-9a71-5a4b1f6e8d10"),
            "the owner's own uid, never the pod's — the group agrees on the owner's (D39)"
        );
        assert_eq!(
            p.owner.namespace.as_deref(),
            Some("default"),
            "an ownerReference carries no namespace; the owner is in the pod's own"
        );
        assert_eq!(p.id.name, "broken-crashloop", "the object is still the pod");
        assert!(
            !p.mirror,
            "a pod with a real controller is not a static pod, and a drain will move it"
        );
    }

    /// D39: kubelet writes an `ownerReference` of kind `Node` onto every static pod, so
    /// on this project's own `just cluster-up` cluster `etcd-*`, `kube-apiserver-*`,
    /// `kube-scheduler-*` and `kube-controller-manager-*` all carry one — `controller:
    /// true` included. Kept, the card loses `kube-system` and draws as a machine.
    ///
    /// **The identity is discarded and the bit is kept.** `kubectl drain` skips a mirror
    /// pod unconditionally, so a control-plane node cordoned for an upgrade still runs
    /// its four static pods when the drain has finished perfectly — and N2, which fires
    /// on "cordoned and still has pods on it", would file a finding on every one of them.
    /// **Capture trip:** the `-n kube-system` capture already listed in the open Phase 2
    /// box retires this.
    #[test]
    fn a_mirror_pods_node_owner_is_discarded_and_it_files_under_itself() {
        let mut object: Pod =
            serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
        object.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "v1".to_string(),
            kind: "Node".to_string(),
            name: "k8rs-control-plane".to_string(),
            uid: "dc9525d6-e89b-4409-81fa-f3e4a57409aa".to_string(),
            controller: Some(true),
            block_owner_deletion: None,
        }]);

        let p = PodSnapshot::from(object);
        println!("owner: {:?} mirror: {}", p.owner, p.mirror);

        assert_eq!(
            p.owner, p.id,
            "a mirror pod files under itself — there is no controller to scale or roll"
        );
        assert_eq!(p.owner.kind, ObjectKind::Pod);
        assert_eq!(
            p.owner.namespace.as_deref(),
            Some("default"),
            "keeping the Node would have taken the namespace off the card with it"
        );
        assert_ne!(
            p.owner.name, "k8rs-control-plane",
            "four control-plane pods must not collapse onto one machine-shaped card"
        );
        assert!(
            p.mirror,
            "the Node reference is the only trace a static pod leaves — the annotation \
             that also says so is stripped by the sanitizer — and dropping it with the \
             identity makes N2 fire on every correctly drained node"
        );
    }

    /// A Node reference that does **not** control the pod is somebody's garbage-collection
    /// link, not the kubelet's. The mirror bit rides on the same match arm as the
    /// discarded owner, so it inherits that arm's filter; asserted because "kind is Node"
    /// and "the *controller* is a Node" are one character apart in the implementation and
    /// a whole rule apart in N2.
    /// **Capture trip:** none — see D40 on why `broken.yaml` is not contorted to produce
    /// a non-controlling reference.
    #[test]
    fn a_node_reference_that_does_not_control_the_pod_does_not_make_it_a_mirror_pod() {
        let mut object: Pod =
            serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
        object.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "v1".to_string(),
            kind: "Node".to_string(),
            name: "k8rs-control-plane".to_string(),
            uid: "dc9525d6-e89b-4409-81fa-f3e4a57409aa".to_string(),
            controller: Some(false),
            block_owner_deletion: None,
        }]);

        let p = PodSnapshot::from(object);
        println!("owner: {:?} mirror: {}", p.owner, p.mirror);

        assert_eq!(
            p.owner, p.id,
            "it still files under itself — nothing controls it"
        );
        assert!(
            !p.mirror,
            "only the kubelet's own controlling reference means a static pod; a drain \
             moves this one, so N2 must count it"
        );
    }

    /// Both `ownerReferences` committed in this repo are controllers, so the filter that
    /// picks one is unproven. An object can carry several references and at most one of
    /// them controls it — `kubectl delete --cascade=orphan` and any operator that adds a
    /// reference for garbage collection produce the non-controlling kind.
    /// **Capture trip:** none needed; a second reference is not something `broken.yaml`
    /// should be contorted to produce.
    #[test]
    fn a_reference_that_does_not_control_the_pod_is_not_its_owner() {
        let mut object: Pod =
            serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
        object.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "apps/v1".to_string(),
            kind: "Deployment".to_string(),
            name: "not-the-controller".to_string(),
            uid: "8c2d17bb-4e6a-4f0b-9d55-2f1a7c3e9b44".to_string(),
            controller: Some(false),
            block_owner_deletion: Some(true),
        }]);

        let p = PodSnapshot::from(object);
        println!("owner: {:?}", p.owner);

        assert_eq!(
            p.owner, p.id,
            "only the reference marked `controller: true` decides the card"
        );
        assert_ne!(p.owner.name, "not-the-controller");
    }

    /// The spec lookup reads `initContainers` *and* `containers`, and no captured init
    /// container declares resources — so dropping the first list from the chain changes
    /// nothing today. A migration container with a memory limit is ordinary.
    /// **Capture trip:** put `resources.limits.memory` on the `migrate` init container in
    /// `scripts/healthy.yaml`, and this synthesis retires.
    #[test]
    fn an_init_containers_limits_are_found_in_the_init_list() {
        let mut object: Pod =
            serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
        object
            .spec
            .as_mut()
            .expect("the captured pod has a spec")
            .init_containers
            .as_mut()
            .expect("this pod declares an init container")[0]
            .resources = Some(ResourceRequirements {
            limits: Some(BTreeMap::from([(
                "memory".to_string(),
                Quantity("128Mi".to_string()),
            )])),
            requests: Some(BTreeMap::from([(
                "cpu".to_string(),
                Quantity("50m".to_string()),
            )])),
            claims: None,
        });

        let p = PodSnapshot::from(object);
        let migrate = container(&p, "migrate");
        println!(
            "migrate: limit={:?} request={:?}",
            migrate.memory_limit, migrate.cpu_request
        );

        assert_eq!(migrate.role, ContainerRole::Init);
        assert_eq!(
            migrate.memory_limit.as_deref(),
            Some("128Mi"),
            "rule 2 must be able to name the limit an init container exceeded"
        );
        assert_eq!(
            migrate.cpu_request.as_deref(),
            Some("50m"),
            "N5 sums these too"
        );
        assert_eq!(
            container(&p, "app").memory_limit.as_deref(),
            Some("64Mi"),
            "and the regular container still finds its own"
        );
    }

    /// **The limit a finding may name is the one the container was actually given.**
    /// `ContainerStatus.resources` is "the compute resource requests and limits that have
    /// been successfully enacted on the running container", and in-place resize (beta and
    /// default-on since 1.33) is what makes it disagree with the spec: patch a crashing
    /// pod's limit 64Mi → 512Mi and the resize can sit `Deferred` because the node cannot
    /// fit it. A spec-first read then prints "exceeded its 512Mi limit · exit 137" about a
    /// container never given 512Mi, and sends an operator hunting a leak in an application
    /// that never had the memory.
    ///
    /// Both directions are pinned on one object, because a decode hardwired to either side
    /// passes half of this: the *enacted* value wins when the two differ, and the spec is
    /// still the answer when the kubelet has reported no resources at all — which is a
    /// shape the captures already contain (`init.json`'s `app` carries `resources: null`).
    /// **Capture trip:** patch the memory limit of a pod onto a node that cannot fit the
    /// new value, and capture it with the resize still pending.
    #[test]
    fn the_memory_limit_is_the_enacted_one_and_not_the_one_the_spec_asked_for() {
        let captured =
            || -> Pod { serde_json::from_value(fixture("oom")).expect("oom.json is a Pod") };

        let mut deferred = captured();
        deferred
            .spec
            .as_mut()
            .expect("the captured pod has a spec")
            .containers[0]
            .resources
            .as_mut()
            .expect("the captured container declares resources")
            .limits
            .as_mut()
            .expect("including a memory limit")
            .insert("memory".to_string(), Quantity("512Mi".to_string()));

        let p = PodSnapshot::from(deferred);
        let c = container(&p, "hog");
        println!(
            "resize deferred: limit={:?} request={:?}",
            c.memory_limit, c.memory_request
        );
        assert_eq!(
            c.memory_limit.as_deref(),
            Some("64Mi"),
            "the status says 64Mi was enacted, so rule 2 may not name the 512Mi the \
             patch asked for and the node refused"
        );
        assert_eq!(
            c.memory_request.as_deref(),
            Some("64Mi"),
            "and N5 charges what the node actually reserved, for the same reason"
        );

        // The other half: a container the kubelet has reported no resources on falls back
        // to the spec, or the limit disappears from every cluster that does not send the
        // field at all.
        let mut no_status_resources = captured();
        no_status_resources
            .spec
            .as_mut()
            .expect("the captured pod has a spec")
            .containers[0]
            .resources
            .as_mut()
            .expect("the captured container declares resources")
            .limits
            .as_mut()
            .expect("including a memory limit")
            .insert("memory".to_string(), Quantity("512Mi".to_string()));
        no_status_resources
            .status
            .as_mut()
            .expect("the captured pod has a status")
            .container_statuses
            .as_mut()
            .expect("it has a container status")[0]
            .resources = None;

        let p = PodSnapshot::from(no_status_resources);
        let c = container(&p, "hog");
        println!("no enacted resources: limit={:?}", c.memory_limit);
        assert_eq!(
            c.memory_limit.as_deref(),
            Some("512Mi"),
            "with nothing enacted to read, what the spec asked for is the best answer \
             there is — not no answer"
        );
    }

    /// **The fallback is per key, not per side.** [`effective`] promises that a `requests`
    /// map the kubelet did populate does not suppress the spec for the keys it left out,
    /// and until now nothing said so: a per-*object* read fails only because
    /// `healthy.json`'s `migrate` happens to carry `"resources": {}` in its status, which
    /// is an accident of one capture rather than a property anything asserts.
    ///
    /// The shape that decides it is one key of two. `oom.json`'s `hog` has its memory
    /// request enacted and no cpu request at all, so adding one to the spec leaves the
    /// status the answer for `memory` and the spec the only source there is for `cpu` —
    /// the doc's own named cost case, a resize *adding* a value where none existed, read
    /// from whichever side has each key. Read per side instead and N5 charges this pod
    /// nothing for a quarter of a CPU it is committed to.
    ///
    /// The mirror shape — a key in `status.resources` the spec never declared — is **not
    /// asserted, so a decode reading the status only for keys the spec names is
    /// indistinguishable from this one.** It is nonetheless **reachable**: under pod-level
    /// resources (KEP-2837) `getMemoryLimit` puts the *pod's* memory limit on a container
    /// cgroup whose own is unset, and `convertContainerStatusResources` copies it back
    /// without testing whether the spec declared that key.
    ///
    /// What *is* structural is the other direction: the status map begins as
    /// `allocatedContainer.Resources.DeepCopy()` and `validateContainerResize` forbids
    /// *removing* a key while permitting an addition, so the **spec's** key set is the
    /// superset-or-equal. That is what makes the shape this test asserts legitimate rather
    /// than invented.
    ///
    /// **Capture trip:** the pod the test above waits for, and — for the mirror shape — a
    /// pod declaring `spec.resources.limits.memory` whose container declares a cpu limit
    /// and no memory limit.
    #[test]
    fn a_key_the_kubelet_did_not_enact_still_reads_from_the_spec() {
        let mut resizing: Pod = serde_json::from_value(fixture("oom")).expect("oom.json is a Pod");
        let requested = resizing
            .spec
            .as_mut()
            .expect("the captured pod has a spec")
            .containers[0]
            .resources
            .as_mut()
            .expect("the captured container declares resources")
            .requests
            .as_mut()
            .expect("including a memory request");
        requested.insert("memory".to_string(), Quantity("512Mi".to_string()));
        requested.insert("cpu".to_string(), Quantity("250m".to_string()));

        // Untouched: the kubelet enacted `memory` alone, exactly as captured. Asserted,
        // because the whole test is about the key that is missing from this map.
        let enacted = resizing
            .status
            .as_ref()
            .expect("the captured pod has a status")
            .container_statuses
            .as_ref()
            .expect("it has a container status")[0]
            .resources
            .as_ref()
            .expect("the kubelet reported enacted resources");
        assert!(
            !enacted
                .requests
                .as_ref()
                .is_some_and(|r| r.contains_key("cpu")),
            "the shape this test rests on: no cpu request was enacted here, got {:?}",
            enacted.requests
        );

        let p = PodSnapshot::from(resizing);
        let c = container(&p, "hog");
        println!(
            "pending resize: cpu={:?} memory={:?}",
            c.cpu_request, c.memory_request
        );
        assert_eq!(
            c.memory_request.as_deref(),
            Some("64Mi"),
            "the key the kubelet did enact is still read from the status, not the spec"
        );
        assert_eq!(
            c.cpu_request.as_deref(),
            Some("250m"),
            "and the key it did not enact falls through to the spec — a populated status \
             is not an answer for every key that is absent from it"
        );
    }

    /// **KEP-2837: the pod itself can hold the request, and then the containers do not.**
    /// `spec.resources` is beta and default-on since 1.34 and the captures are v1.36.1,
    /// but nothing in `scripts/*.yaml` declares one, so every fixture decodes `None` here
    /// and a decode that dropped the field entirely would read as correct. A pod asking
    /// for `cpu: "4"` with no per-container request then decodes as all-`None`, N5 sums
    /// zero and calls the node healthy while four committed CPUs sit invisible.
    ///
    /// **When it is set it replaces the container sum for N5 — it does not add to it** —
    /// so both numbers have to survive the decode, which is why the container assertion
    /// is here too.
    /// **Capture trip:** add `spec.resources.requests` to a pod in `scripts/healthy.yaml`
    /// — it is a healthy shape, not a broken one.
    #[test]
    fn a_pod_that_declares_its_own_request_carries_it_beside_the_containers() {
        let captured: Pod =
            serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");

        let untouched = PodSnapshot::from(captured.clone());
        println!(
            "captured: pod cpu={:?} memory={:?}",
            untouched.cpu_request, untouched.memory_request
        );
        assert_eq!(
            (untouched.cpu_request, untouched.memory_request),
            (None, None),
            "this pod declares no pod-level request, and inventing one from the \
             containers would be a number nobody committed"
        );

        let mut object = captured;
        object
            .spec
            .as_mut()
            .expect("the captured pod has a spec")
            .resources = Some(ResourceRequirements {
            limits: None,
            requests: Some(BTreeMap::from([
                ("cpu".to_string(), Quantity("4".to_string())),
                ("memory".to_string(), Quantity("2Gi".to_string())),
            ])),
            claims: None,
        });

        let p = PodSnapshot::from(object);
        println!(
            "pod-level: cpu={:?} memory={:?}",
            p.cpu_request, p.memory_request
        );
        assert_eq!(
            p.cpu_request.as_deref(),
            Some("4"),
            "four CPUs the scheduler committed, which N5 must not sum to zero"
        );
        assert_eq!(p.memory_request.as_deref(), Some("2Gi"));
        assert_eq!(
            container(&p, "app").cpu_request.as_deref(),
            Some("10m"),
            "and the container keeps its own — N5 replaces the sum with the pod-level \
             value rather than adding to it, and it needs both numbers to do that"
        );
    }

    /// `terminationMessagePolicy: FallbackToLogsOnError` makes the kubelet copy the tail
    /// of the container's log into `terminated.message`, which is what turns rule 6's
    /// action from "check the logs" into the log line. No captured container sets the
    /// policy, so the field is absent everywhere and a decode that dropped it reads as
    /// correct. `Waiting` already carried a message; `Terminated` not carrying one was an
    /// asymmetry with nothing behind it.
    ///
    /// The message is copied through **exactly as the API sent it**, newline included —
    /// bounding and control-character stripping belong to `k8s.rs` at ingest, as the
    /// section header above says, and doing half of it here would leave nobody sure which
    /// half.
    /// **Capture trip:** set `terminationMessagePolicy: FallbackToLogsOnError` on
    /// `scripts/broken.yaml`'s crashlooping container.
    #[test]
    fn a_terminated_container_keeps_the_message_the_kubelet_left_behind() {
        let mut object: Pod =
            serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
        // A hostname, not an address: the captures carry `REDACTED-IP` where the sanitizer
        // took one out, and writing an address-shaped literal back into the same file is
        // confusing at best.
        let tail = "panic: dial tcp db.payments.svc:5432: connect: connection refused\n";
        object
            .status
            .as_mut()
            .expect("the captured pod has a status")
            .container_statuses
            .as_mut()
            .expect("it has a container status")[0]
            .last_state
            .as_mut()
            .expect("the container has died at least once")
            .terminated
            .as_mut()
            .expect("and it died rather than being killed while waiting")
            .message = Some(tail.to_string());

        let p = PodSnapshot::from(object);
        let c = container(&p, "quitter");
        println!("last message: {:?}", c.last_terminated);
        assert_eq!(
            c.last_terminated
                .as_ref()
                .and_then(|t| t.message.as_deref()),
            Some(tail),
            "verbatim (D37): rule 6 shows the application's own last line, and a decode \
             that trimmed or truncated it would pass any looser assertion"
        );
        assert_eq!(
            c.last_terminated.as_ref().map(|t| t.exit_code),
            Some(1),
            "and the rest of the termination is untouched"
        );
    }

    /// **The sidecar, and it is the case with a specific wrong answer available.** An
    /// init container with `restartPolicy: Always` is the native sidecar — GA since 1.29,
    /// and how Istio, Linkerd and the Vault agent run. It is charged like a regular
    /// container (the scheduler's effective request is
    /// `max(max over the init prefix, sum(regular) + sum(restartable init))`) and it is
    /// described like one: "the init container `istio-proxy` is crashlooping" is not
    /// plain language, it is wrong. Under a boolean it decodes as `Init`, which is the
    /// answer that overstates a migration container or drops the proxy's request on
    /// every pod of a meshed node.
    ///
    /// The neighbours are set on purpose (D40's third category): the same `migrate`
    /// container without the field is asserted `Init` two tests up, and the regular `app`
    /// container beside it carries **the same `restartPolicy: Always`** and still decodes
    /// `Regular` — so this cannot pass by answering `Sidecar` for everything, for every
    /// init container, for the whole pod, or for every container that declares the field.
    /// The last of those is the one the list is read for: only the *init* list makes the
    /// policy mean sidecar, and a regular container is charged additively and described as
    /// itself whatever its restart policy says. 1.34 began relaxing where the field may
    /// appear and the fixtures are pinned at v1.36.1 (`tests/fixtures/K8S_VERSION`), so a
    /// server of that generation accepts it there; whether a given cluster's feature gates
    /// do is not what this asserts, because the requirement does not depend on it — the
    /// answer is `Regular` whatever the restart policy says and whoever admitted it.
    /// **Capture trip:** add a `restartPolicy: Always` init container to
    /// `scripts/healthy.yaml` — it is a healthy shape, not a broken one.
    #[test]
    fn an_init_container_that_never_finishes_is_a_sidecar_and_not_an_init_container() {
        let mut object: Pod =
            serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
        let spec = object.spec.as_mut().expect("the captured pod has a spec");
        spec.init_containers
            .as_mut()
            .expect("this pod declares an init container")[0]
            .restart_policy = Some("Always".to_string());
        spec.containers[0].restart_policy = Some("Always".to_string());

        let p = PodSnapshot::from(object);
        println!(
            "{:?}",
            p.containers
                .iter()
                .map(|c| (&c.name, c.role))
                .collect::<Vec<_>>()
        );

        assert_eq!(
            container(&p, "migrate").role,
            ContainerRole::Sidecar,
            "it starts in the init sequence and then keeps running beside the workload, \
             so N5 adds its request instead of taking a maximum over it"
        );
        assert_eq!(
            container(&p, "app").role,
            ContainerRole::Regular,
            "and the container it runs beside is still a regular one — it declares the \
             very same `restartPolicy: Always`, and on a regular container that changes \
             neither how it is charged nor what it is called"
        );
    }

    /// Rule 8's evasion: `subPath` narrows a hostPath mount, so the volume's own path is
    /// not what the container gets. `hostPath: /` with `subPath: run/containerd` hands the
    /// container the node's container runtime state while the volume still records `/` —
    /// the same shape as the `var/run/docker.sock` case rule 8 escalates on, and the one
    /// `scripts/broken.yaml` can produce on a kind node.
    ///
    /// **Captured, not synthesized.** This was a one-field synthesis for as long as the
    /// only committed mount set no `subPath` at all — where a decode that dropped the
    /// field read as correct — and the capture trip it named has landed: the field is now
    /// in `hostpath.json`. What makes the assertion mean anything is the pair: one of the
    /// two mounts of this volume is narrowed and the other is not, so a decode that
    /// dropped `sub_path`, or filled it with the mountPath, or answered one mount's value
    /// for both, fails on the object itself.
    #[test]
    fn a_hostpath_mount_keeps_the_subpath_that_says_what_is_really_mounted() {
        let p = pod("hostpath");
        println!("{:?}", p.host_path_mounts);

        let narrowed = p
            .host_path_mounts
            .iter()
            .find(|m| m.container == "nosy")
            .expect("the captured pod mounts the host in `nosy`");
        assert_eq!(
            narrowed.path, "/",
            "the volume still records the node's root, which is what makes the subPath \
             the only field that says what is really mounted"
        );
        assert_eq!(
            narrowed.sub_path.as_deref(),
            Some("run/containerd"),
            "rule 8 reads the path joined with the subPath; the volume alone says `/` \
             and hides what the container was actually handed"
        );

        let whole = p
            .host_path_mounts
            .iter()
            .find(|m| m.container == "shipper")
            .expect("and the second container mounts the same volume");
        assert_eq!(
            whole.sub_path, None,
            "this mount narrows nothing, so `None` here is a decoded absence and not a \
             default the field was never given: {whole:?}"
        );
    }

    /// `read_only` and `container` both belong to the **mount**, and one hostPath volume
    /// can be mounted twice — a node agent writing to `/var/log` beside a log shipper
    /// reading it is the ordinary shape. With one entry per volume the two collapse and
    /// rule 8 either misses the writable one or reports the read-only one; without the
    /// container name the finding cannot say which of them has the node's root, while
    /// `kubectl describe pod` can.
    ///
    /// **Captured, not synthesized:** the second container the capture trip asked for is
    /// in `hostpath.json`, so the two mounts are two real mounts of one real volume rather
    /// than a clone of the first with its name changed.
    #[test]
    fn two_containers_mounting_one_hostpath_are_two_mounts_and_are_told_apart() {
        let raw = fixture("hostpath");
        let p = pod("hostpath");
        println!("{:?}", p.host_path_mounts);

        // The premise, out of the capture: one hostPath volume, and every mount below is a
        // mount of it. Two entries from two volumes would satisfy every assertion here
        // while proving nothing about the collapse this test exists over.
        let volumes: Vec<&str> = raw["spec"]["volumes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| v.get("hostPath").is_some())
            .map(|v| captured_str(v, &["name"]))
            .collect();
        println!("hostPath volumes: {volumes:?}");
        assert_eq!(
            volumes.len(),
            1,
            "this pod declares one hostPath volume and mounts it twice — that is the \
             shape being asserted: {volumes:?}"
        );

        assert_eq!(
            p.host_path_mounts.len(),
            2,
            "one entry per volume collapses the two and rule 8 sees whichever of them \
             survived: {:?}",
            p.host_path_mounts
        );
        let containers: Vec<&str> = p
            .host_path_mounts
            .iter()
            .map(|m| m.container.as_str())
            .collect();
        assert_eq!(
            containers,
            vec!["nosy", "shipper"],
            "the finding has to name which container has the node's root, and \
             `kubectl describe pod` can"
        );
        let writable: Vec<&str> = p
            .host_path_mounts
            .iter()
            .filter(|m| !m.read_only)
            .map(|m| m.container.as_str())
            .collect();
        assert_eq!(
            writable,
            vec!["nosy"],
            "two mounts of one volume, and rule 8 fires on exactly one of them"
        );
    }

    /// Rule 8 fires *on* a writable mount, so a decode that hardwired `read_only: false`
    /// would pass every assertion a single writable hostPath can make — while turning
    /// every CNI and CSI agent in a real cluster into a critical finding. A read-only host
    /// mount is what a correctly written node agent uses, and the captured mounts set no
    /// `readOnly` key at all where they are writable, so the two are not symmetric in the
    /// JSON either: `true` is present and `false` is an absence.
    ///
    /// **Captured, not synthesized, and on both sides.** `hostpath.json` now mounts one
    /// volume twice with opposite flags — so neither hardwired answer survives the same
    /// pod — and `healthy-hostpath.json` is the posture case the capture trip asked
    /// `scripts/healthy.yaml` for: a node agent reading `/var/log`, which the Phase 4
    /// posture report lists and rule 8 must leave alone.
    #[test]
    fn a_read_only_hostpath_mount_is_not_reported_as_writable() {
        let p = pod("hostpath");
        println!("broken: {:?}", p.host_path_mounts);

        let by_container = |name: &str| -> HostPathMount {
            p.host_path_mounts
                .iter()
                .find(|m| m.container == name)
                .unwrap_or_else(|| panic!("the captured pod mounts the host in {name}"))
                .clone()
        };
        // One volume, one path, two containers, opposite answers: whichever way a decode
        // hardwired the flag, one of these two fails.
        assert!(
            !by_container("nosy").read_only,
            "the mount that declares no `readOnly` key is writable — rule 8's escalator"
        );
        assert!(
            by_container("shipper").read_only,
            "and the mount beside it that declares `readOnly: true` is not, on the very \
             same hostPath: {:?}",
            by_container("shipper")
        );

        // The posture case, on the healthy side: a real node agent's read-only mount of a
        // path that is not the node's root.
        let healthy = pod("healthy-hostpath");
        println!("healthy: {:?}", healthy.host_path_mounts);
        assert_eq!(
            healthy.host_path_mounts,
            vec![HostPathMount {
                path: "/var/log".to_string(),
                sub_path: None,
                sub_path_expr: None,
                read_only: true,
                container: "reader".to_string(),
            }],
            "the path is still the node's, but rule 8 has neither of its escalators here \
             — this one belongs in the posture report and nowhere else"
        );
    }

    /// N6's pod side. Every captured pod was scheduled by the default scheduler with no
    /// constraints of its own, so `node_selector` is empty everywhere and a decode that
    /// always returned an empty map would look right. Pinning a pod to a node class is
    /// the ordinary reason a pod goes Pending forever, which is the finding N6 explains.
    /// **Capture trip:** `broken-pending` should be respun with a `nodeSelector` and a
    /// matching toleration instead of an unschedulable cpu request.
    #[test]
    fn a_pod_that_asks_for_a_particular_node_keeps_what_it_asked_for() {
        let mut object: Pod =
            serde_json::from_value(fixture("pending")).expect("pending.json is a Pod");
        let spec = object.spec.as_mut().expect("the captured pod has a spec");
        // One coherent group: `kubectl` users write these two together, because a
        // nodeSelector aimed at a tainted node class is useless without the toleration.
        spec.node_selector = Some(BTreeMap::from([
            ("disktype".to_string(), "ssd".to_string()),
            ("kubernetes.io/os".to_string(), "linux".to_string()),
        ]));
        spec.tolerations = Some(vec![ApiToleration {
            key: Some("dedicated".to_string()),
            operator: Some("Equal".to_string()),
            value: Some("gpu".to_string()),
            effect: Some("NoSchedule".to_string()),
            toleration_seconds: None,
        }]);

        let p = PodSnapshot::from(object);
        println!(
            "selector: {:?}\ntolerations: {:?}",
            p.node_selector, p.tolerations
        );

        assert_eq!(
            p.node_selector,
            BTreeMap::from([
                ("disktype".to_string(), "ssd".to_string()),
                ("kubernetes.io/os".to_string(), "linux".to_string()),
            ]),
            "N6 cannot name the label that is keeping the pod off every node without this"
        );
        assert_eq!(
            p.tolerations,
            vec![Toleration {
                key: Some("dedicated".to_string()),
                operator: Some("Equal".to_string()),
                value: Some("gpu".to_string()),
                effect: Some("NoSchedule".to_string()),
            }],
            "`Equal` against a value is the toleration form the captured pods never carry"
        );
    }

    /// N2. Every node in the capture was schedulable, so a decode that always answered
    /// `false` reads as correct. `kubectl cordon` sets exactly this field.
    ///
    /// **The capture trip this named has landed and this synthesis is now redundant:**
    /// `break-nodes` cordons a worker, so the node it picks here is already
    /// `unschedulable: true` and the line below sets a field to the value it has.
    /// `nodes_decode_with_their_conditions_taints_and_versions` asserts the real one, out
    /// of the capture; retiring this belongs with the box that goes through the
    /// synthesized tests, not with the one that repaired the assertions the capture
    /// reddened.
    #[test]
    fn a_cordoned_node_decodes_as_unschedulable() {
        let mut object: Node = items::<Node>("nodes")
            .into_iter()
            .find(|n| n.metadata.name.as_deref() == Some("k8rs-worker"))
            .expect("the capture has a worker");
        object
            .spec
            .as_mut()
            .expect("the captured node has a spec")
            .unschedulable = Some(true);

        let n = NodeSnapshot::from(object);
        println!("{}: unschedulable={}", n.id.name, n.unschedulable);
        assert!(
            n.unschedulable,
            "N2 is the whole of 'cordoned and forgotten', and this field is all of it"
        );
    }

    /// **This one does not depict an object the cluster produces — it proves the two
    /// fields are decoded independently of one another**, and the capture structurally
    /// cannot.
    ///
    /// [`Taint::added_at`] and [`Taint::value`] come from different writers: the node
    /// controller stamps `timeAdded` on the taints it adds and gives them no value, and an
    /// operator's `kubectl taint` supplies a value and no `timeAdded`. So no real taint
    /// carries both, and each field's *own* proof is on a real object elsewhere — `value`
    /// on the captured `dedicated=gpu:NoExecute` in
    /// `nodes_decode_with_their_conditions_taints_and_versions`, `added_at` on the
    /// controller's taints via the sweep in
    /// `the_pinned_now_is_not_before_the_captures_it_is_read_against`. What neither can
    /// reach is the pair: a decode that zeroed `value` whenever `added_at` was present, or
    /// the reverse, satisfies both of them, because in every committed object one of the
    /// two is already `None`.
    ///
    /// **The shape is legal, it is merely undemonstrated** — `validateNodeTaints`
    /// (`pkg/apis/core/validation/validation.go`) checks the key, the value, the effect
    /// and `<key,effect>` uniqueness and never looks at `TimeAdded`, so any client that
    /// sets both produces a valid object. That is the distinction D40 draws: this is not a
    /// test against an impossible object, it is a synthesis licensed by a property the
    /// captures cannot exhibit. **Capture trip:** none — a cluster is not asked to write
    /// an object no controller of it writes.
    #[test]
    fn a_taint_keeps_its_value_and_the_time_it_was_added() {
        let mut object: Node = items::<Node>("nodes")
            .into_iter()
            .find(|n| n.metadata.name.as_deref() == Some("k8rs-worker"))
            .expect("the capture has a worker");
        object
            .spec
            .as_mut()
            .expect("the captured node has a spec")
            .taints = Some(vec![ApiTaint {
            key: "dedicated".to_string(),
            value: Some("gpu".to_string()),
            effect: "NoExecute".to_string(),
            time_added: Some(time("2026-08-11T22:50:00Z")),
        }]);

        let n = NodeSnapshot::from(object);
        println!("{:?}", n.taints);
        assert_eq!(
            n.taints,
            vec![Taint {
                key: "dedicated".to_string(),
                value: Some("gpu".to_string()),
                effect: "NoExecute".to_string(),
                added_at: Some(time("2026-08-11T22:50:00Z")),
            }],
            "N6 has to name the taint in full — `dedicated=gpu:NoExecute`, not `dedicated`"
        );
    }

    /// N1 and N3. The capture is of a healthy cluster, so every condition in it is the
    /// benign one and a decode that dropped `status`, `reason` or `last_transition` would
    /// still satisfy the negative assertions. A kubelet that stops posting is what N1 is,
    /// and it is the single most common node failure there is.
    /// **Capture trip:** stop the kubelet on one worker before `just fixtures` (`docker
    /// exec k8rs-worker systemctl stop kubelet`), wait past the 40s grace, and N1 and N3
    /// both get real positive fixtures.
    #[test]
    fn a_notready_node_keeps_the_status_the_reason_and_when_it_changed() {
        let mut object: Node = items::<Node>("nodes")
            .into_iter()
            .find(|n| n.metadata.name.as_deref() == Some("k8rs-worker"))
            .expect("the capture has a worker");
        let status = object
            .status
            .as_mut()
            .expect("the captured node has a status");
        // One field group: this is what the node controller writes when a kubelet stops
        // reporting and the node then runs out of disk.
        for c in status.conditions.iter_mut().flatten() {
            match c.type_.as_str() {
                "Ready" => {
                    c.status = "Unknown".to_string();
                    c.reason = Some("NodeStatusUnknown".to_string());
                    c.message = Some("Kubelet stopped posting node status.".to_string());
                    c.last_transition_time = Some(time("2026-08-11T23:00:00Z"));
                }
                "DiskPressure" => {
                    c.status = "True".to_string();
                    c.reason = Some("KubeletHasDiskPressure".to_string());
                    // The message moves with the other three or the object is one no API
                    // server could emit: the capture's "kubelet has no disk pressure"
                    // beside `status: True` is a contradiction, and a synthesis is only
                    // licensed while every field of it is a value the API produces.
                    c.message = Some("kubelet has disk pressure".to_string());
                    c.last_transition_time = Some(time("2026-08-11T23:05:00Z"));
                }
                _ => {}
            }
        }

        let n = NodeSnapshot::from(object);
        println!("{:?}", n.conditions);

        let ready = n
            .conditions
            .iter()
            .find(|c| c.type_ == "Ready")
            .expect("N1 reads Ready");
        assert_eq!(
            ready.status, "Unknown",
            "a kubelet that went quiet reports Unknown, not False — N1 must see the difference"
        );
        assert_eq!(ready.reason.as_deref(), Some("NodeStatusUnknown"));
        assert_eq!(
            ready.message.as_deref(),
            Some("Kubelet stopped posting node status.")
        );
        assert_eq!(
            ready.last_transition,
            Some(time("2026-08-11T23:00:00Z")),
            "N1 fires only after five minutes, so the timestamp is half the rule"
        );

        let disk = n
            .conditions
            .iter()
            .find(|c| c.type_ == "DiskPressure")
            .expect("N3 reads the pressure conditions");
        assert_eq!(disk.status, "True", "N3 is 'evictions are coming'");
        assert_eq!(disk.reason.as_deref(), Some("KubeletHasDiskPressure"));
        // Asserted so the synthesis above cannot drift back into a contradiction nothing
        // reads: an unasserted synthesized field is how the first one got there.
        assert_eq!(disk.message.as_deref(), Some("kubelet has disk pressure"));
    }

    /// N5 measures pod requests against **allocatable**, and the gap between capacity and
    /// allocatable — what the kubelet and the system daemons reserve — is the entire
    /// subject of the rule. kind reports the two as identical because it reserves
    /// nothing, so reading the wrong one is invisible in the capture; every managed
    /// cluster there is (EKS, GKE, AKS) reserves a substantial slice.
    /// **Capture trip:** none — kind cannot produce this. It needs `--kube-reserved` on
    /// the kubelet, which is a cluster change, not a fixture.
    #[test]
    fn allocatable_is_read_and_not_capacity() {
        let mut object: Node = items::<Node>("nodes")
            .into_iter()
            .find(|n| n.metadata.name.as_deref() == Some("k8rs-worker"))
            .expect("the capture has a worker");
        object
            .status
            .as_mut()
            .expect("the captured node has a status")
            .allocatable = Some(BTreeMap::from([
            ("cpu".to_string(), Quantity("3800m".to_string())),
            ("memory".to_string(), Quantity("3484172Ki".to_string())),
        ]));

        let n = NodeSnapshot::from(object);
        println!(
            "allocatable: {:?} / {:?}",
            n.allocatable_cpu, n.allocatable_memory
        );
        assert_eq!(
            n.allocatable_cpu.as_deref(),
            Some("3800m"),
            "the capture reports the same number for capacity and allocatable, so N5 \
             comparing against capacity would miss every overcommit inside the \
             reservation and look right here"
        );
        assert_eq!(n.allocatable_memory.as_deref(), Some("3484172Ki"));
    }

    /// D45: an empty `state` is not a fourth state, it is a waiting one — upstream's own
    /// `ContainerState` doc says "if none of them is specified, the default one is
    /// ContainerStateWaiting". The captured statuses all carry a populated state, so the
    /// branch has no coverage from any capture; without this, answering `Running` there
    /// would turn rule 7 ("running but not ready") into a false CRITICAL on any container
    /// whose state the kubelet had not filled in yet.
    /// **Capture trip:** none. No cluster can be asked to emit this on purpose; the API
    /// definition is the whole of the evidence, which is why the ruling cites it.
    #[test]
    fn a_container_state_with_nothing_in_it_is_a_waiting_one() {
        let mut object: Pod =
            serde_json::from_value(fixture("crashloop")).expect("crashloop.json is a Pod");
        object
            .status
            .as_mut()
            .expect("the captured pod has a status")
            .container_statuses
            .as_mut()
            .expect("it has a container status")[0]
            .state = Some(ApiContainerState::default());

        let p = PodSnapshot::from(object);
        let c = container(&p, "quitter");
        println!("empty state decodes as: {:?}", c.state);
        assert_eq!(
            c.state,
            ContainerState::Waiting {
                reason: None,
                message: None
            },
            "the API defines the empty case as waiting, so nothing here may invent another"
        );
    }

    /// The other field the API defines by its absence. `status.started` is an
    /// `Option<bool>` upstream and all twelve captured pods set it, so the null case has
    /// never been decoded — and upstream's own `ContainerStatus` doc rules it: "The null
    /// value must be treated the same as false." Answering `true` there would claim the
    /// kubelet had judged a startup probe it has not yet run — wrong on the one class of
    /// workload where this field carries any information at all, the pods that declare a
    /// `startupProbe`. It is not what keeps rule 7 off a rolling update: no committed
    /// fixture declares a `startupProbe`, so `started` there is true the instant the
    /// container runs, and the "since when" is `ready.last_transition` and nothing else
    /// (NOTES § D51).
    ///
    /// The same capture asserts the field's `true` in
    /// `running_but_not_ready_is_distinguishable_from_waiting`, so the pair pins both
    /// answers on one object: a decode that hardwires either one fails one of them.
    /// **Capture trip:** none. A kubelet that has not evaluated a startup probe yet omits
    /// the field, but that window is not something `just fixtures` can be aimed at; the
    /// upstream type and its own doc are the whole of the evidence, as with D45 above.
    #[test]
    fn a_container_status_that_omits_started_is_not_started() {
        let mut object: Pod =
            serde_json::from_value(fixture("readiness")).expect("readiness.json is a Pod");
        object
            .status
            .as_mut()
            .expect("the captured pod has a status")
            .container_statuses
            .as_mut()
            .expect("it has a container status")[0]
            .started = None;

        let p = PodSnapshot::from(object);
        let c = container(&p, "app");
        println!("started omitted decodes as: {}", c.started);
        assert!(
            !c.started,
            "a missing `started` means false, not true — the kubelet has not said this \
             container passed a startup probe, so nothing here may say that it did"
        );
    }

    /// Every workload in the capture is either fully ready or entirely absent, so
    /// `desired` and `ready` could be read from four other replica counters each and stay
    /// green. A rollout in progress separates all of them — which is the only state W2
    /// is ever evaluated in.
    /// **Capture trip:** a Deployment whose new revision cannot start (a bad image on the
    /// second revision) captured mid-rollout gives the Deployment and its ReplicaSet at
    /// once; a DaemonSet with a broken image gives the third.
    #[test]
    fn desired_and_ready_are_read_from_their_own_fields_and_not_a_neighbour() {
        let mut deployment: Deployment = items::<Deployment>("deployments")
            .into_iter()
            .find(|d| d.metadata.name.as_deref() == Some("healthy-deploy"))
            .expect("the capture has a healthy Deployment");
        deployment.spec.as_mut().expect("it has a spec").replicas = Some(5);
        let status = deployment.status.as_mut().expect("it has a status");
        status.replicas = Some(6); // surge: one more pod than wanted exists
        status.ready_replicas = Some(2);
        status.available_replicas = Some(0); // ready, but not past minReadySeconds
        status.updated_replicas = Some(4);
        status.unavailable_replicas = Some(3);

        let w = WorkloadSnapshot::from(deployment);
        println!("deployment: desired={:?} ready={:?}", w.desired, w.ready);
        assert_eq!(
            (w.desired, w.ready),
            (Some(5), Some(2)),
            "desired is what the spec asked for, ready is what is passing probes — and \
             the other four counters are 6, 0, 4 and 3"
        );

        let mut replicaset: ReplicaSet = items::<ReplicaSet>("healthy-replicasets")
            .into_iter()
            .next()
            .expect("the capture has a healthy ReplicaSet");
        replicaset.spec.as_mut().expect("it has a spec").replicas = Some(5);
        let status = replicaset.status.as_mut().expect("it has a status");
        status.replicas = 6;
        status.ready_replicas = Some(2);
        status.available_replicas = Some(0);
        status.fully_labeled_replicas = Some(4);

        let w = WorkloadSnapshot::from(replicaset);
        println!("replicaset: desired={:?} ready={:?}", w.desired, w.ready);
        assert_eq!(
            (w.desired, w.ready),
            (Some(5), Some(2)),
            "a ReplicaSet's `status.replicas` is not optional and is not the desired count"
        );

        let mut daemonset: DaemonSet = items::<DaemonSet>("daemonsets")
            .into_iter()
            .find(|d| d.metadata.name.as_deref() == Some("kindnet"))
            .expect("the capture has kindnet");
        let status = daemonset.status.as_mut().expect("it has a status");
        status.desired_number_scheduled = 4;
        status.number_ready = 2;
        status.current_number_scheduled = 3;
        status.number_available = Some(1);
        status.updated_number_scheduled = Some(0);

        let w = WorkloadSnapshot::from(daemonset);
        println!("daemonset: desired={:?} ready={:?}", w.desired, w.ready);
        assert_eq!(
            (w.desired, w.ready),
            (Some(4), Some(2)),
            "a DaemonSet wants one pod per matching node, and `currentNumberScheduled` \
             is how many exist — not how many are wanted"
        );
    }

    // --- THE POD RULES, AGAINST THE COMMITTED CAPTURES ---
    //
    // Positive *and* negative for every rule, and the negatives are the half that matters:
    // a rule with only a positive is a rule nobody has proved quiet. The healthy captures
    // are asserted **empty**, not "not this one finding" — a false positive from any other
    // rule reaches the same screen and is the same defect.
    //
    // **The clock is the second input, and two tests vary it rather than the capture.**
    // Rules 7 and 12 both have a threshold, and a threshold nobody crosses is a threshold
    // nobody has tested. `now` is a field of the snapshot precisely so a rule can be read
    // at a chosen moment (invariant 5, NOTES § D18), so the same committed pod is analysed
    // just inside and just outside its window. That is not the "one field changed on a real
    // capture" technique above — nothing about the capture moves.

    /// The snapshot [`analyze`] is handed below: pods and a moment. No nodes and no
    /// workloads, because every rule in this box reads a Pod — the joins belong to the
    /// N-series and W-series, which are later boxes and would only add rules that cannot
    /// fire to every assertion here.
    fn pods_at(pods: Vec<PodSnapshot>, now: Time) -> ClusterSnapshot {
        ClusterSnapshot {
            now,
            pods,
            nodes: Vec::new(),
            workloads: Vec::new(),
            server_version: None,
            context: None,
            client_certificate: None,
            namespace_scope: None,
        }
    }

    fn findings_at(names: &[&str], now: Time) -> Vec<Finding> {
        analyze(&pods_at(names.iter().map(|n| pod(n)).collect(), now))
    }

    fn findings(names: &[&str]) -> Vec<Finding> {
        findings_at(names, now())
    }

    /// One finding as `--once` would print it (`screens/once.md`) — so that
    /// `cargo test -- --nocapture` shows the sentences a user reads and not a `Debug` dump
    /// of the struct they came in. CLAUDE.md's "green tests are not working software" gate
    /// is read off this.
    fn card(f: &Finding, now: &Time) -> String {
        let mark = match f.severity {
            Severity::Critical => '●',
            Severity::Warn => '▲',
            Severity::Info => '○',
        };
        let name = match &f.owner.namespace {
            Some(ns) => format!("{ns}/{}", f.owner.name),
            None => f.owner.name.clone(),
        };
        let age = f.age(now).map_or(String::new(), |a| format!(" · {a}"));
        format!(
            "{mark} {name}{age}\n  {}\n  {}\n  → {}\n  $ {}\n",
            f.title,
            f.evidence,
            f.action,
            f.kubectl_cmd
                .as_deref()
                .unwrap_or("(no command shows this)")
        )
    }

    fn titles(all: &[Finding]) -> Vec<&str> {
        all.iter().map(|f| f.title.as_str()).collect()
    }

    fn show(all: &[Finding]) {
        for f in all {
            println!("{}", card(f, &now()));
        }
    }

    /// The one finding on `pod` whose title contains `phrase` — and a failure when there is
    /// not exactly one. "The rule fired" and "the rule fired twice on one container" print
    /// the same green line otherwise.
    fn only<'a>(all: &'a [Finding], pod: &str, phrase: &str) -> &'a Finding {
        let mut hits = all
            .iter()
            .filter(|f| f.object.name == pod && f.title.contains(phrase));
        let found = hits
            .next()
            .unwrap_or_else(|| panic!("nothing on {pod} says {phrase:?} — got {:?}", titles(all)));
        assert!(
            hits.next().is_none(),
            "two findings on {pod} say {phrase:?} — got {:?}",
            titles(all)
        );
        found
    }

    fn nothing(all: &[Finding], why: &str) {
        assert!(all.is_empty(), "{why} — got {:?}", titles(all));
    }

    /// **The numbers and the words that came out of a document, asserted against the
    /// document.** Everything else below is proved by a capture; these cannot be, because
    /// no committed capture sits in the bands they draw — no regular container in the
    /// repository has one or two restarts, and none exited a code outside `1` and `137`.
    /// A constant transcribed from REQUIREMENTS is still a requirement, and without this
    /// test lowering rule 5's warn band to a single restart stays green.
    #[test]
    fn the_thresholds_and_the_exit_table_are_the_ones_the_documents_write_down() {
        assert_eq!(
            (RESTARTS_WARN, RESTARTS_CRITICAL),
            (3, 10),
            "REQUIREMENTS: rule 5 warns at three restarts and turns critical at ten"
        );

        // NOTES § v1 rule set's translation table. Every row has to be a *sentence*: the
        // reader who has just met `137` for the first time is exactly who rule 6 is
        // written for (invariant 14).
        for (code, reason, must_say) in [
            (137, Some("OOMKilled"), "more memory than it was allowed"),
            // **The row NOTES got wrong, and the reason it is asserted twice.** A
            // liveness-probe kill that outlives the grace period lands as exit 137 with
            // reason `Error`; the memory sentence there sends someone to raise a limit on
            // a container whose health endpoint is timing out.
            (137, Some("Error"), "did not stop when it was asked to"),
            (137, None, "did not stop when it was asked to"),
            (143, None, "ordinary shutdown"),
            (1, None, "the application's own error"),
            (2, None, "the application's own error"),
            (126, None, "could not be run"),
            (127, None, "was not found"),
        ] {
            let said = exit_meaning(code, reason)
                .unwrap_or_else(|| panic!("NOTES § v1 rule set translates exit {code}"));
            assert!(
                said.contains(must_say),
                "exit {code} {reason:?} reads {said:?}"
            );
        }
        assert_eq!(
            exit_meaning(42, None),
            None,
            "and a code the table does not cover is not given an invented meaning"
        );

        // The formatter, over a real captured termination with one field moved — the same
        // technique the decode tests use, and for the same reason: no capture carries an
        // exit code outside the table, and this is a string function rather than a rule.
        let mut run = container(&pod("crashloop"), "quitter")
            .last_terminated
            .clone()
            .expect("the captured crash loop records how its last run ended");
        assert!(
            exit_fact(&run).starts_with("exit 1 ("),
            "the number the reader searched for comes first: {}",
            exit_fact(&run)
        );
        run.exit_code = 42;
        assert_eq!(
            exit_fact(&run),
            "exit 42",
            "and where the number alone is the honest answer, the number alone is what shows"
        );
    }

    /// [`mounted_path`] on the shapes the API can produce and the fixtures do not contain.
    /// A pure string function, so it is asserted as one — the escalators above it are three
    /// equality tests and they only mean what they read as if this normalises.
    #[test]
    fn what_the_container_actually_gets_is_normalised_before_it_is_compared() {
        let mount = |path: &str, sub: Option<&str>, expr: Option<&str>| HostPathMount {
            path: path.to_string(),
            sub_path: sub.map(str::to_string),
            sub_path_expr: expr.map(str::to_string),
            read_only: false,
            container: "c".to_string(),
        };

        // `//` and `/.` both pass upstream validation — absolute, no backsteps — and both
        // resolve to the node's root. Unnormalised they are not `"/"`, so they fall into
        // the writable branch: silenced in `kube-system`, and elsewhere advised with
        // "mount it read-only", about the whole machine.
        for spelling in ["/", "//", "/.", "/./", "///."] {
            assert_eq!(
                mounted_path(&mount(spelling, None, None)),
                "/",
                "{spelling} is the node's root"
            );
        }
        // NOTES § D46's own example: the socket is only visible once the subPath is joined.
        assert_eq!(
            mounted_path(&mount("/var/run", Some("docker.sock"), None)),
            "/var/run/docker.sock"
        );
        // And the join narrows as well as widens — this is `hostpath.json`'s own shape.
        assert_eq!(
            mounted_path(&mount("/", Some("run/containerd"), None)),
            "/run/containerd"
        );
        assert_eq!(mounted_path(&mount("/var/log/", None, None)), "/var/log");

        // A `subPathExpr` narrows the mount by something k8rs cannot read, so the path
        // stops being the root — the safe direction, since the alternative is the loudest
        // possible false CRITICAL.
        let expr = mounted_path(&mount("/", None, Some("$(POD_NAME)")));
        assert_eq!(expr, "/$(POD_NAME)");
        assert_ne!(
            expr, "/",
            "a container given one directory does not have the machine"
        );

        // A constant nobody can match is a rule that never fires, and every entry here is
        // compared with `==` against this function's output.
        for socket in RUNTIME_SOCKETS {
            assert_eq!(
                mounted_path(&mount(socket, None, None)),
                socket,
                "{socket} is not in the form this function produces, so rule 8 could never \
                 match it"
            );
            // **The invariant [`is_runtime_socket`]'s fold rests on.** It strips a leading
            // `/var` and compares what is left against this list, which only finds the
            // second spelling of anything if every entry is written under `/run`. An entry
            // elsewhere would be matched under one name and silently missed under the
            // other — the defect this list has already had once (NOTES § D77) — so it is
            // the constant that is asserted and not the stripping.
            assert!(
                socket.starts_with("/run/"),
                "{socket} is not under /run: stripping a leading `/var` can never produce \
                 it, so it would be matched under the one spelling written here and missed \
                 under whatever its second name is"
            );
        }
        // **Every member, by name, and the naming kept in step with the list.** The sweeps
        // over rule 8 *iterate* this constant, so they structurally cannot notice a member
        // gone — "matched nothing" and "had nothing to match" are the same green line
        // (CLAUDE.md § Code phase rules). Docker was the one entry with no canary and
        // deleting it survived every mutation `tester` ran (NOTES § D78).
        let canaries = [
            (
                "/run/docker.sock",
                "NOTES § v1 rule set names Docker's socket as the escalator's own example, \
                 and it is still what a pre-2022 cluster and every Docker-in-Docker build \
                 agent mounts",
            ),
            (
                "/run/containerd/containerd.sock",
                "kind — the cluster every fixture here came off — runs containerd, so a \
                 list that stops at Docker's socket is a rule that cannot fire on its own \
                 test bed",
            ),
            (
                "/run/crio/crio.sock",
                "CRI-O under the spelling a manifest may actually write is the miss NOTES \
                 § D78 exists for",
            ),
            (
                "/run/k3s/containerd/containerd.sock",
                "k3s puts containerd's socket here and nowhere else, and RKE2 embeds k3s's \
                 containerd — the distro half of the audience that meets this in a normal \
                 week",
            ),
            (
                "/run/cri-dockerd.sock",
                "cri-dockerd is in crictl's own default endpoint probe list, and it is \
                 what every node that kept Docker past 1.24 runs, minikube included",
            ),
        ];
        for (socket, why) in canaries {
            assert!(RUNTIME_SOCKETS.contains(&socket), "{socket}: {why}");
        }
        for socket in RUNTIME_SOCKETS {
            assert!(
                canaries.iter().any(|(named, _)| *named == socket),
                "{socket} was added to RUNTIME_SOCKETS without a canary here, so deleting \
                 it again would be green: every sweep below iterates the list"
            );
        }
    }

    /// Rules 1, 5 and 6 on the one pod that earns all three, which is also where every
    /// piece of invariant 14 is visible at once: the loop is explained, the exit code is
    /// translated, and the container's own last line replaces "go and read the logs".
    #[test]
    fn the_crash_looping_pod_gets_the_loop_the_count_and_the_exit() {
        let raw = fixture("crashloop");
        let all = findings(&["crashloop"]);
        show(&all);

        assert_eq!(
            all.len(),
            2,
            "rules 1 and 6, and nothing else — rule 5 stays quiet on a container rule 1 is \
             already describing, one incident being one card: {:?}",
            titles(&all)
        );
        assert_eq!(
            all.iter()
                .filter(|f| f.title.contains("has been restarted"))
                .count(),
            0,
            "and `15 restarts` is already on rule 1's own evidence line: {:?}",
            titles(&all)
        );

        let looping = only(&all, "broken-crashloop", "CrashLoopBackOff");
        assert_eq!(looping.severity, Severity::Critical);
        assert!(
            looping.evidence.contains("container quitter"),
            "the finding names which container: {}",
            looping.evidence
        );
        assert!(
            looping.evidence.contains("15 restarts"),
            "{}",
            looping.evidence
        );
        assert!(
            looping.evidence.contains("the last run lasted 2s"),
            "D51's first fork of a crashloop triage — how long each run survives, which \
             `describe` makes a human subtract at 3am: {}",
            looping.evidence
        );
        assert!(
            looping
                .evidence
                .contains("exit 1 (the application's own error)"),
            "invariant 14: the code is translated, never printed and left: {}",
            looping.evidence
        );
        assert_eq!(
            looping.kubectl_cmd.as_deref(),
            Some("kubectl describe pod broken-crashloop -n default"),
            "and the command shows the state, the last termination and the count the card \
             just claimed"
        );
        assert_eq!(
            looping.owner, looping.object,
            "nothing controls this pod, so it files under itself (D3's fallback)"
        );

        // **The moment the run ended, never the moment it began** — both are in the same
        // struct one line apart, and this capture keeps them two seconds apart, which is
        // what makes the second assertion mean anything at all (`Finding::timestamp`).
        let died = at(
            captured_status(&raw, "containerStatuses", "quitter"),
            &["lastState", "terminated"],
        );
        assert_eq!(
            looping.timestamp,
            Some(captured_time(died, &["finishedAt"]))
        );
        assert_ne!(
            captured_time(died, &["finishedAt"]),
            captured_time(died, &["startedAt"]),
            "a capture whose run started and ended in the same second cannot tell the right \
             field from the wrong one, and is not the fixture for this assertion"
        );
        assert_eq!(
            looping.age(&now()).as_deref(),
            Some("2 hours ago"),
            "a duration, not English parsed back into a number"
        );

        let failed = only(&all, "broken-crashloop", "previous run failed");
        assert_eq!(failed.severity, Severity::Warn);
        assert_eq!(
            failed.action,
            "the last thing it logged was: panic: dial tcp db.payments.svc:5432: connect: \
             connection refused",
            "the kubelet kept the tail of the log, so the card shows it instead of sending \
             the reader to fetch what k8rs is already holding — and it is the *last* line, \
             not the `starting` this capture opens with"
        );
        assert!(
            failed.evidence.contains("ran for 2s"),
            "and how long the run survived, which is the fork between bad configuration and \
             a leak: {}",
            failed.evidence
        );
    }

    /// **Rule 2's permanence, and the two directions that separate it from a suppressor that
    /// would be wrong.**
    ///
    /// `lastState.terminated` never expires, so a container the kernel killed once and that
    /// has served ever since would draw a CRITICAL for the life of the pod — and a single kill
    /// never reaches [`restarting_repeatedly`]'s `>= 3`, so nothing else carries that pod and
    /// nothing ever clears it. But *serving* is not what makes it wrong: a container killed
    /// five minutes ago and running now is exactly what belongs on this screen, because the
    /// next spike will do it again. Only the two together stand the rule down.
    ///
    /// Both directions are asserted, or the clause is half-proven — one of them alone passes
    /// against `if doing_its_job(c)` on its own, and the other against a rule that has stopped
    /// firing at all.
    ///
    /// The shape is `oom.json`'s own container with the kill moved into its past and the
    /// restart count set to the `1` of a container that was killed once, written onto a
    /// **decoded copy** (NOTES § D53 — the committed JSON is not touched). One restart is what
    /// keeps rule 5 out of the answer, so the silence below is rule 2's own and not a count
    /// that happened to fall under a threshold.
    #[test]
    fn an_old_kill_on_a_container_that_has_been_fine_since_is_not_on_the_broken_now_screen() {
        /// The captured OOM, still `137 / OOMKilled`, on a container that is running and
        /// ready again — with the kill placed `mins` before the pinned [`now`].
        fn killed_and_recovered(mins: i64) -> PodSnapshot {
            let when = Time(
                now()
                    .0
                    .checked_sub(SignedDuration::from_mins(mins))
                    .expect("a moment before the pinned now"),
            );
            let mut object: Pod =
                serde_json::from_value(fixture("oom")).expect("oom.json is a Pod");
            let hog = &mut object
                .status
                .as_mut()
                .expect("the captured pod has a status")
                .container_statuses
                .as_mut()
                .expect("the captured pod has a container status")[0];
            assert_eq!(hog.name, "hog");
            hog.restart_count = 1;
            hog.ready = true;
            hog.state = Some(ApiContainerState {
                running: Some(ContainerStateRunning {
                    started_at: Some(when.clone()),
                }),
                ..ApiContainerState::default()
            });
            hog.last_state
                .as_mut()
                .and_then(|s| s.terminated.as_mut())
                .expect("the capture's OOM kill")
                .finished_at = Some(when);
            PodSnapshot::from(object)
        }

        let long_ago = killed_and_recovered(60 * 24 * 30);
        let hog = container(&long_ago, "hog");
        assert!(
            doing_its_job(hog)
                && hog.restarts < RESTARTS_WARN
                && matches!(&hog.last_terminated, Some(run)
                    if run.reason.as_deref() == Some("OOMKilled")),
            "the edit has to leave a serving container that still carries the kill, and a \
             count below rule 5's band so nothing else answers for this pod: {hog:?}"
        );
        nothing(
            &analyze(&pods_at(vec![long_ago], now())),
            "the kernel killed this container a month ago and it has been serving ever \
             since. Nothing is broken *now*, and the card could never be cleared — whether \
             its limit is right is a memory-limit question for the Capacity report (D2)",
        );

        // The other direction, and the reason `doing_its_job` alone is the wrong suppressor:
        // the kill is inside the grace, so it is news.
        let just_now = killed_and_recovered(5);
        let all = analyze(&pods_at(vec![just_now], now()));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "a container the kernel killed five minutes ago is running now on borrowed time, \
             and it will happen again on the next spike: {:?}",
            titles(&all)
        );
        assert_eq!(
            only(&all, "broken-oom", "OOMKilled").severity,
            Severity::Critical
        );
    }

    /// Rule 2, and the one place two rules would otherwise describe a single death.
    #[test]
    fn the_out_of_memory_card_names_the_limit_and_rule_6_stays_out_of_its_way() {
        let raw = fixture("oom");
        let all = findings(&["oom"]);
        show(&all);

        let killed = only(&all, "broken-oom", "OOMKilled");
        assert_eq!(killed.severity, Severity::Critical);
        assert!(
            killed
                .title
                .contains("used more memory than it was allowed")
                && killed.title.contains("kernel killed it"),
            "invariant 14: OOMKilled is explained and then named, never printed alone: {}",
            killed.title
        );
        // The limit the kubelet enacted, read back off `status.resources` — the field D51
        // sent this rule to, so that a pending resize cannot make the card name a figure
        // the container was never given.
        let enacted = captured_str(
            captured_status(&raw, "containerStatuses", "hog"),
            &["resources", "limits", "memory"],
        );
        assert!(
            killed.evidence.contains(&format!("limit {enacted}")),
            "the evidence line carries the enacted limit ({enacted}): {}",
            killed.evidence
        );
        assert!(killed.evidence.contains("exit 137"), "{}", killed.evidence);
        assert_eq!(
            killed.timestamp,
            Some(captured_time(
                at(
                    captured_status(&raw, "containerStatuses", "hog"),
                    &["lastState", "terminated"]
                ),
                &["finishedAt"]
            ))
        );

        assert_eq!(
            all.iter()
                .filter(|f| f.title.contains("previous run failed"))
                .count(),
            0,
            "rule 6 owns the exit-code table and rule 2 owns this death; both firing puts \
             two cards on one event, the weaker of which says 'exit 137, almost always \
             memory' beside one that already names the limit: {:?}",
            titles(&all)
        );
        assert_eq!(
            all.len(),
            2,
            "rules 1 and 2 — this container is crash-looping *and* was OOM-killed, and that \
             is one incident with two causes to name, not three cards: {:?}",
            titles(&all)
        );
        // Rule 1 calls the same translator, so the memory sentence survives where the
        // reason earns it — and this is the only card in the box that still says it.
        let looping = only(&all, "broken-oom", "CrashLoopBackOff");
        assert!(
            looping.evidence.contains("more memory than it was allowed"),
            "exit 137 *with* `OOMKilled` beside it is the memory kill: {}",
            looping.evidence
        );
    }

    /// Rules 3 and 4. Both are a waiting reason plus the runtime's own sentence, and the
    /// sentence is the entire diagnosis in each case (NOTES § D37).
    #[test]
    fn an_unpullable_image_and_a_missing_configmap_each_name_what_to_go_and_fix() {
        let all = findings(&["image", "config"]);
        show(&all);
        assert_eq!(all.len(), 2, "one card each: {:?}", titles(&all));

        let image = only(&all, "broken-image", "image is not usable");
        assert_eq!(image.severity, Severity::Critical);
        assert!(
            image.title.contains("ErrImagePull") || image.title.contains("ImagePullBackOff"),
            "the kubelet alternates between the two as it backs off, and whichever this \
             capture caught is the word the reader sees in `kubectl get pods`: {}",
            image.title
        );
        assert!(
            image
                .evidence
                .contains("image registry.invalid/does-not-exist:v9"),
            "the resolved name is printed beside the runtime's sentence, because rule 3's \
             action is 'check the image name': {}",
            image.evidence
        );
        assert!(
            image.evidence.contains("no such host"),
            "and the runtime's own sentence is what says the pull actually failed: {}",
            image.evidence
        );
        assert_eq!(
            image.timestamp, None,
            "nothing in a container status records when the first pull was attempted, and \
             `screens/alerts.md` would rather leave the right edge blank than borrow a \
             nearby moment"
        );

        // **`describe` never prints `state.waiting.message`.** kubectl's `describeStatus`
        // renders a waiting container's `Reason` and stops, and that message — the sentence
        // naming the registry that refused, or the ConfigMap that is absent — *is* the whole
        // evidence line of both these cards. It reaches `describe` only through an Event,
        // reworded and gone at `--event-ttl`. A teaching command that does not show what the
        // card says is worse than none (invariant 4), which is the same argument rule 12 is
        // already built on.
        assert_eq!(
            image.kubectl_cmd.as_deref(),
            Some("kubectl get pod broken-image -n default -o yaml")
        );

        let config = only(&all, "broken-config", "ConfigMap or Secret");
        assert_eq!(config.severity, Severity::Critical);
        assert!(
            config
                .evidence
                .contains("configmap \"this-configmap-does-not-exist\" not found"),
            "rule 4's whole value is the name of the object that is missing: {}",
            config.evidence
        );
        assert_eq!(
            config.kubectl_cmd.as_deref(),
            Some("kubectl get pod broken-config -n default -o yaml"),
            "for the same reason as rule 3's above"
        );
    }

    /// **`subPathExpr` reaches no capture**, because nothing in `scripts/broken.yaml` uses
    /// one — and the field it guards against produces the loudest wrong card in the box, a
    /// CRITICAL claiming a container has the whole machine when it was given one directory.
    /// So the decode is asserted with the technique the rest of this section uses: one
    /// field, on a real object, set to a value the API demonstrably produces.
    ///
    /// **Capture trip:** a pod in `scripts/broken.yaml` mounting `hostPath: /` with
    /// `subPathExpr: $(POD_NAME)` and the `fieldRef` env var that resolves it, which is the
    /// ordinary way this is written.
    #[test]
    fn a_mount_narrowed_by_an_environment_variable_is_carried_unresolved() {
        let mut object: Pod =
            serde_json::from_value(fixture("hostpath")).expect("hostpath.json is a Pod");
        let spec = object.spec.as_mut().expect("the captured pod has a spec");
        let mount = spec
            .containers
            .iter_mut()
            .find(|c| c.name == "nosy")
            .and_then(|c| c.volume_mounts.as_mut())
            .into_iter()
            .flatten()
            .find(|m| m.name == "root")
            .expect("the capture mounts the host volume in nosy");
        // Upstream forbids both on one mount, so the capture's `subPath` goes as the
        // expression arrives — one edit, not two.
        mount.sub_path = None;
        mount.sub_path_expr = Some("$(POD_NAME)".to_string());

        let p = PodSnapshot::from(object);
        let narrowed = p
            .host_path_mounts
            .iter()
            .find(|m| m.container == "nosy")
            .expect("the mount survives the decode");
        println!("{narrowed:?} -> {}", mounted_path(narrowed));

        assert_eq!(
            narrowed.sub_path_expr.as_deref(),
            Some("$(POD_NAME)"),
            "carried verbatim: the values are in env and in the Secrets behind it, and k8rs \
             reads neither"
        );
        assert_eq!(
            mounted_path(narrowed),
            "/$(POD_NAME)",
            "and it joins like a subPath, so the path stops being the node's root"
        );
        assert_ne!(
            mounted_path(narrowed),
            "/",
            "which is the whole point — a container handed one directory does not have the \
             machine, and rule 8 saying so would be its loudest false CRITICAL"
        );
    }

    /// Rule 7, both sides of its clock. **Without the window this rule fires on every
    /// rolling update**, so the window is the rule (NOTES § D46, § D51).
    #[test]
    fn a_pod_out_of_the_service_is_only_a_finding_once_it_has_been_that_way_a_while() {
        let raw = fixture("readiness");
        let all = findings(&["readiness"]);
        show(&all);
        assert_eq!(all.len(), 1, "rule 7 alone: {:?}", titles(&all));

        let unready = only(&all, "broken-readiness", "not receiving traffic");
        assert_eq!(unready.severity, Severity::Warn);
        assert!(
            unready.evidence.contains("container app"),
            "{}",
            unready.evidence
        );

        // **The since-when is floored at the container's own run start.** `Ready` is the
        // *pod's* condition and does not move until every container is ready, so a
        // container younger than that condition would be dated to a moment it did not
        // exist for. This capture separates the two by five seconds, so the wrong field is
        // visible here rather than hidden behind two equal timestamps.
        let condition = captured_time(captured_condition(&raw, "Ready"), &["lastTransitionTime"]);
        let began = captured_time(
            captured_status(&raw, "containerStatuses", "app"),
            &["state", "running", "startedAt"],
        );
        assert!(
            began.0 > condition.0,
            "a capture whose container started before the pod went unready cannot tell the \
             floor from the condition, and is not the fixture for this assertion"
        );
        assert_eq!(
            unready.timestamp,
            Some(began.clone()),
            "the later of the two, because a container cannot have been out of the Service \
             for longer than its current run has existed"
        );
        assert_ne!(
            unready.timestamp,
            Some(condition),
            "the pod's own condition dates this container to before it was running"
        );

        // Just inside the window: the same captured pod, read at exactly the grace. This is
        // every pod of every rolling update, and it draws nothing.
        nothing(
            &findings_at(&["readiness"], time("2026-08-12T20:55:58Z")),
            "ten minutes unready is a readiness probe with an `initialDelaySeconds`, not an \
             outage",
        );
        // And one second past it.
        assert_eq!(
            findings_at(&["readiness"], time("2026-08-12T20:55:59Z")).len(),
            1,
            "past `progressDeadlineSeconds`' own default is where Kubernetes itself stops \
             calling a rollout healthy"
        );

        // **`started` is read here as a suppressor, which is not the trigger D51 rejected.**
        // No container in any capture declares a `startupProbe`, so all of them report
        // `true`; one that declares a slow startup probe reports `false`, and until it
        // passes the kubelet does not run the readiness probe at all — `ready: false` there
        // means *not asked yet*. **Capture trip:** a pod with a `startupProbe` that has not
        // passed, which is the only object that separates these two readings.
        assert!(
            container(&pod("readiness"), "app").started,
            "the positive fixture has to be past its startup for this rule to reach it"
        );
    }

    /// Rule 5's warn band and the sentence that only holds for a container that is
    /// actually serving — and rule 6's silence beside it, which is the same fixture's
    /// second job.
    #[test]
    fn a_container_that_looks_fine_still_gets_a_card_for_how_often_it_has_died() {
        let all = findings(&["restarts"]);
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "rule 5 alone. **Rule 6 is deliberately silent here**: `lastState.terminated` \
             never expires, so a container that failed once and has served ever since would \
             carry that card for the life of the pod — the largest false-positive volume in \
             the box, and one that needs nothing unusual but uptime: {:?}",
            titles(&all)
        );
        assert!(
            container(&pod("restarts"), "flaky")
                .last_terminated
                .is_some(),
            "the capture does carry a failed previous run, so the silence above is the rule \
             deciding and not the field being absent"
        );

        let counted = only(&all, "broken-restarts", "restarted 3 times");
        assert_eq!(
            counted.severity,
            Severity::Warn,
            "3 is rule 5's warn band and 10 is where it becomes critical (REQUIREMENTS)"
        );
        assert!(
            counted.title.contains("it is serving now"),
            "this container *is* passing its probes, which is the whole of why NOTES words \
             rule 5 'looks healthy now, but something is wrong': {}",
            counted.title
        );

        assert_eq!(
            counted.severity,
            Severity::Warn,
            "and it stays WARN whatever the count while the container is serving: a red card \
             whose own title says it is serving is what teaches a reader to stop believing \
             red (NOTES § D2)"
        );
    }

    /// Rule 8's positives on one captured pod — the node's root and the node's runtime
    /// socket — and the read-only mount of a path that is neither, which is the Analysis
    /// posture row and not a card.
    #[test]
    fn the_two_escalated_host_mounts_both_fire_and_the_ordinary_one_does_not() {
        let all = findings(&["hostpath"]);
        show(&all);
        assert_eq!(all.len(), 2, "one per escalated mount: {:?}", titles(&all));

        // `shipper` mounts `/` and mounts it **read-only**, and it fires anyway: the path
        // alone is the escalator, because read-only access to the node's whole filesystem
        // is still every secret on the machine.
        let root = only(&all, "broken-hostpath", "whole filesystem of the machine");
        assert_eq!(root.severity, Severity::Critical);
        assert!(
            root.evidence.contains("container shipper") && root.evidence.contains("read-only"),
            "{}",
            root.evidence
        );

        // `nosy` mounts the same volume with `subPath: run/containerd`, so what it actually
        // gets is `/run/containerd` — not the node's root, but the directory the node's
        // runtime socket sits in, which is the socket (NOTES § D78). **This is the one
        // captured shape the socket escalator fires on**, and the path it names is the one
        // the container has.
        let socket = only(&all, "broken-hostpath", "drive the container runtime");
        assert_eq!(socket.severity, Severity::Critical);
        assert!(
            socket.evidence.contains("/run/containerd on the node"),
            "the subPath narrows what is mounted and the card has to say what the container \
             really got (D46): {}",
            socket.evidence
        );
        assert!(
            !socket.evidence.contains("/ on the node"),
            "a rule reading `path` alone would call this a mount of the node's root: {}",
            socket.evidence
        );
        assert!(socket.evidence.contains("writable"), "{}", socket.evidence);
        assert!(
            !socket.action.contains("mount it read-only"),
            "and the reader is not told read-only fixes it — that is the writable branch's \
             advice, and on a runtime socket it gives away the node: {}",
            socket.action
        );

        for f in &all {
            assert_eq!(
                f.timestamp, None,
                "a hostPath mount is a standing property, not an event, and a date beside it \
                 sends the reader looking for a change that never happened"
            );
        }

        nothing(
            &findings(&["healthy-hostpath"]),
            "a read-only mount of /var/log is how a log shipper is supposed to work, and \
             D2 sends it to the Analysis posture rows",
        );
    }

    /// **Rule 8's real negative, and the reason the box could not close without this
    /// capture.** Writable host mounts are the normal state of every CNI agent, kube-proxy
    /// and control-plane component, so the rule as specified fires CRITICAL on a healthy
    /// kind cluster.
    #[test]
    fn kube_systems_node_agents_and_static_pods_are_not_host_mount_findings() {
        let pods: Vec<PodSnapshot> = items::<Pod>("kube-system-pods")
            .into_iter()
            .map(PodSnapshot::from)
            .collect();

        // **The exemption is asserted to be exercised, not assumed.** "Nothing fired"
        // and "nothing could have fired" print the same green line, and this capture is
        // the only place either shape exists — so both are counted before the emptiness
        // below means anything.
        let writable = |p: &PodSnapshot| {
            p.host_path_mounts
                .iter()
                .any(|m| !m.read_only && mounted_path(m) != "/")
        };
        let daemonset_pods = pods
            .iter()
            .filter(|p| p.owner.kind == ObjectKind::DaemonSet && writable(p))
            .count();
        let mirror_pods = pods.iter().filter(|p| p.mirror && writable(p)).count();
        println!(
            "{} pods: {daemonset_pods} DaemonSet-owned and {mirror_pods} mirror pods write \
             to their node",
            pods.len()
        );
        assert!(
            daemonset_pods > 0,
            "kindnet and kube-proxy write to `/etc/cni/net.d`, `/run/xtables.lock` and \
             `/var/run/nri`; a capture without one is not this rule's negative"
        );
        assert!(
            mirror_pods > 0,
            "`etcd` writes to `/var/lib/etcd` and is owned by a **Node**, not a DaemonSet — \
             narrowing rule 8 to DaemonSets alone would still fire on every control plane, \
             which is why the exemption reads `mirror || DaemonSet`"
        );

        nothing(
            &analyze(&pods_at(pods, now())),
            "a fresh kind cluster's own kube-system is healthy, and every rule in this box \
             has to be silent on it",
        );
    }

    // --- RULE 8'S SOCKET ESCALATOR, ON PLANTED MOUNTS ---
    //
    // No committed capture mounts a runtime socket — kind's own components have no reason
    // to — and none is edited to grow one (NOTES § D53). So every shape below is planted
    // into a decoded copy of a real captured pod, one coherent group of fields at a time,
    // and each is a shape `kubectl apply` would produce (NOTES § D40).

    /// The captured hostPath pod with its one host volume repointed at `path` and both
    /// mounts of it narrowed by `sub` — `shipper` read-only, `nosy` writable, so what the
    /// two containers get is the same node path under two modes. That is the whole reason
    /// this capture is the one to plant on: it is where the question *does the mode matter*
    /// can be asked at all.
    ///
    /// **`sub` is the mount's `subPath`, and `None` is not the same test as `Some`** — rule 8
    /// reads the join and never `path` alone (NOTES § D46), so a socket assembled out of a
    /// directory and a file has to be swept beside the ones written whole.
    fn host_volume(path: &str, sub: Option<&str>) -> Vec<Finding> {
        let pod = capture_but("hostpath", |p| {
            let spec = p.spec.as_mut().expect("a captured pod has a spec");
            let volume = spec
                .volumes
                .iter_mut()
                .flatten()
                .find(|v| v.host_path.is_some())
                .expect("hostpath.json is the capture that carries a host volume");
            volume
                .host_path
                .as_mut()
                .expect("the volume just found is the host one")
                .path = path.to_string();
            let name = volume.name.clone();
            for mount in spec
                .containers
                .iter_mut()
                .flat_map(|c| c.volume_mounts.iter_mut().flatten())
                .filter(|m| m.name == name)
            {
                mount.sub_path = sub.map(str::to_string);
            }
        });
        analyze(&pods_at(vec![pod], now()))
    }

    /// **Every socket in the list, under both of its names and both of its writings.**
    /// `/var/run` is a symlink to `/run` on every systemd distribution, so the two spellings
    /// are one file on the node and the card may not depend on which one an author typed
    /// ([`is_runtime_socket`]); and a manifest may mount the directory and name the socket
    /// in the `subPath`, which is a socket only once rule 8 joins them (NOTES § D46).
    #[test]
    fn a_runtime_socket_is_the_same_socket_under_either_of_its_two_spellings() {
        for socket in RUNTIME_SOCKETS {
            let var = format!("/var{socket}");
            let (dir, file) = socket
                .rsplit_once('/')
                .expect("every socket in the list is an absolute path to a file");

            for (path, sub, spelling) in [
                (socket.to_string(), None, socket.to_string()),
                (var.clone(), None, var.clone()),
                (dir.to_string(), Some(file), socket.to_string()),
                (format!("/var{dir}"), Some(file), var.clone()),
            ] {
                let all = host_volume(&path, sub);
                show(&all);

                assert_eq!(
                    all.len(),
                    2,
                    "both containers mount {spelling} — written {path:?} + subPath \
                     {sub:?} — and nothing else is wrong with this pod: {:?}",
                    titles(&all)
                );
                for f in &all {
                    assert!(
                        f.title.contains("drive the container runtime"),
                        "and each of them has the machine, not a directory on it: {}",
                        f.title
                    );
                    assert_eq!(f.severity, Severity::Critical);
                    assert!(
                        f.evidence.contains(&format!("{spelling} on the node")),
                        "the card names the path the manifest wrote — the folding of \
                         `/var/run` onto `/run` belongs to the compare, not to the reader \
                         who has to find this mount in their own YAML: {}",
                        f.evidence
                    );
                }
                assert!(
                    all.iter().any(|f| f.evidence.contains("container shipper")
                        && f.evidence.contains("read-only")),
                    "and `shipper`'s bind is read-only, which is no defence: anything that \
                     can talk to this socket can start a privileged container on the node"
                );
            }
        }
    }

    /// **The shape that drew nothing at all**, and the reason rule 8 escalates on the path
    /// rather than on the mode: a *read-only* bind of the runtime socket on a `kube-system`
    /// DaemonSet, where the writable escalator is deliberately silent (NOTES § D70). With
    /// CRI-O carried under one spelling this pod produced no card — a container able to
    /// drive the runtime, invisible (NOTES § D77).
    #[test]
    fn a_read_only_runtime_socket_on_a_node_agent_is_still_the_whole_machine() {
        let mut pods = items::<Pod>("kube-system-pods");
        let agent = pods
            .iter_mut()
            .find(|p| p.metadata.name.as_deref() == Some("kube-proxy-88dlk"))
            .expect("the capture carries kube-proxy, a DaemonSet D70's narrowing silences");
        let spec = agent.spec.as_mut().expect("a captured pod has a spec");
        spec.volumes.get_or_insert_with(Vec::new).push(Volume {
            name: "runtime".to_string(),
            host_path: Some(HostPathVolumeSource {
                path: "/run/crio/crio.sock".to_string(),
                type_: Some("Socket".to_string()),
            }),
            ..Volume::default()
        });
        spec.containers[0]
            .volume_mounts
            .get_or_insert_with(Vec::new)
            .push(VolumeMount {
                name: "runtime".to_string(),
                mount_path: "/run/crio/crio.sock".to_string(),
                read_only: Some(true),
                ..VolumeMount::default()
            });

        let all = analyze(&pods_at(
            pods.into_iter().map(PodSnapshot::from).collect(),
            now(),
        ));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "the rest of this capture is the healthy kube-system the test above proves \
             silent, so one card is the planted mount and nothing else: {:?}",
            titles(&all)
        );

        let card = only(&all, "kube-proxy-88dlk", "drive the container runtime");
        assert_eq!(card.severity, Severity::Critical);
        assert_eq!(card.owner.kind, ObjectKind::DaemonSet);
        assert!(
            card.evidence.contains("/run/crio/crio.sock on the node")
                && card.evidence.contains("read-only"),
            "it says which socket and that read-only did not save it: {}",
            card.evidence
        );
        // **And the action does not order this pod's mount deleted.** kube-proxy is
        // `kube-system`'s own DaemonSet, which is the shape a legitimate holder has too —
        // an nvidia container-toolkit installer, a Falco or Datadog node agent. The most
        // severe card on the screen must not talk a newcomer at 3am into breaking GPU
        // scheduling or their own security agent, so it carries both halves: remove it if
        // this is not a pod that manages or watches containers, and if it is, know what it
        // holds.
        assert!(
            card.action.contains("unless"),
            "the socket card is drawn on legitimate holders by design (NOTES § D70, § D78) \
             and an unconditional 'remove the mount' is wrong for every one of them: {}",
            card.action
        );
        assert!(
            card.action.contains("manage or watch"),
            "and the exception has to name what those holders actually do: Falco and Datadog \
             *watch* the containers on the node, they do not manage them, and Google's own \
             cAdvisor DaemonSet draws this card off a read-only `/var/run`. A reader who \
             cannot find their agent in the verb removes the mount, which is the failure the \
             half exists to prevent (NOTES § D79): {}",
            card.action
        );
        assert!(
            card.action.contains("every node"),
            "and the half that is true when the mount stays has to be said out loud — this \
             pod has root on every machine it runs on, which is the finding for a reader \
             who keeps the mount: {}",
            card.action
        );
    }

    /// The neighbours of a socket that are not one — names that only *look* like a prefix of
    /// one, a file beside it, a `/var/run` path that is no socket at all, and `/var` itself.
    /// Those last two are what the folding could break: `/var/run/netns` has to reach the
    /// ordinary writable branch **under the name it was written with**, and `/var` folds to
    /// the empty string, which every ancestor test written the obvious way calls a prefix of
    /// everything (NOTES § D78).
    ///
    /// **The last row is the join running the other way.** Everywhere else a `subPath`
    /// widens what the volume said — `/` narrowed to `/run/containerd`. Here it narrows out
    /// of trouble: the volume is `/run`, which is every socket on the node, and the container
    /// is handed `/run/netns`, which is none of them. A check that asked the volume's own
    /// path *as well as* the join would call this the machine (NOTES § D46).
    #[test]
    fn a_path_beside_a_runtime_socket_is_not_a_runtime_socket() {
        for (path, sub, gets) in [
            ("/run/crio.sock.bak", None, "/run/crio.sock.bak"),
            ("/run/crio/crio.sock.bak", None, "/run/crio/crio.sock.bak"),
            ("/run/criox", None, "/run/criox"),
            ("/var/run/netns", None, "/var/run/netns"),
            ("/var", None, "/var"),
            ("/run", Some("netns"), "/run/netns"),
        ] {
            let all = host_volume(path, sub);
            show(&all);
            assert!(
                !all.iter()
                    .any(|f| f.title.contains("drive the container runtime")),
                "{gets} is not a control socket — written {path:?} + subPath {sub:?}: {:?}",
                titles(&all)
            );
            let writable = only(&all, "broken-hostpath", "change files on the machine");
            assert!(
                writable.evidence.contains(&format!("{gets} on the node")),
                "and the ordinary branch names what the container was handed: {}",
                writable.evidence
            );
        }
    }

    /// **A directory above the socket is the socket.** A container handed `/run/containerd`
    /// opens `containerd.sock` inside it, and a container handed `/run` opens all five — the
    /// same capability as mounting the socket file, and until D78 was widened it drew the
    /// writable card, whose advice is *mount it read-only*: giving the node away in the
    /// sentence meant to save it.
    ///
    /// Both spellings again, because the fold and the ancestor match have to compose.
    #[test]
    fn a_directory_above_a_runtime_socket_hands_over_the_same_socket() {
        for dir in [
            "/run",
            "/var/run",
            "/run/containerd",
            "/var/run/containerd",
            "/run/crio",
            "/run/k3s",
            "/run/k3s/containerd",
        ] {
            let all = host_volume(dir, None);
            show(&all);

            assert_eq!(
                all.len(),
                2,
                "both containers are handed {dir}, which is a directory the node's runtime \
                 socket lives under: {:?}",
                titles(&all)
            );
            for f in &all {
                assert!(
                    f.title.contains("drive the container runtime"),
                    "what is inside a mounted directory is mounted too: {}",
                    f.title
                );
                assert_eq!(f.severity, Severity::Critical);
                assert!(
                    f.evidence.contains(&format!("{dir} on the node")),
                    "and the card names the directory the manifest wrote: {}",
                    f.evidence
                );
            }
        }
    }

    /// [`is_runtime_socket`] alone, on the inputs the pipeline cannot hand it and the ones
    /// where a prefix test degenerates.
    ///
    /// **The obvious form of the ancestor match is wrong here and this is what catches it**:
    /// strip a leading `/var`, then ask whether a socket starts with `format!("{path}/")`.
    /// For `/var` the strip yields `""`, the prefix becomes `"/"`, and every socket in the
    /// list matches — a CRITICAL *"can drive the container runtime"* card on a pod that
    /// mounts `/var` and nothing else (NOTES § D78).
    #[test]
    fn only_a_real_ancestor_of_a_runtime_socket_counts_as_one() {
        for quiet in [
            "",
            "/",
            "/var",
            "/varlib/run/docker.sock",
            "/runx",
            "/run/cri",
            "/run/crio.sock.bak",
            "/run/criox",
            "/var/lib",
            "/etc/kubernetes",
        ] {
            assert!(
                !is_runtime_socket(quiet),
                "{quiet:?} is neither a runtime socket nor a directory one lives under, and \
                 a container given it cannot reach the runtime"
            );
        }
        for loud in [
            "/run",
            "/var/run",
            "/run/containerd",
            "/var/run/containerd",
            "/run/crio",
            "/run/k3s",
            "/run/k3s/containerd",
        ] {
            assert!(
                is_runtime_socket(loud),
                "{loud:?} contains a runtime socket, so a container given it has the machine"
            );
        }
        for socket in RUNTIME_SOCKETS {
            assert!(is_runtime_socket(socket), "{socket} is the socket itself");
            assert!(
                is_runtime_socket(&format!("/var{socket}")),
                "/var{socket} is the same file under the name every systemd distribution \
                 also has for it"
            );
        }
    }

    /// Rule 12, both sides of its margin.
    #[test]
    fn the_pod_that_will_not_shut_down_says_when_it_was_asked_and_who_is_holding_it() {
        let raw = fixture("stuck");
        let all = findings(&["stuck"]);
        show(&all);
        assert_eq!(all.len(), 1, "rule 12 alone: {:?}", titles(&all));

        let stuck = only(&all, "broken-stuck", "asked to shut down");
        assert_eq!(stuck.severity, Severity::Warn);
        assert!(
            stuck.evidence.contains("k8rs.test/never-removed"),
            "'a finalizer is holding it' and 'the kubelet has not confirmed it' are two \
             causes with unrelated fixes, and the list is the only thing that tells them \
             apart — `kubectl describe pod` does not print it at all: {}",
            stuck.evidence
        );
        assert!(
            stuck.evidence.contains("on node k8rs-worker2"),
            "{}",
            stuck.evidence
        );
        assert_eq!(
            stuck.kubectl_cmd.as_deref(),
            Some("kubectl get pod broken-stuck -n default -o yaml"),
            "and the command is the one that shows a finalizer, which `describe` does not"
        );

        // **The age is the moment the user asked, not the deadline.** The API server wrote
        // `deletionTimestamp` as request time *plus* the grace period, so the deadline is
        // one grace period late, forever (D46).
        let deadline = captured_time(&raw, &["metadata", "deletionTimestamp"]);
        let grace = at(&raw, &["metadata", "deletionGracePeriodSeconds"])
            .as_i64()
            .expect("the capture carries the grace this delete was granted");
        assert_eq!(
            stuck.timestamp,
            Some(Time(
                deadline
                    .0
                    .checked_sub(SignedDuration::from_secs(grace))
                    .expect("five seconds off a captured moment is representable")
            ))
        );
        assert_ne!(
            stuck.timestamp,
            Some(deadline.clone()),
            "the deadline itself is the field the rule may not report, and this capture's \
             {grace}-second grace is what makes the two different values"
        );

        // **Just inside the margin, and the margin is flat.** `deletionTimestamp` already
        // is request + grace, so the kubelet's SIGKILL lands *at* it; a margin that added
        // the grace a second time would leave a StatefulSet pod with a one-hour
        // `terminationGracePeriodSeconds` invisible a full hour past its kill deadline —
        // and those are exactly the workloads whose stuck termination blocks the rollout
        // this rule exists for. Sixty seconds covers kubelet observation, watch latency and
        // ordinary skew, and is not proportional to a number that was already spent.
        nothing(
            &findings_at(&["stuck"], time("2026-08-12T21:44:04Z")),
            "a minute past the deadline is not yet stuck",
        );
        assert_eq!(
            findings_at(&["stuck"], time("2026-08-12T21:44:05Z")).len(),
            1,
            "one second past the margin it is"
        );
        assert!(
            grace < 60,
            "this capture's grace is smaller than the flat margin, so the two cannot be \
             told apart by the boundary above — what the margin may not do is *scale* with \
             it. **Capture trip:** a stuck pod with `terminationGracePeriodSeconds: 3600`, \
             where the old formula stayed silent for an hour"
        );
        nothing(
            &findings_at(&["stuck"], time("2026-08-12T21:43:00Z")),
            "and before the deadline the pod is shutting down normally, which is the case a \
             rule reading `deletionTimestamp` as the request time would flag",
        );
    }

    /// **The negatives, as a set.** Seven captured pods that are working, including the
    /// four shapes this contract was extended for — a native sidecar, pod-level requests, a
    /// pending in-place resize, a limit declared on the pod and not the container — and one
    /// with no limits at all, which is rule 9's case and belongs to the Capacity report.
    #[test]
    fn every_healthy_capture_produces_no_finding_at_all() {
        let healthy = [
            "healthy",
            "healthy-sidecar",
            "healthy-podlevel",
            "healthy-hostpath",
            "resize",
            "podlimit",
            "nolimits",
        ];
        for name in healthy {
            nothing(
                &findings(&[name]),
                &format!(
                    "nothing in {name}.json is broken *now*, which is the only thing Alerts \
                     holds. Not the same claim as 'this pod is fine': `healthy.json` runs on \
                     `k8rs-worker3`, a node the capture caught `Ready: Unknown` under the \
                     node controller's `unreachable` taint, so its status is a fossil the \
                     kubelet stopped updating. That is N1's finding about the node, and no \
                     pod rule in this box may invent one from a status that stopped moving"
                ),
            );
        }
        nothing(
            &findings(&healthy),
            "and they are silent together as well as apart",
        );
    }

    /// **The pod the rule set could not see** (NOTES § D27), and the card that now names it.
    ///
    /// This test is the previous box's guard, turned over rather than deleted: it asserted
    /// that `broken-init` produced *nothing*, which was true and was the blind spot. What
    /// makes it worth keeping is its shape — it asserts the capture's preconditions before it
    /// asserts the outcome, so a capture whose init container had quietly healed cannot pass
    /// a widened rule set by producing nothing and calling that agreement.
    ///
    /// **The diagnosis is which container, not that a container is broken.** `migrate` is in
    /// `Init:CrashLoopBackOff` with fifteen restarts while `app` sits at `PodInitializing`
    /// waiting for it, and a card that named `migrate` without saying what an init container
    /// *is* reads as an application that will not start — sending the reader to the app's
    /// logs, which are empty, because the app has not run (invariant 14).
    #[test]
    fn the_crash_looping_init_container_is_found_and_the_card_says_what_kind_it_is() {
        let init = pod("init");
        let migrate = container(&init, "migrate");
        assert_eq!(migrate.role, ContainerRole::Init);
        assert!(
            migrate.restarts >= RESTARTS_WARN
                && matches!(&migrate.state, ContainerState::Waiting { reason, .. }
                    if reason.as_deref() == Some("CrashLoopBackOff")),
            "a capture whose init container is healthy proves nothing about the gap: {:?}",
            migrate.state
        );
        assert!(
            matches!(&container(&init, "app").state, ContainerState::Waiting { reason, .. }
                if reason.as_deref() == Some("PodInitializing")),
            "and the app container has to be the *healthy* half of the diagnosis — a pod \
             whose app container was broken too would let a card about `app` pass for a card \
             about `migrate`: {:?}",
            container(&init, "app").state
        );

        let all = findings(&["init"]);
        show(&all);
        assert_eq!(
            all.len(),
            2,
            "rules 1 and 6 on `migrate`, and nothing on `app`: a container that is waiting \
             for the init sequence is not itself broken, and a card about it would send the \
             reader to a log that is empty because the process never ran: {:?}",
            titles(&all)
        );

        for f in &all {
            assert!(
                f.evidence.contains("init container migrate"),
                "the finding has to name the init container — 'the app container is fine, \
                 the one before it is not' is the whole diagnosis (D27): {}",
                f.evidence
            );
            assert!(
                f.evidence
                    .contains("the app starts only after this one finishes"),
                "and it has to say what an init container is, in words that need no \
                 glossary. `init container migrate` alone reads as an application that \
                 will not start (invariant 14): {}",
                f.evidence
            );
        }

        let looping = only(&all, "broken-init", "CrashLoopBackOff");
        assert_eq!(
            looping.severity,
            Severity::Critical,
            "the pod cannot start at all, which is as broken as a pod gets"
        );
        assert!(
            looping.evidence.contains("15 restarts"),
            "the init container's own count, not the app container's zero: {}",
            looping.evidence
        );

        let previous = only(&all, "broken-init", "previous run failed");
        assert_eq!(
            previous.severity,
            Severity::Warn,
            "rule 6 is the WARN beside rule 1's CRITICAL wherever the container is *also* \
             broken right now, and it is the exit code that says why"
        );
    }

    /// **The sidecar's negative, and the precondition without which it proves nothing.**
    ///
    /// `healthy-sidecar.json` is in the healthy set above, and a widened rule set being
    /// silent on it would be a green line whatever its `proxy` container decoded as — a
    /// capture whose sidecar came out `Regular` would assert nothing about the role this box
    /// added. So the role is asserted here, on the object, before the silence is claimed.
    ///
    /// A native sidecar *is* reached by rules 1–6 ([`analyze`]) — a crashlooping mesh proxy
    /// is exactly as broken as a crashlooping app container — so the silence here is the
    /// rules agreeing that a running, ready proxy with no restarts and no previous run is
    /// fine, not the rules failing to look.
    #[test]
    fn a_healthy_native_sidecar_is_looked_at_by_every_rule_and_still_says_nothing() {
        let p = pod("healthy-sidecar");
        let proxy = container(&p, "proxy");
        assert_eq!(
            proxy.role,
            ContainerRole::Sidecar,
            "`restartPolicy: Always` on an init container is what makes it a sidecar \
             (D51) — without this the test below is about a regular container"
        );
        assert!(
            proxy.ready
                && matches!(proxy.state, ContainerState::Running { .. })
                && proxy.restarts == 0
                && proxy.last_terminated.is_none(),
            "and it has to be a *working* sidecar for its silence to mean anything: {proxy:?}"
        );
        nothing(
            &findings(&["healthy-sidecar"]),
            "nothing about this proxy is broken, and the rules that now read its array have \
             to say so as plainly as they do for a regular container",
        );
    }

    /// **Rule 7 did not widen with rules 1–6, and this is the only thing that says so.**
    ///
    /// The narrowing is a deliberate silence, and a silence leaves no card to assert — delete
    /// the role guard in [`running_but_not_ready`] and every committed capture still produces
    /// exactly what it produced before, because no capture holds a sidecar that is running and
    /// not ready. So the shape is written onto a decoded copy of `healthy-sidecar.json`: the
    /// proxy stops passing its readiness check, hours before the pinned [`now`] and well past
    /// [`NOT_READY_GRACE`] (NOTES § D53 — the committed JSON is not touched).
    ///
    /// **The control is the identical edit applied to the regular container beside it**, which
    /// *must* draw the card. Without it this test would pass against a rule 7 that had stopped
    /// working altogether, and it would be asserting that a broken rule is quiet rather than
    /// that a working one is narrow.
    ///
    /// Why the narrowing, rather than a card each: rule 7's sentence sends the reader to *the
    /// readiness probe*, and on a meshed pod the proxy is not the container answering the
    /// traffic. What a not-ready sidecar does to its pod's own readiness is a rule of its own
    /// (invariant 13), and it is not this one wearing a wider filter.
    #[test]
    fn rule_seven_stays_on_the_container_that_answers_the_traffic() {
        /// The capture with one container's readiness flipped, and the pod's `Ready`
        /// condition flipped with it so the object is one the apiserver could have written.
        /// The condition's `lastTransitionTime` is left where the capture put it — it is the
        /// since-when rule 7 measures, and moving it would be moving the goalposts.
        fn unready(name: &str) -> PodSnapshot {
            let mut object: Pod = serde_json::from_value(fixture("healthy-sidecar"))
                .expect("healthy-sidecar.json is a Pod");
            let status = object
                .status
                .as_mut()
                .expect("the captured pod has a status");
            for c in status
                .conditions
                .iter_mut()
                .flatten()
                .filter(|c| c.type_ == "Ready" || c.type_ == "ContainersReady")
            {
                c.status = "False".to_string();
            }
            let found = status
                .init_container_statuses
                .iter_mut()
                .chain(status.container_statuses.iter_mut())
                .flatten()
                .find(|c| c.name == name)
                .unwrap_or_else(|| panic!("the capture has no container {name}"));
            found.ready = false;
            PodSnapshot::from(object)
        }

        let sidecar = unready("proxy");
        let proxy = container(&sidecar, "proxy");
        assert!(
            proxy.role == ContainerRole::Sidecar
                && matches!(proxy.state, ContainerState::Running { .. })
                && !proxy.ready,
            "the edit has to leave a *running* sidecar that is not ready — every other \
             condition of rule 7 is already met by the capture: {proxy:?}"
        );
        nothing(
            &analyze(&pods_at(vec![sidecar], now())),
            "a mesh proxy failing its readiness check is not 'the readiness probe of this \
             application is failing', and the card would send the reader to the wrong probe",
        );

        let app = unready("app");
        assert_eq!(
            container(&app, "app").role,
            ContainerRole::Regular,
            "the control has to be the other role, or it proves nothing"
        );
        let all = analyze(&pods_at(vec![app], now()));
        show(&all);
        only(&all, "healthy-sidecar", "not receiving traffic");
        assert_eq!(
            all.len(),
            1,
            "and the identical edit on the regular container beside it does draw the card — \
             without this the test above would pass against a rule 7 that had stopped firing \
             at all: {:?}",
            titles(&all)
        );
    }

    /// **The init container that failed twice and then worked** — the commonest init
    /// container there is, and the one shape that would have turned this box into two
    /// permanent cards on a healthy pod.
    ///
    /// `healthy.json`'s `migrate` is already the settled half of it: terminated, `exit 0`,
    /// `ready: true`. What no capture holds is the *history* — it succeeded first time, so it
    /// has no restart count and no `lastState`, and rules 5 and 6 are silent on it whatever
    /// [`doing_its_job`] answers. The retry is written onto a **decoded copy**, the technique
    /// this file already uses for a shape no capture holds; the committed JSON is not touched
    /// (NOTES § D53).
    ///
    /// Both numbers are chosen to make the failure loud rather than marginal: fifteen
    /// restarts is over [`RESTARTS_CRITICAL`], so without the suppressor rule 5 draws a
    /// **red** card on a pod that is serving, and the failed previous run puts rule 6's
    /// permanent WARN beside it.
    ///
    /// **The control is the same edit with the last attempt still failing**, and it is what
    /// makes the silence above mean something: the suppressor is about the container having
    /// *succeeded*, not about it being an init container, so an init container that gave up
    /// owes both cards. Without this half the test would pass just as well against a rule set
    /// that had stopped reading init containers altogether.
    ///
    /// **It is the container's *current* state that decides, not `lastState`** — the first
    /// draft of this control varied the previous run's exit code and produced nothing at all,
    /// because the container was still sitting on the capture's own `exit 0` and was
    /// correctly suppressed. A control that cannot fail for the right reason is the defect it
    /// was written to catch, one level up.
    #[test]
    fn an_init_container_that_retried_and_then_succeeded_draws_no_card() {
        /// The retry history written onto the decoded capture — fifteen failures, and a
        /// *current* run that ended with the given code: `0` is the init container that got
        /// there in the end, anything else is the one that gave up.
        fn ended(exit_code: i32) -> PodSnapshot {
            fn run(exit_code: i32, from: &str, to: &str) -> ApiContainerState {
                ApiContainerState {
                    terminated: Some(ContainerStateTerminated {
                        exit_code,
                        reason: Some(
                            if exit_code == 0 { "Completed" } else { "Error" }.to_string(),
                        ),
                        started_at: Some(Time(from.parse().expect("a valid time"))),
                        finished_at: Some(Time(to.parse().expect("a valid time"))),
                        ..ContainerStateTerminated::default()
                    }),
                    ..ApiContainerState::default()
                }
            }

            let mut object: Pod =
                serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
            let migrate = &mut object
                .status
                .as_mut()
                .expect("the captured pod has a status")
                .init_container_statuses
                .as_mut()
                .expect("this pod declares an init container")[0];
            assert_eq!(migrate.name, "migrate");
            migrate.restart_count = 15;
            migrate.state = Some(run(
                exit_code,
                "2026-08-12T20:45:04Z",
                "2026-08-12T20:45:06Z",
            ));
            migrate.last_state = Some(run(1, "2026-08-12T20:45:00Z", "2026-08-12T20:45:02Z"));
            PodSnapshot::from(object)
        }

        let succeeded = ended(0);
        let migrate = container(&succeeded, "migrate");
        assert_eq!(migrate.role, ContainerRole::Init);
        assert!(
            migrate.restarts >= RESTARTS_CRITICAL
                && matches!(&migrate.state, ContainerState::Terminated(run) if run.exit_code == 0),
            "the edit has to land on a *finished* init container carrying enough restarts to \
             reach rule 5's red band, or the silence below is unearned: {migrate:?}"
        );

        nothing(
            &analyze(&pods_at(vec![succeeded.clone()], now())),
            "this init container did what it was asked to do — it finished, and the pod has \
             been serving ever since. Its restart count is frozen and its failed previous run \
             is kept for the life of the pod, so a card here is permanent and there is \
             nothing behind it to act on (D2)",
        );

        // The other side of the same edit: the suppressor is about *success*, not about the
        // role. An init container that stopped on a non-zero code is why a pod is not
        // starting, and both rules owe it a card.
        let failed = ended(1);
        let all = analyze(&pods_at(vec![failed], now()));
        show(&all);
        assert_eq!(
            all.len(),
            2,
            "rules 5 and 6 on an init container that gave up: {:?}",
            titles(&all)
        );
        assert_eq!(
            only(&all, "healthy", "restarted 15 times").severity,
            Severity::Critical,
            "and the band is the one `!serving` puts it in — which is exactly the red card \
             the successful run above must not draw"
        );
    }

    /// A committed capture with one field moved — the technique the rest of this file uses
    /// for a shape no capture holds. The committed JSON is never touched (NOTES § D53); the
    /// decoded copy is.
    fn capture_but(name: &str, edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
        let mut object: Pod = serde_json::from_value(fixture(name))
            .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
        edit(&mut object);
        PodSnapshot::from(object)
    }

    /// The captured Pending pod, edited — rule 10's shapes all start here.
    fn pending_but(edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
        capture_but("pending", edit)
    }

    /// One entry of a captured pod's condition array, by type, to be written through.
    fn pod_condition<'a>(pod: &'a mut Pod, type_: &str) -> &'a mut PodCondition {
        pod.status
            .as_mut()
            .and_then(|s| s.conditions.as_mut())
            .into_iter()
            .flatten()
            .find(|c| c.type_ == type_)
            .unwrap_or_else(|| panic!("the capture carries no {type_} condition to edit"))
    }

    /// The `PodScheduled` entry, which is the one every rule-10 shape moves.
    fn scheduled_condition(pod: &mut Pod) -> &mut PodCondition {
        pod_condition(pod, "PodScheduled")
    }

    /// One entry of a captured pod's status arrays, by name — init containers and regular
    /// ones searched together, the way [`container_snapshots`] reads them.
    fn container_status<'a>(pod: &'a mut Pod, name: &str) -> &'a mut ContainerStatus {
        let status = pod.status.as_mut().expect("the capture has a status");
        status
            .init_container_statuses
            .iter_mut()
            .chain(status.container_statuses.iter_mut())
            .flatten()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("the capture reports on no container {name}"))
    }

    /// A container status rewritten to *waiting*, with the kubelet's reason and sentence —
    /// the shape rule 13's positives are built out of.
    fn waiting_at(reason: &str, message: Option<&str>) -> Option<ApiContainerState> {
        Some(ApiContainerState {
            waiting: Some(ContainerStateWaiting {
                reason: Some(reason.to_string()),
                message: message.map(str::to_string),
            }),
            ..ApiContainerState::default()
        })
    }

    /// **Rule 10, and the fixture that would break a rule shaped like its neighbours.**
    /// `broken-pending` has no `containerStatuses` at all — the kubelet never saw it — so
    /// every rule in this file that loops over containers is structurally silent on it, and
    /// the one rule that is *about* it has to read the pod.
    ///
    /// The scheduler's own sentence is the card, verbatim (NOTES § D27, § D37): it is the
    /// answer to the question a beginner asks most often, and no paraphrase of it can name
    /// which nodes refused and for what.
    #[test]
    fn the_pending_pod_carries_the_schedulers_verdict_and_the_sentence_behind_it() {
        let raw = fixture("pending");
        let all = findings(&["pending"]);
        show(&all);
        assert_eq!(all.len(), 1, "rule 10 alone: {:?}", titles(&all));

        assert!(
            pod("pending").containers.is_empty(),
            "a capture whose kubelet had reported on a container would let a \
             container-shaped rule pass this test by accident, and rule 10's whole subject \
             is the pod no kubelet has seen"
        );

        let unplaced = only(&all, "broken-pending", "will take this pod");
        assert_eq!(
            unplaced.severity,
            Severity::Critical,
            "this capture is three hours past its refusal, which is well outside the window \
             below — nothing is going to place it until a human acts"
        );
        assert!(
            unplaced.title.contains("No machine in the cluster")
                && unplaced.title.contains("Pending"),
            "invariant 14: the sentence explains what happened, and then names the word the \
             reader is staring at in `kubectl get pods`: {}",
            unplaced.title
        );

        // **Equality against the capture's own bytes.** D37 is the whole rule here: the
        // scheduler counts the nodes and says what each one refused the pod for, and a
        // finding that summarised, truncated or re-punctuated that has thrown away the only
        // thing it had to offer. `contains` would pass on a card that appended to it.
        let sentence = captured_str(captured_condition(&raw, "PodScheduled"), &["message"]);
        assert_eq!(
            unplaced.evidence,
            format!("the scheduler's own words (a node is one machine): {sentence}"),
            "quoted whole, and framed so a newcomer reads it as a quote rather than as \
             k8rs's own prose — and the four-word gloss is the only thing on this card \
             joining the title's *machine* to the quote's four *node*s (invariant 14)"
        );
        assert!(
            unplaced.evidence.contains("nodes are available"),
            "and the sentence still counts the machines that refused it — a capture whose \
             message no longer does is not this rule's fixture: {}",
            unplaced.evidence
        );

        // **The condition's own transition, which is the *first* refusal.**
        // `UpdatePodCondition` carries the old stamp forward while the status has not
        // changed, and the scheduler rewrites this condition on every retry with the same
        // `False` — so this dates the moment the pod became unplaceable, not the last
        // attempt at it.
        assert_eq!(
            unplaced.timestamp,
            Some(captured_time(
                captured_condition(&raw, "PodScheduled"),
                &["lastTransitionTime"]
            ))
        );
        assert_eq!(
            unplaced.age(&now()).as_deref(),
            Some("3 hours ago"),
            "a duration, not English parsed back into a number"
        );

        // `describe` prints conditions as a Type/Status table with no reason and no
        // message. It does print Events, and the scheduler re-emits `FailedScheduling` on
        // every retry, so the sentence usually *is* reachable there — but an Event expires
        // at `--event-ttl` and a field does not, which is the narrower form of rules 3 and
        // 4's argument (invariant 4). `-o yaml` also shows `spec.affinity`, which this
        // capture's own message blames and `describe` never prints.
        assert_eq!(
            unplaced.kubectl_cmd.as_deref(),
            Some("kubectl get pod broken-pending -n default -o yaml")
        );
        assert!(
            !unplaced.action.contains("the machines have"),
            "the action may only ask for what the command beside it can answer: the node \
             side of that comparison is `kubectl get nodes --show-labels`, and it is N6's \
             to make: {}",
            unplaced.action
        );
    }

    /// **Rule 10's severity ladder, both sides of it.** A flat CRITICAL rested on *"a pod
    /// that places normally never carries this"*, and three routine paths falsify it — an
    /// autoscaler scale-up (where this condition is the *trigger*), `Immediate`-mode volume
    /// provisioning on a fresh StatefulSet replica, and node-group rollover. None needs a
    /// human, and CRITICAL in this file means *this will not run until someone acts*.
    ///
    /// The card is immediate either way — the scheduler's sentence is the good half and it
    /// does not wait. Only the colour does.
    #[test]
    fn a_refusal_the_cluster_may_still_fix_itself_is_amber_until_it_has_had_ten_minutes() {
        // The captured refusal is at 20:45:53Z. Exactly [`NOT_READY_GRACE`] later, which is
        // an autoscaler that has not finished bringing a node up.
        let early = findings_at(&["pending"], time("2026-08-12T20:55:53Z"));
        show(&early);
        assert_eq!(
            early.len(),
            1,
            "the card is immediate — a beginner gets the scheduler's sentence at once, and \
             only the band waits: {:?}",
            titles(&early)
        );
        assert_eq!(
            early[0].severity,
            Severity::Warn,
            "ten minutes unplaced is a scale-up in progress, not an outage — and rule 13 in \
             this same phase takes WARN plus this same window for one healthy look-alike, \
             where this rule has three"
        );

        // One second past it.
        let late = findings_at(&["pending"], time("2026-08-12T20:55:54Z"));
        assert_eq!(
            late[0].severity,
            Severity::Critical,
            "past `progressDeadlineSeconds`' own default is where Kubernetes itself stops \
             calling a rollout healthy, and it is the window rules 7 and 13 borrow — not a \
             number picked for this rule"
        );

        // **No stamp is not read as recent.** A pod that cannot be shown to have just
        // become unplaceable is read as one that has been that way, which is the safe
        // direction — and it is the shape a Kueue-gated pod arrives in from the other
        // side, carrying a *gating* stamp older than its own unschedulability.
        let stampless = pending_but(|p| {
            scheduled_condition(p).last_transition_time = None;
        });
        let all = analyze(&pods_at(vec![stampless], time("2026-08-12T20:45:54Z")));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "a missing stamp costs the age, never the card — rule 7 is the rule that has no \
             finding without a since-when, and this one stands on the verdict alone: {:?}",
            titles(&all)
        );
        assert_eq!(all[0].timestamp, None, "and the right edge is blank");
        assert_eq!(
            all[0].severity,
            Severity::Critical,
            "one second after the capture's own refusal, which would be WARN with a stamp — \
             so this is the absence deciding, not the clock"
        );
    }

    /// **Rule 10's negatives, and the two that matter are Pending for a different reason.**
    /// `Pending` is the phase of a pod waiting on an image pull and of one waiting on a
    /// ConfigMap, and rules 3 and 4 already explain both — a rule 10 that read the phase,
    /// or that read the condition's presence rather than its value, would put a second and
    /// wrong card on each of them.
    #[test]
    fn a_pod_pending_for_a_reason_that_is_not_the_scheduler_gets_no_rule_ten_card() {
        // The negatives are asserted to be in the shape that could trip the rule, before
        // their silence is worth anything: both really are `Pending`, and both really do
        // carry the condition rule 10 reads.
        for name in ["image", "config"] {
            let p = pod(name);
            let scheduled = p
                .scheduled
                .as_ref()
                .expect("the condition does not go away once a node accepts the pod");
            println!(
                "{}: phase={:?} PodScheduled={} reason={:?}",
                p.id.name, p.phase, scheduled.status, scheduled.reason
            );
            assert_eq!(
                p.phase.as_deref(),
                Some("Pending"),
                "a pod whose image will not pull is Pending, and this is the capture that \
                 makes 'Pending' the wrong thing for rule 10 to read"
            );
            assert_eq!(
                scheduled.status, "True",
                "a node did accept it — what is stuck is what happened afterwards"
            );
        }

        let all = findings(&["image", "config"]);
        show(&all);
        assert_eq!(
            all.iter()
                .filter(|f| f.title.contains("will take this pod"))
                .count(),
            0,
            "rules 3 and 4 own these two pods, and a second card saying no machine would \
             have them is both wrong and the loudest thing on the screen: {:?}",
            titles(&all)
        );
        assert_eq!(
            all.len(),
            2,
            "one card each, exactly as before rule 10 existed: {:?}",
            titles(&all)
        );

        // The healthy pod is the other half: `PodScheduled` is `True` there with no reason
        // at all, so a rule testing `scheduled.is_some()` would fire on every working pod
        // in the cluster.
        nothing(
            &findings(&["healthy"]),
            "a scheduled pod keeps the condition rather than dropping it, so presence is \
             not what this rule may test",
        );
    }

    /// **The half of rule 10's gate no capture can reach.** Every fixture carrying
    /// `reason: Unschedulable` also carries `status: "False"`, and every fixture at
    /// `status: "True"` carries no reason at all — so a rule that dropped the status check
    /// and read the reason alone leaves this suite green, and only this test says
    /// otherwise. Measured, not assumed: the gate was mutated to the reason alone and all
    /// 66 tests passed.
    ///
    /// The two are separate strings on `status.conditions`, which is a subresource anyone
    /// with `patch pods/status` may write — a stale or planted `reason` beside a `True`
    /// status is not something the scheduler produces, and it is exactly what invariant 9's
    /// "free text from the API is untrusted" means one level up from a string: a *field
    /// combination* the object model permits and the controller never emits. The card it
    /// would draw is the worst one available here, `No machine in the cluster will take
    /// this pod` over a pod that is running and serving.
    ///
    /// One field, on a real captured object, exactly as the `subPathExpr` and DaemonSet
    /// tests do it — the capture is not edited (NOTES § D53).
    #[test]
    fn a_scheduled_pod_carrying_the_unschedulable_reason_anyway_is_not_a_finding() {
        let mut object: Pod =
            serde_json::from_value(fixture("healthy")).expect("healthy.json is a Pod");
        let condition = object
            .status
            .as_mut()
            .and_then(|s| s.conditions.as_mut())
            .into_iter()
            .flatten()
            .find(|c| c.type_ == "PodScheduled")
            .expect("the captured healthy pod keeps its PodScheduled condition");
        condition.reason = Some("Unschedulable".to_string());
        assert_eq!(
            condition.status, "True",
            "and the status is left as the cluster wrote it — the whole point is the pair"
        );

        let p = PodSnapshot::from(object);
        println!("{:?}", p.scheduled);
        nothing(
            &analyze(&pods_at(vec![p], now())),
            "a pod a node accepted is running, whatever reason string is sitting beside \
             that condition — **and neither half of this gate is redundant**: the reason \
             half is what excludes a gated pod, asserted in \
             `a_pod_the_scheduler_never_judged_is_not_a_pod_it_refused`",
        );
    }

    /// **The two `PodScheduled: False` reasons that are not a refusal**, and the test that
    /// holds the reason half of rule 10's gate in place. Cutting `reason` out of the gate
    /// leaves every other test in this file green.
    ///
    /// `SchedulingGated` is a pod its author asked to be held back — `spec.schedulingGates`,
    /// which is how Kueue, Volcano and Yunikorn queue work — so a CRITICAL on it is k8rs
    /// contradicting a decision the user made, once per queued pod, on a cluster whose whole
    /// point is that the queue is long. `SchedulerError` is an internal failure the
    /// scheduler retries by itself.
    ///
    /// Both are synthesized from the real refusal rather than captured, because three lines
    /// on a committed object is not a capture trip — the shape is one field of one string.
    #[test]
    fn a_pod_the_scheduler_never_judged_is_not_a_pod_it_refused() {
        for reason in ["SchedulingGated", "SchedulerError"] {
            let p = pending_but(|pod| {
                scheduled_condition(pod).reason = Some(reason.to_string());
            });
            let scheduled = p.scheduled.as_ref().expect("the condition is still there");
            println!(
                "{}: PodScheduled={} reason={:?}",
                p.id.name, scheduled.status, scheduled.reason
            );
            assert_eq!(
                scheduled.status, "False",
                "the status is left exactly as the scheduler wrote it — if these were \
                 `True` the status half of the gate would be excluding them and this test \
                 would prove nothing about the reason half"
            );
            nothing(
                &analyze(&pods_at(vec![p], now())),
                &format!(
                    "`{reason}` is not `Unschedulable`: nothing has refused this pod, so \
                     there is no verdict to report and no scheduler sentence to quote"
                ),
            );
        }
    }

    /// **The unscheduled pod somebody deleted — rule 10 hands it to rule 12 and says
    /// nothing.** Both cards would be *true* on it: it is unplaceable, and it is not going
    /// away. Rule 10's action is what disqualifies it — *check what this pod asks for* sends
    /// the reader to audit `nodeSelector`, affinity and requests when the only move left is
    /// finding what is holding the delete, which is rule 12's card and rule 12 names the
    /// finalizer. Alerts is D2's queue of what is broken now **and actionable**, and where
    /// a pod could have run stops being actionable once someone has asked for it to go.
    ///
    /// **It also removes the two-word problem rather than managing it.** `printPod`
    /// overrides the STATUS column to `Terminating` whenever `deletionTimestamp` is set and
    /// the phase is not terminal, while `phase` itself stays `Pending` — which is why
    /// `stuck.json` is `phase: Running` and still shows as Terminating. So this pod would
    /// have drawn rule 10 saying *"it shows as Pending"* beside rule 12 saying
    /// *"it shows as Terminating"*, about one pod, on one screen. The card that had the
    /// wrong word is the card that had no business being there.
    ///
    /// This test asserted that pair agreeing until 2026-08-13; it now asserts rule 10 is
    /// absent, and it is the red run for the `deletion_timestamp` guard.
    #[test]
    fn the_deleted_pod_is_rule_twelves_alone_and_rule_ten_stands_down() {
        let deleted = pending_but(|pod| {
            pod.metadata.deletion_timestamp = Some(time("2026-08-12T20:46:23Z"));
            pod.metadata.deletion_grace_period_seconds = Some(30);
            pod.metadata.finalizers = Some(vec!["k8rs.test/never-removed".to_string()]);
        });
        assert_eq!(
            deleted.scheduled.as_ref().and_then(|c| c.reason.as_deref()),
            Some("Unschedulable"),
            "the trigger is untouched — this pod still satisfies rule 10's gate, which is \
             what makes the silence below the deletion's doing"
        );
        assert_eq!(
            deleted.phase.as_deref(),
            Some("Pending"),
            "and the phase does not move when a pod is deleted, which is why the \
             parenthetical's `phase` check could never have closed this on its own"
        );

        let all = analyze(&pods_at(vec![deleted], now()));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "rule 12 alone: a pod on its way out is rule 12's, and rule 10's action points \
             the reader at the wrong half of the object: {:?}",
            titles(&all)
        );
        let terminating = only(&all, "broken-pending", "asked to shut down");
        assert!(
            terminating.title.contains("Terminating"),
            "and the one card left names the word `kubectl get pods` actually prints: {}",
            terminating.title
        );
        assert!(
            terminating.evidence.contains("k8rs.test/never-removed"),
            "with the finalizer, which is the only thing anyone can act on here: {}",
            terminating.evidence
        );

        // **The minute before rule 12's margin opens draws nothing at all**, and that is
        // correct rather than a hole: for that minute the pod is deleting normally, and
        // neither rule has anything to say about a delete that was accepted seconds ago.
        nothing(
            &analyze(&pods_at(
                vec![pending_but(|pod| {
                    pod.metadata.deletion_timestamp = Some(time("2026-08-12T20:46:23Z"));
                    pod.metadata.deletion_grace_period_seconds = Some(30);
                })],
                time("2026-08-12T20:47:00Z"),
            )),
            "inside rule 12's margin the delete is still in progress, and rule 10 has \
             already stood down — a deliberate gap, not an unhandled one",
        );
    }

    /// **The pod preemption has already found a machine for** — where rule 10's trigger is
    /// true and its sentence is false, which is the one shape those two come apart in.
    ///
    /// kube-scheduler writes `status.nominatedNodeName` in the *same* status patch that
    /// sets `PodScheduled: False / Unschedulable`, and the pair stands for the whole
    /// graceful termination of the victims — 30s by default, minutes with a real grace or a
    /// `preStop` hook, unbounded when a victim will not go. Through all of it the card
    /// would read *"no machine in the cluster will take this pod"* while the API says which
    /// machine is being cleared for it.
    #[test]
    fn a_pod_with_a_machine_already_being_cleared_for_it_is_not_a_pod_nothing_will_take() {
        let nominated = pending_but(|pod| {
            pod.status
                .as_mut()
                .expect("the captured pod has a status")
                .nominated_node_name = Some("k8rs-worker2".to_string());
        });
        println!(
            "nominated={:?} scheduled={:?}",
            nominated.nominated_node_name, nominated.scheduled
        );
        assert_eq!(
            nominated
                .scheduled
                .as_ref()
                .and_then(|c| c.reason.as_deref()),
            Some("Unschedulable"),
            "the trigger is untouched — this pod satisfies every other condition of rule \
             10, which is what makes the silence below the nomination's doing"
        );

        nothing(
            &analyze(&pods_at(vec![nominated], now())),
            "a machine has been chosen and is being cleared, so 'no machine will take this \
             pod' is false — and *'a machine has been chosen, it is waiting for other pods \
             there to shut down'* is a new rule, not a branch of this one (invariant 13). \
             Rule 12 already covers the half that goes wrong, on the victim",
        );
    }

    // --- RULE 13, THE RESIDUAL ---
    //
    // The rule with no positive capture (NOTES § D72), so the order below is the reverse of
    // every other rule's: the negatives are committed captures and the positives are
    // decoded copies. That is not a weaker proof of the negatives — `image.json` and
    // `config.json` are the two pods in the repository that match rule 13's gate in every
    // respect *except* the residual clause, and they are real.

    /// **The two captures rule 13 would fire on if it were not a residual**, and they are
    /// the hardest negatives in the file because nothing about them is synthetic.
    ///
    /// `image.json` and `config.json` are both `phase: Pending`, both `PodScheduled: True`,
    /// both have a container that has never run, and both are **three hours** older than the
    /// pinned `now` — so they clear the ten-minute grace with room to spare and satisfy
    /// every clause of [`placed_but_never_started`] except the one that matters. Dropping
    /// either exclusion — [`EXPLAINED_ELSEWHERE`] or [`UNUSABLE_IMAGE`] — puts a second card
    /// on each: *"it has not been able to start"* beside *"the image is not usable"*, which
    /// is the same incident said twice and is exactly the failure a residual rule risks.
    #[test]
    fn the_two_pods_that_look_like_a_wedge_are_already_explained_by_rules_three_and_four() {
        for (name, phrase) in [
            ("image", "image is not usable"),
            ("config", "ConfigMap or Secret that does not exist"),
        ] {
            let p = pod(name);
            let scheduled = p
                .scheduled
                .as_ref()
                .expect("the capture carries PodScheduled");
            let since = scheduled
                .last_transition
                .as_ref()
                .expect("and the moment it was placed");
            println!(
                "{name}: scheduled={} at {since:?}, {:?} before the pin; containers {:?}",
                scheduled.status,
                now().0.duration_since(since.0),
                p.containers
                    .iter()
                    .map(|c| (&c.name, &c.state, c.last_terminated.is_some()))
                    .collect::<Vec<_>>(),
            );

            // The preconditions are asserted before the outcome, so a capture that had
            // quietly stopped matching rule 13's gate cannot pass this by producing one
            // finding for the wrong reason.
            assert_eq!(
                scheduled.status, "True",
                "{name} is on a machine — this is not rule 10's pod"
            );
            assert!(
                now().0.duration_since(since.0) > NOT_READY_GRACE,
                "{name} was placed {:?} before the pin, and a capture inside the grace \
                 would make the silence below the *clock's* doing rather than the \
                 residual's",
                now().0.duration_since(since.0)
            );
            assert!(
                p.containers.iter().all(|c| c.last_terminated.is_none()
                    && matches!(c.state, ContainerState::Waiting { .. })),
                "and not one of its containers has ever run: {:?}",
                p.containers.iter().map(|c| &c.state).collect::<Vec<_>>()
            );

            let all = findings(&[name]);
            show(&all);
            assert_eq!(
                all.len(),
                1,
                "one incident, one card: {name}.json is explained by the rule that owns its \
                 waiting reason, and rule 13 is what is left *after* those rules — not a \
                 twelfth opinion on the same pod: {:?}",
                titles(&all)
            );
            only(&all, &p.id.name, phrase);
        }
    }

    /// **A migration that is simply taking a long time**, which is the false-positive class
    /// this whole rule is trying not to become.
    ///
    /// Rules 1–6 read init containers now, so a *broken* one gets its own card. A long one
    /// is a different thing entirely: a database migration or a large restore leaves every
    /// regular container at `PodInitializing` for as long as it runs, and nothing is wrong.
    /// Ten minutes is nothing for that work.
    ///
    /// **Both halves are silent for the same reason, and it is not the waiting reason
    /// itself.** [`WAITING_ON_A_SIBLING`] is uninformative on its own — the kubelet writes it
    /// on every container of a pod that declares an init container, wedged or not, which is
    /// what [`a_pod_that_only_ever_says_podinitializing_is_the_wedge_the_rule_was_added_for`]
    /// is about. What silences these two is that there **is** something to point at: here a
    /// running init container, and in the committed `init.json` an init container carrying
    /// `CrashLoopBackOff`, which is rule 1's card. Two cards there, both about `migrate`, and
    /// none about `app`.
    #[test]
    fn an_init_container_still_doing_its_work_is_not_a_pod_that_never_started() {
        // The capture, unedited: `migrate` is looping and `app` is behind it.
        let captured = findings(&["init"]);
        show(&captured);
        assert!(
            captured.iter().all(|f| f.evidence.contains("migrate")),
            "every card on this pod is about the init container that is failing — a card \
             naming `app` sends the reader to logs that are empty, because the app has not \
             run (D27): {:?}",
            titles(&captured)
        );

        // The same pod with the migration *running* instead of looping — twenty minutes
        // into work that legitimately takes an hour.
        let running = capture_but("init", |pod| {
            let migrate = container_status(pod, "migrate");
            migrate.state = Some(ApiContainerState {
                running: Some(ContainerStateRunning {
                    started_at: Some(time("2026-08-12T23:40:00Z")),
                }),
                ..ApiContainerState::default()
            });
            // First attempt, and still on it: the crash loop's history goes with the loop,
            // or rules 5 and 6 answer this test instead of rule 13.
            migrate.restart_count = 0;
            migrate.last_state = None;
        });
        let migrate = container(&running, "migrate");
        let app = container(&running, "app");
        println!("migrate={:?}\napp={:?}", migrate.state, app.state);
        assert!(
            matches!(migrate.state, ContainerState::Running { .. }),
            "the edit has to land on a running init container, or the silence below is the \
             crash loop's doing and not this rule's: {migrate:?}"
        );
        assert!(
            matches!(&app.state, ContainerState::Waiting { reason, .. }
                if reason.as_deref() == Some(WAITING_ON_A_SIBLING)),
            "and the app container has to still be the one waiting its turn — that reason \
             is what this test is about: {app:?}"
        );

        nothing(
            &analyze(&pods_at(vec![running], now())),
            "the pod was placed three hours ago and its app container has never started, so \
             every other clause of rule 13 holds — and it is a migration doing its job. The \
             running init container is both what `PodInitializing` is pointing at and what \
             makes *it has not been able to start* false about this pod; firing here would \
             put a card on every slow migration in the cluster (D2)",
        );
    }

    /// **The wedge itself, in the two shapes a real kubelet produces** — the rule's only
    /// positives, and they are decoded copies because no committed capture is in either
    /// state (NOTES § D72, and the capture-trip item on [`placed_but_never_started`]).
    ///
    /// **What the card has to say is decided by the order the kubelet does its work, not by
    /// what this rule happens to return.** `kubelet.SyncPod` waits for volumes to attach and
    /// mount *before* the runtime creates the sandbox, so:
    ///
    /// - **storage or network missing → the condition is `False`.** A `configMap` volume
    ///   naming an object that does not exist — D72's own proposed capture shape — never
    ///   reaches the sandbox at all.
    /// - **anything after that → the condition is `True`.** The sandbox exists, which is
    ///   itself proof the mounts succeeded, so a card blaming a disk here is contradicted by
    ///   the very field it is reading.
    ///
    /// The first draft of this test asserted the opposite of both, because it asserted what
    /// the implementation returned instead of what the requirement says — which is how the
    /// inversion shipped past a green suite. Each half below therefore also asserts the
    /// sentence it must **not** carry: a swap of the two branches has to fail here, and
    /// "contains the right words" alone would survive it.
    ///
    /// `config.json` is the base rather than a hand-written pod: it is already a scheduled,
    /// three-hour-old pod whose single container has never run, which is every clause of the
    /// gate (D40, D53 — the committed JSON is untouched).
    #[test]
    fn the_wedged_pod_names_the_side_of_the_sandbox_the_kubelet_actually_stopped_on() {
        let wedged = |reason: &'static str, condition: Option<&'static str>| {
            capture_but("config", move |pod| {
                container_status(pod, "app").state = waiting_at(reason, None);
                match condition {
                    Some(status) => {
                        pod_condition(pod, "PodReadyToStartContainers").status = status.to_string();
                    }
                    None => pod
                        .status
                        .as_mut()
                        .and_then(|s| s.conditions.as_mut())
                        .expect("the capture has conditions")
                        .retain(|c| c.type_ != "PodReadyToStartContainers"),
                }
            })
        };
        let one_card = |p: PodSnapshot| {
            let all = analyze(&pods_at(vec![p], now()));
            show(&all);
            assert_eq!(
                all.len(),
                1,
                "rule 13 alone: nothing else in the file reads a container waiting for a \
                 reason no rule owns: {:?}",
                titles(&all)
            );
            only(&all, "broken-config", "not been able to start").clone()
        };

        // --- BEFORE THE SANDBOX: the volume wedge, which is what `False` means ---
        let card = one_card(wedged("ContainerCreating", Some("False")));
        assert_eq!(
            card.severity,
            Severity::Warn,
            "the one healthy thing that still looks exactly like this is a slow pull, and a \
             red card that is sometimes a slow pull is how red stops meaning broken (D2)"
        );
        assert!(
            card.evidence.contains("container app") && card.evidence.contains("ContainerCreating"),
            "the card names the container and quotes the machine's own word for where it \
             stopped — the reasons a kubelet can be stuck on are an open set, so the word is \
             passed through rather than translated: {}",
            card.evidence
        );
        assert!(
            card.evidence.contains("storage"),
            "`False` is written before the sandbox exists, and volumes are attached before \
             the sandbox too — so this is the missing-ConfigMap-volume pod, and a card that \
             names only the network sends its reader to the CNI over a storage fault: {}",
            card.evidence
        );
        assert!(
            !card.evidence.contains("the block is later"),
            "and it must not claim the pod already has what it is waiting for — this is the \
             half of the inversion that told a reader their disks were fine: {}",
            card.evidence
        );
        assert_eq!(
            card.timestamp.as_ref(),
            pod("config")
                .scheduled
                .as_ref()
                .and_then(|c| c.last_transition.as_ref()),
            "the since-when is the moment the scheduler placed it, which is when the machine \
             became responsible for starting it"
        );
        assert_eq!(
            card.kubectl_cmd.as_deref(),
            Some("kubectl describe pod broken-config -n default"),
            "and the command is `describe` and not `-o yaml`, unlike every other card whose \
             evidence is a field: what finishes this diagnosis is a `FailedMount` Event, \
             which only `describe` prints"
        );

        // --- AFTER THE SANDBOX: `True` is proof the mounts already succeeded ---
        for (label, p) in [
            ("still pulling", wedged("ContainerCreating", Some("True"))),
            (
                "the container could not be created",
                wedged("CreateContainerError", Some("True")),
            ),
            // Absent is not a third case: an old server or a kubelet that has said nothing
            // is read as "not False", the only claim that survives both.
            ("no condition at all", wedged("ContainerCreating", None)),
        ] {
            let card = one_card(p);
            assert!(
                card.evidence.contains("storage and its network"),
                "{label}: the sandbox exists, so the mounts succeeded and the network is up \
                 — the card says so and points past them: {}",
                card.evidence
            );
            assert!(
                !card.evidence.contains("has not been able to give"),
                "{label}: and it must not blame storage the pod demonstrably has. This is \
                 the half of the inversion that sent someone hunting a disk while an image \
                 downloaded: {}",
                card.evidence
            );
        }
    }

    /// **The pod that reports `PodInitializing` and nothing else**, which is the shape rule
    /// 13 was silent on when it first shipped — and it is most production pods.
    ///
    /// The kubelet's `defaultWaitingState` is `PodInitializing` for **both** status arrays
    /// whenever a pod declares an init container, so an Istio- or Linkerd-injected pod, a
    /// `vault-agent-init` pod or most Helm charts report exactly this while wedged on a
    /// missing volume: every container says the same uninformative word and the real reason
    /// appears nowhere in the status at all. Reading that word as a pointer — *another
    /// container goes first* — silenced the rule on the whole class it was added for.
    ///
    /// **The preconditions are asserted first**, because a copy that had quietly kept a real
    /// reason on one container would fire for the ordinary residual reason and pass this
    /// test without ever exercising the branch it is about.
    #[test]
    fn a_pod_that_only_ever_says_podinitializing_is_the_wedge_the_rule_was_added_for() {
        let injected = capture_but("init", |pod| {
            let migrate = container_status(pod, "migrate");
            migrate.state = waiting_at(WAITING_ON_A_SIBLING, None);
            migrate.restart_count = 0;
            migrate.last_state = None;
        });
        println!(
            "{:?}",
            injected
                .containers
                .iter()
                .map(|c| (&c.name, &c.state))
                .collect::<Vec<_>>()
        );
        assert!(
            injected.containers.len() == 2
                && injected.containers.iter().all(|c| matches!(
                    &c.state,
                    ContainerState::Waiting { reason, .. }
                        if reason.as_deref() == Some(WAITING_ON_A_SIBLING)
                )),
            "every container has to carry the default waiting state and nothing else, or \
             this fires for an ordinary residual reason and proves nothing: {:?}",
            injected.containers
        );
        assert!(
            nothing_else_to_point_at(&injected),
            "and there has to be nothing for that word to point at — no container running, \
             none carrying a reason of its own"
        );

        let all = analyze(&pods_at(vec![injected], now()));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "one card: this pod has been on a machine for three hours, nothing in it has \
             started, and `PodInitializing` is the only thing it has said — which is exactly \
             as wedged as one saying `ContainerCreating`, and was silence before: {:?}",
            titles(&all)
        );
        let card = only(&all, "broken-init", "not been able to start");
        assert!(
            card.evidence.contains("has not said which step it is on"),
            "and the card says the machine named no step, rather than quoting \
             `PodInitializing` as if it were one — it is the kubelet's default waiting \
             state, and dressing the least informative string in the status up as a \
             diagnosis is invariant 14 backwards: {}",
            card.evidence
        );
    }

    /// **A pod with something already serving is not a pod that has not been able to
    /// start**, and the title is the whole reason for the skip.
    ///
    /// One typo in a sidecar's image leaves a pod `kubectl get pods` shows as `1/2`. A card
    /// saying the pod has not started sends the reader to debug the container that has been
    /// answering traffic for three minutes — and nothing else in [`analyze`] filters that
    /// pod out, because it stays `phase: Pending`.
    ///
    /// **What this costs is named rather than hidden:** the wedged container here draws no
    /// card from any rule in the file. That is the trade — a true silence over a confident
    /// false sentence ([`placed_but_never_started`]).
    #[test]
    fn a_pod_with_something_already_serving_gets_no_card_saying_it_never_started() {
        let half_up = capture_but("healthy-sidecar", |pod| {
            container_status(pod, "app").state = waiting_at(
                "CreateContainerError",
                Some("failed to create containerd task"),
            );
        });
        let proxy = container(&half_up, "proxy");
        let app = container(&half_up, "app");
        println!("proxy={:?}\n  app={:?}", proxy.state, app.state);
        assert!(
            is_running(proxy) && proxy.role == ContainerRole::Sidecar,
            "the sidecar has to still be up — it is the container the card would send the \
             reader away from: {proxy:?}"
        );
        assert!(
            stuck_at_the_starting_line(app, nothing_else_to_point_at(&half_up)).is_some(),
            "and the app container has to satisfy every other clause of the rule, or the \
             silence below is not the skip's doing: {app:?}"
        );

        nothing(
            &analyze(&pods_at(vec![half_up], now())),
            "half of this pod is serving, so *it has not been able to start* is false about \
             it — and a confident plain-language sentence that is false about the pod in \
             front of the reader is the 3am failure this file exists to avoid",
        );
    }

    /// **Two containers, two different failures, and the card may not call them the same
    /// thing.** `InvalidImageName` and `ErrImageNeverPull` need two different fixes; folding
    /// the second into *"1 other container in the same state"* is the card inventing an
    /// agreement the kubelet never reported.
    #[test]
    fn two_containers_stuck_for_different_reasons_are_both_named() {
        let mixed = capture_but("hostpath", |pod| {
            container_status(pod, "nosy").state = waiting_at("CreateContainerError", None);
            container_status(pod, "shipper").state = waiting_at("RunContainerError", None);
        });
        let all = analyze(&pods_at(vec![mixed], now()));
        show(&all);
        let card = only(&all, "broken-hostpath", "not been able to start");
        assert!(
            card.evidence.contains("shipper (RunContainerError)"),
            "the second container is named with its own reason, because it is a different \
             failure with a different fix: {}",
            card.evidence
        );
        assert!(
            !card.evidence.contains("in the same state"),
            "and it is not counted as agreeing with the first — the count is for containers \
             the kubelet actually reported the same way: {}",
            card.evidence
        );
    }

    /// **Every way the kubelet says the image is not coming, answered by rule 3 and not by a
    /// ten-minute wait.** `nginx:doesnotexist` drew rule 3's CRITICAL immediately with the
    /// registry's sentence; `NGINX:::latest` drew nothing for ten minutes and then a WARN
    /// about starting that blamed a disk. Two typos, two unrecognisably different answers.
    ///
    /// The moment below is **seven seconds** after the pod was placed — well inside rule
    /// 13's grace — so this asserts the answer arrives *now*, which is half the point.
    ///
    /// **One list, so this is one test for two rules.** [`UNUSABLE_IMAGE`] is rule 3's
    /// trigger and rule 13's exclusion at the same time, so a reason added to rule 3 that
    /// somebody forgot to exclude from the residual is not a shape that exists.
    #[test]
    fn every_unusable_image_reason_is_rule_threes_card_and_arrives_at_once() {
        let just_placed = time("2026-08-12T20:46:00Z");
        for reason in UNUSABLE_IMAGE {
            let broken = capture_but("config", |pod| {
                container_status(pod, "app").state =
                    waiting_at(reason, Some("the runtime's own sentence"));
            });
            let all = analyze(&pods_at(vec![broken.clone()], just_placed.clone()));
            show(&all);
            assert_eq!(
                all.len(),
                1,
                "{reason}: rule 3 alone, and immediately — the reader does not wait ten \
                 minutes to be told a typo is a typo: {:?}",
                titles(&all)
            );
            let card = only(&all, "broken-config", "image is not usable");
            assert_eq!(
                card.severity,
                Severity::Critical,
                "{reason}: this image is never becoming available on its own"
            );
            assert!(
                card.title.contains(reason) && card.evidence.contains("the runtime's own sentence"),
                "{reason}: the reason names which of the seven it is and the kubelet's \
                 sentence carries the diagnosis: {} / {}",
                card.title,
                card.evidence
            );

            // ...and three hours later it is still rule 3's card, never the residual's.
            let later = analyze(&pods_at(vec![broken], now()));
            assert_eq!(
                titles(&later),
                titles(&all),
                "{reason}: past rule 13's grace the answer must not change into a WARN about \
                 starting — one incident, one card, and the right one"
            );
        }
    }

    /// **Somebody has already given up on the wedged pod and deleted it**, and rule 13
    /// stands down for the reason rule 10 does — the mutation sweep found this clause
    /// holding nothing, the same way [D73](NOTES.md) found rule 10's.
    ///
    /// Both cards are *true* about this pod: it never started, and it is not going away.
    /// Only one is actionable. Rule 13's action sends the reader to the machine's Events to
    /// find out what it is still waiting for, and the answer has stopped mattering the
    /// moment the pod is on its way out; what is left to do is find what is holding the
    /// delete, which is rule 12's card and names the finalizer. Alerts is D2's queue of what
    /// is broken now **and** actionable.
    #[test]
    fn a_wedged_pod_someone_has_already_deleted_is_rule_twelves_alone() {
        let abandoned = capture_but("config", |pod| {
            container_status(pod, "app").state = waiting_at("ContainerCreating", None);
            pod.metadata.deletion_timestamp = Some(time("2026-08-12T21:00:00Z"));
            pod.metadata.deletion_grace_period_seconds = Some(30);
            pod.metadata.finalizers = Some(vec!["k8rs.test/never-removed".to_string()]);
        });
        assert!(
            abandoned
                .containers
                .iter()
                .any(
                    |c| stuck_at_the_starting_line(c, nothing_else_to_point_at(&abandoned))
                        .is_some()
                ),
            "the pod still satisfies every other clause of rule 13, which is what makes the \
             silence below the deletion's doing: {:?}",
            abandoned
                .containers
                .iter()
                .map(|c| &c.state)
                .collect::<Vec<_>>()
        );

        let all = analyze(&pods_at(vec![abandoned], now()));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "rule 12 alone: *what is the machine still waiting for* has stopped being a \
             question anyone can act on once the pod has been asked to go: {:?}",
            titles(&all)
        );
        only(&all, "broken-config", "asked to shut down");
    }

    /// **Two containers of one pod wedged on the same node.** A missing volume blocks every
    /// container of the pod at once — one fault, so one card with a count rather than the
    /// same sentence per container.
    ///
    /// `hostpath.json` is the base because it is the repository's only multi-container pod
    /// whose containers are peers. The planted shape is kept coherent with the cause it
    /// names: `ContainerCreating` on both **and** the condition at `False`, which is what a
    /// volume that will not mount actually produces, since the mount is attempted before the
    /// sandbox exists.
    #[test]
    fn two_containers_stuck_on_the_same_node_are_one_card_with_a_count() {
        let wedged = capture_but("hostpath", |pod| {
            for name in ["nosy", "shipper"] {
                container_status(pod, name).state = waiting_at("ContainerCreating", None);
            }
            pod_condition(pod, "PodReadyToStartContainers").status = "False".to_string();
        });
        assert_eq!(
            wedged
                .containers
                .iter()
                .filter(
                    |c| stuck_at_the_starting_line(c, nothing_else_to_point_at(&wedged)).is_some()
                )
                .count(),
            2,
            "both containers have to reach the rule, or the count below is untested: {:?}",
            wedged
                .containers
                .iter()
                .map(|c| &c.state)
                .collect::<Vec<_>>()
        );

        let all = analyze(&pods_at(vec![wedged], now()));
        show(&all);
        let card = only(&all, "broken-hostpath", "not been able to start");
        assert!(
            card.evidence
                .contains("1 other container in the same state"),
            "one card for the pod, and the second container is a count rather than a second \
             copy of the same sentence — the node is what is wrong, not either container: {}",
            card.evidence
        );
        assert_eq!(
            all.iter()
                .filter(|f| f.title.contains("not been able to start"))
                .count(),
            1,
            "and it is one card and not two, which is the whole reason this rule takes the \
             pod rather than being called per container: {:?}",
            titles(&all)
        );
    }

    /// **The ten minutes, from both sides of the line.** A threshold nobody crosses is a
    /// threshold nobody has tested, and this one is the whole difference between rule 13 and
    /// a card on every cold start of a large image.
    ///
    /// The moment is the pod's own `PodScheduled` transition — `20:45:53Z` in the capture —
    /// so the two readings below are the same wedge at `+10:00` and at `+10:01`.
    #[test]
    fn a_pod_only_just_placed_is_a_slow_pull_and_not_a_wedge() {
        let wedged = capture_but("config", |pod| {
            container_status(pod, "app").state = waiting_at("ContainerCreating", None);
        });
        let placed = wedged
            .scheduled
            .as_ref()
            .and_then(|c| c.last_transition.clone())
            .expect("the capture says when it was placed");
        println!("placed at {placed:?}");

        nothing(
            &analyze(&pods_at(vec![wedged.clone()], time("2026-08-12T20:55:53Z"))),
            "ten minutes to the second is inside the window, not past it: pulling a large \
             image onto a cold node legitimately takes minutes, and a rule firing under \
             `progressDeadlineSeconds`' own default alerts on every cold start",
        );

        let all = analyze(&pods_at(vec![wedged], time("2026-08-12T20:55:54Z")));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "one second later the same pod is a finding — and the pair is what keeps the \
             constant from being deleted with the suite still green: {:?}",
            titles(&all)
        );
    }

    /// **The clause the rule is named after, and it was held in place by nothing** — the
    /// defect [D73](NOTES.md) recorded on rule 10, one box later and caught by looking for
    /// it: deleting `if scheduled.status != "True"` leaves the whole suite green.
    ///
    /// The reason is structural rather than an oversight in the captures. The only pod in
    /// the repository with `PodScheduled: False` is `pending.json`, and no kubelet has ever
    /// seen it, so it has **no container statuses at all** — the walk finds nothing and the
    /// rule is silent for a reason that has nothing to do with the gate. Every other
    /// capture is scheduled. So the shape that tells the two apart has to be planted, and
    /// it is one the API server does not produce: container statuses appear only once a pod
    /// is assigned to a node, and `PodScheduled` never goes back to `False` after that.
    ///
    /// **A shape the API cannot produce is still worth a test when it is the only thing
    /// standing between a card and a lie.** *"This pod was given a machine to run on"* is
    /// false about an unschedulable pod, its `lastTransitionTime` dates the *refusal* rather
    /// than a placement, and rule 10 already owns the pod and quotes the scheduler. The
    /// planted status is what makes the clause fail out loud instead of silently.
    #[test]
    fn a_pod_no_machine_took_was_never_given_one_to_run_on() {
        let refused = pending_but(|pod| {
            pod.status
                .as_mut()
                .expect("the captured pod has a status")
                .container_statuses = Some(vec![ContainerStatus {
                name: "app".to_string(),
                image: "docker.io/library/busybox:latest".to_string(),
                state: waiting_at("ContainerCreating", None),
                ..ContainerStatus::default()
            }]);
        });
        println!(
            "scheduled={:?}\n  containers={:?}",
            refused.scheduled, refused.containers
        );
        assert_eq!(
            refused.scheduled.as_ref().map(|c| c.status.as_str()),
            Some("False"),
            "the pod is still the one no machine would take — only the kubelet's report is \
             planted, and it is planted precisely because the API server never writes one \
             for this pod"
        );
        assert!(
            stuck_at_the_starting_line(&refused.containers[0], nothing_else_to_point_at(&refused))
                .is_some(),
            "and the planted container satisfies every *other* clause of rule 13, or the \
             silence below is the walk finding nothing rather than the gate holding"
        );

        let all = analyze(&pods_at(vec![refused], now()));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "rule 10 alone. *Given a machine to run on* is false about a pod nothing would \
             take, and the moment beside it would date the refusal rather than a placement: \
             {:?}",
            titles(&all)
        );
        only(&all, "broken-pending", "will take this pod");
    }

    /// **A container that has run before is not a container that never started**, which is
    /// what the title claims and therefore what the rule has to mean.
    ///
    /// The shape is real: a container that ran, died, and now cannot be recreated because
    /// the node lost the disk under it — `CreateContainerError`, a reason no rule owns, so
    /// the exclusion list does not reach it. What keeps rule 13 off it is
    /// [`ContainerSnapshot::last_terminated`], and the pod is not invisible meanwhile: the
    /// restarts that got it there are rule 5's card.
    #[test]
    fn a_container_that_ran_and_died_is_not_one_that_never_started() {
        let recreating = capture_but("crashloop", |pod| {
            container_status(pod, "quitter").state = waiting_at(
                "CreateContainerError",
                Some("failed to create containerd task"),
            );
        });
        let quitter = container(&recreating, "quitter");
        println!("{:?}\n  restarts {}", quitter.state, quitter.restarts);
        assert!(
            quitter.last_terminated.is_some()
                && !EXPLAINED_ELSEWHERE.contains(&"CreateContainerError"),
            "the edit has to leave a previous run on the container and pick a reason no \
             other rule owns, or this passes for the wrong reason: {quitter:?}"
        );

        let all = analyze(&pods_at(vec![recreating], now()));
        show(&all);
        assert!(
            !all.iter()
                .any(|f| f.title.contains("not been able to start")),
            "this container started fifteen times — *never started* would be a plain lie \
             about it, and the card that is true here is the restart count: {:?}",
            titles(&all)
        );
        only(&all, "broken-crashloop", "restarted 15 times");
    }

    /// **The captured Pending pod with the verdict taken *off* it** — rule 14's shape, built
    /// on a real capture by removing a field rather than by writing one (NOTES § D40, § D53:
    /// the committed JSON is never touched, the decoded copy is).
    ///
    /// **Removal is the only route to this shape from what is committed, and that is a fact
    /// about clusters rather than about these captures.** Every pod in the repository carries
    /// a `PodScheduled` condition — including the four static pods in `kube-system-pods.json`
    /// that no scheduler ever saw, because the kubelet writes the condition itself for a pod
    /// handed straight to it. Everything else here is what the cluster produced:
    /// `phase: Pending`, no container statuses at all, and a `creationTimestamp` two hours
    /// before the pinned [`now`](now).
    fn never_judged(edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
        pending_but(|pod| {
            pod.status
                .as_mut()
                .expect("the captured pod has a status")
                .conditions = None;
            edit(pod);
        })
    }

    /// **Rule 14, and the pod it must not be confused with** — the same capture with and
    /// without the scheduler's line on it, which is the whole distinction the rule is.
    ///
    /// The unedited capture is `Pending` *with* a verdict: something looked at it and refused
    /// it, which is rule 10's card. Take the verdict away and nothing has looked at it at all,
    /// which no other rule in this file can see — it has no container statuses for rules 1–7,
    /// no condition for rules 10 and 13, and no `deletionTimestamp` for rule 12. Without this
    /// rule that pod produces the empty screen `screens/once.md` promises means *nothing is
    /// broken* (NOTES § D74).
    ///
    /// **Both framings of the absence are fed**, because two different producers reach it: the
    /// API server writes no `conditions` key at all for a pod nothing has judged, and a client
    /// or a prune can leave an empty array. `From<Pod>` collapses them and no rule may depend
    /// on which arrived (CLAUDE.md — a check is proven only for the shapes it was fed).
    #[test]
    fn the_pod_nothing_has_judged_is_not_the_pod_something_refused() {
        let judged = pod("pending");
        assert!(
            judged.scheduled.is_some() && judged.phase.as_deref() == Some("Pending"),
            "the committed capture is Pending *and* carries a PodScheduled line — which is \
             this rule's negative, and the reason its positive has to be made by removal"
        );
        let refused = analyze(&pods_at(vec![judged], now()));
        show(&refused);
        assert_eq!(
            refused.len(),
            1,
            "rule 10 alone on the unedited capture: a pod something refused has been looked \
             at, and two cards about who looked at one pod is the screen contradicting \
             itself: {:?}",
            titles(&refused)
        );
        only(&refused, "broken-pending", "will take this pod");

        let created = captured_time(&fixture("pending"), &["metadata", "creationTimestamp"]);
        for shape in [None, Some(Vec::new())] {
            let unjudged = pending_but(|pod| {
                pod.status
                    .as_mut()
                    .expect("the captured pod has a status")
                    .conditions = shape;
            });
            assert_eq!(
                (unjudged.scheduled.as_ref(), unjudged.phase.as_deref()),
                (None, Some("Pending")),
                "both framings decode to the same absence, and the phase is the capture's own"
            );
            assert_eq!(
                unjudged.creation_timestamp.as_ref(),
                Some(&created),
                "and the moment the waiting started is the one the API server stamped, read \
                 back out of the capture it came from"
            );

            let all = analyze(&pods_at(vec![unjudged], now()));
            show(&all);
            assert_eq!(
                all.len(),
                1,
                "one card, and it is this rule's. Nothing else in the file has anything to \
                 read on this pod: no container statuses for rules 1–7, no condition for \
                 rules 10 and 13, no hostPath for rule 8 and no deletion stamp for rule 12: \
                 {:?}",
                titles(&all)
            );
            let unlooked = only(
                &all,
                "broken-pending",
                "Nothing has even looked at this pod",
            );
            assert!(
                unlooked.title.contains("(it shows as Pending)"),
                "the card names the word `kubectl get pods` prints for this pod. The \
                 parenthetical and the deletion guard are one decision: a deleted pod keeps \
                 `phase: Pending` while the column reads Terminating, so this assertion and \
                 `the_unjudged_pod_someone_deleted_is_rule_twelves_alone` hold one half each: {}",
                unlooked.title
            );
            assert_eq!(
                unlooked.severity,
                Severity::Critical,
                "CRITICAL — nothing healthy looks like this, and the pod will not start on \
                 its own (NOTES § D74)"
            );
            assert!(
                unlooked.evidence.contains("PodScheduled"),
                "the word is named so the reader can find it, and explained by the two states \
                 that both write it rather than left bare (invariant 14): {}",
                unlooked.evidence
            );
            assert!(
                unlooked.action.contains("kube-scheduler")
                    && unlooked.action.contains("spec.schedulerName"),
                "both causes, neither claimed — a scheduler that is not running and a \
                 scheduler named on the pod that nobody runs: {}",
                unlooked.action
            );
            assert_eq!(
                unlooked.kubectl_cmd.as_deref(),
                Some("kubectl get pod broken-pending -n default -o yaml"),
                "`get -o yaml` and not `describe`: an absent condition is visible in the yaml, \
                 and `spec.schedulerName` — the field that separates the two causes — is \
                 printed by neither `describe` nor any Event"
            );
            assert_eq!(
                unlooked.timestamp.as_ref(),
                Some(&created),
                "the age is how long the pod has been waiting for anything to look at it. \
                 There is no event of its own to date it by — that absence is the finding"
            );
        }
    }

    /// **The two minutes, from both sides of the line.** A threshold nobody crosses is a
    /// threshold nobody has tested, and this one is the difference between a card and a red
    /// screen every time the control plane hands over.
    ///
    /// kube-scheduler's leader election defaults to a 15s lease with a 10s renew deadline, so
    /// a handover completes in seconds; two minutes is eight times that (NOTES § D74). **The
    /// seconds below are that requirement's own number and not [`NEVER_JUDGED_GRACE`]'s** —
    /// computing them from the constant would move with any edit to it and prove nothing.
    #[test]
    fn a_pod_created_a_moment_ago_is_a_handover_and_not_a_missing_scheduler() {
        let fresh = never_judged(|_| {});
        let created = fresh
            .creation_timestamp
            .clone()
            .expect("the capture says when the pod arrived");
        let at = |secs: i64| {
            Time(
                created
                    .0
                    .checked_add(SignedDuration::from_secs(secs))
                    .expect("the capture's creation time plus two minutes is representable"),
            )
        };
        println!(
            "created at {created:?}, read at {:?} and {:?}",
            at(120),
            at(121)
        );

        nothing(
            &analyze(&pods_at(vec![fresh.clone()], at(120))),
            "two minutes to the second is inside the window, not past it: leadership moves \
             between schedulers in about fifteen seconds and a pod created during one is not \
             a pod nothing will ever look at",
        );

        let all = analyze(&pods_at(vec![fresh], at(121)));
        show(&all);
        assert_eq!(
            all.len(),
            1,
            "one second later the same pod is a finding — and the pair is what keeps the \
             constant from being deleted with the suite still green: {:?}",
            titles(&all)
        );
    }

    /// **A pod with no arrival time cannot be shown to have waited**, so it draws nothing —
    /// the same direction as rule 13's unstamped condition and the opposite of rule 10's,
    /// because here the grace *is* the gate rather than a severity band.
    ///
    /// The API server stamps every accepted create, so the shape's real producer is a prune
    /// that drops the field on the way in — which is why the field is one `k8s.rs` must keep
    /// (invariant 6). The pod is not invisible in that case; it is invisible in *this file*,
    /// and that is the honest failure for a rule whose whole content is a duration.
    #[test]
    fn a_pod_with_no_arrival_time_cannot_be_shown_to_have_waited() {
        let undated = never_judged(|pod| pod.metadata.creation_timestamp = None);
        println!(
            "phase={:?} scheduled={:?} created={:?}",
            undated.phase, undated.scheduled, undated.creation_timestamp
        );
        assert!(
            undated.phase.as_deref() == Some("Pending") && undated.scheduled.is_none(),
            "every other clause of the rule still holds, so the silence below is the missing \
             stamp and nothing else"
        );
        nothing(
            &analyze(&pods_at(vec![undated], now())),
            "no moment to measure from is no finding: a missing field means no finding \
             (invariant 5), never a default that fires",
        );
        assert_eq!(
            analyze(&pods_at(vec![never_judged(|_| {})], now())).len(),
            1,
            "and the same pod *with* its stamp is a card — without this line the assertion \
             above would pass just as well against a rule that never fires at all"
        );
    }

    /// **A pod that is not `Pending` is a pod something has plainly looked at**, whatever its
    /// conditions array says — it is running, so it was placed and started. The gate is the
    /// phase and not the absence alone.
    ///
    /// The shape is planted because the API server does not produce it: a Running pod always
    /// carries the condition. That is the point — the clause has to fail out loud rather than
    /// be held up by a capture that happens never to test it (NOTES § D73, the clause rule 13
    /// found held in place by nothing).
    #[test]
    fn a_running_pod_missing_its_conditions_is_not_one_nothing_has_looked_at() {
        let strip = |phase: &str| {
            let phase = phase.to_string();
            capture_but("healthy", move |pod| {
                let status = pod.status.as_mut().expect("the captured pod has a status");
                status.conditions = None;
                status.phase = Some(phase);
            })
        };
        let running = strip("Running");
        println!(
            "phase={:?} scheduled={:?} created={:?}",
            running.phase, running.scheduled, running.creation_timestamp
        );
        assert!(
            running.scheduled.is_none() && running.creation_timestamp.is_some(),
            "the absence and the arrival time are both there, so only the phase stands \
             between this pod and the card"
        );
        nothing(
            &analyze(&pods_at(vec![running], now())),
            "*nothing has even looked at this pod* about a pod that is running would be the \
             card contradicting the phase beside it on the same screen",
        );

        // The control: the same pod with only the phase moved. Without it the silence above
        // would also be satisfied by a stamp too young to have cleared NEVER_JUDGED_GRACE.
        assert_eq!(
            analyze(&pods_at(vec![strip("Pending")], now())).len(),
            1,
            "the same pod, Pending, is a card — so the phase is what silenced it and not an \
             arrival time still inside the two minutes"
        );
    }

    /// **The unjudged pod somebody deleted is rule 12's alone.** Both cards are true of it —
    /// nothing looked at it, and it is not going away — and only rule 12's is actionable:
    /// *check whether the scheduler is running* is advice about a pod nobody wants scheduled
    /// any more, while rule 12 names the finalizer holding it (NOTES § D73).
    ///
    /// **It also keeps two words off one pod.** `printPod` prints **Terminating** for any
    /// non-terminal phase carrying a `deletionTimestamp` while `phase` stays `Pending`
    /// underneath, so without the guard this card's *(it shows as Pending)* would sit beside
    /// rule 12's *(it shows as Terminating)* about one pod. The guard and the parenthetical
    /// are one decision in two places, and this is the test that fails if either goes.
    #[test]
    fn the_unjudged_pod_someone_deleted_is_rule_twelves_alone() {
        let deleted = never_judged(|pod| {
            pod.metadata.deletion_timestamp = Some(time("2026-08-12T20:46:23Z"));
            pod.metadata.deletion_grace_period_seconds = Some(30);
            pod.metadata.finalizers = Some(vec!["k8rs.test/never-removed".to_string()]);
        });
        assert_eq!(
            deleted.phase.as_deref(),
            Some("Pending"),
            "the phase does not move when a pod is deleted, which is exactly why the \
             parenthetical cannot be trusted to the phase alone"
        );

        let all = analyze(&pods_at(vec![deleted], now()));
        show(&all);
        assert_eq!(all.len(), 1, "rule 12 alone: {:?}", titles(&all));
        let terminating = only(&all, "broken-pending", "asked to shut down");
        assert!(
            terminating.title.contains("Terminating"),
            "and the one card left names the word `kubectl get pods` actually prints — the \
             word this rule's card would have contradicted: {}",
            terminating.title
        );
    }

    /// The whole committed capture through [`analyze`] at once — every card printed, so
    /// that `cargo test -- --nocapture` shows what a user would actually read, and the
    /// properties every finding owes regardless of which rule made it.
    #[test]
    fn the_whole_capture_through_the_rules_at_once() {
        let all = findings(&CAPTURED_PODS);
        show(&all);
        println!(
            "{} critical, {} warnings",
            all.iter()
                .filter(|f| f.severity == Severity::Critical)
                .count(),
            all.iter().filter(|f| f.severity == Severity::Warn).count()
        );

        assert_eq!(
            all.len(),
            14,
            "two on the crash loop, two on the OOM, one image, one config, one unplaceable, \
             two host mounts, one readiness, one restart counter, one terminating — and two \
             on the init container that was invisible until this box (D27): {:?}",
            titles(&all)
        );

        for f in &all {
            assert_ne!(
                f.severity,
                Severity::Info,
                "no rule reaching the Alerts list produces an Info — D2 sends those to a \
                 report: {}",
                f.title
            );
            assert!(
                !f.title.is_empty() && !f.action.is_empty(),
                "a card is what happened · what it means · what to do, and the third is \
                 what makes it a work queue rather than a lint report: {f:?}"
            );
            let cmd = f
                .kubectl_cmd
                .as_deref()
                .unwrap_or_else(|| panic!("every rule in this box has a command: {}", f.title));
            assert!(
                cmd.contains(&f.object.name) && cmd.contains("-n default"),
                "invariant 4's teaching device points at the object the card is about, in \
                 its own namespace: {cmd}"
            );
            assert_eq!(
                f.owner, f.object,
                "`scripts/broken.yaml` creates bare pods, so every one of these files under \
                 itself — the owned case is asserted below"
            );
        }
    }

    /// **The grouping key on a pod that has a controller**, which is D3's whole premise and
    /// is not visible in any of the twelve bare captures above. This one is a real capture
    /// of a Deployment's pod, so nothing is synthesized.
    #[test]
    fn a_finding_on_an_owned_pod_files_under_the_controller_and_not_the_pod() {
        let pods: Vec<PodSnapshot> = items::<Pod>("owned-pods")
            .into_iter()
            .map(PodSnapshot::from)
            .collect();
        let all = analyze(&pods_at(pods, now()));
        show(&all);

        let looping = only(&all, "broken-owned-7bdb7645c8-vhwcp", "CrashLoopBackOff");
        assert_eq!(
            looping.object.kind,
            ObjectKind::Pod,
            "what the rule looked at is the pod"
        );
        assert_eq!(
            looping.owner.kind,
            ObjectKind::ReplicaSet,
            "and what it files under is the controller — `k8s.rs` resolves this up to the \
             Deployment in Phase 5 (D28), and this layer records what the object said"
        );
        assert_eq!(looping.owner.name, "broken-owned-7bdb7645c8");
        assert_ne!(
            looping.owner, looping.object,
            "D3 groups by owner, and a card per pod is the failure mode it exists to stop"
        );
        assert_eq!(
            looping.kubectl_cmd.as_deref(),
            Some("kubectl describe pod broken-owned-7bdb7645c8-vhwcp -n default"),
            "the command still points at the object, never at the card's title — a \
             `describe pod broken-owned-7bdb7645c8` is a command that does not work"
        );
    }
}
