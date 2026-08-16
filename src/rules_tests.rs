use super::*;

// `ContainerStateRunning` is imported here and not beside the decode's own types: no
// product code in this file constructs one, and the top-level list is what `rules.rs`
// reads off the API.
use k8s_openapi::api::core::v1::{
    ContainerStateRunning, ContainerStateWaiting, HostPathVolumeSource, Taint as ApiTaint,
    Toleration as ApiToleration, Volume, VolumeMount,
};

use k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference;

use std::collections::{BTreeSet, HashSet};

// --- ONE MODULE PER REGION OF rules.rs ---
//
// Each child below holds the tests for the `rules.rs` region it is named after (NOTES § D91).
// What stays in this file is what more than one of them reads, reached as `super::` — a
// helper copied into two modules is the divergence the split is not allowed to grow.

#[path = "rules_tests/snapshot.rs"]
mod snapshot;

#[path = "rules_tests/pod.rs"]
mod pod;

#[path = "rules_tests/node.rs"]
mod node;

#[path = "rules_tests/workload.rs"]
mod workload;

#[path = "rules_tests/certificate.rs"]
mod certificate;

fn deployment(uid: &str) -> ObjectId {
    ObjectId {
        kind: ObjectKind::Deployment,
        namespace: Some("payments".to_string()),
        name: "web".to_string(),
        uid: Some(uid.to_string()),
    }
}

// --- THE DECODE, AGAINST THE COMMITTED CAPTURES ---
//
// Every fixture below came off the kind cluster `scripts/cluster.sh` builds, was
// verified to have reached the state its rule is about, and was sanitized on the way
// out (`just fixtures`). What is asserted is the value a later rule fires on — not
// that the JSON parsed, which is `serde`'s claim and not ours.

fn fixture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} could not be read: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {path} is not JSON: {e}"))
}

/// C1's input is a PEM file, not JSON — `scripts/make-certs.sh` writes the three with
/// pinned dates, so a certificate fixture cannot expire out from under a test.
fn certificate(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/certs/{name}.crt.pem",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("certificate {path} could not be read: {e}"))
}

fn pod(name: &str) -> PodSnapshot {
    let pod: Pod = serde_json::from_value(fixture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
    PodSnapshot::from(pod)
}

/// `kubectl get -A` answers with `kind: List`, which `k8s_openapi::List<T>` refuses
/// — it wants `NodeList` — so the items come out by hand here. JSON plumbing in a
/// test, not a second decode of the snapshot types.
fn items<T: k8s_openapi::serde::de::DeserializeOwned>(name: &str) -> Vec<T> {
    fixture(name)["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{name}.json has no items array"))
        .iter()
        .map(|v| {
            serde_json::from_value(v.clone())
                .unwrap_or_else(|e| panic!("{name}.json item does not decode: {e}"))
        })
        .collect()
}

fn time(s: &str) -> Time {
    Time(
        s.parse()
            .unwrap_or_else(|e| panic!("{s} is not a time: {e}")),
    )
}

// --- WHAT THE CAPTURE ITSELF SAYS START ---
//
// **An expectation read out of the capture, for the fields whose value belongs to the
// cluster rather than to the requirement.** A uid, a timestamp, a restart counter, a
// node name and a scheduler's sentence are all new on every `just fixtures`: a literal
// transcribed from one capture asserts the cluster that produced it, and the next trip
// reddens a test whose requirement never moved (which is exactly what happened here —
// the four-node capture of 2026-08-12 turned seventeen of these red at once).
//
// **It is not a tautology, because the decode is not what it is read from.** These are
// decode assertions and what they exist to catch is a field dropped on the way
// through, filled from its neighbour, or rewritten — comparing the snapshot against
// the *JSON the decode was handed* catches all three, and names the path it must have
// come from while doing it (`initContainerStatuses` is not `containerStatuses`, and
// `startedAt` is not `finishedAt`).
//
// **What it cannot check is that the capture still has the shape the test exists
// for** — a fixture whose restart count fell to zero would satisfy every derived
// assertion above and prove nothing about rule 5. So each use is paired with the
// property the fixture has to keep: a counter inside its rule's band, two moments that
// differ, a message that still names the object it is evidence about.
//
// Absence panics rather than comparing `None` against `None`: a path that stops
// matching the capture is the one failure this technique could otherwise hide.

fn at<'a>(value: &'a serde_json::Value, path: &[&str]) -> &'a serde_json::Value {
    let mut at = value;
    for key in path {
        at = &at[key];
    }
    at
}

