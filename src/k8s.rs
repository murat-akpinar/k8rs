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
//! **Nothing throttles us on this side of the wire, and a wait still has to be drawable**
//! (NOTES § D148). kube-rs ships no client-side rate limiter — tower's is not even compiled in —
//! but it does retry a throttled request fifteen times in silence, and a bootstrap inside that
//! window is indistinguishable from a hang. [`Store::still_listing`] is the state a screen draws
//! it from; § WHAT A THROTTLE LOOKS LIKE is why there is nothing else to draw.
//!
//! **A first sync that never finishes is a state and not a spinner, and there is no deadline in
//! it** (NOTES § D150, `PRIOR-ART § A7`). Each unfinished LIST reports two facts — how many
//! objects it has decoded, and when the last one arrived — because *slow* and *hung* overlap by
//! construction and any threshold that called the twentieth round trip of a 10 000-pod cluster a
//! hang would call a working cluster broken. A LIST that is working moves both numbers; a LIST
//! that is hung moves neither, and [`Listing`] is readable at any instant without one. **Nothing
//! here cancels anything**: the tool does not quit because a cluster is slow.
//!
//! **The oldest API server this build is supported against is Kubernetes 1.29, and a cluster
//! outside that window is told rather than turned away** (NOTES § D149). Nothing k8rs sends is
//! refused by an older server and nothing it reads decodes wrongly on one — both measured, in
//! § HOW OLD A CLUSTER MAY BE — so refusing to start would cost a reader with a broken v1.24
//! cluster everything and buy them nothing. [`version_note`] is the one line they get instead,
//! and it has a second half: a cluster *newer* than [`TYPES_BUILT_FOR`] drops its added fields at
//! decode, which is the failure NOTES § D99 relocated onto the user's machine.
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
    WorkloadSnapshot, minor_version,
};
use futures_util::stream::{BoxStream, Stream, StreamExt, TryStreamExt, select_all};
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, StatefulSet};
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::jiff::Timestamp;
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
    /// **When something last arrived on this watch's initial LIST** — `Init` counts, and so does
    /// every `InitApply` (NOTES § D150). `None` means this watch has produced nothing at all,
    /// which is the state a store is in before the loop's first poll.
    ///
    /// **It is not cleared at `InitDone`** and nothing reads it after one: [`Watch::progress`]
    /// refuses to answer for a complete watch, so a stale stamp from the last bootstrap can
    /// never be presented as a live one. A reconnect's `Init` overwrites it before it could be.
    last_progress: Option<Time>,
}

