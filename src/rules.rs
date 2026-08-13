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
    /// | N1 | the `Ready` condition's `last_transition` — the node's own, and the one it fires on | — |
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
/// Rules 1–8, 10 and 12–14, and the node rules that draw a card — N1, N2 and N3. The W-series and
/// C1 are later boxes of this phase and are deliberately not wired here: a half-built rule is worse
/// than an absent one, because the screen looks complete either way. **N4 and N5 are not missing,
/// they are `Info`**: they are the Versions and Capacity reports' input and no `Info` finding
/// reaches the Alerts list, so `analysis.rs` calls them and this does not (NOTES § D2).
/// **N6 is not here either, and is not missing**: it is the node half of rule 10's card, which is
/// why [`no_node_accepted_it`] takes the nodes.
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
    for node in &snapshot.nodes {
        findings.extend(node_stopped_being_ready(snapshot, node));
        findings.extend(cordoned_with_work_left_on_it(snapshot, node));
        findings.extend(node_running_low(node));
    }
    for pod in &snapshot.pods {
        findings.extend(stuck_terminating(&snapshot.now, pod));
        if finished(pod) {
            continue;
        }
        findings.extend(escalated_host_path(pod));
        findings.extend(no_node_accepted_it(&snapshot.now, pod, &snapshot.nodes));
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

/// **`payments/web`, or `node-3` for something cluster-scoped** — how a card names an object in a
/// line of prose, which is how `screens/alerts.md` writes both. Spelled once because N1's evidence
/// names owners and the renderers name the same objects in the title (Phase 9).
fn qualified(id: &ObjectId) -> String {
    match &id.namespace {
        Some(ns) => format!("{ns}/{}", id.name),
        None => id.name.clone(),
    }
}

/// **`a` · `a and b` · `a, b and 2 more`** — the list `screens/alerts.md` § N1 spells, and the
/// only shape a card ever lists names in. Two is the cap on purpose: the third name is worth less
/// than the sentence's readability, and the count that follows it carries the total anyway.
fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [first, second, rest @ ..] => format!("{first}, {second} and {} more", rest.len()),
    }
}

