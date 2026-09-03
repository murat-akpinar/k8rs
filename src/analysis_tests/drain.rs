//! `analysis.rs` § THE DRAIN SAFETY REPORT — its tests (NOTES § D91).

use super::*;

use crate::rules::analyze;

use k8s_openapi::api::core::v1::{EmptyDirVolumeSource, Volume};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;

// --- DRAIN SAFETY ---
//
// **The producer, against the committed corpus** — the two hand-built panes that stood here until
// this box landed are gone, and what replaces each of them is the same claim asserted against a
// snapshot (the note at the head of this file).
//
// **The counter shapes the capture cannot hold are plants** (NOTES § D40), never edits to a
// committed file (NOTES § D53): the disruption controller answers well inside the time a capture
// takes, so a photographed budget always has its status caught up with its spec, and a workload
// whose `scale` subresource fails is not in `scripts/broken.yaml` at all. Each plant below says
// what a trip would have to do to replace it.

/// **The corpus with the join Drain safety is** — Capacity's snapshot, plus the Deployment's two
/// pods, the two pods that keep files on the machine itself, and the two committed budgets.
/// `broken-pdb-floor` protects `app: healthy-deploy` and those two pods sit on `k8rs-worker2` and
/// `k8rs-worker3`, so exactly two of the four nodes block.
///
/// **`gang` and `restarts` are here because `kubectl drain` named them.** A bare
/// `kubectl drain k8rs-worker` on the fixture cluster refused on *"cannot delete Pods with local
/// storage (use --delete-emptydir-data to override): default/broken-gang, default/broken-restarts"*
/// (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 1), and both sit on
/// `k8rs-worker` — so this snapshot is the node kubectl refused on, and the row about it is
/// checkable against what kubectl said.
pub(super) fn drain_corpus() -> ClusterSnapshot {
    let mut pods = captured_pods();
    pods.extend(captured_deploy_pods());
    pods.extend(["gang", "restarts"].map(captured_pod));
    ClusterSnapshot {
        disruption_budgets: Some(captured_budgets()),
        ..snapshot(pods, captured_nodes())
    }
}

fn with_budgets(
    cluster: ClusterSnapshot,
    budgets: Vec<DisruptionBudgetSnapshot>,
) -> ClusterSnapshot {
    ClusterSnapshot {
        disruption_budgets: Some(budgets),
        ..cluster
    }
}

/// The nodes of a drain pane, in the order it draws them — read off the jump rather than off the
/// text, so a row that stopped naming its node in the first word is caught by the assertion that
/// reads the text and not by this.
fn drained_nodes(report: &Report) -> Vec<&str> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Answer {
                jump: Some(Jump::Object(id)),
                ..
            } if id.kind == ObjectKind::Node => Some(id.name.as_str()),
            _ => None,
        })
        .collect()
}

/// The nodes whose row says a drain would never finish — the claim the blocking assertions are
/// about, read off the row's own sentence rather than off `Critical`, which the node that drains
/// and throws away files now shares with them (`screens/analysis.md` § Drain safety, band order).
fn never_finish(report: &Report) -> Vec<&str> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Answer { text, .. } if text.ends_with(" would never finish draining") => {
                text.split_whitespace().next()
            }
            _ => None,
        })
        .collect()
}

#[test]
fn the_blocked_nodes_lead_the_ready_one_is_still_a_row_and_nothing_badges() {
    let report = super::drain_safety(&drain_corpus(), &[]);
    println!("{}", pane(&report));

    assert_eq!(
        drained_nodes(&report),
        vec![
            "k8rs-worker2",
            "k8rs-worker3",
            "k8rs-worker",
            "k8rs-control-plane"
        ],
        "worst band first, then node name — and `k8rs-control-plane` sorts first alphabetically, \
         which is what makes this an assertion about the band and not about `sort`"
    );
    // **The flag the whole pane assumes, said once and first** — never per row, and never in the
    // command log, which only ever shows a command k8rs actually ran (invariant 4).
    assert!(
        matches!(&report.rows[0], Row::Prose(text)
            if text == "A drain below assumes --ignore-daemonsets, so DaemonSet pods never count \
                        as moving."),
        "{:?}",
        report.rows[0]
    );
    assert_eq!(
        report
            .rows
            .iter()
            .filter(|row| matches!(row, Row::Prose(_)))
            .count(),
        1,
        "and exactly once: a CNI DaemonSet runs on nearly every node, so a per-row repetition \
         would be the loudest line on the busiest pane in the product"
    );
    assert_eq!(
        report.rows[1..].iter().map(severity_of).collect::<Vec<_>>(),
        vec![
            Some(Severity::Critical),
            Some(Severity::Critical),
            Some(Severity::Critical),
            None
        ],
        "the two nodes a drain would never finish on, the node whose drain throws away files — \
         `kubectl drain k8rs-worker` refused on exactly that, and it is Critical beside them and \
         below them — and the node that is ready, which carries no band because there is nothing \
         to judge"
    );
    assert_eq!(
        never_finish(&report),
        vec!["k8rs-worker2", "k8rs-worker3"],
        "and the third Critical is not one of them: it drains, it just costs the reader files"
    );

    // Every row goes somewhere, and no rule fired for any of them: a drain verdict is a report's
    // answer, not a finding (`screens/analysis.md` § Drain safety). The flag line is not one of
    // them — it is read and never selected, like every `Prose`.
    for row in &report.rows[1..] {
        assert!(
            matches!(jump_of(row), Some(Jump::Object(id)) if id.kind == ObjectKind::Node),
            "a drain row jumps to its node and never to a finding"
        );
    }
    assert_eq!(
        selectable(&report).len(),
        4,
        "one selectable row per node, the ready one included"
    );
    assert_eq!(
        report.badge, None,
        "this report never badges — the sidebar has room for a number and the reason is the \
         whole row here ([`Report::badge`])"
    );
    assert!(
        !report.title.contains("drain safety"),
        "the heading is a sentence and not the sidebar's label: {}",
        report.title
    );
}

#[test]
fn the_blocked_row_names_the_rule_its_two_numbers_and_the_way_out() {
    let report = super::drain_safety(&drain_corpus(), &[]);
    let row = row_for(&report, "k8rs-worker2");

    assert_eq!(
        text_of(row),
        "k8rs-worker2 would never finish draining",
        "the band says how bad it is and the row's own words say what it means — `BLOCKS` is \
         gone (`screens/analysis.md` rule 1)"
    );
    let detail = detail_of(row);
    assert_eq!(
        detail[0],
        "default/broken-pdb-floor keeps at least 2 copies of the pods it protects, and right now \
         exactly 2 are healthy. A drain has to take one away, so it waits forever.",
        "the two numbers are `status.desiredHealthy` and `status.currentHealthy` — the committed \
         capture's own (reports/2026-08-21-family-c-corpus-drain-and-capacity.md § 1)"
    );
    assert_eq!(
        action_of(row),
        "run one more copy of what it protects, or lower the minimum it must keep",
        "no jargon in the way out either — the reader is not told to *relax the disruption \
         budget* (`screens/analysis.md` § Drain safety)"
    );
    for paragraph in detail {
        assert!(
            !paragraph.contains("PodDisruptionBudget") && !paragraph.contains("evict"),
            "this report's whole point is that a reader who has never met one learns what it \
             does from the sentence: {paragraph}"
        );
    }
}

#[test]
fn a_node_that_blocks_and_has_pods_nothing_would_restart_says_both() {
    let report = super::drain_safety(&drain_corpus(), &[]);

    // `k8rs-worker3` carries the Deployment's pod *and* two pods started by hand — a reader who
    // cleared the block and met the second problem with no warning is the silent miss this
    // project refuses (NOTES § D46).
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker3")).len(),
        2,
        "the block and the pods nothing would restart, in that order"
    );
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker3"))[1],
        "2 pods here were started by hand, with no Deployment behind them. A drain deletes them \
         and nothing brings them back."
    );
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker2")).len(),
        2,
        "and the same node with one such pod draws the singular"
    );
    // **`1`, not `One`** — the digit convention every other counted row on this page keeps; this
    // was the only counted *paragraph* that spelled its number as a word (NOTES § D134,
    // `screens/analysis.md` § *A paragraph reads differently depending on whether it is the row's
    // own text*). The trailing *"One other rule … has not caught up either"* line is not a
    // paragraph of this family and this turn was not asked to touch it.
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker2"))[1],
        "1 pod here was started by hand, with no Deployment behind it. A drain deletes it and \
         nothing brings it back."
    );

    // **And a node that blocks and has nothing else wrong draws one paragraph** — the mockup's
    // own `node-2`. Without this state the guard on the second paragraph can be `>= 0` and every
    // blocked node quietly gains a *"0 pods here were started by hand"* line; the mutation run
    // found exactly that and this is what closes it.
    let only_the_deployment = ClusterSnapshot {
        pods: captured_deploy_pods(),
        ..drain_corpus()
    };
    let report = super::drain_safety(&only_the_deployment, &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");
    assert_eq!(severity_of(row), Some(Severity::Critical));
    assert_eq!(
        detail_of(row).len(),
        1,
        "the block, and nothing under it that is not true: {:?}",
        detail_of(row)
    );
}

