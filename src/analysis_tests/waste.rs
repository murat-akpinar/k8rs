//! `analysis.rs` § THE WASTE REPORT — its tests (NOTES § D91).

use super::*;

// --- WASTE ---
//
// **The producer, against the committed corpus** — the two hand-built panes that stood here until
// this box landed are gone. The corpus holds one of each row this report draws: a Service with a
// selector and nothing behind it, a Service with no selector at all, a `Bound` claim nobody
// mounts beside one a pod does, and three pods that are over — the two that finished on their
// own and the one a node removed. **The ReplicaSet parked at
// zero is the one row no capture holds** — the trip's cluster has never had one — so it is a
// plant (NOTES § D40), and what a trip would have to do to replace it is on the test.

/// **The corpus with the four lists Waste fetches**, plus the three pods that are over and the
/// one pod that mounts a claim. `captured_pods` holds nothing terminal and nothing mounting a
/// disk, so without these four the report's middle rows have neither a positive nor a negative.
///
/// **`evicted` and `failed` are the split's two directions on one phase** — both `Failed`, both
/// [`finished`], and only the first carries `status.reason: Evicted` — so the pane this corpus
/// draws is the negative for each row as much as the positive for the other (NOTES § D155).
pub(super) fn waste_corpus() -> ClusterSnapshot {
    let mut pods = captured_pods();
    pods.extend(["succeeded", "failed", "evicted", "healthy-disk"].map(captured_pod));
    ClusterSnapshot {
        services: Some(captured_services()),
        endpoint_slices: Some(captured_slices()),
        claims: Some(captured_claims()),
        replica_sets: Some(captured_replica_sets()),
        ..snapshot(pods, captured_nodes())
    }
}

#[test]
fn the_service_matching_no_pod_leads_and_every_row_is_an_answer() {
    let report = super::waste(&waste_corpus(), &[]);
    println!("{}", pane(&report));

    assert_eq!(
        selectable(&report),
        vec![
            "default/broken-noendpoints matches no pod",
            "default/broken-unused-disk is 128Mi nobody is using",
            "1 pod was removed by a node and remains",
            "2 pods finished and were never removed",
        ],
        "the 503 nobody can explain first, then the disk, then the two pileups — the removed row \
         ahead of the completed one because it names a cause and carries the action, not because \
         it is louder (`screens/analysis.md` § *The pileup splits in two, one per cause*)"
    );
    assert_eq!(
        report.rows.len(),
        4,
        "and there is no `Prose` on this pane at all — every line is a row"
    );
    assert_eq!(
        report.rows.iter().map(severity_of).collect::<Vec<_>>(),
        vec![
            Some(Severity::Critical),
            Some(Severity::Warn),
            Some(Severity::Info),
            Some(Severity::Info)
        ],
        "the 503, the disk nobody mounts, and then two `Info` pileups: what a removed pod costs \
         today is an etcd entry and a longer pod list, exactly the completed row's own cost, and \
         a `Warn` that clears only by deleting its own evidence is what stops an alert screen \
         being believed (`screens/analysis.md` § *The pileup splits in two, one per cause*)"
    );
    assert_eq!(
        report.title, "Things that cost you something for nothing",
        "unscoped, the title says nothing about scope (`screens/analysis.md` rule 6)"
    );
    assert_eq!(report.badge, None);

    // **The per-object rows record a destination and the counted row does not** — the Service and
    // the disk are one object each; `2 pods` stands for a set, and [`Jump`] has no case for one
    // (NOTES § D128).
    let Some(Jump::Object(service)) = jump_of(&report.rows[0]) else {
        panic!("the Service row jumps to the Service, and no finding names it");
    };
    assert_eq!(
        service.kind,
        ObjectKind::Other("Service".to_string()),
        "a core-group kind is unqualified (NOTES § D36)"
    );
    assert_eq!(service.namespace.as_deref(), Some("default"));
    assert_eq!(service.name, "broken-noendpoints");
    assert!(matches!(jump_of(&report.rows[1]), Some(Jump::Object(id))
            if id.kind == ObjectKind::Other("PersistentVolumeClaim".to_string())));
    assert_eq!(jump_of(&report.rows[2]), None);
    assert_eq!(jump_of(&report.rows[3]), None);

    assert_eq!(
        detail_of(&report.rows[0]),
        ["This Service points at nothing. Anything calling it gets a 503."],
        "the explanation is what makes this row readable at 3am"
    );
    assert_eq!(action_of(&report.rows[0]), "fix its selector, or delete it");
    assert_eq!(
        report
            .rows
            .iter()
            .filter(|row| !action_of(row).is_empty())
            .count(),
        2,
        "the Service and the pods a node removed. The disk has no way out — deleting a claim \
         deletes what is on it, and this report does not know whether that matters — and neither \
         has a pod that ran to completion on its own"
    );
}

#[test]
fn a_service_with_no_selector_is_not_a_service_matching_no_pod() {
    // `kubernetes` in `default` has its endpoints written by the API server itself and carries no
    // selector — one exists on every cluster ever built, so a report that flagged it would open
    // with a false row on every single pane it ever draws.
    let services = captured_services();
    let empty: Vec<&str> = services
        .iter()
        .filter(|s| s.selector.is_empty())
        .map(|s| s.id.name.as_str())
        .collect();
    assert_eq!(
        empty,
        ["kubernetes"],
        "one hand-managed Service in the capture, or this test has no subject"
    );

    // **And it is skipped for having no selector, not for having endpoints.** Its slice is
    // emptied on the way in, which is the shape a hand-managed Service in trouble actually has.
    let orphaned = captured_item_but::<EndpointSlice, EndpointSliceSnapshot>(
        "endpointslices",
        "kubernetes",
        |slice| slice.endpoints = Some(Vec::new()),
    );
    assert_eq!(orphaned.endpoints, 0, "the plant is an empty slice");
    let cluster = ClusterSnapshot {
        endpoint_slices: Some(
            captured_slices()
                .into_iter()
                .filter(|s| s.id.name != "kubernetes")
                .chain([orphaned])
                .collect(),
        ),
        ..waste_corpus()
    };
    let report = super::waste(&cluster, &[]);
    println!("{}", pane(&report));
    assert!(
        !selectable(&report)
            .iter()
            .any(|text| text.contains("kubernetes")),
        "*matches no pod* is not a thing to say about a Service whose endpoints are managed by \
         hand ([`crate::rules::ServiceSnapshot::selector`])"
    );

    // The negative, one field apart: give the same Service a selector and it is flagged, so the
    // assertion above is about the selector and not about a Service the report never sees.
    let with_selector =
        captured_item_but::<Service, ServiceSnapshot>("services", "kubernetes", |service| {
            service.spec.get_or_insert_with(Default::default).selector =
                Some(labels(&[("app", "whatever")]));
        });
    let cluster = ClusterSnapshot {
        services: Some(
            captured_services()
                .into_iter()
                .filter(|s| s.id.name != "kubernetes")
                .chain([with_selector])
                .collect(),
        ),
        ..cluster
    };
    assert!(
        selectable(&super::waste(&cluster, &[]))
            .iter()
            .any(|text| text == &"default/kubernetes matches no pod")
    );
}

