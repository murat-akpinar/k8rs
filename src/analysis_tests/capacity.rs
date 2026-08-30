//! `analysis.rs` § THE CAPACITY REPORT — its tests (NOTES § D91).

use super::*;

// --- CAPACITY ---
//
// **The producer, against the committed corpus** — the four hand-built panes that stood here
// until this box landed are gone, and what replaces each of them is the same claim asserted
// against a snapshot (the note at the head of this file).

/// **One node's allocatable set relative to what the pods on it already promise** — the plant
/// every state below is built from. Nothing in the capture is over the line (every node is a
/// twelve-CPU machine running pods that ask for milli-CPUs), and the value written on is the one
/// the same snapshot produced, so no committed capture is edited (NOTES § D53).
///
/// `delta` is millicores: `-1` is one short of what the pods promise, `0` is packed exactly full
/// — legal and ordinary, and the line N5 is silent at (NOTES § D81).
fn cpu_allocatable_at(mut cluster: ClusterSnapshot, name: &str, delta: i64) -> ClusterSnapshot {
    let index = index_of(&cluster, name);
    let asked = {
        let node = &cluster.nodes[index];
        promised(
            &pods_on(&cluster, node),
            Some("1"),
            |p| p.cpu_request.as_deref(),
            |c| c.cpu_request.as_deref(),
            |p| p.overhead_cpu.as_deref(),
        )
        .expect("every captured request parses")
        .0
    };
    assert!(
        asked > 0,
        "a node planted at zero promised cpu proves nothing about a boundary: {name}"
    );
    cluster.nodes[index].allocatable_cpu = Some(format!("{}m", asked + delta));
    cluster
}

/// The same, on the dimension that kills a running pod rather than stopping the next one. `delta`
/// is whole bytes here, because that is the unit an allocatable is written in.
fn memory_allocatable_at(mut cluster: ClusterSnapshot, name: &str, delta: i64) -> ClusterSnapshot {
    let index = index_of(&cluster, name);
    let asked = {
        let node = &cluster.nodes[index];
        promised(
            &pods_on(&cluster, node),
            Some("1"),
            |p| p.memory_request.as_deref(),
            |c| c.memory_request.as_deref(),
            |p| p.overhead_memory.as_deref(),
        )
        .expect("every captured memory request parses")
        .0
    };
    assert!(
        asked > 1000 && asked % 1000 == 0,
        "a byte count times 1000 — if this is not whole the exact allocatable cannot be spelled"
    );
    cluster.nodes[index].allocatable_memory = Some((asked / 1000 + delta).to_string());
    cluster
}

pub(super) fn one_node_over_on_cpu(name: &str) -> ClusterSnapshot {
    cpu_allocatable_at(corpus(), name, -1)
}

fn one_node_over_on_memory(name: &str) -> ClusterSnapshot {
    memory_allocatable_at(corpus(), name, -1)
}

/// **A cluster on which every pod is capped** — `broken-podlimit` declares its memory limit once
/// at pod level and its CPU limit on its container, so the limits row has nothing to count and
/// the states that only differ when it is empty become reachable.
fn only_capped() -> ClusterSnapshot {
    snapshot(vec![captured_pod("podlimit")], captured_nodes())
}

/// **metrics-server's answer, planted out of the corpus's own sums.** There is no committed
/// metrics capture and there cannot be one from this cluster — `kubectl top nodes` answers
/// `error: Metrics API not available` on it (`reports/2026-08-21-family-c-corpus-drain-and-\
/// capacity.md` § 5) — so each node's usage is what the pods on it already promise, read back
/// through [`promised`] and spelled in `{n}m` and [`bytes`]'s own binary suffixes. A cluster where
/// every pod uses exactly what it asked for is the one shape whose usage numbers the capture
/// itself contains.
///
/// **Two things it may not be cited for, and both were.** Its numbers are the row's own sum, so
/// no test built on it can tell a `using` line that read the map from one that re-derived the
/// requests — that is [`metrics_saying`]'s. And these are not the units metrics-server writes:
/// those are nanocores and `Ki`, also [`metrics_saying`]'s.
///
/// `absent` is the nodes left **out** of the answer — one that joined between polls, which is not
/// the same fact as *no metrics at all* and is why [`Metrics::Read`] carries a map.
pub(super) fn metrics_read(cluster: &ClusterSnapshot, absent: &[&str]) -> Metrics {
    let mut answered = BTreeMap::new();
    for node in &cluster.nodes {
        if absent.contains(&node.id.name.as_str()) {
            continue;
        }
        let pods = pods_on(cluster, node);
        let sum = |of_pod: fn(&PodSnapshot) -> Option<&str>,
                   of_container: fn(&ContainerSnapshot) -> Option<&str>,
                   of_overhead: fn(&PodSnapshot) -> Option<&str>| {
            promised(&pods, Some("1"), of_pod, of_container, of_overhead)
                .expect("every captured quantity parses")
                .0
        };
        answered.insert(
            node.id.name.clone(),
            NodeUsage {
                cpu: format!(
                    "{}m",
                    sum(
                        |p| p.cpu_request.as_deref(),
                        |c| c.cpu_request.as_deref(),
                        |p| p.overhead_cpu.as_deref()
                    )
                ),
                memory: bytes(sum(
                    |p| p.memory_request.as_deref(),
                    |c| c.memory_request.as_deref(),
                    |p| p.overhead_memory.as_deref(),
                )),
            },
        );
    }
    Metrics::Read(answered)
}

/// **metrics-server's answer with nobody's arithmetic in it** — one node at a time, in the units
/// metrics-server itself writes: CPU in **nanocores** (`137669270n`) and memory in **`Ki`**.
///
/// It exists because [`metrics_read`] cannot answer either of the two questions this plant does.
/// Its usage is the request sum read back, so every node's `using …` line is byte-identical to
/// the row above it and an implementation that ignored the map entirely would draw the same pane;
/// and its spellings are the report's own, so the framing a value actually arrives in had never
/// been fed (NOTES § D31). Nodes not named here are absent from the answer, which is
/// [`metrics_read`]'s `absent`.
pub(super) fn metrics_saying(usage: &[(&str, &str, &str)]) -> Metrics {
    Metrics::Read(
        usage
            .iter()
            .map(|(node, cpu, memory)| {
                (
                    (*node).to_string(),
                    NodeUsage {
                        cpu: (*cpu).to_string(),
                        memory: (*memory).to_string(),
                    },
                )
            })
            .collect(),
    )
}

