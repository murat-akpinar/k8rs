//! The rule engine — the bottom of the pyramid. Pure functions over a snapshot:
//! no network, no terminal, no globals, no `Result`, no clock call — the snapshot
//! carries `now`, so a fixture cannot expire (CLAUDE.md invariant 5).
//!
//! The contract `rules.rs`, `views.rs` and the `--once` printer meet on. The
//! rules, the snapshot types and the timestamp are later boxes of Phase 3.

// Nothing constructs a `Finding` yet. `expect` rather than `allow` because it expires by
// itself — but not necessarily at this file's freeze: `Severity::Info` has no producer
// here today (`analysis.rs` gets one in Phase 4; whether N4 adds one here is N4's box),
// and while one item stays unconstructed the expectation still holds, so deleting the
// line then turns the build red instead. It expires when the *last* item here is
// constructed — possibly phases later, surfacing as `-D warnings` pointing into a frozen
// file — and whichever box does that deletes this line: pre-authorised, not a freeze
// violation. `--all-targets` evaluates it per target and the test target already
// constructs `Info`, so bin and test can flip at different boxes. Module-wide blind spot
// accepted in NOTES § D38.
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

/// How bad it is. **Declaration order is severity order** — the derived `Ord` sorts
/// the Alerts list and `--once`, so reordering these variants reorders every screen
/// with no other symptom, which is why a test asserts it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Broken now: something is not doing its job and someone has to answer it.
    Critical,
    /// Wrong now, broken soon. It still needs an answer, just not this minute.
    Warn,
    /// Worth knowing; nothing here is broken. **No rule reaching the Alerts list produces
    /// one** — NOTES § D2 sends those to a report; a rule can live in this file and still
    /// be `Info` (N4, kubelet skew → the Versions report). Both files share this scale.
    Info,
}

/// The kind of Kubernetes object a finding names (NOTES § D36).
///
/// **An `ownerReference` of kind `Node` is discarded, never carried into `owner`.**
/// kubelet writes one onto every static (mirror) pod, so on kind and any kubeadm cluster
/// `etcd-*`, `kube-apiserver-*`, `kube-scheduler-*` and `kube-controller-manager-*`
/// carry one; kept, they lose `kube-system`, collapse onto one card, and draw as a
/// machine — `views.rs` picks the card shape from `owner.kind`. A mirror pod files
/// under **itself**: `owner.kind` is `Pod` and `owner == object`, so the card stays
/// `kube-system/etcd-k8rs-control-plane`. `ObjectKind::Node` appears in the `owner`
/// role **only** when the finding is about the node itself — N1–N3, where
/// `owner == object`.
///
/// Beyond D3's four kinds: `CronJob`, because filing its pods under the Job whose
/// name carries the schedule tick is identity churn — the card's name changes every
/// tick and its age resets with it, so a failure six days old never looks older than
/// one tick. `ReplicaSet` and `Other`, because the owner chain genuinely stops at
/// each: `--cascade=orphan` for the first, an Argo Rollout for the second.
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
    /// `Other("StatefulSet.apps.kruise.io")`, `Other("Rollout.argoproj.io")` — **or**, for
    /// the one subject with no API object behind it, what the thing is: rule C1's identity
    /// is `Other("kubeconfig")`, namespace `None`, name = the kubeconfig **context
    /// name** (the identifier the user recognises), uid `None`.
    ///
    /// **`Kind.group` is not how `kubectl` spells a resource** — that is the lowercase
    /// *plural* plus the group, `statefulsets.apps.kruise.io`. It is merely *accepted* as
    /// an argument, because the RESTMapper registers each singular as
    /// `strings.ToLower(kind)` and lowercases the argument before matching
    /// (`restmapper.go`, `coerceResourceForMatching`). Nothing breaks here; the
    /// consequence is Phase 7's — **a `kubectl_cmd` built from an `Other(_)` must
    /// lowercase it**, or invariant 4's teaching device prints a form the user will not
    /// find in any documentation.
    Other(String),
}

/// One Kubernetes object, identified the way a human would identify it. Every
/// `Finding` carries two: the one it is filed under and the one it is about.
///
/// Two questions, two mechanisms (NOTES § D38): *are these the same object?* is the
/// derived `Eq` over all four fields, uid included, which D22's confirm dialog asks;
/// *which card is this?* is [`ObjectId::group_key`], uid excluded — D3's one card per
/// owner.
///
/// `Hash` is **deliberately not derived**: keying a map on the whole identity stops
/// compiling, so the wrong grouping key is unrepresentable rather than discouraged.
/// The case it stops is a Deployment deleted and recreated under the same name — its
/// two generations differ only in uid, and over four fields that is two cards for one
/// workload.
///
/// **The error arrives one line later than you expect, with bad advice.**
/// `HashMap<ObjectId, _>` *declares* fine — the `Hash` bound sits on
/// `insert`/`get`/`entry`, not on `HashMap::new` — and when it fires rustc says
/// `help: consider annotating ObjectId with #[derive(Hash)]`, offering the two-cards
/// bug as the fix. Add `group_key()` to the call, not `Hash` here — except when the
/// call is *counting*, where the answer is a `Vec`, not a key (see [`Finding::object`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectId {
    pub kind: ObjectKind,
    /// `payments` in `payments/web`. `None` means cluster-scoped, an `Option` because
    /// `""` is a lie that reaches the screen: it draws as `/node-3` and builds
    /// `kubectl describe pod node-3 -n ""` — a command that does not work, printed in
    /// the record invariant 4 says may not lie.
    pub namespace: Option<String>,
    /// The name, read per role. In `owner`: the controller's name, resolved up to the
    /// Deployment when there is one and stopping at the ReplicaSet when the chain does,
    /// where the hashed name is the honest answer. In `object`: the object's own name —
    /// W1's object is a ReplicaSet, so `broken-quota-59654c756` belongs there.
    /// Resolving one to the other is `k8s.rs`'s job (Phase 5, NOTES § D28).
    pub name: String,
    /// The object's UID, so a confirmation cannot act on the object that replaced the
    /// one the user selected (NOTES § D22) — in the Alerts view the selected object
    /// *is* a `Finding`, so the uid must survive inside one.
    ///
    /// **A workload owner always carries a uid**, since it is a required field of
    /// `metav1.OwnerReference`; rule C1's kubeconfig certificate is the only `None`,
    /// and a `None` on a workload silently disables D22.
    ///
    /// **The members of a group must agree on the *owner's* uid** — never on their own;
    /// the pods on a card are different pods. [`ObjectId::group_key`] merges owner uid-A
    /// and uid-B onto one card, so when the targeted card holds both, the dialog refuses
    /// and offers a re-read rather than picking one (Phase 7/9, this file freezes first).
    ///
    /// **`resourceVersion` is deliberately not a field.** It belongs to the moment
    /// the dialog opens, not the moment the rule ran, so a rule-time copy is stale
    /// by construction and would turn the 409 conflict check into a guess.
    pub uid: Option<String>,
}

impl ObjectId {
    /// The identity findings are grouped by: kind, namespace, name — **not** the uid.
    /// One card per owner (NOTES § D3), decided here rather than in `views.rs`, where
    /// a second definition would drift from this one.
    pub fn group_key(&self) -> (&ObjectKind, Option<&str>, &str) {
        (&self.kind, self.namespace.as_deref(), &self.name)
    }
}

/// One thing that is wrong, in three parts: what happened · what it means · what to do.
///
/// **Every string reachable from here is untrusted, identities included** — invariant
/// 9's own example is a crafted *name* rewriting the terminal. Nothing here strips
/// control characters **and nothing downstream does either yet**; the first code to
/// show a `Finding` is this phase's last box, the temporary `main.rs`, and where the
/// guard goes is its decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// How bad it is, and therefore where it lands in the list.
    pub severity: Severity,
    /// **What happened**, translated: "Containers exceeded their memory limit and
    /// were killed by the kernel", not `OOMKilled` printed and left (invariant
    /// 14). The raw reason may follow in brackets; it never replaces the sentence.
    pub title: String,
    /// **What it means** — the fields and numbers that prove the title:
    /// `limit 256Mi · exit 137 · 47 restarts`. A controller's own status message is
    /// quoted **verbatim** (NOTES § D37); what is absolute is what k8rs *fetches* —
    /// never Secret data, never an environment variable value. The type cannot enforce
    /// that; rule authors do.
    pub evidence: String,
    /// **What to do** about it, in one line the reader can act on.
    pub action: String,
    /// The `kubectl` command that shows the same thing, as the user would have typed
    /// it — the teaching device (invariant 4). Display text only: k8rs never executes
    /// it and never feeds it back into a process. `None` means **no such command
    /// exists** — never "the rule author had not got round to it". C1 reads a
    /// certificate off local disk and no kubectl line shows that; with a bare `String`
    /// the two cases are one value no test can tell apart.
    pub kubectl_cmd: Option<String>,
    /// **The grouping key** — what this finding is filed under. One card per owner,
    /// never per pod: a DaemonSet unhappy on forty nodes is one finding reading
    /// "3 of 40 pods" (NOTES § D3). `rules.rs` decides this identity; `views.rs` does
    /// the grouping. It equals `object` whenever nothing controls the subject **or its
    /// controller is a Node and is discarded**: a bare pod, a node the finding is about
    /// (N1–N3), **a mirror pod — kubelet does set `Controller: true` on that Node
    /// reference, and it is dropped anyway** (see [`ObjectKind`]), and rule C1, where
    /// both are the kubeconfig `ObjectId`.
    pub owner: ObjectId,
    /// **What the finding is actually about** — whatever the rule looked at, which is
    /// not always a pod: a ReplicaSet for W1, a Deployment for W2, a node for N1–N3.
    ///
    /// **The numerator of D3's "3 of 40 pods" is the number of distinct `object`s in
    /// the group whose `kind` is `Pod`, and a group with none of those has no
    /// `n of m` at all** — the shape `screens/alerts.md` already gives a node card.
    /// This is the whole spec `views.rs` (Phase 9) gets, so both halves are load-bearing:
    ///
    /// - *distinct objects, not findings*: `tests/fixtures/crashloop.json` is one pod
    ///   satisfying rules 1 and 6 at once — the loop it is in, and how the run before
    ///   this one ended — so counting findings draws "2 of 5 pods" for a single sick
    ///   pod, in the direction that teaches a beginner not to believe the screen.
    ///   `oom.json` is the same shape through rules 1 and 2, and a pod can reach three
    ///   without being unusual. **The number here was 4 and named rules 1, 5, 6 and 7**;
    ///   both were wrong, and each in a way worth keeping written down, because the same
    ///   two mistakes are available to the next rule anyone adds. Rule 7 requires
    ///   [`ContainerState::Running`] and a container in a crash loop is `Waiting`, so it
    ///   was never in this list; rule 5 stays quiet on a container rule 1 is already
    ///   describing, one incident being one card.
    /// - *`Pod` objects only*: W1's object is a ReplicaSet
    ///   (`tests/fixtures/quota-replicasets.json` — `ReplicaFailure`,
    ///   `status.replicas: 0`), so counting it prints "1 of 0 pods", the failure class
    ///   D28 added the workload watch to stop; counting a Deployment as one of its own
    ///   pods breaks it the other way.
    ///
    /// **Distinct is the whole `object`, uid included**: `ObjectId` has `Eq` and no
    /// `Hash`, so a `Vec` and a linear `contains`, not a `HashSet` of `group_key()`.
    /// The two dedups provably agree here — the count is `Pod`-only above, and one
    /// namespace cannot hold two pods of one name — so this catches no divergence and
    /// states an intent instead: `group_key()` answers *which card*, the whole identity
    /// *what is counted on it*, and only the second survives counting anything else.
    ///
    /// The denominator is not here — it is the group's total pod count, from the
    /// snapshot. This is also the only source for the detail view's first act
    /// (`screens/detail.md`: "`⏎` first lists *which* pods are affected").
    pub object: ObjectId,
    /// **When the event this card is about happened — the moment, never the phrase.**
    /// "4 min ago" is computed at draw time, and what a renderer calls for a finding is
    /// [`Finding::age`] — never the free [`age`] on this field. `ui.rs` (Phase 11) and the
    /// `--once` printer make that one call, so the two cannot disagree about the same
    /// finding (NOTES § D18, `screens/once.md`), and a rule test asserts a duration
    /// instead of parsing English back into a number.
    ///
    /// **A rule may fill this only from a field that records the event itself**, and
    /// which field that is has one answer per rule, not one per author. A timestamp is
    /// always *available* somewhere near an object — that is what makes this worth
    /// spelling out, because the wrong one is never missing, it is three lines away and
    /// it draws:
    ///
    /// | rule | the field | the one it is not |
    /// |---|---|---|
    /// | 1, 2, 6 | [`Terminated::finished_at`] on [`ContainerSnapshot::last_terminated`] — when the run ended | `started_at`, one line above it in the same struct, which is when that run *began* |
    /// | 7 | the **later** of [`PodSnapshot::ready`]'s `last_transition` and the container's own `started_at` — see below | [`PodSnapshot::scheduled`]'s, three lines away: a pod up six days that went unready four minutes ago would read `6 days ago`, and that is the number someone correlates with a deploy |
    /// | 12 | `deletionTimestamp − grace` — the moment the user asked (NOTES § D46) | the `deletionTimestamp` itself, which is the deadline and is short by exactly one grace period, forever |
    /// | N2 | the cordon taint's [`Taint::added_at`] (NOTES § D65), with the caveat below | — |
    /// | N3 | *that* condition's `last_transition` — DiskPressure's, PIDPressure's | `Ready`'s, off the same flat [`NodeSnapshot::conditions`] `Vec`, which dates the card to the node's boot |
    /// | N6 | the pod's `scheduled` `last_transition` — N6's subject is the Pending pod | the blocking node's taint `added_at`: it is stamped, it is nearby, and it answers when the *node* changed |
    ///
    /// The table is the rules whose wrong field is closest to hand, not the whole rule
    /// set. A rule that is not in it owes the same answer — the moment its own event
    /// happened, or `None` — and owes it in a test, which is the only place the
    /// distinction between "the right field" and "a field that renders" is visible.
    ///
    /// **Rule 7's row needs two fields, and the second is a floor rather than a
    /// preference.** `Ready` is a condition of the *pod* and does not move until every
    /// container is ready, while the rule fires per container — so a container that has
    /// existed for thirty seconds inside a pod that has been unready for an hour would
    /// draw `1 hour ago` about a process that cannot have been out of the Service for
    /// more than thirty seconds. That is this very row's own defect one level down. The
    /// answer is the later of the two moments: the condition says when the pod left the
    /// endpoints, [`ContainerState::Running`]'s `started_at` says how much of that this
    /// container was even alive for, and no container can have been failing its readiness
    /// probe before it started running.
    ///
    /// **Rule 8 is `None`, and not for the reason it looks like.** `spec.volumes` is
    /// immutable, so the pod's creation time *is* when its mount became dangerous — the
    /// number would be accurate. It is left out because the card describes a standing
    /// property rather than an event, and a date beside it reads as *"something
    /// happened"*, sending the reader looking for a change that never occurred. A column
    /// that is always populated and sometimes means something else is worse than one that
    /// is sometimes empty, because nothing on screen marks the rows where it lied
    /// (`screens/alerts.md` § *No number we cannot produce*).
    ///
    /// **N2's number dates the taint, not the cordon**, and the wording a card builds on
    /// it has to survive that. Anything that rewrites `node.spec.taints` wholesale —
    /// `kubectl edit`, a GitOps controller reconciling Node objects, a manifest re-apply —
    /// drops the mirrored taint and the node lifecycle controller re-adds it with a fresh
    /// stamp, and a taint that pre-dated the cordon carries no stamp at all. So *"cordoned
    /// about 2 hours ago"* is sayable and *"someone's maintenance window has been open for
    /// two hours"* is not — that argument is exactly what `screens/alerts.md` already
    /// deleted once for lack of a number, and a resettable clock does not bring it back.
    ///
    /// **`None` is the empty right edge**, and it has two producers that draw
    /// identically: no field to read — `kubectl taint` is client-side and stamps no
    /// `timeAdded`, so a hand-applied taint has no moment (NOTES § D43, § D65) — and a
    /// moment [`age`] refuses, which is the far-future guard on that function. Both are
    /// a bare title line in both renderers.
    ///
    /// **An `Option`, and not a zero.** `Time` has no "absent" value, so the epoch is
    /// what a non-optional field would carry, and [`age`] dates it honestly: a five-figure
    /// day count — *20678 days ago* against the pin the tests use — which is a confident
    /// wrong answer on the screen whose whole promise is that a number on it can be
    /// believed.
    ///
    /// **This field says how a finding *renders*, and nothing about how it sorts.**
    /// `screens/alerts.md` wants the cards with no age **last** inside their severity
    /// band; the derived `Ord` on `Option` puts `None` **first**, so the reflex
    /// `sort_by_key(|f| (f.severity, f.timestamp))` in Phase 9 produces the reverse of the
    /// requirement. Nothing is broken here today — [`Finding`] derives no `Ord` — and the
    /// note exists because the next reader gets this wrong for free.
    pub timestamp: Option<Time>,
}