#[test]
fn a_service_with_something_behind_it_is_not_flagged_ready_or_not() {
    // `broken-sts` has two endpoints and `kube-dns` two, so three of the four Services in the
    // capture are silent — which is what keeps the one row this pane opens with worth reading.
    let report = super::waste(&waste_corpus(), &[]);
    assert_eq!(
        selectable(&report)
            .iter()
            .filter(|text| text.contains("matches no pod"))
            .count(),
        1
    );

    // **Ready or not**: a pod failing its readiness probe is Alerts' rule 7 and is already on the
    // other screen; counting it as *nothing* here would put one pod on two screens saying two
    // different things ([`crate::rules::EndpointSliceSnapshot::endpoints`]). Emptying
    // `broken-sts`'s slice is the fact that moves the row, and nothing else does.
    let emptied = captured_item_but::<EndpointSlice, EndpointSliceSnapshot>(
        "endpointslices",
        "broken-sts-jt74f",
        |slice| slice.endpoints = Some(Vec::new()),
    );
    let cluster = ClusterSnapshot {
        endpoint_slices: Some(
            captured_slices()
                .into_iter()
                .filter(|s| s.id.name != "broken-sts-jt74f")
                .chain([emptied])
                .collect(),
        ),
        ..waste_corpus()
    };
    let report = super::waste(&cluster, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        selectable(&report)
            .iter()
            .filter(|text| text.contains("matches no pod"))
            .count(),
        2,
        "and a Service whose slice went empty joins the row"
    );

    // A Service with no slice at all reaches the same answer by the other door: nothing is behind
    // it, which is what the row asks.
    let no_slices = ClusterSnapshot {
        endpoint_slices: Some(Vec::new()),
        ..waste_corpus()
    };
    assert_eq!(
        selectable(&super::waste(&no_slices, &[]))
            .iter()
            .filter(|text| text.contains("matches no pod"))
            .count(),
        3,
        "the three Services that carry a selector, and not the fourth"
    );
}

/// **One more slice for `broken-sts`, off its own committed one** — the D40 plant (NOTES § D40),
/// emptied or kept as the case needs and renamed so the two are two objects.
fn sts_slice(name: &str, keep_its_endpoints: bool) -> EndpointSliceSnapshot {
    captured_item_but(
        "endpointslices",
        "broken-sts-jt74f",
        |slice: &mut EndpointSlice| {
            slice.metadata.name = Some(name.to_string());
            if !keep_its_endpoints {
                slice.endpoints = Some(Vec::new());
            }
        },
    )
}

#[test]
fn a_service_behind_two_endpointslices_is_counted_across_both_of_them() {
    // **The shape the corpus cannot hold.** Every captured Service has exactly one slice, so the
    // `.sum()` this row is built on has never been asked to add anything: an implementation that
    // answered off the *first* matching slice passes every other test in this file. A dual-stack
    // Service has two — one per IP family, and one of them may hold nothing — and any Service past
    // 100 endpoints has several. The wrong answer is *matches no pod* about a Service that works,
    // which this section's own doc calls the loudest this pane can give.
    //
    // **What a trip would have to do to capture it instead**: build the kind cluster with both IP
    // families and give `broken-sts`'s Service `ipFamilyPolicy: RequireDualStack`, or scale a
    // workload past the 100-endpoint slice limit. The break sequence does neither, so the second
    // slice is planted rather than photographed.
    //
    // **The empty slice is first on purpose**, because that is the order in which a `find` reads
    // as a `sum`.
    for (name, second_slice_full, matches_nothing) in [
        ("one slice empty, the other behind two pods", true, false),
        ("both slices empty", false, true),
    ] {
        let mut slices = vec![
            sts_slice("broken-sts-v6", false),
            sts_slice("broken-sts-jt74f", second_slice_full),
        ];
        slices.extend(
            captured_slices()
                .into_iter()
                .filter(|slice| slice.service.as_deref() != Some("broken-sts")),
        );
        let cluster = ClusterSnapshot {
            endpoint_slices: Some(slices),
            ..waste_corpus()
        };
        let report = super::waste(&cluster, &[]);
        println!("{name}\n{}", pane(&report));
        assert_eq!(
            selectable(&report)
                .iter()
                .any(|text| text.starts_with("default/broken-sts ")),
            matches_nothing,
            "{name}: {:?}",
            selectable(&report)
        );
    }
}

#[test]
fn one_name_in_two_namespaces_is_two_services_and_neither_answers_for_the_other() {
    // **The shape a join keyed on the name alone gets wrong, and the corpus cannot hold it**: no
    // Service in the capture repeats another's name in a second namespace, so a lookup that
    // dropped the namespace passes every other test in this file. On a real cluster it is the
    // ordinary case — `web` exists in `staging` and in `payments` — and the wrong answer runs both
    // ways: the one with nothing behind it goes silent because the other's endpoints were counted
    // for it, which is a 503 this pane was built to name and now does not.
    //
    // `default/broken-sts` is the captured Service with two endpoints behind it; the copy planted
    // in `kube-system` differs from it in its namespace and nothing else, and no slice in
    // `kube-system` names it (NOTES § D40).
    let elsewhere: ServiceSnapshot =
        captured_item_but("services", "broken-sts", |service: &mut Service| {
            service.metadata.namespace = Some("kube-system".to_string());
        });
    let cluster = ClusterSnapshot {
        services: Some(captured_services().into_iter().chain([elsewhere]).collect()),
        ..waste_corpus()
    };
    let report = super::waste(&cluster, &[]);
    println!("{}", pane(&report));
    let rows = selectable(&report);
    assert!(
        rows.contains(&"kube-system/broken-sts matches no pod"),
        "the copy with no slice in its own namespace is the one that matches no pod: {rows:?}"
    );
    assert!(
        !rows
            .iter()
            .any(|row| row.starts_with("default/broken-sts ")),
        "and the captured one, which has two endpoints behind it, is not: {rows:?}"
    );
}