/// **Is this pod over?** — `Succeeded` or `Failed`, whose restart counts, last exits and requests
/// belong to the **Waste** report and to nobody's node (NOTES § D2, § D71). Asked by [`analyze`]
/// before the pod rules and by every node rule that joins pods to a node: a `Succeeded` Job pod
/// keeps its `nodeName` for as long as nobody collects it ([`PodSnapshot::phase`]).
fn finished(pod: &PodSnapshot) -> bool {
    matches!(pod.phase.as_deref(), Some("Succeeded" | "Failed"))
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
///
/// **N6 is this card's second half, not a second card.** The node rules answer *which* taint or
/// label is doing the refusing ([`what_is_blocking_it`]), and that answer lands in the evidence and
/// the action of this finding — two findings for one pod is what stops the list being believable
/// (NOTES § D28). The subject stays the pod, so the identity is the pod's and the node is named in
/// the evidence (NOTES § D37, `screens/alerts.md` § N6). **When the join has no answer — no node
/// list, or nothing the two halves can be pinned on — the card is exactly what it was.**
fn no_node_accepted_it(now: &Time, pod: &PodSnapshot, nodes: &[NodeSnapshot]) -> Option<Finding> {
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
    let blocking = what_is_blocking_it(pod, nodes);
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
        // **N6's answer first, then the scheduler's sentence, verbatim and framed**
        // (NOTES § D37). k8rs's own diagnosis leads because it is the plain-language one and it
        // names the field to change; the quote stays because it is the only place the *other*
        // refusals appear — this pod's own message counts four nodes and two different reasons.
        // The prefix does two invariant-14 jobs: it says a machine wrote this, and it glosses the
        // one word that would otherwise split the card into two vocabularies — the title says
        // *machine*, the scheduler says *node*.
        evidence: blocking
            .iter()
            .map(|b| b.evidence.clone())
            .chain(
                scheduled
                    .message
                    .as_deref()
                    .map(|m| format!("the scheduler's own words (a node is one machine): {m}")),
            )
            .collect::<Vec<_>>()
            .join(FACTS),
        // **Only the half the command can answer**, when the nodes are not there to answer the
        // other one: asking for work the command beside it cannot start points invariant 4's
        // teaching device away from itself. No reference to the line above either — it is empty
        // whenever the message is missing and there is nothing to blame.
        action: blocking.map_or_else(
            || {
                "check what this pod asks for: the node labels it selects, which machines \
                 it says it can run on, and how much cpu and memory it requests"
                    .to_string()
            },
            |b| b.action,
        ),
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

// --- THE NODE RULES START ---
//
// N1–N6 of NOTES § Node rules. **Three of the six draw a node card** — N1, N2, N3 — and
// [`analyze`] wires those. **N4 and N5 are `Info`** and are the Versions and Capacity reports'
// input, so they are computed here and read by `analysis.rs` (Phase 4): a skewed kubelet and an
// over-promised node are risks, not outages (NOTES § D2). **N6 is not a card at all** —
// [`what_is_blocking_it`] is the node half of rule 10's, and it files under the pod.
//
// **Four of the six join the pods to a node, and a partial pod list makes two of them lie.** N2's
// count *is* its trigger and N5's sum *is* its verdict, so both go silent under a namespace scope
// and the screen names the check that did not run — Phase 9's banner, not a finding from here
// (NOTES § D43, § D46). N1's count is evidence rather than trigger, so it fires either way and
// drops the line; N6 reads node taints and the pod's own spec, which are in scope by definition.

/// **How long a node may be un-Ready before it is an outage** — five minutes, and the number is
/// borrowed rather than tuned: it is `--default-unreachable-toleration-seconds`, the 300 seconds
/// the admission controller writes on to every pod in the cluster, which is Kubernetes' own answer
/// to *how long do we wait for a node before moving what is on it* (NOTES § Node rules).
const NODE_DOWN_GRACE: SignedDuration = SignedDuration::from_mins(5);

/// **The taint the node lifecycle controller mirrors from `spec.unschedulable`**, and the only
/// place a cordon carries a time — `kubectl cordon` writes the boolean, the controller writes this
/// and stamps it (NOTES § D65, [`Taint::added_at`]).
const CORDON_TAINT: &str = "node.kubernetes.io/unschedulable";

/// **The two taints that mean an autoscaler is deliberately emptying this node**, on which N2 is
/// silent: the node is cordoned *with pods on it* for the whole eviction window by design, so a
/// card here fires repeatedly on a cluster doing exactly what it was configured to do. A
/// scale-down that never finishes is the **Drain safety** report's row, not an Alerts card
/// (NOTES § D43).
const SCALE_DOWN_TAINTS: [&str; 2] = ["ToBeDeletedByClusterAutoscaler", "karpenter.sh/disrupted"];

/// **The three conditions that mean the kubelet is about to start evicting**, each with the noun
/// N3's title uses and the fix its action asks for (`screens/alerts.md` § N3). One table, because
/// the three cards differ in nothing else.
const PRESSURES: [(&str, &str, &str); 3] = [
    (
        "DiskPressure",
        "disk space",
        "free up disk space on this node",
    ),
    ("MemoryPressure", "memory", "free up memory on this node"),
    (
        "PIDPressure",
        "process IDs",
        "find what is creating so many processes",
    ),
];

/// **How many minor versions a kubelet may be behind the control plane and still be supported** —
/// **three**, which is upstream's own window since 1.28: *"kubelet may be up to three minor
/// versions older than kube-apiserver"* (NOTES § D81, reversing the two this project first wrote
/// down; two was the rule for a kubelet older than 1.25).
///
/// **The number belongs to upstream and not to us, because the card makes a claim about upstream**
/// — *"too far behind to be supported"* — and at two it told everybody mid-upgrade that a
/// supported cluster was unsupported.
const SUPPORTED_SKEW: u32 = 3;

/// **Only these two effects keep a pod off a node.** `PreferNoSchedule` is a preference the
/// scheduler will overrule to place a pod, so a card blaming one would name a taint that is not
/// refusing anything.
const BLOCKING_EFFECTS: [&str; 2] = ["NoSchedule", "NoExecute"];

/// **The taints Kubernetes writes and Kubernetes removes** — what each one means, and what to do
/// about it, because *"add a toleration for it"* is the wrong answer to every row here
/// (NOTES § D81).
///
/// **Never tell the reader to tolerate a taint the node controller manages.** On a single-node
/// cluster — kind, minikube, k3s, Docker Desktop, which is who this tool is for — `kubectl cordon`
/// followed by a deploy made N6 print *"add a toleration for node.kubernetes.io/unschedulable"*
/// when the answer is `kubectl uncordon`. `unreachable` is worse: it asks the reader to schedule
/// onto a dead machine, the taint cannot be removed because the controller re-adds it in seconds,
/// and N1 is drawing *"this node has stopped responding"* on the same screen. And
/// `ToBeDeletedByClusterAutoscaler` is one this very file already calls *an operation in progress*
/// in [`SCALE_DOWN_TAINTS`], so offering to tolerate it is two rules disagreeing about one taint.
///
/// It is a **translation**, not a suppression: the card still says which machines and why, in the
/// words invariant 14 asks for — a bare `node.kubernetes.io/unschedulable` on screen is
/// `CrashLoopBackOff` printed and left. **A taint that is not here keeps the toleration wording**,
/// which is right for exactly the case it was written for: `node-role.kubernetes.io/control-plane`
/// on a single-node kubeadm cluster.
///
/// **No row promises a card that may not be there.** N1 waits [`NODE_DOWN_GRACE`] before it draws
/// anything, and the `not-ready` / `unreachable` taints do not wait at all — `nodelifecycle`'s
/// `doNoScheduleTaintingPass` runs off the node informer, so the taint lands a fraction of a second
/// after `Ready` flips, while the card is five minutes away. (The 300 seconds everyone remembers
/// belongs to the **NoExecute** taint: eviction, not scheduling.) So a runtime that dies at 03:02
/// and a deploy at 03:03 would have sent the reader hunting a card that arrives at 03:07 — and
/// a node with no `Ready` condition at all never gets one. The rows point at the machine and stop
/// there; the evidence line one row up has already named it (NOTES § D81).
///
/// **`node-role.kubernetes.io/control-plane` must never join this table**, and the reason is
/// structural rather than a judgement about that one taint: every row here is a taint whose removal
/// is either impossible — the controller re-adds it — or pointless, because it clears itself. The
/// control-plane taint is neither. Nothing changes on its own, so *"wait"* or *"check the machine"*
/// would strand the reader, and both halves of the untranslated wording are the real answers: the
/// documented single-node kubeadm fix is literally
/// `kubectl taint nodes --all node-role.kubernetes.io/control-plane-`.
///
/// **`network-unavailable` names the network plugin, and that is the right *single* answer with a
/// ceiling worth writing down.** The other producer of `NodeNetworkUnavailable=True` is the cloud
/// **route controller**, waiting for routes to the node's pod CIDR — a control-plane problem, not
/// something on that machine. Route-based networking is legacy (GKE, EKS and AKS are VPC-native or
/// CNI-driven now), and cloud jargon does not belong on a card a kind user can see, so the common
/// producer wins the sentence.
///
/// **`memory-pressure` survives one trap worth naming.** The `PodTolerationRestriction` admission
/// plugin adds an `Exists` toleration for it to every non-BestEffort pod, which would make *"stops
/// placing new pods"* true only of BestEffort ones. That plugin is **not** default-enabled in 1.36,
/// so the sentence holds on a default cluster; where it *is* enabled, [`tolerated`] matches the
/// auto-toleration and this branch is never reached at all. Both directions are safe.
///
/// Each middle string reads after *"node-1 is"* / *"node-1 and node-2 are"*, and carries no
/// pronoun, so one row serves both. **Each action carries [`inflected`]'s tokens** for the same
/// reason: the case this table exists for is one machine, and six of these rows used to say
/// *"those machines"* about it. **The two autoscaler taints are not in this table**: they are
/// [`SCALE_DOWN_TAINTS`], read by [`managed_taint`] straight off the list N2 already uses, so a
/// third autoscaler arrives in one place rather than two.
const MANAGED_TAINTS: [(&str, &str, &str); 9] = [
    (
        "node.kubernetes.io/unschedulable",
        "cordoned, and a cordoned machine refuses every new pod",
        "allow new pods on {machines} again once the work is done ({uncordon})",
    ),
    (
        "node.kubernetes.io/not-ready",
        "not ready, and nothing is placed on a machine that says it cannot run pods",
        "check {machines} first — this pod is placed on its own once a machine is ready again",
    ),
    (
        "node.kubernetes.io/unreachable",
        "not answering, and nothing is placed on a machine the cluster cannot reach",
        "check {machines} first — this pod is placed on its own once a machine comes back",
    ),
    (
        "node.kubernetes.io/memory-pressure",
        "low on memory, and Kubernetes stops placing new pods on a machine in that state",
        "free up memory on {machines}, or add another machine to the cluster",
    ),
    (
        "node.kubernetes.io/disk-pressure",
        "low on disk space, and Kubernetes stops placing new pods on a machine in that state",
        "free up disk space on {machines}, or add another machine to the cluster",
    ),
    (
        "node.kubernetes.io/pid-pressure",
        "low on process IDs, and Kubernetes stops placing new pods on a machine in that state",
        "find what is creating so many processes on {machines}, or add another machine",
    ),
    (
        "node.kubernetes.io/network-unavailable",
        "without a working network yet, and nothing is placed on a machine that has none",
        "check the network plugin on {machines} — nothing can be placed there until it comes up",
    ),
    (
        "node.cloudprovider.kubernetes.io/uninitialized",
        "still being set up, and nothing is placed on a machine that has not finished joining",
        "wait for {machines} to finish joining; if that never happens, the cloud controller is \
         what has not answered",
    ),
    (
        "karpenter.sh/unregistered",
        "still being set up, and nothing is placed on a machine that has not finished joining",
        "wait for {machines} to finish joining — a nodepool starting from zero passes through \
         this on its own",
    ),
];

/// What a taint the node controller manages means, and what to do about it — [`MANAGED_TAINTS`]
/// plus the two [`SCALE_DOWN_TAINTS`], which N2 already holds and which are the sharpest row of
/// all: this file calls that node *an operation in progress* in one rule, so it may not offer to
/// tolerate it in another.
///
/// `None` is a taint somebody at this cluster wrote themselves, where *"add a toleration, or
/// remove the taint"* is exactly the right advice.
fn managed_taint(key: &str) -> Option<(&'static str, &'static str)> {
    if SCALE_DOWN_TAINTS.contains(&key) {
        return Some((
            "being taken out of the cluster on purpose, so nothing new is placed there",
            "wait for the replacement machine, or find out why the cluster is not adding one",
        ));
    }
    MANAGED_TAINTS
        .iter()
        .find(|(managed, _, _)| *managed == key)
        .map(|&(_, means, action)| (means, action))
}

/// **One machine or several, in the action as well as in the evidence** — `{machines}` and
/// `{uncordon}`, filled from the machines the card is about (NOTES § D81).
///
/// The evidence line has inflected since it was written ([`listed`] and its `is`/`are`); the
/// actions said *"those machines"* whatever the count, on a table whose primary case is a
/// one-machine kind or minikube cluster.
///
/// **`{uncordon}` carries the names**, because a command printed without them is the one line in
/// this file that does not run as written — and *"without memorising long kubectl commands"* is
/// what this tool is for (invariant 4). `kubectl uncordon` takes any number of nodes, so the same
/// substitution serves both counts.
fn inflected(action: &str, names: &[String]) -> String {
    let machines = if names.len() == 1 {
        "that machine"
    } else {
        "those machines"
    };
    action.replace("{machines}", machines).replace(
        "{uncordon}",
        &format!("kubectl uncordon {}", names.join(" ")),
    )
}

/// `kubectl describe node …` — the command behind every node card. It prints the conditions with
/// their reasons and messages (N1, N3), `Unschedulable` and the non-terminated pods it is carrying
/// (N2), and the *Allocated resources* table N5's two numbers come from.
///
/// **The one thing it does not print is `timeAdded`**, so N2's age is the single claim on any of
/// these cards the command cannot show, and that is recorded on N2 rather than paid for with a
/// command that shows nothing else (NOTES § D69, § D81). A node is cluster-scoped, so there is no
/// `-n` to append.
fn describe_node(id: &ObjectId) -> Option<String> {
    Some(format!("kubectl describe node {}", id.name))
}

/// One `status.conditions[]` entry of a node, by type — N1 and N3's whole input, and **the reason
/// both read their own condition's `last_transition`**: the list is flat, `Ready`'s stamp is three
/// lines from DiskPressure's, and a DiskPressure card dated the node's boot time is what taking
/// the wrong one produces (NOTES § D69).
fn node_condition<'a>(node: &'a NodeSnapshot, type_: &str) -> Option<&'a Condition> {
    node.conditions.iter().find(|c| c.type_ == type_)
}