impl Finding {
    /// **How long ago this finding's event happened, or nothing** — the whole render
    /// decision, and **the call a renderer makes for a finding**: the Alerts view and
    /// `--once` both come through here, and neither reaches past it to [`age`], which is
    /// the header's door rather than a card's.
    ///
    /// It exists rather than leaving `timestamp.as_ref().and_then(|t| age(now, t))` to be
    /// written in `ui.rs` and again in the `--once` printer: that is one expression in two
    /// files by two authors, and the house rule is that shared code is extracted rather
    /// than retyped. It also removes the way the free function can be called wrong —
    /// [`age`]`(&event, &now)` with the arguments swapped compiles, never panics, and
    /// paints *every* card `just now`, which reads as a cluster that has just fallen over.
    /// Here there is one argument and it is the one the caller has.
    ///
    /// `None` means **draw no age at all**: no timestamp, or one [`age`] itself refuses.
    /// The two are the same blank on screen and deliberately not distinguished here.
    pub fn age(&self, now: &Time) -> Option<String> {
        self.timestamp.as_ref().and_then(|t| age(now, t))
    }
}

/// **How long ago it happened, in the words the screens already print** — the one place
/// those words are spelled, so that two renderers cannot disagree about the same moment
/// (`screens/once.md`).
///
/// **For a finding, a renderer calls [`Finding::age`] and not this**, which is the
/// swap-proof way in and the whole render decision in one call. What comes here directly
/// is the age that hangs off no `Finding` and has no `self` to reach it by: the header's
/// stale-vitals age, `nodes 3/3 (40s ago)` in `screens/states.md`, which is also where the
/// seconds rung below gets its spelling.
///
/// Pure like the rest of this file — no clock call, the moment arrives as an argument
/// (invariant 5). **`now` is the *caller's* moment, and which moment that is depends on
/// what is being aged**: [`ClusterSnapshot::now`] for a finding rendered in that analysis
/// pass, and for every test in this file; a freshly read clock for the header's
/// stale-vitals age (`screens/states.md`), which measures how old the data on screen is
/// and therefore has to keep advancing while the snapshot does not. What wakes a redraw
/// is Phase 10/11's question and is deliberately not answered here. The subtraction is
/// `now − event`, that way round, or every age on a healthy cluster is negative
/// (NOTES § D18).
///
/// The ladder is the strings `screens/` draws, and nothing invented beside them:
///
/// | age | text | where the spelling comes from |
/// |---|---|---|
/// | ahead by more than [`SKEW_ALLOWANCE`] | **`None`** — draw nothing | `screens/alerts.md` § *No number we cannot produce* |
/// | ahead by less, or under one whole second | `just now` | NOTES § D18 |
/// | under a minute | `40s ago` | `screens/states.md`, the header's stale-vitals age |
/// | under an hour | `4 min ago` | `screens/alerts.md`, `screens/once.md` — finding ages, both |
/// | under a day | `2 hours ago`, `1 hour ago` | nothing draws one yet; it follows the days rung below |
/// | a day or more | `6 days ago`, `1 day ago` | `screens/alerts.md`, the age its cordon card used to carry |
///
/// **Every rung truncates, and `just now` swallows the sub-second gap on purpose.**
/// An event 400ms old is "just now" and not `0s ago`, which is a string no screen draws
/// and which reads as a stopped clock; the same truncation is why 4m59s is still
/// `4 min ago`, the way a reader counts.
///
/// **`min` stays abbreviated and unpluralised** because that is how both screens spell
/// it; hours and days are words, and a word gets its singular.
///
/// **The `None` rung is a wrong-field guard, not a clock feature.** Inside
/// [`SKEW_ALLOWANCE`] a future timestamp is a laptop behind the cluster and "just now" is
/// the honest reading (NOTES § D55). Beyond it, a clock is not the likeliest explanation
/// any more: the rule was pointed at a field that is future-dated *by design*, and this
/// file is full of them — C1's `notAfter`, C2/C4's after this file freezes, rule 12's raw
/// `deletionTimestamp` while the pod is still inside its grace window, which
/// `the_pinned_now_is_not_before_the_captures_it_is_read_against` already documents as
/// legitimately ahead of `now`. [`Finding::timestamp`]'s `Option` catches *"there is no
/// field"*; without this rung, *"the wrong field"* renders as a plausible English
/// sentence and nothing anywhere says so. So it draws the same blank the missing field
/// draws, which is `screens/alerts.md`'s own rule applied to the one case the code used
/// to exempt from it. A genuinely mis-set laptop is not this function's to announce —
/// the header says it in plain language, and that is its own box.
///
/// **What is not clamped is the other half of the skew.** A laptop *ahead* of the cluster
/// inflates every age instead of negating it, and that is the half that manufactures
/// findings on a healthy cluster (NOTES § D55). It is left visible, because hiding it
/// would hide a wrong clock rather than survive one — and no object timestamp can reveal
/// it anyway, the honest source being the API server's `Date` header (Phase 5).
///
/// **The arithmetic is `Timestamp::duration_since`, never `-`** (NOTES § D54):
/// subtracting two timestamps yields a seconds-only `Span` whose `.get_minutes()` is `0`
/// over a 43-minute gap, so the screen would read "0 min ago" and no type would object.
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

/// **How far into the future a timestamp may sit and still be read as a wrong clock
/// rather than a wrong field** — five minutes, and past it [`age`] draws nothing.
///
/// The number is not free and it is not tuned: five minutes is the clock-skew tolerance
/// the ecosystem already settled on — Kerberos' allowable clock skew, the leeway
/// implementations grant a JWT's `nbf`/`exp`, the slack in most TLS handshake validity
/// checks. It covers an unsynced laptop, a VM resumed from suspend, a WSL2 host after
/// sleep; it does not cover a certificate that expires next year or a deletion deadline
/// thirty minutes out, and those are the values a mis-pointed rule actually produces.
const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(5);