pub(super) fn with_metrics(cluster: ClusterSnapshot, metrics: Metrics) -> ClusterSnapshot {
    ClusterSnapshot {
        metrics: Some(metrics),
        ..cluster
    }
}

/// Every node row's indented paragraphs, by node name.
fn details(report: &Report) -> Vec<(&str, Vec<&str>)> {
    node_names(report)
        .into_iter()
        .map(|name| {
            (
                name,
                detail_of(row_for(report, name))
                    .iter()
                    .map(String::as_str)
                    .collect(),
            )
        })
        .collect()
}

/// The node rows of a report, by the name each one begins with — the sort order, read back off
/// the strings the pane actually draws rather than off the snapshot they were built from.
fn node_names(report: &Report) -> Vec<&str> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Answer {
                text,
                jump: Some(Jump::Object(id)),
                ..
            } if id.kind == ObjectKind::Node => text.split_whitespace().next(),
            _ => None,
        })
        .collect()
}

/// The limits row's count, or `None` when the pane draws no such row.
fn uncapped_count(report: &Report) -> Option<usize> {
    report.rows.iter().find_map(|row| match row {
        Row::Answer { text, .. } if text.contains("no memory or CPU limit") => {
            text.split_whitespace().next().and_then(|n| n.parse().ok())
        }
        _ => None,
    })
}

#[test]
fn the_flagged_node_leads_and_the_rest_follow_by_name() {
    // `k8rs-worker3` is last alphabetically, so an order that merely sorted by name would put it
    // last — which is what makes this an assertion about *flagged first* and not about `sort`.
    let report = super::capacity(&one_node_over_on_cpu("k8rs-worker3"), &[]);
    println!("{}", pane(&report));
    assert_eq!(
        node_names(&report),
        vec![
            "k8rs-worker3",
            "k8rs-control-plane",
            "k8rs-worker",
            "k8rs-worker2"
        ],
        "flagged nodes first, then node name (screens/analysis.md § Capacity, *Many nodes*)"
    );

    // One band in the pane, on the one node that is not fine — every other row makes no
    // judgement, which is what keeps the glyph worth looking at.
    assert_eq!(severity_of(&report.rows[0]), Some(Severity::Warn));
    for row in &report.rows[1..4] {
        assert_eq!(severity_of(row), None);
    }

    // The badge counts flagged nodes and nothing else.
    let badge = report
        .badge
        .expect("one node is over, so the sidebar says so");
    assert_eq!(badge.value, "1");
    assert_eq!(badge.severity, Severity::Warn);
    assert_eq!(
        super::capacity(&corpus(), &[]).badge,
        None,
        "nothing over the line is no badge at all — `Some(\"0\")` says a different thing"
    );
}

#[test]
fn a_node_row_names_both_dimensions_and_hands_on_the_rules_own_way_out() {
    let cluster = one_node_over_on_cpu("k8rs-worker3");
    let report = super::capacity(&cluster, &[]);
    let row = row_for(&report, "k8rs-worker3");
    println!("{}", text_of(row));

    // **Both dimensions on every row, always**: CPU overcommitment stops the next pod that asks
    // for CPU from fitting, memory overcommitment gets a running one killed, and a report that
    // names one and not the other teaches the wrong lesson about which to watch.
    let text = text_of(row);
    assert!(text.contains(" cpu · "), "one row, both dimensions: {text}");
    assert!(
        text.starts_with("k8rs-worker3   "),
        "the name leads and nothing is right-aligned into a column: {text}"
    );

    // The consequence, and only the one that applies: this node is over on CPU alone.
    assert_eq!(
        detail_of(row),
        ["A pod that asks for CPU will not fit here until something moves off."],
        "the sentence says what happens, and it does not say *nothing new can start here* — a \
         pod that requests nothing is placed on a full node all day (NOTES § D81)"
    );

    // **N5's own sentence, not a second one written here.** A row and the rule behind it telling
    // a reader to do two different things is the divergence NOTES § D46 is about.
    let n5 = node_overcommitted(&cluster, &cluster.nodes[index_of(&cluster, "k8rs-worker3")])
        .expect("the plant is over the line");
    assert_eq!(action_of(row), n5.action);
    assert!(!action_of(row).is_empty());

    // Every node row goes to its node, and no rule fired for the healthy ones: a per-node sum is
    // a report's answer, not a finding.
    let Some(Jump::Object(id)) = jump_of(row) else {
        panic!("a node row jumps to its node");
    };
    assert_eq!(id.kind, ObjectKind::Node);
    assert_eq!(id.name, "k8rs-worker3");
    assert!(id.uid.is_some(), "the capture carries the node's uid");
    assert_eq!(
        action_of(row_for(&report, "k8rs-worker")),
        "",
        "a node that is fine has nothing to do about it, and empty is drawn by leaving the line \
         out"
    );
    assert!(detail_of(row_for(&report, "k8rs-worker")).is_empty());
}

#[test]
fn the_memory_consequence_is_a_different_sentence_and_both_can_be_drawn_at_once() {
    let memory = super::capacity(&one_node_over_on_memory("k8rs-worker"), &[]);
    assert_eq!(
        detail_of(row_for(&memory, "k8rs-worker")),
        ["If these pods use what they asked for, one of them is killed."],
        "memory overcommitment kills a running pod; CPU overcommitment does not, and one \
         sentence for both would teach the wrong lesson"
    );

    // Both at once: two paragraphs, and the order is the row's own — CPU then memory, as the
    // numbers read left to right. A `Vec` is exactly what makes that assertable (NOTES § D129).
    let mut both = one_node_over_on_memory("k8rs-worker");
    let index = index_of(&both, "k8rs-worker");
    both.nodes[index].allocatable_cpu = one_node_over_on_cpu("k8rs-worker").nodes[index]
        .allocatable_cpu
        .clone();
    let row = super::capacity(&both, &[]);
    let row = row_for(&row, "k8rs-worker");
    println!("{:?}", detail_of(row));
    assert_eq!(detail_of(row).len(), 2);
    assert!(detail_of(row)[0].contains("will not fit"));
    assert!(detail_of(row)[1].contains("killed"));
    assert_eq!(
        severity_of(row),
        Some(Severity::Warn),
        "one band for both dimensions — this whole screen is *risky later*, and the kill itself \
         is Alerts' rule 2 (NOTES § D2)"
    );
}