/// The pods this node is carrying that are still a going concern — the join N1, N2 and N5 are.
/// A pod that finished is charged to nobody and was not *running* anywhere ([`finished`]).
fn pods_on<'a>(snapshot: &'a ClusterSnapshot, node: &NodeSnapshot) -> Vec<&'a PodSnapshot> {
    snapshot
        .pods
        .iter()
        .filter(|p| p.node.as_deref() == Some(node.id.name.as_str()) && !finished(p))
        .collect()
}

/// **Would `kubectl drain` actually move this pod?** — N2's whole narrowing, and the difference
/// between *"a drain left something behind"* and *"pods run here"*.
///
/// `kubectl/pkg/drain/filters.go` skips DaemonSet pods and mirror (static) pods **regardless of
/// flags**, so a perfectly drained node still runs kindnet and kube-proxy, and a cordoned
/// control-plane node still runs four static pods. Counting those fires N2 on every node an
/// operator drained correctly (NOTES § D46).
///
/// **And `skipDeletedFilter`, which is the same false positive arriving from the other side**: a
/// pod already terminating is one the drain has evicted and is waiting on, so counting it puts the
/// card on a drain that is *running* — the state D43 refused to alarm about for an autoscaler
/// (NOTES § D81).
///
/// The two filters that are deliberately **not** here are `localStorageFilter` and
/// `unreplicatedFilter`: those make a drain *refuse* rather than skip, which is more reason to
/// count the pod, not less.
fn a_drain_would_move(pod: &PodSnapshot) -> bool {
    !pod.mirror && pod.owner.kind != ObjectKind::DaemonSet && pod.deletion_timestamp.is_none()
}