#[test]
fn a_slice_that_names_no_service_answers_for_none_however_it_is_named() {
    // **The other half of the key, and the shape the corpus cannot hold either.** A slice belongs
    // to a Service by the `kubernetes.io/service-name` label and by nothing else; a slice carrying
    // no such label is hand-managed and says nothing about any Service
    // ([`crate::rules::EndpointSliceSnapshot::service`]). The controller names its slices
    // `<service>-<hash>`, so in every capture the label and the object's own name agree — and a
    // join that read the *name* passes every other test in this file. Nothing makes a
    // hand-managed slice keep that convention, so the plant is a slice named exactly
    // `broken-sts` with the label taken off (NOTES § D40).
    //
    // **The empty label is the third shape and it is a different one.** The API server accepts
    // `kubernetes.io/service-name: ""`, and the decode keeps a present-but-empty label as
    // `Some("")` rather than `None` (`rules.rs` § the snapshot types), so it reaches the join by
    // the other door. It is fed here because the pipeline can hand it over
    // ([D29](NOTES.md#d29)), not because a branch guards it: this arm pins a shape and expects
    // the same answer as the arm above it.
    let planted = |label: Option<&str>| {
        captured_item_but::<EndpointSlice, EndpointSliceSnapshot>(
            "endpointslices",
            "broken-sts-jt74f",
            |slice| {
                slice.metadata.name = Some("broken-sts".to_string());
                if let Some(labels) = slice.metadata.labels.as_mut() {
                    match label {
                        Some(value) => labels
                            .insert("kubernetes.io/service-name".to_string(), value.to_string()),
                        None => labels.remove("kubernetes.io/service-name"),
                    };
                }
            },
        )
    };
    // **One field apart**, so what moves the row is the label and not the rename.
    for (name, label, matches_nothing) in [
        ("the label as captured", Some("broken-sts"), false),
        ("the label taken off", None, true),
        ("the label present and empty", Some(""), true),
    ] {
        let cluster = ClusterSnapshot {
            endpoint_slices: Some(
                captured_slices()
                    .into_iter()
                    .filter(|slice| slice.id.name != "broken-sts-jt74f")
                    .chain([planted(label)])
                    .collect(),
            ),
            ..waste_corpus()
        };
        let report = super::waste(&cluster, &[]);
        println!("{name}\n{}", pane(&report));
        assert_eq!(
            selectable(&report)
                .iter()
                .any(|row| row.starts_with("default/broken-sts ")),
            matches_nothing,
            "{name} — the two endpoints are behind `broken-sts` only while the slice says so: {:?}",
            selectable(&report)
        );
    }
}

#[test]
fn services_present_with_the_slices_missing_is_not_every_service_matching_nothing() {
    // **Two fields and one row, so both must be `Some`**
    // ([`crate::rules::ClusterSnapshot::endpoint_slices`]). Reading a missing slice list as *no
    // endpoints* is the loudest possible wrong answer this pane could give: every Service on the
    // cluster, opened with a red row saying it is unreachable.
    for (name, cluster) in [
        (
            "slices missing",
            ClusterSnapshot {
                endpoint_slices: None,
                ..waste_corpus()
            },
        ),
        (
            "services missing",
            ClusterSnapshot {
                services: None,
                ..waste_corpus()
            },
        ),
    ] {
        let report = super::waste(&cluster, &[]);
        println!("{name}\n{}", pane(&report));
        assert!(
            !selectable(&report)
                .iter()
                .any(|text| text.contains("matches no pod")),
            "{name}: not one Service is called broken"
        );
        let [(reason, ask_for)] = not_computed(&report)[..] else {
            panic!("{name}: the section says it did not run, and says it once");
        };
        assert!(
            reason.contains("not checked"),
            "{name}: it names the check that is off: {reason}"
        );
        assert!(
            ask_for.contains("services") && ask_for.contains("endpointslices"),
            "{name}: and both halves of the way out, because the reader cannot tell which of the \
             two failed: {ask_for}"
        );
    }

    // And the rest of the pane keeps its true answers — a report that still works must not be
    // made to look broken (`screens/analysis.md` § *What each report needs*).
    let report = super::waste(
        &ClusterSnapshot {
            endpoint_slices: None,
            ..waste_corpus()
        },
        &[],
    );
    assert_eq!(
        selectable(&report),
        vec![
            "default/broken-unused-disk is 128Mi nobody is using",
            "1 pod was removed by a node and remains",
            "2 pods finished and were never removed",
        ]
    );
}

#[test]
fn nothing_to_find_is_an_answer_and_nobody_looked_is_a_row() {
    // The distinction the six `Option`s on the snapshot exist for: `Some(vec![])` is *every
    // Service reaches a pod*, `None` is *nobody looked*, and drawing the first for the second
    // teaches a reader their cluster is clean when it is unread.
    let looked = ClusterSnapshot {
        services: Some(Vec::new()),
        endpoint_slices: Some(Vec::new()),
        claims: Some(Vec::new()),
        replica_sets: Some(Vec::new()),
        pods: Vec::new(),
        ..waste_corpus()
    };
    let report = super::waste(&looked, &[]);
    println!("{}", pane(&report));
    assert!(
        not_computed(&report).is_empty(),
        "four empty lists are four answers, not four excuses"
    );

    // **A pane of nothing but excuses is one excuse.** Rule 7's letter is *one `NotComputed` per
    // section* and three sections obey it; its stated reason is that two ways out over an empty
    // space is two for a reader who can only take one, and that is what three of them stacked over
    // nothing are. This is one ordinary RBAC shape — a namespaced role with none of the three
    // cluster verbs — and not a corner. Drain safety's whole pane is one row for the same reason.
    let report = super::waste(&nothing_could_be_read(), &[]);
    println!("{}", pane(&report));
    let [(reason, ask_for)] = not_computed(&report)[..] else {
        panic!("nothing answered, so the pane is one row: {report:?}");
    };
    assert_eq!(report.rows.len(), 1, "and that row is the whole pane");
    assert!(
        selectable(&report).is_empty() && !report.rows.iter().any(|r| matches!(r, Row::Prose(_))),
        "and a pane that could not look does not also say there is nothing to find"
    );
    // **One way out, and it lists everything the report needs** — the half a reader can act on,
    // and the reason folding three rows into one may not lose anything.
    for resource in [
        "services",
        "endpointslices",
        "persistentvolumeclaims",
        "replicasets",
    ] {
        assert!(
            ask_for.contains(resource),
            "{resource} is not in the one way out this pane offers: {ask_for}"
        );
    }
    assert!(
        !reason.contains("403") && !reason.contains("RBAC"),
        "it names the check in plain language, never the status code: {reason}"
    );

    // **When some section still answers, the per-section rows stay** — that half of rule 7 was
    // right, and the precedent is Capacity's `Still counted, from what you can see:`. The pods are
    // a `Vec` that is always read, so one pod that finished is a section that answered.
    let one_answer = ClusterSnapshot {
        pods: vec![captured_pod("succeeded")],
        ..nothing_could_be_read()
    };
    let report = super::waste(&one_answer, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        not_computed(&report).len(),
        3,
        "one line per section that could not run — the Services and their slices are one row \
         between them"
    );
    assert_eq!(
        selectable(&report).len(),
        1,
        "the section that answered keeps its row"
    );
    let asks: BTreeSet<&str> = not_computed(&report)
        .iter()
        .map(|(_, ask_for)| *ask_for)
        .collect();
    assert_eq!(
        asks.len(),
        3,
        "no two sections ask for the same thing, or one of them names a permission that is not \
         the one it needs: {asks:?}"
    );

    // **And a section that ran and found nothing is a section that answered**, even though it
    // draws no row: the fold's sentence names all four lists, and here one of them was read. Two
    // rows is what this pane says, and the third is not invented to make it one.
    let two_unread = ClusterSnapshot {
        replica_sets: Some(Vec::new()),
        ..nothing_could_be_read()
    };
    let report = super::waste(&two_unread, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        not_computed(&report).len(),
        2,
        "the two that could not be read, and nothing claimed about the one that could"
    );
}

/// **Every list this report fetches unread, and nothing that finished** — the namespaced role
/// with none of the three cluster verbs, which is the shape that used to stack three excuses over
/// an empty pane.
pub(super) fn nothing_could_be_read() -> ClusterSnapshot {
    ClusterSnapshot {
        services: None,
        endpoint_slices: None,
        claims: None,
        replica_sets: None,
        pods: Vec::new(),
        ..waste_corpus()
    }
}

