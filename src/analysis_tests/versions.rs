//! `analysis.rs` § THE VERSIONS REPORT — its tests (NOTES § D91).

use super::*;

use crate::rules::kubelet_too_far_behind;

// --- VERSIONS ---
//
// **The producer, against the committed corpus.** Every captured node runs the version the
// capture stamped into `K8S_VERSION`, which is the *healthy* state and the only one a kind
// cluster built in one command can be in — so every flagged state below is a kubelet version
// moved on the decoded snapshot (NOTES § D40), never an edit to `nodes.json` (NOTES § D53).
//
// **The window is three minor versions and it is not this file's number**: it is
// `kubelet_too_far_behind`'s, and NOTES § D81 is where it was corrected from two. The boundary is
// walked *with* the rule rather than written down beside it, so a changed constant moves the test
// and the code together — and the one `3` below is read back out of N4's own sentence, which is
// where the window is named.

/// The version the capture came off, read from the file `just fixtures` stamps rather than
/// written down here — the same source `rules_tests`' snapshot uses. A literal would assert the
/// trip that took the capture and not the report.
fn server_version() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/K8S_VERSION"
    ))
    .expect("the capture stamps the version it came from")
    .trim()
    .to_string()
}

/// The corpus as the Versions report is handed it: the captured nodes, and the control plane's
/// own version beside them.
pub(super) fn version_corpus() -> ClusterSnapshot {
    ClusterSnapshot {
        server_version: Some(server_version()),
        ..corpus()
    }
}

/// One node's kubelet moved to a version of its own — the plant every flagged state is built
/// from. The control plane stays where the capture put it.
pub(super) fn kubelet_at(
    mut cluster: ClusterSnapshot,
    node: &str,
    version: &str,
) -> ClusterSnapshot {
    let i = index_of(&cluster, node);
    cluster.nodes[i].kubelet_version = Some(version.to_string());
    cluster
}

/// The control-plane line — **the row under the pane's own `Versions` heading**, and one that is
/// read and never selected. The heading is matched rather than skipped, so a pane that lost it
/// fails here too instead of quietly renumbering.
fn control_plane(report: &Report) -> &str {
    match &report.rows[..] {
        [Row::Prose(heading), Row::Prose(line), ..] if heading == "Versions" => line.as_str(),
        other => panic!(
            "the first two lines of this pane are its heading and the control plane's: {other:?}"
        ),
    }
}

