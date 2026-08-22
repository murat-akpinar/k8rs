//! The cluster's side of the snapshot: the permanent watch, and the store the rules read.
//!
//! **The prune is the decode, and there is no separate pruning step here on purpose.** Invariant
//! 6 asks for "the fields the snapshot types in `rules.rs` name, across metadata, spec *and*
//! status" (NOTES § D69); the `From` impls in `rules.rs` § SNAPSHOT TYPES *are* that list, so
//! converting on the way in keeps exactly it and nothing else. No field list is repeated here:
//! the second copy is the one that goes stale, and a prune written from the structs is what
//! drops pod-level `spec.restartPolicy` — consumed by the decode, carried by no snapshot type,
//! and load-bearing for rules 1, 5 and 15 (NOTES § D97).
//!
//! **What that buys is the resident set and nothing else** (NOTES § D115). The API server has no
//! way to send a subset of `status`, so the whole object arrives and is deserialized before a
//! field is dropped; pruning removes no byte from the network and no microsecond from the
//! decode. It serves the resident-set budget `REQUIREMENTS.md` states at a cluster size, and a
//! comment here claiming it serves first paint would be a defect.
//!
//! **Nothing escapes [`Store::snapshot`] until every initial LIST has landed** (NOTES § D28). A
//! rule cannot tell a short list from a small cluster — invariant 5 leaves it no way to ask — so
//! a snapshot published mid-bootstrap makes rule 10 say *none of the 3 nodes have that label* on
//! a 200-node cluster.
//!
//! **[`drive`] is the loop, and it is a pump and nothing else.** Every decision about what an
//! event means lives in [`Store`], which is tested on its own by feeding it events by hand; the
//! loop's own test is that driving the same events through it lands the same store. The five
//! watches share one task and one `&mut Store`, so there is no lock and no channel, and the
//! loop names no kind: the three the browser's rule is written about (invariant 12) are worth
//! avoiding here for the same reason, one layer down.
//!
//! **What is still missing is the `Client`**, and with it the five `Api<K>`s [`drive`] would be
//! handed: that is the `connect()` box further down Phase 5. Nothing here has met an API
//! server.

// `expect` rather than `allow` because it expires by itself, and `not(test)` because this file's
// own tests construct and read every item — under `cargo test` the expectation would be
// fulfilled by nothing and `-D warnings` rejects an unfulfilled expectation. The precedent, and
// the accepted module-wide blind spot, is `analysis.rs`'s (NOTES § D38).
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the watch loop that drives this store is a later box of Phase 5"
    )
)]

#[cfg(test)]
#[path = "k8s_tests.rs"]
mod tests;

use crate::rules::{ClusterSnapshot, NodeSnapshot, ObjectId, PodSnapshot, WorkloadSnapshot};
use futures_util::stream::{BoxStream, Stream, StreamExt, TryStreamExt, select_all};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::runtime::watcher::{self, Event};
use std::collections::BTreeMap;

// --- THE STORE START ---

/// What a watch stream replaces in place: namespace and name.
///
/// **Not the uid.** A name deleted and recreated is one identity to a watch, and the store must
/// hold the latest rather than both. The uid still travels on the snapshot, for the reason
/// [`ObjectId::uid`](crate::rules::ObjectId::uid) gives.
type Key = (Option<String>, String);

/// The one thing this file needs of a snapshot type, so the key is computed in one place.
trait Watched {
    fn id(&self) -> &ObjectId;
}

impl Watched for PodSnapshot {
    fn id(&self) -> &ObjectId {
        &self.id
    }
}

impl Watched for NodeSnapshot {
    fn id(&self) -> &ObjectId {
        &self.id
    }
}

impl Watched for WorkloadSnapshot {
    fn id(&self) -> &ObjectId {
        &self.id
    }
}

fn key(object: &impl Watched) -> Key {
    let id = object.id();
    (id.namespace.clone(), id.name.clone())
}

/// One watch stream's objects, and whether its first LIST has landed.
struct Watch<T> {
    /// The last **complete** answer, and the only thing [`Store::snapshot`] reads.
    live: BTreeMap<Key, T>,
    /// `Some` while a LIST is in flight, filled by `InitApply` and swapped into `live` whole at
    /// `InitDone` — kube's own instruction for the event, and what keeps a relist from being
    /// observable as a cluster that briefly lost its pods (NOTES § D28).
    filling: Option<BTreeMap<Key, T>>,
    /// False until the first `InitDone`, and never false again. A reconnect re-lists into
    /// `filling`, so the last complete answer stays readable while it does: D28 forbids
    /// publishing a **partial** list, and blanking the screen on every watch restart would be a
    /// worse answer to it than a few seconds of stale.
    complete: bool,
}

