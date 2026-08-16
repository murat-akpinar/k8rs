//! `rules.rs` § THE WORKLOAD RULES — its tests (NOTES § D91).

use super::*;

// --- THE WORKLOAD RULES, AGAINST THE COMMITTED CAPTURES ---
//
// The W-series is the one family whose subject is not a pod, and both positives come off the
// same kind cluster as everything else: `scripts/cluster.sh` puts a `deny-all-pods`
// ResourceQuota in `k8rs-quota`, so `broken-quota`'s ReplicaSet was refused every pod it
// asked for (W1) and its Deployment then timed out waiting for them (W2). Both conditions
// are in the committed captures, `ProgressDeadlineExceeded` included.
//
// **What no capture holds is the quiet half of W2.** Its suppression clause needs a
// Deployment that timed out *and* has a failing pod under it — and on this cluster the one
// Deployment that timed out has no pods at all, which is the entire point of it. Those go
// through the one-field-on-a-decoded-copy technique the rest of this file uses (NOTES § D40,
// § D53), and each names why the 2026-08-13 trip could not bring the object.
//
// **The negatives are the half that matters again**, and for a new reason: W2's gate is a
// *query over the findings already made*, so it can fail in two opposite directions — a
// second card for one problem, or a Deployment that goes silent because something unrelated
// fired. Both directions are asserted below, with the link deliberately cut in one of them.

/// Every Deployment in `deployments.json`, decoded — six workloads, of which exactly one has
/// given up.
fn captured_deployments() -> Vec<WorkloadSnapshot> {
    items::<Deployment>("deployments")
        .into_iter()
        .map(Into::into)
        .collect()
}

/// One committed ReplicaSet capture, decoded. `just fixtures` writes each of these as a
/// `kubectl get -A` List, so they all arrive through [`items`].
fn captured_replicasets(name: &str) -> Vec<WorkloadSnapshot> {
    items::<ReplicaSet>(name)
        .into_iter()
        .map(Into::into)
        .collect()
}

/// One captured ReplicaSet with fields moved — [`capture_but`]'s counterpart for the object W1 is
/// about. Every one of these captures is a `kubectl get -A` List whose first item is the one the
/// W tests are written around. The committed JSON is never touched (NOTES § D53).
fn replicaset_but(name: &str, edit: impl FnOnce(&mut ReplicaSet)) -> ReplicaSet {
    let mut object: ReplicaSet = serde_json::from_value(fixture(name)["items"][0].clone())
        .unwrap_or_else(|e| panic!("{name}.json's first item is not a ReplicaSet: {e}"));
    edit(&mut object);
    object
}

/// One Deployment out of `deployments.json` with one field moved — [`capture_but`]'s
/// counterpart for the object W2 is about. The committed JSON is never touched (NOTES § D53).
fn deployment_but(name: &str, edit: impl FnOnce(&mut Deployment)) -> WorkloadSnapshot {
    let mut object: Deployment =
        serde_json::from_value(captured_item(&fixture("deployments"), name).clone())
            .unwrap_or_else(|e| panic!("{name} is not a Deployment in deployments.json: {e}"));
    edit(&mut object);
    WorkloadSnapshot::from(object)
}

/// The `ReplicaFailure` entry of a captured ReplicaSet, to be written through — the one condition
/// W1 reads, and the one every `quota-replicasets` copy below moves.
fn replicaset_failure(r: &mut ReplicaSet) -> &mut ReplicaSetCondition {
    r.status
        .as_mut()
        .and_then(|s| s.conditions.as_mut())
        .into_iter()
        .flatten()
        .find(|c| c.type_ == "ReplicaFailure")
        .expect("the capture carries a ReplicaFailure condition")
}

/// One condition of a captured Deployment, to be written through — [`pod_condition`]'s twin.
fn deployment_condition<'a>(d: &'a mut Deployment, type_: &str) -> &'a mut DeploymentCondition {
    d.status
        .as_mut()
        .and_then(|s| s.conditions.as_mut())
        .into_iter()
        .flatten()
        .find(|c| c.type_ == type_)
        .unwrap_or_else(|| panic!("the capture carries no {type_} condition to edit"))
}

/// **The `Progressing` condition a Deployment carries once its deadline has passed**, written onto
/// a decoded copy.
///
/// **Status, reason, message and stamp move together, and that is not tidiness.** Upstream writes
/// all four in one `NewDeploymentCondition` call, so moving only the reason produces a card that
/// says a rollout gave up and then quotes a controller saying it is still progressing — a shape no
/// cluster emits, and a test built on it proves nothing about the real one. The first draft of
/// these tests did exactly that, and only reading the printed card showed it.
///
/// **`lastTransitionTime` is the fourth, and it was the one left behind.** `Finding::timestamp`
/// reads that field, so a copy that kept the old stamp dated a dead rollout by the minute it was
/// last *progressing* — D69's wrong-field shape one step removed.
///
/// All four are `deployments.json`'s own, and the wording is asserted here so the synthesis cannot
/// drift from what the cluster actually writes (NOTES § D40, § D53).
fn timed_out(d: &mut Deployment, replicaset: &str) {
    let deployments = fixture("deployments");
    let captured = captured_condition(captured_item(&deployments, "broken-quota"), "Progressing");
    let wording = captured_str(captured, &["message"]);
    assert!(
        wording.ends_with("has timed out progressing."),
        "the wording below is the capture's, not this test's — and the capture now says {wording:?}"
    );
    let stamp = captured_time(captured, &["lastTransitionTime"]);
    let c = deployment_condition(d, "Progressing");
    c.status = "False".to_string();
    c.reason = Some("ProgressDeadlineExceeded".to_string());
    c.message = Some(format!(
        "ReplicaSet \"{replicaset}\" has timed out progressing."
    ));
    c.last_transition_time = Some(stamp);
}

/// **A captured pod hung off `broken-owned`'s ReplicaSet** — the one field moved for every
/// suppression case below, because this cluster's other pods have no controller and W2's
/// suppression is a walk up the chain from one (NOTES § D53).
///
/// **The uid is the captured ReplicaSet's**, not a fresh one: [`ObjectId`] compares on identity,
/// so a synthesized uid leaves the chain unwalkable and the suppression under test unreachable —
/// the test would then pass because nothing *could* suppress rather than because nothing should.
fn adopted_by_broken_owned(p: &mut Pod) {
    p.metadata.owner_references = Some(vec![OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "ReplicaSet".to_string(),
        name: "broken-owned-7bdb7645c8".to_string(),
        uid: captured_str(
            &fixture("owned-replicasets")["items"][0],
            &["metadata", "uid"],
        )
        .to_string(),
        controller: Some(true),
        block_owner_deletion: Some(true),
    }]);
}