/// **N1 — the node stopped saying it is ready, and everything on it is a question mark.**
/// `conditions[Ready]` at anything but `True` for longer than [`NODE_DOWN_GRACE`]. CRITICAL.
///
/// **The card has to reach the pods, not only the node** (NOTES § D71). Every pod rule in this file
/// reads pod *status*, and the status of a pod whose kubelet stopped posting is a fossil that never
/// expires — a crash-looping pod on a node that went quiet ten minutes ago still reads `Running`,
/// so nothing else on the screen mentions the workload that is actually down. So the evidence names
/// **owners**, up to two alphabetically and then a count, with the total beside it
/// (`screens/alerts.md` § N1) — a bare number would answer N2's question, not this one.
///
/// **Two statuses, two cards, and only one of them is `screens/alerts.md`'s.** `Unknown` is a
/// kubelet that went quiet, which is the one the fossil argument is about and the one the screen
/// draws. `False` is a kubelet that answered and said no — a container runtime that will not start,
/// a full disk, a CNI that never came up — and *"has stopped responding"* is simply false about it,
/// which invariant 14 does not allow. The condition's own message finishes that card, since the
/// kubelet's sentence is the diagnosis there and there is no such sentence on a silent node.
///
/// **An undated condition still fires**, rule 10's direction and not rule 13's: a node that cannot
/// be shown to have gone down just now is read as one that has been down, and the card draws no age
/// rather than no card.
///
/// **Under a namespace scope the evidence line is dropped** rather than counted from a fraction of
/// the pods: *"one pod was running here"* about a node carrying forty reads as complete and is the
/// wrong number this screen exists not to print (NOTES § D43). The card itself is unaffected — the
/// node's own condition is not namespaced.
fn node_stopped_being_ready(snapshot: &ClusterSnapshot, node: &NodeSnapshot) -> Option<Finding> {
    let ready = node_condition(node, "Ready")?;
    if ready.status == "True" {
        return None;
    }
    let since = ready.last_transition.as_ref();
    if since.is_some_and(|t| snapshot.now.0.duration_since(t.0) <= NODE_DOWN_GRACE) {
        return None;
    }
    // The API's tri-state, and anything that is not the two known values is treated as the silent
    // case: a status this code cannot read is not evidence that the kubelet answered.
    let answered = ready.status == "False";
    let mut facts = Vec::new();
    if snapshot.namespace_scope.is_none() {
        let pods = pods_on(snapshot, node);
        // Sorted and de-duplicated in one step, which is what alphabetical *by owner* means when
        // forty pods share three of them.
        let owners: Vec<String> = pods
            .iter()
            .map(|p| qualified(&p.owner))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        if !owners.is_empty() {
            facts.push(format!(
                "{} {} running here ({})",
                listed(&owners),
                match (answered, owners.len()) {
                    (false, 1) => "was",
                    (false, _) => "were",
                    (true, 1) => "is",
                    (true, _) => "are",
                },
                counted(pods.len() as i64, "pod")
            ));
        }
    }
    // The kubelet's own sentence, carried verbatim and **framed the way rule 10 frames the
    // scheduler's** (NOTES § D37, § D81): the prefix says a machine wrote this, and glosses the
    // one word that would otherwise leave the card in two vocabularies. Only on the branch that
    // has a sentence to carry — a machine that went quiet wrote nothing.
    if answered {
        facts.extend(ready.message.as_deref().map(|m| {
            format!(
                "the kubelet's own words (the kubelet is the part of Kubernetes that runs on \
                 the machine): {m}"
            )
        }));
    }
    Some(Finding {
        severity: Severity::Critical,
        title: if answered {
            "This node says it cannot run pods — nothing new will start here until it can"
        } else {
            "This node has stopped responding — nothing on it can be trusted until it does"
        }
        .to_string(),
        evidence: facts.join(FACTS),
        action: if answered {
            "check the machine itself: what the kubelet says is wrong is above, and the \
             kubelet's own log on that machine says the rest"
        } else {
            "check the node itself: is it powered on and reachable?"
        }
        .to_string(),
        kubectl_cmd: describe_node(&node.id),
        owner: node.id.clone(),
        object: node.id.clone(),
        timestamp: ready.last_transition.clone(),
    })
}

/// **N2 — somebody cordoned this node and the drain never finished.** `spec.unschedulable`, and
/// **only while a drain would still have to move something off it**. WARN.
///
/// **The count is the trigger, not decoration** (NOTES § D43, § D46). A cordoned node with nothing
/// movable left is *parked* — a finished drain nobody turned back on — which is a Capacity row and
/// not an outage. What a drain would move is [`a_drain_would_move`]'s question.
///
/// **Silent on a node an autoscaler is retiring** ([`SCALE_DOWN_TAINTS`]) and **silent under a
/// namespace scope**, where the count comes out of a fraction of the pods and a zero would silence
/// the rule with nothing on the screen to show it happened.
///
/// **The age is the cordon taint's, and it dates the taint rather than the cordon** — anything that
/// rewrites `spec.taints` wholesale re-stamps it — so the card says *"cordoned about 2 hours ago"*
/// and builds no argument on it (NOTES § D65, § D69). A hand-applied `kubectl taint` stamps nothing
/// and the right edge is simply empty.
///
/// **`describe node`, and the age is the one thing it cannot back** (NOTES § D69 offered either
/// this or a `-o jsonpath` that shows `timeAdded`; D81 took this one). `describe` prints
/// `Unschedulable: true`, which is the title, and the `Non-terminated Pods` table, which is the
/// count — and the count is the *trigger*, so it is on every one of these cards while the age is
/// on some. The jsonpath line backs only the age, shows nothing at all when the taint carries no
/// stamp, and hands a beginner a JSON blob.
fn cordoned_with_work_left_on_it(
    snapshot: &ClusterSnapshot,
    node: &NodeSnapshot,
) -> Option<Finding> {
    if !node.unschedulable || snapshot.namespace_scope.is_some() {
        return None;
    }
    if node
        .taints
        .iter()
        .any(|t| SCALE_DOWN_TAINTS.contains(&t.key.as_str()))
    {
        return None;
    }
    let movable = pods_on(snapshot, node)
        .into_iter()
        .filter(|p| a_drain_would_move(p))
        .count();
    if movable == 0 {
        return None;
    }
    Some(Finding {
        severity: Severity::Warn,
        title: "This node refuses new pods (cordoned)".to_string(),
        evidence: format!(
            "{} here would still have to move",
            counted(movable as i64, "pod")
        ),
        // It states the lifecycle and does not accuse: true whether the cordon was five minutes ago
        // or five months ago, which is what lets the same sentence sit on a card with no age
        // (`screens/alerts.md`).
        action: "allow new pods once the work is done".to_string(),
        kubectl_cmd: describe_node(&node.id),
        owner: node.id.clone(),
        object: node.id.clone(),
        timestamp: node
            .taints
            .iter()
            .find(|t| t.key == CORDON_TAINT)
            .and_then(|t| t.added_at.clone()),
    })
}