#[test]
fn the_pods_nothing_would_restart_are_the_ones_kubectl_drain_itself_refuses() {
    let report = super::drain_safety(&drain_corpus(), &[]);
    let row = row_for(&report, "k8rs-worker");

    // **Which two pods those are, off the capture rather than off a citation.** The class is
    // `kubectl drain`'s own — it refuses to delete a pod that declares no controller without
    // `--force`, and § 3 of `reports/2026-08-21-family-c-corpus-drain-and-capacity.md` is that
    // refusal run on this cluster, naming `default/broken-nolimits` among nine. It does **not**
    // name `broken-overhead`: that run was taken on a trip that had placed the pod on another
    // node, so the report proves the class and the committed objects prove the membership.
    let cluster = drain_corpus();
    let node = &cluster.nodes[index_of(&cluster, "k8rs-worker")];
    let orphans: Vec<&str> = pods_on(&cluster, node)
        .into_iter()
        .filter(|pod| a_drain_would_move(pod) && pod.owner == pod.id)
        .map(|pod| pod.id.name.as_str())
        .collect();
    assert_eq!(
        orphans,
        [
            "broken-overhead",
            "broken-nolimits",
            "broken-gang",
            "broken-restarts"
        ],
        "all four carry no `metadata.ownerReferences`, which is what `owner == id` means \
         ([`crate::rules::PodSnapshot::owner`])"
    );
    // **The row this node draws is the *local storage* one**, because two of those four also
    // keep files on the machine — so the orphan sentence is a paragraph under it rather than the
    // row's own text, and it still counts all four. `kubectl drain` reports each pod under one
    // reason only (its filters short-circuit); this pane deliberately counts both, because they
    // are two facts a reader needs (`screens/analysis.md` § *A node that would throw away
    // files*).
    assert_eq!(
        text_of(row),
        "k8rs-worker drains, but throws away files on 2 pods"
    );
    assert_eq!(
        detail_of(row)[1],
        "4 pods here were started by hand, with no Deployment behind them. A drain deletes them \
         and nothing brings them back.",
        "and the orphan count is not deduplicated against the local-storage one: {:?}",
        detail_of(row)
    );

    // **The row whose own text is the orphan count**, on a node with nothing else on it.
    let alone = ClusterSnapshot {
        pods: vec![captured_pod("nolimits"), captured_pod("overhead")],
        ..drain_corpus()
    };
    let row = &super::drain_safety(&alone, &[]);
    let row = row_for(row, "k8rs-worker");
    assert_eq!(text_of(row), "k8rs-worker has 2 pods nothing would restart");
    assert_eq!(
        action_of(row),
        "save what you need off them first",
        "a drain deletes them and nothing brings them back, so the way out is not *drain anyway*"
    );
    assert_eq!(
        detail_of(row).len(),
        1,
        "one indented paragraph on this row"
    );
    // **Under its own row the paragraph does not say the count again** — the row above it already
    // says *2 pods*, and *2 pods here were started by hand* on the next line is the same number
    // twice on adjacent lines. The self-contained form is right under a **different** row and
    // wrong under this one (NOTES § D134).
    assert_eq!(
        detail_of(row)[0],
        "They were started by hand, with no Deployment behind them. A drain deletes them and \
         nothing brings them back."
    );

    // The singular is a different sentence and not a plural with an `s` taken off.
    let one = ClusterSnapshot {
        pods: vec![captured_pod("nolimits")],
        ..drain_corpus()
    };
    let report = super::drain_safety(&one, &[]);
    assert_eq!(
        text_of(row_for(&report, "k8rs-worker")),
        "k8rs-worker has 1 pod nothing would restart"
    );
    assert_eq!(
        action_of(row_for(&report, "k8rs-worker")),
        "save what you need off it first"
    );
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker"))[0],
        "It was started by hand, with no Deployment behind it. A drain deletes it and nothing \
         brings it back."
    );
}

