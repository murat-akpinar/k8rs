use super::*;

use crate::rules::ObjectKind;
use k8s_openapi::jiff::SignedDuration;
use k8s_openapi::serde::de::DeserializeOwned;
use std::collections::BTreeSet;

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
///
/// **Every event at the same instant**, which is what a test that is not about timing wants:
/// the store stamps [`Listing::since`] from what it is handed (NOTES § D150), so a fixed clock
/// keeps these stores comparable. The tests that *are* about timing hand out their own.
fn list<K>(store: &mut Store, feed: fn(&mut Store, &Time, Event<K>), objects: Vec<K>) {
    feed(store, &now(), Event::Init);
    for object in objects {
        feed(store, &now(), Event::InitApply(object));
    }
    feed(store, &now(), Event::InitDone);
}

/// One permanent watch, named so a test can leave it out, and the complete initial LIST it
/// would deliver. **Not [`Listing`]**, which is the product type for an unfinished one.
type NamedStream = (&'static str, Box<dyn Fn(&mut Store)>);

/// **The five permanent watches** (invariant 6), each as the complete LIST it would deliver,
/// named so a test can leave exactly one of them out.
fn streams() -> Vec<NamedStream> {
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
    store.pod(&now(), Event::Init);
    for pod in pods.iter().take(2).cloned() {
        store.pod(&now(), Event::InitApply(pod));
    }
    assert!(
        store.snapshot(now()).is_none(),
        "two pods of {} were published as the whole cluster",
        pods.len()
    );
    store.pod(&now(), Event::InitDone);
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
    store.pod(&now(), Event::Init);
    store.pod(&now(), Event::InitApply(object::<Pod>("crashloop")));
    let during = store
        .snapshot(now())
        .expect("a relist must not close the gate the first LIST opened");
    assert_eq!(
        during.pods.len(),
        listed_pods,
        "a relist in flight was published as the cluster"
    );
    store.pod(&now(), Event::InitDone);
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
    store.pod(&now(), Event::InitDone);
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
    store.pod(&now(), Event::Apply(pod.clone()));
    let before = store
        .snapshot(now())
        .expect("every initial LIST landed")
        .pods
        .len();
    pod.metadata.uid = Some("11111111-2222-3333-4444-555555555555".to_string());
    store.pod(&now(), Event::Apply(pod));
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
    store.pod(&now(), Event::Apply(here));
    store.pod(&now(), Event::Apply(elsewhere));
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
    store.pod(&now(), Event::Delete(stranger));
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
    store.pod(&now(), Event::Delete(victim));
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
    store.daemon_set(&now(), Event::Apply(twin));
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
    store.pod(&now(), Event::Apply(pod));
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
    thin_store.pod(&now(), Event::Apply(lean));
    let mut fat_store = bootstrapped();
    fat_store.pod(&now(), Event::Apply(fat));
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
    apply: fn(&mut Store, &Time, Event<K>),
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
        by_hand.pod(&now(), event);
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

// --- THE INITIAL LIST ---
//
// **What a paged LIST changes, and what it does not — with the pages themselves synthesised.** kube
// emits `Init` / `InitApply`* / `InitDone` across however many HTTP responses the LIST took
// (`kube-runtime-4.2.0/src/watcher.rs:523`, `:548`, `:555-559`), and there is no cluster here to
// deliver them — so what is proven below is that the store answers a *paged* sequence correctly,
// never that kube produces that sequence. The source is read; the round trip is not.

/// **The three upstream facts [`INITIAL_LIST_PAGE`] was reasoned against, pinned** (NOTES §
/// D147). The number is only *decided* here — the `connect()` box is what hands it to kube — so
/// what this turn can still guard is the ground it was chosen on: that kube pages at all, that
/// `page_size` has any effect under the strategy kube picks, and that neither call is bounded by
/// a timeout kube sets for us. `page_size` is an `Option` whose `None` is the one unbounded
/// `LIST pods -A` that `PRIOR-ART § A2` is about, and a kube upgrade that moves any of the three
/// has to come past this test rather than past nobody.
#[test]
fn kube_still_pages_the_initial_list_at_the_number_this_repo_chose() {
    assert_eq!(
        watcher::Config::default().page_size,
        Some(INITIAL_LIST_PAGE),
        "kube-runtime's default page size is no longer the {INITIAL_LIST_PAGE} that \
         INITIAL_LIST_PAGE's reasoning was written against (watcher.rs:276) — read it again \
         before following the new default"
    );
    assert_eq!(
        watcher::Config::default().initial_list_strategy,
        watcher::InitialListStrategy::ListWatch,
        "page_size has no effect under StreamingList (watcher.rs:256), so the number above \
         would be sent by nobody"
    );
    assert_eq!(
        watcher::Config::default().timeout,
        None,
        "kube grew a default timeout, and it is one field for the list call and the watch \
         (watcher.rs:400, :414) — a bounded LIST would now also re-LIST the cluster on that \
         period"
    );
}

/// **A LIST that arrives in pages is still one LIST** (NOTES § D147, D28). The gate is shut at
/// every page boundary — not merely before the first one — it opens once, and it opens on the
/// union of every page rather than on the last one.
#[test]
fn a_paged_initial_list_keeps_the_gate_shut_until_the_last_page() {
    let pods = items::<Pod>("kube-system-pods");
    let pages: Vec<&[Pod]> = pods.chunks(4).collect();
    assert!(
        pages.len() > 2 && pages.last().expect("chunks of a non-empty capture").len() < 4,
        "{} pods in pages of 4 is {} pages: this test needs several, and a short last one",
        pods.len(),
        pages.len()
    );

    let mut store = all_but("pods");
    store.pod(&now(), Event::Init);
    assert!(
        store.snapshot(now()).is_none(),
        "the first page had not even arrived and the store published a cluster"
    );
    for (number, page) in pages.iter().enumerate() {
        for pod in page.iter().cloned() {
            store.pod(&now(), Event::InitApply(pod));
        }
        assert!(
            store.snapshot(now()).is_none(),
            "page {} of {} was published as the whole cluster",
            number + 1,
            pages.len()
        );
    }

    store.pod(&now(), Event::InitDone);
    let snapshot = store.snapshot(now()).expect("every page landed");
    assert_eq!(
        snapshot
            .pods
            .iter()
            .map(|pod| pod.id.name.clone())
            .collect::<BTreeSet<_>>(),
        pods.iter()
            .map(|pod| pod
                .metadata
                .name
                .clone()
                .expect("a captured pod has a name"))
            .collect::<BTreeSet<_>>(),
        "the pages before the last one were dropped instead of accumulated"
    );

    let published = store.snapshot(now());
    store.pod(&now(), Event::InitDone);
    assert_eq!(
        store.snapshot(now()),
        published,
        "a second InitDone with no LIST behind it changed the answer"
    );
}

/// **A page that fails abandons every page before it.** A `continue` token the API server has
/// already compacted comes back `410 Gone`; kube reports `InitialListFailed` and resets its
/// machine to `Empty`, whose next poll emits a fresh `Init` (`watcher.rs:584`, `:523`). So the
/// store must answer with the second attempt alone — the half-list the first attempt delivered
/// is not a cluster that ever existed, and pagination is what makes that the ordinary path
/// rather than a rare one (NOTES § D147).
#[tokio::test]
async fn a_page_that_fails_restarts_the_list_and_the_pages_before_it_never_land() {
    let abandoned = items::<Pod>("kube-system-pods");
    let relisted = object::<Pod>("crashloop");
    assert!(
        !abandoned
            .iter()
            .any(|pod| pod.metadata.name == relisted.metadata.name),
        "the relisted pod is one of the abandoned ones, so the assertion below would hold \
         whether the abandoned pages survived or not"
    );
    let mut events: Vec<watcher::Result<Event<Pod>>> = vec![Ok(Event::Init)];
    events.extend(
        abandoned
            .iter()
            .cloned()
            .map(|pod| Ok(Event::InitApply(pod))),
    );
    events.push(Err(watcher::Error::InitialListFailed(kube::Error::Api(
        Box::new(kube::core::Status {
            code: 410,
            reason: "Expired".to_string(),
            message: "The provided continue parameter is too old to display a consistent list \
                      result"
                .to_string(),
            ..Default::default()
        }),
    ))));
    events.extend(listing(vec![relisted]));

    let mut store = all_but("pods");
    drive(vec![one_watch(events, Store::pod)], &mut store).await;

    let snapshot = store
        .snapshot(now())
        .expect("the second attempt listed and the gate opened");
    let names: BTreeSet<&str> = snapshot
        .pods
        .iter()
        .map(|pod| pod.id.name.as_str())
        .collect();
    assert_eq!(
        names,
        BTreeSet::from(["broken-crashloop"]),
        "the pages the failed attempt had already delivered were published as part of the cluster"
    );
    assert!(
        store.failure().is_some(),
        "the page that failed was swallowed: nothing on the store says the LIST restarted"
    );
}

// --- WHAT A THROTTLE LOOKS LIKE ---
//
// **There is no client-side rate limiter to test, and what is below is what was left instead**
// (NOTES § D148). tower's limiter is not compiled into this binary — the `limit` module is behind
// a Cargo feature `kube-client` does not enable — so no assertion could tell a build that has one
// from a build that does not. What the source reading left behind that a test *can* hold is the
// two states a screen has to draw from: which LIST is still outstanding, and what a throttle
// looks like once it stops being silent. **The silence between them is covered by nothing here**
// — kube retries a 429 below `watcher()`, in a tower layer with no callback — and it is recorded
// rather than tested.

/// The five streams [`all_but`] names, each with the kind [`Store::still_listing`] must report
/// when it is the one left out.
///
/// Written out rather than derived from the store, so the assertions below are the requirement
/// and not the implementation read back: [`all_but`] selects a stream by a string that predates
/// this table, so a `still_listing` that reported nodes as pods fails the loop rather than
/// agreeing with itself.
fn kind_of_stream() -> Vec<(&'static str, ObjectKind)> {
    vec![
        ("pods", ObjectKind::Pod),
        ("nodes", ObjectKind::Node),
        ("deployments", ObjectKind::Deployment),
        ("statefulsets", ObjectKind::StatefulSet),
        ("daemonsets", ObjectKind::DaemonSet),
    ]
}

/// **A bootstrap that is taking a while names the lists it is still waiting for** (NOTES § D148).
/// [`Store::snapshot`] answers `None` for every reason at once, so it is the only thing a screen
/// could draw a wait from and it says nothing about the wait; this is the state that does.
///
/// The order is asserted, not just the set: it is what stops a header re-shuffling between two
/// draws of the same unfinished bootstrap. Empty and `snapshot` being `Some` are asserted
/// together, because a `still_listing` that emptied early would open the gate with it.
#[test]
fn a_bootstrap_that_is_still_running_names_the_lists_it_is_waiting_for() {
    let every_kind: Vec<ObjectKind> = kind_of_stream().into_iter().map(|(_, kind)| kind).collect();
    assert_eq!(
        outstanding_kinds(&Store::default()),
        every_kind,
        "a store that has heard nothing reported one of the five watches as already listed"
    );
    for (stream, kind) in kind_of_stream() {
        assert_eq!(
            outstanding_kinds(&all_but(stream)),
            vec![kind],
            "four of the five LISTs landed and the store did not name the {stream} one as the \
             outstanding watch"
        );
    }
    let done = bootstrapped();
    assert_eq!(
        outstanding_kinds(&done),
        Vec::<ObjectKind>::new(),
        "every initial LIST landed and the store still says it is waiting for one"
    );
    assert!(
        done.snapshot(now()).is_some(),
        "the gate and this state disagree about the same store"
    );

    // The shape a real bootstrap spends its whole time in, and the one `all_but` never
    // produces: `Init` sent, objects arriving, no `InitDone` yet (NOTES § D29).
    let mut mid_list = all_but("pods");
    mid_list.pod(&now(), Event::Init);
    for pod in items::<Pod>("kube-system-pods") {
        mid_list.pod(&now(), Event::InitApply(pod));
    }
    assert_eq!(
        outstanding_kinds(&mid_list),
        vec![ObjectKind::Pod],
        "a LIST that had delivered objects but not finished was reported as landed"
    );

    // And a **relist** does not put a finished watch back on the list: `complete` is never false
    // again, because the last complete answer stays readable while the new LIST fills
    // (NOTES § D28). A screen that took this state from a reconnect would blank on every one.
    let mut relisting = bootstrapped();
    relisting.pod(&now(), Event::Init);
    assert_eq!(
        outstanding_kinds(&relisting),
        Vec::<ObjectKind>::new(),
        "a reconnect put the pods watch back among the outstanding ones"
    );
}

/// The kinds of an unfinished bootstrap, for the assertions that are about *which* watch rather
/// than about what it has to show for itself.
fn outstanding_kinds(store: &Store) -> Vec<ObjectKind> {
    store
        .still_listing()
        .into_iter()
        .map(|listing| listing.kind)
        .collect()
}

/// One [`Listing`] by kind, or a panic naming what was there instead.
fn listing_for(store: &Store, kind: ObjectKind) -> Listing {
    let outstanding = store.still_listing();
    outstanding
        .iter()
        .find(|listing| listing.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} among {:?}", outstanding_kinds(store)))
        .clone()
}

/// **A throttle that outlives kube's retries arrives as a code, not as prose** (NOTES § D148).
/// The API server's own limiter answers `429` with a `Retry-After`, and `kube-client` retries it
/// silently fifteen times before the error is ever ours (`client/retry.rs:108`,
/// `client/builder.rs:251`) — so the one thing this layer owes the screen is that the fifteenth
/// failure is still *distinguishable* when it lands, rather than flattened into a sentence.
///
/// **Half of that guard is the compiler and is meant to be**: the `match` below stops compiling
/// if `failure` is ever stored as a `String`, which is the change that would quietly turn a
/// throttle, a 403 and a dead API server into one banner. The runtime half is that the code that
/// comes out is the code that went in.
///
/// `code` and not `reason`: a 429 whose body does not parse as a `Status` — a proxy's HTML, say —
/// is rebuilt by kube as `Status::failure(text, "Failed to parse error data").with_code(429)`
/// (`client/mod.rs:556-557`), so the numeric code survives that path and the reason does not.
///
/// **The `Status` below is written, not captured**, and nothing asserts its wording: no cluster
/// here has ever been throttled, so what a real Priority-and-Fairness rejection says is one of
/// the things § WHAT A THROTTLE LOOKS LIKE lists as unsettled.
#[tokio::test]
async fn a_throttled_api_server_reaches_the_store_as_a_code_and_not_as_prose() {
    let mut store = all_but("pods");
    drive(
        vec![one_watch::<Pod>(
            vec![Err(watcher::Error::InitialListFailed(kube::Error::Api(
                Box::new(kube::core::Status {
                    code: 429,
                    reason: "TooManyRequests".to_string(),
                    message: "the server has received too many requests and has asked the \
                              client to try again later"
                        .to_string(),
                    ..Default::default()
                }),
            )))],
            Store::pod,
        )],
        &mut store,
    )
    .await;

    let code = match store.failure() {
        Some(watcher::Error::InitialListFailed(kube::Error::Api(status))) => status.code,
        other => panic!(
            "a 429 on the initial LIST did not reach the store as a typed API error: {other:?}"
        ),
    };
    assert_eq!(
        code, 429,
        "the store kept an API failure but not which one it was, so nothing downstream can tell \
         a throttled server from a forbidden one"
    );
    assert_eq!(
        outstanding_kinds(&store),
        vec![ObjectKind::Pod],
        "the LIST that failed is not reported as outstanding, so a screen would show the \
         throttle and claim the pods had been read"
    );
    assert!(
        store.snapshot(now()).is_none(),
        "a watch that never listed published a cluster anyway (NOTES § D28)"
    );
}

// --- A FIRST SYNC THAT DOES NOT FINISH ---
//
// **`PRIOR-ART § A7` asks that a first sync which never completes become a state rather than a
// wait, and these are the two facts that make it one** (NOTES § D150). No deadline is tested
// because none exists: *slow* and *hung* overlap by construction — 10 000 pods is twenty
// sequential round trips at `INITIAL_LIST_PAGE` — so what is proven here is that a caller can
// tell them apart **without** a number. A LIST that is working produces a count that moves and
// a stamp that advances; a hung one produces neither, and both are readable at any instant.
//
// **What is not proven here is the hang itself.** Every stream below is `stream::iter` over a
// `Vec`, which cannot stall — a real stall is a socket with no keepalive (NOTES § D148) against
// a server nothing in this repo has met. What these tests hold is that the store *records*
// enough to see one; that a real one is seen is a cluster measurement.

/// **A LIST that has begun and delivered nothing is a state, not a silence** — the exact shape
/// k9s [#4044](https://github.com/derailed/k9s/issues/4044) leaves on screen forever.
///
/// `Init` arrives before kube makes the request at all (`watcher.rs:522-527` returns it from
/// `State::Empty` with no `.await`), so a watch that will never answer still stamps a start —
/// which is what makes *nothing has arrived* a fact with a duration rather than an absence.
#[test]
fn a_list_that_has_delivered_nothing_still_says_when_it_started() {
    let begun = at("2026-08-22T09:00:00Z");
    let mut store = all_but("pods");
    store.pod(&begun, Event::Init);

    let waiting = listing_for(&store, ObjectKind::Pod);
    println!(
        "hung LIST: {:?} · so far {} · since {:?} · {}",
        waiting.kind,
        waiting.so_far,
        waiting.since,
        crate::rules::age(
            &now(),
            waiting.since.as_ref().expect("the Init stamped a start")
        )
        .expect("a start in the past renders as an age")
    );
    assert_eq!(
        waiting.so_far, 0,
        "a LIST that has delivered no objects claimed to have some"
    );
    assert_eq!(
        waiting.since,
        Some(begun),
        "the Init that opened the LIST left no stamp, so nothing downstream can say how long \
         this cluster has been silent"
    );

    // And before the loop's first poll there is not even that: no watch has produced anything,
    // which is a different state from *begun and quiet* and must not read as the same one.
    let unstarted = listing_for(&Store::default(), ObjectKind::Pod);
    assert_eq!(
        (unstarted.so_far, unstarted.since),
        (0, None),
        "a store that has heard nothing invented a moment it heard something"
    );
}

/// **A LIST that is working produces a count that moves and a stamp that advances**, and the
/// stamp is the **last** object's rather than the `Init`'s — which is the difference between
/// seeing a stall inside a page and only seeing one between pages (NOTES § D150).
#[test]
fn a_list_that_is_working_moves_both_numbers() {
    let opened = at("2026-08-22T09:00:00Z");
    let mut store = all_but("pods");
    store.pod(&opened, Event::Init);

    let pods = items::<Pod>("kube-system-pods");
    assert!(
        pods.len() > 2,
        "the capture holds {} pods, too few for a count to be seen moving",
        pods.len()
    );
    let mut seen = Vec::new();
    for (i, pod) in pods.into_iter().enumerate() {
        // Built by arithmetic rather than by formatting a second field, so a capture that ever
        // grows past 59 pods moves the stamp on instead of asking `at` to parse `09:00:60Z`.
        let arrived = Time(opened.0 + SignedDuration::from_secs(i as i64 + 1));
        store.pod(&arrived, Event::InitApply(pod));
        let listing = listing_for(&store, ObjectKind::Pod);
        seen.push((listing.so_far, listing.since.clone()));
        assert_eq!(
            listing.since,
            Some(arrived),
            "the stamp is the Init's or an earlier object's, so a stall inside a page would be \
             invisible until the page ended"
        );
    }
    println!(
        "working LIST: {} steps, count {} -> {}, stamp {:?} -> {:?}",
        seen.len(),
        seen[0].0,
        seen[seen.len() - 1].0,
        seen[0].1,
        seen[seen.len() - 1].1
    );
    assert!(
        seen.windows(2).all(|w| w[1].0 > w[0].0),
        "the count did not rise on every object, so a working LIST is indistinguishable from a \
         stalled one: {seen:?}"
    );
    assert!(
        seen.windows(2).all(|w| w[1].1 > w[0].1),
        "the stamp did not advance on every object: {seen:?}"
    );
}

/// **A healthy watch cannot make a hung one look alive.** The stamp is per watch, and only the
/// two events an initial LIST is made of set it — so four watches finishing and then carrying
/// ordinary `Apply` traffic leave the fifth's silence exactly as long as it was.
///
/// This is the same failure `Store::failure` had to refuse in NOTES § D148, one field over: with
/// one shared stamp, the busiest watch in the cluster would erase the evidence of the stuck one.
#[test]
fn ordinary_traffic_on_the_other_watches_does_not_refresh_a_stuck_one() {
    let opened = at("2026-08-22T09:00:00Z");
    let mut store = all_but("pods");
    store.pod(&opened, Event::Init);

    let much_later = at("2026-08-22T09:30:00Z");
    for node in items::<Node>("nodes") {
        store.node(&much_later, Event::Apply(node));
    }
    for deployment in items::<Deployment>("deployments") {
        store.deployment(&much_later, Event::Apply(deployment));
    }

    let stuck = listing_for(&store, ObjectKind::Pod);
    assert_eq!(
        stuck.since,
        Some(opened.clone()),
        "traffic on another watch moved the stuck watch's stamp forward, which is how a hung \
         bootstrap gets reported as a busy one"
    );
    assert_eq!(
        stuck.so_far, 0,
        "another watch's objects were counted against this LIST"
    );

    // And an `Apply` on the *stuck* watch's own kind would not count either — it is not part of
    // an initial LIST. There is no such event before `InitDone` in kube's own sequence, so this
    // asserts the rule rather than a shape the wire produces.
    store.pod(&much_later, Event::Apply(object::<Pod>("crashloop")));
    assert_eq!(
        listing_for(&store, ObjectKind::Pod).since,
        Some(opened),
        "an Apply refreshed the initial LIST's stamp"
    );
}

/// **A LIST that failed part-way and started again reports the new attempt, not the dead one**
/// — the ordinary path when a `continue` token has been compacted (NOTES § D147), where kube
/// reports `InitialListFailed` and emits a fresh `Init` on the next poll.
///
/// Without this, a screen would keep showing the abandoned attempt's count beside a stamp that
/// never moves again, which reads as *stuck at 3* rather than *starting over*.
#[test]
fn a_relist_after_a_failure_starts_its_count_again() {
    let first = at("2026-08-22T09:00:00Z");
    let mut store = all_but("pods");
    store.pod(&first, Event::Init);
    for pod in items::<Pod>("kube-system-pods").into_iter().take(3) {
        store.pod(&first, Event::InitApply(pod));
    }
    let abandoned = listing_for(&store, ObjectKind::Pod);
    assert_eq!(
        abandoned.so_far, 3,
        "the first attempt did not buffer three"
    );

    let retry = at("2026-08-22T09:00:30Z");
    store.pod(&retry, Event::Init);
    let fresh = listing_for(&store, ObjectKind::Pod);
    println!(
        "relist: {} objects at {:?} -> {} objects at {:?}",
        abandoned.so_far, abandoned.since, fresh.so_far, fresh.since
    );
    assert_eq!(
        (fresh.so_far, fresh.since),
        (0, Some(retry)),
        "the retry inherited the abandoned attempt's count or its stamp"
    );
}

/// **The loop is where the clock is read, and it reaches the store** (NOTES § D150). Every test
/// above hands the store an instant by hand; this one proves the wiring — that `updates` stamps
/// each event as it arrives, so a real watch's `Listing` is not permanently `None`.
///
/// It asserts a **range**, not a value: the instant is a real `Timestamp::now()`, so the only
/// honest assertion is that it lies between two the test took itself.
#[tokio::test]
async fn the_loop_stamps_every_event_it_pumps() {
    let before = Time(Timestamp::now());
    let mut store = all_but("pods");
    let mut opening = vec![Ok(Event::Init)];
    opening.extend(
        items::<Pod>("kube-system-pods")
            .into_iter()
            .map(|pod| Ok(Event::InitApply(pod))),
    );
    drive(vec![one_watch(opening, Store::pod)], &mut store).await;
    let after = Time(Timestamp::now());

    let listing = listing_for(&store, ObjectKind::Pod);
    let stamped = listing.since.clone().expect("the loop stamped nothing");
    println!(
        "driven LIST: so far {} · stamped {:?} (between {:?} and {:?})",
        listing.so_far, stamped, before, after
    );
    assert!(
        listing.so_far > 0,
        "the loop pumped no objects into the store"
    );
    assert!(
        stamped >= before && stamped <= after,
        "the loop's stamp is outside the window the test held open, so it did not come from the \
         moment the event arrived: {stamped:?}"
    );
}

// --- THE INGEST GUARD ---
//
// **What is proven here is what the store *kept*, never what something printed.** The printer
// already had a guard (`main.rs`'s `sanitize`, NOTES § D122) and it is the half that was never
// in doubt; the half nothing below this file implemented is that a 50 MB field is not held.
//
// Three routes reach a screen by three different mechanisms and each gets its own framing
// (NOTES § D31): a kubelet's `state.waiting.message`, which rules 3 and 4 quote whole; a
// `metadata.finalizer`, which rule 12 puts in `evidence` verbatim and which anyone with `patch`
// on pods can set; and a `spec.volumes[].hostPath.path`, which Posture prints as a row's own
// subject and which anyone who can create a pod chooses.

/// One pod through the whole ingest path — decode, strip, bound — and the snapshot it became.
fn ingested_pod(pod: Pod) -> PodSnapshot {
    let mut store = all_but("pods");
    list(&mut store, Store::pod, vec![pod]);
    let mut snapshot = store.snapshot(now()).expect("every initial LIST landed");
    assert_eq!(
        snapshot.pods.len(),
        1,
        "one pod was listed and the store holds a different number"
    );
    snapshot.pods.remove(0)
}

/// The three fields the security gate names by hand, each on the capture that carries it.
fn route(name: &str) -> (&'static str, Vec<&'static str>) {
    match name {
        "message" => (
            "image",
            vec![
                "status",
                "containerStatuses",
                "0",
                "state",
                "waiting",
                "message",
            ],
        ),
        "finalizer" => ("stuck", vec!["metadata", "finalizers", "0"]),
        "hostPath" => ("hostpath", vec!["spec", "volumes", "0", "hostPath", "path"]),
        other => panic!("{other} is not one of the three routes the gate names"),
    }
}

/// A capture with one field replaced, all the way through the ingest path.
///
/// The field is asserted to have been there first: a path that no longer resolves would create
/// the value instead of replacing it, and the test would prove nothing while staying green.
fn poisoned(name: &str, poison: &str) -> PodSnapshot {
    let (fixture, path) = route(name);
    let mut document = capture(fixture);
    let mut at = &mut document;
    for step in &path {
        at = match step.parse::<usize>() {
            Ok(index) => at
                .get_mut(index)
                .unwrap_or_else(|| panic!("{fixture}.json has no [{index}] on the {name} route")),
            Err(_) => at
                .get_mut(*step)
                .unwrap_or_else(|| panic!("{fixture}.json has no {step} on the {name} route")),
        };
    }
    assert!(
        at.is_string(),
        "{fixture}.json no longer carries a string at the end of the {name} route, so \
         poisoning it proves nothing"
    );
    *at = serde_json::json!(poison);
    ingested_pod(serde_json::from_value(document).expect("the poisoned capture decodes"))
}

/// What the store kept for that field, read back off the snapshot type.
fn kept(name: &str, pod: &PodSnapshot) -> String {
    match name {
        "message" => pod
            .containers
            .iter()
            .find_map(|container| match &container.state {
                ContainerState::Waiting { message, .. } => message.clone(),
                _ => None,
            })
            .expect("the capture's container is waiting with a message"),
        "finalizer" => pod
            .finalizers
            .first()
            .expect("the capture's pod carries a finalizer")
            .clone(),
        "hostPath" => pod
            .host_path_mounts
            .first()
            .expect("the capture's pod mounts a host path")
            .path
            .clone(),
        other => panic!("{other} is not one of the three routes the gate names"),
    }
}

const ROUTES: [&str; 3] = ["message", "finalizer", "hostPath"];

/// What each route is bounded at — the class the field belongs to, asserted rather than assumed.
fn cap_of(name: &str) -> usize {
    match name {
        "finalizer" => IDENTIFIER,
        _ => FREE_TEXT,
    }
}

/// **Control characters die at ingest, wherever in the value they sit** (invariant 9, NOTES
/// § D29, § D146): as the whole value, inside one, at either boundary, and as a C1 escape a
/// UTF-8 stream can carry. Fed down all three routes, because each reaches the screen by its own
/// mechanism.
///
/// **Both classes of control character are here, and one shape carries both at once**: a
/// whitespace control becomes a single space, every other one is removed, and `one\ntwo\u{1b}…`
/// proves the two happen in the same pass rather than one of them happening twice. The
/// neighbours are the rest of the ruling — a run however it is spelled, either end, and a break
/// beside a space the cluster sent, which stays the cluster's.
#[test]
fn control_characters_die_at_ingest_wherever_they_sit() {
    for (shape, poison, survives) in [
        ("the whole value", "\u{1b}\u{7}\u{0}", ""),
        ("inside a value", "before\u{1b}[2Jafter", "before[2Jafter"),
        ("at the front", "\nleading", "leading"),
        ("at the back", "trailing\u{7f}", "trailing"),
        ("a C1 escape", "one\u{9b}two", "onetwo"),
        ("a tab and a carriage return", "a\tb\rc", "a b c"),
        ("a vertical tab and a form feed", "a\u{b}b\u{c}c", "a b c"),
        (
            "NEL, the one C1 that is whitespace",
            "one\u{85}two",
            "one two",
        ),
        (
            "both classes in one value",
            "one\ntwo\u{1b}[2Jthree",
            "one two[2Jthree",
        ),
        (
            "a run of breaks, however it is spelled",
            "one\r\n\r\ntwo",
            "one two",
        ),
        ("a break the sentence ended with", "starting\n", "starting"),
        (
            "a break beside a space the cluster sent",
            "one \n two",
            "one  two",
        ),
        (
            "two spaces the cluster sent, which stay two",
            "one  two",
            "one  two",
        ),
        (
            "a non-breaking space, which is not a control",
            "one\u{a0}two",
            "one\u{a0}two",
        ),
        ("nothing but breaks", "\n\r\n\t", ""),
    ] {
        for name in ROUTES {
            let pod = poisoned(name, poison);
            let stored = kept(name, &pod);
            assert!(
                !stored.chars().any(char::is_control),
                "{shape} survived the {name} route: {stored:?}"
            );
            assert_eq!(
                stored, survives,
                "the {name} route stripped more or less than the control characters of {shape}"
            );
        }
    }
}

/// **A value at the bound is kept whole; one byte over is cut, and the cut is marked.** The two
/// classes are asserted against each other in the same run: a 513-byte *message* is untouched
/// and a 513-byte *finalizer* is not, which is what says the fields were sorted into classes
/// rather than all given one number.
#[test]
fn a_value_at_the_bound_is_kept_whole_and_one_byte_over_is_marked() {
    for name in ROUTES {
        let cap = cap_of(name);

        let exact = "a".repeat(cap);
        let stored = kept(name, &poisoned(name, &exact));
        assert_eq!(
            stored, exact,
            "a {name} of exactly {cap} bytes was changed on the way in"
        );

        let over = "a".repeat(cap + 1);
        let stored = kept(name, &poisoned(name, &over));
        assert_eq!(
            stored,
            format!("{}{SHORTENED}", "a".repeat(cap)),
            "a {name} of {} bytes was not cut at {cap} and marked",
            cap + 1
        );
        assert!(
            stored.ends_with(SHORTENED),
            "the cut on the {name} route is silent, which is what widgets.md § 7 forbids"
        );
    }

    let past_an_identifier = "a".repeat(IDENTIFIER + 1);
    for free_text in ["message", "hostPath"] {
        assert_eq!(
            kept(free_text, &poisoned(free_text, &past_an_identifier)),
            past_an_identifier,
            "a {free_text} was cut at the identifier bound, so the two classes are one number"
        );
    }
    assert!(
        kept("finalizer", &poisoned("finalizer", &past_an_identifier)).ends_with(SHORTENED),
        "a finalizer was given the free-text bound, so the two classes are one number"
    );
}

/// **The shape the security gate names**: 10 MB down each of the three routes, and nothing near
/// it is held. What bounding buys is the resident set and not latency (NOTES § D115) — the whole
/// object still arrives and is still deserialized before a field is dropped.
#[test]
fn an_enormous_value_is_never_held_whole() {
    for name in ROUTES {
        let cap = cap_of(name);
        let enormous = "K".repeat(10_000_000);
        let stored = kept(name, &poisoned(name, &enormous));
        assert_eq!(
            stored.len(),
            cap + SHORTENED.len(),
            "10 MB down the {name} route left {} bytes in the store",
            stored.len()
        );
        println!(
            "{name}: 10000000 bytes in, {} bytes kept — {}",
            stored.len(),
            &stored[cap - 8..]
        );
    }
}

/// **A multi-byte character is never cut in half.** `String::truncate` slices bytes and panics
/// in the middle of one, and a crafted pod name is exactly how somebody would find that. Fed a
/// 2-, 3- and 4-byte character straddling the cut, so the walk back to a boundary is exercised
/// at one, two and three steps.
#[test]
fn a_multi_byte_character_is_never_cut_in_half() {
    for name in ROUTES {
        let cap = cap_of(name);
        for (character, width) in [("ç", 2), ("€", 3), ("🙂", 4)] {
            for straddle in 1..width {
                // The character starts `width - straddle` bytes before the cut, so the cut lands
                // inside it and the walk has to step back that far.
                let head = cap - (width - straddle);
                let poison = format!("{}{character}", "a".repeat(head));
                assert!(
                    poison.len() > cap,
                    "the {character} does not reach past the cut, so nothing straddles it"
                );
                let stored = kept(name, &poisoned(name, &poison));
                assert_eq!(
                    stored,
                    format!("{}{SHORTENED}", "a".repeat(head)),
                    "a {width}-byte character straddling the {name} cut by {straddle} was not \
                     dropped whole"
                );
                assert!(
                    !stored.contains('\u{fffd}'),
                    "the cut produced a replacement character, so it sliced a character in half"
                );
            }

            // And the character that ends exactly on the bound is kept whole.
            let poison = format!("{}{character}", "a".repeat(cap - width));
            assert_eq!(poison.len(), cap);
            assert_eq!(
                kept(name, &poisoned(name, &poison)),
                poison,
                "a {width}-byte character ending exactly on the {name} bound was cut anyway"
            );
        }

        // **A substituted space at the cut, with a four-byte character straddling it.** The
        // strip only ever shortens — a control character is one or two bytes and a space is one
        // — so nothing in the walk may assume the value it measures is the value that arrived.
        let head = "a".repeat(cap - 3);
        assert_eq!(
            kept(name, &poisoned(name, &format!("{head}\n🙂"))),
            format!("{head} {SHORTENED}"),
            "a break became a space beside a straddling character and the {name} cut moved"
        );
    }
}

/// **Strip first, then bound** — so the bound counts what is actually kept. 10 MB of escape
/// sequences leaves nothing behind and is not reported as shortened: nothing that could have
/// been shown was lost, and a marker there would be the record lying the other way.
#[test]
fn a_value_of_nothing_but_control_characters_is_not_reported_as_shortened() {
    for name in ROUTES {
        let stored = kept(name, &poisoned(name, &"\u{1b}".repeat(10_000_000)));
        assert_eq!(
            stored, "",
            "10 MB of escape sequences down the {name} route left something in the store"
        );
    }
}

/// **A delete has to find what the apply stored.** The key is namespace and name, both bounded,
/// so a name long enough to be cut has to be cut the same way on both events or the object
/// becomes unremovable.
#[test]
fn a_delete_finds_the_object_whose_name_was_shortened() {
    let mut document = capture("crashloop");
    document["metadata"]["name"] = serde_json::json!("z".repeat(IDENTIFIER + 40));
    let pod: Pod = serde_json::from_value(document).expect("the poisoned capture decodes");
    let mut store = bootstrapped();
    let before = store
        .snapshot(now())
        .expect("every initial LIST landed")
        .pods
        .len();
    store.pod(&now(), Event::Apply(pod.clone()));
    let during = store.snapshot(now()).expect("every initial LIST landed");
    assert_eq!(
        during.pods.len(),
        before + 1,
        "the oversized pod never landed"
    );
    assert!(
        during
            .pods
            .iter()
            .any(|pod| pod.id.name.ends_with(SHORTENED)),
        "the oversized name reached the store uncut"
    );
    store.pod(&now(), Event::Delete(pod));
    assert_eq!(
        store
            .snapshot(now())
            .expect("every initial LIST landed")
            .pods
            .len(),
        before,
        "the delete built a different key than the apply and the pod could not be removed"
    );
}

/// **Two keys that shorten to the same thing are one key, and which value survives is decided
/// rather than left to iteration order.** The first in key order keeps its value, so one store is
/// one answer; this is the one place the guard loses something instead of shortening it, and it
/// is named here so it cannot be discovered later as a surprise.
#[test]
fn two_labels_that_shorten_to_the_same_key_become_one_and_the_first_keeps_its_value() {
    let mut document = capture("crashloop");
    let shared = "k".repeat(IDENTIFIER);
    document["metadata"]["labels"] = serde_json::json!({
        format!("{shared}aaa"): "first",
        format!("{shared}bbb"): "second",
    });
    let pod: Pod = serde_json::from_value(document).expect("the poisoned capture decodes");
    let labels = ingested_pod(pod).labels;
    assert_eq!(
        labels.len(),
        1,
        "two keys that cut to the same 512 bytes stayed two keys: {labels:?}"
    );
    assert_eq!(
        labels
            .get(&format!("{shared}{SHORTENED}"))
            .map(String::as_str),
        Some("first"),
        "the second colliding label overwrote the first"
    );
}

// --- THE INGEST GUARD, OVER EVERY CAPTURE IN THE REPO ---

/// A control character and a run far past both bounds, in one value.
fn poison() -> String {
    format!("\u{1b}[2J{}", "P".repeat(20_000))
}

/// Every string in a captured document replaced by [`poison`], with two exemptions and no
/// others: anything shaped like a timestamp, and **the object's own `apiVersion`/`kind`**, which
/// `k8s-openapi` checks on the way in. Neither reaches a snapshot type. An `ownerReference`'s
/// `apiVersion` and `kind` are *not* exempt — they are unvalidated free text, they build
/// `ObjectKind::Other`, and `rules.rs` names this guard as the thing that has to cover them.
fn poison_every_string(value: &mut serde_json::Value, poison: &str, root: bool) {
    match value {
        serde_json::Value::String(text) => {
            let bytes = text.as_bytes();
            let is_a_timestamp =
                bytes.len() >= 11 && bytes[4] == b'-' && bytes[7] == b'-' && bytes[10] == b'T';
            if !is_a_timestamp {
                *text = poison.to_string();
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                poison_every_string(item, poison, false);
            }
        }
        serde_json::Value::Object(fields) => {
            for (name, field) in fields {
                if root && (name == "apiVersion" || name == "kind") {
                    continue;
                }
                poison_every_string(field, poison, false);
            }
        }
        _ => {}
    }
}

/// Every committed capture, one object at a time, with its `kind` — a `kind: List` unpacked.
fn every_captured_object() -> Vec<(String, String, serde_json::Value)> {
    let directory = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut objects = Vec::new();
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .expect("the fixture directory is readable")
        .map(|entry| entry.expect("a fixture directory entry is readable").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    files.sort();
    for path in files {
        let name = path
            .file_stem()
            .expect("a .json file has a stem")
            .to_string_lossy()
            .into_owned();
        let document: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("the capture is readable"))
                .expect("the capture is JSON");
        match document["items"].as_array() {
            Some(items) => {
                for item in items.clone() {
                    let kind = item["kind"].as_str().unwrap_or_default().to_string();
                    objects.push((name.clone(), kind, item));
                }
            }
            None => {
                let kind = document["kind"].as_str().unwrap_or_default().to_string();
                objects.push((name, kind, document));
            }
        }
    }
    assert!(
        objects.len() > 50,
        "only {} objects were found in tests/fixtures, so this sweep is reading the wrong place",
        objects.len()
    );
    objects
}

/// One captured object of a watched kind, through the real ingest path, as the store's own
/// `Debug` of what it kept. `None` for a kind no watch decodes.
///
/// Each of the three workload kinds goes down its own stream, because each is a different API
/// type — the one thing the driver's `Store::deployment` / `stateful_set` / `daemon_set` split
/// exists for.
fn ingested_dump(kind: &str, document: serde_json::Value) -> Option<String> {
    let mut store;
    let workloads = match kind {
        "Pod" => {
            let pod = serde_json::from_value(document).expect("a captured Pod decodes");
            return Some(format!("{:?}", ingested_pod(pod)));
        }
        "Node" => {
            store = all_but("nodes");
            let node = serde_json::from_value(document).expect("a captured Node decodes");
            list(&mut store, Store::node, vec![node]);
            return Some(format!(
                "{:?}",
                store.snapshot(now()).expect("listed").nodes
            ));
        }
        "Deployment" => {
            store = all_but("deployments");
            let workload = serde_json::from_value(document).expect("a captured Deployment decodes");
            list(&mut store, Store::deployment, vec![workload]);
            store.snapshot(now()).expect("listed").workloads
        }
        "StatefulSet" => {
            store = all_but("statefulsets");
            let workload =
                serde_json::from_value(document).expect("a captured StatefulSet decodes");
            list(&mut store, Store::stateful_set, vec![workload]);
            store.snapshot(now()).expect("listed").workloads
        }
        "DaemonSet" => {
            store = all_but("daemonsets");
            let workload = serde_json::from_value(document).expect("a captured DaemonSet decodes");
            list(&mut store, Store::daemon_set, vec![workload]);
            store.snapshot(now()).expect("listed").workloads
        }
        _ => return None,
    };
    Some(format!("{workloads:?}"))
}

/// **Every capture in the repo, poisoned in every string it has, through the real ingest path.**
///
/// The assertion is made on the `Debug` of what the store *kept*: a control character survives
/// as its `\u{..}` escape and an unbounded field survives as a run longer than any bound, so a
/// `Bounded` impl that does nothing cannot pass — whichever of the three watched types, and
/// whichever nested type, it belongs to.
#[test]
fn no_captured_object_can_carry_an_unbounded_or_unprintable_field_through_ingest() {
    let poison = poison();
    let too_long = "P".repeat(FREE_TEXT + 1);
    let mut swept = 0;
    for (fixture, kind, mut document) in every_captured_object() {
        poison_every_string(&mut document, &poison, true);
        let Some(dump) = ingested_dump(&kind, document) else {
            continue;
        };
        let where_from = format!("{fixture}.json ({kind})");
        swept += 1;
        assert!(
            !dump.contains(&too_long),
            "{where_from} kept a field longer than {FREE_TEXT} bytes"
        );
        assert!(
            !dump.contains("\\u{"),
            "{where_from} kept a control character"
        );
        assert!(
            dump.contains(SHORTENED),
            "{where_from} came through the guard with nothing marked, so it proves nothing"
        );
    }
    println!("{swept} poisoned objects swept through ingest");
    assert!(
        swept > 40,
        "only {swept} objects reached the three watched kinds, so the sweep is nearly empty"
    );
}

/// **The negative side of the bound: no object a real cluster sent is ever shortened.** Every
/// committed Pod, Node and workload through the ingest path, and nothing in any of them carries
/// the marker — which is the claim `IDENTIFIER` and `FREE_TEXT` are chosen to make, and the
/// one that would fail first if either number were set too low.
#[test]
fn no_captured_object_is_shortened_by_the_guard() {
    let mut compared = 0;
    for (fixture, kind, document) in every_captured_object() {
        let Some(dump) = ingested_dump(&kind, document) else {
            continue;
        };
        compared += 1;
        assert!(
            !dump.contains(SHORTENED),
            "{fixture}.json ({kind}) was shortened by the guard, so \
             {IDENTIFIER}/{FREE_TEXT} are below what a real cluster sends"
        );
    }
    println!("{compared} captured objects came through the guard with nothing shortened");
    assert!(compared > 40, "only {compared} objects were compared");
}

/// **A real cluster does send control characters, and this is the message that decided how they
/// are handled** (NOTES § D146). The committed `crashloop` capture carries a kubelet termination
/// message with two newlines in it — found by this box, not assumed — and rules 1 and 5 put that
/// message on a card. Removing the newline glued two words into `startingpanic`, which is not a
/// word and is not a sentence a beginner can read; the whole result is asserted here so it can
/// never come back.
#[test]
fn a_newline_a_real_kubelet_sent_becomes_a_space_and_never_glues_two_words() {
    let pod: Pod = object("crashloop");
    let sent = PodSnapshot::from(pod.clone()).containers[0]
        .last_terminated
        .as_ref()
        .and_then(|ended| ended.message.clone())
        .expect("the capture's container terminated with a message");
    assert_eq!(
        sent, "starting\npanic: dial tcp db.payments.svc:5432: connect: connection refused\n",
        "crashloop.json's termination message has changed, so this test is about a different \
         message than the one D146 was ruled on"
    );

    let kept = ingested_pod(pod).containers[0]
        .last_terminated
        .as_ref()
        .and_then(|ended| ended.message.clone())
        .expect("the message survived the guard");
    assert_eq!(
        kept, "starting panic: dial tcp db.payments.svc:5432: connect: connection refused",
        "the message a card will print is not the sentence the kubelet wrote"
    );
    assert!(
        !kept.contains("startingpanic"),
        "the two words are glued back together: {kept:?}"
    );
    println!("crashloop lastState message: {sent:?}\n                        kept: {kept:?}");
}

// --- THE FIELD LIST, DERIVED RATHER THAN TYPED ---
//
// **The failure this box exists to prevent is a field nobody remembered.** The sweep above only
// proves what the captures happen to carry; this proves what the *types* carry. `rules.rs`'s own
// struct definitions are the field list — the same reason the prune is the decode — so the list
// is read off them here instead of being written down a second time where it could go stale.

/// `rules.rs` and `k8s.rs` as text, read at compile time.
const RULES_SOURCE: &str = include_str!("rules.rs");
const K8S_SOURCE: &str = include_str!("k8s.rs");

/// `pub struct Foo {` / `enum Foo {` — the name, for a type declared at the top level.
fn type_header(line: &'static str) -> Option<&'static str> {
    let rest = line
        .strip_prefix("pub struct ")
        .or_else(|| line.strip_prefix("struct "))
        .or_else(|| line.strip_prefix("pub enum "))
        .or_else(|| line.strip_prefix("enum "))?;
    let name = rest.strip_suffix(" {")?;
    (!name.contains(['<', ' '])).then_some(name)
}

/// One line of a type body as (field, type), covering struct fields, struct-like enum variants
/// and tuple variants — for which the variant's own name stands in for a field name.
fn field_of(line: &'static str) -> Option<(&'static str, &'static str)> {
    let mut body = line.trim();
    if body.starts_with("//") || body.starts_with('#') {
        return None;
    }
    // `Running { started_at: Option<Time> },`
    if let Some((head, rest)) = body.split_once(" { ")
        && head.starts_with(char::is_uppercase)
    {
        body = rest;
    }
    // `Other(String),`
    if let Some((name, rest)) = body.split_once('(')
        && let Some(inner) = rest.strip_suffix("),")
        && name.starts_with(char::is_uppercase)
    {
        return Some((name, inner));
    }
    let body = body
        .trim_end_matches(',')
        .trim_end()
        .trim_end_matches('}')
        .trim_end();
    let (name, kind) = body.split_once(": ")?;
    let name = name.strip_prefix("pub ").unwrap_or(name);
    (!name.contains(' ')).then_some((name, kind))
}

/// Every type `rules.rs` declares, with its fields.
fn declared_types() -> BTreeMap<&'static str, Vec<(&'static str, &'static str)>> {
    let mut types = BTreeMap::new();
    let mut open: Option<(&'static str, Vec<(&'static str, &'static str)>)> = None;
    for line in RULES_SOURCE.lines() {
        if let Some(name) = type_header(line) {
            open = Some((name, Vec::new()));
            continue;
        }
        if open.is_none() {
            continue;
        }
        if line == "}" {
            let (name, fields) = open.take().expect("a type is open");
            types.insert(name, fields);
            continue;
        }
        if let Some(field) = field_of(line) {
            open.as_mut().expect("a type is open").1.push(field);
        }
    }
    types
}

/// Splits a line of Rust into the words a field name could be.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|character: char| !character.is_alphanumeric() && character != '_')
}

/// The one region of `k8s.rs` that is allowed to be the answer.
fn guard_region() -> &'static str {
    let start = K8S_SOURCE
        .find("// --- THE INGEST GUARD START ---")
        .expect("k8s.rs no longer has an ingest guard region");
    let end = K8S_SOURCE
        .find("// --- THE INGEST GUARD END ---")
        .expect("the ingest guard region is not closed");
    &K8S_SOURCE[start..end]
}

/// The body of `impl Bounded for <type>`, or `None` if there is no such impl.
fn bounded_impl(type_name: &str) -> Option<&'static str> {
    let region = guard_region();
    let head = format!("\nimpl Bounded for {type_name} {{\n");
    let at = region.find(&head)? + head.len();
    let rest = &region[at..];
    Some(&rest[..rest.find("\n}\n")?])
}

