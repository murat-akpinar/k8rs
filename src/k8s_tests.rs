use super::*;

use crate::rules::ObjectKind;
use k8s_openapi::serde::de::DeserializeOwned;

// --- THE CAPTURES, AND HOW A STREAM IS SYNTHESISED ---
//
// Every fixture below came off the kind cluster `scripts/cluster.sh` builds and is never edited
// to make a test pass (NOTES § D53). **What is hand-built here is the *stream*, not the
// objects**: there is no cluster in this turn, so the `Init` / `InitApply` / `InitDone` /
// `Apply` / `Delete` sequence a `watcher` would deliver is written out and the captured objects
// are carried on it. **That is the half a live API server would replace**: these tests prove
// what the store does with a sequence, never that kube delivers that sequence — kube's own
// `Event` doc is the source for the shape, and the box that drives a real `watcher` is the one
// that confirms it.

fn capture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} could not be read: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {path} is not JSON: {e}"))
}

/// A capture holding one object.
fn object<T: DeserializeOwned>(name: &str) -> T {
    serde_json::from_value(capture(name))
        .unwrap_or_else(|e| panic!("{name}.json does not decode: {e}"))
}

/// A `kubectl get -A` capture, which is a `kind: List` — the shape the initial LIST arrives in.
fn items<T: DeserializeOwned>(name: &str) -> Vec<T> {
    let list = capture(name);
    let items = list["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{name}.json has no items array"))
        .clone();
    assert!(
        !items.is_empty(),
        "{name}.json is empty, so a list built from it proves nothing"
    );
    items
        .into_iter()
        .map(|item| {
            serde_json::from_value(item)
                .unwrap_or_else(|e| panic!("{name}.json item does not decode: {e}"))
        })
        .collect()
}

fn at(moment: &str) -> Time {
    Time(
        moment
            .parse()
            .unwrap_or_else(|e| panic!("{moment} is not a time: {e}")),
    )
}

/// The caller's clock, pinned. Nothing in `k8s.rs` reads one (invariant 5, NOTES § D18).
fn now() -> Time {
    at("2026-08-22T09:15:00Z")
}

/// One stream's complete initial LIST: `Init`, an `InitApply` per object, `InitDone`.
fn list<K>(store: &mut Store, feed: fn(&mut Store, Event<K>), objects: Vec<K>) {
    feed(store, Event::Init);
    for object in objects {
        feed(store, Event::InitApply(object));
    }
    feed(store, Event::InitDone);
}

/// One permanent watch, named so a test can leave it out, and the complete initial LIST it
/// would deliver.
type Listing = (&'static str, Box<dyn Fn(&mut Store)>);

/// **The five permanent watches** (invariant 6), each as the complete LIST it would deliver,
/// named so a test can leave exactly one of them out.
fn streams() -> Vec<Listing> {
    vec![
        (
            "pods",
            Box::new(|store| list(store, Store::pod, items::<Pod>("kube-system-pods"))),
        ),
        (
            "nodes",
            Box::new(|store| list(store, Store::node, items::<Node>("nodes"))),
        ),
        (
            "deployments",
            Box::new(|store| list(store, Store::deployment, items::<Deployment>("deployments"))),
        ),
        (
            "statefulsets",
            Box::new(|store| {
                list(
                    store,
                    Store::stateful_set,
                    items::<StatefulSet>("statefulsets"),
                )
            }),
        ),
        (
            "daemonsets",
            Box::new(|store| list(store, Store::daemon_set, items::<DaemonSet>("daemonsets"))),
        ),
    ]
}

/// A store every watch has finished its first LIST into — the state every test that is not
/// about the gate starts in.
fn bootstrapped() -> Store {
    let mut store = Store::default();
    for (_, run) in streams() {
        run(&mut store);
    }
    store
}

/// Every stream but one.
fn all_but(missing: &str) -> Store {
    let mut store = Store::default();
    let mut left_out = false;
    for (name, run) in streams() {
        if name == missing {
            left_out = true;
        } else {
            run(&mut store);
        }
    }
    assert!(
        left_out,
        "{missing} is not one of the five streams, so nothing was left out"
    );
    store
}

fn pod_named<'a>(snapshot: &'a ClusterSnapshot, name: &str) -> &'a PodSnapshot {
    snapshot
        .pods
        .iter()
        .find(|pod| pod.id.name == name)
        .unwrap_or_else(|| panic!("no pod named {name} in the snapshot"))
}

