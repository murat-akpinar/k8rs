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
//! **The browser's sidebar is every kind the cluster says it serves, and none of them is named
//! here** (invariant 12). [`browsable`] takes the answer rather than fetching it and decides the
//! three things a screen cannot: a kind that cannot be listed is not offered, a CRD's own words
//! go through the same strip and bound as a watched object, and the order is ours because the two
//! calls that produce the list — `Discovery::groups()` and `ApiGroup::resources_by_stability()` —
//! both end in a hash map. § EVERY KIND THE CLUSTER SERVES
//! is what each discovery entry point costs in round trips, and the four ways it fails —
//! including the one where a server too old for the aggregated call answers `Ok` with an empty
//! cluster in it.
//!
//! **The browser's rows are the API server's own printed table, and the fallback under it is one
//! column** (invariant 12, § THE BROWSER'S ROWS). [`Fetch`] is where a list is asked for and
//! [`Table`] is what comes back — cells that are not all strings, an identity per row that the
//! `includeObject` default is kept for, and the plain object list read for what it is whether it
//! arrives as a `200` the Accept header negotiated or after a `406` from an aggregated API server.
//!
//! **A browser view is the one watch that is not permanent** (§ KEEPING A BROWSER VIEW FRESH).
//! A metadata watch says *that* something changed and [`Browsing`] decides when that is worth a
//! re-fetch. [`REFRESH_FLOOR`] is the floor between an answer and the next question, the pending
//! flag clears when a fetch is issued rather than when it returns, and one fetch is on the wire at
//! a time — the two halves of `PRIOR-ART § A5` (NOTES § D154).
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
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::jiff::{SignedDuration, Timestamp};
// `serde` and `serde_json` are reached through `k8s-openapi`'s own re-exports rather than named a
// second time in `Cargo.toml` — the same door `jiff` already comes through above, and invariant
// 10's narrowest possible answer: a crate already in the build, not even needing to be named.
use k8s_openapi::serde::Deserialize;
use k8s_openapi::serde_json::Value;
use kube::Resource;
use kube::core::response::reason;
use kube::core::{DynamicObject, gvk::GroupVersionKind};
use kube::discovery::{ApiCapabilities, ApiResource, Scope, verbs};
use kube::runtime::watcher::{self, Event};
use std::collections::{BTreeMap, BTreeSet};

// --- THE INGEST GUARD START ---
//
// **Every string a snapshot type carries is cleaned and bounded here, and nowhere else.**
// Invariant 9 owes a strip to every printer and the security gate owes a bound to every field;
// both are paid once, on the way into the decode, so no downstream consumer has to remember.
// `main.rs`'s `sanitize` was the same rule one layer up and is superseded by this
// (NOTES § D122); it stays until `dev-ui` removes it at Phase 12.
//
// **It stopped being the same rule for a day, and the second copy is gone rather than
// widened.** This guard grew the zero-width and bidi characters `char::is_control` does not
// answer for ([`unprintable`], NOTES § D154); `sanitize` did not, and the temporary driver's
// fixture path never meets this file — `main.rs` builds its snapshot straight off `rules.rs`'s
// `From` impls — so `k8rs some-pod.json` printed a U+202E straight to the terminal for as long
// as the two spellings coexisted. **`sanitize` now calls [`unprintable`]**, which is why it is
// `pub(crate)`: one predicate, one place, and the next widening reaches both paths without
// anyone remembering (CLAUDE.md § Single point of change). What `sanitize` still owns is the
// *disposal* — it removes and never substitutes, where [`text`] turns a removed whitespace
// break into one space — because a driver that strips at the `format!` cannot tell a value
// from the sentence around it.
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

/// **A character with no printed form of its own** — what this guard removes, and it is wider
/// than `char::is_control` (invariant 9, NOTES § D154).
///
/// `is_control` is Unicode `Cc` and nothing else, so U+202E RIGHT-TO-LEFT OVERRIDE, U+200B ZERO
/// WIDTH SPACE, U+00AD SOFT HYPHEN and U+FEFF walked straight through it — **compiled and run,
/// not reasoned from the name**: `is_control` was evaluated on each of them and
/// `"prod\u{202e}reversed"` came back out of [`text`] unchanged. A bidi override reverses every
/// character after it, so `prod\u{202e}dc` is a row that reads *prodcd* and matches neither
/// spelling in a search; a zero-width character hides a difference between two names outright.
/// That is Trojan Source in a row, and § THE BROWSER'S ROWS is what makes it reachable from
/// every cell of every kind the cluster serves — a CRD's `additionalPrinterColumns`, an Events
/// `MESSAGE` — instead of from the named `String` fields the snapshot types carry.
///
/// **Ranges rather than the codepoints somebody has been seen using**: `200b..=200f` is the
/// zero-width and bidi-mark block, `202a..=202e` the embeddings and overrides, `2060..=206f` the
/// word joiner, the invisible operators, the bidi *isolates* — the modern spelling of the
/// override — and the deprecated format characters; the two singletons are the soft hyphen and
/// the byte-order mark. **Not one of them is `char::is_whitespace`**, so none becomes a space in
/// [`text`] — they separated nothing.
///
/// **Two of them do carry meaning somewhere, and that cost is paid deliberately.** U+200C and
/// U+200D sit inside the first range: the joiner builds a multi-person emoji out of several, and
/// both change how Persian and Indic text is shaped. A *message* containing one renders
/// differently after this. They are removed anyway, because they are also how two names are made
/// to look like one, and a Kubernetes name is a DNS label with neither of them in it.
///
/// **What is deliberately left in is anything that prints.** U+2028 and U+2029 are drawn as a
/// glyph by a terminal rather than obeyed, and U+00A0 is a space that is visible as one;
/// removing either would change text the cluster meant to send.
pub(crate) fn unprintable(character: char) -> bool {
    character.is_control()
        || matches!(character,
            '\u{ad}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{206f}' | '\u{feff}')
}