/// **`ObjectKind` has no impl of its own** — it is one arm inside `ObjectId`'s, because it has
/// exactly one text-carrying variant and no other owner. It is the only type allowed to be
/// answered by the region as a whole rather than by its own impl.
const BOUNDED_INSIDE_ANOTHER_IMPL: [&str; 1] = ["ObjectKind"];

/// **Every `String` a watched snapshot type can carry is named by the ingest guard**, derived
/// from `rules.rs` rather than typed out here. A field added to a snapshot type and forgotten in
/// `k8s.rs` fails this test; a generic sentence about "names and messages" is what lets one be
/// missed (todo.md, Phase 5 § Security gate).
#[test]
fn every_string_a_watched_snapshot_type_carries_is_named_by_the_ingest_guard() {
    let types = declared_types();
    assert!(
        types.len() > 20,
        "only {} types were parsed out of rules.rs, so this guard is reading nothing",
        types.len()
    );

    // The three types the five permanent watches decode into, and everything they reach.
    let mut reachable = BTreeSet::new();
    let mut queue = vec!["PodSnapshot", "NodeSnapshot", "WorkloadSnapshot"];
    while let Some(name) = queue.pop() {
        if !reachable.insert(name) {
            continue;
        }
        for (_, kind) in types.get(name).into_iter().flatten() {
            for word in words(kind) {
                if types.contains_key(word) {
                    queue.push(word);
                }
            }
        }
    }
    for expected in [
        "ObjectId",
        "ObjectKind",
        "Condition",
        "ContainerSnapshot",
        "ContainerState",
        "Terminated",
        "ExitRule",
        "HostPathMount",
        "Toleration",
        "Taint",
    ] {
        assert!(
            reachable.contains(expected),
            "{expected} is not reachable from the three watched types, so the walk is broken"
        );
    }
    for fetched in [
        "ClusterSnapshot",
        "ServiceSnapshot",
        "NodeUsage",
        "Selector",
    ] {
        assert!(
            !reachable.contains(fetched),
            "{fetched} is not watched and the walk reached it anyway"
        );
    }

    let mut checked = Vec::new();
    for name in &reachable {
        let carries_text: Vec<_> = types[name]
            .iter()
            .filter(|(_, kind)| words(kind).any(|word| word == "String"))
            .map(|(field, _)| *field)
            .collect();
        if carries_text.is_empty() {
            continue;
        }
        let body = if BOUNDED_INSIDE_ANOTHER_IMPL.contains(name) {
            guard_region()
        } else {
            bounded_impl(name).unwrap_or_else(|| {
                panic!("{name} carries {carries_text:?} and k8s.rs has no `impl Bounded` for it")
            })
        };
        for field in carries_text {
            assert!(
                words(body).any(|word| word == field),
                "{name}.{field} is a String the store keeps and the ingest guard never names it"
            );
            checked.push(format!("{name}.{field}"));
        }
    }

    for named_by_the_security_gate in [
        "ContainerState.message",
        "PodSnapshot.finalizers",
        "HostPathMount.path",
        "ObjectKind.Other",
        "ObjectId.name",
        "Condition.message",
    ] {
        assert!(
            checked
                .iter()
                .any(|found| found == named_by_the_security_gate),
            "{named_by_the_security_gate} was not among the {} fields derived from rules.rs, so \
             this guard is looking in the wrong place",
            checked.len()
        );
    }
    println!(
        "{} String fields derived from rules.rs, all named by the guard:",
        checked.len()
    );
    println!("  {}", checked.join(" · "));
    assert!(
        checked.len() >= 45,
        "only {} String fields were derived, which is fewer than the snapshot types carry",
        checked.len()
    );
}