// --- THE BOOTSTRAP GATE ---

/// A rule cannot tell a short list from a small cluster, so nothing may be published until
/// every initial LIST has landed (NOTES § D28). Each of the five is proven to gate on its own:
/// a check that only ever ran with the same stream missing would pass over four holes.
#[test]
fn no_snapshot_escapes_until_every_initial_list_has_landed() {
    for (missing, _) in streams() {
        assert!(
            all_but(missing).snapshot(now()).is_none(),
            "the {missing} watch had not finished its first LIST and a snapshot escaped anyway"
        );
    }
    assert!(
        bootstrapped().snapshot(now()).is_some(),
        "every initial LIST landed and the store still published nothing"
    );
}

/// The partial list itself, not merely a missing one: two pods of fourteen are in the store and
/// they may not be observable as the cluster.
#[test]
fn a_list_in_flight_is_not_observable() {
    let pods = items::<Pod>("kube-system-pods");
    assert!(
        pods.len() > 2,
        "the capture must hold more than the two listed below, or partial and complete are the \
         same thing"
    );
    let mut store = all_but("pods");
    store.pod(Event::Init);
    for pod in pods.iter().take(2).cloned() {
        store.pod(Event::InitApply(pod));
    }
    assert!(
        store.snapshot(now()).is_none(),
        "two pods of {} were published as the whole cluster",
        pods.len()
    );
    store.pod(Event::InitDone);
    let snapshot = store.snapshot(now()).expect("the last LIST landed");
    assert_eq!(
        snapshot.pods.len(),
        2,
        "the stream said two pods and the store must say two, not the fourteen in the capture"
    );
}

/// **A relist is not a bootstrap.** The last complete answer stays readable while a watch
/// re-lists, and the partial one is never visible — which is what the buffer is for. Stale is a
/// far smaller lie than *your cluster has one pod in it*, and blanking the screen on every
/// watch restart would be a worse answer to D28 than either.
#[test]
fn a_relist_shows_the_last_complete_answer_and_never_a_partial_one() {
    let listed_pods = items::<Pod>("kube-system-pods").len();
    assert!(
        listed_pods > 1,
        "the relist below lists one pod, so a capture of one pod would make both halves of \
         this test the same number"
    );
    let mut store = bootstrapped();
    store.pod(Event::Init);
    store.pod(Event::InitApply(object::<Pod>("crashloop")));
    let during = store
        .snapshot(now())
        .expect("a relist must not close the gate the first LIST opened");
    assert_eq!(
        during.pods.len(),
        listed_pods,
        "a relist in flight was published as the cluster"
    );
    store.pod(Event::InitDone);
    let after = store
        .snapshot(now())
        .expect("the relist landed and the store went quiet");
    assert_eq!(
        after.pods.len(),
        1,
        "the relist listed one pod, so the fourteen it did not list are gone (kube's own \
         contract for InitDone)"
    );
    assert_eq!(
        pod_named(&after, "broken-crashloop").id.name,
        "broken-crashloop"
    );
}

/// An `InitDone` with no `Init` in front of it is a broken stream, and a broken stream that
/// reports *your cluster has no nodes* is the failure the gate exists to prevent.
#[test]
fn an_init_done_with_no_list_behind_it_publishes_nothing() {
    let mut store = all_but("pods");
    store.pod(Event::InitDone);
    assert!(
        store.snapshot(now()).is_none(),
        "a stream that never listed anything opened the gate on an empty pod list"
    );
}

// --- ADD, MODIFY, DELETE ---