/// [`pods_at`] with the workload list filled in — the snapshot a W rule is actually handed.
fn with_workloads(pods: Vec<PodSnapshot>, workloads: Vec<WorkloadSnapshot>) -> ClusterSnapshot {
    ClusterSnapshot {
        workloads,
        ..pods_at(pods, now())
    }
}

/// **W1's positive, and the copy of its own condition that must not become a second card.**
///
/// The Deployment controller mirrors `ReplicaFailure` up onto the Deployment — the identical
/// sentence is on `broken-quota` in `deployments.json` — so the kind gate in the rule is
/// load-bearing, and this is the assertion that would notice if it were dropped.
#[test]
fn the_replicaset_that_was_refused_says_so_in_the_api_servers_own_words() {
    let mut everything = captured_replicasets("quota-replicasets");
    everything.extend(captured_replicasets("healthy-replicasets"));
    everything.extend(captured_deployments());
    let all = analyze(&with_workloads(Vec::new(), everything));
    show(&all);

    let card = only(&all, "broken-quota-59654c756", "refused to create");
    assert_eq!(
        card.object.kind,
        ObjectKind::ReplicaSet,
        "what the rule looked at is the ReplicaSet, which is the object that was refused"
    );
    assert_eq!(card.owner.kind, ObjectKind::Deployment);
    assert_eq!(
        card.owner.name, "broken-quota",
        "and it files under the name the user deployed, never the hashed one (D28)"
    );
    assert_eq!(
        card.severity,
        Severity::Critical,
        "not one pod of this workload exists, so this is an outage and not a change that \
         failed to land"
    );

    let raw = fixture("quota-replicasets");
    let failure = captured_condition(&raw["items"][0], "ReplicaFailure");
    assert!(
        card.evidence.contains(captured_str(failure, &["message"])),
        "verbatim means verbatim (D37): the quota's own refusal is the whole diagnosis, and \
         a paraphrase of it sends the reader looking for a broken image — {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("exceeded quota: deny-all-pods"),
        "and the message the capture still carries has to be the one that names the quota: {}",
        card.evidence
    );
    assert!(
        card.evidence.starts_with("0 of 1 pod ready"),
        "and the card carries a count, because a dot with no number cannot tell *all three \
         refused* from *one of three* — sourced from the Deployment the header names, which \
         is the same object the band was read off (D82): {}",
        card.evidence
    );
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&captured_time(failure, &["lastTransitionTime"])),
        "the age is that condition's own stamp — `Available` sits one entry away in the \
         same flat list (D69)"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl get replicaset broken-quota-59654c756 -n k8rs-quota -o yaml"),
        "`kubectl describe rs` prints Type/Status/Reason and drops the message this whole \
         card is made of, so the teaching command is the one that shows it (D46, D71)"
    );

    assert_eq!(
        all.len(),
        1,
        "one refusal, one card: the healthy ReplicaSet carries no such condition, the \
         Deployment carries a mirrored copy of the failing one and must not draw a second, \
         and the rollout that timed out because of it is W2's suppressed case: {:?}",
        titles(&all)
    );
}

/// **W1's other band — the refused *rollout*, where the ReplicaSet's own counters are a
/// service that is 100% up** (NOTES § D82).
///
/// A refused rollout leaves the **new** ReplicaSet reading `0 of 1` for as long as the refusal
/// stands, while the **old** one carries every request — and there are no pods under the new
/// one, so nothing else on the screen contradicts a CRITICAL drawn off it. A band read from the
/// refused object pages for a service that never went down, which is the card that teaches a
/// user to mute the tool in week one.
///
/// **The ReplicaSet here is the committed capture, untouched**: `spec.replicas: 1`,
/// `status.replicas: 0`, no `readyReplicas` at all. Only the Deployment above it is moved, and
/// as one coherent story — three wanted, two of them still answering, the third refused
/// **The 2026-08-13 trip did not bring the other half, and this is why:** `scripts/broken.yaml`'s
/// quota denies every pod in `k8rs-quota` from the moment the namespace exists, so nothing there
/// is ever running to be scaled *up*. That shape needs a quota tightened under a Deployment that
/// is already serving, which is a second sequence rather than a second manifest.
///
/// The control is the same ReplicaSet under the captured Deployment, which really is down.
#[test]
fn a_refused_rollout_is_amber_while_the_deployment_above_it_is_still_serving() {
    let refused = captured_replicasets("quota-replicasets");
    let raw = fixture("quota-replicasets");
    assert_eq!(
        raw["items"][0]["status"]["readyReplicas"],
        serde_json::Value::Null,
        "the capture has to still be the shape this test is about — the refused ReplicaSet \
         reports no ready pods of its own"
    );

    let still_up = deployment_but("broken-quota", |d| {
        d.spec
            .as_mut()
            .expect("a captured Deployment has a spec")
            .replicas = Some(3);
        d.status
            .as_mut()
            .expect("a captured Deployment has a status")
            .ready_replicas = Some(2);
    });
    let all = analyze(&with_workloads(
        Vec::new(),
        refused.iter().cloned().chain([still_up]).collect(),
    ));
    show(&all);

    let card = only(&all, "broken-quota-59654c756", "refused to create");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "two of the three pods are still answering, and they answer under the Deployment \
         rather than under the ReplicaSet that was refused — a card that pages for this is a \
         card nobody reads twice (D82)"
    );
    assert!(
        card.evidence.starts_with("2 of 3 pods ready"),
        "and the count is that same Deployment's, not the refused ReplicaSet's `0 of 1` — \
         one object per line, and it is the object in the header (D82): {}",
        card.evidence
    );
    assert_eq!(
        all.len(),
        1,
        "and the Deployment's own timeout is still the same refusal, not a second card: {:?}",
        titles(&all)
    );

    // The control, and the half that makes the assertion above able to fail: the captured
    // Deployment reports nothing ready at all, and the identical ReplicaSet then pages.
    let down = analyze(&with_workloads(
        Vec::new(),
        refused
            .into_iter()
            .chain(
                captured_deployments()
                    .into_iter()
                    .filter(|w| w.id.name == "broken-quota"),
            )
            .collect(),
    ));
    show(&down);
    assert_eq!(
        only(&down, "broken-quota-59654c756", "refused to create").severity,
        Severity::Critical,
        "nothing of this workload is serving, so the same ReplicaSet is an outage"
    );
}