/// **What the bound buys is the resident set, and here it is measured rather than argued**
/// (NOTES § D115). One pod whose every string is a megabyte long — 250 times `FREE_TEXT` and
/// 2000 times `IDENTIFIER` — and the bytes the store kept of it.
/// The whole object still arrives and is still deserialized before a field is dropped — that is
/// the half no bound can change, and the half this number does not claim.
#[test]
fn a_pod_of_megabyte_fields_costs_the_store_kilobytes() {
    let mut document = capture("hostpath");
    poison_every_string(&mut document, &"K".repeat(1_000_000), true);
    let sent = serde_json::to_string(&document)
        .expect("the poisoned document re-serialises")
        .len();
    let pod: Pod = serde_json::from_value(document).expect("the poisoned Pod decodes");
    let kept = format!("{:?}", ingested_pod(pod)).len();
    println!("hostpath.json poisoned: {sent} bytes on the wire, {kept} bytes kept by the store");
    assert!(
        sent > 10_000_000,
        "the poisoned object is only {sent} bytes, so it is not the shape the gate names"
    );
    assert!(
        kept < 64 * 1024,
        "the store kept {kept} bytes of a {sent}-byte object"
    );
}

// --- HOW OLD A CLUSTER MAY BE ---
//
// **The floor is 1.29 and the derivation is in the product file, not here** (NOTES § D149). What
// these tests hold is the half a cluster is not needed for: that every `gitVersion` shape a real
// API server returns reaches the comparison, that both boundaries sit where the constants
// say, that an unreadable version is silence rather than a guess, and that nothing the server
// sent is ever echoed back.
//
// **What is *not* proven here is the thing that matters most and needs a v1.24 cluster**: that a
// server below the floor accepts the LIST and the watch this file sends. Everything behind that
// claim was read off `kube-core-4.2.0/src/params.rs`, the KEP and the API's own reference — the
// wire itself has never been observed against an old server, and the fake-server harness box is
// the one that could observe it without a cluster.