/// One untrusted string, made safe to hold and safe to print.
///
/// **An [`unprintable`] character that is whitespace becomes one space; every other one is
/// removed** (NOTES § D146, § D154). `char::is_whitespace` is what decides which — `HT`, `LF`,
/// `VT`, `FF`, `CR` and `NEL`, and nothing else this guard removes — so the split is the
/// standard library's and not a list kept here. A boundary deleted glues two words into one;
/// `ESC`, `NUL`, `DEL`, the rest of the C1 range and every zero-width and bidi character have
/// no readable equivalent and are what invariant 9 exists for.
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
        if !unprintable(character) {
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
        maybe(&mut self.reason, IDENTIFIER);
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

/// **What the sidebar keeps of one kind the cluster serves** (§ EVERY KIND THE CLUSTER SERVES).
///
/// **Not a snapshot type, and here anyway.** Every other impl above belongs to something a watch
/// decoded; this one belongs to a discovery answer, and its strings are the *only* ones in this
/// file that a person outside the cluster's control plane can choose — a CRD's `spec.names` is
/// whatever the manifest said. That is invariant 9's own class of input, so it goes through the
/// same door rather than a second one beside it.
impl Bounded for Browsable {
    fn bound(&mut self) {
        text(&mut self.group, IDENTIFIER);
        text(&mut self.version, IDENTIFIER);
        text(&mut self.kind, IDENTIFIER);
        text(&mut self.plural, IDENTIFIER);
        for verb in &mut self.verbs {
            text(verb, IDENTIFIER);
        }
    }
}

/// **What the browser keeps of one column the server printed** (§ THE BROWSER'S ROWS).
///
/// Here for [`Bounded for Browsable`](Browsable)'s reason and one more: the name of a CRD's
/// `additionalPrinterColumns` entry is written by whoever wrote the manifest, so a column header is
/// the same untrusted class as a plural (invariant 9).
impl Bounded for Column {
    fn bound(&mut self) {
        text(&mut self.name, IDENTIFIER);
    }
}

/// **A printed cell is [`FREE_TEXT`] and every one of them is, because nothing kind-agnostic says
/// which are words and which are sentences.**
///
/// D146 splits the two classes by what the value is drawn as, and a `Table` cell can be either: an
/// Events table's `MESSAGE` is a sentence, a Pod's `Ready` is `1/1`. The wire carries no signal
/// that separates them — `columnDefinitions[].type` is `string`/`integer`/`date`, and `format` is
/// `name` or empty — and a list of which columns are sentences would be a per-kind list, which is
/// the thing invariant 12 refuses. So the class that keeps a sentence whole is the one used, and
/// the narrower bound stays on the identity beside it, which is an identifier in every row.
impl Bounded for Row {
    fn bound(&mut self) {
        for cell in &mut self.cells {
            text(cell, FREE_TEXT);
        }
        maybe(&mut self.namespace, IDENTIFIER);
        maybe(&mut self.name, IDENTIFIER);
        maybe(&mut self.uid, IDENTIFIER);
    }
}

impl Bounded for Table {
    fn bound(&mut self) {
        self.columns.bound();
        self.rows.bound();
    }
}

/// **The whole of what happens to an API object on its way into the store**: decode, which is
/// the prune, then strip and bound.
///
/// One function so there is one answer — a second entry point into the store is a second place
/// to forget the guard. [`Watch::take`] for a watched object, [`Store::owner_fetched`] for a
/// ReplicaSet fetched by uid, [`browsable`] for a kind discovery named, and the browser's
/// [`Table`] for either shape a list comes back in. **The doc said *the only caller* until
/// 2026-08-22** and the second one had landed a box earlier; the count is not the point, and it
/// is not kept here any more — the single door is.
///
/// **The `From` is what makes the door unavoidable for the browser's rows.** [`Table`] does not
/// implement `Deserialize`, so `Client::request::<Table>` cannot compile: the only way to hold one
/// is `ingest::<TableResponse, _>`, whichever of the two shapes the body turned out to be, and it
/// ends here.
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
    /// **Every ReplicaSet a pod named as its controller, keyed by the uid the pod named** —
    /// § RESOLVING AN OWNER is the whole of what this is for.
    ///
    /// **Keyed by uid and not by name**, because a rollback re-creates a ReplicaSet under the
    /// same name with a new uid, and the two are different objects with different pods.
    /// `Err` is a fetch that did not produce one, kept so the same reference is not asked
    /// about again on the next pass ([`Why`]).
    owners: BTreeMap<String, Result<WorkloadSnapshot, Why>>,
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
    /// then name order, so two calls over one store are the same list twice — the resolved
    /// ReplicaSets included, which follow the three watched kinds and are sorted the same way
    /// ([`Store::resolved_sets`]).
    ///
    /// **This is also where a pod's owner is walked up to the workload the reader deployed**
    /// (§ RESOLVING AN OWNER). It is the one place a snapshot type is edited on the way out
    /// rather than on the way in, because the answer comes from a second object that arrives on
    /// its own schedule.
    pub fn snapshot(&self, now: Time) -> Option<ClusterSnapshot> {
        if !self.listed() {
            return None;
        }
        Some(ClusterSnapshot {
            now,
            // **The pods come out with their owner walked up to the workload the reader
            // deployed** (§ RESOLVING AN OWNER). A pod whose ReplicaSet the cache cannot answer
            // for keeps the ReplicaSet, which is its true controller and is not a guess.
            pods: self
                .pods
                .live
                .values()
                .map(|pod| self.with_owner(pod))
                .collect(),
            nodes: self.nodes.live.values().cloned().collect(),
            // **The three watched kinds, then the ReplicaSets the cache resolved.** W1 is
            // written about a ReplicaSet and reads this list, and so does `rules.rs`'s own
            // `workload_owner` — the file driver in `main.rs` already puts a ReplicaSet in both
            // fields for that reason, and this is the live path saying the same thing.
            workloads: self
                .deployments
                .live
                .values()
                .chain(self.stateful_sets.live.values())
                .chain(self.daemon_sets.live.values())
                .cloned()
                .chain(self.resolved_sets())
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
            //
            // **The owner cache is not this list and may not be poured into it**
            // (§ RESOLVING AN OWNER). Waste's row is *ReplicaSets parked at 0 replicas*, and a
            // parked ReplicaSet has no pods — so it is exactly what the owner cache structurally
            // never holds. Filling this from the cache would answer *nothing is parked* off a
            // list that cannot contain a parked one, which is D129's reassuring wrong answer
            // with a new cause.
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

// --- RESOLVING AN OWNER START ---
//
// **A pod's `ownerReferences` names its ReplicaSet, and the card has to read `web`** — the name
// the reader deployed — rather than `web-7d4f5c6b8`, the name a controller generated
// (todo.md § Phase 5, NOTES § D3). `rules.rs` files every pod finding under
// [`PodSnapshot::owner`], `ObjectId::name`'s own doc says that name is *"the controller's,
// resolved up to the Deployment where there is one"*, and it names this file as the place that
// resolves it.
//
// **The hash is never chopped off the string, and that is the whole reason this costs a network
// call.** `web-7d4f5c6b8` minus a dash and ten characters is a guess that is right most of the
// time, and `metadata.name` has no field saying which part was generated: a Deployment may
// legitimately be called `web-7d4f5c6b8`, and a ReplicaSet's suffix is a hash of the pod
// template, not a fixed length. The answer is the ReplicaSet's **own** controlling
// `ownerReference` — which carries a **uid**, a value no string operation on the pod could ever
// produce. That is what [`Store::with_owner`] copies across, and what the tests assert.
//
// **Fetched on demand, cached by uid, never watched** (invariant 6). Watching ReplicaSets would
// add a permanent stream over the busiest object a rollout produces to answer a question about
// names; a `get` per distinct ReplicaSet a live pod names, kept until no pod names it, is the
// same answer for one request per rollout.
//
// **Keyed by uid rather than by name, because a rollback re-uses the name.** Rolling back to a
// previous pod template re-creates a ReplicaSet with the *same* generated hash and a new uid, so
// a name-keyed cache would hand the new object's pods the old object's answer. The uid is also
// what the fetch is checked against: the `get` goes out by name, and an object that comes back
// under a different uid is not the one that was asked about.
//
// **A cache miss does not hold the snapshot back**, and that is a decision rather than an
// omission. NOTES § D148 measured what one request costs when a server is throttling: kube
// retries fifteen times inside a tower layer with no callback, so a single `get` can be silent
// for **two and a half to eight minutes**. Gating [`Store::snapshot`] on resolution would put
// every alert on this screen behind that window. So the snapshot is published with the
// ReplicaSet as the owner, and the heading changes to the Deployment when the answer lands. This
// is not the partial-list case NOTES § D28 forbids: a short list makes a rule *count wrongly*,
// while an unresolved owner names the pod's true controller, one step lower than the reader
// would have named it.
//
// **What is not here is the `get` itself**, for the reason § THE INITIAL LIST gives about
// `page_size`: there is no `Client` in this build yet, and a function no test can fail on is
// what the mutation gate exists to catch. Everything a fetch's answer means is decided here and
// proven here; the `connect()` box supplies one line —
// `Api::<ReplicaSet>::namespaced(client, ns).get(&name).await` — and hands the result to
// [`Store::owner_fetched`] **unchanged**. Not `get_opt`, which folds a 404 into `Ok(None)` and
// throws away the difference between *deleted mid-rollout* and *never existed*.

/// **Why a pod's ReplicaSet owner has not been walked up to the workload above it.**
///
/// Four different facts, and none of them may be presented as *the group is called
/// `web-7d4f5c6b8`* without saying which one it is. **The words are the caller's**, as with
/// [`Listing`]: invariant 14's plain language is the screen's decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// **Nothing has asked yet.** The ordinary state of every ReplicaSet a new pod names, and
    /// the one state that is an instruction: these are the references the caller fetches.
    NotAsked,
    /// **The API server said the ReplicaSet is not there** — a `404`, or an object that came
    /// back under a different uid.
    ///
    /// **This is normal traffic, not a fault.** Every rollout deletes the ReplicaSet it
    /// replaced, and a pod read a moment before that can name one that is already gone.
    Gone,
    /// **The API server refused** — a `403`. The kubeconfig's role cannot `get replicasets`,
    /// which is one missing verb on one resource and degrades exactly this feature: the cards
    /// still draw, headed with the ReplicaSet.
    ///
    /// **The verb and the resource are not carried as data because they are constants of the one
    /// call site** — `get replicasets` and nothing else is ever asked for here — so a screen
    /// naming them, which the security gate requires of every 403, reads them off this variant.
    /// A field for a value with one possible content is a second copy of it.
    Refused,
    /// **The fetch produced neither the object nor a refusal** — a timeout, a socket that died,
    /// a `500`, a `429` that outlived kube's retries.
    ///
    /// **One variant for all of them on purpose.** From the reader's side they are one fact —
    /// *k8rs could not ask* — and NOTES § D148 is why nothing here can tell them apart anyway:
    /// the wait happens below the client in a layer with no callback and no counter.
    Failed,
}

/// **One ReplicaSet a pod names as its controller, which the cache cannot answer for**, and why.
///
/// The shape is [`Listing`]'s: facts, not sentences, in namespace-then-name order.
///
/// **It derives `Debug` where [`Store`] deliberately does not**, for [`Listing`]'s reason: an
/// identity and a four-way enum are values that never touched a credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved {
    /// The ReplicaSet, exactly as the pod's `ownerReference` named it — namespace, name and the
    /// uid the fetch is keyed and checked on.
    pub id: ObjectId,
    pub why: Why,
}

/// The uid a ReplicaSet owner is cached under, or `None` where there is nothing safe to key on.
///
/// **Two refusals, and both are load-bearing.** An owner that is not a ReplicaSet is already the
/// workload the reader deployed and there is nothing above it to walk to. An owner whose uid is
/// **empty** is refused because the uid is the cache key: two different ReplicaSets would share
/// the entry `""` and each would be handed the other's Deployment. The API server rejects an
/// `ownerReference` with no uid, so this is a shape only something between us and it can
/// produce — which is exactly invariant 9's class of input, one layer past the strip.
///
/// **The ceiling, named rather than guarded**: the uid has already been through [`ingest`], so
/// two uids longer than [`IDENTIFIER`] sharing a 512-byte prefix would collapse into one entry —
/// the same loss [`pairs`] documents for two label keys (NOTES § D146). A uid the API server
/// generates is 36 bytes.
fn owner_uid(owner: &ObjectId) -> Option<&str> {
    if owner.kind != ObjectKind::ReplicaSet {
        return None;
    }
    owner.uid.as_deref().filter(|uid| !uid.is_empty())
}

/// **What one failed fetch means**, from the answer the API server gave.
///
/// **Read off `code` as well as `reason`, because kube's own helpers cannot be used here.**
/// `Status::is_not_found` and `Status::is_forbidden` are `self.reason == reason || (!is_known(
/// reason) && self.code == code)` — and the `reason` they pass `is_known` is the *constant*
/// they were called with, which is always known, so the `code` half is dead. Go's original
/// tests the **response's** reason there. The consequence is measured in `k8s_tests.rs`: a
/// `Status` carrying `code: 403` and no reason answers `is_forbidden() == false`, and that is
/// the exact shape kube builds when a proxy's refusal does not parse as a `Status` —
/// `Status::failure(text, "Failed to parse error data").with_code(403)`
/// (`kube-client-4.2.0/src/client/mod.rs:556`).
fn why(error: &kube::Error) -> Why {
    let kube::Error::Api(status) = error else {
        return Why::Failed;
    };
    match (status.code, status.reason.as_str()) {
        (404, _) | (_, reason::NOT_FOUND) => Why::Gone,
        (403, _) | (_, reason::FORBIDDEN) => Why::Refused,
        _ => Why::Failed,
    }
}

impl Store {
    /// **Every ReplicaSet a live pod names as its controller that the cache has no object for**,
    /// one entry per ReplicaSet however many pods name it.
    ///
    /// **Two callers, one list, for [`Store::still_listing`]'s reason.** The fetcher takes the
    /// [`Why::NotAsked`] entries; a screen shows the rest, so a heading that stayed at the
    /// generated name always has a fact behind it and never a silence.
    ///
    /// **A failure stays in the answer and is therefore not asked again.** A `403` on
    /// `replicasets` is a standing fact about the kubeconfig's role, and a caller that re-read
    /// it as *not asked* would send one refused request per pod per pass — the retry loop the
    /// security gate forbids by name. **Nothing here retries, ever, and the ceiling that names
    /// is a transient [`Why::Failed`]** — a socket that died once leaves the heading at the
    /// ReplicaSet for the life of the process. Retry policy belongs to the reconnect box, which
    /// is where per-watch identity arrives; this half is the one that is true without it,
    /// exactly as [`Store::failure`] is.
    pub fn unresolved_owners(&self) -> Vec<Unresolved> {
        let mut found = BTreeMap::new();
        for pod in self.pods.live.values() {
            let Some(uid) = owner_uid(&pod.owner) else {
                continue;
            };
            let why = match self.owners.get(uid) {
                None => Why::NotAsked,
                Some(Ok(_)) => continue,
                Some(Err(why)) => *why,
            };
            found
                .entry((pod.owner.namespace.as_deref(), pod.owner.name.as_str(), uid))
                .or_insert_with(|| Unresolved {
                    id: pod.owner.clone(),
                    why,
                });
        }
        found.into_values().collect()
    }

    /// **One fetch's answer**, filed under the uid that was asked about.
    ///
    /// `id` is the [`Unresolved::id`] that was handed out, and it rather than the returned
    /// object is what the entry is keyed on: an `Err` carries no object to read a uid from, and
    /// an `Ok` under the wrong uid is the case below.
    ///
    /// **An object that comes back under a different uid is [`Why::Gone`]**, not an answer. The
    /// `get` goes out by name, and a name can have been re-used since the pod was read — a
    /// rollback re-creates a ReplicaSet with the same generated hash — so the object on the wire
    /// may be a different one that happens to be called the same thing. Its Deployment could
    /// even be the right one; *could* is not what a card's heading may rest on.
    ///
    /// **The object goes through [`ingest`] like everything else**: the same prune, the same
    /// strip and the same bound as a watched object, so nothing reaching a card by this route
    /// skips invariant 9. That is also what makes the cached value the whole `WorkloadSnapshot`
    /// rather than a resolved name — W1 reads `status.conditions[ReplicaFailure]` off it.
    ///
    /// **An `id` that is not a ReplicaSet, or carries no usable uid, is dropped** rather than
    /// filed under a key that would collide ([`owner_uid`]). Nothing hands one out.
    pub fn owner_fetched(&mut self, id: &ObjectId, answer: Result<ReplicaSet, kube::Error>) {
        let Some(uid) = owner_uid(id).map(str::to_string) else {
            return;
        };
        let answer = match answer {
            Ok(set) => {
                let set: WorkloadSnapshot = ingest(set);
                if set.id.uid.as_deref() == Some(uid.as_str()) {
                    Ok(set)
                } else {
                    Err(Why::Gone)
                }
            }
            Err(error) => Err(why(&error)),
        };
        self.owners.insert(uid, answer);
        self.prune_owners();
    }

    /// **Everything no live pod names any more, dropped.**
    ///
    /// **Run here because here is the only place the cache can grow.** A rollout an hour for a
    /// month is a ReplicaSet an hour, and nothing else would ever take one out; pruning where
    /// an entry is added bounds the map by what the pods referenced at that moment, without a
    /// timer and without a size to tune.
    fn prune_owners(&mut self) {
        let referenced: BTreeSet<&str> = self
            .pods
            .live
            .values()
            .filter_map(|pod| owner_uid(&pod.owner))
            .collect();
        self.owners
            .retain(|uid, _| referenced.contains(uid.as_str()));
    }

    /// The ReplicaSet this owner reference resolved to, if the cache holds one.
    fn resolved(&self, owner: &ObjectId) -> Option<&WorkloadSnapshot> {
        self.owners.get(owner_uid(owner)?)?.as_ref().ok()
    }

    /// **One pod, with its owner walked one step up.**
    ///
    /// [`WorkloadSnapshot::owner`] on the resolved ReplicaSet is already the answer: `rules.rs`
    /// built it from that object's own controlling `ownerReference`, so a ReplicaSet a
    /// Deployment controls yields the Deployment, and one nothing controls — or one an operator's
    /// CRD controls, which `ObjectKind::from_api` leaves as `Other(_)` — yields whatever is
    /// actually true, including itself. **One hop and no loop**: the chain NOTES § D28 describes
    /// is Pod → ReplicaSet → Deployment and stops there.
    fn with_owner(&self, pod: &PodSnapshot) -> PodSnapshot {
        let mut pod = pod.clone();
        if let Some(set) = self.resolved(&pod.owner) {
            pod.owner = set.owner.clone();
        }
        pod
    }

    /// The ReplicaSets the cache resolved, in namespace then name order — the order
    /// [`Store::snapshot`] promises for the watched kinds, so one list comes out sorted the same
    /// way throughout.
    ///
    /// **The uid is in the sort key and not only in the map key.** Two ReplicaSets can share a
    /// namespace and a name across a rollback, and a sort that ignored the uid would be free to
    /// order them either way between calls.
    fn resolved_sets(&self) -> Vec<WorkloadSnapshot> {
        let mut sets: Vec<WorkloadSnapshot> = self
            .owners
            .values()
            .filter_map(|answer| answer.as_ref().ok())
            .cloned()
            .collect();
        sets.sort_by_key(|set| (key(set), set.id.uid.clone()));
        sets
    }
}

// --- RESOLVING AN OWNER END ---

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
// **The sixth is where the number came from.** Rule 13 — `rules.rs`'s `placed_but_never_started`
// — *read* the condition through `is_some_and(|c| c.status == "False")`, so an absent condition
// fell into the `else` and the card **stated a fact**: *"this pod has its storage and its
// network, so the block is later"*. On a server that never set the condition nothing had said
// that, and it sent the reader whose ConfigMap is missing to look at the CNI.
//
// **That branch has been three arms since 2026-08-22, and the floor did not move with it**
// (NOTES § D156, ruling 4). An absent condition — and a present `Unknown` one — take an arm that
// claims neither side, so the one case this number was measured from can no longer misdirect
// anybody on any server. What holds 1.29 up afterwards is the case nobody has audited: D149's
// generalisable half asks the same question of every `else` over an API `Option` in this
// repository, it is still open, and a lower floor would be a claim about branches nobody has
// read. **The live defect is gone and the unfinished audit is not**, which is a weaker
// foundation than the original and is why the number stays where it is rather than following
// the fix.
//
// **Where the condition starts existing is measured off the API types and not off the gate
// table**, because the two disagree: the gate is listed as alpha at 1.28, but
// `staging/src/k8s.io/api/core/v1/types.go` carries no `PodReadyToStartContainers` constant on
// `release-1.28`, and no `PodHasNetwork` one on `release-1.25` … `release-1.27` either — the
// old name was a kubelet-internal constant. It appears in the public `PodConditionType` block
// for the first time on **`release-1.29`** (`types.go:3005`) — which is also what the table's
// *beta, default on* column gives, so the disagreement above is confined to the *alpha 1.28*
// cell and nothing else moved: a gate can be alpha while the public type still carries no
// constant, and *when the field can exist at all* is the type's question. That is the whole
// derivation of the number — the oldest minor on which the one sentence measured to be unbacked
// is backed by something the server actually said. It was never a proof that the same holds of
// *every* sentence k8rs prints, which is what D149's open half is for.
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

// --- EVERY KIND THE CLUSTER SERVES START ---
//
// **The browser's sidebar is whatever the API server says it serves, and this is where that
// answer stops being kube's and becomes something a screen may believe** (todo.md § Phase 5,
// invariant 12). A kind written down anywhere above this line is the design failure that
// invariant names; what is written down here is one filter, one strip and one order.
//
// **The fetch itself is the `connect()` box**, for the reason § THE INITIAL LIST gives about
// `page_size`: with no `Client` in this build, a function whose whole body is
// `Discovery::new(client).run_aggregated().await` is a line no test can fail on, and the
// mutation gate exists to catch exactly that. Everything the answer *means* is decided here.
//
// **What each entry point costs, counted off the calls and not off the doc comment above them**
// (NOTES § D147 read the initial LIST the same way). `G` is the groups a server serves and
// `V(g)` the versions each one serves; every path below is
// `kube-client-4.2.0/src/discovery/`:
//
// | call | round trips | what they are |
// |---|---|---|
// | `Discovery::new(c).run()` | `2 + ΣV(g)` | `/apis`, then **one per group *version*** (`apigroup.rs:96-99`), then `/api` and one per core version (`apigroup.rs:115-118`) |
// | `Discovery::new(c).run_aggregated()` | **2** | `/apis` and `/api`, at any cluster size (`mod.rs:171-200`) |
// | `discovery::group(c, g)` | `1 + V(g)` | `/apis` — or `/api` for the core group — then that one group's versions (`oneshot.rs:41-51`) |
// | `discovery::pinned_group(c, gv)` | 1 | `/apis/<g>/<v>` alone (`apigroup.rs:187-201`) |
// | `discovery::pinned_kind(c, gvk)` | 1 | the same call with one kind picked out (`apigroup.rs:164-184`) |
// | `Discovery::resolve_gvk(gvk)` | 0 | a lookup over what `run*` already cached (`mod.rs:234`) |
//
// **`run()`'s own doc says `N+2` where `N` is the number of groups (`mod.rs:87`), and the loop
// is per version.** A group serving `v1` and `v1beta1` costs two, and a cluster with CRDs on it
// is the ordinary case rather than the exotic one. **The calls are also sequential** —
// `for g in api_groups.groups { … .await? }` (`mod.rs:118-124`) — so they are `ΣV(g)` waits one
// after another on the path that draws the first screen. **How long one round trip takes is not
// measured here and cannot be**: this file has never met an API server, and `G` itself is a
// number only a cluster can say.
//
// **So the sidebar is built from `run_aggregated()`, and a fallback under it is not
// optional.** Two calls rather than thirty-odd sequential ones is the first-paint argument, and
// the partial-failure shape in § What breaks is the larger one.
//
// **Its floor is 1.27 and not the 1.26 kube's doc claims** (`mod.rs:137`, `:166`). Read off
// `AggregatedDiscoveryEndpoint.md` in `kubernetes/website` on 2026-08-22, the same source
// § HOW OLD A CLUSTER MAY BE used: **alpha at 1.26 and default `false`**, beta and on from
// **1.27**, GA 1.30, gate since removed. So on a 1.26 server, and on any older one, the
// aggregated call is answered by a server that does not know the type — which is the first
// failure below, not an error.
//
// ## What breaks, and which half of it is proven here
//
// **1. A server that does not serve aggregated discovery answers `Ok` with nothing in it.**
// The Accept header carries a `,application/json` fallback (`kube-core-4.2.0/src/discovery/
// v2.rs:12`), so such a server replies with the ordinary `APIGroupList`; `Client::request`
// deserialises whatever came back straight into `APIGroupDiscoveryList`
// (`kube-client-4.2.0/src/client/mod.rs:281-291`, plain `serde_json::from_str`), whose fields
// are all `#[serde(default)]` and which denies no unknown field — so `groups` is ignored,
// `items` defaults to empty, and `run_aggregated` returns `Ok` with **zero groups**. kube's own
// doc says the opposite in as many words: *"If the server does not support Aggregated Discovery,
// this will return an error"* (`mod.rs:168-170`). It does not, and an empty sidebar is exactly
// what a cluster that serves nothing would look like. **Proven, not read**: `k8s_tests.rs` feeds
// a real `APIGroupList`'s own serialisation to the aggregated type and reads back nothing. What
// is *not* proven is that an old server answers that header with that body — that is HTTP
// content negotiation against a real API server, and nobody here has one.
//
// **2. `run()` cannot express a partial failure at all.** One group's fetch failing ends the
// whole run: the `?` is inside the loop (`apigroup.rs:97`, `mod.rs:121`). So a group that `/apis`
// names and that cannot then answer for itself takes the entire sidebar with it, and the reader
// loses every kind because one is unreachable. **The mechanism is read off the loop; the shape
// that produces it is not measured here** — an aggregated API server whose backing pod is down,
// `metrics.k8s.io` being the one every cluster has, is the case to point a measurement at.
//
// A fallback that wants partial results cannot use `Discovery` for it; it is
// `client.list_api_groups()` plus one
// `list_api_group_resources` per group version with the errors kept per group, which is code
// that belongs to `connect()` and is named here so that box does not rediscover it.
//
// **3. The aggregated call has the partial-failure answer and kube throws it away.** The wire
// type carries `freshness` per group version — *"Stale indicates the discovery document could
// not be retrieved and the returned discovery document may be significantly out of date"*
// (`kube-core-4.2.0/src/discovery/v2.rs:61-68`) — and `GroupVersionData::from_v2` builds
// `{ version, resources }` from it and keeps no trace (`discovery/parse.rs:94-108`), while
// `ApiGroup` has no field for one either (`apigroup.rs:74-81`). So through `kube::discovery` a
// stale group and a current one are the same value, and a screen cannot say *this group's list
// may be out of date* however much it wants to. Reaching it means calling
// `client.list_api_groups_aggregated()` directly and reading `items[].versions[].freshness`
// before handing the rest on — one call, the same call, not an extra one.
//
// **4. A 403 on discovery is not a 403 on a kind.** Both arrive as `kube::Error::Api(Status)`,
// and they mean different things: refused on `/apis` is *no sidebar at all*, refused on one
// kind's list is *this one row cannot open*. **[`why`] is not the function for the first one** —
// its four arms are about a ReplicaSet fetched by name, and `/apis` is not a thing that is
// deleted mid-rollout — so discovery gets no reason enum of its own either: it is one call with
// one outcome, and the caller keeps the `kube::Error` it was handed. What does carry over
// unchanged is that **nothing retries**, which is [`Store::unresolved_owners`]'s rule and holds
// here for its reason (NOTES § D151): a standing refusal re-asked once a pass is the retry loop
// the security gate forbids by name.
//
// ## What an entry carries, and the one thing it does not
//
// **The verbs are the resource's, never the reader's.** `list` in `operations` means this
// resource can be listed by *somebody*; it says nothing about whether this kubeconfig may. The
// only call that answers that is a `SelfSubjectAccessReview`, which is performed with `create`
// and therefore lives in `ops.rs` and nowhere else (invariant 1, NOTES § D23). So [`browsable`]
// drops what cannot be listed **at all** and keeps what can; a kind the reader is refused stays
// in the sidebar and answers `403` when it is opened, and telling them that is the browser's job,
// not this filter's.
//
// **Subresources are gone before this is called, as long as the server is well behaved.** The
// legacy path skips any resource whose name contains `/` (`parse.rs:79`) and the aggregated one
// nests a *declared* subresource under its parent (`parse.rs:128-132`), so `pods/log` does not
// reach this function from either on an ordinary cluster. **What the aggregated path does not do
// is check the top-level name at all** — `parse_v2_resource` is `plural: res.resource
// .unwrap_or_default()` (`parse.rs:115-132`) with no filter on it — and `run_aggregated()` is the
// call this file makes, so a `/` in a plural is a shape the pipeline *can* produce after all.
// **That is [`path_safe`]'s subject, in § THE BROWSER'S ROWS, and it is not this filter**: this one
// is about what a resource is, that one about what a URL may be built from, and only the second
// has ever met a byte that leaves the machine.
//
// **`namespaced` is `Scope::Namespaced`, and an omitted scope reads as cluster-wide.** The
// aggregated parse is `match res.scope.as_deref() { Some("Namespaced") => Namespaced, _ =>
// Cluster }` (`parse.rs:115-118`), so a server that sent no scope, or one k8rs cannot spell,
// silently becomes `Cluster` — which is `screens/resources.md`'s rule for whether the `ns:`
// label is drawn at all. It degrades toward showing every namespace rather than the wrong one.
//
// **Three fields the API sends are unreachable through `kube::discovery`**: `shortNames`,
// `singularResource` and `categories` are on the wire (`v2.rs:88-104`) and neither parse keeps
// them (`parse.rs:21-27`, `:120-126`). `categories` is the nearest thing the API has to the
// sidebar's *workloads / network / storage / config / cluster* sections, and it would not have
// been enough anyway — it carries `all` and little else. **Those five sections are k8rs's own
// categorisation and cannot come from discovery**; that is the Phase 9 sidebar box's to settle,
// and it is named here because "not a hard-coded list" is true of the *kinds* and cannot be made
// true of the sections by this call.
//
// ## Two things the caller must do, because kube will not
//
// **The order is ours.** `Discovery::groups()` is `HashMap::values()` (`mod.rs:206-208`) and
// `ApiGroup::resources_by_stability()` ends in `HashMap::into_values()` (`apigroup.rs:320-326`),
// so both hand back iteration order — a sidebar built straight off either would be in a
// different order on every launch. [`browsable`] sorts, and `groups_alphabetical()`
// (`mod.rs:213-218`) is the group-level equivalent kube provides.
//
// **The per-group accessor is `resources_by_stability()` and not `recommended_resources()`.**
// The latter is `versioned_resources(preferred_version_or_latest())` (`apigroup.rs:284-287`), and
// **preference is a group concept** — kube's own doc calls this the common pitfall
// (`apigroup.rs:60-64`). A group whose preferred version is `v1` because one CRD reached `v1`
// silently omits every sibling CRD still served only at `v1alpha1`, which is a kind the cluster
// serves vanishing out of the list this box exists to build. `resources_by_stability()` keys by
// kind across all versions of the group and picks the most stable each (`apigroup.rs:310-327`),
// which is one entry per kind and no kind lost.
//
// **A plural can still appear twice and both rows are real.** `events` is served by `core/v1`
// and by `events.k8s.io/v1`; they are different resources and [`browsable`] keeps both, adjacent,
// because the sort is by plural first. Which one a sidebar draws — or how it tells them apart —
// is `views.rs`'s, and it needs the group to do it, which is why the group travels.
//
// ## Discovery is a photograph, and there is no loop here
//
// A CRD installed while k8rs is open is absent from an answer taken at connect, and so is one
// deleted. **Re-running costs the same two calls** as the first run, which is cheap enough that
// the question is only *when*, never *whether we can afford it*. **The trigger is not built in
// this box and no timer is added by it**: invariant 6 is about watches and this is not one, and
// a periodic re-discovery is the poll that invariant exists to refuse. The two triggers that
// cost nothing when nothing changed are a key the reader presses on the browser, and a `404`
// from a Table fetch on a kind this list still names — the answer the API server gives for a
// kind that has gone. Both belong to the boxes that own those calls.

/// **One kind the browser may offer, as the cluster described it.**
///
/// Every field is the API server's own word for something: nothing here is a name k8rs chose,
/// which is invariant 12 stated as a struct. The four strings and the flag reconstruct kube's
/// [`ApiResource`] exactly, through
/// [`ApiResource::from_gvk_with_plural`](kube::discovery::ApiResource::from_gvk_with_plural) —
/// the constructor that is *told* the plural rather than the one that guesses it — so a caller
/// that needs one for `Api::all_with` loses nothing by this type not carrying it.
///
/// **Every string has been through the ingest guard** ([`Bounded`], § THE INGEST GUARD): a CRD's
/// plural is whatever its manifest said, and a manifest is written by somebody who does not have
/// to be friendly (invariant 9). **The ceiling: the guard bounds each string and not the number
/// of them**, so ten thousand CRDs are ten thousand entries — the open collection-bound box in
/// todo.md § Phase 5 is that class and this is one more member of it.
///
/// **The subresources discovery also reports are dropped, and the one thing that would want them
/// is named rather than left to be found.** `ApiCapabilities::subresources` carries `scale`,
/// `status` and `log` per kind; nothing in `todo.md` or `screens/` decides a key's availability
/// from them today — the `scale` box is written against `Api::patch_scale` on the workload kinds
/// — so carrying them would be a field with no reader. A box that later needs one re-runs
/// discovery for it, which § EVERY KIND THE CLUSTER SERVES prices at two round trips.
///
/// **It derives `Debug` where [`Store`] deliberately does not**, for [`Listing`]'s reason: a
/// group, a version and a plural never touched a credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Browsable {
    /// The API group, empty for the core one — `""`, `apps`, `example.com`.
    pub group: String,
    /// The version this kind is offered at: the most stable one the group serves it at, if the
    /// caller used `resources_by_stability()` as the region above says it must.
    pub version: String,
    /// The PascalCase kind — `Deployment`. What a `Table` response comes back as.
    pub kind: String,
    /// The plural, lowercase — `deployments`. **What the sidebar draws and what the URL path
    /// carries**, which is why it is not derived from [`Browsable::kind`]: kube's own
    /// pluraliser calls itself a guess that "for CRDs with complex pluralisations it can fail"
    /// (`kube-core-4.2.0/src/discovery/mod.rs:50-56`), and the server sent the real one.
    pub plural: String,
    /// Whether objects of this kind live in a namespace — `screens/resources.md` draws the `ns:`
    /// label from this and from nothing else.
    pub namespaced: bool,
    /// **What the resource supports, which is not what the reader is allowed to do** (the region
    /// above). Kept whole rather than reduced to the one verb this file filters on, because it
    /// is the only thing here that a second round trip would be needed to get back.
    pub verbs: Vec<String>,
}

impl From<(ApiResource, ApiCapabilities)> for Browsable {
    fn from((resource, capabilities): (ApiResource, ApiCapabilities)) -> Self {
        Self {
            group: resource.group,
            version: resource.version,
            kind: resource.kind,
            plural: resource.plural,
            namespaced: capabilities.scope == Scope::Namespaced,
            verbs: capabilities.operations,
        }
    }
}

/// **Every kind the cluster serves that a browser can actually open**, in one order.
///
/// The caller does the fetching and hands the answer over — the region above says which calls,
/// what they cost and which accessor loses a kind. This takes the flattened
/// `(ApiResource, ApiCapabilities)` pairs of every group and answers what may be offered.
///
/// **Three things happen and nothing else does.** A resource that cannot be *listed* is dropped,
/// because the browser has one verb and a row that answers `405` is worse than an absent one.
/// What survives goes through [`ingest`] — the same strip and bound as a watched object, for the
/// reason [`Bounded for Browsable`](Browsable) gives. Then it is sorted, because the two calls
/// that produce the list end in a hash map and a sidebar that reshuffles itself between launches
/// is unusable.
///
/// **A kind whose own words cannot build a URL is still offered here, and cannot be opened.** The
/// region above is why one can exist at all; [`path_safe`] is where it is refused, one layer
/// later, at the only place a discovery word becomes a request. Refusing it *here* would be the
/// stronger answer and is deliberately not taken this turn: it would also drop a kind the ingest
/// guard had just shortened or stripped, which reverses two things this box proved, and what a
/// screen should say about a row that cannot open is not a question this file settles alone.
///
/// **Sorted by plural, then group, then version** — by the word the sidebar draws first, so the
/// two `events` land next to each other rather than at opposite ends, and by the group second so
/// that pair has a fixed order too. **After the bound and not before**: two plurals cut to the
/// same 512 bytes would otherwise be ordered by text nobody can see.
///
/// **Nothing is de-duplicated.** One kind per group per call is what
/// `resources_by_stability()` yields, and the same plural under two groups is two resources, not
/// one repeated.
///
/// **An empty answer comes back empty, and it is not a cluster with no kinds in it.** The one
/// failure that reaches this function *as an answer* rather than as an `Err` is a server too old
/// for the aggregated call, which returns `Ok` with zero groups (failure 1 above); a refusal and
/// a dead API server never get here at all. Nothing here can tell those apart — the caller that
/// made the call is the only place that knows which happened.
pub fn browsable(
    served: impl IntoIterator<Item = (ApiResource, ApiCapabilities)>,
) -> Vec<Browsable> {
    let mut kinds: Vec<Browsable> = served
        .into_iter()
        .filter(|(_, capabilities)| capabilities.supports_operation(verbs::LIST))
        .map(ingest)
        .collect();
    kinds.sort_by(|one, two| {
        (&one.plural, &one.group, &one.version).cmp(&(&two.plural, &two.group, &two.version))
    });
    kinds
}

// --- EVERY KIND THE CLUSTER SERVES END ---

// --- THE BROWSER'S ROWS START ---
//
// **The columns come off the wire and are not written down here** (invariant 12,
// `screens/resources.md`). The API server prints a list the way `kubectl get` would — one
// `columnDefinitions` array, one `cells` array per row — and everything in this region either
// carries that answer or refuses to look inside it. A `match` on a kind anywhere below is the
// design failure invariant 12 names.
//
// **Measured off a live kind cluster (v1.36.1) on 2026-08-22 through `kubectl proxy`, with the
// Accept header below.** The two shapes are `tests/fixtures/table-pods.json` and
// `tests/fixtures/table-deployments.json`:
//
// | what | measured |
// |---|---|
// | top level | `{apiVersion: meta.k8s.io/v1, kind: Table, columnDefinitions, rows, metadata}` |
// | columns — pods / nodes / deployments | 9 / 10 / 8, of which 5 / 5 / 5 at `priority: 0` |
// | `priority > 0` | exactly what `kubectl -o wide` adds |
// | cell types across the deployments table | `["number", "string"]` |
// | `?includeObject=None` against the default | 7 339 vs 142 584 bytes, 14 kube-system pods |
//
// **A cell is not a string.** A Deployment's `Up-to-date` and `Available` arrive as JSON numbers,
// so `cells: Vec<String>` would have failed to deserialise on every Deployment table a real
// cluster serves. [`cell`] is the one place a cell becomes text.
//
// ## `includeObject`, and why the 19× is paid
//
// The default is `Metadata` — every row carries a whole `PartialObjectMetadata`, `managedFields`
// and all — and `?includeObject=None` sends `"object": null` instead, for a nineteenth of the
// bytes. **The default is kept**, because `screens/resources.md` spends the row's object twice and
// neither spend has a second source: a finding is matched onto a row *by name, namespace and uid*
// (the `●` marker, which is that screen's first rule), and every operation in `screens/dialogs.md`
// needs an explicitly selected object to name (invariant 2). The cells cannot supply them — the
// committed pods table has nine columns and none of them is a namespace, and **a cross-namespace
// list does not add one**. Measured 2026-08-22: `/api/v1/pods` — 53 rows drawn from three
// namespaces — comes back with the same nine columns as `/api/v1/namespaces/kube-system/pods`.
// `kubectl get pods -A` prints a `NAMESPACE` column because **kubectl prepends it client-side**;
// the server never sends one.
//
// **What that costs a screen is named here so the box that draws one inherits it rather than
// rediscovering it.** `Fetch::table(kind, None)` on a namespaced kind lists every namespace, so a
// screen drawing the `priority: 0` cells and nothing else shows six identical `kube-root-ca.crt`
// rows with a cursor sitting on one of them (`/api/v1/configmaps` on the same cluster, 14 rows,
// six of that name). **The identity is recoverable and that is exactly why the default is kept** —
// [`Row::namespace`] is right there under it — and it is **not** recoverable under
// `?includeObject=None`, which is how `tests/fixtures/table-deployments.json` was captured and why
// every one of its six rows, living in four namespaces, carries `namespace: None`.
//
// **Nothing of `managedFields` is *retained*, and that is not the same as nothing being paid.**
// The first draft of this paragraph said none of the 19× is paid in memory, and both halves of
// that were wrong.
//
// **Retained: nothing.** `managedFields` is not named by [`MetadataResponse`], so serde skips it —
// `serde_json-1.0.106/src/de.rs:1093` `ignore_value` keeps a depth stack of one byte per brace and
// `ignore_str` copies no characters at all. The prune is the decode, as it is for a watched object
// (see the module doc).
//
// **Paid transiently in memory, twice over.** `kube-client-4.2.0/src/client/mod.rs:298-299`
// collects the body to `Bytes` and then builds a `String` from `body_bytes.to_vec()` — a second
// whole copy alive at the same time — before `serde_json::from_str` at `:287` ever sees it. At the
// default `includeObject` that is two copies of the large body, not of the small one.
//
// **Paid in CPU, and here is the number.** Measured on the real `serde_json::from_str` path, best
// of 50 runs each: 12 204 bytes parses in **185.4 µs** and 676 056 bytes in **6.452 ms** — 55× the
// bytes for 34.8× the time, near enough linear. At this box's own 19× that is **roughly 12× the
// parse cost of every refresh**, at up to once a second per open view. **[`REFRESH_FLOOR`] bounds
// how often a request is made and not what one costs**, so it caps this at 1 Hz and does not
// reduce it.
//
// **`includeObject` is not sent, and the default is written down here instead of asked for.**
// Sending `?includeObject=Metadata` would put k8rs on a query kubectl does not use for the same
// answer, and [`Fetch::plain`] would then have to strip it again — `includeObject` is a `Table`
// option and means nothing to the object list a `406` falls back to. What would reverse this is one
// measurement of a re-fetch at cluster size, which no machine in this repo can take.
//
// ## The 406, and the half of it that is not proven
//
// **The Accept header carries `,application/json` so an ordinary server negotiates the plain list
// by itself**; the case the fallback exists for is an aggregated API server that refuses the whole
// header (`screens/resources.md`). **The kind cluster this region was measured on runs no
// aggregated API server**, so [`not_acceptable`] is proven against a `Status` this repo built and
// never against one a server sent. What a real 406 body contains is the half that stays unproven.
//
// **The predicate reads `Status::code`, and there is one shape it cannot see.** kube parses the
// error body into a `Status`, and stamps the HTTP code onto a synthetic one only when that parse
// fails (`kube-client-4.2.0/src/client/mod.rs:551-558`). Every field of `Status` is
// `#[serde(default)]`, so a `406` whose body is JSON that is *not* a `Status` parses as one with
// `code: 0` and the fallback does not fire. **Driven through kube's own branch with eight bodies**,
// the set that is missed is `{}`, an object with other keys such as `{"error": …}`, the RFC 7807
// problem-details shape, and — the one nobody would have guessed — **`[]`**, because serde builds
// an all-default struct out of a sequence just as happily. A plain-text or HTML body *does* fire,
// because that parse fails and kube stamps the real HTTP code. Closing the rest needs a server to
// ask.
//
// **The fallback can draw one column.** A plain object list carries no columns at all, and the only
// thing in one that is not per-kind is `metadata` — so [`Table::from_objects`] synthesises a `Name`
// column and nothing else: no Ready, no Status, no Age. Age is not an oversight, it is a clock, and
// this file does not read one (invariant 5, NOTES § D18); it would have to become a column of
// RFC 3339 text, which is not what the reader was looking at a moment before.
//
// **And the `406` is not the only way that list arrives.** The Accept header's own
// `,application/json` half means an ordinary server that cannot print a `Table` answers **200**
// with it, which [`not_acceptable`] never sees — so the branch that reads it is in the decode,
// on `kind`, and [`TableResponse`] is where that is written down.
//
// **How many rows and how many cells is deliberately not answered here.** [`text`] bounds each
// value and nothing bounds the number of them — the same ceiling [`Browsable`] carries, and the
// open collection-bound box in todo.md § Phase 5 is where a reader is told that a list was cut.
//
// **A Table can be paged and the server says how much is left, and neither fact reaches this
// file.** `?limit=5` answers `metadata.remainingItemCount: 48` and a `metadata.continue` token;
// the same call unpaged answers 53 rows and a `metadata` of nothing but `resourceVersion`
// (measured 2026-08-22). **[`TableResponse`] names no `metadata` field at all**, so the fetch is
// unpaged and nothing here can say *there are 4 947 more rows* or ask for the next page — a
// 5 000-row namespace is 5 000 rows decoded, held and drawn, which is also what
// [`REFRESH_FLOOR`]'s numbers are about. It is deliberately not added by this box: a field with
// no reader is one nobody can prove, the box that would read it has no phase yet, and **this file
// freezes after Phase 6** (todo.md § Phase 5, NOTES § D116) — so that box raises it before then,
// as [`Column`]'s dropped `type` must.
//
// **One thing in this region is a sink and not a display, and it is the only refusal in it.** A
// group, a version and a plural become a *URL path*, and § EVERY KIND THE CLUSTER SERVES is where
// they came from — an answer an aggregated API server writes. [`path_safe`] is that door.

/// **One word that is about to become part of a URL path** — a group, a version, a plural, or the
/// namespace a view is scoped to.
///
/// **The trust boundary is real** (§ EVERY KIND THE CLUSTER SERVES): `run_aggregated()` copies
/// `resources[].resource` straight into the plural with no check on it
/// (`kube-client-4.2.0/src/discovery/parse.rs:115-132`), so on a cluster with an aggregated
/// APIService registered, the string [`Fetch::table`] would interpolate into a path is chosen by
/// whoever runs that API server. A plural of `pods/../secrets` is a row labelled *widgets* that
/// lists Secrets with the reader's own credentials, and one containing `?` or `#` puts query
/// parameters on a call the command log prints without them, which is invariant 4's record lying.
/// It cannot reach a different *host* — the base URL is the kubeconfig's — and that is the whole
/// of what keeps it from being worse. **The same rule that covers a name building a filesystem
/// path covers a name building a URL** (the security gate); this is that sink.
///
/// **What is allowed is what every Kubernetes name already is**: ASCII alphanumerics, `-` and `.`,
/// beginning with an alphanumeric. An API group is a DNS subdomain and **may begin with a digit**,
/// so a leading letter is not required — requiring one would refuse a real CRD group. The empty
/// word is refused here and the empty *group* is allowed by [`Fetch::table`], because the core
/// group is spelled `""` and contributes no path segment.
///
/// **The namespace goes through it too**, and for a reason that is about time rather than about
/// trust: today it is `--namespace`, and the picker further down Phase 5 fills it from the
/// cluster's own list. A predicate that is applied to every word cannot be the one that was
/// forgotten when the source changed.
///
/// **A denylist of `/`, `..`, `?` and `#` was the other way to write this, and it is the wrong
/// one** — for invariant 1's reason one layer down. Percent-encoding, `;` parameters and whatever
/// the next URL parser disagrees about are all things a list of four characters does not know
/// about.
///
/// **Where this does *not* run is [`browsable`], and that is a question rather than a decision.**
/// Moving it there would drop such a kind from the sidebar entirely, which is the stronger answer
/// and also reverses two things the discovery box proved — a CRD that names itself with control
/// characters is *offered with its name stripped*, and a runaway plural is *offered shortened*,
/// and neither survives this predicate afterwards. What a screen should show for a row that cannot
/// be opened is not this file's to settle alone.
fn path_safe(word: &str) -> bool {
    word.starts_with(|character: char| character.is_ascii_alphanumeric())
        && word.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '.'
        })
}

