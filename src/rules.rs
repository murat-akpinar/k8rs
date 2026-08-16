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
use k8s_openapi::jiff::{SignedDuration, Timestamp};
use std::collections::BTreeMap;
use x509_parser::pem::parse_x509_pem;

/// How bad it is. **Declaration order is severity order** — the derived `Ord` sorts the
/// Alerts list and `--once`, and a test asserts it (NOTES § D35).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Broken now: something is not doing its job and someone has to answer it.
    Critical,
    /// Wrong now, broken soon. It still needs an answer, just not this minute.
    Warn,
    /// Worth knowing; nothing here is broken. **Nothing drawn in the Alerts list is an `Info`**
    /// (NOTES § D2) — a rule can live in this file and still be `Info` (N4's kubelet skew → the
    /// Versions report), and C1's expiring band is one [`analyze`] itself returns for a report to
    /// read (NOTES § D87). Both files share this scale.
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
    /// | 5 | serving: `started_at` on [`ContainerState::Running`] — when the run the counter last opened began, and the field that ages the card out (NOTES § D100); otherwise [`Terminated::finished_at`] like the row above | the *previous* run's [`Terminated::started_at`], which dates the run before this one |
    /// | 7 | the **later** of [`PodSnapshot::ready`]'s `last_transition` and the container's own `started_at` — a floor, since `Ready` is pod-scoped (NOTES § D71) | [`PodSnapshot::scheduled`]'s |
    /// | 8 | **`None`** — a standing property, not an event (NOTES § D69) | `metadata.creationTimestamp` |
    /// | 12 | `deletionTimestamp − grace` (NOTES § D46) | the `deletionTimestamp` itself, the deadline |
    /// | 14 | `metadata.creationTimestamp` — the one rule whose event never happened (NOTES § D74) | — |
    /// | 15 | [`Terminated::finished_at`] on the run in [`ContainerSnapshot::state`] — the run the container is stopped in **now**, which is the whole of what tells this rule from rules 1, 2 and 6 | `last_terminated`'s, a run this container has never had |
    /// | N1 | the `Ready` condition's `last_transition` — the node's own, and the one it fires on | — |
    /// | N2 | the cordon taint's [`Taint::added_at`], which dates the taint and not the cordon (NOTES § D65) | — |
    /// | N3 | *that* condition's `last_transition` | `Ready`'s, off the same flat `Vec` |
    /// | N6 | the pod's `scheduled` `last_transition` | the blocking node's taint `added_at` |
    ///
    /// A rule not in the table owes the same answer, and owes it in a test.
    ///
    /// **`None` is the empty right edge** — no field to read, or a moment [`age`] refuses.
    ///
    /// **Being an `Option` is not what keeps the epoch off a card, and this note used to say it
    /// was.** A zero stamp reaches these types as a *value*: containerd leaves `StartedAt` at `0`
    /// on a start failure and the kubelet marshals `time.Unix(0, 0)` as a real RFC3339 stamp, so
    /// the field arrives `Some(1970-01-01T00:00:00Z)` and dates as *20678 days ago*. [`lasted`] is
    /// where that is caught, and it is caught by reading the value rather than by the type. No
    /// field feeding *this* one is known to carry it — `finishedAt` is real on every shape
    /// measured — but the sentence that said it could not happen is what made the shape
    /// unthinkable next door.
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
/// **`screens/widgets.md` § 1b is the whole of this ladder** — one table, read top to bottom, so
/// the three renderers that draw an age cannot drift apart (NOTES § D68).
///
/// | age | text |
/// |---|---|
/// | ahead by more than [`SKEW_ALLOWANCE`] | **`None`** — draw nothing |
/// | ahead by less, or under one whole second | `just now` |
/// | 1 s … 59 s | `40s ago` |
/// | 1 min … 59 min | `4 min ago` |
/// | 1 h … 47 h | `1 hour ago`, `47 hours ago` |
/// | 48 h and up | `2 days ago`, `6 days ago` |
///
/// **The hours rung runs to 48, not to 24.** `1 day ago` covered 24h01m through 47h59m, a whole
/// day of resolution thrown away in the one band where the reader is asking *before or after
/// yesterday's change window?* — and `kubectl`'s own `HumanDuration` prints `30h`, `47h`, `2d3h`,
/// so k8rs was coarser than the command it teaches. **`1 day ago` is therefore not a reachable
/// string**, and neither is `0s ago`; both absences are the spec's.
///
/// Every rung truncates, and **`min` stays abbreviated and unpluralised** because that is how
/// the screens spell it (NOTES § D68).
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
    } else if elapsed.as_hours() < 48 {
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

/// **The subtraction and its three refusals, as a number rather than as a sentence** — one run's
/// length, or `None` where the record cannot support one (NOTES § D113).
///
/// **It exists because two callers want different things from one fact and must not disagree about
/// which runs have it.** [`lasted`] spells it for the reader; [`finished_action`] compares it
/// against [`PROBE_FLOOR`] to decide which of its doors goes first. Re-deriving the second from
/// the first would mean parsing `2s` back out of a string, and re-deriving it beside this would be
/// two copies of the epoch guard.
///
/// The three refusals are [`lasted`]'s, and that doc carries why each one is here: no start or no
/// end, a start at the Unix epoch (**a container whose start failed**, and a real value the API
/// sends), and an end before the beginning.
fn run_length(run: &Terminated) -> Option<SignedDuration> {
    let elapsed = run
        .finished_at
        .as_ref()?
        .0
        .duration_since(ever_started(run)?.0);
    (elapsed >= SignedDuration::ZERO).then_some(elapsed)
}

/// **When the run began, or `None` where it never began at all** — the *start* half of
/// [`run_length`]'s three refusals, split out because one caller may only have that half
/// (NOTES § D113).
///
/// **The epoch is a value the API really sends**: containerd fills the other four fields and
/// leaves `StartedAt` at `0` when it never got the process going, and `time.Unix(0, 0)` is not
/// Go's zero time, so it marshals as a real RFC3339 stamp that no `Option` on the path can see
/// (NOTES § D112).
///
/// **[`failed_run_action`] keys on this and not on [`run_length`], and the difference is a
/// clock.** `run_length` also refuses a run whose `finishedAt` is *before* its `startedAt` —
/// which is not a container that never ran but a **backwards clock step between two wall-clock
/// stamps**: `chrony`'s `makestep` after a bad RTC, a VM resumed from a snapshot. That container
/// ran, has a `containerID`, and its log holds the panic; keyed on `run_length` it was told
/// *what they name is not in the image* under [`describe`], with the duration missing from the
/// same card because [`ran_for`] shares that predicate — so nothing on the screen let the reader
/// see the inconsistency. Before this family that case cost a duration line, which is a miss;
/// after it, it cost a false diagnosis. Reasoned from what the two fields are, not measured.
fn ever_started(run: &Terminated) -> Option<&Time> {
    run.started_at
        .as_ref()
        .filter(|t| t.0 != Timestamp::UNIX_EPOCH)
}

/// **The earliest a health check can end a run, on a pod that declares none of the probe's
/// timings** — `initialDelaySeconds: 0`, `periodSeconds: 10`, `failureThreshold: 3`, which are the
/// API's defaults. The probe runs at 0s, 10s and 20s and the *third* consecutive failure is the
/// one that kills, so twenty seconds is the floor and not a round number anybody liked
/// (NOTES § D113).
///
/// **It orders [`finished_action`]'s doors and closes none of them.** A pod that sets a shorter
/// `periodSeconds` moves the real floor down, so the number is read as *unlikely* and never as
/// *impossible* — which is why the card that reads it says a health check *rarely* kills a run
/// this short and still names the `Killing` line.
const PROBE_FLOOR: SignedDuration = SignedDuration::from_secs(20);

/// **How long one container run lasted** — `2s`, `40 min`, `3 hours`, `6 days`. Rules 1, 5 and
/// 6 all show it, because it is the first fork of every crashloop triage and
/// `kubectl describe` leaves the subtraction to a human (NOTES § D51).
///
/// **Not [`age`] with the suffix taken off.** A span is not a moment, so both of `age`'s
/// special rungs are wrong here: a run that lasted no measurable time is an ordinary instant
/// crash, and *"under a second"* is the fact rather than a refusal to answer. The rungs and
/// the pluralisation are still shared, through [`counted`].
///
/// **The days rung starts at 24 hours here and at 48 in [`age`], and that is not a missed
/// edit.** `age` answers *when*, where `1 day ago` throws away the resolution a reader needs to
/// place an event either side of yesterday; this answers *how long for*, where `1 day` is the
/// natural reading and `30 hours` is not. A find-and-replace across the two breaks this one.
///
/// `None` when either end is missing, when the run ended before it began, **and when the run
/// never started at all.**
///
/// **The epoch is a value the API really sends, and it means *this never ran*.** A container
/// whose start failed — a mistyped `command`, one of the commonest broken-pod states there is —
/// carries `startedAt: 1970-01-01T00:00:00Z` beside a real `finishedAt`: containerd
/// (`internal/cri/server/container_start.go:67-73`) sets the other four fields and leaves
/// `StartedAt` at `0`, and the kubelet writes `metav1.NewTime(cs.StartedAt)` unconditionally.
/// `time.Unix(0, 0)` is **not** Go's zero time, so it marshals as a real RFC3339 stamp rather
/// than as `null` and no `Option` anywhere on the path can see it. Measured on kind v1.36.1, where
/// the subtraction printed **`ran for 20681 days`** and was still doing so seven restarts later
/// (`reports/2026-08-16-terminated-record-stamps-and-authors.md` § 1).
///
/// **`None` and not `0s`**, because those are different sentences: the exit code and the
/// runtime's message carry the diagnosis, and a duration clause about a run that never began is
/// noise at best. **One guard here rather than three at the callers** — [`crash_looping`],
/// [`previous_run_failed`] and [`stopped_for_good`] all reach this through [`ran_for`].
fn lasted(run: &Terminated) -> Option<String> {
    let elapsed = run_length(run)?;
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

/// **`ran for 4s` — the one spelling of a run's duration on a card, for every rule that prints
/// one.** [`crash_looping`] wrote *the last run lasted …* and [`previous_run_failed`] and
/// [`stopped_for_good`] wrote *ran for …* off the same [`lasted`] call, which is NOTES § D85's own
/// class — one fact, two wordings, on one screen. It also cost a card: [`one_card_per_action`]
/// collapses a repeated sentence only where the beaten card's facts are a *subset* of the
/// survivor's, so two spellings of one fact kept two byte-identical cards on
/// [`Ending::CodeUnknown`], in the mechanism written to prevent exactly that.
///
/// **The shorter spelling won**, because rule 1's title already says *the last run on record*, so
/// the context the longer wording carried is on the card either way.
///
/// **That reason used to be second and a wrong number was first** — *rule 1's cards sit at the
/// ten-line cap with no slack*, written the day before `screens/alerts.md` measured its own parts
/// and found the card cap was **12**, not 10 (NOTES § D113). The slack was real but the cap was
/// not, so the argument is gone and the conclusion is not: two spellings of one fact on one screen
/// is NOTES § D85 at any cap, and the fold below needs a subset either way.
///
/// **`None` on [`Ending::CodeUnknown`], because `finishedAt` is not a fact about the run there.**
/// containerd stamps it when it *recovers* and finds the task gone
/// (`internal/cri/server/restart.go:353-357`), not when the run ended: measured on kind v1.36.1, a
/// container that ran for 50 seconds behind a node that was away for three minutes printed
/// **`ran for 3 min`**, and a node down overnight turns the same run into *ran for 8 hours*
/// (`reports/2026-08-16-terminated-record-stamps-and-authors.md` § 2). The card that is careful
/// not to name an ending may not state a duration nobody measured.
///
/// **Answered arm by arm**, so the next ending added has to say whether its `finishedAt` measures
/// the run at all (NOTES § D95). The other five fall through to [`lasted`], which is the right
/// place for *no stamps* and *stamps that do not subtract*; what cannot be delegated to it is *a
/// stamp that measures something else*, because nothing about the value says so.
///
/// **It also buys the fold on that ending.** With no duration there [`previous_run_failed`]'s
/// facts reduce to [`container_fact`], which is a strict subset of both neighbours', so
/// [`one_card_per_action`] collapses the repeated sentence on **both** shapes instead of on
/// whichever of the two happened to print a duration. **On every other ending the subset is held
/// the other way round** — [`restarting_repeatedly`] carries [`ran_for`] deliberately, for the
/// same fold (NOTES § D113).
///
/// Otherwise `None` follows [`lasted`]: a record with no stamps, or one that never started, has no
/// duration and no card invents one.
fn ran_for(run: &Terminated) -> Option<String> {
    match ending(run) {
        Ending::CodeUnknown => return None,
        Ending::Finished
        | Ending::Stopped
        | Ending::Failed
        | Ending::Unwatched
        | Ending::RestartRule => {}
    }
    Some(format!("ran for {}", lasted(run)?))
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
    /// **The run the container is stopped in *now*, as opposed to
    /// [`last_terminated`](ContainerSnapshot::last_terminated), which is the run before this
    /// one.** An init container that failed and is not being retried sits here — `Init:Error`,
    /// which NOTES § D27 lists beside `Init:CrashLoopBackOff` — and so does a regular container
    /// that stopped for good inside a pod that is still `Running`, which is
    /// [`stopped_for_good`]'s subject and the one card any rule draws off this field
    /// (NOTES § D96).
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
    /// **The policy this container is actually under** — its own `spec.containers[].restartPolicy`
    /// where it declares one, and the pod's `spec.restartPolicy` where it does not. Rule 15's
    /// fourth condition, and the one field that answers *will Kubernetes start this again*
    /// ([`stopped_for_good`], NOTES § D96).
    ///
    /// **Derived here rather than joined later**, out of the same two fields [`ContainerRole`] is
    /// derived from — and **the two readings are not the same one**: the role asks the
    /// container's own field alone, because a *regular* container is `Regular` whatever the pod
    /// says, while this one falls back on purpose.
    ///
    /// **Two spec paths feed one field, and the prune has to keep both** (invariant 6):
    /// `spec.containers[].restartPolicy`, which [`ContainerRole`] already needed, **and
    /// `spec.restartPolicy`, which is new here and is named by no other snapshot field** — a
    /// prune written by reading these structs would keep the first and drop the second, and this
    /// rule would then be silent on every pod that does not override per container, which is
    /// almost all of them.
    ///
    /// **`None` fires nothing.** The API server defaults `spec.restartPolicy` to `Always` on every
    /// accepted create, so an empty answer means the field was pruned or the object never reached
    /// validation, and neither is a licence to guess.
    ///
    /// **It is a policy and not a verdict, and the difference is a gap this layer does not close**:
    /// `spec.containers[].restartPolicyRules` can override it upward per exit code. The generated
    /// types carry that field at the `v1_36` feature `Cargo.toml` pins — it arrives at `v1_34` —
    /// but no snapshot field here names it, so nothing prunes it in and no rule reads it; reading
    /// it is a box of its own (NOTES § D99). [`ContainerSnapshot::restarts`] answers in its place,
    /// and goes on answering after the field is read: no cluster below 1.34 can carry the field at
    /// all, and the pin sits above the cluster on purpose (NOTES § D97, § D99). The case is argued
    /// once, at [`stopped_for_good`].
    pub restart_policy: Option<String>,
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

/// One pod, reduced to what rules 1–8, 10 and 12–15 read, plus the pod half of the N5 and
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
    /// **How many pods exist on the template the controller is currently rolling out** —
    /// `status.updatedReplicas` for a Deployment or StatefulSet, `updatedNumberScheduled` for a
    /// DaemonSet, and a ReplicaSet's own **required** `status.replicas`: a ReplicaSet *is* one
    /// template, so every pod it has is a pod on the version it is rolling out (NOTES § D82).
    ///
    /// **`None` is zero, the [`ready`](WorkloadSnapshot::ready) reading and not the
    /// [`desired`](WorkloadSnapshot::desired) one** — `updatedReplicas` carries `omitempty`, so
    /// the API server omits it exactly when no pod of the new version exists, which is the worst
    /// state W2 has to report. **A ReplicaSet therefore never answers `None` here**, and reading
    /// its absent field as *"none of them are updated"* was what made [`short_of_pods`] true of
    /// every healthy ReplicaSet alive (NOTES § D82).
    ///
    /// **It is the number W2 prints on its amber card, and `readyReplicas` cannot be.** A
    /// RollingUpdate Deployment keeps its old pods for as long as the new ones cannot start
    /// whenever `maxUnavailable` resolves to 0 — the 25% default *rounds down* to 0 at one, two or
    /// three replicas, and `broken-rollout` sets it to 0 outright — so `readyReplicas` stays equal
    /// to `spec.replicas` for the whole of a failed rollout, and `2 of 2 ready` is a true sentence
    /// about a rollout that is dead (NOTES § D42, § D82). Which shape each of the three counters
    /// is the only one to see is argued once, in [`short_of_pods`].
    ///
    /// **A StatefulSet holds this below `desired` forever, by design**, so a future rule that
    /// read it the way [`short_of_pods`] does would fire permanently on a healthy one: an
    /// `updateStrategy` with a `rollingUpdate.partition`, or `OnDelete`, leaves every pod below
    /// the partition on the old revision until a human touches it. No v1 rule reads a StatefulSet
    /// here — W2 is gated to Deployments — and one that did would need that gate first.
    pub updated: Option<i32>,
    /// **How many pods the workload wants that are not answering** — `status.unavailableReplicas`
    /// for a Deployment, `status.numberUnavailable` for a DaemonSet. **A StatefulSet and a
    /// ReplicaSet have no such field and answer `None`**, which reads as zero and costs them
    /// nothing: neither kind surges, so the two counters above already see everything either of
    /// them can be short of.
    ///
    /// **The only counter that sees a rollout of one replica** (NOTES § D82). At `replicas: 1`
    /// upstream's `ResolveFenceposts` gives `maxSurge: 1, maxUnavailable: 0`: the new ReplicaSet
    /// is scaled to exactly one and the old one is left at one, so a second revision that cannot
    /// start reads `spec.replicas 1 · readyReplicas 1 · updatedReplicas 1` — [`ready`] and
    /// [`updated`] both say the workload is whole, and the surge that is not landing is visible
    /// only here.
    ///
    /// The Deployment controller writes it `sum(replicaset.spec.replicas) - availableReplicas`,
    /// floored at zero, which is why a healthy Deployment has none — **the ReplicaSets' spec, never
    /// the Deployment's own**, so a Deployment scaled to zero reaches none only once the controller
    /// has scaled those down and written the status back, and carries a positive counter against
    /// `spec.replicas: 0` until it does ([`short_of_pods`]). **`None` is zero**, the [`ready`]
    /// reading again: `omitempty` omits it exactly when nothing is unavailable.
    ///
    /// [`ready`]: WorkloadSnapshot::ready
    /// [`updated`]: WorkloadSnapshot::updated
    pub unavailable: Option<i32>,
    /// W1: `ReplicaFailure` with reason `FailedCreate`, message verbatim. W2: `Progressing` with
    /// reason `ProgressDeadlineExceeded` — which fires only while the counters above show a
    /// shortfall and no finding that explains a shortfall is already on the list.
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
            // **The miss happens, and what it costs is asserted rather than argued** — the API
            // *can* produce a status with no declaration, and immutability is not what would
            // prevent it: a node implementation that is not a kubelet is. On Tencent TKE virtual
            // nodes the provider injects a managed logging container into
            // `status.containerStatuses` with no entry in `spec.containers` (k9s #4145), and
            // virtual-kubelet, serverless nodes and sandboxed runtimes all sit in that gap.
            // `declared` is then `None`, so the container decodes with no requests and no limits
            // and takes its role from the list its status arrived in — which is
            // `a_container_status_with_no_declaration_decodes_with_nothing_the_spec_would_have_given_it`,
            // not a claim made here. `ephemeralContainers` is a separate matter: that list grows
            // by design and its statuses are deliberately not read (NOTES § D46).
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
            let own = declared.and_then(|c| c.restart_policy.as_deref());
            let restartable = own == Some("Always");
            let role = match (is_init, restartable) {
                (true, true) => ContainerRole::Sidecar,
                (true, false) => ContainerRole::Init,
                (false, _) => ContainerRole::Regular,
            };
            // **The same two fields, read the other way**: the role wants the container's own
            // answer and nothing else, and the effective policy falls back to the pod's
            // ([`ContainerSnapshot::restart_policy`]). One expression each, off one lookup.
            let restart_policy = own.or(spec.restart_policy.as_deref()).map(str::to_string);
            ContainerSnapshot {
                name: s.name,
                image: s.image,
                role,
                restart_policy,
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
    updated: Option<i32>,
    unavailable: Option<i32>,
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
        updated,
        unavailable,
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
            status.updated_replicas,
            status.unavailable_replicas,
            conditions(status.conditions),
        )
    }
}

/// **Covered from the 2026-08-13 capture on.** `tests/fixtures/statefulsets.json` was an empty
/// list until the trip and this impl shipped with no test that could fail; `broken-sts` is
/// partially ready, which is the one state that tells `spec.replicas` from
/// `status.readyReplicas` (NOTES § D40).
impl From<StatefulSet> for WorkloadSnapshot {
    fn from(s: StatefulSet) -> Self {
        let status = s.status.unwrap_or_default();
        workload(
            ObjectKind::StatefulSet,
            s.metadata,
            s.spec.and_then(|s| s.replicas),
            status.ready_replicas,
            status.updated_replicas,
            // A StatefulSet replaces its pods in place, ordinal by ordinal, and has no such
            // field ([`WorkloadSnapshot::unavailable`]).
            None,
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
            // A ReplicaSet is one version of one template, so every pod it has is on the
            // version it is rolling out — and `status.replicas` is required, never absent
            // ([`WorkloadSnapshot::updated`]).
            Some(status.replicas),
            // It cannot surge, so it has no such field either
            // ([`WorkloadSnapshot::unavailable`]).
            None,
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
            // `updatedNumberScheduled` and `numberUnavailable` are the two the API marks
            // optional, so both arrive already shaped the way the fields above read them.
            status.updated_number_scheduled,
            status.number_unavailable,
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
/// **Read by rules 2, 5, 7, 10 and 13** — one threshold for one question, so changing it moves all
/// five. Rule 5 joined them on 2026-08-15 for rule 2's question exactly: *is this old news on a
/// container that has been fine since?* (NOTES § D100).
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
/// Rules 1–8, 10, 12–15, the node rules that draw a card — N1, N2 and N3 — the W-series, W1 and
/// W2, and **C1, whose two bands leave here through different doors** (NOTES § D87): the expiring
/// half is `Info` and is the Certificates report's input, the expired half is `Critical` and is an
/// Alerts card, and C1 is called from here — where N4 and N5 are not — because that second half has
/// to be. **N4 and N5 are not missing, they are `Info`**: they are the Versions and Capacity
/// reports' input and have no second band that is a card, so `analysis.rs` calls them and this does
/// not (NOTES § D2). **N6 is not here either, and is not missing**: it is the node half of
/// rule 10's card, which is why [`no_node_accepted_it`] takes the nodes.
///
/// **Rules 1–6 read every container the pod has**, in either status array and whichever of
/// [`ContainerRole`] it is (NOTES § D27, § D75). **Rule 7 is the one exception and reads regular
/// containers only.** **Rule 15 reads every container too and reaches only the regular ones**, out
/// of the policy it gates on rather than out of a role check ([`stopped_for_good`]). **Rules 8 and
/// 10 are not container rules at all** — rule 10 reads a pod condition, which is what lets it fire
/// on a pod that has no containers — and **rule 13 is a third shape**: one card about the *pod*,
/// reached by walking its containers.
///
/// **A pod that finished is not broken now**, so rules 1–8, 10, 13, 14 and 15 skip `Succeeded` and
/// `Failed`: this screen holds what is broken *now*, and their restart counts and last exits are
/// not that (NOTES § D2, § D71).
///
/// **And that is where those pods stop — they are on no k8rs screen today** (NOTES § D96). This
/// said their counts *belong to the Waste report* until 2026-08-15, which is a promise and not a
/// destination: `analysis.rs` does not exist, the Waste report's charter is Evicted/Completed
/// **pileups** rather than a per-pod diagnosis of a Job pod that died a minute ago, and Jobs are
/// not watched at all. The skip is still right; what it hands the pod to is nothing.
///
/// **Rule 12 is deliberately outside the skip** and is the one pod rule called *before* the
/// gate: a `Succeeded` pod that will not go away is still stuck ([`stuck_terminating`]).
/// **Both phases are captured** — `succeeded.json` and `failed.json`, each carrying
/// the restart count and the failed previous run the skip has to swallow.
pub fn analyze(snapshot: &ClusterSnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();
    // First, because it is the one finding that explains why every other one might be missing: a
    // credential that has run out reaches nothing to iterate.
    findings.extend(kubeconfig_certificate_expiring(snapshot));
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
            // Collected per container rather than appended straight to the list, because
            // [`one_card_per_action`] is scoped to the container and this loop is where that
            // scope exists (NOTES § D102).
            //
            // **Each rule is labelled with what it read**, which is the one thing only this
            // caller knows: a `Finding` does not carry the rule that drew it, and re-deriving the
            // split from the container would be a second copy of eight rules' triggers
            // ([`lost_run_yields_to_the_present`], NOTES § D113). [`Reads::Now`] is a rule whose
            // card is about what the container is doing at this moment; [`Reads::Record`] is one
            // whose whole subject is a run that is already over.
            //
            // **[`restarting_repeatedly`] is [`Reads::Now`] and it is the one that needs saying.**
            // Its *trigger* is `restartCount`, which is history — but its card is about a
            // container that is restarting or serving now, it stands down on the current state in
            // three places, and it carries a count nothing else on the screen does. Both halves of
            // the label matter and both come out the same way: it is a card the reader may not
            // lose, and it is a card whose presence means the container's present is accounted for.
            let cards = vec![
                (Reads::Now, crash_looping(pod, c)),
                (Reads::Record, out_of_memory(&snapshot.now, pod, c)),
                (Reads::Now, image_not_pulled(pod, c)),
                (Reads::Now, container_config_missing(pod, c)),
                (Reads::Now, restarting_repeatedly(&snapshot.now, pod, c)),
                (Reads::Record, previous_run_failed(pod, c)),
                (Reads::Now, running_but_not_ready(&snapshot.now, pod, c)),
                (Reads::Now, stopped_for_good(pod, c)),
            ];
            findings.extend(one_card_per_action(lost_run_yields_to_the_present(cards)));
        }
    }
    // **The W-series runs last, and in two passes rather than one.** W2 asks whether anything
    // already on the list explains the shortfall it is about to report, so every pod rule *and* W1
    // have to have finished first (NOTES § D28) — and the second pass collects before it appends,
    // so a Deployment cannot be suppressed by a card drawn about it in the same pass.
    for w in &snapshot.workloads {
        findings.extend(pods_were_never_created(snapshot, w));
    }
    // Not every finding, only the ones that say why a pod is not ready ([`explains_a_shortfall`]).
    let explained: Vec<&ObjectId> = findings
        .iter()
        .filter(|f| explains_a_shortfall(snapshot, f))
        .map(|f| workload_owner(snapshot, &f.owner))
        .collect();
    let gave_up: Vec<Finding> = snapshot
        .workloads
        .iter()
        .filter_map(|w| rollout_gave_up(w, &explained))
        .collect();
    findings.extend(gave_up);
    findings
}

/// **Which half of the container a rule read** — its `state`, or the `lastState` record of a run
/// that is over. [`analyze`] labels each call with it and nothing else does, because a [`Finding`]
/// does not carry the rule that drew it (NOTES § D113).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reads {
    /// The rule's trigger is what the container is doing at this moment.
    Now,
    /// The rule's trigger is `lastState.terminated` — a run that has already ended.
    Record,
}

/// **A card about a run nobody watched end may not stand beside a card about what the container is
/// doing now** (NOTES § D113).
///
/// **Because that card can never be dated and so can never age off the screen.** The kubelet
/// synthesizes an [`Ending::Unwatched`] record with `reason`, `message` and `exitCode` and no
/// `finishedAt` at all ([`last_log_line`]), so [`Finding::timestamp`] is `None`, the age column is
/// blank, and the card sits in the ageless block at the bottom of its severity band for the life
/// of the pod. Every other permanence in this file was answered with a clock — rule 2's
/// [`NOT_READY_GRACE`], rule 5's serving card, rule 6's [`doing_its_job`] — and this is the first
/// one where there is no stamp to read. Measured on `lost-notready`: a failing readiness probe,
/// beside a loss long over.
///
/// **A restart count in its evidence was the other proposal and was rejected**: the count is every
/// restart from every cause, and on a card whose subject is *one* lost status it reads as *this
/// happened N times* — `PRIOR-ART.md`'s incomplete-denominator class. The reader genuinely cannot
/// tell *once* from *ongoing* from the object, so the card does not claim either; what it can be
/// asked is whether it should be on screen at all, which is a suppressor question and not a
/// wording one.
///
/// **This is [`analyze`]'s decision and not a rule's**, for [`one_card_per_action`]'s reason: no
/// rule may be made to know what its neighbour drew (NOTES § D102). Nothing is deleted where the
/// lost status *is* the trouble — a container whose only card is this one keeps it, which is
/// exactly the shape it is the answer for.
///
/// **The candidate is narrow on three keys.** The action has to be [`unwatched_action`]; the rule
/// has to have read a record rather than the container's current state; and the card has to carry
/// **no [`Finding::timestamp`]**, which is this suppressor's own premise made a condition rather
/// than a fact assumed about it. So [`crash_looping`]'s and [`restarting_repeatedly`]'s cards on
/// the same ending are never candidates, whatever they say — rule 1's is about a container backing
/// off *right now*, and rule 5's carries a restart count nothing else on the screen has — and a
/// lost-run card that somehow *does* date keeps its place, because the age column can then retire
/// it the ordinary way. Today that leaves [`previous_run_failed`] as the only rule that can lose a
/// card here.
///
/// **Two mechanisms hold that, and only one of them is the label — which this doc got wrong for a
/// turn** (NOTES § D113). [`restarting_repeatedly`], [`running_but_not_ready`],
/// [`image_not_pulled`] and [`container_config_missing`] are held by their [`Reads`] label and by
/// nothing else: flip one and the undatable card ships beside theirs. [`crash_looping`] and
/// [`stopped_for_good`] are held by the **container's state** — no other rule labelled
/// [`Reads::Now`] can draw about a container in `CrashLoopBackOff` or sitting in
/// `state.terminated`, so `present` is `false` there and their labels are never consulted at all.
/// `every_rule_that_reads_the_present_is_proved_to_be_one` asserts which is which, because a rule
/// that starts co-firing is a rule whose label suddenly becomes load-bearing with nothing saying
/// so.
///
/// **The stamp condition is also what keeps [`one_card_per_action`]'s own rule intact**: that fold
/// refuses to delete a card carrying a [`Finding::timestamp`] the survivor lacks, and a suppressor
/// whose whole justification is *this card can never be dated* may not be the thing that deletes a
/// dated one (NOTES § D102).
///
/// **What it changes over [`one_card_per_action`] alone** is the pairs that fold has no answer
/// for: rules 1 and 5 already collapse rule 6's card by saying the same sentence, so what is new
/// is rule 6's lost-run card standing beside rules 3, 4, 7 and 15 — different sentences, one
/// container, and only one of them about the present.
fn lost_run_yields_to_the_present(cards: Vec<(Reads, Option<Finding>)>) -> Vec<Finding> {
    let present = cards
        .iter()
        .any(|(reads, f)| *reads == Reads::Now && f.is_some());
    cards
        .into_iter()
        .filter_map(|(reads, f)| {
            f.filter(|f| {
                !(present
                    && reads == Reads::Record
                    && f.timestamp.is_none()
                    && f.action == unwatched_action())
            })
        })
        .collect()
}

/// **One card per action string, about one container** — the second copy of a shared sentence
/// says nothing new, so the card carrying it goes (NOTES § D102).
///
/// **This is [`analyze`]'s decision and not a rule's**, for [`explains_a_shortfall`]'s reason: no
/// rule may be made to know what its neighbour drew, or the two grow a dependency the file's
/// purity does not survive. What a rule owes is a true card; what this owes is that two true cards
/// about one container do not tell one story twice.
///
/// **The scope is the container, and the reason is the converse of the obvious one.** A pod-wide
/// fold cannot eat the neighbour's card *today* — measured, not argued: moving the fold out of the
/// caller's loop leaves the suite green, because every card the container rules draw leads with
/// [`container_fact`] and no two containers share one, so the subset clause below refuses every
/// cross-container pair on the first fact. **That is a property of eight rules, not of this
/// function**: the day one of them draws a card whose evidence does not lead with the container —
/// or leads with a fact that came off the API as free text, which [`restarting_repeatedly`]'s
/// image already is — a pod-wide fold starts deleting the neighbour's card, silently. The
/// caller's `for c in &pod.containers` costs nothing and is the insurance;
/// [`Finding::object`] is the pod, so nothing on a card would have said otherwise.
///
/// **The more severe survives** — `Critical` is declared first, so the smaller [`Severity`] wins —
/// **and a tie goes to the rule that ran first**, which is the order [`analyze`] already calls them
/// in. Nothing else moves: the survivors keep their emission order, because a `sort` would reorder
/// every card in the file to settle two.
///
/// **A shared sentence is not enough on its own: the card that goes has to add nothing.** Every
/// fact on it — its evidence split on [`FACTS`] — must already be on the card that beats it, and it
/// may carry no [`Finding::timestamp`] the survivor lacks. **This is checked and not assumed.** A
/// duplicated sentence is a cheap failure; a fact deleted off the screen because a neighbour
/// happened to word its advice the same way is not, and the drop is one function away from the
/// rules whose evidence decides it. The pair this fold was written for passes because rule 6's
/// only fact is [`container_fact`], which is the survivor's first — and because [`lasted`] is
/// `None` on a record with no stamps, which is three inferences away from here and is exactly the
/// kind of thing that stops being true without anyone noticing.
///
/// **Today this fires on three endings and every pair is against [`previous_run_failed`].**
/// [`Ending::Unwatched`] through [`unwatched_action`] and [`Ending::CodeUnknown`] through
/// [`no_exit_code_action`] are the two where three rules answer with one sentence, each drawn
/// beside [`crash_looping`] and beside [`restarting_repeatedly`]. **[`Ending::Failed`] is the
/// third and it is new** (NOTES § D113): all three rules take [`failed_run_action`] whole, so a
/// `Failed` card with no termination message on it folds too — `broken-notfound` draws **one**
/// card where it drew two. The inventory is asserted over the corpus by
/// `only_rule_6_shares_a_sentence_with_a_neighbour_and_only_where_nothing_read_the_ending` rather
/// than left here as prose, which is what caught this paragraph going stale.
///
/// **What keeps the `Failed` fold honest is a subset relation held on purpose, and it is held from
/// both sides.** Rule 6's facts are [`container_fact`] · the quote · [`ran_for`]; the survivors
/// carry the first and third, and [`restarting_repeatedly`] carries [`ran_for`] **because of this
/// fold** — added on 2026-08-16, when rules 5 and 6 became identical from the arrow down and a
/// duration was the only thing left standing between two byte-identical cards (NOTES § D113).
///
/// **So the caveat runs in two directions, and neither is an accident to rely on.** A later box
/// adding any *other* fact to rule 6 stops the fold on **every** container rather than only on
/// those with a message, and the reader gets two cards saying one sentence everywhere. A later box
/// *removing* [`ran_for`] from a survivor does the same thing from the other end. The card that
/// still stands is the one carrying the container's last words, which is a fact worth a card —
/// that is the operator review's ruling, and this is the relation it rests on.
///
/// **All four collapse, and getting there took two fixes rather than a tolerance.** Rule 6's only
/// fact on either ending is [`container_fact`], which is the survivor's first. It was not so for
/// one turn: [`crash_looping`] and [`previous_run_failed`] spelled one duration two ways, and on
/// [`CodeUnknown`](Ending::CodeUnknown) — the one ending whose record carries real stamps — that
/// left two byte-identical ten-line cards in a sixteen-row pane. (Ten was what those two measured;
/// the pane's cap is 12 and the number in this sentence is the drawing, not the budget —
/// NOTES § D113.) **Then the duration turned out
/// not to belong on that ending at all**: containerd stamps `finishedAt` when it recovers rather
/// than when the run ended, so [`ran_for`] refuses it, and rule 6 adds nothing to either
/// neighbour. Two defects, one visible only through this clause. **The inventory is asserted over the corpus and not left here as a claim** —
/// a rule that starts wording its advice like a neighbour would otherwise begin deleting cards
/// with nothing going red.
///
/// **One of the keys is free text from the API, and where it lands got the guard stronger**
/// (invariant 9, NOTES § D113). Rule 6's `Failed` arm handed [`last_words`] a
/// `Terminated::message` the workload wrote, and until 2026-08-16 that was the card's whole
/// **action** — the fold's primary key, where a crafted message equalling another rule's advice
/// would have deleted a card. It is a fact on the **evidence** line now, which the subset clause
/// reads: a crafted message can only *add* a fact, and a card with one more fact is **harder** to
/// beat, not easier. The exposure is closed rather than moved.
///
/// **The frame is still the guard on what is left.** [`last_words`] wraps the quote in a constant
/// prefix no fact any other rule prints begins with, so a crafted message cannot equal a
/// neighbour's fact however exactly it is copied — pinned by a test, because a guard nobody wrote
/// down is one the next edit removes.
fn one_card_per_action(cards: Vec<Finding>) -> Vec<Finding> {
    // O(n²) over at most eight cards, which is why the drops are picked rather than sorted for.
    let beaten: Vec<bool> = cards
        .iter()
        .enumerate()
        .map(|(i, f)| {
            cards.iter().enumerate().any(|(j, g)| {
                g.action == f.action
                    && (g.severity, j) < (f.severity, i)
                    && (f.timestamp.is_none() || f.timestamp == g.timestamp)
                    && f.evidence
                        .split(FACTS)
                        .all(|fact| g.evidence.split(FACTS).any(|kept| kept == fact))
            })
        })
        .collect();
    cards
        .into_iter()
        .zip(beaten)
        .filter_map(|(f, beaten)| (!beaten).then_some(f))
        .collect()
}

/// `kubectl describe pod …` — the one command that shows a container's current state, how its
/// last run ended, its restart count, the limits it is running under and its mounts, and
/// `Liveness:` / `Readiness:` / `Startup:` per container out of `describeContainers`. That is
/// what rules 1–8 claim, checked per card (NOTES § D71).
///
/// **`Command:` and `Args:` are in that list too**, which is what backs the one action
/// [`failed_run_action`] hands out on the arm where the run never started — that container has no
/// log at all, so [`previous_logs`] would be a command with nothing behind it (NOTES § D113).
///
/// **It is also what rules 1 and 5 word their endings against** (NOTES § D85, § D88): a clean or
/// polite ending is told apart from a kill by the `Killing` event and the probe lines, and the
/// node a killer outside Kubernetes would have run on is the `Node:` line here — all three are in
/// this output and none of them is in `get -o yaml`. Rule 1's `exit
/// 0` branch took `get -o yaml` until 2026-08-14 so that it could name `restartPolicy` — which
/// cost it the events that are the only thing able to correct the card, for a field its own
/// state already implies (NOTES § D88). **No card in this file names `restartPolicy` any more.**
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

/// `kubectl get <resource> … -o yaml` — for the cards whose evidence is a field `describe` does
/// not print at all: rule 12's `metadata.finalizers`, rules 3 and 4's `state.waiting.message`,
/// which kubectl's `describeStatus` never renders, and **the W-series'
/// condition messages**, which `describeReplicaSet` and `describeDeployment` both reduce to a
/// `Type / Status / Reason` table. A teaching command that does not show what the card says is
/// worse than none (invariant 4, NOTES § D46, § D71).
///
/// **The resource word is the caller's** because it is the one part that is not derivable here:
/// `ObjectKind` is a kind and `kubectl` takes a resource, and mapping between them is API
/// discovery's job, not a table in this file (invariant 12).
fn get_yaml(resource: &str, id: &ObjectId) -> Option<String> {
    Some(format!(
        "kubectl get {resource} {}{} -o yaml",
        id.name,
        in_namespace(id)
    ))
}

/// `kubectl logs <pod> -c <container>` — **what the container itself said, which no other command
/// in this file shows** ([`stopped_for_good`] is its first caller, and [`previous_logs`] is the
/// one variant of it).
///
/// **No `--previous`, and it is *which run* that decides it rather than whether the log is
/// reachable** (invariant 4, NOTES § D96). The flag serves the run *before* the current one, and
/// every other ending in this file is read out of `lastState`; rule 15's container is stopped in
/// the run it is sitting in **now**, so `--previous` would send this reader to a run that never
/// happened. Measured on kind v1.36.1 with the node healthy: the bare command returned the
/// container's own last line, no flag and no error.
///
/// **That measurement is a happy path and the paragraph it used to end justified more than it
/// showed** (NOTES § D97) — see the last paragraph here, which is the correction and is why the
/// card's action no longer promises the log is there. The `--previous` argument is untouched by
/// it: a run that never happened has no log under any flag.
///
/// **`-c` is always written, never only for a pod with more than one container.** The flag is
/// harmless on a single-container pod, and the card names one container out of however many the
/// reader's pod has — a command that made them guess which is the record invariant 4 says may not
/// lie.
///
/// **No `--tail`, and the reason is what the command log is** (invariant 4, NOTES § D97). This
/// line is the *equivalent command the user would have typed*, and `kubectl logs <pod> -c <name>`
/// is exactly that; a flag we picked makes the line ours rather than theirs, and teaches a
/// default nobody chose. A reader who wants less pipes it. Asked by the operator review and
/// answered here so it is answered rather than re-asked.
///
/// **What it cannot promise is that the log is there.** This is the only command in this file
/// that goes to the **kubelet on the node** rather than to the API server, so a node that has
/// stopped answering returns `connection refused` while the pod status the rule read sits frozen
/// and unchanged. The command is still the right one — it is where the answer is when there is
/// one — but [`stopped_for_good`]'s action may not say the log is still there, and does not.
fn logs(id: &ObjectId, container: &str) -> Option<String> {
    Some(format!(
        "kubectl logs {} -c {container}{}",
        id.name,
        in_namespace(id)
    ))
}

/// `kubectl logs <pod> -c <container> -n <ns> --previous` — **the same command with the flag
/// [`logs`] refuses**, for the one card whose subject is a run that is already over
/// ([`previous_run_failed`]'s general [`Failed`](Ending::Failed) arm, NOTES § D113).
///
/// **Two functions and not a parameter**, because the flag is a *claim* rather than an option:
/// [`logs`]' doc argues at length that rule 15's container is stopped in the run it is sitting in
/// now, so `--previous` there points at a run that never happened. A caller appending the flag
/// behind that doc is how the argument stops being true without the doc changing.
///
/// **Why it is servable here.** The kubelet gates the flag on `lastState.terminated.containerID`,
/// and this ending is the one whose record carries it — which is exactly what
/// [`Ending::Unwatched`] does *not*, and why that arm keeps [`describe`]. `describe` printing no
/// logs at all was the defect: the sentence was right and the command under it was not
/// (invariant 4).
///
/// **It is always the run the card is about, and the first draft said otherwise** (NOTES § D113).
/// *`lastState` freezes, so the flag serves a later run* was written from D112 and is not what
/// D112 says: `kubelet_pods.go:2616` gates the **synthesized** write, so only a container's first
/// *lost* status is frozen — that is [`Ending::Unwatched`], which carries no `containerID` and
/// keeps [`describe`] anyway. Measured across ordinary restarts, the record advances every time
/// (`reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md` § 1): `startedAt`
/// `05:33:16` → `05:33:53` between `restartCount` 2 and 3, with the `containerID` moving with it.
/// It cannot diverge even in principle — the kubelet resolves `--previous` **through**
/// `lastState.terminated.containerID` (`validateContainerLogStatus`), which is the record this
/// rule read.
///
/// **What it cannot promise is that the log is still on the node, and that window is real.** The
/// kubelet keeps **one** dead container per container name
/// (`--maximum-dead-containers-per-container`, default 1, unset on kind), so the moment the
/// current container exits it becomes the newest dead one and the container `lastState` names is
/// collected: measured `unable to retrieve container logs for containerd://…` for the whole
/// window in which the container is `terminated` rather than running — about 29 s between early
/// restarts and minutes later on. **[`doing_its_job`] is false in exactly that window, so
/// [`previous_run_failed`] draws on it.** In the settled `CrashLoopBackOff` back-off the flag
/// works, 15 of 15 samples. This is [`logs`]' own warning one notch worse, and it is why the
/// action names the log as the place the answer was written rather than promising to hand it over.
///
/// **In `waiting` the flag is redundant rather than wrong**: the kubelet resolves a waiting
/// container's plain `kubectl logs` through the same field, and both returned byte-identical
/// output. The flag is still written, because the card's subject is a run that is over and the
/// command log is what the reader would have typed (invariant 4).
///
/// The other half of [`logs`]' warning applies unchanged: this is the only command in this file
/// that goes to the kubelet, so a node that has stopped answering returns `connection refused`.
fn previous_logs(id: &ObjectId, container: &str) -> Option<String> {
    Some(format!("{} --previous", logs(id, container)?))
}

/// One `status.conditions[]` entry, by type — **and the reason every caller looks its own one
/// up**: the list is flat, so `Ready`'s stamp sits three lines from `DiskPressure`'s and a
/// Deployment's `Available` sits beside its `Progressing`, and a card dated or reasoned from the
/// neighbour is what taking the first one produces (NOTES § D69).
fn condition<'a>(conditions: &'a [Condition], type_: &str) -> Option<&'a Condition> {
    conditions.iter().find(|c| c.type_ == type_)
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