/// `1 hour` / `2 hours` — the rungs whose unit is a word the reader pluralises. Not the
/// minutes rung: both screens spell that `4 min`.
///
/// **The ` ago` is the caller's**, because two callers need the same length of time in two
/// tenses: [`age`] says when something happened and appends it, [`lasted`] says how long
/// something took and does not. Splitting it here rather than writing the ladder twice is
/// what keeps the two from ever disagreeing about what 90 minutes is called.
fn counted(n: i64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

/// **How long one container run lasted** — `2s`, `40 min`, `3 hours`, `6 days`. Rules 1,
/// 5 and 6 all show it, because *"it runs for about two seconds and then exits 1"* and
/// *"it ran for forty minutes and then exited 1"* are the first fork of every crashloop
/// triage and `kubectl describe` leaves the subtraction to a human
/// ([`Terminated::started_at`], NOTES § D51).
///
/// **Not [`age`] with the suffix taken off.** The question is a span between two moments,
/// so both of `age`'s special rungs are wrong here: `just now` describes a moment and not
/// a length, and the `None` a far-future timestamp earns there is a wrong-*field* guard —
/// a run that lasted no measurable time is an ordinary instant crash, and *"under a
/// second"* is the fact rather than a refusal to answer. The rungs and the pluralisation
/// are still shared, through [`counted`].
///
/// `None` when either end is missing, and when the run ended before it began — a clock
/// that ran backwards is not a duration this may report as a number.
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
// What a rule is allowed to look at, and the single decode that fills it. Reduced
// structs, not wrapped API objects: `docs/architecture.md` § Performance budgets the
// store at roughly a tenth of the object, and a field nobody reads is a field nobody
// can be wrong about. Every field below names the rule that reads it; a rule with no
// field here cannot be written, and a field with no rule is one nobody maintains.
//
// The decode lives here rather than in `k8s.rs` because it is the one place a fixture
// and a live watch event meet: `docs/architecture.md` § Testing says the decode path is
// covered by the rule tests, and it can only be covered once. Phase 5 feeds pruned
// objects into these `From` impls and stores what comes out.
//
// Missing field means `None` or empty — never a panic and never a `Result`
// (invariant 5). The `From` trait is the mechanical guarantee of that: it cannot fail.
//
// **Two things this decode deliberately does not do, and the phase that owns each.**
// Every message and name below is copied through exactly as the API sent it: control
// characters are *not* stripped (invariant 9) and lengths are *not* bounded — a 50MB
// message would be stored whole. Both belong to `k8s.rs` at ingest (Phase 5's security
// gate: "stripped at ingest, so no downstream code has to remember"), which means they
// must happen on the way *into* these impls. A `From` that receives a raw object is
// receiving untrusted text.
//
// **The fields that carry it are wider than the security gate's own list**, which reads
// "names, messages, annotations, log lines". Three more reach a card from here and Phase 5's
// strip has to cover each: `ownerReferences[].kind` and `.apiVersion` (see
// [`ObjectKind::from_api`]), and **`metadata.finalizers`** — anyone with `patch` on pods can
// put any string in that array, it is not validated beyond being a qualified name's shape,
// and rule 12 joins it straight into a `Finding`'s evidence line.

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

/// What the container was actually **given**, falling back to what it asked for.
///
/// `status.resources` is *"the compute resource requests and limits that have been
/// successfully enacted on the running container"* and `spec` is the request; in-place
/// resize (beta and default-on since 1.33) is what makes the two disagree. Patch a
/// crashing pod's limit 128Mi → 512Mi, have the resize sit `Deferred` because the node
/// cannot fit it, and a spec-first read makes rule 2 print "exceeded its 512Mi limit ·
/// exit 137" about a container that was never given 512Mi — an operator sent hunting a
/// leak in an application that never had the memory. So: enacted first, spec second, and
/// spec is still the answer for a container the kubelet has not reported resources on.
///
/// `status.allocatedResources` is deliberately not consulted: for the request half it
/// repeats what `status.resources.requests` says, and it would buy a third precedence
/// step with no new fact.
///
/// **The fallback is per key, and upstream computes it the same way.** A server too old
/// to populate `status.resources` is inside the supported version window, and there the
/// spec is the only source there is — so a missing key falls through rather than reading
/// as "nothing was enacted". That is not a house convention:
/// `component-helpers/resource`'s `maxResourceList` takes a key present on *either* side
/// (`if value, ok := list[name]; !ok || ...`), so `determineEffectiveRequests` is
/// `max(spec, actuated, allocated)` per key and `determineEffectiveLimits` is
/// `max(spec, actuated)` — and `PodRequests`, which is what N5 re-computes, carries the
/// comment *"The computation is part of the API and must be reviewed as an API change"*.
///
/// **Read per *object* instead — status present, therefore spec unread — and that
/// disagrees with the API on the one shape where the two differ.** The shape is
/// reachable: `convertContainerStatusResources` copies
/// `allocatedContainer.Resources.DeepCopy()`, the *allocated* map and not the spec, while
/// `validateContainerResize` forbids **removing** a resource key on a resize and permits
/// adding one — so the spec's key set can grow past the allocated one, and until that
/// resize is enacted the status is missing a key the spec has. There a per-object read
/// charges N5 nothing for a request the scheduler itself is already counting.
///
/// The cost is the narrow case of a resize adding a key to a map that **already exists**
/// — a memory limit beside an existing cpu limit: until the node enacts it, the spec's
/// new value is what gets named. (A resize adding the map itself reads identically
/// whether the fallback is per key or per side, so it is not the case that decides this.)
/// One case is knowingly left wrong: when the resize is rejected as `Infeasible` upstream
/// drops the spec entirely — requests become `max(actuated, allocated)` and limits the
/// enacted map alone — and [`PodSnapshot`] carries no `PodResizePending` condition to
/// notice, because no v1 rule reads one.
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

/// How a container stopped. Read by rule 2 (`OOMKilled`) and rule 6 (the exit-code
/// table); `finished_at` is when it last died, which is the timestamp rules 1, 2 and 6
/// have to show an age from.
///
/// `signal` is deliberately left out — 137 already carries it, and a second way to say
/// SIGKILL is a second thing to keep true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Terminated {
    pub reason: Option<String>,
    pub exit_code: i32,
    /// When this run *began*, so a finding can say **how long it lasted**. "Restarted 5
    /// times" is the same sentence for two unrelated incidents; "it runs for about two
    /// seconds and then exits 1" and "it ran for forty minutes and then exited 1" are
    /// the first fork of every crashloop triage — bad configuration on one side, a leak
    /// or a downstream timeout on the other. `kubectl describe` prints both timestamps
    /// and leaves the subtraction to a human at 3am; this is one of the few places k8rs
    /// can do the work instead of restating the object.
    pub started_at: Option<Time>,
    pub finished_at: Option<Time>,
    /// The kubelet's own last word on the run, carried verbatim like every other
    /// controller message (NOTES § D37). Usually absent — but under
    /// `terminationMessagePolicy: FallbackToLogsOnError` the kubelet puts the tail of
    /// the container's log here, which turns rule 6's action from "check the logs" into
    /// the log line. [`ContainerState::Waiting`] already carried a message; this one not
    /// having one was an asymmetry with nothing behind it.
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

/// What the container is doing *now* — an enum because upstream sets exactly one of the
/// three, and rule 7 ("running but not ready") is only distinguishable from rule 1
/// ("waiting in CrashLoopBackOff") by which one it is. Three `Option`s would let a rule
/// read a waiting reason off a terminated container.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContainerState {
    /// Rules 1, 3, 4: the reason is `CrashLoopBackOff` / `ImagePullBackOff` /
    /// `CreateContainerConfigError`, and the message is the runtime's own sentence.
    Waiting {
        reason: Option<String>,
        message: Option<String>,
    },
    /// Rule 7's state, and `started_at` is **when the current run began** — how long this
    /// process has been up, which rules 1, 5 and 6 need for the other half of their
    /// evidence: they age the *death* from `last_terminated`, and "it came back up forty
    /// seconds later" is readable nowhere else.
    ///
    /// **It is not rule 7's "since when".** A start time says when the process began, not
    /// whether it ever became ready, so "started 10 minutes ago and `ready: false`" is
    /// still every container waiting on a slow first readiness probe. The only source for
    /// *not ready since* is [`PodSnapshot::ready`]'s `last_transition` — the same ruling
    /// as [`ContainerSnapshot::started`], for the same reason (NOTES § D51).
    ///
    /// `Option` because upstream declares it one.
    Running { started_at: Option<Time> },
    /// An init container that failed and is not being retried sits here — `Init:Error`,
    /// which D27 lists beside `Init:CrashLoopBackOff`.
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
            // Not a fourth state: `ContainerState`'s own upstream doc says "if none of
            // them is specified, the default one is ContainerStateWaiting", so an empty
            // state *is* a waiting one with nothing said about why (NOTES § D45). Rules
            // 1, 3 and 4 match on a named reason, so this fires nothing — which is what
            // an `Unknown` variant did, at the cost of contradicting the API.
            Self::Waiting {
                reason: None,
                message: None,
            }
        }
    }
}

/// What kind of container this is — **three states, not a boolean**, because a native
/// sidecar is an init container that never finishes and the two arithmetics are opposite.
///
/// The scheduler's effective pod request is
/// `max( max over the init prefix , sum(regular) + sum(restartable init) )`: a
/// [`Sidecar`](ContainerRole::Sidecar) is **additive**, an [`Init`](ContainerRole::Init)
/// is not. With one flag N5 either overstates a 2Gi migration container or drops the
/// mesh proxy's request on every pod of a meshed node — and there is no third answer
/// that is right for both.
///
/// **That formula is an approximation, deliberately.** Upstream's `resource.PodRequests`
/// walks the init list *in order*, carrying the running sidecar total forward, so a
/// plain init container declared *after* a sidecar is charged on top of it; the
/// order-free version above understates that rare pod. It is the only implementable one
/// here, because [`PodSnapshot::containers`] explicitly does not promise an order.
///
/// It is also invariant 14: "the init container `istio-proxy` is crashlooping" is not
/// plain language, it is wrong. Rules 1–6 need the distinction for the sentence too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerRole {
    /// `spec.containers[]` — the workload itself.
    Regular,
    /// `spec.initContainers[]`, runs to completion before the regular containers start.
    Init,
    /// `spec.initContainers[]` with `restartPolicy: Always` — the native sidecar, GA
    /// since 1.29 and how Istio, Linkerd and the Vault agent run. It starts in the init
    /// sequence and then keeps running beside the workload, so it is charged like a
    /// regular container and described like one.
    Sidecar,
}

/// One container of a pod, init and regular in the same list.
///
/// **One list, not two.** Rules 1–6 read `initContainerStatuses` as well as
/// `containerStatuses`, and a pod at `Init:CrashLoopBackOff` producing no finding at all
/// was a real blind spot, not a hypothetical one (NOTES § D27). Two fields would let a
/// rule iterate one and forget the other; one field with a role cannot be half-read, and
/// the role is what lets the finding say *which* container — the whole diagnosis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerSnapshot {
    pub name: String,
    /// `status.image` — what the runtime actually resolved, required upstream. Rule 3's
    /// action is "check the image name, the tag or the pull secret", and the name reaches
    /// the user today only inside containerd's own sentence, which CRI-O words
    /// differently. It is also what makes rules 1, 5 and 6 concrete: "`app`
    /// (nginx:1.27) has restarted 12 times".
    pub image: String,
    /// Which of the three this is (NOTES § D27 for why both arrays are read at all).
    pub role: ContainerRole,
    /// Rule 7: running but not passing its readiness probe, so the Service dropped it.
    pub ready: bool,
    /// `status.started` — true once the container has passed its **startup probe** and
    /// run its `postStart` hook; a null value is treated the same as false (upstream).
    ///
    /// **A boot signal only where a `startupProbe` is declared, which most workloads do
    /// not do.** The same upstream sentence finishes: *"Is always true when no
    /// startupProbe is defined and container is running and has passed the postStart
    /// lifecycle hook"* — so for the majority of real pods it flips true the instant the
    /// container runs and discriminates nothing at all. No container in any committed
    /// fixture declares a `startupProbe`.
    ///
    /// **Rule 7's "since when" is [`PodSnapshot::ready`]'s `last_transition`, never this
    /// field.** `Running && !ready && started` reads like "was serving and stopped" and
    /// is in fact every pod of every rolling update, every node reboot and every
    /// scale-up — the false CRITICAL this contract was sent back to remove, rebuilt out
    /// of the sentence written to close it (NOTES § D51). `initialDelaySeconds` belongs
    /// to the *readiness* probe; `started` knows nothing about it.
    pub started: bool,
    /// Rule 5, thresholds ≥3 and ≥10.
    pub restarts: i32,
    pub state: ContainerState,
    /// `lastState.terminated` — how the *previous* run ended. Rules 2 and 6.
    pub last_terminated: Option<Terminated>,
    /// N5 sums these per node against the node's allocatable — unless the pod declares
    /// [`PodSnapshot::cpu_request`] / [`PodSnapshot::memory_request`], which replace the
    /// sum rather than adding to it.
    ///
    /// All three read **what the kubelet enacted first and the spec second** — see
    /// [`effective`] for why they can differ and which one a finding may name.
    pub cpu_request: Option<String>,
    pub memory_request: Option<String>,
    /// Rule 2's evidence: "exceeded its 64Mi limit" — the limit it was actually running
    /// under, never the one a pending resize asked for ([`effective`]).
    pub memory_limit: Option<String>,
}

/// A hostPath volume as one container actually mounts it. Rule 8 decides which of these
/// is bad — `/`, docker.sock, or writable — and the Phase 4 posture report lists the
/// rest, so what is stored is the fact, not the verdict.
///
/// `read_only` belongs to the *mount*, not the volume: the same hostPath can be mounted
/// read-only by one container and writable by another, and only the second is rule 8's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPathMount {
    /// The volume's `hostPath.path` — where on the node it starts.
    pub path: String,
    /// The mount's `subPath` — what of it the container actually gets. **Rule 8 reads
    /// `path` joined with this, never `path` alone**: `hostPath: /var/run` with
    /// `subPath: docker.sock` records `/var/run` on its own, and rule 8's docker.sock
    /// escalator never sees the socket it is looking for. `None` is the whole path.
    pub sub_path: Option<String>,
    /// The mount's `subPathExpr` — the same narrowing, written with environment variables
    /// in it (`$(POD_NAME)`), and **carried deliberately unresolved**.
    ///
    /// It cannot be resolved here and should not be: the values sit in
    /// `spec.containers[].env` and, through `valueFrom`, in ConfigMaps and Secrets that
    /// k8rs does not read and the security gate does not let it read. So this is a string
    /// with a `$(…)` in it, and the only fact it carries is the one that matters —
    /// **something narrows this mount and we cannot say what.**
    ///
    /// That fact points one way only. `hostPath: /` with `subPathExpr: $(POD_NAME)` gives
    /// the container a single directory, and a rule reading `path` alone announces *"has
    /// the whole filesystem of the machine it runs on mounted inside it"* — CRITICAL,
    /// false, and the loudest wrong card in the box. Carried, [`mounted_path`] joins it
    /// like a `subPath` and the escalator stops matching, which is the safe direction:
    /// the mount can still be reported as writable, and it is never called the node's
    /// root. The cost is a miss the other way — an expression that expands *to* a socket
    /// path is not recognised — and there is no way to close that without the env values.
    ///
    /// Upstream forbids `subPath` and `subPathExpr` on the same mount
    /// (`validateVolumeMounts`: they are mutually exclusive), so at most one of these two
    /// is ever set.
    pub sub_path_expr: Option<String>,
    pub read_only: bool,
    /// Which container mounts it. Without it the finding cannot say *who* has the node's
    /// root — `kubectl describe pod` can — and two containers mounting one volume produce
    /// two entries the rule cannot tell apart.
    pub container: String,
}

/// What the pod will put up with — N6 answers *which* taint is blocking it, and it can
/// only say "untolerated" by holding these. `tolerationSeconds` is left out: it times an
/// eviction after the fact, it does not decide whether the pod can be scheduled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toleration {
    pub key: Option<String>,
    pub operator: Option<String>,
    pub value: Option<String>,
    pub effect: Option<String>,
}