fn captured_str<'a>(value: &'a serde_json::Value, path: &[&str]) -> &'a str {
    at(value, path).as_str().unwrap_or_else(|| {
        panic!("the capture carries no string at {path:?}, so nothing here is compared against it")
    })
}

/// `i32` because that is what the API declares a restart count and every replica
/// counter as, and what the snapshot types carry.
fn captured_i32(value: &serde_json::Value, path: &[&str]) -> i32 {
    let n = at(value, path).as_i64().unwrap_or_else(|| {
        panic!("the capture carries no number at {path:?}, so nothing here is compared against it")
    });
    i32::try_from(n)
        .unwrap_or_else(|_| panic!("{path:?} is {n}, which is not the i32 the API declares"))
}

fn captured_time(value: &serde_json::Value, path: &[&str]) -> Time {
    time(captured_str(value, path))
}

/// One container's captured status, out of the array it has to have come from.
/// **Which array is part of the assertion** — a number read back from
/// `containerStatuses` is not evidence about the init one, and reading both lists is
/// the whole of D27.
fn captured_status<'a>(
    pod: &'a serde_json::Value,
    array: &str,
    name: &str,
) -> &'a serde_json::Value {
    pod["status"][array]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("the capture has no {name} in status.{array}"))
}

/// One condition of a captured object, by type — the array [`PodSnapshot::ready`],
/// [`PodSnapshot::scheduled`] and every `NodeSnapshot` condition are picked out of by
/// name, and picking by index instead is the defect the healthy pod exists to catch.
fn captured_condition<'a>(object: &'a serde_json::Value, type_: &str) -> &'a serde_json::Value {
    object["status"]["conditions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["type"] == type_)
        .unwrap_or_else(|| panic!("the capture carries no {type_} condition"))
}

/// One item of a `kubectl get -A` capture, by name — the List counterpart of
/// [`captured_status`].
fn captured_item<'a>(list: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    list["items"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|i| i["metadata"]["name"] == name)
        .unwrap_or_else(|| panic!("the capture has no {name} in its items"))
}
// --- WHAT THE CAPTURE ITSELF SAYS END ---

/// **The moment every snapshot in this file is read at.** One helper rather than a
/// literal at each construction site: the pin is a single fact about the committed
/// captures, and a copy of a fact is a copy that drifts.
///
/// **The value is not free.** `scripts/certs-test.sh` extracts this literal out of this
/// function, refuses to disagree with it, and asserts the committed certificates against
/// it on every `just check`. **Moving it moves `scripts/certs-test.sh` and
/// `scripts/make-certs.sh` in the same change** — the pin is one fact spelled in four
/// places across two ownership rows (NOTES § D57).
///
/// It also lands after every `Time` the snapshot types *expose*, which
/// `the_pinned_now_is_not_before_the_captures_it_is_read_against` asserts rather than
/// leaves to trust.
///
/// **The shape of the value is the midnight after the *newest* capture** — near enough that
/// a fixture's age is one an operator would recognise, and round enough to be repeated in
/// three other files without transcription error.
///
/// **It was the midnight after *the capture day* until 2026-08-16, and the corpus stopped
/// having one** (NOTES § D57, § D97). `neverback.json` was taken on 2026-08-15 as a single
/// fixture rather than a re-run of everything, so the pin sat between two capture dates and
/// rendered that capture's card with no age at all —
/// [`the_pinned_now_is_not_before_the_captures_it_is_read_against`] is what said so. The rule
/// that survives is the general one: **the pin follows the corpus**, because every capture
/// ever taken is newer than a clock pinned before it.
///
/// **The corpus is one trip again since 2026-08-16** — `just fixtures` re-took all of it from one
/// cluster in one morning, and four pod captures landed with it — so the two readings agree today
/// and the general rule is what is written down (NOTES § D114). **A repin is an edit in two
/// ownership rows**: `scripts/certs-test.sh` extracts this literal, compares it against its own
/// `now=`, and asserts three certificate day-counts against it, so it moves in the same change or
/// `just check` goes red on the guard rather than on anything here.
fn now() -> Time {
    time("2026-08-17T00:00:00Z")
}

fn container<'a>(pod: &'a PodSnapshot, name: &str) -> &'a ContainerSnapshot {
    pod.containers
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("{} has no container {name}", pod.id.name))
}

