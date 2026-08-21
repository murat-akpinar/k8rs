//! `analysis.rs` § THE POSTURE REPORT — its tests (NOTES § D91).

use super::*;

use crate::rules::{analyze, mounted_path};

// --- POSTURE ---
//
// **The producer, against the committed corpus.** `screens/analysis.md` § Posture is the pane's
// only drawing, so what is asserted here is that screen's own sentences plus the one claim no
// screen makes: that this pane and rule 8 partition the cluster's host mounts exactly, with
// neither an overlap nor a gap.
//
// **Both sides of that partition are captured, not planted** — which is what makes it worth
// asserting. `kube-system` runs read-only mounts and writable ones on DaemonSet and mirror pods;
// `hostpath.json` and `socket.json` carry the three escalated ones. Every *other* state below is
// a plant on the decoded snapshot (NOTES § D40) and never an edit to a committed file
// (NOTES § D53): the pod moved out of `kube-system` — the clause rule 8 has no exported reader
// for — one pod mounting a directory twice, one copied across six namespaces, one that has
// finished, and one whose path normalises away.

/// **The corpus with every captured host mount in it** — Capacity's snapshot plus the three
/// single-pod captures that carry one, which is what puts both sides of the partition on one
/// cluster.
///
/// `hostpath.json` hands a container `/` narrowed to `run/containerd` and another the machine's
/// root; `socket.json` binds `/var/run/docker.sock`; `healthy-hostpath.json` mounts `/var/log`
/// read-only. The first three are rule 8's and the fourth is this pane's, and no test below is
/// told which — they are read back off the two producers.
pub(super) fn posture_corpus() -> ClusterSnapshot {
    let mut pods = captured_pods();
    pods.extend(["hostpath", "socket", "healthy-hostpath"].map(captured_pod));
    ClusterSnapshot { pods, ..corpus() }
}

/// Every (pod, mount) pair the cluster holds, over the pods rule 8 itself iterates — it skips a
/// pod that has finished, and so does this pane.
fn every_mount(cluster: &ClusterSnapshot) -> Vec<(&PodSnapshot, &HostPathMount)> {
    cluster
        .pods
        .iter()
        .filter(|pod| !finished(pod))
        .flat_map(|pod| pod.host_path_mounts.iter().map(move |mount| (pod, mount)))
        .collect()
}