#[test]
fn a_node_carrying_only_static_and_daemonset_pods_is_ready_to_drain() {
    // **`a_drain_would_move` is the whole narrowing.** `k8rs-control-plane` runs four static pods
    // and two DaemonSet pods that a drain never evicts, and two CoreDNS pods that it does. Drop
    // the two CoreDNS pods — a subset of a capture, nothing edited — and the machine still
    // carrying six pods is *ready to drain*, not busy.
    let mut pods: Vec<PodSnapshot> = captured_items::<Pod>("kube-system-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();
    let before = pods.len();
    pods.retain(|pod| !pod.id.name.starts_with("coredns"));
    assert_eq!(
        before - pods.len(),
        2,
        "the two CoreDNS pods and nothing else"
    );
    let still_here = pods
        .iter()
        .filter(|pod| pod.node.as_deref() == Some("k8rs-control-plane"))
        .count();
    assert_eq!(
        still_here, 6,
        "four static pods and two DaemonSet pods are still on the node, or this proves nothing"
    );

    let cluster = with_budgets(snapshot(pods, captured_nodes()), captured_budgets());
    let report = super::drain_safety(&cluster, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        text_of(row_for(&report, "k8rs-control-plane")),
        "k8rs-control-plane is ready to drain — nothing on it would move",
        "*0 pods move* about a node carrying six reads as an error rather than as an answer"
    );
    assert_eq!(severity_of(row_for(&report, "k8rs-control-plane")), None);
}

#[test]
fn a_pod_a_drain_has_already_evicted_is_not_a_pod_the_drain_still_has_to_move() {
    // `broken-stuck` is on `k8rs-worker2`, declares no controller, and carries a
    // `deletionTimestamp` — the drain has evicted it and is waiting on it. Counting it would put
    // *2 pods nothing would restart* on a node where one of them is already going.
    let with_stuck = ClusterSnapshot {
        pods: vec![captured_pod("podlimit"), captured_pod("stuck")],
        ..drain_corpus()
    };
    let report = super::drain_safety(&with_stuck, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        text_of(row_for(&report, "k8rs-worker2")),
        "k8rs-worker2 has 1 pod nothing would restart",
        "the terminating pod is not counted"
    );

    // The negative: with the timestamp cleared — the same object one second before the delete
    // was accepted — it is counted, so the assertion above is about the field and not about a
    // pod that was never there.
    let going = captured_pod("stuck");
    assert!(
        going.deletion_timestamp.is_some(),
        "the capture is of a pod being deleted, or nothing below moves"
    );
    let not_going = PodSnapshot {
        deletion_timestamp: None,
        ..going
    };
    let before_delete = ClusterSnapshot {
        pods: vec![captured_pod("podlimit"), not_going],
        ..drain_corpus()
    };
    assert_eq!(
        text_of(row_for(
            &super::drain_safety(&before_delete, &[]),
            "k8rs-worker2"
        )),
        "k8rs-worker2 has 2 pods nothing would restart"
    );
}

/// The findings the real pipeline hands a producer — [`analyze`]'s own output over the same
/// snapshot, never a hand-built slice. Every assertion about the row N1 supplies is made through
/// this, so what is proven is the join and not this file's idea of what a card looks like.
fn findings(cluster: &ClusterSnapshot) -> Vec<Finding> {
    analyze(cluster)
}

/// **The pane the sweeps read, with the findings the real pipeline hands it** — the corpus is
/// carrying a node whose kubelet stopped posting (`nodes.json`'s `k8rs-worker3` at
/// `Ready: Unknown`), and the row about it exists only when N1's card is in hand. Built here
/// because the corpus and the loader are, and read by [`super::every_report`].
pub(super) fn a_node_that_went_quiet() -> Report {
    let cluster = drain_corpus();
    let cards = findings(&cluster);
    super::drain_safety(&cluster, &cards)
}

#[test]
fn a_node_whose_drain_would_throw_away_files_says_so_before_the_drain_is_run() {
    // **The row this box exists for.** A bare `kubectl drain k8rs-worker` on the fixture cluster
    // refused: *"cannot delete Pods with local storage (use --delete-emptydir-data to override):
    // default/broken-gang, default/broken-restarts"*
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 1). The flag that gets
    // past it deletes those files, which is the *"say so before, not forty minutes in"* this
    // report is for.
    let report = super::drain_safety(&drain_corpus(), &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker");

    assert_eq!(
        text_of(row),
        "k8rs-worker drains, but throws away files on 2 pods",
        "the two pods kubectl named, and the row says the drain *finishes* — this is not the \
         `would never finish draining` band"
    );
    assert_eq!(
        severity_of(row),
        Some(Severity::Critical),
        "completing is not the same danger as never completing, but it is a worse one than \
         *nothing recreates this pod*: the reader may not know there was anything on the disk"
    );
    // **Under its own row, which already says *on 2 pods*, the paragraph does not count them
    // again** (NOTES § D134). The self-contained form is what a *louder* row folds in.
    assert_eq!(
        detail_of(row)[0],
        "They keep files on this machine's own disk — what Kubernetes calls an emptyDir volume — \
         and a drain deletes them with the pods.",
        "the jargon is explained where it is used, never printed and left (invariant 14)"
    );
    assert_eq!(
        action_of(row),
        "copy what you need off them first — the replacement pods start with an empty disk",
        "and it teaches the difference from the orphan row: what does not come back is only what \
         was on this one machine's disk"
    );
    assert!(
        matches!(jump_of(row), Some(Jump::Object(id)) if id.name == "k8rs-worker"),
        "navigation to the node, never to an operation"
    );

    // **Singular through the whole row**, and not a plural with an `s` taken off.
    let one = ClusterSnapshot {
        pods: vec![captured_pod("gang")],
        ..drain_corpus()
    };
    let report = super::drain_safety(&one, &[]);
    let row = row_for(&report, "k8rs-worker");
    assert_eq!(
        text_of(row),
        "k8rs-worker drains, but throws away files on 1 pod"
    );
    assert_eq!(
        detail_of(row)[0],
        "It keeps files on this machine's own disk — what Kubernetes calls an emptyDir volume — \
         and a drain deletes it with the pod."
    );
    assert_eq!(
        action_of(row),
        "copy what you need off it first — the replacement pod starts with an empty disk"
    );

    // **Counted over the pods a drain would actually move, and nothing else** — a DaemonSet pod's
    // own emptyDir is never touched, because the drain never evicts the pod. One field moved on
    // the way in (NOTES § D40); the cluster has no DaemonSet with an emptyDir to capture.
    let daemon = ClusterSnapshot {
        pods: vec![captured_pod_but("gang", |pod| {
            pod.metadata.owner_references = Some(vec![OwnerReference {
                api_version: "apps/v1".to_string(),
                kind: "DaemonSet".to_string(),
                name: "a-daemonset".to_string(),
                uid: "uid-a-daemonset".to_string(),
                controller: Some(true),
                block_owner_deletion: None,
            }]);
        })],
        ..drain_corpus()
    };
    assert!(
        daemon.pods[0].local_storage_disk,
        "the plant still keeps files on the node — only who owns it moved"
    );
    assert_eq!(
        text_of(row_for(&super::drain_safety(&daemon, &[]), "k8rs-worker")),
        "k8rs-worker is ready to drain — nothing on it would move",
        "a drain never evicts a DaemonSet pod, so its emptyDir is not this row's concern"
    );
}

/// **The Deployment's pods — the corpus's only pods with a controller behind them — each given
/// one `emptyDir` of one medium, all put on one node.** Neither the corpus nor the fixture
/// cluster names a medium anywhere (`reports/2026-08-21-family-c-drain-rows-and-the-two-new-\
/// decodes.md` § 2), so `medium: Memory` is a plant (NOTES § D40) and a trip that runs a pod with
/// one replaces it. Owned rather than bare on purpose: an orphan count under these rows would be
/// a second reason for the node's text and this asserts the medium's own.
fn owned_pods_keeping_files(medium: Option<&str>, count: usize) -> Vec<PodSnapshot> {
    let medium = medium.map(str::to_string);
    let planted: Vec<PodSnapshot> = captured_items::<Pod>("healthy-deploy-pods")
        .into_iter()
        .take(count)
        .map(|mut pod| {
            let spec = pod.spec.as_mut().expect("the capture has a spec");
            spec.node_name = Some("k8rs-worker".to_string());
            spec.volumes.get_or_insert_with(Vec::new).push(Volume {
                name: "scratch".to_string(),
                empty_dir: Some(EmptyDirVolumeSource {
                    medium: medium.clone(),
                    ..EmptyDirVolumeSource::default()
                }),
                ..Volume::default()
            });
            PodSnapshot::from(pod)
        })
        .collect();
    // **A derived list asserts it found something** (CLAUDE.md § Tests must not lie): *the
    // capture holds fewer pods than this asked for* and *the plant did not apply* would otherwise
    // reach the assertions as a quieter count rather than as a failure here.
    assert_eq!(
        planted.len(),
        count,
        "healthy-deploy-pods.json holds fewer pods than this test needs"
    );
    assert!(
        planted
            .iter()
            .all(|pod| pod.local_storage_disk || pod.local_storage_memory),
        "and every one of them keeps files somewhere, or the row under test is about nothing"
    );
    planted
}

/// The corpus with no budget in it — these pods carry `app: healthy-deploy`, which
/// `broken-pdb-floor` selects, and a blocked row would win the text off every assertion below.
fn no_budgets(pods: Vec<PodSnapshot>) -> ClusterSnapshot {
    ClusterSnapshot {
        pods,
        ..with_budgets(drain_corpus(), Vec::new())
    }
}

#[test]
fn a_node_keeping_files_in_memory_only_needs_one_more_flag_and_loses_nothing() {
    // **A row may not be built on a premise that is false for a common pod** (NOTES § D134).
    // `kubectl drain`'s own filter asks presence only, so a tmpfs refuses a bare drain exactly as
    // a disk-backed `emptyDir` does — and there is nothing to copy off it. Istio's injector adds
    // one to every meshed pod, so the Critical *copy your files off first* row would have fired on
    // every node of every meshed cluster, over a volume that never held anything worth keeping.
    let report = super::drain_safety(
        &no_budgets(owned_pods_keeping_files(Some("Memory"), 2)),
        &[],
    );
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker");

    assert_eq!(
        text_of(row),
        "k8rs-worker drains, but needs one more flag for 2 pods",
        "the drain *finishes* and nothing is lost — this is neither Critical row"
    );
    assert_eq!(
        severity_of(row),
        Some(Severity::Info),
        "nothing here is lost, so ranking it above the orphan row would teach the wrong lesson \
         about which glyph means *act now*"
    );
    assert_eq!(
        detail_of(row)[0],
        "They keep files in memory only — what Kubernetes calls an emptyDir volume set to use \
         memory — and a bare drain refuses to touch them. Nothing is lost: that storage empties \
         every time the container restarts anyway."
    );
    assert_eq!(
        action_of(row),
        "add --delete-emptydir-data when you drain — there is nothing on these pods to copy off \
         first",
        "the one thing a reader can actually do about it — *copy what you need off them* is \
         advice about a volume with nothing to copy"
    );
    assert!(
        !report
            .rows
            .iter()
            .any(|row| matches!(row, Row::Prose(text) if text.starts_with("Every node"))),
        "and the node is not *ready to drain*: a bare `kubectl drain --ignore-daemonsets` \
         genuinely refuses on these pods"
    );

    // Singular through the whole row, and not a plural with an `s` taken off.
    let one = super::drain_safety(
        &no_budgets(owned_pods_keeping_files(Some("Memory"), 1)),
        &[],
    );
    let row = row_for(&one, "k8rs-worker");
    assert_eq!(
        text_of(row),
        "k8rs-worker drains, but needs one more flag for 1 pod"
    );
    assert_eq!(
        detail_of(row)[0],
        "It keeps files in memory only — what Kubernetes calls an emptyDir volume set to use \
         memory — and a bare drain refuses to touch it. Nothing is lost: that storage empties \
         every time the container restarts anyway."
    );
    assert_eq!(
        action_of(row),
        "add --delete-emptydir-data when you drain — there is nothing on this pod to copy off \
         first"
    );

    // **The unset medium is the other row**, or the split is asserting the plant rather than the
    // field: the same two pods with `emptyDir: {}` draw Critical and *throws away files*.
    let disk = super::drain_safety(&no_budgets(owned_pods_keeping_files(None, 2)), &[]);
    let row = row_for(&disk, "k8rs-worker");
    assert_eq!(
        text_of(row),
        "k8rs-worker drains, but throws away files on 2 pods"
    );
    assert_eq!(severity_of(row), Some(Severity::Critical));
}

#[test]
fn the_memory_row_sorts_below_the_orphan_one_and_folds_under_the_disk_one() {
    // **Band order**: real, permanent loss outranks a flag reminder, which outranks nothing at
    // all (`screens/analysis.md` § Drain safety).
    let mut pods = owned_pods_keeping_files(Some("Memory"), 2);
    // Two bare pods on another node — nothing recreates them, which is a `Warn` above this row.
    pods.extend(["nolimits", "overhead"].map(|name| {
        let mut pod = captured_pod(name);
        pod.node = Some("k8rs-worker2".to_string());
        pod
    }));
    let report = super::drain_safety(&no_budgets(pods), &[]);
    println!("{}", pane(&report));
    assert_eq!(
        drained_nodes(&report),
        vec![
            "k8rs-worker2",
            "k8rs-worker",
            "k8rs-control-plane",
            "k8rs-worker3"
        ],
        "the pods nothing would restart first, then the flag reminder, then the ready nodes by \
         name"
    );

    // **A node with both mediums reads as the disk one, and the memory fact folds under it** —
    // self-contained, because it is its own fact and not a continuation of the disk count.
    let mut both = owned_pods_keeping_files(Some("Memory"), 1);
    both.extend(["gang", "restarts"].map(captured_pod));
    let report = super::drain_safety(&no_budgets(both), &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker");
    assert_eq!(
        text_of(row),
        "k8rs-worker drains, but throws away files on 2 pods",
        "real loss outranks a flag reminder"
    );
    assert_eq!(
        detail_of(row),
        [
            "They keep files on this machine's own disk — what Kubernetes calls an emptyDir \
             volume — and a drain deletes them with the pods.",
            "2 pods here were started by hand, with no Deployment behind them. A drain deletes \
             them and nothing brings them back.",
            "1 pod here keeps files in memory only — what Kubernetes calls an emptyDir volume set \
             to use memory — and a drain needs the same extra flag to touch it. Nothing is lost: \
             that storage empties every time the container restarts anyway."
        ],
        "in band order, and the folded memory form names its own count because the row above it \
         does not"
    );
    assert_eq!(
        action_of(row),
        "copy what you need off them first — the replacement pods start with an empty disk",
        "and the way out is the loudest problem's, not a second flag to remember"
    );

    // The plural folded form, on the same shape with two memory pods.
    let mut both = owned_pods_keeping_files(Some("Memory"), 2);
    both.extend(["gang", "restarts"].map(captured_pod));
    let report = super::drain_safety(&no_budgets(both), &[]);
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker"))[2],
        "2 pods here keep files in memory only — what Kubernetes calls an emptyDir volume set to \
         use memory — and a drain needs the same extra flag to touch them. Nothing is lost: that \
         storage empties every time the container restarts anyway."
    );
}

#[test]
fn a_node_that_has_stopped_responding_would_never_finish_draining() {
    // **The false green light this box closes.** `nodes.json`'s `k8rs-worker3` sits at
    // `Ready: Unknown`, and with no blocking budget the pane used to print *"is ready to drain"*
    // about the same object N1's Alerts card calls *"stopped responding"*
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 2). A drain of it cordons
    // the node, evicts, and then waits for a confirmation that never comes.
    // `broken-restarts` keeps files on the machine it runs on and the capture put it on
    // `k8rs-worker`; moved here it makes this node carry all three facts at once, which is what
    // the row's `detail` has to show without letting any of them take the text.
    let mut elsewhere = captured_pod("restarts");
    elsewhere.node = Some("k8rs-worker3".to_string());
    let cluster = ClusterSnapshot {
        // No budget, so nothing else can be what makes the row Critical — this is the shape the
        // review caught, and with the two committed budgets `k8rs-worker3` blocks anyway.
        disruption_budgets: Some(Vec::new()),
        pods: vec![captured_pod("healthy"), elsewhere],
        ..drain_corpus()
    };
    let quiet = super::drain_safety(&cluster, &[]);
    assert_eq!(
        text_of(row_for(&quiet, "k8rs-worker3")),
        "k8rs-worker3 drains, but throws away files on 1 pod",
        "with no findings in hand there is no card to read, and the pane says what it can see"
    );

    let cards = findings(&cluster);
    let report = super::drain_safety(&cluster, &cards);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");
    assert_eq!(
        text_of(row),
        "k8rs-worker3 would never finish draining",
        "the same headline the budget shapes get: the mechanism differs, the answer to *what \
         happens if you drain this* does not"
    );
    assert_eq!(severity_of(row), Some(Severity::Critical));
    assert_eq!(
        detail_of(row)[0],
        "This node has stopped responding. A drain cannot confirm a pod is gone until it answers \
         again, so it waits forever."
    );

    // **The way out is N1's own sentence, read off the card rather than written again** — a
    // reader who has met the Alerts card meets the same instruction here, not a second one to
    // reconcile (NOTES § D46).
    let n1 = cards
        .iter()
        .find(|f| f.object.name == "k8rs-worker3" && f.severity == Severity::Critical)
        .expect("N1 fires on the node the capture caught at Ready: Unknown");
    assert_eq!(
        n1.title, "This node has stopped responding — nothing on it can be trusted until it does",
        "the card this row is reading, named so the two sentences can be compared by a human"
    );
    assert_eq!(action_of(row), n1.action);
    assert_eq!(
        action_of(row),
        "check the node itself: is it powered on and reachable?"
    );

    // **And no other problem is dropped because the first one is louder** — all three facts are
    // true of this node at once, in band order, under a row that says one of them.
    assert_eq!(
        detail_of(row)[1..],
        [
            "1 pod here keeps files on this machine's own disk — what Kubernetes calls an \
             emptyDir volume — and a drain deletes them with the pod.",
            "2 pods here were started by hand, with no Deployment behind them. A drain deletes \
             them and nothing brings them back."
        ]
    );

    // **`⏎` opens the node and not the finding** — every row on this pane goes to the machine,
    // which is what the reader acts on; the card itself is on Alerts, where N1 put it.
    assert!(
        matches!(jump_of(row), Some(Jump::Object(id)) if id.name == "k8rs-worker3"),
        "{:?}",
        jump_of(row)
    );
}

/// **`k8rs-worker3`'s `Ready` condition moved, and nothing else.** `nodes.json` caught it at
/// `Unknown`; the shape below needs `False` and no capture holds one (NOTES § D40,
/// `reports/2026-08-21-family-c-drain-rows-and-the-two-new-decodes.md` § 6). A trip that stops
/// containerd on a worker replaces it.
fn kubelet_said_no(cluster: &mut ClusterSnapshot, at: Option<Time>) {
    let index = index_of(cluster, "k8rs-worker3");
    let ready = cluster.nodes[index]
        .conditions
        .iter_mut()
        .find(|c| c.type_ == "Ready")
        .expect("the captured node carries a Ready condition");
    assert_eq!(
        ready.status, "Unknown",
        "the plant is one field, and this is it"
    );
    ready.status = "False".to_string();
    if let Some(at) = at {
        ready.last_transition = Some(at);
    }
}

const CANNOT_TELL: &str = "This node says it cannot run pods right now — the same thing its \
                           Alerts card says. A kubelet that is still talking might still confirm \
                           an eviction, or it might not. k8rs cannot tell which from here, so \
                           this pane will not guess.";