/// **Every pod capture in the repository**, and the claim is checked rather than assumed:
/// `just fixtures` guards each file, and `the_whole_capture_through_the_rules_at_once` names
/// which of these are allowed to draw nothing, so a fixture added to `tests/fixtures` and not
/// to this list shows up as a capture no test reads.
///
/// Named once because four things read the same set — the join, the pin guard, the whole-capture
/// run and every node-rule join through [`every_captured_pod`] — and a second copy is a second
/// list to keep in step. **The pin guard is why completeness matters**: it walks only what is in
/// this snapshot, so a capture left out of it is a capture whose timestamps were never compared
/// against [`now`].
const CAPTURED_PODS: [&str; 36] = [
    "config",
    "crashloop",
    "exit0",
    "failed",
    "gang",
    "healthy-hostpath",
    "healthy-podlevel",
    "healthy-retry",
    "healthy-sidecar",
    "healthy-unreadysidecar",
    "healthy",
    "hostpath",
    "image",
    "init",
    "neverback",
    "neverrules",
    "nolimits",
    "notfound",
    "oom",
    "oomserving",
    "pending",
    "podlimit",
    "probe0",
    "readiness",
    "reboot",
    "resize",
    "restarts",
    "restarts10",
    "restarts10serving",
    "sigterm",
    "socket",
    "startup",
    "stuck",
    "succeeded",
    "unjudged",
    "wedged",
];

/// The snapshot a rule would be handed if it ran over the whole committed capture at
/// once: every pod, every node, the Deployments, and the pinned [`now`](now).
fn fixture_snapshot() -> ClusterSnapshot {
    ClusterSnapshot {
        now: now(),
        pods: CAPTURED_PODS.iter().map(|n| pod(n)).collect(),
        nodes: items::<Node>("nodes").into_iter().map(Into::into).collect(),
        workloads: items::<Deployment>("deployments")
            .into_iter()
            .map(Into::into)
            .collect(),
        server_version: Some(
            std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/K8S_VERSION"
            ))
            .expect("the capture stamps the version it came from")
            .trim()
            .to_string(),
        ),
        context: Some("kind-k8rs".to_string()),
        client_certificate: Some(certificate("healthy-client")),
        // `just fixtures` captures `-A`, so this snapshot covers the whole cluster
        // and N2 and N5 are allowed to run over it.
        namespace_scope: None,
    }
}

/// The snapshot [`analyze`] is handed below: pods and a moment. No nodes and no
/// workloads, because every rule in this box reads a Pod — the joins belong to the
/// N-series and W-series, which are later boxes and would only add rules that cannot
/// fire to every assertion here.
fn pods_at(pods: Vec<PodSnapshot>, now: Time) -> ClusterSnapshot {
    ClusterSnapshot {
        now,
        pods,
        nodes: Vec::new(),
        workloads: Vec::new(),
        server_version: None,
        context: None,
        client_certificate: None,
        namespace_scope: None,
    }
}

/// **Five minutes into the newest run this pod holds** — a moment where every card the pod draws
/// is still news, whichever container carries one. For the tests whose subject is not rule 5 at
/// all but whose pod has a serving container in rule 5's band: that card ages out at
/// [`NOT_READY_GRACE`] (NOTES § D100), and at the pin it would be missing from a set the test is
/// comparing against another set. [`now`] for a pod with nothing running — no card such a pod
/// draws can age out, because rule 5's clause is the serving branch's alone.
fn while_its_cards_draw(p: &PodSnapshot) -> Time {
    p.containers
        .iter()
        .filter_map(|c| match &c.state {
            ContainerState::Running { started_at } => started_at.clone(),
            _ => None,
        })
        .max_by_key(|t| t.0)
        .map_or_else(now, |began| {
            Time(
                began
                    .0
                    .checked_add(SignedDuration::from_mins(5))
                    .expect("a moment inside a captured run"),
            )
        })
}

/// One finding as `--once` would print it (`screens/once.md`) — so that
/// `cargo test -- --nocapture` shows the sentences a user reads and not a `Debug` dump
/// of the struct they came in. CLAUDE.md's "green tests are not working software" gate
/// is read off this.
fn card(f: &Finding, now: &Time) -> String {
    let mark = match f.severity {
        Severity::Critical => '●',
        Severity::Warn => '▲',
        Severity::Info => '○',
    };
    let name = match &f.owner.namespace {
        Some(ns) => format!("{ns}/{}", f.owner.name),
        None => f.owner.name.clone(),
    };
    let age = f.age(now).map_or(String::new(), |a| format!(" · {a}"));
    format!(
        "{mark} {name}{age}\n  {}\n  {}\n  → {}\n  $ {}\n",
        f.title,
        f.evidence,
        f.action,
        f.kubectl_cmd
            .as_deref()
            .unwrap_or("(no command shows this)")
    )
}

fn titles(all: &[Finding]) -> Vec<&str> {
    all.iter().map(|f| f.title.as_str()).collect()
}