/// **What the browser asks for**: the `Table` the API server would print for `kubectl get`, with
/// the plain object list as the negotiated second choice (`screens/resources.md`).
///
/// The two halves are not interchangeable: without `,application/json` a server with no `Table`
/// support answers `406` instead of the list, and without the `Table` half every screen would be
/// a hand-written column list, which is what invariant 12 refuses.
const TABLE_ACCEPT: &str = "application/json;as=Table;g=meta.k8s.io;v=v1,application/json";

/// What [`Fetch::plain`] asks for after a `406`: the ordinary object list, no `Table` at all.
const PLAIN_ACCEPT: &str = "application/json";

/// The one HTTP status that means *ask again without the `Table` header* (the region above).
const NOT_ACCEPTABLE: u16 = 406;

/// What the server calls the printed answer, and the whole of how [`TableResponse`] tells the two
/// shapes apart.
const TABLE_KIND: &str = "Table";

/// The column a plain object list is drawn as, spelled the way the API server spells its own
/// (`columnDefinitions[0].name` is `Name` in both committed tables). Casing on screen is
/// `views.rs`'s.
const NAME_COLUMN: &str = "Name";

/// **One browser fetch: where it goes, and what it will accept there.**
///
/// **Not an `http::Request`, and that is a choice with two halves.** The caller builds one from
/// this with `kube::core::Request::new(&fetch.path).list(&ListParams::default())` — `list` is on
/// invariant 1's allowlist and supplies the `GET` — and inserts [`Fetch::accept`]; that line is
/// `connect()`'s for § EVERY KIND THE CLUSTER SERVES's reason. The half that is *not* about
/// ownership: `http::Request` is not `Clone`, so a fallback derived from the request that was
/// refused could not be, and [`Fetch::plain`] is exactly that derivation. **The path a `406`
/// retries is structurally the path that was refused**, never one rebuilt beside it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetch {
    /// The URL path, built by kube from what discovery said —
    /// `/api/v1/namespaces/kube-system/pods`, `/apis/apps/v1/deployments`. No query string: the
    /// region above is why `includeObject` is absent.
    pub path: String,
    /// The `Accept` header, and the whole of what separates the two fetches.
    pub accept: &'static str,
}