/// **Is this pod over?** — `Succeeded` or `Failed`, whose restart counts and last exits are not
/// what is broken *now* (NOTES § D2, § D71), and whose requests belong **to nobody's node**.
/// Asked by [`analyze`] before the pod rules and by every node rule that joins pods to a node: a
/// `Succeeded` Job pod keeps its `nodeName` for as long as nobody collects it
/// ([`PodSnapshot::phase`]).
///
/// **The two halves have different fates and the sentence used to blur them** (NOTES § D96). The
/// node half is a live claim — N5 must not charge a machine for a pod that has stopped running,
/// and it does not. The Alerts half named the **Waste** report as where those counts go, and that
/// report does not exist and would not hold them: its charter is Evicted/Completed *pileups*, not
/// a per-pod diagnosis. What is true is that the pod leaves this screen and reaches no other.
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
/// [`Sidecar`](ContainerRole::Sidecar); **[`Ending::Finished`] for an
/// [`Init`](ContainerRole::Init)**, because "serving" means nothing about a container that runs
/// once and finishes and the other expression answers *no* for every init container that ever
/// succeeded (NOTES § D75). **A failed init container is deliberately not settled by this.**
///
/// **It asks [`ending`] rather than spelling `exit_code == 0` again**: the two agreed by hand
/// until 2026-08-14 and nothing held them together, which is the shape of the defect NOTES § D85
/// exists to close.
///
/// **And it answers every variant by name rather than comparing against one**, because the whole
/// mechanism NOTES § D95 rests on is that adding an [`Ending`] stops the file compiling until each
/// reader says what the new ending means. `== Ending::Finished` opted out of that: the two `137`
/// readings were classified here silently when they were added, and the answer being the right
/// one was luck rather than a decision. A match with named arms is what makes the claim true of
/// this reader too.
///
/// **The init branch is captured**: `healthy-retry.json` is an init container that failed three
/// times before it exited `0`, so both rules have something to suppress on a real object.
fn doing_its_job(c: &ContainerSnapshot) -> bool {
    match (&c.state, c.role) {
        (ContainerState::Running { .. }, _) => c.ready,
        (ContainerState::Terminated(run), ContainerRole::Init) => match ending(run) {
            Ending::Finished => true,
            // A run that failed, one nothing watched end, one the pod's own rule removed and one
            // nobody read the ending of are all *not finished* — but each is spelled out so that
            // the next ending added has to be answered here as well (NOTES § D95). The last of
            // them is the one worth saying out loud: with no code anybody read, *it ended well*
            // is the single thing this reader may not conclude.
            Ending::Stopped
            | Ending::Failed
            | Ending::Unwatched
            | Ending::RestartRule
            | Ending::CodeUnknown => false,
        },
        _ => false,
    }
}

