//! `analysis.rs` § THE RESTARTS REPORT — its tests (NOTES § D91).

use super::*;

use crate::rules::{
    ContainerRole, ContainerState, RESTARTS_WARN, age, analyze, container_fact, doing_its_job,
};

use k8s_openapi::api::core::v1::ContainerStatus;
use k8s_openapi::jiff::SignedDuration;

// --- RESTARTS ---
//
// **Both sides of the qualifying test are captured, not planted** (NOTES § D53), and the negatives
// are the half worth naming, because each one is a *different* way of not being in this pane's
// set. Measured off the committed captures, at or above rule 5's threshold:
//
// * serving and drawn — `restarts10serving` (10), `gang`'s two (3 each, one pod), `reboot` (3),
//   `restarts` (3): five. **This pane caps nothing**, so a sixth and a seventh are drawn too; the
//   readiness plant below is what reaches them.
// * `Running` but **not ready** — `probe0` (13) and `restarts10` (10). These are the two the
//   filter change was for: `Running` would have drawn them on a pane whose every row is a
//   container that is serving. What still covers them is **rule 5's non-serving branch**, which
//   ages out only for a serving container and so cards this shape permanently, whatever its role
//   — not rule 7, whose `running_but_not_ready` returns `None` for any role but `Regular`, so a
//   native sidecar failing the identical probe gets no rule 7 card at all.
// * not `Running` at all — `crashloop` (10) and `sigterm` (15) are `Waiting`; `oom` (10),
//   `succeeded` (3) and `init` (10) are `Terminated`.
// * `oomserving` is `Running`, ready, and at **1** restart — two under the threshold, the only
//   negative the count alone excludes.
// * `healthy-retry` is the shape that qualifies and still draws nothing: an init container that
//   failed three times and then exited `0`, so `doing_its_job` answers *yes* while its state is
//   `Terminated` and there is no current run to put an age on.
//
// Five shapes below are plants on the decoded snapshot (NOTES § D40), each one the corpus cannot
// hold: a restart count sitting on either side of the threshold, a readiness check passing on a
// pod that fails it (the only route past five drawable containers), the under-eight-second window
// where `state.running.startedAt` is still `None` (NOTES § D100) together with a start past
// [`age`]'s skew allowance, a container list that is not already in alphabetical order, and two
// containers tied on their count with runs of different ages.

/// **Every capture that carries a restarting container, on one cluster.** The five that are drawn,
/// the two that are `Running` and not ready, the five that are not `Running`, the one under the
/// threshold, and the init container that qualifies without a run to measure.
pub(super) fn restarts_corpus() -> ClusterSnapshot {
    let mut pods = captured_pods();
    pods.extend(
        [
            "gang",
            "probe0",
            "reboot",
            "restarts10",
            "restarts10serving",
            "restarts",
            "crashloop",
            "oom",
            "sigterm",
            "succeeded",
            "healthy-retry",
            "init",
            "oomserving",
        ]
        .map(captured_pod),
    );
    ClusterSnapshot { pods, ..corpus() }
}

/// The `(namespace/pod, container)` pairs the screen's filter admits — **serving now and at rule
/// 5's threshold** — read off the **snapshot**, so what the producer draws is checked against the
/// screen's rule and not against the producer's own answer.
fn qualifying(cluster: &ClusterSnapshot) -> BTreeSet<(String, &str)> {
    pairs(cluster)
        .filter(|(_, c)| {
            matches!(c.state, ContainerState::Running { .. })
                && doing_its_job(c)
                && c.restarts >= RESTARTS_WARN
        })
        .map(|(pod, c)| (qualified(&pod.id), c.name.as_str()))
        .collect()
}

/// The subset of those that can also be **drawn** — a current run with an age this screen can
/// print. The two sets are not the same one, and the pane behaves differently on the difference.
fn drawable(cluster: &ClusterSnapshot) -> BTreeSet<(String, &str)> {
    pairs(cluster)
        .filter(|(_, c)| {
            matches!(c.state, ContainerState::Running { .. })
                && doing_its_job(c)
                && c.restarts >= RESTARTS_WARN
        })
        .filter(|(_, c)| match &c.state {
            ContainerState::Running {
                started_at: Some(started),
            } => age(&cluster.now, started).is_some(),
            _ => false,
        })
        .map(|(pod, c)| (qualified(&pod.id), c.name.as_str()))
        .collect()
}