impl Fetch {
    /// The `Table` fetch for one kind the cluster said it serves.
    ///
    /// **The path is kube's own**, through [`ApiResource::from_gvk_with_plural`] — the constructor
    /// that is *told* the plural rather than the one that guesses it ([`Browsable::plural`]) — so
    /// this cannot disagree with the URL an `Api<K>` would have built for the same kind.
    ///
    /// **A namespace is dropped for a kind that has none.** `namespaced` is discovery's own flag
    /// (invariant 12), and `/api/v1/namespaces/payments/nodes` is a path no server answers; the
    /// screen has the same condition for whether it draws the `ns:` label at all.
    ///
    /// **`None` is a word that cannot be a path segment** ([`path_safe`]) — the only refusal here,
    /// and the reason this is not infallible. **The namespace is judged by the same predicate**,
    /// and only after the line above has dropped it for a cluster-scoped kind, so a stray one
    /// cannot refuse a fetch that would never have carried it. An earlier draft exempted the
    /// namespace and reasoned from its source — *the caller typed it* — which holds only while
    /// that source is `--namespace`: the namespace picker further down Phase 5 is fed from the
    /// cluster's own list, and `x?watch=true` puts a query parameter on a call the command log
    /// prints without one.
    pub fn table(kind: &Browsable, namespace: Option<&str>) -> Option<Self> {
        let namespace = kind.namespaced.then_some(namespace).flatten();
        if !(kind.group.is_empty() || path_safe(&kind.group))
            || !path_safe(&kind.version)
            || !path_safe(&kind.plural)
            || namespace.is_some_and(|namespace| !path_safe(namespace))
        {
            return None;
        }
        let gvk = GroupVersionKind::gvk(&kind.group, &kind.version, &kind.kind);
        let resource = ApiResource::from_gvk_with_plural(&gvk, &kind.plural);
        Some(Self {
            path: DynamicObject::url_path(&resource, namespace),
            accept: TABLE_ACCEPT,
        })
    }