/// The identity a watch replaces in place is namespace and name. A pod deleted and recreated
/// under the same name carries a new uid, and the store must hold one of them, not both.
#[test]
fn a_recreated_name_replaces_rather_than_joins() {
    let mut store = bootstrapped();
    let mut pod = object::<Pod>("crashloop");
    store.pod(Event::Apply(pod.clone()));
    let before = store
        .snapshot(now())
        .expect("every initial LIST landed")
        .pods
        .len();
    pod.metadata.uid = Some("11111111-2222-3333-4444-555555555555".to_string());
    store.pod(Event::Apply(pod));
    let after = store.snapshot(now()).expect("every initial LIST landed");
    assert_eq!(
        after.pods.len(),
        before,
        "the same namespace and name arrived twice and the store kept both"
    );
    assert_eq!(
        pod_named(&after, "broken-crashloop").id.uid.as_deref(),
        Some("11111111-2222-3333-4444-555555555555"),
        "the second Apply did not replace the first"
    );
}

/// Two namespaces, one name — the case a key built from the name alone loses.
#[test]
fn the_namespace_is_part_of_the_identity() {
    let mut store = bootstrapped();
    let here = object::<Pod>("crashloop");
    let mut elsewhere = here.clone();
    elsewhere.metadata.namespace = Some("payments".to_string());
    store.pod(Event::Apply(here));
    store.pod(Event::Apply(elsewhere));
    let snapshot = store.snapshot(now()).expect("every initial LIST landed");
    let mut namespaces: Vec<_> = snapshot
        .pods
        .iter()
        .filter(|pod| pod.id.name == "broken-crashloop")
        .filter_map(|pod| pod.id.namespace.clone())
        .collect();
    namespaces.sort();
    assert_eq!(
        namespaces,
        vec!["default".to_string(), "payments".to_string()],
        "two pods of the same name in different namespaces are two objects"
    );
}

#[test]
fn a_delete_removes_only_the_object_it_names() {
    let mut store = bootstrapped();
    let stranger = object::<Pod>("healthy");
    let stranger_name = stranger
        .metadata
        .name
        .clone()
        .expect("a captured pod is named");
    let opening = store.snapshot(now()).expect("every initial LIST landed");
    let listed_pods = opening.pods.len();
    assert!(
        !opening.pods.iter().any(|pod| pod.id.name == stranger_name),
        "{stranger_name} is in the listed set, so deleting it below is not the unknown-object \
         case this test is for"
    );
    store.pod(Event::Delete(stranger));
    assert_eq!(
        store
            .snapshot(now())
            .expect("every initial LIST landed")
            .pods
            .len(),
        listed_pods,
        "deleting an object the store never held removed something else"
    );
    let victim = items::<Pod>("kube-system-pods")
        .into_iter()
        .next()
        .expect("the capture holds pods");
    let name = victim
        .metadata
        .name
        .clone()
        .expect("a captured pod is named");
    store.pod(Event::Delete(victim));
    let snapshot = store.snapshot(now()).expect("every initial LIST landed");
    assert_eq!(
        snapshot.pods.len(),
        listed_pods - 1,
        "the delete removed nothing"
    );
    assert!(
        !snapshot.pods.iter().any(|pod| pod.id.name == name),
        "{name} was deleted and is still in the snapshot"
    );
}

// --- WHAT THE SNAPSHOT SAYS ---