/// The nodes this pane flagged, in the order it drew them.
fn flagged(report: &Report) -> Vec<&str> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Answer {
                jump: Some(Jump::Object(id)),
                ..
            } => Some(id.name.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_pane_carries_its_own_versions_heading_whatever_it_could_work_out() {
    // **Two reports share the Certificates pane** (`screens/analysis.md` § *Certificates and
    // Versions*), so `views.rs` draws that pane's heading from the first report's `title` and this
    // report's title is never drawn at all — the count of reports and the count of panes are two
    // facts ([`Report`]). The literal `Versions` the screen's own variant table assigns to a
    // `Row::Prose` therefore has nowhere else to come from: the alternative is a per-report string
    // hard-coded in `views.rs`, which is exactly what [`Report::rows`] refuses for the empty
    // state, for the same reason.
    for (name, report) in [
        (
            "every kubelet matching",
            super::versions(&version_corpus(), &[]),
        ),
        (
            "a machine behind",
            super::versions(
                &kubelet_at(version_corpus(), "k8rs-worker3", "v1.32.4"),
                &[],
            ),
        ),
        (
            "no control plane version",
            super::versions(
                &ClusterSnapshot {
                    server_version: None,
                    ..version_corpus()
                },
                &[],
            ),
        ),
        (
            "no node list",
            super::versions(
                &ClusterSnapshot {
                    nodes: Vec::new(),
                    ..version_corpus()
                },
                &[],
            ),
        ),
    ] {
        println!("{}", pane(&report));
        assert_eq!(
            report.rows.first(),
            Some(&Row::Prose("Versions".to_string())),
            "{name}: the section is labelled whether or not it could answer anything"
        );
        // A heading is read and never selected, exactly as the sidebar's own group headers are.
        assert!(
            !selectable(&report).contains(&"Versions"),
            "{name}: the cursor may not land on it"
        );
        // **And the title is untouched by it** (invariant 14): the heading is a row, and a report
        // still may not name itself with the sidebar label its heading is.
        assert_eq!(
            report.title, "What version everything here is running",
            "{name}"
        );
    }
}

#[test]
fn a_cluster_whose_kubelets_all_match_says_so_and_flags_nobody() {
    let report = super::versions(&version_corpus(), &[]);
    println!("{}", pane(&report));

    assert_eq!(report.title, "What version everything here is running");
    assert_eq!(
        control_plane(&report),
        format!("Control plane {} · 4 of 4 kubelets match", server_version()),
        "the version is printed as the API server wrote it — a `v1.36.1+k3s1` re-spelled as \
         `1.36` is a number the reader cannot find in their own kubectl output"
    );
    assert!(flagged(&report).is_empty());
    // Rule 8, in this report's own words.
    assert_eq!(
        report.rows.last(),
        Some(&Row::Prose(
            "Every machine is running the same version as the control plane. Nothing to do."
                .to_string()
        ))
    );
    assert_eq!(
        report.badge, None,
        "the one badge on this pane is `certificates`', and every mockup draws `versions` bare"
    );
    assert_eq!(
        report.rows.len(),
        3,
        "the heading, the control-plane line and the closing sentence"
    );
}

#[test]
fn the_flagged_row_names_the_machine_its_kubelet_and_the_rules_own_way_out() {
    // Four behind a 1.36 control plane is 1.32 — the case N4 exists for, and one more than the
    // window allows.
    let cluster = kubelet_at(version_corpus(), "k8rs-worker3", "v1.32.4");
    let report = super::versions(&cluster, &[]);
    println!("{}", pane(&report));

    assert_eq!(flagged(&report), vec!["k8rs-worker3"]);
    let row = row_for(&report, "k8rs-worker3");
    assert_eq!(
        text_of(row),
        "k8rs-worker3 runs kubelet v1.32.4",
        "the machine and the version it runs, in one sentence and not a column"
    );
    // **The band is the pane's, and the rule's `Info` is its routing** (NOTES § D87) — the same
    // reading Capacity's node row already lands on for N5.
    assert_eq!(severity_of(row), Some(Severity::Warn));
    assert_eq!(
        detail_of(row),
        ["4 releases behind the control plane, which is further back than Kubernetes supports."]
    );

    // **N4's own sentence, not a second one written here** (NOTES § D46): the way out cites
    // upstream's window, and a row telling the reader a different number from the rule behind it
    // is the divergence this project refuses.
    let n4 = kubelet_too_far_behind(
        cluster.server_version.as_deref(),
        &cluster.nodes[index_of(&cluster, "k8rs-worker3")],
    )
    .expect("the rule flags this node too, or the report invented the row");
    assert_eq!(action_of(row), n4.action);
    assert!(
        action_of(row).contains("3 minor versions"),
        "and that sentence is where the window is named: {}",
        action_of(row)
    );

    // A jump is navigation and never reaches an operation.
    assert!(matches!(
        jump_of(row),
        Some(Jump::Object(id)) if id.kind == ObjectKind::Node && id.name == "k8rs-worker3"
    ));
    // The line above it still counts the rest.
    assert!(control_plane(&report).ends_with("3 of 4 kubelets match"));
}

#[test]
fn the_report_and_n4_never_disagree_about_a_node() {
    // **The comparison is N4's own, on N4's own numbers.** The report may not flag a node the
    // rule calls fine, or the other way round — the defect NOTES § D46 is about, asserted over
    // the whole window rather than at one point.
    for behind in 0..=6u32 {
        let cluster = kubelet_at(
            version_corpus(),
            "k8rs-worker2",
            &format!("v1.{}.0", 36 - behind),
        );
        let node = &cluster.nodes[index_of(&cluster, "k8rs-worker2")];
        let rule = kubelet_too_far_behind(cluster.server_version.as_deref(), node).is_some();
        let report = super::versions(&cluster, &[]);
        assert_eq!(
            flagged(&report).contains(&"k8rs-worker2"),
            rule,
            "{behind} versions behind: the rule says {rule} and the pane drew {:?}",
            flagged(&report)
        );
    }
}

#[test]
fn a_kubelet_inside_the_window_is_counted_but_never_flagged() {
    // **The state the old mockup got wrong** (NOTES § D81): a cluster mid-upgrade whose kubelets
    // are a release or two back is healthy, and flagging it is what made `1.31 (1) too far
    // behind` a lie against a 1.34 control plane.
    let cluster = kubelet_at(version_corpus(), "k8rs-worker", "v1.35.0");
    let report = super::versions(&cluster, &[]);
    println!("{}", pane(&report));

    assert!(flagged(&report).is_empty(), "one release back is supported");
    assert!(
        control_plane(&report).ends_with("3 of 4 kubelets match"),
        "and it is still counted as not matching, which is a fact and not a verdict: {}",
        control_plane(&report)
    );
    // **The second closing sentence, and why there are two.** *Every kubelet matches* is false
    // here, and a pane that said it over a cluster mid-upgrade would be wrong in the direction
    // that makes a reader stop looking.
    assert_eq!(
        report.rows.last(),
        Some(&Row::Prose(
            "Every machine is inside the window Kubernetes supports. Nothing to do.".to_string()
        ))
    );
}

#[test]
fn the_furthest_behind_leads_and_the_rest_follow_by_name() {
    let cluster = kubelet_at(
        kubelet_at(
            kubelet_at(version_corpus(), "k8rs-worker", "v1.30.0"),
            "k8rs-worker2",
            "v1.31.0",
        ),
        "k8rs-control-plane",
        "v1.30.0",
    );
    let report = super::versions(&cluster, &[]);
    println!("{}", pane(&report));

    assert_eq!(
        flagged(&report),
        vec!["k8rs-control-plane", "k8rs-worker", "k8rs-worker2"],
        "furthest behind first, then the node name — the machine that has to be upgraded first \
         is the one that must not be below the fold"
    );
    assert!(control_plane(&report).ends_with("1 of 4 kubelets match"));
}

#[test]
fn without_a_control_plane_version_nothing_is_compared_and_the_pane_says_so() {
    // **Comparing against a guess is the one thing this pane may not do**, which is N4's own
    // rule and the reason the report has to say it rather than fall silent.
    let report = super::versions(
        &ClusterSnapshot {
            server_version: None,
            ..version_corpus()
        },
        &[],
    );
    println!("{}", pane(&report));

    assert_eq!(
        report.rows.len(),
        2,
        "the pane's heading, and then the widest cause as the only thing under it"
    );
    let [(reason, ask_for)] = not_computed(&report)[..] else {
        panic!("one NotComputed and nothing else");
    };
    assert!(reason.starts_with("Not checked."));
    assert!(reason.contains("the version the control plane is running"));
    assert!(!ask_for.is_empty(), "and it says what to do about it");
    // Never the jargon (invariant 14).
    for line in [reason, ask_for] {
        assert!(!line.contains("403") && !line.contains("RBAC") && !line.contains("skew"));
    }
}

#[test]
fn without_a_node_list_the_control_plane_line_stays() {
    // **This report's whole difference from Capacity's empty node list**: two reads, and only
    // one of them failed (`screens/analysis.md` § *What each report needs*).
    let report = super::versions(
        &ClusterSnapshot {
            nodes: Vec::new(),
            ..version_corpus()
        },
        &[],
    );
    println!("{}", pane(&report));

    assert_eq!(
        control_plane(&report),
        format!("Control plane {}", server_version()),
        "the half that stands on its own draws alone, and never `0 of 0 kubelets match`, which \
         reads as an answer when it is the absence of one"
    );
    let [(reason, ask_for)] = not_computed(&report)[..] else {
        panic!("and the other half says it could not run");
    };
    assert!(reason.contains("the list of nodes"));
    assert_eq!(
        ask_for,
        "Ask for permission to list nodes across the whole cluster."
    );
    assert_eq!(
        report.rows.len(),
        3,
        "the heading, the half that stands on its own, and the half that could not run"
    );
}

#[test]
fn a_one_node_cluster_is_not_told_that_one_of_one_kubelets_match() {
    // kind, minikube, k3s and Docker Desktop are who this tool is for, so the single-node
    // cluster is the common case rather than the rounding one (invariant 14).
    let one = |version: &str| {
        let mut cluster = version_corpus();
        cluster.nodes.truncate(1);
        cluster.nodes[0].kubelet_version = Some(version.to_string());
        super::versions(&cluster, &[])
    };
    assert!(
        control_plane(&one(&server_version())).ends_with("· its kubelet is the same version"),
        "{}",
        control_plane(&one(&server_version()))
    );
    assert!(
        control_plane(&one("v1.35.0")).ends_with("· its kubelet is a different version"),
        "{}",
        control_plane(&one("v1.35.0"))
    );
}

#[test]
fn a_version_k8rs_cannot_read_is_not_counted_as_matching_and_is_not_flagged() {
    // **Both shapes the pipeline can produce** (NOTES § D29): a node that reports no kubelet
    // version at all, and one whose version string does not start with two numbers. N4 answers
    // nothing on either, and a pane that guessed would be inventing a distance.
    for version in [None, Some("not a version".to_string())] {
        let mut cluster = version_corpus();
        cluster.nodes[0].kubelet_version = version.clone();
        let report = super::versions(&cluster, &[]);
        assert!(
            flagged(&report).is_empty(),
            "{version:?} is not a distance from anything"
        );
        // **And it is not counted as a non-match either, which is the denominator fix**
        // (`screens/analysis.md` § *Certificates and Versions*). `3 of 4 kubelets match` beside a
        // closing sentence that says one machine could not be worked out is two claims about the
        // same node: the number calls it a non-match, the sentence says nothing is known about
        // it. The line now separates the two facts.
        assert!(
            control_plane(&report).ends_with("3 kubelets match, 1 could not be checked"),
            "{version:?}: {}",
            control_plane(&report)
        );
    }

    // **The negative, and it is what keeps the split form off every ordinary cluster**: with
    // every machine measured the line is `N of M` exactly as it was.
    let all_read = kubelet_at(version_corpus(), "k8rs-worker", "v1.35.0");
    assert!(
        control_plane(&super::versions(&all_read, &[])).ends_with("3 of 4 kubelets match"),
        "{}",
        control_plane(&super::versions(&all_read, &[]))
    );

    // **The one-node cluster gets the sentence it never had**, and it is not a count.
    let mut alone = version_corpus();
    alone.nodes.truncate(1);
    alone.nodes[0].kubelet_version = None;
    assert_eq!(
        control_plane(&super::versions(&alone, &[])),
        format!(
            "Control plane {} · its kubelet could not be checked",
            server_version()
        ),
        "*0 of 1 kubelets match* about the only machine there is, is the line a beginner reads \
         twice (invariant 14)"
    );

    // **Singular on the other side of the same line**, when one machine matches and one could
    // not be checked.
    let mut two = version_corpus();
    two.nodes.truncate(2);
    two.nodes[1].kubelet_version = Some("not a version".to_string());
    assert_eq!(
        control_plane(&super::versions(&two, &[])),
        format!(
            "Control plane {} · 1 kubelet matches, 1 could not be checked",
            server_version()
        )
    );
}

#[test]
fn a_machine_k8rs_could_not_measure_is_not_a_machine_it_called_fine() {
    // **The closing sentence is a claim, and it may not cover a machine nobody checked.** A node
    // whose kubelet version cannot be read draws no row — there is no distance to print — so the
    // only thing between the reader and *"Nothing to do"* over an unmeasured machine is what that
    // sentence says. Three shapes reach it (NOTES § D29): a node reporting no version at all, one
    // whose string does not start with two numbers, and one on another major, where the number is
    // readable and the distance is not.
    for unreadable in [
        None,
        Some("not a version".to_string()),
        Some("v2.0.0".to_string()),
    ] {
        let mut cluster = version_corpus();
        cluster.nodes[0].kubelet_version = unreadable.clone();
        let report = super::versions(&cluster, &[]);
        println!("{}", pane(&report));
        assert!(flagged(&report).is_empty(), "{unreadable:?} flags nobody");
        let Some(Row::Prose(closing)) = report.rows.last() else {
            panic!("the pane closes on a sentence when it has flagged nobody");
        };
        assert!(
            !closing.starts_with("Every machine"),
            "{unreadable:?}: one machine was not measured, so *every machine* is a claim this \
             pane cannot make: {closing}"
        );
        // **And it may not say it could not *read* what it read.** `v2.0.0` is a version string
        // k8rs parses perfectly well; what it cannot do is measure a distance across a major
        // number, which is `kubelet_too_far_behind`'s own refusal. A record that says the wrong
        // thing about why is invariant 4 in the small, and one sentence covering three shapes has
        // to be true of all three.
        assert!(
            !closing.contains("could not read"),
            "{unreadable:?}: one sentence covers all three shapes, and `v2.0.0` is read \
             perfectly well — what cannot be done with it is the comparison, so a sentence \
             naming a failed read is false about it: {closing}"
        );
        assert!(
            closing.contains("could not work out how far behind"),
            "and it says what was not measured rather than falling silent: {closing}"
        );
    }

    // The negative: with every version readable the sentence is allowed to say *every machine*,
    // or the assertion above passes on a pane that never says it.
    let inside = kubelet_at(version_corpus(), "k8rs-worker", "v1.35.0");
    let Some(Row::Prose(closing)) = super::versions(&inside, &[]).rows.last().cloned() else {
        panic!("the same shape");
    };
    assert_eq!(
        closing,
        "Every machine is inside the window Kubernetes supports. Nothing to do."
    );
}

#[test]
fn a_kubelet_ahead_of_the_control_plane_and_one_on_another_major_are_not_this_panes() {
    // Upstream forbids a kubelet ahead of its control plane outright and NOTES words N4 as
    // *behind*; a major number read across a boundary is not a distance at all. Both are the
    // rule's own `checked_sub` and `major != server_major`, and the pane may not invent a card
    // the rule set does not contain (invariant 13).
    for version in ["v1.40.0", "v2.0.0"] {
        let cluster = kubelet_at(version_corpus(), "k8rs-worker3", version);
        let report = super::versions(&cluster, &[]);
        println!("{}", pane(&report));
        assert!(flagged(&report).is_empty(), "{version} draws no row");
    }
}

#[test]
fn a_control_plane_version_k8rs_cannot_read_counts_nothing_and_claims_nothing() {
    // **`server_version: Some(…)` is not the same as *k8rs can compare against it***. It is the
    // string the API server reported, and nothing guarantees it starts with two numbers — so the
    // pane still prints it, because it is what the reader's own `kubectl version` shows, and
    // counts nothing against it. The alternative is `0 of 4 kubelets match`, which reads as an
    // answer when it is the absence of one.
    let report = super::versions(
        &ClusterSnapshot {
            server_version: Some("unknown".to_string()),
            ..version_corpus()
        },
        &[],
    );
    println!("{}", pane(&report));

    assert_eq!(control_plane(&report), "Control plane unknown");
    assert!(
        flagged(&report).is_empty(),
        "nothing is measurable against it"
    );
    let Some(Row::Prose(closing)) = report.rows.last() else {
        panic!("the pane still closes on a sentence");
    };
    assert!(
        !closing.starts_with("Every machine"),
        "and it claims nothing about any machine: {closing}"
    );
    // **The version was read — it is printed one line up.** What could not be done with it is the
    // comparison, and this is the other cause the closing sentence used to fold into *"could not
    // read the version some of these machines are running"*, which was false about both halves.
    assert!(
        !closing.contains("could not read"),
        "the line above this one prints the version, so nothing here failed to read it: {closing}"
    );
    assert!(
        closing.contains("control plane"),
        "and this cause is the control plane's, not some machine's: {closing}"
    );
}

#[test]
fn a_namespace_scope_changes_nothing_on_this_pane() {
    // **It joins no pods**, unlike Capacity and Drain safety: a node object read under a narrow
    // view is the same node object, so the answer is whole and the title claims no scope it does
    // not have.
    let cluster = kubelet_at(version_corpus(), "k8rs-worker3", "v1.32.4");
    let wide = super::versions(&cluster, &[]);
    let scoped = super::versions(
        &ClusterSnapshot {
            namespace_scope: Some("payments".to_string()),
            ..cluster
        },
        &[],
    );
    assert_eq!(wide, scoped);
    assert!(
        !scoped.title.contains("payments"),
        "nodes are cluster-scoped, and a namespace on this heading would be a claim about a \
         scope the answer does not have"
    );
}