    /// The same list again, asked for as an ordinary object list — what a `406` falls back to.
    pub fn plain(&self) -> Self {
        Self {
            path: self.path.clone(),
            accept: PLAIN_ACCEPT,
        }
    }
}

/// **The server refused the `Table` header itself** — the one failure that is answered by asking
/// again rather than by telling the reader (the region above).
///
/// Everything else is the reader's news: a `403` on one kind is *this row cannot open*, a `404` is
/// a kind discovery still names and the cluster no longer serves. Neither is retried, which is
/// [`Store::unresolved_owners`]'s rule and holds here for its reason (NOTES § D151).
///
/// **Whoever reads that `404` keys on `Status::code` and never on `Status::reason`**, and the
/// deleted-CRD case is precisely the one that proves it. A group nobody serves answers with the
/// literal body `404 page not found` — not a `Status` at all — so kube builds
/// `Status::failure(text, "Failed to parse error data").with_code(404)`
/// (`kube-client-4.2.0/src/client/mod.rs:551-558`) and `.reason` is that phrase rather than
/// `NotFound`. A resource missing from a group the server *does* serve answers a real `Status`
/// with `reason: NotFound`. **The code is the same in both and the reason is not**, which is why
/// this predicate reads one field.
pub fn not_acceptable(failure: &kube::Error) -> bool {
    matches!(failure, kube::Error::Api(status) if status.code == NOT_ACCEPTABLE)
}

