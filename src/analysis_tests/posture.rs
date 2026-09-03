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
        "Read-only, mounted by 1 pod in default — outside kube-system, so k8rs cannot tell \
         what it is.",
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
        "Read-only, mounted by 1 pod in default — outside kube-system, so k8rs cannot tell \
         what it is.",
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
        "Read-only, mounted by 1 pod in payments — outside kube-system, so k8rs cannot tell \
         what it is.",
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

    // **The namespace clause is the half this test is about**, and it is unchanged by the box
    // that gave these rows their tail: `healthy-hostpath` copied into `one` runs outside
    // `kube-system`, so every row here has a pod the check does not clear and the sentence says
    // so after the list, never inside it.
    assert_eq!(
        spread(&["one"]),
        "Read-only, mounted by 1 pod in one — outside kube-system, so k8rs cannot tell what \
         it is."
    );
    assert_eq!(
        spread(&["one", "two"]),
        "Read-only, mounted by 2 pods in one and two. At least one of them is outside \
         kube-system, so k8rs cannot tell what it is."
    );
    assert_eq!(
        spread(&["one", "two", "three"]),
        "Read-only, mounted by 3 pods in one, three and two. At least one of them is outside \
         kube-system, so k8rs cannot tell what it is.",
        "three named, in the order `kubectl get -A` prints them and not the order they arrived"
    );
    // The fourth is where the sentence stops naming and starts counting.
    assert_eq!(
        spread(&["one", "two", "three", "four"]),
        "Read-only, mounted by 4 pods in four, one, three and 1 more. At least one of them is \
         outside kube-system, so k8rs cannot tell what it is.",
        "three named, and what is left off is a count and never the total"
    );
    assert_eq!(
        spread(&["one", "two", "three", "four", "five", "six"]),
        "Read-only, mounted by 6 pods in five, four, one and 3 more. At least one of them is \
         outside kube-system, so k8rs cannot tell what it is."
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
    assert_eq!(
        under(&report, "/var/log"),
        "Read-only, mounted by 1 pod — outside kube-system, so k8rs cannot tell what it is.",
        "a pod with no namespace is in no namespace, which is not `kube-system` either"
    );
}

