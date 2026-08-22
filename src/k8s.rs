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
//! **Every string a snapshot type carries is cleaned and bounded on the way in** — invariant 9's
//! strip and the security gate's size bound, paid once in [`ingest`] so nothing downstream has to
//! remember. A control character that is whitespace becomes one space and every other one is
//! removed (NOTES § D146). What the bound buys is the resident set again, and not latency: the
//! 50 MB field still arrives and is still deserialized before it is cut.
//!
//! **The initial LIST arrives in pages of 500 and kube follows the `continue` tokens itself**
//! (NOTES § D147). Pages are invisible to the gate above — one `Init`, one `InitApply` per
//! object across every page, one `InitDone` — and the page size is a number this repo chose
//! rather than one inherited silently, written down at [`INITIAL_LIST_PAGE`].
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

use crate::rules::{
    ClusterSnapshot, Condition, ContainerSnapshot, ContainerState, ExitRule, HostPathMount,
    NodeSnapshot, ObjectId, ObjectKind, PodSnapshot, Taint, Terminated, Toleration,
    WorkloadSnapshot,
};
use futures_util::stream::{BoxStream, Stream, StreamExt, TryStreamExt, select_all};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use kube::runtime::watcher::{self, Event};
use std::collections::BTreeMap;

// --- THE INGEST GUARD START ---
//
// **Every string a snapshot type carries is cleaned and bounded here, and nowhere else.**
// Invariant 9 owes a strip to every printer and the security gate owes a bound to every field;
// both are paid once, on the way into the decode, so no downstream consumer has to remember.
// `main.rs`'s `sanitize` is the same rule one layer up and is superseded by this (NOTES § D122);
// it stays until `dev-ui` removes it at Phase 12.
//
// **The one string this file keeps that does not come through here is [`Store::failure`]**,
// which is kube's `watcher::Error` and not a `String` this file owns. Its own doc carries the
// instruction, and the reconnect box that replaces the field is where it stops being an
// instruction.
//
// **The field list is not repeated as a list.** It is one `Bounded` impl per snapshot type, and
// `rules.rs`'s own struct definitions are what says a field exists — the same reason the prune
// is the decode (see the module doc). `k8s_tests.rs` derives the list from `rules.rs` and
// refuses an impl that does not name every `String` a watched type carries, so *every* is
// mechanical rather than a claim.
//
// **Two classes, and the rule for a new field is what the value is drawn as**: a word the
// reader scans is an [`IDENTIFIER`], a sentence or a path is [`FREE_TEXT`]. Both numbers, the
// census they were chosen against, the marker, and the two ceilings this guard does not close —
// collection *lengths*, and [`Store::failure`] — are NOTES § D146.

/// The longest **identifier** kept from one field: 512 bytes (NOTES § D146).
///
/// A value that names or codes something and is read as a word: names, namespaces, uids, an
/// `ownerReference`'s kind, container names, images, label and selector keys and values, taints,
/// tolerations, finalizers, claims, phase, a condition's type and status, a `reason` the rules
/// compare with `==`, quantities, the kubelet version, a restart-rule action.
const IDENTIFIER: usize = 512;

/// The longest **sentence or path** kept from one field: 4096 bytes (NOTES § D146).
///
/// Waiting, termination and condition **messages**, which rules 3, 4 and 10 put on the card
/// verbatim (NOTES § D37) — and `hostPath` with its two subpaths, which Posture prints as a
/// row's own subject. The two classes cannot be one number: the longest message in the committed
/// captures is 362 bytes, already 71% of 512, while every identifier beside it is under 50.
const FREE_TEXT: usize = 4096;

/// What a cut looks like to the reader, in plain language and attributed to us (invariant 14,
/// NOTES § D146).
///
/// `screens/widgets.md` § 7 forbids a *silent* cut and a *byte* cut, not every cut. This is
/// neither.
const SHORTENED: &str = "… (shortened by k8rs)";