/// Three kinds, three streams, one list — and a Deployment and a DaemonSet of the same name in
/// the same namespace are two workloads, which is what the store keeping a map per stream buys.
#[test]
fn the_three_workload_kinds_arrive_in_one_list_without_colliding() {
    let expected = items::<Deployment>("deployments").len()
        + items::<StatefulSet>("statefulsets").len()
        + items::<DaemonSet>("daemonsets").len();
    let mut store = bootstrapped();
    let workloads = store
        .snapshot(now())
        .expect("every initial LIST landed")
        .workloads;
    assert_eq!(
        workloads.len(),
        expected,
        "a workload was lost between the three streams"
    );
    for kind in [
        ObjectKind::Deployment,
        ObjectKind::StatefulSet,
        ObjectKind::DaemonSet,
    ] {
        assert!(
            workloads.iter().any(|workload| workload.id.kind == kind),
            "no {kind:?} reached the snapshot"
        );
    }

    let taken = items::<Deployment>("deployments")
        .into_iter()
        .next()
        .expect("the capture holds deployments")
        .metadata;
    let mut twin = items::<DaemonSet>("daemonsets")
        .into_iter()
        .next()
        .expect("the capture holds daemonsets");
    let shared = taken.name.clone().expect("a captured deployment is named");
    twin.metadata.name.clone_from(&taken.name);
    twin.metadata.namespace.clone_from(&taken.namespace);
    store.daemon_set(Event::Apply(twin));
    let sharing: Vec<_> = store
        .snapshot(now())
        .expect("every initial LIST landed")
        .workloads
        .into_iter()
        .filter(|workload| workload.id.name == shared)
        .map(|workload| workload.id.kind)
        .collect();
    assert_eq!(
        sharing.len(),
        2,
        "a Deployment and a DaemonSet are both named {shared} and the store kept one of them"
    );
    assert!(
        sharing.contains(&ObjectKind::Deployment) && sharing.contains(&ObjectKind::DaemonSet),
        "the two workloads sharing the name {shared} are not one of each: {sharing:?}"
    );
}

/// `now` is the caller's, carried as a value — twice, so a constant cannot pass for it.
#[test]
fn now_is_the_callers_and_reaches_the_snapshot_unchanged() {
    let store = bootstrapped();
    for moment in ["2026-08-22T09:15:00Z", "2019-01-01T00:00:00Z"] {
        assert_eq!(
            store
                .snapshot(at(moment))
                .expect("every initial LIST landed")
                .now,
            at(moment),
            "the snapshot's now is not the one the caller handed in"
        );
    }
}

/// **`None` is *nobody looked*, and it is not an empty list** (NOTES § D129). None of these is
/// watched, and the store must not answer for a fetch it never made.
#[test]
fn nothing_the_store_did_not_read_is_reported_as_read() {
    let snapshot = bootstrapped()
        .snapshot(now())
        .expect("every initial LIST landed");
    assert_eq!(
        snapshot.replica_sets, None,
        "ReplicaSets are fetched on demand, never watched"
    );
    assert_eq!(snapshot.services, None);
    assert_eq!(snapshot.endpoint_slices, None);
    assert_eq!(snapshot.claims, None);
    assert_eq!(snapshot.disruption_budgets, None);
    assert_eq!(snapshot.certificate_requests, None);
    assert_eq!(
        snapshot.metrics, None,
        "the metrics probe is a later box, and it did not run"
    );
    assert_eq!(
        snapshot.server_version, None,
        "connect() is a later box, so N4 says nothing"
    );
    assert_eq!(snapshot.context, None);
    assert_eq!(
        snapshot.client_certificate, None,
        "C1's certificate never came from a watch"
    );
    assert_eq!(
        snapshot.namespace_scope, None,
        "every namespace, as far as this store knows"
    );
}

// --- THE PRUNE ---

/// **The field a prune written from the structs drops** (NOTES § D97). `spec.restartPolicy` at
/// *pod* level is consumed by the decode and named by no snapshot type: `ContainerSnapshot`
/// carries the effective policy, not the two it was computed from. Drop it and rule 15 goes
/// silent on every pod that does not override per container, and every Istio sidecar decodes as
/// a plain init container.
#[test]
fn the_pod_level_restart_policy_reaches_the_store() {
    let pod = object::<Pod>("crashloop");
    let spec = pod.spec.clone().expect("a captured pod has a spec");
    assert_eq!(
        spec.restart_policy.as_deref(),
        Some("Always"),
        "the capture no longer carries a pod-level restartPolicy, so this test proves nothing"
    );
    assert!(
        spec.containers
            .iter()
            .all(|container| container.restart_policy.is_none()),
        "a container overrides the pod's policy in this capture, so the fallback is not what \
         would be read"
    );
    let mut store = bootstrapped();
    store.pod(Event::Apply(pod));
    let stored = store.snapshot(now()).expect("every initial LIST landed");
    let containers = &pod_named(&stored, "broken-crashloop").containers;
    assert!(
        !containers.is_empty(),
        "the pod decoded with no containers at all"
    );
    assert!(
        containers
            .iter()
            .all(|container| container.restart_policy.as_deref() == Some("Always")),
        "the pod-level spec.restartPolicy did not survive into the store (NOTES § D97)"
    );
}