#[test]
fn a_kubelet_that_answered_and_said_no_is_a_question_this_pane_cannot_close() {
    // **Neither verdict is defensible about a `Ready: False` node, so the pane says so**
    // (NOTES § D134). A drain waits on the **kubelet** to confirm the container is gone, and
    // `KubeletNotReady: container runtime is down` and `PLEG is not healthy` are kubelets that
    // post status and cannot stop a container — their evicted pods sit `Terminating` forever,
    // identically to the `Unknown` case; `NetworkPluginNotReady` is one that can.
    // `conditions[Ready].status` cannot separate them. The first draft ruled the node *ready to
    // drain*, on the same screen where N1's card says it cannot run pods — two screens, one
    // node, opposite advice, which is the blocker this closes.
    let mut cluster = ClusterSnapshot {
        disruption_budgets: Some(Vec::new()),
        pods: Vec::new(),
        ..drain_corpus()
    };
    kubelet_said_no(&mut cluster, None);

    let cards = findings(&cluster);
    let n1 = cards
        .iter()
        .find(|f| f.object.name == "k8rs-worker3" && f.severity == Severity::Critical)
        .expect("N1 fires on both halves — the card is there, and it is not the *silent* one");
    assert_eq!(
        n1.title, "This node says it cannot run pods — nothing new will start here until it can",
        "the answered half, whose card is what this row points a reader back at"
    );

    let report = super::drain_safety(&cluster, &cards);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");
    assert_eq!(
        text_of(row),
        "k8rs-worker3 can't be checked until it is ready again",
        "not *ready to drain*, and not *would never finish draining* — a fact k8rs cannot finish \
         computing from here"
    );
    assert_eq!(
        severity_of(row),
        None,
        "no band: nothing here is urgent by k8rs's own account, because k8rs's own account is \
         exactly what is missing — the family the stale-budget row already sits in"
    );
    assert_eq!(detail_of(row), [CANNOT_TELL]);
    assert_eq!(
        action_of(row),
        "check the node's Alerts card for what is wrong, then look again once it says ready",
        "and it is **not** N1's own action, unlike the `Unknown` row: N1's `False` action ends \
         *what the kubelet says is wrong is above*, and there is no *above* here — this pane \
         never repeats the kubelet's message"
    );
    assert_ne!(
        action_of(row),
        n1.action,
        "the two screens carry one diagnosis and two different ways out on purpose"
    );
    assert!(
        matches!(jump_of(row), Some(Jump::Object(id)) if id.name == "k8rs-worker3"),
        "{:?}",
        jump_of(row)
    );

    // **`ready` is false all the same**, so the all-clear sentence is not drawn over a node
    // nothing could be checked about — the distinction the stale-budget row already makes.
    assert!(
        !report
            .rows
            .iter()
            .any(|row| matches!(row, Row::Prose(text) if text.starts_with("Every node"))),
        "*every node could be drained right now* is false about a node nobody knows about: {:?}",
        report.rows
    );
    // And it sorts with the ready nodes, never with the two Critical kinds.
    assert_eq!(
        drained_nodes(&report),
        vec![
            "k8rs-control-plane",
            "k8rs-worker",
            "k8rs-worker2",
            "k8rs-worker3"
        ],
        "band 0, so the order is the node names"
    );

    // **The negative that makes the plant mean something**: the same node left at `Unknown` is
    // the other row entirely.
    let unknown = super::drain_safety(&drain_corpus(), &findings(&drain_corpus()));
    assert_eq!(
        text_of(row_for(&unknown, "k8rs-worker3")),
        "k8rs-worker3 would never finish draining"
    );
}

#[test]
fn a_node_that_cannot_be_checked_still_says_what_a_drain_would_have_cost() {
    // **The other true facts are folded under it, in band order** — a reader who fixes the node
    // is not then surprised by what draining it would have cost (`screens/analysis.md` § *A node
    // that has stopped responding*). `broken-restarts` keeps files on the machine it runs on and
    // the capture put it on `k8rs-worker`; moved here it makes this node carry three facts.
    let mut elsewhere = captured_pod("restarts");
    elsewhere.node = Some("k8rs-worker3".to_string());
    let mut cluster = ClusterSnapshot {
        disruption_budgets: Some(Vec::new()),
        pods: vec![captured_pod("healthy"), elsewhere],
        ..drain_corpus()
    };
    kubelet_said_no(&mut cluster, None);
    let report = super::drain_safety(&cluster, &findings(&cluster));
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");
    assert_eq!(
        text_of(row),
        "k8rs-worker3 can't be checked until it is ready again",
        "this row wins the text over the local-storage one, because what it says is that the \
         verdict itself is missing"
    );
    assert_eq!(
        detail_of(row),
        [
            CANNOT_TELL,
            "1 pod here keeps files on this machine's own disk — what Kubernetes calls an \
             emptyDir volume — and a drain deletes them with the pod.",
            "2 pods here were started by hand, with no Deployment behind them. A drain deletes \
             them and nothing brings them back."
        ]
    );
}

#[test]
fn a_budget_at_its_floor_outranks_a_kubelet_that_only_said_no() {
    // **A genuine budget block is checked first and wins the row** — a budget refuses at the API
    // server, before the kubelet is ever asked to confirm anything, so *would never finish
    // draining* stays true about that node whether or not its kubelet is answering (NOTES § D134).
    // Only a node with **no** genuine block falls through to the row above.
    let mut cluster = drain_corpus();
    kubelet_said_no(&mut cluster, None);
    let report = super::drain_safety(&cluster, &findings(&cluster));
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");
    assert_eq!(text_of(row), "k8rs-worker3 would never finish draining");
    assert_eq!(severity_of(row), Some(Severity::Critical));
    assert_eq!(
        detail_of(row)[0],
        "default/broken-pdb-floor keeps at least 2 copies of the pods it protects, and right now \
         exactly 2 are healthy. A drain has to take one away, so it waits forever.",
        "the budget's own paragraph leads — there is no *nothing here can be trusted* sentence to \
         put above it, which is what separates this from the `Unknown` node"
    );
    assert_eq!(
        action_of(row),
        "run one more copy of what it protects, or lower the minimum it must keep"
    );
    assert!(
        !detail_of(row).iter().any(|line| line == CANNOT_TELL),
        "and the row does not also say it could not be checked: {:?}",
        detail_of(row)
    );
}

#[test]
fn a_ready_condition_that_flipped_moments_ago_changes_nothing_on_this_pane() {
    // **Inside `NODE_DOWN_GRACE` — the same five minutes N1 itself waits — the row does not
    // appear at all.** No N1 finding exists yet, so the node is read as if nothing were wrong: a
    // five-second blip does not stall the whole pane (`screens/analysis.md` § *A node that has
    // stopped responding*).
    let mut cluster = ClusterSnapshot {
        disruption_budgets: Some(Vec::new()),
        pods: Vec::new(),
        ..drain_corpus()
    };
    kubelet_said_no(&mut cluster, Some(now()));
    let cards = findings(&cluster);
    assert!(
        !cards
            .iter()
            .any(|f| f.object.name == "k8rs-worker3" && f.object.kind == ObjectKind::Node),
        "N1 waits five minutes before it says anything, and this pane reads N1: {cards:?}"
    );
    let report = super::drain_safety(&cluster, &cards);
    println!("{}", pane(&report));
    assert_eq!(
        text_of(row_for(&report, "k8rs-worker3")),
        "k8rs-worker3 is ready to drain — nothing on it would move",
        "exactly as before this row existed"
    );

    // The same for the `Unknown` half — the grace is one rule's, read once, and both halves of
    // this pane wait on it.
    let mut inside = ClusterSnapshot {
        disruption_budgets: Some(Vec::new()),
        pods: Vec::new(),
        ..drain_corpus()
    };
    let index = index_of(&inside, "k8rs-worker3");
    inside.nodes[index]
        .conditions
        .iter_mut()
        .find(|c| c.type_ == "Ready")
        .expect("the captured node carries a Ready condition")
        .last_transition = Some(now());
    let cards = findings(&inside);
    assert_eq!(
        text_of(row_for(
            &super::drain_safety(&inside, &cards),
            "k8rs-worker3"
        )),
        "k8rs-worker3 is ready to drain — nothing on it would move"
    );
}

#[test]
fn n1_is_the_only_critical_node_rule_which_is_what_makes_the_pick_by_identity_enough() {
    // **The pin under [`super::not_ready`].** A `Finding` carries no rule id, so the row
    // above picks N1 out of the slice by kind, name and band — and *band* is only a discriminator
    // while N1 is the one node rule that draws `Critical`. The day a second one does, this test is
    // where it goes red, rather than the drain pane quietly putting another rule's sentence under
    // *would never finish draining*.
    //
    // The cluster is every node state the node rules answer at once: one that went quiet, one
    // cordoned with work left on it, and one under memory pressure.
    let mut cluster = ClusterSnapshot {
        disruption_budgets: Some(Vec::new()),
        ..drain_corpus()
    };
    let cordoned = index_of(&cluster, "k8rs-worker");
    cluster.nodes[cordoned].unschedulable = true;
    let pressure = index_of(&cluster, "k8rs-worker2");
    // The captured node already carries `MemoryPressure: False`, and the rules read a condition
    // by type with `find` — so the plant moves the one that is there rather than appending a
    // second one nothing would ever reach.
    let memory = cluster.nodes[pressure]
        .conditions
        .iter_mut()
        .find(|c| c.type_ == "MemoryPressure")
        .expect("a captured node carries all four conditions");
    assert_eq!(memory.status, "False");
    memory.status = "True".to_string();

    let cards = findings(&cluster);
    let about_nodes: Vec<(&str, Severity, &str)> = cards
        .iter()
        .filter(|f| f.object.kind == ObjectKind::Node)
        .map(|f| (f.object.name.as_str(), f.severity, f.title.as_str()))
        .collect();
    println!("node cards: {about_nodes:#?}");
    assert!(
        about_nodes.len() >= 3,
        "three node rules answered, or this proves nothing about telling them apart: \
         {about_nodes:?}"
    );
    let critical: Vec<&str> = about_nodes
        .iter()
        .filter(|(_, severity, _)| *severity == Severity::Critical)
        .map(|(name, _, _)| *name)
        .collect();
    assert_eq!(
        critical,
        vec!["k8rs-worker3"],
        "N1's node and nobody else's — N2 and N3 are both `Warn`, which is what lets \
         `not_ready` read the band as an identity"
    );

    // **And the pane agrees**: the row belongs to the one node the card is about, and neither the
    // cordoned node nor the one under memory pressure — each of which has a `Warn` card of its
    // own, with its own name on it — reads as a machine that stopped answering.
    let report = super::drain_safety(&cluster, &cards);
    println!("{}", pane(&report));
    assert_eq!(
        never_finish(&report),
        vec!["k8rs-worker3"],
        "one card, one row: a pick that matched a band *or* a kind would put this sentence on \
         every node in the list"
    );
}