/// One untrusted string, made safe to hold and safe to print.
///
/// **A control character that is whitespace becomes one space; every other one is removed**
/// (NOTES § D146). `char::is_whitespace` is what decides which — `HT`, `LF`, `VT`, `FF`, `CR`
/// and `NEL`, and nothing else in the control range — so the split is the standard library's
/// and not a list kept here. A boundary deleted glues two words into one; `ESC`, `NUL`, `DEL`
/// and the rest of the C1 range have no readable equivalent and are what invariant 9 exists for.
///
/// **The space is added only between two characters that were kept, and only where there is not
/// one already**, which settles the three end cases in one condition: a run of breaks however it
/// is spelled is one boundary, a leading or trailing break separated the value from nothing and
/// is dropped, and a break beside an ordinary space adds nothing. **No character that prints as
/// itself is ever changed or removed** — a space the cluster sent stays, and two spaces it sent
/// stay two.
///
/// Strip first, then bound, so the bound counts the bytes that are actually kept — a megabyte of
/// escape sequences, or of newlines, leaves nothing behind and is not reported as shortened,
/// because nothing that could be shown was lost.
///
/// **The cut steps back to a character boundary**, which `String::truncate` does not: it panics
/// on a byte in the middle of a multi-byte character, and a crafted pod name is exactly how
/// somebody would find that. A UTF-8 character is at most four bytes, so the boundary is at most
/// three below the cap; the walk is a bounded range rather than a `while`, so no input and no
/// edit of this function can turn it into a hang. Byte 0 is always a boundary, which is what
/// makes the fallback safe rather than a second panic.
fn text(value: &mut String, cap: usize) {
    let mut kept = String::new();
    let mut break_pending = false;
    for character in value.chars() {
        if !character.is_control() {
            if break_pending && !kept.is_empty() && !kept.ends_with(' ') {
                kept.push(' ');
            }
            break_pending = false;
            kept.push(character);
        } else if character.is_whitespace() {
            break_pending = true;
        }
    }
    *value = kept;
    if value.len() <= cap {
        return;
    }
    let cut = (cap.saturating_sub(3)..=cap)
        .rev()
        .find(|&index| value.is_char_boundary(index))
        .unwrap_or(0);
    value.truncate(cut);
    value.push_str(SHORTENED);
}

/// [`text`], for a field the API may omit.
fn maybe(value: &mut Option<String>, cap: usize) {
    if let Some(value) = value {
        text(value, cap);
    }
}

/// [`text`] over both halves of a label-style map.
///
/// The map is rebuilt because a `BTreeMap` key cannot be edited in place. **Two keys that cut to
/// the same bytes become one entry, and the first in key order keeps its value** — the one place
/// this guard loses something instead of shortening it (NOTES § D146).
fn pairs(map: &mut BTreeMap<String, String>, cap: usize) {
    let mut bounded = BTreeMap::new();
    for (mut key, mut value) in std::mem::take(map) {
        text(&mut key, cap);
        text(&mut value, cap);
        bounded.entry(key).or_insert(value);
    }
    *map = bounded;
}

/// A snapshot type that has been through the guard above.
trait Bounded {
    fn bound(&mut self);
}

impl<T: Bounded> Bounded for Option<T> {
    fn bound(&mut self) {
        if let Some(value) = self {
            value.bound();
        }
    }
}

impl<T: Bounded> Bounded for Vec<T> {
    fn bound(&mut self) {
        for item in self {
            item.bound();
        }
    }
}

impl Bounded for ObjectId {
    fn bound(&mut self) {
        // `ownerReferences[].kind` and `.apiVersion` are unvalidated free text and the `Other`
        // arm carries both into a string that reaches a card — `rules.rs`'s
        // `ObjectKind::from_api` says so and names this guard.
        if let ObjectKind::Other(kind) = &mut self.kind {
            text(kind, IDENTIFIER);
        }
        maybe(&mut self.namespace, IDENTIFIER);
        text(&mut self.name, IDENTIFIER);
        maybe(&mut self.uid, IDENTIFIER);
    }
}

impl Bounded for Condition {
    fn bound(&mut self) {
        text(&mut self.type_, IDENTIFIER);
        text(&mut self.status, IDENTIFIER);
        maybe(&mut self.reason, IDENTIFIER);
        maybe(&mut self.message, FREE_TEXT);
    }
}

impl Bounded for Terminated {
    fn bound(&mut self) {
        maybe(&mut self.reason, IDENTIFIER);
        maybe(&mut self.message, FREE_TEXT);
    }
}

impl Bounded for ContainerState {
    fn bound(&mut self) {
        match self {
            Self::Waiting { reason, message } => {
                maybe(reason, IDENTIFIER);
                maybe(message, FREE_TEXT);
            }
            Self::Running { .. } => {}
            Self::Terminated(ended) => ended.bound(),
        }
    }
}

impl Bounded for ExitRule {
    fn bound(&mut self) {
        text(&mut self.action, IDENTIFIER);
        maybe(&mut self.operator, IDENTIFIER);
    }
}