/// The headline field, on the wire shape rather than on the struct: the sanitizer strips
/// `managedFields` from every committed capture, so it is put back here — and asserted to have
/// landed, or the test would be comparing two identical objects and passing on nothing.
#[test]
fn managed_fields_change_nothing_the_store_keeps() {
    let lean: Pod = object("crashloop");
    assert!(
        lean.metadata.managed_fields.is_none(),
        "the capture already carries managedFields, so the injection below proves nothing"
    );
    let mut document = capture("crashloop");
    document["metadata"]["managedFields"] = serde_json::json!([{
        "manager": "kubectl-client-side-apply",
        "operation": "Update",
        "apiVersion": "v1",
        "time": "2026-08-21T21:10:00Z",
        "fieldsType": "FieldsV1",
        "fieldsV1": {
            "f:metadata": { "f:labels": { ".": {}, "f:app": {} } },
            "f:spec": { "f:containers": { "k:{\"name\":\"quitter\"}": { ".": {}, "f:image": {} } } }
        }
    }]);
    let fat: Pod = serde_json::from_value(document).expect("the injected pod decodes");
    assert!(
        fat.metadata.managed_fields.is_some(),
        "the injection never reached the decoded object"
    );

    let mut thin_store = bootstrapped();
    thin_store.pod(Event::Apply(lean));
    let mut fat_store = bootstrapped();
    fat_store.pod(Event::Apply(fat));
    assert_eq!(
        thin_store.snapshot(now()),
        fat_store.snapshot(now()),
        "managedFields reached the store and changed what it holds"
    );
}

// --- THE STORE'S SNAPSHOT, THROUGH THE RULES ---

/// **The whole pipeline, which is the run this box has without a cluster**: a synthesised watch
/// stream through [`updates`], through [`drive`], into the [`Store`], out as a
/// [`ClusterSnapshot`] and into `analyze`.
///
/// `main.rs`'s [`load`](crate::load) builds the same type from files, and two builders of one
/// type is where a second copy of a rule grows; the guard against it is that both feed the same
/// consumer. What is asserted is that the rules read what the loop landed — a snapshot that
/// decoded into the right shape but lost its containers, its conditions or its owners would
/// still be a `ClusterSnapshot` and would report nothing at all.
#[tokio::test]
async fn the_rules_read_what_the_loop_lands() {
    let mut pods = listing(items::<Pod>("kube-system-pods"));
    for name in ["crashloop", "oom", "pending", "stuck"] {
        pods.push(Ok(Event::Apply(object::<Pod>(name))));
    }
    let mut store = Store::default();
    drive(
        vec![
            one_watch(pods, Store::pod),
            one_watch(listing(items::<Node>("nodes")), Store::node),
            one_watch(
                listing(items::<Deployment>("deployments")),
                Store::deployment,
            ),
            one_watch(
                listing(items::<StatefulSet>("statefulsets")),
                Store::stateful_set,
            ),
            one_watch(listing(items::<DaemonSet>("daemonsets")), Store::daemon_set),
        ],
        &mut store,
    )
    .await;
    let snapshot = store
        .snapshot(now())
        .expect("all five initial LISTs landed");
    let findings = crate::rules::analyze(&snapshot);
    for finding in &findings {
        println!(
            "{:?}  {}/{}  {}",
            finding.severity,
            finding.object.namespace.as_deref().unwrap_or(""),
            finding.object.name,
            finding.title
        );
    }
    assert!(
        !findings.is_empty(),
        "the loop handed the rules a snapshot they found nothing in, over a corpus captured \
         from broken pods"
    );
    for name in ["broken-crashloop", "broken-oom", "broken-pending"] {
        assert!(
            findings.iter().any(|finding| finding.object.name == name),
            "no finding about {name}, which the loop was given and the rules have a card for"
        );
    }
}