#[test]
fn the_disk_row_is_about_a_bound_claim_nothing_mounts() {
    let report = super::waste(&waste_corpus(), &[]);
    let row = row_for(&report, "default/broken-unused-disk");
    assert_eq!(
        text_of(row),
        "default/broken-unused-disk is 128Mi nobody is using",
        "the size is `status.capacity.storage` — what was provisioned, not what was asked for — \
         and it is spelled by the same `bytes` the Capacity rows use"
    );
    // **The StatefulSet caveat is on every row of this kind, and it is the row's own paragraph**
    // — `statefulset.spec.persistentVolumeClaimRetentionPolicy.whenScaled` defaults to `Retain`,
    // so a StatefulSet scaled down for the weekend, or caught mid rolling-update, puts its pods'
    // own database volumes here. Deleting one is the classic irrecoverable Kubernetes mistake,
    // and *technically true, operationally a trap* is not a sentence this report may leave
    // standing (NOTES § D134, `screens/analysis.md` § Waste).
    assert_eq!(
        detail_of(row),
        [
            "A disk was reserved for it and no pod is mounting it. It stays reserved until \
             somebody deletes it. A StatefulSet keeps its pods' disks by default, even after it \
             is scaled down, so some of this is normal."
        ]
    );
    assert_eq!(
        severity_of(row),
        Some(Severity::Warn),
        "and it stays `Warn` — nothing about a claim k8rs can see tells it whether the \
         StatefulSet that made this one is still around, so an idle disk with a real cost is \
         still worth a look; the caveat only stops the sentence pushing a reader at the delete key"
    );
    assert!(
        !selectable(&report)
            .iter()
            .any(|text| text.contains("healthy-disk")),
        "and the claim the `healthy-disk` pod mounts is silent, which is what makes the row above \
         a claim about mounting rather than about existing"
    );

    // **Any pod naming it counts, finished ones included.** A `Succeeded` Job pod is not using
    // the disk this second, but *"nobody is using it"* about a disk a CronJob mounts hourly is a
    // row that gets a volume deleted.
    let finished_mount = PodSnapshot {
        phase: Some("Succeeded".to_string()),
        ..captured_pod("healthy-disk")
    };
    let cluster = ClusterSnapshot {
        pods: vec![finished_mount],
        ..waste_corpus()
    };
    assert!(
        !selectable(&super::waste(&cluster, &[]))
            .iter()
            .any(|text| text.contains("healthy-disk")),
        "a pod that finished still says something mounts this disk"
    );

    // **`Bound` only.** A `Pending` claim has reserved no disk yet, so billing the reader for it
    // is a number this report may not print.
    let pending = captured_item_but::<PersistentVolumeClaim, ClaimSnapshot>(
        "persistentvolumeclaims",
        "broken-unused-disk",
        |claim| {
            claim.status.get_or_insert_with(Default::default).phase = Some("Pending".to_string());
        },
    );
    let cluster = ClusterSnapshot {
        claims: Some(vec![pending]),
        ..waste_corpus()
    };
    let report = super::waste(&cluster, &[]);
    println!("{}", pane(&report));
    assert!(
        !selectable(&report)
            .iter()
            .any(|text| text.contains("broken-unused-disk")),
        "the phase is what keeps the report from billing the reader for storage that was never \
         provisioned"
    );

    // A size k8rs cannot read costs the row its number and not its row.
    let unreadable = captured_item_but::<PersistentVolumeClaim, ClaimSnapshot>(
        "persistentvolumeclaims",
        "broken-unused-disk",
        |claim| {
            claim
                .status
                .get_or_insert_with(Default::default)
                .capacity
                .get_or_insert_with(Default::default)
                .insert(
                    "storage".to_string(),
                    k8s_openapi::apimachinery::pkg::api::resource::Quantity(
                        "one hundred and twenty eight".to_string(),
                    ),
                );
        },
    );
    let cluster = ClusterSnapshot {
        claims: Some(vec![unreadable]),
        ..waste_corpus()
    };
    assert_eq!(
        text_of(row_for(
            &super::waste(&cluster, &[]),
            "default/broken-unused-disk"
        )),
        "default/broken-unused-disk is reserved and nobody is using it"
    );
}

#[test]
fn a_disk_a_generic_ephemeral_volume_stands_up_is_not_a_disk_nobody_is_using() {
    // **The row that called a running pod's disk unused.** `spec.volumes[].ephemeral` is a
    // sibling of `persistentVolumeClaim` on the same entry, and the API server creates a claim
    // named `<pod name>-<volume name>` for it — `Bound`, mounted by a running pod, and named by
    // no `claimName` anywhere (`kubectl explain pod.spec.volumes.ephemeral.volumeClaimTemplate`,
    // `reports/2026-08-21-family-c-analysis-report-family-review.md` § 4). Read from the pod
    // side, which is the side that knows both halves ([`crate::rules::PodSnapshot::claims`]).
    //
    // Both halves are plants (NOTES § D40): no pod on the fixture cluster declares an ephemeral
    // volume, so the claim it would stand up is not in the capture either — it is the captured
    // orphan claim under the name the API server would have given it.
    let pod = captured_pod_but("healthy-disk", |pod| {
        pod.spec
            .as_mut()
            .expect("the capture has a spec")
            .volumes
            .get_or_insert_with(Vec::new)
            .push(k8s_openapi::api::core::v1::Volume {
                name: "scratch".to_string(),
                ephemeral: Some(k8s_openapi::api::core::v1::EphemeralVolumeSource {
                    volume_claim_template: Some(Default::default()),
                }),
                ..Default::default()
            });
    });
    assert_eq!(
        pod.claims,
        [
            "healthy-disk".to_string(),
            "healthy-disk-scratch".to_string()
        ],
        "the plant is one volume, and the claim it stands up is named by the pod and the volume"
    );
    let stood_up: ClaimSnapshot = captured_item_but::<PersistentVolumeClaim, ClaimSnapshot>(
        "persistentvolumeclaims",
        "broken-unused-disk",
        |claim| claim.metadata.name = Some("healthy-disk-scratch".to_string()),
    );
    assert_eq!(stood_up.phase.as_deref(), Some("Bound"));

    let cluster = ClusterSnapshot {
        pods: vec![pod],
        claims: Some(vec![stood_up.clone()]),
        ..waste_corpus()
    };
    let report = super::waste(&cluster, &[]);
    println!("{}", pane(&report));
    assert!(
        !selectable(&report)
            .iter()
            .any(|text| text.contains("healthy-disk-scratch")),
        "a running pod has it open, and *nobody is using it* about a disk in use is the row that \
         gets a volume deleted: {:?}",
        selectable(&report)
    );

    // **The negative, one field apart**: the same claim under any other name is named by nothing
    // in the snapshot and is exactly what this row is for.
    let renamed = captured_item_but::<PersistentVolumeClaim, ClaimSnapshot>(
        "persistentvolumeclaims",
        "broken-unused-disk",
        |claim| claim.metadata.name = Some("healthy-disk-scratchpad".to_string()),
    );
    let report = super::waste(
        &ClusterSnapshot {
            claims: Some(vec![renamed]),
            ..cluster
        },
        &[],
    );
    assert!(
        selectable(&report)
            .iter()
            .any(|text| text.starts_with("default/healthy-disk-scratchpad is 128Mi nobody")),
        "{:?}",
        selectable(&report)
    );
}