fn show(all: &[Finding]) {
    show_at(all, &now());
}

/// The same, for the tests that read a capture at a moment of their own: the age on the card is
/// the one *that* reader sees, and printing it against the pin would draw none at all.
fn show_at(all: &[Finding], now: &Time) {
    for f in all {
        println!("{}", card(f, now));
    }
}

/// The one finding on `pod` whose title contains `phrase` — and a failure when there is
/// not exactly one. "The rule fired" and "the rule fired twice on one container" print
/// the same green line otherwise.
fn only<'a>(all: &'a [Finding], pod: &str, phrase: &str) -> &'a Finding {
    let mut hits = all
        .iter()
        .filter(|f| f.object.name == pod && f.title.contains(phrase));
    let found = hits
        .next()
        .unwrap_or_else(|| panic!("nothing on {pod} says {phrase:?} — got {:?}", titles(all)));
    assert!(
        hits.next().is_none(),
        "two findings on {pod} say {phrase:?} — got {:?}",
        titles(all)
    );
    found
}

fn nothing(all: &[Finding], why: &str) {
    assert!(all.is_empty(), "{why} — got {:?}", titles(all));
}

/// A committed capture with one field moved — the technique the rest of this file uses
/// for a shape no capture holds. The committed JSON is never touched (NOTES § D53); the
/// decoded copy is.
fn capture_but(name: &str, edit: impl FnOnce(&mut Pod)) -> PodSnapshot {
    let mut object: Pod = serde_json::from_value(fixture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
    edit(&mut object);
    PodSnapshot::from(object)
}

/// **The one pod of `owned-pods.json`, by the name the capture gives it.** A ReplicaSet's
/// pods carry a generated five-character suffix minted fresh on every `just fixtures`,
/// while the ReplicaSet's own name is a hash of the pod template and does not move — so
/// the suffix is read out of the capture and the hash is written down.
fn owned_pod_name() -> String {
    let pods = items::<Pod>("owned-pods");
    assert_eq!(
        pods.len(),
        1,
        "`broken-owned` runs one replica, and every assertion below names *the* pod"
    );
    pods[0]
        .metadata
        .name
        .clone()
        .expect("a captured pod has a name")
}

/// The captured node list, decoded — the other half of every join below.
fn captured_nodes() -> Vec<NodeSnapshot> {
    items::<Node>("nodes").into_iter().map(Into::into).collect()
}

/// **Every pod the capture holds, in both namespaces it photographed.** The node rules are
/// joins, and joining only the twelve `default` pods hides the two shapes N2 exists to skip:
/// `kube-system` is where the DaemonSet and the static pods are, and on this cluster they are
/// the only ones there are.
fn every_captured_pod() -> Vec<PodSnapshot> {
    CAPTURED_PODS
        .iter()
        .map(|n| pod(n))
        .chain(
            items::<Pod>("kube-system-pods")
                .into_iter()
                .map(PodSnapshot::from),
        )
        .collect()
}

/// [`pods_at`] with the node list filled in — the snapshot a node rule is actually handed.
fn cluster(pods: Vec<PodSnapshot>, nodes: Vec<NodeSnapshot>) -> ClusterSnapshot {
    ClusterSnapshot {
        nodes,
        ..pods_at(pods, now())
    }
}

/// The one node in the capture whose `Ready` is not `True`, read out of the JSON rather than
/// transcribed: `break-nodes` picks which worker it stops, and a literal here would assert the
/// capture that happened to be taken (NOTES § D65).
fn the_quiet_node(raw: &serde_json::Value) -> &str {
    let down: Vec<&str> = raw["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|n| captured_condition(n, "Ready")["status"] != "True")
        .map(|n| captured_str(n, &["metadata", "name"]))
        .collect();
    assert_eq!(
        down.len(),
        1,
        "`break-nodes` stops exactly one kubelet, and N1's positive is that node: {down:?}"
    );
    down[0]
}

/// Captured nodes carrying one taint and no labels — the whole cluster refusing for one reason.
/// **`count` is not decoration**: the table's actions inflect, and the case it exists for is one
/// machine, so both sides have to be drawn (NOTES § D81).
fn tainted(count: usize, key: &str, value: Option<&str>, effect: &str) -> Vec<NodeSnapshot> {
    captured_nodes()
        .into_iter()
        .take(count)
        .map(|mut n| {
            n.taints = vec![Taint {
                key: key.to_string(),
                value: value.map(str::to_string),
                effect: effect.to_string(),
                added_at: None,
            }];
            n.labels.clear();
            n
        })
        .collect()
}