/// **N3 — the node is running out of something and the kubelet is about to start evicting.**
/// Any of [`PRESSURES`] at `True`. WARN: nothing is down yet, and that is the whole point of
/// arriving before it is.
///
/// **`True` and nothing else.** The three pressures read `Unknown` on a node whose kubelet stopped
/// posting, which is N1's answer and not this one — a rule that read "not False" as a pressure
/// would file *evictions are coming* on a machine nobody can reach.
///
/// **All of them, when more than one is true**, joined into one sentence rather than one card
/// picking a resource and hiding the other (`screens/alerts.md` § N3).
///
/// **The age is that condition's own `last_transition`, never `Ready`'s off the same flat list**,
/// or a DiskPressure card carries the node's boot time (NOTES § D69). With two pressures at once it
/// is the **earlier** of them: the card's question is how long this has been going on.
fn node_running_low(node: &NodeSnapshot) -> Option<Finding> {
    let low: Vec<(&str, &str, &Condition)> = PRESSURES
        .iter()
        .filter_map(|&(type_, noun, fix)| {
            let c = node_condition(node, type_).filter(|c| c.status == "True")?;
            Some((noun, fix, c))
        })
        .collect();
    if low.is_empty() {
        return None;
    }
    let nouns: Vec<String> = low.iter().map(|&(noun, _, _)| noun.to_string()).collect();
    Some(Finding {
        severity: Severity::Warn,
        title: format!(
            "This node is running low on {} — Kubernetes may start evicting pods to free it up",
            nouns.join(" and ")
        ),
        // The condition's reason is `KubeletHasDiskPressure`, which says nothing the title has not
        // already said in a sentence (invariant 14), and the kubelet writes no message on these.
        evidence: String::new(),
        action: format!(
            "{}, or move some pods elsewhere",
            low.iter()
                .map(|&(_, fix, _)| fix)
                .collect::<Vec<_>>()
                .join(" and ")
        ),
        kubectl_cmd: describe_node(&node.id),
        owner: node.id.clone(),
        object: node.id.clone(),
        timestamp: low
            .iter()
            .filter_map(|&(_, _, c)| c.last_transition.clone())
            .min_by_key(|t| t.0),
    })
}

/// **N4 — this machine's kubelet is too far behind the control plane to be supported.**
/// `status.nodeInfo.kubeletVersion` against [`ClusterSnapshot::server_version`], more than
/// [`SUPPORTED_SKEW`] minor versions. **`Info`, and it does not reach Alerts**: an unsupported
/// kubelet is a risk to answer this month, not an outage to answer now — it is the **Versions**
/// report's input (NOTES § D2).
///
/// **No server version, no finding.** Comparing against a guess is the one thing this rule may not
/// do, and *"the control plane's version could not be read"* is a sentence about the whole cluster
/// rather than about this node — the Versions report says it, in the slot where its own answer
/// would have been (`screens/analysis.md`).
///
/// **A kubelet *ahead* of the control plane is not this rule's**, and the `checked_sub` is where
/// that is decided: upstream forbids it outright, NOTES words N4 as *behind*, and inventing the
/// other card here would be a rule the set does not contain (invariant 13).
///
/// **Different majors compare as nothing.** There has only ever been one, and a minor number read
/// across a major boundary is not a distance.
///
/// **`get nodes -o wide` and not `kubectl version`**: the number this card is *about* is this
/// node's kubelet, which that command prints for every node at once; the control-plane half is
/// `kubectl version`, and no single command shows both (invariant 4).
fn kubelet_too_far_behind(server_version: Option<&str>, node: &NodeSnapshot) -> Option<Finding> {
    let (server_major, server_minor) = minor_version(server_version?)?;
    let kubelet = node.kubelet_version.as_deref()?;
    let (major, minor) = minor_version(kubelet)?;
    if major != server_major {
        return None;
    }
    let behind = server_minor.checked_sub(minor)?;
    if behind <= SUPPORTED_SKEW {
        return None;
    }
    Some(Finding {
        severity: Severity::Info,
        title: "This machine's kubelet is too far behind the control plane to be supported"
            .to_string(),
        evidence: [
            format!("kubelet {kubelet}"),
            format!("control plane {}", server_version?),
            format!("{} behind", counted(i64::from(behind), "version")),
        ]
        .join(FACTS),
        // Upstream's window, cited rather than asserted as a number of this project's own
        // (NOTES § D81).
        action: format!(
            "upgrade the kubelet on this machine — Kubernetes supports a kubelet at most {} \
             older than the control plane",
            counted(i64::from(SUPPORTED_SKEW), "minor version")
        ),
        kubectl_cmd: Some("kubectl get nodes -o wide".to_string()),
        owner: node.id.clone(),
        object: node.id.clone(),
        // Nothing records when this kubelet was installed, and the node's creation time is not it.
        timestamp: None,
    })
}

