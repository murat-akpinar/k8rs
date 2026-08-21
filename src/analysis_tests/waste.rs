//! `analysis.rs` § THE WASTE REPORT — its tests (NOTES § D91).

use super::*;

// --- WASTE ---
//
// **The producer, against the committed corpus** — the two hand-built panes that stood here until
// this box landed are gone. The corpus holds one of each row this report draws: a Service with a
// selector and nothing behind it, a Service with no selector at all, a `Bound` claim nobody
// mounts beside one a pod does, and two pods that finished and stayed. **The ReplicaSet parked at
// zero is the one row no capture holds** — the trip's cluster has never had one — so it is a
// plant (NOTES § D40), and what a trip would have to do to replace it is on the test.

/// **The corpus with the four lists Waste fetches**, plus the two finished pods and the one pod
/// that mounts a claim. `captured_pods` holds nothing finished and nothing mounting a disk, so
/// without these three the report's second and third rows have neither a positive nor a negative.
pub(super) fn waste_corpus() -> ClusterSnapshot {
    let mut pods = captured_pods();
    pods.extend(["succeeded", "failed", "healthy-disk"].map(captured_pod));
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
            "2 pods finished and were never removed",
        ],
        "the 503 nobody can explain first, then the disk, then the pileup \
         (`screens/analysis.md` § Waste)"
    );
    assert_eq!(
        report.rows.len(),
        3,
        "and there is no `Prose` on this pane at all — every line is a row"
    );
    assert_eq!(
        report.rows.iter().map(severity_of).collect::<Vec<_>>(),
        vec![
            Some(Severity::Critical),
            Some(Severity::Warn),
            Some(Severity::Info)
        ],
        "the 503, the disk nobody mounts, and the pileup that is often deliberate"
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
        1,
        "only the Service has a way out — deleting a disk deletes what is on it, and this report \
         does not know whether that matters"
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

#[test]
fn the_finished_pod_row_is_one_row_over_evicted_and_completed() {
    // One row, because [`finished`] is already the predicate over `Succeeded | Failed` and the
    // reader does one thing about both. `broken-succeeded` and `broken-failed` are the two.
    let report = super::waste(&waste_corpus(), &[]);
    let row = row_for(&report, "2");
    assert_eq!(text_of(row), "2 pods finished and were never removed");
    // **`Info`, and the sentence says the normal case out loud.** A CronJob keeps three finished
    // Jobs by default and keeps them forever
    // (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 11), so `Warn` over a
    // fact that is often deliberate teaches the wrong lesson the first time a reader chases it
    // and finds nothing to fix (`screens/analysis.md` § Waste).
    assert_eq!(severity_of(row), Some(Severity::Info));
    assert_eq!(
        detail_of(row),
        [
            "Kubernetes keeps a few finished Jobs by default, so some of this is normal. They use \
          no CPU or memory — they only make every pod list longer."
        ]
    );
    assert_eq!(jump_of(row), None, "it stands for a set (NOTES § D128)");

    // **No threshold**: the box says *pileup* and every number that could stand for one would be
    // invented here, so one pod left behind is one row saying so.
    let one = ClusterSnapshot {
        pods: vec![captured_pod("succeeded")],
        ..waste_corpus()
    };
    let report = super::waste(&one, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        text_of(row_for(&report, "1")),
        "1 pod finished and was never removed",
        "and the singular is a different sentence, not a plural with an `s` taken off"
    );
    assert_eq!(
        detail_of(row_for(&report, "1")),
        [
            "Kubernetes keeps a few finished Jobs by default, so some of this is normal. It \
          uses no CPU or memory — it only makes every pod list longer."
        ]
    );

    // The negative: a cluster whose pods are all still running draws no such row.
    let running = ClusterSnapshot {
        pods: vec![captured_pod("healthy")],
        ..waste_corpus()
    };
    assert!(
        !selectable(&super::waste(&running, &[]))
            .iter()
            .any(|text| text.contains("finished"))
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
    let row = row_for(&report, "1");
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
        7,
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
        matches!(&scoped.rows[..], [Row::Prose(text)]
            if text.starts_with("Nothing here is going to waste")),
        "rule 8's sentence, and not a `NotComputed`: every list was read, they were just short \
         ({:?})",
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
        matches!(&report.rows[0], Row::Prose(text)
            if text.starts_with("Nothing here is going to waste")),
        "{:?}",
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