/// **The number a counted row opens with**, summed over every row of this pane that carries the
/// marker — `0` when no such row is drawn, because *no row* is how this section says none.
///
/// **Summed rather than found**, so a section that drew its row twice reads as double and fails
/// the partition below instead of matching on the first one and passing.
///
/// **It cannot say a row is absent, and no test here may ask it to.** `0` is the answer for *no
/// row* and for a row reading `0 pods finished and were never removed` alike, so a section that
/// lost its `> 0` guard goes on satisfying `counted(..) == 0` — measured, and it is why the
/// negative below asserts over [`selectable`] instead.
fn counted(report: &Report, marker: &str) -> usize {
    selectable(report)
        .iter()
        .filter(|text| text.contains(marker))
        .map(|text| {
            text.split(' ')
                .next()
                .and_then(|number| number.parse::<usize>().ok())
                .unwrap_or_else(|| panic!("a counted row opens with its number: {text}"))
        })
        .sum()
}

/// **The one `Answer` on this pane whose text carries `marker`.** [`row_for`] matches on the
/// first *word*, and since the split this pane can draw `1 pod was removed by a node and remains`
/// above `1 replicaset is parked at 0 replicas` — so a counted row is found by what it says and
/// never by the number it opens with.
fn row_saying<'a>(report: &'a Report, marker: &str) -> &'a Row {
    let mut found = report
        .rows
        .iter()
        .filter(|row| matches!(row, Row::Answer { text, .. } if text.contains(marker)));
    let row = found.next().unwrap_or_else(|| {
        panic!(
            "no row on this pane says {marker:?}: {:?}",
            selectable(report)
        )
    });
    assert!(
        found.next().is_none(),
        "two rows on this pane say {marker:?}, so this lookup is answering about the wrong one"
    );
    row
}

/// What each of the two rows says it counts — the marker [`counted`] and [`row_saying`] read
/// them back by.
const REMOVED_BY_A_NODE: &str = "removed by a node";
const FINISHED_ON_ITS_OWN: &str = "finished and";

/// **Rule 8's sentence for this pane, whole.** Its tail is the half the split changed — it used
/// to end *"and nothing finished is lying around"*, which was one row's claim standing for two —
/// so a test that stopped at `starts_with` on the opening clause read back none of the change
/// (NOTES § D155).
const NOTHING_WASTED: &str = "Nothing here is going to waste. Every Service reaches a pod, every \
                              disk that was reserved is mounted, and no pod — finished or removed \
                              by a node — is left lying around.";

#[test]
fn the_pileup_splits_in_two_and_the_pods_a_node_removed_come_first() {
    // The corpus is one of each: `broken-evicted` is the pod a node killed to get its ephemeral
    // storage back, `broken-succeeded` and `broken-failed` are the two that ended on their own
    // — and `broken-failed` is `Failed` with `status.reason: DeadlineExceeded`, which is the
    // negative that matters (NOTES § D155, `screens/analysis.md` § *The pileup splits in two*).
    let report = super::waste(&waste_corpus(), &[]);
    println!("{}", pane(&report));

    let removed = row_saying(&report, REMOVED_BY_A_NODE);
    assert_eq!(text_of(removed), "1 pod was removed by a node and remains");
    // **`Info`, the same band as the row below it.** The killing already happened; what is left
    // behind costs an etcd entry and a longer pod list, which is the completed row's own cost —
    // and an evicted pod is collected only past 12 500 finished pods on the node (NOTES § D71),
    // so a `Warn` would stay lit for good, clearable only by deleting this pane's own evidence
    // (`screens/analysis.md` § *The pileup splits in two, one per cause*).
    assert_eq!(severity_of(removed), Some(Severity::Info));
    // **Both causes, and the API's word once in brackets.** `status.reason: Evicted` has two
    // producers in the kubelet and the capture behind this row is the second — a pod over its own
    // declared storage limit, on a node whose three pressure conditions were all `False` — so
    // *the node ran out of room* alone would be false of the only object this row is measured
    // against (`reports/2026-08-23-waste-evicted-row-operator-review.md` §§ 2–4). The word is in
    // parentheses after the translation, the shape every term this project translates uses, and
    // this is the only place on the screen it appears at all: `kubectl get pods` prints `Error`
    // for this object, not `Evicted` (§ 6).
    assert_eq!(
        detail_of(removed),
        ["Either the node was short, or the pod went over its own disk limit (Evicted)."]
    );
    // **The action is the whole reason this row left the other one, and it points at the object.**
    // These pods did not finish; something killed them. Nothing this row can see says which
    // resource or which node ran out, and N3 is silent for the commoner cause — it needs a
    // pressure condition `True` right now, which the pod-limit mechanism never sets — so the
    // reader is sent to the pod's own `status.message`, which names the exact resource and
    // moment.
    assert_eq!(
        action_of(removed),
        "look at one of the pods — its own message names what ran out",
        "and it never names a node, a resource, or another screen"
    );
    assert_eq!(jump_of(removed), None, "it stands for a set (NOTES § D128)");

    let completed = row_saying(&report, FINISHED_ON_ITS_OWN);
    assert_eq!(text_of(completed), "2 pods finished and were never removed");
    // **`Info`, unmoved.** A CronJob keeps three finished Jobs by default and keeps them forever
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 11), so `Warn` over a
    // fact that is often deliberate teaches the wrong lesson the first time a reader chases it
    // and finds nothing to fix (`screens/analysis.md` § Waste).
    assert_eq!(severity_of(completed), Some(Severity::Info));
    assert_eq!(
        detail_of(completed),
        [
            "Kubernetes keeps a few finished Jobs by default, so some of this is normal. They use \
          no CPU or memory — they only make every pod list longer."
        ]
    );
    assert_eq!(
        action_of(completed),
        "",
        "a pod that ran to completion on its own is not worth chasing, and this row still \
         offers nothing to do"
    );
    assert_eq!(jump_of(completed), None);

    // **First of the two, and not because it is louder** — both are `Info`. It leads because it
    // is the more specific statement, naming a cause where the row below names the absence of
    // one, and because it is the row that carries an action.
    let order: Vec<usize> = selectable(&report)
        .iter()
        .enumerate()
        .filter(|(_, text)| text.contains(REMOVED_BY_A_NODE) || text.contains(FINISHED_ON_ITS_OWN))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(
        order.len(),
        2,
        "both rows are on this pane: {:?}",
        selectable(&report)
    );
    assert!(
        order[0] < order[1] && selectable(&report)[order[0]].contains(REMOVED_BY_A_NODE),
        "the pods a node killed sort ahead of the ones that finished: {:?}",
        selectable(&report)
    );
}