// Written out rather than derived: `#[derive(Default)]` would demand `T: Default`, which no
// snapshot type implements and none needs to.
impl<T> Default for Watch<T> {
    fn default() -> Self {
        Self {
            live: BTreeMap::new(),
            filling: None,
            complete: false,
        }
    }
}

impl<T: Watched> Watch<T> {
    /// One watch event, applied.
    ///
    /// **`K` is the API object and it does not survive this line**: `T::from` is `rules.rs`'s
    /// decode, which is the prune (see the module doc). A `Delete` is converted too — the whole
    /// object is already in memory, and one conversion means one place the identity is derived.
    fn take<K>(&mut self, event: Event<K>)
    where
        T: From<K>,
    {
        match event {
            Event::Init => self.filling = Some(BTreeMap::new()),
            Event::InitApply(object) => {
                let object = T::from(object);
                self.filling
                    .get_or_insert_default()
                    .insert(key(&object), object);
            }
            // **`filling` being `None` publishes nothing**, rather than publishing an empty
            // cluster. kube sends `Init` before every `InitDone`, so this only fires on a broken
            // stream — and a broken stream that says "your cluster has no nodes" is the failure
            // this gate exists to prevent.
            Event::InitDone => {
                if let Some(listed) = self.filling.take() {
                    self.live = listed;
                    self.complete = true;
                }
            }
            Event::Apply(object) => {
                let object = T::from(object);
                self.live.insert(key(&object), object);
            }
            Event::Delete(object) => {
                self.live.remove(&key(&T::from(object)));
            }
        }
    }
}

/// Everything the permanent watch holds, and the gate that decides whether any of it may be
/// read.
///
/// Five streams and three snapshot types: the three workload kinds decode to the same
/// [`WorkloadSnapshot`] but arrive on their own watches, so each keeps its own map. That is what
/// lets a Deployment and a DaemonSet of the same name coexist, and what lets one of the three
/// relist without touching the other two.
///
/// **No `Debug`.** Nothing here holds a credential today, but the type is the one every API
/// object flows into, and the security gate's rule is mechanical rather than a per-field
/// judgement call.
#[derive(Default)]
pub struct Store {
    pods: Watch<PodSnapshot>,
    nodes: Watch<NodeSnapshot>,
    deployments: Watch<WorkloadSnapshot>,
    stateful_sets: Watch<WorkloadSnapshot>,
    daemon_sets: Watch<WorkloadSnapshot>,
    /// The last failure any watch reported, and **nothing clears it**.
    ///
    /// **Reconnect and backoff are a later box and are not here.** kube's `watcher` retries by
    /// itself on the next poll and calls every one of these errors retryable, so what this turn
    /// owes is that the failure is not swallowed while that box is unwritten — a 403 on one
    /// watch, a dead API server and a resourceVersion the server has forgotten all arrive here.
    ///
    /// **It is not cleared by the next event, and the first draft of it was.** A failure belongs
    /// to the watch that raised it and only that watch can say it is over; with one field and no
    /// per-watch identity, "cleared by the next event" means the four healthy watches erase a
    /// standing 403 on the fifth with their own ordinary traffic, which is swallowing it with
    /// extra steps. Keeping it is the half that is true without the identity the reconnect box
    /// introduces, and that box is what replaces this field rather than reading it forever.
    ///
    /// **Whatever renders it strips it first** (invariant 9): the text is the API server's.
    failure: Option<watcher::Error>,
}

impl Store {
    pub fn pod(&mut self, event: Event<Pod>) {
        self.pods.take(event);
    }

    pub fn node(&mut self, event: Event<Node>) {
        self.nodes.take(event);
    }

    pub fn deployment(&mut self, event: Event<Deployment>) {
        self.deployments.take(event);
    }

    pub fn stateful_set(&mut self, event: Event<StatefulSet>) {
        self.stateful_sets.take(event);
    }

    pub fn daemon_set(&mut self, event: Event<DaemonSet>) {
        self.daemon_sets.take(event);
    }

    /// The last thing that went wrong on any watch, since the loop started.
    pub fn failure(&self) -> Option<&watcher::Error> {
        self.failure.as_ref()
    }

    /// Every initial LIST has landed, so there is something honest to publish (NOTES § D28).
    fn listed(&self) -> bool {
        [
            self.pods.complete,
            self.nodes.complete,
            self.deployments.complete,
            self.stateful_sets.complete,
            self.daemon_sets.complete,
        ]
        .into_iter()
        .all(|complete| complete)
    }