#[test]
fn the_report_and_n5_never_disagree_about_a_node() {
    // **The defect NOTES § D46 names, asserted directly**: the row's band and the rule's verdict
    // are two readings of one sum, and a reader who sees a banded row and an unbanded card — or
    // the reverse — has no way to tell which one to believe.
    for cluster in [
        corpus(),
        one_node_over_on_cpu("k8rs-worker"),
        one_node_over_on_memory("k8rs-worker3"),
    ] {
        let report = super::capacity(&cluster, &[]);
        let banded: Vec<(&str, bool)> = cluster
            .nodes
            .iter()
            .map(|n| {
                (
                    n.id.name.as_str(),
                    severity_of(row_for(&report, &n.id.name)).is_some(),
                )
            })
            .collect();
        let n5: Vec<(&str, bool)> = cluster
            .nodes
            .iter()
            .map(|n| {
                (
                    n.id.name.as_str(),
                    node_overcommitted(&cluster, n).is_some(),
                )
            })
            .collect();
        println!("{banded:?}");
        assert_eq!(banded, n5);
    }
    // And the loop above is not vacuous: one of the three clusters really does band a node.
    assert!(
        super::capacity(&one_node_over_on_cpu("k8rs-worker"), &[])
            .badge
            .is_some()
    );
}

#[test]
fn the_sandbox_charge_is_in_the_number_the_row_prints() {
    // The report reads [`promised`], so what NOTES § D124 unfroze in `charged` reaches this pane
    // — and it reaches only the node the sandboxed pod is on, which is what makes this an
    // assertion about the charge rather than about the sum being some number.
    let with = corpus();
    let placed = captured_pod("overhead")
        .node
        .expect("the capture records the node broken-overhead runs on");
    let mut without = corpus();
    for pod in &mut without.pods {
        pod.overhead_cpu = None;
        pod.overhead_memory = None;
    }
    let (with, without) = (super::capacity(&with, &[]), super::capacity(&without, &[]));
    for name in node_names(&with) {
        let (a, b) = (
            text_of(row_for(&with, name)),
            text_of(row_for(&without, name)),
        );
        println!("{name}\n  with    {a}\n  without {b}");
        if name == placed {
            assert_ne!(
                a, b,
                "{name} carries a RuntimeClass overhead and the row is short by it \
                              without one"
            );
        } else {
            assert_eq!(a, b, "no pod on {name} declares one");
        }
    }
}

#[test]
fn the_row_for_a_probe_nobody_ran_may_not_report_what_the_answer_was() {
    // **`metrics: None` is *k8rs did not ask*, and it is the value through the whole of Phase 4**
    // ([`crate::rules::ClusterSnapshot::metrics`]). The other three sentences in this slot are
    // claims about the *cluster*; this one may make none of them, because nothing was asked —
    // *this cluster does not have it installed* is false on every cluster that does.
    let report = super::capacity(&corpus(), &[]);
    let rows = not_computed(&report);
    assert_eq!(rows.len(), 1, "one `NotComputed` per section, never two");
    let (reason, ask_for) = rows[0];
    println!("{reason}\n{ask_for}");
    assert!(
        reason.contains("metrics-server"),
        "it still names where the number would have come from: {reason}"
    );
    assert!(
        !reason.contains("does not have it installed")
            && !reason.contains("did not answer")
            && !reason.contains("not allowed"),
        "k8rs never asked, so it may not report what the answer was: {reason}"
    );
    assert!(
        !strings_of(&report).iter().any(|s| s.starts_with("using ")),
        "and no row draws a measurement, because there is none to draw"
    );
}

pub(super) fn scoped(namespace: &str) -> ClusterSnapshot {
    ClusterSnapshot {
        namespace_scope: Some(namespace.to_string()),
        ..corpus()
    }
}

pub(super) fn no_nodes() -> ClusterSnapshot {
    ClusterSnapshot {
        nodes: Vec::new(),
        ..corpus()
    }
}

#[test]
fn a_namespace_scope_switches_the_node_section_off_and_the_limits_row_keeps_counting() {
    let report = super::capacity(&scoped("payments"), &[]);
    println!("{}", pane(&report));

    assert_eq!(report.rows.len(), 3);
    let Row::NotComputed { reason, ask_for } = &report.rows[0] else {
        panic!("the promised/usable section is what switches off");
    };
    assert!(reason.contains("Not checked here"));
    assert!(
        reason.contains("payments"),
        "it names the namespace the reader can see, which is the whole reason the number would \
         come out low: {reason}"
    );
    assert!(
        !reason.contains("403") && !reason.contains("RBAC") && !reason.contains("scoped snapshot"),
        "{reason}"
    );
    // Both causes in one sentence: the screen cannot tell a `--namespace` flag from a 403
    // fallback and does not need to (NOTES § D46).
    assert!(ask_for.contains("read access") && ask_for.contains("--namespace"));

    // The limits row keeps counting, under the line that stops it being read as cluster-wide.
    assert!(
        matches!(&report.rows[1], Row::Prose(t) if t == "Still counted, from what you can see:")
    );
    assert_eq!(
        uncapped_count(&report),
        uncapped_count(&super::capacity(&corpus(), &[]))
    );

    // **Nothing is drawn where nothing was computed** — no dash, no placeholder, no greyed-out
    // list, and no line of any kind names a node.
    assert!(
        !strings_of(&report).iter().any(|s| s.contains("k8rs-")),
        "a list of dashes invites the reader to look for the one row that does have a number"
    );
    assert_eq!(report.badge, None);
    assert_eq!(
        selectable(&report),
        vec!["8 workloads have no memory or CPU limit"],
        "the switched-off section and its heading are lines, not rows `⏎` may land on"
    );

    // **And with nothing left to count, the pane is that one line and nothing else** — a report
    // that could not be computed at all, which is one `Row::NotComputed` and no cursor
    // (NOTES § D127). The heading is not drawn over an empty space.
    let nothing_left = ClusterSnapshot {
        namespace_scope: Some("payments".to_string()),
        ..only_capped()
    };
    let report = super::capacity(&nothing_left, &[]);
    println!("{}", pane(&report));
    assert_eq!(report.rows.len(), 1);
    assert!(selectable(&report).is_empty());
}