/// One pod, reduced to what rules 1–8, 10 and 12 read, plus the pod half of the N5 and
/// N6 joins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PodSnapshot {
    pub id: ObjectId,
    /// The card this pod's findings file under (NOTES § D3) — itself when nothing
    /// controls it, and itself when a Node does (NOTES § D39). Phase 5 resolves the
    /// ReplicaSet named here up to its Deployment; this layer records what the object
    /// said.
    pub owner: ObjectId,
    /// A static pod — the kubelet runs it off a file on the node and mirrors it into the
    /// API. **The bit is kept even though the Node identity behind it is discarded**
    /// (NOTES § D39): N2 must count only the pods a drain would actually move, and
    /// **neither a mirror pod nor a DaemonSet pod is ever evicted** — though for two
    /// different reasons, and only one of them is unconditional.
    /// `kubectl/pkg/drain/filters.go`: `mirrorPodFilter` returns a plain skip, while
    /// `daemonSetFilter` *aborts the whole drain* with `daemonSetFatal` unless
    /// `--ignore-daemonsets` is passed, and warns when it is. Either way the pod stays,
    /// so N2 must not count it: a correctly drained kind worker still runs kindnet and
    /// kube-proxy, and a cordoned control plane still runs its four static pods —
    /// without this, N2 fires on every node that *was* drained properly.
    ///
    /// **Sourced from the `ownerReference` of kind `Node`, not from the
    /// `kubernetes.io/config.mirror` annotation**: the fixture sanitizer strips
    /// annotations, so an annotation-sourced bit would decode `false` in every capture
    /// and could never be tested. Upstream keys the drain filter on the *annotation*, so
    /// the two sources have to agree — and they do, by construction:
    /// `kubelet/pod/mirror_client.go`'s `CreateMirrorPod` writes the annotation and the
    /// Node `ownerReference` in the same function, and a `getNodeUID()` that fails aborts
    /// the create rather than producing one without the other.
    pub mirror: bool,
    /// `spec.nodeName` — the join N5 and N6 are, and empty while the pod is unscheduled.
    pub node: Option<String>,
    /// Rule 7 is about a pod that is *Running*, and N5 cannot sum without it: a
    /// `Succeeded` Job pod keeps its `nodeName` for as long as nobody collects it, and
    /// its requests are charged to nobody — summing them would report an overcommit that
    /// the scheduler does not see.
    pub phase: Option<String>,
    /// **Driven by `status`, not by `spec`:** every container rule reads a status field,
    /// so a container the kubelet has not reported on cannot produce a finding, and
    /// inventing one would hand rule 7 a `ready: false` for every container that has not
    /// started yet. The cost is that an unscheduled pod contributes no requests — which
    /// is right for N5, since it is on no node to overcommit.
    ///
    /// `ephemeralContainerStatuses` is left out: a container someone attached with
    /// `kubectl debug` is not a workload, and a finding about one would be a finding
    /// about the person debugging.
    ///
    /// **The order is not a contract.** Init statuses lead today only because the decode
    /// chains the two arrays in that order, and reversing the chain breaks no assertion in
    /// this file — so nothing downstream may read this list by index or assume the init
    /// ones come first. Find a container by name, and when a screen wants init containers
    /// first, order them by [`ContainerSnapshot::role`] — the field says what each one is
    /// whatever order the API sent, which is exactly why it is not left implicit in the
    /// position. (`ContainerRole` deliberately has no `Ord`: which role sorts first is a
    /// display decision, and this file does not make those.)
    pub containers: Vec<ContainerSnapshot>,
    /// **The pod's own request** (`spec.resources.requests`, KEP-2837 — beta and
    /// default-on since 1.34), and **when it is set it replaces the container sum for
    /// N5, it does not add to it**: it is the whole pod's reservation, which is the
    /// point of the feature.
    ///
    /// A pod declaring `spec.resources.requests: {cpu: "4"}` and nothing per container
    /// decodes with all-`None` containers, so an N5 that only sums containers reports
    /// the node healthy while four committed CPUs sit invisible. That is
    /// [`ClusterSnapshot::namespace_scope`]'s shape a second time — a pure function
    /// reading zeros with no way to tell "requests nothing" from "requests something I
    /// did not look at".
    ///
    /// Pod-level *limits* are not carried, and that is **a known gap, not a clean
    /// boundary** — rule 2 does read a memory limit. Under the same KEP the limit that
    /// killed a container can sit on the pod while the container declares none:
    /// `kuberuntime_container_linux.go`'s `getMemoryLimit` puts the pod's limit on the
    /// container's cgroup whenever the container's own is unset. When the container
    /// declares *some* limit the kubelet copies the enacted value back and
    /// [`ContainerSnapshot::memory_limit`] sees it anyway; when it declares none at all,
    /// `convertContainerStatusResources` skips the whole block (`if resources.Limits !=
    /// nil`) and the number exists nowhere this snapshot reads — so rule 2 would say
    /// "exceeded its memory limit" with no figure while `spec.resources.limits.memory`
    /// sits unread. The field can wait for Phase 4 under D42; the rationale cannot,
    /// because "no v1 rule reads one" is what it used to say and nobody revisits that.
    pub cpu_request: Option<String>,
    pub memory_request: Option<String>,
    /// `conditions[PodScheduled]` — rule 10's whole input: the scheduler writes both the
    /// verdict and its own sentence here (NOTES § D27).
    pub scheduled: Option<Condition>,
    /// `conditions[Ready]`, kept whole beside `scheduled` for its `last_transition`.
    /// **It is the only source of "not ready since" there is** — no container status
    /// carries such a field anywhere, and rule 7 ("Running and `ready: false`") without a
    /// since-when also describes every container between start and its first successful
    /// readiness probe. That is every rolling update with an `initialDelaySeconds`, every
    /// node reboot and every scale-up, painted onto the one screen whose promise is
    /// *only what is broken*.
    ///
    /// `None` for a pod the kubelet has not reached — `pending.json` carries
    /// `PodScheduled` and nothing else.
    pub ready: Option<Condition>,
    /// Rule 12. **Not the moment the delete was accepted: it is request time plus the
    /// grace period** — `apiserver/pkg/registry/rest/delete.go` sets
    /// `metav1.Now().Add(gracePeriodSeconds)`, and `stuck.json` shows it, deleted at
    /// `23:16:54` with a 5-second grace. So the moment the user asked is
    /// `deletion_timestamp - grace_period_seconds`, and the pod is overdue once `now`
    /// passes `deletion_timestamp` itself; a rule reading this as the request time
    /// doubles its own threshold and reports an age one grace period short, forever.
    /// Cleared never — the pod object goes away instead.
    ///
    /// The subtraction is always the *metadata* grace, never the spec fallback below:
    /// the API server writes both fields in the same accepted delete, so whenever this
    /// is `Some` the grace beside it is the one that was actually granted.
    pub deletion_timestamp: Option<Time>,
    /// Rule 12's threshold, and it is the pod's own, never a constant. Reads
    /// `metadata.deletionGracePeriodSeconds` first — the grace this *delete* was granted
    /// — and falls back to `spec.terminationGracePeriodSeconds`, which is what the pod
    /// asked for. They differ exactly when someone passed `--grace-period`, and using
    /// the spec value there would keep a force-deleted pod quiet for 30 seconds it was
    /// never given.
    pub grace_period_seconds: Option<i64>,
    /// `metadata.finalizers` — who still has to sign off before the object can go.
    /// Rule 12 promises "a finalizer *or* the kubelet is holding it", and those are two
    /// causes with completely different actions; without the list the finding is a coin
    /// flip. `kubectl describe pod` does not print finalizers at all, so this is one of
    /// the few places k8rs says strictly *more* than describe rather than less.
    pub finalizers: Vec<String>,
    /// Rule 8.
    pub host_path_mounts: Vec<HostPathMount>,
    /// N6, the pod side. **`spec.affinity` is deliberately not here** — NOTES § Node
    /// rules names `nodeSelector`, and node affinity is a term tree that no v1 rule
    /// reads. N6 explains a `nodeSelector` and stays silent about affinity rather than
    /// guessing.
    pub node_selector: BTreeMap<String, String>,
    /// N6, the other half of "which taint is blocking it".
    pub tolerations: Vec<Toleration>,
}

/// A node taint, N6's other half.
///
/// **`added_at` is `Option` because of *who wrote the taint*, not which effect it
/// carries.** The node lifecycle controller stamps `timeAdded` on every taint it adds,
/// before any effect is looked at — `SwapNodeControllerTaint`,
/// `pkg/controller/util/node/controller_utils.go` — while `kubectl taint` is client-side
/// and stamps none. `nodes.json` carries both halves: the cordon's mirrored
/// `node.kubernetes.io/unschedulable` (`NoSchedule`) and the unreachable node's
/// `node.kubernetes.io/unreachable` (`NoSchedule` *and* `NoExecute`) each arrive with a
/// `timeAdded`, both being the controller's; the operator's own `dedicated=gpu:NoExecute`
/// arrives without one.
///
/// So **N2 can say "cordoned about 2 hours ago"** — the timestamp is in the object — and
/// the `Option` is here for the taint somebody applied by hand, which is the one that has
/// no time to give.
///
/// **What it dates is the taint, not the cordon, and the difference is a whole argument.**
/// Anything that rewrites `node.spec.taints` wholesale — `kubectl edit`, a GitOps
/// controller reconciling Node objects, a manifest re-apply — drops the mirrored taint,
/// and the node lifecycle controller puts it straight back with a **fresh** `timeAdded`
/// while `spec.unschedulable` never moved. The stamp is therefore a *floor*: the node has
/// been cordoned at least this long, possibly far longer, and a taint that was on the node
/// before any of this carries a stamp about itself or none at all. So a card may say
/// *"cordoned about 2 hours ago"* and may not say *"someone's maintenance window has been
/// open for two hours"* — the accusation `screens/alerts.md` deleted once already for lack
/// of a number, which a resettable clock does not earn back ([`Finding::timestamp`]
/// carries the same caveat).
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
    /// Cluster-scoped, so `namespace` is `None`. N1–N3 file their findings under it in
    /// both roles — `owner == object` (NOTES § D39), which is why there is no separate
    /// owner field here.
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

/// One Deployment, StatefulSet, DaemonSet or ReplicaSet — the objects that know a pod
/// was *supposed* to exist.
///
/// **Why four kinds decode into one type.** W1 reads a ReplicaSet's `ReplicaFailure` and
/// W2 a Deployment's `Progressing`, so those two are required outright. The other two
/// are in the permanent watch set (NOTES § D28) and produce the same three facts —
/// desired, ready, conditions — so a second type would carry no extra field and a
/// missing decode would mean `k8s.rs` reaching back into this file after it freezes.
///
/// **The blind spot this closes:** when the pods were never created there is nothing for
/// a pod rule to iterate, and k8rs reported a healthy cluster (NOTES § D28).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkloadSnapshot {
    pub id: ObjectId,
    /// A ReplicaSet's Deployment, so W1's finding files under the name the user deployed
    /// rather than under a hashed one. Itself when nothing controls it.
    pub owner: ObjectId,
    /// **How many the controller was told to run** — the top half of the shortfall W2
    /// measures. `spec.replicas` for a Deployment, StatefulSet or ReplicaSet; a DaemonSet
    /// has no such field and answers with `status.desiredNumberScheduled`, which carries
    /// no `omitempty` and is therefore always `Some`.
    ///
    /// **`None` is not zero here** — the opposite of [`ready`](WorkloadSnapshot::ready)
    /// below. `spec.replicas` is a `*int32` upstream, so a workload deliberately scaled to
    /// zero serialises `0` and decodes `Some(0)`; `None` means the field was absent, and
    /// the API server defaults it to **1** on all three kinds (`apps/v1/defaults.go`),
    /// never to 0. So `desired.unwrap_or(0)` says the workload wants nothing where the
    /// API says it wants one — the opposite direction from [`ready`](WorkloadSnapshot::ready),
    /// which is why the two `Option`s here cannot share a habit.
    pub desired: Option<i32>,
    /// **How many of them are passing their probes — and `None` means zero, not
    /// "unknown".** `readyReplicas` is a plain `int32` with `omitempty` on Deployment,
    /// StatefulSet and ReplicaSet alike, so the API server omits it *exactly* when it is
    /// 0 — which is the state W1 and W2 exist for. Both fixtures are in it:
    /// `deployments.json`'s `broken-quota` wants one replica, reports one unavailable and
    /// carries no `readyReplicas` at all, and its ReplicaSet reports `replicas: 0`. A
    /// DaemonSet's `numberReady` is required and decodes `Some(0)` for the same fact —
    /// one meaning, two shapes.
    ///
    /// So this reads as `ready.unwrap_or(0)`. A W2 written `if let (Some(d), Some(r))` —
    /// the obvious shape given a bare `Option` — goes silent on **total** outage, which
    /// is the exact blind spot the workload watch was added to close (NOTES § D28).
    pub ready: Option<i32>,
    /// W1: `ReplicaFailure`, message verbatim. W2: `Progressing` with reason
    /// `ProgressDeadlineExceeded` — which fires only when the two counters above show a
    /// shortfall and no pod-level finding already explains it.
    pub conditions: Vec<Condition>,
}