impl Bounded for ContainerSnapshot {
    fn bound(&mut self) {
        text(&mut self.name, IDENTIFIER);
        text(&mut self.image, IDENTIFIER);
        maybe(&mut self.restart_policy, IDENTIFIER);
        self.restart_rules.bound();
        self.state.bound();
        self.last_terminated.bound();
        maybe(&mut self.cpu_request, IDENTIFIER);
        maybe(&mut self.memory_request, IDENTIFIER);
        maybe(&mut self.memory_limit, IDENTIFIER);
        maybe(&mut self.cpu_limit, IDENTIFIER);
        maybe(&mut self.allocated_cpu, IDENTIFIER);
        maybe(&mut self.allocated_memory, IDENTIFIER);
    }
}

impl Bounded for HostPathMount {
    fn bound(&mut self) {
        text(&mut self.path, FREE_TEXT);
        maybe(&mut self.sub_path, FREE_TEXT);
        maybe(&mut self.sub_path_expr, FREE_TEXT);
        text(&mut self.container, IDENTIFIER);
    }
}

impl Bounded for Toleration {
    fn bound(&mut self) {
        maybe(&mut self.key, IDENTIFIER);
        maybe(&mut self.operator, IDENTIFIER);
        maybe(&mut self.value, IDENTIFIER);
        maybe(&mut self.effect, IDENTIFIER);
    }
}

impl Bounded for Taint {
    fn bound(&mut self) {
        text(&mut self.key, IDENTIFIER);
        maybe(&mut self.value, IDENTIFIER);
        text(&mut self.effect, IDENTIFIER);
    }
}

impl Bounded for PodSnapshot {
    fn bound(&mut self) {
        self.id.bound();
        self.owner.bound();
        maybe(&mut self.node, IDENTIFIER);
        maybe(&mut self.phase, IDENTIFIER);
        self.containers.bound();
        maybe(&mut self.cpu_request, IDENTIFIER);
        maybe(&mut self.memory_request, IDENTIFIER);
        self.scheduled.bound();
        maybe(&mut self.nominated_node_name, IDENTIFIER);
        self.ready.bound();
        self.ready_to_start_containers.bound();
        for finalizer in &mut self.finalizers {
            text(finalizer, IDENTIFIER);
        }
        self.host_path_mounts.bound();
        pairs(&mut self.node_selector, IDENTIFIER);
        self.tolerations.bound();
        pairs(&mut self.labels, IDENTIFIER);
        maybe(&mut self.cpu_limit, IDENTIFIER);
        maybe(&mut self.memory_limit, IDENTIFIER);
        maybe(&mut self.overhead_cpu, IDENTIFIER);
        maybe(&mut self.overhead_memory, IDENTIFIER);
        for claim in &mut self.claims {
            text(claim, IDENTIFIER);
        }
    }
}

impl Bounded for NodeSnapshot {
    fn bound(&mut self) {
        self.id.bound();
        self.conditions.bound();
        self.taints.bound();
        pairs(&mut self.labels, IDENTIFIER);
        maybe(&mut self.kubelet_version, IDENTIFIER);
        maybe(&mut self.allocatable_cpu, IDENTIFIER);
        maybe(&mut self.allocatable_memory, IDENTIFIER);
    }
}

impl Bounded for WorkloadSnapshot {
    fn bound(&mut self) {
        self.id.bound();
        self.owner.bound();
        self.conditions.bound();
    }
}

/// **The whole of what happens to an API object on its way into the store**: decode, which is
/// the prune, then strip and bound.
///
/// One function so there is one answer, and [`Watch::take`] is the only caller — a second entry
/// point into the store is a second place to forget the guard.
fn ingest<K, T: From<K> + Bounded>(object: K) -> T {
    let mut object = T::from(object);
    object.bound();
    object
}