/// `v1.36.1` → `(1, 36)`, and `v1.29.7-gke.1104000` → `(1, 29)` — the major and minor of a version
/// string, which is all N4 compares. Anything that does not start with two numbers answers `None`
/// and N4 says nothing rather than guessing at a distance.
fn minor_version(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.trim_start_matches('v').split('.');
    let number = |part: Option<&str>| -> Option<u32> {
        let digits: String = part?.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    };
    Some((number(parts.next())?, number(parts.next())?))
}

/// **N5 — more has been promised to the pods on this node than the node has.** The sum of what
/// they request against `status.allocatable`. **`Info`, and it does not reach Alerts**: it is the
/// **Capacity** report's input, and nothing here is down — it is why the next thing to start here
/// will not (NOTES § D2, `screens/analysis.md` § Capacity).
///
/// **Silent under a namespace scope**, where the sum is taken over a fraction of the pods: a low
/// number here does not read as *missing*, it reads as *fine*, which is the one wrong answer this
/// rule exists to prevent (NOTES § D43, § D46).
///
/// **The arithmetic is [`charged`]'s**, and its two traps are what the rule is for: a native
/// sidecar is *added* rather than maxed, and a pod-level request **replaces** the container sum
/// rather than adding to it (NOTES § D46, § D51).
fn node_overcommitted(snapshot: &ClusterSnapshot, node: &NodeSnapshot) -> Option<Finding> {
    if snapshot.namespace_scope.is_some() {
        return None;
    }
    let pods = pods_on(snapshot, node);
    let cpu = promised(
        &pods,
        node.allocatable_cpu.as_deref(),
        |p| p.cpu_request.as_deref(),
        |c| c.cpu_request.as_deref(),
    );
    let memory = promised(
        &pods,
        node.allocatable_memory.as_deref(),
        |p| p.memory_request.as_deref(),
        |c| c.memory_request.as_deref(),
    );
    let mut over: Vec<(&str, String, String)> = Vec::new();
    // **Strictly greater, on integers.** A node packed to exactly its allocatable is legal and
    // ordinary — `noderesources.Fit` admits while `request <= allocatable - requested`, and
    // `describe node` prints `cpu 3920m (100%)` without comment (NOTES § D81).
    if let Some((asked, has)) = cpu.filter(|(asked, has)| asked > has) {
        over.push((
            "CPU",
            format!("{} cpu", cpu_text(asked)),
            format!("{} cpu", cpu_text(has)),
        ));
    }
    if let Some((asked, has)) = memory.filter(|(asked, has)| asked > has) {
        over.push(("memory", bytes(asked), bytes(has)));
    }
    if over.is_empty() {
        return None;
    }
    Some(Finding {
        severity: Severity::Info,
        title: format!(
            // **Not "nothing new can start here"**, which the reader's own cluster contradicts
            // the first time they deploy something that requests nothing: a BestEffort pod is
            // placed on a node at 100% of its requests all day (invariant 14).
            "This node has promised more {} than it has",
            over.iter()
                .map(|&(noun, _, _)| noun)
                .collect::<Vec<_>>()
                .join(" and ")
        ),
        // The report's own two columns, in the words it heads them with
        // (`screens/analysis.md` § Capacity): what the pods were promised, and what the machine
        // actually has to give.
        evidence: over
            .iter()
            .map(|(noun, asked, has)| format!("{noun}: promised {asked} · usable {has}"))
            .collect::<Vec<_>>()
            .join(FACTS),
        action: "move some pods to another node, or lower what they ask for (their requests)"
            .to_string(),
        kubectl_cmd: describe_node(&node.id),
        owner: node.id.clone(),
        object: node.id.clone(),
        // A standing arithmetic rather than an event: nothing in either object records when the
        // sum crossed the line (NOTES § D69, [`Finding::timestamp`]).
        timestamp: None,
    })
}

/// What the pods on this node ask for in total, and what the node has to give, both in
/// [`quantity_milli`]'s integer unit — `None` when the node does not say what it has, or when any
/// quantity in the sum does not parse.
///
/// **A quantity that cannot be read stops the whole node rather than being skipped.** A skipped
/// request understates a sum whose entire job is to notice that a node is over-promised, and a
/// missing card is the safe direction where a wrong number is not (invariant 5). An overflow takes
/// the same road, which is what `checked_add` is for.
fn promised(
    pods: &[&PodSnapshot],
    allocatable: Option<&str>,
    of_pod: impl Fn(&PodSnapshot) -> Option<&str>,
    of_container: impl Fn(&ContainerSnapshot) -> Option<&str>,
) -> Option<(i64, i64)> {
    let has = quantity_milli(allocatable?)?;
    let mut asked: i64 = 0;
    for pod in pods {
        asked = asked.checked_add(charged(pod, &of_pod, &of_container)?)?;
    }
    Some((asked, has))
}

/// **What the scheduler charges this pod to the node it is on** —
/// `max( max over the init containers , sum(regular) + sum(restartable-init) )`, or the pod-level
/// request where one is declared.
///
/// **A native sidecar is additive and an ordinary init container is not** (NOTES § D46): a sidecar
/// runs beside the app for the whole life of the pod, and dropping 100m per meshed pod is six CPUs
/// invisible on sixty of them. **A pod-level request replaces the container sum** rather than adding
/// to it — a pod declaring only `spec.resources.requests` decodes with all-`None` containers, and a
/// summing rule calls the node healthy with four committed CPUs unaccounted for (NOTES § D51).
///
/// **The formula is order-free and upstream's is not** — it carries the sidecar total forward
/// through the init list in order — so this understates the rare pod that declares a plain init
/// container *after* a sidecar. [`PodSnapshot::containers`] promises no order, so the exact one is
/// not computable here ([`ContainerRole`]).
fn charged(
    pod: &PodSnapshot,
    of_pod: impl Fn(&PodSnapshot) -> Option<&str>,
    of_container: impl Fn(&ContainerSnapshot) -> Option<&str>,
) -> Option<i64> {
    if let Some(whole_pod) = of_pod(pod) {
        return quantity_milli(whole_pod);
    }
    let mut running: i64 = 0;
    let mut init_peak: i64 = 0;
    for c in &pod.containers {
        let value = match of_container(c) {
            // Nothing requested is zero requested — the field is optional and its absence is not a
            // number that could not be read.
            None => 0,
            Some(q) => quantity_milli(q)?,
        };
        match c.role {
            ContainerRole::Init => init_peak = init_peak.max(value),
            ContainerRole::Regular | ContainerRole::Sidecar => {
                running = running.checked_add(value)?
            }
        }
    }
    Some(running.max(init_peak))
}