/// **One list as the API server itself would print it**, columns and all.
///
/// Nothing in this type names a kind, and nothing that builds one reads what kind it came from
/// (invariant 12). It derives `Debug` for [`Listing`]'s reason: **nothing k8rs puts in here is a
/// credential**, and neither a column header nor a printed cell is a field this file read off a
/// secret.
///
/// **That is a claim about our types and not about the cluster's choices, and the difference is
/// worth writing down.** It is true of the built-in printers — a `Table` for Pods or Deployments
/// prints what `kubectl get` prints. A CRD's columns are `additionalPrinterColumns`, a JSONPath
/// its author chose into a spec its author wrote, so a cell of one contains whatever that author
/// pointed it at. Nothing here can tell the two apart; what holds either way is that the value
/// went through [`text`] and is bounded, and that this type is `Debug` and not `Display` — it
/// reaches a log only where something writes one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    /// The server's `columnDefinitions`, in the order it sent them.
    pub columns: Vec<Column>,
    /// One row per object, each with at least one cell per column.
    pub rows: Vec<Row>,
}

/// One column the server said this kind is printed with.
///
/// **Three fields the wire carries are dropped: `type`, `format` and `description`.** Nothing
/// draws them today and a field with no reader is one nobody can prove ([`Browsable`] drops
/// discovery's `shortNames` for the same rule); `description` is also most of the response's
/// column bytes — 350 of them on `Name` alone. **`type` is the one a later box might want**, for
/// right-aligning a number, and `screens/resources.md` draws every column left-aligned today: it is
/// one word in [`ColumnResponse`] when a screen needs it, and **this file freezes after Phase 6**
/// (todo.md § Phase 5, NOTES § D116), so the box that wants it raises it before then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    /// The header the server chose — `Name`, `Ready`, `Up-to-date`. Cased for the screen by
    /// `views.rs`.
    pub name: String,
    /// **`0` is what plain `kubectl get` prints and anything above it is what `-o wide` adds.**
    /// `screens/resources.md` draws the `priority: 0` set and no more — without that filter every
    /// screen is the wide view. The filter is the screen's, because which columns fit is.
    pub priority: i32,
}