/// Everything a rule may read, at one instant.
///
/// Assembled by `k8s.rs` from the watch streams (Phase 5), never decoded from a single
/// API object — there is none. **Deliberately no `Default`, and since
/// [`now`](ClusterSnapshot::now) landed the type enforces that rather than asking for
/// discipline:** `Time` has no `Default` impl upstream, so `#[derive(Default)]` here no
/// longer compiles, and a hand-written one would have to invent a moment — the epoch,
/// handed to every rule as the current time, which is the exact failure invariant 5
/// exists to prevent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClusterSnapshot {
    /// **What time it is — the one clock a rule may read, and it reads it as a field.**
    /// Rule 12 compares it against a
    /// [`deletion_timestamp`](PodSnapshot::deletion_timestamp): it fires on
    /// `now − deletionTimestamp > max(30s, grace)` (NOTES § D55), and the age it reports
    /// is `now − (deletionTimestamp − grace)` — measured from the moment the user asked,
    /// because the deadline is one grace period later than that and an age taken from it
    /// is short by exactly that much, forever (NOTES § D46). C1 compares it against a
    /// certificate's `notAfter`; the "4 min ago" on the Alerts screen is [`Finding::age`]
    /// subtracting a timestamp the finding carried **from it** — that way round, or the
    /// age is negative on a healthy cluster (D18's second
    /// consequence, and now built). None of them calls a clock, because
    /// `analyze(&Snapshot) -> Vec<Finding>` is a pure function (invariant 5) and a clock
    /// call is the impurity that hides: it takes no argument, returns no error, and reads
    /// as arithmetic (NOTES § D18).
    ///
    /// **Captured once per analysis pass**, by `k8s.rs` (Phase 5) — never once per rule.
    /// Rules asking separately disagree by however long the pass took, and they disagree
    /// about one object: rule 12 saying a pod was asked to shut down 4 minutes ago, beside
    /// a finding the renderer ages at 5, is one screen contradicting itself over a single
    /// pod.
    ///
    /// **The failure this prevents is a rotting test.** A rule that called a clock would
    /// need its fixtures re-captured every time a certificate inside one expired, and the
    /// cheap repair for a test that starts failing on a Tuesday for no reason is to
    /// weaken it. With the moment in the input, `tests/fixtures/certs` pins its `notAfter`
    /// dates and `scripts/certs-test.sh` asserts "24 days left" as something still true
    /// in 2029.
    ///
    /// **`Time`, not a bare `jiff::Timestamp`.** `meta::v1::Time` is
    /// `pub struct Time(pub jiff::Timestamp)` and k8s-openapi re-exports the library, so
    /// this is the same type every decoded API timestamp already is: a comparison is two
    /// values of one type, with no `.0` at each site and no conversion layer to get
    /// wrong. It derives `Ord`, so the comparison is `<=`.
    ///
    /// **The arithmetic gets none of that, and it has three traps** (NOTES § D54,
    /// § D56). Every *duration* site needs `.0` on both sides — the newtype carries no
    /// operators of its own. `a - b` on two timestamps yields a **seconds-only `Span`**,
    /// so `.get_minutes()` over a 43-minute gap returns `0` and the screen reads "stuck
    /// 0 minutes ago"; the call that behaves is `Timestamp::duration_since`, which
    /// cannot panic and answers with a `SignedDuration`. And taking a grace period back
    /// off a deadline is `checked_sub`, never `-`: v1.36.1 accepted
    /// `terminationGracePeriodSeconds: 9223372036854775807` in a server-side dry-run
    /// against the live kind cluster, and the plain subtraction panics on it — anyone
    /// with `create` and `delete` on pods could otherwise kill the TUI through a pure
    /// function invariant 5 says cannot fail.
    ///
    /// **Not an `Option`.** A snapshot always has a moment. An `Option` would push a
    /// "what if there is no time" branch into every rule that reads one, and the only
    /// answer available there is the value the caller already had.
    ///
    /// **Clock skew is real, and its two halves are not symmetric** (NOTES § D55). The
    /// timestamps around it come from the API server and this one from the user's
    /// laptop. A laptop **behind** the cluster makes ages *negative* — rule 12 goes
    /// silent and D18's renderer draws "just now" — and that half is detectable from the
    /// snapshot alone, because any timestamp in it later than `now` says so, which is
    /// what `the_pinned_now_is_not_before_the_captures_it_is_read_against` asserts and
    /// what the header will eventually say in plain language. A laptop **ahead** of it
    /// inflates every age instead, and that is the half that manufactures findings on a
    /// healthy cluster — a correctly-progressing rollout read as overdue pods. **No
    /// object timestamp can reveal it**; the honest source is the API server's own
    /// `Date` response header, a Phase 5 `k8s.rs` question. Neither half is clamped
    /// here, where clamping would hide a wrong clock rather than survive one.
    pub now: Time,
    pub pods: Vec<PodSnapshot>,
    pub nodes: Vec<NodeSnapshot>,
    pub workloads: Vec<WorkloadSnapshot>,
    /// The control plane's version, for N4's skew comparison. `k8s.rs` reads it with
    /// `apiserver_version`; `None` means it could not be read, and N4 says so instead of
    /// comparing against a guess.
    pub server_version: Option<String>,
    /// **Rule C1's first input, and the reason a kubeconfig is anywhere near this
    /// struct.** C1 is the one finding with no API object behind it, and its input has
    /// to arrive here like every other rule's: `analyze(&Snapshot) -> Vec<Finding>` is
    /// the whole signature invariant 5 describes, so a second entry point taking PEM
    /// bytes would be an amendment to it — a stop, not a convenience (NOTES § D51).
    ///
    /// The kubeconfig **context name** is what the user calls this cluster, and it is
    /// C1's `ObjectId` name. `None` when the kubeconfig names no current context.
    pub context: Option<String>,
    /// The kubeconfig's client **certificate**, PEM bytes as they sit on disk. "Your
    /// access to this cluster expires in 24 days" is a thing only k8rs tells the user —
    /// no `kubectl` command shows it, which is why C1's `kubectl_cmd` is `None`.
    ///
    /// **The certificate and nothing else off the kubeconfig** — never the private key,
    /// never a token, never an exec plugin's output. A certificate is public material
    /// the API server already holds; a key or a token copied into our own types is one
    /// `Debug` away from a backtrace (invariant 8, and the security gate's token
    /// hygiene). `None` whenever the user authenticates any other way — a token, an exec
    /// plugin, OIDC — and C1 says nothing rather than guessing.
    pub client_certificate: Option<Vec<u8>>,
    /// **How much of the cluster [`pods`](ClusterSnapshot::pods) covers.** `None` = every
    /// namespace; `Some(ns)` = that one only. Set by `--namespace` **and** by the 403
    /// fallback, because to a rule the two are the same fact.
    ///
    /// N2 and N5 both join every pod on a node, so both are disabled under a namespace
    /// scope and say so rather than computing a partial answer — NOTES § D43,
    /// `todo.md`'s node-rules box and `docs/architecture.md` § Error handling all
    /// require it. A rule is a pure function with no globals (invariant 5), so it cannot
    /// ask anywhere else, and without this field a small cluster and a namespace-scoped
    /// view of a big one decode identically: `node-3` cordoned with 40 pods, none of them
    /// in `payments`, N2 counts zero and files nothing. A **silent miss** — nothing on
    /// the screen shows it happened.
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