fn pairs(cluster: &ClusterSnapshot) -> impl Iterator<Item = (&PodSnapshot, &ContainerSnapshot)> {
    cluster
        .pods
        .iter()
        .flat_map(|pod| pod.containers.iter().map(move |c| (pod, c)))
}

/// The first container status of a decoded capture — where the `ContainerStatus` plants land.
fn first_status(pod: &mut Pod) -> &mut ContainerStatus {
    &mut pod
        .status
        .as_mut()
        .expect("the capture carries a status")
        .container_statuses
        .as_mut()
        .expect("the capture carries a container")[0]
}

/// One pod, alone on the cluster: `restarts.json`'s `flaky`, which is serving with a start on the
/// record, with one field moved.
fn one_serving_container(edit: impl FnOnce(&mut ContainerStatus)) -> ClusterSnapshot {
    let pod = captured_pod_but("restarts", |pod| edit(first_status(pod)));
    ClusterSnapshot {
        pods: vec![pod],
        ..corpus()
    }
}

/// **The corpus with the readiness check passing on pods that fail it** — a one-field plant on the
/// decoded snapshot (NOTES § D40), and the only route past the five drawable containers the
/// captures hold, which is what a claim about *more than five rows* needs.
fn also_serving(names: &[&str]) -> ClusterSnapshot {
    let mut cluster = restarts_corpus();
    for pod in &mut cluster.pods {
        if names.contains(&pod.id.name.as_str()) {
            for container in &mut pod.containers {
                container.ready = true;
            }
        }
    }
    cluster
}