/// One object, as a row of printed cells and the identity underneath it.
///
/// **The identity is `None` on every row when the server was asked with `?includeObject=None`**,
/// which k8rs does not ask (the region above) but `tests/fixtures/table-deployments.json` is, so
/// the decode survives both shapes rather than assuming one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// **At least one cell per column**, in the columns' order. A server that sent fewer is padded
    /// with empty cells so a renderer cannot index past the end; a server that sent *more* keeps
    /// them, because cutting a collection is the open box named above and not this one's to decide.
    pub cells: Vec<String>,
    /// The namespace of the object this row is, when the server sent one.
    pub namespace: Option<String>,
    /// Its name — what a kubectl line and every dialog names.
    ///
    /// **Nothing judges it beyond [`text`]'s strip and its 512-byte bound, and the promise above
    /// is where that stops being enough.** A name is a DNS label on any cluster that made it, but
    /// this is the API server's word and not a rule this file enforces: a row named
    /// `web -n kube-system` builds the command-log line
    /// `kubectl delete pod web -n kube-system -n payments`, which is invariant 4's record lying
    /// about what ran. **There is no command log and no `ops.rs` yet, so this is not the box that
    /// fixes it** — it is named here because this doc comment is what promises those two uses.
    pub name: Option<String>,
    /// Its uid: what a finding is matched onto a row by, and what keeps a cursor on the same object
    /// when a re-fetch reorders the rows. **Not the name**, for [`Store`]'s reason — a name deleted
    /// and recreated is a different object.
    pub uid: Option<String>,
}

/// **The answer to a browser fetch, in either shape it can come back in** — and `kind` is what
/// says which.
///
/// **One type rather than two, because the choice is not the caller's to make.** The Accept header
/// asks for a `Table` and offers `,application/json` under it, so a server that cannot print one
/// answers **200** with the ordinary object list — no `406`, nothing for [`not_acceptable`] to
/// see. A type that named only `columnDefinitions` and `rows` decoded that body to *zero columns
/// and zero rows, with no error*: six Deployments in, an empty screen out. The `406` fallback
/// ([`Fetch::plain`]) sends the same body, so both paths land on the same branch.
///
/// **The branch reads `kind`, which is the server's own word for what it sent** — `Table`, or the
/// `List` / `PodList` / `DeploymentList` of anything else. **Its ceiling, named rather than left
/// to be found: a `Table` body carrying no `kind` at all reads as an empty list**, because the
/// `else` has to be one of the two. Every capture in this repo carries one and the API server sets
/// it on every response k8rs asks for, so a body without one is malformed rather than a third
/// shape this file chooses between.
///
/// Only what [`Table`] keeps is named, so the rest — every row's `managedFields` above all — is
/// walked past by serde. **Every field defaults and no unknown field is refused**, which is what
/// makes one decode cover both `includeObject` shapes and a server that adds a field later
/// (NOTES § D152 reads kube's own discovery type the same way).
#[derive(Deserialize)]
#[serde(crate = "k8s_openapi::serde", rename_all = "camelCase")]
pub struct TableResponse {
    /// `Table`, or the list kind of whatever the server printed instead.
    #[serde(default)]
    kind: String,
    /// **A watch event sends this once per stream and `[]` on every event after the first**
    /// (§ KEEPING A BROWSER VIEW FRESH, measured). Nothing breaks today — no watch event reaches
    /// this type — and the `resize` in the `From` impl below is a no-op against zero columns, so
    /// **a later box feeding events through it would draw the second event with no headers at
    /// all**. Whoever writes that box carries the first event's columns forward itself.
    #[serde(default)]
    column_definitions: Vec<ColumnResponse>,
    #[serde(default)]
    rows: Vec<RowResponse>,
    /// A plain object list's own objects — empty on a `Table`, and the only thing read when the
    /// server sent one of those instead.
    #[serde(default)]
    items: Vec<ObjectResponse>,
}

#[derive(Deserialize)]
#[serde(crate = "k8s_openapi::serde")]
struct ColumnResponse {
    #[serde(default)]
    name: String,
    #[serde(default)]
    priority: i32,
}

#[derive(Deserialize)]
#[serde(crate = "k8s_openapi::serde")]
struct RowResponse {
    /// **`Value` and not `String`**: a Deployment's table sends numbers (the region above).
    #[serde(default)]
    cells: Vec<Value>,
    /// `null` under `?includeObject=None`, absent on a server that sends neither.
    #[serde(default)]
    object: Option<ObjectResponse>,
}

/// A row's object under `includeObject=Metadata`, and an item of a plain object list — the same
/// two braces either way, so it is one type.
#[derive(Deserialize)]
#[serde(crate = "k8s_openapi::serde")]
struct ObjectResponse {
    #[serde(default)]
    metadata: MetadataResponse,
}

/// **The three fields of a row's object k8rs keeps**, and the whole reason `managedFields` costs
/// nothing to hold: it is not named here, so serde never builds it.
#[derive(Default, Deserialize)]
#[serde(crate = "k8s_openapi::serde")]
struct MetadataResponse {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    uid: Option<String>,
}