// --- THE INGEST GUARD END ---

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
    /// **`K` is the API object and it does not survive this line**: [`ingest`] is `rules.rs`'s
    /// decode — which is the prune (see the module doc) — followed by the strip and the bound. A
    /// `Delete` goes through it too: the whole object is already in memory, one conversion means
    /// one place the identity is derived, and the key a delete looks up has to be built the same
    /// way the key it stored was.
    fn take<K>(&mut self, event: Event<K>)
    where
        T: From<K> + Bounded,
    {
        match event {
            Event::Init => self.filling = Some(BTreeMap::new()),
            Event::InitApply(object) => {
                let object = ingest(object);
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
                let object = ingest(object);
                self.live.insert(key(&object), object);
            }
            Event::Delete(object) => {
                let object: T = ingest(object);
                self.live.remove(&key(&object));
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
            // down Phase 5 fills them; until then N4 and C1 correctly say nothing. **They do not
            // pass through [`ingest`]**, so `server_version` — the API server's own text — owes
            // [`text`] at the point that box sets it.
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

// --- THE INITIAL LIST START ---
//
// **The initial LIST is already paged, and what is left to decide is the number** (NOTES § D147).
//
// **What kube 4.2.0 does by default, read off its source rather than recalled.**
// `kube-runtime-4.2.0/src/watcher.rs:276` sets `page_size: Some(500)` in `Config::default()` —
// "same default page size limit as client-go", `:404` carries it into the `ListParams` that
// becomes the query string's `limit` (`kube-core-4.2.0/src/params.rs:102`), and
// `State::InitPage` sends the server's own `continue` token back on the next request (`:562`,
// `params.rs:105`) — one page per round trip, until a page comes back without one. So the
// unpaginated `LIST pods -A` that `PRIOR-ART § A2` is about cannot happen through this client:
// there is no pagination to write here, only a number to choose and a gate to prove against it.
//
// **Paging is invisible to [`Store`], and that is what the gate above depends on.** One `Init`
// (`:523`), one `InitApply` per object across every page (`:548`), and one `InitDone` emitted
// only once a page has drained and carried no `continue` token (`:555-559`) — so several HTTP
// responses are one LIST, `filling` accumulates across all of them, and `live` is swapped once.
//
// **A page that fails abandons the pages before it**: `:584` reports `InitialListFailed` and
// resets the machine to `Empty`, whose next poll emits a fresh `Init`. That is the ordinary path
// for a `continue` token the API server has already compacted, not a rare one, and it is what
// makes `Event::Init` clearing `filling` load-bearing rather than defensive.
//
// **What source-reading cannot answer is what a page costs.** That is a round trip against a
// real API server, and this file has never met one. The arithmetic below is the half that can
// be decided without a cluster; where the paint budget stops holding is the other half.

/// Objects per page of an initial LIST: **500** — the number kube also defaults to, chosen here
/// rather than inherited, and sent by the `connect()` box below this one (NOTES § D147).
///
/// **The binding constraint is memory, not round trips.** kube buffers a whole page of decoded
/// objects before it emits the first `InitApply` (`watcher.rs:574`), so one page per watch sits
/// on top of the store for as long as that page takes to drain. Measured over the 55 pod objects
/// in the committed captures, the median is 3708 bytes of JSON and the largest 5662 — **with
/// `managedFields` already stripped by the sanitizer**, so a live object is larger by an amount
/// only a cluster can say. A 500-object page is therefore ~1.9 MB at the median and a 5000-object
/// one ~19 MB, against the `< 50MB RSS at ~1000 pods` (`REQUIREMENTS.md`) that the store itself
/// also has to fit inside.
///
/// **And at the size the budget is stated at, a larger page buys almost nothing.** ~1000 pods is
/// two round trips at 500 and one at 1000, and the one it saves is bought by doubling the
/// response the API server has to build whole — the k9s failure `PRIOR-ART § A2` describes. The
/// twenty sequential round trips a 10 000-pod cluster costs at this size are real, and **neither
/// number is a measured crossing point**: NOTES § D115 says in as many words that ~1000 and
/// 10 000 are the sizes the budget and `PRIOR-ART § A2` were *written* at. Which page size is
/// faster, and at what cluster size the paint budget stops holding, are one measurement against a
/// real API server, and nothing in this file has met one.
const INITIAL_LIST_PAGE: u32 = 500;

// **Where the number is applied is the `connect()` box, not this one.** `page_size` reaches
// kube as `watcher::Config::default().page_size(INITIAL_LIST_PAGE)`, and there is no
// `watch_config()` here holding that one line early: measured, such a function is
// *indistinguishable* from `Config::default()` — the two configs compare equal, so no test
// could tell it from the silent inheritance it exists to prevent, and a function no test can
// fail on is what the mutation gate is for (NOTES § D147). What can be proven without a
// `Client` is that kube's default is still the number this reasoning was written against, and
// `k8s_tests.rs` proves exactly that.
//
// Everything else on that config is kube's default on purpose. `ListSemantic::MostRecent` is
// the quorum read; `Any` would serve the list from the API server's watch cache, which is
// cheaper and can be stale, and a first paint that is stale is D28's lie with a different
// cause. `InitialListStrategy::ListWatch` rather than `StreamingList`, which kube's own doc
// (`watcher.rs:243`) says needs a server-side feature gate — which servers have it on is the
// oldest-supported-API-server box, not this one. No selectors: invariant 6 watches every pod,
// node and workload. And **`Config::timeout` is left unset deliberately**: it is one field for
// both calls (`:400`, `:414`), so a timeout short enough to bound the initial LIST would also
// cap the watch and re-LIST the whole cluster on that period.

// --- THE INITIAL LIST END ---

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
