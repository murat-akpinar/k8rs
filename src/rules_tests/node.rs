//! `rules.rs` § THE NODE RULES — its tests (NOTES § D91).

use super::*;

// --- THE NODE RULES, AGAINST THE COMMITTED CAPTURES ---
//
// `scripts/cluster.sh break-nodes` gives each worker exactly one broken state, so three of
// the six rules have a real positive: `k8rs-worker3`'s kubelet stopped posting (N1),
// `k8rs-worker` is cordoned with pods on it (N2), and `k8rs-worker2` carries an operator's
// `dedicated=gpu:NoExecute` (N6). The other three cannot be captured off a healthy-enough
// cluster — no node in it is under pressure, no kubelet is behind the control plane, and
// nothing is over-promised — so those are planted into a **decoded copy** the same way rule
// 8's socket escalators are, one coherent group of fields at a time (NOTES § D40, § D53).
//
// **The negatives are the half that matters here**, because five of the six rules join the
// pods to a node and a join is the easiest thing in this file to get quietly wrong: a count
// that includes what a drain would never move fires N2 on every node an operator drained
// correctly, and a sum that maxes a sidecar instead of adding it reports a full node healthy.

/// One captured node with one field moved — [`capture_but`]'s counterpart for the object the
/// N-series is about. The committed JSON is never touched (NOTES § D53).
fn node_but(name: &str, edit: impl FnOnce(&mut Node)) -> NodeSnapshot {
    let mut object: Node = serde_json::from_value(captured_item(&fixture("nodes"), name).clone())
        .unwrap_or_else(|e| panic!("{name} is not a Node in nodes.json: {e}"));
    edit(&mut object);
    NodeSnapshot::from(object)
}

/// One condition of a captured node, to be written through — [`condition_of`]'s node twin.
fn node_condition_mut<'a>(node: &'a mut Node, type_: &str) -> &'a mut NodeCondition {
    node.status
        .as_mut()
        .expect("a captured node has a status")
        .conditions
        .iter_mut()
        .flatten()
        .find(|c| c.type_ == type_)
        .unwrap_or_else(|| panic!("the capture carries no {type_} condition"))
}

/// **One captured pod on `node` that a drain would still have to move** — the live half of the
/// N2 counts below, read out of the capture rather than named. Which worker the scheduler put a
/// pod on is its business and moves on every `just fixtures`, so a name here asserts the trip
/// that happened rather than the requirement.
fn a_pod_a_drain_would_move_on(node: &str) -> PodSnapshot {
    CAPTURED_PODS
        .iter()
        .map(|n| pod(n))
        .find(|p| p.node.as_deref() == Some(node) && a_drain_would_move(p) && !finished(p))
        .unwrap_or_else(|| {
            panic!(
                "no captured pod on {node} that a drain would move, so the count below is untested"
            )
        })
}

/// The pods of a snapshot that are **running** on one node, by the field the join is made on —
/// [`pods_on`]'s expectation, re-derived. A pod that has finished keeps its `nodeName` until
/// something collects it (`succeeded.json`, `failed.json`), and every N-series count is about
/// work the machine is still doing, so the phase filter belongs on both sides of the comparison.
fn on_node<'a>(pods: &'a [PodSnapshot], node: &str) -> Vec<&'a PodSnapshot> {
    pods.iter()
        .filter(|p| p.node.as_deref() == Some(node) && !finished(p))
        .collect()
}

/// **N1, and the gap it was written to close** (NOTES § D71). The capture's own `healthy` pod
/// runs on the node whose kubelet stopped posting, and its status is a fossil: `Running`,
/// `ready: true`, no restarts, forever. Every other rule in this file reads pod status, so
/// without this card the workload that is actually offline produces nothing at all and Alerts
/// says a node is down in one place and nothing about what went down with it.
///
/// **The evidence names owners, not a count** — that is N2's question, and this card's job is to
/// hand the reader a workload to go and check, because no other card will.
#[test]
fn the_node_that_went_quiet_names_the_workloads_that_went_with_it() {
    let raw = fixture("nodes");
    let quiet = the_quiet_node(&raw);
    let pods = every_captured_pod();
    let all = analyze(&cluster(pods.clone(), captured_nodes()));
    show(&all);

    // The fossil, first: nothing else on the screen mentions this node's workload.
    let here = on_node(&pods, quiet);
    assert!(
        here.len() >= 4,
        "the node that stopped answering is carrying real work, or this rule is being \
         proved on an empty machine: {}",
        here.len()
    );
    // **Which pod is the fossil belongs to the scheduler**, so it is found by the property
    // rather than by name: still `Running`, every container still `ready`, on a machine the
    // control plane has given up on. A capture where the scheduler happened to place things
    // differently must not redden a requirement that never moved (NOTES § D65).
    let fossil = here
        .iter()
        .find(|p| p.phase.as_deref() == Some("Running") && p.containers.iter().all(|c| c.ready))
        .expect(
            "the node break-nodes stopped is carrying a pod whose status still reads healthy — \
             without one, D71's premise is not in the capture at all",
        );
    println!(
        "{} on {quiet}: phase {:?}, ready {:?}",
        fossil.id.name,
        fossil.phase,
        fossil
            .containers
            .iter()
            .map(|c| c.ready)
            .collect::<Vec<_>>()
    );
    assert!(
        !all.iter()
            .any(|f| f.object.kind == ObjectKind::Pod && f.object.name == fossil.id.name),
        "and no pod rule fires for it, because every one of them reads that fossil: \
         without this node card Alerts is silent about the workload that is actually \
         offline (D71): {:?}",
        titles(&all)
    );

    let card = only(&all, quiet, "stopped responding");
    assert_eq!(
        card.severity,
        Severity::Critical,
        "a machine the control plane cannot reach is broken now, not risky later (D2)"
    );
    assert_eq!(
        card.owner, card.object,
        "a node has no owner to file under (D39)"
    );
    assert_eq!(card.object.kind, ObjectKind::Node);
    assert_eq!(
        card.object.namespace, None,
        "a node is cluster-scoped, and `infra/node-3` is a card nobody can act on"
    );

    // The requirement re-derived, not the implementation re-read: up to two owners
    // alphabetically, then how many were left out, then the total pod count beside it.
    let mut owners: Vec<String> = here
        .iter()
        .map(|p| match &p.owner.namespace {
            Some(ns) => format!("{ns}/{}", p.owner.name),
            None => p.owner.name.clone(),
        })
        .collect();
    owners.sort();
    owners.dedup();
    assert!(
        owners.len() > 2,
        "the capture has to reach past the two-name cap for the `and N more` half to be \
         proved at all: {owners:?}"
    );
    assert_eq!(
        card.evidence,
        format!(
            "{}, {} and {} more were running here ({} pods)",
            owners[0],
            owners[1],
            owners.len() - 2,
            here.len()
        ),
        "`screens/alerts.md` § N1 — two names, then a count, and the total in brackets"
    );

    assert_eq!(
        card.timestamp,
        Some(captured_time(
            captured_condition(captured_item(&raw, quiet), "Ready"),
            &["lastTransitionTime"]
        )),
        "the `Ready` condition's own transition — the moment the node stopped being one"
    );
    assert_eq!(
        card.age(&now()).as_deref(),
        Some("47 min ago"),
        "a duration off the pinned now, not English parsed back into a number"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some(format!("kubectl describe node {quiet}").as_str()),
        "`describe node` prints the conditions with their reasons and the pods the node is \
         carrying — both halves of this card (invariant 4)"
    );

    // And the three nodes that are answering draw nothing of their own.
    let node_cards: Vec<&str> = all
        .iter()
        .filter(|f| f.object.kind == ObjectKind::Node)
        .map(|f| f.object.name.as_str())
        .collect();
    println!("node cards: {node_cards:?}");
    assert!(
        !node_cards.contains(&"k8rs-worker2"),
        "a node that is Ready, uncordoned and under no pressure has no card: {node_cards:?}"
    );
}

/// **The five minutes are Kubernetes' own, and both sides of them are tested.** A node the
/// control plane has not heard from for four minutes is a kubelet restart, a node upgrade or a
/// network blip, and every one of those resolves without anybody being paged.
#[test]
fn a_node_is_given_the_same_five_minutes_kubernetes_gives_it() {
    let raw = fixture("nodes");
    let quiet = the_quiet_node(&raw);
    let stopped = captured_time(
        captured_condition(captured_item(&raw, quiet), "Ready"),
        &["lastTransitionTime"],
    );
    let at = |secs: i64| {
        let moment = Time(
            stopped
                .0
                .checked_add(SignedDuration::from_secs(secs))
                .expect("a few minutes after the capture is a moment"),
        );
        analyze(&ClusterSnapshot {
            now: moment,
            ..cluster(every_captured_pod(), captured_nodes())
        })
    };

    let inside = at(300);
    assert!(
        !inside
            .iter()
            .any(|f| f.title.contains("stopped responding")),
        "exactly five minutes in is still the window Kubernetes itself waits before it \
         moves anything: {:?}",
        titles(&inside)
    );
    let outside = at(301);
    assert!(
        outside
            .iter()
            .any(|f| f.title.contains("stopped responding")),
        "one second past it is an outage: {:?}",
        titles(&outside)
    );

    // **The number is borrowed, not picked** — and the capture carries the proof, on every
    // pod: `--default-unreachable-toleration-seconds` is what the admission controller writes,
    // and it is how long Kubernetes waits before it starts evicting from a node it cannot
    // reach. `Toleration` deliberately drops `tolerationSeconds`, so this reads the JSON.
    let tolerations = fixture("crashloop")["spec"]["tolerations"].clone();
    let unreachable = tolerations
        .as_array()
        .into_iter()
        .flatten()
        .find(|t| t["key"] == "node.kubernetes.io/unreachable")
        .expect("the admission controller writes this onto every pod in the cluster");
    println!("{unreachable}");
    assert_eq!(
        NODE_DOWN_GRACE.as_secs(),
        unreachable["tolerationSeconds"]
            .as_i64()
            .expect("the toleration carries its seconds"),
        "N1's window is the one the cluster itself is running with"
    );
}

/// **A kubelet that answered and said no is not a kubelet that went quiet**, and the card may
/// not say *"has stopped responding"* about a machine that is talking (invariant 14). The
/// kubelet's own sentence is the diagnosis on that branch, so it is carried verbatim
/// (NOTES § D37) — where a silent node has no sentence to carry.
///
/// **Planted:** no captured node is `Ready: False`. `break-nodes` stops a kubelet, which
/// produces `Unknown`; `False` is what a live kubelet writes when its container runtime or its
/// network will not come up, and the message below is `pkg/kubelet/kubelet.go`'s own
/// `runtimeState` sentence. **Capture trip:** a node with a broken CNI retires this.
#[test]
fn a_node_that_answered_and_said_no_is_a_different_card_from_one_that_went_quiet() {
    let refusing = node_but("k8rs-worker2", |n| {
        let ready = node_condition_mut(n, "Ready");
        ready.status = "False".to_string();
        ready.reason = Some("KubeletNotReady".to_string());
        ready.message = Some(
            "container runtime network not ready: NetworkReady=false reason:NetworkPluginNotReady \
             message:Network plugin returns error: cni plugin not initialized"
                .to_string(),
        );
        ready.last_transition_time = Some(time("2026-08-12T21:00:00Z"));
    });
    let pods = every_captured_pod();
    let all = analyze(&cluster(pods.clone(), vec![refusing]));
    show(&all);

    let card = only(&all, "k8rs-worker2", "cannot run pods");
    assert_eq!(card.severity, Severity::Critical);
    assert!(
        !card.title.contains("stopped responding") && !card.action.contains("powered on"),
        "the machine is answering — asking whether it is powered on wastes the first \
         thing the reader does: {} / {}",
        card.title,
        card.action
    );
    // **Framed the way rule 10 frames the scheduler's sentence** (D81): glued straight on to the
    // owner list with a `·`, a kubelet's `NetworkReady=false reason:NetworkPluginNotReady` reads
    // as k8rs's own prose, and the reader meets four pieces of jargon with nothing saying who
    // wrote them.
    assert!(
        card.evidence.contains(
            "the kubelet's own words (the kubelet is the part of Kubernetes that runs on the \
             machine): container runtime network not ready"
        ),
        "the kubelet said what is wrong; the frame says a machine wrote it and glosses the one \
         word the card would otherwise leave bare (D37, invariant 14): {}",
        card.evidence
    );
    let here = on_node(&pods, "k8rs-worker2");
    assert!(
        card.evidence
            .contains(&format!("are running here ({} pods)", here.len())),
        "and the tense follows: these pods are still reporting, because the kubelet that \
         reports them is up: {}",
        card.evidence
    );
}