/// One cell as text.
///
/// **A string is kept as itself and everything else is its JSON**, which is `1` for a number and
/// `true` for a boolean rather than the quoted forms `Value::to_string` would give a string.
/// `null` is an empty cell — a server that sent nothing for a column has not sent the word "null".
/// An array or an object has no printed form the API defines, so it keeps its JSON and the guard
/// bounds it like anything else.
fn cell(value: Value) -> String {
    match value {
        Value::String(text) => text,
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

impl From<TableResponse> for Table {
    fn from(response: TableResponse) -> Self {
        if response.kind != TABLE_KIND {
            return Self::from_objects(response.items);
        }
        let columns: Vec<Column> = response
            .column_definitions
            .into_iter()
            .map(|column| Column {
                name: column.name,
                priority: column.priority,
            })
            .collect();
        let rows = response
            .rows
            .into_iter()
            .map(|row| {
                let mut cells: Vec<String> = row.cells.into_iter().map(cell).collect();
                // Padded and never cut: a renderer that walks the columns cannot index past the
                // end, and nothing the server sent is thrown away ([`Row::cells`]). `max` rather
                // than a `<` guard around the same `resize`, because the two spellings differ
                // only when the lengths are equal — where `resize` does nothing either way — and
                // a branch no input can tell apart is a branch no test can fail on.
                cells.resize(cells.len().max(columns.len()), String::new());
                let metadata = row.object.map(|object| object.metadata).unwrap_or_default();
                Row {
                    cells,
                    namespace: metadata.namespace,
                    name: metadata.name,
                    uid: metadata.uid,
                }
            })
            .collect();
        Self { columns, rows }
    }
}

impl Table {
    /// **A plain object list, drawn as the one column a list of anything can honestly be drawn
    /// as** (the region above): `metadata` is the only thing in an object that is not per-kind,
    /// and an Age column would need a clock this file does not read.
    fn from_objects(items: Vec<ObjectResponse>) -> Self {
        Self {
            columns: vec![Column {
                name: NAME_COLUMN.to_string(),
                priority: 0,
            }],
            rows: items
                .into_iter()
                .map(|item| Row {
                    cells: vec![item.metadata.name.clone().unwrap_or_default()],
                    namespace: item.metadata.namespace,
                    name: item.metadata.name,
                    uid: item.metadata.uid,
                })
                .collect(),
        }
    }
}

// --- THE BROWSER'S ROWS END ---

// --- KEEPING A BROWSER VIEW FRESH START ---
//
// **A `Table` can be watched, and is still not watched here** (`screens/resources.md`,
// NOTES § D154): `?watch=true` under the Accept header above answers `200` and streams `Table`
// objects. A browser view watches metadata instead — the smallest thing that says *something
// changed* — and re-fetches the Table.
//
// **The reason is kube's, and it is not the wire cost.** The first draft of this region argued a
// 37× overhead: *every event re-sends the entire column schema, 3 086 bytes of
// `columnDefinitions` to carry an 82-byte row*. **That was the first event of a fresh stream,
// generalised.** Measured on the same image it was written against (kind v1.36.1, 2026-08-22,
// 18 events off a pods Table watch): `columnDefinitions` is sent **once per stream** and is `[]`
// on every event after the first — event 1 carried 9 columns in 5 764 bytes, events 2–18 carried
// none, mean 3 062 bytes. A deployments watch: one event with 8 columns, ten with none.
//
// **So the shape below is the more expensive one on the wire, and it is chosen knowing that.**
// A Table watch event is ~3 062 bytes and *already carries the row's identity* — `rows[0].object`
// is the same `PartialObjectMetadata` a metadata watch sends. A metadata event is ~2 624 bytes,
// 14% smaller, **and then owes a whole Table re-fetch at 6 852 bytes per row**. One change in a
// 500-row namespace: ~3 KB the other way, 2.6 KB + 3.4 MB this way.
//
// **What that buys is `watcher`, and `watcher` is the part that is hard to get right.**
// `kube::runtime::watcher` takes `K: Resource + Clone + DeserializeOwned + Debug + Send`
// (`kube-runtime-4.2.0/src/watcher.rs:787`) and **kube has no `Table` type at all** — the string
// `as=Table` appears nowhere in `kube-core` or `kube-client` — because a Table is a *rendering*
// of a list and not a resource the API server serves. Streaming one means
// `Client::request_stream` / `request_events` (`kube-client-4.2.0/src/client/mod.rs:307,340`),
// which frame lines and decode them and do nothing else: **this file would own the
// `resourceVersion` bookkeeping, the `410 Gone` relist, and the `Event::Init` that relist has to
// emit** — three things `watcher` already does, and the middle one cannot be proven here without
// a server that expires a resourceVersion on demand. (**Backoff is not one of them**: there is
// none inside either entry point — `watcher.rs:26` and `:806` both tell the caller to apply its
// own — so it is owed the same either way and is no part of this trade.)
// **What would reverse it**: a `Table` stream with those guarantees, or a re-fetch measured at a
// size where 3.4 MB per change is the thing that hurts — the numbers on [`REFRESH_FLOOR`] are
// where that argument would start.
//
// **The watch is `connect()`'s and only the policy is here**, for § EVERY KIND THE CLUSTER
// SERVES's reason. The line, over the same [`ApiResource`] [`Fetch::table`] builds its path from:
//
// ```ignore
// let api: Api<PartialObjectMeta<DynamicObject>> = Api::all_with(client, &resource);
// watcher::watcher(api, watcher::Config::default())
// ```
//
// **Not `metadata_watcher`**, which this region named until the review compiled it:
// `watcher.rs:850` carries `#[deprecated(since = "3.1.0")]` and `just check` runs clippy with
// `-D warnings`, so the one line this region tells `connect()` to write would have failed the
// gate. `PartialObjectMeta<K>` takes `K::DynamicType` as its own — `metadata.rs:149-151` in
// `kube-core-4.2.0` — which for `DynamicObject` is an `ApiResource`, so `all_with` and
// `namespaced_with` work unchanged and [`Browsable::namespaced`] still picks between them. It is
// a line no test can fail on; *when* the re-fetch happens is not, and that is [`Browsing`].
//
// **A kind that can be listed and cannot be watched exists, and nothing here knows it.** Of the
// 42 resources this cluster advertises `list` on, exactly one does not advertise `watch`:
// `componentstatuses`, which answers `get,list` and is a built-in rather than a CRD.
// [`browsable`] filters on `list` alone, so it is offered, and a caller that opens a watch on it
// gets `405 MethodNotAllowed` — *watch is not supported on resources of kind
// "componentstatuses"* — with no state here to stop it retrying.
// [`Browsable::verbs`] is already carried, so the caller's check is
// `verbs.iter().any(|verb| verb == "watch")`; **no state for it is built here**, because what a
// screen offers instead — a manual refresh key — is a ruling nobody has made, and a field with no
// reader is one nobody can prove.
//
// **The permanent set does not grow here.** Invariant 6 watches five streams — Pods, Nodes and the
// three workload kinds, which are the Alerts view's inputs — and [`Store`] holds exactly those
// five and no sixth. [`Browsing`] is a plain value the caller owns and drops, holding no stream of
// its own: closing a view drops the one it opened, and forty permanent streams is the failure this
// shape exists to avoid (`screens/resources.md`). `k8s_tests.rs` derives that five off this file
// rather than trusting the sentence.
//
// **`PRIOR-ART § A5` is the defect this region is one step away from** — k9s merged *skip the
// reconcile when nothing changed* and reverted it a month later, because a coalescer that drops the
// last event of a burst shows stale data forever and passes every test that does not assert the
// state *after* the storm. **It has two halves and the first draft closed one.** The pending flag
// is cleared when a fetch is *issued*, never when it returns, so a change arriving mid-flight
// re-arms rather than being answered by a response that predates it — that is the first. The
// second is that nothing stopped a *second* fetch going out beside the first: three seconds of
// body, a change per floor, and HTTP/2 promising no ordering, so the older answer can land last
// and leave the view on pre-change rows with nothing pending to re-arm. [`Browsing::done`] is the
// second half — one fetch at a time, and the floor measured from the answer (NOTES § D154).

/// **The shortest gap between one Table answer and the next Table question: one second.**
///
/// **A floor between fetches, not a delay before one.** A view that has been quiet re-fetches the
/// instant something changes, and a rollout that emits hundreds of metadata events costs one list
/// per second instead of hundreds. A plain debounce — wait for quiet, then fetch — would have the
/// opposite failure: while a deploy is rolling there *is* no quiet, so it would not fire at all.
///
/// **It is the lower bound and not the period.** The gap is measured from the moment a fetch came
/// back ([`Browsing::done`]), and only one is ever on the wire, so the real cycle is *this plus
/// however long the last fetch took*: a 30 ms answer refreshes at 1 Hz, a 3 s answer every 4 s.
/// **The cluster tunes it, and there is no second constant** — which is what makes one fixed
/// number defensible at every size, because the size is what moves the other half.
///
/// **What one refresh costs, measured** (kind v1.36.1, 2026-08-22): **6 852 bytes per row** on the
/// wire at the default `includeObject` (§ THE BROWSER'S ROWS is why the default is kept), and a
/// parse that is near-linear — 12 204 bytes in **185.4 µs**, 676 056 bytes in **6.452 ms**. So a
/// refresh is ~343 KB at 50 rows, ~3.4 MB at 500, ~34 MB at 5 000. **At a fixed 1 Hz that last one
/// is 34 MB/s out of a single open view**, which is the class of cost `PRIOR-ART § A` collects
/// the complaints about k9s's poll loop for.
///
/// **What the paragraph above fixes is the pile-up, and it is honest about what it does not
/// fix.** One fetch at a time means a slow answer cannot queue three more behind it, so the
/// worst case is one refresh in flight rather than a growing pile — that is the unbounded half.
/// Whether a 34 MB list comes back in well under a second is the cluster's answer and not this
/// constant's, and where it does, 1 Hz still costs 34 MB/s. **The half that bounds *that* is
/// paging**, which § THE BROWSER'S ROWS names as absent and the open collection-bound box in
/// todo.md § Phase 5 is what closes.
///
/// **Invariant 7's ~100 ms is not this number**: that one coalesces *paints*, and a paint costs
/// nothing on the wire.
const REFRESH_FLOOR: SignedDuration = SignedDuration::from_secs(1);

/// **One open browser view, and the whole of when its list is re-read.**
///
/// **It holds no stream and no client**, which is what makes *a closed view drops its stream* the
/// caller's one line rather than this type's: the metadata watch's stream lives beside this value
/// and is dropped with it (the region above). Nothing here can outlive a view or keep one open.
///
/// **It derives `Debug`** for [`Listing`]'s reason — a kind, a path and a timestamp never touched
/// a credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Browsing {
    /// The kind being browsed, exactly as discovery described it.
    kind: Browsable,
    /// **The one request this view ever makes, built once at open.** The kind and the namespace do
    /// not change while a view is open — switching either opens a new one, because the fetch this
    /// one is halfway through is for the old scope — so there is nothing to rebuild per refresh,
    /// and [`path_safe`] is judged once rather than on every poll.
    fetch: Fetch,
    /// **Something arrived on the metadata watch that this view's rows do not yet show.** Set by
    /// [`Browsing::changed`] and cleared the moment a fetch is *issued* — never when one returns
    /// (`PRIOR-ART § A5`, the region above).
    stale: bool,
    /// **A fetch this view handed out has not been answered yet.** The other half of
    /// `PRIOR-ART § A5`, and what makes at most one Table of this view exist at a time.
    outstanding: bool,
    /// When the last fetch *came back*, or `None` for a view that has never completed one — which
    /// is what makes the first fetch owed immediately. **Not when one was issued**: the floor is a
    /// gap between an answer and the next question (the region above).
    returned: Option<Time>,
}

impl Browsing {
    /// A view just opened: its first Table is owed at once.
    ///
    /// **`None` is a kind that cannot be browsed at all** — [`Fetch::table`]'s one refusal, taken
    /// here so that a view which exists is a view that can fetch, and [`Browsing::issue`] can
    /// answer `None` for exactly one reason.
    pub fn open(kind: Browsable, namespace: Option<&str>) -> Option<Self> {
        Some(Self {
            fetch: Fetch::table(&kind, namespace)?,
            kind,
            stale: false,
            outstanding: false,
            returned: None,
        })
    }

    /// The kind this view is showing — what a title and a kubectl line are built from.
    pub fn kind(&self) -> &Browsable {
        &self.kind
    }

    /// **Something changed on the metadata watch.** No clock: the floor is measured from the last
    /// fetch that came back, not from the change, so the moment the change arrived is not a fact
    /// this needs.
    pub fn changed(&mut self) {
        self.stale = true;
    }

    /// **The fetch [`Browsing::issue`] handed out is off the wire** — and this is where the floor
    /// starts (NOTES § D154, the region above).
    ///
    /// **Owed for every fetch that was issued, whatever became of it.** A refusal, a `404` and a
    /// dead socket are answers too: this says *the request is finished*, not *it worked*. A caller
    /// that drops one without saying so freezes its own view, which is the ceiling this shape has
    /// and the reason it is stated here rather than found later.
    ///
    /// **A failure does not re-arm anything.** [`Browsing::stale`] was cleared at issue, so a view
    /// whose fetch failed shows what it had until the next change — the rule
    /// [`Store::unresolved_owners`] holds to, for its reason (NOTES § D151): retrying a `403` at
    /// the floor is a loop the security gate refuses. A caller that *wants* the retry says
    /// [`Browsing::changed`], which is the same word the watch uses and costs one fetch per floor.
    pub fn done(&mut self, now: &Time) {
        self.outstanding = false;
        self.returned = Some(now.clone());
    }

    /// **When [`Browsing::issue`] will next hand out a fetch** — `now` for one already owed, a
    /// future moment while [`REFRESH_FLOOR`] holds one back, and `None` when there is nothing to
    /// wake up for: nothing has changed, **or a fetch is already on the wire** and the answer to
    /// it is the event that moves this on.
    ///
    /// The loop that drives a view sleeps on this: with a screen that draws on events (invariant 7)
    /// and nothing else, a fetch held back by the floor would otherwise wait for the next unrelated
    /// event to release it. It is the same shape [`Listing`] names — the state is readable here and
    /// something above still has to ask.
    pub fn due_at(&self, now: &Time) -> Option<Time> {
        if self.outstanding {
            return None;
        }
        match &self.returned {
            None => Some(now.clone()),
            Some(_) if !self.stale => None,
            // `checked_add` and not `+`, for NOTES § D54's reason. The only input that can
            // overflow it is a clock reading within a second of the end of time, and the fallback
            // is *due now* rather than *never again*, so the degenerate case still redraws.
            Some(last) => Some(Time(last.0.checked_add(REFRESH_FLOOR).unwrap_or(now.0))),
        }
    }

    /// **The fetch this view owes, if it owes one — and asking issues it.** `None` means *nothing
    /// to fetch yet* — nothing changed, the floor has not passed, or the last fetch has not been
    /// answered — and never *this view cannot fetch*: a kind that could never be fetched was
    /// refused at [`Browsing::open`].
    ///
    /// `&mut` and a verb rather than a question, because the two halves cannot be allowed to come
    /// apart: a caller that read *due* and then fetched without saying so would re-fetch on every
    /// poll for a whole floor, and one that never says [`Browsing::done`] gets no second fetch at
    /// all.
    pub fn issue(&mut self, now: &Time) -> Option<Fetch> {
        let due = self.due_at(now)?;
        if now.0 < due.0 {
            return None;
        }
        self.stale = false;
        self.outstanding = true;
        Some(self.fetch.clone())
    }
}

// --- KEEPING A BROWSER VIEW FRESH END ---