#[test]
fn the_widest_path_leads_and_the_rest_follow_by_name() {
    // **Inside a group**, which on `kube-system-pods.json` + `nodes.json` is the whole pane:
    // every pod that mounts anything off the node there is a DaemonSet or a mirror pod, so this
    // is the order the pane has when nothing has to be lifted out of it. The group boundary
    // itself is the test below.
    let report = super::posture(&corpus(), &[]);
    println!("{}", pane(&report));
    assert!(
        paths(&report).len() > 10 && !paths(&report).contains(&"/var/log"),
        "the premise: a full pane, and every pod on it clears the check: {:?}",
        paths(&report)
    );
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
    let report = super::posture(&corpus(), &[]);
    assert!(opening(&report).starts_with("Nothing here is broken."));
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

// --- THE ROW WITH A POD OUTSIDE KUBE-SYSTEM ---
//
// **Three claims, and the corpus already holds the shape.** `healthy-hostpath` reads `/var/log`
// from one pod in `default`: nothing rule 8 escalates, nothing NOTES § D2 sends to Alerts, and —
// before this box — a row sorted last of fourteen under a paragraph saying network, storage and
// metrics agents are supposed to do this
// (`reports/2026-08-22-phase-4-close-cross-family-review.md` § 4). The plants below are the two
// shapes no capture has: a pod outside `kube-system` on a path pods inside it mount *too*, and
// one reading a path they write.
//
// **None of the sentences asserted below says what such a pod is**, because the check cannot:
// it reads a namespace and an owner kind, and Rook, Longhorn, Cilium and every CSI node plugin
// fail it while being exactly the agents this pane is otherwise full of (NOTES § D70).

/// One captured `kube-system` pod with its namespace moved (NOTES § D40) — the field rule 8's
/// silence turns on, and the only one changed.
fn moved_out_of_kube_system(name: &str, pods: &mut [PodSnapshot]) {
    let pod = pods
        .iter_mut()
        .find(|pod| pod.id.name.starts_with(name))
        .unwrap_or_else(|| panic!("the capture runs {name}"));
    assert_eq!(pod.id.namespace.as_deref(), Some("kube-system"));
    pod.id.namespace = Some("payments".to_string());
}

/// The pane's opening paragraph, which is a `Prose` row and never a selectable one.
fn opening(report: &Report) -> &str {
    match report.rows.first() {
        Some(Row::Prose(text)) => text.as_str(),
        other => panic!("the first line of this pane is read, never selected: {other:?}"),
    }
}

#[test]
fn a_row_with_a_pod_outside_kube_system_leads_the_pane() {
    // **The row an operator would act on sorts first, and pod count is what buried it**
    // (`screens/analysis.md` § Posture): a pod the check clears mounts its path on every node it
    // runs on, so one pod reading one directory loses every tie to them.
    let mixed = super::posture(&posture_corpus(), &[]);
    println!("{}", pane(&mixed));
    let drawn = paths(&mixed);
    assert_eq!(
        drawn.first(),
        Some(&"/var/log"),
        "the row with a pod outside kube-system leads, whatever the counts beside it: {drawn:?}"
    );

    // **And nothing else moved**: with that pod gone the rest of the pane is the same list in
    // the same order, so this is a lift out of one group and not a second sort key.
    let agents = super::posture(&corpus(), &[]);
    println!("{}", pane(&agents));
    assert!(
        drawn.len() > 10,
        "a full pane, or the tail below proves nothing: {drawn:?}"
    );
    assert_eq!(drawn[1..], paths(&agents)[..], "the tail is untouched");
    assert_eq!(
        paths(&agents).first(),
        Some(&"/lib/modules"),
        "and the negative: with no such row on it, the widest path leads as it always did"
    );

    // **A row leaves the group the moment one contributing pod fails the check, whatever else
    // mounts the same path** — `/lib/modules` is read by eight pods and one is enough — **and two
    // such rows sort among themselves by the key that has not changed.**
    let mut pods = posture_corpus().pods;
    moved_out_of_kube_system("kindnet", &mut pods);
    let two = super::posture(&ClusterSnapshot { pods, ..corpus() }, &[]);
    println!("{}", pane(&two));
    assert_eq!(
        paths(&two)[..2],
        ["/lib/modules", "/var/log"],
        "most widely mounted first inside the group, and both of them above every row the check \
         cleared: {:?}",
        paths(&two)
    );
}

#[test]
fn the_opening_paragraph_stops_saying_nothing_is_broken_when_a_pod_runs_outside_kube_system() {
    // **A pane that opens by saying nothing is broken while holding a row it cannot vouch for is
    // telling two stories at once** (`screens/analysis.md` § Posture). It is still not an alarm:
    // NOTES § D2 keeps a plain read-only hostPath off Alerts, and the band and the badge are
    // asserted unchanged by the sweep below this.
    //
    // **The subject is the pod, not the row.** The flag is true when any one contributor fails
    // the check, so a sentence about *the row* would be false of the pods on it that cleared.
    let agents = super::posture(&corpus(), &[]);
    assert_eq!(
        opening(&agents),
        "Nothing here is broken. Network, storage and metrics agents are supposed to do this — \
         the list says who can, not what to go and fix.",
        "every pod on this pane clears the check, so the sentence is untouched"
    );

    let mixed = super::posture(&posture_corpus(), &[]);
    println!("{}", pane(&mixed));
    assert_eq!(
        opening(&mixed),
        "Network, storage and metrics agents are supposed to do this. The top row has a pod \
         outside kube-system, so k8rs cannot tell what it is. Nothing is marked broken; it still \
         says who can, not what to go and fix.",
        "one pod on it runs outside kube-system, and its row is the one at the top"
    );
    assert_eq!(
        mixed
            .rows
            .iter()
            .filter(|row| matches!(row, Row::Prose(_)))
            .count(),
        1,
        "still one paragraph, not one per group"
    );

    // **It names no proportion on purpose**: an ordinary app namespace has no pods in
    // `kube-system` at all, so a scoped view is routinely *every* row and the sentence has to
    // stay true of that render too.
    let only = super::posture(
        &ClusterSnapshot {
            pods: vec![captured_pod("healthy-hostpath")],
            namespace_scope: Some("default".to_string()),
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&only));
    assert_eq!(opening(&only), opening(&mixed));
}

#[test]
fn a_row_with_a_pod_outside_kube_system_says_so_in_its_own_sentence() {
    // **The reorder alone is not legible** — *near the top* means nothing to a reader who does
    // not already know the sort key (`screens/analysis.md` § Posture) — so each of the three
    // shapes says what the check found, and each is asserted against the row that is the same
    // shape with every pod behind it cleared.
    //
    // **None of them says what the pod *is***, because the check cannot: a pod in
    // `longhorn-system` fails it and is a node agent (NOTES § D70).
    let mixed = super::posture(&posture_corpus(), &[]);
    let agents = super::posture(&corpus(), &[]);

    // Read-only, one pod: the committed capture's own row. The em dash is load-bearing — a
    // `which` clause would bind to `default` and call the namespace the thing outside
    // `kube-system`.
    assert_eq!(
        under(&mixed, "/var/log"),
        "Read-only, mounted by 1 pod in default — outside kube-system, so k8rs cannot tell \
         what it is."
    );

    // Read-only, several pods, one of them failing the check: eight pods mount `/lib/modules`
    // and the sentence is *at least one*, which is as much as the check knows and all it claims.
    let mut pods = posture_corpus().pods;
    moved_out_of_kube_system("kindnet", &mut pods);
    let moved = super::posture(&ClusterSnapshot { pods, ..corpus() }, &[]);
    println!("{}", pane(&moved));
    assert_eq!(
        under(&moved, "/lib/modules"),
        "Read-only, mounted by 8 pods in kube-system and payments. At least one of them is \
         outside kube-system, so k8rs cannot tell what it is."
    );
    assert_eq!(
        under(&agents, "/lib/modules"),
        "Read-only, mounted by 8 pods in kube-system.",
        "and the negative is the same eight pods with none of them moved"
    );

    // Writable, several pods, one of them a reader the check does not clear: every writer is
    // still in `kube-system`, and the sentence stops short of claiming every *pod* is.
    // The plant is the corpus plus one pod in `default` reading a path `kube-system`'s own pods
    // write — `healthy-hostpath`, with the one field that says which directory changed
    // (NOTES § D40).
    let mut reader = captured_pod("healthy-hostpath");
    assert!(
        reader.host_path_mounts[0].read_only,
        "the plant reads: a writable mount on that pod is rule 8's card and never this pane's"
    );
    reader.host_path_mounts[0].path = "/run/xtables.lock".to_string();
    let mut pods = posture_corpus().pods;
    pods.push(reader);
    let lock = super::posture(&ClusterSnapshot { pods, ..corpus() }, &[]);
    println!("{}", pane(&lock));
    assert_eq!(
        under(&lock, "/run/xtables.lock"),
        "Mounted by 9 pods in default and kube-system, and at least one of them can write to \
         it. The ones that write are in kube-system; not every pod here is."
    );
    assert_eq!(
        under(&agents, "/run/xtables.lock"),
        "Mounted by 8 pods in kube-system, and at least one of them can write to it. Kubernetes \
         runs its own node agents this way.",
        "and the negative is the lock with every pod on it cleared"
    );
    assert_eq!(
        under(&agents, "/var/lib/etcd"),
        "Mounted by 1 pod in kube-system, which can write to it. Kubernetes runs its own node \
         agents this way.",
        "the one-pod writable sentence is untouched, and the test below is why it can be"
    );
}

#[test]
fn every_pod_that_writes_to_a_path_on_this_pane_runs_in_kube_system() {
    // **A writable, one-pod row outside `kube-system` cannot be built** (`screens/analysis.md`
    // § Posture): the row's only contributor wrote, so [`super::left_by_rule_8`] let a writable
    // mount through, which it does only for a pod that clears the check — every other writable
    // mount is rule 8's card. That is the premise `host_paths` asserts, so it is measured over
    // every captured mount here rather than reasoned about.
    //
    // **The predicate is spelled out here and not called out of `analysis.rs`**: a test that
    // asked the code what the check is would agree with it by construction. This is the third
    // spelling of rule 8's clause and the only one outside product code.
    let cluster = posture_corpus();
    let writers: Vec<_> = every_mount(&cluster)
        .into_iter()
        .filter(|(pod, mount)| !mount.read_only && super::left_by_rule_8(pod, mount))
        .collect();
    assert!(
        writers.len() > 5,
        "walked {} writable mounts this pane keeps — the corpus stopped carrying them and this \
         would pass on nothing",
        writers.len()
    );
    for (pod, mount) in &writers {
        assert!(
            pod.id.namespace.as_deref() == Some("kube-system")
                && (pod.mirror || pod.owner.kind == ObjectKind::DaemonSet),
            "{} writes {} and does not clear the check, so this pane must not have been the \
             screen it landed on",
            pod.id.name,
            mounted_path(mount)
        );
    }

    // **The negative, on the one field that decides it**: the same writable mount on a pod that
    // is not one of them never reaches this pane at all.
    let mut pods = cluster.pods.clone();
    moved_out_of_kube_system("kube-proxy", &mut pods);
    let moved = pods
        .iter()
        .find(|pod| pod.id.namespace.as_deref() == Some("payments"))
        .expect("the plant is the moved pod");
    let lock = moved
        .host_path_mounts
        .iter()
        .find(|mount| mounted_path(mount) == "/run/xtables.lock")
        .expect("kube-proxy takes the iptables lock writable");
    assert!(!lock.read_only && !super::left_by_rule_8(moved, lock));
}

#[test]
fn a_row_does_not_change_group_because_some_other_pods_mount_went_to_alerts() {
    // **A pod rule 8 escalated contributes nothing to this row at all** ([`super::Mounters`]),
    // and the group is one of the things it contributes nothing to: a pod in `default` *writing*
    // `/lib/modules` is an Alerts card, and the row the node's own agents read is still theirs.
    // The alternative is a pane whose opening paragraph is rewritten by a pod that already has a
    // card on another screen — one object saying two different things (NOTES § D46).
    //
    // **The negative is the same path with the same outside namespace and one bit different**:
    // `a_row_the_nodes_own_agents_did_not_mount_alone_leads_the_pane` moves `kindnet` out of
    // `kube-system` and `/lib/modules` *does* leave the group, because that pod is on no other
    // screen. The bit is `read_only`, and it is the whole difference.
    //
    // **The plant is `healthy-hostpath` with two fields moved** (NOTES § D40): the directory it
    // mounts, and the bit that decides which screen it lands on.
    let mut escalating = captured_pod("healthy-hostpath");
    assert_eq!(escalating.id.namespace.as_deref(), Some("default"));
    escalating.host_path_mounts[0].path = "/lib/modules".to_string();
    escalating.host_path_mounts[0].read_only = false;

    let mut pods = corpus().pods;
    pods.push(escalating);
    let cluster = ClusterSnapshot { pods, ..corpus() };
    assert!(
        analyze(&cluster)
            .iter()
            .any(|f| f.evidence.contains("/lib/modules on the node")),
        "the premise: rule 8 draws the card, so the pod is answered for somewhere else"
    );

    let report = super::posture(&cluster, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        under(&report, "/lib/modules"),
        "Read-only, mounted by 8 pods in kube-system.",
        "the eight node agents that read it are still the whole of the row"
    );
    assert!(
        opening(&report).starts_with("Nothing here is broken."),
        "and the pane still opens the way a pane of node agents opens: {}",
        opening(&report)
    );
}

#[test]
fn the_writable_row_no_producer_can_build_still_has_a_sentence_that_is_true() {
    // **The one claim on this pane that is not read off a producer, and it says why.** A row that
    // is writable, single-pod and outside `kube-system` cannot be built — the test above measures
    // that over every captured mount, and [`super::host_paths`] asserts it — so
    // [`super::Mounters::sentence`] is called directly here, which is the only way to read the
    // arm at all.
    //
    // **It is worth an arm because the reason it is unreachable is one NOTES § D70 already calls
    // too narrow.** Widen the `kube-system` clause and this row becomes buildable; an arm that
    // fell through to the single-pod writable sentence above it would then tell a reader that a
    // pod in `longhorn-system` is one of the node's own agents, in a release build where the
    // assertion is compiled out.
    let row = |outside| {
        super::Mounters {
            pods: 1,
            namespaces: BTreeSet::from(["longhorn-system".to_string()]),
            writable: true,
            outside_kube_system: outside,
        }
        .sentence()
    };
    assert_eq!(
        row(true),
        "Mounted by 1 pod in longhorn-system, which can write to it. That pod is outside \
         kube-system, so k8rs cannot tell what it is.",
        "it says where the pod runs and then stops, because that is all the check knows"
    );
    // **The negative is the same row with the check cleared**, which is the sentence this pane
    // draws today and the one that would be wrong above.
    assert_eq!(
        row(false),
        "Mounted by 1 pod in longhorn-system, which can write to it. Kubernetes runs its own \
         node agents this way."
    );
}