/// **An undated condition still draws the card**, rule 10's direction and not rule 13's: a node
/// that cannot be *shown* to have just gone quiet is read as one that has been quiet, which is
/// the safe direction — and the right edge is empty rather than borrowed from somewhere else.
///
/// **Planted:** every captured condition carries its stamp; a prune that dropped the field is
/// what this is about, and no capture can hold one (invariant 6).
#[test]
fn a_node_whose_condition_carries_no_stamp_still_draws_the_card_without_an_age() {
    let raw = fixture("nodes");
    let quiet = the_quiet_node(&raw).to_string();
    let undated = node_but(&quiet, |n| {
        node_condition_mut(n, "Ready").last_transition_time = None;
    });
    let all = analyze(&cluster(every_captured_pod(), vec![undated]));
    show(&all);

    let card = only(&all, &quiet, "stopped responding");
    assert_eq!(card.timestamp, None);
    assert_eq!(
        card.age(&now()),
        None,
        "no field to point at is the empty right edge, never a zero that draws as 1970"
    );
}

/// **N2, and the count that is its trigger.** The cordoned node in the capture carries a mix of
/// ordinary pods and node agents, and a drain would move only the first kind: `kubectl drain`
/// never evicts a DaemonSet pod or a static pod, whatever flags it is given, so counting what
/// runs there would put this card on every node an operator drained perfectly (NOTES § D46).
/// Both counts are re-derived from the capture below rather than written down, because which
/// pods the scheduler put on the cordoned node is its business and moves on every trip.
#[test]
fn the_cordoned_node_counts_only_the_pods_a_drain_would_actually_move() {
    let raw = fixture("nodes");
    let cordoned = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|n| n["spec"]["unschedulable"] == true)
        .map(|n| captured_str(n, &["metadata", "name"]))
        .expect("`break-nodes` cordons one worker, and N2's positive is that node");
    let pods = every_captured_pod();
    let all = analyze(&cluster(pods.clone(), captured_nodes()));
    show(&all);

    let here = on_node(&pods, cordoned);
    // Re-derived from the pod's own fields rather than by calling [`a_drain_would_move`], which
    // would agree with any narrowing it happened to have: the three `kubectl drain` skips are a
    // static pod, a DaemonSet's pod, and one the drain has already evicted.
    let skipped: Vec<&str> = here
        .iter()
        .filter(|p| {
            p.mirror || p.owner.kind == ObjectKind::DaemonSet || p.deletion_timestamp.is_some()
        })
        .map(|p| p.id.name.as_str())
        .collect();
    println!("{} pods on {cordoned}, drain skips {skipped:?}", here.len());
    assert!(
        !skipped.is_empty(),
        "kindnet and kube-proxy run on every kind node, and without them in the snapshot \
         this test cannot tell a filtered count from an unfiltered one"
    );

    let card = only(&all, cordoned, "refuses new pods");
    assert_eq!(card.severity, Severity::Warn);
    assert_eq!(card.title, "This node refuses new pods (cordoned)");
    assert_eq!(
        card.evidence,
        format!(
            "{} pods here would still have to move",
            here.len() - skipped.len()
        ),
        "the number a `kubectl drain` would actually move — the same computation the next \
         command the reader types performs (`screens/alerts.md`)"
    );
    assert_eq!(
        card.action, "allow new pods once the work is done",
        "it states the lifecycle and does not accuse: true whether the cordon was five \
         minutes ago or five months ago"
    );

    // **The command has to be able to show the number beside it.** `kubectl describe node`
    // prints `Taints:` and never `timeAdded`, so this one card does not point at it (D69).
    // **`describe node`, not the jsonpath line** (D81 reversing D69's other horn): it prints
    // `Unschedulable: true` and the `Non-terminated Pods` table, which are the title and the
    // count — and the count is the trigger, so it is on every one of these cards.
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some(format!("kubectl describe node {cordoned}").as_str()),
        "the age is the one claim this command cannot back, and it is the optional half"
    );
    assert_eq!(
        card.timestamp,
        Some(captured_time(
            captured_item(&raw, cordoned)["spec"]["taints"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|t| t["key"] == CORDON_TAINT)
                .expect("the node lifecycle controller mirrors the boolean onto a taint"),
            &["timeAdded"]
        )),
        "the age is the taint's, which the controller stamps — never `Ready`'s, which does \
         not move when a node is cordoned (D65)"
    );
    assert_eq!(card.age(&now()).as_deref(), Some("47 min ago"));
}

/// **A node a drain finished with is parked, not broken** — and both of the two shapes a drain
/// refuses to move are in the capture, on two different nodes. Counting either of them is what
/// puts N2 on a correctly drained node, which is the false positive the narrowing exists for
/// (NOTES §  D43, § D46).
///
/// **Half planted:** the control plane is not cordoned in the capture, so the boolean is moved
/// on to a decoded copy of it — one field, and the field is a `kubectl cordon` away.
#[test]
fn a_cordoned_node_with_nothing_a_drain_would_move_draws_no_card() {
    let system: Vec<PodSnapshot> = items::<Pod>("kube-system-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();

    // The DaemonSet half, on the node the capture really did cordon.
    let raw = fixture("nodes");
    let cordoned = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|n| n["spec"]["unschedulable"] == true)
        .map(|n| captured_str(n, &["metadata", "name"]))
        .expect("one worker is cordoned in the capture");
    let agents: Vec<PodSnapshot> = on_node(&system, cordoned).into_iter().cloned().collect();
    println!(
        "{cordoned} keeps {:?}",
        agents.iter().map(|p| &p.id.name).collect::<Vec<_>>()
    );
    assert!(
        !agents.is_empty() && agents.iter().all(|p| p.owner.kind == ObjectKind::DaemonSet),
        "kindnet and kube-proxy are what a drained kind node is left running"
    );
    let drained = analyze(&cluster(agents, captured_nodes()));
    show(&drained);
    assert!(
        !drained.iter().any(|f| f.title.contains("refuses new pods")),
        "a node whose last two pods are DaemonSet pods is parked, and Alerts holds only \
         what is broken: {:?}",
        titles(&drained)
    );

    // The static-pod half: a control-plane node cordoned for an upgrade still runs four pods
    // no drain can move, and its own `coredns` replicas are deliberately left out — those a
    // drain *would* move, and this is the case where nothing is left.
    let statics: Vec<PodSnapshot> = system
        .iter()
        .filter(|p| p.mirror || p.owner.kind == ObjectKind::DaemonSet)
        .filter(|p| p.node.as_deref() == Some("k8rs-control-plane"))
        .cloned()
        .collect();
    assert!(
        statics.iter().filter(|p| p.mirror).count() >= 4,
        "the kubelet mirrors etcd, the apiserver, the scheduler and the controller manager"
    );
    let upgrading = analyze(&cluster(
        statics,
        vec![node_but("k8rs-control-plane", |n| {
            n.spec
                .as_mut()
                .expect("a captured node has a spec")
                .unschedulable = Some(true);
        })],
    ));
    show(&upgrading);
    assert!(
        !upgrading
            .iter()
            .any(|f| f.title.contains("refuses new pods")),
        "four static pods are not a half-finished drain: {:?}",
        titles(&upgrading)
    );
}

/// **N2 is silent while an autoscaler is deliberately emptying the node** — it is cordoned with
/// pods on it for the whole eviction window by design, so a card here fires repeatedly on a
/// cluster doing exactly what it was configured to do. A scale-down that never finishes is the
/// Drain safety report's row (NOTES § D43).
///
/// **Planted:** no cloud autoscaler runs on kind. Both taints are declared upstream — the
/// cluster-autoscaler one carries the unix second of the scale-down in its value, Karpenter's
/// carries no value at all — and both are `NoSchedule`.
#[test]
fn a_node_an_autoscaler_is_taking_away_is_not_a_half_finished_drain() {
    let raw = fixture("nodes");
    let cordoned = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|n| n["spec"]["unschedulable"] == true)
        .map(|n| captured_str(n, &["metadata", "name"]))
        .expect("one worker is cordoned in the capture")
        .to_string();
    let pods = every_captured_pod();

    // Not vacuous: the same node without the taint is N2's positive.
    let plain = analyze(&cluster(pods.clone(), captured_nodes()));
    assert!(
        plain.iter().any(|f| f.title.contains("refuses new pods")),
        "this node is N2's positive, or the silence below proves nothing"
    );

    for (key, value) in [
        ("ToBeDeletedByClusterAutoscaler", Some("1755037382")),
        ("karpenter.sh/disrupted", None),
    ] {
        let retiring = node_but(&cordoned, |n| {
            n.spec
                .as_mut()
                .expect("a captured node has a spec")
                .taints
                .get_or_insert_with(Vec::new)
                .push(ApiTaint {
                    key: key.to_string(),
                    value: value.map(str::to_string),
                    effect: "NoSchedule".to_string(),
                    time_added: None,
                });
        });
        let all = analyze(&cluster(pods.clone(), vec![retiring]));
        show(&all);
        assert!(
            !all.iter().any(|f| f.title.contains("refuses new pods")),
            "{key} means an operation in progress, not one that stopped half way: {:?}",
            titles(&all)
        );
    }
}

/// **N2 and N5 do not run at all when the view is one namespace** (NOTES § D43, § D46). Both
/// join every pod on a node, and a fraction of the pods turns N2's count into a silence and
/// N5's sum into a number that reads as *fine*. The screen says which check is off — Phase 9's
/// banner, and deliberately not a finding from this file.
///
/// **N1 is unaffected as a card and loses its evidence line**: the node's own condition is not
/// namespaced, but *"one pod was running here"* about a node carrying forty is the wrong
/// number this screen exists not to print.
#[test]
fn the_two_rules_that_need_every_pod_do_not_answer_from_one_namespace() {
    let raw = fixture("nodes");
    let quiet = the_quiet_node(&raw);
    let scoped = ClusterSnapshot {
        namespace_scope: Some("default".to_string()),
        ..cluster(every_captured_pod(), captured_nodes())
    };
    let all = analyze(&scoped);
    show(&all);

    assert!(
        !all.iter().any(|f| f.title.contains("refuses new pods")),
        "N2 counts what a drain would move, and it cannot see the pods to count: {:?}",
        titles(&all)
    );
    assert!(
        node_overcommitted(
            &scoped,
            scoped.nodes.first().expect("the capture has nodes")
        )
        .is_none(),
        "and N5 does not add up a fraction of a node's pods"
    );

    let card = only(&all, quiet, "stopped responding");
    assert_eq!(
        card.evidence, "",
        "the card stays — the node's condition is not namespaced — and the line that would \
         have counted from a partial list is simply not drawn: {}",
        card.evidence
    );
}

/// **N3 names every pressure the node has, and dates each from its own condition.** Reading
/// `Ready`'s `lastTransitionTime` off the same flat list is the trap this rule is one of three
/// warned about: a DiskPressure card would carry the node's boot time (NOTES § D69).
///
/// **Planted:** nothing in the capture is under pressure — the unreachable node's three read
/// `Unknown`, which is N1's answer, not this one. `True` with `KubeletHasDiskPressure` is what
/// the kubelet writes when the image filesystem crosses its eviction threshold.
#[test]
fn the_node_running_low_names_what_it_is_low_on_and_when_that_started() {
    let disk = time("2026-08-12T22:00:00Z");
    let memory = time("2026-08-12T23:00:00Z");
    let pressured = node_but("k8rs-worker2", |n| {
        let c = node_condition_mut(n, "DiskPressure");
        c.status = "True".to_string();
        c.reason = Some("KubeletHasDiskPressure".to_string());
        c.last_transition_time = Some(disk.clone());
    });
    let all = analyze(&cluster(every_captured_pod(), vec![pressured]));
    show(&all);

    let card = only(&all, "k8rs-worker2", "running low");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "evictions are coming is wrong-now-broken-soon, which is what amber means (D2)"
    );
    assert_eq!(
        card.title,
        "This node is running low on disk space — Kubernetes may start evicting pods to free \
         it up",
        "`screens/alerts.md` § N3, word for word — and `DiskPressure` is not a word a \
         beginner has met (invariant 14)"
    );
    assert_eq!(
        card.action,
        "free up disk space on this node, or move some pods elsewhere"
    );
    assert_eq!(
        card.timestamp,
        Some(disk.clone()),
        "*that* condition's transition. `Ready` on this node moved at 20:45:35Z, which is \
         when the machine booted, and a card dated by it is a card dated by nothing (D69)"
    );

    // Two at once: one card, both named, and the earlier of the two stamps it — the question
    // the age answers is how long this has been going on.
    let both = node_but("k8rs-worker2", |n| {
        for (type_, at) in [("DiskPressure", &disk), ("MemoryPressure", &memory)] {
            let c = node_condition_mut(n, type_);
            c.status = "True".to_string();
            c.last_transition_time = Some(at.clone());
        }
    });
    let all = analyze(&cluster(Vec::new(), vec![both]));
    show(&all);
    let card = only(&all, "k8rs-worker2", "running low");
    assert!(
        card.title.contains("running low on disk space and memory"),
        "naming one and hiding the other is what `screens/alerts.md` § N3 forbids: {}",
        card.title
    );
    assert!(
        card.action.contains("free up disk space") && card.action.contains("free up memory"),
        "and the action answers both, or half the node stays broken: {}",
        card.action
    );
    assert_eq!(card.timestamp, Some(disk));

    // The negative, from the capture: `Unknown` is not `True`.
    let healthy = analyze(&cluster(Vec::new(), captured_nodes()));
    show(&healthy);
    assert!(
        !healthy.iter().any(|f| f.title.contains("running low")),
        "the unreachable node's pressures read `Unknown`, and filing *evictions are coming* \
         on a machine nobody can reach is the shape this test exists to catch: {:?}",
        titles(&healthy)
    );
}

