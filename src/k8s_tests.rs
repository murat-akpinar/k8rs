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
            one_watch(pods, |s| &mut s.pods),
            one_watch(listing(items::<Node>("nodes")), |s| &mut s.nodes),
            one_watch(listing(items::<Deployment>("deployments")), |s| {
                &mut s.deployments
            }),
            one_watch(listing(items::<StatefulSet>("statefulsets")), |s| {
                &mut s.stateful_sets
            }),
            one_watch(listing(items::<DaemonSet>("daemonsets")), |s| {
                &mut s.daemon_sets
            }),
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

/// **One argument names the watch, and it is the only one** (NOTES § D162) — `updates` takes the
/// [`Watch`] the stream feeds rather than a method plus somewhere to file a failure, so no test
/// here can wire a pod stream to the node watch's failures and still compile.
fn one_watch<K, T>(
    events: Vec<watcher::Result<Event<K>>>,
    of: fn(&mut Store) -> &mut Watch<T>,
) -> BoxStream<'static, Update>
where
    K: Send + 'static,
    T: Watched + From<K> + Bounded + Send + 'static,
{
    updates(stream::iter(events), of)
}

/// What a caller reads for one watch, or `None` when that watch has nothing wrong with it.
fn trouble_for(store: &Store, kind: ObjectKind) -> Option<Trouble<'_>> {
    store.troubles().into_iter().find(|t| t.kind == kind)
}