/// **An owner that is named and absent is *unknown*, not *down*** (NOTES § D82).
///
/// An Argo `Rollout` owns its ReplicaSets **directly**, with no Deployment in between, and
/// invariant 12 forbids decoding a CR — so [`ObjectKind::from_api`] resolves that owner to an
/// `Other(_)` which nothing will ever put in `snapshot.workloads`. A canary ReplicaSet a quota
/// refused, while the stable ReplicaSet beside it serves 100% of the traffic, is then a lookup
/// that misses. Falling back to the ReplicaSet the rule looked at reads the `readyReplicas: 0` it
/// has **by definition of having been refused**, and pages CRITICAL for a service that never went
/// down — with no pods under that ReplicaSet, so nothing else on the screen contradicts it. A 403
/// on `deployments` with `replicasets` still readable is the second way in, and it needs no CRD at
/// all.
///
/// **The 2026-08-13 trip did not bring it, and no trip will:** an owner k8rs does not decode means
/// a third-party controller installed on the fixture cluster, which is a cluster change rather
/// than a fixture — the argument
/// `the_api_group_decides_which_kind_an_owner_reference_names` already records against
/// OpenKruise. So the reference is moved on a decoded copy (NOTES § D53).
///
/// The control is the same untouched capture under an owner the snapshot *does* carry, which
/// really is down — the half that makes the band above able to fail.
#[test]
fn a_refusal_under_an_owner_the_snapshot_cannot_name_is_not_called_an_outage() {
    let canary = replicaset_but("quota-replicasets", |r| {
        r.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "argoproj.io/v1alpha1".to_string(),
            kind: "Rollout".to_string(),
            name: "broken-quota".to_string(),
            uid: "b7e41f02-9c6d-4a18-8f3b-5d2ac70e1946".to_string(),
            controller: Some(true),
            block_owner_deletion: Some(true),
        }]);
    });
    let canary = WorkloadSnapshot::from(canary);
    assert_eq!(
        canary.owner.kind,
        ObjectKind::Other("Rollout.argoproj.io".to_string()),
        "the copy only proves something while the owner really is a kind no watch decodes \
         and no snapshot can carry"
    );
    assert!(
        canary.ready.unwrap_or(0) == 0 && canary.desired == Some(1),
        "and while the ReplicaSet the rule looked at really does read `0 of 1` — which is \
         the number a fallback would page on"
    );

    let all = analyze(&with_workloads(Vec::new(), vec![canary]));
    show(&all);
    let card = only(&all, "broken-quota-59654c756", "refused to create");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "the stable version is serving every request and this tool cannot see it — an \
         unresolvable owner is unknown, and unknown is not an outage (D82)"
    );
    assert!(
        !card.evidence.contains(" ready"),
        "and with no resolvable workload there is no count to print: a number about the \
         wrong object is worse than none: {}",
        card.evidence
    );
    assert!(
        card.evidence.contains("exceeded quota: deny-all-pods"),
        "while the refusal itself, which is the whole diagnosis, still reaches the card: {}",
        card.evidence
    );

    // The control — the same ReplicaSet under the owner it was really captured with, which
    // has nothing ready at all.
    let down = analyze(&with_workloads(
        Vec::new(),
        captured_replicasets("quota-replicasets")
            .into_iter()
            .chain(
                captured_deployments()
                    .into_iter()
                    .filter(|w| w.id.name == "broken-quota"),
            )
            .collect(),
    ));
    show(&down);
    assert_eq!(
        only(&down, "broken-quota-59654c756", "refused to create").severity,
        Severity::Critical,
        "an owner the snapshot *can* name, with nothing serving under it, is the outage"
    );
}

/// **A condition with no message leaves no blank row** — the one branch of [`controller_said`]
/// that no capture reaches.
///
/// Both W rules quote a condition and both fire on conditions upstream always writes a message
/// with, so this branch exists for a server that does not. `message` is `+optional` on
/// `ReplicaSetCondition`, so the object below is one the API allows; what it must not produce is
/// an evidence line that is the empty string, which draws as a blank row under the title and
/// reads as a card that failed to load rather than one with nothing to quote.
#[test]
fn a_condition_with_nothing_to_quote_still_says_something() {
    let object = replicaset_but("quota-replicasets", |r| {
        replicaset_failure(r).message = None;
    });

    let all = analyze(&with_workloads(Vec::new(), vec![object.into()]));
    show(&all);
    let card = only(&all, "broken-quota-59654c756", "refused to create");
    assert!(
        !card.evidence.trim().is_empty(),
        "an empty evidence line is a blank row on the card, and a reader cannot tell it from \
         a card that failed to draw"
    );
    assert!(
        !card.evidence.contains("the reason Kubernetes gave"),
        "and it may not open a quote it has nothing to put in: {}",
        card.evidence
    );
}

/// **`FailedDelete` is not W1's condition, and the filter that keeps it out is a ruling**
/// (NOTES § D82).
///
/// Upstream's `replica_set_utils.go` writes `ReplicaFailure` under two reasons. The other one is
/// a scale-**down** the API refused — a webhook on `DELETE pods` whose backend is unreachable,
/// which is Gatekeeper, Kyverno or any admission controller having a bad day. Every sentence on
/// this card is wrong about it: nothing was refused creation, the service is up, and the counters
/// would read *"2 of 1 pod"*. **No card in v1**; a card for it is a new rule (invariant 13).
///
/// **The 2026-08-13 trip did not bring it, and this is why:** a refused scale-down needs a
/// validating admission webhook, which is a second workload deployed into the fixture cluster for
/// no purpose but to reject one request — a cluster change rather than a fixture. So the whole
/// story is written onto a decoded copy at once — reason, message and both counters
/// (NOTES § D53). They move **together** because upstream writes them together: a
/// creation refusal's message left standing under a deletion's reason is a shape no cluster
/// emits, and a test built on it proves nothing about the real one.
///
/// **The control is the committed capture, untouched**, and it comes first — a W1 that had
/// stopped firing at all would otherwise pass this test by saying nothing about everything.
#[test]
fn a_scale_down_the_api_refused_is_not_a_card_about_creating_pods() {
    let capture = replicaset_but("quota-replicasets", |_| {});
    let created = analyze(&with_workloads(Vec::new(), vec![capture.into()]));
    show(&created);
    only(&created, "broken-quota-59654c756", "refused to create");

    // The scale-down the API refused: one wanted, two still running, and the delete of the
    // second one rejected by a webhook whose backend is down.
    let object = replicaset_but("quota-replicasets", |r| {
        r.spec
            .as_mut()
            .expect("a captured ReplicaSet has a spec")
            .replicas = Some(1);
        let status = r
            .status
            .as_mut()
            .expect("a captured ReplicaSet has a status");
        status.ready_replicas = Some(2);
        status.replicas = 2;
        let c = replicaset_failure(r);
        c.reason = Some("FailedDelete".to_string());
        c.message = Some(
            "unable to delete pods: Internal error occurred: failed calling webhook \
             \"deny.example.com\": dial tcp 10.96.0.9:443: connect: connection refused"
                .to_string(),
        );
    });

    let deleted = analyze(&with_workloads(Vec::new(), vec![object.into()]));
    show(&deleted);
    nothing(
        &deleted,
        "a scale-down the API refused is not a card titled \"refused to create the pods\", and \
         it is not one reading \"2 of 1 pod\" either — the service is up and v1 says nothing",
    );
}