/// **N4 — the kubelet the control plane no longer supports.** `Info`, and the whole of its
/// negative side is the capture: `just fixtures` cross-checks the control plane's kubelet
/// against `tests/fixtures/K8S_VERSION`, so a fixture that acquires a skew is announced rather
/// than discovered (NOTES § D65).
///
/// **Planted:** every kubelet in the capture is the version the cluster was built at. A node
/// three minors behind is what an upgrade that stalled on one node group looks like.
#[test]
fn the_kubelet_too_far_behind_the_control_plane_to_be_supported() {
    let server = Some("v1.36.1");
    for node in captured_nodes() {
        assert_eq!(
            kubelet_too_far_behind(server, &node),
            None,
            "{} runs the version the cluster was built at",
            node.id.name
        );
    }

    let behind = |version: &str| {
        node_but("k8rs-worker2", |n| {
            n.status
                .as_mut()
                .expect("a captured node has a status")
                .node_info
                .as_mut()
                .expect("a captured node reports its kubelet version")
                .kubelet_version = version.to_string();
        })
    };
    let found = kubelet_too_far_behind(server, &behind("v1.32.0"))
        .expect("four minors behind is past the window upstream publishes");
    println!("{}", card(&found, &now()));
    assert_eq!(
        found.severity,
        Severity::Info,
        "an unsupported kubelet is a risk to answer this month, not an outage — it is the \
         Versions report's row and never an Alerts card (D2)"
    );
    assert!(
        found.evidence.contains("kubelet v1.32.0")
            && found.evidence.contains("control plane v1.36.1")
            && found.evidence.contains("4 versions behind"),
        "both numbers and the distance between them: {}",
        found.evidence
    );
    assert_eq!(
        found.kubectl_cmd.as_deref(),
        Some("kubectl get nodes -o wide"),
        "the command prints the number this card is about, for every node at once"
    );
    assert_eq!(
        found.timestamp, None,
        "nothing records when a kubelet was installed"
    );

    assert_eq!(
        kubelet_too_far_behind(server, &behind("v1.33.0")),
        None,
        "**exactly three minors behind is supported**, and this is the row the first version of \
         this rule got wrong: upstream says a kubelet may be up to three minor versions older \
         than kube-apiserver, so at two everybody mid-upgrade was told a supported cluster was \
         not (D81)"
    );
    assert_eq!(
        kubelet_too_far_behind(server, &behind("v1.37.0")),
        None,
        "a kubelet *ahead* of the control plane is a different fault and not one of the \
         eleven rules — inventing a card for it here is scope creep (invariant 13)"
    );
    assert_eq!(
        kubelet_too_far_behind(None, &behind("v1.32.0")),
        None,
        "with no control-plane version there is nothing to compare against, and comparing \
         against a guess is the one thing this rule may not do"
    );
    assert_eq!(
        SUPPORTED_SKEW, 3,
        "the number is upstream's own window and the card makes a claim about it: *kubelet may \
         be up to three minor versions older than kube-apiserver* (D81)"
    );
    assert!(
        found.action.contains("at most 3 minor versions older"),
        "and the card cites that window rather than asserting a number of its own: {}",
        found.action
    );
}

/// The version strings a real cluster answers with, none of which is `v1.36.1`.
#[test]
fn a_version_is_read_as_far_as_its_minor_and_no_further() {
    for (version, want) in [
        ("v1.36.1", Some((1, 36))),
        ("1.36.1", Some((1, 36))),
        ("v1.29.7-gke.1104000", Some((1, 29))),
        ("v1.28.15-eks-1234567", Some((1, 28))),
        ("v1.31.4+k3s1", Some((1, 31))),
        ("v1.30.0-rc.2", Some((1, 30))),
        ("", None),
        ("v1", None),
        ("kubelet", None),
    ] {
        println!("{version:>24} -> {:?}", minor_version(version));
        assert_eq!(
            minor_version(version),
            want,
            "{version} is where a distribution's own suffix meets N4's subtraction"
        );
    }
}

/// **N5's arithmetic, on the three captured pods that each break a different naive version of
/// it** (NOTES § D46, § D51). None of the three is planted: `just fixtures` captured them for
/// exactly this rule.
#[test]
fn what_a_node_is_charged_for_a_pod_is_the_number_the_scheduler_uses() {
    // Millicores, so every number below is an exact integer (D81).
    let cpu = |p: &PodSnapshot| {
        charged(
            p,
            |p| p.cpu_request.as_deref(),
            |c| c.cpu_request.as_deref(),
            |p| p.overhead_cpu.as_deref(),
        )
        .expect("every captured request parses")
    };

    // A native sidecar is *added*, never maxed: it runs beside the app for the whole life of
    // the pod. Maxing drops 100m per meshed pod, which is six CPUs invisible on sixty of them.
    let sidecar = pod("healthy-sidecar");
    assert!(
        sidecar
            .containers
            .iter()
            .any(|c| c.role == ContainerRole::Sidecar),
        "the capture declares `restartPolicy: Always` on an init container, or this proves \
         nothing: {:?}",
        sidecar
            .containers
            .iter()
            .map(|c| c.role)
            .collect::<Vec<_>>()
    );
    println!("sidecar pod charged {}m cpu", cpu(&sidecar));
    assert_eq!(
        cpu(&sidecar),
        20,
        "10m for the app and 10m for the sidecar beside it — a maxing sum answers 10m"
    );

    // A pod-level request *replaces* the container sum. The pod below asks for 100m at the pod
    // level and 10m in its one container: adding them answers 0.11, and reading only the
    // containers answers 0.01, on a pod that has committed 100m of the node.
    let pod_level = pod("healthy-podlevel");
    println!("pod-level pod charged {}m cpu", cpu(&pod_level));
    assert_eq!(
        cpu(&pod_level),
        100,
        "KEP-2837: the pod-level number is the one the scheduler charges (D51)"
    );

    // An init container that requests nothing costs nothing, and a pod with no requests at all
    // is not a pod with unknown requests.
    println!("init pod charged {}m cpu", cpu(&pod("init")));
    assert_eq!(cpu(&pod("init")), 0);

    // A quantity that cannot be read stops the node rather than being skipped: an understated
    // sum is a card that says the node is fine, which is the one wrong answer here.
    let broken = capture_but("healthy", |p| {
        p.status
            .as_mut()
            .expect("a captured pod has a status")
            .container_statuses
            .as_mut()
            .expect("the kubelet reported on this container")[0]
            .resources = None;
        p.spec
            .as_mut()
            .expect("a captured pod has a spec")
            .containers[0]
            .resources
            .as_mut()
            .expect("the capture declares requests")
            .requests
            .as_mut()
            .expect("the capture declares a cpu request")
            .insert("cpu".to_string(), Quantity("not a number".to_string()));
    });
    assert_eq!(
        charged(
            &broken,
            |p| p.cpu_request.as_deref(),
            |c| c.cpu_request.as_deref(),
            |p| p.overhead_cpu.as_deref(),
        ),
        None,
        "and it says so rather than guessing low"
    );
}

/// **The RuntimeClass charge is a third term, and a `spec`-only sum is short by it**
/// (NOTES § D46, § D124). `spec.overhead` is what a sandboxed runtime — Kata, gVisor — costs the
/// node before the pod's own containers are counted, and upstream's `resource.PodRequests` adds
/// it *after* the init/sidecar max and *after* a pod-level request has replaced the container
/// sum, so neither of the other two branches can absorb it.
///
/// **Not planted: `broken-overhead` was captured for exactly this**, and every number below is
/// read out of the capture rather than transcribed — the overhead is written by the RuntimeClass
/// admission plugin, not by the manifest, so a literal here would assert the trip that took it.
#[test]
fn the_runtime_class_overhead_is_charged_on_top_of_whatever_the_containers_ask_for() {
    let cpu = |p: &PodSnapshot| {
        charged(
            p,
            |p| p.cpu_request.as_deref(),
            |c| c.cpu_request.as_deref(),
            |p| p.overhead_cpu.as_deref(),
        )
    };
    let memory = |p: &PodSnapshot| {
        charged(
            p,
            |p| p.memory_request.as_deref(),
            |c| c.memory_request.as_deref(),
            |p| p.overhead_memory.as_deref(),
        )
    };
    let milli = |q: &str| quantity_milli(q).unwrap_or_else(|| panic!("{q} is a captured quantity"));

    let raw = fixture("overhead");
    let charge = milli(captured_str(&raw, &["spec", "overhead", "cpu"]));
    let charge_memory = milli(captured_str(&raw, &["spec", "overhead", "memory"]));
    let asked = milli(captured_str(
        &raw["spec"]["containers"][0],
        &["resources", "requests", "cpu"],
    ));
    let asked_memory = milli(captured_str(
        &raw["spec"]["containers"][0],
        &["resources", "requests", "memory"],
    ));
    assert!(
        charge > 0 && charge_memory > 0,
        "the capture still declares a RuntimeClass overhead, or this test proves nothing: \
         {charge}m cpu, {charge_memory} milli-bytes"
    );

    // **The positive and its negative are one capture and one field.** Taking `spec.overhead` off
    // the same object leaves the pod as its container alone, so the difference between the two is
    // the charge and nothing else — an assertion that read only the sum could be satisfied by a
    // container the capture happens to carry.
    let sandboxed = pod("overhead");
    let plain = capture_but("overhead", |p| {
        p.spec.as_mut().expect("a captured pod has a spec").overhead = None;
    });
    println!(
        "broken-overhead charged {:?}m cpu / {:?} milli-bytes, and {:?}m / {:?} without its \
         RuntimeClass",
        cpu(&sandboxed),
        memory(&sandboxed),
        cpu(&plain),
        memory(&plain)
    );
    assert_eq!(
        cpu(&plain),
        Some(asked),
        "with no overhead it is its container"
    );
    assert_eq!(memory(&plain), Some(asked_memory));
    assert_eq!(
        cpu(&sandboxed),
        Some(asked + charge),
        "the scheduler charges the sandbox as well as the container it holds"
    );
    assert_eq!(memory(&sandboxed), Some(asked_memory + charge_memory));

    // **A pod-level request replaces the container sum; it does not replace the overhead.**
    // Planted the D40 way, out of the corpus's own strings: `healthy-podlevel` declares its
    // request once at pod level and runs on the default runtime, and the value written on is the
    // one `overhead.json` carries in its own spec.
    let pod_level = milli(captured_str(
        &fixture("healthy-podlevel"),
        &["spec", "resources", "requests", "cpu"],
    ));
    let sandboxed_pod_level = capture_but("healthy-podlevel", |p| {
        let spec = p.spec.as_mut().expect("a captured pod has a spec");
        assert_eq!(
            spec.overhead, None,
            "the capture runs on the default runtime"
        );
        spec.overhead = Some(BTreeMap::from([(
            "cpu".to_string(),
            Quantity(captured_str(&raw, &["spec", "overhead", "cpu"]).to_string()),
        )]));
    });
    println!(
        "pod-level pod with a sandbox charged {:?}m cpu",
        cpu(&sandboxed_pod_level)
    );
    assert_eq!(
        cpu(&sandboxed_pod_level),
        Some(pod_level + charge),
        "upstream adds the overhead after the pod-level request has replaced the sum, so a \
         branch that returns early on the pod-level number drops it"
    );

    // An overhead that cannot be read stops the node rather than being skipped — the same
    // direction every other quantity in this sum takes, because an understated sum is a card
    // that says the node is fine.
    let unreadable = capture_but("overhead", |p| {
        p.spec
            .as_mut()
            .expect("a captured pod has a spec")
            .overhead
            .as_mut()
            .expect("the capture declares one")
            .insert("cpu".to_string(), Quantity("not a number".to_string()));
    });
    assert_eq!(
        cpu(&unreadable),
        None,
        "and it says so rather than guessing low"
    );
}