#[test]
fn an_empty_node_list_is_a_login_that_may_not_list_nodes_and_not_a_namespace_scope() {
    // **`ClusterSnapshot::nodes` is a `Vec` and a cluster always has nodes**, so empty *is* this
    // state (`screens/analysis.md` § *Capacity's remaining states*) — a real RBAC shape, distinct
    // from a scope: this login may read pods everywhere and nodes nowhere.
    let report = super::capacity(&no_nodes(), &[]);
    println!("{}", pane(&report));
    let rows = not_computed(&report);
    assert_eq!(rows.len(), 1);
    let (reason, ask_for) = rows[0];
    assert!(reason.contains("list nodes"), "{reason}");
    assert!(
        !reason.contains("--namespace") && !ask_for.contains("--namespace"),
        "there is no flag to drop here, and offering one is advice that does nothing: {ask_for}"
    );
    assert!(ask_for.contains("list nodes"));
    assert_ne!(
        (reason, ask_for),
        not_computed(&super::capacity(&scoped("payments"), &[]))[0],
        "two different causes, two different ways out"
    );
    assert_eq!(report.badge, None);
    assert_eq!(
        uncapped_count(&report),
        Some(8),
        "the limits row still counts"
    );

    // **Both at once draws one row, and it is the scope's** — rule 7, the one that switched off
    // more: a scope also narrows the pods the limits row counts.
    let both = ClusterSnapshot {
        nodes: Vec::new(),
        ..scoped("payments")
    };
    assert_eq!(
        not_computed(&super::capacity(&both, &[])),
        not_computed(&super::capacity(&scoped("payments"), &[]))
    );
}

#[test]
fn a_report_with_nothing_to_say_says_so_in_its_own_words() {
    // `broken-podlimit` declares its memory limit once at pod level and its CPU limit on the
    // container, so nothing on this cluster is uncapped and no node is near its allocatable.
    let report = super::capacity(&only_capped(), &[]);
    println!("{}", pane(&report));
    assert_eq!(
        uncapped_count(&report),
        None,
        "no row, rather than a row saying zero"
    );
    assert_eq!(report.badge, None);
    let Some(Row::Prose(text)) = report.rows.last() else {
        panic!("the pane ends in the sentence rule 8 asks for");
    };
    assert!(text.contains("Nothing to do."), "{text}");
    assert!(
        !report.rows.is_empty() && node_names(&report).len() == 4,
        "the pane is never empty, because a cluster always has nodes"
    );

    // And the sentence is not drawn when there *is* something to say — the half that makes the
    // assertion above discriminate.
    let flagged = super::capacity(&one_node_over_on_cpu("k8rs-worker"), &[]);
    assert!(
        !strings_of(&flagged)
            .iter()
            .any(|s| s.contains("Nothing to do.")),
        "a pane with a flagged node has something to say"
    );
}

#[test]
fn one_node_is_a_pane_and_not_a_broken_one() {
    // A laptop cluster — the shape that must not look like a failure.
    let one = captured_nodes()
        .into_iter()
        .find(|n| n.id.name == "k8rs-worker")
        .expect("the capture has this node");
    let report = super::capacity(&snapshot(captured_pods(), vec![one]), &[]);
    println!("{}", pane(&report));
    assert_eq!(node_names(&report), vec!["k8rs-worker"]);
    assert!(text_of(row_for(&report, "k8rs-worker")).contains(" of "));
    assert_eq!(
        uncapped_count(&report),
        Some(8),
        "the limits row counts every workload in the snapshot, not only the ones on this node"
    );
}

#[test]
fn a_node_name_longer_than_the_region_is_neither_padded_nor_broken() {
    // Wrapping is `views.rs`'s and it wraps at a space (`screens/analysis.md` rule 4). What is
    // asserted here is only that this layer does nothing about width: the long row is the short
    // row with a different name in front of it, byte for byte.
    let long = "ip-10-0-134-201.eu-west-1.compute.internal";
    let mut cluster = corpus();
    let index = index_of(&cluster, "k8rs-worker");
    cluster.nodes[index].id.name = long.to_string();
    // The join is by name, so the pods move with it — otherwise this compares a row with pods
    // against a row with none and passes for the wrong reason.
    for pod in &mut cluster.pods {
        if pod.node.as_deref() == Some("k8rs-worker") {
            pod.node = Some(long.to_string());
        }
    }
    let renamed = super::capacity(&cluster, &[]);
    let plain = super::capacity(&corpus(), &[]);

    let (a, b) = (
        text_of(row_for(&renamed, long)),
        text_of(row_for(&plain, "k8rs-worker")),
    );
    println!("{a}\n{b}");
    assert_eq!(
        a.strip_prefix(long),
        b.strip_prefix("k8rs-worker"),
        "nothing pads to a column width two layers below the renderer"
    );
    assert!(
        !a.contains('\n'),
        "and nothing breaks a line it cannot measure"
    );
}

#[test]
fn a_node_whose_numbers_cannot_be_read_keeps_its_row() {
    // **Both shapes the pipeline can produce** (NOTES § D29): a node that does not say what it
    // has, and a pod on it whose request does not parse. A node dropped from the pane instead is
    // one machine silently absent from the report — the defect NOTES § D81 paid for once.
    let silent = {
        let mut c = corpus();
        let i = index_of(&c, "k8rs-worker");
        c.nodes[i].allocatable_cpu = None;
        c
    };
    let unreadable = {
        let mut c = corpus();
        for pod in &mut c.pods {
            if pod.node.as_deref() == Some("k8rs-worker") {
                for container in &mut pod.containers {
                    container.cpu_request = Some("not a number".to_string());
                }
            }
        }
        c
    };
    for cluster in [silent, unreadable] {
        let report = super::capacity(&cluster, &[]);
        println!("{}", pane(&report));
        let row = row_for(&report, "k8rs-worker");
        assert_eq!(node_names(&report).len(), 4, "the machine keeps its row");
        assert_eq!(severity_of(row), None, "an unread number is not a verdict");
        assert!(detail_of(row).last().unwrap().contains("could not read"));
        assert!(matches!(jump_of(row), Some(Jump::Object(_))));
        assert_eq!(
            report.badge, None,
            "and nothing is badged off a number nobody has"
        );
        // **Both dimensions are still on the row** (`screens/analysis.md` § Capacity): the one
        // that could not be read says so, and the one that could keeps its numbers. Only CPU is
        // unreadable in either plant above.
        assert!(
            text_of(row).starts_with("k8rs-worker   cpu could not be worked out · "),
            "{}",
            text_of(row)
        );
        assert!(
            text_of(row).ends_with("Gi"),
            "the memory sum came out and is drawn: {}",
            text_of(row)
        );
        // The other three still answer — one node's unreadable quantity does not take the pane.
        assert!(text_of(row_for(&report, "k8rs-worker2")).contains(" of "));
    }
}