/// **W2's positive and every negative the capture can offer**, over the Deployments alone —
/// no ReplicaSets and no pods, so nothing is there to suppress anything and each Deployment
/// answers for itself.
///
/// **The negative that answers the question the box left open** is `broken-owned`: it is
/// short of pods — one wanted, none ready — and its `Progressing` never reached the
/// deadline, so it says nothing at all. Every `kubectl apply` is short of pods for a while,
/// and `progressDeadlineSeconds` is Kubernetes' own answer to when that stops being normal;
/// a third rule reading the shortfall on its own would fire on every rollout in progress.
#[test]
fn the_rollout_that_ran_out_of_time_is_the_only_deployment_that_says_anything() {
    let deployments = captured_deployments();
    let all = analyze(&with_workloads(Vec::new(), deployments.clone()));
    show(&all);

    let card = only(&all, "broken-quota", "gave up");
    assert_eq!(card.object.kind, ObjectKind::Deployment);
    assert_eq!(
        card.owner, card.object,
        "nothing controls a Deployment, so it is its own card"
    );
    assert_eq!(card.severity, Severity::Critical);

    let raw = fixture("deployments");
    let quota = captured_item(&raw, "broken-quota");
    let progressing = captured_condition(quota, "Progressing");
    assert!(
        card.evidence
            .contains(captured_str(progressing, &["message"])),
        "the controller's own sentence, verbatim (D37) — it names the revision that timed \
         out, which is where the reader goes next: {}",
        card.evidence
    );
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&captured_time(progressing, &["lastTransitionTime"])),
        "**the wrong stamp is one entry away and it draws**: this Deployment's \
         `ReplicaFailure` transitioned a minute earlier and `Available` earlier still (D69)"
    );
    assert_ne!(
        card.timestamp,
        Some(captured_time(
            captured_condition(quota, "ReplicaFailure"),
            &["lastTransitionTime"]
        )),
        "and the capture is what makes that assertion mean something: the two stamps differ"
    );
    assert_eq!(
        card.kubectl_cmd.as_deref(),
        Some("kubectl get deployment broken-quota -n k8rs-quota -o yaml"),
        "`kubectl describe deployment` reduces its conditions to Type/Status/Reason, and \
         prints `available` where this card counts `ready`"
    );
    assert!(
        card.evidence.starts_with("0 of 1 pod ready"),
        "**the counter has to follow the band** (D82): this Deployment is on its first \
         revision — `observedGeneration: 1`, no old version anywhere — so a red card reading \
         `0 of 1 pod on the new version` says an old one is still up, which is the opposite \
         triage decision at 3am: {}",
        card.evidence
    );

    let owned = deployments
        .iter()
        .find(|w| w.id.name == "broken-owned")
        .expect("the capture holds a Deployment that is short of pods and has not given up");
    println!(
        "broken-owned: desired={:?} ready={:?} progressing={:?}",
        owned.desired,
        owned.ready,
        condition(&owned.conditions, "Progressing").and_then(|c| c.reason.as_deref())
    );
    assert!(
        short_of_pods(owned),
        "the negative only proves something while this Deployment is genuinely short"
    );
    assert_eq!(
        all.len(),
        1,
        "one Deployment gave up and five did not — a rollout still running, one short of \
         pods that never reached its deadline, and three that are simply fine: {:?}",
        titles(&all)
    );
}

/// **W2 stands down when W1 has already said why** — the two cards that would otherwise
/// describe one refusal, one naming the quota and one naming the clock it ran out.
///
/// The control comes first and is the half that makes this test able to fail: alone, the
/// same Deployment draws the card.
#[test]
fn the_rollout_card_stands_down_when_the_replicaset_has_already_said_why() {
    let object: Deployment = serde_json::from_value(fixture("quota-deployment"))
        .expect("quota-deployment.json is a Deployment");
    let deployment = WorkloadSnapshot::from(object);

    let alone = analyze(&with_workloads(Vec::new(), vec![deployment.clone()]));
    show(&alone);
    only(&alone, "broken-quota", "gave up");

    let both: Vec<WorkloadSnapshot> = captured_replicasets("quota-replicasets")
        .into_iter()
        .chain([deployment])
        .collect();
    let all = analyze(&with_workloads(Vec::new(), both));
    show(&all);
    only(&all, "broken-quota-59654c756", "refused to create");
    assert_eq!(
        all.len(),
        1,
        "the refusal is on the list, so the timeout it caused is not a second card — two \
         findings for one problem is how the list stops being believable (D28): {:?}",
        titles(&all)
    );
}