/// **What the change above is worth on the corpus: one node's whole sum** (NOTES § D124's first
/// condition). The node is read out of the capture — the scheduler picks it, and `broken-overhead`
/// has moved between trips.
///
/// `|_| None` is the arithmetic exactly as it was before the overhead landed, which is what makes
/// this a measurement rather than a restatement of the code.
#[test]
fn a_spec_only_sum_is_short_by_every_sandbox_charge_on_the_node() {
    let sandboxed = pod("overhead");
    let placed = sandboxed
        .node
        .clone()
        .expect("the capture records the node the scheduler gave broken-overhead");
    let node = captured_nodes()
        .into_iter()
        .find(|n| n.id.name == placed)
        .unwrap_or_else(|| panic!("the capture has no node {placed}"));
    let here: Vec<PodSnapshot> = every_captured_pod()
        .into_iter()
        .filter(|p| p.node.as_deref() == Some(placed.as_str()))
        .collect();
    let borrowed: Vec<&PodSnapshot> = here.iter().collect();

    for (dimension, of_pod, of_container, of_overhead, allocatable) in [
        (
            "cpu",
            (|p: &PodSnapshot| p.cpu_request.as_deref()) as fn(&PodSnapshot) -> Option<&str>,
            (|c: &ContainerSnapshot| c.cpu_request.as_deref())
                as fn(&ContainerSnapshot) -> Option<&str>,
            (|p: &PodSnapshot| p.overhead_cpu.as_deref()) as fn(&PodSnapshot) -> Option<&str>,
            node.allocatable_cpu.as_deref(),
        ),
        (
            "memory",
            |p: &PodSnapshot| p.memory_request.as_deref(),
            |c: &ContainerSnapshot| c.memory_request.as_deref(),
            |p: &PodSnapshot| p.overhead_memory.as_deref(),
            node.allocatable_memory.as_deref(),
        ),
    ] {
        let charge =
            of_overhead(&sandboxed).map(|q| quantity_milli(q).expect("a captured quantity"));
        let (with, has) = promised(&borrowed, allocatable, of_pod, of_container, of_overhead)
            .expect("every captured quantity on this node parses");
        let (without, _) = promised(&borrowed, allocatable, of_pod, of_container, |_| None)
            .expect("every captured quantity on this node parses");
        println!(
            "{placed} {dimension}: {} pods promise {without} spec-only against {with} with \
             overhead, of {has} allocatable",
            here.len()
        );
        assert_eq!(
            Some(with - without),
            charge,
            "{placed}'s {dimension} sum is short by exactly the sandbox charges on it"
        );
    }
}

/// **A node over-promised, out of the capture's own strings.** No node in the capture is:
/// `broken-resize` asks for the whole machine's memory and the kubelet **deferred** the resize, so
/// what it was actually given is 64Mi ([`effective`], NOTES § D51). Landing that resize is one
/// field, and the value planted is the one the same capture already carries in its own `spec`.
///
/// Shared by N5's card test and by the one that proves `analyze` leaves it out, because a rule
/// that cannot be shown to fire proves nothing about being excluded.
fn over_promised() -> ClusterSnapshot {
    let raw = fixture("resize");
    let asked_for = captured_str(
        &raw["spec"]["containers"][0],
        &["resources", "requests", "memory"],
    )
    .to_string();
    let landed = capture_but("resize", |p| {
        let status = p.status.as_mut().expect("a captured pod has a status");
        let enacted = status.container_statuses.as_mut().expect("one container")[0]
            .resources
            .as_mut()
            .expect("the kubelet enacted the original request");
        enacted
            .requests
            .as_mut()
            .expect("with a memory request in it")
            .insert("memory".to_string(), Quantity(asked_for.clone()));
    });
    assert_eq!(
        container(&landed, "app").memory_request.as_deref(),
        Some(asked_for.as_str()),
        "one field moved on a decoded copy, to the value the same capture asks for (D40)"
    );
    // **Which node it landed on is the scheduler's, so it is read rather than named** — the same
    // reason [`the_quiet_node`] exists. `broken-resize` was on `k8rs-worker3` on the 2026-08-13
    // trip and on `k8rs-worker` on the 2026-08-16 one; a literal here filtered the planted pod
    // out of its own snapshot and left N5 with nothing to fire on, which reads as a rule that
    // stopped working rather than as a pod that moved (NOTES § D114).
    let placed = landed
        .node
        .clone()
        .expect("the capture records the node the scheduler gave broken-resize");
    let node = captured_nodes()
        .into_iter()
        .find(|n| n.id.name == placed)
        .unwrap_or_else(|| panic!("the capture has no node {placed}, which broken-resize runs on"));
    let pods: Vec<PodSnapshot> = every_captured_pod()
        .into_iter()
        .chain([landed])
        .filter(|p| p.node.as_deref() == Some(placed.as_str()))
        .collect();
    assert!(
        pods.len() > 1,
        "the over-promise is one pod asking for the whole machine *plus its neighbours*, so the \
         node has to have some: {} pods on {placed}",
        pods.len()
    );
    cluster(pods, vec![node])
}

/// **N5 — the node has promised more than it has.** `Info`, and the Capacity report's input:
/// nothing is down, which is why it is not on Alerts (NOTES § D2, `screens/analysis.md`).
///
/// **Planted, out of the capture's own strings.** No node in the capture is over-promised —
/// `broken-resize` asks for the whole machine's memory and the kubelet **deferred** the
/// resize, so what it was actually given is 64Mi ([`effective`], NOTES § D51). Landing that
/// resize is one field, and the value planted is the one the same capture already carries in
/// its own `spec`.
#[test]
fn the_node_that_promised_more_than_it_has() {
    let allocatable = captured_nodes()
        .into_iter()
        .find(|n| n.id.name == "k8rs-worker3")
        .and_then(|n| n.allocatable_memory)
        .expect("a node reports what it can give");
    let snapshot = over_promised();
    let found = node_overcommitted(&snapshot, &snapshot.nodes[0])
        .expect("one pod holding the whole machine plus its neighbours is over the line");
    println!("{}", card(&found, &now()));

    assert_eq!(found.severity, Severity::Info);
    assert_eq!(found.object.kind, ObjectKind::Node);
    assert!(
        found.title.contains("promised more memory than it has"),
        "`screens/analysis.md` § Capacity words the row this feeds: {}",
        found.title
    );
    assert!(
        !found.title.contains("nothing new can start"),
        "a pod that requests nothing is placed on a node at 100% of its requests all day, and \
         a beginner who tries it must not be contradicted by their own cluster (D81): {}",
        found.title
    );
    assert!(
        found.evidence.contains(&format!(
            "usable {}",
            bytes(quantity_milli(&allocatable).expect("a captured quantity parses"))
        )),
        "measured against this node's own allocatable, in a unit a manifest is written in: {}",
        found.evidence
    );
    assert_eq!(found.timestamp, None, "an arithmetic is not an event (D69)");

    // The negative is the capture as committed: the resize is still deferred there.
    let real = cluster(every_captured_pod(), captured_nodes());
    for node in &real.nodes {
        assert_eq!(
            node_overcommitted(&real, node),
            None,
            "{} is a twelve-CPU machine running pods that ask for milli-CPUs",
            node.id.name
        );
    }
}

/// The two string functions the Capacity numbers pass through, on values the API writes and
/// this file's own arithmetic produces. Pure functions, so they are asserted as ones — the
/// card above only means what it reads if these do.
#[test]
fn a_quantity_becomes_a_number_and_a_number_becomes_a_size_a_human_reads() {
    // **Every shape the pipeline can hand this**, not the six the committed fixtures happen to
    // carry: each suffix arm was individually deletable with the suite green (D81). `None` is a
    // right answer here; a panic and a wrapped number are not (invariant 5).
    #[rustfmt::skip]
    let table: [(&str, Option<i64>); 46] = [
        // What the API and a manifest actually write.
        ("0",        Some(0)),
        ("100m",     Some(100)),
        ("1",        Some(1_000)),
        ("1.5",      Some(1_500)),
        ("1500m",    Some(1_500)),
        ("1Ki",      Some(1_024_000)),
        ("1Mi",      Some(1_048_576_000)),
        ("1Gi",      Some(1_073_741_824_000)),
        ("1Ti",      Some(1_099_511_627_776_000)),
        ("1Pi",      Some(1_125_899_906_842_624_000)),
        // 1024^6 * 1000 is 1.15e21 — past i64, which is an exabyte node nobody has.
        ("1Ei",      None),
        ("64Ei",     None),
        // Decimal and binary suffixes are different numbers and must not be confused.
        ("100M",     Some(100_000_000_000)),
        ("100Mi",    Some(104_857_600_000)),
        ("1k",       Some(1_000_000)),
        // **Every decimal arm, at a size where it is observable.** `1E` and `1Ei` are past i64,
        // so the whole-unit rows below them answer `None` — and `None` is also what deleting the
        // arm produces, which is how five of these went untested (D81).
        ("1G",       Some(1_000_000_000_000)),
        ("1T",       Some(1_000_000_000_000_000)),
        ("1P",       Some(1_000_000_000_000_000_000)),
        ("0.001E",   Some(1_000_000_000_000_000_000)),
        ("0.001Ei",  Some(1_152_921_504_606_846_976)),
        // Kubernetes has no `K` — only `k`. Answering 1000 here would invent a suffix.
        ("1K",       None),
        // Sub-milli rounds up, `Quantity::MilliValue`'s own direction.
        ("1n",       Some(1)),
        ("1u",       Some(1)),
        ("0n",       Some(0)),
        ("0.5m",     Some(1)),
        // **The exponent form parses** — upstream's grammar has it, `ParseQuantity` accepts it,
        // and a *quoted* `"1e3"` round-trips off a real apiserver verbatim (D81). The doc
        // sentence that used to justify `None` here was a claim about apiserver behaviour that a
        // `--dry-run=server` contradicts, and it cost a whole node its Capacity row.
        ("1e3",      Some(1_000_000)),
        ("1E3",      Some(1_000_000)),
        ("1e-3",     Some(1)),
        // Upstream puts the exponent *in place of* a suffix, so this is not a quantity.
        ("1e3Ki",    None),
        // Not numbers.
        ("1.2.3",    None),
        ("",         None),
        ("NaN",      None),
        ("inf",      None),
        ("m",        None),
        ("100mm",    None),
        ("100Mib",   None),
        // A request cannot be negative, and the sign is not even scanned.
        ("-1",       None),
        ("-100m",    None),
        ("+5",       None),
        // Whitespace is not trimmed anywhere upstream of this, so it is not a number.
        (" 5 ",      None),
        ("5 ",       None),
        // i64::MAX itself: x1000 is past i64, so it is not a number this can carry.
        ("9223372036854775807", None),
        ("9223372036854775", Some(9_223_372_036_854_775_000)),
        ("9223372036854776", None),
        // Past i128, which is where the mantissa parse itself has to give up.
        ("100000000000000000000000000000000000000000", None),
        (".5",       Some(500)),
    ];
    for (q, want) in table {
        let got = quantity_milli(q);
        println!("{q:>44} -> {got:?}");
        assert_eq!(
            got, want,
            "{q:?} is a shape the pipeline hands this function"
        );
    }
    assert_eq!(
        quantity_milli("5."),
        Some(5_000),
        "a trailing point is upstream's grammar too"
    );

    for (milli, want) in [
        (67_108_864_000, "64Mi"),
        (24_860_065_792_000, "23.1Gi"),
        (1_024_000, "1Ki"),
        (1_610_612_736_000, "1.5Gi"),
        // Below a kibibyte Kubernetes writes the bare number, and so does this — a floor the
        // card cannot reach, since no node's allocatable is 512 bytes.
        (512_000, "512"),
    ] {
        println!("{milli:>20} -> {}", bytes(milli));
        assert_eq!(bytes(milli), want);
    }
    assert_eq!(cpu_text(9_100), "9.1");
    assert_eq!(cpu_text(12_000), "12");
    assert_eq!(cpu_text(1), "0.001", "a 1m request is not nothing");
    assert_eq!(cpu_text(0), "0");
}

/// **`100m` × n is where the `f64` fired.** The property that replaced it, asserted as a property
/// and not as one lucky row: exact, and independent of how many.
#[test]
fn millicores_sum_exactly_and_a_float_does_not() {
    let one = quantity_milli("100m").expect("a millicore request parses");
    for n in 1..=100i64 {
        let integer: i64 = (0..n).map(|_| one).sum();
        assert_eq!(
            integer,
            quantity_milli(&format!("{}m", n * 100)).expect("parses"),
            "{n} x 100m must equal {}m exactly",
            n * 100
        );
    }
    // The bug this replaced, reproduced so the test above is known to discriminate.
    let float: f64 = (0..3).map(|_| 100.0 * 1e-3).sum();
    println!("f64: 3 x 100m = {float:.20} vs 0.3 = {:.20}", 0.3_f64);
    assert!(
        float > 0.3,
        "if this ever stops being true the float bug was never reachable and the integer \
         rewrite proved nothing"
    );
}

fn truncate(q: &str) -> String {
    if q.chars().count() > 44 {
        format!("{}…({} chars)", &q[..30], q.len())
    } else {
        q.escape_debug().to_string()
    }
}