/// The rows of a Posture pane, by the path each one names.
fn paths(report: &Report) -> Vec<&str> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Answer { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// The sentence under the row that names this path.
fn under<'a>(report: &'a Report, path: &str) -> &'a str {
    report
        .rows
        .iter()
        .find_map(|row| match row {
            Row::Answer { text, detail, .. } if text == path => Some(detail[0].as_str()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no row on this pane names {path}"))
}

#[test]
fn every_captured_host_mount_is_on_exactly_one_screen() {
    // **The claim this box exists to make.** Rule 8 keeps the escalated case and everything it
    // leaves is here: a mount on both screens is one pod saying two different things
    // (NOTES § D46), and a mount on neither is a hostPath k8rs never mentions at all.
    let cluster = posture_corpus();
    let report = super::posture(&cluster, &[]);
    println!("{}", pane(&report));

    let mounts = every_mount(&cluster);
    assert!(
        mounts.len() > 30,
        "walked {} mounts — the corpus stopped carrying them and every assertion below would \
         pass on nothing",
        mounts.len()
    );
    let (mine, rule_8): (Vec<_>, Vec<_>) = mounts
        .iter()
        .partition(|(pod, mount)| super::left_by_rule_8(pod, mount));
    assert!(
        !mine.is_empty() && !rule_8.is_empty(),
        "both sides have to have something in them, or *exactly one* is proven by an empty set"
    );

    // **Rule 8's side, counted off `analyze` itself** rather than off this file's idea of it.
    // The evidence line of every card it draws names the path on the node, and no other rule in
    // `rules.rs` writes that phrase.
    let cards: Vec<Finding> = analyze(&cluster)
        .into_iter()
        .filter(|f| f.evidence.contains(" on the node"))
        .collect();
    assert_eq!(
        cards.len(),
        rule_8.len(),
        "rule 8 draws one card per escalated mount, so its cards and the mounts this pane \
         refuses are the same count — a clause that drifted apart from rule 8's shows up as a \
         mount on two screens or on none.\ncards: {:?}\nrefused: {:?}",
        cards.iter().map(|f| &f.evidence).collect::<Vec<_>>(),
        rule_8
            .iter()
            .map(|(_, m)| mounted_path(m))
            .collect::<Vec<_>>()
    );

    // **And the pane draws exactly the other side**, deduplicated to one row per path.
    let expected: BTreeSet<String> = mine.iter().map(|(_, m)| mounted_path(m)).collect();
    assert_eq!(
        paths(&report)
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<String>>(),
        expected
    );

    // **Named entries on both sides**, because *extracted nothing* and *nothing to extract*
    // print the same line (CLAUDE.md § a derived list asserts it found something).
    assert!(
        paths(&report).contains(&"/var/log"),
        "`healthy-hostpath` reads the node's logs read-only, which is this pane's whole subject"
    );
    for escalated in ["/", "/run/containerd", "/var/run/docker.sock"] {
        assert!(
            !paths(&report).contains(&escalated),
            "{escalated} is rule 8's card and may not also be a row here"
        );
        assert!(
            cards
                .iter()
                .any(|f| f.evidence.contains(&format!("{escalated} on the node"))),
            "{escalated} has to be somebody's, and it is not this pane's"
        );
    }
}

#[test]
fn a_writable_mount_is_this_panes_only_when_rule_8_stays_silent_about_it() {
    // **The clause rule 8 has no exported reader for**, so it is the one this file spells a
    // second time and the one a plant has to hold honest: rule 8 says nothing about a writable
    // host mount on node infrastructure in `kube-system`, and everything it says nothing about
    // is here.
    let cluster = posture_corpus();
    let kube_proxy = cluster
        .pods
        .iter()
        .find(|pod| pod.id.name.starts_with("kube-proxy"))
        .expect("the capture runs kube-proxy, which writes to /run/xtables.lock");
    let lock = kube_proxy
        .host_path_mounts
        .iter()
        .find(|m| mounted_path(m) == "/run/xtables.lock")
        .expect("kube-proxy takes the iptables lock writable, or this proves nothing");
    assert!(!lock.read_only, "the plant's subject is a writable mount");
    assert!(
        super::left_by_rule_8(kube_proxy, lock),
        "a DaemonSet pod in kube-system is node infrastructure and rule 8 stays quiet about it"
    );

    // **The mirror-pod half of the same clause, on a captured object.** `etcd` and
    // `kube-apiserver` are mirror pods rather than DaemonSet pods, so a predicate that kept only
    // the DaemonSet half would drop their writable mounts off *both* screens — which is what
    // dropping it does: `/var/lib/etcd` and `/etc/kubernetes/pki/etcd` land in neither set.
    let etcd = cluster
        .pods
        .iter()
        .find(|pod| pod.mirror && pod.id.name.starts_with("etcd"))
        .expect("the capture runs etcd as a mirror pod");
    assert_ne!(
        etcd.owner.kind,
        ObjectKind::DaemonSet,
        "so the DaemonSet half cannot be what lets it through"
    );
    let data = etcd
        .host_path_mounts
        .iter()
        .find(|m| mounted_path(m) == "/var/lib/etcd")
        .expect("etcd writes its data to the node");
    assert!(!data.read_only && super::left_by_rule_8(etcd, data));

    // **The same mount on a pod that is not node infrastructure is rule 8's**, and the plant is
    // one field — the namespace — moved on the way in (NOTES § D40).
    let elsewhere = PodSnapshot {
        id: ObjectId {
            namespace: Some("payments".to_string()),
            ..kube_proxy.id.clone()
        },
        ..kube_proxy.clone()
    };
    assert!(
        !super::left_by_rule_8(&elsewhere, lock),
        "the same writable mount outside kube-system is an Alerts card, so this pane must not \
         also draw it"
    );
    let report = super::posture(
        &ClusterSnapshot {
            pods: vec![elsewhere.clone()],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&report));
    assert!(
        !paths(&report).contains(&"/run/xtables.lock"),
        "and the pane agrees with the predicate"
    );
    assert!(
        analyze(&ClusterSnapshot {
            pods: vec![elsewhere],
            ..corpus()
        })
        .iter()
        .any(|f| f.evidence.contains("/run/xtables.lock on the node")),
        "rule 8 is the one that draws it, or the mount fell off both screens"
    );
}

#[test]
fn one_row_per_host_path_and_the_count_is_pods_and_not_mounts() {
    let report = super::posture(&posture_corpus(), &[]);
    println!("{}", pane(&report));

    // **A DaemonSet mounting one path on every node is one line**, which is the whole shape of
    // this pane (`screens/analysis.md` § Posture). `/lib/modules` is mounted by two DaemonSets
    // across four nodes in the capture, and it is one row.
    assert_eq!(
        paths(&report)
            .iter()
            .filter(|p| **p == "/lib/modules")
            .count(),
        1,
        "one row per host path, whatever the pods"
    );
    assert!(
        under(&report, "/lib/modules").starts_with("Read-only, mounted by 8 pods"),
        "and the count is the pods behind it: {}",
        under(&report, "/lib/modules")
    );

    // **Counted per pod and not per mount.** A pod that mounts one directory into two of its
    // containers can read it once; counting the mounts would make a sidecar look like a second
    // reader.
    let twice = {
        let mut pod = captured_pod("healthy-hostpath");
        let mut second = pod.host_path_mounts[0].clone();
        second.container = "sidecar".to_string();
        pod.host_path_mounts.push(second);
        pod
    };
    assert_eq!(twice.host_path_mounts.len(), 2, "the plant is two mounts");
    let report = super::posture(
        &ClusterSnapshot {
            pods: vec![twice],
            ..corpus()
        },
        &[],
    );
    assert_eq!(paths(&report), vec!["/var/log"]);
    assert_eq!(
        under(&report, "/var/log"),
        "Read-only, mounted by 1 pod in default.",
        "two containers of one pod are one pod that can read it"
    );
}

#[test]
fn a_path_anything_can_write_to_is_not_a_read_only_path() {
    // **The writable sentence, on captured objects first.** `etcd` writes its data to the node
    // and is a mirror pod, so rule 8 stays silent and the row is this pane's — and the sentence
    // has to say so, or the pane tells a reviewer the opposite of what the manifest says.
    let cluster = posture_corpus();
    let report = super::posture(&cluster, &[]);
    assert_eq!(
        under(&report, "/var/lib/etcd"),
        "Mounted by 1 pod in kube-system, which can write to it. Kubernetes runs its own node \
         agents this way.",
        "one pod, and it is the one that can write"
    );
    assert!(
        under(&report, "/run/xtables.lock")
            .starts_with("Mounted by 8 pods in kube-system, and at least one of them can write"),
        "and the plural sentence, off the iptables lock every kube-proxy takes: {}",
        under(&report, "/run/xtables.lock")
    );

    // **Both plants are node agents, and they have to be**: a writable mount on anything else is
    // rule 8's card and never reaches this pane at all ([`super::left_by_rule_8`]). So the mixed
    // path is planted on `etcd`, whose own mount of it is already writable.
    let etcd = cluster
        .pods
        .iter()
        .find(|pod| pod.mirror && pod.id.name.starts_with("etcd"))
        .expect("the capture runs etcd as a mirror pod")
        .clone();
    let at = etcd
        .host_path_mounts
        .iter()
        .position(|m| mounted_path(m) == "/var/lib/etcd")
        .expect("etcd writes its data to the node");

    // **One writable container makes the pod's mount writable.** A path a pod reads in one
    // container and writes in another is not a read-only path, and a reviewer told *Read-only*
    // about it has been told the wrong thing.
    let mixed = {
        let mut pod = etcd.clone();
        let mut reader = pod.host_path_mounts[at].clone();
        reader.container = "reader".to_string();
        reader.read_only = true;
        pod.host_path_mounts.push(reader);
        pod
    };
    assert!(
        !mixed.host_path_mounts[at].read_only && mixed.host_path_mounts.last().unwrap().read_only,
        "the plant is one writable mount and one read-only one, on one pod"
    );
    let one_pod = super::posture(
        &ClusterSnapshot {
            pods: vec![mixed],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&one_pod));
    assert_eq!(
        under(&one_pod, "/var/lib/etcd"),
        "Mounted by 1 pod in kube-system, which can write to it. Kubernetes runs its own node \
         agents this way."
    );

    // **And one writable pod makes the path writable**, which is the fold across pods rather
    // than inside one: a directory nine pods read and one writes is not a read-only directory.
    let quiet = {
        let mut pod = etcd.clone();
        pod.id.name = "etcd-reader".to_string();
        pod.host_path_mounts[at].read_only = true;
        pod
    };
    assert!(quiet.host_path_mounts[at].read_only && !etcd.host_path_mounts[at].read_only);
    let two_pods = super::posture(
        &ClusterSnapshot {
            pods: vec![quiet, etcd],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&two_pods));
    assert_eq!(
        under(&two_pods, "/var/lib/etcd"),
        "Mounted by 2 pods in kube-system, and at least one of them can write to it. Kubernetes \
         runs its own node agents this way."
    );
}

#[test]
fn a_pod_that_mounts_one_path_both_ways_is_on_alerts_and_not_also_here() {
    // **The partition is per (pod, path) and not per mount**, which is the shape that put one pod
    // and one directory on both screens at once
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 6). Rule 8 and
    // [`super::left_by_rule_8`] are exact complements *per mount* — so a pod outside the node
    // infrastructure that mounts `/var/log` read-only in one container and writable in another
    // gets an Alerts card for the writable mount and, until this fix, a `Read-only, mounted by 1
    // pod` row here for the other one.
    //
    // **No capture holds the shape**: grouping every hostPath mount of every committed pod by
    // resolved path finds no group with two `readOnly` values, so this is a plant (NOTES § D40) —
    // one mount added to a captured pod, in a second container.
    let mut pod = captured_pod("healthy-hostpath");
    assert_eq!(pod.id.namespace.as_deref(), Some("default"));
    assert_eq!(pod.host_path_mounts.len(), 1);
    assert!(
        pod.host_path_mounts[0].read_only,
        "the captured mount is the read-only one, which is this pane's"
    );
    let mut writable = pod.host_path_mounts[0].clone();
    writable.container = "sidecar".to_string();
    writable.read_only = false;
    pod.host_path_mounts.push(writable);

    // The premise, stated rather than assumed: the two mounts are one path, and the two
    // predicates disagree about them — one card, one row, one pod.
    assert_eq!(
        mounted_path(&pod.host_path_mounts[0]),
        mounted_path(&pod.host_path_mounts[1])
    );
    assert!(super::left_by_rule_8(&pod, &pod.host_path_mounts[0]));
    assert!(!super::left_by_rule_8(&pod, &pod.host_path_mounts[1]));

    let cluster = ClusterSnapshot {
        pods: vec![pod],
        ..corpus()
    };
    let report = super::posture(&cluster, &[]);
    println!("{}", pane(&report));
    assert!(
        !paths(&report).contains(&"/var/log"),
        "the pod is answered for on Alerts, and a second sentence here calling the same directory \
         read-only is one pod saying two different things (NOTES § D46): {:?}",
        paths(&report)
    );
    assert!(
        analyze(&cluster)
            .iter()
            .any(|f| f.evidence.contains("/var/log on the node")),
        "and rule 8 is the one that draws it, or the mount fell off both screens"
    );

    // **The negative is the same pod with the second mount read-only**: nothing is escalated, and
    // the row comes back.
    let mut quiet = cluster.pods[0].clone();
    quiet.host_path_mounts[1].read_only = true;
    let report = super::posture(
        &ClusterSnapshot {
            pods: vec![quiet],
            ..corpus()
        },
        &[],
    );
    assert_eq!(paths(&report), vec!["/var/log"]);
    assert_eq!(
        under(&report, "/var/log"),
        "Read-only, mounted by 1 pod in default.",
        "two containers of one pod are still one pod that can read it"
    );

    // **And another pod mounting the same path keeps its own row** — the drop is the pod's, not
    // the path's.
    let escalating = cluster.pods[0].clone();
    let elsewhere = {
        let mut pod = captured_pod("healthy-hostpath");
        pod.id.name = "healthy-hostpath-two".to_string();
        pod.id.namespace = Some("payments".to_string());
        pod
    };
    let report = super::posture(
        &ClusterSnapshot {
            pods: vec![escalating, elsewhere],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&report));
    assert_eq!(
        under(&report, "/var/log"),
        "Read-only, mounted by 1 pod in payments.",
        "the pod on both screens is the one that is dropped, and the count says so"
    );
}

#[test]
fn a_row_names_three_namespaces_and_then_counts_the_rest() {
    // **Which namespaces can read a path is the half an operator acts on; every one of them is
    // the half that makes the pane unreadable** (`screens/analysis.md` § Posture). The plant is
    // one captured pod copied into a namespace at a time — one field moved, nothing invented.
    let one = captured_pod("healthy-hostpath");
    let spread = |namespaces: &[&str]| {
        let pods: Vec<PodSnapshot> = namespaces
            .iter()
            .map(|namespace| PodSnapshot {
                id: ObjectId {
                    namespace: Some((*namespace).to_string()),
                    ..one.id.clone()
                },
                ..one.clone()
            })
            .collect();
        let report = super::posture(&ClusterSnapshot { pods, ..corpus() }, &[]);
        under(&report, "/var/log").to_string()
    };

    assert_eq!(spread(&["one"]), "Read-only, mounted by 1 pod in one.");
    assert_eq!(
        spread(&["one", "two"]),
        "Read-only, mounted by 2 pods in one and two."
    );
    assert_eq!(
        spread(&["one", "two", "three"]),
        "Read-only, mounted by 3 pods in one, three and two.",
        "three named, in the order `kubectl get -A` prints them and not the order they arrived"
    );
    // The fourth is where the sentence stops naming and starts counting.
    assert_eq!(
        spread(&["one", "two", "three", "four"]),
        "Read-only, mounted by 4 pods in four, one, three and 1 more.",
        "three named, and what is left off is a count and never the total"
    );
    assert_eq!(
        spread(&["one", "two", "three", "four", "five", "six"]),
        "Read-only, mounted by 6 pods in five, four, one and 3 more."
    );
    // **A pod with no namespace names none**, rather than an empty `in ` hanging off the
    // sentence. It cannot exist in the API and nothing is invented for one that decoded without.
    let nowhere = PodSnapshot {
        id: ObjectId {
            namespace: None,
            ..one.id.clone()
        },
        ..one.clone()
    };
    let report = super::posture(
        &ClusterSnapshot {
            pods: vec![nowhere],
            ..corpus()
        },
        &[],
    );
    assert_eq!(under(&report, "/var/log"), "Read-only, mounted by 1 pod.");
}

#[test]
fn the_widest_path_leads_and_the_rest_follow_by_name() {
    let report = super::posture(&posture_corpus(), &[]);
    println!("{}", pane(&report));
    let drawn = paths(&report);
    let counts: Vec<(usize, &str)> = drawn
        .iter()
        .map(|path| {
            let sentence = under(&report, path);
            let n: usize = sentence
                .split_whitespace()
                .find_map(|word| word.parse().ok())
                .expect("every sentence on this pane counts pods");
            (n, *path)
        })
        .collect();
    let mut sorted = counts.clone();
    sorted.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    assert_eq!(
        counts, sorted,
        "most widely mounted first, then the path — how widely a path is exposed is the review \
         this pane is for, and the alternative puts it below the fold"
    );
    assert!(
        counts.first().is_some_and(|(n, _)| *n > 1),
        "and the corpus actually has a path more than one pod mounts, or the order proves \
         nothing: {counts:?}"
    );
}

#[test]
fn the_opening_paragraph_is_part_of_the_report_and_goes_when_there_is_no_list() {
    // **Without it the pane reads as an accusation**, and every row on it is something the
    // cluster is supposed to have (`screens/analysis.md` § Posture).
    let report = super::posture(&posture_corpus(), &[]);
    let Some(Row::Prose(opening)) = report.rows.first() else {
        panic!("the first line of this pane is read, never selected");
    };
    assert!(opening.starts_with("Nothing here is broken."));
    assert!(
        matches!(report.rows[1], Row::Answer { .. }),
        "and the list starts directly under it"
    );
    assert!(
        report
            .rows
            .iter()
            .filter(|r| matches!(r, Row::Prose(_)))
            .count()
            == 1,
        "one paragraph, not one per section"
    );

    // **The empty pane is its own sentence and drops the opening one**: *the list says who can*
    // over no list at all is a sentence about nothing.
    let bare = super::posture(
        &ClusterSnapshot {
            pods: vec![captured_pod("healthy")],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&bare));
    assert_eq!(
        bare.rows,
        vec![Row::Prose(
            "Nothing here mounts a path from the node it runs on. That is rarer than it sounds \
             — most clusters run a network or storage agent that does."
                .to_string()
        )],
        "rule 8's empty state, in this report's own words and nothing else"
    );
}

#[test]
fn a_path_that_normalises_to_nothing_draws_no_row() {
    // **`hostPath: {path: "."}`** — the one shape that reaches `mounted_path` and comes back
    // empty: it is relative, so it keeps no root, and `.` is the element the normaliser drops
    // (NOTES § D79). A row whose text is the empty string is a blank line with a sentence
    // indented under it, which reads as a defect rather than as an answer.
    let nameless = {
        let mut pod = captured_pod("healthy-hostpath");
        pod.host_path_mounts[0].path = ".".to_string();
        pod
    };
    assert_eq!(
        mounted_path(&nameless.host_path_mounts[0]),
        "",
        "the plant only proves something while this is the value that arrives"
    );
    assert!(
        super::left_by_rule_8(&nameless, &nameless.host_path_mounts[0]),
        "and rule 8 leaves it here, which is why this pane is the one that has to refuse it"
    );
    let report = super::posture(
        &ClusterSnapshot {
            pods: vec![nameless],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&report));
    assert!(paths(&report).is_empty());
    assert_eq!(
        report.rows.len(),
        1,
        "and the pane is the empty one rather than one carrying a nameless row: {:?}",
        report.rows
    );
}

#[test]
fn a_pod_that_has_finished_is_on_neither_screen() {
    // `analyze` skips the pod rules for a pod that is over, so rule 8 draws no card for one —
    // and a `Succeeded` pod is reading nothing off its node either. A pane that listed it would
    // be the only place in k8rs that says it can.
    let over = {
        let mut pod = captured_pod("healthy-hostpath");
        pod.phase = Some("Succeeded".to_string());
        pod
    };
    assert!(finished(&over));
    let cluster = ClusterSnapshot {
        pods: vec![over],
        ..corpus()
    };
    assert!(paths(&super::posture(&cluster, &[])).is_empty());
    assert!(
        !analyze(&cluster)
            .iter()
            .any(|f| f.evidence.contains(" on the node")),
        "and rule 8 says nothing about it either, which is what makes this consistent rather \
         than a hole"
    );
}

#[test]
fn it_runs_unchanged_when_the_view_is_scoped_and_the_title_says_which_namespace() {
    // **hostPath is a pod field**, so a narrower view is a shorter list and never a wrong
    // number — the same promise Waste makes (`screens/analysis.md` § *What each report needs*).
    let cluster = posture_corpus();
    let wide = super::posture(&cluster, &[]);
    let scoped = super::posture(
        &ClusterSnapshot {
            namespace_scope: Some("kube-system".to_string()),
            ..cluster
        },
        &[],
    );
    println!("{}", pane(&scoped));

    assert_eq!(
        wide.title, "Pods that can read the node's own filesystem",
        "an unscoped pane says nothing about scope"
    );
    assert_eq!(
        scoped.title, "Pods in kube-system that can read the node's own filesystem",
        "and the dangerous state is the labelled one (README rule 5)"
    );
    assert_eq!(
        paths(&wide),
        paths(&scoped),
        "the rows themselves are untouched: a scope narrows the pod list `k8s.rs` hands over, \
         never the arithmetic here — there is none"
    );
    assert!(
        scoped
            .rows
            .iter()
            .all(|row| !matches!(row, Row::NotComputed { .. })),
        "and nothing on this pane is switched off, because it needs no permission Alerts does \
         not already have"
    );
}

#[test]
fn nothing_on_this_pane_judges_opens_or_badges() {
    let report = super::posture(&posture_corpus(), &[]);
    assert_eq!(
        report.badge, None,
        "a permanent number beside `posture` would nag about a list that is correct"
    );
    for row in &report.rows {
        let Row::Answer {
            severity,
            action,
            jump,
            detail,
            ..
        } = row
        else {
            continue;
        };
        assert_eq!(
            *severity,
            Some(Severity::Info),
            "Info on every row: the pane makes no judgement, and a band that varied would be one"
        );
        assert_eq!(
            *jump, None,
            "a row stands for a set, and `Jump` has no case for one"
        );
        assert!(
            action.is_empty(),
            "there is nothing to do, which is the point"
        );
        assert_eq!(detail.len(), 1, "one paragraph, and it is the sentence");
    }
}