/// **The chain D28 describes, walked: a pod's crash loop explains its Deployment's rollout,
/// two steps up.** The pod's owner is a ReplicaSet and W2's subject is the Deployment above
/// it, so the link runs through `WorkloadSnapshot::owner` on the ReplicaSet — which is why a
/// snapshot that is missing the ReplicaSet cannot walk it, and the third case below is what
/// that does.
///
/// **The 2026-08-13 trip did not bring it, and `deployments.json` says why.** `broken-owned` ran
/// for over an hour with its only pod in `CrashLoopBackOff`, and its `Progressing` condition still
/// reads `True / NewReplicaSetAvailable` beside `Available: False` — the deadline never trips once
/// the ReplicaSet has managed to *create* its pod, whatever that pod then does. So `broken-owned`
/// is that Deployment in every respect but that one reason, and it is moved on a decoded copy
/// (NOTES § D40, § D53).
#[test]
fn a_crash_loop_two_steps_under_the_deployment_is_the_same_problem_and_not_a_second_card() {
    let gave_up = deployment_but("broken-owned", |d| timed_out(d, "broken-owned-7bdb7645c8"));
    let pods: Vec<PodSnapshot> = items::<Pod>("owned-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();
    let replicaset = captured_replicasets("owned-replicasets");
    let chain: Vec<WorkloadSnapshot> = replicaset
        .iter()
        .cloned()
        .chain([gave_up.clone()])
        .collect();

    // The pod's own card is the premise of every silence below, and it is asked for without
    // naming the rule that draws it: `scripts/cluster.sh` § `[owned]` certifies the capture in
    // either half of the backoff loop, so the title is rule 1's or rule 5's depending on the
    // trip (NOTES § D114).
    let name = owned_pod_name();
    let says_something = |all: &[Finding]| all.iter().any(|f| f.object.name == name);
    let whole = analyze(&with_workloads(pods.clone(), chain.clone()));
    show(&whole);
    assert!(
        says_something(&whole),
        "the pod has to be drawing a card, or the suppression below is about nothing: {:?}",
        titles(&whole)
    );
    assert!(
        whole.iter().all(|f| !f.title.contains("gave up")),
        "the pod under it already says why the rollout never finished: {:?}",
        titles(&whole)
    );

    // Control one — the same three objects minus the pod. Nothing explains the shortfall, so
    // the card is drawn, which is what makes the silence above a suppression rather than a
    // rule that never fires.
    let no_pods = analyze(&with_workloads(Vec::new(), chain));
    show(&no_pods);
    only(&no_pods, "broken-owned", "gave up");

    // Control two — the pod is there and its ReplicaSet is not, so the chain cannot be
    // walked. **W2 draws, and that is the direction this is built to fail in**: an owner that
    // cannot be resolved is unknown, not related, and reading unknown as related would let
    // any pod's crash loop anywhere in the snapshot silence every W2 in it.
    let no_link = analyze(&with_workloads(pods, vec![gave_up]));
    show(&no_link);
    only(&no_link, "broken-owned", "gave up");
    assert!(
        says_something(&no_link),
        "and the pod is still saying it — the two cards stand together here, which is the \
         direction an unresolvable owner has to fail in: {:?}",
        titles(&no_link)
    );
}

/// **A card about a pod that is answering requests does not explain why a rollout has none**
/// (NOTES § D28, § D82).
///
/// D28's clause is *"no pod-level finding already **explains** the shortfall"*, and every finding
/// under the owner is not that list. `restarts.json` is a pod that is **Running and Ready** with
/// three restarts, and rule 5's own sentence about it says *"it is serving now"* — so a W2 it
/// silenced would be a Critical hidden behind a card that says nothing is wrong. Rule 8's
/// hostPath card, rule 12's and rule 14's are the same shape.
///
/// **The two directions are asserted against the same Deployment**, so the difference between
/// them is the pod and nothing else. **The 2026-08-13 trip did not bring it:** a rollout only
/// times out while its pods are *failing*, so a serving pod under a dead rollout is a pair the
/// cluster will not hold at once — this cluster's serving pod has no controller at all, and the
/// `ownerReference` is the one field moved (NOTES § D53).
#[test]
fn a_pod_that_is_serving_does_not_explain_a_rollout_that_has_no_pods() {
    let gave_up = deployment_but("broken-owned", |d| timed_out(d, "broken-owned-7bdb7645c8"));
    let replicaset = captured_replicasets("owned-replicasets");
    let chain: Vec<WorkloadSnapshot> = replicaset.iter().cloned().chain([gave_up]).collect();

    let serving = capture_but("restarts", adopted_by_broken_owned);
    assert!(
        serving.ready.as_ref().is_some_and(|c| c.status == "True"),
        "the whole test is that this pod is answering requests while it restarts"
    );
    // **Without this the serving half passes for the wrong reason.** A synthesized owner whose
    // uid did not match the captured ReplicaSet's would leave the chain unwalkable, and W2 would
    // then draw because nothing *could* suppress it rather than because nothing should.
    assert_eq!(
        serving.owner, replicaset[0].id,
        "the adopted pod has to actually hang off the ReplicaSet under the Deployment, or the \
         suppression this test is about was never reachable"
    );
    // Read inside this pod's run, because rule 5's serving card ages out at the pin and the
    // whole test is what that card does *not* suppress (NOTES § D100).
    let moment = while_its_cards_draw(&serving);
    let with_serving = analyze(&ClusterSnapshot {
        now: moment.clone(),
        ..with_workloads(vec![serving], chain.clone())
    });
    show_at(&with_serving, &moment);
    only(&with_serving, "broken-restarts", "restarted 3 times");
    only(&with_serving, "broken-owned", "gave up");

    // The other direction, and the one D28 is actually about: a pod that is *not* serving is
    // the reason the rollout has no pods, and the two cards would be one problem.
    let failing: Vec<PodSnapshot> = items::<Pod>("owned-pods")
        .into_iter()
        .map(PodSnapshot::from)
        .collect();
    let with_failing = analyze(&with_workloads(failing, chain));
    show(&with_failing);
    // The premise, and it is asked without naming a rule: the pod has to draw *something*, or
    // the silence below is a rollout card nothing was suppressing. Which card it is depends on
    // where in the backoff loop the capture landed, and `scripts/cluster.sh` § `[owned]`
    // certifies both faces (NOTES § D114).
    let name = owned_pod_name();
    assert!(
        with_failing.iter().any(|f| f.object.name == name),
        "the failing pod is what explains the shortfall, so it has to be saying so: {:?}",
        titles(&with_failing)
    );
    assert!(
        with_failing.iter().all(|f| !f.title.contains("gave up")),
        "a pod that is not ready is exactly the shortfall, so this stays one card: {:?}",
        titles(&with_failing)
    );
}

/// **The third shape `explains_a_shortfall` has to answer for: a pod with no `Ready` condition at
/// all** (NOTES § D29, § D82).
///
/// The two cases beside this one feed the other two framings — `owned-pods` is `Ready: False` and
/// `restarts` is `Ready: True` — so the arm that reads a pod carrying no such condition was only
/// ever asserted by reading the source. `broken-pending` is that pod: no node accepted it, so the
/// kubelet never wrote a `Ready` line at all, and `pending.json`'s decode is asserted `None`
/// elsewhere in this file. It is also the shape that matters most rather than the leftover — an
/// unschedulable pod is the most common true explanation of a rollout that never finished.
///
/// **The 2026-08-13 trip did not bring it:** `broken-pending` is refused by a `nodeSelector` no
/// node carries, and `scripts/broken.yaml` puts that selector on a bare pod rather than under a
/// Deployment — so this cluster's unschedulable pod has no controller at all, and the
/// `ownerReference` is the one field moved ([`adopted_by_broken_owned`], NOTES § D53).
///
/// The control is the same chain with no pod, which draws the card.
#[test]
fn a_pod_no_node_would_take_explains_the_rollout_that_is_waiting_for_it() {
    let waiting = capture_but("pending", adopted_by_broken_owned);
    assert_eq!(
        waiting.ready, None,
        "the framing this test exists for: not `Ready: False` but no Ready condition at all, \
         which is what the kubelet writes for a pod it never saw"
    );
    let replicaset = captured_replicasets("owned-replicasets");
    assert_eq!(
        waiting.owner, replicaset[0].id,
        "and the adopted pod has to really hang off the ReplicaSet under the Deployment, or \
         the suppression this test is about was never reachable"
    );

    let gave_up = deployment_but("broken-owned", |d| timed_out(d, "broken-owned-7bdb7645c8"));
    let chain: Vec<WorkloadSnapshot> = replicaset.into_iter().chain([gave_up]).collect();

    let all = analyze(&with_workloads(vec![waiting], chain.clone()));
    show(&all);
    only(&all, "broken-pending", "will take this pod");
    assert!(
        all.iter().all(|f| !f.title.contains("gave up")),
        "the pod nothing would schedule *is* the shortfall, so this is one card and not two: \
         {:?}",
        titles(&all)
    );

    // The control — the same Deployment and ReplicaSet with no pod under them. Nothing
    // explains the shortfall, so the card is drawn, which is what makes the silence above a
    // suppression rather than a rule that never fires.
    let alone = analyze(&with_workloads(Vec::new(), chain));
    show(&alone);
    only(&alone, "broken-owned", "gave up");
}

/// **The hop up the chain is one hop off a ReplicaSet, never off whatever workload was found**
/// (NOTES § D82).
///
/// A Deployment that carries a controlling `ownerReference` is ordinary — ECK, OLM and most
/// operators that emit Deployments set one — and a hop taken off it lands on the operator's CR.
/// W1's finding then resolves to the CR while W2's Deployment resolves to itself, the suppression
/// stops matching, and one quota refusal draws two Criticals whose second card is headed with a
/// name its own `$ kubectl` line does not mention.
///
/// **The 2026-08-13 trip did not bring it, and no trip will:** a Deployment emitted by an operator
/// means a third-party controller installed on the fixture cluster, which is a cluster change
/// rather than a fixture. So the reference is added on a decoded copy (NOTES § D53).
#[test]
fn a_deployment_an_operator_owns_is_still_where_the_chain_stops() {
    let owned = deployment_but("broken-quota", |d| {
        d.metadata.owner_references = Some(vec![OwnerReference {
            api_version: "example.com/v1".to_string(),
            kind: "TheOperatorsKind".to_string(),
            name: "the-operators-cr".to_string(),
            uid: "0f5b1c88-7a3e-42d1-9c0b-6e2f8a41d3b7".to_string(),
            controller: Some(true),
            block_owner_deletion: Some(true),
        }]);
    });
    assert_ne!(
        owned.owner, owned.id,
        "the copy only proves something while the Deployment really is owned by something else"
    );

    let all = analyze(&with_workloads(
        Vec::new(),
        captured_replicasets("quota-replicasets")
            .into_iter()
            .chain([owned])
            .collect(),
    ));
    show(&all);
    assert_eq!(
        all.len(),
        1,
        "one quota refusal is one card whether or not an operator owns the Deployment — a \
         second one would be headed `the-operators-cr` while the command under it names \
         `broken-quota`: {:?}",
        titles(&all)
    );
    only(&all, "broken-quota-59654c756", "refused to create");
}

/// **W2's amber band, and the shortfall `readyReplicas` cannot see** (NOTES § D82).
///
/// Whenever `maxUnavailable` resolves to **0** a RollingUpdate Deployment never removes its old
/// pods, so `status.readyReplicas` stays equal to `spec.replicas` for the whole of a failed
/// rollout. `ready < desired` is false there, on a Deployment whose own condition says it gave up,
/// which made this band unreachable in practice and W2 silent on the most common failed rollout
/// there is.
///
/// **`scripts/broken.yaml` sets it to 0 outright, and that is what this fixture exercises** — not
/// the 25% default rounding down to the same 0 at one, two or three replicas, which reaches this
/// arithmetic by the other road. The two are worth keeping apart: the default is why the hole is
/// common, the explicit setting is why it is in the capture.
///
/// **The counters here are the committed capture's, every one of them**: `spec.replicas: 2`,
/// `maxUnavailable: 0`, `readyReplicas: 2`, `replicas: 3`, `updatedReplicas: 1`. Only the
/// `Progressing` condition is moved, because `broken-rollout` was captured before its deadline
/// expired — one field, the one no cluster held still long enough to capture (NOTES § D40, § D53).
#[test]
fn a_rollout_stuck_behind_maxunavailable_zero_still_reports_its_shortfall() {
    let deployments = fixture("deployments");
    let raw = captured_item(&deployments, "broken-rollout");
    println!(
        "broken-rollout capture: spec.replicas={} maxUnavailable={} status={}",
        raw["spec"]["replicas"],
        raw["spec"]["strategy"]["rollingUpdate"]["maxUnavailable"],
        raw["status"]
    );
    assert_eq!(
        raw["status"]["readyReplicas"], raw["spec"]["replicas"],
        "this test is only about something while the capture is the shape that defeats \
         `ready < desired` — every pod the Deployment asked for is ready and the rollout is \
         still stuck"
    );

    let stalled = deployment_but("broken-rollout", |d| {
        timed_out(d, "broken-rollout-5967d47d5b");
    });
    let all = analyze(&with_workloads(Vec::new(), vec![stalled]));
    show(&all);

    let card = only(&all, "broken-rollout", "gave up");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "the two old pods are still answering: the change did not land, the service did \
         not go down"
    );
    assert!(
        card.evidence.starts_with("1 of 2 pods on the new version"),
        "and the counter on the card is the one that is actually short — `2 of 2 ready` is \
         true here and says nothing at all about a rollout that is dead: {}",
        card.evidence
    );

    // The same stuck rollout once the old pods have gone too: nothing is ready, one pod
    // exists on the new version. **The band flips and the counter has to flip with it** — a
    // red card reading `1 of 2 pods on the new version` says an old version is still serving
    // and leaves the number that justifies the band off the screen entirely (D82).
    let down = deployment_but("broken-rollout", |d| {
        d.status
            .as_mut()
            .expect("a captured Deployment has a status")
            .ready_replicas = Some(0);
        timed_out(d, "broken-rollout-5967d47d5b");
    });
    assert_eq!(
        (down.ready, down.updated, down.desired),
        (Some(0), Some(1), Some(2)),
        "one field moved, and the other two are the capture's"
    );
    let all = analyze(&with_workloads(Vec::new(), vec![down]));
    show(&all);
    let card = only(&all, "broken-rollout", "gave up");
    assert_eq!(card.severity, Severity::Critical);
    assert!(
        card.evidence.starts_with("0 of 2 pods ready"),
        "the number under a red band is the one the red band was read off: {}",
        card.evidence
    );
}