/// **Nothing this function is handed may take the process down** — it parses a string that came
/// off the API, and a rule may not panic (invariant 5). The two long ones are not theoretical:
/// `kubectl apply --dry-run=server` against the kind cluster **accepts and stores them verbatim**,
/// so a watch really can hand the decode one (NOTES § D81).
#[test]
fn quantity_milli_never_panics() {
    let mut hostile: Vec<String> = vec![
        String::new(),
        ".".to_string(),
        "..".to_string(),
        "...".to_string(),
        "-".to_string(),
        "0.".to_string(),
        ".0".to_string(),
        "e".to_string(),
        "e-".to_string(),
        "1e".to_string(),
        "1e999999999".to_string(),
        "1e-999999999".to_string(),
        "\u{0}".to_string(),
        "1\u{1b}[2J".to_string(),
        "1".repeat(200),
        format!("{}.{}", "9".repeat(100), "9".repeat(100)),
        format!("0.{}n", "9".repeat(60)),
        format!("{}Ei", "9".repeat(30)),
        // i128::MAX/1000 as the mantissa with the point 20 places in: the numerator lands 727
        // short of i128::MAX, and `numerator + denominator - 1` used to be an unchecked add.
        "1701411834604692.31731687303715884105n".to_string(),
        "170141183460469231731687303715884105n".to_string(),
        "170141183460469231731687303715884105m".to_string(),
        format!("1.{}n", "0".repeat(30)),
    ];
    for suffix in [
        "", "m", "n", "u", "k", "M", "G", "T", "P", "E", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "e9",
    ] {
        hostile.push(format!("{}.{}{suffix}", "9".repeat(40), "9".repeat(30)));
        hostile.push(format!("{}{suffix}", "9".repeat(38)));
    }
    let mut panicked: Vec<String> = Vec::new();
    for q in &hostile {
        match std::panic::catch_unwind(|| quantity_milli(q)) {
            Ok(got) => {
                println!("{:>50} -> {got:?}", truncate(q));
                assert!(
                    got.is_none_or(|m| m >= 0),
                    "{} answered {got:?} — a negative request is a number no quantity can \
                     mean, and in release that is what the unchecked add produced instead of \
                     the panic",
                    truncate(q)
                );
            }
            Err(_) => {
                println!("{:>50} -> PANIC", truncate(q));
                panicked.push(q.clone());
            }
        }
    }
    assert!(
        panicked.is_empty(),
        "a quantity string off the API took a pure rule down (invariant 5): {:?}",
        panicked.iter().map(|q| truncate(q)).collect::<Vec<_>>()
    );
}

/// **The same string, arriving the way it actually would** — through the decode, off a pod spec,
/// into the rule. A panic in a helper is a bug; a panic reachable from a rule is invariant 5. In
/// release, where the add wrapped instead of panicking, the answer was a *negative* sum, which the
/// comparison reads as a node promising less than nothing.
#[test]
fn the_overflow_reaches_the_rule_through_a_real_pod() {
    const HOSTILE: &str = "170141183460469231731687303715884105n";
    let pod = capture_but("healthy", |p| {
        p.spec
            .as_mut()
            .expect("a captured pod has a spec")
            .containers[0]
            .resources
            .as_mut()
            .expect("the capture declares requests")
            .requests
            .as_mut()
            .expect("with a cpu request in it")
            .insert("cpu".to_string(), Quantity(HOSTILE.to_string()));
        for status in p
            .status
            .as_mut()
            .expect("a captured pod has a status")
            .container_statuses
            .iter_mut()
            .flatten()
        {
            status
                .resources
                .as_mut()
                .expect("the kubelet enacted a request")
                .requests
                .as_mut()
                .expect("with a cpu request in it")
                .insert("cpu".to_string(), Quantity(HOSTILE.to_string()));
        }
    });
    println!(
        "decoded cpu_request: {:?}",
        pod.containers
            .iter()
            .map(|c| c.cpu_request.as_deref())
            .collect::<Vec<_>>()
    );
    let node = captured_nodes()
        .into_iter()
        .find(|n| Some(n.id.name.as_str()) == pod.node.as_deref())
        .expect("the pod's node is in the capture");
    let snapshot = cluster(vec![pod], vec![node.clone()]);
    let got = std::panic::catch_unwind(|| node_overcommitted(&snapshot, &node));
    match &got {
        Ok(f) => println!("N5 answered {:?}", f.as_ref().map(|f| &f.evidence)),
        Err(_) => println!("N5 PANICKED"),
    }
    assert!(
        got.is_ok(),
        "one pod requesting a large-but-legal quantity took the rule engine down"
    );
    if let Ok(Some(f)) = got {
        assert!(
            !f.evidence.contains('-'),
            "the sum wrapped and the card printed a negative: {}",
            f.evidence
        );
    }
}

/// **A quoted exponent arrives off a watch, and it used to take the whole node with it.** An
/// unquoted `1e3` is canonicalised to `1k` by the apiserver; a quoted `"1e3"` — how every chart
/// that quotes its quantities writes it — is stored and returned verbatim, because `Quantity`
/// caches the string it was parsed from. Refusing it made `promised` answer `None` for the node,
/// which is one machine silently absent from the Capacity report (NOTES § D81).
#[test]
fn a_quoted_exponent_is_a_number_and_not_a_node_lost_from_the_report() {
    let pod = capture_but("healthy", |p| {
        for status in p
            .status
            .as_mut()
            .expect("a captured pod has a status")
            .container_statuses
            .iter_mut()
            .flatten()
        {
            status
                .resources
                .as_mut()
                .expect("the kubelet enacted a request")
                .requests
                .as_mut()
                .expect("with a cpu request in it")
                .insert("cpu".to_string(), Quantity("1e3".to_string()));
        }
    });
    assert_eq!(
        container(&pod, "app").cpu_request.as_deref(),
        Some("1e3"),
        "the decode carries the string the apiserver stored, exponent and all"
    );
    let node = captured_nodes()
        .into_iter()
        .find(|n| Some(n.id.name.as_str()) == pod.node.as_deref())
        .expect("the pod's node is in the capture");
    let here: Vec<PodSnapshot> = every_captured_pod()
        .into_iter()
        .filter(|p| p.node.as_deref() == Some(node.id.name.as_str()))
        .chain([pod])
        .collect();
    let snapshot = cluster(here, vec![node.clone()]);
    let borrowed: Vec<&PodSnapshot> = snapshot.pods.iter().collect();
    let sum = promised(
        &borrowed,
        node.allocatable_cpu.as_deref(),
        |p| p.cpu_request.as_deref(),
        |c| c.cpu_request.as_deref(),
        |p| p.overhead_cpu.as_deref(),
    );
    println!("cpu sum for {}: {sum:?}", node.id.name);
    let (asked, _) = sum.expect(
        "one quoted exponent among the pods must not delete the whole node from the report",
    );
    assert!(
        asked >= quantity_milli("1e3").expect("the exponent form parses"),
        "and the value counts towards the sum rather than being skipped: {asked}"
    );
}

/// **N4 and N5 are computed in this file and do not reach Alerts** — `Severity::Info` is the
/// line D2 draws, and these two rules are the ones `analyze` does not call at all. **Both rules are
/// shown to fire on the snapshot first**: an exclusion asserted over a rule that answers `None`
/// anyway is an exclusion that stays green the day somebody wires it in (D81).
///
/// **C1 is the one `Info` that does leave `analyze`, and it is not in these snapshots** — neither
/// carries a client certificate, so the loop below is about N4 and N5 and says nothing about the
/// band D87 routes to the Certificates report.
#[test]
fn the_two_info_rules_are_the_reports_input_and_never_an_alerts_card() {
    let skewed = node_but("k8rs-worker2", |n| {
        n.status
            .as_mut()
            .expect("a captured node has a status")
            .node_info
            .as_mut()
            .expect("a captured node reports its kubelet version")
            .kubelet_version = "v1.30.0".to_string();
    });
    let snapshot = ClusterSnapshot {
        server_version: Some("v1.36.1".to_string()),
        ..cluster(every_captured_pod(), vec![skewed.clone()])
    };
    assert!(
        kubelet_too_far_behind(snapshot.server_version.as_deref(), &skewed).is_some(),
        "N4 answers on this snapshot, which is what `analysis.rs` will call it for"
    );

    // N5's own: the capture is not over-promised, so its exclusion has to be asserted over the
    // planted snapshot that is — the half this test used to leave to chance.
    let promised = over_promised();
    let full = promised.nodes.first().expect("the planted node is there");
    assert!(
        node_overcommitted(&promised, full).is_some(),
        "N5 answers on this snapshot too, or the assertion below is about a rule that was \
         never going to say anything"
    );
    let over = analyze(&promised);
    show(&over);
    assert!(
        !over.iter().any(|f| f.title.contains("promised more")),
        "an over-promised node is the Capacity report's row: {:?}",
        titles(&over)
    );

    let all = analyze(&snapshot);
    show(&all);
    for f in all.iter().chain(over.iter()) {
        assert_ne!(
            f.severity,
            Severity::Info,
            "N4 and N5 are not called from here and no certificate is in these snapshots, so \
             nothing this returns may be an Info (D2, § D87): {}",
            f.title
        );
    }
    assert!(
        !all.iter().any(|f| f.title.contains("kubelet")),
        "a skewed kubelet is the Versions report's row: {:?}",
        titles(&all)
    );
}

/// **N6 — the node half of rule 10's card, and not a second card** (NOTES § D28). The captured
/// Pending pod asks for `disktype=ssd` and no node in the cluster is labelled that way, which
/// the scheduler's own sentence agrees with from the other side: *3 node(s) didn't match Pod's
/// node affinity/selector*.
#[test]
fn the_pending_pod_is_told_which_label_nothing_in_the_cluster_has() {
    let nodes = captured_nodes();
    let all = analyze(&cluster(vec![pod("pending")], nodes.clone()));
    show(&all);
    assert_eq!(
        all.iter()
            .filter(|f| f.object.kind == ObjectKind::Pod)
            .count(),
        1,
        "one card about the pod, never a second one about the node that refused it — two \
         findings for one pod is what stops the list being believable (D28): {:?}",
        titles(&all)
    );

    let card = only(&all, "broken-pending", "will take this pod");
    assert_eq!(
        card.object.kind,
        ObjectKind::Pod,
        "D37: the subject is the pod that cannot run, and the node is named in the evidence"
    );
    let wanted = pod("pending").node_selector;
    let unmatched: Vec<&String> = wanted
        .iter()
        .filter(|(k, v)| !nodes.iter().any(|n| n.labels.get(*k) == Some(*v)))
        .map(|(k, _)| k)
        .collect();
    assert_eq!(
        unmatched.len(),
        1,
        "the capture asks for two labels and the cluster has one of them — `kubernetes.io/os` \
         is on every node, and that is what stops this being a test of *any* selector: {wanted:?}"
    );
    assert!(
        card.evidence.contains(&format!(
            "it asks for a node labelled {}=ssd, and none of the {} nodes have that label",
            unmatched[0],
            nodes.len()
        )),
        "`screens/alerts.md` § N6, and it names the label nothing has rather than the whole \
         selector: {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("the scheduler's own words"),
        "the quote stays: it is the only place the *other* refusals appear (D37): {}",
        card.evidence
    );
    assert_eq!(
        card.action,
        format!(
            "change the nodeSelector, or label a node {}=ssd",
            unmatched[0]
        )
    );
    assert_eq!(
        card.timestamp,
        Some(captured_time(
            captured_condition(&fixture("pending"), "PodScheduled"),
            &["lastTransitionTime"]
        )),
        "the pod's own wait, never the blocking node's taint `added_at` — the two clocks \
         answer different questions and only one of them is this card's (D69)"
    );
    assert_eq!(
        card.severity,
        Severity::Critical,
        "`screens/alerts.md` draws N6 amber, and rule 10's ladder overrides it: this card is \
         the same card, and an hour and a quarter unplaced is past the ten minutes anything \
         resolves in"
    );
}