/// The `Prose` lines of a pane, in order — the opening paragraph, the overflow line, and the
/// sentence an empty pane says in its own words.
fn prose(report: &Report) -> Vec<&str> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Prose(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// **The opening paragraph, which may not tell the reader nothing is broken.** Rule 5's serving
/// card ages out only once the current run passes `NOT_READY_GRACE`, and this pane qualifies a
/// container the moment it is serving — so for ten minutes after every restart the two sets
/// overlap and the old wording denied what Alerts was saying at that moment
/// (`screens/analysis.md` § Restarts).
const OPENING: &str = "Every container below is serving right now. A restart count never clears \
                       itself — the second number, how long this run has lasted, is the signal.";

#[test]
fn no_container_is_drawn_that_the_screen_does_not_qualify() {
    // **The claim the pane exists to make, in the direction that can go wrong quietly.** Three
    // different ways of not being in this set are in the corpus at counts *above* the threshold:
    // `Running` and not ready, `Waiting`, and `Terminated`. A producer that filtered on the count
    // alone draws all of them; one that filtered on `Running` still draws the first two.
    let cluster = restarts_corpus();
    let report = super::restarts(&cluster, &[]);
    println!("{}", pane(&report));

    let allowed = drawable(&cluster);
    // A derived list asserts it found something (CLAUDE.md § Tests must not lie): were this
    // empty, every claim below it would pass over nothing.
    assert_eq!(
        allowed.len(),
        5,
        "the corpus draws five containers, which is the cap exactly: {allowed:?}"
    );
    assert!(
        allowed.contains(&("default/broken-gang".to_string(), "trigger")),
        "the gang-restart shape is the one this pane most exists for"
    );
    // **The two sets are the same one here, and the only thing that can separate them is a run
    // whose age has not arrived** — D100's eight-second window, which no capture can hold, so it
    // is planted in `a_run_k8rs_cannot_put_an_age_on_draws_no_row_and_makes_no_claim_either` and
    // asserted there as this same difference. `healthy-retry`'s init container used to sit in the
    // gap permanently: it exited `0`, so `doing_its_job` answers yes, and it is `Terminated`, so
    // it can never be drawn — an opening paragraph over an empty pane, for ever.
    assert_eq!(&qualifying(&cluster) - &allowed, BTreeSet::new());

    // Nothing is silently dropped, and nothing is cut: this pane scrolls.
    assert_eq!(selectable(&report).len(), allowed.len());
    for row in &report.rows {
        let Row::Answer { text, .. } = row else {
            continue;
        };
        assert!(
            allowed
                .iter()
                .any(|(pod, container)| text == &format!("{pod} · container {container}")),
            "this row names a container the screen does not qualify: {text}"
        );
    }

    // **The captured negatives on their own**, so the cap cannot be what is hiding them. Counts of
    // 13, 10, 10, 15, 3, 10 and 1 — every one of them drawn by a producer that filtered on the
    // count alone, and the first two still drawn by one that filtered on `Running`.
    let negatives = ClusterSnapshot {
        pods: [
            "probe0",
            "restarts10",
            "crashloop",
            "oom",
            "sigterm",
            "succeeded",
            "init",
            "oomserving",
        ]
        .map(captured_pod)
        .to_vec(),
        ..corpus()
    };
    assert!(qualifying(&negatives).is_empty());
    assert_eq!(
        selectable(&super::restarts(&negatives, &[])),
        Vec::<&str>::new(),
        "not one of these is serving above the threshold, and each already has its own card"
    );
}

#[test]
fn a_container_that_is_running_but_not_ready_is_not_serving_and_is_not_here() {
    // **The blocker this filter change closed, on the two captures that carry it.** `probe0` at 13
    // restarts and `restarts10` at 10 are `Running`, so a `ContainerState::Running` filter drew
    // them on a pane whose opening paragraph says every container below is *serving*. The filter
    // is `doing_its_job`, the suppressor rules 2, 5 and 6 already share — and what still covers
    // the pair is rule 5's non-serving branch, which never ages out for this shape and holds for
    // any role, not rule 7, which returns `None` for anything but a `Regular` container.
    let cluster = restarts_corpus();
    for (pod, container) in [("broken-probe0", "app"), ("broken-restarts10", "flaky")] {
        let c = pairs(&cluster)
            .find(|(p, c)| p.id.name == pod && c.name == container)
            .map(|(_, c)| c)
            .unwrap_or_else(|| panic!("the corpus has no {pod}/{container}"));
        assert!(
            c.restarts >= RESTARTS_WARN
                && matches!(c.state, ContainerState::Running { .. })
                && !c.ready,
            "{pod}/{container} is the shape: above the threshold, running, and not ready"
        );
        assert!(!doing_its_job(c), "so it is not serving");
    }
    let report = super::restarts(&cluster, &[]);
    let drawn = selectable(&report);
    for pod in ["broken-probe0", "broken-restarts10"] {
        // The prefix and not a substring: `broken-restarts10` is one inside
        // `broken-restarts10serving`, which is a row this pane is supposed to draw.
        let named = format!("default/{pod} ·");
        assert!(
            !drawn.iter().any(|text| text.starts_with(&named)),
            "{pod} is on a pane that says nothing below it is broken: {drawn:?}"
        );
    }
    // And rule 7 is the card they carry instead, so neither is a container k8rs stays silent
    // about — it is on the screen that may say something is wrong, and off the one that may not.
    let findings = analyze(&cluster);
    let not_receiving: Vec<&str> = findings
        .iter()
        .filter(|f| f.title.starts_with("Running, but not receiving traffic"))
        .map(|f| f.object.name.as_str())
        .collect();
    assert_eq!(not_receiving, ["broken-probe0", "broken-restarts10"]);
}

#[test]
fn the_worst_leads_and_the_row_is_the_container_fact_with_both_numbers_under_it() {
    // **Sort and row shape at once**, against the committed captures' own numbers: 10 · 3 · 3 · 3
    // · 3, so one count decides the lead and the four-way tie under it is decided by the second
    // number — youngest run first. Their captured starts are `23:12:04` (reboot), `22:43:53`
    // (restarts), `22:43:27` (gang bystander) and `22:43:24` (gang trigger), which is the order
    // below and is **not** `namespace/pod` order: `broken-gang` sorts first alphabetically and
    // comes third and fourth here.
    let report = super::restarts(&restarts_corpus(), &[]);
    println!("{}", pane(&report));
    assert_eq!(
        selectable(&report),
        [
            "default/broken-restarts10serving · container flaky",
            "default/broken-reboot · container app",
            "default/broken-restarts · container flaky",
            "default/broken-gang · container bystander",
            "default/broken-gang · container trigger",
        ]
    );
    assert_eq!(
        detail_of(&report.rows[1]),
        [
            "Restarted 10 times since this pod started.".to_string(),
            "This run started 2 days ago.".to_string(),
        ],
        "both numbers, one paragraph each, and never divided"
    );
    // The opening paragraph is the only prose on this pane — there is no fold line to add, and
    // the paragraph may not tell the reader nothing is broken.
    assert_eq!(prose(&report), [OPENING]);
}

/// `gang`'s two containers, equal at 3 restarts, renamed so that **younger-first and alphabetical
/// disagree**: the sidecar `zz-proxy` started at `22:43:27` and the regular `aa-app` at
/// `22:43:24`. The list order is the kubelet's, which `rules.rs` builds as `init.chain(main)` and
/// is therefore not alphabetical by construction.
fn a_sidecar_and_an_app_tied_on_their_count() -> PodSnapshot {
    let mut pod = captured_pod("gang");
    pod.containers[0].name = "zz-proxy".to_string();
    pod.containers[0].role = ContainerRole::Sidecar;
    pod.containers[1].name = "aa-app".to_string();
    pod
}

/// **The row this pane must draw for one container — composed from [`container_fact`], never
/// copied out of it.**
///
/// `container_fact` was widened to `pub(crate)` *for this reader*, and the screen's rule is that
/// the row's identity is that function's output verbatim, never a second wording of a role. A
/// literal here cannot hold that rule: a producer that stopped calling the function and inlined
/// the same sentence passes a literal pin, and passes the mutation gate too, because
/// cargo-mutants cannot express *replace a call with an equal literal*. So the pin is the call.
/// `src/rules_tests/pod.rs` pins the same function the same way.
fn expected_row(pod: &PodSnapshot, container: &str) -> String {
    let c = pod
        .containers
        .iter()
        .find(|c| c.name == container)
        .unwrap_or_else(|| panic!("the plant has no container {container}"));
    format!("{} · {}", qualified(&pod.id), container_fact(c))
}

#[test]
fn a_tie_on_the_count_breaks_on_the_younger_run_and_not_on_the_name() {
    // **The second number was computed one line above the comparator and thrown away.** These two
    // are three seconds apart and both spell `1 hour ago`, so a comparator reading the *rung*
    // ties and falls through to the name — which is why `Cycling` keeps the moment beside its
    // spelling. Younger first puts the sidecar ahead of a container that sorts before it
    // alphabetically, so this order can only come from the run.
    let pod = a_sidecar_and_an_app_tied_on_their_count();
    let report = super::restarts(
        &ClusterSnapshot {
            pods: vec![pod.clone()],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&report));
    assert_eq!(
        selectable(&report),
        [expected_row(&pod, "zz-proxy"), expected_row(&pod, "aa-app")]
    );
    assert_eq!(
        detail_of(&report.rows[1])[1],
        detail_of(&report.rows[2])[1],
        "and both runs spell the same rung, so the string could not have ordered them"
    );
}

#[test]
fn a_role_is_spelled_by_container_fact_and_an_equal_run_breaks_on_the_name() {
    // **The name leg is the third tie-break and needs the first two tied to be reachable at all**
    // — equal counts and now an equal run, which no capture holds, so the start moves onto one
    // moment (NOTES § D40). `rules.rs` builds `containers` as `init.chain(main)`, so a pod with a
    // restarting sidecar and a restarting regular container is non-alphabetical by construction
    // and this leg is what puts it right.
    let mut pod = a_sidecar_and_an_app_tied_on_their_count();
    for container in &mut pod.containers {
        container.state = ContainerState::Running {
            started_at: Some(now()),
        };
    }
    let report = super::restarts(
        &ClusterSnapshot {
            pods: vec![pod.clone()],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&report));
    assert_eq!(
        selectable(&report),
        // Alphabetical, against the source order — and each identity is `container_fact`'s own
        // output, because a second wording of a role is the defect `ContainerRole`'s doc names.
        [expected_row(&pod, "aa-app"), expected_row(&pod, "zz-proxy")]
    );
    // **A derived expectation asserts it derived something** (CLAUDE.md § Tests must not lie): a
    // `container_fact` that stopped glossing a role would compose two rows this pane matched
    // happily. The gloss is a trailing parenthetical; its words are `rules.rs`' to choose.
    assert!(
        expected_row(&pod, "zz-proxy").ends_with(')')
            && !expected_row(&pod, "aa-app").ends_with(')'),
        "a sidecar's identity carries a gloss and a regular container's does not"
    );
}

#[test]
fn a_set_larger_than_five_draws_every_row_and_folds_none_of_them() {
    // **This pane scrolls; it does not cap** (`screens/analysis.md` § Restarts). The five-row
    // budget it borrowed from Waste is *per section* — four sections share one pane there — and
    // this pane has one section with nothing to starve. Measured on a one-node kind cluster,
    // three node reboots took the qualifying set from 6 to 17 and the kept slots went to five
    // containers that had stopped restarting, while the one on a live ten-minute cycle became an
    // unselectable `and 1 more`.
    for also in [
        &["broken-probe0"][..],
        &["broken-probe0", "broken-restarts10"][..],
    ] {
        let cluster = also_serving(also);
        let drawn = drawable(&cluster);
        assert_eq!(drawn.len(), 5 + also.len(), "more than the old cap");
        let report = super::restarts(&cluster, &[]);
        println!("{}", pane(&report));
        assert_eq!(
            selectable(&report).len(),
            drawn.len(),
            "every qualifying container has a row the cursor can land on"
        );
        assert_eq!(
            prose(&report),
            [OPENING],
            "and no `and N more` line, which is a `Row::Prose` the cursor cannot reach"
        );
    }
}

#[test]
fn both_numbers_are_the_container_s_own_and_the_row_jumps_to_its_pod() {
    // **`gang.json` is two qualifying containers in one pod** — the case D101 rules is never
    // merged and never summed. Each row carries that container's own count and its own run, and
    // both jump to the one pod the reader sees them both from.
    let pod = captured_pod("gang");
    let cluster = ClusterSnapshot {
        pods: vec![pod.clone()],
        ..corpus()
    };
    let report = super::restarts(&cluster, &[]);
    println!("{}", pane(&report));

    assert_eq!(
        selectable(&report),
        [
            "default/broken-gang · container bystander",
            "default/broken-gang · container trigger",
        ],
        "two containers, two rows, neither merged into the other"
    );
    // The two runs began three seconds apart, which is one restart of one pod and two separate
    // clocks all the same: each row measures its own.
    for row in report
        .rows
        .iter()
        .filter(|r| matches!(r, Row::Answer { .. }))
    {
        assert_eq!(
            detail_of(row),
            [
                "Restarted 3 times since this pod started.".to_string(),
                "This run started 2 days ago.".to_string(),
            ],
            "the count first, then the run's own age off `state.running.startedAt`"
        );
        assert_eq!(
            severity_of(row),
            Some(Severity::Info),
            "the pane makes no judgement, and a band that varied would be one"
        );
        assert!(
            action_of(row).is_empty(),
            "there is nothing to fix — the container is serving"
        );
        assert_eq!(
            jump_of(row),
            Some(&Jump::Object(pod.id.clone())),
            "there is no finding here, which is the whole reason the row exists"
        );
    }
}

#[test]
fn the_run_is_measured_from_the_start_of_the_run_and_not_from_the_end_of_the_last_one() {
    // **D100's measurement, as a test.** The two synthesized `137`s of a gang restart leave
    // `lastState.terminated.finishedAt` null, so a producer that reached for the field that
    // looks equivalent draws nothing on the one shape this pane most exists for.
    let pod = captured_pod("gang");
    assert!(
        pod.containers.iter().all(|c| c
            .last_terminated
            .as_ref()
            .is_some_and(|t| t.finished_at.is_none())),
        "the capture is the one D100 measured: a terminated record with no end on it"
    );
    let report = super::restarts(
        &ClusterSnapshot {
            pods: vec![pod],
            ..corpus()
        },
        &[],
    );
    assert_eq!(selectable(&report).len(), 2);
}

#[test]
fn a_run_k8rs_cannot_put_an_age_on_draws_no_row_and_makes_no_claim_either() {
    // **Both numbers or no row — and the empty sentence may not stand in for the missing one.**
    // The under-eight-second window after a restart leaves `startedAt` null (NOTES § D100), and a
    // start far enough ahead of `now` is a wrong field rather than a wrong clock, which [`age`]
    // answers `None` to. The container is serving and above the threshold either way, so
    // *"nothing has restarted enough to matter"* would be false about it: the pane keeps its
    // opening paragraph and draws neither a row nor a claim
    // (`screens/analysis.md` § Restarts).
    let unaged = |edit: fn(&mut ContainerStatus)| {
        let cluster = one_serving_container(edit);
        // **This is the one shape that qualifies and cannot be drawn**, and it is transient.
        assert_eq!(
            &qualifying(&cluster) - &drawable(&cluster),
            BTreeSet::from([("default/broken-restarts".to_string(), "flaky")]),
            "it still qualifies — that is the whole point"
        );
        let report = super::restarts(&cluster, &[]);
        println!("{}", pane(&report));
        assert_eq!(selectable(&report), Vec::<&str>::new());
        assert_eq!(
            prose(&report),
            [OPENING],
            "the opening paragraph and nothing else — no row, and no claim that nothing qualifies"
        );
    };
    unaged(|status| {
        status
            .state
            .as_mut()
            .expect("the capture carries a state")
            .running
            .as_mut()
            .expect("this container is running")
            .started_at = None;
    });
    unaged(|status| {
        status
            .state
            .as_mut()
            .expect("the capture carries a state")
            .running
            .as_mut()
            .expect("this container is running")
            .started_at = Some(Time(now().0 + SignedDuration::from_hours(24)));
    });
    // And the same capture untouched is drawn, or the two above prove only that the plant broke
    // the fixture.
    assert_eq!(
        selectable(&super::restarts(&one_serving_container(|_| {}), &[])).len(),
        1
    );
}

#[test]
fn the_threshold_is_rule_5_s_own_and_the_empty_sentence_names_the_same_line() {
    // **The boundary, from both sides.** A container one restart under the threshold draws no
    // row — and the sentence the pane says instead is a claim about every container it *would*
    // have drawn, so the number in that sentence and the number in the qualifying test are one
    // number or the pane lies.
    let under = super::restarts(
        &one_serving_container(|status| status.restart_count = RESTARTS_WARN - 1),
        &[],
    );
    println!("{}", pane(&under));
    assert_eq!(selectable(&under), Vec::<&str>::new());
    assert_eq!(
        prose(&under),
        [format!(
            "Nothing here has restarted enough to matter. Every container serving right now has \
             restarted {} or fewer times since its pod started.",
            RESTARTS_WARN - 1
        )]
    );
    assert_eq!(
        selectable(&super::restarts(
            &one_serving_container(|status| status.restart_count = RESTARTS_WARN),
            &[],
        ))
        .len(),
        1,
        "and one more restart is a row"
    );
    // A cluster with no pods at all says the same sentence, and it is the only row on the pane.
    let nothing = super::restarts(
        &ClusterSnapshot {
            pods: Vec::new(),
            ..corpus()
        },
        &[],
    );
    assert_eq!(nothing.rows.len(), 1);
    assert_eq!(prose(&nothing), prose(&under));
    // **The clusters the wording is for** (`screens/analysis.md` § *Empty, and nothing
    // qualifies*), and neither is `Running`-free: `probe0` at 13 restarts and `restarts10` at 10
    // are running and not ready, `crashloop` at 10 and `sigterm` at 15 are crash-looping. Every
    // one of them carries a card, and *every container running right now* — never mind *every
    // container* — would have swept the first two into *2 or fewer*.
    for not_serving in [&["crashloop", "sigterm"][..], &["probe0", "restarts10"][..]] {
        let cluster = ClusterSnapshot {
            pods: not_serving.iter().map(|n| captured_pod(n)).collect(),
            ..corpus()
        };
        let report = super::restarts(&cluster, &[]);
        println!("{}", pane(&report));
        assert_eq!(prose(&report), prose(&under));
    }
}

#[test]
fn an_init_container_that_finished_well_is_not_in_a_run_and_is_not_in_this_set() {
    // **`doing_its_job` answers *finished well* on its `Init` arm, which is not *in a run right
    // now*** — and this pane's second number is the age of a run. `healthy-retry`'s `wait-for-db`
    // failed three times and then exited `0`, so the health half says yes while its state is
    // `Terminated`: under a filter of health and count alone it qualified and could never be
    // drawn, so a cluster it was the only member of drew the opening paragraph over an empty
    // pane, permanently — withholding the true sentence rather than saying it (NOTES § D101).
    let pod = captured_pod("healthy-retry");
    let container = &pod.containers[0];
    assert!(
        doing_its_job(container)
            && container.restarts >= RESTARTS_WARN
            && !matches!(container.state, ContainerState::Running { .. }),
        "the capture is the shape: healthy by the shared suppressor, above the threshold, and in \
         no current run"
    );
    let cluster = ClusterSnapshot {
        pods: vec![pod],
        ..corpus()
    };
    assert!(qualifying(&cluster).is_empty());
    let report = super::restarts(&cluster, &[]);
    println!("{}", pane(&report));
    assert_eq!(
        prose(&report),
        [format!(
            "Nothing here has restarted enough to matter. Every container serving right now has \
             restarted {} or fewer times since its pod started.",
            RESTARTS_WARN - 1
        )],
        "the empty sentence, and not an opening paragraph with nothing under it"
    );
}

#[test]
fn the_empty_sentence_carries_the_namespace_the_title_already_names() {
    // **A row carries its scope in its own `namespace/pod` prefix; this line has no row.** Under
    // `--namespace payments` — or the 403 fallback that fills the same field — only `payments` was
    // ever read, so the unscoped wording asserts something about every serving container in the
    // cluster while the title above it says one namespace. `kube-system/etcd` at forty restarts
    // and serving is what makes it false (`screens/analysis.md` § *Restarts under one namespace*).
    //
    // **The driver pins `namespace_scope: None`, so the binary cannot reach this state and only a
    // test can.** Nothing asserted it before 2026-08-22: the title test read only the title, and
    // the scoped case in every other test ran over a corpus that had rows.
    let scoped = super::restarts(
        &ClusterSnapshot {
            namespace_scope: Some("payments".to_string()),
            pods: vec![captured_pod("healthy")],
            ..corpus()
        },
        &[],
    );
    println!("{}", pane(&scoped));
    assert_eq!(
        scoped.title, "Containers in payments that keep restarting",
        "the pane is scoped, which is the whole premise"
    );
    assert_eq!(
        prose(&scoped),
        [format!(
            "Nothing here has restarted enough to matter. Every container serving right now in \
             payments has restarted {} or fewer times since its pod started.",
            RESTARTS_WARN - 1
        )]
    );
    // And unscoped it names no namespace at all — rule 6, the same one the title follows.
    let unscoped = super::restarts(
        &ClusterSnapshot {
            pods: vec![captured_pod("healthy")],
            ..corpus()
        },
        &[],
    );
    assert!(
        !prose(&unscoped)[0].contains(" in "),
        "an unscoped pane says nothing about scope: {:?}",
        prose(&unscoped)
    );
}

#[test]
fn no_row_says_how_the_last_run_ended() {
    // **The convention with no gate behind it** (D101, D85). `Terminated`'s `reason` and
    // `exit_code` are `pub`, so a raw `exit 137` in a row is reachable — and re-spelling
    // `rules.rs`' private translation in a second file is the divergence D85 exists to prevent.
    let cluster = restarts_corpus();
    let endings: BTreeSet<&str> = pairs(&cluster)
        .filter_map(|(_, c)| c.last_terminated.as_ref()?.reason.as_deref())
        .collect();
    // A derived list asserts it found something: an empty set sweeps nothing.
    assert!(
        endings.contains("RestartingAllContainers") && endings.len() >= 4,
        "the corpus carries the endings this pane must not repeat: {endings:?}"
    );
    let report = super::restarts(&cluster, &[]);
    assert!(
        !selectable(&report).is_empty(),
        "and the pane has rows to check, or this sweep reads nothing"
    );
    for s in strings_of(&report) {
        for ending in &endings {
            assert!(!s.contains(ending), "{s} spells how a run ended: {ending}");
        }
        assert!(
            !s.contains("exit"),
            "{s} spells an exit code, which is `rules.rs`' translation to make"
        );
    }
}

#[test]
fn the_title_names_a_namespace_only_where_there_is_one() {
    assert_eq!(
        super::restarts(&restarts_corpus(), &[]).title,
        "Containers that keep restarting"
    );
    assert_eq!(
        super::restarts(
            &ClusterSnapshot {
                namespace_scope: Some("payments".to_string()),
                ..restarts_corpus()
            },
            &[],
        )
        .title,
        "Containers in payments that keep restarting",
        "a scope narrows the list; it never switches the check off"
    );
}

#[test]
fn what_alerts_is_showing_changes_nothing_here() {
    // **`findings` is unread on purpose** and not for Capacity's reason: the row's claim is
    // narrower than a card's — count and age, nothing about current health — so there is nothing
    // to reconcile.
    //
    // **Measured, and it is a fact about this corpus rather than a promise**: `analyze` returns no
    // finding at all for the five containers this pane draws, because `doing_its_job` is the
    // suppressor rules 2, 5 and 6 already share and it is the same predicate the pane filters on.
    // The producer still does not subtract the slice, which is what keeps that from becoming a
    // dependency.
    let cluster = restarts_corpus();
    let findings = analyze(&cluster);
    let drawn = drawable(&cluster);
    assert_eq!(drawn.len(), 5);
    for (pod, _) in &drawn {
        let name = pod.rsplit('/').next().expect("a qualified name has a tail");
        assert!(
            !findings.iter().any(|f| f.object.name == name),
            "{name} carries a card, so this comment's measurement is stale: {:?}",
            findings.iter().map(|f| &f.object.name).collect::<Vec<_>>()
        );
    }
    // And the slice is genuinely non-empty, or the loop above swept nothing.
    assert!(
        findings.iter().any(|f| f.object.name == "broken-probe0"),
        "the corpus does produce cards — for the containers this pane refuses"
    );
    assert_eq!(
        super::restarts(&cluster, &findings),
        super::restarts(&cluster, &[])
    );
}

#[test]
fn nothing_on_this_pane_badges_or_says_it_could_not_run() {
    // No badge: the count of qualifying containers only grows — a settled restart from a node
    // reboot stays in the tally until its pod is replaced — so the badge would read nonzero
    // permanently. No `NotComputed`: this reads pod data alone, which is already watched.
    for cluster in [
        restarts_corpus(),
        ClusterSnapshot {
            namespace_scope: Some("payments".to_string()),
            ..restarts_corpus()
        },
        ClusterSnapshot {
            pods: Vec::new(),
            ..corpus()
        },
    ] {
        let report = super::restarts(&cluster, &[]);
        assert!(
            !report.rows.is_empty(),
            "every one of these panes says something, the empty one in its own words"
        );
        assert_eq!(report.badge, None);
        assert!(not_computed(&report).is_empty());
    }
}