/// **How the run before this one ended, in the shapes the rules have to tell apart** —
/// and **the one place a rule decides what an exit code means**; [`exit_meaning`]
/// translates the codes for the reader, and nothing else in this file branches on them
/// (NOTES § D85). [`crash_looping`] picks which loop it is looking at from this,
/// [`restarting_repeatedly`] picks what it may claim about the restarts, and
/// [`previous_run_failed`] takes its exemption list, its title and its action from it;
/// [`doing_its_job`] asks only whether it is [`Finished`](Ending::Finished).
///
/// `exit 0` and `exit 143` were spelled out in rule 6 alone until 2026-08-14, so rule 1 called a
/// batch job that finished a crash — two rules reading one container and disagreeing one card
/// line apart.
///
/// **A fourth and a fifth variant joined them, and that is what makes this an ending rather than a
/// code** (NOTES § D95). The two `137` reasons the kubelet writes itself arrived on 2026-08-15,
/// and [`CodeUnknown`](Ending::CodeUnknown) — `255` beside [`CODE_UNKNOWN`], which is what a node
/// restart leaves behind — on 2026-08-16, for the same reason each time: three rules read this
/// object and a `reason` check inside one of them leaves the other two silently wrong. They were read here as
/// [`Failed`](Ending::Failed) while [`exit_meaning`] two functions later told the reader the
/// number meant something else, so rules 1 and 5 printed *keeps crashing* and *something keeps
/// killing it* over a translation denying both. **Adding them here is what forces the answer**:
/// every `match` on this enum stops compiling until it says what the two mean, which a `reason`
/// check inside one rule would not have done.
///
/// **`OOMKilled` is deliberately not a variant.** Rule 2 owns the labelled kill and draws its own
/// card; *something keeps killing it* is true of it, so rules 1 and 5 need no arm and rule 6
/// exempts it by reason (NOTES § D71, § D95).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ending {
    /// `exit 0` — the program did what it was told to do and stopped.
    Finished,
    /// `exit 143` — 128 + SIGTERM: something asked the container to stop and it did.
    Stopped,
    /// **A run Kubernetes never watched end** — `137` beside [`STATUS_LOST`], the number the
    /// kubelet writes where a status went missing. Nothing is known to have ended this run, so
    /// no card may name a killer, and there is no `logs --previous` to send anyone to: the
    /// kubelet gates that flag on the `containerID` this record does not carry.
    Unwatched,
    /// **The pod's own restart rule removed the container** — `137` beside [`RESTART_ALL`].
    /// Nothing failed here that this record names: a container exited, the `restartPolicyRules`
    /// **declared on that container** said restart them all, and the kubelet did. The *effect*
    /// is pod-wide and the declaration is not — see [`RESTART_ALL`], which carries where the
    /// field actually lives (NOTES § D96).
    RestartRule,
    /// **The container was found dead and nobody read how it ended** — `255` beside
    /// [`CODE_UNKNOWN`], which is what a node restart leaves behind. Unlike
    /// [`Unwatched`](Ending::Unwatched) the record is a real one: containerd *found* the
    /// containers, so the stamps and the `containerID` are there and `logs --previous` works.
    /// What is missing is only the code, and the `255` standing in its place is not the
    /// application's.
    CodeUnknown,
    /// Everything else, a code with no accepted meaning included.
    Failed,
}

/// **Each key is exactly as wide as it takes to identify the ending, and no wider.** Three shapes
/// of key sit below and they are one rule, not a rule and its exceptions:
///
/// - **the code alone, where the number means one thing wherever it comes from** — `0` and `143`,
///   which every runtime and every application spell the same way;
/// - **the code alone, where no application could have written it** — `-1`, outside the POSIX
///   range;
/// - **the code beside the reason its writer pairs it with**, where the number *is* ambiguous and
///   the pair is what the writer made unambiguous — the two `137` readings this function keys,
///   and `255`. (`137`'s third and fourth readings are [`exit_meaning`]'s; they are the same
///   ending here.)
///
/// **A reason is never read on its own**, in any of the three: a reason with no code beside it is
/// a string one runtime happens to use and another spells differently.
///
/// **The pairs.** [`STATUS_LOST`] and [`RESTART_ALL`] are synthesized with `137` and arrive no
/// other way (`kubelet_pods.go` at v1.36.1, and `failed.json` is the captured pair);
/// [`CODE_UNKNOWN`] arrives with `255` from **containerd** rather than from the kubelet at all. So
/// a reason beside any other code is a pair nothing produces, and it falls through to the ordinary
/// reading of the number on purpose. Rule 6's guard read the reason alone until 2026-08-15, which
/// is why three unreachable pairs move here: [`STATUS_LOST`] beside `1` takes the ordinary *the
/// last run on record failed* title, and [`RESTART_ALL`] beside `1` or `5` draws a card instead of
/// being silent. **Nothing the API can produce is affected, and no test asserts otherwise**:
/// pinning behaviour to an object no cluster writes is what NOTES § D29 and § D95 refuse.
///
/// **`-1`: the code no application can write.** CRI-O writes it where it could not read an exit
/// status ([`CODE_UNKNOWN`]), and `-1` is outside the POSIX range `0..=255`: no process can report it, so
/// the number is already unambiguous and a reason beside it would narrow nothing. Its reason is
/// `"Error"`, which is what an ordinary application failure carries, so keying the pair there
/// would key on the one field that says nothing.
///
/// **`255` is keyed on the pair for a reason the box that opened it denied.** It asked for a row
/// on the number alone; a program that runs `exit -1` in a shell reports `255` too, and a
/// code-alone row would tell that reader their program did not fail. What the pair costs is a
/// runtime that spells its unknown differently falling through to the bare number — a miss, never
/// a lie.
///
/// **The premise is about these two reasons and not about exit codes in general**, because the
/// general claim is false and this file cites the counter-example: `kubelet_pods.go:2705-2723`
/// synthesizes `Terminated { reason: "Completed", exitCode: 0 }` for an init container whose
/// status the runtime lost, so a *real-looking* code does **not** prove the run was watched.
///
/// **That one is silence on purpose, and the source is what decided it** (NOTES § D112). It was
/// recorded here as a live gap until 2026-08-16, on the reading that the `reason` could key a
/// third ending; the literal's reason is `Completed`, which is byte for byte what a watched finish
/// writes, so the reason separates no object the API can produce. What the kubelet is doing there
/// is **deducing rather than guessing**: the write is gated on `HasAnyRegularContainerCreated`,
/// and the regular containers are started only once every non-restartable init container has
/// succeeded. So [`Finished`](Ending::Finished) is the true reading, [`doing_its_job`] answering
/// *yes* is correct, and a card would be a permanent WARN on every static pod in every cluster
/// after a kubelet restart — the class the source's own comment names first.
///
/// **It is written into `state.terminated`, and it can still reach `lastState` from there.** That
/// was recorded here as *rules 1, 5 and 6 never reach it*, and at v1.36.1 that is false:
/// `convertContainerStatus` (`kubelet_pods.go:2294-2306`), under the
/// `RestartAllContainersOnContainerExits` gate this file already records as beta-on-by-default
/// ([`RESTART_ALL`]), copies `oldStatus.State.Terminated` into `lastState` when the containerID
/// changes — and the literal carries an *empty* one, so a recreated init container satisfies the
/// comparison. The verdict does not move, because [`Finished`](Ending::Finished) is the true
/// reading wherever the record sits; what moves is that the silence rests on the reading and not
/// on the field being out of reach.
///
/// **And the gate is weaker than the deduction it licenses**, which is worth writing down rather
/// than boxing: `HasAnyRegularContainerCreated` counts a regular container in `Exited` as created,
/// while `computeInitContainerActions` computes its own `podHasInitialized` from the same list and
/// deliberately excludes `Exited` — *"If the node is rebooted, all containers will be in the
/// exited state … the kubelet should not mistakenly think that the newly created podSandbox has
/// been initialized"*. So the status path can write this literal during a sandbox rebuild about an
/// init container that has not run yet in the new sandbox. It is still true of the past — that
/// init container did succeed once — and silence is still right; the doc simply may not read
/// tighter than the gate is.
///
/// The one thing it shares with the two reasons above is the missing stamp, and that is where its
/// one consequence is handled: [`last_log_line`] refuses the kubelet's sentence riding it. Pinned
/// by `a_lost_init_container_status_reads_as_finished_and_that_is_the_true_reading`.
fn ending(run: &Terminated) -> Ending {
    match (run.exit_code, run.reason.as_deref()) {
        (0, _) => Ending::Finished,
        (143, _) => Ending::Stopped,
        (137, Some(STATUS_LOST)) => Ending::Unwatched,
        (137, Some(RESTART_ALL)) => Ending::RestartRule,
        (255, Some(CODE_UNKNOWN)) => Ending::CodeUnknown,
        // **CRI-O's spelling of the same event, and it needs no reason beside it.** `-1` is not a
        // POSIX exit status — those are `0..=255` — so no process can report it and nothing else
        // writes it; the objection that made `255` a pair (`exit -1` in a shell reports `255`)
        // does not apply. See [`CODE_UNKNOWN`] for the provenance of both.
        (-1, _) => Ending::CodeUnknown,
        _ => Ending::Failed,
    }
}

/// **The runtime's own word for a container it found dead without an exit status, and the reason
/// `255` is read beside rather than alone.** It is **containerd's** and not the kubelet's:
/// `unknownContainerStatus()` in containerd's CRI plugin is `{ExitCode: 255, Reason: "Unknown"}`,
/// and `kuberuntime_container.go:760-763` copies `Reason`, `Message`, `ExitCode` and `FinishedAt`
/// straight out of the CRI status. So another runtime spelling it differently falls through to
/// the bare number — **a miss, never a lie** — and that is the whole cost of keying the pair.
///
/// **The pair, because the number alone means nothing** (NOTES § D112). A program that runs
/// `exit -1` in a shell reports `255`, and a row keyed on the code would tell that reader their
/// program did not fail.
///
/// **A node restart is the producer measured on kind v1.36.1**: containerd's state survives the
/// reboot, the containers are *found*, dead, and this is the pair it reports for them. The
/// kubelet only copies it through — which is why the record carries real stamps and a real
/// `containerID`, the thing that separates it from [`STATUS_LOST`] and the reason
/// `logs --previous` works here. It is the commonest abnormal `lastState` a cluster produces, and
/// every card about it read as the application's own failure until 2026-08-16.
///
/// **CRI-O writes `-1` / `"Error"` for the same event, and [`ending`] keys that one on the code
/// alone.** `server/container_status.go:107-130` at `cri-o/cri-o` `main`: where the exit code
/// cannot be determined it reports `ExitCode: -1` with `errorReason` and the runtime's own error
/// string in `Message`. **The reason cannot be the key there** — `"Error"` is what an ordinary
/// application failure carries — but the *code* can, because `-1` is outside the POSIX range
/// `0..=255` and no process can report it. **A source-derived pin and not a captured one**: no
/// CRI-O node exists on any host this repository builds on, and no fixture is invented for it
/// (NOTES § D29, § D40) — the same footing [`RESTART_ALL`] stands on.
const CODE_UNKNOWN: &str = "Unknown";

/// **The kubelet's own `reason` for a run it never watched end, and the third meaning of `137`**
/// (NOTES § D90, § D93). `convertToAPIContainerStatuses` writes
/// `Terminated { reason: ContainerStatusUnknown, exitCode: 137 }` in two places, both of them a
/// status it could not read rather than a kill it saw: where the runtime reports the container
/// `Unknown` while the previous status said `Running`, and where the container has gone from the
/// runtime's list altogether. The comment beside the number in that file is
/// `// this code indicates an error` — it is a placeholder, not a signal anything sent, which is
/// why it gets a row of its own rather than the SIGKILL sentence.
///
/// **The whole object is those three fields and nothing else**: `startedAt`, `finishedAt` and
/// `containerID` are all absent — the struct literal at `kubelet_pods.go:2621-2625` sets `Reason`,
/// `Message` and `ExitCode` and stops, and an unset `metav1.Time` marshals to `null`
/// (`apimachinery/pkg/apis/meta/v1/time.go:162`) — because the kubelet is describing a run it did
/// not see. So a card of this class has no age and no *ran for*
/// ([`lasted`], [`Finding::timestamp`]) — measured
/// on kind v1.36.1, where `crictl rmp -f` on the sandbox is the producer and a node reboot is
/// **not**: containerd's state survives that, the containers are found dead, and the kubelet
/// writes `exit 255` / [`CODE_UNKNOWN`] instead, which is [`Ending::CodeUnknown`].
const STATUS_LOST: &str = "ContainerStatusUnknown";

/// **The fourth meaning of `137`, and the only one that is the pod getting what it asked for.**
/// `RestartAllContainersOnContainerExits` is `{Version: 1.36, Default: true, Beta}` — on by
/// default at the version `tests/fixtures/K8S_VERSION` pins — and when a container exits into a
/// matching rule **whose action is `RestartAllContainers`**, the kubelet removes the other
/// containers to restart them together, writing
/// `Terminated { reason: RestartingAllContainers, exitCode: 137 }` with the same three fields and
/// no more. Verified in `kube_features.go` and `kubelet_pods.go` at v1.36.1, not taken on report
/// (NOTES § D93).
///
/// **A matching rule is not enough — the *action* is what makes it a gang restart, and the
/// published schema hides that** (NOTES § D97). `kubectl explain` and the OpenAPI document both
/// say the only action is `Restart`; the **validator** accepts two, and they behave differently:
/// measured on kind v1.36.1, `action: Restart` restarted the failing container five times and
/// never touched its sibling, while `action: RestartAllContainers` moved them in lockstep. So
/// this reason is written by one action out of two, which makes [`Ending::RestartRule`] narrower
/// than *a container had restart rules* — better founded, not weaker.
///
/// **The rules are declared on a *container*, and only the effect is pod-wide.** There is no
/// `pod.spec.restartPolicyRules` at v1.36.1 — `kubectl explain` answers that the field does not
/// exist — it is `spec.containers[].restartPolicyRules`. That is why
/// [`restart_rule_action`] sends the reader to *the container whose spec declares the rule*
/// rather than to the pod, and it is the field the boxed rule that would name the trigger has to
/// look at. **Confirmed against a live v1.36.1 cluster** (NOTES § D96), which carries the rest of
/// the corrected restart-policy table with it rather than repeating it here.
///
/// **"The other containers" is what the kubelet *does*, and not who ends up carrying the
/// record.** The second operator review measured the record landing in **every** container's
/// `lastState`, the one whose own exit triggered the rule included — its own bad exit is in
/// `state.terminated`, which no rule reads. So no card drawn off this reason may tell the reader
/// that *this* container is the innocent one ([`restart_rule_action`], NOTES § D93, § D95).
const RESTART_ALL: &str = "RestartingAllContainers";