/// **On a one-node cluster the sentence has to still be a sentence.** The count and its noun come
/// from [`counted`], and the plural frame around it produced *"none of the 1 node have that
/// label"* — `1 restarts`' own defect, one rule over, on the cluster this tool is most often
/// pointed at: kind, minikube, k3s and Docker Desktop are all one node (NOTES § D81).
///
/// **The four-node sentence is asserted beside it, unchanged**, because it is the string
/// `screens/alerts.md` § N6 draws and because a fix that reads right at one and wrong at four
/// passes on the half it was written for.
#[test]
fn the_one_node_cluster_is_not_told_none_of_its_1_node_have_the_label() {
    let nodes = captured_nodes();
    assert!(
        nodes.len() > 1,
        "the capture is the plural half of this test, and a one-node capture would make the \
         second assertion below true for free: {} nodes",
        nodes.len()
    );
    let only_machine = vec![nodes.first().cloned().expect("the capture has nodes")];
    let all = analyze(&cluster(vec![pod("pending")], only_machine));
    show(&all);
    let card = only(&all, "broken-pending", "will take this pod");
    assert!(
        card.evidence
            .contains("the cluster's one node does not have that label"),
        "one machine is named as one machine, in words: {}",
        card.evidence
    );
    // **The negative is asked of N6's own sentence and not of the whole line** (NOTES § D29,
    // § D31). The scheduler's quote beside it says `1 node(s) had untolerated taint(s)` — the
    // API's words, kept verbatim (NOTES § D37) — and a search over the joined evidence would be
    // answered by that instead of by the clause under test. N6's sentence is the first fact,
    // which is the order `screens/alerts.md` § N6 states.
    let n6 = card
        .evidence
        .split(FACTS)
        .next()
        .expect("the card has an evidence line");
    assert!(
        !n6.contains("1 node"),
        "and never as a count glued to a plural verb — `none of the 1 node have` is a format \
         string showing through (invariant 14): {n6}"
    );

    let many = analyze(&cluster(vec![pod("pending")], nodes.clone()));
    show(&many);
    assert!(
        only(&many, "broken-pending", "will take this pod")
            .evidence
            .contains(&format!(
                "none of the {} nodes have that label",
                nodes.len()
            )),
        "and the sentence the captured cluster draws is the one it always drew"
    );
}

/// **The other answer: a taint every machine that could take the pod is carrying.** The
/// capture's `dedicated=gpu:NoExecute` is on one worker and the Pending pod tolerates it — so
/// the pod is given the tolerations every *other* pod in the capture has, which is what a pod
/// that was never told about the gpu nodes looks like.
#[test]
fn the_pending_pod_is_told_which_taint_is_refusing_it() {
    let gpu = captured_nodes()
        .into_iter()
        .find(|n| n.taints.iter().any(|t| t.key == "dedicated"))
        .expect("`break-nodes` taints one worker");
    let untolerating = capture_but("pending", |p| {
        let spec = p.spec.as_mut().expect("a captured pod has a spec");
        // The two tolerations the admission controller writes on to every pod, taken off
        // another capture rather than typed here — the gpu one is what is being removed.
        spec.tolerations =
            serde_json::from_value(fixture("crashloop")["spec"]["tolerations"].clone())
                .expect("every captured pod carries the default pair");
        spec.node_selector = None;
    });
    let all = analyze(&cluster(vec![untolerating], vec![gpu.clone()]));
    show(&all);

    let card = only(&all, "broken-pending", "will take this pod");
    assert!(
        card.evidence.contains(&format!(
            "{} is tainted dedicated=gpu, and this pod does not tolerate that taint",
            gpu.id.name
        )),
        "`screens/alerts.md` § N6 — the node is named in the evidence, and `key=value` is \
         how `kubectl taint` spells the thing the action asks for: {}",
        card.evidence
    );
    assert_eq!(
        card.action,
        "add a toleration for dedicated=gpu, or remove the taint"
    );

    // And the pod as captured *does* tolerate it, so the same node says nothing about taints.
    let all = analyze(&cluster(vec![pod("pending")], vec![gpu]));
    let card = only(&all, "broken-pending", "will take this pod");
    assert!(
        !card.evidence.contains("does not tolerate"),
        "the capture's own toleration matches this taint, and a rule that blamed it anyway \
         would send the reader to add a toleration they already have: {}",
        card.evidence
    );
}

/// **When the join cannot pin the refusal on one thing, the card is exactly what it was.**
/// Three shapes reach that branch, and the middle one is the one worth having a test for: a
/// taint on some of the machines but not all of them means something else is refusing the
/// rest, and a card blaming the taint sends the reader to fix half a problem.
#[test]
fn a_refusal_the_nodes_cannot_explain_leaves_rule_ten_saying_what_it_always_said() {
    let raw = fixture("pending");
    let sentence = captured_str(captured_condition(&raw, "PodScheduled"), &["message"]);
    let plain = format!("the scheduler's own words (a node is one machine): {sentence}");

    // No node list at all — a snapshot that has not been given the node watch. "None of the 0
    // nodes have that label" is the sentence this guard exists to stop.
    let all = analyze(&pods_at(vec![pod("pending")], now()));
    assert_eq!(
        only(&all, "broken-pending", "will take this pod").evidence,
        plain
    );

    // A taint on one candidate machine and not the other.
    let mixed = analyze(&cluster(
        vec![capture_but("pending", |p| {
            let spec = p.spec.as_mut().expect("a captured pod has a spec");
            spec.tolerations = None;
            spec.node_selector = None;
        })],
        vec![
            node_but("k8rs-worker2", |_| {}),
            node_but("k8rs-worker", |n| {
                n.spec.as_mut().expect("a captured node has a spec").taints = None;
            }),
        ],
    ));
    show(&mixed);
    let card = only(&mixed, "broken-pending", "will take this pod");
    assert_eq!(
        card.evidence, plain,
        "one machine is tainted and the other is not, so the taint is not the answer — and \
         an answer that is only true of half the cluster is worse than none: {}",
        card.evidence
    );
    assert!(
        card.action.contains("check what this pod asks for"),
        "and the action falls back to the one the command beside it can start: {}",
        card.action
    );
}

/// **Whether a pod puts up with a taint is upstream's `ToleratesTaint`, field for field** — and
/// every row below is a shape a real manifest writes. Getting any of them backwards is a card
/// that names a taint the pod already tolerates, or silence about the one that is refusing it.
#[test]
fn a_toleration_matches_a_taint_the_way_the_scheduler_matches_it() {
    let taint = Taint {
        key: "dedicated".to_string(),
        value: Some("gpu".to_string()),
        effect: "NoExecute".to_string(),
        added_at: None,
    };
    let toleration = |key: &str, operator: &str, value: Option<&str>, effect: Option<&str>| {
        let mut p = pod("crashloop");
        p.tolerations = vec![Toleration {
            key: Some(String::from(key)),
            operator: Some(String::from(operator)),
            value: value.map(String::from),
            effect: effect.map(String::from),
        }];
        p
    };

    for (label, pod, want) in [
        (
            "the exact pair, with the effect",
            toleration("dedicated", "Equal", Some("gpu"), Some("NoExecute")),
            true,
        ),
        (
            "`Exists` ignores the value",
            toleration("dedicated", "Exists", None, Some("NoExecute")),
            true,
        ),
        (
            "an empty effect tolerates every effect",
            toleration("dedicated", "Equal", Some("gpu"), None),
            true,
        ),
        (
            "the wrong value is not a match",
            toleration("dedicated", "Equal", Some("tpu"), Some("NoExecute")),
            false,
        ),
        (
            "nor is the wrong effect",
            toleration("dedicated", "Equal", Some("gpu"), Some("NoSchedule")),
            false,
        ),
        (
            "nor the wrong key",
            toleration("workload", "Equal", Some("gpu"), Some("NoExecute")),
            false,
        ),
        (
            "an operator nothing implements tolerates nothing",
            toleration("dedicated", "Superset", Some("gpu"), None),
            false,
        ),
    ] {
        println!("{label}: {:?}", pod.tolerations);
        assert_eq!(tolerated(&pod, &taint), want, "{label}");
    }

    // The two the API writes without an operator or without a key at all.
    let mut defaulted = pod("crashloop");
    defaulted.tolerations = vec![Toleration {
        key: Some("dedicated".to_string()),
        operator: None,
        value: Some("gpu".to_string()),
        effect: None,
    }];
    assert!(
        tolerated(&defaulted, &taint),
        "an absent operator is `Equal`, which is upstream's own default"
    );
    let mut everything = pod("crashloop");
    everything.tolerations = vec![Toleration {
        key: None,
        operator: Some("Exists".to_string()),
        value: None,
        effect: None,
    }];
    assert!(
        tolerated(&everything, &taint),
        "an empty key with `Exists` tolerates every taint there is — how a DaemonSet that \
         must run everywhere is written"
    );
    assert!(
        !tolerated(&pod("crashloop"), &taint),
        "and the default pair the admission controller writes tolerates neither"
    );
}

/// **A taint that does not stop anything is not an answer.** `PreferNoSchedule` is a
/// preference the scheduler overrules to place a pod, so a card blaming one would name a taint
/// that is not refusing anybody.
#[test]
fn a_soft_taint_is_never_named_as_the_thing_refusing_a_pod() {
    let soft = node_but("k8rs-worker2", |n| {
        n.spec
            .as_mut()
            .expect("a captured node has a spec")
            .taints
            .as_mut()
            .expect("this worker carries the operator's taint")[0]
            .effect = "PreferNoSchedule".to_string();
    });
    let all = analyze(&cluster(
        vec![capture_but("pending", |p| {
            let spec = p.spec.as_mut().expect("a captured pod has a spec");
            spec.tolerations = None;
            spec.node_selector = None;
        })],
        vec![soft],
    ));
    show(&all);
    assert!(
        !only(&all, "broken-pending", "will take this pod")
            .evidence
            .contains("does not tolerate"),
        "the scheduler places pods on `PreferNoSchedule` machines every day: {:?}",
        titles(&all)
    );
}

/// **The whole committed capture — every pod in both namespaces, every node — through
/// [`analyze`] at once.** `cargo test -- --nocapture` prints what a user would actually read,
/// and the node cards are the half the pod-only run above cannot show.
#[test]
fn the_whole_capture_including_its_nodes_through_the_rules_at_once() {
    let all = analyze(&cluster(every_captured_pod(), captured_nodes()));
    show(&all);
    println!(
        "{} critical, {} warnings, {} info",
        all.iter()
            .filter(|f| f.severity == Severity::Critical)
            .count(),
        all.iter().filter(|f| f.severity == Severity::Warn).count(),
        all.iter().filter(|f| f.severity == Severity::Info).count(),
    );

    let nodes: Vec<(&str, &str)> = all
        .iter()
        .filter(|f| f.object.kind == ObjectKind::Node)
        .map(|f| (f.object.name.as_str(), f.title.as_str()))
        .collect();
    println!("{nodes:#?}");
    assert_eq!(
        nodes.len(),
        2,
        "one node stopped answering and one is cordoned with work left on it — the two \
         states `break-nodes` puts on this cluster that reach Alerts: {nodes:?}"
    );
    for f in &all {
        assert_ne!(f.severity, Severity::Info, "D2: {}", f.title);
        assert!(
            !f.title.is_empty() && !f.action.is_empty(),
            "what happened · what it means · what to do: {f:?}"
        );
        let cmd = f
            .kubectl_cmd
            .as_deref()
            .unwrap_or_else(|| panic!("every rule in this box has a command: {}", f.title));
        assert!(
            cmd.contains(&f.object.name),
            "invariant 4's teaching device points at the object the card is about: {cmd}"
        );
        assert_eq!(
            f.object.namespace.is_none(),
            f.object.kind == ObjectKind::Node,
            "a node is cluster-scoped and everything else here is not: {:?}",
            f.object
        );
    }
}

/// The node the capture works hardest, and what the pods on it are charged in millicores.
///
/// **Busiest by what N5 actually sums, not by pod count** — the rule adds one number per
/// container that asks for cpu, so a node carrying twelve pods that ask for nothing stresses the
/// arithmetic less than one carrying four that do, and the float control in the ordering test
/// needs values that differ before it discriminates at all.
fn busiest() -> (NodeSnapshot, Vec<PodSnapshot>, i64) {
    let all = every_captured_pod();
    let (node, here) = captured_nodes()
        .into_iter()
        .map(|n| {
            let here: Vec<PodSnapshot> = all
                .iter()
                .filter(|p| p.node.as_deref() == Some(n.id.name.as_str()))
                .cloned()
                .collect();
            (n, here)
        })
        .max_by_key(|(_, here)| {
            here.iter()
                .flat_map(|p| &p.containers)
                .filter(|c| c.cpu_request.is_some())
                .count()
        })
        .expect("the capture has nodes");
    let borrowed: Vec<&PodSnapshot> = here.iter().collect();
    let (asked, _) = promised(
        &borrowed,
        Some("1"),
        |p| p.cpu_request.as_deref(),
        |c| c.cpu_request.as_deref(),
        |p| p.overhead_cpu.as_deref(),
    )
    .expect("every captured request parses");
    assert!(
        asked > 0,
        "a boundary proved at zero promised cpu is not a boundary: {} pods on {}",
        here.len(),
        node.id.name
    );
    (node, here, asked)
}