#[test]
fn only_the_pods_a_node_removed_leave_the_completed_row() {
    // **The literal `Evicted` and nothing else** (NOTES § D155), fed two kinds of input.
    //
    // `DeadlineExceeded` is the committed negative — `failed.json` carries it — and
    // `NodeAffinity`, `Terminated`, `NodeShutdown` and `OutOfcpu` are the other reasons
    // `screens/analysis.md` § *The pileup splits in two* names as staying in the completed row;
    // this report has no capture of any of them, which is exactly why they belong here as a
    // plant. **Every one of the four is a string the kubelet actually writes** — `Terminated` for
    // a running pod caught by graceful node shutdown and `NodeShutdown` for one rejected during
    // it (`nodeshutdown_manager.go:84,88`), `OutOfcpu` for an admission rejection
    // (`lifecycle/predicate.go`) — because a plant that is not a real value proves the predicate
    // against a shape no cluster can hand it. `OutOfcpu` also probes a `starts_with`-shaped
    // mistake from the other direction: it is the only one whose case differs mid-word.
    // **`evicted` and `EvictedByVPA` are near-misses
    // this file made up**, and neither is claimed to be a value any API writes: they are the two
    // shapes a match that lowercased or `starts_with`-ed its way to an answer would get wrong,
    // and a predicate proven only against words that look nothing like the target is proven
    // against the easy half (NOTES § D29).
    let planted = |reason: Option<&str>| {
        let word = reason.map(str::to_string);
        captured_pod_but("evicted", move |pod| {
            pod.status
                .as_mut()
                .expect("the capture carries a status")
                .reason = word;
        })
    };
    for reason in [
        None,
        Some("DeadlineExceeded"),
        Some("NodeAffinity"),
        Some("Terminated"),
        Some("NodeShutdown"),
        Some("OutOfcpu"),
        Some("evicted"),
        Some("EvictedByVPA"),
    ] {
        let pod = planted(reason);
        assert!(finished(&pod), "the plant is still a pod that is over");
        let report = super::waste(
            &ClusterSnapshot {
                pods: vec![pod],
                ..waste_corpus()
            },
            &[],
        );
        assert_eq!(
            counted(&report, REMOVED_BY_A_NODE),
            0,
            "{reason:?} is not the word, so this pod stays in the completed row: {:?}",
            selectable(&report)
        );
        assert_eq!(
            counted(&report, FINISHED_ON_ITS_OWN),
            1,
            "{reason:?}: and it is counted there rather than dropped: {:?}",
            selectable(&report)
        );
    }

    // The positive, one field apart, so the eight above are refused by the word and not by
    // something else about the plant (NOTES § D29).
    let report = super::waste(
        &ClusterSnapshot {
            pods: vec![planted(Some("Evicted"))],
            ..waste_corpus()
        },
        &[],
    );
    println!("{}", pane(&report));
    assert_eq!(counted(&report, REMOVED_BY_A_NODE), 1);
    assert_eq!(
        counted(&report, FINISHED_ON_ITS_OWN),
        0,
        "and it left the completed row rather than being counted twice"
    );
}

#[test]
fn the_two_rows_partition_the_pods_that_are_over() {
    // **The invariant a third branch would break in silence** (NOTES § D155): the two rows always
    // sum to the count the one row used to draw, no pod lands on both, and none falls through and
    // lands on neither. One equality catches all three — a pod counted twice makes the sum too
    // big, a pod dropped makes it too small.
    //
    // [`finished`] is the outer gate and is untouched by the split, so it is what the sum is
    // measured against rather than a number typed here.
    let over = ["succeeded", "failed", "evicted"].map(captured_pod);
    let running = ["healthy", "healthy-disk"].map(captured_pod);
    let pods: Vec<PodSnapshot> = over.iter().chain(running.iter()).cloned().collect();
    let expected = pods.iter().filter(|pod| finished(pod)).count();
    assert_eq!(expected, 3, "three of the five captures are over");

    let report = super::waste(
        &ClusterSnapshot {
            pods,
            ..waste_corpus()
        },
        &[],
    );
    println!("{}", pane(&report));
    let removed = counted(&report, REMOVED_BY_A_NODE);
    let completed = counted(&report, FINISHED_ON_ITS_OWN);
    assert_eq!(
        removed + completed,
        expected,
        "the two rows sum to what [`finished`] answers, or a pod is on both rows or on neither: \
         {removed} + {completed} against {expected}"
    );
    // And neither side is zero, or the equality above holds over an empty partition.
    assert_eq!((removed, completed), (1, 2));
}

#[test]
fn each_pileup_row_has_its_own_singular_and_a_cluster_with_neither_draws_nothing() {
    // **No threshold on either row**: the box says *pileup* and every number that could stand for
    // one would be invented here, so one pod left behind is one row saying so.
    let one_of_each = super::waste(
        &ClusterSnapshot {
            pods: vec![captured_pod("succeeded"), captured_pod("evicted")],
            ..waste_corpus()
        },
        &[],
    );
    println!("{}", pane(&one_of_each));
    assert_eq!(
        text_of(row_saying(&one_of_each, REMOVED_BY_A_NODE)),
        "1 pod was removed by a node and remains",
        "the singular is a different sentence, not a plural with an `s` taken off"
    );
    assert_eq!(
        text_of(row_saying(&one_of_each, FINISHED_ON_ITS_OWN)),
        "1 pod finished and was never removed",
        "and so is the other row's"
    );
    // **The action does not change with the count** (`screens/analysis.md` § *The pileup splits
    // in two*), and neither does either explanation's subject — except the completed row's, whose
    // sentence is about the pods rather than to the reader.
    let removed = row_saying(&one_of_each, REMOVED_BY_A_NODE);
    assert_eq!(
        action_of(removed),
        "look at one of the pods — its own message names what ran out"
    );
    assert_eq!(
        detail_of(removed),
        ["Either the node was short, or the pod went over its own disk limit (Evicted)."],
        "one sentence for one pod and for four — nothing in it is counted"
    );
    let completed = row_saying(&one_of_each, FINISHED_ON_ITS_OWN);
    assert_eq!(
        detail_of(completed),
        [
            "Kubernetes keeps a few finished Jobs by default, so some of this is normal. It \
          uses no CPU or memory — it only makes every pod list longer."
        ]
    );

    // **The negative for both rows, asserted as an absence and never as a count.** A cluster
    // whose pods are all still running draws neither row — and `0` is what [`counted`] answers
    // for *no row* and for a row reading `0 pods finished and were never removed` alike, so a
    // section that lost its `> 0` guard would go on passing a count of zero. What separates them
    // is whether the pane says the words at all.
    let running = super::waste(
        &ClusterSnapshot {
            pods: vec![captured_pod("healthy")],
            ..waste_corpus()
        },
        &[],
    );
    println!("{}", pane(&running));
    let pileups: Vec<&str> = selectable(&running)
        .into_iter()
        .filter(|text| text.contains(REMOVED_BY_A_NODE) || text.contains(FINISHED_ON_ITS_OWN))
        .collect();
    assert!(
        pileups.is_empty(),
        "neither pileup draws a row at all, rather than a row saying zero: {pileups:?} in {:?}",
        selectable(&running)
    );
}