/// **A Kubernetes quantity as an integer, in the API's own unit ×1000** — millicores for a cpu
/// value and milli-bytes for a memory one. `"500m"` → `500`, `"12"` → `12_000`, `"64Mi"` →
/// `67_108_864_000`. The one place this file turns a quantity string into arithmetic, which is why
/// [`quantity`] leaves them as strings: N5 is the only rule that needs the number
/// ([`ContainerSnapshot::cpu_request`]).
///
/// **Integer, and that is the whole of why this function exists** (NOTES § D81). `100m` has no
/// exact binary representation, so an `f64` sum of a node's pods lands a hair above an allocatable
/// that is the same number — and N5 fired on a node packed *exactly* full, printing `promised
/// 0.3 cpu · usable 0.3 cpu`, flapping as watch events reordered the pods. The parse is exact as
/// well as the sum: the mantissa and the scale are multiplied as `i128` and divided once at the
/// end, so nothing rounds until [`cpu_text`] or [`bytes`] prints it.
///
/// **A sub-milli value rounds up**, which is `Quantity::MilliValue`'s own direction — charging a
/// node the whole milli it cannot subdivide.
///
/// **Every arithmetic step is checked, because a rule may not panic** (invariant 5). A quantity is
/// a string off the API and the apiserver's grammar admits far more than a node ever has:
/// `170141183460469231731687303715884105m` is **accepted and stored verbatim** by a live v1.36.1
/// server (`kubectl apply --dry-run=server`), and an unchecked add on it took the rule engine down
/// in debug and answered a *negative* number of millicores in release, which the comparison then
/// read as a full node (NOTES § D81).
///
/// **The exponent form parses**, and the sentence that used to say otherwise was a claim about
/// apiserver behaviour that a `--dry-run=server` contradicts: an *unquoted* `1e3` is canonicalised
/// to `1k`, but a **quoted** `"1e3"` — how every chart that quotes its quantities writes it —
/// round-trips verbatim, because `Quantity` caches the string it was parsed from. It is in
/// upstream's own grammar (`[eE][+-]?[0-9]+`) and `ParseQuantity` accepts it; refusing it cost one
/// whole node, silently absent from the Capacity report. Upstream's grammar puts the exponent
/// *in place of* a suffix, so `1e3Ki` is not a quantity here either.
///
/// `None` for a suffix this does not know, for a negative — a request cannot be one, and the minus
/// sign is not even scanned — and for a value past `i64`, which is an exabyte node nobody has.
fn quantity_milli(q: &str) -> Option<i64> {
    let end = q
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(q.len());
    let (number, suffix) = q.split_at(end);
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    if whole.is_empty() && fraction.is_empty() {
        return None;
    }
    // `"1.5"` is mantissa 15 over one decimal place; `"1.2.3"` leaves a `.` in the fraction and
    // fails to parse, which is the answer a quantity that is not one deserves.
    let mantissa: i128 = format!("{whole}{fraction}").parse().ok()?;
    let places = 10i128.checked_pow(u32::try_from(fraction.len()).ok()?)?;
    let (multiply, divide): (i128, i128) = match suffix {
        "" => (1, 1),
        "n" => (1, 1_000_000_000),
        "u" => (1, 1_000_000),
        "m" => (1, 1_000),
        "k" => (1_000, 1),
        "M" => (1_000_000, 1),
        "G" => (1_000_000_000, 1),
        "T" => (1_000_000_000_000, 1),
        "P" => (1_000_000_000_000_000, 1),
        "E" => (1_000_000_000_000_000_000, 1),
        "Ki" => (1024, 1),
        "Mi" => (1024 * 1024, 1),
        "Gi" => (1024 * 1024 * 1024, 1),
        "Ti" => (1024_i128.pow(4), 1),
        "Pi" => (1024_i128.pow(5), 1),
        "Ei" => (1024_i128.pow(6), 1),
        _ => exponent(suffix)?,
    };
    let numerator = mantissa.checked_mul(multiply)?.checked_mul(1000)?;
    let denominator = places.checked_mul(divide)?;
    // **Checked, like every other step.** `numerator + denominator` is the one addition here and
    // it is reachable from a pod the apiserver accepts: unchecked it panicked in debug and wrapped
    // to a negative in release (NOTES § D81). `div_ceil` is still unstable for signed integers.
    i64::try_from(numerator.checked_add(denominator - 1)? / denominator).ok()
}

/// `e3` → ×1000, `E-6` → ÷1000000 — upstream's `decimalExponent`, which sits where a suffix sits
/// and is why it is reached from the same `match` ([`quantity_milli`]).
fn exponent(suffix: &str) -> Option<(i128, i128)> {
    let power: i32 = suffix
        .strip_prefix('e')
        .or_else(|| suffix.strip_prefix('E'))?
        .parse()
        .ok()?;
    let scale = 10_i128.checked_pow(power.unsigned_abs())?;
    // `is_negative()` rather than `power < 0`: at zero the two branches are the same value, so a
    // comparison here is a line no test can ever distinguish (NOTES § D81).
    Some(if power.is_negative() {
        (1, scale)
    } else {
        (scale, 1)
    })
}

/// A number with its trailing zeros taken off — `9.100` is a screen that looks generated.
fn trimmed(text: String) -> String {
    match text.split_once('.') {
        None => text,
        Some(_) => text.trim_end_matches('0').trim_end_matches('.').to_string(),
    }
}

/// `12`, `9.1`, `0.001` — millicores as the decimal `screens/analysis.md` § Capacity draws, and
/// the only place a cpu number stops being an integer.
fn cpu_text(milli: i64) -> String {
    trimmed(format!("{}.{:03}", milli / 1000, milli % 1000))
}