// Written out rather than derived: `#[derive(Default)]` would demand `T: Default`, which no
// snapshot type implements and none needs to.
impl<T> Default for Watch<T> {
    fn default() -> Self {
        Self {
            live: BTreeMap::new(),
            filling: None,
            complete: false,
            last_progress: None,
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
    fn take<K>(&mut self, now: &Time, event: Event<K>)
    where
        T: From<K> + Bounded,
    {
        // **Stamped on the two events an initial LIST is made of, and on no others**
        // (NOTES § D150). `Apply` and `Delete` are ordinary watch traffic on a watch that has
        // already listed; refreshing the stamp from them would let four healthy watches make a
        // fifth one's hung LIST look alive, which is the shape D148's `failure` field already
        // had to refuse once.
        if matches!(event, Event::Init | Event::InitApply(_)) {
            self.last_progress = Some(now.clone());
        }
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

    /// **What this watch's unfinished initial LIST has to show for itself**, or `None` once it
    /// has finished one (NOTES § D150).
    ///
    /// The count is `filling`'s own length rather than a counter kept beside it: kube buffers a
    /// page and drains it one `InitApply` at a time (NOTES § D147), so the map *is* the tally,
    /// and a second one could drift from it. It is objects **decoded and kept**, which is not
    /// the same as objects the server has sent — a page in flight is invisible here, and no
    /// number in this file can see it.
    fn progress(&self) -> Option<(usize, Option<Time>)> {
        (!self.complete).then(|| {
            (
                self.filling.as_ref().map_or(0, BTreeMap::len),
                self.last_progress.clone(),
            )
        })
    }
}

/// **One initial LIST that has not finished, and what it has to show for itself**
/// (NOTES § D150). `PRIOR-ART § A7` is the failure this exists for: a first sync that never
/// completes has to become a **state**, not a spinner, and k9s
/// [#4044](https://github.com/derailed/k9s/issues/4044) is what a spinner costs — a wait with
/// nothing to see it by while `kubectl get` on the same context returns instantly.
///
/// **There is no deadline here, and that is the decision rather than an omission.** *Slow* and
/// *hung* overlap by construction: `REQUIREMENTS.md` budgets first paint under a second at
/// ~1000 pods, and 10 000 pods is twenty sequential round trips at [`INITIAL_LIST_PAGE`], so
/// any number that called the twentieth trip a hang would call a working cluster broken. So no
/// number is picked. The two facts below are reported instead, and between them they separate
/// the two cases without one: **a LIST that is working produces a count that moves, and a LIST
/// that is hung produces one that does not.**
///
/// **A count alone would not have been enough, and D148 is why.** The watch sockets carry no
/// TCP keepalive, so a connection that dies mid-list stalls with no error and no further
/// events — and a screen that draws on events (invariant 7) never redraws to show the count
/// standing still. [`since`](Listing::since) is what a redraw on a timer can read a duration
/// off; without it the frozen number and a screen that simply is not repainting look identical.
///
/// **Nothing here cancels anything.** The clause this type answers says *becomes a state*, not
/// *gives up*, and nothing in this design may quit because a cluster is slow.
///
/// **The ceiling, named rather than left to be discovered: this makes the state *readable*, and
/// something still has to ask.** Invariant 7 blocks when idle, so a screen that draws only on
/// events never redraws during exactly the silence this type describes — the seconds would
/// advance and nothing would repaint them. A redraw on a timer while a bootstrap is outstanding
/// is `ui.rs`'s to write, and until it exists these two facts are only as fresh as whatever else
/// caused the last draw. **Putting the timer here instead would be worse**: it would make this
/// file own a paint schedule it cannot see the screen of.
///
/// **It derives `Debug` where [`Store`] deliberately does not**, and the difference is not an
/// oversight: a kind, a count and a timestamp are three things that never touched a credential,
/// while `Store` is the type every API object flows into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listing {
    /// Which watch is still listing.
    pub kind: ObjectKind,
    /// **Objects this LIST has decoded and kept so far.** Not what the server has sent: a page
    /// in flight is invisible here, and kube emits nothing for it until the whole page has
    /// landed and started draining (NOTES § D147). So `0` is the ordinary reading for the whole
    /// of the first round trip, and it is also the reading `PRIOR-ART § A7`'s hang never leaves.
    pub so_far: usize,
    /// **When the last thing arrived on this LIST** — the `Init` that opens it counts, so a
    /// watch that has begun always has one.
    ///
    /// **A `Time` and not a duration**, because this file does not read a clock to answer a
    /// question (NOTES § D144): the caller holds `now` and turns this into *how long* with
    /// [`crate::rules::age`], which is already the plain-language renderer for exactly that and
    /// is not copied here.
    ///
    /// **`None` is *this watch has produced nothing at all*** — the state a store is in before
    /// the loop's first poll, and the one it would stay in if a watch stream never yielded.
    pub since: Option<Time>,
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
    pub fn pod(&mut self, now: &Time, event: Event<Pod>) {
        self.pods.take(now, event);
    }

    pub fn node(&mut self, now: &Time, event: Event<Node>) {
        self.nodes.take(now, event);
    }

    pub fn deployment(&mut self, now: &Time, event: Event<Deployment>) {
        self.deployments.take(now, event);
    }

    pub fn stateful_set(&mut self, now: &Time, event: Event<StatefulSet>) {
        self.stateful_sets.take(now, event);
    }

    pub fn daemon_set(&mut self, now: &Time, event: Event<DaemonSet>) {
        self.daemon_sets.take(now, event);
    }

    /// The last thing that went wrong on any watch, since the loop started.
    pub fn failure(&self) -> Option<&watcher::Error> {
        self.failure.as_ref()
    }

    /// The initial LISTs that have not landed, in the order the watches are declared — each
    /// with what it has to show for itself.
    ///
    /// **The only thing that says anything more about a bootstrap than that one is running**
    /// (NOTES § D148, § D150). [`Store::snapshot`] answers `None` for every reason at once — D28
    /// forbids publishing a partial list — so on its own a screen can say *waiting* and stop
    /// there, which is the shape `PRIOR-ART § A3` warns about: a wait with nothing to see it by.
    ///
    /// **Empty means every LIST landed**, and [`Store::listed`] is derived from this same call
    /// rather than from the five flags a second time, so the gate and the state a screen draws
    /// the wait from cannot disagree. Empty is also **free**: a `Vec` that collects nothing
    /// allocates nothing, which is what keeps the gate cheap on a path [`Store::snapshot`] takes
    /// on every analysis pass.
    ///
    /// **It cannot say *why*, and no field here could.** A LIST inside kube's silent retry
    /// window (§ WHAT A THROTTLE LOOKS LIKE), a LIST against an enormous cluster and a socket
    /// that died with no keepalive (NOTES § D148) are one answer here; the wait happens below
    /// `watcher()` in a tower layer with no callback. What separates them is the *shape over
    /// time* of the two numbers, which is the caller's to watch and this call's to report.
    ///
    /// **Pods is the kind whose size is reasoned about** — [`INITIAL_LIST_PAGE`] is derived from
    /// it — so *still reading: pods, 4 500 so far* names the long list rather than leaving a hang
    /// to be guessed at. **How the other four behave at size is not measured** and no claim about
    /// it is made here.
    ///
    /// **The words are the caller's.** This returns facts, not sentences: invariant 14's plain
    /// language is the screen's decision and `views.rs` is not this file's.
    pub fn still_listing(&self) -> Vec<Listing> {
        [
            (ObjectKind::Pod, self.pods.progress()),
            (ObjectKind::Node, self.nodes.progress()),
            (ObjectKind::Deployment, self.deployments.progress()),
            (ObjectKind::StatefulSet, self.stateful_sets.progress()),
            (ObjectKind::DaemonSet, self.daemon_sets.progress()),
        ]
        .into_iter()
        .filter_map(|(kind, progress)| {
            let (so_far, since) = progress?;
            Some(Listing {
                kind,
                so_far,
                since,
            })
        })
        .collect()
    }

    /// Every initial LIST has landed, so there is something honest to publish (NOTES § D28).
    ///
    /// Derived from [`Store::still_listing`] rather than from the five fields a second time: the
    /// gate and the state the screen draws the wait from cannot disagree if only one of them
    /// reads the flags.
    fn listed(&self) -> bool {
        self.still_listing().is_empty()
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

// --- WHAT A THROTTLE LOOKS LIKE START ---
//
// **kube-rs does not rate-limit us, and the thing it does instead is harder to see than a queue
// would have been** (NOTES § D148). Read off the crates on disk, not recalled.
//
// **There is no client-side limiter, in the mechanical form of that claim.** client-go's is
// `rest.Config{QPS, Burst}` and `PRIOR-ART § A3` is the k9s thread about what it costs. tower's
// equivalent is `tower::limit::rate` — and `tower-0.5.3/src/lib.rs:175` gates the whole `limit`
// module behind a Cargo feature, which `kube-client-4.2.0/Cargo.toml:296-303` does not enable
// (`buffer`, `filter`, `util`, `retry`, and nothing else; `cargo tree -e features -i tower` over
// this repo lists no `limit` either). **So the module is not compiled into this binary at all** —
// there is nothing a grep could have missed. The three kube trees contain no other limiter: the
// only `Burst` in them is a pod QoS class in two test fixtures
// (`kube-runtime-4.2.0/src/wait.rs:508`, `:541`),
// and `Cargo.lock` carries no `governor`, `leaky-bucket` or `ratelimit`. **k8rs queues no request
// inside itself**, so A3's *make the queue visible* half has nothing to show.
//
// **What kube has instead is a retry layer that absorbs a 429 in silence, fifteen times.**
// `Config::default_retry` is `true` in all three constructors — `new` (`config/mod.rs:199`),
// `incluster` (`:284`) and the `new_from_loader` that every kubeconfig path ends at (`:347`) —
// and `client/builder.rs:251` turns that into `RetryLayer::new(RetryPolicy::server_retry())`.
// `server_retry()` is 5 ms → 1000 s exponential, **15 retries**, server-aware
// (`client/retry.rs:108-110`); it retries 429, 503 and 504 (`:114-119`) and nothing else — a
// transport error returns `None` and is not retried (`:164`). It takes the *longer* of its own
// backoff and the server's `Retry-After` (`:151-161`), which it parses only as `u64` seconds
// (`:154`), so the HTTP-date form of that header is silently ignored. **Our calls qualify**: a
// LIST or a WATCH is a GET, `Body::empty()` is `Kind::Once(None)` (`client/body.rs:37-39`), and
// `try_clone` — which `clone_request` needs (`retry.rs:170`) — returns `Some` for `Kind::Once`
// (`:60-67`).
//
// **How long that silence lasts is arithmetic over those constants, not a measurement.** The
// bases are `5 ms × 2^i` for i = 0..14, capped at 1000 s and never reaching it (5 ms × 2^14 =
// 81.92 s), and tower adds `uniform(0, base × 2.0)` on top (`retry.rs:87`,
// `tower-0.5.3/src/retry/backoff.rs:152-167`, `:176-183`), so each wait is `base .. 3 × base`.
// The fifteen sum to **164 s** at the floor and **~491 s** at the jitter ceiling: a persistently
// throttling API server keeps k8rs quiet for **between about two and a half and about eight
// minutes** before the first error is ours at all — longer still if `Retry-After` beats the
// backoff at every step.
//
// **And nothing can see it.** The wait is a `tokio::time::sleep` inside the tower stack, below
// `watcher()`, with no callback and no counter; its only trace is a `tracing` debug span
// (`builder.rs:254`) and invariant 10 gives us no subscriber. So during a throttle
// [`Store::snapshot`] is `None`, [`Store::failure`] is `None`, and the only honest thing on this
// store is [`Store::still_listing`]. **That is A3 one layer lower**: not a queue whose depth
// could be drawn, just a wait.
//
// **The retry is kept on, deliberately.** `default_retry: false` would put every 429 straight
// onto [`Store::failure`] where a screen could name it — but kube's bare `watcher()` restarts
// "normally immediately" and its backoff is opt-in (`watcher.rs:778-779`), so k8rs would then
// hammer a server that has just said *stop*. Being polite to the cluster is the right default;
// the price is the silence, and the answer to silence is a state on screen, not a knob. Turning
// it off, or adding a tower layer that counts retries in flight, both need a `Client` — so both
// belong to `connect()` and the reconnect box, not here.
//
// **Once a throttle does become ours it is fully distinguishable, and that is the half worth
// guarding.** Any 4xx/5xx becomes `Error::Api(Box<Status>)` — parsed from the body, or rebuilt as
// `Status::failure(text, "Failed to parse error data").with_code(…)` when the body is not a
// `Status` at all (`client/mod.rs:544-558`). **So `Status::code` survives both branches and
// `reason` only the first**, which is why anything downstream should key on the number. `Status`
// carries `code`, `reason`, `message` and `details.retry_after_seconds`
// (`kube-core-4.2.0/src/response.rs:34`, `:50`, `:39`, `:199-200`), and it reaches this file
// inside `watcher::Error::{InitialListFailed, WatchStartFailed, WatchFailed}` (`watcher.rs:31`,
// `:35`, `:43`) — typed, and kept typed by [`Store::failure`] (NOTES § D145).
//
// **The other way this path looks hung, and it is worse: a dead connection is never noticed.**
// `Config` sets `connect_timeout` 30 s and `write_timeout` 295 s (`config/mod.rs:418-419`) and
// leaves **`read_timeout` unset** (`:191`, `:273`, `:339`); the connector is a bare
// `HttpConnector::new()` (`client/builder.rs:117`) whose `TcpKeepaliveConfig::default()` is
// all-`None`, so `into_tcpkeepalive()` yields `None` and `set_tcp_keepalive` is never called
// (`hyper-util-0.1.20/src/client/legacy/connect/http.rs:94-98`, `:104-110`, `:842-843`).
// **SO_KEEPALIVE is off on the watch sockets.** A connection that dies without a FIN or an RST —
// a laptop suspending, a NAT entry expiring, a load balancer dropping an idle flow — raises no
// error and hits no deadline: [`drive`] simply blocks, [`Store::failure`] stays `None`, and the
// store keeps answering with the cluster as it last was. **It is not fixable from here.**
// `read_timeout` is client-wide, a healthy watch is idle for long stretches, and kube's own
// params doc says clients "should not assume bookmarks are returned at any specific interval"
// (`kube-core-4.2.0/src/params.rs:329`) — so there is no period a read deadline could safely
// use. That is the *deadline on the first watch sync* box, next in this phase, and the reconnect
// box after it.
//
// **What source-reading cannot settle**, stated because the box asked for a number: whether any
// real API server ever throttles a five-watch client at all, what its Priority-and-Fairness
// `Retry-After` actually says, and whether its 429 body carries `retry_after_seconds` as well as
// the header. Those are one cluster measurement, and nothing in this file has met one.

// --- WHAT A THROTTLE LOOKS LIKE END ---

// --- HOW OLD A CLUSTER MAY BE START ---
//
// **The floor is Kubernetes 1.29, and it is not the three-minor support window** (NOTES § D149).
// Upstream keeps branches for the last three minors, which is a fact about the Kubernetes project
// and not about this tool. The number below came out of two questions asked against the objects.
//
// **Question one: what does an older server *omit*, and is omission safe?** NOTES § D99 says an
// absent optional field decodes to `None` and invariant 5 already makes that *no finding*. Six
// fields the snapshot types name arrived inside the window, each behind a gate whose graduation
// is one file in `kubernetes/website` under
// `content/en/docs/reference/command-line-tools-reference/feature-gates/<Gate>.md`, read on
// 2026-08-22 rather than recalled:
//
// | field | gate | alpha | beta, default on |
// |---|---|---|---|
// | `spec.containers[].restartPolicyRules` | `ContainerRestartRules` | 1.34 | 1.35 |
// | `status.terminatingReplicas` | `DeploymentReplicaSetTerminatingReplicas` | 1.33 | 1.35 |
// | `spec.resources` (pod-level) | `PodLevelResources` | 1.32 | 1.34 |
// | `status.containerStatuses[].allocatedResources` | `InPlacePodVerticalScaling` | 1.27 | 1.33 |
// | `initContainers[].restartPolicy: Always` | `SidecarContainers` | 1.28 | 1.29 (locked 1.33) |
// | the `PodReadyToStartContainers` condition | `PodReadyToStartContainersCondition` | 1.28 | 1.29 |
//
// Five of the six are self-explaining: a cluster that cannot run a native sidecar has no sidecar
// to describe, so the absence *is* the answer, and every rule already reads it that way.
// **The sixth is the floor.** Rule 13 — `rules.rs`'s `placed_but_never_started` — reads the
// condition through `is_some_and(|c| c.status == "False")`, so an *absent* condition falls into
// the `else` and the card **states a fact**: *"this pod has its storage and its network, so the
// block is later"*. On a server that never set the condition nothing said that, and the comment
// above that branch names the reader it misdirects — the one whose ConfigMap is missing, sent to
// look at the CNI.
//
// **Where the condition starts existing is measured off the API types and not off the gate
// table**, because the two disagree: the gate is listed as alpha at 1.28, but
// `staging/src/k8s.io/api/core/v1/types.go` carries no `PodReadyToStartContainers` constant on
// `release-1.28`, and no `PodHasNetwork` one on `release-1.25` … `release-1.27` either — the
// old name was a kubelet-internal constant. It appears in the public `PodConditionType` block
// for the first time on **`release-1.29`** (`types.go:3005`). So 1.29 is the oldest minor on
// which every sentence k8rs prints is backed by something the server actually said, and that is
// the whole derivation.
//
// **D99's two exceptions were checked against this surface and both are empty.** A *required*
// field does not become `None`, it becomes `Default` — a value, not an absence — so the 64
// non-`Option` fields in the closure these five watches decode were diffed against
// `api/openapi-spec/swagger.json` on `kubernetes/kubernetes` branches `release-1.19`,
// `release-1.24`, `release-1.32` and `release-1.36`: **none of the 64 is absent from any of
// them.** Three whole *types* are (`ContainerRestartRule` and `ContainerRestartRuleOnExitCodes`
// from all three older specs, `ResourceClaim` from 1.19 and 1.24), and each is reached only
// through an `Option`, so an old server sends no such object for the trap to fire on. The one
// row that looks like a hit is not one: six kinds list `metadata` as optional in their schema
// while the generated type has it non-`Option` — and **`release-1.36` says exactly the same**, so
// it is a constant of the spec rather than anything that moved.
//
// The other exception is a group/version move answering **404 rather than `None`**. It cannot
// reach these five: `Pod` and `Node` are `core/v1` and the three workloads are `apps/v1`, and
// both groups carry all five in `release-1.19`'s spec, the oldest checked here.
//
// **Question two: is anything we *send* refused?** That half omission does not cover, and it is
// the one that hangs rather than degrades. Read off the vendored crate, not recalled: the initial
// LIST sends `limit` (`kube-core-4.2.0/src/params.rs:102`) and then `continue` (`:105`) — gate
// `APIListChunking`, beta and on by default at **1.9**, stable at 1.29 — and the watch sends
// `watch=true` (`:378`), `timeoutSeconds=290` (`:381`) and `allowWatchBookmarks=true` (`:390`) —
// gate `WatchBookmark`, stable at **1.17**, and in the API reference's own words a client
// "shouldn't assume bookmarks are returned at any specific interval, nor can clients assume that
// the API server will send any `BOOKMARK` event even when requested". Nothing else:
// `ListSemantic::MostRecent` sets neither `resourceVersion` nor `resourceVersionMatch`
// (`kube-runtime-4.2.0/src/watcher.rs:395`), and there are no selectors.
//
// **`sendInitialEvents` is the parameter that would set a real floor, and this design never sends
// it.** `to_watch_params` sets it only under `InitialListStrategy::StreamingList`
// (`watcher.rs:416-417`, and `params.rs:392-394` is where it would reach the wire), which
// § THE INITIAL LIST keeps off. That closes the question D147
// deferred to this box, and it closes it twice over, because the parameter has **two** bad
// answers rather than one: a server that predates it *ignores* it and the promised `BOOKMARK`
// never comes — k9s's [#4044](https://github.com/derailed/k9s/issues/4044), a spinner with no
// error — and a server that knows it with the gate off **rejects the watch with 403**
// (KEP-3157, `keps/sig-api-machinery/3157-watch-list/README.md`). The gate is not a straight
// line either: `WatchList` was alpha 1.27-1.31, beta-**on** at 1.32, beta-**off** again at 1.33,
// and beta-on from 1.34 (the website's own `WatchList.md`, which the KEP's graduation table
// contradicts by claiming GA at 1.34 — the disagreement is why this cites the rendered one).
// Switching `streaming_lists()` on for speed therefore buys a 403 on 1.33 and a hang below 1.27.
//
// **So k8rs runs on a cluster below the floor rather than refusing it**, because refusing is only
// better than reporting when something is unsafe, and question two found nothing an old server
// rejects. A tool that will not start tells a reader with a broken v1.24 cluster nothing at all
// about their broken v1.24 cluster; a tool that starts and says what it cannot see tells them
// both. What it must not do is stay silent, which is what this build does today.

/// The oldest API server this build is supported against: **Kubernetes 1.29**, the oldest minor
/// on which every sentence k8rs prints is backed by something the server said (NOTES § D149, and
/// the region comment above for the derivation).
const OLDEST_SERVER: u32 = 29;

/// The Kubernetes minor these types were generated from — `k8s-openapi`'s `v1_36` feature in
/// `Cargo.toml`, the **newest** the crate offers (NOTES § D99).
///
/// **A server above this drops its added fields at decode**, silently and exactly as the old pin
/// dropped 1.36's. That is the failure D99 calls unaffordable and relocates onto the user's
/// machine rather than eliminating, and [`version_note`] is where it stops being silent.
const TYPES_BUILT_FOR: u32 = 36;

/// **What to tell the user about this cluster's version, or `None` when there is nothing to say**
/// (NOTES § D149). One line, in plain language, for someone who has never heard the phrase *minor
/// version* (invariant 14).
///
/// **It takes a string and not a `Client`** so that the shapes a real server returns can be fed
/// to it without one: `v1.31.2` from kind, `v1.29.4+k3s1` from k3s, `v1.28.3-gke.1286000` from
/// GKE and `v1.30.0-rc.2` from a pre-release all parse through [`minor_version`], which is
/// `rules.rs`'s and is not copied here — N4 and the Versions report already read the same string.
///
/// **A version it cannot parse says nothing**, which is N4's habit rather than a new one: a
/// warning derived from a guess is worse than no warning, and `apiserver_version` is free text
/// from the API like any other.
///
/// **Nothing the server sent is ever echoed back.** The message is built from the two integers
/// [`minor_version`] parsed out, so a `gitVersion` carrying control characters cannot reach a
/// terminal through this line (invariant 9). The bound is structural, not a filter.
///
/// **Neither answer refuses to run**, and the too-old one names the *shape* of what goes wrong
/// rather than the one card it is about. That card is a defect in a frozen file, reported by this
/// box and fixed by a later one; a user-facing string that enumerates today's bugs is the second
/// copy that goes stale, and this one would go stale the day the defect is fixed.
pub(crate) fn version_note(server_version: &str) -> Option<String> {
    // Ordered rather than differenced. N4 refuses to compare across majors because a *distance*
    // read over a major boundary is not a distance; an *order* is well defined, and a major
    // nobody has seen is more outside this window than an old minor is, not less.
    let version = minor_version(server_version)?;
    if version < (1, OLDEST_SERVER) {
        let (major, minor) = version;
        return Some(format!(
            "This cluster is Kubernetes {major}.{minor}, and k8rs has only been checked against \
             1.{OLDEST_SERVER} and newer. It will still run — nothing it does here is unsafe — \
             but a cluster this old does not publish everything the checks read, so some of them \
             will stay quiet here, and a few can say more than this cluster actually told them."
        ));
    }
    if version > (1, TYPES_BUILT_FOR) {
        let (major, minor) = version;
        return Some(format!(
            "This cluster is Kubernetes {major}.{minor}, and this copy of k8rs was built to \
             understand 1.{TYPES_BUILT_FOR}. It will still run, but anything Kubernetes added \
             after 1.{TYPES_BUILT_FOR} is invisible to it — including, sometimes, the reason \
             something is broken. A newer k8rs is the fix."
        ));
    }
    None
}

// --- HOW OLD A CLUSTER MAY BE END ---

// --- THE DRIVER START ---

/// One watch event with its kind already decided: the only thing five streams of three
/// different API types have in common, so [`drive`] itself names none of them.
type Update = Box<dyn FnOnce(&mut Store) + Send>;

/// One `watcher()` stream, ready for [`drive`].
///
/// `apply` is the [`Store`] method for the kind the stream carries — `Store::pod` for a Pod
/// watch — and this is the only line in the driver where a kind is named at all.
///
/// **The one clock read in this file, and it is here because this is the only place that knows
/// when an event arrived** (NOTES § D150). [`Store`] never calls one: [`Store::snapshot`] is
/// handed `now` because one analysis pass is one instant (NOTES § D18, § D144), and an event is
/// the other shape — it happens at a moment nobody downstream can reconstruct, so the loop that
/// receives it stamps it. That keeps the store a recorder, testable by feeding it instants, and
/// leaves the clock in the half of the file that already does I/O.
///
/// **It is read per event and not per poll.** A page of 500 drains as 500 `InitApply`s and each
/// gets its own stamp, so [`Listing::since`] means *the last object landed*, not *the batch
/// started* — which is the difference between seeing a stall inside a page and seeing it only
/// between pages.
fn updates<K: Send + 'static>(
    watch: impl Stream<Item = watcher::Result<Event<K>>> + Send + 'static,
    apply: fn(&mut Store, &Time, Event<K>),
) -> BoxStream<'static, watcher::Result<Update>> {
    watch
        .map_ok(move |event| {
            let now = Time(Timestamp::now());
            Box::new(move |store: &mut Store| apply(store, &now, event)) as Update
        })
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