#[test]
fn replicasets_parked_at_zero_are_counted_and_a_live_one_is_not() {
    // **The one row in this report no capture holds.** Every ReplicaSet the trip's cluster has
    // ever produced is running its pods; a parked one appears after a Deployment has rolled
    // forward and the old set has been scaled to nothing. **What a trip would have to do to
    // replace this plant**: roll `healthy-deploy` to a second image and capture again — the old
    // ReplicaSet is then `spec.replicas: 0` and is the object itself.
    let report = super::waste(&waste_corpus(), &[]);
    assert!(
        !selectable(&report)
            .iter()
            .any(|text| text.contains("parked")),
        "the captured ReplicaSet is running two pods, so nothing is parked"
    );

    let parked = captured_item_but::<ReplicaSet, WorkloadSnapshot>(
        "healthy-replicasets",
        "healthy-deploy-7f84bdfb9b",
        |set| {
            set.spec.get_or_insert_with(Default::default).replicas = Some(0);
        },
    );
    assert_eq!(parked.desired, Some(0), "the plant is the one field");
    let cluster = ClusterSnapshot {
        replica_sets: Some(vec![parked.clone()]),
        ..waste_corpus()
    };
    let report = super::waste(&cluster, &[]);
    println!("{}", pane(&report));
    let row = row_saying(&report, "parked");
    assert_eq!(text_of(row), "1 replicaset is parked at 0 replicas");
    assert_eq!(
        severity_of(row),
        Some(Severity::Info),
        "nothing here is broken"
    );
    assert_eq!(detail_of(row), ["Left behind when deployments moved on."]);
    assert_eq!(jump_of(row), None);

    let two = ClusterSnapshot {
        replica_sets: Some(vec![parked.clone(), parked.clone()]),
        ..waste_corpus()
    };
    assert!(
        selectable(&super::waste(&two, &[]))
            .iter()
            .any(|text| text == &"2 replicasets are parked at 0 replicas")
    );

    // **`Some(0)` and never `None`.** An absent `spec.replicas` is defaulted to 1 by the API
    // server, so a count that read it as zero would flag every workload whose field the prune
    // dropped ([`crate::rules::WorkloadSnapshot::desired`]).
    let unstated = ClusterSnapshot {
        replica_sets: Some(vec![WorkloadSnapshot {
            desired: None,
            ..parked
        }]),
        ..waste_corpus()
    };
    assert!(
        !selectable(&super::waste(&unstated, &[]))
            .iter()
            .any(|text| text.contains("parked"))
    );
}

/// **The corpus with `extra` more Services matching nothing** — `broken-noendpoints` cloned
/// under names that sort after it, which is the shape the capture cannot hold: the trip's cluster
/// has four Services (NOTES § D40).
fn orphan_services(extra: usize) -> ClusterSnapshot {
    let mut services = captured_services();
    services.extend((1..=extra).map(|n| {
        captured_item_but::<Service, ServiceSnapshot>("services", "broken-noendpoints", move |s| {
            s.metadata.name = Some(format!("orphan-{n}"));
        })
    }));
    ClusterSnapshot {
        services: Some(services),
        ..waste_corpus()
    }
}

/// Nine Services matching no pod — four past the cap, so the section is cut and says so.
pub(super) fn overflowing() -> ClusterSnapshot {
    orphan_services(8)
}

#[test]
fn a_section_longer_than_the_pane_is_cut_and_says_what_it_cut() {
    // **The answer [`Row::Answer::jump`] says the Waste box owes** — a cap and an overflow row,
    // because the per-object rows are unbounded and the pane is 16 body lines. On a cluster with
    // 812 broken Services the answer the reader can act on is the count, not 812 rows.
    //
    // **A plant, and the capture could not hold this either**: the trip's cluster has four
    // Services. What a trip would have to do to replace it is deploy nine Services whose
    // selectors match nothing, which is nine objects for one row's arithmetic.
    let report = super::waste(&overflowing(), &[]);
    println!("{}", pane(&report));

    let named: Vec<&str> = selectable(&report)
        .into_iter()
        .filter(|text| text.contains("matches no pod"))
        .collect();
    assert_eq!(
        named,
        vec![
            "default/broken-noendpoints matches no pod",
            "default/orphan-1 matches no pod",
            "default/orphan-2 matches no pod",
            "default/orphan-3 matches no pod",
            "default/orphan-4 matches no pod",
        ],
        "five named, in namespace-then-name order — the order the reader's own `kubectl get -A` \
         prints"
    );
    assert!(
        matches!(&report.rows[5], Row::Prose(text) if text == "and 4 more Services match no pod"),
        "and the rest are one line, directly under them: {:?}",
        report.rows[5]
    );
    assert_eq!(
        selectable(&report).len(),
        8,
        "the overflow line is a `Row::Prose` and the cursor may not land on it — it opens \
         nothing, and not even a *set* a later `Jump` case could be built for (NOTES § D127)"
    );

    // **Every section that can grow is cut, and each says what it cut in its own words** — a
    // second section built on the same helper is where one overflow line comes to name the other
    // section's objects, and nothing but its own test can see that.
    let mut claims = captured_claims();
    claims.extend((1..=8).map(|n| {
        captured_item_but::<PersistentVolumeClaim, ClaimSnapshot>(
            "persistentvolumeclaims",
            "broken-unused-disk",
            move |claim| claim.metadata.name = Some(format!("idle-{n}")),
        )
    }));
    let disks = super::waste(
        &ClusterSnapshot {
            claims: Some(claims),
            ..waste_corpus()
        },
        &[],
    );
    println!("{}", pane(&disks));
    assert_eq!(
        selectable(&disks)
            .iter()
            .filter(|text| text.contains("nobody is using"))
            .count(),
        5
    );
    assert!(
        disks.rows.iter().any(
            |row| matches!(row, Row::Prose(text) if text == "and 4 more disks nobody is using")
        ),
        "the disks' own overflow line, not the Services': {disks:?}"
    );

    // **Per section and not per pane**: one loud section may not starve the others. The disk row
    // and the pileup are still there under the cut.
    assert!(
        selectable(&report)
            .iter()
            .any(|text| text.contains("broken-unused-disk"))
            && selectable(&report)
                .iter()
                .any(|text| text.contains("finished"))
    );

    // Exactly at the cap there is no overflow line — an *and 0 more* is the off-by-one this
    // assertion exists to catch.
    let exact = super::waste(&orphan_services(4), &[]);
    assert_eq!(
        exact
            .rows
            .iter()
            .filter(|row| matches!(row, Row::Prose(_)))
            .count(),
        0,
        "five orphans is five rows and nothing else"
    );
    assert_eq!(
        selectable(&exact)
            .iter()
            .filter(|text| text.contains("matches no pod"))
            .count(),
        5
    );
}