/// **The rollout of *one* replica, which is the size the other two counters are both blind to**
/// (NOTES § D82).
///
/// At `replicas: 1` upstream's `ResolveFenceposts` gives `maxSurge: 1, maxUnavailable: 0`: the new
/// ReplicaSet is scaled to exactly one and the old one is left at one. A second revision that
/// cannot start therefore reads `spec.replicas 1 · readyReplicas 1 · updatedReplicas 1` — the old
/// pod answers, one pod exists on the new template, and both of W2's original counters say the
/// workload is whole while its own condition says it gave up. This is the commonest Deployment
/// size there is.
///
/// **The counters here are `broken-owned`'s own, and it is captured with the two that matter**:
/// `spec.replicas: 1`, `updatedReplicas: 1`, `unavailableReplicas: 1`. Two are moved — the old
/// pod's `readyReplicas`, and the `status.replicas: 2` that goes with a surge of one, because a
/// capture where one pod exists and one is ready and one is unavailable is not a shape any cluster
/// emits (NOTES § D40, § D53). **The 2026-08-13 trip did not bring it:** `broken-rollout` is the
/// mid-rollout Deployment `scripts/broken.yaml` grows, and it runs two replicas — the one-replica
/// variant, where the surge and the shortfall land on the same single pod, is a manifest the file
/// does not have.
///
/// **The mitigation, so this is not read as bigger than it is:** in the ordinary version of this
/// shape the new pod has a card of its own — a bad image, a crash loop — and `explained` suppresses
/// W2 anyway. The hole is the one where the new pod produces no card at all, and it is narrow.
#[test]
fn a_rollout_of_one_replica_is_short_where_both_other_counters_read_whole() {
    let deployments = fixture("deployments");
    let raw = captured_item(&deployments, "broken-owned");
    println!(
        "broken-owned capture: spec={} status={}",
        raw["spec"], raw["status"]
    );
    assert_eq!(
        (
            raw["spec"]["replicas"].as_i64(),
            raw["status"]["updatedReplicas"].as_i64(),
            raw["status"]["unavailableReplicas"].as_i64(),
        ),
        (Some(1), Some(1), Some(1)),
        "three of the four counters are the capture's, and the test is only about something \
         while they are"
    );

    let stuck = deployment_but("broken-owned", |d| {
        let status = d
            .status
            .as_mut()
            .expect("a captured Deployment has a status");
        // The old pod, still answering — the half a first-revision capture cannot hold.
        status.ready_replicas = Some(1);
        // ...and the two pods that exist while it does, which is what a surge of one is.
        status.replicas = Some(2);
        timed_out(d, "broken-owned-7bdb7645c8");
    });
    assert_eq!(
        (stuck.desired, stuck.ready, stuck.updated, stuck.unavailable),
        (Some(1), Some(1), Some(1), Some(1)),
        "the shape this test is about: every counter W2 had before this one reads whole"
    );
    assert!(
        stuck.ready >= stuck.desired && stuck.updated >= stuck.desired,
        "neither of the two original arms of `short_of_pods` can see this rollout"
    );
    assert!(
        short_of_pods(&stuck),
        "and the third one has to, or W2 is silent on a dead rollout of the commonest \
         Deployment size there is (D82)"
    );

    let all = analyze(&with_workloads(Vec::new(), vec![stuck]));
    show(&all);
    let card = only(&all, "broken-owned", "gave up");
    assert_eq!(
        card.severity,
        Severity::Warn,
        "the old pod is still answering: the change did not land, the service did not go down"
    );
    assert!(
        card.evidence.starts_with("1 pod not answering"),
        "and the counter on the card is the only one that is short — `1 of 1 ready` and \
         `1 of 1 on the new version` are both true here and both say a rollout that is dead \
         has finished (D82): {}",
        card.evidence
    );
    assert_eq!(
        card.timestamp.as_ref(),
        Some(&captured_time(
            captured_condition(captured_item(&deployments, "broken-quota"), "Progressing"),
            &["lastTransitionTime"]
        )),
        "the card is dated by the moment the rollout gave up — the stamp that arrived with \
         the reason and the message, not the one this Deployment carried while it was still \
         progressing (D69)"
    );
    assert_ne!(
        card.timestamp,
        Some(captured_time(
            captured_condition(raw, "Progressing"),
            &["lastTransitionTime"]
        )),
        "and the capture is what makes that assertion mean something: the two stamps differ"
    );
}