/// **What an exit code means, in the words a beginner needs** — NOTES § v1 rule set's
/// translation table, and nothing invented beside it. `None` is a code with no accepted meaning,
/// where the number alone is the honest answer.
///
/// **`0` and `143` are the two entries that say *nothing failed***, which is why
/// [`previous_run_failed`] refuses to fire on either. They stay here because rule 1 does print
/// them, and printing `exit 0` bare under a card about crashing is what NOTES § D85 is
/// about — a translation missing is a contradiction shipped.
///
/// **`0`'s row names the ending and not an agent**, because a program that traps `SIGTERM` and
/// shuts down tidily reports it too: *finished successfully* read as *the program chose to stop*
/// and stood one line above [`finished_action`], whose whole subject is that the code cannot say
/// who ended the run (NOTES § D85, § D88).
///
/// **`255` needs its `reason` too, and for the same money.** With [`CODE_UNKNOWN`] beside it the
/// container was found dead with no exit status — a node restart is the producer measured on kind
/// v1.36.1 — and the number is a stand-in the runtime wrote, not the application's. **Bare `255`
/// stays untranslated on purpose**: `exit -1` in a shell reports it, and a code-alone row would
/// tell that reader their program did not fail.
///
/// **137 needs the `reason` beside it, and it has four readings rather than two** (NOTES § D71,
/// § D90, § D93): with [`Terminated::reason`] `OOMKilled` the kernel took the container for using
/// too much memory; with [`STATUS_LOST`] nothing was killed that Kubernetes watched, and the
/// number is what it wrote where a status went missing; with [`RESTART_ALL`] the pod's own
/// restart rule removed it on purpose; **with anything else the row names the signal and stops
/// there.**
///
/// **That last row named a cause until 2026-08-15, and the object supports none** — *did not stop
/// when it was asked to — a failing liveness probe, or a shutdown that hangs* is three claims
/// deep. An init container may hold no probe at all (`validateInitContainers`, [`killed_action`]);
/// a genuine cgroup kill arrives without the word on a host short of memory (NOTES § D84); and a
/// rebuilt sandbox kills a container nothing asked to stop (NOTES § D90). **Who sent the signal
/// is the action's question, because only the action knows the role** — a translation printed by
/// rules 1, 5 and 6 alike cannot answer it three ways (NOTES § D88, § D93).
fn exit_meaning(code: i32, reason: Option<&str>) -> Option<&'static str> {
    Some(match code {
        0 => "the run ended without an error",
        137 if reason == Some("OOMKilled") => {
            "killed by the kernel for using more memory than it was allowed"
        }
        137 if reason == Some(STATUS_LOST) => {
            "Kubernetes lost track of the container and wrote this code in its place"
        }
        137 if reason == Some(RESTART_ALL) => {
            "removed so Kubernetes could restart every container in the pod, which is what this \
             pod asked for"
        }
        137 => {
            "killed with SIGKILL — a stop the program cannot refuse, and the code does not say \
             what sent it"
        }
        143 => "stopped with SIGTERM, which is an ordinary shutdown and not an error",
        // **One word shorter than it was on 2026-08-16, and the word was load-bearing for the
        // layout rather than for the meaning.** [`previous_run_failed`] spends its whole title on
        // this translation, and with *already* in it the title wrapped to **four** lines at the 51
        // columns a card's body has — the only title in the file over `screens/alerts.md`'s new
        // three-line cap, which that file states for the first time (NOTES § D113). The clause it
        // is in still says the node arrived after the fact; what went is the emphasis.
        255 if reason == Some(CODE_UNKNOWN) => {
            "the node found the container dead, so this number stands in for a code nobody read"
        }
        // **A different sentence for the same ending, because the runtimes tell different
        // stories.** CRI-O does not say it found the container dead; it says it could not work out
        // what the container ended with ([`CODE_UNKNOWN`]). One ending, two translations, and the
        // translation is where the difference belongs.
        -1 => {
            "the node could not tell what code the container ended with, so this number stands in"
        }
        1 | 2 => "the application's own error",
        126 => "the command was found but could not be run",
        127 => "the command was not found",
        // **Hedged, because this one code has two authors and the object does not say which**
        // (NOTES § D113). It is overwhelmingly the runtime failing to start the container —
        // containerd labels it `reason: StartError`, CRI-O writes a bare `Error` for the same
        // event, so the pair the `255` row is keyed on cannot be keyed here — and a program is
        // still free to call `exit(128)` itself. *Usually* is the honest word: it connects the
        // number to [`failed_run_action`]'s advice one row down without asserting a cause
        // the record cannot carry, and printing the number bare beside that advice was the jargon
        // left unexplained (invariant 14).
        128 => "usually a container the runtime could not start",
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
/// **`Kubernetes recorded this: …`** — the one wording for [`Terminated::message`], because two
/// rules print it, and since 2026-08-16 **both put it in the same place**: a fact on the evidence
/// line, ahead of the duration, on [`previous_run_failed`]'s `Failed` arm and on
/// [`stopped_for_good`] (NOTES § D113). It was rule 6's whole *action* until then, which is one
/// fact in two slots as well as two rules — and it is the slot that mattered: an action is k8rs's
/// own words, and a card built out of this field cannot be measured against a five-line budget
/// because no rule author bounds what a runtime writes. Two spellings of one fact on one screen is
/// where NOTES § D85 starts; two *slots* is the same defect with the length attached.
///
/// **It says who *recorded* the line and never who wrote it, because the object cannot tell.** It
/// read *the last thing it logged was: …* until 2026-08-16, and three different authors reach this
/// field: the container (`terminationMessagePath`, or the log tail under
/// `terminationMessagePolicy: FallbackToLogsOnError`), the kubelet (its four synthesized
/// literals), and **the runtime**. The last of those is what broke the claim —
/// [`last_log_line`]'s stamp guard tells the kubelet's placeholders from a container's words and
/// cannot see the runtime at all: containerd writes its start-failure error into `Message` beside
/// a real `FinishedAt` (`internal/cri/server/container_start.go:67-73`), and CRI-O does the same
/// for a stopped container whose exit code it could not read
/// (`server/container_status.go:107-130`). Measured on kind v1.36.1, the old frame printed
/// *the last thing it logged was: failed to create containerd task: …* about a container that
/// logged nothing. **No `reason` separates them either** — CRI-O's is `"Error"`, which is what an
/// ordinary application failure carries — so box 966's own done-when decides it: the sentence
/// stops claiming authorship rather than guessing it.
///
/// **It still frames.** The constant prefix is what stops a crafted message equalling a static
/// action and deleting a card through [`one_card_per_action`] (invariant 9), and it names an owner
/// rather than leaving a bare noun for the reader to attach (invariant 14).
fn last_words(line: &str) -> String {
    format!("Kubernetes recorded this: {line}")
}

/// **`None` on a record the kubelet synthesized, because its message is a placeholder rather than
/// anything that happened** (NOTES § D88, § D93). Each of the four literals in `kubelet_pods.go`
/// at v1.36.1 — `:2385` and `:2624` ([`STATUS_LOST`]), `:2584` ([`RESTART_ALL`]) and `:2717`, the
/// init container whose status the runtime lost — writes `Reason`, `Message` and `ExitCode` and
/// stops, because the kubelet is describing a run it never watched. Anything arriving through a
/// CRI status carries `FinishedAt` beside its `Message`, filled in the same block
/// (`kuberuntime_container.go:760-763`).
///
/// **What the stamp separates is the kubelet's own literals from everything a CRI status
/// carries — and that is not the same as separating authors.** [`last_words`] records the three
/// that reach this field; the runtime is one of them, and its errors ride a stamped record, so
/// they pass this guard by design and the *frame* is what stopped claiming an author. Reading
/// this guard as *who wrote it* is the mistake this whole area exists to stop, so it is named
/// here rather than left to be inferred from what it happens to filter.
///
/// **The two known instances were held shut by two accidents and neither was the fix** — one by
/// arm order inside [`previous_run_failed`], one by an exemption granted for an unrelated reason —
/// so a fifth literal would have printed a placeholder where a rule's own advice belongs, with
/// nothing in its way.
///
/// **It fails towards a miss and never towards a card that says nothing.** A real message on a
/// record with no `finishedAt` loses a line the reader could have had; the alternative is a card
/// whose whole *what to do* is the kubelet telling itself it could not find something.
fn last_log_line(run: &Terminated) -> Option<&str> {
    run.finished_at.as_ref()?;
    run.message
        .as_deref()?
        .lines()
        .map(str::trim_end)
        .rfind(|l| !l.is_empty())
}

/// **What to do about an [`Ending::Finished`], per role — one reading of `exit 0` for the whole
/// file.** Rules 1 and 5 both reach it, for [`stopped_action`]'s reason (NOTES § D85, § D88).
///
/// **A clean exit says how the run ended and never who ended it.** A program that traps `SIGTERM`
/// and shuts down tidily reports `0`, and the kubelet writes `0` / `Completed` whether the
/// program chose to stop or something outside asked it to. So neither the
/// [`Regular`](ContainerRole::Regular) nor the [`Sidecar`](ContainerRole::Sidecar) sentence picks
/// between *it stopped itself* and *something stopped it*: both send the reader to the two places
/// that tell those apart, and **that is what puts both rules on [`describe`]** — an action may
/// only name what its own command prints (invariant 4), and `get -o yaml` carries no events at
/// all.
///
/// **Door 1 is the killer, not the probe** (NOTES § D88). Every kubelet-initiated stop goes
/// through `killContainer`, which records a `Killing` event whatever asked for it — a liveness or
/// startup probe, an eviction, an in-place resize with `resizePolicy: RestartContainer` — so
/// `Killing` is the line that proves a kill happened and the one the card names. **`Unhealthy` is
/// not**, because a failing *readiness* probe writes it with no kill behind it, and a reader who
/// greps the word the card gave them would close this door on the wrong evidence. **The node is
/// named beside it** for the killer that writes no event at all: `earlyoom` sends the same signal
/// from outside Kubernetes, and the program's own handler decides whether that arrives here as
/// `143` ([`stopped_action`]) or as `0`. **`systemd-oomd` was named here too until 2026-08-16 and
/// cannot reach either card**: it kills a cgroup with `cgroup.kill`, which is SIGKILL and arrives
/// as `137` (NOTES § D113).
///
/// **The events clause carries their lifetime on all three arms now, and rule 5 is what it is
/// for** (NOTES § D113). `--event-ttl` defaults to an hour; the [`Init`](ContainerRole::Init) arm
/// has said so since D88 and the other two sent the reader to the same events with no window at
/// all. On [`crash_looping`] that costs nothing — backoff caps at five minutes, so a `Killing`
/// line is always minutes old — but [`restarting_repeatedly`] draws about a container that is
/// *serving* at ten restarts, whose last run may have ended hours ago. There `Events: <none>`
/// reads to a beginner as *nothing stopped it*, which is the reading that walks them into the Job
/// door for a Deployment that is fine.
///
/// **And the doors are ordered by how long the run lasted, which is the first time any rule in
/// this file orders its own action** (NOTES § D113). Three things hold it up. **It reorders and
/// never deletes**: a short run does not prove no probe fired, so every door open on the long arm
/// is open on the short one. **The fact it orders by is already on the card** — `ran_for` is on
/// the evidence line one row above, so the reader can see why the order is what it is, and the
/// parameter is `Option` precisely so a caller whose card does *not* print a duration passes
/// `None` and gets the unordered sentence: [`restarting_repeatedly`]'s evidence is the container,
/// the exit code and the image, so it does. **And the threshold is derived** — [`PROBE_FLOOR`]
/// carries the arithmetic.
///
/// **The short arms drop the events' lifetime and only they may**, because only [`crash_looping`]
/// passes a duration, and its container is in `CrashLoopBackOff` — where the backoff caps at five
/// minutes and the events are always fresh. That is the same reasoning that made the window
/// harmless on rule 1 in the first place, applied to the arm rule 1 alone can reach.
///
/// **On [`Regular`](ContainerRole::Regular) that leaves three branches and not two**, because
/// *it stopped itself* is itself two: something stopped it, which the events and the node settle;
/// it stopped itself and is meant to, which a Job or a CronJob is built for; or it stopped itself
/// and is not meant to, which is quitting early and a bug in the program. **The third is what the
/// fix for the missing first one deleted** — and it is not a corner: an `nginx` with the stock
/// entrypoint and no `daemon off;`, a `sh -c './server &'`, a Java `main` that returns while its
/// daemon threads die, all exit `0` in under a second under a policy that restarts them, and all
/// arrive on the shape `exit0.json` holds. A pair missing that branch offers every one of them a
/// CronJob (NOTES § D88).
///
/// **The first of those doors leads with a subject and not an ellipsis** (invariant 14): *meant
/// to, it belongs in a Job* parses, and reads as a telegram to the person this card is written
/// for. *If not* one clause later is the ordinary kind, because what it drops — *meant* — was
/// written out immediately before it. The room for *if that is meant* came out of the clause
/// above them, *the program ends itself* → *it ends itself*, and not out of a door.
///
/// **[`Init`](ContainerRole::Init) is asked a different question rather than a softer version of
/// the same one**: a plain init container is *meant* to finish, so nothing about the ending needs
/// explaining and what does is what ran it again — an answer outside the container either way. It
/// therefore claims nothing about who ended the run, and it names no probe, because
/// `validateInitContainers` forbids all three on an init container that is not restartable
/// ([`stopped_action`]).
///
/// **Its events clause is hedged, because the commonest rebuild writes no event at all**
/// (measured on kind v1.36.1). `SyncPod` records `SandboxChanged` only where it finds a sandbox
/// that *changed* — `podContainerChanges.SandboxID` non-empty; where the sandbox is simply gone,
/// which is what a node reboot, a runtime restart or `crictl rmp` leaves behind, the kubelet runs
/// every init container again while logging at V(4) and emitting nothing. So the card says the
/// events *often do not say why* rather than that they say so, and the node follows them with no
/// *after that* in front of it: it is the answer for the reader who is late **and** for the one
/// whose events never carried the reason. **The second *again* paid for the hedge**, and the
/// question one clause earlier — *ask what ran it again* — is what keeps *runs them all* a
/// re-run rather than a first one.
///
/// **The hour names the cluster and not this tool** (invariant 14, NOTES § D88): `--event-ttl`
/// defaults to an hour, and a card that dated itself against its own screen furniture would say
/// nothing the reader can check — on rule 1's card the restart count it used to name can still
/// be `0`.
///
/// **All three arms are five wrapped lines or fewer at `screens/alerts.md`'s 49 columns**, which
/// that file makes a `rules.rs` requirement rather than a layout preference, and
/// `the_clean_exit_actions_fit_the_card_they_are_drawn_on` holds them there. What came out to
/// reach it was the preamble and the restatements, never a door: three readings that cannot be
/// told apart from the object are what the card is *for*. **The measure breaks on spaces, and on
/// characters only where a token is wider than the line** — the conservative reading in both
/// directions: the `re-runs` the [`Init`](ContainerRole::Init) arm was first rewritten with put it
/// at five lines under a wrapper that splits at a hyphen and six under one that does not, and a
/// budget met only by the generous counter is not met; while a token with no space in it costs
/// every line it fills, which is what the renderer does with one (`screens/alerts.md`
/// § The height) and what a measure handing it a single line would hide (NOTES § D88).
fn finished_action(role: ContainerRole, ran_for: Option<SignedDuration>) -> &'static str {
    let short = ran_for.is_some_and(|d| d < PROBE_FLOOR);
    match (role, short) {
        (ContainerRole::Regular, false) => {
            "exit 0 does not say who ended the run — check the pod's events (kept an hour) for a \
             Killing line, the node for a memory killer. If nothing did it ends itself: if that \
             is meant it belongs in a Job or a CronJob; if not, it is quitting early"
        }
        (ContainerRole::Regular, true) => {
            "a health check rarely kills a run this short, so start with the program: if it ends \
             itself on purpose it belongs in a Job or a CronJob; if not, it quits early. A \
             Killing line in the events, or a memory killer on the node, says otherwise"
        }
        (ContainerRole::Sidecar, false) => {
            "exit 0 does not say who ended the run — check the pod's events (kept an hour) for a \
             Killing line, the node for a memory killer. If nothing did it ends itself, and this \
             one must run as long as the app does: finishing at all is the bug"
        }
        (ContainerRole::Sidecar, true) => {
            "a health check rarely kills a run this short, so start with the program: this one \
             must run as long as the app does, so finishing at all is the bug. A Killing line in \
             the events, or a memory killer on the node, says otherwise"
        }
        // **The duration decides nothing here**, because this arm opens no door a probe could
        // have taken: an init container that is not restartable may hold no probe at all
        // (`validateInitContainers`), so there is no first door to demote.
        (ContainerRole::Init, _) => {
            "finishing is what an init container is for, so ask what ran it again: Kubernetes \
             runs them all when the pod's sandbox is rebuilt. The events often do not say why, \
             and last about an hour; the node the pod is on is where the reason is"
        }
    }
}

/// **What to do about an [`Ending::Stopped`], per role — one reading of `exit 143` for the whole
/// file.** Rules 1 and 5 both reach it and must not disagree about one container, which is the
/// defect NOTES § D85 opens with; the sentences were byte-identical in both until they were
/// lifted here.
///
/// **[`Init`](ContainerRole::Init) is told the opposite of the other two on purpose**:
/// `validateInitContainers` forbids all three probes on an init container that is not
/// restartable, so *check the liveness probe* is advice it cannot follow.
///
/// Both are worded against [`describe`], which prints the probes, the events and — since the
/// resize door below — the pod's resize conditions; the node's system log is outside kubectl
/// either way.
///
/// **`systemd-oomd` is gone from both arms, because it cannot produce the card it was printed on**
/// (NOTES § D113). It kills a whole cgroup with `cgroup.kill`, which is SIGKILL and arrives as
/// `137`; this card is about `143`, a stop the program was asked for and obeyed. Only `earlyoom`
/// sends a catchable signal, so only `earlyoom` belongs here — the other name sent the reader
/// grepping their node's log for a tool that could never appear on this ending.
///
/// **The third producer is the kubelet resizing the pod in place**, `resizePolicy:
/// RestartContainer`, which VPA drives on a loop at this repo's target version — a reader whose
/// pod is being resized was being sent past the answer to two places holding nothing.
///
/// **The clause names the events, and the first draft named a field `describe` does not print**
/// (invariant 4, NOTES § D113). `kubectl describe pod | grep -ic resizePolicy` is **0**: *that can
/// be set to restart it* pointed at `get -o yaml`, which is the trade [`restart_rule_action`] makes
/// explicitly and this arm cannot, having four other doors on `describe`. What `describe` does
/// show is measured and **durable**: `Killing … Container app resize requires restart`,
/// `ResizeStarted` and `ResizeCompleted` under `Events:`. The `PodResizeInProgress` condition the
/// first draft leaned on is transient and gone by the time this card is drawn, which is the same
/// clause being wrong twice
/// (`reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md` § 3).
///
/// **On [`Regular`](ContainerRole::Regular) and [`Sidecar`](ContainerRole::Sidecar), and that is a
/// citation now rather than a hedge.** A restartable init container **is** resizable and the
/// kubelet restarts it — `kubectl patch --subresource resize` succeeded and `restartCount` went to
/// 1 — while a plain init container is refused by the API itself: *must not be set to
/// 'RestartContainer' for non-sidecar initContainers*. So the [`Init`](ContainerRole::Init) arm's
/// claim that the stop came from the node or from the program is **exhaustive**, measured.
///
/// **The log clause leads, because it is the one that lost its subject.** *Its own log* sat after
/// *the node, where a memory killer…*, whose nearest antecedent is the node — a door pointing at
/// the wrong room. Naming the container up front costs nothing and fixes the reference.
///
/// **What paid for it on [`Init`](ContainerRole::Init) is a restatement** (NOTES § D90's own
/// method): *check whether the program exits 143 of its own accord* is *or from the program
/// itself*, three clauses earlier, said again.
///
/// **On the other arm the first draft called the log clause a restatement and it was not.** *The
/// container's own logs will show nothing worse than a shutdown* names a **place not to look**,
/// where the clause in front of it names a **cause**, and a beginner's first move on a dead
/// container is `kubectl logs` — so cutting it would have closed a door and the comment defending
/// it would have taught the next reader that closing one is free. What was cut instead is the
/// *why* gloss on the probes (*a failing health check stops a container that never crashed*),
/// which the closing clause now carries in four words: **the log holds a shutdown and not a
/// crash** says both that the log is empty of answers and why the probe is the first place to
/// look. Every door the six-line version opened is open, one more is open than the five-line draft
/// had, and both arms measure five wrapped lines at 49 columns (NOTES § D113).
fn stopped_action(role: ContainerRole) -> &'static str {
    match role {
        ContainerRole::Regular | ContainerRole::Sidecar => {
            "the container's own log holds a shutdown and not a crash, so check the liveness and \
             startup probes, then the pod's events for a resize that restarted it, then the node, \
             where a memory killer such as earlyoom sends the same signal"
        }
        ContainerRole::Init => {
            "Kubernetes does not allow health checks on this kind of container, so the stop came \
             from the node or from the program itself — look for a memory killer such as earlyoom \
             in the node's system log, and for a program that exits 143 itself"
        }
    }
}

/// **What a run that failed needs next, and the fork is whether it ever ran** — shared whole by
/// [`crash_looping`] and [`previous_run_failed`], which both answer [`Ending::Failed`] and may not
/// answer it two ways about one container (NOTES § D85, § D113).
///
/// Returns the sentence and **whether the card's command has to serve that run's log**, because
/// the two are one decision: an action may only name what its own command prints (invariant 4),
/// and it was a card naming a log under [`describe`] that opened this whole area.
///
/// **The key is `run_length`, not the exit code, and the first draft got that wrong.** `126` and
/// `127` were read as *the container never started* — they are not. Measured on kind v1.36.1
/// (`reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md` § 2), they are what a
/// **shell inside a container that ran** reports, with real stamps, a real `containerID`, and the
/// whole diagnosis on one line of its log:
///
/// | `command` | code | `reason` | `startedAt` | `logs --previous` |
/// |---|---|---|---|---|
/// | `sh -c "exec /etc/hostname"` | `126` | `Error` | real | `sh: exec: …: Permission denied` |
/// | `sh -c "exec /usr/bin/nope"` | `127` | `Error` | real | `sh: exec: …: not found` |
/// | `["/definitely-not-here"]` | `128` | `StartError` | **epoch** | *(empty)* |
///
/// `tests/fixtures/notfound.json` is the middle row — `sh -c 'exec /usr/local/bin/server
/// --serve'` — so the counter-example was a committed capture the whole time. **And `128` is not a
/// safe key either**: a program may call `os.Exit(128)` itself, and that container ran and has a
/// log.
///
/// **It also cost a card its own consistency.** *What they name is not in the image* stood as the
/// action over an evidence line reading `exit 126 (the command was found but could not be run)` —
/// *found* against *not in the image*, NOTES § D85's class inside one card rather than between
/// two.
///
/// **[`ever_started`] is the discriminator that is actually true of *never ran*** — the epoch
/// `startedAt` Family A found, which containerd writes when it never got the process going.
///
/// **What the `None` arm means is *there is no log*, not *there is nothing to resolve it
/// through*** (NOTES § D113). An earlier draft of this paragraph said only a CRI status carries a
/// `containerID`, so `None` meant the flag could not resolve — **and the epoch record came
/// through CRI and carries one**, measured
/// (`reports/2026-08-16-terminated-record-stamps-and-authors.md` § 1). `--previous` on it is
/// perfectly servable; it returns nothing, exit status 0, because the process never wrote a byte.
/// The conclusion stands and the reason is the plain one: a card may not send a reader to an
/// empty log. **The snapshot carries no `containerID` at all** (invariant 6 pruned it), so the
/// start stamp is the only proxy there is — and it is a better one than the exit code, which is
/// what this replaced.
///
/// **`137` answers ahead of both**, because a kill from outside is neither of these questions and
/// its own log holds no error to find ([`killed_action`]). **It is not shared with rule 1's
/// caller in the other direction**: rule 6 has returned on `OOMKilled` by the time it asks and
/// rule 1 has not, so a shared `137` would put *a kill for using too much memory is not always
/// labelled as one* beside [`out_of_memory`]'s card saying it was — confirmed by the operator
/// review.
///
/// **The log arm's sentence is about what the log *holds*, and it took two goes to get there**
/// (NOTES § D113). *The application's own error* was wrong first, because a shell's `not found` is
/// not the application's; *that is where the program said what went wrong* was wrong second, on
/// the shape this same round added — a container the kernel SIGKILLed **said nothing**, it was cut
/// off mid-sentence, which is the premise [`killed_action`] is built on one arm up. What is true
/// of every code this arm now covers is that the log holds **the last thing written before the run
/// ended**: an application's error, a shell's message, or the output a kill interrupted. The
/// reader is told where to look and never what they will find, which is also what stops this
/// promising a diagnosis on a card that cannot know there is one.
///
/// **A labelled kill reaches it deliberately** — [`out_of_memory`] draws beside that card with the
/// fix, and what this one adds is the question rule 2 does not answer: what the container was
/// doing when the kernel took it.
fn failed_run_action(run: &Terminated, role: ContainerRole) -> (&'static str, bool) {
    // **[`killed_action`]'s own precondition, enforced where all three callers route through it**
    // (NOTES § D113). That sentence is for the `137` *nothing labelled*: it opens with *a kill for
    // using too much memory is not always labelled as one*, which is false of a container the
    // kernel labelled. `ending` sends the other three reasons elsewhere, so `OOMKilled` is the one
    // survivor and this is the whole of the check. Without it [`crash_looping`] put that hedge on
    // a CRITICAL card over its own evidence line reading *killed by the kernel for using more
    // memory than it was allowed*, beside [`out_of_memory`] asserting the label in its title —
    // three contradictions on one screen about `oom.json`, a committed capture.
    if run.exit_code == 137 && run.reason.as_deref() != Some("OOMKilled") {
        return (killed_action(role), false);
    }
    match ever_started(run) {
        // **The container never got as far as running**, so there is no log to read and
        // [`describe`] is what prints the `Command:` and `Args:` this sentence names.
        None => (
            "check the container's command and arguments — what they name is not in the image",
            false,
        ),
        // **A labelled kill falls through to here rather than getting an arm of its own.**
        // [`out_of_memory`] draws beside it and carries the fix; what this card owes is a next
        // step that does not repeat it and does not deny it. The log is where the program's last
        // output before the kernel took it is, which is a real second question — *what was it
        // allocating* — and it is the one thing rule 2's card does not answer.
        Some(_) => (
            "read that run's log — it holds the last thing written before the run ended, from \
             the program or from the shell that started it",
            true,
        ),
    }
}

/// **What to do about the `137` that is left once the other three readings are gone, per role** —
/// reached through [`failed_run_action`] by all three rules that read an ending, and the third of
/// the per-role sentences [`finished_action`] and [`stopped_action`] make.
///
/// **Every reason anything writes for itself is gone by the time this is called**, which is a
/// stronger statement than *not `OOMKilled`* and is why these two sentences may talk about a kill
/// without qualifying one. **That precondition is now enforced where the callers meet, and it was
/// not for one turn** (NOTES § D113): [`previous_run_failed`] returns on `OOMKilled` and
/// [`crash_looping`] and [`restarting_repeatedly`] do not, so a shared `137` keyed on the code
/// alone put *a kill for using too much memory is not always labelled as one* on a CRITICAL card
/// above [`out_of_memory`] asserting the label in its title, about `oom.json`. The check lives in
/// [`failed_run_action`], once, rather than in each caller. [`RESTART_ALL`],
/// [`STATUS_LOST`] and [`CODE_UNKNOWN`] are all answered by [`ending`] — the first leaves through
/// the silent arm of the `match`, the other two take arms of their own, and none can fall through
/// to here. (The last of them could not reach this sentence anyway: it is `255`.) What is
/// left is what a runtime writes for an ordinary bad exit — `Error`, on containerd — or no reason
/// at all, and the sentences below read the same either way. **The list is spelled here and
/// nowhere else** — it was written twice, went stale the first time a fourth reading was added,
/// and went stale again in the same turn that said so, because [`RESTART_ALL`] stopped returning
/// at the top and this copy still said it did (NOTES § D93, § D95).
///
/// **It was role-blind until 2026-08-15**, sending
/// an init container after a probe `validateInitContainers` forbids it while the sentences beside
/// it refused to name one on the same container — rule 5's card and rule 6's, one object,
/// contradicting each other on one screen (NOTES § D85).
///
/// **Deliberately not the log arm**: this kill came from outside the application, so the
/// container's own logs hold no error to find, and that sentence is the whole reason this arm
/// exists rather than falling through to *read that run's log* (NOTES § D85).
///
/// **Memory is on both arms even though rule 2 owns the labelled kill**: on a host without
/// headroom a genuine cgroup OOM arrives here as `137`/`Error` with the word simply lost, which
/// is the condition under which OOM kills happen and the one shape where nothing else on the
/// screen says *memory* (NOTES § D84).
///
/// **Neither arm names what sent the signal**, because the code does not ([`exit_meaning`]): they
/// open the doors and leave them open, the shape NOTES § D88, § D90 and § D93 settled next door.
/// **Rules 1 and 5 were deliberately left out of that split** — their `Failed` arm is a box of its
/// own, for the reason NOTES § D93 records.
///
/// **"Whether it stops when asked to" is a hypothesis, not a field, and that is on purpose.**
/// `describe` prints no such thing — `Termination Grace Period` shows only while a pod is
/// deleting — and the clause was queried for it and kept: the reader tests it against the
/// `Killing` event and the exit code the card's own title carries. `describe` *does* print the
/// probes and both limits, init containers included, which is the rest of the sentence
/// (invariant 4; asked and answered three times in this area, so it is written down here).
///
/// **`startup.json` is the captured `Regular` arm** — a `startupProbe` that never passes, killed
/// at the grace period. **No committed capture reaches the `Init` arm**; it is proved on a
/// decoded plant off `healthy-retry.json` (NOTES § D40).
fn killed_action(role: ContainerRole) -> &'static str {
    match role {
        ContainerRole::Regular | ContainerRole::Sidecar => {
            "check the liveness and startup probes, whether it stops when asked to, and the \
             memory limit: a kill for using too much memory is not always labelled as one, and \
             this kill came from outside the application, so its own logs will not say why"
        }
        ContainerRole::Init => {
            "check the memory limit: a kill for using too much memory is not always labelled as \
             one. Kubernetes allows this kind of container no health check, and this kill came \
             from outside the application, so its own logs will not say why"
        }
    }
}