    /// The cluster as the rules read it, or `None` while any watch is still inside its first
    /// LIST.
    ///
    /// **`now` is the caller's**, captured once per pass and carried as a value, so no rule ever
    /// calls a clock (invariant 5, NOTES § D18). This file does not read a clock either: one
    /// snapshot per analysis pass means the caller is the only place that can capture it once.
    ///
    /// The lists are cloned out because [`ClusterSnapshot`] owns its contents and is frozen —
    /// **one deep copy of the watched set per call**, which is the ceiling to watch if a large
    /// cluster ever redraws faster than invariant 7's coalescing. They come out in namespace
    /// then name order, so two calls over one store are the same list twice.
    pub fn snapshot(&self, now: Time) -> Option<ClusterSnapshot> {
        if !self.listed() {
            return None;
        }
        Some(ClusterSnapshot {
            now,
            pods: self.pods.live.values().cloned().collect(),
            nodes: self.nodes.live.values().cloned().collect(),
            workloads: self
                .deployments
                .live
                .values()
                .chain(self.stateful_sets.live.values())
                .chain(self.daemon_sets.live.values())
                .cloned()
                .collect(),
            // Read from the API server once at connect, and from the kubeconfig — neither came
            // from a watch (`docs/architecture.md` § Data flow). The `connect()` box further
            // down Phase 5 fills them; until then N4 and C1 correctly say nothing.
            server_version: None,
            context: None,
            client_certificate: None,
            // `None` is every namespace as far as this store knows. The `--namespace` box is
            // further down Phase 5.
            namespace_scope: None,
            // **`None` is *nobody looked*, and it is not `Some(vec![])`** (NOTES § D129). None of
            // these is watched (invariant 6); `k8s.rs` fetches them when a report's pane opens,
            // which is a later box, and an empty `Vec` here would tell Waste that nothing is
            // going to waste over lists it never read.
            replica_sets: None,
            services: None,
            endpoint_slices: None,
            claims: None,
            disruption_budgets: None,
            certificate_requests: None,
            metrics: None,
        })
    }
}

// --- THE STORE END ---

// --- THE DRIVER START ---

/// One watch event with its kind already decided: the only thing five streams of three
/// different API types have in common, so [`drive`] itself names none of them.
type Update = Box<dyn FnOnce(&mut Store) + Send>;

/// One `watcher()` stream, ready for [`drive`].
///
/// `apply` is the [`Store`] method for the kind the stream carries — `Store::pod` for a Pod
/// watch — and this is the only line in the driver where a kind is named at all.
fn updates<K: Send + 'static>(
    watch: impl Stream<Item = watcher::Result<Event<K>>> + Send + 'static,
    apply: fn(&mut Store, Event<K>),
) -> BoxStream<'static, watcher::Result<Update>> {
    watch
        .map_ok(move |event| Box::new(move |store: &mut Store| apply(store, event)) as Update)
        .boxed()
}

/// **Every watch into one store, until they all end.** A `watcher()` stream is documented not
/// to end — kube says it recovers on the next poll after an `Err` rather than finishing — which
/// is read off its doc and not off a cluster; a stream that *does* end is the ceiling below.
///
/// **An `Err` is not the end of the loop, and that is the whole of this function's error
/// handling.** `?` here, or `try_for_each`, would end the stream on the first failure, which is
/// k9s [#3922](https://github.com/derailed/k9s/issues/3922) exactly: one blip and the tool
/// never reconnects. So the failure is recorded and the next poll is taken. **The `Err` arm
/// reaches no watch at all** — only an `Update` can, and a failure is not one — which is what
/// keeps it from opening the bootstrap gate: a stream that fails part-way through its initial
/// LIST has not listed, and D28 still says nothing may be published.
///
/// **The ceiling, for the reconnect box.** `select_all` drops a stream that finishes and the
/// loop runs on with the rest, so a watch that ended would leave its kind frozen at whatever it
/// last held, presented as live with nothing saying so. Noticing that, backing off, and putting
/// either state on screen are that box's; what is decided here is only that a failure does not
/// take the other four watches down with it.
async fn drive(watches: Vec<BoxStream<'static, watcher::Result<Update>>>, store: &mut Store) {
    let mut merged = select_all(watches);
    while let Some(next) = merged.next().await {
        match next {
            Ok(update) => update(store),
            Err(failure) => store.failure = Some(failure),
        }
    }
}

// --- THE DRIVER END ---