#[test]
fn a_dimension_that_could_be_read_is_judged_even_when_the_other_one_could_not() {
    // **The report and N5 may not disagree about one node** (NOTES § D46). `promised` answers per
    // dimension, and so does [`node_overcommitted`] — so a node whose CPU quantity is unreadable
    // and whose *memory* is over its allocatable is a node the rule flags, and the row used to
    // hide it behind `could not be worked out` and leave it out of the badge
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § nit 11).
    let mut cluster = memory_allocatable_at(corpus(), "k8rs-worker", -1);
    let index = index_of(&cluster, "k8rs-worker");
    cluster.nodes[index].allocatable_cpu = None;
    let report = super::capacity(&cluster, &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker");

    assert!(
        text_of(row).starts_with("k8rs-worker   cpu could not be worked out · "),
        "{}",
        text_of(row)
    );
    assert_eq!(
        severity_of(row),
        Some(Severity::Warn),
        "the readable half is over the line, and N5 says so about this same node"
    );
    assert_eq!(
        node_overcommitted(&cluster, &cluster.nodes[index])
            .map(|f| f.action)
            .as_deref(),
        Some(action_of(row)),
        "and the way out is the rule's own, never a second one written here"
    );
    assert_eq!(
        report.badge.as_ref().map(|b| b.value.as_str()),
        Some("1"),
        "the badge counts the flagged nodes, and this one is flagged"
    );
    assert_eq!(
        detail_of(row),
        [
            "If these pods use what they asked for, one of them is killed.",
            "One of the numbers here — what this node has to give, or what a pod on it asked \
             for — is written in a way k8rs could not read."
        ],
        "what was found first, what is missing last"
    );

    // **And when neither side could be read there is no dimension to name** — the screen's own
    // row, and no band, because nothing was measured and so nothing was judged.
    let mut neither = cluster;
    neither.nodes[index].allocatable_memory = None;
    let report = super::capacity(&neither, &[]);
    println!("{}", pane(&report));
    let row = row_for(&report, "k8rs-worker");
    assert_eq!(text_of(row), "k8rs-worker   could not be worked out");
    assert_eq!(severity_of(row), None);
    assert_eq!(action_of(row), "");
    assert_eq!(detail_of(row).len(), 1);
    assert_eq!(report.badge, None);
}