/// **What to do about an [`Ending::Unwatched`] — one of the shared sentences, and the first that
/// was the same for every role** (NOTES § D95). The ordinal it used to carry counted a list that
/// has since lost a member and gained one, which is what a count in prose does (NOTES § D113). Rules 1, 5 and 6 all draw this ending
/// and all three say this; it was rule 6's alone until 2026-08-15, while rule 1 sent the reader
/// to a log the API refuses to serve and rule 5 to a memory limit nothing measured.
///
/// **Role-blind because nothing here is a kill**, which is also what keeps it clear of
/// `validateInitContainers`: a sentence that names no probe cannot name one an init container may
/// not have, so the split the other four make has nothing to split on.
///
/// **It names the sandbox and not the node's uptime.** The producer measured on kind v1.36.1 is
/// `crictl rmp -f` on the pod's sandbox; a node reboot is not one — containerd's state survives
/// it and the kubelet writes `exit 255` / [`CODE_UNKNOWN`] — and restarting containerd is a
/// no-op, the
/// shims outlive it. **It names the commoner of the two producers and the tail carries the
/// other**: a wedged shim reaches this reason with no sandbox rebuilt anywhere, which is what
/// *the node the pod is on is where the reason is* answers (`screens/alerts.md` § The height).
fn unwatched_action() -> &'static str {
    "no signal was recorded, so nothing here says what ended the run — a rebuilt pod sandbox \
     takes its containers with it, and the events rarely say so. The node the pod is on is where \
     the reason is"
}

/// **What to do about an [`Ending::RestartRule`], shared by rules 1 and 5** — rule 6 is silent on
/// this ending, so the sentence has two callers rather than three (NOTES § D95).
///
/// **It does not say *this container is fine*, and that is the whole of the wording.** The
/// kubelet writes the same synthesized record into *every* container's `lastState`, **the one
/// that triggered the restart included** — its own bad exit is in `state.terminated`, which no
/// rule reads — so a card that told the reader to look elsewhere would be wrong on exactly the
/// container that failed. Hence the closing clause: the container the reader is looking at may be
/// the one that set this off.
///
/// **It sends them to the spec, and that is why this arm's command is [`get_yaml`] and not
/// [`describe`]** — the trade rules 3, 4 and 12 already make where `describe` does not print the
/// field the card names (invariant 4, NOTES § D95). `restartPolicyRules` is declared on the
/// container that can trigger the gang restart, so on a measured pod it narrows to exactly one,
/// and on a single-container pod it resolves to that one.
///
/// **The two doors it does not open were measured shut on kind v1.36.1.** *Look for the container
/// with an exit code of its own* was this sentence until 2026-08-15 and it names a thing to find
/// that the pod does not contain: on a settled pod every container prints `Exit Code: 137` and
/// the trigger's own code is gone, on a thrashing one it was visible in 12 of 40 one-second
/// samples, and on a single-container pod there is nothing to compare. The pod's events are the
/// other dead end — rate-limited to `x2`/`x3` against 130 real restarts, and expiring. That is
/// [`killed_action`]'s own rule applied here: an action may name a *thing to find out*, never a
/// *thing to find* its command does not show.
///
/// **What to do when the container has a restart count and no record of the run at all** — the
/// role-blind like [`unwatched_action`] and [`no_exit_code_action`], for the same reason: it
/// names no probe and no kill (NOTES § D113).
///
/// **Shared by rules 1 and 5, which is what stopped them contradicting each other.** Rule 5's arm
/// was written for this shape; rule 1's fell through to *read the previous run's logs* — a log the
/// API refuses to serve, because the kubelet gates `logs --previous` on
/// `lastState.terminated.containerID` and the record that would carry it is the record this arm is
/// about. Two rules reading one container and answering one shape two ways is NOTES § D85's own
/// class, so the answer is one sentence and not two.
///
/// **It no longer says *the pod has kept the count***, which rule 5's card can support and rule
/// 1's cannot: `CrashLoopBackOff` is the wait *before* the next start and
/// [`ContainerSnapshot::restarts`] can still be `0` there. What both cards do support is that the
/// run is gone. Rule 5's count is on its own title either way.
///
/// **Role-blind for [`unwatched_action`]'s reason**: it names no probe and no kill, so there is
/// nothing `validateInitContainers` can refuse.
///
/// **It names a thing to find out and not a thing to find** ([`killed_action`]'s rule): the events
/// *may* still hold it, and the honest answer when they do not is to wait for the next restart,
/// which writes the record back. Both are in [`describe`], which is what every caller of this arm
/// puts on the card.
fn no_record_action() -> &'static str {
    "the run that ended is not on the pod any more, so nothing here says why. Check the pod's \
     events, which may still name what stopped it — and if they have expired too, the next \
     restart will write the run back, so watch rather than guess"
}

/// **What to do about an [`Ending::RestartRule`], shared by rules 1 and 5** — rule 6 is silent on
/// this ending, so the sentence has two callers rather than three (NOTES § D95).
///
/// **It does not say *this container is fine*, and that is the whole of the wording.** The
/// kubelet writes the same synthesized record into *every* container's `lastState`, **the one
/// that triggered the restart included** — its own bad exit is in `state.terminated`, which no
/// rule reads — so a card that told the reader to look elsewhere would be wrong on exactly the
/// container that failed. Hence the closing clause: the container the reader is looking at may be
/// the one that set this off.
///
/// **It sends them to the spec, and that is why this arm's command is [`get_yaml`] and not
/// [`describe`]** — the trade rules 3, 4 and 12 already make where `describe` does not print the
/// field the card names (invariant 4, NOTES § D95). `restartPolicyRules` is declared on the
/// container that can trigger the gang restart, so on a measured pod it narrows to exactly one,
/// and on a single-container pod it resolves to that one.
///
/// **The two doors it does not open were measured shut on kind v1.36.1.** *Look for the container
/// with an exit code of its own* was this sentence until 2026-08-15 and it names a thing to find
/// that the pod does not contain: on a settled pod every container prints `Exit Code: 137` and
/// the trigger's own code is gone, on a thrashing one it was visible in 12 of 40 one-second
/// samples, and on a single-container pod there is nothing to compare. The pod's events are the
/// other dead end — rate-limited to `x2`/`x3` against 130 real restarts, and expiring. That is
/// [`killed_action`]'s own rule applied here: an action may name a *thing to find out*, never a
/// *thing to find* its command does not show.
///
/// **Role-blind, for [`unwatched_action`]'s reason**: it names no probe, so there is nothing to
/// refuse an init container.
fn restart_rule_action() -> &'static str {
    "this record does not say which container exited — the one whose spec declares the restart \
     rule (restartPolicyRules) can set it off, and that may be this container"
}

/// **What to do about an [`Ending::CodeUnknown`] — a shared sentence, and one of the three that
/// are the same for every role.** Rules 1, 5 and 6 all draw this ending and all three say this.
///
/// **Role-blind for [`unwatched_action`]'s reason**: nothing here is a kill and nothing here is a
/// health check, so there is no probe to name and nothing `validateInitContainers` can refuse.
///
/// **It names a thing to find out and not a thing to find** ([`killed_action`]'s rule, NOTES
/// § D93, § D95). The first draft ended *and read the previous run's own log* under a card whose
/// command is [`describe`], which prints no logs at all (invariant 4). The pod's events are what
/// `describe` does print, and *did the node restart* is a question they can answer.
///
/// **The second reason given for leaving the log out was measured false and is withdrawn**
/// (NOTES § D113). *`lastState` freezes, so `--previous` serves a different run* generalised D112
/// into a property of the field: only the **synthesized** write is gated, and the record advances
/// at every ordinary restart. So the door this ending has and [`Unwatched`](Ending::Unwatched) has
/// not is open — the record carries the `containerID` the kubelet gates the flag on, and
/// [`previous_logs`] exists now. **What keeps this arm on [`describe`] is the sentence and not the
/// flag**: nobody read how that run ended, so its log is not where the answer is; the events are,
/// and they are what this action names.
fn no_exit_code_action() -> &'static str {
    "the code is a stand-in the node wrote, not the application's — a machine restart is what \
     usually leaves one, and the pod's events are where that shows"
}