#[test]
fn a_budget_protecting_only_pods_a_drain_never_moves_blocks_nothing() {
    // A budget over the kindnet DaemonSet: `kubectl drain` skips DaemonSet pods regardless of
    // flags, so the budget has nothing to say about any node — and a report that joined on the
    // pod list alone would call all four nodes blocked.
    let daemon = captured_budget_but("broken-pdb-floor", |b| {
        b.metadata.namespace = Some("kube-system".to_string());
        b.spec.get_or_insert_with(Default::default).selector = Some(
            k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector {
                match_labels: Some(labels(&[("app", "kindnet")])),
                match_expressions: None,
            },
        );
    });
    let cluster = with_budgets(drain_corpus(), vec![daemon]);
    let report = super::drain_safety(&cluster, &[]);
    println!("{}", pane(&report));
    assert!(
        never_finish(&report).is_empty(),
        "a budget over pods a drain never moves blocks no drain"
    );

    // The negative, one field apart: the same budget over the Deployment's pods — which a drain
    // *does* move — blocks the two nodes carrying them.
    let real = captured_budget_but("broken-pdb-floor", |b| {
        b.metadata.namespace = Some("kube-system".to_string());
    });
    let elsewhere = with_budgets(drain_corpus(), vec![real]);
    assert!(
        never_finish(&super::drain_safety(&elsewhere, &[])).is_empty(),
        "and a budget in another namespace protects nothing here either — a PDB only ever \
         protects pods beside it"
    );
    assert_eq!(
        never_finish(&super::drain_safety(&drain_corpus(), &[])),
        vec!["k8rs-worker2", "k8rs-worker3"],
        "while the captured budget, in its own namespace, blocks the two nodes carrying the pods \
         it protects"
    );
}

#[test]
fn the_budget_with_room_is_the_negative_that_lets_the_blocking_one_fail() {
    // `healthy-pdb-room` allows one eviction, so a node carrying its pods drains. It protects
    // `app: broken-rollout`, which no captured pod carries, so the join is proven from the other
    // side too: the budget is in the snapshot and changes nothing.
    let room = captured_budgets()
        .into_iter()
        .find(|b| b.id.name == "healthy-pdb-room")
        .expect("the capture holds both budgets");
    assert_eq!(room.disruptions_allowed, Some(1));
    let only_room = with_budgets(drain_corpus(), vec![room]);
    let report = super::drain_safety(&only_room, &[]);
    println!("{}", pane(&report));
    assert!(
        never_finish(&report).is_empty(),
        "nothing blocks when the only budget has room"
    );

    // And one field apart — the counter itself — the same object blocks the nodes it protects.
    let at_floor = captured_budget_but("healthy-pdb-room", |b| {
        let status = b.status.get_or_insert_with(Default::default);
        status.disruptions_allowed = Some(0);
        status.current_healthy = Some(1);
        status.desired_healthy = Some(1);
        b.spec.get_or_insert_with(Default::default).selector = Some(
            k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector {
                match_labels: Some(labels(&[("app", "healthy-deploy")])),
                match_expressions: None,
            },
        );
    });
    let blocked = super::drain_safety(&with_budgets(drain_corpus(), vec![at_floor]), &[]);
    assert_eq!(
        detail_of(row_for(&blocked, "k8rs-worker3"))[0],
        "default/healthy-pdb-room keeps at least 1 copy of the pods it protects, and right now \
         exactly 1 is healthy. A drain has to take one away, so it waits forever."
    );
}