/// Every watch that is *failing*, as opposed to merely finished. `ended` is true of every stream
/// in this file — `stream::iter` runs out — so a failure assertion says which watch, and an
/// `ended` assertion is its own test below.
fn failing_kinds(store: &Store) -> Vec<ObjectKind> {
    store
        .troubles()
        .into_iter()
        .filter(|t| t.failure.is_some())
        .map(|t| t.kind)
        .collect()
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
        vec![one_watch(pod_events().into_iter().map(Ok).collect(), |s| {
            &mut s.pods
        })],
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
            one_watch(listing(items::<Pod>("kube-system-pods")), |s| &mut s.pods),
            one_watch(listing(items::<Node>("nodes")), |s| &mut s.nodes),
            one_watch(listing(items::<Deployment>("deployments")), |s| {
                &mut s.deployments
            }),
            one_watch(listing(items::<StatefulSet>("statefulsets")), |s| {
                &mut s.stateful_sets
            }),
            one_watch(listing(items::<DaemonSet>("daemonsets")), |s| {
                &mut s.daemon_sets
            }),
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
///
/// **Three failure positions are fed and only one thing is asserted: that the events after them
/// arrived.** An `Err` in the middle of an initial LIST is a shape kube cannot produce under
/// `ListWatch` — both list failures return `State::Empty` and re-`Init` (`watcher.rs:568`,
/// `:584`, `:523`) — so it is fed for coverage (NOTES § D29) and nothing is asserted about *what
/// it did to the half-filled list*. That question is unruled, and the sibling test
/// [`a_page_that_fails_restarts_the_list_and_the_pages_before_it_never_land`] asserts the
/// opposite outcome for the shape that is reachable. **Counting the pods would have pinned the
/// unruled half by accident**; naming a pod that arrived after the last `Err` pins the
/// requirement instead.
#[tokio::test]
async fn a_failed_watch_does_not_end_the_loop() {
    let mut events: Vec<watcher::Result<Event<Pod>>> = vec![Err(watcher::Error::NoResourceVersion)];
    events.extend(listing(items::<Pod>("kube-system-pods")));
    events.insert(3, Err(watcher::Error::NoResourceVersion));
    events.push(Err(watcher::Error::NoResourceVersion));
    events.push(Ok(Event::Apply(object::<Pod>("crashloop"))));

    let mut store = all_but("pods");
    drive(vec![one_watch(events, |s| &mut s.pods)], &mut store).await;
    let snapshot = store
        .snapshot(now())
        .expect("the LIST landed after the failures and the gate opened");
    assert!(
        snapshot
            .pods
            .iter()
            .any(|pod| pod.id.name == "broken-crashloop"),
        "the loop stopped at one of the three failures and the event after them never arrived"
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
    drive(vec![one_watch(events, |s| &mut s.pods)], &mut store).await;
    assert!(
        store.snapshot(now()).is_none(),
        "a watch that failed part-way through its first LIST published a partial cluster"
    );
    assert_eq!(
        failing_kinds(&store),
        vec![ObjectKind::Pod],
        "the failure was swallowed, or landed on a watch that did not raise it: nothing on the \
         store says the pod watch broke"
    );
}

/// **A failure is not erased by the next thing that goes right.** Four healthy watches deliver
/// ordinary traffic every second, so a failure cleared by any success is a failure nobody can
/// ever see; the fifth watch's 403 has to survive the other four.
#[tokio::test]
async fn a_failure_survives_the_events_that_follow_it() {
    let mut store = all_but("pods");
    drive(
        vec![one_watch::<Pod, _>(
            vec![Err(watcher::Error::NoResourceVersion), Ok(Event::Init)],
            |s| &mut s.pods,
        )],
        &mut store,
    )
    .await;
    assert_eq!(
        failing_kinds(&store),
        vec![ObjectKind::Pod],
        "an `Init` arrived after the failure and took the record of it away — but kube returns \
         `Init` before the request is made, so it is no evidence the watch recovered"
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
            one_watch::<Pod, _>(
                vec![
                    Err(watcher::Error::NoResourceVersion),
                    Err(watcher::Error::NoResourceVersion),
                ],
                |s| &mut s.pods,
            ),
            one_watch(listing(items::<Node>("nodes")), |s| &mut s.nodes),
            one_watch(listing(items::<Deployment>("deployments")), |s| {
                &mut s.deployments
            }),
            one_watch(listing(items::<StatefulSet>("statefulsets")), |s| {
                &mut s.stateful_sets
            }),
            one_watch(listing(items::<DaemonSet>("daemonsets")), |s| {
                &mut s.daemon_sets
            }),
        ],
        &mut store,
    )
    .await;
    assert!(
        store.snapshot(now()).is_none(),
        "the pod watch never listed and the other four published the cluster without it"
    );
    assert_eq!(
        failing_kinds(&store),
        vec![ObjectKind::Pod],
        "the failure was swallowed, or the four healthy watches were reported as failing too"
    );

    list(&mut store, Store::pod, items::<Pod>("kube-system-pods"));
    let snapshot = store
        .snapshot(now())
        .expect("the fifth watch answered and the gate opened");
    assert!(
        !snapshot.nodes.is_empty() && !snapshot.workloads.is_empty(),
        "the four watches that succeeded lost their objects to the one that failed"
    );
}

/// **The defect NOTES § D145 could only refuse by never clearing: four healthy watches erasing a
/// fifth's standing failure with their own ordinary traffic.** The old field was store-wide, so
/// *cleared by the next event* meant cleared by somebody else's event. Here the four healthy
/// watches deliver a full bootstrap **and then a stream of `Apply`s after** the pod watch has
/// already failed, and the pod watch's failure has to be exactly where it was.
///
/// **Two `drive` calls and not one**, because `select_all` interleaves and the point of the test
/// is the *order*: the traffic has to arrive after the failure, or the test proves nothing about
/// erasing it.
#[tokio::test]
async fn ordinary_traffic_on_the_other_watches_does_not_clear_a_failing_one() {
    let mut store = Store::default();
    drive(
        vec![
            one_watch::<Pod, _>(vec![Err(watcher::Error::NoResourceVersion)], |s| {
                &mut s.pods
            }),
            one_watch(listing(items::<Node>("nodes")), |s| &mut s.nodes),
            one_watch(listing(items::<Deployment>("deployments")), |s| {
                &mut s.deployments
            }),
            one_watch(listing(items::<StatefulSet>("statefulsets")), |s| {
                &mut s.stateful_sets
            }),
            one_watch(listing(items::<DaemonSet>("daemonsets")), |s| {
                &mut s.daemon_sets
            }),
        ],
        &mut store,
    )
    .await;
    assert_eq!(
        failing_kinds(&store),
        vec![ObjectKind::Pod],
        "the pod watch's failure did not survive its own bootstrap round"
    );

    // Every healthy watch now delivers ordinary traffic, which is what a live cluster does every
    // second of the day.
    drive(
        vec![
            one_watch(
                items::<Node>("nodes")
                    .into_iter()
                    .map(Event::Apply)
                    .map(Ok)
                    .collect(),
                |s| &mut s.nodes,
            ),
            one_watch(
                items::<Deployment>("deployments")
                    .into_iter()
                    .map(Event::Apply)
                    .map(Ok)
                    .collect(),
                |s| &mut s.deployments,
            ),
        ],
        &mut store,
    )
    .await;

    let pods = trouble_for(&store, ObjectKind::Pod).expect("the pod watch reported nothing");
    println!(
        "what a caller reads · kind {:?} · failing {} · ended {} · text {:?}",
        pods.kind,
        pods.failure.is_some(),
        pods.ended,
        pods.failure.map(ToString::to_string),
    );
    assert!(
        pods.failure.is_some(),
        "the other watches' ordinary traffic erased the pod watch's failure, which is the whole \
         of the defect NOTES § D145 had to make the field monotone to refuse"
    );
    assert_eq!(
        failing_kinds(&store),
        vec![ObjectKind::Pod],
        "a watch that never failed was reported as failing"
    );
}

/// **A watch that has already listed recovers *without* re-listing, and that is measured off
/// kube rather than reasoned about** (NOTES § D162). `State::Watching` on `Some(Err(err))`
/// returns `Error::WatchFailed` and goes straight back to `State::Watching { stream }` —
/// `kube-runtime-4.2.0/src/watcher.rs:706-712` — so the next thing an established watch emits
/// after a blip is an ordinary `Apply`, with no `Init` and no `InitDone` anywhere near it.
///
/// **A clear point of *the next complete LIST* would therefore never fire on this path** and the
/// banner would stand for the rest of the session after one blip: D145's named cost, which per
/// watch identity exists to stop paying.
#[tokio::test]
async fn an_established_watch_recovers_on_ordinary_traffic_with_no_relist() {
    let mut events = listing(items::<Pod>("kube-system-pods"));
    events.push(Err(watcher::Error::NoResourceVersion));
    events.push(Ok(Event::Apply(object::<Pod>("crashloop"))));

    let mut store = all_but("pods");
    drive(vec![one_watch(events, |s| &mut s.pods)], &mut store).await;

    assert_eq!(
        failing_kinds(&store),
        Vec::new(),
        "an established watch delivered again after a blip and the store still calls it failing"
    );
    let snapshot = store.snapshot(now()).expect("every initial LIST landed");
    assert!(
        snapshot
            .pods
            .iter()
            .any(|pod| pod.id.name == "broken-crashloop"),
        "the Apply that proved the watch had recovered never reached the store"
    );
}

/// **The `Init` that opens a relist is a LIST beginning, not a LIST landing, so it clears
/// nothing.** kube returns it from `State::Empty` with no `.await`, before the request is made
/// (`watcher.rs:522-527`), so a watch whose LIST is refused forever emits `Err`, `Init`, `Err`,
/// `Init` — and a clear on `Init` would leave it reading healthy on about half the instants a
/// screen sampled it. That is NOTES § D145's defect rebuilt inside one watch.
///
/// **Four of these tests pin the boundary between them, and none of them alone does.** The clear
/// point is *this watch delivered a complete answer* (NOTES § D162): `Init` here and `InitApply`
/// in [`a_relist_in_flight_does_not_withdraw_the_failure_it_is_answering`] are the two that do
/// not qualify, and [`a_page_that_fails_restarts_the_list_and_the_pages_before_it_never_land`]
/// (a finished LIST) and [`an_established_watch_recovers_on_ordinary_traffic_with_no_relist`]
/// (ordinary traffic on a watch that already listed) are the two that do.
#[tokio::test]
async fn a_watch_that_keeps_failing_its_list_is_never_reported_healthy() {
    let mut events: Vec<watcher::Result<Event<Pod>>> = Vec::new();
    for _ in 0..4 {
        events.push(Err(watcher::Error::NoResourceVersion));
        events.push(Ok(Event::Init));
    }
    let mut store = all_but("pods");
    drive(vec![one_watch(events, |s| &mut s.pods)], &mut store).await;

    assert_eq!(
        failing_kinds(&store),
        vec![ObjectKind::Pod],
        "a watch that has done nothing but fail and restart its LIST was reported healthy, \
         because the `Init` it emits before every attempt was read as a LIST landing"
    );
    assert!(
        store.snapshot(now()).is_none(),
        "a watch that never listed published a cluster anyway (NOTES § D28)"
    );
}

/// **A watch is not declared recovered by an event it is not trusted to be listed by**
/// (NOTES § D162). An `InitDone` with no `Init` before it is the broken stream `Watch::take`
/// already refuses to publish from — *your cluster has no pods* — and the same distrust has to
/// reach the failure beside it, or one file says two things about one event.
///
/// kube never sends that sequence (`watcher.rs:548`, `:555-559` are reached only from
/// `State::InitPage`, which only `Init` enters), so this is defensive exactly as that gate is —
/// and it is a shape a synthetic stream can feed and a real one cannot, which is the whole
/// reason to feed it here.
#[tokio::test]
async fn an_init_done_that_never_had_an_init_neither_publishes_nor_clears() {
    let mut store = all_but("pods");
    drive(
        vec![one_watch::<Pod, _>(
            vec![Err(watcher::Error::NoResourceVersion), Ok(Event::InitDone)],
            |s| &mut s.pods,
        )],
        &mut store,
    )
    .await;

    assert_eq!(
        failing_kinds(&store),
        vec![ObjectKind::Pod],
        "an `InitDone` this watch never saw an `Init` for was read as a recovery, while the \
         very same event was distrusted too much to publish a single pod from"
    );
    assert!(
        store.snapshot(now()).is_none(),
        "a watch that never listed published a cluster anyway (NOTES § D28)"
    );
}

/// **A stream that finishes is recorded on its own watch instead of being dropped in silence**
/// (NOTES § D162). `select_all` removes a finished stream and carries on with the rest, so
/// without the marker [`updates`] appends, the pod watch here would keep answering with the pods
/// it last held and nothing anywhere would say they had stopped arriving.
///
/// **The gate is deliberately left open.** This watch listed completely before it ended, so what
/// it holds is a real answer that is merely no longer fresh; closing the gate would replace
/// *stale, and it says so* with *nothing, and it does not*.
#[tokio::test]
async fn a_stream_that_ends_is_recorded_and_the_other_watches_are_not() {
    let mut store = all_but("pods");
    drive(
        vec![one_watch(listing(items::<Pod>("kube-system-pods")), |s| {
            &mut s.pods
        })],
        &mut store,
    )
    .await;

    let ended: Vec<ObjectKind> = store
        .troubles()
        .into_iter()
        .filter(|t| t.ended)
        .map(|t| t.kind)
        .collect();
    println!(
        "watches that stopped: {ended:?} · failing: {:?}",
        failing_kinds(&store)
    );
    assert_eq!(
        ended,
        vec![ObjectKind::Pod],
        "a stream that ran out was dropped by `select_all` without a word, so its kind is \
         presented as live"
    );
    assert_eq!(
        failing_kinds(&store),
        Vec::new(),
        "a stream that ended cleanly was reported as having failed"
    );
    assert!(
        store.snapshot(now()).is_some(),
        "a watch that listed completely and then ended had its whole cluster withheld"
    );
}

/// **A failure is not withdrawn by the relist that is still in flight to answer it**
/// (NOTES § D162). After `NoResourceVersion` (`watcher.rs:568`) and `InitialListFailed` (`:584`)
/// kube returns `State::Empty`, which emits `Init` (`:523`) and then `InitApply`s (`:548`) — so
/// the objects arriving after those two errors are a **relist that has not finished**, and the
/// first of them is not the watch delivering an answer.
///
/// **The half that makes it a defect rather than a wrong word is the second assertion.**
/// `complete` is never reset, so a relisting watch is invisible to `still_listing` too. A clear
/// here takes both facts quiet at once — and a relist is sitting in `api.list()`, the half of
/// `k8s.rs` § WHAT A THROTTLE LOOKS LIKE that has **no** deadline: a watch poll unblocks at
/// ~295 s, a LIST against a dead socket never does. The store would read healthy for the life of
/// the process while serving a cluster from before the failure.
///
/// The `InitDone` at the end is what is allowed to clear it, and it is asserted too, or this
/// test would pass on a watch that never recovers at all.
#[tokio::test]
async fn a_relist_in_flight_does_not_withdraw_the_failure_it_is_answering() {
    let pods = items::<Pod>("kube-system-pods");
    let mut store = all_but("pods");
    drive(
        vec![one_watch(listing(pods.clone()), |s| &mut s.pods)],
        &mut store,
    )
    .await;
    assert_eq!(
        failing_kinds(&store),
        Vec::new(),
        "a watch that listed cleanly was reported as failing"
    );

    // The 410 a compacted `continue` token produces, then the relist kube starts for it — its
    // `Init` and its first object, and no more.
    let mut relist: Vec<watcher::Result<Event<Pod>>> = vec![
        Err(watcher::Error::InitialListFailed(kube::Error::Api(
            Box::new(kube::core::Status {
                code: 410,
                reason: "Expired".to_string(),
                ..Default::default()
            }),
        ))),
        Ok(Event::Init),
    ];
    relist.push(Ok(Event::InitApply(
        pods.first().cloned().expect("the capture holds pods"),
    )));
    drive(vec![one_watch(relist, |s| &mut s.pods)], &mut store).await;

    println!(
        "mid-relist · failing {:?} · outstanding {:?} · pods {:?}",
        failing_kinds(&store),
        outstanding_kinds(&store),
        store.snapshot(now()).map(|c| c.pods.len()),
    );
    assert_eq!(
        failing_kinds(&store),
        vec![ObjectKind::Pod],
        "one object of an unfinished relist withdrew the failure it was sent to answer, and \
         `complete` is still true so nothing else on this store says a LIST is running: the \
         store reads fully healthy while it serves a cluster from before the 410"
    );

    // And the LIST that finishes *is* what clears it, or the rule above would just be "never".
    drive(
        vec![one_watch::<Pod, _>(vec![Ok(Event::InitDone)], |s| {
            &mut s.pods
        })],
        &mut store,
    )
    .await;
    assert_eq!(
        failing_kinds(&store),
        Vec::new(),
        "the relist finished and the failure it answered is still standing"
    );
}

/// **All five rows of `troubles()` are exercised, not just the first.** Every other assertion in
/// this file names `ObjectKind::Pod`, so deleting a row from the five-entry table in
/// `Store::troubles` broke nothing — the `write-guard.py` `CANARIES` rule, one file over: a
/// derived list has to assert it found something, and finding only the first entry is not that.
///
/// The sibling of [`a_bootstrap_that_is_still_running_names_the_lists_it_is_waiting_for`], off
/// the same [`kind_of_stream`] table so the two cannot drift.
#[tokio::test]
async fn every_watch_reports_its_own_failure_under_its_own_kind() {
    for (failing, kind) in kind_of_stream() {
        let store = five_watches_one_failing(failing).await;
        assert_eq!(
            failing_kinds(&store),
            vec![kind.clone()],
            "the {failing} watch failed and `troubles()` did not report it as {kind:?} — a row \
             of the five-kind table is missing, or names the wrong watch"
        );
    }
}

/// The five watches driven together with exactly one of them delivering nothing but an error.
async fn five_watches_one_failing(failing: &str) -> Store {
    fn or_broken<K>(
        events: Vec<watcher::Result<Event<K>>>,
        broken: bool,
    ) -> Vec<watcher::Result<Event<K>>> {
        if broken {
            vec![Err(watcher::Error::NoResourceVersion)]
        } else {
            events
        }
    }
    assert!(
        kind_of_stream().iter().any(|(name, _)| *name == failing),
        "{failing} is not one of the five streams"
    );
    let mut store = Store::default();
    drive(
        vec![
            one_watch(
                or_broken(listing(items::<Pod>("kube-system-pods")), failing == "pods"),
                |s| &mut s.pods,
            ),
            one_watch(
                or_broken(listing(items::<Node>("nodes")), failing == "nodes"),
                |s| &mut s.nodes,
            ),
            one_watch(
                or_broken(
                    listing(items::<Deployment>("deployments")),
                    failing == "deployments",
                ),
                |s| &mut s.deployments,
            ),
            one_watch(
                or_broken(
                    listing(items::<StatefulSet>("statefulsets")),
                    failing == "statefulsets",
                ),
                |s| &mut s.stateful_sets,
            ),
            one_watch(
                or_broken(
                    listing(items::<DaemonSet>("daemonsets")),
                    failing == "daemonsets",
                ),
                |s| &mut s.daemon_sets,
            ),
        ],
        &mut store,
    )
    .await;
    store
}

/// **`ended` is never cleared, and this is what pins it** (NOTES § D162). It is an *absent*
/// line, so `cargo mutants` has nothing to mutate: the guard has to be a test or it is nothing.
///
/// **Fed through [`Store::pod`] and not through a second [`drive`], and the first draft of this
/// test was the second and proved nothing.** [`updates`] appends the end marker as its stream's
/// last item, so a second `drive` sets `ended` back to `true` on its way out whatever
/// [`Watch::take`] did in between — the assertion passed with the clear planted. The claim lives
/// in `take`, so the events go into `take`.
///
/// **A watch that stopped and was resubscribed is not a watch that never stopped**, and the
/// objects landing again may not be allowed to make it look like one.
#[tokio::test]
async fn objects_arriving_again_do_not_take_back_the_end_of_a_watch() {
    let mut store = all_but("pods");
    drive(
        vec![one_watch(listing(items::<Pod>("kube-system-pods")), |s| {
            &mut s.pods
        })],
        &mut store,
    )
    .await;
    assert!(
        trouble_for(&store, ObjectKind::Pod).is_some_and(|t| t.ended),
        "the stream ran out and nothing recorded it"
    );

    // Everything a resubscribed watch delivers, straight into the store: a whole fresh LIST and
    // then ordinary traffic. Every one of these clears a *failure*; none may clear this.
    for event in listing(items::<Pod>("kube-system-pods")) {
        store.pod(&now(), event.expect("the synthesised LIST holds no errors"));
    }
    store.pod(&now(), Event::Apply(object::<Pod>("crashloop")));

    assert!(
        trouble_for(&store, ObjectKind::Pod).is_some_and(|t| t.ended),
        "a whole LIST and an Apply landed on a watch whose stream had ended and took the record \
         of it away, so a kind nothing is watching any more is presented as live"
    );
}

/// **There is no retry budget, and this is the count that would have caught one.** k9s called
/// `BailOut` after five ([#3922](https://github.com/derailed/k9s/issues/3922)), so a VPN blip
/// over lunch meant the tool was gone on return. Twenty failures here, and the LIST that follows
/// them still lands.
///
/// It is a different question from [`a_failed_watch_does_not_end_the_loop`], which catches a `?`:
/// a budget is not an early `return`, it is a counter, and a counter survives every test that
/// only feeds one or two failures.
#[tokio::test]
async fn twenty_failures_do_not_use_up_a_budget_that_does_not_exist() {
    let pods = items::<Pod>("kube-system-pods");
    let mut events: Vec<watcher::Result<Event<Pod>>> = (0..20)
        .map(|_| Err(watcher::Error::NoResourceVersion))
        .collect();
    events.extend(listing(pods.clone()));

    let mut store = all_but("pods");
    drive(vec![one_watch(events, |s| &mut s.pods)], &mut store).await;

    let snapshot = store
        .snapshot(now())
        .expect("the LIST after twenty failures landed and the gate opened");
    assert_eq!(
        snapshot.pods.len(),
        pods.len(),
        "the loop stopped somewhere in twenty failures, so there is a budget after all"
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
         (watcher.rs:400, :414) — the watch would then close and reconnect on that period \
         instead of ~295s. It would NOT bound the LIST: `to_list_params` copies the field but \
         `ListParams::populate_qp` never serialises it (kube-core params.rs:94-122, and :381 is \
         the only timeoutSeconds in that crate, on the watch builder)"
    );
}

/// **A LIST that arrives in pages is still one LIST** (NOTES § D147, D28). The gate is shut at
/// every page boundary — not merely before the first one — it opens once, and it opens on the
/// **`Config::timeout` bounds the watch and does not reach the LIST at all** — the fact
/// `k8s.rs` § WHAT A THROTTLE LOOKS LIKE now rests on, pinned here because it is a claim about a
/// dependency and the doc that makes it cannot notice kube changing its mind.
///
/// `to_list_params` copies the field into the `ListParams` (`watcher.rs:400`) and
/// `ListParams::populate_qp` never serialises it, so a deadline set there is dropped by the
/// query builder one screen away — while `ListParams::timeout`'s own doc claims *"Defaults to
/// 290s"* (`kube-core-4.2.0/src/params.rs:137-139`). **Asserted off the URL kube would actually
/// send** rather than off either doc, and the watch is asserted beside it or the test would pass
/// just as well on a build that had lost `timeoutSeconds` everywhere.
#[test]
fn a_list_timeout_never_reaches_the_wire_and_a_watch_one_does() {
    let request = kube::core::Request::new("/api/v1/pods");
    let listed = request
        .list(&kube::api::ListParams {
            timeout: Some(60),
            ..Default::default()
        })
        .expect("a LIST request builds");
    let watched = request
        .watch(
            &kube::api::WatchParams {
                timeout: Some(60),
                ..Default::default()
            },
            "1",
        )
        .expect("a watch request builds");
    println!("LIST  {}\nWATCH {}", listed.uri(), watched.uri());
    assert!(
        !listed.uri().to_string().contains("timeoutSeconds"),
        "kube now sends the list timeout, so § WHAT A THROTTLE LOOKS LIKE's \"the initial LIST \
         is unbounded\" has stopped being true: {}",
        listed.uri()
    );
    assert!(
        watched.uri().to_string().contains("timeoutSeconds=60"),
        "the watch stopped carrying the timeout, so the ~295s bound the same section claims for \
         a severed watch is gone too: {}",
        watched.uri()
    );
}

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
    drive(vec![one_watch(events, |s| &mut s.pods)], &mut store).await;

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
    // **The 410 is cleared by the relist that answered it, and that is the point of the box.**
    // The old store-wide field could not clear, because it could not tell whose failure it held
    // (NOTES § D145); this one can, because the events that cleared it arrived on the same watch
    // (NOTES § D162). What survives is the *observation* — the pages the dead attempt delivered
    // were still thrown away, asserted above.
    assert_eq!(
        failing_kinds(&store),
        Vec::new(),
        "the LIST that failed was answered by a complete relist on the same watch and the store \
         still reports it as failing"
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
        vec![one_watch::<Pod, _>(
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
            |s| &mut s.pods,
        )],
        &mut store,
    )
    .await;

    let refused = trouble_for(&store, ObjectKind::Pod).expect("the pod watch reported nothing");
    let code = match refused.failure {
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
    drive(vec![one_watch(opening, |s| &mut s.pods)], &mut store).await;
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

/// **The invisible characters `char::is_control` does not answer for**, one per class the
/// operator review measured on 2026-08-22 and both ends of every range the guard names: bidi
/// marks and overrides, a bidi isolate, a zero-width space, a word joiner, a soft hyphen and a
/// byte-order mark.
const INVISIBLE: [(&str, char); 8] = [
    ("U+200B ZERO WIDTH SPACE", '\u{200b}'),
    ("U+200F RIGHT-TO-LEFT MARK", '\u{200f}'),
    ("U+202A LEFT-TO-RIGHT EMBEDDING", '\u{202a}'),
    ("U+202E RIGHT-TO-LEFT OVERRIDE", '\u{202e}'),
    ("U+2060 WORD JOINER", '\u{2060}'),
    ("U+206F NOMINAL DIGIT SHAPES", '\u{206f}'),
    ("U+00AD SOFT HYPHEN", '\u{00ad}'),
    ("U+FEFF ZERO WIDTH NO-BREAK SPACE", '\u{feff}'),
];

/// **A character that prints as nothing dies at ingest too, and `char::is_control` is not what
/// says so** (invariant 9, NOTES § D154). `is_control` is Unicode `Cc` and nothing else, so
/// every codepoint above walked through the guard until this test was written: `prod\u{202e}dc`
/// is a row that reads *prodcd* on the screen and matches neither in a search, which is Trojan
/// Source in a cell.
///
/// **Each class is fed on its own and at four framings** (NOTES § D29, § D31): the whole value,
/// inside one, and at either end — and down all three routes, one of which
/// (`metadata.finalizers[0]`) is an element of an array rather than a field of its own.
///
/// **None of them becomes a space.** They are not `char::is_whitespace`, so nothing here is a
/// word boundary that a deletion would glue shut — `prod` and `dc` were never two words.
#[test]
fn an_invisible_character_dies_at_ingest_like_a_control_one() {
    for (label, invisible) in INVISIBLE {
        for (shape, poison, survives) in [
            ("the whole value", invisible.to_string(), ""),
            ("inside a value", format!("prod{invisible}dc"), "proddc"),
            ("at the front", format!("{invisible}leading"), "leading"),
            ("at the back", format!("trailing{invisible}"), "trailing"),
        ] {
            for name in ROUTES {
                let stored = kept(name, &poisoned(name, &poison));
                if name == "message" {
                    println!("{shape:>16}  {poison:?} -> {stored:?}   ({label})");
                }
                assert_eq!(
                    stored, survives,
                    "{label} as {shape} survived the {name} route: {stored:?}"
                );
                assert!(
                    !stored.chars().any(unprintable),
                    "{label} as {shape} reached a screen through the {name} route"
                );
            }
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

/// One captured object, through the real ingest path, as the `Debug` of what was kept. `None` for
/// a kind nothing in `k8s.rs` decodes.
///
/// Each of the three workload kinds goes down its own stream, because each is a different API
/// type — the one thing the driver's `Store::deployment` / `stateful_set` / `daemon_set` split
/// exists for. **A `Table` is not a watched object and is swept here anyway**: its cells and column
/// headers are API free text through the same one door, so the two sweeps below cover the browser's
/// rows for free rather than through a second copy of themselves.
fn ingested_dump(kind: &str, document: serde_json::Value) -> Option<String> {
    let mut store;
    let workloads = match kind {
        "Table" => {
            let response: TableResponse =
                serde_json::from_value(document).expect("a captured Table decodes");
            let decoded: Table = ingest(response);
            return Some(format!("{decoded:?}"));
        }
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

/// Every type one source file declares, with its fields. `rules.rs` for the snapshot types
/// below, `k8s.rs` for [`Browsable`] — one parser rather than two that could disagree.
fn declared_types(
    source: &'static str,
) -> BTreeMap<&'static str, Vec<(&'static str, &'static str)>> {
    let mut types = BTreeMap::new();
    let mut open: Option<(&'static str, Vec<(&'static str, &'static str)>)> = None;
    for line in source.lines() {
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
    let types = declared_types(RULES_SOURCE);
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

// --- RESOLVING AN OWNER ---
//
// **Every ReplicaSet below is a committed capture** (NOTES § D53) and the joins between them are
// the cluster's own: `owned-pods.json`'s single pod names `owned-replicasets.json`'s single
// ReplicaSet by uid, and that ReplicaSet names a Deployment by uid. Nothing here is edited to
// make a test pass; the one synthesized shape is the empty-uid `ownerReference`, which the API
// server rejects and no capture can hold (NOTES § D40), and it says so where it is built.
//
// **What is not proven here is the `get`**: there is no `Client` in this build, so every fetch
// below is its *answer*, handed to `Store::owner_fetched` the way the `connect()` box will hand
// it one. What the network does is that box's to show.

/// The one ReplicaSet of a single-item capture.
fn replica_set(name: &str) -> ReplicaSet {
    let mut sets = items::<ReplicaSet>(name);
    assert_eq!(
        sets.len(),
        1,
        "{name}.json holds more than one ReplicaSet, so `the` is the wrong word for it"
    );
    sets.remove(0)
}

/// A store whose five watches have listed, with one named pod capture in place of the default.
fn store_with_pods(capture: &str) -> Store {
    let mut store = all_but("pods");
    list(&mut store, Store::pod, items::<Pod>(capture));
    store
}

fn api_error(code: u16, reason: &str) -> kube::Error {
    kube::Error::Api(
        kube::core::Status::failure("refused", reason)
            .with_code(code)
            .boxed(),
    )
}

/// **The whole point of the box, and the uid is what proves it was not a string operation.**
///
/// `broken-owned-7bdb7645c8` minus its hash is `broken-owned`, so the *name* alone cannot tell a
/// resolution from a chopped suffix. The Deployment's **uid** can: it is nowhere in the pod, in
/// the pod's `ownerReference`, or in the ReplicaSet's name, and only the ReplicaSet's own
/// `ownerReferences` carries it.
#[test]
fn the_group_reads_the_deployment_and_the_uid_is_what_proves_it() {
    let set = replica_set("owned-replicasets");
    let deployment = set
        .metadata
        .owner_references
        .clone()
        .expect("the captured ReplicaSet has no ownerReferences")
        .remove(0);
    assert_eq!(deployment.kind, "Deployment");
    assert!(
        !set.metadata.name.as_deref().unwrap_or_default().is_empty()
            && set.metadata.name.as_deref() != Some(deployment.name.as_str()),
        "the capture's ReplicaSet and Deployment share a name, so this test could not tell a \
         resolution from doing nothing"
    );
    assert!(
        !deployment
            .name
            .contains(deployment.uid.split('-').next().unwrap_or("")),
        "the Deployment's uid would be readable out of its name, which would weaken the \
         assertion below"
    );

    let mut store = store_with_pods("owned-pods");
    let before = store.snapshot(now()).expect("every watch listed");
    let pod = pod_named(&before, "broken-owned-7bdb7645c8-bwdfd");
    assert_eq!(
        (&pod.owner.kind, pod.owner.name.as_str()),
        (&ObjectKind::ReplicaSet, "broken-owned-7bdb7645c8"),
        "with nothing fetched the owner is the ReplicaSet, and nothing may chop its hash off"
    );

    let want = store.unresolved_owners();
    assert_eq!(want.len(), 1);
    assert_eq!(want[0].why, Why::NotAsked);
    store.owner_fetched(&want[0].id, Ok(set));

    let after = store.snapshot(now()).expect("every watch listed");
    let pod = pod_named(&after, "broken-owned-7bdb7645c8-bwdfd");
    assert_eq!(
        (
            &pod.owner.kind,
            pod.owner.namespace.as_deref(),
            pod.owner.name.as_str(),
            pod.owner.uid.as_deref()
        ),
        (
            &ObjectKind::Deployment,
            Some("default"),
            deployment.name.as_str(),
            Some(deployment.uid.as_str())
        ),
        "the card must file under the Deployment the reader deployed, uid included"
    );
    assert!(
        store.unresolved_owners().is_empty(),
        "the answer landed, so nothing is outstanding"
    );
}

/// **One entry per ReplicaSet, not one per pod** — two copies of one workload are one fetch and
/// one card (NOTES § D3).
#[test]
fn many_pods_of_one_replicaset_are_one_reference_and_one_group() {
    let mut store = store_with_pods("healthy-deploy-pods");
    let pods = items::<Pod>("healthy-deploy-pods");
    assert_eq!(pods.len(), 2, "the capture must hold the two copies");

    let want = store.unresolved_owners();
    assert_eq!(
        want.len(),
        1,
        "two pods named one ReplicaSet and it must be asked about once"
    );
    store.owner_fetched(&want[0].id, Ok(replica_set("healthy-replicasets")));

    let snapshot = store.snapshot(now()).expect("every watch listed");
    let owners: Vec<&ObjectId> = snapshot.pods.iter().map(|pod| &pod.owner).collect();
    assert_eq!(owners.len(), 2);
    assert_eq!(
        owners[0], owners[1],
        "both copies must land on one group key"
    );
    assert_eq!(
        (&owners[0].kind, owners[0].name.as_str()),
        (&ObjectKind::Deployment, "healthy-deploy")
    );
}

/// **A refusal, a deletion and a dead socket are three different facts**, and none of them may
/// become *the group is called `broken-owned-7bdb7645c8`* with nothing said about why.
///
/// The owner is checked in every arm as well as the fact: a failed fetch must leave the pod's
/// true controller in place, never a guess at the name above it.
#[test]
fn a_refusal_a_deletion_and_a_dead_socket_are_three_different_facts() {
    for (answer, expected) in [
        (api_error(403, "Forbidden"), Why::Refused),
        (api_error(404, "NotFound"), Why::Gone),
        (api_error(500, "InternalError"), Why::Failed),
        (
            kube::Error::Service(Box::new(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out",
            ))),
            Why::Failed,
        ),
    ] {
        let mut store = store_with_pods("owned-pods");
        let want = store.unresolved_owners();
        store.owner_fetched(&want[0].id, Err(answer));

        let outstanding = store.unresolved_owners();
        assert_eq!(
            outstanding.len(),
            1,
            "a failed fetch must stay reportable, not vanish"
        );
        assert_eq!(outstanding[0].why, expected);
        assert_eq!(
            outstanding[0].id, want[0].id,
            "the reference reported back is the one that was asked about"
        );

        let snapshot = store.snapshot(now()).expect("every watch listed");
        let pod = pod_named(&snapshot, "broken-owned-7bdb7645c8-bwdfd");
        assert_eq!(
            (&pod.owner.kind, pod.owner.name.as_str()),
            (&ObjectKind::ReplicaSet, "broken-owned-7bdb7645c8"),
            "a fetch that failed must leave the ReplicaSet, which is true, rather than a name \
             nothing answered for"
        );
        assert!(
            snapshot
                .workloads
                .iter()
                .all(|w| w.id.kind != ObjectKind::ReplicaSet),
            "nothing was resolved, so no ReplicaSet may reach the workload list"
        );
    }
}

/// **kube's own `is_forbidden` and `is_not_found` cannot decide this**, which is why `why` reads
/// `code` as well as `reason`. Measured against the crate rather than read off its doc.
///
/// `Status::reason_or_code` is `self.reason == reason || (!is_known(reason) && self.code ==
/// code)`, and the `reason` handed to `is_known` is the **constant** the helper was called with
/// — always known — so the `code` half never runs. Go's original tests the *response's* reason
/// there. The shape it costs is the one kube builds itself when a refusal does not parse as a
/// `Status`: `Status::failure(text, "Failed to parse error data").with_code(403)`
/// (`kube-client-4.2.0/src/client/mod.rs:556`), which is what an authorizing proxy in front of
/// the API server produces.
#[test]
fn a_refusal_that_carries_only_a_status_code_is_still_a_refusal() {
    let proxy = kube::core::Status::failure("", "Failed to parse error data").with_code(403);
    assert!(
        !proxy.is_forbidden(),
        "kube's helper started answering this, so `why` can be simplified to use it"
    );
    assert_eq!(why(&kube::Error::Api(proxy.boxed())), Why::Refused);

    let deleted = kube::core::Status::failure("", "Failed to parse error data").with_code(404);
    assert!(!deleted.is_not_found());
    assert_eq!(why(&kube::Error::Api(deleted.boxed())), Why::Gone);

    // And the other way round: the reason with no code, which is what the API server's own
    // `Status` body carries when `code` is absent.
    for (reason, expected) in [("Forbidden", Why::Refused), ("NotFound", Why::Gone)] {
        assert_eq!(
            why(&kube::Error::Api(
                kube::core::Status::failure("refused", reason).boxed()
            )),
            expected,
            "a {reason} with no HTTP code must still be read"
        );
    }
}

/// **A name can be re-used, so the uid decides.** A rollback re-creates a ReplicaSet with the
/// same generated hash and a new uid; the `get` goes out by name and can bring back a different
/// object than the pod named.
#[test]
fn an_object_that_comes_back_under_another_uid_is_not_the_one_that_was_asked_about() {
    let asked = replica_set("owned-replicasets");
    let other = replica_set("healthy-replicasets");
    assert_ne!(
        asked.metadata.uid, other.metadata.uid,
        "the two captures must differ in uid or this test proves nothing"
    );

    let mut store = store_with_pods("owned-pods");
    let want = store.unresolved_owners();
    store.owner_fetched(&want[0].id, Ok(other));

    assert_eq!(
        store.unresolved_owners()[0].why,
        Why::Gone,
        "a different object under the same question is not an answer"
    );
    let snapshot = store.snapshot(now()).expect("every watch listed");
    assert_eq!(
        pod_named(&snapshot, "broken-owned-7bdb7645c8-bwdfd")
            .owner
            .name,
        "broken-owned-7bdb7645c8",
        "and the pod must not be filed under the other object's Deployment"
    );
}

/// **The cache holds the whole ReplicaSet, not a resolved name** — the box's own clause, because
/// W1 reads `status.conditions[ReplicaFailure]` off this object and files the card under the
/// Deployment above it.
#[test]
fn the_cached_object_is_the_whole_replicaset_and_reaches_the_rules_as_one() {
    let captured = replica_set("owned-replicasets");
    let mut store = store_with_pods("owned-pods");
    let want = store.unresolved_owners();
    store.owner_fetched(&want[0].id, Ok(captured.clone()));

    let snapshot = store.snapshot(now()).expect("every watch listed");
    let resolved: Vec<&WorkloadSnapshot> = snapshot
        .workloads
        .iter()
        .filter(|w| w.id.kind == ObjectKind::ReplicaSet)
        .collect();
    assert_eq!(
        resolved.len(),
        1,
        "W1 is written about a ReplicaSet and reads `workloads`, so the resolved one has to be \
         in it"
    );
    let expected: WorkloadSnapshot = ingest(captured);
    assert_eq!(
        *resolved[0], expected,
        "every field of the ingested object must survive the cache, not only the name"
    );
    assert_eq!(
        resolved[0].owner.name, "broken-owned",
        "and its own owner is what a card drawn about it heads with"
    );

    // The condition W1 actually reads, on the capture that carries one. **It cannot be reached
    // through the cache**: `broken-quota-59654c756` has `status.replicas: 0`, so no pod names it
    // and no owner reference asks for it — see the report on this box.
    let refused: WorkloadSnapshot = ingest(replica_set("quota-replicasets"));
    let failure = refused
        .conditions
        .iter()
        .find(|c| c.type_ == "ReplicaFailure")
        .expect("the quota capture carries the condition W1 is written about");
    assert_eq!(
        (failure.status.as_str(), failure.reason.as_deref()),
        ("True", Some("FailedCreate"))
    );
    assert!(
        failure
            .message
            .as_deref()
            .unwrap_or_default()
            .contains("quota"),
        "the API server's own refusal must survive the ingest verbatim, and it reads {:?}",
        failure.message
    );
}

/// **`replica_sets` is a different list and the cache may not be poured into it** (NOTES § D129).
/// Waste's row is *ReplicaSets parked at 0 replicas*, and a parked one has no pods — which is
/// exactly what an owner cache structurally never holds.
#[test]
fn resolving_an_owner_does_not_answer_the_report_that_lists_every_replicaset() {
    let mut store = store_with_pods("owned-pods");
    let want = store.unresolved_owners();
    store.owner_fetched(&want[0].id, Ok(replica_set("owned-replicasets")));
    assert!(
        store
            .snapshot(now())
            .expect("every watch listed")
            .replica_sets
            .is_none(),
        "nobody listed the ReplicaSets, and `Some` here would tell Waste it had"
    );
}

/// **The cache is bounded by what the pods reference**, so a month of rollouts is not a month of
/// ReplicaSets held in memory.
#[test]
fn an_owner_no_pod_names_any_more_stops_being_cached() {
    let mut store = store_with_pods("owned-pods");
    let want = store.unresolved_owners();
    store.owner_fetched(&want[0].id, Ok(replica_set("owned-replicasets")));
    assert_eq!(
        store
            .snapshot(now())
            .expect("listed")
            .workloads
            .iter()
            .filter(|w| w.id.kind == ObjectKind::ReplicaSet)
            .count(),
        1
    );

    // The pod goes away, and the next answer to land is what sweeps the entry it held open.
    store.pod(&now(), Event::Delete(items::<Pod>("owned-pods").remove(0)));
    store.pod(
        &now(),
        Event::Apply(items::<Pod>("healthy-deploy-pods").remove(0)),
    );
    let second = store.unresolved_owners();
    assert_eq!(
        second.len(),
        1,
        "the new pod's ReplicaSet is the outstanding one"
    );
    store.owner_fetched(&second[0].id, Ok(replica_set("healthy-replicasets")));

    let snapshot = store.snapshot(now()).expect("listed");
    let sets: Vec<&str> = snapshot
        .workloads
        .iter()
        .filter(|w| w.id.kind == ObjectKind::ReplicaSet)
        .map(|w| w.id.name.as_str())
        .collect();
    assert_eq!(
        sets,
        ["healthy-deploy-7f84bdfb9b"],
        "the ReplicaSet nothing names any more must be gone, not merely unreferenced"
    );
}

/// **An `ownerReference` with no uid is never asked about**, because the uid is the cache key
/// and two ReplicaSets sharing the entry `""` would each be handed the other's Deployment.
///
/// **Synthesized, and it names what it is waiting for** (NOTES § D40): `ValidateOwnerReferences`
/// rejects an empty uid, so no capture of a real cluster can carry this. What can produce it is
/// something between k8rs and the API server, which is invariant 9's class of input.
#[test]
fn an_owner_reference_with_no_uid_is_never_asked_about() {
    // Read off the capture rather than off the store, so the plant below is a change to a real
    // reference and not to whatever the store happened to make of one.
    let real = items::<Pod>("owned-pods")[0]
        .metadata
        .owner_references
        .clone()
        .and_then(|refs| refs.into_iter().find(|o| o.controller == Some(true)))
        .expect("the capture's pod has a controlling ownerReference");
    assert!(!real.uid.is_empty(), "and that reference carries a uid");

    let mut blanked = items::<Pod>("owned-pods");
    for owner in blanked[0].metadata.owner_references.iter_mut().flatten() {
        owner.uid = String::new();
    }
    let mut store = all_but("pods");
    list(&mut store, Store::pod, blanked);
    assert!(
        store.unresolved_owners().is_empty(),
        "a reference with no uid must not become a fetch, and must not become the key `\"\"`"
    );

    // And an answer filed against it is dropped rather than cached under a colliding key.
    store.owner_fetched(
        &ObjectId {
            kind: ObjectKind::ReplicaSet,
            namespace: Some("default".to_string()),
            name: real.name,
            uid: Some(String::new()),
        },
        Ok(replica_set("owned-replicasets")),
    );
    assert!(
        store
            .snapshot(now())
            .expect("listed")
            .workloads
            .iter()
            .all(|w| w.id.kind != ObjectKind::ReplicaSet),
        "an answer with no key to file under must not reach the snapshot"
    );
}

/// **A pod whose controller is not a ReplicaSet is already at the top of its chain**, and nothing
/// is fetched for it — a DaemonSet, a StatefulSet, a pod nobody controls, and a static pod whose
/// `Node` owner `rules.rs` discards (NOTES § D39) are all their own answer.
#[test]
fn only_a_replicaset_owner_is_ever_fetched() {
    let store = bootstrapped();
    let snapshot = store.snapshot(now()).expect("every watch listed");
    let kinds: Vec<&ObjectKind> = snapshot.pods.iter().map(|pod| &pod.owner.kind).collect();
    for shape in [
        ObjectKind::DaemonSet,
        ObjectKind::Pod,
        ObjectKind::ReplicaSet,
    ] {
        assert!(
            kinds.contains(&&shape),
            "the kube-system capture must hold a {shape:?} owner and it holds {kinds:?}"
        );
    }
    let asked = store.unresolved_owners();
    assert_eq!(
        asked.len(),
        1,
        "one ReplicaSet is named in that capture and only it may be fetched, not {asked:?}"
    );
    assert_eq!(asked[0].id.kind, ObjectKind::ReplicaSet);
    assert_eq!(asked[0].id.name, "coredns-589f44dc88");
}

/// **The card, not the snapshot** — the resolution has to reach [`crate::rules::Finding::owner`],
/// which is what `views.rs` will group by (NOTES § D3). Every other test here stops at the
/// snapshot, and a rewrite that never reached a finding would pass all of them.
///
/// It prints both headings, so `cargo test -- --nocapture` is this box's own run.
#[test]
fn the_finding_a_pod_draws_files_under_the_deployment_once_the_owner_resolves() {
    // The node captures come with the other four watches and draw their own cards; this test is
    // about the pod's.
    let about_the_pod = |cards: Vec<crate::rules::Finding>| -> Vec<crate::rules::Finding> {
        cards
            .into_iter()
            .filter(|f| f.object.name == "broken-owned-7bdb7645c8-bwdfd")
            .collect()
    };
    let mut store = store_with_pods("owned-pods");
    let before = about_the_pod(crate::rules::analyze(
        &store.snapshot(now()).expect("every watch listed"),
    ));
    assert!(
        !before.is_empty(),
        "the captured pod draws no card, so this test would prove nothing about headings"
    );
    for finding in &before {
        println!(
            "unresolved: {:?} {}/{}",
            finding.owner.kind,
            finding.owner.namespace.as_deref().unwrap_or("-"),
            finding.owner.name
        );
        assert_eq!(finding.owner.kind, ObjectKind::ReplicaSet);
    }

    let want = store.unresolved_owners();
    store.owner_fetched(&want[0].id, Ok(replica_set("owned-replicasets")));
    let after = about_the_pod(crate::rules::analyze(
        &store.snapshot(now()).expect("every watch listed"),
    ));
    assert_eq!(
        after.len(),
        before.len(),
        "resolving an owner may change what a card is filed under and nothing else"
    );
    for finding in &after {
        println!(
            "resolved:   {:?} {}/{}",
            finding.owner.kind,
            finding.owner.namespace.as_deref().unwrap_or("-"),
            finding.owner.name
        );
        assert_eq!(
            (
                &finding.owner.kind,
                finding.owner.name.as_str(),
                finding.owner.uid.as_deref()
            ),
            (
                &ObjectKind::Deployment,
                "broken-owned",
                Some("65cd2217-b556-49ca-b69a-db40239997c1")
            )
        );
    }
    for cards in [&before, &after] {
        assert!(
            cards
                .iter()
                .all(|f| f.owner.group_key() == cards[0].owner.group_key()),
            "every card of one pod files under one group key, before the resolution and after \
             it — two group keys is D3's two-cards bug"
        );
    }
}

// --- EVERY KIND THE CLUSTER SERVES ---
//
// **Not one input below is a kind any cluster actually serves, and that is the assertion.**
// `k8s.rs` § EVERY KIND THE CLUSTER SERVES has no list of kinds in it to fail against, so these
// feed invented CRDs — `widgets`, `sprockets` — and every claim about built-ins made in that
// region is a claim about `kube::discovery`, proven by reading its source and cited there rather
// than restated here. If [`browsable`] could tell a built-in from a CRD, these tests would be
// unable to see it; invariant 12 is that it cannot.
//
// **What is synthesised is the discovery *answer*, not the objects in it.** There is no cluster
// in this turn, so the `(ApiResource, ApiCapabilities)` pairs a `Discovery` run would hand over
// are built here — from kube's own constructor, so the derived `api_version` is whatever kube
// would have derived. That is the half a live API server replaces: these prove what k8rs does
// with an answer, never that a server gives that answer.

/// One resource as discovery would describe it. `verbs` is the resource's own list — never the
/// reader's permissions, which is the distinction `k8s.rs` § EVERY KIND THE CLUSTER SERVES is
/// about.
fn served(
    group: &str,
    version: &str,
    kind: &str,
    plural: &str,
    scope: Scope,
    verbs: &[&str],
) -> (ApiResource, ApiCapabilities) {
    let gvk = kube::core::gvk::GroupVersionKind::gvk(group, version, kind);
    (
        ApiResource::from_gvk_with_plural(&gvk, plural),
        ApiCapabilities {
            scope,
            subresources: vec![],
            operations: verbs.iter().map(|verb| (*verb).to_string()).collect(),
        },
    )
}

/// A listable, namespaced CRD — the ordinary entry, for a test that is about something else.
fn listable(group: &str, kind: &str, plural: &str) -> (ApiResource, ApiCapabilities) {
    served(
        group,
        "v1",
        kind,
        plural,
        Scope::Namespaced,
        &["get", "list", "watch"],
    )
}

/// **A kind the browser cannot open is not offered, and the scope decides the namespace label.**
///
/// The filter is one verb: `list` is the whole of what the Resources view does, so a resource
/// that supports `create` and nothing else — the shape every access-review endpoint has — would
/// be a row that answers `405` to the only key that opens it. The two flags are asserted
/// together because they come off the same [`ApiCapabilities`] and one is easy to read as the
/// other.
#[test]
fn a_kind_that_cannot_be_listed_is_never_offered_and_the_scope_decides_the_namespace_label() {
    let answer = browsable(vec![
        listable("example.com", "Widget", "widgets"),
        served(
            "example.com",
            "v1",
            "Sprocket",
            "sprockets",
            Scope::Cluster,
            &["get", "list"],
        ),
        // The access-review shape: performed, never listed.
        served(
            "example.com",
            "v1",
            "Review",
            "reviews",
            Scope::Cluster,
            &["create"],
        ),
        // Readable one at a time and not enumerable — the same refusal for a different reason.
        served(
            "example.com",
            "v1",
            "Ledger",
            "ledgers",
            Scope::Namespaced,
            &["get", "watch"],
        ),
    ]);
    for kind in &answer {
        println!(
            "offered: {}/{} {} ({})",
            kind.group,
            kind.version,
            kind.plural,
            if kind.namespaced {
                "namespaced"
            } else {
                "cluster-wide"
            }
        );
    }
    assert_eq!(
        answer
            .iter()
            .map(|kind| (kind.plural.as_str(), kind.namespaced))
            .collect::<Vec<_>>(),
        vec![("sprockets", false), ("widgets", true)],
        "the sidebar offers exactly the kinds that can be listed, each with the scope discovery \
         gave it"
    );
    assert!(
        browsable(vec![]).is_empty(),
        "nothing served has to come back as nothing offered — the shape a server too old for \
         the aggregated call produces, which returns Ok with no groups in it rather than an error"
    );
}

/// **The order is k8rs's, because kube hands back `HashMap` iteration order.**
///
/// `Discovery::groups()` and `ApiGroup::resources_by_stability()` both end in a hash map
/// (`kube-client-4.2.0/src/discovery/mod.rs:206-208`, `apigroup.rs:320-326`), so a sidebar built
/// straight off either is in a different order on every launch. Sorting by the plural first is
/// what puts a plural two groups both serve next to itself instead of at opposite ends of the
/// list — `events` is the real instance, and these are two invented ones for the same reason the
/// section head gives.
#[test]
fn the_order_is_ours_and_one_plural_two_groups_serve_lands_together() {
    let answer = browsable(vec![
        listable("z.example.com", "Widget", "widgets"),
        listable("example.com", "Sprocket", "sprockets"),
        served(
            "a.example.com",
            "v2",
            "Widget",
            "widgets",
            Scope::Namespaced,
            &["list"],
        ),
        served(
            "a.example.com",
            "v1",
            "Widget",
            "widgets",
            Scope::Namespaced,
            &["list"],
        ),
    ]);
    let order: Vec<String> = answer
        .iter()
        .map(|kind| format!("{}/{}/{}", kind.plural, kind.group, kind.version))
        .collect();
    for row in &order {
        println!("sidebar: {row}");
    }
    assert_eq!(
        order,
        vec![
            "sprockets/example.com/v1",
            "widgets/a.example.com/v1",
            "widgets/a.example.com/v2",
            "widgets/z.example.com/v1",
        ],
        "plural, then group, then version — and nothing was dropped for sharing a plural, \
         because two groups serving one plural is two resources"
    );
}

/// **A CRD names itself, so its name is untrusted text like any other** (invariant 9).
///
/// `spec.names.plural` is whatever the manifest said, and the sidebar prints it — so an `ESC`
/// in it reaches a terminal unless something strips it. One case per string [`Browsable`]
/// carries, because a guard is proven only for the fields it was fed (NOTES § D29), and the
/// group and version are in the list too: they are as much the CRD author's as the plural is.
#[test]
fn a_crd_that_names_itself_with_control_characters_cannot_rewrite_a_terminal() {
    let evil = "wid\u{1b}[2Jgets";
    for (field, answer) in [
        (
            "group",
            browsable(vec![listable(evil, "Widget", "widgets")]),
        ),
        (
            "kind",
            browsable(vec![listable("example.com", evil, "widgets")]),
        ),
        (
            "plural",
            browsable(vec![listable("example.com", "Widget", evil)]),
        ),
        (
            "version",
            browsable(vec![served(
                "example.com",
                evil,
                "Widget",
                "widgets",
                Scope::Namespaced,
                &["list"],
            )]),
        ),
        (
            "verbs",
            browsable(vec![served(
                "example.com",
                "v1",
                "Widget",
                "widgets",
                Scope::Namespaced,
                &["list", evil],
            )]),
        ),
    ] {
        let kind = answer.first().unwrap_or_else(|| {
            panic!("the {field} case dropped the only entry, so it proves nothing")
        });
        println!("{field}: {kind:?}");
        let strings: Vec<&str> = [
            kind.group.as_str(),
            kind.version.as_str(),
            kind.kind.as_str(),
            kind.plural.as_str(),
        ]
        .into_iter()
        .chain(kind.verbs.iter().map(String::as_str))
        .collect();
        assert!(
            strings.iter().all(|text| !text.contains('\u{1b}')),
            "an escape sequence a CRD put in its own {field} reached a screen: {strings:?}"
        );
        // The control character and nothing else: `text` removes what cannot be printed and
        // leaves what can, so the rest of the escape sequence is still there as plain text.
        let stripped = evil.replace('\u{1b}', "");
        assert!(
            strings.iter().any(|text| *text == stripped),
            "the {field} case did not come back as {stripped:?} — the strip took more, or less, \
             than the control character: {strings:?}"
        );
    }
}

/// **And a name longer than the bound is cut where every other field is cut**, with the same
/// visible marker (NOTES § D146). A CRD's plural has a length limit; a *verb* an aggregated API
/// server reports has none that k8rs can see.
#[test]
fn a_discovery_field_over_the_bound_is_shortened_and_says_so() {
    let long = "w".repeat(IDENTIFIER * 2);
    let answer = browsable(vec![served(
        "example.com",
        "v1",
        "Widget",
        &long,
        Scope::Namespaced,
        &["list", &long],
    )]);
    let kind = answer.first().expect("one listable kind was served");
    println!(
        "plural: {} bytes, verb: {} bytes",
        kind.plural.len(),
        kind.verbs[1].len()
    );
    for (field, value) in [("plural", &kind.plural), ("verb", &kind.verbs[1])] {
        assert!(
            value.ends_with(SHORTENED),
            "a {field} of {} bytes was kept whole, or cut without saying so",
            long.len()
        );
        assert!(
            value.len() <= IDENTIFIER + SHORTENED.len(),
            "a {field} of {} bytes came back as {} — the bound did not hold",
            long.len(),
            value.len()
        );
    }
}

/// **Every `String` [`Browsable`] carries is named by its `Bounded` impl**, derived from
/// `k8s.rs` rather than typed out here — the same guard the snapshot types get above, over the
/// one type in this file that holds text the cluster's own users wrote. A field added to it and
/// forgotten in the ingest guard fails this test.
#[test]
fn every_string_the_sidebar_keeps_is_named_by_the_ingest_guard() {
    let types = declared_types(K8S_SOURCE);
    let fields = types
        .get("Browsable")
        .expect("k8s.rs no longer declares Browsable, or declares it differently");
    let carries_text: Vec<&str> = fields
        .iter()
        .filter(|(_, kind)| words(kind).any(|word| word == "String"))
        .map(|(field, _)| *field)
        .collect();
    assert!(
        carries_text.len() >= 4,
        "only {carries_text:?} were parsed out of Browsable, so this guard is reading nothing"
    );
    let body = bounded_impl("Browsable")
        .expect("Browsable carries text and the ingest guard region has no impl Bounded for it");
    for field in carries_text {
        println!("bounded: Browsable.{field}");
        assert!(
            words(body).any(|word| word == field),
            "Browsable.{field} is a String the sidebar keeps and the ingest guard never names it"
        );
    }
}

/// **A server that does not serve aggregated discovery answers `Ok` with an empty cluster in
/// it** — the first failure `k8s.rs` § EVERY KIND THE CLUSTER SERVES lists, and the one kube's
/// own doc denies: *"If the server does not support Aggregated Discovery, this will return an
/// error"* (`kube-client-4.2.0/src/discovery/mod.rs:168-170`).
///
/// The gate is beta and on from 1.27 and alpha-off at 1.26 (`AggregatedDiscoveryEndpoint.md`,
/// `kubernetes/website`, read 2026-08-22), and § HOW OLD A CLUSTER MAY BE lets k8rs run below
/// its own 1.29 floor — so this is a server k8rs meets rather than a hypothetical one.
///
/// **What is proven here is the decode and not the negotiation.** The legacy body is built from
/// `k8s-openapi`'s own `APIGroupList` and serialised by its own impl, so no JSON is hand-written;
/// that a 1.26 API server answers kube's Accept header with that body is HTTP content
/// negotiation against a real server, and nothing in this repo has one.
#[test]
fn a_server_too_old_for_aggregated_discovery_decodes_to_no_groups_and_no_error() {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{
        APIGroup, APIGroupList, GroupVersionForDiscovery,
    };
    use kube::core::discovery::v2::APIGroupDiscoveryList;

    let legacy = APIGroupList {
        groups: vec![APIGroup {
            name: "example.com".to_string(),
            versions: vec![GroupVersionForDiscovery {
                group_version: "example.com/v1".to_string(),
                version: "v1".to_string(),
            }],
            preferred_version: Some(GroupVersionForDiscovery {
                group_version: "example.com/v1".to_string(),
                version: "v1".to_string(),
            }),
            server_address_by_client_cidrs: None,
        }],
    };
    let body = serde_json::to_string(&legacy).expect("k8s-openapi serialises its own type");
    println!("what the old server sends: {body}");

    // `Client::request` is exactly this line over the response text
    // (`kube-client-4.2.0/src/client/mod.rs:281-291`).
    let aggregated: APIGroupDiscoveryList =
        serde_json::from_str(&body).expect("the aggregated type rejected a legacy discovery body");
    println!("what run_aggregated() would build from it: {aggregated:?}");
    assert!(
        aggregated.items.is_empty(),
        "a legacy discovery body no longer decodes to an empty aggregated one, so the silent \
         empty sidebar this test pins has changed shape"
    );
    assert!(
        !legacy.groups.is_empty(),
        "the body fed in named no groups, so an empty answer would have proven nothing"
    );
}

// --- WHAT ELSE THE CLUSTER SERVES ---
//
// **These name real groups where the region above names none, and that is the difference between
// the two functions rather than a lapse.** [`browsable`] must not be able to tell a built-in from
// a CRD (invariant 12), so its inputs are invented; [`capabilities`] answers *is cert-manager
// installed*, and a probe that could not name `cert-manager.io` would answer nothing. The invented
// CRDs are still here — they are the **negative**: `widgets` and `sprockets` are what a cluster
// with none of these on it serves, and every row has to stay absent for them.
//
// Every input is built with `served()` above, so what is synthesised is the discovery answer and
// not a cluster, exactly as it is for the sidebar.

/// One resource in the core group — what *every* working API server serves, so a fixture built
/// around it has the shape a real discovery answer has.
///
/// **[`capabilities`] never looks for it.** The check is `served.is_empty()`, and any non-empty
/// answer is a real one; an answer with no core group in it at all still probes normally. It is
/// here because the inputs should look like the wire, and because it is what makes the
/// *nothing installed* case a non-empty answer rather than the *nothing discovered* one.
fn core_group() -> (ApiResource, ApiCapabilities) {
    served(
        "",
        "v1",
        "Pod",
        "pods",
        Scope::Namespaced,
        &["get", "list", "watch"],
    )
}

/// Each capability, the group that turns it on, and one kind that group really serves.
///
/// `Linkerd` appears twice because it ships two groups and either one means it is installed.
const ROWS: &[(Capability, &str, &str, &str)] = &[
    (
        Capability::Metrics,
        "metrics.k8s.io",
        "NodeMetrics",
        "nodes",
    ),
    (
        Capability::DisruptionBudgets,
        "policy",
        "PodDisruptionBudget",
        "poddisruptionbudgets",
    ),
    (
        Capability::CertManager,
        "cert-manager.io",
        "Certificate",
        "certificates",
    ),
    (
        Capability::Prometheus,
        "monitoring.coreos.com",
        "Prometheus",
        "prometheuses",
    ),
    (
        Capability::Istio,
        "networking.istio.io",
        "VirtualService",
        "virtualservices",
    ),
    (
        Capability::Linkerd,
        "linkerd.io",
        "ServiceProfile",
        "serviceprofiles",
    ),
    (
        Capability::Linkerd,
        "policy.linkerd.io",
        "Server",
        "servers",
    ),
    (
        Capability::Cilium,
        "cilium.io",
        "CiliumNetworkPolicy",
        "ciliumnetworkpolicies",
    ),
];

/// **Every [`Capability`] there is, walked out of an exhaustive `match`.**
///
/// [`ROWS`] is hand-written, and nothing hand-written is complete by construction: `cargo mutants`
/// mutates function bodies and not `const` data, and `test-guard` counts test attributes, so a row
/// deleted from that table takes its only coverage with it and every gate stays green. `after`'s
/// `match` is what makes the compiler the reader instead — a variant added to [`Capability`] has
/// no arm, the tests stop building until it is given one, and the arm it is given is what walks it
/// into the check below (CLAUDE.md § *A derived list asserts it found something*, one level up:
/// the list here is of variants, not of findings).
///
/// **What it does not close**, said plainly: an arm that answers `None` where the chain should
/// have continued leaves its own variant unwalked, and nothing here can see that. The compile
/// error is what makes writing one a deliberate act rather than an oversight, and it is as far as
/// a `match` reaches without a derive macro — a crate invariant 10 refuses. Keep the chain a
/// chain: exactly one arm ends it.
///
/// (Spelling the test attribute out in this comment would have been *counted* as one, which is a
/// red `just check` and not a typo — the guard said so before this sentence existed.)
fn every_capability() -> Vec<Capability> {
    fn after(capability: Capability) -> Option<Capability> {
        match capability {
            Capability::Metrics => Some(Capability::DisruptionBudgets),
            Capability::DisruptionBudgets => Some(Capability::CertManager),
            Capability::CertManager => Some(Capability::Prometheus),
            Capability::Prometheus => Some(Capability::Istio),
            Capability::Istio => Some(Capability::Linkerd),
            Capability::Linkerd => Some(Capability::Cilium),
            Capability::Cilium => None,
        }
    }
    std::iter::successors(Some(Capability::Metrics), |capability| after(*capability)).collect()
}

/// **No [`Capability`] is missing a row above**, and therefore none is missing the test that walks
/// them.
///
/// The defect this closes was measured, not imagined: deleting the `Cilium` row left the whole
/// suite green, because with it gone only the negative — `cilium.io` absent from a bare cluster —
/// still ran, and nothing at all asserted that `cilium.io` turns anything *on*.
#[test]
fn every_capability_has_a_row_in_the_table() {
    let covered: BTreeSet<Capability> = ROWS.iter().map(|(capability, ..)| *capability).collect();
    let missing: Vec<Capability> = every_capability()
        .into_iter()
        .filter(|capability| !covered.contains(capability))
        .collect();
    println!("ROWS covers {covered:?}");
    assert!(
        missing.is_empty(),
        "these capabilities have no row above, so nothing asserts what turns them on: {missing:?}"
    );
}

/// **Every row of NOTES § Capability probe turns on when its group is served, and only its own.**
///
/// The assertion is the whole set and not `contains`, so a row that also switched a neighbour on —
/// `policy.linkerd.io` reading as `policy`, `cert-manager.io` as a substring of somebody's CRD
/// group — fails here rather than somewhere a screen would show it. **That is also where most of
/// the negatives are**: each pass asserts the other six capabilities absent, and the test below
/// adds the case where all of them are.
#[test]
fn each_capability_turns_on_for_its_own_group_and_switches_on_nothing_else() {
    for (capability, group, kind, plural) in ROWS {
        let answer = capabilities(&[
            core_group(),
            listable("example.com", "Widget", "widgets"),
            served(
                group,
                "v1",
                kind,
                plural,
                Scope::Namespaced,
                &["get", "list", "watch"],
            ),
        ]);
        println!("{group} serves {kind} -> {answer:?}");
        assert_eq!(
            answer,
            Some(BTreeSet::from([*capability])),
            "a cluster serving {group}/{kind} answered {answer:?}"
        );
    }
}

/// **A group that serves several kinds is one capability, which is the ordinary shape and not an
/// edge case.**
///
/// `metrics.k8s.io` serves exactly two — `nodes` and `pods` — so every cluster with
/// metrics-server on it hands this function the row twice. A set is what absorbs that; a list
/// would have put metrics-server in the answer twice and left every consumer to notice.
#[test]
fn a_group_that_serves_more_than_one_kind_is_still_one_capability() {
    let answer = capabilities(&[
        core_group(),
        served(
            "metrics.k8s.io",
            "v1beta1",
            "NodeMetrics",
            "nodes",
            Scope::Cluster,
            &["get", "list"],
        ),
        served(
            "metrics.k8s.io",
            "v1beta1",
            "PodMetrics",
            "pods",
            Scope::Namespaced,
            &["get", "list"],
        ),
    ]);
    println!("both metrics kinds -> {answer:?}");
    assert_eq!(
        answer,
        Some(BTreeSet::from([Capability::Metrics])),
        "metrics-server's two kinds have to be one capability"
    );
}

/// **A cluster with none of them on it says so, and a group that merely spells like one is not a
/// hit.**
///
/// The absent case is the one the box is about: rule 1 has the feature print *not installed in
/// this cluster*, which it may only do because it was told. The near misses are here because a
/// guard is proven only for the shapes it was fed (NOTES § D29) and only for the framing it was
/// written for (§ D31), and they are of two framings — a capability's name as a **suffix** of a
/// longer group, and as a **prefix** of one.
///
/// **The suffix framing is fed twice, and the separator is the whole reason.** `not-metrics.k8s.io`
/// and `my-cilium.io` join with a hyphen; `acme.cert-manager.io`, `custom.metrics.k8s.io` and
/// `external.metrics.k8s.io` join with a dot — and those three are **groups that really ship**,
/// not invented. `acme.cert-manager.io` is in cert-manager's own CRD manifest and came back in a
/// live discovery answer beside `cert-manager.io`
/// (`reports/2026-08-26-capability-probe-group-strings.md` §§ 1-2); the other two are what
/// Prometheus Adapter and KEDA register, read off those projects and never on a cluster here.
/// Widening either arm to a dot-joined suffix — `ends_with(".metrics.k8s.io")`,
/// `ends_with(".cert-manager.io")` — is caught only by the dotted three and **passes** the
/// hyphenated two, measured both ways: without those three the widened arms answer `Some({})` and
/// this test is green, with them it answers `Some({Metrics, CertManager})` and fails. That is why
/// an invented near miss is not a substitute for one that ships. What the widening would cost:
/// [`Capability::Metrics`] on a cluster with an adapter and no metrics-server, and the Capacity
/// report promising real usage numbers nothing can serve.
///
/// **Each sibling is fed alone, on purpose.** A real cert-manager install serves
/// `acme.cert-manager.io` *and* `cert-manager.io`; feeding both would turn the row on for the
/// right reason and hide a wrong one, so the assertion could not fail. Isolating the sibling is
/// what makes it a guard rather than a screenshot. (`custom.metrics.k8s.io` without
/// `metrics.k8s.io` needs no isolating — an adapter runs on clusters with no metrics-server.)
///
/// **The prefix framing has no real sibling to use.** `metrics.k8s.io.example.com` and
/// `cert-manager.io.example.com` put a capability's whole name in front of a longer group; nothing
/// ships a group of that shape, so an invented string is the only way to feed the framing at all.
///
/// `policy` carrying a kind that is not `PodDisruptionBudget` is the third thing that must not be a
/// hit, and it is the next test rather than a fixture here — it needs its own failure message.
#[test]
fn nothing_installed_answers_none_of_them_and_a_near_miss_is_not_a_hit() {
    let answer = capabilities(&[
        core_group(),
        listable("example.com", "Widget", "widgets"),
        listable("sprockets.example.com", "Sprocket", "sprockets"),
        // Real siblings that ship beside a capability's group and are not it.
        listable("acme.cert-manager.io", "Order", "orders"),
        listable("custom.metrics.k8s.io", "MetricValue", "metricvalues"),
        listable(
            "external.metrics.k8s.io",
            "ExternalMetricValue",
            "externalmetricvalues",
        ),
        // Groups that carry a capability's name in a longer string and are not it.
        listable("not-metrics.k8s.io", "Widget", "widgets"),
        listable("metrics.k8s.io.example.com", "Widget", "widgets"),
        listable("cert-manager.io.example.com", "Widget", "widgets"),
        listable("my-cilium.io", "Widget", "widgets"),
    ]);
    println!("a cluster with none of them: {answer:?}");
    assert_eq!(
        answer,
        Some(BTreeSet::new()),
        "a cluster with none of these installed has to answer that it was asked and has none"
    );
}

/// **`policy` carrying a kind that is not `PodDisruptionBudget` is a historical shape, kept
/// deliberately.**
///
/// It is the only input that exercises the kind half of the `("policy", "PodDisruptionBudget")`
/// arm, and **no supported server can produce it**: `PodSecurityPolicy` left the group at 1.25,
/// D149's floor is 1.29, and `policy/v1` serves exactly one resource at 1.36 — measured, a
/// `--runtime-config` disabling either the version or that one resource takes the whole group off
/// `/apis` with it (`reports/2026-08-26-capability-probe-group-strings.md` § 3). So the narrowing
/// is unreachable-but-harmless on every cluster k8rs supports, `k8s.rs` says so where it is
/// written, and this test is what keeps the arm from being widened by someone who reads that and
/// concludes the kind may as well go. Pointing it at a reachable state is not available: there is
/// no second `policy` kind left to point at.
#[test]
fn policy_without_the_disruption_budget_kind_is_not_drain_safety() {
    let answer = capabilities(&[
        core_group(),
        served(
            "policy",
            "v1beta1",
            "PodSecurityPolicy",
            "podsecuritypolicies",
            Scope::Cluster,
            &["get", "list", "watch"],
        ),
    ]);
    println!("policy serving only PodSecurityPolicy -> {answer:?}");
    assert_eq!(
        answer,
        Some(BTreeSet::new()),
        "the group alone is not the capability — drain safety needs PodDisruptionBudget"
    );
}

/// **A discovery answer that named nothing is `None`, and that is not *none of them installed*.**
///
/// The first of the four failures `k8s.rs` § EVERY KIND THE CLUSTER SERVES lists: a server too old
/// for the aggregated call answers `Ok` with zero groups. Read as *absent*, every feature then
/// says *not installed in this cluster* about a cluster that has them all — one plain false
/// sentence per screen, which is invariant 14 broken in the confident direction.
///
/// The two answers are asserted **against each other**, because a `None` that happened to equal
/// the empty-set answer would prove nothing.
///
/// **The second input is not a bare cluster and must not be read as one.** A cluster exactly as
/// `kind create cluster` left it answers 51 resources including `policy v1`, so its real answer is
/// `Some({DisruptionBudgets})` (`reports/2026-08-26-capability-probe-group-strings.md` § 2). What
/// is fed here is the narrower thing this test needs: an answer that *named resources* and named
/// none of these, which is the only way to hold `None` up against a real `Some`.
#[test]
fn a_discovery_answer_that_named_nothing_is_not_a_cluster_with_nothing_installed() {
    let nothing_discovered = capabilities(&[]);
    let named_none_of_them = capabilities(&[core_group()]);
    println!("zero groups came back: {nothing_discovered:?}");
    println!("an answer naming none of them came back: {named_none_of_them:?}");
    assert_eq!(
        nothing_discovered, None,
        "a discovery answer with nothing in it was read as a fact about the cluster"
    );
    assert_eq!(
        named_none_of_them,
        Some(BTreeSet::new()),
        "a real answer with none of these in it is the empty set, not the missing answer"
    );
    assert_ne!(
        nothing_discovered, named_none_of_them,
        "the two cannot share a spelling — one is a sentence a screen prints, the other is not a \
         fact anybody has"
    );
}

/// **The probe reads the bytes the server sent, because [`ingest`] rewrites them.**
///
/// [`text`] *removes* an unprintable character rather than replacing it — it only inserts a space
/// between two characters it kept — so **six** spellings of `metrics.k8s.io`, not one, come out of
/// the sidebar's guard as `metrics.k8s.io` exactly: a zero-width space, a bidi override, a soft
/// hyphen, a leading BOM, an embedded NUL, and a plain trailing newline. A probe reading
/// [`Browsable`] would report metrics-server present on a cluster whose only such group is any one
/// of them — the strip is doing what invariant 9 asks and the comparison is the wrong place to
/// stand behind it.
///
/// Both halves are asserted for every spelling: the probe refuses it, **and** the sidebar really
/// does produce the word that would have been believed. Without the second, this test would pass
/// on a strip that never touched the character.
#[test]
fn a_lookalike_group_is_refused_because_the_probe_never_sees_the_stripped_spelling() {
    for (what, lookalike) in [
        ("zero width space", "metrics.k8s\u{200b}.io"),
        ("bidi override", "metrics.k8s\u{202e}.io"),
        ("soft hyphen", "metrics.k8s\u{ad}.io"),
        ("leading BOM", "\u{feff}metrics.k8s.io"),
        ("embedded NUL", "metrics.k8s\0.io"),
        ("trailing newline", "metrics.k8s.io\n"),
    ] {
        let served_by_the_cluster = vec![
            core_group(),
            served(
                lookalike,
                "v1",
                "NodeMetrics",
                "nodes",
                Scope::Cluster,
                &["get", "list"],
            ),
        ];
        let answer = capabilities(&served_by_the_cluster);
        println!("{what}: {lookalike:?} probed as {answer:?}");
        assert_eq!(
            answer,
            Some(BTreeSet::new()),
            "a {what} group that is not metrics.k8s.io turned metrics-server on"
        );

        let sidebar = browsable(served_by_the_cluster);
        let groups: Vec<&str> = sidebar.iter().map(|kind| kind.group.as_str()).collect();
        println!("{what}: the same input through the sidebar's guard: {groups:?}");
        assert!(
            groups.contains(&"metrics.k8s.io"),
            "the ingest guard no longer produces the spelling this {what} case exists to refuse, \
             so it proves nothing: {groups:?}"
        );
    }
}

/// **A capability is what the cluster serves, not what anybody may do to it.**
///
/// The verbs belong to the resource and not to the reader (§ EVERY KIND THE CLUSTER SERVES), and
/// [`browsable`] drops what cannot be listed **at all** because a sidebar row has one verb. A
/// capability has no verb: an aggregated API server offering one kind nobody can enumerate is
/// still that product, installed. So the probe reads the answer and never the sidebar — asserted
/// by taking both from one input and getting different sizes.
#[test]
fn a_capability_whose_kind_cannot_be_listed_is_still_installed() {
    let unlistable = vec![
        core_group(),
        served(
            "metrics.k8s.io",
            "v1beta1",
            "NodeMetrics",
            "nodes",
            Scope::Cluster,
            &["get"],
        ),
    ];
    let answer = capabilities(&unlistable);
    let sidebar: Vec<String> = browsable(unlistable)
        .iter()
        .map(|kind| format!("{}/{}", kind.group, kind.plural))
        .collect();
    println!("probe: {answer:?}");
    println!("sidebar: {sidebar:?}");
    assert_eq!(
        answer,
        Some(BTreeSet::from([Capability::Metrics])),
        "metrics-server is installed on this cluster and the probe missed it"
    );
    assert!(
        !sidebar.iter().any(|row| row.starts_with("metrics.k8s.io/")),
        "the sidebar kept the unlistable kind, so this input no longer shows the difference \
         between the two readings: {sidebar:?}"
    );
}
// --- THE BROWSER'S ROWS ---
//
// **Two committed captures and no hand-written JSON** (NOTES § D53): `table-pods.json` is the
// default `includeObject`, nine columns, an object under every row; `table-deployments.json` is
// `?includeObject=None`, eight columns, `"object": null`, and **number cells** — the shape a
// `Vec<String>` would have refused. Both came off the kind cluster on 2026-08-22 through
// `kubectl proxy` with the Accept header the product code sends.
//
// **What is built in Rust here is the response, never the objects**: the ragged row and the
// crafted cell are shapes no healthy server produces, so no capture has them and none is edited
// to (`k8s.rs` § THE BROWSER'S ROWS). And the `406` is a `Status` this file constructs — the kind
// cluster runs no aggregated API server, so **the fallback is proven against a synthesised refusal
// and never against one a server sent**.

/// One committed `Table` capture, through the real ingest path.
fn table(name: &str) -> Table {
    let response: TableResponse = serde_json::from_value(capture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Table: {e}"));
    ingest(response)
}

/// The column headers of a decoded table, in the server's order.
fn headers(table: &Table) -> Vec<&str> {
    table.columns.iter().map(|c| c.name.as_str()).collect()
}

/// **What a screen would have to work with**, printed so a reader of the test output can see it:
/// the `priority: 0` columns `screens/resources.md` draws, and nothing else. The widths are this
/// function's own — `views.rs` owns the real ones — so this is evidence, not a layout.
///
/// **And it measures width with `str::len`, which is bytes**, so a CJK or emoji cell would
/// misalign every column to its right in the output below. That is fine for evidence a person
/// reads once and it is **not** a thing `views.rs` may inherit: a real column budget is display
/// width, not bytes and not `chars().count()` either.
fn drawn(table: &Table) -> String {
    let narrow: Vec<usize> = (0..table.columns.len())
        .filter(|&index| table.columns[index].priority == 0)
        .collect();
    let width = |index: usize| {
        table
            .rows
            .iter()
            .map(|row| row.cells[index].len())
            .chain(std::iter::once(table.columns[index].name.len()))
            .max()
            .unwrap_or(0)
    };
    let line = |cells: &dyn Fn(usize) -> String| {
        narrow
            .iter()
            .map(|&index| format!("{:<pad$}", cells(index), pad = width(index)))
            .collect::<Vec<_>>()
            .join("  ")
    };
    let mut out = line(&|index| table.columns[index].name.to_uppercase());
    for row in &table.rows {
        out.push('\n');
        out.push_str(&line(&|index| row.cells[index].clone()));
    }
    out
}

/// One kind, as discovery would have described it and [`browsable`] would have kept it.
fn browsed(group: &str, version: &str, kind: &str, plural: &str, scope: Scope) -> Browsable {
    browsable(vec![served(group, version, kind, plural, scope, &["list"])])
        .pop()
        .expect("a listable kind survives browsable()")
}

/// A `Table` response built here, for a shape no healthy server sends.
fn response(columns: &[&str], rows: Vec<Vec<Value>>) -> TableResponse {
    TableResponse {
        kind: TABLE_KIND.to_string(),
        items: vec![],
        column_definitions: columns
            .iter()
            .map(|name| ColumnResponse {
                name: (*name).to_string(),
                priority: 0,
            })
            .collect(),
        rows: rows
            .into_iter()
            .map(|cells| RowResponse {
                cells,
                object: None,
            })
            .collect(),
    }
}

/// A refusal with one HTTP code on it, as `Client::request` would hand it over
/// (`kube-client-4.2.0/src/client/mod.rs:551-558`).
fn refused(code: u16) -> kube::Error {
    kube::Error::Api(
        kube::core::response::Status {
            code,
            ..Default::default()
        }
        .boxed(),
    )
}

/// **The columns are the server's, and two kinds do not share one list.**
///
/// **What this proves is that the columns are per-kind, which is weaker than invariant 12 and the
/// first draft of this comment claimed the stronger thing.** A hand-written map from kind to
/// columns would produce two different header lists just as well, so `assert_ne!` cannot see the
/// difference between reading them off the wire and looking them up. What *would* is a `Table` of
/// a kind no built-in printer knows — a CRD, whose columns can only have come from the server —
/// and **no CRD `Table` is captured in this repo**, so that half is unproven here. The mechanical
/// half of invariant 12 is covered elsewhere: `k8s.rs` § THE BROWSER'S ROWS contains no kind at
/// all, and the tests around [`Fetch::table`] feed invented CRDs to everything that does.
///
/// **The `priority` split is the second half**: `screens/resources.md` draws the `priority: 0` set,
/// and without it every screen is `kubectl -o wide`. Nine columns of which five are narrow is the
/// measurement, not a guess.
#[test]
fn the_columns_come_from_the_server_and_two_kinds_never_share_one_list() {
    let pods = table("table-pods");
    let deployments = table("table-deployments");
    println!("pods: {:?}", headers(&pods));
    println!("deployments: {:?}", headers(&deployments));
    println!("\nwhat a screen gets, pods:\n{}", drawn(&pods));
    println!(
        "\nwhat a screen gets, deployments:\n{}",
        drawn(&deployments)
    );

    assert_eq!(
        headers(&pods),
        [
            "Name",
            "Ready",
            "Status",
            "Restarts",
            "Age",
            "IP",
            "Node",
            "Nominated Node",
            "Readiness Gates"
        ],
        "the pods table's columns are not the ones the API server sent"
    );
    assert_eq!(
        headers(&deployments),
        [
            "Name",
            "Ready",
            "Up-to-date",
            "Available",
            "Age",
            "Containers",
            "Images",
            "Selector"
        ],
        "the deployments table's columns are not the ones the API server sent"
    );
    assert_ne!(
        headers(&pods),
        headers(&deployments),
        "two kinds came back with identical columns, which is what a hard-coded list looks like"
    );

    for (kind, decoded, wide) in [("pods", &pods, 4), ("deployments", &deployments, 3)] {
        let narrow = decoded
            .columns
            .iter()
            .filter(|column| column.priority == 0)
            .count();
        println!(
            "{kind}: {} columns, {narrow} at priority 0",
            decoded.columns.len()
        );
        assert_eq!(
            narrow, 5,
            "{kind} no longer has five narrow columns, so the wide/narrow split has moved"
        );
        assert_eq!(
            decoded.columns.len() - narrow,
            wide,
            "{kind} lost the columns `kubectl -o wide` adds, so priority is not being kept"
        );
    }
}

/// **A cell is not a string, and this is the defect the deployments capture exists to prevent.**
///
/// `[.rows[].cells[]] | map(type) | unique` over that table is `["number", "string"]`, so
/// `cells: Vec<String>` would have failed to deserialise every Deployment table a real cluster
/// serves. The fixture is asserted to still carry a number first — otherwise a capture that lost
/// them would leave this test green and testing nothing.
#[test]
fn a_number_cell_decodes_and_comes_out_as_the_text_a_column_prints() {
    let raw = capture("table-deployments");
    let shapes: BTreeSet<&str> = raw["rows"]
        .as_array()
        .expect("the capture has rows")
        .iter()
        .flat_map(|row| row["cells"].as_array().expect("a row has cells"))
        .map(|cell| match cell {
            serde_json::Value::Number(_) => "number",
            serde_json::Value::String(_) => "string",
            _ => "other",
        })
        .collect();
    println!("cell types in table-deployments.json: {shapes:?}");
    assert!(
        shapes.contains("number"),
        "table-deployments.json no longer carries a number cell, so this test proves nothing"
    );

    let decoded = table("table-deployments");
    assert_eq!(
        decoded.rows[0].cells,
        [
            "broken-owned",
            "0/1",
            "1",
            "0",
            "34h",
            "quitter",
            "busybox",
            "app=broken-owned"
        ],
        "a number cell did not come out as the bare text a column prints"
    );

    // The negative: a table whose cells are all strings is carried through untouched.
    let pods = capture("table-pods");
    let sent: Vec<String> = pods["rows"][0]["cells"]
        .as_array()
        .expect("the first pod row has cells")
        .iter()
        .map(|cell| {
            cell.as_str()
                .expect("every pod cell is a string")
                .to_string()
        })
        .collect();
    assert_eq!(
        table("table-pods").rows[0].cells,
        sent,
        "a table of plain string cells was not carried through as it was sent"
    );
}

/// **A row carries the identity the default `includeObject` sends, and carries none when the
/// server was asked for none** — the two shapes `k8s.rs` § THE BROWSER'S ROWS decides between,
/// both decoded by one type.
///
/// The identity is what `screens/resources.md` matches a finding onto a row by and what every
/// dialog names, so the `None` half is the cost of the alternative rather than a mode k8rs uses.
#[test]
fn a_row_carries_its_identity_under_the_default_and_none_under_include_object_none() {
    let pods = table("table-pods");
    assert!(!pods.rows.is_empty(), "the pods capture has no rows");
    for row in &pods.rows {
        println!("{:?}/{:?} uid {:?}", row.namespace, row.name, row.uid);
        assert_eq!(
            row.namespace.as_deref(),
            Some("kube-system"),
            "a row of a kube-system table lost its namespace"
        );
        assert_eq!(
            row.name.as_deref(),
            Some(row.cells[0].as_str()),
            "the row's object and its Name cell disagree about what it is"
        );
        assert!(
            row.uid.as_ref().is_some_and(|uid| uid.len() == 36),
            "a row came through without the uid a finding is matched onto it by"
        );
    }

    let raw = capture("table-deployments");
    assert!(
        raw["rows"][0]["object"].is_null(),
        "table-deployments.json is no longer the ?includeObject=None capture, so the second \
         shape is not being decoded here"
    );
    let deployments = table("table-deployments");
    assert!(
        !deployments.rows.is_empty(),
        "the deployments capture has no rows"
    );
    for row in &deployments.rows {
        assert_eq!(
            (&row.namespace, &row.name, &row.uid),
            (&None, &None, &None),
            "a row with no object under it invented an identity"
        );
    }
}

/// **A cell is free text from the API and goes through the one guard** (invariant 9,
/// NOTES § D146), **at the class that keeps a sentence whole**.
///
/// The three assertions are the three things that could be wrong: a control character survives, a
/// runaway cell is kept whole, or the class is [`IDENTIFIER`] and an Events table's `MESSAGE`
/// column loses its second half. The last is why 513 bytes is asserted *untouched*.
#[test]
fn a_crafted_cell_is_stripped_and_bounded_and_a_sentence_length_one_is_not_cut() {
    let sentence = "S".repeat(IDENTIFIER + 1);
    let runaway = "R".repeat(FREE_TEXT + 1);
    let decoded: Table = ingest(response(
        &["Name", "Message"],
        vec![
            vec![
                Value::String("\u{1b}[2Jweb\u{7}\u{202e}".to_string()),
                Value::String(sentence.clone()),
            ],
            vec![Value::String("api".to_string()), Value::String(runaway)],
        ],
    ));

    println!("name cell: {:?}", decoded.rows[0].cells[0]);
    // `\u{1b}`, `\u{7}` and `\u{202e}` are gone and `[2J` is not: D146 removes what has no
    // printed form and never touches a character that prints as itself. What the crafted cell
    // loses is its power to move the reader's cursor and to reverse the text after it, not its
    // text. **A cell is where this class is worth the most** — every kind the cluster serves
    // reaches a screen through this one path, a CRD's own `additionalPrinterColumns` included.
    assert_eq!(
        decoded.rows[0].cells[0], "[2Jweb",
        "a crafted cell reached the screen with a control character still in it"
    );
    assert!(
        !decoded.rows[0].cells[0].chars().any(unprintable),
        "a character with no printed form survived into a cell a terminal will print"
    );
    assert_eq!(
        decoded.rows[0].cells[1],
        sentence,
        "a {}-byte cell was cut, so a message column is being bounded as an identifier",
        sentence.len()
    );
    assert!(
        decoded.rows[1].cells[1].ends_with(SHORTENED),
        "a cell past {FREE_TEXT} bytes was kept whole, or cut without saying so"
    );
    assert!(
        decoded.rows[1].cells[1].len() <= FREE_TEXT + SHORTENED.len(),
        "the cell bound did not hold"
    );
}

/// **A row shorter than the column list is padded, and a longer one keeps every cell it was
/// sent.** The first is a renderer that would index past the end; the second is the collection
/// bound this box is explicitly not deciding — nothing here cuts a list.
#[test]
fn a_short_row_is_padded_to_the_columns_and_a_long_one_is_never_cut() {
    let decoded: Table = ingest(response(
        &["Name", "Ready", "Age"],
        vec![
            vec![Value::String("web".to_string())],
            vec![
                Value::String("api".to_string()),
                Value::String("1/1".to_string()),
                Value::String("3d".to_string()),
                Value::String("extra".to_string()),
            ],
        ],
    ));
    println!(
        "short: {:?}\nlong:  {:?}",
        decoded.rows[0].cells, decoded.rows[1].cells
    );
    assert_eq!(
        decoded.rows[0].cells,
        ["web", "", ""],
        "a row with one cell and three columns was not padded, so a renderer walking the columns \
         would index past the end"
    );
    assert_eq!(
        decoded.rows[1].cells,
        ["api", "1/1", "3d", "extra"],
        "a cell the server sent was thrown away, which is a collection bound this box does not \
         decide"
    );
}

/// **The request is the path kube would have built and the Accept header the box names, byte for
/// byte** — and the `406` fallback is the *same* path, never one rebuilt beside it.
///
/// The namespace is dropped for a cluster-scoped kind because `/api/v1/namespaces/x/nodes` is a
/// path no server answers, and `namespaced` is discovery's own flag rather than a list of kinds
/// (invariant 12). The CRD row is what says none of this knows a built-in from anything else.
#[test]
fn the_table_fetch_is_one_path_one_header_and_the_fallback_reuses_the_path() {
    let cases = [
        (
            browsed("", "v1", "Pod", "pods", Scope::Namespaced),
            Some("kube-system"),
            "/api/v1/namespaces/kube-system/pods",
        ),
        (
            browsed("", "v1", "Pod", "pods", Scope::Namespaced),
            None,
            "/api/v1/pods",
        ),
        (
            browsed("apps", "v1", "Deployment", "deployments", Scope::Namespaced),
            Some("payments"),
            "/apis/apps/v1/namespaces/payments/deployments",
        ),
        (
            browsed("", "v1", "Node", "nodes", Scope::Cluster),
            Some("payments"),
            "/api/v1/nodes",
        ),
        (
            browsed(
                "example.com",
                "v1alpha1",
                "Widget",
                "widgets",
                Scope::Namespaced,
            ),
            Some("payments"),
            "/apis/example.com/v1alpha1/namespaces/payments/widgets",
        ),
    ];
    for (kind, namespace, path) in &cases {
        let fetch = Fetch::table(kind, *namespace).expect("an ordinary kind builds a path");
        println!("{} ns={namespace:?} -> {}", kind.plural, fetch.path);
        assert_eq!(&fetch.path, path, "the path for {} is wrong", kind.plural);
        assert_eq!(
            fetch.accept, "application/json;as=Table;g=meta.k8s.io;v=v1,application/json",
            "the Accept header is not the one todo.md § Phase 5 names"
        );
        assert!(
            !fetch.path.contains("includeObject"),
            "includeObject is on the wire; k8s.rs § THE BROWSER'S ROWS keeps the server default \
             instead, and the doc comment is the record of it"
        );

        let fallback = fetch.plain();
        assert_eq!(
            fallback.path, fetch.path,
            "the 406 fallback went somewhere else than the request that was refused"
        );
        assert_eq!(
            fallback.accept, "application/json",
            "the fallback still asks for a Table"
        );
    }
}

/// **A kind whose own words cannot build a URL is never fetched** ([`path_safe`]).
///
/// **The threat is not hypothetical and it is not a CRD**: `run_aggregated()` copies
/// `resources[].resource` into the plural unchecked (`parse.rs:115-132`), so whoever runs an
/// aggregated API server on the cluster chooses this string. A row labelled *widgets* that GETs
/// `/api/v1/namespaces/kube-system/secrets` with the reader's own credentials is the shape.
///
/// **The last two cases are the ones nobody would have thought to feed**: a plural the ingest
/// guard itself *shortened* ends in `… (shortened by k8rs)`, which is our text and not the
/// cluster's and is still not a path; and the guard leaves `[2J` behind when it removes the escape
/// that made it dangerous, which is D146 working exactly as written and still not a path either.
#[test]
fn a_kind_whose_own_words_cannot_build_a_url_is_never_fetched() {
    let long = "w".repeat(IDENTIFIER + 1);
    let refused: [(&str, &str, &str); 8] = [
        ("example.com", "v1", "pods/../secrets"),
        ("example.com", "v1", "widgets?labelSelector=x"),
        ("example.com", "v1", "widgets#anchor"),
        ("example.com", "v1", ".."),
        ("example.com", "v1", ""),
        ("example.com", "v1", "wid gets"),
        ("../../apis/apps", "v1", "widgets"),
        ("example.com", "v1/../../v2", "widgets"),
    ];
    for (group, version, plural) in refused {
        let kind = browsed(group, version, plural, plural, Scope::Namespaced);
        println!("refused: {group:?} {version:?} {plural:?}");
        assert_eq!(
            Fetch::table(&kind, Some("kube-system")),
            None,
            "{group}/{version} {plural} built a URL path — a request the reader never asked \
             for, going out with the reader's own credentials"
        );
        assert!(
            Browsing::open(kind, Some("kube-system")).is_none(),
            "a view opened on a kind no request can be built for"
        );
    }

    // The ingest guard's own two outputs, fed to the sink that has to survive them.
    for plural in ["\u{1b}[2Jwidgets".to_string(), long.clone()] {
        let kind = browsed("example.com", "v1", "Widget", &plural, Scope::Namespaced);
        println!("after the guard: {:?}", kind.plural);
        assert_ne!(
            kind.plural, plural,
            "the ingest guard left this plural alone, so this case is not about what it produces"
        );
        assert_eq!(
            Fetch::table(&kind, None),
            None,
            "a plural the guard rewrote was still used to build a URL"
        );
    }

    // The negative: every ordinary shape a real cluster serves is kept.
    let kept: [(&str, &str, &str); 6] = [
        ("", "v1", "pods"),
        ("apps", "v1", "deployments"),
        ("policy", "v1", "poddisruptionbudgets"),
        ("storage.k8s.io", "v1", "csistoragecapacities"),
        ("cert-manager.io", "v1alpha2", "certificaterequests"),
        ("1password.com", "v1", "onepassworditems"),
    ];
    for (group, version, plural) in kept {
        let kind = browsed(group, version, "Widget", plural, Scope::Namespaced);
        let fetch = Fetch::table(&kind, Some("payments"));
        println!("kept: {group:?} {version:?} {plural:?} -> {fetch:?}");
        assert!(
            fetch.is_some(),
            "{group}/{version} {plural} is a shape a real cluster serves and it was refused"
        );
    }
}

/// **The namespace is judged by the same predicate, one argument along.**
///
/// The first draft exempted it and reasoned from the source — *the caller typed it* — which is
/// true only while the source is `--namespace`. The namespace **picker** is a later box in this
/// same phase and it is fed from the cluster's own namespace list, so the exemption had a shelf
/// life. `x?watch=true` is the exact failure [`path_safe`]'s doc names for a plural: a query
/// parameter on a call the command log prints without it, which is invariant 4's record lying.
///
/// **A cluster-scoped kind drops the namespace before it is judged**, so a stray one cannot
/// refuse a fetch that would never have carried it.
#[test]
fn a_namespace_that_cannot_be_a_path_segment_is_refused_too() {
    let pods = browsed("", "v1", "Pod", "pods", Scope::Namespaced);
    let nodes = browsed("", "v1", "Node", "nodes", Scope::Cluster);

    for namespace in ["../../../api/v1/secrets", "x?watch=true", "a#b", "", "a b"] {
        println!(
            "namespaced {namespace:?} -> {:?}",
            Fetch::table(&pods, Some(namespace)).map(|fetch| fetch.path)
        );
        assert_eq!(
            Fetch::table(&pods, Some(namespace)),
            None,
            "a namespace of {namespace:?} built a URL path — the reader's credentials on a \
             request the reader did not make"
        );
        assert!(
            Browsing::open(pods.clone(), Some(namespace)).is_none(),
            "a view opened on a namespace no request can be built for"
        );
        // The cluster-scoped kind never carries it, so it is never judged.
        assert_eq!(
            Fetch::table(&nodes, Some(namespace))
                .expect("a cluster-scoped kind drops the namespace before judging it")
                .path,
            "/api/v1/nodes"
        );
    }

    for namespace in ["kube-system", "payments", "default", "team-1.example"] {
        let fetch = Fetch::table(&pods, Some(namespace)).expect("an ordinary namespace is a path");
        println!("kept: {namespace:?} -> {}", fetch.path);
        assert_eq!(fetch.path, format!("/api/v1/namespaces/{namespace}/pods"));
    }
}

/// **Only a `406` asks again; every other refusal is the reader's news.**
///
/// **Synthesised, and it says so**: the kind cluster this box was measured on runs no aggregated
/// API server, so no capture of a real `406` exists and this drives the branch with a `Status`
/// built here. What it cannot prove is what a real aggregated server puts in that body —
/// `k8s.rs` § THE BROWSER'S ROWS names the one shape the predicate would miss.
#[test]
fn only_a_406_asks_again_for_the_plain_object_list() {
    assert!(
        not_acceptable(&refused(406)),
        "a 406 did not fall back, so a browser breaks on somebody's aggregated API"
    );
    for code in [0, 200, 401, 403, 404, 409, 410, 429, 500, 503] {
        println!("{code} -> {}", not_acceptable(&refused(code)));
        assert!(
            !not_acceptable(&refused(code)),
            "a {code} asked again without the Table header, which answers the reader's 403 with a \
             second request instead of telling them"
        );
    }
    assert!(
        !not_acceptable(&kube::Error::TlsRequired),
        "a failure that never reached the API server was read as a refusal of the Table header"
    );
}

/// **A `200` that is not a `Table` is read as the object list it is**, and a one-column table is
/// what a plain list can honestly be drawn as (invariant 12: `metadata` is the only thing in one
/// that is not per-kind, and a column of ages would need a clock this file does not read —
/// invariant 5, NOTES § D18).
///
/// **This is the Accept header's own second half, not the `406` path.** `,application/json` means
/// a server that cannot print a `Table` answers **200** with the ordinary list, so
/// [`not_acceptable`] never sees it and the fallback never fires — the decode is the only place
/// that can notice. A body whose `kind` is not `Table` used to decode to
/// `Table { columns: [], rows: [] }` with no error at all: six Deployments in, an empty screen
/// out, silently.
///
/// Driven by a committed `kind: List` capture, which is the exact shape both that server and the
/// `406` fallback send.
#[test]
fn a_two_hundred_that_is_not_a_table_is_read_as_the_object_list_it_is() {
    let raw = capture("deployments");
    let sent = raw["items"]
        .as_array()
        .expect("the capture is a list")
        .len();
    assert_eq!(
        raw["kind"], "List",
        "deployments.json is no longer a plain object list, so this test is about another shape"
    );
    let response: TableResponse =
        serde_json::from_value(raw).expect("a captured object list decodes through the one type");
    let decoded: Table = ingest(response);

    println!(
        "{sent} items -> {:?} / {} rows",
        headers(&decoded),
        decoded.rows.len()
    );
    assert_eq!(
        headers(&decoded),
        ["Name"],
        "the fallback drew a column the API server never printed"
    );
    assert_eq!(
        decoded.rows.len(),
        sent,
        "the fallback lost objects the list carried — an empty screen for a namespace that has \
         six Deployments in it"
    );
    assert!(
        sent > 1,
        "the capture has one item, so a row count proves nothing"
    );
    for row in &decoded.rows {
        assert_eq!(
            row.cells.len(),
            1,
            "a fallback row has cells the one column cannot draw"
        );
        assert_eq!(
            row.name.as_deref(),
            Some(row.cells[0].as_str()),
            "the fallback's cell and its identity disagree"
        );
        assert!(
            row.namespace.is_some() && row.uid.is_some(),
            "a fallback row lost the identity every dialog needs: {row:?}"
        );
    }

    // The other direction: a real `Table` is still read as one by the same decode, so the branch
    // cannot be *always the list* any more than it could be *always the table*.
    let table = table("table-pods");
    assert_eq!(
        table.columns.len(),
        9,
        "a Table body was read as an object list"
    );
    assert_eq!(table.rows.len(), 14);
}

/// **Every `String` the browser's rows keep is named by the ingest guard**, derived off `k8s.rs`
/// rather than typed out here — the same guard [`Browsable`] and the snapshot types get. A field
/// added to [`Row`] or [`Column`] and forgotten in the guard fails this.
#[test]
fn every_string_the_browsers_rows_keep_is_named_by_the_ingest_guard() {
    let types = declared_types(K8S_SOURCE);
    for (name, least) in [("Row", 4), ("Column", 1)] {
        let fields = types.get(name).unwrap_or_else(|| {
            panic!("k8s.rs no longer declares {name}, or declares it differently")
        });
        let carries_text: Vec<&str> = fields
            .iter()
            .filter(|(_, kind)| words(kind).any(|word| word == "String"))
            .map(|(field, _)| *field)
            .collect();
        assert!(
            carries_text.len() >= least,
            "only {carries_text:?} were parsed out of {name}, so this guard is reading nothing"
        );
        let body = bounded_impl(name).unwrap_or_else(|| {
            panic!("{name} carries text and the ingest guard region has no impl Bounded for it")
        });
        for field in carries_text {
            println!("bounded: {name}.{field}");
            assert!(
                words(body).any(|word| word == field),
                "{name}.{field} is a String a row keeps and the ingest guard never names it"
            );
        }
    }
}

// --- KEEPING A BROWSER VIEW FRESH ---
//
// **`PRIOR-ART § A5` is what these are written against**: k9s merged *skip the reconcile when
// nothing changed* and reverted it a month later, because a coalescer that drops the last event of
// a burst shows stale data forever and passes every test that stops before the storm ends. So the
// three that matter here all end *after* the last change: one asserts the fetch that serves it
// happens, one asserts a change arriving mid-flight is not answered by a response that predates
// it, and one asserts no second fetch is on the wire beside the first — the half the review found
// open, where an answer arriving out of order leaves the view on pre-change rows with nothing
// pending to re-arm (NOTES § D154).
//
// **Every fetch is answered with `done()`, and that is not ceremony.** The floor is measured from
// the moment an answer came back, so a test that issues and never completes is testing a frozen
// view — which is itself asserted, once, rather than left to be the accidental shape of the rest.
//
// **No clock is read.** Every moment below is handed in, as it is everywhere else in this file
// (invariant 5, NOTES § D18).

/// A moment this many milliseconds after another.
fn after(base: &Time, millis: i64) -> Time {
    Time(
        base.0
            .checked_add(SignedDuration::from_millis(millis))
            .expect("a test moment fits in a timestamp"),
    )
}

/// A view on one kind, freshly opened.
fn opened() -> Browsing {
    Browsing::open(
        browsed("apps", "v1", "Deployment", "deployments", Scope::Namespaced),
        Some("payments"),
    )
    .expect("an ordinary kind can be browsed")
}

/// **A view fetches the moment it opens, and then not again until something changes** — the
/// no-polling half of invariant 6 at the one place a browser could quietly reintroduce one.
#[test]
fn a_view_fetches_when_it_opens_and_then_only_when_something_changes() {
    let open = now();
    let mut view = opened();

    // What a pane title and the `ns:` label are drawn off (`screens/resources.md`): the view
    // remembers the kind discovery described, not a name k8rs made up for it.
    assert_eq!(view.kind().plural, "deployments");
    assert_eq!(view.kind().group, "apps");
    assert!(
        view.kind().namespaced,
        "the view lost the flag the ns: label is drawn from"
    );

    let first = view
        .issue(&open)
        .expect("a view that has never fetched owes one at once");
    println!("opened -> {first:?}");
    assert_eq!(
        first.path, "/apis/apps/v1/namespaces/payments/deployments",
        "the view fetched something other than the kind and namespace it was opened on"
    );
    assert_eq!(first.accept, TABLE_ACCEPT);
    view.done(&open);

    for millis in [0, 500, 1_000, 60_000] {
        assert!(
            view.issue(&after(&open, millis)).is_none(),
            "a view nothing changed on re-fetched {millis}ms later, which is the poll invariant 6 \
             refuses"
        );
    }
    assert_eq!(
        view.due_at(&after(&open, 60_000)),
        None,
        "a view with nothing pending told the loop to wake up anyway"
    );
}

/// **A storm costs one fetch per floor, and the last change of it is never dropped**
/// (`PRIOR-ART § A5`). The assertion that matters is the one *after* the storm: a coalescer that
/// swallowed the final event would leave the view showing what it showed before the deploy, and
/// every test that stopped at the last `changed()` would still be green.
#[test]
fn a_storm_costs_one_fetch_per_floor_and_the_last_change_is_still_served() {
    let open = now();
    let mut view = opened();
    view.issue(&open).expect("the opening fetch");
    view.done(&open);

    // The rollout: events every 50ms for half a second.
    for step in 1..=10 {
        view.changed();
        let moment = after(&open, step * 50);
        assert!(
            view.issue(&moment).is_none(),
            "a fetch went out {}ms after the last one, under the {REFRESH_FLOOR:?} floor",
            step * 50
        );
    }
    assert_eq!(
        view.due_at(&after(&open, 500)),
        Some(after(&open, 1_000)),
        "the loop was not told when to come back for the fetch the floor is holding"
    );

    let served = view
        .issue(&after(&open, 1_000))
        .expect("the storm's last change is served once the floor has passed");
    println!("storm of 10 changes -> one fetch at +1000ms: {served:?}");
    view.done(&after(&open, 1_000));
    assert!(
        view.issue(&after(&open, 5_000)).is_none(),
        "the view kept fetching after the storm ended with nothing left to show"
    );
}

/// **A change that arrives while a fetch is in flight is not answered by that fetch.**
///
/// The response left the server before the change happened, so it cannot contain it. This is why
/// the pending flag is cleared when a request is *issued* and not when it returns — the one line
/// `PRIOR-ART § A5` says k9s got wrong in the other direction.
#[test]
fn a_change_that_lands_mid_flight_is_not_swallowed_by_the_request_it_overtook() {
    let open = now();
    let mut view = opened();
    view.issue(&open).expect("the opening fetch");

    // The request is still on the wire when the cluster changes again.
    view.changed();
    view.done(&after(&open, 100));

    assert!(
        view.issue(&after(&open, 100)).is_none(),
        "the floor did not hold a fetch that came back 100ms after the last one went out"
    );
    assert!(
        view.issue(&after(&open, 1_100)).is_some(),
        "a change that arrived while a fetch was in flight was cleared by that fetch's own \
         issue, so the view shows a cluster state that is one change out of date forever"
    );
}

/// **A view that has been quiet re-fetches at once** — the floor is a gap between fetches, not a
/// delay before one, so a reader who deletes a pod does not wait a second to watch the row go.
#[test]
fn a_quiet_view_refetches_the_moment_something_finally_changes() {
    let open = now();
    let mut view = opened();
    view.issue(&open).expect("the opening fetch");
    view.done(&open);

    let quiet = after(&open, 600_000);
    view.changed();
    assert_eq!(
        view.due_at(&quiet),
        Some(after(&open, 1_000)),
        "the deadline moved with the change rather than staying on the last fetch, which is a \
         debounce and not a floor"
    );
    assert!(
        view.issue(&quiet).is_some(),
        "a view idle for ten minutes still waited for the floor after one change"
    );
}

/// **Two fetches are never on the wire at once, and the cluster sets the pace** — the half of
/// `PRIOR-ART § A5` that clearing the flag at *issue* does not close.
///
/// Clearing at issue keeps a change from being answered by a response that predates it. It does
/// nothing about the second fetch going out while the first is still on the wire: at
/// 6852 bytes/row a 5000-row namespace is a 34 MB body, it takes however long the cluster takes,
/// and a rolling deploy issues one per floor. **HTTP/2 gives no ordering guarantee**, so the
/// answer to the older request can land last and leave the view showing rows from before the
/// change that asked for it — with nothing pending to re-arm, until some unrelated change minutes
/// later. Holding one at a time is what makes that arrival order impossible rather than unlikely.
///
/// **It is also where [`REFRESH_FLOOR`] stops being the whole answer**: the gap is measured from
/// the moment an answer came back, so a fetch that takes three seconds is followed a second
/// later and not two seconds *ago*. No second constant, and the cluster tunes it.
#[test]
fn two_fetches_are_never_on_the_wire_at_once_and_a_slow_cluster_sets_the_pace() {
    let open = now();
    let mut view = opened();
    view.issue(&open).expect("the opening fetch");

    // The cluster keeps changing while that first fetch is still on the wire. The last moment is
    // ten minutes in, which is the ceiling stated rather than left to be found: a caller that
    // never says `done()` has frozen its own view, and no floor and no change will unfreeze it.
    for millis in [1_000, 2_000, 3_000, 600_000] {
        view.changed();
        assert!(
            view.issue(&after(&open, millis)).is_none(),
            "a second fetch went out {millis}ms in with the first still on the wire: two bodies \
             of one list are held at once, and the older answer can arrive last"
        );
        assert_eq!(
            view.due_at(&after(&open, millis)),
            None,
            "the loop was told to wake up at {millis}ms for a fetch that cannot be issued until \
             the one in flight comes back"
        );
    }

    // It lands after three seconds. The change that arrived while it was out is still owed.
    view.done(&after(&open, 3_000));
    assert_eq!(
        view.due_at(&after(&open, 3_000)),
        Some(after(&open, 4_000)),
        "the floor was measured from the moment the fetch was issued, so a fetch slower than the \
         floor is followed by one that was due before it even came back"
    );
    assert!(
        view.issue(&after(&open, 3_999)).is_none(),
        "the floor did not hold, so a three-second fetch would be re-issued the moment it landed"
    );
    let next = view
        .issue(&after(&open, 4_000))
        .expect("the change that arrived mid-flight is served once the answer is in");
    println!("3s fetch, changes at +1s/+2s/+3s -> one fetch at +4000ms: {next:?}");

    // And the pace is the cluster's: the same view against a 30ms answer is back at the floor.
    view.done(&after(&open, 4_030));
    view.changed();
    assert_eq!(
        view.due_at(&after(&open, 4_030)),
        Some(after(&open, 5_030)),
        "a fast answer did not put the view back on the floor, so the pace is not the cluster's"
    );
}

/// **Only the Alerts view's inputs are watched permanently** (invariant 6,
/// `screens/resources.md`), derived off `k8s.rs` rather than asserted in prose. A sixth `Watch<>`
/// on [`Store`] — a browser kind promoted to a permanent stream, which is the forty-streams
/// failure this architecture exists to avoid — fails here.
#[test]
fn only_the_alerts_views_inputs_are_watched_permanently() {
    let types = declared_types(K8S_SOURCE);
    let permanent: Vec<&str> = types
        .get("Store")
        .expect("k8s.rs no longer declares Store, or declares it differently")
        .iter()
        .filter(|(_, kind)| kind.starts_with("Watch<"))
        .map(|(field, _)| *field)
        .collect();
    println!("permanent watches: {permanent:?}");
    assert_eq!(
        permanent,
        [
            "pods",
            "nodes",
            "deployments",
            "stateful_sets",
            "daemon_sets"
        ],
        "the permanent watch set is no longer invariant 6's five"
    );

    let browsing = types
        .get("Browsing")
        .expect("k8s.rs no longer declares Browsing, or declares it differently");
    assert!(
        !browsing.is_empty(),
        "no fields were parsed out of Browsing, so this half of the guard reads nothing"
    );
    for (field, kind) in browsing {
        assert!(
            !kind.contains("Watch<"),
            "Browsing.{field} holds a watch ({kind}); a browser view's stream is the caller's and \
             dies when the view is dropped"
        );
    }
}