/// **Every shape a real `gitVersion` takes reaches the comparison** — k3s's `+k3s1`, GKE's
/// `-gke.NNNNN`, a release candidate's `-rc.N`, a plain kind one, and one with no leading `v` —
/// the last of those is not a shape anybody has been observed returning, it is the one
/// [`minor_version`]'s `trim_start_matches('v')` exists for, pinned so a later edit cannot drop
/// it silently.
///
/// **Each is fed at a minor that is out of range on purpose**, because in range the answer is
/// `None` and so is the answer for a string [`minor_version`] could not parse at all — a test
/// built on the four real strings as they ship would pass just as well against a parser that
/// understood none of them.
///
/// **And each carries a *different* minor.** Holding them all at 1.24 was this test's first
/// draft, and its red run found it: a message with `1.24` written into it as a literal passed
/// every row. The number has to move for the assertion to be about the parse rather than about
/// the branch.
#[test]
fn every_shape_a_real_gitversion_takes_reaches_the_comparison() {
    for (version, minor) in [
        ("v1.24.4+k3s1", "1.24"),
        ("v1.26.3-gke.1286000", "1.26"),
        ("v1.20.0-rc.2", "1.20"),
        ("v1.28.2", "1.28"),
        ("1.19.16", "1.19"),
    ] {
        let note = version_note(version)
            .unwrap_or_else(|| panic!("{version} is below the floor and drew no note at all"));
        println!("{version:>22} -> {note}");
        assert!(
            note.contains(&format!("Kubernetes {minor},")),
            "{version} parsed to something other than {minor}: {note}"
        );
    }
    // The same four at the versions they are actually shipped as: two are inside the window and
    // one is the floor itself, so the right answer for all three is silence.
    for version in ["v1.29.4+k3s1", "v1.30.0-rc.2", "v1.31.2"] {
        assert_eq!(
            version_note(version),
            None,
            "{version} is inside the supported window and drew a note anyway"
        );
    }
}