/// **Exactly full is silent, one millicore over fires.** The line itself, from both sides — every
/// committed fixture is comfortably over or comfortably under it, which is how an exactly-packed
/// node fired unnoticed (NOTES § D81). `noderesources.Fit` admits while
/// `request <= allocatable - requested`, and `describe node` prints `cpu 3920m (100%)` without
/// comment.
///
/// The allocatable is not hand-written: it is this node's own pods' sum, spelled back in the unit
/// the API writes.
#[test]
fn n5_is_silent_at_the_line_and_fires_one_millicore_past_it() {
    let (node, here, asked) = busiest();
    println!("{}: {} pods promise {asked}m cpu", node.id.name, here.len());
    for (allocatable, fires) in [
        (format!("{}m", asked + 1), false),
        (format!("{asked}m"), false),
        (format!("{}m", asked - 1), true),
    ] {
        let mut n = node.clone();
        n.allocatable_cpu = Some(allocatable.clone());
        n.allocatable_memory = None;
        let snapshot = cluster(here.clone(), vec![n.clone()]);
        let got = node_overcommitted(&snapshot, &n);
        println!(
            "  allocatable {allocatable:>8} -> {:?}",
            got.as_ref().map(|f| &f.title)
        );
        assert_eq!(
            got.is_some(),
            fires,
            "promised {asked}m against allocatable {allocatable}: a node packed to exactly its \
             allocatable is legal and ordinary, and one milli past it is not"
        );
        // The blocker's second symptom: the card printed two identical numbers.
        if let Some(f) = got {
            let (promised_text, usable_text) = (cpu_text(asked), cpu_text(asked - 1));
            println!("  evidence: {}", f.evidence);
            assert_ne!(
                promised_text, usable_text,
                "a card whose two numbers print the same says nothing"
            );
            assert!(
                f.evidence
                    .contains(&format!("promised {promised_text} cpu"))
                    && f.evidence.contains(&format!("usable {usable_text} cpu")),
                "the card must print the two numbers it compared: {}",
                f.evidence
            );
        }
    }
}

/// **The memory half of the same line, and it is not a copy of the test above.** N5 has two
/// branches; with only the cpu one on the line, `cargo mutants` turned the memory comparison into
/// `>=` and the whole suite stayed green — the blocker still live on the other half of the same
/// rule, printing `promised 290Mi · usable 290Mi` (NOTES § D81).
#[test]
fn n5_is_silent_at_the_memory_line_too() {
    let all = every_captured_pod();
    let (node, here) = captured_nodes()
        .into_iter()
        .map(|n| {
            let here: Vec<PodSnapshot> = all
                .iter()
                .filter(|p| p.node.as_deref() == Some(n.id.name.as_str()))
                .cloned()
                .collect();
            (n, here)
        })
        .max_by_key(|(_, here)| here.len())
        .expect("the capture has nodes");
    let borrowed: Vec<&PodSnapshot> = here.iter().collect();
    let (asked, _) = promised(
        &borrowed,
        Some("1"),
        |p| p.memory_request.as_deref(),
        |c| c.memory_request.as_deref(),
        |p| p.overhead_memory.as_deref(),
    )
    .expect("every captured memory request parses");
    assert!(
        asked > 0,
        "a memory boundary proved at zero promised bytes is not one"
    );
    assert_eq!(
        asked % 1000,
        0,
        "a byte count times 1000 — if this is not whole the exact allocatable below cannot be \
         spelled"
    );
    println!(
        "{}: {} pods promise {asked} milli-bytes",
        node.id.name,
        here.len()
    );

    for (allocatable, fires) in [
        ((asked / 1000 + 1).to_string(), false),
        ((asked / 1000).to_string(), false),
        ((asked / 1000 - 1).to_string(), true),
    ] {
        let mut n = node.clone();
        n.allocatable_cpu = None;
        n.allocatable_memory = Some(allocatable.clone());
        let snapshot = cluster(here.clone(), vec![n.clone()]);
        let got = node_overcommitted(&snapshot, &n);
        println!(
            "  allocatable {allocatable:>14} -> {:?}",
            got.as_ref().map(|f| &f.evidence)
        );
        assert_eq!(
            got.is_some(),
            fires,
            "promised {asked} milli-bytes against allocatable {allocatable}: exactly full is \
             legal and ordinary, one byte past it is not"
        );
    }
}

/// **The same pods, summed in a different order, must reach the same verdict.** The blocker's
/// second symptom: watch events reorder `snapshot.pods`, so a sum that is not order-free makes the
/// card flap on a node sitting near the line (NOTES § D81).
#[test]
fn n5s_verdict_does_not_depend_on_the_order_the_pods_arrive_in() {
    let (node, here, asked) = busiest();
    let mut n = node.clone();
    n.allocatable_cpu = Some(format!("{asked}m"));
    n.allocatable_memory = None;

    let mut orders: Vec<Vec<PodSnapshot>> = Vec::new();
    for rotate in 0..here.len() {
        let mut o = here.clone();
        o.rotate_left(rotate);
        orders.push(o.clone());
        o.reverse();
        orders.push(o);
    }
    let mut sorted = here.clone();
    sorted.sort_by(|a, b| a.id.name.cmp(&b.id.name));
    orders.push(sorted.clone());
    sorted.reverse();
    orders.push(sorted);

    let verdicts: Vec<Option<String>> = orders
        .iter()
        .map(|o| {
            let snapshot = cluster(o.clone(), vec![n.clone()]);
            node_overcommitted(&snapshot, &n).map(|f| f.evidence)
        })
        .collect();
    println!(
        "{} orderings of {} pods -> {} distinct verdict(s)",
        verdicts.len(),
        here.len(),
        verdicts.iter().collect::<BTreeSet<_>>().len()
    );
    assert!(
        verdicts.iter().all(|v| v == &verdicts[0]),
        "the same pods in a different order reached a different verdict: {verdicts:?}"
    );
    assert_eq!(
        verdicts[0], None,
        "at exactly the line, every order is silent"
    );

    // The float sum this replaced is *not* order-free, so the assertion above discriminates
    // rather than being true of any arithmetic at all.
    let floats: Vec<f64> = orders
        .iter()
        .map(|o| {
            o.iter()
                .flat_map(|p| &p.containers)
                .filter_map(|c| c.cpu_request.as_deref())
                .filter_map(|q| q.strip_suffix('m'))
                .filter_map(|d| d.parse::<f64>().ok())
                .map(|m| m * 1e-3)
                .sum()
        })
        .collect();
    let distinct = floats.iter().map(|f| f.to_bits()).collect::<BTreeSet<_>>();
    println!(
        "the same sums as f64: {} distinct bit patterns",
        distinct.len()
    );
    assert!(
        distinct.len() > 1,
        "if even the float sum is order-free on this node, the ordering assertion above is \
         proved on the wrong input: {floats:?}"
    );
}

/// The captured unschedulable pod with its own asks cleared, so the taint branch is the one
/// reached and one taint is the whole reason the cluster refuses it.
fn unplaceable() -> PodSnapshot {
    let mut p = pod("pending");
    p.node_selector.clear();
    p.tolerations.clear();
    p.nominated_node_name = None;
    p.deletion_timestamp = None;
    assert_eq!(
        p.scheduled.as_ref().and_then(|c| c.reason.as_deref()),
        Some("Unschedulable"),
        "the capture is a pod no node accepted, or N6 is being proved on the wrong pod"
    );
    p
}

fn n6_card(key: &str, value: Option<&str>) -> Finding {
    n6_card_on(1, key, value)
}

fn n6_card_on(machines: usize, key: &str, value: Option<&str>) -> Finding {
    no_node_accepted_it(
        &now(),
        &unplaceable(),
        &tainted(machines, key, value, "NoSchedule"),
    )
    .expect("a pod nothing scheduled draws a card")
}

/// **N6 never tells the reader to tolerate a taint Kubernetes manages, and every row is a key the
/// reader can actually hit** (NOTES § D81). On a single-node cluster — kind, minikube, k3s, Docker
/// Desktop, which is who this tool is for — a `kubectl cordon` and a deploy is all it takes, and
/// the old wording answered *"add a toleration for node.kubernetes.io/unschedulable"* when the
/// answer is `kubectl uncordon`. Two are worse than useless: `unreachable` asked the reader to
/// schedule onto a dead machine while N1 drew *"this node has stopped responding"* on the same
/// screen, and `ToBeDeletedByClusterAutoscaler` is a taint this same file calls *an operation in
/// progress* in N2.
///
/// **The list is read off the constants, not transcribed**, so a row added without a sentence is
/// caught here rather than shipped; and each key is required to reach *its own* answer, because a
/// table that translated every taint into one sentence would pass a test that only asked whether
/// the raw key was gone.
#[test]
fn a_taint_kubernetes_manages_is_translated_and_never_offered_as_a_toleration() {
    let machine = captured_nodes()
        .into_iter()
        .next()
        .expect("the capture has a node")
        .id
        .name;
    let answers: [(&str, &str); 11] = [
        ("node.kubernetes.io/unschedulable", "kubectl uncordon"),
        ("node.kubernetes.io/not-ready", "check that machine first"),
        ("node.kubernetes.io/unreachable", "check that machine first"),
        ("node.kubernetes.io/memory-pressure", "free up memory"),
        ("node.kubernetes.io/disk-pressure", "free up disk space"),
        ("node.kubernetes.io/pid-pressure", "so many processes"),
        ("node.kubernetes.io/network-unavailable", "network plugin"),
        (
            "node.cloudprovider.kubernetes.io/uninitialized",
            "finish joining",
        ),
        ("karpenter.sh/unregistered", "finish joining"),
        ("ToBeDeletedByClusterAutoscaler", "replacement machine"),
        ("karpenter.sh/disrupted", "replacement machine"),
    ];
    let managed: Vec<&str> = MANAGED_TAINTS
        .iter()
        .map(|&(k, _, _)| k)
        .chain(SCALE_DOWN_TAINTS)
        .collect();
    assert_eq!(
        managed,
        answers.iter().map(|&(k, _)| k).collect::<Vec<_>>(),
        "the table plus the two autoscaler taints — read off the constants, so a row added \
         without an answer below cannot ship quietly"
    );

    for (key, must_say) in answers {
        let card = n6_card(key, None);
        println!("\n{key}\n  {}\n  → {}", card.evidence, card.action);
        assert!(
            !card.action.contains("add a toleration"),
            "{key} is written by the node controller and removed by it — tolerating it is never \
             the answer, and for `unreachable` it is advice to schedule onto a dead machine: {}",
            card.action
        );
        assert!(
            !card.evidence.contains(key) && !card.action.contains(key),
            "and the raw key never reaches the screen — `{key}` printed bare is \
             `CrashLoopBackOff` printed and left (invariant 14): {} / {}",
            card.evidence,
            card.action
        );
        assert!(
            card.action.contains(must_say),
            "{key} needs the answer that actually clears it, and no other row's: {}",
            card.action
        );
        assert!(
            card.evidence.contains(&machine),
            "a translation is not a suppression: the card still says which machine ({machine}): \
             {}",
            card.evidence
        );
        // **No row promises a card that may not be on the screen.** N1 waits five minutes and
        // these taints wait not at all, so a runtime that dies at 03:02 and a deploy at 03:03
        // would have sent the reader hunting a node card that arrives at 03:07 (D81).
        assert!(
            !card.action.contains("on this screen") && !card.action.contains("card"),
            "{key}: the evidence has already named the machine, and a pointer at a card that \
             is not drawn yet is worse than no pointer: {}",
            card.action
        );
        // And no token survives into the sentence a user reads.
        assert!(
            !card.action.contains('{') && !card.action.contains('}'),
            "{key} printed an unsubstituted token: {}",
            card.action
        );
    }
}

/// **The one machine this table exists for, and the several it also has to serve.** The evidence
/// line has inflected since it was written; six of the eleven actions said *"those machines"*
/// whatever the count, on a table whose primary case is a one-node kind or minikube cluster
/// (NOTES § D81).
///
/// **And the `uncordon` line has to run as typed.** `(kubectl uncordon)` with no node is the only
/// command in this file that errors out when pasted, on a product whose pitch is *without
/// memorising long kubectl commands* (invariant 4).
#[test]
fn the_managed_actions_say_one_machine_when_there_is_one_and_name_it_in_the_command() {
    let names: Vec<String> = captured_nodes()
        .into_iter()
        .take(2)
        .map(|n| n.id.name)
        .collect();
    assert_eq!(names.len(), 2, "the capture has more than one node");

    for (key, singular, plural) in [
        (
            "node.kubernetes.io/unschedulable",
            format!(
                "allow new pods on that machine again once the work is done (kubectl uncordon {})",
                names[0]
            ),
            format!(
                "allow new pods on those machines again once the work is done (kubectl uncordon \
                 {} {})",
                names[0], names[1]
            ),
        ),
        (
            "node.kubernetes.io/disk-pressure",
            "free up disk space on that machine, or add another machine to the cluster".to_string(),
            "free up disk space on those machines, or add another machine to the cluster"
                .to_string(),
        ),
        (
            "node.kubernetes.io/not-ready",
            "check that machine first — this pod is placed on its own once a machine is ready \
             again"
                .to_string(),
            "check those machines first — this pod is placed on its own once a machine is ready \
             again"
                .to_string(),
        ),
    ] {
        let one = n6_card_on(1, key, None);
        let two = n6_card_on(2, key, None);
        println!("\n{key}\n  one:  {}\n  two:  {}", one.action, two.action);
        assert_eq!(one.action, singular, "{key}, on one machine");
        assert_eq!(two.action, plural, "{key}, on two");
    }

    // The command with its argument, checked as a command rather than as a string: what is
    // printed is what `kubectl uncordon` takes, one node or several.
    let card = n6_card_on(2, "node.kubernetes.io/unschedulable", None);
    let (_, command) = card
        .action
        .split_once("(kubectl uncordon ")
        .expect("the action carries the command");
    let argument = command.trim_end_matches(')');
    assert_eq!(
        argument.split_whitespace().collect::<Vec<_>>(),
        names.iter().map(String::as_str).collect::<Vec<_>>(),
        "every machine the card is about, and `kubectl uncordon` takes them all: {argument:?}"
    );
}