/// **A workload that wants zero pods is not short of pods** (NOTES § D82).
///
/// Scaling a rollout that gave up down to zero is how someone stops the bleeding, and it leaves the
/// object in the one shape [`short_of_pods`]' third arm cannot reason its way out of.
/// `spec.replicas` is written by the user and `status.unavailableReplicas` by the controller, as
/// `sum(replicaset.spec.replicas) - availableReplicas` — never the Deployment's own
/// `spec.replicas` — so the watch delivers an explicit `0` beside the status of a moment ago, and
/// `ProgressDeadlineExceeded` is sticky enough to still be sitting on it. Before the `desired > 0`
/// gate that was a CRITICAL card reading *"1 pod not answering"* about a Deployment the user had
/// just deliberately turned off.
///
/// **One field moved on a decoded copy.** `broken-owned` is captured as the whole shape but for the
/// replica count — `unavailableReplicas: 1`, no `readyReplicas` at all — so only `spec.replicas`
/// moves, plus the `Progressing` condition every W2 test has to write (NOTES § D40, § D53).
/// **The 2026-08-13 trip did not bring it, and a trip cannot:** the shape exists only between the
/// `scale` call and the controller writing the status back, so capturing it means winning a race
/// against a controller — not something `just fixtures` can be asked to do repeatably.
#[test]
fn a_deployment_scaled_to_zero_is_not_short_of_the_pods_it_no_longer_wants() {
    let deployments = fixture("deployments");
    let raw = captured_item(&deployments, "broken-owned");
    assert_eq!(
        (
            raw["status"]["unavailableReplicas"].as_i64(),
            raw["status"]["readyReplicas"].as_i64(),
        ),
        (Some(1), None),
        "the capture carries the half that matters — a positive unavailable counter and no \
         ready one — and this test is only about something while it does"
    );

    let switched_off = deployment_but("broken-owned", |d| {
        d.spec
            .as_mut()
            .expect("a captured Deployment has a spec")
            .replicas = Some(0);
        timed_out(d, "broken-owned-7bdb7645c8");
    });
    println!(
        "scaled to zero: desired={:?} ready={:?} updated={:?} unavailable={:?}",
        switched_off.desired, switched_off.ready, switched_off.updated, switched_off.unavailable
    );
    assert_eq!(
        (switched_off.desired, switched_off.unavailable),
        (Some(0), Some(1)),
        "an explicit zero beside a status that has not caught up: the coexistence the doc \
         comment on `short_of_pods` used to promise was impossible"
    );
    assert!(
        !short_of_pods(&switched_off),
        "a workload that wants no pods cannot be short of them — and `unavailable > 0` is the \
         one arm that does not learn that from the arithmetic (D82)"
    );

    let all = analyze(&with_workloads(Vec::new(), vec![switched_off]));
    show(&all);
    nothing(
        &all,
        "a Deployment the user has just deliberately turned off is not an outage, and a red \
         card about the pod it is in the middle of stopping is the screen crying wolf",
    );
}