/// **A cluster below the floor is told, and is not refused** (NOTES § D149). The boundary is
/// asserted from both sides, because an off-by-one here either warns every supported cluster or
/// never warns at all — and both read as *working*.
///
/// **A major nobody has seen sorts below 1.29 rather than being ignored**: the comparison is an
/// order and not N4's distance, so `v0.9` is old rather than unknown.
#[test]
fn a_cluster_below_the_floor_is_told_and_still_runs() {
    for version in ["v1.24.3", "v1.28.15", "v0.9.0"] {
        let note = version_note(version).unwrap_or_else(|| panic!("{version} drew no note"));
        println!("{version:>10} -> {note}");
        assert!(
            note.contains("only been checked against 1.29 and newer"),
            "{version} is below the floor and got the wrong note: {note}"
        );
        assert!(
            note.contains("still run"),
            "the note for {version} does not say k8rs runs anyway: {note}"
        );
    }
    assert_eq!(
        version_note("v1.29.0"),
        None,
        "the floor itself was reported as below the floor"
    );
}

/// **A cluster newer than these types is told too** — the second sentence NOTES § D99 says this
/// box owes, because a server above the pin drops its added fields at decode and no fixture guard
/// in this repo can see it happen on somebody else's machine.
///
/// Both sides of this boundary again, and a major above 1 sorts above rather than being ignored.
#[test]
fn a_cluster_newer_than_these_types_is_told_too() {
    for version in ["v1.37.0", "v1.99.1", "v2.0.0"] {
        let note = version_note(version).unwrap_or_else(|| panic!("{version} drew no note"));
        println!("{version:>10} -> {note}");
        assert!(
            note.contains("built to understand 1.36"),
            "{version} is above the pin and got the wrong note: {note}"
        );
        assert!(
            note.contains("A newer k8rs is the fix"),
            "the note for {version} does not say what to do about it: {note}"
        );
    }
    assert_eq!(
        version_note("v1.36.1"),
        None,
        "the version the fixtures were captured from was reported as too new"
    );
}