/// **Rule 1 — the container is going round a restart loop and Kubernetes has started waiting
/// between the restarts.** `state.waiting.reason == CrashLoopBackOff`, CRITICAL: whatever the
/// loop is, this container is not doing its job right now and will not start doing it without a
/// change.
///
/// **Which loop it is comes from how the last run ended** ([`ending`], NOTES § D85), and only
/// [`Failed`](Ending::Failed) is a crash: `exit 0` is a run that ended without an error — which
/// says nothing about *who* ended it, the whole of [`finished_action`] — and `exit 143` is a
/// shutdown signal delivered and obeyed, usually by something outside the container. Each branch
/// owes its own action, because *read the previous run's logs* sent the reader of the other two to
/// a log holding no answer — and on 2026-08-16 it stopped being any branch's answer at all, for
/// the reason it was never the right one on this rule: no command on this card prints a log
/// (invariant 4, NOTES § D113).
///
/// **[`Unwatched`](Ending::Unwatched) is the branch where that action is not merely unhelpful but
/// impossible** (NOTES § D93). The kubelet gates `logs --previous` on
/// `lastState.terminated.containerID` and the synthesized status carries none, so the API answers
/// `previous terminated container … not found` — measured. The title goes with it: nothing is
/// known to have crashed, and *keeps crashing* over a translation calling the number a
/// placeholder is D85's contradiction in this rule's own words.
///
/// **That arm is as unproven as the one below it, and only one of them said so** (NOTES § D95).
/// `CrashLoopBackOff` beside `lastState.reason: ContainerStatusUnknown` was never produced in
/// about twenty attempts on kind v1.36.1: removing the sandbox under a running container writes
/// the synthesized record and restarts it with **no** backoff; doing it repeatedly writes real
/// `Error` records and then the record freezes; doing it to a container already backing off
/// leaves its real `exit 1` in place. The kubelet gate D93 recorded is why — the synthesized
/// write is skipped when `LastTerminationState.Terminated != nil`, and a container that has
/// earned a backoff necessarily has one. **Not proven unreachable, never produced**, and the
/// shapes it is tested on are planted rather than captured (NOTES § D40).
///
/// **[`RestartRule`](Ending::RestartRule)'s arm exists and is barely reachable, which is said
/// rather than assumed.** The restart-all path purges every container from the runtime, so
/// `doBackOff` finds no exited record to back off from and `CrashLoopBackOff` does not appear —
/// measured at about one restart every 11s behind an 8s sleep, which is no backoff at all. The
/// arm is written because the enum makes the rule answer, and it is truthful if a cluster ever
/// does produce the pair; the shape it is tested on is planted, not captured (NOTES § D40, § D93).
///
/// **The titles say what one `lastState` can support and no more.** The snapshot holds one run,
/// so *nothing has crashed* was an absolute drawn from a single sample: a container that failed
/// four times and then exited `0` stays in `CrashLoopBackOff` with a clean `lastState`. For the
/// same reason the restart is present tense — `CrashLoopBackOff` is the wait *before* the next
/// start, and [`ContainerSnapshot::restarts`] can still be `0`, which is why the evidence line
/// guards it.
///
/// **And they say *which* run on 2026-08-16, which is the half that was still wrong.** `lastState`
/// is *the last run Kubernetes wrote down*, not *the run before this one*: the kubelet writes its
/// synthesized record only where `LastTerminationState.Terminated` is still empty, so on a
/// container that has terminated before, the entry freezes — measured at nine restarts under one
/// unchanged `finishedAt`. *The container's last run* was therefore false of every such container,
/// and the three titles that said it now say *the last run on record*, or say what **the record**
/// does. That is [`restarting_repeatedly`]'s own framing rather than a third one, so the two rules
/// drawing about one container cannot disagree about what they read (NOTES § D85, § D95).
///
/// **The action then splits on [`ContainerRole`], because the same exit means different things
/// per role.** `exit 0` on a [`Sidecar`](ContainerRole::Sidecar) is KEP-753's Job — pod
/// `restartPolicy: Never`, `initContainers[].restartPolicy: Always` — where *move it to a Job*
/// is advice about the workload it already is; and `exit 143` on an
/// [`Init`](ContainerRole::Init) cannot be a probe, because `validateInitContainers` forbids all
/// three probes on an init container that is not restartable. Both live in [`finished_action`]
/// and [`stopped_action`], shared with rule 5 rather than written twice — two rules reading one
/// container and disagreeing is where NOTES § D85 starts.
///
/// **All three roles reach the [`Finished`](Ending::Finished) branch, `Init` included.**
/// `doBackOff` keys on the container's name and never reads the exit code, and `SyncPod` runs
/// init containers through it like any other — so a plain init container that succeeded while
/// its backoff entry was still live, and whose pod's sandbox is then rebuilt under it, is re-run
/// and lands in `CrashLoopBackOff` with `exit 0` behind it. `healthy-retry.json` is one rebuild
/// from that object. This was reported unreachable while rule 5's arm was being written, on the
/// reasoning that the kubelet moves on to the next init container — which is what it does
/// *inside* one sandbox and not across a rebuild (NOTES § D88).
///
/// **A node reboot is not one of the rebuilds that reach this branch, though it is one of rule
/// 5's**: `kl.backOff` is a `flowcontrol.Backoff` built in-process by `NewMainKubelet`, so a
/// kubelet that has just started carries an empty map and has nothing to make the re-run
/// container wait. What reaches this branch is a sandbox that dies while the kubelet does
/// not — a CNI or sandbox flap the kubelet records as `SandboxChanged`, a `crictl rmp`, a
/// container-runtime restart — landing inside a backoff window earlier failures earned
/// (NOTES § D88).
///
/// **Six of the seven branches carry [`describe`]** — the pod's events are what separate a
/// program that finished from one something else stopped, and they are in no other output. The
/// `Finished` branch named `restartPolicy` under `get -o yaml` until 2026-08-14, which
/// bought a field the state already implies — a container backing off from a clean exit is under
/// a policy that restarts one — at the price of hiding the only evidence that could correct the
/// card (NOTES § D88). **[`RestartRule`](Ending::RestartRule) is the one that goes the other
/// way**, for the opposite reason: its action names `restartPolicyRules`, which the state does
/// *not* imply and `describe` does not print at all ([`restart_rule_action`], NOTES § D95).
/// [`CodeUnknown`](Ending::CodeUnknown)'s action asks whether the node restarted — a question only
/// the events can answer — and the seventh branch, **the container with no record at all**, sends
/// the reader to the same events for the same reason ([`no_record_action`], NOTES § D113).
///
/// **The severity does not move with the branch.** It answers *is this container serving*, and
/// on a container the kubelet is backing off from the answer is no in all three — an amber card
/// beside a red `CrashLoopBackOff` in `kubectl get pods` teaches the reader to trust the other
/// tool (NOTES § D2).
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
        facts.extend(ran_for(run));
        facts.push(exit_fact(run));
    }
    // **The run is bound by the `match`, not looked up again inside an arm** (NOTES § D113). The
    // scrutinee was `…map(ending)`, so an arm needing the run itself had to re-open the `Option`
    // the `match` had already destructured — behind a panicking unwrap that could not fire
    // *today*, because the arm had proved the `Option` full. **This file carries no such call and
    // carried none before this family**, and the reason its purity holds is that nothing in it
    // *can* fail rather than that nobody has yet made it (invariants 5 and 8): a scrutinee
    // changed or an arm added turns a human's promise into a panic on somebody's terminal.
    // Pairing the ending with the run hands that promise to the compiler and deletes the second
    // lookup with it. The count is the assertion — `grep -c` over this file for either unwrapping
    // call is **0**, and that is checkable without a build.
    let (title, action, cmd) = match c.last_terminated.as_ref().map(|run| (ending(run), run)) {
        // **The one call in the file that passes a duration**, because this card is the one that
        // prints one: [`ran_for`] is in `facts` above, so the order the action arrives in has a
        // visible reason on the card ([`finished_action`], NOTES § D113).
        Some((Ending::Finished, run)) => (
            "The last run on record finished cleanly, and Kubernetes is restarting it \
             (CrashLoopBackOff)",
            finished_action(c.role, run_length(run)),
            describe(&pod.id),
        ),
        Some((Ending::Stopped, _)) => (
            "The last run on record was stopped, and Kubernetes is restarting it \
             (CrashLoopBackOff)",
            stopped_action(c.role),
            describe(&pod.id),
        ),
        Some((Ending::Unwatched, _)) => (
            "Kubernetes did not record how the run it last saw ended, and is restarting it \
             (CrashLoopBackOff)",
            unwatched_action(),
            describe(&pod.id),
        ),
        // **`describe` again**: the action asks whether the node restarted, and the pod's events
        // are the only output that can answer it.
        Some((Ending::CodeUnknown, _)) => (
            "The last run on record has no exit code of its own, and Kubernetes is restarting it \
             (CrashLoopBackOff)",
            no_exit_code_action(),
            describe(&pod.id),
        ),
        // The one arm of this rule whose command is not `describe`: its action names
        // `restartPolicyRules`, which lives in the spec and in no part of describe's output
        // (invariant 4, [`restart_rule_action`], NOTES § D95).
        Some((Ending::RestartRule, _)) => (
            "The pod's own restart rule removed the container, and Kubernetes is restarting it \
             (CrashLoopBackOff)",
            restart_rule_action(),
            get_yaml("pod", &pod.id),
        ),
        // **[`failed_run_action`], shared with rules 5 and 6 rather than answered a second way**
        // (NOTES § D113). This arm said *read the previous run's logs — that is where it says why
        // it exits* under a card whose command is [`describe`], which prints no logs at all —
        // invariant 4 in the small — while rule 5's card about the same ending sent the reader to
        // the memory limit and the probe. The two rules never co-fire, so nothing is folded away;
        // what is gone is one ending answered two ways.
        //
        // **[`failed_run_action`] answers this whole ending, and for one turn this arm did not
        // ask it.** `Ending::Failed` covers every code the four keyed pairs do not, so this arm
        // handed `notfound.json` — `exit 127`, a committed capture — a memory limit and a probe
        // over an evidence line reading *the command was not found*. One ending, one answer.
        Some((Ending::Failed, run)) => {
            let (action, log) = failed_run_action(run, c.role);
            (
                "Container keeps crashing, and each restart waits longer (CrashLoopBackOff)",
                action,
                if log {
                    previous_logs(&pod.id, &c.name)
                } else {
                    describe(&pod.id)
                },
            )
        }
        // **The half where the log was not merely unhelpful but unreachable.** With no `lastState`
        // there is no `lastState.terminated.containerID`, and the kubelet gates `logs --previous`
        // on exactly that field — so the card was in this arm *because* the flag its advice
        // implied could not work. **The title goes with it**: *keeps crashing* is a claim about
        // runs the pod no longer holds, and the count beside it can be `0`. What the object
        // supports is the wait and the missing record ([`no_record_action`], shared with rule 5).
        None => (
            "Kubernetes is restarting this container and the run that ended is not on the pod \
             (CrashLoopBackOff)",
            no_record_action(),
            describe(&pod.id),
        ),
    };
    Some(Finding {
        severity: Severity::Critical,
        title: title.to_string(),
        evidence: facts.join(FACTS),
        action: action.to_string(),
        kubectl_cmd: cmd,
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
/// **Captured**: `oomserving.json` is an OOM kill in `lastState` on a container that is running
/// and ready again, and both directions of the recency clause are read off it at two moments.
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
///
/// **This is the only `format!` title in the file that interpolates `state.waiting.reason`, and
/// what bounds it is this rule's own allowlist** (NOTES § D113). The guard three lines below —
/// `if !UNUSABLE_IMAGE.contains(&reason) { return None; }` — is not a trigger only: it is the
/// bound, because a `reason` outside those seven never reaches the `format!` at all. The seven are
/// compile-time constants, the longest is `SignatureValidationFailed` at 25 characters, and the
/// title it makes measures **2 lines with 10 free** against `screens/alerts.md`'s three-line cap.
///
/// **The first draft of this paragraph said the opposite** — *nothing on the path checks that what
/// arrives is one of them* — and reasoned about a 39-character threshold for an input the function
/// refuses. Recorded here as what is true rather than deleted, because the shape of the mistake is
/// the reusable part: a free-text field reached by an allowlist is bounded by the allowlist, and
/// the ingest bound `k8s.rs` owes every other free-text field is a second line of defence here
/// rather than the only one.
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
        kubectl_cmd: get_yaml("pod", &pod.id),
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
        kubectl_cmd: get_yaml("pod", &pod.id),
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
/// **And it changes with how the last run ended, for the reason rule 1's did** ([`ending`],
/// NOTES § D85): *something keeps killing it* is not what `exit 0` says, and *check the memory
/// limit and the liveness probe* is the same sentence one line down — a container cannot breach
/// its memory limit and come back as `143`, because a cgroup breach is a `SIGKILL`. So each
/// ending owns its own action, and the claim that something is killing this container is left to
/// [`Failed`](Ending::Failed).
///
/// **The two `137` reasons the kubelet writes itself get arms rather than an exemption, and the
/// exemption was the tempting answer** (NOTES § D95). Rule 6 goes silent on
/// [`RestartRule`](Ending::RestartRule) because *the last run on record failed* is its whole subject
/// and that ending refuses it. This rule's subject is the **count**, which is real under both
/// reasons and is the only thing left saying anything at all: one restart-rule firing writes the
/// same synthesized record into every container's `lastState`, so a rule 5 exemption would leave
/// a pod thrashing 31 times in six minutes with no card on the screen. What goes instead is the
/// claim and the action — *something keeps killing it* is a positive claim of repeated killing on
/// a run nothing is recorded as having killed, and the memory limit and liveness
/// probe are doors onto a kill under an evidence line saying no kill was seen.
///
/// **Both new claims are worded about the *record* and never about *the last run*, and that is
/// the whole of their wording** (NOTES § D95). `lastState` does not merely outlive the incident,
/// it **freezes**: the kubelet skips the synthesized write when a previous termination is already
/// there, so the record stood still while `restartCount` went 7 → 16 on a measured cluster. *Its
/// last run has no ending on record* is therefore false the moment anything has run since — the
/// boxed *last run Kubernetes wrote down* class, in its sharpest form, because a clause worded
/// tightly around one run inherits it where the count-shaped sentence it replaced did not. What
/// the object supports is what the record says, so that is what the clause says.
///
/// **They are also short because the card has a height** (`screens/alerts.md` § How wide a card
/// is, and how tall). A three-digit `restartCount` is ordinary — a measured cluster reached 132 in
/// ten minutes — and at 51 columns the clause is what decides whether the title wraps to two lines
/// or three, and a title over three wrapped lines is over `screens/alerts.md`'s own cap for the
/// part (NOTES § D113).
/// `the_cards_this_box_ships_fit_the_height_they_are_drawn_at` measures them there.
///
/// **What is still open is the fan-out, and it is not a wording problem**: one firing draws one
/// card per container in the pod — six on a six-container pod, one event — because every one of
/// them carries the record. Finding the trigger needs `state.terminated`, which no rule reads,
/// and it is boxed with the rest of that class (NOTES § D95).
///
/// **A clean ending names no agent, because one exit code cannot carry one**: an application
/// that traps SIGTERM and shuts down tidily reports `0`, and the kubelet writes `0` /
/// `Completed` whether the program chose to stop or something outside asked it to. The asymmetry
/// is NOTES § D85's own — a *positive* claim of repeated killing survives on a non-zero exit
/// beside a count, while *nothing killed it* needs to know who did, and one code does not say.
/// So the [`Finished`](Ending::Finished) arm leaves both readings open and sends the reader to
/// the `Killing` event and to the node, which are where a kill is recorded and where the one
/// kind that records nothing comes from ([`finished_action`]) —
/// **and [`describe`] is therefore *that arm's* command, not [`get_yaml`]**: `restartPolicy` and
/// the events are in different outputs, and an action may only name what its own command prints
/// (invariant 4) — which is the same rule that sends the
/// [`RestartRule`](Ending::RestartRule) arm the other way, to the spec field `describe` omits.
/// **Rule 1 answers the same ending with the same sentence and the same command**
/// ([`finished_action`]): its container is backing off rather than serving, which changes what
/// the card is about and not what one exit code can say about who ended the run (NOTES § D88).
///
/// **The claim also stays inside one `lastState`, which is all the snapshot holds.** Each arm
/// describes *that run*, not the whole count — *nothing is killing it* over ten restarts and one
/// clean sample would be NOTES § D85's own absolute rebuilt here — and **the arm with no
/// previous run at all claims nothing**: `restartCount` outlives a `lastState` the kubelet has
/// dropped, and a count on its own supports the count. **It offers [`describe`] like every arm
/// but [`RestartRule`](Ending::RestartRule), and deliberately not `kubectl logs --previous`**:
/// the kubelet gates that flag on
/// `lastState.terminated.containerID` — the very field whose absence puts a card in this arm —
/// so there is no state in which this arm fires and that command returns a log. The events are
/// what may still hold something, and they are in `describe`.
///
/// **Each role is told what is true of it, and every ending splits by role** — in
/// [`finished_action`], [`stopped_action`] and [`failed_run_action`], all three *shared* with rule 1
/// rather than copied beside it: two rules reading one container and disagreeing is where
/// NOTES § D85 starts, and two byte-identical copies are that defect with a delay on it.
/// **The splits are not all the same shape.** [`Stopped`](Ending::Stopped) and
/// [`Failed`](Ending::Failed) split two ways, `Regular` and `Sidecar` sharing a line, because
/// `validateInitContainers` forbids all three probes on an init container that is not
/// restartable, so any sentence naming one is a dead end there.
///
/// **[`Init`](ContainerRole::Init) is split out on [`Finished`](Ending::Finished) for a reason
/// neither rule can gate away.** A plain init container whose pod's sandbox was rebuilt — a node
/// reboot, a containerd restart, a killed sandbox — is re-run from the start while `restartCount`
/// and `lastState` persist on the same pod object, so it arrives here `Running`, `ready: false`,
/// past the band, with a clean previous run behind it. Given the sidecar's sentence it would be
/// told *finishing is the bug* under an evidence line reading *the app starts only after this one
/// finishes*, and sent after a probe its own [`Stopped`](Ending::Stopped) arm refuses to name.
/// [`Sidecar`](ContainerRole::Sidecar) is not meant to finish; [`Init`](ContainerRole::Init) is,
/// and is asked what re-ran it instead. **[`Finished`](Ending::Finished) therefore splits three
/// ways here — and in rule 1 too, whose `CrashLoopBackOff` was read as gating that shape out and
/// does not**: [`crash_looping`] carries the producer, and the claim of unreachability was made
/// while this arm was being written (NOTES § D88).
///
/// **Severity is WARN whenever the container is serving, whatever the count**, and **the ending
/// does not move it**: the band answers *is this container serving*, and a container that keeps
/// finishing early and is not ready now is as down as one that keeps crashing. A lifetime counter
/// carries no *rate*, and REQUIREMENTS marks the two numbers *(suggestion)* (NOTES § D71). The age
/// is when the counter last went up — **and for a serving container that moment is the start of
/// the run it is in**, `state.running.startedAt`, not the frozen `lastState` that a restart rule
/// leaves with no stamp at all (NOTES § D100).
///
/// **The serving card ages out of the screen at [`NOT_READY_GRACE`], and nothing else does**
/// (NOTES § D100). `restartCount` never goes down, so a container that used its three restarts
/// and has served ever since would carry a permanent WARN on a screen whose subject is *what is
/// broken now* — rule 2's question exactly, answered with rule 2's threshold and rule 2's
/// `is_some_and`: **a container with no start time keeps its card, and so does one whose start
/// time is in the future**, because the exemption is proved rather than assumed. A container that
/// is **not** serving never ages out, whatever the count.
///
/// **What it costs, measured against the file rather than assumed:** a container on a long cycle
/// — an OOM every thirty minutes, a JVM that dies on the nightly batch — is off this card for
/// most of that cycle, and **no other rule here fills the gap while it is serving**.
/// [`previous_run_failed`] stands down on the same `doing_its_job`, [`out_of_memory`] on the same
/// clause with the same threshold, and [`crash_looping`] needs a backoff state the container is
/// not in. So between ten minutes after a restart and the next one the screen is quiet about it.
/// Accepted for the reason rule 2 already accepts it on a kill that is worse to miss than a
/// count: the alternative is a restart *rate*, which is either history — invariant 5 forbids it —
/// or `restarts ÷ pod age`, a second number deciding what the clock means (NOTES § D100, whose
/// own sentence about rule 6 drawing throughout does not hold: that rule is silent on a serving
/// container by design).
///
/// **[`RESTART_ALL`] joins `CrashLoopBackOff` in the waiting exemption for a reason about
/// sampling, not about gang restarts** (NOTES § D100). A pod whose restart rule fires parks every
/// container in that waiting reason for about two seconds of every cycle, and the severity above
/// is keyed on `serving` — so the same card was measured flipping WARN ↔ CRITICAL every restart,
/// 1104 samples against 354 on one container. The count is unaffected and the card returns the
/// moment the container is running again: what is refused is a *point sample of a transient*
/// deciding what the user sees — the same objection this rule's `CrashLoopBackOff` exemption
/// makes, one reason over. **What it costs, if that state ever stopped being transient:** a
/// container held in it draws nothing from this rule, and nothing from rule 13 either, whose
/// first condition is a container that has never run. A wedged kubelet is the N-series' subject
/// and not this one's, but the hole is named rather than left to be found.
///
/// **Both halves of the band are captured**: `restarts10.json` is past ten restarts and not
/// serving, and `restarts10serving.json` is the same count on a container that is — read at a
/// moment inside its run, since at the pinned `now` it has been serving 49 hours and the clause
/// above has stood it down. **No ending but [`Failed`](Ending::Failed) is**, and no committed
/// capture reaches one: every captured restart history exits non-zero, and the two objects that do
/// end cleanly — `exit0.json` and `sigterm.json` — are in `CrashLoopBackOff`, which is this rule's
/// own exemption. The others are synthesized on decoded copies (NOTES § D40). **No committed
/// capture holds [`RESTART_ALL`] in `state.waiting` either** — that object is on the capture trip
/// (NOTES § D100), so the new exemption is proved on a decoded copy and not on bytes.
fn restarting_repeatedly(now: &Time, pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    // An init container that has finished successfully is out of this rule's subject altogether,
    // not merely a milder case of it: its count is frozen for the life of the pod, and every
    // sentence below is about a container that is *still* being restarted (NOTES § D75).
    if c.role == ContainerRole::Init && doing_its_job(c) {
        return None;
    }
    if c.restarts < RESTARTS_WARN
        || matches!(
            waiting(c).map(|(r, _)| r),
            Some("CrashLoopBackOff" | RESTART_ALL)
        )
    {
        return None;
    }
    // Every container that reaches here is judged by the expression this rule always used, and
    // the one init case that differs returned above — so the title below cannot say "it is
    // serving now" about a container that has stopped.
    let serving = doing_its_job(c);
    // *Serving* implies [`ContainerState::Running`] here — the one other arm `doing_its_job`
    // answers `true` on returned above — so this is `None` only where the API omitted the field,
    // and `is_some_and` keeps the card in that case and on a future stamp (NOTES § D100).
    let running_since = match &c.state {
        ContainerState::Running { started_at } => started_at.as_ref(),
        _ => None,
    };
    if serving && running_since.is_some_and(|t| now.0.duration_since(t.0) > NOT_READY_GRACE) {
        return None;
    }
    // `claim` carries its own leading comma so the last arm can add nothing at all, and every
    // arm's action names only what its own command prints (invariant 4).
    //
    // **It goes on both titles and not only the serving one** (NOTES § D102). It was the serving
    // branch's alone until 2026-08-16, which put the diagnosis on the *only* card that never
    // co-fires with rule 6 — [`previous_run_failed`] leaves on [`doing_its_job`], so the pair the
    // fold collapses is always the *non*-serving one and every fold left the bare count behind.
    // What went with rule 6's card was a title-line sentence, and the evidence line that was left
    // holding it is the one line `screens/alerts.md` § The height may cut. This is not a rule
    // knowing what its neighbour drew: [`ending`] already told this rule how the run ended, and
    // the title is where a rule says what it read.
    // The run is bound by the `match` rather than looked up again inside an arm, for
    // [`crash_looping`]'s reason (NOTES § D113).
    let (claim, action, cmd) = match c.last_terminated.as_ref().map(|run| (ending(run), run)) {
        // **The duration, and until 2026-08-16 this passed `None`** ([`finished_action`],
        // NOTES § D113). The reason it did was that this rule's evidence line carried no
        // duration, so ordering by one would have been a visible order with a hidden reason —
        // and the line carries one now, added for the fold two paragraphs down. The constraint is
        // met, so the rule that was waiting on it applies: rules 1 and 5 order this ending the
        // same way, off a fact both cards show.
        Some((Ending::Finished, run)) => (
            ", and the last run on record finished cleanly",
            finished_action(c.role, run_length(run)),
            describe(&pod.id),
        ),
        Some((Ending::Stopped, _)) => (
            ", and the last run on record was stopped",
            stopped_action(c.role),
            describe(&pod.id),
        ),
        Some((Ending::Unwatched, _)) => (
            ", and the record names no ending",
            unwatched_action(),
            describe(&pod.id),
        ),
        // Not `describe`, for the reason rule 1's matching arm carries.
        Some((Ending::RestartRule, _)) => (
            ", and the record names the pod's rule",
            restart_rule_action(),
            get_yaml("pod", &pod.id),
        ),
        // **It says the code is not the container's, never that there is none** — the evidence
        // line one row down prints `exit 255`, and *carries no exit code* was this rule denying
        // it. Rules 1 and 6 say *has no exit code of its own* and survive the same evidence;
        // three words were doing the work and this clause had dropped them (NOTES § D85).
        Some((Ending::CodeUnknown, _)) => (
            ", and the exit code is not its own",
            no_exit_code_action(),
            describe(&pod.id),
        ),
        // **[`failed_run_action`], the third caller and the last** (NOTES § D113). This arm had a
        // sentence of its own — *check the memory limit and the liveness probe* — and on
        // `restarts10.json` it stood on the CRITICAL card while rule 6's WARN card below it said
        // *read that run's log* about the same `exit 1`, whose own translation on this card's
        // evidence line is *the application's own error*. The count is this rule's subject; how
        // the run ended is [`ending`]'s, and it is read right here, so there was never a fork
        // this rule could not ask.
        Some((Ending::Failed, run)) => {
            let (action, log) = failed_run_action(run, c.role);
            (
                ", but something keeps killing it",
                action,
                if log {
                    previous_logs(&pod.id, &c.name)
                } else {
                    describe(&pod.id)
                },
            )
        }
        None => ("", no_record_action(), describe(&pod.id)),
    };
    // The exit code goes ahead of the image: every clause the serving title adds is read off it,
    // and evidence is the one card line the screen may cut (`screens/alerts.md` § The height).
    //
    // **The duration joined them on 2026-08-16, and it is the fold that asked for it**
    // (NOTES § D113). Rules 5 and 6 answer [`Ending::Failed`] with one sentence and one command
    // now, so on `restarts10.json` they drew two cards identical from the arrow down — and the
    // only thing keeping [`one_card_per_action`] from collapsing them was that rule 6 carried
    // `ran for under a second` and this rule did not. A duration is not a reason to spend a card:
    // it is the first fork of every crashloop triage (NOTES § D51), this rule reads the same run
    // rule 6 does, and [`ran_for`] is the one spelling of it. **It goes ahead of the image for the
    // reason the image is last** — the image is what the three-line cut takes, and a duration is
    // not.
    //
    // **The fold's subset clause is now maintained by a choice rather than by an accident**, which
    // is what [`one_card_per_action`]'s own caveat asks for: rule 6's facts are
    // [`container_fact`] · the quote · [`ran_for`], and this list holds the first and third
    // deliberately. The card that still stands is the one carrying the container's last words,
    // which is a fact worth a card.
    let mut facts = vec![container_fact(c)];
    if let Some(run) = &c.last_terminated {
        facts.push(exit_fact(run));
        facts.extend(ran_for(run));
    }
    facts.push(c.image.clone());
    Some(Finding {
        severity: if c.restarts >= RESTARTS_CRITICAL && !serving {
            Severity::Critical
        } else {
            Severity::Warn
        },
        title: if serving {
            format!(
                "Container has been restarted {} times — it is serving now{claim}",
                c.restarts
            )
        } else {
            format!("Container has been restarted {} times{claim}", c.restarts)
        },
        evidence: facts.join(FACTS),
        action: action.to_string(),
        kubectl_cmd: cmd,
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        // The moment the counter last went up, from whichever field records it for this shape:
        // for a serving container that is the start of the run it is in, and the ending it came
        // out of carries no stamp at all when a restart rule wrote it (NOTES § D100).
        timestamp: if serving {
            running_since.cloned()
        } else {
            c.last_terminated
                .as_ref()
                .and_then(|t| t.finished_at.clone())
        },
    })
}

/// **Rule 6 — the last run this container has on record ended badly, and here is what the code
/// means.** *Previous* was the word until 2026-08-16 and the object does not support it: the
/// record is the last run Kubernetes **wrote down**, which is the run before this one on every
/// ordinary restart and is *not* on the one shape D112 found — a container whose first lost status
/// the kubelet synthesized, where the write is skipped while a previous termination is still
/// there. That shape is [`Ending::Unwatched`], and it is the only one, measured
/// (`reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md` § 1, NOTES § D113).
/// `lastState.terminated.exitCode`, WARN: the run that failed is over, and where the container is
/// *currently* broken rules 1 to 4 say so as CRITICAL beside this.
///
/// **Two exits are not findings** — `0` and `143`, every rolling update and every scale-down
/// (NOTES § v1 rule set) — and **`OOMKilled` belongs to rule 2**: one event, one card. The two
/// codes are [`ending`]'s to name, not this rule's, because rule 1 reads the same container and
/// has to reach the same verdict about it (NOTES § D85).
///
/// **[`RESTART_ALL`] is the third exemption, and it is an exemption rather than a wording
/// problem** (NOTES § D93). A container in this pod declared `restartPolicyRules`
/// ([`RESTART_ALL`] — the field is a container's, the effect is the pod's, NOTES § D96); the
/// kubelet removed this one because those rules said to. Nothing failed and nothing was killed
/// from outside, so a WARN card is the false-positive class this rule's whole design exists to
/// remove — one per restart-rule firing, forever, on a field that never expires (NOTES § D71).
///
/// **What the exemption costs, stated for both phases rather than the convenient one.** The
/// container whose own bad exit triggered the restart draws its card **once its record has landed
/// in `lastState`** — `kubelet_pods.go:2299-2302` propagates it there when the containerID
/// changes, because `RestartAllContainers` removes the old container instead of leaving it for the
/// kubelet to query. **Before that it has not.** In the phase a snapshot is likeliest to catch,
/// the synthesized `137` sits in *every* container's `lastState`, the trigger's included, and the
/// trigger's own `exit 3` is in `state.terminated` — **which no rule reads as an ending.** The
/// one place the current terminated state is read at all is [`doing_its_job`], and it asks only
/// whether an init container finished; rules 1, 5 and 6 take their ending from `lastState`. So
/// rule 6 is quiet on the whole pod and only [`restarting_repeatedly`]'s count remains. Still the
/// right trade, but it is a hole and not a hand-off (NOTES § D93).
///
/// **The exemption also removes a card carrying the kubelet's own sentence, and that is a side
/// effect and not the fix.** The kubelet writes it into `message` beside that reason, and the
/// log-line arm would have printed it as the container's. **What holds that shut is now
/// structural rather than two accidents**: the quote is read only inside the
/// [`Failed`](Ending::Failed) arm, so neither reason can reach it by construction rather than by
/// arm order and an early return (NOTES § D95) — and since 2026-08-16 it reaches the evidence line
/// rather than the action, so even where it is read it is no longer the card's advice
/// (NOTES § D113). **And a third reason the kubelet writes its own
/// message for would reach a frame that no longer claims an author** — [`last_words`] carries why
/// (NOTES § D88, § D93).
///
/// **And quiet on a container that is serving now, because this field never expires** — the
/// largest false-positive volume in this box, needing no unusual manifest, only uptime
/// (NOTES § D71); that history belongs to [`restarting_repeatedly`], which has a threshold under
/// it. **"Serving" is the wrong word for an init container, and [`doing_its_job`] is where that is
/// decided.**
///
/// **When the record carries a message it goes on the evidence line, and the advice is the exit
/// code's** ([`Terminated::message`], [`last_words`], NOTES § D113). The quote is printed
/// *whoever wrote it*, because the container, the kubelet and the runtime all reach that field and
/// nothing on the record tells them apart — and it is printed as **evidence**, because the action
/// is the rule's own sentence on every arm. It replaced the advice until 2026-08-16, which is how
/// a mistyped `command` came to have containerd's `runc` error as its whole *what to do*.
///
/// **This rule's command is not one command, and that is new** (invariant 4, NOTES § D113). Four
/// of its five arms carry [`describe`]; the general [`Failed`](Ending::Failed) arm — the one whose
/// action is *read the logs of that run* — carries [`previous_logs`], because it is **the one card
/// in the file where that sentence is semantically right**: its subject is a run that is over, and
/// this ending is the one whose record holds the `containerID` the kubelet gates `--previous` on.
/// It stood under `describe`, which prints no logs at all, one match arm from the sentence
/// [`crash_looping`] had just lost for the same reason. **The codes
/// [`failed_run_action`] sends to the command and arguments keeps `describe`**: that container
/// never ran, so there is no previous log to serve, and `Command:` and `Args:` are what the reader
/// is being sent to.
/// **Both exit exemptions are captured** — `exit0.json` and
/// `sigterm.json`, on containers that are *not* serving, so the exemption rather than
/// [`doing_its_job`] is what silences them — and `notfound.json` reaches the `126`–`128` action
/// with no termination message beside it, which `restarts10.json`'s bare `exit 1` and
/// `init.json`'s are the general *read the logs* arm beside. **`startup.json` reaches the `137`
/// arm**, whose kill came from outside the application: the general arm sent that reader hunting
/// an error the container never logged (NOTES § D85).
///
/// **`137` means four things and this rule draws two of them** ([`exit_meaning`],
/// [`killed_action`], NOTES § D90, § D93). `OOMKilled` and [`RESTART_ALL`] never arrive — both
/// return above — [`STATUS_LOST`] is not a kill at all and moves the title with it, because this
/// rule's own subject, *the last run on record failed*, is the one claim that shape cannot support.
/// What
/// is left is a SIGKILL whose sender the code does not name. **The role split is the point of the
/// box**: that arm handed every role *check the liveness and startup probes* while rule 5's card
/// on the same container said Kubernetes allows an init container none, and the two print
/// together (NOTES § D85).
///
/// **The translation for [`RESTART_ALL`] still reaches the screen, from rules 1 and 5**, which
/// print [`exit_fact`] whatever this rule does — so the exemption silences the card and not the
/// reading (NOTES § D93).
///
/// **The exemption list, the title and the [`STATUS_LOST`] action each read the reason string in
/// their own place until 2026-08-15** — three branches on one question that had to agree by hand.
/// They are one `match` on [`ending`] now, which is where rules 1 and 5 read the same question
/// from, so [`Unwatched`](Ending::Unwatched) reaching the quoted evidence is a shape the compiler
/// refuses rather than one arm order happens to prevent. **The class fix under it landed on
/// 2026-08-16**: [`last_log_line`] refuses the field on any record with no `finishedAt`, which is
/// what every synthesized message rides — so the two ad-hoc mechanisms that used to hide one
/// defect are no longer what holds it shut.
///
/// **What keeps every title of this rule inside `screens/alerts.md`'s three-line cap is
/// [`ending`]'s pairings, not a budget** (NOTES § D113). The title is a frame plus
/// [`exit_fact`], and of the 36 frame × translation combinations the two functions can spell,
/// **five measure four lines**. All five are unreachable — [`ending`] never pairs that frame with
/// that translation — so moving one pairing puts a card over the cap with nobody having touched a
/// word. `every_card_the_rule_set_draws_fits_the_four_caps_it_is_budgeted_for` is what would say
/// so, and it is the reason the sweep plants every `(code, reason)` pair rather than only the
/// captured ones.
///
/// **The titles say what one `lastState` supports, and that is a 2026-08-16 change.** `lastState`
/// is *the last run Kubernetes wrote down*, and on the one shape that separates the two it is not
/// the run before this one: the kubelet skips the **synthesized** write while a previous
/// termination is still there, so a container's first *lost* status stands unchanged while it goes
/// on restarting — measured at nine restarts under one entry. **On an ordinary restart the record
/// advances**, `startedAt` and `containerID` together, which is the correction of 2026-08-16
/// (NOTES § D113). So *the container's previous run failed* became *the last run on record
/// failed*, and the
/// [`Unwatched`](Ending::Unwatched) title says what the **record** does — the framing
/// [`restarting_repeatedly`]'s two clauses already used, rather than a third one.
///
/// **[`CodeUnknown`](Ending::CodeUnknown) takes an arm above the log line for
/// [`Unwatched`](Ending::Unwatched)'s reason and one more.** Nothing read how that run ended, so
/// *failed* is not the record's claim to make — and the general arm below would have sent the
/// reader after *the application's own error* on a number the application never chose, which is
/// the commonest abnormal `lastState` a cluster produces.
///
/// **No committed capture reaches [`STATUS_LOST`], [`RESTART_ALL`], [`CODE_UNKNOWN`] or the
/// `Init` side of [`killed_action`]** — the first three were measured on a kind v1.36.1 cluster
/// and never captured, the fourth needs an init container killed from outside. All are proved on
/// decoded plants off committed captures (NOTES § D40).
fn previous_run_failed(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let run = c.last_terminated.as_ref()?;
    // `OOMKilled` is rule 2's card and is exempted by its reason: it is a kill, so it stays
    // [`Ending::Failed`] for rules 1 and 5, which have nothing to say about it that
    // *something keeps killing it* does not already say (NOTES § D71, § D93).
    if run.reason.as_deref() == Some("OOMKilled") || doing_its_job(c) {
        return None;
    }
    // --- WHAT THE ENDING DECIDES START ---
    // **The exemption, the title and the action are one question and are asked once**
    // (NOTES § D93). Each read the `reason` string in its own place until 2026-08-15 — three
    // branches that had to agree by hand, in a rule whose neighbours branch on the same object
    // one function away. **The cards below are byte for byte the ones that shipped then, for
    // every object the API can produce** — and that boundary is the claim, not a hedge on it:
    // the old guard read the reason at any exit code, [`ending`] reads it only beside `137`, so
    // three pairs no kubelet writes changed answer. [`ending`] carries which and why.
    //
    // **The silences.** `exit 0` and `exit 143` are every rolling update and every scale-down;
    // [`RestartRule`](Ending::RestartRule) is a container's `restartPolicyRules` doing what they
    // were declared to do (NOTES § D96). Nothing failed in any of the three, and a WARN card for
    // a policy working correctly is the false-positive class this rule is designed around
    // (NOTES § D71).
    //
    // **[`Unwatched`](Ending::Unwatched) gets its own opening, because *failed* is false of the
    // object.** The run was never watched to an end, so the rule's own subject is the one thing
    // it may not assert: the container measured healthy either side of it on kind v1.36.1, and
    // *it failed* stood one line above a translation calling the number a placeholder and an
    // action saying nothing here says what ended the run (NOTES § D85, § D93). **Silence was the
    // other door and it was refused**: with no `lastState.terminated.containerID` the kubelet
    // will not serve `logs --previous`, so a card that said nothing would leave the reader
    // hunting a log the API refuses.
    //
    // **A branch keyed on the null `finishedAt` instead of the reason would draw the same card,
    // and that is now [`ending`]'s property rather than this rule's.** [`RESTART_ALL`] is
    // stamp-less too and its reason is not this one, so it separates the two keys exactly — it
    // just never reaches the title, because the arm above returns. Narrow that silence and the
    // keys diverge on the first object the cluster writes, which is why
    // `a_container_the_pods_own_restart_rule_removed_is_not_a_run_that_failed` is what holds the
    // ruling up. Nor is stamp-less a synonym for the reason in the kubelet at all:
    // `kubelet_pods.go:2705-2723` writes a fourth bare literal, `Completed` / `0`, for an init
    // container whose status the runtime lost. Within the reachable set the difference is
    // unobservable *here*, so no fixture is invented to prove it (NOTES § D29, § D93) — and
    // **stamp-less is load-bearing one function over**, where [`last_log_line`] uses exactly that
    // property to tell the kubelet's own literals from anything a CRI status carries. Which is
    // not the same as telling *authors* apart; [`last_words`] carries that.
    // **The quote rides the arm rather than the fact list, and only the [`Failed`](Ending::Failed)
    // arm has one** (NOTES § D113). Putting [`last_log_line`] unconditionally in the evidence
    // would give the [`CodeUnknown`](Ending::CodeUnknown) card a fact its neighbours have not —
    // CRI-O writes a message onto that ending and containerd stamps it — and
    // [`one_card_per_action`]'s subset clause would then stop collapsing the pair D102 built it
    // for. The two endings that fold add nothing; the one that does not is the one that quotes.
    let (title, (action, cmd), quote) = match ending(run) {
        Ending::Finished | Ending::Stopped | Ending::RestartRule => return None,
        // **The log line does not answer first here, and it no longer needs an arm order to stop
        // it.** The kubelet writes its own sentence into `message` beside this reason — *the
        // container could not be located when the pod was terminated* — which the arm below would
        // otherwise print as this card's whole *what to do*: a placeholder where the advice
        // belongs. The door is shut by construction, not by the order these arms are written in.
        // Nothing was killed that Kubernetes watched, so every door [`killed_action`] opens is
        // about a signal no record holds (NOTES § D90, § D93).
        Ending::Unwatched => (
            format!(
                "Kubernetes did not record how the run it last saw ended — {}",
                exit_fact(run)
            ),
            (unwatched_action().to_string(), describe(&pod.id)),
            None,
        ),
        // **Answered ahead of the [`Failed`](Ending::Failed) arm for the same reason
        // [`Unwatched`](Ending::Unwatched) is, and then some.** Nothing read how this run ended, so *the last run on record failed*
        // is a
        // claim the record does not carry — and the general arm below would send the reader after
        // *the application's own error* on a code the application never chose.
        Ending::CodeUnknown => (
            format!(
                "The last run on record has no exit code of its own — {}",
                exit_fact(run)
            ),
            (no_exit_code_action().to_string(), describe(&pod.id)),
            None,
        ),
        Ending::Failed => (
            format!("The last run on record failed — {}", exit_fact(run)),
            // **The record decides this and the cluster's own string never does, because the
            // action is k8rs's own words** (`screens/alerts.md` § The height, NOTES § D113).
            // A message on the record used to *replace* the advice, and the
            // commonest broken-pod state there is — a mistyped `command` — therefore put
            // containerd's whole `runc` error where the *what to do* belongs: seven wrapped lines
            // of *failed to create containerd task: failed to create shim task: OCI runtime create
            // failed…*, which tells a beginner nothing to do (invariant 14). It is also the one
            // input no rule author can bound, so the five-line budget was unenforceable while it
            // stood here. The quote is evidence now, on the line the screen may cut, and this arm
            // always says something in the rule's own voice.
            //
            // **[`failed_run_action`] answers this whole ending, for this rule and for
            // [`crash_looping`] and [`restarting_repeatedly`] alike** (NOTES § D113). One ending
            // answered two ways is where
            // NOTES § D85 starts, and the version of it this arm shipped put *check the memory
            // limit and the liveness probe* on the CRITICAL card over an evidence line reading
            // *the command was not found*. The command comes back with the sentence, because
            // whether the log is the answer and whether the card may name one are one decision.
            {
                let (action, log) = failed_run_action(run, c.role);
                (
                    action.to_string(),
                    if log {
                        previous_logs(&pod.id, &c.name)
                    } else {
                        describe(&pod.id)
                    },
                )
            },
            // [`last_log_line`] has already dropped the one class that is never worth printing —
            // a kubelet placeholder on a record it synthesized.
            last_log_line(run).map(last_words),
        ),
    };
    // --- WHAT THE ENDING DECIDES END ---
    // Past the silences the `reason` is the bare word `Error`, or [`STATUS_LOST`] where the
    // kubelet never watched the run end. Neither goes in the evidence: [`exit_fact`] has put both
    // into the title as a sentence already (invariant 14).
    // **[`ContainerSnapshot::restarts`] is deliberately not a fact here, and the one-clause
    // evidence a [`STATUS_LOST`] card draws is not a reason to add it.** The object carries the
    // count — `kubelet_pods.go:2628-2630` bumps it — but it counts restarts from every cause,
    // and on a card whose subject is *one* lost status it reads as *this happened N times*: the
    // incomplete-denominator class PRIOR-ART.md catalogues and this tool exists not to do. The
    // reader who needs a count gets it from [`restarting_repeatedly`], framed by a threshold.
    // **The question underneath is real and unanswered** — the object does not say whether this
    // was once or is ongoing — so the card does not claim either (NOTES § D93).
    let mut facts = vec![container_fact(c)];
    // **Ahead of the duration, the same order [`stopped_for_good`] puts it in** — two rules
    // printing one fact may not disagree about where it sits, and this is the fact that survives
    // the node (NOTES § D97, § D113).
    facts.extend(quote);
    facts.extend(ran_for(run));
    Some(Finding {
        severity: Severity::Warn,
        title,
        evidence: facts.join(FACTS),
        action,
        kubectl_cmd: cmd,
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
/// **The `started` suppressor is captured**: `startup.json` declares a `startupProbe` that never
/// passes, which is the only object that separates it from the [`ContainerState::Running`] gate.
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

/// **Rule 15 — the container has stopped, and nothing is starting it again.** CRITICAL, on
/// a container that is stopped **in the run it is sitting in now** (NOTES § D96). Four conditions,
/// all of them required:
///
/// **Present tense here as on the card, and that is a correction rather than a style** (NOTES
/// § D97). This headline read *Kubernetes **will not** start it again* while the card said the
/// same thing, and the prediction is false in a shape this rule reaches: pod `Always` with a
/// container's own `Never`, node rebooted, and the container comes back, because the kubelet reads
/// the **pod's** policy when it rebuilds a sandbox. What the four conditions below actually
/// establish is a fact about **now** — the run is over and nothing is starting another — and the
/// rule's own doc may not claim more than its card does.
///
/// | condition | why it is there |
/// |---|---|
/// | [`ContainerState::Terminated`] | the subject is `state.terminated`, the run the container is in **now** — not `lastState`, which is the run before this one and every other rule's field |
/// | [`ending`] is [`Failed`](Ending::Failed) | rule 6's exemptions, inherited whole rather than respelled: `exit 0` and `exit 143` are not faults, and the two `137` reasons the kubelet writes itself mean the container is coming back |
/// | `restarts == 0` | the false-positive guard, below |
/// | [`restart_policy`](ContainerSnapshot::restart_policy) is `Never` | the only policy that reaches this rule on a bad exit: `Always` restarts everything and `OnFailure` restarts a non-zero exit, so the truth table collapses to one arm |
///
/// **`Never` is not read as a synonym for *nothing will restart it*, and the field that speaks to
/// it most directly is not one this rule consults.** `spec.containers[].restartPolicyRules` can
/// only *add* restarts — the API rejects a `DoNotRestart` action outright — so a container declaring a retry
/// rule on its own exit code comes back under `Never`, which is KEP-5307's headline use case and
/// would be this rule's headline false positive: measured on kind v1.36.1, a pod `Never` with one
/// retry rule on `exit 3` sat in `CrashLoopBackOff` at five restarts. **The generated types carry
/// that field at the `v1_36` feature `Cargo.toml` pins** — it arrives at `v1_34` — **but nothing
/// here reads it yet**: no snapshot field names it; reading it is a box of its own (NOTES § D99).
/// **`restarts == 0` is the guard, and reading the field will not retire it** (NOTES § D97,
/// unchanged by the pin): a container that has already been restarted is not a container that will
/// not be restarted, whatever declared it — and **no cluster below 1.34 can carry the field at
/// all**, while the pin sits above the cluster on purpose (NOTES § D99), so the count is the only
/// one of the two that answers on every cluster k8rs meets. The field, when this rule learns to
/// read it, joins the count rather than replacing it. **The residual gap is one window** — the
/// first exit, before the first retry — and it is a gap rather than a bound: this rule draws a
/// card that is wrong for as long as that window lasts.
///
/// **Only a [`Regular`](ContainerRole::Regular) container reaches this rule, and by construction
/// rather than by a check** (NOTES § D96, measured). A [`Sidecar`](ContainerRole::Sidecar) *is* an
/// init container with `restartPolicy: Always`, so its effective policy is `Always` and never
/// `Never` — the fourth condition refuses it out of the same field its role came from, absolutely
/// and at every moment. A plain [`Init`](ContainerRole::Init) container that fails and will not be
/// retried takes the whole pod to `phase: Failed`, which leaves through [`finished`] before
/// [`analyze`] reaches any container — **and the door is wider than the pod-level reading, which
/// matters because this field has two** (NOTES § D97). The first draft argued it only for pod
/// `Never`; the review built the shape the effective-policy fallback opens — pod `Always` with an
/// init container declaring its own `restartPolicy: Never`, `exit 1` — the API accepts it, and the
/// pod still goes `phase: Failed`. So both readings of the fourth condition leave by the same
/// door, which is the claim this rule needs rather than the narrower one it had.
/// **That half is a settled state and not an instant**: between the
/// kubelet writing the failed status and the phase moving, a snapshot can catch such a pod still
/// `Pending`, and this rule draws its card about the init container. That card is **true** —
/// the container did stop and nothing will run it again — so it is a card that disappears rather
/// than one that was wrong, which is why the window is named here and not guarded against.
/// **So no role split and no probe is named**, and the guard `validateInitContainers` forces on
/// the other actions in this file has nothing to guard here.
///
/// **The clean-exit half of the shape is deliberately silent** (NOTES § D96, leg 7). A container
/// that exits `0` under `Never` beside a sibling that is still running is doing exactly what the
/// policy means; the fault a reader would want named there — *the Job never completes because the
/// helper is still running* — is a claim about the Job above the pod, and Jobs are not watched
/// (invariant 6). That silence is [`ending`]'s [`Finished`](Ending::Finished) arm.
///
/// **`OOMKilled` reaches this rule rather than rule 2, and that is right rather than an
/// oversight.** [`out_of_memory`] reads `lastState`, which a container that was never restarted
/// does not have, so it is structurally silent on this shape; the kill is
/// [`Failed`](Ending::Failed) here as everywhere ([`ending`]), and [`exit_fact`] puts the kernel's
/// own reading of `137` on the card. One card, and it is this one.
///
/// **The command is the first [`logs`] in this file, and the reason it may be one is the same
/// reason the rule exists**: the pod is still there and so is the container's log, with no
/// `--previous` and no error — measured. Every other ending in this file is a run that is over
/// inside a container that has been replaced, which is why they all send the reader to
/// [`describe`] instead.
///
/// **The action is not one sentence any more, and the ending is what decides its first clause**
/// (2026-08-16). Two of the three parts — *nothing is waiting to start it again* and *whatever
/// needed this container is still without it* — are true on every ending this rule draws, so they
/// are written once. The *why* clause is not: *that is where it says why it stopped* is a promise
/// only a run somebody watched end can keep, and [`Ending::CodeUnknown`] reached this rule folded
/// into the [`Failed`](Ending::Failed) arm and inherited it for one turn — an evidence line
/// saying nobody read the code, one line above an action saying the log says why.
///
/// The age is [`Terminated::finished_at`] on that run ([`Finding::timestamp`]).
fn stopped_for_good(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let ContainerState::Terminated(run) = &c.state else {
        return None;
    };
    // **Answered arm by arm rather than compared against one, and the arm now carries the
    // sentence** (NOTES § D95). The two clauses below the match are true on every ending this
    // rule draws; the *why* clause is not, so it comes from here — a fixed *read its log, that is
    // where it says why it stopped* is a promise `CodeUnknown` cannot keep, and it was shipped for
    // one turn because that ending was folded into the `Failed` arm and inherited its wording.
    let why = match ending(run) {
        Ending::Failed => "read its log — that is where it says why it stopped",
        // **The title holds up here and the promise does not**: the container was found dead, so
        // it *has* stopped, and under `Never` at zero restarts nothing is starting it again — but
        // nobody read how the run ended, so the log holds no *why* to send anyone after. What the
        // object supports is where the number came from, which is also what [`exit_fact`] has
        // already put on the evidence line.
        //
        // **A node restart is not what reaches this arm, though it is what writes the pair.** It
        // takes *every* container with it, so the sibling this rule needs to keep the pod out of
        // [`finished`]'s door goes too: measured on kind v1.36.1 with two `sleep` containers under
        // `restartPolicy: Never`, the pod came back `phase: Failed` and `analyze` dropped it
        // before any container rule ran
        // (`reports/2026-08-16-terminated-record-stamps-and-authors.md` § 4). What can reach it is
        // a **lone** loss — one container's shim gone with containerd restarting under it — which
        // leaves the sibling running and the pod `Running`.
        //
        // **Measured at the width it is drawn at**: the whole action is four wrapped lines like
        // the arm above it, which is what keeps this card at ten lines once the two-line title
        // and the three-line evidence are on it (`screens/alerts.md` § The height). A first draft
        // said the same thing in 112 characters and drew a twelve-line card.
        Ending::CodeUnknown => "the node wrote that number, not the app, so read its log",
        // Nothing failed: under `Never` a clean exit is the policy doing what it says, and `143`
        // is a shutdown asked for and obeyed.
        Ending::Finished | Ending::Stopped => return None,
        // Neither can hold this title up. `Unwatched` is a run nothing watched end, so *it has
        // stopped* is the one claim that record cannot make — rule 6's own reasoning about the
        // same reason — and a container the kubelet lost is restarted even under `Never`, its
        // record landing in `lastState`. `RestartRule` is a restart already under way by
        // definition.
        Ending::Unwatched | Ending::RestartRule => return None,
    };
    if c.restarts != 0 || c.restart_policy.as_deref() != Some("Never") {
        return None;
    }
    let mut facts = vec![container_fact(c), exit_fact(run)];
    // **Ahead of the duration on purpose, because it is the fact that survives the node.** The
    // kubelet writes this into the API server; the log the action sends the reader to lives on
    // the machine. When that machine is the reason the card is on screen at all, this clause is
    // the only thing on it that still answers — so it may not be the one the three-line cut
    // takes first (NOTES § D97, [`last_words`]).
    facts.extend(last_log_line(run).map(last_words));
    facts.extend(ran_for(run));
    Some(Finding {
        severity: Severity::Critical,
        // **Present tense, because the card may not predict** (NOTES § D97). *nothing **will**
        // start it again* is the same false promise the action carried, in the line the reader
        // reads first: pod `Always` with a container's own `Never` and a node reboot brings the
        // container back, measured. What the object supports is what is true *now* — the run is
        // over and nothing is starting another — which is also what the action says, and one card
        // may not hold two tenses about one fact (NOTES § D85).
        //
        // No policy name anywhere on this card either. `kubectl logs` prints no part of the spec,
        // and an action may name only what its own command shows (invariant 4, NOTES § D88) — so
        // the reason it is not coming back is written as the plain sentence instead.
        title: "This container has stopped and nothing is starting it again".to_string(),
        evidence: facts.join(FACTS),
        // **Two promises came out of this sentence on 2026-08-15, both measured false on a review
        // cluster** (NOTES § D97). *its log is still there* was written from one happy-path
        // measurement: [`logs`] is the only command in this file that goes to the kubelet, every
        // condition above is read from a status that **freezes when the kubelet dies**, and a card
        // measured unchanged for eight minutes past a stopped kubelet printed a command answering
        // `connection refused`. And *nothing will run it again inside this pod* is false in the
        // one shape the effective policy exists for: pod `Always` with a container's own `Never`,
        // node rebooted, and the container came back — the kubelet reads the **pod's** policy when
        // it rebuilds a sandbox.
        //
        // **What is left is what every measured shape agrees on, and it is a door rather than a
        // verdict**: nothing is *waiting* to start it, so the pod has to be replaced, and until it
        // is, whatever needed this container does not have it. Those two clauses are the card's
        // *what to do* — a finding whose last line only says what will not happen has two parts of
        // the three (NOTES § D97) — and they are true on **every** ending this rule draws, so they
        // are written once here and the ending decides only the clause that precedes them.
        action: format!(
            "{why}. Nothing is waiting to start it again, so the pod has to be replaced; until \
             it is, whatever needed this container is still without it"
        ),
        kubectl_cmd: logs(&pod.id, &c.name),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: run.finished_at.clone(),
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
/// No age ([`Finding::timestamp`]). **Two captured shapes reach the socket escalator** —
/// `hostpath.json`'s `nosy` is handed `/run/containerd`, and `socket.json` binds
/// `/var/run/docker.sock` read-only, which is the `/var/run` fold and the exact match. The
/// *sockets themselves* are still planted for the sweep over the whole list: the fixtures' cluster
/// runs containerd, so no capture off it can carry a live Docker or CRI-O socket — `socket.json`'s
/// is a `FileOrCreate` mount of the path and nothing is listening on it (NOTES § D40, § D78).
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
        kubectl_cmd: get_yaml("pod", &pod.id),
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
/// **Its positive is captured**: `wedged.json` asks for a `configMap` that does not exist, so the
/// kubelet never reaches the sandbox — `ContainerCreating` with `PodReadyToStartContainers: False`,
/// which is the storage branch of the evidence line. The `True` and absent branches stay decoded
/// copies: nothing on this cluster reaches the sandbox and then stops.
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
/// **Its positive is captured**: `unjudged.json` names `schedulerName: does-not-exist`, so the API
/// server wrote no `PodScheduled` condition at all. The empty-array framing of the same absence
/// stays a decoded copy — no server produces it.
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
        kubectl_cmd: get_yaml("pod", &pod.id),
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
///
/// **It is the one pod rule [`analyze`] calls *before* the [`finished`] gate, and that ordering
/// is deliberate** (NOTES § D96). Every other pod rule asks *is this broken now* and a pod that
/// is over is not; this one asks *did an operation somebody started finish*, and a pod held by a
/// finalizer **after** it completed is squarely its subject. From inside the gate that pod is
/// invisible — `Succeeded` plus a `deletionTimestamp` is exactly the shape a stuck finalizer
/// leaves behind.
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
        kubectl_cmd: get_yaml("pod", &pod.id),
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

/// One `status.conditions[]` entry of a node, by type — N1 and N3's whole input. Named separately
/// from [`condition`] because the node rules read a `NodeSnapshot` and not a slice; the lookup
/// itself, and the reason it is by type, are spelled there.
fn node_condition<'a>(node: &'a NodeSnapshot, type_: &str) -> Option<&'a Condition> {
    condition(&node.conditions, type_)
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

// --- THE WORKLOAD RULES START ---
//
// The W-series of NOTES § D28 — the two rules whose subject is not a pod. When the pods were never
// created there is nothing for a pod rule to iterate over, `kubectl get pods` is empty and k8rs
// reported a healthy cluster; that is the blind spot these close, and it is the one failure that
// would make the Alerts screen not believable.
//
// Both read a controller's own `status.conditions[]` and quote its message **verbatim**
// (NOTES § D37) — the quota's refusal is the whole diagnosis, and paraphrasing it is how a tool
// becomes useless.

/// **Is nothing of this workload serving?** — the split between the two severities both W rules
/// use. Nothing ready is an outage; two of three ready is a change that did not land, with the
/// workload still up, and paging for that is how a screen stops being read.
///
/// **The workload this is asked about is not always the one the rule looked at**, and W1 is where
/// that matters — see [`the_workload_that_serves`].
///
/// **`ready.unwrap_or(0)`, and that `None` is a zero**: `readyReplicas` carries `omitempty`, so
/// the API server omits it exactly when it is 0 — which is the state these rules exist for
/// ([`WorkloadSnapshot::ready`], NOTES § D53).
fn nothing_is_serving(w: &WorkloadSnapshot) -> bool {
    w.ready.unwrap_or(0) == 0
}

/// **Is this workload short of the pods it was told to run?** — W2's second gate, so a Deployment
/// that timed out and has since caught up does not keep a card that is no longer true.
///
/// **Three counters, and W2's evidence line names them back in this order** (NOTES § D82):
///
/// - `ready < desired` — pods that exist and are not passing their probes. **The only arm a
///   ReplicaSet or a StatefulSet has**: neither kind carries an unavailable counter, and a
///   ReplicaSet's `updated` counts the pods it has rather than the pods that work —
///   `broken-owned-7bdb7645c8` is one crash-looping pod of one wanted, so its `updated` equals its
///   `desired` and nothing else here would see it.
/// - `updated < desired` — pods on the current template that were **never created**. On a
///   Deployment this also covers everything the arm above does, because `unavailable == 0` there
///   can only mean the ReplicaSets have not been scaled up to `desired` yet.
/// - `unavailable > 0` — the **surge that is not landing**, which is every rollout of one replica:
///   `maxUnavailable` resolves to 0, the old pod stays up and is counted ready, one pod exists on
///   the new template, and both arms above read whole ([`WorkloadSnapshot::unavailable`]).
///
/// **The `Option`s are read in two directions, and that is not a slip.** An absent `readyReplicas`,
/// `updatedReplicas` or `unavailableReplicas` is **zero**; an absent `spec.replicas` is **one**,
/// which is what the API server defaults it to — `desired.unwrap_or(0)` would say the workload
/// wants nothing ([`WorkloadSnapshot::desired`], NOTES § D53).
///
/// **A workload that wants zero pods is not short of pods, and only the third arm has to be told**
/// (NOTES § D82). At `desired == 0` the first two are false by arithmetic — nothing is below zero.
/// The third is not derived from `spec.replicas` at all: upstream writes `unavailableReplicas` as
/// `sum(replicaset.spec.replicas) - availableReplicas`, floored at zero, and the user writes
/// `spec.replicas` — two fields, two authors, no shared instant. So `kubectl scale --replicas=0`,
/// which is how a broken rollout is stopped, puts an explicit `Some(0)` beside the status of a
/// moment ago, `unavailable: 1`; and W2's other two gates still pass, because a
/// `ProgressDeadlineExceeded` written before the scale-down is sticky. Ungated, that is a CRITICAL
/// card about a workload the user has just deliberately turned off. **The gate is written in front
/// of all three anyway**, because that is where the sentence above is stated once rather than
/// hidden inside the arm that happens to need it; the first two pass through it unchanged.
fn short_of_pods(w: &WorkloadSnapshot) -> bool {
    let desired = w.desired.unwrap_or(1);
    desired > 0
        && (w.ready.unwrap_or(0) < desired
            || w.updated.unwrap_or(0) < desired
            || w.unavailable.unwrap_or(0) > 0)
}

/// **`0 of 1 pod ready`** — the count both W rules print, spelled once. Its subject is always the
/// object in the card's header: W2 reads the Deployment it fired on, W1 the workload above the
/// ReplicaSet that was refused ([`the_workload_that_serves`], NOTES § D82).
fn ready_count(w: &WorkloadSnapshot) -> String {
    format!(
        "{} of {} ready",
        w.ready.unwrap_or(0),
        counted(i64::from(w.desired.unwrap_or(1)), "pod")
    )
}

/// **The controller's own sentence, framed so it is not read as k8rs's** — the prefix says a
/// machine wrote what follows, the way rule 10 frames the scheduler's and N1 the kubelet's
/// (NOTES § D37). The quote is never paraphrased: on W1 the API server's refusal *is* the whole
/// diagnosis.
///
/// Both W rules fire on conditions upstream always writes with a message, so the fallback is a
/// shape no cluster has been seen to produce — and an empty evidence line is a blank row on the
/// card, which is worse than saying there was nothing to quote.
fn controller_said(c: &Condition) -> String {
    match c.message.as_deref() {
        Some(m) => format!("the reason Kubernetes gave: {m}"),
        None => "Kubernetes recorded no reason for it".to_string(),
    }
}

/// **The workload whose readiness decides W1's severity — the Deployment above the ReplicaSet,
/// not the ReplicaSet itself** (NOTES § D82).
///
/// A refused *rollout* leaves the new ReplicaSet reading `0 of N` forever while the old one carries
/// every request, so a band read off the refused object pages CRITICAL for a service that is
/// wholly up — and there are no pods under that ReplicaSet, so nothing else on the screen
/// contradicts it. Reading past its own object is the shape [`no_node_accepted_it`] already has.
///
/// **A ReplicaSet nothing controls answers with itself**, because its own `owner` is itself
/// ([`owner_of`]) — which is the right answer there: a bare ReplicaSet is the whole workload.
///
/// **An owner that is named and absent is `None`, and that is the direction this fails in on
/// purpose** — the same direction [`workload_owner`] fails in, one screen down. Absent is
/// *unknown*, not *down*, and it arises: an Argo `Rollout` owns its ReplicaSets directly, which
/// [`ObjectKind::from_api`] resolves to an `Other(_)` no watch decodes and invariant 12 forbids
/// decoding; a 403 on `deployments` with `replicasets` still readable does the same. Falling back
/// to the ReplicaSet the rule looked at would read the `readyReplicas: 0` it has **by definition
/// of having been refused** and page CRITICAL for a canary while the stable version serves every
/// request — with no pods under it for the rest of the screen to contradict.
fn the_workload_that_serves<'a>(
    snapshot: &'a ClusterSnapshot,
    w: &'a WorkloadSnapshot,
) -> Option<&'a WorkloadSnapshot> {
    snapshot.workloads.iter().find(|o| o.id == w.owner)
}

/// **W1 — the pods were never created.** `ReplicaSet.status.conditions[ReplicaFailure]` at `True`
/// with reason `FailedCreate`, and the API server's refusal shown word for word (NOTES § D28,
/// § D37). CRITICAL while nothing of the workload is serving, WARN while something still is.
///
/// **A ReplicaSet and nothing else, and that gate is load-bearing rather than defensive**: the
/// Deployment controller copies `ReplicaFailure` up onto the Deployment as well —
/// `tests/fixtures/deployments.json` carries the identical message on `broken-quota` — so a rule
/// that read every workload would file two cards on one refusal, which is exactly what D28 forbids.
///
/// **`FailedCreate` and nothing else, and that is a ruling rather than an oversight**
/// (NOTES § D82). Upstream's `replica_set_utils.go` writes this condition under two reasons, and
/// the other is `FailedDelete` — a scale-*down* the API refused, which every sentence on this card
/// is wrong about: nothing was refused creation, the counters read *"2 of 1 pod"*, and the service
/// is up. A card for it would be a new rule and is not in v1 (invariant 13).
///
/// **The count on the evidence line is the header's**, not the refused ReplicaSet's: the card
/// files under the Deployment, the band is read off the Deployment, and `0 of 1 pod ready` about
/// it is one object on the line where the ReplicaSet's own count beside that band was two
/// (NOTES § D82). **A workload that cannot be resolved prints no count rather than borrowing
/// one** — and there is no count from anywhere else to fall back on, because `Finding::object` is
/// a ReplicaSet and D69's `n of m` counts pods.
///
/// The card files under the ReplicaSet's **owner**, so the reader sees the name they deployed and
/// not a hashed one ([`WorkloadSnapshot::owner`]).
///
/// **The action names the three causes rather than pointing at the quote**, because the quote is
/// an `Option` and an action that referred to it would be wrong on the card that has none.
///
/// **This card outlives its cause by minutes and there is no fix here.** Upstream's
/// `calculateStatus` clears `ReplicaFailure` only on a resync that succeeds, and the ReplicaSet
/// controller's rate limiter backs off to about sixteen minutes — a card was seen standing eight
/// seconds after the quota that caused it was removed, and it would have stood far longer. The
/// screen has no manual refresh to offer, so the honest thing is that the reader knows: this is a
/// condition the controller has not revisited yet, not a refusal happening now.
fn pods_were_never_created(snapshot: &ClusterSnapshot, w: &WorkloadSnapshot) -> Option<Finding> {
    if w.id.kind != ObjectKind::ReplicaSet {
        return None;
    }
    let failure = condition(&w.conditions, "ReplicaFailure")
        .filter(|c| c.status == "True" && c.reason.as_deref() == Some("FailedCreate"))?;
    let serves = the_workload_that_serves(snapshot, w);
    Some(Finding {
        severity: if serves.is_some_and(nothing_is_serving) {
            Severity::Critical
        } else {
            Severity::Warn
        },
        // **Not *"the pods were never created"***, which is D28's name for the rule and not a
        // sentence that survives the amber band: two of three running and the third refused is the
        // same condition, and a title saying none exist is a card that lies about the count
        // printed directly under it.
        title: "Kubernetes refused to create the pods this workload asked for".to_string(),
        evidence: match serves {
            Some(o) => [ready_count(o), controller_said(failure)].join(FACTS),
            None => controller_said(failure),
        },
        action: "find what refused them — usually a quota that is full, a policy that rejected \
                 the pod, or a volume that does not exist"
            .to_string(),
        kubectl_cmd: get_yaml("replicaset", &w.id),
        owner: w.owner.clone(),
        object: w.id.clone(),
        timestamp: failure.last_transition.clone(),
    })
}

/// **The workload a finding is really filed against, one step further up than its own `owner`.**
/// A pod's controller is its ReplicaSet while W2's subject is the Deployment above it, so the chain
/// is walked through the snapshot's own ReplicaSets — which is where `k8s.rs` puts them, fetched on
/// demand rather than watched (NOTES § D28). Anything the snapshot does not carry answers with
/// itself, and so does anything that is already at the top, which is where W1's owner already sits.
///
/// **A ReplicaSet the snapshot does not hold answers with itself, and that is the direction this
/// fails in on purpose.** An owner that cannot be resolved is *unknown*, not *related*: reading
/// unknown as related would let one pod's crash loop, anywhere in the snapshot, silence every W2 in
/// it. The cost of failing the other way is a second card saying the same thing; the cost of this
/// way is the silence the whole W-series exists to end (NOTES § D28).
///
/// **One hop, and only ever off a ReplicaSet**: the chain D28 describes is exactly
/// Pod → ReplicaSet → Deployment, and the kind gate is what holds the code to that sentence
/// (NOTES § D82). Hopping off whatever workload was found instead walks off the top of the chain
/// the moment a Deployment carries a controlling `ownerReference` — which the Deployments ECK,
/// OLM and most operators emit all do — landing on the operator's CR, so W1's card and W2's stop
/// resolving to the same object and one refusal draws two cards, the second headed with a name
/// its own `kubectl` line does not mention.
fn workload_owner<'a>(snapshot: &'a ClusterSnapshot, owner: &'a ObjectId) -> &'a ObjectId {
    if owner.kind != ObjectKind::ReplicaSet {
        return owner;
    }
    snapshot
        .workloads
        .iter()
        .find(|w| w.id == *owner)
        .map_or(owner, |w| &w.owner)
}

/// **Does this finding say why pods are not ready?** — the filter on what may silence W2
/// (NOTES § D28, § D82).
///
/// D28's clause is *"no pod-level finding already **explains** the shortfall"*, and the list of
/// every finding is not that list. Rule 5's amber card says in its own sentence that the container
/// **is serving now**; rule 8's hostPath card, rule 12's and rule 14's are about pods that answer
/// requests too. Any of them silencing a Deployment whose rollout is dead is a screen that hides
/// the outage behind the note beside it.
///
/// **The discriminator is the pod's `Ready` condition, because that is the same arithmetic the
/// shortfall is measured in**: `status.readyReplicas` counts pods whose `Ready` is `True`, so a
/// pod that is not counted there is exactly a pod that is part of the shortfall.
///
/// **Not [`doing_its_job`], which is rule 5's discriminator and is per *container*.** It reads a
/// pod with no container statuses at all as serving — and that is precisely rules 10 and 14's
/// shape, the unscheduled pod, which is the most common true explanation of a rollout that never
/// finished.
///
/// **Anything that is not a pod passes**, which is what keeps W1 suppressing W2: one quota
/// refusal is one card, and the object it is filed on is a ReplicaSet.
fn explains_a_shortfall(snapshot: &ClusterSnapshot, f: &Finding) -> bool {
    !snapshot
        .pods
        .iter()
        .any(|p| p.id == f.object && p.ready.as_ref().is_some_and(|c| c.status == "True"))
}

/// **W2 — the rollout gave up.** `Deployment.status.conditions[Progressing]` with reason
/// `ProgressDeadlineExceeded`, while the counters still show a shortfall (NOTES § D28). CRITICAL
/// while nothing is serving, WARN while the previous version still is.
///
/// **Three gates, and the third is the rule's whole character.** It is a Deployment, because that
/// is the only kind that has this condition and the only one whose resource word the command below
/// can be sure of. It is [`short_of_pods`], because the condition outlives the timeout it recorded
/// and a rollout that caught up is not a card. And **nothing on the list already explains the
/// shortfall** ([`explains_a_shortfall`]) — otherwise a crash-looping Deployment produces a pod
/// card *and* this one, for one problem, and the reader stops trusting the count at the top of the
/// screen.
///
/// **Only `reason` is read, where W1 reads `status` too, and the asymmetry is safe rather than
/// tidy.** Upstream's `NewDeploymentCondition` writes status, reason and message in one call, so
/// `ProgressDeadlineExceeded` never arrives on a condition that is still `True`.
///
/// **A Deployment merely short of pods says nothing here.** Every `kubectl apply` is short of pods
/// for a while; `progressDeadlineSeconds` is Kubernetes' own answer to when that stops being normal,
/// and this rule reads that answer rather than inventing a second one.
///
/// **The evidence names the counter that is actually short, in [`short_of_pods`]' own order**, so
/// the number on the card is always the one the band was read off (NOTES § D82). `ready < desired`
/// comes first and takes every CRITICAL with it — nothing serving is nothing ready. *"On the new
/// version"* is therefore printed only where `ready >= desired`, which is exactly where an old
/// version demonstrably still is serving, and the third line is reached only when both counters
/// read whole and the surge is what is missing — where it is never `0 pods`, because a workload
/// with nothing unavailable did not get past the gate.
///
/// **The action names no command.** `kubectl rollout undo` errors on a single-revision Deployment
/// — *"no rollout history found"*, which is the shipped `broken-quota` fixture exactly — and on a
/// paused one, and this card cannot tell either apart without a field the contract does not carry.
/// An action line that names a command is under invariant 4's honesty rule as much as
/// `kubectl_cmd` is.
///
/// `explained` is every finding that explains a shortfall, each resolved through
/// [`workload_owner`] — built once by [`analyze`] rather than here, because it is a query over the
/// whole list. **Membership is identity, not [`ObjectId::group_key`]**: a Deployment deleted and
/// recreated under the same name is a different object, and its predecessor's pods explain nothing
/// about its rollout (NOTES § D38).
fn rollout_gave_up(w: &WorkloadSnapshot, explained: &[&ObjectId]) -> Option<Finding> {
    if w.id.kind != ObjectKind::Deployment {
        return None;
    }
    let progressing = condition(&w.conditions, "Progressing")
        .filter(|c| c.reason.as_deref() == Some("ProgressDeadlineExceeded"))?;
    if !short_of_pods(w) || explained.contains(&&w.id) {
        return None;
    }
    let desired = w.desired.unwrap_or(1);
    let updated = w.updated.unwrap_or(0);
    Some(Finding {
        severity: if nothing_is_serving(w) {
            Severity::Critical
        } else {
            Severity::Warn
        },
        title: "This rollout gave up — Kubernetes has stopped waiting for it to finish".to_string(),
        evidence: [
            if w.ready.unwrap_or(0) < desired {
                ready_count(w)
            } else if updated < desired {
                format!(
                    "{updated} of {} on the new version",
                    counted(i64::from(desired), "pod")
                )
            } else {
                format!(
                    "{} not answering",
                    counted(i64::from(w.unavailable.unwrap_or(0)), "pod")
                )
            },
            controller_said(progressing),
        ]
        .join(FACTS),
        action: "find out why the new pods will not start, then fix the deployment and apply it \
                 again — or put the version that worked back"
            .to_string(),
        kubectl_cmd: get_yaml("deployment", &w.id),
        owner: w.owner.clone(),
        object: w.id.clone(),
        timestamp: progressing.last_transition.clone(),
    })
}
// --- THE WORKLOAD RULES END ---

// --- THE CERTIFICATE RULES START ---
//
// The C-series of NOTES § Certificate rules. **C1 is the only one that needs no cluster at all** —
// its whole input is the kubeconfig, which is why it is the one rule that still answers when every
// other one has nothing to read.
//
// **It is also the one finding with no API object behind it**, so its identity is spelled out
// rather than decoded: `Other("kubeconfig")`, no namespace, the context name, and the only `None`
// uid in the product (NOTES § D39, § D51).
//
// **Nothing off the certificate reaches a string here except a moment this file formatted
// itself.** The subject, the issuer and the extensions are never read, so the only untrusted text
// on this card is the context name — which arrives from the kubeconfig through `k8s.rs` and is
// stripped there with every other name (invariant 9, Phase 5).

/// **How close to running out the kubeconfig's client certificate has to be before C1 says
/// so** — thirty days (NOTES § Certificate rules), which is a working notice period rather than a
/// tuned number: long enough to ask a human who is on holiday, short enough not to sit on the
/// screen for a quarter.
///
/// Spelled in hours because that is what a `const SignedDuration` counts in.
const CERT_EXPIRY_WARN: SignedDuration = SignedDuration::from_hours(30 * 24);

/// **When this certificate stops being accepted, or nothing at all** — the whole of C1's parsing,
/// and every way it can fail answers the same way (invariant 5: no `Result`, and a panic in here
/// takes the tool down at startup).
///
/// Three of those ways are ordinary — the field is not a certificate, it is a truncated one, the
/// PEM is something else wearing a certificate's file name — and **the fourth is a decision**:
/// RFC 5280 §4.1.2.5 spells *"no well-defined expiry"* as `99991231235959Z`, which is past the end
/// of jiff's `Timestamp` range, so the conversion answers `Err`. **A certificate that never expires
/// has no expiry to warn about, so it produces no finding** (NOTES § D56).
///
/// **The label is checked rather than trusted to fail later.** A `PRIVATE KEY` block would not
/// parse as a certificate anyway, but *would not parse* is an accident of the parser, where
/// **this file reads certificates and nothing else** is the property invariant 8 wants — and the
/// file that carries the certificate is the file the key sits beside.
///
/// **The bytes are assumed to be bounded already.** This reads whatever slice it is handed;
/// refusing a kubeconfig big enough to matter belongs to the read, which is `k8s.rs`'s in Phase 5
/// (CLAUDE.md § Security gate — *sizes are bounded*).
fn expires_at(pem: &[u8]) -> Option<Timestamp> {
    // Only the first block, which is the leaf: a kubeconfig that carries a chain writes the
    // client's own certificate first, and it is the one whose expiry locks the user out.
    let (_, block) = parse_x509_pem(pem).ok()?;
    if block.label != "CERTIFICATE" {
        return None;
    }
    let certificate = block.parse_x509().ok()?;
    Timestamp::from_second(certificate.validity().not_after.timestamp()).ok()
}

/// **Whole days, in the words this card prints** — `22 days`, `1 day`, and **`less than a day`
/// where a truncated `0 days` would be both wrong and the most urgent thing C1 ever says**.
///
/// The sign is dropped: the caller's sentence carries the direction — *expires in* one way,
/// *expired … ago* the other — and the same length has to read correctly in both.
fn in_days(span: SignedDuration) -> String {
    let days = span.as_hours().abs() / 24;
    if days == 0 {
        "less than a day".to_string()
    } else {
        counted(days, "day")
    }
}

/// **C1 — the login on this machine is running out, or has run out.** The kubeconfig's client
/// certificate, reported from [`CERT_EXPIRY_WARN`] onward and as a failure once it is past
/// (NOTES § Certificate rules, § D51).
///
/// **The one rule in this file whose severity decides which *screen* the card appears on rather
/// than only how loud it is** (NOTES § D87). [`Severity::Info`] already means *this finding lives
/// in a report, not in Alerts* — N4 and N5 use it exactly that way — so the expiring band takes
/// that door to the Certificates report D2 sent it to, and only the expired band reaches Alerts,
/// as `Critical`, because being locked out this second is broken-now by D2's own dividing line.
/// Collapsing the two into one band is the change D87 forbids.
///
/// **The one card whose subject is the reader's own laptop**, and the evidence line says so:
/// nothing in the cluster is broken, and no amount of looking at the cluster will show this. It is
/// also **the one card no `kubectl` command teaches** — `kubectl config view` prints the
/// certificate's *path*, never its dates, and `kubeadm certs check-expiration` reads files on a
/// control-plane node's disk that a laptop does not have (NOTES § Certificate rules). So
/// `kubectl_cmd` is `None` in its documented sense — *no such command exists* — rather than as an
/// omission (invariant 4, [`Finding::kubectl_cmd`]).
///
/// **`timestamp` is `None`, and `notAfter` is the field it is not.** The field is the moment the
/// event on the card happened; a certificate's expiry is a deadline, future-dated by nature, and
/// [`age`] refuses it on purpose (NOTES § D69). The past-dated half is refused for the same
/// reason it is refused on rule 8: an expired credential is a standing property of a file, and the
/// two bands of one rule may not draw a right edge on one card and a blank on the other.
///
/// **`None` with no current context, which is a real state rather than a defensive one**
/// (NOTES § D51): the name on this card is the one the user recognises, and there is no second
/// name to fall back to that would not be invented. A kubeconfig with no current context is also
/// one k8rs cannot connect with at all, so the screen the reader is on says so already.
///
/// **A certificate that is not valid *yet* is deliberately not modelled** — that is a third state
/// with its own sentence, no fixture reaches it, and `scripts/certs-test.sh` asserts the committed
/// three are all past their `notBefore` so it cannot arrive by accident.
fn kubeconfig_certificate_expiring(snapshot: &ClusterSnapshot) -> Option<Finding> {
    let context = snapshot.context.as_deref()?;
    let expires_at = expires_at(snapshot.client_certificate.as_deref()?)?;
    let left = expires_at.duration_since(snapshot.now.0);
    if left > CERT_EXPIRY_WARN {
        return None;
    }
    // RFC 5280 §4.1.2.5: the certificate is valid *through* `notAfter`, so the deadline itself is
    // still inside the window and only what is past it has run out.
    let expired = left < SignedDuration::ZERO;
    let id = ObjectId {
        kind: ObjectKind::Other("kubeconfig".to_string()),
        namespace: None,
        name: context.to_string(),
        uid: None,
    };
    Some(Finding {
        severity: if expired {
            Severity::Critical
        } else {
            Severity::Info
        },
        title: if expired {
            format!(
                "Your kubeconfig certificate expired {} ago — the cluster is refusing you",
                in_days(left)
            )
        } else {
            format!("Your kubeconfig certificate expires in {}", in_days(left))
        },
        evidence: format!(
            "{} until {expires_at}{FACTS}this is the file on your own machine that proves who you \
             are — nothing in the cluster is broken",
            // Tense, because *valid until 2026-08-09* on a red card reads as though it still is.
            if expired { "was valid" } else { "valid" }
        ),
        action: if expired {
            "ask whoever gave you access for a new kubeconfig — k8rs cannot renew it, and kubectl \
             has stopped working for you too"
        } else {
            "ask whoever gave you access for a new kubeconfig before that date — k8rs cannot renew \
             it, and after it kubectl stops working for you too"
        }
        .to_string(),
        kubectl_cmd: None,
        owner: id.clone(),
        object: id,
        timestamp: None,
    })
}
// --- THE CERTIFICATE RULES END ---

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