#[test]
fn a_budget_at_zero_because_its_workload_is_down_is_not_a_budget_at_its_floor() {
    // **The same zero and a different sentence** (NOTES § D130). The live cluster was caught in
    // exactly this state hours after the capture — `broken-pdb-floor allowed 0 · current 0 ·
    // desired 2` (reports/2026-08-21-family-c-corpus-drain-and-capacity.md § 13.4) — so the
    // plant is a shape the cluster demonstrably produces and the capture happened to miss.
    // *"A drain takes one away"* is false about it and **run one more copy** is not the way out.
    let down = captured_budget_but("broken-pdb-floor", |b| {
        b.status
            .get_or_insert_with(Default::default)
            .current_healthy = Some(0);
    });
    let report = super::drain_safety(&with_budgets(drain_corpus(), vec![down]), &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");

    assert_eq!(
        severity_of(row),
        Some(Severity::Critical),
        "the drain still hangs, so the band does not move"
    );
    assert_eq!(
        detail_of(row)[0],
        "default/broken-pdb-floor keeps at least 2 copies of the pods it protects, and right now \
         none are healthy. It will not let any be moved until they are back — a drain would wait \
         on pods that are already down."
    );
    assert_eq!(
        action_of(row),
        "get the pods it protects healthy again first, then drain",
        "the way out is the workload's, not the budget's — *run one more copy* answers a floor \
         and answers nothing here"
    );
    assert_ne!(
        detail_of(row)[0],
        detail_of(row_for(
            &super::drain_safety(&drain_corpus(), &[]),
            "k8rs-worker3"
        ))[0],
        "and it is a different sentence from the floor's, which is the whole claim — the plant \
         differs from the capture in `status.currentHealthy` alone"
    );
}

/// **The corpus narrowed to the two pods a budget can be read off**, so the budget's own row is
/// the one thing on the node — `k8rs-worker2` and `k8rs-worker3` carry one `healthy-deploy` pod
/// each, owned by a ReplicaSet, with no local storage. On [`drain_corpus`] itself those nodes
/// also carry pods nothing would restart, which is a *louder* row: a budget assertion made there
/// would be asserting the orphan row.
fn only_the_deployments_pods() -> ClusterSnapshot {
    ClusterSnapshot {
        pods: captured_deploy_pods(),
        ..drain_corpus()
    }
}

#[test]
fn a_budget_whose_numbers_have_not_caught_up_is_a_moment_to_wait_and_not_a_drain_that_hangs() {
    // **Rebanded, and the reband is the requirement** (`screens/analysis.md` § *A budget that has
    // not caught up yet*). Upstream's eviction handler refuses *every* eviction while
    // `metadata.generation` is ahead of `status.observedGeneration` — with the same
    // `TooManyRequests` it returns for a full budget — but that refusal is normally over in well
    // under a second and resolves without an operator. It used to draw under this pane's loudest
    // band with *"look again in a moment"* as its way out, which is the band teaching a reader to
    // distrust it.
    //
    // The base is a plant too, and deliberately: a budget with room. Without it the generation
    // check would be asserted on an object that blocks anyway, and a report that never read
    // `generation` at all would pass.
    let with_room = |b: &mut PodDisruptionBudget| {
        b.status
            .get_or_insert_with(Default::default)
            .disruptions_allowed = Some(1);
    };
    let base = captured_budget_but("broken-pdb-floor", with_room);
    let quiet = super::drain_safety(&with_budgets(only_the_deployments_pods(), vec![base]), &[]);
    assert_eq!(
        text_of(row_for(&quiet, "k8rs-worker3")),
        "k8rs-worker3 is ready to drain — 1 pod moves",
        "with room and its numbers current, this budget stops nothing"
    );

    let stale = captured_budget_but("broken-pdb-floor", |b| {
        with_room(b);
        b.metadata.generation = Some(2);
    });
    let report = super::drain_safety(&with_budgets(only_the_deployments_pods(), vec![stale]), &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");
    assert_eq!(
        text_of(row),
        "k8rs-worker3 needs a moment before it can be checked",
        "the row no longer claims the drain would hang — nothing is known yet either way"
    );
    assert_eq!(
        severity_of(row),
        None,
        "no band at all: the same family `node   could not be worked out` sits in on Capacity, a \
         fact k8rs cannot answer yet rather than a verdict"
    );
    assert_eq!(
        detail_of(row)[0],
        "default/broken-pdb-floor was just changed and Kubernetes has not finished counting its \
         healthy pods — the change is version 2, the count is from version 1.",
        "both numbers are on the screen, because a reader has to see them before believing the \
         row (`DisruptionBudgetSnapshot::generation`)"
    );
    assert_eq!(
        action_of(row),
        "wait a few seconds and look again — if it never catches up, check that the cluster's \
         controller manager is running"
    );
    assert_eq!(
        detail_of(row).len(),
        1,
        "one budget is one paragraph — *0 other rules* under it is the off-by-one this asserts \
         against: {:?}",
        detail_of(row)
    );

    // **And it sorts with the ready nodes**, not above them: no urgency to signal, so no reason
    // to outrank *is ready to drain* — the two differ only in which sentence a reader sees.
    assert_eq!(
        drained_nodes(&report),
        vec![
            "k8rs-control-plane",
            "k8rs-worker",
            "k8rs-worker2",
            "k8rs-worker3"
        ],
        "four nodes in name order, two of them waiting on this budget and two of them ready — a \
         band of its own would have put the two waiting first"
    );

    // **A budget nobody has computed at all reaches the same row by the other door.**
    // `observedGeneration` is an `int64` upstream, so absent decodes as 0 and `0 < 1` is the
    // comparison the API server makes — but `None` is not zero on this type, so the sentence
    // says what it saw rather than printing a version nobody wrote.
    let never = captured_budget_but("broken-pdb-floor", |b| {
        with_room(b);
        b.status
            .get_or_insert_with(Default::default)
            .observed_generation = None;
    });
    let report = super::drain_safety(&with_budgets(only_the_deployments_pods(), vec![never]), &[]);
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker3"))[0],
        "default/broken-pdb-floor was just changed and Kubernetes has not finished counting its \
         healthy pods — the change is version 1, the count has not been worked out at all."
    );

    // And a budget with no `metadata.generation` makes no comparison: there is no number to be
    // behind, so nothing is claimed.
    let ungenerated = captured_budget_but("broken-pdb-floor", |b| {
        with_room(b);
        b.metadata.generation = None;
    });
    assert_eq!(
        text_of(row_for(
            &super::drain_safety(
                &with_budgets(only_the_deployments_pods(), vec![ungenerated]),
                &[]
            ),
            "k8rs-worker3"
        )),
        "k8rs-worker3 is ready to drain — 1 pod moves"
    );
}

#[test]
fn two_budgets_still_counting_are_one_paragraph_and_a_count() {
    // **The same shape a blocked node's second budget takes** — the first is named and the rest
    // are counted, because what the reader does about them is one thing and the pane has one row
    // to say it in. Two budgets mid-lag on one node at once is rare and it is not a corner: the
    // controller reaches them one at a time, so a node whose two workloads were both just edited
    // is exactly this.
    let stale = |name: &str| {
        captured_budget_but("broken-pdb-floor", |b| {
            b.metadata.name = Some(name.to_string());
            b.metadata.generation = Some(2);
            b.status
                .get_or_insert_with(Default::default)
                .disruptions_allowed = Some(1);
        })
    };
    let report = super::drain_safety(
        &with_budgets(
            only_the_deployments_pods(),
            vec![stale("second-budget"), stale("first-budget")],
        ),
        &[],
    );
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");

    assert_eq!(
        text_of(row),
        "k8rs-worker3 needs a moment before it can be checked"
    );
    assert_eq!(
        detail_of(row),
        [
            "default/first-budget was just changed and Kubernetes has not finished counting its \
             healthy pods — the change is version 2, the count is from version 1.",
            "One other rule on this node has not caught up either."
        ],
        "the first by `(namespace, name)` is the one named, whatever order they arrived in"
    );
}

#[test]
fn a_budget_still_counting_is_a_trailing_line_when_something_louder_won_the_row() {
    // **Stacks under whichever row won the text, and is never the loudest thing on it**
    // (`screens/analysis.md` § *A budget that has not caught up yet*). `k8rs-worker3` on the
    // corpus carries two pods nothing would restart, which outranks a budget nobody has finished
    // counting — so the transient fact keeps its line and loses the row.
    let stale = captured_budget_but("broken-pdb-floor", |b| {
        b.status
            .get_or_insert_with(Default::default)
            .disruptions_allowed = Some(1);
        b.metadata.generation = Some(2);
    });
    let report = super::drain_safety(&with_budgets(drain_corpus(), vec![stale]), &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");

    assert_eq!(
        text_of(row),
        "k8rs-worker3 has 2 pods nothing would restart",
        "the louder row keeps the text"
    );
    assert_eq!(severity_of(row), Some(Severity::Warn));
    assert_eq!(
        detail_of(row).last().map(String::as_str),
        Some(
            "and default/broken-pdb-floor's numbers have not caught up yet — check again in a \
              moment"
        ),
        "one trailing line, so the reader who clears the orphans is not surprised by a check that \
         had not run: {:?}",
        detail_of(row)
    );
    assert_eq!(
        action_of(row),
        "save what you need off them first",
        "the way out belongs to the row that won the text, and *wait a few seconds* is not it"
    );
}

#[test]
fn a_budget_the_controller_could_not_compute_says_so_instead_of_inventing_the_numbers() {
    // `SyncFailed` — the controller could not resolve the workload's `scale` subresource at all
    // (a CRD owner, a missing verb), so the three counters beside it are not a measurement of
    // anything. **The counter is set to 1 here on purpose**: upstream's `failSafe` writes 0, and
    // a row that reached this sentence only through the zero would be reading the counter it has
    // just been told to distrust.
    //
    // **What a trip would have to do to replace this plant**: add a PDB selecting the pods of a
    // CRD the disruption controller has no `scale` access to. `scripts/broken.yaml` has none.
    let unsynced = captured_budget_but("broken-pdb-floor", |b| {
        let status = b.status.get_or_insert_with(Default::default);
        status.disruptions_allowed = Some(1);
        status
            .conditions
            .as_mut()
            .into_iter()
            .flatten()
            .find(|c| c.type_ == "DisruptionAllowed")
            .expect("the captured budget carries the condition the plant moves")
            .reason = "SyncFailed".to_string();
    });
    let report = super::drain_safety(&with_budgets(drain_corpus(), vec![unsynced]), &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker2");

    assert_eq!(severity_of(row), Some(Severity::Critical));
    assert_eq!(
        detail_of(row)[0],
        "Kubernetes could not work out how many copies of the pods default/broken-pdb-floor \
         protects are healthy, so it will not let any of them be moved. The numbers on it are \
         not a measurement of anything."
    );
    assert_eq!(
        action_of(row),
        "check what default/broken-pdb-floor points at — this happens when it names something \
         Kubernetes cannot count copies of"
    );
}

#[test]
fn a_budget_the_controller_has_not_looked_at_yet_refuses_nothing_on_its_counter_alone() {
    // `disruptions_allowed: None` is *the controller has not reached this budget*, and reading it
    // as zero calls every freshly created budget blocking
    // ([`crate::rules::DisruptionBudgetSnapshot::disruptions_allowed`]). Its own generation is
    // still current here, so the counter is the only thing under test.
    let fresh = captured_budget_but("broken-pdb-floor", |b| {
        b.status
            .get_or_insert_with(Default::default)
            .disruptions_allowed = Some(0);
    });
    assert_eq!(
        fresh.disruptions_allowed,
        Some(0),
        "the capture's own zero, restated — the negative for the `None` below"
    );
    let unknown = DisruptionBudgetSnapshot {
        disruptions_allowed: None,
        ..fresh
    };
    let report = super::drain_safety(&with_budgets(drain_corpus(), vec![unknown]), &[]);
    assert_eq!(
        text_of(row_for(&report, "k8rs-worker3")),
        "k8rs-worker3 has 2 pods nothing would restart",
        "an absent counter fires nothing"
    );

    // And the counters the sentence is built from can be absent while the refusal is not: the row
    // still fires and says only what it can show.
    let bare = captured_budget_but("broken-pdb-floor", |b| {
        let status = b.status.get_or_insert_with(Default::default);
        status.current_healthy = Some(0);
        status.desired_healthy = Some(0);
    });
    let bare = DisruptionBudgetSnapshot {
        current_healthy: None,
        desired_healthy: None,
        ..bare
    };
    let report = super::drain_safety(&with_budgets(drain_corpus(), vec![bare]), &[]);
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker3"))[0],
        "default/broken-pdb-floor will not let any of the pods it protects be moved right now."
    );
}

#[test]
fn two_budgets_on_one_node_are_ordered_the_way_kubectl_prints_them() {
    // **The order is `(namespace, name)`, and the joined `namespace/name` it used to be is not
    // the same answer**: `'-'` (0x2D) sorts before `'/'` (0x2F), so `team-a/api` came out ahead of
    // `team/web` while `kubectl get pdb -A` prints `team web` first
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 7). The row's own action
    // is the first budget's, so the order is on the first blocking row's way out — not a
    // cosmetic.
    //
    // The plant is the captured blocking budget and the captured Deployment pod, each copied into
    // two namespaces whose names are a prefix pair (NOTES § D40) — the shape the whole difference
    // turns on and one no capture has.
    let on_one_node = |namespace: &str| {
        let mut pod = captured_deploy_pods()
            .into_iter()
            .find(|pod| pod.node.as_deref() == Some("k8rs-worker3"))
            .expect("the Deployment has a pod on this node");
        pod.id.namespace = Some(namespace.to_string());
        pod.owner.namespace = Some(namespace.to_string());
        pod
    };
    let budget = |namespace: &str, name: &str| {
        captured_budget_but("broken-pdb-floor", |b| {
            b.metadata.namespace = Some(namespace.to_string());
            b.metadata.name = Some(name.to_string());
        })
    };
    let cluster = ClusterSnapshot {
        pods: vec![on_one_node("team"), on_one_node("team-a")],
        ..with_budgets(
            drain_corpus(),
            vec![budget("team-a", "api"), budget("team", "web")],
        )
    };
    let report = super::drain_safety(&cluster, &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");

    assert!(
        detail_of(row)[0].starts_with("team/web keeps at least"),
        "`kubectl get pdb -A` prints `team web` before `team-a api`, and the joined form reverses \
         exactly that pair: {:?}",
        detail_of(row)
    );
    assert_eq!(
        detail_of(row)[1],
        "team-a/api blocks the drain too.",
        "and the second is named on one line under the first, not given a paragraph of its own: \
         {:?}",
        detail_of(row)
    );
    // The negative: with the *order of arrival* reversed the answer does not move, or this is an
    // assertion about the input rather than about the sort.
    let reversed = ClusterSnapshot {
        disruption_budgets: Some(vec![budget("team", "web"), budget("team-a", "api")]),
        ..cluster
    };
    assert_eq!(
        detail_of(row_for(
            &super::drain_safety(&reversed, &[]),
            "k8rs-worker3"
        ))[0],
        detail_of(row)[0]
    );
}

#[test]
fn every_other_budget_blocking_the_node_is_named_and_the_list_caps_at_two() {
    // **A count is not something a reader can act on.** The row used to say *"2 other rules on
    // this node would stop the drain too"*, so a reader who cleared the named budget drained
    // again and hung on a name that appeared nowhere on the pane
    // (`reports/2026-08-21-family-c-drain-rows-and-the-two-new-decodes.md` § 7, NOTES § D134).
    // The loudest budget still supplies the paragraph; the rest are **named**, capped by
    // `rules.rs`'s own `listed` — the *up to two, then and N more* shape N1's evidence line
    // already uses, reused rather than invented.
    let floors = |names: &[&str]| {
        let extra: Vec<DisruptionBudgetSnapshot> = names
            .iter()
            .map(|name| {
                captured_budget_but("broken-pdb-floor", |b| {
                    b.metadata.name = Some((*name).to_string());
                })
            })
            .collect();
        with_budgets(
            drain_corpus(),
            captured_budgets().into_iter().chain(extra).collect(),
        )
    };

    // Two blocking budgets, one other. `healthy-pdb-room` is in the corpus and blocks nothing, so
    // what is named here is *blocking* budgets and not every budget over the node.
    let report = super::drain_safety(&floors(&["aaa-second-floor"]), &[]);
    println!("{}", pane(&report));
    let detail = detail_of(row_for(&report, "k8rs-worker3"));
    assert_eq!(
        detail[0],
        "default/aaa-second-floor keeps at least 2 copies of the pods it protects, and right now \
         exactly 2 are healthy. A drain has to take one away, so it waits forever.",
        "named first because it sorts first — the order the reader's own `kubectl get pdb -A` \
         prints, and not the order the list arrived in"
    );
    assert_eq!(
        detail[1], "default/broken-pdb-floor blocks the drain too.",
        "singular reads *blocks*, and the name is the thing the reader goes and looks at"
    );
    assert_eq!(
        detail.len(),
        3,
        "the block, the others named on one line, and the pods nothing would restart — a node's \
         row is one line with its explanation under it, so forty budgets may not draw forty \
         paragraphs"
    );

    // Three blocking, both others named — and the verb agrees.
    let report = super::drain_safety(&floors(&["aaa-second-floor", "zzz-third-floor"]), &[]);
    println!("{}", pane(&report));
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker3"))[1],
        "default/broken-pdb-floor and default/zzz-third-floor block the drain too."
    );

    // Six blocking, where the cap starts to matter: two names and a count, never six names.
    let report = super::drain_safety(
        &floors(&[
            "aaa-1-floor",
            "aaa-2-floor",
            "aaa-3-floor",
            "aaa-4-floor",
            "aaa-5-floor",
        ]),
        &[],
    );
    println!("{}", pane(&report));
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker3"))[1],
        "default/aaa-2-floor, default/aaa-3-floor and 3 more block the drain too.",
        "`listed`'s own shape — the third name is worth less than the sentence's readability, \
         and the count that follows carries the total anyway"
    );
}