/// **A version string nothing can parse says nothing** — N4's habit, and the reason it is a habit:
/// a warning derived from a guess is worse than no warning. `apiserver_version` is free text from
/// the API server like any other field, and a vendor is free to return something this parser has
/// never seen.
#[test]
fn a_version_nobody_can_parse_says_nothing() {
    for version in ["", "v", "v1", "1.", "vNext", "v1.x", "unknown", "v.29.0"] {
        assert_eq!(
            version_note(version),
            None,
            "{version:?} could not be parsed and a note was drawn from it anyway"
        );
    }
}

/// **Nothing the server sent is echoed back** (invariant 9). A crafted `gitVersion` is free text
/// from the API and would otherwise reach a terminal through the one line printed at connect;
/// the note is built from the two integers [`minor_version`] parsed out, so the bound is
/// structural rather than a filter that could miss a byte.
///
/// The three framings NOTES § D31 names, because the escape need not be the whole string: it
/// sits after the digits, in the middle, and doubled.
#[test]
fn the_note_never_repeats_what_the_server_sent() {
    for version in [
        "v1.24.3-\u{1b}[2J\u{7}rm -rf ~",
        "v1.24.3\u{1b}]0;pwned\u{7}-gke.1",
        "v1.24.\u{1b}[1m3\u{1b}[0m",
    ] {
        let note = version_note(version)
            .unwrap_or_else(|| panic!("{version:?} is below the floor and drew no note"));
        assert!(
            note.contains("Kubernetes 1.24,"),
            "{version:?} did not reach the comparison: {note}"
        );
        assert!(
            !note.chars().any(char::is_control),
            "a control character from the server reached the note: {note:?}"
        );
        assert!(
            !note.contains("pwned") && !note.contains("rm -rf"),
            "the server's own bytes were repeated back: {note:?}"
        );
    }
}