// --- THE DRIVER ---
//
// **The loop is a pump and these tests only ever ask whether it pumps faithfully.** What an
// event *means* is `Store`'s, proven above by feeding it by hand; what is proven here is that
// the same events through `drive` land the same store — so the loop cannot quietly become a
// second implementation of the store's rules. The streams are `stream::iter` over a `Vec`,
// which is the half a live API server would replace.

use futures_util::stream;

/// The sequence a Pod watch delivers over one bootstrap and two later changes.
fn pod_events() -> Vec<Event<Pod>> {
    let listed = items::<Pod>("kube-system-pods");
    let leaving = listed.first().cloned().expect("the capture holds pods");
    let mut events = vec![Event::Init];
    events.extend(listed.into_iter().map(Event::InitApply));
    events.push(Event::InitDone);
    events.push(Event::Apply(object::<Pod>("crashloop")));
    events.push(Event::Delete(leaving));
    events
}

fn one_watch<K: Send + 'static>(
    events: Vec<watcher::Result<Event<K>>>,
    apply: fn(&mut Store, Event<K>),
) -> BoxStream<'static, watcher::Result<Update>> {
    updates(stream::iter(events), apply)
}

/// **The loop is a faithful pump.** The same events by hand and through `drive` land the same
/// store, byte for byte — and the store is asserted to hold something first, or two empty
/// snapshots would compare equal and prove nothing.
#[tokio::test]
async fn the_loop_lands_what_the_store_lands_when_it_is_fed_by_hand() {
    let mut by_hand = bootstrapped();
    for event in pod_events() {
        by_hand.pod(event);
    }
    let expected = by_hand.snapshot(now()).expect("every initial LIST landed");
    assert!(
        expected.pods.len() > 1,
        "the events below leave one pod or none, so an empty store would pass this test"
    );
    assert!(
        expected
            .pods
            .iter()
            .any(|pod| pod.id.name == "broken-crashloop"),
        "the Apply after the list never landed even by hand"
    );

    let mut by_loop = bootstrapped();
    drive(
        vec![one_watch(
            pod_events().into_iter().map(Ok).collect(),
            Store::pod,
        )],
        &mut by_loop,
    )
    .await;
    assert_eq!(
        by_loop.snapshot(now()),
        Some(expected),
        "the loop and the store disagree about the same sequence of events"
    );
}

/// All five watches on one task, and each wired to the method for its own kind: driving the
/// five initial LISTs from nothing lands exactly the store the helper builds by hand.
#[tokio::test]
async fn the_five_watches_share_one_loop_and_each_reaches_its_own_kind() {
    let mut store = Store::default();
    drive(
        vec![
            one_watch(listing(items::<Pod>("kube-system-pods")), Store::pod),
            one_watch(listing(items::<Node>("nodes")), Store::node),
            one_watch(
                listing(items::<Deployment>("deployments")),
                Store::deployment,
            ),
            one_watch(
                listing(items::<StatefulSet>("statefulsets")),
                Store::stateful_set,
            ),
            one_watch(listing(items::<DaemonSet>("daemonsets")), Store::daemon_set),
        ],
        &mut store,
    )
    .await;
    let driven = store
        .snapshot(now())
        .expect("all five initial LISTs landed");
    assert!(!driven.pods.is_empty() && !driven.nodes.is_empty() && !driven.workloads.is_empty());
    assert_eq!(
        Some(driven),
        bootstrapped().snapshot(now()),
        "the loop's five watches did not land where the same five land by hand"
    );
}

/// One stream's complete initial LIST as a `watcher` would deliver it, every event an `Ok`.
fn listing<K>(objects: Vec<K>) -> Vec<watcher::Result<Event<K>>> {
    let mut events = vec![Ok(Event::Init)];
    events.extend(
        objects
            .into_iter()
            .map(|object| Ok(Event::InitApply(object))),
    );
    events.push(Ok(Event::InitDone));
    events
}

