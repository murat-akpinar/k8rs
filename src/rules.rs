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
    ///   satisfying rules 1, 5, 6 and 7 at once, so counting findings draws "4 of 5
    ///   pods" for a single sick pod — wrong by 4×, in the direction that teaches a
    ///   beginner not to believe the screen.
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
/// So **N2 can say "cordoned 2 hours ago"** — the timestamp is in the object — and the
/// `Option` is here for the taint somebody applied by hand, which is the one that has no
/// time to give.
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
    /// certificate's `notAfter`; the "4 min ago" on the Alerts screen is the *renderer*
    /// subtracting a timestamp the finding carried **from it** — that way round, or the
    /// age is negative on a healthy cluster (D18's second
    /// consequence, and the next box's). None of them calls a clock, because
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

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{Taint as ApiTaint, Toleration as ApiToleration};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;
    use k8s_openapi::jiff::SignedDuration;
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
                    read_only: false,
                    container: "nosy".to_string(),
                },
                HostPathMount {
                    path: "/".to_string(),
                    sub_path: None,
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
}