/// **`TYPES_BUILT_FOR` is a copy of the `k8s-openapi` feature in `Cargo.toml`, so it is compared
/// with it rather than trusted.** A stale copy does not fail quietly: it tells everyone on the
/// pin's own version that their cluster is newer than this build, which is the wrong half of
/// NOTES § D99's table stated backwards.
///
/// The crate publishes the enabled feature as a `k8s_if_ge_1_NN!` macro that expands to its
/// contents or to nothing, so the ladder below reads the pin rather than repeating it. **Its top
/// rung is the crate's own maximum** — a pin raised past that needs a newer `k8s-openapi`, which
/// is a dependency change under invariant 10 and gets read by a human either way.
#[test]
fn the_version_these_types_were_built_for_is_the_pin() {
    // `max` rather than plain assignment: a rung that only writes is a dead store to clippy,
    // and every rung below the enabled one is exactly that.
    let mut pinned = 32u32;
    k8s_openapi::k8s_if_ge_1_33! { pinned = pinned.max(33); }
    k8s_openapi::k8s_if_ge_1_34! { pinned = pinned.max(34); }
    k8s_openapi::k8s_if_ge_1_35! { pinned = pinned.max(35); }
    k8s_openapi::k8s_if_ge_1_36! { pinned = pinned.max(36); }
    println!("k8s-openapi feature in Cargo.toml: v1_{pinned}; TYPES_BUILT_FOR: {TYPES_BUILT_FOR}");
    assert_eq!(
        pinned, TYPES_BUILT_FOR,
        "Cargo.toml pins k8s-openapi at v1_{pinned} and k8s.rs tells users it understands \
         1.{TYPES_BUILT_FOR}"
    );
}