#[test]
fn the_limits_row_asks_both_levels_and_charges_nothing_to_a_pod_that_finished() {
    // One pod at a time, because each of these breaks a different naive version of the count.
    for (name, want) in [
        // Nothing declared anywhere.
        ("nolimits", Some(1)),
        // A pod-level memory limit and a container CPU limit: capped, and a count that asked only
        // the containers would report it unlimited (NOTES § D51).
        ("podlimit", None),
        // Its app container is capped both ways; its *init* container has no CPU limit, and an
        // init container with none can take the whole machine for as long as it runs.
        ("healthy", Some(1)),
        ("overhead", None),
        // A pod that finished is charged to nobody and takes no node.
        ("succeeded", None),
    ] {
        let report = super::capacity(&snapshot(vec![captured_pod(name)], captured_nodes()), &[]);
        println!("{name} -> {:?}", uncapped_count(&report));
        assert_eq!(uncapped_count(&report), want, "{name}");
    }

    // **One inflects.** The row is a sentence a beginner reads, not a template.
    let one = super::capacity(
        &snapshot(vec![captured_pod("nolimits")], captured_nodes()),
        &[],
    );
    assert_eq!(
        text_of(row_for(&one, "1")),
        "1 workload has no memory or CPU limit"
    );
    assert_eq!(
        detail_of(row_for(&one, "1")),
        ["Nothing stops one taking a whole node."]
    );
    assert_eq!(
        jump_of(row_for(&one, "1")),
        None,
        "it stands for a set of objects and `Jump` has no case for one (NOTES § D128)"
    );

    // **The shape the pod-level half of the count exists for** (NOTES § D29). On a running pod
    // the kubelet writes the pod-level limit down into `status.containerStatuses[].resources` and
    // `effective` reports it as the container's own, so `broken-podlimit` answers *capped*
    // through its containers alone. A container whose status carries no `resources` and whose
    // name matches no spec entry decodes with nothing — the gap `rules.rs` names for
    // virtual-kubelet, serverless nodes and sandboxed runtimes — and there the pod-level limit is
    // the only thing capping it.
    let declared_once = captured_pod_but("podlimit", |p| {
        let spec = p.spec.as_mut().expect("a captured pod has a spec");
        // The cpu limit moves from the container up to the pod, which is where a pod declaring
        // its resources once puts it. The value is the capture's own (NOTES § D40, § D53).
        let container = spec.containers[0]
            .resources
            .take()
            .expect("the capture declares container resources");
        let pod_level = spec
            .resources
            .as_mut()
            .expect("and pod-level ones beside them");
        let cpu = container
            .limits
            .as_ref()
            .and_then(|l| l.get("cpu"))
            .expect("the capture declares a container cpu limit")
            .clone();
        pod_level
            .limits
            .get_or_insert_with(Default::default)
            .insert("cpu".to_string(), cpu);
        p.status
            .as_mut()
            .expect("a captured pod has a status")
            .container_statuses
            .as_mut()
            .expect("the kubelet reported on this container")[0]
            .resources = None;
    });
    assert_eq!(
        declared_once.containers.len(),
        1,
        "the plant keeps its container, or `all` over an empty list answers the question for it"
    );
    assert!(
        declared_once
            .containers
            .iter()
            .all(|c| c.cpu_limit.is_none() && c.memory_limit.is_none()),
        "and that container declares nothing, or the assertion below passes through the branch \
         it is not about: {:?}",
        declared_once
            .containers
            .iter()
            .map(|c| (c.cpu_limit.as_deref(), c.memory_limit.as_deref()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        uncapped_count(&super::capacity(
            &snapshot(vec![declared_once], captured_nodes()),
            &[]
        )),
        None,
        "the pod-level limits are what cap this pod, and a count that asked only the containers \
         would report it unlimited (NOTES § D51)"
    );

    // **A pod the kubelet has not reported on is not counted, because nothing can be read about
    // it.** `PodSnapshot::containers` is built from `status.containerStatuses`, so a Pending pod
    // decodes with none at all — not with all-`None` containers — and `all` over an empty list
    // would answer *capped* for a pod nobody looked at.
    let unreported = captured_pod("pending");
    assert!(
        unreported.containers.is_empty(),
        "broken-pending never started, so the kubelet reported on nothing"
    );
    assert_eq!(
        uncapped_count(&super::capacity(
            &snapshot(vec![unreported], captured_nodes()),
            &[]
        )),
        None,
        "every workload this row counts is one that provably has no limit"
    );

    // And the corpus count moves with what is added to it, in the direction the predicate says —
    // **once per workload**, so a second copy of a pod that is already counted moves nothing.
    let base = uncapped_count(&super::capacity(&corpus(), &[])).expect("the corpus has some");
    let mut again = corpus();
    again.pods.push(captured_pod("nolimits"));
    assert_eq!(
        uncapped_count(&super::capacity(&again, &[])),
        Some(base),
        "`broken-nolimits` is already in the corpus, so a second copy of it is the same workload \
         — this is the count that used to say `{}`",
        base + 1
    );
    let mut more = corpus();
    more.pods.push(captured_pod_but("nolimits", |pod| {
        pod.metadata.name = Some("broken-nolimits-two".to_string());
        pod.metadata.uid = Some("uid-broken-nolimits-two".to_string());
    }));
    assert_eq!(
        uncapped_count(&super::capacity(&more, &[])),
        Some(base + 1),
        "and a pod nothing controls is its own workload, so a second uncontrolled one is a \
         second workload"
    );
    let mut capped = corpus();
    capped.pods.push(captured_pod("podlimit"));
    assert_eq!(uncapped_count(&super::capacity(&capped, &[])), Some(base));
}

#[test]
fn the_limits_row_counts_workloads_and_not_the_copies_of_them() {
    // **The number and the noun have to agree** (PRIOR-ART § F2,
    // `reports/2026-08-21-family-c-analysis-report-family-review.md` § 5). On the fixture cluster
    // this row counted `41 workloads` about ten controllers; on a cluster running 50-replica
    // Deployments it counted the replicas. `workload` means a controller everywhere else in this
    // product, and one Deployment with no limit set is one thing to go and fix.
    //
    // The plant is the Deployment's two captured pods with their limits taken off — they sit on
    // two different nodes and share one ReplicaSet, which is the shape a per-pod count cannot
    // tell from two separate workloads (NOTES § D40).
    let uncapped: Vec<PodSnapshot> = captured_deploy_pods()
        .into_iter()
        .map(|mut pod| {
            for container in &mut pod.containers {
                container.cpu_limit = None;
                container.memory_limit = None;
            }
            pod
        })
        .collect();
    assert_eq!(uncapped.len(), 2);
    assert_eq!(
        uncapped[0].owner, uncapped[1].owner,
        "two copies of one workload, which is the whole plant"
    );
    assert_ne!(uncapped[0].node, uncapped[1].node, "and on two nodes");
    assert_eq!(
        uncapped_count(&super::capacity(
            &snapshot(uncapped.clone(), captured_nodes()),
            &[]
        )),
        Some(1),
        "one row about one Deployment, and `2 workloads` is the sentence this fixes"
    );

    // The negative, one field apart: give one copy a different controller and there are two
    // workloads to fix.
    let mut split = uncapped;
    split[1].owner.name = "healthy-deploy-somethingelse".to_string();
    assert_eq!(
        uncapped_count(&super::capacity(&snapshot(split, captured_nodes()), &[])),
        Some(2)
    );
}

/// Every node row of a pane as its reader sees it — the name **and** the numbers beside it, so a
/// comparison across two panes catches a sum that moved and not only a row that appeared.
fn node_rows(report: &Report) -> Vec<String> {
    node_names(report)
        .into_iter()
        .map(|name| text_of(row_for(report, name)).to_string())
        .collect()
}

#[test]
fn a_pod_whose_node_is_gone_belongs_to_no_row_and_is_still_counted_as_a_workload() {
    // **The shape, and the ruling on it** (NOTES § D183): a node deleted while the pods bound to
    // it still name it. Per-node answers cannot hold such a pod — each one names a machine and
    // sums what is on it — while the limits row asks about workloads and not about machines, so
    // it must.
    let before = super::capacity(&corpus(), &[]);
    let rows = node_rows(&before);
    let count = uncapped_count(&before).expect("the corpus has uncapped workloads");
    assert_eq!(
        rows.len(),
        4,
        "the rows the assertion below searches — *found nothing* and *nothing to find* print the \
         same green (CLAUDE.md § Tests must not lie)"
    );

    let report = super::capacity(&with_a_pod_whose_node_left(corpus()), &[]);
    println!("{}", pane(&report));

    // **On no row, and no row is a millicore different for it**: the node rows are the same four
    // the corpus draws, carrying the same sums, with no fifth one added for the machine.
    assert_eq!(
        node_rows(&report),
        rows,
        "a pod whose machine is gone belongs to no per-node row rather than being missing from \
         one, and its 350m is charged to nobody (NOTES § D183)"
    );
    // **And no row stands for the machine that left**, whatever shape it might have taken: one
    // would print on every scale-down of every cluster that runs an autoscaler (NOTES § D183).
    // Read over every string the pane carries, title and badge included, because a row that is
    // not a node row would not be in the comparison above.
    let strings = strings_of(&report);
    assert!(
        strings.iter().any(|s| s.contains("k8rs-worker2")),
        "the strings searched below are the pane's own — *found nothing* and *nothing to find* \
         print the same green"
    );
    assert!(
        !strings.iter().any(|s| s.contains(NODE_THAT_LEFT)),
        "nothing on this pane names a machine the snapshot does not have: {strings:?}"
    );

    // **And it is counted, because *which of my workloads has no limit* is asked about workloads
    // and not about machines.** A node-scoped denominator here would make the count drop when a
    // machine left — the number moving for a reason the reader did not cause.
    assert_eq!(
        uncapped_count(&report),
        Some(count + 1),
        "the limits row filters on `finished` and on nothing else (NOTES § D183)"
    );
}

#[test]
fn at_exactly_its_allocatable_a_node_says_nothing_and_one_step_past_it_says_why() {
    // **The line, from both sides, on the report's side of it** (NOTES § D81). A node packed to
    // exactly its allocatable is legal and ordinary — `noderesources.Fit` admits while
    // `request <= allocatable - requested`, and `describe node` prints `cpu 3920m (100%)` without
    // comment — so a row that flags it is a row nobody can act on.
    for (delta, says) in [(1, false), (0, false), (-1, true)] {
        let cpu = cpu_allocatable_at(corpus(), "k8rs-worker", delta);
        let memory = memory_allocatable_at(corpus(), "k8rs-worker", delta);
        for (dimension, cluster) in [("cpu", cpu), ("memory", memory)] {
            let report = super::capacity(&cluster, &[]);
            let row = row_for(&report, "k8rs-worker");
            println!(
                "{dimension} {delta:+} -> {:?} {:?}",
                severity_of(row),
                detail_of(row)
            );
            assert_eq!(
                severity_of(row).is_some(),
                says,
                "{dimension} at {delta:+} of what the pods promise"
            );
            assert_eq!(
                detail_of(row).len(),
                usize::from(says),
                "and the consequence is drawn exactly when the band is: {:?}",
                detail_of(row)
            );
        }
    }
}

#[test]
fn the_nothing_to_do_sentence_needs_every_half_of_nothing_to_be_true() {
    // **Three ways to have something to say, and the sentence is drawn only when none of them
    // holds.** An uncapped workload, a node over its allocatable, and a node whose numbers could
    // not be read are separate facts, and a pane that says *Nothing to do.* under any of them is
    // telling a reader to stop looking.
    let says_nothing = |cluster: &ClusterSnapshot| {
        strings_of(&super::capacity(cluster, &[]))
            .iter()
            .any(|s| s.contains("Nothing to do."))
    };
    assert!(
        says_nothing(&only_capped()),
        "the state the sentence is for"
    );

    // Capped everywhere, but one node is over the line.
    let over = cpu_allocatable_at(only_capped(), "k8rs-worker2", -1);
    assert!(
        super::capacity(&over, &[]).badge.is_some(),
        "the plant really does flag a node, or the assertion below is about nothing"
    );
    assert!(!says_nothing(&over));

    // Capped everywhere, but one node did not say what it has.
    let mut unreadable = only_capped();
    let index = index_of(&unreadable, "k8rs-worker2");
    unreadable.nodes[index].allocatable_cpu = None;
    assert!(!says_nothing(&unreadable));

    // And an uncapped workload alone is enough — the corpus, where no node is near its line.
    assert!(!says_nothing(&corpus()));
}

#[test]
fn metrics_that_answer_put_a_measurement_under_every_node_and_name_nothing() {
    // **A dependency that is working is not news** (`screens/analysis.md` § *Live usage*): when
    // the numbers are there, the pane draws them and says nothing at all about where they came
    // from. The slot that explains their absence is empty, not filled with a reassurance.
    let cluster = corpus();
    let cluster = with_metrics(cluster.clone(), metrics_read(&cluster, &[]));
    let report = super::capacity(&cluster, &[]);
    println!("{}", pane(&report));

    for (name, detail) in details(&report) {
        assert_eq!(
            detail.len(),
            1,
            "{name} draws its measurement and nothing else — no node here is over its line"
        );
        assert!(
            detail[0].starts_with("using ") && detail[0].contains(" cpu and "),
            "{name}: {:?}",
            detail[0]
        );
    }
    assert!(
        not_computed(&report).is_empty(),
        "nothing on this pane says why a number is missing, because none is: {:?}",
        not_computed(&report)
    );
    assert!(
        !strings_of(&report)
            .iter()
            .any(|s| s.contains("metrics-server")),
        "and nothing names the dependency that answered"
    );

    // **The measurement is spelled by the same two functions as the row above it** — printing the
    // API's own string would put `234Mi` and `245366784000` on adjacent lines. The corpus's
    // `k8rs-worker` promises 450m, so its usage line says so in `cpu_text`'s spelling.
    let worker = detail_of(row_for(&report, "k8rs-worker"));
    println!("{:?}", worker[0]);
    assert_eq!(worker[0], "using 0.45 cpu and 234Mi");
}

#[test]
fn a_node_using_far_less_than_it_asked_for_says_two_different_numbers() {
    // **The ordinary shape of every cluster anybody runs**, and the one this file had never fed:
    // what a pod asked for is a reservation and what it uses is a measurement, and the whole
    // reason the `using …` line exists is that the two are different numbers. Every metrics test
    // beside this one is built on [`metrics_read`], whose usage *is* the request sum read back —
    // so a `using` that ignored the map and re-derived the requests would pass all of them.
    //
    // **And the quantities arrive spelled the way metrics-server spells them** (NOTES § D31, the
    // framing a value comes in): `n` and `Ki` are in [`quantity_milli`]'s table and had never
    // reached it from this file.
    let cluster = with_metrics(
        corpus(),
        metrics_saying(&[("k8rs-worker", "137669270n", "1035316Ki")]),
    );
    let report = super::capacity(&cluster, &[]);
    println!("{}", pane(&report));

    let row = row_for(&report, "k8rs-worker");
    // `137669270n` is 137.66927 millicores, which [`quantity_milli`] charges as the whole 138 it
    // cannot subdivide; `1035316Ki` is 1_060_163_584 bytes, which [`bytes`] truncates to 1011Mi.
    assert_eq!(detail_of(row), ["using 0.138 cpu and 1011Mi"]);
    assert_eq!(
        text_of(row),
        "k8rs-worker   0.45 of 12 cpu · 234Mi of 23.1Gi",
        "and the row above it is the promise — a different number in both dimensions, which is \
         the whole point of drawing them one under the other"
    );
    assert!(
        not_computed(&report).is_empty(),
        "the cluster answered, so nothing on this pane explains an absence"
    );
}

#[test]
fn a_usage_number_k8rs_cannot_read_draws_no_measurement_at_all() {
    // **The third shape [`using`]'s `None` covers**, documented and never fed until this test:
    // beside *nobody probed* and *this node was not in the answer* there is *the answer did not
    // parse*. A quantity off the metrics API is a string like any other, and the apiserver's
    // grammar admits far more than a node ever has (NOTES § D81).
    //
    // **The whole paragraph goes, and a readable half goes with it** — asserted here rather than
    // only stated. `using 0.138 cpu and` has no ending, and *using 0.138 cpu* alone is a second
    // sentence shape for a measurement that is a nice-to-have beside a row whose own two numbers
    // are complete without it. Nothing is drawn where nothing was measured, which is the
    // direction every absent number on this screen takes.
    for usage in [
        ("137669270n", "not a quantity"),
        ("not a quantity", "1035316Ki"),
        ("not a quantity", "also not one"),
    ] {
        let cluster = with_metrics(
            corpus(),
            metrics_saying(&[("k8rs-worker", usage.0, usage.1)]),
        );
        let report = super::capacity(&cluster, &[]);
        assert!(
            detail_of(row_for(&report, "k8rs-worker")).is_empty(),
            "{usage:?} draws half a sentence"
        );
        // And the slot that explains an absence stays empty: the cluster answered, so nothing
        // about the *cluster* is missing — this is one number k8rs could not read.
        assert!(not_computed(&report).is_empty(), "{usage:?}");
    }
}

#[test]
fn a_node_the_metrics_did_not_report_on_draws_no_measurement_and_the_pane_still_says_nothing() {
    // A node that joined between polls. **This is not *no metrics at all*** — every other node
    // keeps its number, and the slot that explains an absence stays empty, because the answer is
    // not missing for the cluster.
    let cluster = corpus();
    let cluster = with_metrics(cluster.clone(), metrics_read(&cluster, &["k8rs-worker2"]));
    let report = super::capacity(&cluster, &[]);
    println!("{}", pane(&report));

    assert_eq!(
        details(&report)
            .into_iter()
            .map(|(name, detail)| (name, detail.len()))
            .collect::<Vec<_>>(),
        vec![
            ("k8rs-control-plane", 1),
            ("k8rs-worker", 1),
            ("k8rs-worker2", 0),
            ("k8rs-worker3", 1)
        ],
        "nothing is drawn where nothing was measured — no `—`, no per-row parenthetical"
    );
    assert!(
        not_computed(&report).is_empty(),
        "one node missing from the answer is not the cluster having no answer"
    );
}

#[test]
fn every_way_the_probe_can_fail_draws_its_own_way_out_and_no_two_are_the_same() {
    // **Five states, one slot** (`screens/analysis.md` § *Live usage*). Four are the cluster's and
    // one is k8rs's own; each names the check and the way out, and a reader can only act on the
    // right one — *install it*, *check its pods*, *ask for the permission*, *nothing to do*.
    let mut seen: Vec<(String, String)> = Vec::new();
    for (name, metrics, wants) in [
        ("nobody asked", None, "Nothing to ask for"),
        (
            "not installed",
            Some(Metrics::NotInstalled),
            "Install metrics-server",
        ),
        (
            "silent",
            Some(Metrics::Silent),
            "Check that its pods are running",
        ),
        ("denied", Some(Metrics::Denied), "metrics.k8s.io"),
    ] {
        let cluster = ClusterSnapshot {
            metrics,
            ..corpus()
        };
        let report = super::capacity(&cluster, &[]);
        let rows = not_computed(&report);
        assert_eq!(
            rows.len(),
            1,
            "{name}: one `NotComputed` per section, never two"
        );
        let (reason, ask_for) = rows[0];
        println!("{name}\n  {reason}\n  {ask_for}");
        assert!(ask_for.contains(wants), "{name}: {ask_for}");
        assert!(
            !reason.contains("403")
                && !reason.contains("RBAC")
                && !reason.contains("metrics.k8s.io"),
            "{name} names the check in plain language, never the API or the status code: {reason}"
        );
        // Nothing is drawn where nothing was measured: not one node carries a `using` line.
        assert!(
            details(&report).iter().all(|(_, detail)| detail.is_empty()),
            "{name} draws a measurement it does not have"
        );
        seen.push((reason.to_string(), ask_for.to_string()));
    }
    let distinct: BTreeSet<&(String, String)> = seen.iter().collect();
    assert_eq!(
        distinct.len(),
        4,
        "four causes, four ways out — two states drawing one sentence is a reader told to do the \
         wrong thing about half of them: {seen:?}"
    );

    // And the negative for all four: a cluster that answered draws none of them.
    let answered = corpus();
    assert!(
        not_computed(&super::capacity(
            &with_metrics(answered.clone(), metrics_read(&answered, &[])),
            &[]
        ))
        .is_empty()
    );
}

#[test]
fn the_measurement_comes_before_the_sentence_that_says_what_it_means() {
    // **Order is the claim, not the count** (NOTES § D129): swapped, the reader meets the
    // consequence before the number it is about. A `Vec` is what makes it assertable.
    let cluster = one_node_over_on_cpu("k8rs-worker3");
    let cluster = with_metrics(cluster.clone(), metrics_read(&cluster, &[]));
    let report = super::capacity(&cluster, &[]);
    println!("{}", pane(&report));

    let flagged = detail_of(row_for(&report, "k8rs-worker3"));
    assert_eq!(flagged.len(), 2);
    assert!(flagged[0].starts_with("using "), "{:?}", flagged[0]);
    assert!(flagged[1].contains("will not fit"), "{:?}", flagged[1]);
    assert_eq!(
        detail_of(row_for(&report, "k8rs-worker")).len(),
        1,
        "a healthy node draws the measurement alone, because there is nothing to explain"
    );
}

#[test]
fn the_node_section_being_off_takes_the_metrics_row_with_it_however_the_probe_went() {
    // **The fifth row of `screens/analysis.md` § *Live usage*'s table**: a usage number with
    // nothing to compare it against is PRIOR-ART § F2's number with no denominator, so the
    // section is one `NotComputed` and that is the whole of it (rule 7).
    let answered = corpus();
    let answered = metrics_read(&answered, &[]);
    for (name, cluster) in [
        ("scoped, nobody asked", scoped("payments")),
        ("no nodes, nobody asked", no_nodes()),
        (
            "scoped, metrics answered",
            with_metrics(scoped("payments"), answered.clone()),
        ),
        (
            "no nodes, metrics answered",
            with_metrics(no_nodes(), answered.clone()),
        ),
        (
            "scoped, no metrics-server",
            ClusterSnapshot {
                metrics: Some(Metrics::NotInstalled),
                ..scoped("payments")
            },
        ),
    ] {
        let report = super::capacity(&cluster, &[]);
        assert_eq!(
            not_computed(&report).len(),
            1,
            "{name}: one row, and it is the node section's"
        );
        assert!(
            !strings_of(&report)
                .iter()
                .any(|s| s.contains("metrics-server") || s.starts_with("using ")),
            "{name}: no metrics row at all, and no measurement either"
        );
    }
}

#[test]
fn the_denied_metrics_row_names_the_group_that_is_the_only_thing_telling_the_two_nodes_apart() {
    // **The one `ask_for` where the resource plural alone misleads** (NOTES § D187). Every other
    // one names a resource that exists in exactly one group; `nodes` exists in two, the reader
    // very likely already holds the core one, and the group is the whole of the difference. The
    // requirement is that a reader can write the Role rule off this line — verb, resource, group —
    // not that the sentence matches a literal.
    let cluster = ClusterSnapshot {
        metrics: Some(Metrics::Denied),
        ..corpus()
    };
    let report = super::capacity(&cluster, &[]);
    let (_, ask_for) = not_computed(&report)[0];
    println!("denied → {ask_for}");
    for token in ["list", "nodes", "metrics.k8s.io"] {
        assert!(
            ask_for.contains(token),
            "the Role rule cannot be written without `{token}`: {ask_for}"
        );
    }
}