#[test]
fn waste_runs_unchanged_when_scoped_and_only_the_title_changes() {
    // Every input this report has is namespaced and every number on it is the length of a list
    // rather than a share of a total, so a narrower view is a shorter list and never a wrong
    // number (`screens/analysis.md` § *Waste under one namespace*, PRIOR-ART § F2).
    let wide = super::waste(&waste_corpus(), &[]);
    let scoped = super::waste(
        &ClusterSnapshot {
            namespace_scope: Some("payments".to_string()),
            ..waste_corpus()
        },
        &[],
    );
    println!("{}", pane(&scoped));

    assert_eq!(
        scoped.title, "Things in payments that cost you something for nothing",
        "the dangerous state is the narrow one, so it is the labelled one \
         (`screens/analysis.md` rule 6)"
    );
    assert_ne!(wide.title, scoped.title);
    assert_eq!(
        wide.rows, scoped.rows,
        "and nothing else moves — no `NotComputed`, no dropped section"
    );
    assert!(
        not_computed(&scoped).is_empty(),
        "a report that still works must not be made to look broken"
    );
}

/// **The corpus as one namespace's own view of it** — every list narrowed the way `k8s.rs` owes
/// this report, and the scope set. `kube-system` is the one namespace in the corpus with pods and
/// no claims, which is what makes it the honest short list.
fn only_in(namespace: &str) -> ClusterSnapshot {
    let wide = waste_corpus();
    let mine = |ns: Option<&str>| ns == Some(namespace);
    ClusterSnapshot {
        namespace_scope: Some(namespace.to_string()),
        pods: wide
            .pods
            .iter()
            .filter(|p| mine(p.id.namespace.as_deref()))
            .cloned()
            .collect(),
        claims: wide.claims.as_ref().map(|claims| {
            claims
                .iter()
                .filter(|c| mine(c.id.namespace.as_deref()))
                .cloned()
                .collect()
        }),
        services: wide.services.as_ref().map(|services| {
            services
                .iter()
                .filter(|s| mine(s.id.namespace.as_deref()))
                .cloned()
                .collect()
        }),
        endpoint_slices: wide.endpoint_slices.as_ref().map(|slices| {
            slices
                .iter()
                .filter(|s| mine(s.id.namespace.as_deref()))
                .cloned()
                .collect()
        }),
        replica_sets: wide.replica_sets.as_ref().map(|sets| {
            sets.iter()
                .filter(|r| mine(r.id.namespace.as_deref()))
                .cloned()
                .collect()
        }),
        ..wide
    }
}

#[test]
fn a_scoped_pane_is_a_shorter_list_and_never_a_wrong_number() {
    // **The shape the pipeline actually hands this report under a scope** (NOTES § D29): every
    // list narrowed together, which is the pairing [`super::disks_nobody_mounts`] names as
    // `k8s.rs`'s to keep. The test above feeds the *unnarrowed* one — the same lists with only
    // the scope field set — and proves the title moves; this one proves the promise underneath
    // it, that a narrower view is a shorter list and never a wrong number
    // (`screens/analysis.md` § *Waste under one namespace*).
    let wide = super::waste(&waste_corpus(), &[]);
    let scoped = super::waste(&only_in("kube-system"), &[]);
    println!("{}", pane(&scoped));

    assert_eq!(
        selectable(&wide),
        vec![
            "default/broken-noendpoints matches no pod",
            "default/broken-unused-disk is 128Mi nobody is using",
            "1 pod was removed by a node and remains",
            "2 pods finished and were never removed",
        ],
        "what the whole cluster has to say, for the narrow pane to be a subset of"
    );
    assert!(
        selectable(&scoped).is_empty(),
        "everything wasteful in the corpus is in `default`, so `kube-system`'s own view has \
         nothing to report — and says so rather than reporting a fraction of somebody else's \
         numbers: {:?}",
        selectable(&scoped)
    );
    assert!(
        matches!(&scoped.rows[..], [Row::Prose(text)] if text == NOTHING_WASTED),
        "rule 8's sentence whole, and not a `NotComputed`: every list was read, they were just \
         short ({:?})",
        scoped.rows
    );
    assert!(not_computed(&scoped).is_empty());
    assert_eq!(
        scoped.title, "Things in kube-system that cost you something for nothing",
        "and the title still says which namespace, because the number under it is a namespace's"
    );

    // **The other end of the same promise, and it is the one nothing here can defend against.**
    // A pod list narrowed to one namespace against a claim list that was not is the mismatch
    // [`super::disks_nobody_mounts`]' own doc names: every claim outside the scope has no pod in
    // the snapshot to mount it, so it is drawn as a disk nobody is using — `default/healthy-disk`
    // included, which a running `default` pod has open. **This report cannot see the mismatch**;
    // handing it two lists of one scope is `k8s.rs`'s, and the test is here so the contract is
    // written where it would be broken.
    let mismatched = ClusterSnapshot {
        claims: waste_corpus().claims,
        ..only_in("kube-system")
    };
    let report = super::waste(&mismatched, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        selectable(&report),
        vec![
            "default/broken-unused-disk is 128Mi nobody is using",
            "default/healthy-disk is 64Mi nobody is using",
        ],
        "the second row is the wrong answer a mismatched pair produces, and `healthy-disk.json` \
         is the pod that mounts it"
    );
}

/// **The corpus with the wasteful objects left out** — every Service that reaches a pod, the disk
/// that is mounted, the ReplicaSet that is running, and no pod that has finished.
pub(super) fn nothing_wasted() -> ClusterSnapshot {
    ClusterSnapshot {
        pods: vec![captured_pod("healthy-disk")],
        services: Some(
            captured_services()
                .into_iter()
                .filter(|s| s.id.name != "broken-noendpoints")
                .collect(),
        ),
        claims: Some(
            captured_claims()
                .into_iter()
                .filter(|c| c.id.name != "broken-unused-disk")
                .collect(),
        ),
        ..waste_corpus()
    }
}

#[test]
fn nothing_wasted_says_so_in_this_reports_own_words() {
    // Rule 8: a report with nothing to say says it as one `Row::Prose`, so `views.rs` carries no
    // per-report empty text (NOTES § D128). The cluster is the corpus with the wasteful objects
    // left out — every Service that reaches a pod, the disk that is mounted, the ReplicaSet that
    // is running, and no pod that has finished.
    let report = super::waste(&nothing_wasted(), &[]);
    println!("{}", pane(&report));

    assert_eq!(report.rows.len(), 1);
    assert!(
        matches!(&report.rows[0], Row::Prose(text) if text == NOTHING_WASTED),
        "the whole sentence, tail included: its `no pod — finished or removed by a node` is the \
         clause the split changed, and both rows have to be covered by it or the pane promises \
         over one of them only: {:?}",
        report.rows[0]
    );
    assert!(
        selectable(&report).is_empty(),
        "the sentence is read, never selected — this pane has no cursor and drops `⏎ open` from \
         its footer ([`Row`]'s doc)"
    );
    assert_eq!(report.badge, None);
    assert!(
        !report.title.is_empty(),
        "an empty report is a pane, not a blank screen"
    );

    // **And it is never said over a section that did not run.** A pane carrying one
    // `NotComputed` has not established that nothing is going to waste.
    let unread = ClusterSnapshot {
        claims: None,
        ..nothing_wasted()
    };
    let report = super::waste(&unread, &[]);
    println!("{}", pane(&report));
    assert!(
        !report.rows.iter().any(|row| matches!(row, Row::Prose(_))),
        "the sentence is gone the moment one section could not look: {report:?}"
    );
    assert_eq!(not_computed(&report).len(), 1);
}
