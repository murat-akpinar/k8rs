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
    /// The kind as the API reported it — **or**, for the one subject with no API
    /// object behind it, what the thing is: rule C1's identity is
    /// `Other("kubeconfig")`, namespace `None`, name = the kubeconfig **context
    /// name** (the identifier the user recognises), uid `None`.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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
}