/// **The negative half, and the key the table deliberately leaves out.** A suppression broad
/// enough to swallow `node-role.kubernetes.io/control-plane` — the single-node kubeadm case the
/// toleration wording was written for — passes every positive above. The last two are keys that
/// merely *look* managed: the table is matched whole, never by prefix.
#[test]
fn an_operators_own_taint_still_says_add_a_toleration() {
    for (key, value, named) in [
        (
            "node-role.kubernetes.io/control-plane",
            None,
            "node-role.kubernetes.io/control-plane",
        ),
        ("dedicated", Some("gpu"), "dedicated=gpu"),
        (
            "node.kubernetes.io/unschedulable-by-us",
            None,
            "node.kubernetes.io/unschedulable-by-us",
        ),
        (
            "karpenter.sh/unregistered-ish",
            None,
            "karpenter.sh/unregistered-ish",
        ),
    ] {
        let card = n6_card(key, value);
        println!("\n{key}\n  {}\n  → {}", card.evidence, card.action);
        assert_eq!(
            managed_taint(key),
            None,
            "{key} is somebody's own taint and must not be in the managed table"
        );
        assert!(
            card.evidence.contains(named) && card.action.contains(named),
            "{key} is a taint a human at this cluster applied, so the card names it and offers \
             the two things kubectl accepts: {} / {}",
            card.evidence,
            card.action
        );
        assert!(
            card.action.contains("add a toleration"),
            "{key}: {}",
            card.action
        );
    }
}

/// The table itself: every row a real key, no duplicates, and no sentence that reads wrong after
/// the machine names the card puts in front of it.
#[test]
fn the_managed_taint_table_is_well_formed() {
    let keys: Vec<&str> = MANAGED_TAINTS.iter().map(|&(k, _, _)| k).collect();
    assert_eq!(
        keys.iter().collect::<BTreeSet<_>>().len(),
        keys.len(),
        "a duplicated row: {keys:?}"
    );
    for &(key, means, action) in &MANAGED_TAINTS {
        assert!(
            !SCALE_DOWN_TAINTS.contains(&key),
            "{key} is in both tables, so which sentence wins depends on the lookup order"
        );
        assert!(
            !means.is_empty() && !action.is_empty() && !means.contains(key),
            "{key} translates to nothing, or to itself: {means:?} / {action:?}"
        );
        assert!(
            !means.starts_with("is ") && !means.starts_with("are "),
            "{key}'s sentence carries its own verb and will read `node-1 is is …`: {means}"
        );
        // The three pressure rows used to end *"…on a machine that is"* — legal ellipsis, and the
        // only sentence in the table that stops mid-clause, which at a glance reads truncated
        // (NOTES § D81).
        assert!(
            !means.ends_with(" is") && !means.ends_with(" are") && !means.ends_with(" has"),
            "{key}'s sentence ends on a stranded verb and reads as if it were cut off: {means}"
        );
    }
}

/// **A pod the drain has already evicted is not work the drain still has to do** — upstream's
/// `skipDeletedFilter`, and the same false positive D43 killed for autoscalers arriving from the
/// other side: counting it puts the card on a drain that is *running* (NOTES § D81).
#[test]
fn n2_does_not_count_a_pod_the_drain_has_already_evicted() {
    let terminating = pod("stuck");
    assert!(
        terminating.deletion_timestamp.is_some(),
        "`stuck.json` is the captured pod somebody asked to shut down, which is what a drain \
         leaves behind while it waits"
    );
    // The machine is the one the capture put the pod on — the join has to close, and which
    // worker the scheduler picked moves on every trip.
    let machine = terminating
        .node
        .clone()
        .expect("the captured pod names the machine it is terminating on");
    let draining = node_but(&machine, |n| {
        n.spec
            .as_mut()
            .expect("a captured node has a spec")
            .unschedulable = Some(true);
    });

    let all = analyze(&cluster(vec![terminating.clone()], vec![draining.clone()]));
    show(&all);
    assert!(
        !all.iter().any(|f| f.title.contains("refuses new pods")),
        "one pod, already terminating: that is a drain in flight, not one that stopped half \
         way: {:?}",
        titles(&all)
    );

    // And the pod beside it that a drain has *not* reached is still counted, so the silence
    // above is the filter and not an empty join. Its neighbour is read out of the capture for
    // the reason the machine is.
    let neighbour = a_pod_a_drain_would_move_on(&machine);
    println!("beside it on {machine}: {}", neighbour.id.name);
    let all = analyze(&cluster(vec![terminating, neighbour], vec![draining]));
    show(&all);
    let card = only(&all, &machine, "refuses new pods");
    assert_eq!(
        card.evidence, "1 pod here would still have to move",
        "one of the two, and the count is what `kubectl drain` would actually still move"
    );
}

/// **A pod that finished is charged to nobody and alarms about nothing** — [`finished`], which
/// gates both `analyze`'s pod rules and [`pods_on`], so N1's list, N2's movable count and N5's sum
/// all run through it. Deleting it left the suite green (NOTES § D81): the plant that existed was
/// on a *healthy* capture, where skipping and not skipping produce the same silence.
///
/// **Captured, not planted.** This was a phase written onto a decoded copy for as long as no
/// committed object was over; the 2026-08-13 trip brought both — `succeeded.json` is a pod whose
/// container ran to `exit 0` after three failed attempts, `failed.json` one that never got there
/// and carries `exit 137` beside four restarts. Both keep their `nodeName`, which is the whole
/// reason [`finished`] exists, and both are loud enough underneath to draw two cards apiece the
/// moment their phase says they are still running — which is the control below.
#[test]
fn a_pod_that_finished_is_charged_to_nobody_and_alarms_about_nothing() {
    for name in ["succeeded", "failed"] {
        let done = pod(name);
        let phase = done
            .phase
            .clone()
            .expect("the capture says which way this pod ended");
        assert!(
            finished(&done),
            "{name}.json is a pod that is over, whatever it did on the way: {phase}"
        );

        // **The control, and it is the same object.** A restart count and a failed previous run
        // are what rules 5 and 6 read, and both captures carry them — so the same bytes with the
        // phase moved back to `Running` are loud, and the silence below is the skip rather than a
        // pod nothing was ever wrong with. Deleting `finished` left the suite green once already
        // (NOTES § D81) because the plant that stood here was on a *healthy* capture.
        let still_running = capture_but(name, |p| {
            p.status
                .as_mut()
                .expect("a captured pod has a status")
                .phase = Some("Running".to_string());
        });
        let noisy = analyze(&pods_at(vec![still_running], now()));
        show(&noisy);
        // **One card is enough since 2026-08-16** (NOTES § D113): rules 5 and 6 answer a failed
        // ending with one sentence and rule 5 now carries the duration, so where rule 6 adds no
        // fact its card folds. What this half needs is that the pod is *not silent* while it
        // runs, which is what makes the silence below a property of `finished` and not of the
        // capture.
        assert!(
            !noisy.is_empty(),
            "{name}.json draws cards while it is running, or the silence below proves nothing: \
             {:?}",
            titles(&noisy)
        );

        nothing(
            &analyze(&pods_at(vec![done.clone()], now())),
            format!(
                "a {phase} pod's restart counts and last exits are not what is broken *now*, \
                 which is the whole of what this screen holds (D2). They do not go anywhere \
                 else either — the Waste report does not exist and its charter is \
                 Evicted/Completed pileups rather than a per-pod diagnosis (D96)"
            )
            .as_str(),
        );

        // The node half: it keeps its `nodeName`, and neither the drain count nor the node's
        // own pod list may include it. Which machine that is belongs to the scheduler, so the
        // cordon is applied to whichever node the capture names.
        let machine = done.node.clone().expect(
            "a finished pod keeps the node it ran on, which is the whole reason this \
                     filter is needed",
        );
        let cordoned = node_but(&machine, |n| {
            n.spec
                .as_mut()
                .expect("a captured node has a spec")
                .unschedulable = Some(true);
        });
        let alone = analyze(&cluster(vec![done.clone()], vec![cordoned.clone()]));
        show(&alone);
        assert!(
            !alone.iter().any(|f| f.title.contains("refuses new pods")),
            "a drain moves nothing off a node whose only pod is {phase}: {:?}",
            titles(&alone)
        );

        // Beside a live pod, the count is one — so the silence above is the filter and not an
        // empty join, and N1's total is the same number from the other rule. The neighbour is
        // picked by [`a_drain_would_move`] so it cannot be a pod that is already terminating,
        // which the same filter skips for its own reason — one exclusion at a time, or neither
        // is being tested.
        let live = a_pod_a_drain_would_move_on(&machine);
        println!("beside it on {machine}: {}", live.id.name);
        let both = cluster(vec![done, live], vec![cordoned]);
        let all = analyze(&both);
        assert_eq!(
            only(&all, &machine, "refuses new pods").evidence,
            "1 pod here would still have to move",
            "one of the two: {phase} is not work a drain has left to do"
        );
        let down = node_but(&machine, |n| {
            node_condition_mut(n, "Ready").status = "Unknown".to_string();
        });
        let all = analyze(&ClusterSnapshot {
            nodes: vec![down],
            ..both
        });
        assert!(
            only(&all, &machine, "stopped responding")
                .evidence
                .contains("(1 pod)"),
            "and N1 counts what was running there, which a {phase} pod was not: {}",
            only(&all, &machine, "stopped responding").evidence
        );
    }
}

/// **The node whose labels the pod actually accepts is the one whose taints are read.** N6 filters
/// the machines by `nodeSelector` before it looks for a blocking taint, and nothing exercised that
/// filter: the captured Pending pod asks for a label no node has, so the rule answers before it
/// reaches the filter, and the managed-taint tests clear the selector, which makes `.all()`
/// vacuously true either way — inverting the comparison survived the whole suite (NOTES § D81).
///
/// So: two machines, each with a taint of its own, and only one of them labelled the way the pod
/// asks. Naming the other machine's taint is the bug, and the two cards read differently.
#[test]
fn n6_reads_the_taints_of_the_machines_the_pod_would_accept_and_not_the_others() {
    let wanted = ("disktype", "ssd");
    let candidate = node_but("k8rs-worker2", |n| {
        n.metadata
            .labels
            .get_or_insert_with(Default::default)
            .insert(wanted.0.to_string(), wanted.1.to_string());
    });
    let elsewhere = node_but("k8rs-worker", |n| {
        n.spec.as_mut().expect("a captured node has a spec").taints = Some(vec![ApiTaint {
            key: "dedicated".to_string(),
            value: Some("cpu".to_string()),
            effect: "NoSchedule".to_string(),
            time_added: None,
        }]);
    });
    assert!(
        !elsewhere.labels.contains_key(wanted.0),
        "the second machine must not carry the label, or both are candidates and the filter is \
         not being tested"
    );
    assert!(
        candidate
            .taints
            .iter()
            .any(|t| t.key == "dedicated" && t.value.as_deref() == Some("gpu")),
        "the candidate keeps the operator's own captured taint, which is the string the card \
         has to name"
    );

    let asking = capture_but("pending", |p| {
        let spec = p.spec.as_mut().expect("a captured pod has a spec");
        spec.tolerations = None;
        spec.node_selector = Some(
            [(wanted.0.to_string(), wanted.1.to_string())]
                .into_iter()
                .collect(),
        );
    });
    let all = analyze(&cluster(vec![asking], vec![candidate, elsewhere]));
    show(&all);

    let card = only(&all, "broken-pending", "will take this pod");
    assert!(
        card.evidence
            .contains("k8rs-worker2 is tainted dedicated=gpu"),
        "the machine the pod's own nodeSelector accepts is the one whose taint is refusing it: {}",
        card.evidence
    );
    assert!(
        !card.evidence.contains("k8rs-worker ") && !card.evidence.contains("dedicated=cpu"),
        "and the machine the pod never asked for contributes nothing — reading its taints \
         instead names a fix that changes nothing: {}",
        card.evidence
    );
    assert_eq!(
        card.action,
        "add a toleration for dedicated=gpu, or remove the taint"
    );
}