#[test]
fn a_selector_is_read_on_both_halves_and_an_absent_one_picks_nothing() {
    // **The first `LabelSelector` matcher in this repository**, so its truth table is asserted
    // directly as well as through a pane: upstream's `Requirement.Matches`, operator by operator.
    let held = labels(&[("app", "healthy-deploy"), ("tier", "web")]);
    let requirement = |operator: &str, key: &str, values: &[&str]| SelectorRequirement {
        key: key.to_string(),
        operator: operator.to_string(),
        values: values.iter().map(|v| (*v).to_string()).collect(),
    };
    let expression = |operator: &str, key: &str, values: &[&str]| Selector {
        match_labels: BTreeMap::new(),
        match_expressions: vec![requirement(operator, key, values)],
    };

    let table: Vec<(&str, Selector, bool)> = vec![
        (
            "matchLabels, both held",
            Selector {
                match_labels: held.clone(),
                match_expressions: Vec::new(),
            },
            true,
        ),
        (
            "matchLabels, one wrong",
            Selector {
                match_labels: labels(&[("app", "other")]),
                match_expressions: Vec::new(),
            },
            false,
        ),
        (
            "In, held",
            expression("In", "app", &["healthy-deploy", "other"]),
            true,
        ),
        ("In, not held", expression("In", "app", &["other"]), false),
        (
            "In, key absent",
            expression("In", "missing", &["healthy-deploy"]),
            false,
        ),
        (
            "NotIn, held",
            expression("NotIn", "app", &["healthy-deploy"]),
            false,
        ),
        (
            "NotIn, other value",
            expression("NotIn", "app", &["other"]),
            true,
        ),
        // The one row of upstream's table a hand-written matcher gets backwards.
        (
            "NotIn, key absent",
            expression("NotIn", "missing", &["other"]),
            true,
        ),
        ("Exists", expression("Exists", "app", &[]), true),
        (
            "Exists, absent",
            expression("Exists", "missing", &[]),
            false,
        ),
        (
            "DoesNotExist",
            expression("DoesNotExist", "missing", &[]),
            true,
        ),
        (
            "DoesNotExist, held",
            expression("DoesNotExist", "app", &[]),
            false,
        ),
        // An operator this code does not know must match nothing, exactly as upstream's
        // `LabelSelectorAsSelector` errors rather than guessing.
        (
            "an operator nobody knows",
            expression("Gt", "app", &["1"]),
            false,
        ),
        // Both halves are `and`, so one failing expression sinks a matching label.
        (
            "both halves, expression fails",
            Selector {
                match_labels: labels(&[("app", "healthy-deploy")]),
                match_expressions: vec![requirement("Exists", "missing", &[])],
            },
            false,
        ),
        // **Present and empty is upstream's `labels.Everything()`** — every pod in the
        // namespace, not none of them, and it falls straight out of `all` over an empty list
        // rather than being a case in the code.
        ("present but empty", Selector::default(), true),
    ];
    for (name, selector, expected) in table {
        assert_eq!(
            super::selects(Some(&selector), &held),
            expected,
            "{name}: upstream's own answer for this row"
        );
    }
    // And the row the table cannot hold, because it is the absence of a selector rather than a
    // shape of one: a `null` selector picks nothing at all in `policy/v1`, the reverse of the
    // older API's reading.
    assert!(
        !super::selects(None, &held),
        "absent: a null selector matches no pods"
    );

    // And through the pane, because a check is proven only for the shapes the real pipeline
    // hands it (NOTES § D29): a budget written with `matchExpressions` alone is legal and
    // ordinary, and a matcher reading `matchLabels` only would call the node safe to drain.
    let expressions = captured_budget_but("broken-pdb-floor", |b| {
        b.spec.get_or_insert_with(Default::default).selector = Some(
            k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector {
                match_labels: None,
                match_expressions: Some(vec![
                    k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelectorRequirement {
                        key: "app".to_string(),
                        operator: "In".to_string(),
                        values: Some(vec!["healthy-deploy".to_string()]),
                    },
                ]),
            },
        );
    });
    assert!(
        expressions
            .selector
            .as_ref()
            .is_some_and(|s| s.match_labels.is_empty() && !s.match_expressions.is_empty()),
        "the plant is the expression half alone, or it proves nothing"
    );
    let report = super::drain_safety(&with_budgets(drain_corpus(), vec![expressions]), &[]);
    println!("{}", pane(&report));
    assert_eq!(
        severity_of(row_for(&report, "k8rs-worker3")),
        Some(Severity::Critical),
        "a budget written with expressions alone protects its pods just as much"
    );

    // The absent half through the same door is
    // [`an_empty_selector_protects_every_pod_in_its_namespace_and_an_absent_one_none`]'s, which
    // is where it has to sit: proving *absent* against a pane means proving it beside *empty*,
    // or the assertion passes on the fold that used to make the two one value.
}

#[test]
fn an_empty_selector_protects_every_pod_in_its_namespace_and_an_absent_one_none() {
    // **`{}` and `null` are two different budgets and the snapshot used to decode them to one
    // value.** Upstream says so in the generated docs this repository already vendors —
    // `PodDisruptionBudgetSpec::selector`: *"A null selector will match no pods, while an empty
    // ({}) selector will select all pods within the namespace"*, and `LabelSelector` itself:
    // *"An empty label selector matches all objects. A null label selector matches no
    // objects."* Reading the empty one as nothing is the **false green light** — *"k8rs-worker
    // is ready to drain"* over a budget that hangs the drain — which is the one direction this
    // report may not be wrong in (NOTES § D46).
    let everything = captured_budget_but("broken-pdb-floor", |b| {
        b.spec.get_or_insert_with(Default::default).selector =
            Some(k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector::default());
    });
    assert_eq!(
        everything.selector,
        Some(Selector::default()),
        "the plant is a selector that is *present* and empty — the value an absent one must no \
         longer share"
    );

    // `broken-pdb-floor` lives in `default` and protects `app: healthy-deploy`, whose two pods
    // sit on `k8rs-worker2` and `k8rs-worker3`. `k8rs-worker` carries four other `default` pods
    // and is the node the widened selector reaches — on the corpus its loudest row is the files a
    // drain would throw away, and a budget that protects those pods makes it a drain that never
    // ends instead.
    let report = super::drain_safety(&with_budgets(drain_corpus(), vec![everything]), &[]);
    println!("{}", pane(&report));
    assert_eq!(
        severity_of(row_for(&report, "k8rs-worker")),
        Some(Severity::Critical),
        "an empty selector protects every pod in `default`, so the node carrying two of them \
         blocks — reading it as *protects nothing* is the green light in front of a drain that \
         hangs"
    );
    assert!(
        text_of(row_for(&report, "k8rs-worker")).contains("would never finish draining"),
        "and it says so in the row, not only in the band"
    );

    // **The namespace is still half the join.** `k8rs-control-plane` carries only `kube-system`
    // pods, and a `default` budget saying *everything* says nothing about them.
    assert_eq!(
        severity_of(row_for(&report, "k8rs-control-plane")),
        None,
        "*every pod in the namespace* is not *every pod*, and a budget cannot reach out of its \
         own namespace"
    );

    // The other half of the pair, through the same door: absent is still nothing at all.
    let none = captured_budget_but("broken-pdb-floor", |b| {
        b.spec.get_or_insert_with(Default::default).selector = None;
    });
    assert_eq!(
        none.selector, None,
        "an absent selector decodes to `None`, which is the value `policy/v1` reads as *selects \
         no pods*"
    );
    let report = super::drain_safety(&with_budgets(drain_corpus(), vec![none]), &[]);
    assert!(never_finish(&report).is_empty(), "and it blocks no node");
}

/// **A cluster no drain would hesitate over** — the corpus with no budgets and with only the
/// static and DaemonSet pods a drain never moves. Every node is ready, which is the state the
/// report's closing sentence is about.
pub(super) fn nothing_to_drain() -> ClusterSnapshot {
    ClusterSnapshot {
        disruption_budgets: Some(Vec::new()),
        pods: captured_items::<Pod>("kube-system-pods")
            .into_iter()
            .map(PodSnapshot::from)
            .filter(|pod| !pod.id.name.starts_with("coredns"))
            .collect(),
        ..drain_corpus()
    }
}

#[test]
fn every_node_ready_says_so_in_this_reports_own_words() {
    // The empty state is a real and good one, and it is a `Row::Prose` under the node rows —
    // rule 8, so `views.rs` carries no per-report empty text (NOTES § D128).
    let report = super::drain_safety(&nothing_to_drain(), &[]);
    println!("{}", pane(&report));

    assert_eq!(
        report.rows.len(),
        6,
        "the flag line, four node rows and the sentence — an empty pane is not what a clean \
         answer looks like"
    );
    // **Three clauses, one per class the sentence depends on** — it used to enumerate two of the
    // three, so a node that would throw away files read as *all clear* if it were ever reached
    // (NOTES § D134). One clause covers both emptyDir mediums on purpose: either kind is enough
    // to keep this sentence from being drawn, so *keeps its own files* is true without saying
    // which, the same way *was started by hand* does not say which controller is missing.
    assert!(
        matches!(&report.rows[5], Row::Prose(text)
            if text == "Every node could be drained right now. Nothing on this cluster is \
                        protected by a rule a drain would wait on, nothing on it was started by \
                        hand, and nothing on it keeps its own files, on disk or in memory."),
        "the sentence is last, under the rows it is about, and it names every class it rests \
         on: {:?}",
        report.rows[5]
    );
    // **The DaemonSet assumption still holds inside the empty state** — *could be drained right
    // now* means *with `--ignore-daemonsets`*, the same as every row above it
    // (`screens/analysis.md` § Drain safety).
    assert!(
        matches!(&report.rows[0], Row::Prose(text)
            if text == "A drain below assumes --ignore-daemonsets, so DaemonSet pods never count \
                        as moving."),
        "{:?}",
        report.rows[0]
    );
    assert_eq!(
        selectable(&report).len(),
        4,
        "and it is read, never selected"
    );

    // The negative: one node that is not ready takes the sentence away — and leaves the flag
    // line, which is about every row on the pane rather than about the pane being empty.
    let busy = super::drain_safety(&drain_corpus(), &[]);
    assert!(
        !busy
            .rows
            .iter()
            .any(|row| matches!(row, Row::Prose(text) if text.starts_with("Every node"))),
        "a pane with something to say does not also say there is nothing"
    );
    assert!(matches!(&busy.rows[0], Row::Prose(text) if text.starts_with("A drain below assumes")));
}

#[test]
fn a_pod_whose_node_is_gone_lands_on_no_row_and_takes_no_sentence_away() {
    // **The shape, and the ruling on it** (NOTES § D183): a node deleted while the pods bound to
    // it still name it. Every row here names a machine and says what a drain would move off it,
    // so a pod whose machine is gone belongs to no row rather than being missing from one — and
    // the closing sentence, the one cluster-wide claim this pane makes, is folded from those same
    // rows and so does not move for it either.
    let before = super::drain_safety(&nothing_to_drain(), &[]);

    let after = super::drain_safety(&with_a_pod_whose_node_left(nothing_to_drain()), &[]);
    println!("{}", pane(&after));

    // **No fifth row for the machine that left, and the four it draws are the same four** — read
    // off the jump, so a row that took the plant's node as an identity is caught here even if it
    // never printed the name.
    assert_eq!(drained_nodes(&after), drained_nodes(&before));
    // **And nothing on any of them moved.** The plant is a bare pod a drain would move and
    // nothing would restart: charged to a node it gives that node a band and a row of its own,
    // and charged to nobody it changes nothing (NOTES § D183).
    assert_eq!(pane(&after), pane(&before));
    assert!(
        pane(&after).contains("Every node could be drained right now."),
        "and the pane those comparisons ran over is the all-clear one, whose sentence has a \
         cluster for its subject — two `NotComputed` panes would compare equal and assert \
         nothing (CLAUDE.md § Tests must not lie)"
    );
}