/// `23.1Gi` — milli-bytes in the unit the manifest that asked for them was written in, on
/// Kubernetes' own binary suffixes, largest one that leaves a number above 1.
///
/// **Truncated, never rounded up**, so the node's own capacity is never made to look bigger than
/// it is. **Below a kibibyte it prints the bare number, which is how Kubernetes itself spells
/// bytes** — and no node's allocatable is ever that small, so the card cannot reach it: it is the
/// arithmetic's floor, not a case with a screen behind it.
fn bytes(milli: i64) -> String {
    let value = milli / 1000;
    for (unit, scale) in [
        ("Gi", 1024_i64.pow(3)),
        ("Mi", 1024_i64.pow(2)),
        ("Ki", 1024),
    ] {
        if value >= scale {
            return format!(
                "{}{unit}",
                trimmed(format!(
                    "{}.{}",
                    value / scale,
                    (value % scale) * 10 / scale
                ))
            );
        }
    }
    format!("{value}")
}

/// The node half of rule 10's card — what a card says instead of guessing, and what it asks the
/// reader to do about it.
struct Blocking {
    evidence: String,
    action: String,
}

/// **N6 — which taint or which label is keeping this pod off every machine.** The node half of
/// [`no_node_accepted_it`]'s card, and `None` whenever the join cannot pin the refusal on one
/// thing, where rule 10 keeps the strings it has (`screens/alerts.md` § N6).
///
/// Two answers, asked in this order:
///
/// 1. **A label nothing in the cluster has.** A `nodeSelector` no node satisfies is unconditional —
///    no taint reasoning can help, and the sentence names the labels rather than the count.
/// 2. **A taint every remaining machine carries and this pod does not tolerate.** *Every* one, not
///    the most common: with a taint on two nodes of three, something else is refusing the third and
///    a card blaming the taint would send the reader to fix half a problem.
///
/// **`spec.affinity` is deliberately not read** — NOTES § Node rules names `nodeSelector`, and node
/// affinity is a term tree no v1 rule walks ([`PodSnapshot::node_selector`]). A pod refused for an
/// affinity term therefore falls to the `None` branch, which is the honest answer rather than a
/// wrong one.
///
/// **No nodes, no answer.** An empty list is a snapshot that has not been given the node watch, not
/// a cluster with no machines, and *"none of the 0 nodes"* is the sentence that mistake writes.
fn what_is_blocking_it(pod: &PodSnapshot, nodes: &[NodeSnapshot]) -> Option<Blocking> {
    if nodes.is_empty() {
        return None;
    }
    let wanted: Vec<String> = pod
        .node_selector
        .iter()
        .filter(|(key, value)| !nodes.iter().any(|n| n.labels.get(*key) == Some(*value)))
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    if let Some(first) = wanted.first() {
        return Some(Blocking {
            evidence: format!(
                "it asks for a node labelled {}, and none of the {} have {}",
                listed(&wanted),
                counted(nodes.len() as i64, "node"),
                if wanted.len() == 1 {
                    "that label"
                } else {
                    "those labels"
                }
            ),
            // One label to add is a command someone can run; a list of them is a decision, and the
            // first is the one the sentence above led with.
            action: format!("change the nodeSelector, or label a node {first}"),
        });
    }
    // Every machine whose labels do satisfy the pod. A selector each of whose labels exists
    // somewhere but on no single node leaves this empty, and that has no one-thing answer either.
    let candidates: Vec<&NodeSnapshot> = nodes
        .iter()
        .filter(|n| {
            pod.node_selector
                .iter()
                .all(|(key, value)| n.labels.get(key) == Some(value))
        })
        .collect();
    let blocking = candidates
        .iter()
        .flat_map(|n| &n.taints)
        .filter(|t| BLOCKING_EFFECTS.contains(&t.effect.as_str()) && !tolerated(pod, t))
        .find(|t| {
            candidates.iter().all(|n| {
                n.taints
                    .iter()
                    .any(|u| (&u.key, &u.value, &u.effect) == (&t.key, &t.value, &t.effect))
            })
        })?;
    let names: Vec<String> = candidates.iter().map(|n| n.id.name.clone()).collect();
    let machines = format!(
        "{} {}",
        listed(&names),
        if names.len() == 1 { "is" } else { "are" }
    );
    // A taint Kubernetes put there is translated, never named raw and never offered as something
    // to tolerate ([`managed_taint`], NOTES § D81).
    if let Some((means, action)) = managed_taint(&blocking.key) {
        return Some(Blocking {
            evidence: format!("{machines} {means}"),
            action: inflected(action, &names),
        });
    }
    // Somebody at this cluster wrote this one. `gpu=true`, or the bare key for a taint with no
    // value — the two spellings `kubectl taint` itself accepts, so the action is a line the reader
    // can type.
    let named = match &blocking.value {
        Some(value) => format!("{}={value}", blocking.key),
        None => blocking.key.clone(),
    };
    Some(Blocking {
        evidence: format!("{machines} tainted {named}, and this pod does not tolerate that taint"),
        action: format!("add a toleration for {named}, or remove the taint"),
    })
}

/// **Does this pod put up with that taint?** — upstream's `Toleration.ToleratesTaint`, field for
/// field: an empty effect tolerates every effect, an empty key tolerates every key, `Exists`
/// ignores the value and the default operator is `Equal` ([`Toleration`]).
///
/// An operator this code does not know tolerates nothing, which is upstream's answer too — and the
/// safe direction, since the alternative is a card that stays quiet about a real block.
fn tolerated(pod: &PodSnapshot, taint: &Taint) -> bool {
    pod.tolerations.iter().any(|t| {
        let effect_matches = t
            .effect
            .as_deref()
            .is_none_or(|e| e.is_empty() || e == taint.effect);
        let key_matches = t
            .key
            .as_deref()
            .is_none_or(|k| k.is_empty() || k == taint.key);
        effect_matches
            && key_matches
            && match t.operator.as_deref().unwrap_or("Equal") {
                "Exists" => true,
                "Equal" | "" => {
                    t.value.as_deref().unwrap_or("") == taint.value.as_deref().unwrap_or("")
                }
                _ => false,
            }
    })
}

// --- THE NODE RULES END ---

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