/// The controller that owns this object, or the object itself when there is none — and
/// whether the controller that was discarded was a **Node**, which is what makes a pod a
/// mirror pod ([`PodSnapshot::mirror`]).
///
/// The second half rides along here because this is the one place the Node reference is
/// seen at all; asking a second time would mean a second traversal that can disagree with
/// this one. Workloads take the identity and drop the bit — a Node does not control one.
///
/// An `ownerReference` carries no namespace because an owner cannot be in another one,
/// so the object's own namespace is the answer.
fn owner_of(meta: &ObjectMeta, own: &ObjectId) -> (ObjectId, bool) {
    let controller = meta
        .owner_references
        .iter()
        .flatten()
        .find(|o| o.controller == Some(true));
    // Only a *controlling* reference decides anything. A non-controlling one is
    // somebody's garbage-collection link, and a non-controlling Node reference is not a
    // static pod. `find` is the whole search because there is at most one to find:
    // `ValidateOwnerReferences` rejects a second — "Only one reference can have
    // Controller set to true".
    let Some(o) = controller else {
        return (own.clone(), false);
    };
    // Resolved once, and the decision reads off the resolved kind rather than off the
    // string: `Node` in somebody's CRD group is an ordinary owner, not the kubelet.
    let kind = ObjectKind::from_api(&o.api_version, &o.kind);
    // A Node owner is discarded and the object files under itself: kubelet writes one
    // onto every mirror pod, and kept, `kube-system/etcd-*` loses its namespace and
    // draws as a machine (NOTES § D39).
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
    /// The `kind` string **read together with its `apiVersion`**, because a kind string
    /// on its own does not name a kind.
    ///
    /// OpenKruise is deliberately drop-in: its Advanced StatefulSet is
    /// `apps.kruise.io/v1beta1, Kind: StatefulSet` and its Advanced DaemonSet
    /// `apps.kruise.io/v1alpha1, Kind: DaemonSet`; Volcano's Job is
    /// `batch.volcano.sh/v1alpha1`. Matched on the kind alone each becomes the built-in
    /// variant, and the card lying is the small half — the large half is Phase 7 aiming
    /// `scale` at `apps/v1 statefulsets/<name>`: a 404, or a *different* object that
    /// happens to share the name. A write pointed at the wrong object is not a display
    /// bug. An Argo Rollout was safe here only by the accident of a unique kind string.
    ///
    /// **The group decides, not the whole `apiVersion`.** A Kubernetes type is named by
    /// its group and its kind; the version is how it is serialised, and an `apps/v1beta1`
    /// StatefulSet is the same StatefulSet as an `apps/v1` one. Anything this project has
    /// no branch for stays as text, qualified — inventing a variant for it would be
    /// per-kind code (invariant 12).
    ///
    /// **Both arguments are unvalidated free text when they come off an
    /// `ownerReference`.** apimachinery's `validateOwnerReference` requires only that
    /// `kind` is non-empty and that `apiVersion` parses to a non-empty *version*; the
    /// group half and every byte of the kind are whatever the writer sent. The two
    /// `Other` arms below carry them into a string that reaches a card, so a pod anyone can
    /// create in their own namespace can put terminal escapes on the screen — invariant
    /// 9's crafted-name attack through two fields the security gate's own wording
    /// ("names, messages, annotations, log lines") does not list. Phase 5's ingest strip
    /// has to cover `ownerReferences[].kind` and `.apiVersion` as well as the names.
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
            // Container names are unique across both arrays — Kubernetes enforces it — so
            // one scan finds the declaration this status belongs to. Pods have a handful
            // of containers, so a scan is cheaper than building a map per pod.
            //
            // **The miss has no test because the API cannot produce the object.** A status
            // naming a container the spec does not declare would leave `declared` `None` —
            // no requests, no limits, `restartable` false, so an init status would decode
            // as `Init` with nothing behind it. Both container lists are immutable after
            // create (only `image`, and `resources` under in-place resize, may change;
            // containers are never added or removed), so the kubelet cannot report on a
            // container that is not in the spec. The one list that *does* grow is
            // `ephemeralContainers`, whose statuses are deliberately not read (see
            // `PodSnapshot::containers`). Synthesizing the miss would be a shape no API
            // server emits, which D40 does not license — so the absence of a test here is
            // a ruling, not an oversight.
            let declared = spec
                .init_containers
                .iter()
                .flatten()
                .chain(spec.containers.iter())
                .find(|c| c.name == s.name);
            let requested = declared.and_then(|c| c.resources.as_ref());
            // What the node actually enacted, which is not always what the spec asks for.
            let enacted = s.resources;
            // `restartPolicy: Always` on an *init* container is the native sidecar. The
            // regular list is not asked — not because upstream forbids the field there
            // (1.34 began relaxing that), but because a regular container is charged
            // additively and described as itself whatever its restart policy says, so
            // the answer would be `Regular` either way.
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
                // A status with no `state` at all takes the same road as one whose state
                // is set but empty: `unwrap_or_default` hands the `From` impl above an
                // all-`None` `ContainerState`, and it answers with the waiting the API
                // says that means. One construction of that case, not two.
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
        // `spec.ephemeralContainers` is not walked, for the same reason their statuses
        // are not read (see `PodSnapshot::containers`): a container someone attached with
        // `kubectl debug` is not a workload, and rule 8 firing on one would be a finding
        // about the person debugging.
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
        // Both are picked by name off the same array. A pod carries five conditions and
        // `PodScheduled` is the last of them, so neither can be "the first one".
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
            scheduled: condition("PodScheduled"),
            ready: condition("Ready"),
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

/// **No test covers this impl, and none can yet.** `tests/fixtures/statefulsets.json` is
/// an empty list — nothing in `scripts/broken.yaml` or `scripts/healthy.yaml` produces a
/// StatefulSet — and there is no committed object to change one field on, so the
/// technique the tests below it use does not reach here either. Synthesizing a whole
/// StatefulSet would be the hand-written JSON CLAUDE.md forbids, with extra steps. The
/// impl stays because `k8s.rs` watches the kind (NOTES § D28) and this file freezes at
/// the end of Phase 3; the open Phase 2 capture trip owns closing the gap.
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
// One function per rule of NOTES § v1 rule set, each one pure and each one returning what
// it found rather than reporting how it failed: a missing field is `None` and no finding,
// never a default and never a `Result` (invariant 5). The clock arrives as
// [`ClusterSnapshot::now`], so a fixture cannot expire.
//
// **What is in here is D2's line — broken *now*.** Rule 9 (no limits declared) and the
// plain read-only hostPath are risks rather than outages and belong to the Analysis
// reports; rule 11 (probe failures) needs an Events watch this project does not open.
// Their absence is the design, not a gap.
//
// **Every string below is written for someone in their first month** (invariant 14).
// `CrashLoopBackOff`, `OOMKilled` and an exit code are each explained in a sentence and
// then named in brackets, so the reader learns the word rather than being handed it.

/// The evidence line's separator, spelled once — `screens/alerts.md` draws
/// `limit 256Mi · exit 137 · 47 restarts`, and two rules picking different glue is a
/// screen that looks assembled from two products.
const FACTS: &str = " · ";

/// Rule 5's two bands (REQUIREMENTS: restarts ≥3 warn, ≥10 critical).
const RESTARTS_WARN: i32 = 3;
const RESTARTS_CRITICAL: i32 = 10;

/// **How long a pod may sit `Running` and unready before rule 7 says anything** — ten
/// minutes, and the number is borrowed rather than tuned.
///
/// It is `progressDeadlineSeconds`' default, which is Kubernetes' own answer to *"how long
/// may a pod take to become ready before that counts as a failure"*: a Deployment marks its
/// rollout `ProgressDeadlineExceeded` at exactly this point and not before. Rule 7 firing
/// sooner would put k8rs at odds with the controller that owns the rollout — a card saying
/// *not receiving traffic* while the cluster still considers the rollout healthy — and
/// `Running` + `ready: false` is every container between start and its first successful
/// readiness probe, so a shorter window paints the screen on every deploy, every node
/// reboot and every scale-up (NOTES § D46, § D51).
const NOT_READY_GRACE: SignedDuration = SignedDuration::from_mins(10);

/// **The margin on rule 12's deadline** (NOTES § D55). A pod is briefly overdue between
/// its own deadline and the kubelet's SIGKILL landing even on a perfect clock, and a laptop
/// running fast makes every pod deleted in the last ten minutes look overdue — the half of
/// clock skew that manufactures findings and that no object timestamp reveals. Thirty
/// seconds costs nothing: a pod actually held by a finalizer is held for minutes or forever.
const OVERDUE_MARGIN: SignedDuration = SignedDuration::from_secs(60);

/// The namespace whose CNI, kube-proxy and control-plane pods mount the node on purpose —
/// see [`escalated_host_path`].
const NODE_NAMESPACE: &str = "kube-system";

/// **Every control socket that is the machine.** A process that can talk to one of these
/// can start a privileged container on the node, so a **read-only** bind of it is still
/// full root — which is why rule 8 escalates on the path and not on the mode.
///
/// **Docker's socket is not the list.** NOTES § v1 rule set names `/var/run/docker.sock`,
/// which was the whole runtime landscape when that line was written and is now the one
/// runtime these fixtures' own cluster does *not* run: kind runs **containerd**, and a
/// list that stops at Docker means the single most common cluster-takeover shape produces
/// nothing at all on the cluster this project is tested against. Worse than nothing,
/// actually — a `kube-system` DaemonSet mounting the runtime socket falls through to the
/// writable branch, which that namespace's exemption then silences.
///
/// `/var/run` is a symlink to `/run` on every systemd distribution, so each socket appears
/// under both spellings and a manifest may use either; the rule may not depend on which one
/// an author happened to type. CRI-O's default is the `/var/run` form.
const RUNTIME_SOCKETS: [&str; 5] = [
    "/var/run/docker.sock",
    "/run/docker.sock",
    "/run/containerd/containerd.sock",
    "/var/run/containerd/containerd.sock",
    "/var/run/crio/crio.sock",
];

/// **Every rule in this file, over one snapshot** — the signature invariant 5 names, and
/// the only entry point `k8s.rs` and the `--once` printer are given.
///
/// Rules 1–8 and 12. Rule 10 (Pending, and why), rules 1–6 over `initContainerStatuses`,
/// the N-series, W-series and C1 are later boxes of this phase and are deliberately not
/// wired here: a half-built rule is worse than an absent one, because the screen looks
/// complete either way.
///
/// **Rules 1–7 read `containerStatuses` only**, which is [`ContainerRole::Regular`] on the
/// merged list. The init containers are one box away (NOTES § D27) and reading them needs
/// the finding to name *which* init container, so it is a change to every sentence in those
/// rules and not a change to this loop. **Rule 8 is not in that sentence** and must not be
/// read as if it were: its input is `spec.volumes` and the mounts against it, which
/// [`host_path_mounts`] walks across the init containers as well — a hostPath is mounted
/// whatever list the container was declared in, and no status is consulted to know it.
///
/// **A pod that finished successfully is not broken now**, so rules 1–8 skip it. `Succeeded`
/// is the state every `restartPolicy: OnFailure` Job pod ends in, and it keeps its restart
/// count and its last non-zero exit for as long as nobody collects it — rules 5 and 6 would
/// then report the history of a job that worked, in the hundreds, on the one screen whose
/// promise is *only what is broken* (NOTES § D2). Rule 12 is deliberately outside the skip:
/// a `Succeeded` pod that will not go away is still stuck.
///
/// **`Failed` is in the skip for the same reason and it is the commoner half.** An Evicted
/// pod is `phase: Failed` with its restart count and its last non-zero exit still on it, and
/// terminated pods are collected only above `--terminated-pod-gc-threshold` — 12500 by
/// default — so on any cluster that has ever been under memory pressure they pile up, and
/// each would draw two permanent cards for a pod that will never run again. NOTES routes
/// Evicted and Completed pileups to the **Waste** report, not here. Nothing is lost: a
/// `restartPolicy: Never` pod that failed carries `state: Terminated` with an empty
/// `lastState`, so rules 1–6 read nothing off it either way.
///
/// **No committed capture is in either state**, because `scripts/broken.yaml` and
/// `scripts/healthy.yaml` create no Job and evict nothing — so the skip is reasoned and
/// unproven in both halves, and deleting it leaves the suite green. **Capture trip:** a
/// completed `restartPolicy: OnFailure` Job pod with two or more restarts, and a pod evicted
/// by a `memory.available` pressure threshold.
pub fn analyze(snapshot: &ClusterSnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();
    for pod in &snapshot.pods {
        findings.extend(stuck_terminating(&snapshot.now, pod));
        if matches!(pod.phase.as_deref(), Some("Succeeded" | "Failed")) {
            continue;
        }
        findings.extend(escalated_host_path(pod));
        for c in pod
            .containers
            .iter()
            .filter(|c| c.role == ContainerRole::Regular)
        {
            findings.extend(crash_looping(pod, c));
            findings.extend(out_of_memory(pod, c));
            findings.extend(image_not_pulled(pod, c));
            findings.extend(container_config_missing(pod, c));
            findings.extend(restarting_repeatedly(pod, c));
            findings.extend(previous_run_failed(pod, c));
            findings.extend(running_but_not_ready(&snapshot.now, pod, c));
        }
    }
    findings
}

/// `kubectl describe pod …` — the one command that shows a container's current state, how
/// its last run ended, its restart count, the limits it is running under and its mounts.
/// That is what rules 1–8 claim, so it is the command invariant 4 shows beside them.
///
/// **Rule 12 does not use it**: `describe` prints no finalizers at all (NOTES § D46), and a
/// teaching command that does not show what the card says is worse than none.
fn describe(id: &ObjectId) -> Option<String> {
    Some(format!(
        "kubectl describe pod {}{}",
        id.name,
        in_namespace(id)
    ))
}

/// `kubectl get pod … -o yaml` — for the three cards whose evidence is a field `describe`
/// does not print at all.
///
/// Rule 12's is `metadata.finalizers`, which `describe` has never rendered. **Rules 3 and 4
/// are the same failure and were missed**: kubectl's `describeStatus` prints a waiting
/// container's `Reason` and stops, so `state.waiting.message` — which is the *entire*
/// evidence line of both cards, the sentence naming the registry that refused or the
/// ConfigMap that is absent — reaches `describe` only indirectly, through an Event that
/// rewords it and disappears at `--event-ttl`. A teaching command that does not show what
/// the card says is worse than none (invariant 4).
fn get_yaml(id: &ObjectId) -> Option<String> {
    Some(format!(
        "kubectl get pod {}{} -o yaml",
        id.name,
        in_namespace(id)
    ))
}

/// ` -n <namespace>`, or nothing at all when there is none. The flag is appended rather
/// than always written because `-n ""` is a command that does not work, printed in the
/// record invariant 4 says may not lie ([`ObjectId::namespace`]).
fn in_namespace(id: &ObjectId) -> String {
    id.namespace
        .as_deref()
        .map_or_else(String::new, |ns| format!(" -n {ns}"))
}

/// The reason and the runtime's own sentence, for a container that is waiting — and
/// `None` for one that is running or has stopped, which is what keeps rules 1, 3 and 4
/// from reading a waiting reason off a container in another state ([`ContainerState`]).
///
/// Also `None` for a container that is waiting and has been given no reason, which the
/// decode produces for an empty `state` (NOTES § D45). Every caller matches on a named
/// reason, so the two collapse to the same answer: nothing to fire on.
fn waiting(c: &ContainerSnapshot) -> Option<(&str, Option<&str>)> {
    match &c.state {
        ContainerState::Waiting { reason, message } => {
            Some((reason.as_deref()?, message.as_deref()))
        }
        _ => None,
    }
}

/// **What an exit code means, in the words a beginner needs** — NOTES § v1 rule set's
/// translation table, and nothing invented beside it. `None` is a code with no accepted
/// meaning, where the number alone is the honest answer.
///
/// 143 is in the table and is the one entry that says *nothing is wrong*, which is why
/// [`previous_run_failed`] refuses to fire on it: a container that was asked to stop and
/// stopped is not a finding. It stays here because rule 1 does print it — a container that
/// is crash-looping and whose last run was a clean SIGTERM is a real and confusing state,
/// and the sentence is what unconfuses it.
///
/// **137 needs the `reason` beside it, and NOTES' own table is corrected here.** That table
/// reads *"137 — SIGKILL, almost always OOM"*, which was written for a rule that had no
/// reason field to consult. It does now, and the two cases are not the same incident: with
/// [`Terminated::reason`] `OOMKilled` the kernel took the container for using too much
/// memory; **without it**, 137 is the kubelet's own SIGKILL after a SIGTERM the process did
/// not answer inside its grace period — a failing `livenessProbe` or a shutdown that hangs.
/// Printing the memory sentence there sends someone at 3am to raise a limit on a container
/// whose liveness endpoint is timing out, which is the most expensive kind of wrong because
/// raising the limit appears to help for a while.
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

/// `exit 137 (killed with SIGKILL, …)` — the number first, because that is what the reader
/// will search for, and the sentence in brackets like every other piece of jargon on these
/// cards. Takes the whole [`Terminated`], because [`exit_meaning`] needs the reason that
/// sits beside the code and a caller passing one without the other is the bug this signature
/// removes.
fn exit_fact(run: &Terminated) -> String {
    match exit_meaning(run.exit_code, run.reason.as_deref()) {
        Some(meaning) => format!("exit {} ({meaning})", run.exit_code),
        None => format!("exit {}", run.exit_code),
    }
}

/// **The last thing the container actually said**, out of the kubelet's termination
/// message — `None` when it left none, which is the usual case ([`Terminated::message`]).
///
/// **The last non-empty line, not the first.** Under `terminationMessagePolicy:
/// FallbackToLogsOnError` this field is the *tail* of the container's log, so the first
/// line is whatever the process printed on the way up — `tests/fixtures/crashloop.json`
/// starts its with `starting` and ends it with the panic that killed it. Taking the first
/// line would print the boot banner and call it a cause.
///
/// **One line, and this is where that is decided.** A card is three to five lines
/// (`screens/widgets.md` § 2) and a `Finding`'s fields are each one of them, so a value
/// with newlines in it does not fit a slot — the choice of *which* line is a rule's, the
/// same way the choice of which timestamp is. It is not truncation: `screens/widgets.md`
/// § 7 forbids k8rs shortening a string itself, and bounding a huge value is `k8s.rs`'s job
/// at ingest, one phase up.
fn last_log_line(run: &Terminated) -> Option<&str> {
    run.message
        .as_deref()?
        .lines()
        .map(str::trim_end)
        .rfind(|l| !l.is_empty())
}

/// **Rule 1 — the container keeps crashing and Kubernetes has started waiting between
/// restarts.** `state.waiting.reason == CrashLoopBackOff`, CRITICAL: this container is not
/// doing its job right now.
///
/// The age is [`Terminated::finished_at`] on the previous run — when it last died — and
/// never `started_at` one line above it, which is when that run began
/// ([`Finding::timestamp`]).
fn crash_looping(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let (reason, _) = waiting(c)?;
    if reason != "CrashLoopBackOff" {
        return None;
    }
    let mut facts = vec![format!("container {}", c.name)];
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
/// **The limit named is the one that was enacted**, never the one a pending in-place resize
/// asked for: [`ContainerSnapshot::memory_limit`] reads `status.resources` first for exactly
/// this sentence, or the card sends an operator hunting a leak in an application that was
/// never given the memory (NOTES § D51). When no limit is readable the term is left out
/// rather than guessed — a pod-level limit can kill a container that declares none, and
/// this snapshot does not carry one, so the number genuinely is not here
/// ([`PodSnapshot::cpu_request`]).
fn out_of_memory(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let run = c.last_terminated.as_ref()?;
    if run.reason.as_deref() != Some("OOMKilled") {
        return None;
    }
    let mut facts = vec![format!("container {}", c.name)];
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

/// **Rule 3 — the image could not be downloaded, so the container never started.**
/// `state.waiting.reason` of `ErrImagePull` or `ImagePullBackOff` — the kubelet alternates
/// between the two as it backs off and retries, and both mean the same thing. CRITICAL.
///
/// The runtime's own sentence is quoted verbatim (NOTES § D37) because it is the only place
/// the actual failure appears — a name typo, a missing tag and a registry that needs
/// credentials all look identical without it. The resolved image name is printed beside it
/// from [`ContainerSnapshot::image`] rather than dug out of that sentence, which containerd
/// and CRI-O word differently.
///
/// No age: a failed pull is a state the kubelet is still retrying, and nothing in the
/// container status records when the first attempt was made.
fn image_not_pulled(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let (reason, message) = waiting(c)?;
    if !matches!(reason, "ErrImagePull" | "ImagePullBackOff") {
        return None;
    }
    let mut facts = vec![
        format!("container {}", c.name),
        format!("image {}", c.image),
    ];
    facts.extend(message.map(str::to_string));
    Some(Finding {
        severity: Severity::Critical,
        title: format!("Container image could not be downloaded ({reason})"),
        evidence: facts.join(FACTS),
        action: "check the image name and tag, and whether this namespace has a pull secret \
                 for that registry"
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
/// The kubelet's message names the missing object (`configmap "…" not found`), and that
/// name is the whole of what the reader has to go and create or correct — so it is quoted
/// verbatim rather than summarised.
fn container_config_missing(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let (reason, message) = waiting(c)?;
    if reason != "CreateContainerConfigError" {
        return None;
    }
    let mut facts = vec![format!("container {}", c.name)];
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

/// **Rule 5 — the container has been restarted enough times that something is wrong even
/// if it looks fine now.** `restartCount`, WARN at [`RESTARTS_WARN`] and CRITICAL at
/// [`RESTARTS_CRITICAL`].
///
/// **Quiet on a container rule 1 is already describing.** `broken-oom` used to draw three
/// CRITICALs for one incident — *keeps crashing* (whose evidence already reads `15
/// restarts`), *used more memory*, and *has been restarted 15 times* — and the third
/// carries nothing the first two do not. That is the principle [`previous_run_failed`]
/// already applies to rule 2 by name, one step over: one incident, one card. NOTES' *"even
/// if Running … looks healthy now"* says what this rule is for, and it is the container
/// that looks fine, not the one visibly in a loop.
///
/// **The title changes with the state and the reason is that it would otherwise be false.**
/// NOTES' wording is the whole point of the rule for a container that is up and serving,
/// and would be a lie about one that has stopped.
///
/// **Severity is WARN whenever the container is serving, whatever the count.** A container
/// up six weeks with a nightly leak-restart reaches forty while passing every probe, and
/// `RESTARTS_CRITICAL` would put it in the same red band as `CrashLoopBackOff` — a red card
/// whose own title says it is serving is what teaches a reader to stop believing red
/// (NOTES § D2). REQUIREMENTS marks those two numbers *(suggestion)*, and the argument for
/// bending the top one here is that a lifetime counter carries no *rate*: forty restarts in
/// six weeks and forty in an hour are the same integer, and this snapshot cannot tell them
/// apart. The band stays; what it may reach on a working container does not.
///
/// **That leaves the CRITICAL branch with no capture behind it.** It now needs a container
/// with ten or more restarts that is neither serving nor in a crash loop — a real but
/// transient state, caught between restarts or waiting on something else — and no committed
/// fixture is in it, so a mutation that makes the band unreachable stays green. **Capture
/// trip:** a pod photographed mid-restart, or one whose many restarts ended in a different
/// waiting reason such as `ImagePullBackOff` after a tag was moved. The same capture closes
/// the `&& !serving` half from the other side — `broken-restarts` sits at three, below
/// `RESTARTS_CRITICAL`, so no fixture can tell the two spellings of this severity apart and
/// only the constants themselves are asserted.
///
/// The age is when the counter last went up, which is [`Terminated::finished_at`] on the
/// previous run: the restart is the event this card is about. A container with a count and
/// no previous run recorded has no such moment and draws none ([`Finding::timestamp`]).
fn restarting_repeatedly(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    if c.restarts < RESTARTS_WARN || waiting(c).map(|(r, _)| r) == Some("CrashLoopBackOff") {
        return None;
    }
    let serving = c.ready && matches!(c.state, ContainerState::Running { .. });
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
        evidence: [format!("container {}", c.name), c.image.clone()].join(FACTS),
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
/// `lastState.terminated.exitCode`, WARN: the run that failed is over, and where the
/// container is *currently* broken rules 1 to 4 say so as CRITICAL beside this.
///
/// **Two exits are not findings.** `0` is a success, and `143` is SIGTERM — a container
/// that was asked to stop and stopped, which is every rolling update and every scale-down
/// (NOTES § v1 rule set). Firing on either would put a card on a healthy cluster.
///
/// **Neither exemption has a capture behind it, and that is stated rather than hidden.**
/// Every regular container in the repository whose previous run is recorded exited `1` or
/// `137`, so a mutation that deletes these two comparisons stays green — the one place in
/// this box where the suite cannot tell right from wrong. **Capture trip:** two pods in
/// `scripts/broken.yaml`, both with `restartPolicy: Always` — one whose command exits `0`
/// and restarts, and one that a failing `livenessProbe` stops, where an unhandled SIGTERM
/// leaves `143`. Until then the two `if`s are reasoned and unproven.
///
/// **`OOMKilled` belongs to rule 2, so this stays quiet on it.** Both would otherwise fire
/// on one death, and the second card would be the weaker of the two: *"exit 137, almost
/// always memory"* beside *"used more memory than it was allowed"*, which already names the
/// limit. One event, one card.
///
/// **And quiet on a container that is serving now, because this field never expires.**
/// `lastState.terminated` is kept for the life of the pod, so a container that restarted
/// once six months ago and has answered every request since would otherwise draw *"The
/// container's previous run failed · 180 days ago"* for ever. That is the largest
/// false-positive *volume* in this box and the cheapest to reach — it needs no unusual
/// manifest, only uptime — and a permanent card on a healthy workload is exactly what makes
/// an empty Alerts screen unbelievable (NOTES § D2). A serving container's restart history
/// belongs to [`restarting_repeatedly`], which has a threshold under it; a single old
/// failure has none and never will.
///
/// **When the kubelet kept the container's last words, they replace the advice.** Telling
/// someone to go and read a log k8rs is already holding is the shape of a tool that
/// restates the object instead of answering ([`Terminated::message`]).
fn previous_run_failed(pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
    let run = c.last_terminated.as_ref()?;
    if run.exit_code == 0
        || run.exit_code == 143
        || run.reason.as_deref() == Some("OOMKilled")
        || (c.ready && matches!(c.state, ContainerState::Running { .. }))
    {
        return None;
    }
    // The kubelet's `reason` for a non-zero exit is the bare word `Error`, which says
    // nothing the title has not already said in a sentence — printing it would be jargon
    // on the card for its own sake (invariant 14).
    let mut facts = vec![format!("container {}", c.name)];
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

/// **Rule 7 — the container is up but its readiness check is failing, so the Service has
/// stopped sending it traffic.** WARN, and the hardest rule in this file to keep quiet.
///
/// Four conditions, and every one of them is load-bearing:
///
/// - the pod is `Running` ([`PodSnapshot::phase`]),
/// - **the container is in [`ContainerState::Running`]**, which is what tells this rule
///   apart from rule 1 — a container waiting in a crash loop is also `ready: false`, and
///   describing it as *running but not receiving traffic* is a wrong diagnosis printed
///   confidently,
/// - it is not ready ([`ContainerSnapshot::ready`]),
/// - and it has been that way for longer than [`NOT_READY_GRACE`].
///
/// **The since-when is [`PodSnapshot::ready`]'s `last_transition` and nothing else.**
/// `Running && !ready` is *also* every container between start and its first successful
/// readiness probe, so without a clock the rule fires on every rolling update with an
/// `initialDelaySeconds`, every node reboot and every scale-up. It is specifically not
/// [`ContainerSnapshot::started`], which is `true` the instant a container runs wherever no
/// `startupProbe` is declared — that field discriminates nothing on the overwhelming
/// majority of real workloads and would rebuild the very false positive this rule was sent
/// back to remove (NOTES § D51).
///
/// **The since-when is floored at the container's own run start**, because `Ready` is a
/// condition of the *pod* and this rule fires per container. It does not move until every
/// container is ready, so a container thirty seconds old inside a pod that has been unready
/// for an hour would be dated `1 hour ago` — and the ten-minute grace would be bypassed
/// altogether, which is how a crash-looping container caught between restarts fired this
/// rule instantly on top of rules 5 and 6. A container cannot have been out of the Service
/// for longer than its current run has existed, so the answer is the **later** of the two
/// moments ([`Finding::timestamp`]).
///
/// **`started` is read here as a suppressor, and that is not what D51 rejected.** D51
/// rejected it as a *trigger* and that ruling stands: `Running && !ready && started` is
/// every pod of every rolling update, because the field is always true once a container runs
/// where no `startupProbe` is declared. Read the other way round it says something the
/// trigger reading cannot: `Running && !started` is reachable **only** where a
/// `startupProbe` *is* declared and has not yet passed — and while it has not passed, the
/// kubelet does not run the readiness probe at all, so `ready: false` there means *not asked
/// yet*, not *answered wrongly*. Without this, every Cassandra, Elasticsearch and Vault pod
/// with `failureThreshold: 60, periodSeconds: 30` draws a card at ten minutes while it is
/// booting exactly as its author intended. A field can discriminate nothing in one direction
/// and everything in the other; that is the whole distinction, and it is written here
/// because the next reader will otherwise read this line as D51 being violated.
///
/// **Both of those two checks are unproven, and on the committed captures they are
/// redundant with each other.** No fixture declares a `startupProbe`, so every container in
/// the repository reports `started: true`, and every container that is not `Running` reports
/// `false` — so deleting either one leaves the suite green while the other happens to cover
/// it. The state check is kept for a structural reason rather than a defensive one: it is
/// where `started_at` comes from, and the floor above cannot be computed without it.
/// **Capture trip:** one pod with a slow `startupProbe` that has not passed separates all
/// three readings at once.
///
/// **No condition, no finding.** A pod whose `Ready` condition has not been written has no
/// since-when to test against, and the safe answer there is silence rather than the version
/// of this rule that has no clock at all.
fn running_but_not_ready(now: &Time, pod: &PodSnapshot, c: &ContainerSnapshot) -> Option<Finding> {
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
        // `screens/alerts.md` and `screens/once.md` both draw this card, word for word.
        // Two renderers and one rule already share these strings; the screen spec and the
        // rule that fills it must not be a third place they can drift.
        title: "Running, but not receiving traffic — the readiness check is failing".to_string(),
        evidence: [format!("container {}", c.name), c.image.clone()].join(FACTS),
        action: "check the readiness probe: the path, the port, and whether the application \
                 answers it yet"
            .to_string(),
        kubectl_cmd: describe(&pod.id),
        owner: pod.owner.clone(),
        object: pod.id.clone(),
        timestamp: Some(since.clone()),
    })
}

/// **Rule 8 — a mount that hands the container the machine, not a directory.** CRITICAL,
/// and only the escalated case: `/`, the runtime socket, or a writable host directory
/// (NOTES § v1 rule set, *Severity escalators*). The plain read-only hostPath is how every
/// CNI, CSI and node agent is *supposed* to work and belongs to the Analysis posture rows,
/// not to a work queue (NOTES § D2).
///
/// **What the container gets is `path` joined with the mount's `subPath`**, never `path`
/// alone: `hostPath: /var/run` with `subPath: docker.sock` is a bind of the socket itself
/// and the escalator has to see it (NOTES § D46). The join cuts both ways and that is
/// correct — `hostPath: /` with `subPath: run/containerd` is a mount of `/run/containerd`
/// and is not the node's root.
///
/// **Node infrastructure in `kube-system` is silent on the writable escalator alone.**
/// Every CNI agent, kube-proxy and control-plane component writes to the node by
/// construction, so the rule as written fires CRITICAL on a healthy cluster — kindnet and
/// kube-proxy on every node, and `etcd` on every control plane. Narrowing to *DaemonSet-owned*
/// is not enough on its own: `etcd`, `kube-apiserver` and `kube-controller-manager` are
/// **mirror pods**, owned by a Node and not by any workload
/// ([`PodSnapshot::mirror`], NOTES § D39). Both shapes are node infrastructure by
/// construction, and both are exempted; nothing else is.
///
/// **The other two escalators fire straight through that silence**, because neither is
/// normal even for a node agent: a CNI plugin needs `/etc/cni/net.d`, not `/`, and nothing
/// in `kube-system` needs the runtime socket.
///
/// **The socket escalator has no capture behind it.** Nothing committed mounts
/// `docker.sock` — `hostpath.json` was photographed with `/` and a `subPath` of
/// `run/containerd` instead — so deleting that branch leaves the suite green, and it is the
/// second of the two places in this box where that is true. **Capture trip:** a pod in
/// `scripts/broken.yaml` with `hostPath: /var/run` and `subPath: docker.sock`, which is
/// NOTES § D46's own example and would prove the join and the escalator in one object. The
/// list this branch reads is proven *reachable* — every entry of [`RUNTIME_SOCKETS`] is
/// asserted to be in the form [`mounted_path`] produces, so none of them is a constant that
/// could never match — but nothing proves the branch itself fires, which is a different
/// claim.
///
/// **The narrowing is `kube-system` only, and that is a known limit.** A CSI driver in
/// `longhorn-system` mounts writable host paths just as legitimately and gets a card it has
/// not earned. Widening it needs a signal this snapshot does not carry — the plan's
/// narrowing is deliberate and is not quietly extended here.
///
/// No age, and not because none could be computed: `spec.volumes` is immutable, so the
/// pod's creation time *is* when the mount became dangerous. The card describes a standing
/// property rather than an event, and a date beside it sends the reader looking for a change
/// that never happened ([`Finding::timestamp`]).
fn escalated_host_path(pod: &PodSnapshot) -> Vec<Finding> {
    let node_agent = pod.id.namespace.as_deref() == Some(NODE_NAMESPACE)
        && (pod.mirror || pod.owner.kind == ObjectKind::DaemonSet);
    pod.host_path_mounts
        .iter()
        .filter_map(|m| {
            let path = mounted_path(m);
            // The three escalators, and the order matters: the two that are about *what*
            // is mounted are asked first, so they answer for a node agent that the
            // writable one stays quiet about.
            let (title, action) = if path == "/" {
                (
                    "A container has the whole filesystem of the machine it runs on mounted \
                     inside it",
                    "mount only the directory the container actually needs, not the root",
                )
            } else if RUNTIME_SOCKETS.contains(&path.as_str()) {
                (
                    "A container can drive the container runtime, which is full control of \
                     that machine",
                    "remove the mount — a read-only bind of this socket is still full control",
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
/// mount's `subPath` — or by its [`sub_path_expr`](HostPathMount::sub_path_expr), which
/// joins the same way and stays unresolved on purpose, so that `/` narrowed by
/// `$(POD_NAME)` reads as `/$(POD_NAME)` and stops being the node's root. Upstream forbids
/// both at once, so the `or` picks whichever exists.
///
/// **The result is normalised, and the three string compares above it only mean what they
/// read as if it is.** `hostPath: {path: "//"}` passes upstream validation — it is
/// absolute and contains no backsteps — and resolves to `/` on the node, but `"//" == "/"`
/// is false, so an unnormalised rule drops the node's whole root filesystem into the
/// writable branch: silenced outright in `kube-system`, and elsewhere advised with *"mount
/// it read-only if the container only needs to read it"*. `/.` is the same trick. So:
/// repeated separators collapsed, `.` elements dropped, trailing separator gone.
///
/// `..` is deliberately **not** resolved. `ValidatePathNoBacksteps` rejects it in a
/// hostPath and `validateLocalDescendingPath` rejects it in a subPath, so it cannot arrive;
/// if it ever did, leaving it in the string matches no escalator and lands in the writable
/// branch, which is the safe direction for a path this function would have to guess at.
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
    // An absolute path keeps its leading separator and a root that emptied out is `/`.
    // A relative one cannot come off the API — `hostPath.path` must be absolute — and is
    // returned as it arrived rather than being given a root it never had.
    if joined.starts_with('/') {
        format!("/{kept}")
    } else {
        kept
    }
}

/// **Rule 12 — the pod was asked to shut down and is still here.** WARN: nothing is down,
/// but an operation somebody started has not finished, and until it does the replacement
/// pod does not start and the node does not drain.
///
/// **`deletionTimestamp` is a deadline, not a moment.** The API server writes *request time
/// plus the grace period* (NOTES § D46), so the pod is overdue once `now` passes the field
/// itself — a rule reading it as the moment of the request would double its own threshold.
///
/// **The trigger carries a margin of [`OVERDUE_MARGIN`]**, or the pod's own grace period
/// where that is longer (NOTES § D55): a pod is briefly overdue between its deadline and
/// the kubelet's SIGKILL landing, and a laptop running fast makes every recently deleted
/// pod look stuck.
///
/// **The age is the moment the user asked**, `deletionTimestamp − grace`, and the
/// subtraction is `checked_sub`: a `terminationGracePeriodSeconds` of `i64::MAX` is a value
/// the live API server accepted in a dry run, and a plain `-` panics on it — anyone with
/// `create` and `delete` on pods could otherwise kill the TUI through a function invariant 5
/// says cannot fail (NOTES § D56). It answers `None` there rather than a wrong moment.
///
/// **The finalizers are the whole diagnosis.** *"A finalizer is holding it"* and *"the
/// kubelet has not confirmed it is gone"* are two causes with unrelated fixes, and
/// `kubectl describe pod` does not print finalizers at all — which is why the command beside
/// this card is `get -o yaml` and not `describe`.
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
    use k8s_openapi::api::core::v1::{Taint as ApiTaint, Toleration as ApiToleration};
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
    /// **The value is not free.** `scripts/certs-test.sh` hardcodes the same instant as
    /// "the reference `now` C1's tests ask about", extracts this literal out of this
    /// function and refuses to disagree with it, and asserts the committed certificates
    /// against it on every `just check` — `expiring-client` is 23 days from expiry there,
    /// inside C1's 30-day window, and `expired-client` is 4 days past. A different literal
    /// here and C1's arithmetic is computed from two instants, only one of which the build
    /// checks. **Moving it moves `scripts/certs-test.sh` and `scripts/make-certs.sh` in
    /// the same change** — the pin is one fact spelled in four places across two ownership
    /// rows (NOTES § D57), and the two halves cannot be repinned a turn apart without a
    /// red build in between that reads like a clock bug.
    ///
    /// It also lands after every `Time` the snapshot types *expose* — the newest is
    /// `nodes.json`'s unreachable taint at `2026-08-12T21:43:53Z`, 2h16m earlier. That is
    /// not a coincidence left to trust; it is what
    /// `the_pinned_now_is_not_before_the_captures_it_is_read_against` asserts. The
    /// captures carry four more kinds of timestamp that these types drop at ingest, and
    /// that guard's doc lists them — it is a guard over the contract, not over the JSON.
    ///
    /// **The shape of the value is the midnight after the capture day**, and both pins so
    /// far have had it: 43 minutes after the first capture's newest moment, 2h16m after
    /// this one's. Near enough that a fixture's age is an age an operator would recognise
    /// — a pin a week out would make every finding in the suite ancient and every
    /// below-threshold rule case unwritable — and round enough to be repeated in three
    /// other files without transcription error.
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

    /// D27's blind spot: this pod's app container is fine and the init one is dead, and
    /// reading only `containerStatuses` produces no finding at all.
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

    /// Rule 8 fires on `/`, docker.sock or a writable mount, and the Phase 4 posture
    /// report lists the read-only ones — so the decode carries the fact and not a verdict.
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
    /// **The grace is `Some` for exactly one field, and that is the whole point of the
    /// third slot.** Eight of the nine labels the captures fill are moments that have
    /// already happened, so the value *is* the moment.
    /// [`PodSnapshot::deletion_timestamp`] is a **deadline** —
    /// the apiserver writes request time *plus* grace — so it legitimately points at the
    /// future for every pod inside its grace period, which is rule 12's negative fixture,
    /// the "shutting down normally, do not alert" case. Comparing the deadline itself
    /// against `now` rejects that pod and blames the user's clock.
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
            for c in [&p.scheduled, &p.ready].into_iter().flatten() {
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

    /// **A pin behind the timestamps the snapshot exposes makes every duration in the
    /// suite run backwards, and nothing else here would notice.** `now` is the user's
    /// laptop and the fixture timestamps are the API server's, so a pin earlier than
    /// they are is D55's *slow* half — the laptop behind the cluster — entered by
    /// construction and permanently: rule 12 would compute
    /// "asked to shut down in 43 minutes", C1 "expires in -3 days", and the renderer would
    /// draw the whole suite as "just now" — the branch that exists for a machine with a
    /// wrong clock. Every other assertion in this file reads a field; not one of them
    /// subtracts two times, so this is the only place the pin can be wrong out loud.
    ///
    /// **The sweep is labelled, not counted.** "61 timestamps, all fine" cannot tell nine
    /// fields walked from one field walked sixty-one times, and a sweep that reached
    /// nothing prints the same green line as one with nothing to reach (CLAUDE.md — a
    /// derived list asserts it found something). So the labels reached are asserted to
    /// cover the ones the captures fill, and each walk is named separately: deleting any
    /// one of them turns this red.
    ///
    /// **What that does *not* buy is a guard against a new field, and the distinction is
    /// the whole of what this test is worth.** A new *variant* is caught by the compiler,
    /// not by the assertion: the sweep's `match &c.state` is exhaustive, so adding
    /// `Paused { paused_at }` to [`ContainerState`] fails there and nowhere else — one
    /// error, ``error[E0004]: non-exhaustive patterns: `&rules::ContainerState::Paused
    /// { .. }` not covered``.
    /// A new **field** is caught by nothing. Adding `creation_timestamp: Option<Time>` to
    /// [`PodSnapshot`] and decoding it in `From<Pod>` leaves this test green on the same
    /// nine labels, with a `Time` in the snapshot that no assertion has ever compared
    /// against `now`. That is the likely case, not the exotic one: all nine fields D46
    /// added and all six D51 corrected arrived exactly that way. **A box that adds a
    /// `Time` to these types adds its walk here in the same change**, and no mechanism
    /// will remind it.
    ///
    /// **This is a guard over the contract, not over the captures, and the gap between
    /// those two is four fields.** The JSON carries timestamps these types drop at
    /// ingest, so the pin is asserted against none of them: `metadata.creationTimestamp`
    /// on every object (`ObjectId` is kind, namespace, name and uid, and no v1 rule
    /// reads an object's age), a pod's `status.startTime`, and the two [`Condition`]
    /// keeps no room for — `NodeCondition.lastHeartbeatTime`, `23:16:13Z` in
    /// `nodes.json` and the likeliest of the four to arrive, since N1's "how long has
    /// this node been unreachable" is what it answers, and
    /// `DeploymentCondition.lastUpdateTime`. All four sit before the pin today; nothing
    /// asserts that they do, and NOTES § D42 lets Phase 4 add any of them — **the
    /// walk arrives in the same change as the field**, which is the rule stated above
    /// with the four names it applies to first.
    ///
    /// **`node.taint.added_at` is the field this sweep predicted and has now caught.** It
    /// reached nothing while the only committed taint was the control plane's, and the
    /// comment below said so and left the assertion a superset for exactly that reason.
    /// `break-nodes` cordons a worker and stops a third one's kubelet, and the node
    /// controller stamps `timeAdded` on every taint it writes — three of them across
    /// those two nodes — so the label is filled today and is asserted with the rest. What that surfaced is a **correction to
    /// [`Taint::added_at`]'s own doc**, which said upstream writes the field only for
    /// `NoExecute` taints: `nodes.json` carries `node.kubernetes.io/unschedulable`,
    /// `NoSchedule`, *with* a `timeAdded`, and the operator's own
    /// `dedicated=gpu:NoExecute`, applied with `kubectl taint`, **without** one. The
    /// division is who wrote the taint and not which effect it has; that doc now says so,
    /// and what it means for N2 — which can date a cordon after all — is NOTES'.
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

        // A superset, not an equality: reaching *more* than this is a new walk over a
        // field the captures started filling, which is right and must not be a red build.
        // That is not hypothetical — it is what `node.taint.added_at` did on the capture
        // of 2026-08-12, when `break-nodes` first cordoned a node and stopped a kubelet;
        // an exact-set assertion would have failed with nothing wrong. It is in the list
        // now, because a field the captures fill is a field this has to keep reaching.
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
            // What has to be in the past is the moment the thing *happened*. For eight of
            // the nine labels that is the value; for the deadline it is the value minus
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

    /// **The two snapshots that decode identically and mean opposite things.** N2 and N5
    /// both join every pod on a node, so both are disabled under a namespace scope and
    /// say so rather than computing a partial answer (D43). A rule is a pure function
    /// with no globals, so the only way it can know is this field — and without it "a
    /// small cluster" and "one namespace of a big one" are the same value: `node-3`
    /// cordoned with 40 pods, none of them in `payments`, N2 counts zero and files
    /// nothing. A missing finding, with nothing on the screen to show it happened.
    ///
    /// One value, two producers: `--namespace payments` and the 403 fallback on the
    /// cluster-wide pod LIST. To a rule they are the same fact, which is why it is a
    /// namespace and not a flag naming which of them set it.
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
    /// asserted, and that leaves a decode reading the status only for keys the spec names
    /// indistinguishable from this one.** It is also **reachable**, which is a correction:
    /// no committed capture contains such a key (all 23 were scanned) and none of the
    /// obvious manifests produce one, but "a shape nobody could produce" was too strong.
    /// A kubelet enacts a key the container spec never declared under pod-level resources
    /// (KEP-2837): `kuberuntime_container_linux.go`'s `getMemoryLimit` puts the *pod's*
    /// memory limit on a container cgroup whose own limit is unset, the runtime reports it
    /// back, and `convertContainerStatusResources` copies it in: nothing there tests
    /// whether the spec declared that key, only that `resources.Limits != nil` — one
    /// limit of any kind is enough to open the block, and the memory branch inside it is
    /// unconditional where the cpu ones at least compare against `MinMilliCPULimit` /
    /// `MinShares` first. Pod `limits: {memory: 128Mi}` with container
    /// `limits: {cpu: 100m}` therefore decodes a memory limit no container asked for.
    ///
    /// What *is* structural is the other direction, and it is stronger than counting
    /// captures: the status map begins life as `allocatedContainer.Resources.DeepCopy()`,
    /// so it holds every allocated key, and `validateContainerResize` forbids *removing*
    /// one on a resize ("resource requests cannot be removed") while permitting an
    /// addition — so the **spec's** key set is the superset-or-equal, growing past the
    /// allocated one and never shrinking below it. That is what makes the shape this test
    /// does assert legitimate rather than invented.
    /// **Capture trip:** two objects — the pod the test above is waiting for, whose memory
    /// limit was patched onto a node that cannot fit it and captured with the resize still
    /// pending; and, for the mirror shape, a pod declaring `spec.resources.limits.memory`
    /// whose container declares a cpu limit and no memory limit.
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
        }
        assert!(
            RUNTIME_SOCKETS.contains(&"/run/containerd/containerd.sock"),
            "kind — the cluster every fixture here came off — runs containerd, so a list \
             that stops at Docker's socket is a rule that cannot fire on its own test bed"
        );
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

        let image = only(&all, "broken-image", "image could not be downloaded");
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

    /// Rule 8's positives, both of them, on one captured pod — and the read-only mount of a
    /// path that is not the node's root, which is the Analysis posture row and not a card.
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
        // gets is `/run/containerd` — not the node's root. It fires on the writable
        // escalator instead, and the path it names is the one the container has.
        let writable = only(&all, "broken-hostpath", "change files on the machine");
        assert!(
            writable.evidence.contains("/run/containerd on the node"),
            "the subPath narrows what is mounted and the card has to say what the container \
             really got (D46): {}",
            writable.evidence
        );
        assert!(
            !writable.evidence.contains("/ on the node"),
            "a rule reading `path` alone would call this a mount of the node's root: {}",
            writable.evidence
        );
        assert!(
            writable.evidence.contains("writable"),
            "{}",
            writable.evidence
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

    /// **What this box deliberately does not answer**, asserted so that the next box has a
    /// test that goes from red to green rather than a claim in a commit message.
    #[test]
    fn the_rules_this_box_does_not_carry_are_silent_and_not_half_built() {
        // Rules 1–6 over `initContainerStatuses` is the box after next (D27). This pod's
        // init container is in `Init:CrashLoopBackOff` with fifteen restarts and its app
        // container is merely waiting on it — so the pod produces nothing today, which is
        // exactly the blind spot D27 describes and the reason it is a box.
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
        nothing(
            &findings(&["init"]),
            "reading `initContainerStatuses` is the box after next, and half-building it \
             would leave a finding that cannot say which container it is about",
        );

        // Rule 10 (Pending, and why) is the very next box, and its input is already
        // decoded and already asserted — it is the rule that is missing, not the field.
        let pending = pod("pending");
        assert_eq!(
            pending.scheduled.as_ref().and_then(|c| c.reason.as_deref()),
            Some("Unschedulable"),
        );
        nothing(&findings(&["pending"]), "rule 10 has its own box");
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
            11,
            "two on the crash loop, two on the OOM, one image, one config, two host mounts, \
             one readiness, one restart counter, one terminating: {:?}",
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