#[test]
fn under_one_namespace_the_whole_pane_is_one_not_computed() {
    // This report says it more loudly than the others: *"18 pods move, node-1 is ok"* is a green
    // light for an operation that then hangs on a pod the report could not see.
    let report = super::drain_safety(&scoped_drain("payments"), &[]);
    println!("{}", pane(&report));

    assert_eq!(
        report.rows.len(),
        1,
        "the whole report, not a section of one"
    );
    assert!(
        selectable(&report).is_empty(),
        "a lone `NotComputed` is a line, not a row `⏎` may land on"
    );
    assert_eq!(report.badge, None);
    let off = not_computed(&report);
    let [(reason, ask_for)] = off[..] else {
        panic!("one `NotComputed` and nothing else: {report:?}");
    };
    assert!(
        reason.contains("payments"),
        "and it names the scope the reader is in: {reason}"
    );
    assert!(
        !reason.contains("403")
            && !reason.contains("RBAC")
            && !reason.contains("PodDisruptionBudget"),
        "the reason is in plain language, and `PodDisruptionBudget` is the jargon this report's \
         own rows spell as *the rules that say how many copies must stay up*: {reason}"
    );
    assert!(
        ask_for.contains("read access") && ask_for.contains("--namespace"),
        "one sentence covering both causes, as Capacity's does"
    );
}

pub(super) fn scoped_drain(namespace: &str) -> ClusterSnapshot {
    ClusterSnapshot {
        namespace_scope: Some(namespace.to_string()),
        ..drain_corpus()
    }
}

#[test]
fn the_three_causes_have_three_ways_out_and_the_widest_one_is_drawn() {
    // Rule 7: one `NotComputed` per section, and when two things are off at once the one that
    // switched off more is the one drawn. Two reasons stacked over an empty pane is two ways out
    // for a reader who can only take one.
    let no_nodes = ClusterSnapshot {
        nodes: Vec::new(),
        ..drain_corpus()
    };
    let no_budgets = ClusterSnapshot {
        disruption_budgets: None,
        ..drain_corpus()
    };
    let all_three = ClusterSnapshot {
        namespace_scope: Some("payments".to_string()),
        nodes: Vec::new(),
        disruption_budgets: None,
        ..drain_corpus()
    };

    let ways_out: Vec<String> = [&no_nodes, &no_budgets, &all_three]
        .iter()
        .map(|cluster| {
            let report = super::drain_safety(cluster, &[]);
            println!("{}", pane(&report));
            assert_eq!(report.rows.len(), 1, "each is the whole pane");
            match &report.rows[0] {
                Row::NotComputed { ask_for, .. } => ask_for.clone(),
                other => panic!("expected one `NotComputed`, got {other:?}"),
            }
        })
        .collect();
    assert_eq!(
        ways_out[0], "Ask for permission to list nodes across the whole cluster.",
        "a login that cannot list nodes asks for nodes"
    );
    assert_eq!(
        ways_out[1], "Ask for permission to list poddisruptionbudgets across the whole cluster.",
        "and one that cannot read the budgets asks for those — the resource by the name an admin \
         types, so the ask can be acted on"
    );
    assert_eq!(
        ways_out[2],
        "Ask for cluster-wide read access, or drop the --namespace flag if you set one.",
        "and with all three off at once the widest fact is the one drawn"
    );
    assert_eq!(
        ways_out[1..].iter().collect::<BTreeSet<_>>().len(),
        2,
        "no two of these say the same thing, or one of the three causes has no way out of its own"
    );
}

#[test]
fn a_budget_whose_first_sync_failed_blocks_the_drain_instead_of_asking_for_a_moment() {
    // **The two fields fed at once, which is the shape that shipped the defect**
    // (`reports/2026-08-22-phase-4-close-cross-family-review.md` § 1): every other test here
    // plants the generation gap *or* `SyncFailed`, and the two rows they prove are opposite ends
    // of this pane's band order.
    //
    // Upstream, `release-1.34`: `failSafe` sets `Status.DisruptionsAllowed = 0` and the
    // `SyncFailed` condition and **does not** advance `Status.ObservedGeneration` — only
    // `updatePdbStatus` does, with `ObservedGeneration: pdb.Generation`. So a budget whose *first*
    // sync failed sits at `generation: 1` / `observedGeneration: 0` forever, and
    // `eviction.go`'s `checkAndDecrement` refuses every eviction of its pods on that comparison
    // alone. Asked in the old order it drew the pane's quietest row: *"wait a few seconds and look
    // again"*, no band, sorted among the ready nodes — and which of the two questions wins when
    // both are true is NOTES § D139.
    //
    // **The plant is failSafe's own output, field for field** — `disruptions_allowed` stays at the
    // capture's 0 because that is what `failSafe` writes, and `observed_generation` is cleared
    // because that is the field it refuses to advance. Cleared and not set to zero: upstream
    // declares it `int64` with `omitempty`, so a status that has never been written carries no
    // key at all and decodes to `None` here. **An explicit zero is legal on the wire too** and is
    // fed at the end of this test, because *absent* and *`Some(0)`* are two shapes of one fact and
    // only one of them is what a capture would hold (NOTES § D29).
    //
    // **What a trip would have to do to replace it**: create a PDB selecting the pods of a CRD the
    // disruption controller cannot resolve `scale` on, and photograph it before anything fixes it.
    // `scripts/broken.yaml` has no such workload.
    let never_synced = |reason: &str| {
        let reason = reason.to_string();
        captured_budget_but("broken-pdb-floor", move |b| {
            let status = b.status.get_or_insert_with(Default::default);
            status.observed_generation = None;
            status
                .conditions
                .as_mut()
                .into_iter()
                .flatten()
                .find(|c| c.type_ == "DisruptionAllowed")
                .expect("the captured budget carries the condition the plant moves")
                .reason = reason;
        })
    };

    let stuck = never_synced("SyncFailed");
    assert_eq!(
        (
            stuck.generation,
            stuck.observed_generation,
            stuck.disruptions_allowed
        ),
        (Some(1), None, Some(0)),
        "the plant is upstream's `failSafe` shape and the assertion says so out loud: a spec at \
         version 1 the controller never counted, and the zero it wrote instead of a count"
    );
    let report = super::drain_safety(&with_budgets(only_the_deployments_pods(), vec![stuck]), &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker3");

    assert_eq!(
        text_of(row),
        "k8rs-worker3 would never finish draining",
        "the eviction API refuses every eviction of this budget's pods until a human fixes it — \
         *needs a moment before it can be checked* is a description of a wait that never ends"
    );
    assert_eq!(
        severity_of(row),
        Some(Severity::Critical),
        "with a band, which the stale row deliberately has none of"
    );
    assert_eq!(
        detail_of(row)[0],
        "Kubernetes could not work out how many copies of the pods default/broken-pdb-floor \
         protects are healthy, so it will not let any of them be moved. The numbers on it are \
         not a measurement of anything.",
        "the sentence is `blocks_a_drain`'s `SyncFailed` one and not a second copy: the counters \
         are not a measurement whether the controller failed on version 1 or on version 9"
    );
    assert_eq!(
        action_of(row),
        "check what default/broken-pdb-floor points at — this happens when it names something \
         Kubernetes cannot count copies of"
    );
    assert_eq!(
        detail_of(row).len(),
        1,
        "and the stale sentence is not appended under it — the budget is named once, by the row \
         that says what is wrong with it: {:?}",
        detail_of(row)
    );
    assert_eq!(
        never_finish(&report),
        vec!["k8rs-worker2", "k8rs-worker3"],
        "both nodes carrying its pods block, and neither is a node the corpus blocks anyway"
    );
    assert_eq!(
        drained_nodes(&report),
        vec![
            "k8rs-worker2",
            "k8rs-worker3",
            "k8rs-control-plane",
            "k8rs-worker"
        ],
        "sorted with the other blockers and above the ready nodes — `k8rs-control-plane` sorts \
         first by name, so this is an assertion about the band"
    );

    // **The negative, one field apart.** The same budget, the same generation gap, the reason the
    // capture came with: a controller that is merely a second behind keeps the calm row. And it
    // is a real negative rather than a restatement — this budget is at its floor
    // (`currentHealthy: 2`, `desiredHealthy: 2`, `disruptionsAllowed: 0`), so a report that
    // stopped asking the generation question at all would draw *would never finish draining* here
    // too, off counters that are not a measurement of anything.
    let behind = never_synced("InsufficientPods");
    let calm = super::drain_safety(
        &with_budgets(only_the_deployments_pods(), vec![behind]),
        &[],
    );
    println!("{}", pane(&calm));
    let row = row_for(&calm, "k8rs-worker3");

    assert_eq!(
        text_of(row),
        "k8rs-worker3 needs a moment before it can be checked",
        "nothing has said it cannot be counted — only that it has not been counted yet"
    );
    assert_eq!(severity_of(row), None, "and no band, as before");
    assert_eq!(
        detail_of(row)[0],
        "default/broken-pdb-floor was just changed and Kubernetes has not finished counting its \
         healthy pods — the change is version 1, the count has not been worked out at all."
    );

    // **And the other spelling of *never counted*, both ways round.** `omitempty` is why the
    // plants above carry no key, but a written zero decodes to `Some(0)` and is behind by the same
    // comparison the API server makes — so the row it draws may not depend on which spelling
    // arrived.
    let spelled_zero = |snapshot: DisruptionBudgetSnapshot| DisruptionBudgetSnapshot {
        observed_generation: Some(0),
        ..snapshot
    };
    assert_eq!(
        text_of(row_for(
            &super::drain_safety(
                &with_budgets(
                    only_the_deployments_pods(),
                    vec![spelled_zero(never_synced("SyncFailed"))]
                ),
                &[]
            ),
            "k8rs-worker3"
        )),
        "k8rs-worker3 would never finish draining",
        "a written zero is the same never-counted budget as an absent key"
    );
    assert_eq!(
        text_of(row_for(
            &super::drain_safety(
                &with_budgets(
                    only_the_deployments_pods(),
                    vec![spelled_zero(never_synced("InsufficientPods"))]
                ),
                &[]
            ),
            "k8rs-worker3"
        )),
        "k8rs-worker3 needs a moment before it can be checked",
        "and it is still the reason that decides, not the spelling of the number"
    );
}