/// **A failure does not end the loop.** `?` or `try_for_each` here would stop on the first one,
/// which is how k9s lost its reconnector permanently to a single blip.
#[tokio::test]
async fn a_failed_watch_does_not_end_the_loop() {
    let pods = items::<Pod>("kube-system-pods");
    let mut events: Vec<watcher::Result<Event<Pod>>> = vec![Err(watcher::Error::NoResourceVersion)];
    events.extend(listing(pods.clone()));
    events.insert(3, Err(watcher::Error::NoResourceVersion));
    let mut store = all_but("pods");
    drive(vec![one_watch(events, Store::pod)], &mut store).await;
    let snapshot = store
        .snapshot(now())
        .expect("the LIST landed after the failures and the gate opened");
    assert_eq!(
        snapshot.pods.len(),
        pods.len(),
        "the loop stopped at a failure and the events after it never arrived"
    );
}

/// **And a failure cannot open the gate.** A watch that fails part-way through its first LIST
/// has not listed, and D28 still says nothing may be published.
#[tokio::test]
async fn a_watch_that_fails_mid_list_publishes_nothing() {
    let pods = items::<Pod>("kube-system-pods");
    let mut events: Vec<watcher::Result<Event<Pod>>> = vec![Ok(Event::Init)];
    events.extend(
        pods.into_iter()
            .take(2)
            .map(|pod| Ok(Event::InitApply(pod))),
    );
    events.push(Err(watcher::Error::NoResourceVersion));
    let mut store = all_but("pods");
    drive(vec![one_watch(events, Store::pod)], &mut store).await;
    assert!(
        store.snapshot(now()).is_none(),
        "a watch that failed part-way through its first LIST published a partial cluster"
    );
    assert!(
        store.failure().is_some(),
        "the failure was swallowed: nothing on the store says the watch broke"
    );
}

/// **A failure is not erased by the next thing that goes right.** Four healthy watches deliver
/// ordinary traffic every second, so a failure cleared by any success is a failure nobody can
/// ever see; the fifth watch's 403 has to survive the other four.
#[tokio::test]
async fn a_failure_survives_the_events_that_follow_it() {
    let mut store = all_but("pods");
    drive(
        vec![one_watch(
            vec![Err(watcher::Error::NoResourceVersion), Ok(Event::Init)],
            Store::pod,
        )],
        &mut store,
    )
    .await;
    assert!(
        store.failure().is_some(),
        "an event arrived after the failure and took the record of it away"
    );
}

/// No watches is not a cluster with nothing in it: the loop ends at once and the gate stays
/// shut.
#[tokio::test]
async fn a_loop_with_no_watches_publishes_nothing() {
    let mut store = Store::default();
    drive(Vec::new(), &mut store).await;
    assert!(
        store.snapshot(now()).is_none(),
        "a loop that was given no watches at all reported a cluster"
    );
}

/// **A 403 arrives on one watch, not on all five.** A watch that only ever fails must not take
/// the other four down with it: they finish their lists, the gate stays shut on the one that
/// did not, and it opens the moment that one is answered.
#[tokio::test]
async fn a_watch_that_only_fails_leaves_the_other_four_alone() {
    let mut store = Store::default();
    drive(
        vec![
            one_watch(
                vec![
                    Err(watcher::Error::NoResourceVersion),
                    Err(watcher::Error::NoResourceVersion),
                ],
                Store::pod,
            ),
            one_watch(listing(items::<Node>("nodes")), Store::node),
            one_watch(
                listing(items::<Deployment>("deployments")),
                Store::deployment,
            ),
            one_watch(
                listing(items::<StatefulSet>("statefulsets")),
                Store::stateful_set,
            ),
            one_watch(listing(items::<DaemonSet>("daemonsets")), Store::daemon_set),
        ],
        &mut store,
    )
    .await;
    assert!(
        store.snapshot(now()).is_none(),
        "the pod watch never listed and the other four published the cluster without it"
    );
    assert!(store.failure().is_some(), "the failure was swallowed");

    list(&mut store, Store::pod, items::<Pod>("kube-system-pods"));
    let snapshot = store
        .snapshot(now())
        .expect("the fifth watch answered and the gate opened");
    assert!(
        !snapshot.nodes.is_empty() && !snapshot.workloads.is_empty(),
        "the four watches that succeeded lost their objects to the one that failed"
    );
}