/// **A ReplicaSet is not short of pods for having no `updatedReplicas`** (NOTES § D82).
///
/// [`short_of_pods`] is a general-looking helper on the shared [`WorkloadSnapshot`], and W2's kind
/// gate is the only thing keeping it off a ReplicaSet today — `analysis.rs` is the next file up and
/// takes the same type. Read a ReplicaSet's absent `updatedReplicas` as `unwrap_or(0)` and *the
/// question does not apply* becomes *none of them are on the current version*, which is true of
/// every healthy ReplicaSet alive. Its required `status.replicas` is the answer: a ReplicaSet is
/// one template, so every pod it has is a pod on it.
///
/// **Both buckets are asserted non-empty**, or a capture set that drifted to all-healthy or
/// all-broken would leave half of this passing by testing nothing.
#[test]
fn a_replicaset_is_short_of_pods_only_when_it_actually_is() {
    let mut short = Vec::new();
    let mut whole = Vec::new();
    for name in [
        "quota-replicasets",
        "healthy-replicasets",
        "rollout-replicasets",
        "owned-replicasets",
    ] {
        for rs in captured_replicasets(name) {
            println!(
                "{}: desired={:?} ready={:?} updated={:?} short={}",
                rs.id.name,
                rs.desired,
                rs.ready,
                rs.updated,
                short_of_pods(&rs)
            );
            assert_eq!(
                short_of_pods(&rs),
                rs.ready.unwrap_or(0) < rs.desired.unwrap_or(1),
                "a ReplicaSet is short exactly when fewer pods are passing their probes than \
                 it was told to run, and readiness is the only arm this kind has — {} \
                 disagrees",
                rs.id.name
            );
            if short_of_pods(&rs) {
                &mut short
            } else {
                &mut whole
            }
            .push(rs.id.name.clone());
        }
    }
    println!("short={short:?} whole={whole:?}");
    assert!(
        short.contains(&"broken-quota-59654c756".to_string()),
        "the refused ReplicaSet has `status.replicas: 0` against a desired 1 and has to stay \
         short, or this test passes by finding nothing: {short:?}"
    );
    assert!(
        whole.contains(&"healthy-deploy-7f84bdfb9b".to_string()),
        "and the healthy one — two of two ready, two on its one template — has to stay \
         whole: {whole:?}"
    );
    assert!(
        short.contains(&"broken-owned-7bdb7645c8".to_string()),
        "and the crash-looping one is what keeps the readiness arm from being redundant: \
         its one pod **exists**, so `updated` equals `desired` and no other arm sees it — \
         `unavailable` is a counter this kind does not have (D82): {short:?}"
    );
}

/// **W2's action names no command, because there is no command it can promise.**
///
/// `kubectl rollout undo` errors on a **single-revision** Deployment — *"no rollout history
/// found"* — which is `broken-quota`, the shipped fixture this very card is drawn on
/// (`observedGeneration: 1`, no pod ever created). On a **paused** Deployment it errors
/// differently: *"you cannot rollback a paused deployment"*. The contract carries neither
/// `spec.paused` nor a revision count, so an action line naming the command is a card telling
/// the reader to run something that will fail in front of them (NOTES § D82).
#[test]
fn the_rollout_card_promises_no_command_it_cannot_know_will_run() {
    let object: Deployment = serde_json::from_value(fixture("quota-deployment"))
        .expect("quota-deployment.json is a Deployment");
    let raw = fixture("quota-deployment");
    assert_eq!(
        raw["status"]["observedGeneration"], 1,
        "the fixture this card is drawn on is on its first revision, so `rollout undo` has \
         nothing to go back to"
    );

    let all = analyze(&with_workloads(Vec::new(), vec![object.into()]));
    show(&all);
    let card = only(&all, "broken-quota", "gave up");
    assert!(
        !card.action.contains("kubectl"),
        "the action may not name a command this card cannot know will run: {}",
        card.action
    );
    assert!(
        !card.action.contains("nothing else on this screen"),
        "nor may it claim the rest of the screen is silent — the suppression it is standing \
         on is narrower than that (D82): {}",
        card.action
    );
}

/// **The whole committed capture with its workloads joined on** — every pod in both
/// namespaces, every node, every Deployment and every ReplicaSet, through [`analyze`] at
/// once. `cargo test -- --nocapture` prints what a user would actually read.
#[test]
fn the_whole_capture_including_its_workloads_through_the_rules_at_once() {
    let pods: Vec<PodSnapshot> = every_captured_pod()
        .into_iter()
        .chain(
            items::<Pod>("owned-pods")
                .into_iter()
                .map(PodSnapshot::from),
        )
        .collect();
    let workloads: Vec<WorkloadSnapshot> = captured_deployments()
        .into_iter()
        .chain(captured_replicasets("quota-replicasets"))
        .chain(captured_replicasets("healthy-replicasets"))
        .chain(captured_replicasets("rollout-replicasets"))
        .chain(captured_replicasets("owned-replicasets"))
        .collect();
    let all = analyze(&ClusterSnapshot {
        workloads,
        ..cluster(pods, captured_nodes())
    });
    show(&all);

    let workload_cards: Vec<(&str, &str)> = all
        .iter()
        .filter(|f| {
            matches!(
                f.object.kind,
                ObjectKind::ReplicaSet | ObjectKind::Deployment
            )
        })
        .map(|f| (f.object.name.as_str(), f.title.as_str()))
        .collect();
    println!("{workload_cards:#?}");
    assert_eq!(
        workload_cards.len(),
        1,
        "over the whole capture the W-series has exactly one thing to say — the quota \
         refused `broken-quota`'s pods — and the rollout that timed out because of it is \
         the same problem, not a second card: {workload_cards:?}"
    );
    assert_eq!(workload_cards[0].0, "broken-quota-59654c756");

    for f in &all {
        assert_ne!(f.severity, Severity::Info, "D2: {}", f.title);
        let cmd = f
            .kubectl_cmd
            .as_deref()
            .unwrap_or_else(|| panic!("every rule in this box has a command: {}", f.title));
        assert!(
            cmd.contains(&f.object.name),
            "invariant 4's teaching device points at the object the card is about: {cmd}"
        );
    }
}
