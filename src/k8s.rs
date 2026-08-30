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
//! **The same answer says which optional pieces the cluster brought with it**
//! (§ WHAT ELSE THE CLUSTER SERVES, NOTES § Capability probe). [`capabilities`] is a read of the
//! discovery result and not a call, so metrics-server, PodDisruptionBudgets, cert-manager,
//! kube-prometheus-stack and the three meshes cost nothing to ask about. It answers *present*,
//! never *permitted* and never *where* — and it distinguishes **nothing was discovered** from
//! **this is absent**, because the second is a sentence a screen puts in front of a reader and
//! the first is not a fact anybody has.
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
//! **[`connect`] is the one place that meets an API server** (§ CONNECTING, NOTES § D16). It is a
//! function and not a step in `main` because the Phase 11 context switcher is the same call made
//! again over a dropped [`Session`]; it builds the client from the kubeconfig and nowhere else,
//! reads one discovery answer for both the sidebar and [`capabilities`], and hands back the five
//! watch streams with the backoff `PRIOR-ART § B3` asks for already on them. Only what cannot be
//! connected *with* is an error there: a question the server refuses travels as a `Result` inside
//! the session, because a kubeconfig that may not `get /apis` still watches pods.

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
    CertificateRequestSnapshot, ClaimSnapshot, ClusterSnapshot, Condition, ContainerRole,
    ContainerSnapshot, ContainerState, DisruptionBudgetSnapshot, EndpointSliceSnapshot, ExitRule,
    HostPathMount, Metrics, NodeSnapshot, NodeUsage, ObjectId, ObjectKind, PodSnapshot, Selector,
    SelectorRequirement, ServiceSnapshot, Taint, Terminated, Toleration, WorkloadSnapshot,
    expires_at, minor_version,
};
use futures_util::io::{AsyncBufRead, AsyncRead, AsyncReadExt};
use futures_util::stream::{self, BoxStream, Stream, StreamExt, select_all};
use k8s_openapi::NamespaceResourceScope;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::certificates::v1::CertificateSigningRequest;
use k8s_openapi::api::core::v1::{Node, PersistentVolumeClaim, Pod, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{APIGroupList, Time};
use k8s_openapi::jiff::fmt::rfc2822::DateTimeParser;
use k8s_openapi::jiff::{SignedDuration, Timestamp};
// `serde` and `serde_json` are reached through `k8s-openapi`'s own re-exports rather than named a
// second time in `Cargo.toml` — the same door `jiff` already comes through above, and invariant
// 10's narrowest possible answer: a crate already in the build, not even needing to be named.
use k8s_openapi::serde::Deserialize;
use k8s_openapi::serde_json::Value;
use kube::api::LogParams;
use kube::client::{Body, ConfigExt};
use kube::config::{AuthInfo, Config, KubeConfigOptions, Kubeconfig};
use kube::core::params::{GetParams, ListParams};
use kube::core::response::reason;
use kube::core::{DynamicObject, Request, Status, gvk::GroupVersionKind};
use kube::discovery::{ApiCapabilities, ApiGroup, ApiResource, Discovery, Scope, verbs};
use kube::runtime::WatchStreamExt;
use kube::runtime::utils::Backoff;
use kube::runtime::watcher::{self, Event, watcher};
use kube::{Api, Client, Resource};
use std::collections::{BTreeMap, BTreeSet};
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::ServerName;

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
// **The one string this file keeps that does not come through here is [`Trouble::failure`]**,
// which is kube's `watcher::Error` and not a `String` this file owns. The reconnect box gave it
// per-watch identity (NOTES § D162) and did not change that: it is still kube's type, and the
// instruction it carries is still that whatever renders it strips it first.
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
// collection *lengths*, and [`Trouble::failure`] — are NOTES § D146.

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

/// **The one snapshot type here that no watch decodes** (invariant 6 — CSRs are fetched when a
/// report asks for them, § WHAT A REPORT ASKS FOR).
///
/// **It is here anyway, and the reason is that it was not.** The fixture path in `main.rs` decodes
/// a `CertificateSigningRequest` straight through `rules.rs`'s `From`, and until the live fetch
/// landed there was no second path — so nothing carried this type past [`ingest`] and its absence
/// cost nothing. A live fetch that skipped this door would put `spec.signerName` and a condition's
/// `message`, both free text the API server wrote, on a screen unstripped and unbounded
/// (invariant 9, the security gate's *sizes are bounded*).
///
/// **`issued` is a `bool` and has nothing to bound**, which is
/// [`CertificateRequestSnapshot::issued`]'s own point: the bit is kept and the certificate's bytes
/// are not.
impl Bounded for CertificateRequestSnapshot {
    fn bound(&mut self) {
        self.id.bound();
        text(&mut self.signer_name, IDENTIFIER);
        self.conditions.bound();
    }
}

/// **The four types the five report fetches decode into that no watch also decodes**
/// (§ WHAT A REPORT ASKS FOR) — the same door
/// [`Bounded for CertificateRequestSnapshot`](CertificateRequestSnapshot) came through and for the
/// same reason: a fetch that skipped it would put a label a user chose and a `reason` the
/// disruption controller wrote onto a screen unstripped and unbounded (invariant 9, the security
/// gate's *sizes are bounded*).
///
/// **Four impls for five fetches, and the missing one is deliberate.** A ReplicaSet fetched for
/// Waste's *parked at 0 replicas* row decodes into [`WorkloadSnapshot`], which the workload watch
/// already brought through this guard — which is the point of `rules.rs` giving four kinds one
/// type.
///
/// **A selector's keys are bounded as well as its values.** A label key is
/// `prefix/name` and the prefix is a DNS subdomain the object's author chose; `pairs` caps both
/// halves, and a guard that bounded only the value would leave the half that is drawn first.
impl Bounded for ServiceSnapshot {
    fn bound(&mut self) {
        self.id.bound();
        pairs(&mut self.selector, IDENTIFIER);
    }
}

impl Bounded for EndpointSliceSnapshot {
    fn bound(&mut self) {
        self.id.bound();
        // `kubernetes.io/service-name`'s *value*, which is a Service name and an identifier.
        // The count beside it is a `usize` and has nothing to strip.
        maybe(&mut self.service, IDENTIFIER);
    }
}

impl Bounded for ClaimSnapshot {
    fn bound(&mut self) {
        self.id.bound();
        maybe(&mut self.phase, IDENTIFIER);
        // A quantity, like every other one here: the API's own string, kept as a word
        // ([`ClaimSnapshot::capacity`]).
        maybe(&mut self.capacity, IDENTIFIER);
    }
}

impl Bounded for DisruptionBudgetSnapshot {
    fn bound(&mut self) {
        self.id.bound();
        self.selector.bound();
        // `DisruptionAllowed`'s `reason` is what separates *the workload is at its floor* from
        // *the controller could not compute this at all*, and its `message` is a sentence
        // ([`Condition`]'s impl caps them apart).
        self.conditions.bound();
    }
}

/// **What one node reported it is using**, from the one call in this file that is *polled*
/// (§ WHAT A NODE IS USING).
///
/// **Both are [`IDENTIFIER`]s, like every other quantity this guard sees** — the values measured
/// off a live metrics API are `76530604n` and `1107584Ki` (§ WHAT A NODE IS USING has the
/// reading) — words a reader scans rather than sentences. `analysis.rs` parses them with
/// `rules::quantity_milli` and prints the number, so nothing unbounded is drawn either way; what
/// the cap stops is a metrics API answering something that is not a quantity and putting a
/// megabyte on the heap per node.
impl Bounded for NodeUsage {
    fn bound(&mut self) {
        text(&mut self.cpu, IDENTIFIER);
        text(&mut self.memory, IDENTIFIER);
    }
}

/// **[`pairs`]'s shape, for a map whose value is a type rather than a string** — the key is a
/// node name the cluster wrote, so the map is rebuilt for that function's reason, and two keys
/// that cut to the same bytes collapse the same way.
///
/// **Only [`Metrics::Read`] carries anything the cluster wrote.** The other three arms are this
/// file's own classification of a call that did not answer with usage (§ WHAT A NODE IS USING),
/// and hold no string at all.
impl Bounded for Metrics {
    fn bound(&mut self) {
        let Metrics::Read(nodes) = self else { return };
        let mut bounded = BTreeMap::new();
        for (mut name, mut usage) in std::mem::take(nodes) {
            text(&mut name, IDENTIFIER);
            // **The strip can empty a name that was not empty**, and this is the only place that
            // can happen: `\u{200b}\u{feff}` is a name the decode accepts and [`text`] removes
            // whole. Dropping it here is what makes [`NodeMetricsList`]'s *no entry under the
            // empty string* true of the map a reader gets rather than only of the map the decode
            // built — two hostile names strip to the same `""` and collide, which is the
            // collision that doc forbids arriving one function later (`tester`'s F2, 2026-08-29).
            if name.is_empty() {
                continue;
            }
            usage.bound();
            bounded.entry(name).or_insert(usage);
        }
        *nodes = bounded;
    }
}

/// **A label selector is free text twice over** — a `matchLabels` key and value, and a
/// `matchExpressions` entry's key, operator and every value in its list.
///
/// **`operator` is bounded rather than validated.** `rules.rs` keeps it as the API's own string
/// on purpose — *"a reader that does not know an operator must match nothing and say so"* — so
/// this layer's job is the strip and the cap, not the vocabulary.
impl Bounded for Selector {
    fn bound(&mut self) {
        pairs(&mut self.match_labels, IDENTIFIER);
        self.match_expressions.bound();
    }
}

impl Bounded for SelectorRequirement {
    fn bound(&mut self) {
        text(&mut self.key, IDENTIFIER);
        text(&mut self.operator, IDENTIFIER);
        for value in &mut self.values {
            text(value, IDENTIFIER);
        }
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

// --- WHAT WENT WRONG START ---
//
// **Every failed call in this file is classified here, and nowhere else** (todo.md § Phase 5,
// `PRIOR-ART § C1`). k9s tells `401` from `403` internally and still prints
// `Ruroh? 'v1/pods' command not found` when a credential expires, because a generic handler
// between the call and the screen replaced the typed error — its own log had the truth three
// lines earlier. The defect is not the wording, which is invariant 14's; it is that a *second*
// place decided what an error meant.
//
// **So there is one [`Fault`], one [`fault`], and every site holding a typed error reads them.**
// [`NotConnected`], [`Trouble::failure`], [`Session::version`] and [`Session::served`] each
// carried an error nobody interpreted. § RESOLVING AN OWNER carried a *second* classifier,
// `why()`, and it is **deleted rather than left beside this one**: it read a `401` as *k8rs could
// not ask*, which is C1 in miniature, and two functions reading one error and disagreeing is the
// defect class this repo has paid most for (CLAUDE.md § step 6). `Unresolved::why` is now an
// `Option<Fault>` whose `None` is the old `NotAsked` — nothing has asked yet is the one state
// that is not a failure at all.
//
// **A `Fault` is a fact and never a sentence, and it carries no string whatever.** The words are
// the caller's, exactly as for [`Listing`], [`Trouble`] and [`Unresolved`]. Six unit variants is
// invariant 9 made structural (NOTES § D160): nothing the API server wrote can reach a screen
// through this type, so no reader of it has to remember to strip anything.
//
// **The 403 names its verb and resource at the call site and not here.** The security gate
// requires a refusal to name them and there is exactly one place that knows: `get replicasets`,
// `get /version`, `get /apis` and `list`/`watch` *pods* are constants of the sites that ask, so a
// field for them would be a second copy of a value with one possible content — the deleted
// `Why::Refused` said so first and this inherits it. It is also the only shape that can carry
// NOTES § D160's `nonResourceURL` refusal, whose measured `Status` has an **empty `details`**:
// a formatter reading `details.group`/`details.kind` prints an empty sentence there, and the
// only true one names the path.
//
// **The mid-session credential failure, measured on the built binary** — the shape this
// region's own doc got wrong twice before it was produced. A kubeconfig whose `exec` program
// answers once, with a credential already inside kube's sixty-second refresh window, and exits 1
// on every call after that (2026-08-27):
//
// ```text
// before  ▲ k8rs is not getting pods from this cluster: nothing usable came back when k8rs
//           tried to `list` and `watch` pods.
// after   ▲ k8rs is not getting pods from this cluster: the program this kubeconfig logs in
//           with (`…/flaky-login`) gave k8rs nothing to sign in with.
// ```
//
// A network sentence for a failure on the reader's own machine is `PRIOR-ART § C1` inside the box
// written to close it. [`fault`]'s `Service` arm is the fix and carries the mechanism.
//
// **What this cannot see is written down at [`answer`] rather than left for a reader to
// discover**: a refusal whose body is JSON that is not a `Status` arrives with its HTTP code
// already gone, and comes out *nothing usable came back*. Measured on the built binary, not
// reasoned — an authorizing proxy is both the case the taxonomy was written for and the case
// most likely to answer JSON.
//
// **Select, never format, is unchanged and this region is the selection.** `Display` on a `kube`
// error interpolates its source down to an `exec` plugin's stdout ([`Trouble::failure`] carries
// the chain, `docs/security.md` § Token hygiene). What is read here is `Status::code` and
// `Status::reason` and what comes back is an enum, so a caller that goes through this never
// touches the text — and [`Session::renewal`] is the one string a screen may name beside a
// [`Fault::Expired`], read off the reader's own kubeconfig rather than off anything the cluster
// sent.

/// **What one failed call actually says** — six facts, no sentence and no string.
///
/// Ordered as the reader meets them: the four that never reached the cluster, then the three the
/// cluster answered, then everything that produced no usable answer at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// **The kubeconfig file itself** — not found, unreadable, or not valid YAML. Nothing was
    /// sent anywhere, so this answer contains no fact about any cluster.
    ///
    /// **It is the file and nothing else, which it was not until 2026-08-27** (`k8s-admin`). One
    /// variant covered all of `KubeconfigError` and [`because`](crate::because) returned one
    /// constant over it, so *"the kubeconfig could not be read, or names no such context"* was
    /// printed for a `client-certificate` path that had moved and for a cluster entry with no
    /// `server:` line — **both measured, and both false**: the file read perfectly and the
    /// context was there. That is `PRIOR-ART § C1` through a second door, in the box written to
    /// close it, so the arm is three faults now and each is something different to go and fix.
    Kubeconfig,
    /// **The file loaded and does not have the context k8rs was asked for** —
    /// `KubeconfigError::CurrentContextNotSet` or `LoadContext`.
    ///
    /// **Two causes, one sentence, and that is a judgement rather than an oversight**: a
    /// `--context` naming something the file does not have, and a file with no `current-context:`
    /// to fall back on, are both *k8rs does not know which cluster you mean* and both are fixed
    /// in the same two places. **The name is not carried** — it is argv and could be shown, but
    /// the reader typed it a second ago and plumbing it through would buy a word.
    NoContext,
    /// **The file and the context are both fine and something they point at is not** — a
    /// `client-certificate` path that is not on the disk, a cluster entry with no `server:`, a
    /// URL that will not parse, a context naming a cluster the file does not define.
    ///
    /// **This is the 3am one and it had no sentence of its own.** Nothing is wrong with the file
    /// as a file, so telling the reader to check whether it is readable sends them to `cat` a
    /// kubeconfig that is perfectly fine.
    BadEntry,
    /// **The kubeconfig names a program to log in with, and that program produced no
    /// credential** — it is not on the disk, it exited non-zero, or what it printed was not an
    /// `ExecCredential`. **Nothing was sent to the cluster**, so this is neither *refused* nor
    /// *nothing answered*, and
    /// `a_credential_plugin_that_never_answers_is_a_client_that_could_not_be_built` is the shape
    /// it was measured on.
    ///
    /// **Reachable at connect and mid-session, and the second half took an arm of its own** —
    /// this doc claimed `Client::send`'s downcast would deliver it and that claim was **produced
    /// and refuted** (`k8s-admin`, 2026-08-27, then measured again here). A login program that
    /// answers once and then exits 1 printed *nothing usable came back* — a network sentence for
    /// a failure on the reader's own machine, which is `PRIOR-ART § C1` inside the box written to
    /// close it. The auth layer boxes `auth::Error`, never `kube::Error`, so `Client::send`'s
    /// downcast misses and `unwrap_or_else(Error::Service)` fires ([`fault`] has the line
    /// numbers and the arm that fixes it).
    ///
    /// **The same arm covers `UnrefreshableTokenResponse`** (`auth/mod.rs:224-226`) — the plugin
    /// that stops returning an `expirationTimestamp`, whose credential kube can no longer refresh
    /// — which travelled the same path and read the same wrong way.
    ///
    /// **The failure is on the reader's own machine and nothing about the cluster is wrong**,
    /// which is why it is worth a variant of its own: this is the one code-execution path in the
    /// whole trust model (NOTES § D19).
    NoCredential,
    /// **A credential reached the server and the server no longer accepts it** — `401`.
    ///
    /// **The ordinary case on EKS, GKE and AKS** (NOTES § D19): those kubeconfigs hold no token,
    /// they name a binary that mints a short-lived one, and it expires *during* a session. It is
    /// not [`Refused`](Fault::Refused) — telling a beginner *you are not allowed to list pods*
    /// when the truth is *your login timed out* sends them to their platform team for nothing —
    /// and it is not [`Unanswered`](Fault::Unanswered), because the server answered.
    ///
    /// [`Session::renewal`] is what a screen may name beside it.
    ///
    /// # Not kube's `reason::EXPIRED`, which this file also imports
    ///
    /// **They are 200 lines apart and mean opposite things.** `kube::core::response::reason`'s
    /// `EXPIRED` is `"Expired"` on a `410` — *the `resourceVersion` you asked from is too old*,
    /// which a watch answers by re-listing and which is routine desync, not a credential.
    /// [`answer`] matches it on neither a code arm nor a reason arm, so it comes out
    /// [`Unanswered`](Fault::Unanswered) for the second before `InitDone` clears the line; that
    /// behaviour is boxed in `backlog.md` (`k8s-admin`, 2026-08-27) and is **not** to be closed
    /// by routing `reason::EXPIRED` here. A relist is not a dead login, and wiring the two words
    /// together would tell somebody to go and sign in again because their watch caught up.
    Expired,
    /// **The server knows who this is and will not allow it** — `403`. One feature degraded and
    /// never a session that failed (§ CONNECTING); the caller names the verb and the resource,
    /// because the caller is what knows them.
    Refused,
    /// **The server has nothing to answer with** — `404`.
    ///
    /// **On a ReplicaSet fetch this is ordinary traffic rather than a fault**: every rollout
    /// deletes the ReplicaSet it replaced, and a pod read a moment earlier can name one that is
    /// already gone (§ RESOLVING AN OWNER).
    Gone,
    /// **Nothing usable came back** — a socket that died, a timeout, a `5xx`, a `429` that
    /// outlived kube's retries, a body that would not decode, a watch answer with no
    /// `resourceVersion`, a proxy protocol kube will not speak.
    ///
    /// **One variant for all of them on purpose.** From the reader's side they are one fact —
    /// *k8rs could not ask* — and NOTES § D148 is why nothing here could tell them apart anyway:
    /// the wait happens below the client in a tower layer with no callback and no counter.
    Unanswered,
}

impl Fault {
    /// **Retrying cannot clear this one, so waiting for it is waiting for a person**
    /// (todo.md § Phase 5, `PRIOR-ART § B4`).
    ///
    /// **The one question the bootstrap gate has to ask, and the only place it is answered**
    /// ([`Watch::settled`]). D28 says a partial list may not be published, and until this
    /// existed it was read as *nothing may be published while any watch has not listed* — which
    /// for a watch the cluster **refuses** is forever, because the retry under it never stops
    /// and never succeeds. A `nodes` watch is cluster-scoped and cannot be granted by a
    /// namespaced `Role`, so that is the ordinary enterprise developer and not an edge case: the
    /// tool showed *loading* for the life of the process while `kubectl get pods -n mine`
    /// answered instantly beside it.
    ///
    /// **The line is *will the retry loop, unaided, clear this* and nothing else.** Only
    /// [`Unanswered`](Fault::Unanswered) is on the other side of it — a socket that died, a
    /// timeout, a `5xx`, a `429` — and it is exactly the case D28's *do not blank on a blip*
    /// paragraph was written about: the next poll may well answer, so a watch that is merely
    /// retrying keeps the gate shut and the driver prints [`Trouble`] lines meanwhile.
    ///
    /// **Everything else needs a human before the next attempt can go differently** — RBAC
    /// edited, a login renewed, a `exec` program repaired, a kubeconfig fixed, a kind that this
    /// server does not serve. k8rs waiting for it is k8rs waiting for something that is not
    /// happening on this connection.
    ///
    /// **Exhaustive and no `_` arm**, for [`kubeconfig_fault`]'s reason one region up: the
    /// default a catch-all would pick is *keep the whole tool blank*, and a variant added later
    /// should stop the build rather than inherit that.
    pub(crate) fn standing(self) -> bool {
        match self {
            Fault::Unanswered => false,
            // The three kubeconfig faults cannot reach a watch — the client is already built by
            // then — and they are here because the match is exhaustive, not because a watch can
            // raise one. Each is still a fact a retry cannot change.
            Fault::Kubeconfig
            | Fault::NoContext
            | Fault::BadEntry
            | Fault::NoCredential
            | Fault::Expired
            | Fault::Refused
            | Fault::Gone => true,
        }
    }
}

/// **What one `Status` says.**
///
/// **The HTTP code decides and `reason` is the fallback, because each is absent in a shape the
/// other covers.** kube parses a 4xx/5xx body into a `Status`; when that parse *fails* it builds
/// `Status::failure(&text, "Failed to parse error data").with_code(status.as_u16())`
/// (`kube-client-4.2.0/src/client/mod.rs:551-558`). **The code is the real HTTP one and never a
/// constant, and the reason is set — to kube's own placeholder** (`Status::failure` writes it,
/// `kube-core-4.2.0/src/response.rs:79-88`). So that shape carries a true number beside a word
/// no server ever sent, which is precisely why the number is asked first. An earlier draft of
/// this comment said `.with_code(403)` and *a code and no reason*; both were read off a call site
/// instead of the definition (`tester`, 2026-08-27).
///
/// The other way round is a `Status` body with no `code`: every field of `Status` is
/// `#[serde(default)]`.
///
/// **kube's own helpers answer neither.** `Status::is_forbidden` is
/// `self.reason == reason || (!is_known(reason) && self.code == code)`, and the `reason` handed to
/// `is_known` is the *constant* the helper was called with — always known — so the `code` half is
/// dead. Go's original tests the **response's** reason there.
/// `a_refusal_that_carries_only_a_status_code_is_still_a_refusal` measures it against the crate.
///
/// # The shape this cannot see, measured rather than reasoned
///
/// **A `403` whose body is JSON that is not a `Status` reaches here as `code: 0, reason: ""` and
/// is classified [`Fault::Unanswered`].** Every field of `Status` is `#[serde(default)]` and
/// nothing denies unknown fields, so `{"error":"forbidden by policy"}` — and `{}` — *parse
/// successfully* into an all-default `Status`, kube's `with_code` fallback never runs, and the
/// HTTP status is gone before this function is called. **The built binary against a local
/// listener answering `403` to everything, 2026-08-27:**
///
/// ```text
/// {"error":"forbidden by policy"}   nothing usable came back when k8rs tried to `get /version`
/// {}                                nothing usable came back when k8rs tried to `get /version`
/// Forbidden          (text/plain)   this kubeconfig is not allowed to `get /version`
/// a real v1 Status   (application/json)  this kubeconfig is not allowed to `get /version`
/// ```
///
/// **So kube's fallback fires only for a body that is not JSON at all**, and the region's own
/// example — an authorizing proxy — is the case most likely to answer JSON: oauth2-proxy, an
/// auth-annotated ingress and an API gateway all do. `a_json_body_that_is_not_a_status_loses_its_
/// http_code_inside_kube` pins it so this cannot quietly become a claim again.
///
/// **It is not recoverable from a `kube::Error`.** `Error::Api` carries the parsed `Status` and
/// nothing else — no response, no code, no headers — so there is no field left to read. The
/// degenerate parse *is* detectable — `code == 0`, an empty `reason`, and a `status` field of
/// `None` where everything that builds an error `Status` on purpose sets `Some(Failure)`, kube's
/// own `Status::failure` included (`kube-core-4.2.0/src/response.rs:79-88`) — but detecting it
/// recovers no number, and a seventh variant that produced the same sentence would be a
/// distinction with no difference.
/// The only route that could recover it is a `ClientBuilder::with_layer` above the transport that
/// rewrites such a response into a real `Status` before kube parses it; that is machinery this
/// box does not need and no box has claimed.
///
/// **The code is asked first rather than second, which the version of this in § RESOLVING AN
/// OWNER did not settle.** That one matched `(404, _) | (_, NOT_FOUND)` before
/// `(403, _) | (_, FORBIDDEN)`, so a `Status` carrying `code: 403` and `reason: NotFound` — which
/// only something between us and the API server can produce — read as *gone* rather than
/// *refused*. Nesting removes the tie-break entirely: the transport's own number wins, and the
/// body's word is read only when there is no number.
fn answer(status: &Status) -> Fault {
    match status.code {
        401 => Fault::Expired,
        403 => Fault::Refused,
        404 => Fault::Gone,
        _ => match status.reason.as_str() {
            reason::UNAUTHORIZED => Fault::Expired,
            reason::FORBIDDEN => Fault::Refused,
            reason::NOT_FOUND => Fault::Gone,
            _ => Fault::Unanswered,
        },
    }
}

/// **Which of the three kubeconfig faults one `KubeconfigError` is** (§ WHAT WENT WRONG).
///
/// **No catch-all, and `KubeconfigError` is not `#[non_exhaustive]`, so that is a choice with
/// teeth**: a variant kube adds becomes a compile error here rather than falling to a default.
/// The default a `_` arm would have to pick is one of the three sentences, and this whole finding
/// is one of those sentences being printed for something it was not true of — so *the build stops
/// and somebody reads the new variant* is the only defensible answer.
///
/// **Three groups over `KubeconfigError`'s fifteen variants** — counted, because the review that
/// found this said nineteen and this file's own first draft said sixteen, and neither had been
/// read off the enum (`config/mod.rs:33-95`; `LoadDataError` adds three more *beneath* four of
/// them, which is probably where a larger number comes from). **The grouping is what the reader
/// does next**: fix the file, fix which context is named, or fix something the file points at. A
/// fourth group would need a fourth place to go and there is not one.
///
/// **`LoadClusterOfContext` is a [`Fault::BadEntry`] and not a [`Fault::NoContext`]**, which is
/// the one variant worth arguing: the context was found, and the cluster block it names is
/// missing. The context is not the thing to fix.
///
/// **The catch-all is the file**, because the variants left in it are the merge failures —
/// `KindMismatch`, `ApiVersionMismatch` — which are two `KUBECONFIG` paths that will not combine,
/// and that is a fact about the files.
fn kubeconfig_fault(error: &kube::config::KubeconfigError) -> Fault {
    use kube::config::KubeconfigError as Bad;
    match error {
        Bad::CurrentContextNotSet | Bad::LoadContext(_) => Fault::NoContext,
        Bad::LoadClusterOfContext(_)
        | Bad::MissingClusterUrl
        | Bad::ParseClusterUrl(_)
        | Bad::ParseProxyUrl(_)
        | Bad::LoadCertificateAuthority(_)
        | Bad::LoadClientCertificate(_)
        | Bad::LoadClientKey(_)
        | Bad::ParseCertificates(_) => Fault::BadEntry,
        // The file as a file: not found, unreadable, unparseable — and the two merge failures,
        // which are two `KUBECONFIG` paths that will not combine.
        Bad::FindPath
        | Bad::ReadConfig(_, _)
        | Bad::Parse(_)
        | Bad::KindMismatch
        | Bad::ApiVersionMismatch => Fault::Kubeconfig,
    }
}

/// **What one failed call means, from the error we were handed** — the classifier, and the only
/// one (§ WHAT WENT WRONG).
///
/// **`pub` because `main.rs` holds two of these itself**: [`Session::version`] and
/// [`Session::served`] are each a `Result` that travels rather than a failure that stops the
/// session (§ CONNECTING), and a driver that printed one generic sentence over both would be
/// `PRIOR-ART § C1` exactly.
pub fn fault(error: &kube::Error) -> Fault {
    match error {
        kube::Error::Api(status) => answer(status),
        // The `exec` plugin produced nothing to send; [`Fault::NoCredential`] has both routes in.
        kube::Error::Auth(_) => Fault::NoCredential,
        // **Defensive, and deliberately so.** kube folds a `KubeconfigError` into its own error
        // with `#[from]`, on the paths that read the file themselves — `Config::infer` and
        // `Client::try_default`, both banned by name in `scripts/security-guard.py`, so nothing
        // here reaches this arm today. What it buys is that the two arms of [`NotConnected`]
        // cannot come to disagree about the same file if one ever does.
        kube::Error::InferKubeconfig(error) => kubeconfig_fault(error),
        // **The mid-session credential failure, which does not arrive as `Auth`** — measured on
        // the built binary against a login program that answers once and then exits 1
        // (2026-08-27, § WHAT WENT WRONG has the run). kube's auth layer is a tower
        // `AsyncPredicate` whose `check` ends `.map_err(Into::into)` into a `tower::BoxError`
        // (`auth/mod.rs:200-205`), so the boxed concrete type is `auth::Error` and never
        // `kube::Error`; `Client::send` downcasts to `kube::Error`, misses, and falls to
        // `unwrap_or_else(Error::Service)` (`client/mod.rs:222-233`). Without this arm the one
        // failure whose fix is on the reader's own machine printed *nothing usable came back* —
        // `PRIOR-ART § C1` surviving inside the box written to close it.
        //
        // **The type is read and never its text.** `kube::client::AuthError`'s `Display` walks
        // down to `AuthExecRun { out: … }`, which is the plugin's stdout and therefore a
        // credential (`docs/security.md` § Token hygiene). `downcast_ref` asks *which type is
        // this* and formats nothing, which is *select, never format* at its narrowest.
        kube::Error::Service(boxed)
            if boxed.downcast_ref::<kube::client::AuthError>().is_some() =>
        {
            Fault::NoCredential
        }
        _ => Fault::Unanswered,
    }
}

impl NotConnected {
    /// **Why there was nothing to connect with** — the two arms read through one classifier
    /// (§ WHAT WENT WRONG).
    pub fn fault(&self) -> Fault {
        match self {
            NotConnected::Kubeconfig(error) => kubeconfig_fault(error),
            NotConnected::Client { failure, .. } => fault(failure),
        }
    }

    /// **The program this kubeconfig logs in with**, where there was a kubeconfig to read one
    /// from — [`Session::renewal`] for a connection that never became a session.
    ///
    /// **`None` for [`NotConnected::Kubeconfig`] is the honest answer and not a shortcut**: that
    /// arm is the file failing to load, so there is no `exec` block to have read.
    pub fn renewal(&self) -> Option<&str> {
        match self {
            NotConnected::Kubeconfig(_) => None,
            NotConnected::Client { renewal, .. } => renewal.as_deref(),
        }
    }
}

impl Trouble<'_> {
    /// **Why this watch is not delivering**, or `None` for a stream that finished without ever
    /// saying why — the `ended`-with-no-`failure` shape [`Trouble::failure`] names.
    ///
    /// **The classification is [`watch_fault`]'s and this is one call of it.** The bootstrap gate
    /// reads the same error through the same function ([`Watch::settled`]), so a screen and the
    /// gate can never come to disagree about what one failure meant.
    pub fn fault(&self) -> Option<Fault> {
        self.failure.map(watch_fault)
    }
}

/// **What one `watcher::Error` means** — [`Trouble::fault`]'s body, lifted out because
/// [`Watch::settled`] asks the same question of the same value one region down and § WHAT WENT
/// WRONG is the *one* place a failure is classified.
///
/// **Four of `watcher::Error`'s five variants carry a `Status` and three of them wrap it**
/// (§ WHAT A THROTTLE LOOKS LIKE): `WatchError` holds one *directly* rather than behind
/// `kube::Error::Api`, which is the arm a `403` on the watch verb arrives through after the
/// initial LIST has already succeeded.
///
/// **`NoResourceVersion` is [`Fault::Unanswered`]** and that is a deliberate collapse: the server
/// answered, but with something no watch can be built on, and *k8rs could not read it* is the only
/// thing a reader can act on either way. It is also the one watch failure
/// [`Fault::standing`] answers `false` for, which is right: kube re-lists after it.
fn watch_fault(failure: &watcher::Error) -> Fault {
    match failure {
        watcher::Error::InitialListFailed(error)
        | watcher::Error::WatchStartFailed(error)
        | watcher::Error::WatchFailed(error) => fault(error),
        watcher::Error::WatchError(status) => answer(status),
        watcher::Error::NoResourceVersion => Fault::Unanswered,
    }
}

// --- WHAT WENT WRONG END ---

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
    /// **The last failure *this* watch reported, and no other** (NOTES § D162). The store-wide
    /// field this replaces had to be monotone because it could not tell whose failure it held
    /// (NOTES § D145); identity is what buys the clearing back, and [`Watch::take`] is where it
    /// is cleared.
    ///
    /// **Whatever renders it strips it first** (invariant 9): the text is the API server's, and
    /// this is kube's type rather than a `String` § THE INGEST GUARD owns (NOTES § D146).
    failure: Option<watcher::Error>,
    /// **This watch's stream finished, so nothing will ever arrive on it again**
    /// (NOTES § D162). `select_all` drops a finished stream and carries on with the rest, which
    /// is why this is a field and not an absence: without it a kind would sit frozen at whatever
    /// it last held and be read as live.
    ///
    /// **Never cleared.** A stream that ended cannot deliver the event that would clear it, and
    /// [`updates`] appends this as the last item of the stream, so nothing follows it.
    ended: bool,
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
            failure: None,
            ended: false,
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
        // **This watch's own failure is over when this watch has delivered a complete answer,
        // and a relist in flight is not one** (NOTES § D162). Two events qualify and the line
        // between them and the other three is drawn off kube's state machine, not off what the
        // names suggest:
        //
        // * **`InitDone` for a LIST this watch started** — a whole list, landed. The
        //   `filling.is_some()` half is the arm below's own distrust, spelled once: a stream
        //   broken enough to finish a LIST it never started is the thing that gate exists to
        //   refuse, and a watch may not be declared recovered by an event it is not trusted to
        //   be listed by. kube sends `Init` before every `InitDone` (`:548` and `:555-559` are
        //   reached only from `State::InitPage`, which only `Init` enters), so this is
        //   defensive in exactly the way that arm is.
        // * **`Apply` and `Delete`** — ordinary traffic on a watch that already listed, and on
        //   two paths **the only evidence a recovery can have**. `State::Watching` on
        //   `Some(Err(_))` returns `WatchFailed` and goes straight back to `State::Watching`
        //   with the *same stream* (`kube-runtime-4.2.0/src/watcher.rs:709`), and
        //   `State::InitListed` on a failed watch start stays `InitListed` (`:650-652`) and
        //   resumes into `Watching`. **Neither re-lists**, so a clear point of *the next
        //   complete LIST* would never fire on them and one blip would stand for the session —
        //   D145's named cost, which per-watch identity exists to stop paying.
        //
        // **`Init` and `InitApply` do not clear, and `InitApply` is the one that reads as
        // though it should.** `NoResourceVersion` (`:568`) and `InitialListFailed` (`:584`) both
        // return `State::Empty`, which emits `Init` (`:523`) and then `InitApply`s (`:548`) — so
        // after those two the objects arriving are a **relist that has not finished**, and
        // withdrawing the failure on the first of them announces a recovery the LIST has not
        // achieved. It is worse than a wrong word: `complete` is never reset, so
        // [`Watch::progress`] returns `None` for a relisting watch and [`Store::still_listing`]
        // says nothing either. Clearing here would take **both** facts quiet at once — and the
        // call this relist is sitting in is `api.list()`, which is the **unbounded** half of
        // § WHAT A THROTTLE LOOKS LIKE: the watch poll unblocks at ~295 s, a LIST against a
        // keepalive-less socket never does. So the store would read perfectly healthy, for as
        // long as the process ran, while it served a cluster from before the failure.
        //
        // **This is the same fact D150 reads for a different question, not a contradiction of
        // it.** There, `Init` and `InitApply` both count, because the question is *is this LIST
        // moving* and an object arriving proves it is. Here neither counts, because the question
        // is *has this watch delivered an answer* and a LIST in flight has not.
        //
        // **Stated for `ListWatch`, which is kube's default (`:201-202`) and the strategy
        // `k8s_tests.rs` pins.** Under `StreamingList` an interrupted initial watch stays on its
        // own stream and `InitApply` would be same-stream evidence, exactly as `Apply` is here;
        // the rule below still terminates on that path, at the end-bookmark's `InitDone`.
        let answered = match &event {
            Event::Init | Event::InitApply(_) => false,
            Event::InitDone => self.filling.is_some(),
            _ => true,
        };
        if answered {
            self.failure = None;
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
            //
            // **It catches a LIST that never *started*, not one that was *interrupted*.** An
            // `Init`, some `InitApply`s, an `Err` and then an `InitDone` leaves `filling` full,
            // and this arm publishes it as a complete cluster that is short whatever the failed
            // page held. Nothing here can tell that apart — kube does not send it, because both
            // list failures return `State::Empty` and re-`Init` (`:568`, `:584`, `:523`), which
            // is what `k8s_tests.rs`'s pin of `ListWatch` is worth. Said plainly because this
            // gate is described as defensive and one of the two broken shapes walks through it.
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
        (!self.complete && !self.settled()).then(|| {
            (
                self.filling.as_ref().map_or(0, BTreeMap::len),
                self.last_progress.clone(),
            )
        })
    }

    /// **This watch has said everything it is going to say without a person intervening**
    /// (todo.md § Phase 5, `PRIOR-ART § B4`) — the explicit exception NOTES § D28 did not carry.
    ///
    /// **It is read by [`Watch::progress`] alone, which is what makes it one change and not
    /// two.** [`Store::still_listing`] drops a watch this is true of, and [`Store::listed`] is
    /// derived from that call — so *the screen stops claiming a LIST is moving* and *the gate
    /// opens* are the same line, and cannot be fixed apart.
    ///
    /// **One way to be finished, and it is a standing failure** ([`Fault::standing`]). A refused
    /// watch runs `Err → Init → list() → 403` forever (§ THE DRIVER), and every `Init` restamps
    /// [`Listing::since`] — so before this it reported *0 so far, since just now*, several times a
    /// second, for the life of the process. That is a screen actively lying about progress, and a
    /// gate that never opened behind it: `snapshot()` answered `None` and every rule was silent,
    /// including the ones about the four kinds the cluster was serving perfectly.
    ///
    /// **What it deliberately does not cover is [`Fault::Unanswered`]**, which is where D28's own
    /// *do not blank on a blip* paragraph lives: the next poll may answer, the retry is the fix,
    /// and the driver prints a [`Trouble`] line meanwhile rather than nothing.
    ///
    /// **[`Watch::ended`] is not in it either, and that is a decision rather than an oversight.**
    /// A stream that finished before it listed will also never list, so on the face of it it
    /// belongs — but it is **unreachable in a running k8rs**: kube's `watcher()` is a
    /// `stream::unfold` that provably cannot end and [`StandingBackoff`] never closes one
    /// (§ THE DRIVER). What it *is* reachable from is a test, where `stream::iter` runs out at
    /// the end of the events the test wrote — so putting it here would open the gate in six
    /// tests whose whole subject is that D28 keeps it shut, on an artefact of their fixture
    /// rather than on anything the product does. Closing it needs those fixtures to stop ending,
    /// which is a box and not a clause (`backlog.md`).
    ///
    /// **What the caller then publishes for this kind is an empty list, not a short one.** That
    /// is a different claim from the partial LIST D28 refuses — a partial one is indistinguishable
    /// from a small cluster, and this one is announced twice over: [`Store::troubles`] names the
    /// kind and the verb, and `analysis.rs`'s *Reading what a node has needs permission to list
    /// nodes* rows draw the same fact where a number would have been.
    fn settled(&self) -> bool {
        self.failure
            .as_ref()
            .map(watch_fault)
            .is_some_and(Fault::standing)
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

/// **One watch that is not delivering, and why** (NOTES § D162). `PRIOR-ART § B3` is the failure
/// this exists for: k9s ran its first refresh outside the retry loop and called `BailOut` after
/// five, so one blip killed the reconnector and a VPN drop over lunch meant the tool was gone on
/// return. Nothing here counts retries and nothing here gives up: [`drive`] cannot return on a
/// failure, and this is the state that says one is standing.
///
/// **At least one of the two below is always true** — [`Store::troubles`] does not report a watch
/// that has neither.
///
/// **Nothing here is a duration.** *How long* is [`Listing::since`]'s question for a bootstrap
/// that has not landed; a failure is answered by *is it still standing*, which is one read of
/// [`failure`](Trouble::failure) and needs no clock. Adding a second stamp would give this file a
/// second meaning for the one clock read § THE DRIVER owns (NOTES § D150), and no screen has
/// asked for one.
///
/// **No `Debug`, and for a harder reason than [`Store`]'s** (NOTES § D162): the borrow at
/// [`failure`](Trouble::failure) reaches an `exec` plugin's stdout, so a `{:?}` on this type
/// prints a bearer token. Nothing formats a `Trouble`; without the derive, nothing can begin to
/// by accident — a renderer selects fields, and [`failure`](Trouble::failure) says which.
pub struct Trouble<'a> {
    /// Which watch.
    pub kind: ObjectKind,
    /// **Whether this watch ever delivered a complete initial LIST** (`Watch::complete`).
    ///
    /// **It is the difference between *stale* and *never read*, which a screen must not
    /// conflate** (`screens/widgets.md` § 1a: *a vital that cannot be read is blank, never
    /// guessed*, and one line down, *stale vitals stay visible and say how old they are*). Both
    /// arrive here as a watch in trouble; only this field tells them apart, and without it the
    /// header prints a measured `0 nodes` for a refusal and blanks a real count for a blip.
    ///
    /// **`false` inside a report means a standing failure, always.** [`Store::snapshot`] answers
    /// `None` until every watch has listed *or settled*, and settling is a standing failure
    /// ([`Watch::settled`]) — so a watch that reaches a rendered report unlisted is one the
    /// cluster refuses, not one that is still working.
    pub listed: bool,
    /// **The last failure this watch reported and has not recovered from.** `None` beside an
    /// `ended` of `true` is a stream that finished without ever saying why.
    ///
    /// # A renderer selects fields off this error. It never formats it whole.
    ///
    /// **`format!("{}", failure)` can print a bearer token, and so can `{:?}`.** Not a
    /// theoretical reach — the whole chain is `#[error("…{0}")]`, which interpolates the source
    /// at every hop:
    ///
    /// ```text
    /// watcher.rs:30   InitialListFailed(kube_client::Error)
    ///                 "failed to perform initial object list: {0}"
    /// error.rs:104    Auth(AuthError)
    ///                 "auth error: {0}"
    /// auth/mod.rs:55  AuthExecRun { cmd, status, out: std::process::Output }
    ///                 "auth exec command '{cmd}' failed with status {status}: {out:?}"
    /// ```
    ///
    /// `std::process::Output`'s `Debug` renders `stdout` **as a string when it is valid UTF-8**
    /// — measured, not read off a definition: a one-line program formatting the `Output` of a
    /// script that prints an `ExecCredential` and exits 1 puts the token in the output verbatim.
    /// And an `exec` credential plugin writes exactly that JSON to stdout. The trigger is
    /// ordinary rather than exotic: an EKS/GKE/AKS `exec` block whose SSO session expired
    /// mid-session, or a wrapper script tripping `set -e` after it emitted the credential.
    ///
    /// **So the rule is *select*, never *format*:** the variant, and `Status.code` /
    /// `Status.reason` where there is one (§ WHAT A THROTTLE LOOKS LIKE lists which variants
    /// carry a `Status` and how). Keeping this typed rather than a `String` is what makes that
    /// possible, which is why the boundary is here (NOTES § D145, § D146).
    ///
    /// **[`Trouble::fault`] is that selection already made**, and a renderer wanting to know
    /// *why* should call it rather than reach in here a second time: one classifier, one answer
    /// (§ WHAT WENT WRONG).
    ///
    /// **This is not the rule beside it.** Invariant 9 — strip control characters — is owed
    /// *as well*, on whatever text is selected, because it is the API server's. Stripping does
    /// nothing about a token: a token prints as itself. **`scripts/security-guard.py` refuses a
    /// *derived* `Debug` on a declaration it parses** — that is what took one off [`Trouble`] —
    /// **and it sees no format call at all** (NOTES § D164). So *select, never format* is owed by
    /// whoever writes the screen, and no script will catch them getting it wrong.
    pub failure: Option<&'a watcher::Error>,
    /// **This watch's stream finished**, so what its kind holds is the last thing it ever held.
    /// kube documents a `watcher()` stream as recovering rather than finishing — read off its
    /// doc, never observed — so in a live cluster this is expected to stay `false`; it is a field
    /// because *expected* is not *guaranteed*, and the alternative to a field was silence.
    pub ended: bool,
}

/// **The four facts about a cluster that no watch carries**, handed to the store once
/// (NOTES § D169).
///
/// [`Store`] is a store and not a connection: it is fed by five streams and has no client, no
/// kubeconfig and no way to ask a server anything. These come from [`connect`] — one from the API
/// server, two from the reader's own file, and one from what the run was asked for and what the
/// cluster allowed ([`Coverage`]) — so somebody has to carry them across, and [`Identity::of`] is
/// that one step. Every field is `Option` and `None` is *nobody looked* (NOTES § D129), which is
/// exactly what a [`Store::default`] that was never identified is.
///
/// **Each is already stripped and bounded where it was read** — [`session`] for the version,
/// [`kubeconfig_context`] and [`kubeconfig_certificate`] for the next two — so nothing here strips
/// a second time. The certificate is bytes and is never printed as text: the only thing that reads
/// it is `rules.rs`'s PEM parser. **The namespace is the one whose guard is not a strip**: it goes
/// into a URL as well as onto a screen, so [`coverage`] runs [`namespace_name`] over it and whoever
/// prints it owes [`text`]'s job separately — `main.rs`'s header does it at the sentence.
///
/// **Taken once at connect and never refreshed, so every one of them can be stale as well as
/// absent**
/// (`k8s-admin`, 2026-08-28). [`Store::identify`] runs after [`connect`] and the per-watch
/// reconnect never calls it again (NOTES § D161), so these are frozen for the life of the process.
/// A control plane upgraded while k8rs is open leaves Versions counting kubelets against the old
/// string; a session an `exec` plugin holds open outlives the complete static pair beside it —
/// the shape [`kubeconfig_certificate`] knowingly accepts — and C1's `Critical` *the cluster is
/// refusing you* card then stands forever while everything works. That band **is** drawn in
/// Alerts: only `Info` is filtered out of the card block (NOTES § D87).
/// **No refresh in this box** — what is written here is the ceiling, so the next reader does not
/// take `None` for the only way one of these can be wrong.
///
/// **No `Debug`, for [`Store`]'s own reason**: nothing here holds a credential — a certificate is
/// the public half, and the key is deliberately not read — but two of the three come off a
/// kubeconfig, and the security gate's rule about that is mechanical rather than a per-field
/// judgement call.
#[derive(Default)]
pub(crate) struct Identity {
    /// [`crate::rules::ClusterSnapshot::server_version`] — what the API server calls itself.
    pub(crate) server_version: Option<String>,
    /// [`crate::rules::ClusterSnapshot::context`] — what the reader calls this cluster, and C1's
    /// object name.
    pub(crate) context: Option<String>,
    /// [`crate::rules::ClusterSnapshot::client_certificate`] — C1's PEM bytes, the certificate
    /// alone and never the key ([`kubeconfig_certificate`]).
    pub(crate) client_certificate: Option<Vec<u8>>,
    /// [`crate::rules::ClusterSnapshot::namespace_scope`] — **how much of the cluster the watches
    /// above this store are actually covering**, `None` for every namespace.
    ///
    /// **It arrives here rather than through a stream for the reason the three above do**: no
    /// watch carries it. It is decided once, at [`connect_with`], out of `--namespace` and out of
    /// what the cluster answered a cluster-wide pod LIST with ([`Coverage`]) — and the rules read
    /// it as one fact, because to a rule *the reader asked for one namespace* and *the cluster
    /// allows only one* are the same fact (NOTES § D46).
    pub(crate) namespace_scope: Option<String>,
}

impl Identity {
    /// **What one connected session says about itself**, for the store to publish.
    ///
    /// **`Err` on the version is `None`, and it is the same `None` as *never asked***
    /// (NOTES § D129): the reason a version could not be read is [`Session::version`]'s to keep
    /// and `main.rs`'s startup line to print, and N4's answer either way is *say nothing rather
    /// than compare against a guess*. A snapshot field that carried the difference would be a
    /// second place to keep it.
    pub(crate) fn of(session: &Session) -> Self {
        Identity {
            // **Empty is `None`, the same way [`renewal`] and [`kubeconfig_context`] answer it.**
            // A `gitVersion` of nothing but control characters strips to `""` in [`session`], and
            // `Some("")` is a version that was read — the Versions pane draws `Control plane `
            // with a trailing space and then says the string cannot be compared against, about a
            // string that is not there. Three readers of one shape, and this was the one that
            // disagreed (`k8s-admin`, 2026-08-28).
            server_version: session
                .version
                .as_ref()
                .ok()
                .filter(|version| !version.is_empty())
                .cloned(),
            context: session.context.clone(),
            client_certificate: session.client_certificate.clone(),
            // **The scope the watches were actually built with, and not the flag that was
            // typed** ([`Coverage::namespace`]): a `--namespace` that was never honoured, or a
            // fallback the reader never asked for, would both make this field a claim about
            // intent rather than about the list the rules are reading.
            namespace_scope: session.coverage.namespace().map(str::to_string),
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
    /// **Every ReplicaSet a pod named as its controller, keyed by the uid the pod named** —
    /// § RESOLVING AN OWNER is the whole of what this is for.
    ///
    /// **Keyed by uid and not by name**, because a rollback re-creates a ReplicaSet under the
    /// same name with a new uid, and the two are different objects with different pods.
    /// `Err` is a fetch that did not produce one, kept so the same reference is not asked
    /// about again on the next pass ([`Fault`]).
    owners: BTreeMap<String, Result<WorkloadSnapshot, Fault>>,
    /// **The facts the watches cannot deliver** ([`Identity`]) — empty until [`Store::identify`]
    /// is called, which the file driver never does and the live one does once.
    identity: Identity,
    /// **Rule C3's input, filed by [`Store::certificates_fetched`]** — `None` until somebody
    /// fetches it, and `None` again when they tried and were refused
    /// ([`crate::rules::ClusterSnapshot::certificate_requests`], § WHAT A REPORT ASKS FOR).
    ///
    /// **`None` is *nobody looked* and `Some(vec![])` is *nothing to find*** (NOTES § D129), which
    /// is the distinction `analysis::kubelets_waiting_to_join` branches on: the first draws the
    /// row that says the check did not run, the second says nothing is waiting.
    certificate_requests: Option<Vec<CertificateRequestSnapshot>>,
    /// **Waste's and Drain safety's inputs, filed by [`Store::reports_fetched`]** — the same
    /// `None`-is-*nobody looked* rule as the field above, five fields at once ([`ReportLists`],
    /// § WHAT A REPORT ASKS FOR).
    reports: ReportLists,
    /// **The Capacity report's live usage, filed by [`Store::metrics_polled`]** — the one field
    /// here that is refilled on a timer rather than read once (§ WHAT A NODE IS USING).
    ///
    /// **The `Option` is *nobody asked* and the four arms inside it are everything else**
    /// ([`crate::rules::Metrics`], whose doc is precise about why that fifth state is the
    /// `Option` and not a fifth variant). A run without `--analysis` never polls, so it stays
    /// `None` and `analysis::live_usage_row` draws the one sentence that names k8rs rather than
    /// the cluster.
    metrics: Option<Metrics>,
}

impl Store {
    /// **Take the three facts no watch carries** ([`Identity`], NOTES § D169) — called once,
    /// after [`connect`], before the first snapshot.
    ///
    /// **A setter and not a constructor argument**, because [`Store::default`] is what the file
    /// driver and every test that has no cluster build: a store that was never identified is a
    /// store that could not ask, and `None` says exactly that.
    pub(crate) fn identify(&mut self, identity: Identity) {
        self.identity = identity;
    }

    /// **File one CSR list**, or the refusal that is the same `None` the store started with
    /// (§ WHAT A REPORT ASKS FOR).
    ///
    /// **A setter for [`Store::identify`]'s reason**: the store has no client and cannot ask
    /// anybody anything, so the fetch is the caller's and this is where its answer lands.
    ///
    /// **It takes the `Option` rather than a `Result`, because there is no second answer to
    /// give.** [`certificate_requests`] has already collapsed every failure into *nobody looked*,
    /// which is the state the pane draws a `NotComputed` row for and names no cause on
    /// (`analysis::kubelets_waiting_to_join`). A `Result` here would be a fault nothing reads.
    pub(crate) fn certificates_fetched(
        &mut self,
        requests: Option<Vec<CertificateRequestSnapshot>>,
    ) {
        self.certificate_requests = requests;
    }

    /// **File the five lists a report joins**, or the refusals that are the same `None` the store
    /// started with ([`report_lists`], § WHAT A REPORT ASKS FOR).
    ///
    /// **One setter and one value, for [`ReportLists`]'s reason** — five setters are five things
    /// a caller can forget one of, and a store holding Services because somebody called four of
    /// the five is the *every Service matches nothing* answer `rules.rs` warns about, arrived at
    /// by a typo. **A field that is `None` because the cluster refused it is a different thing and
    /// still reachable** ([`ReportLists`]): what one value removes is the half-filled store nobody
    /// meant, not the partial answer a role produces.
    ///
    /// **It takes the `Option`s rather than a `Result`, for [`Store::certificates_fetched`]'s
    /// reason**: each fetch has already collapsed every failure into *nobody looked*, and a fault
    /// nothing reads is a fault nothing draws.
    pub(crate) fn reports_fetched(&mut self, lists: ReportLists) {
        self.reports = lists;
    }

    /// **File one metrics poll**, whichever of the four things it found (§ WHAT A NODE IS USING).
    ///
    /// **It takes the [`crate::rules::Metrics`] rather than an `Option`, so a caller cannot file
    /// *nobody asked* after having asked.** That state is the store's starting one and the only
    /// honest way back to it is never to have polled; every poll produces one of the four, and
    /// [`node_usage`] has already collapsed the failures into three of them.
    ///
    /// **Called more than once, unlike every other setter here** — this is the poll, so the last
    /// answer wins, and every transition between the four is drawn within one [`METRICS_POLL`]:
    /// a metrics-server that is installed, one that comes back up, a role that is granted the
    /// verb. Nothing here is a state the store gets stuck in (NOTES § D181).
    pub(crate) fn metrics_polled(&mut self, metrics: Metrics) {
        self.metrics = Some(metrics);
    }

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

    /// **Every watch that is not delivering, and why** — empty when all five are healthy
    /// (NOTES § D162).
    ///
    /// **Built like [`Store::still_listing`] and read beside it**: the same five watches in the
    /// same declared order, each answering for itself and for nothing else. That is the whole
    /// of what this box bought — the field this replaced was store-wide, so it had to be
    /// monotone or four healthy watches would erase the fifth's standing 403 with their own
    /// ordinary traffic (NOTES § D145). With identity, a watch may clear its own.
    ///
    /// **The two facts are not one enum, because they answer different questions.** `failure`
    /// is *what went wrong and may still be going wrong*; `ended` is *nothing will arrive here
    /// again*. Both together is the ordinary shape of a watch that died of the failure beside
    /// it, and either alone is a real state: a watch retrying a 403 has no `ended`, and a stream
    /// that finished cleanly has no `failure`.
    ///
    /// **The gate is not closed by either, and since the namespace box a standing refusal does
    /// not *hold* it either** (NOTES § D28, § D162, todo.md § Phase 5). A watch that ends after
    /// listing holds a real answer that is merely no longer fresh — blanking the screen would
    /// replace *stale, and it says so* with *nothing, and it does not*. A watch the cluster
    /// **refuses** before it lists used to leave [`Store::snapshot`] shut for the life of the
    /// process; it now counts as answered ([`Watch::settled`]), and this line is the other half of
    /// that answer — the kind is named here, with its verb and its resource, instead of the whole
    /// tool showing *loading*. A watch that *ends* before it lists still holds the gate, and
    /// [`Watch::settled`] says why that one was left alone.
    ///
    /// # This call and [`Store::still_listing`] can no longer disagree about a kind
    ///
    /// **A refused watch is reported here and not there.** It re-`Init`s in a tight loop and
    /// every `Init` refreshes [`Listing::since`], so it used to appear there as a LIST moving
    /// briskly (NOTES § D150 explains why `Init` counts *for that question*) and callers were
    /// told to join the two by kind and let this one win. [`Watch::settled`] removes it from that
    /// call instead, so a screen reading one and not the other is no longer wrong.
    ///
    /// **The words are the caller's**, exactly as for [`Store::still_listing`]: this returns
    /// facts, and invariant 14's plain language is `views.rs`'s.
    pub fn troubles(&self) -> Vec<Trouble<'_>> {
        // Five rows of four values rather than five `&Watch<_>`: the five watches carry three
        // different element types, so there is no one reference type to put in an array.
        [
            (
                ObjectKind::Pod,
                &self.pods.failure,
                self.pods.ended,
                self.pods.complete,
            ),
            (
                ObjectKind::Node,
                &self.nodes.failure,
                self.nodes.ended,
                self.nodes.complete,
            ),
            (
                ObjectKind::Deployment,
                &self.deployments.failure,
                self.deployments.ended,
                self.deployments.complete,
            ),
            (
                ObjectKind::StatefulSet,
                &self.stateful_sets.failure,
                self.stateful_sets.ended,
                self.stateful_sets.complete,
            ),
            (
                ObjectKind::DaemonSet,
                &self.daemon_sets.failure,
                self.daemon_sets.ended,
                self.daemon_sets.complete,
            ),
        ]
        .into_iter()
        .filter(|(_, failure, ended, _)| failure.is_some() || *ended)
        .map(|(kind, failure, ended, listed)| Trouble {
            kind,
            listed,
            failure: failure.as_ref(),
            ended,
        })
        .collect()
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
    /// # A watch that is not coming back is not listing, and this call no longer says it is
    ///
    /// **On one shape this used to read healthy and be wrong, and the fix is at the source rather
    /// than in a rule for callers to remember.** A watch the cluster refuses runs
    /// `Err → Init → list() → 403` (§ THE DRIVER), and `Init` stamps [`Listing::since`] every
    /// time round (NOTES § D150, deliberately — an `Init` proves the watch *began*). So a
    /// permanently refused watch reported *pods, 0 so far, since just now*, several times a
    /// second, forever: D150's separator is *a hung LIST produces numbers that do not move*, and
    /// this one's moved beautifully while nothing whatever was happening. [`Watch::settled`] now
    /// drops it here, so the two calls cannot disagree and no caller has to join them to be
    /// right.
    ///
    /// **A watch that is failing *transiently* is still reported here, and that is the same
    /// decision rather than a leftover.** `Init` restamping `since` is then true: a LIST really
    /// is being retried, and it may well land. [`Store::troubles`] names it in parallel, which is
    /// what tells *this is taking a while* from *this is taking a while and erroring*.
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

    /// Every initial LIST has landed **or is never going to**, so there is something honest to
    /// publish (NOTES § D28, and [`Watch::settled`] is the exception D28 did not carry).
    ///
    /// Derived from [`Store::still_listing`] rather than from the five fields a second time: the
    /// gate and the state the screen draws the wait from cannot disagree if only one of them
    /// reads the flags. That is also why the exception is written once, in [`Watch::progress`],
    /// and not spelled again here — a second copy of it is a gate and a screen coming apart.
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
            // from a watch (`docs/architecture.md` § Data flow), which is why they arrive through
            // [`Store::identify`] rather than through a stream (NOTES § D169). A store nobody
            // identified answers `None` for all three, and N4 and C1 correctly say nothing.
            //
            // **They do not pass through [`ingest`]**, so each owes [`text`] where it is read:
            // `server_version` in [`session`] because it is the API server's own string
            // (invariant 9), and `context` in [`kubeconfig_context`] because a kubeconfig is
            // written by tooling as often as by hand. `client_certificate` is bytes nothing ever
            // prints as text — the one reader is `rules.rs`'s PEM parser
            // ([`kubeconfig_certificate`]).
            server_version: self.identity.server_version.clone(),
            context: self.identity.context.clone(),
            client_certificate: self.identity.client_certificate.clone(),
            // **`None` is every namespace, and it is the store's own field rather than a
            // guess** ([`Identity::namespace_scope`]). A store nobody identified — the file
            // driver's, and every test that has no cluster — answers `None`, which is exactly
            // what a fixture on disk covers.
            namespace_scope: self.identity.namespace_scope.clone(),
            // **`None` is *nobody looked*, and it is not `Some(vec![])`** (NOTES § D129). None of
            // these is watched (invariant 6); [`Store::reports_fetched`] fills them from
            // [`report_lists`] on a run that draws reports, and each stays `None` when nobody
            // asked or the cluster refused — an empty `Vec` here would tell Waste that nothing is
            // going to waste over lists it never read.
            //
            // **The owner cache is not this list and may not be poured into it** — the whole
            // argument is on [`replica_sets`], which is the fetch that does answer Waste's
            // *parked at 0 replicas* row, and it is not restated here: the second copy is the one
            // that goes stale, and this one had already gone stale by 2026-08-29.
            replica_sets: self.reports.replica_sets.clone(),
            services: self.reports.services.clone(),
            endpoint_slices: self.reports.endpoint_slices.clone(),
            claims: self.reports.claims.clone(),
            disruption_budgets: self.reports.disruption_budgets.clone(),
            // The sixth on-demand list, filed one setter over because C3 owns it
            // ([`Store::certificates_fetched`]).
            certificate_requests: self.certificate_requests.clone(),
            // **`None` is *k8rs never asked*, which is a run with no report open — and it is not
            // one of [`crate::rules::Metrics`]'s four arms** (§ WHAT A NODE IS USING). Filled by
            // [`Store::metrics_polled`] every thirty seconds while a report wants it, so unlike
            // the six lists above this one is not a photograph.
            metrics: self.metrics.clone(),
        })
    }
}

// --- THE STORE END ---

// --- WHAT A REPORT ASKS FOR START ---
//
// **The lists no watch carries, fetched when a report wants one and never on a timer**
// (invariant 6, todo.md § Phase 5). The permanent watch is Alerts' five kinds and nothing else; a
// report's own inputs are read on demand, once, and a `None` that stays `None` is what the pane
// draws its *not checked* row from.
//
// **Six lists, one shape** ([`whole_list`]). C3's CertificateSigningRequests came first and the
// five [`ReportLists`] carries — ReplicaSets, Services, EndpointSlices, PVCs and
// PodDisruptionBudgets — followed into the same region. **Deployments are not among them**: they
// are on the permanent watch already ([`Store::deployment`]), and a second read of a kind the
// store is streaming would be two answers to one question.
//
// **Every failure is one silence, and that is why nothing here is a `Result`.** § WHAT WENT WRONG
// is the one place a failed call is classified, and it is read by whoever owes the reader a
// sentence. Nobody does here: `analysis::kubelets_waiting_to_join` draws its `NotComputed` row
// with **no cause named**, deliberately, because `None` is *nobody fetched it*, *this login may
// not list them* and *the answer never came* ([`REPORT_FETCH`]) at once, and the field cannot tell
// the three apart. That is [`skew`]'s shape exactly —
// *it owes no sentence, so it needs no taxonomy* — and it is what keeps a `403` on a cluster-scoped
// verb from becoming a second classifier.
//
// **`list certificatesigningrequests` is cluster-scoped, so the refusal is the ordinary case
// rather than the exception** (`crate::rules::ClusterSnapshot::certificate_requests`, the security
// gate's *a 403 degrades that one feature*). It degrades one row of one pane, it never touches the
// session, and **nothing retries it** — [`Store::unresolved_owners`]'s rule (NOTES § D151): a
// standing refusal re-asked once a pass is the retry loop the security gate forbids by name.
//
// **The five beside it are namespaced kinds, and on an unscoped run they are read cluster-wide,
// which is a different refusal with the same answer.** `Api::all` sends `/api/v1/services`, not
// `/api/v1/namespaces/x/services`, and a request that carries no namespace can only be authorized
// cluster-wide — so what it needs is `list services` granted by a ClusterRole, which a namespaced
// Role never provides however many namespaces that Role covers. **On a scoped run they follow the
// scope instead** and that refusal does not arise; see the paragraph below.
//
// **The two refusals are different populations and are not ordered, which an earlier draft here
// got backwards** (`k8s-admin`, 2026-08-29, against upstream's `viewRules()`). The built-in `view`
// ClusterRole grants every one of the five this region fetches — `replicasets`, `services`,
// `endpointslices`, `persistentvolumeclaims`, `poddisruptionbudgets` — and grants
// `certificatesigningrequests` in neither `view` nor `edit`. So the ordinary cluster-wide
// read-only principal gets the five and is refused the sixth, while a namespaced Role scoped to
// its own namespace now gets the five there and is still refused the sixth. Neither refusal is
// *likelier*; they are refused by different roles.
//
// It degrades the same way whichever it is: [`ReportLists`] keeps that one field `None`, the pane
// draws *not checked*, and nothing asks again.
//
// **The five namespaced ones follow the scope, since 2026-08-29** ([`scoped`], the same function
// the watches use). The paragraph above described the state before the `--namespace` box landed
// and stayed false for one review round after it: a developer whose `Role` grants Services,
// EndpointSlices, PVCs and PDBs **in their own namespace** was told *Ask for permission to list
// services … across the whole cluster* — access they do not need, which is `PRIOR-ART § B4` by
// name (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R9). `screens/analysis.md`
// records the design the other way round: *Waste runs unchanged when the view is scoped, because
// every input it has is namespaced*.
//
// **`certificatesigningrequests` is the one that stays cluster-wide**, because it is
// cluster-scoped and there is no namespaced path to send it down. Its refusal under a namespaced
// role is unchanged and is still the ordinary case.

/// **How long one fetch on this path gets before it is the same silence a refusal is** — ten
/// seconds (§ WHAT A REPORT ASKS FOR).
///
/// **It exists because the failure it bounds is a hang, not a slow answer.** `Config::read_timeout`
/// is `None` in every kube constructor — `config/mod.rs:191`, `:273`, `:339` — so nothing under
/// this call bounds it, and a middlebox that accepts the connection, reads the request and answers
/// nothing holds it forever. That is [`SERVING_PROBE`]'s reasoning, which this box wrote one
/// function away and did not apply here: the fetch runs on the startup path *after* the greeting
/// has gone to stderr, so an unbounded one prints `k8rs: watching — …` and then nothing at all,
/// looking connected (`tester`, 2026-08-28).
///
/// **The same ten seconds, and deliberately shorter than kube's own 30-second `connect_timeout`**
/// (`config/mod.rs:418`) — which bounds *connecting* and not answering, so it is not a fallback
/// this could have leaned on. This call owes the reader nothing: a link too slow for ten seconds
/// produces exactly the `None` a 403 produces, and `analysis::kubelets_waiting_to_join` draws one
/// *not checked* row for both.
///
/// **The four calls `connect_with` makes have no such bound and are not this box's to give one**
/// (§ CONNECTING). They are older than this region, a session cannot start without them, and a
/// deadline on them is a decision about what k8rs does when a cluster half-answers — which is a
/// box, not a line. What is true and worth writing down is that the hole is the same one.
///
/// **The scope probe reuses it rather than inventing a second number** ([`coverage`]). It bounds
/// the same failure for the same reason — a call between the client and the first watch, with
/// nothing under it that has a read deadline — and two constants for one hang is two numbers to
/// keep in step.
///
/// **`pub(crate)` because one of its two call sites is `main.rs`'s**, which is the only
/// difference from [`SERVING_PROBE`]: the deadline is a parameter for that constant's reason — a
/// bound nothing can wait out is a bound nothing tests — and the caller that has to be honest
/// about it is the driver.
pub(crate) const REPORT_FETCH: std::time::Duration = std::time::Duration::from_secs(10);

/// **One list, whole, through the one ingest door, or the silence that is every failure** —
/// the shape all six fetches in this region are (§ WHAT A REPORT ASKS FOR).
///
/// `Some(vec![])` is *this cluster has none*, which is a different fact from this function's
/// `None` and is drawn differently (NOTES § D129).
///
/// **The answer must be whole, and this sends one unpaged request to get it.** `Some(first 500 of
/// 4000)` tells the report that four kubelets are waiting when four hundred are — the *short list
/// read as a small cluster* failure NOTES § D28 shuts the whole snapshot for.
/// `ListParams::default()` really does send no `limit`: measured against a stub API server, it goes
/// out as `…/certificatesigningrequests?` with an empty query, while the watches beside it carry
/// `&limit=500` (`the_certificate_requests_a_cluster_lists_reach_the_snapshot` and
/// `the_five_lists_a_cluster_answers_reach_the_snapshot` assert every one of the six paths).
///
/// **A `limit` is not the same thing as a page, and saying so here was wrong** (`k8s-admin`,
/// 2026-08-29). Paging to completion also produces a whole answer, and § THE INITIAL LIST in this
/// same file documents kube doing exactly that on every watch — `limit=500`, the server's
/// `continue` token sent back until a page arrives without one, *"several HTTP responses are one
/// LIST"*. **`kubectl` chunks the same way by default**: `kubectl get --help` under client
/// `v1.36.3` prints `--chunk-size=500: Return large lists in chunks rather than all at once. Pass
/// 0 to disable.` — the same minor as the `v1.36.1` kind node this repo captures against. So
/// `kubectl get rs -A` pages, and what this line sends is what `--chunk-size=0` sends.
/// **So one request is a choice about round trips, not about wholeness**,
/// and the real reason it is made this way is that paging is a `continue`-token loop with a
/// compaction failure mode of its own — § THE INITIAL LIST carries that loop's whole argument for
/// the watch path. Adding a second one here is a box with its own tests, not a line
/// (`backlog.md`).
///
/// **What one request costs is one allocation the size of that kind's whole list, and the number
/// worth naming is ReplicaSets.** A ReplicaSet carries the entire pod template, and
/// `revisionHistoryLimit` defaults to **10** — so a cluster with 1000 Deployments holds up to
/// 10 000 template-carrying ReplicaSets, and this holds each list twice at its peak: the response
/// body and the decoded `Vec`. **The six run concurrently ([`report_lists`]), so the peak is their
/// sum**, not the largest of them. **Named rather than capped**, because a cap is the wrong answer
/// in the first paragraph wearing a different hat — a truncated list is a confident short answer,
/// and the honest fixes are paging or a refusal, both of which are boxes. Every *string* on these
/// objects is already bounded field by field on the way through [`ingest`]; what is unbounded is
/// the object **count**.
///
/// **Nothing here is watched, ever** (invariant 6). The Alerts view's inputs are the five
/// permanent watches; these are one fetch each for one report's rows.
///
/// **Bounded by the caller's `deadline`, because nothing under it is** — [`REPORT_FETCH`], which
/// the one call site names. An unbounded version of this line printed the greeting and then hung
/// forever against a listener that answers nothing (`tester`, 2026-08-28).
///
/// **Every object goes through [`ingest`] like every other**, which is the strip, the bound and
/// the prune at once (invariant 9, § THE INGEST GUARD).
///
/// **The `Api` is the caller's and not built here**, which is what lets one body serve both the
/// cluster-scoped kind and the five that follow the scope ([`scoped`], the region doc). Building it
/// here would need a `Coverage` and a `NamespaceResourceScope` bound that
/// `certificatesigningrequests` cannot satisfy, and a second copy of this body is the second place
/// the deadline or the ingest door can be forgotten.
async fn whole_list<K, T>(api: Api<K>, deadline: std::time::Duration) -> Option<Vec<T>>
where
    K: Resource + Clone + std::fmt::Debug + k8s_openapi::serde::de::DeserializeOwned + 'static,
    <K as Resource>::DynamicType: Default,
    T: From<K> + Bounded,
{
    let listed = tokio::time::timeout(deadline, api.list(&ListParams::default()))
        .await
        .ok()?
        .ok()?;
    Some(listed.items.into_iter().map(ingest).collect())
}

/// **Every CertificateSigningRequest the cluster has**, or `None` when there was no answer to
/// have — rule C3's input ([`whole_list`], which is the whole of the shape).
///
/// **The prune follows the decode here, and the difference is not academic.** [`whole_list`]
/// deserializes the full typed `CertificateSigningRequest` into `listed.items` first; the prune is
/// the `From` impl inside `.map(ingest)` *afterwards*. So `spec.request` — the CSR's own PEM body
/// — and `spec.extra`, the requester's credential id, **are on the heap for the length of that
/// map, over the whole list**. What is true is the thing that matters downstream: neither is a
/// field [`CertificateRequestSnapshot`] has, so **nothing past the decode can hold one, print one
/// or put one in a `Debug` line**. An earlier draft here said *neither is ever held*, which is a
/// stronger claim than the code makes (`k8s-admin`, 2026-08-29). **Decode first, prune second is
/// the shape of all six fetches** — they share [`whole_list`] — so this paragraph is about the
/// whole region and sits here only because the CSR is the one carrying secrets.
///
/// What the guard strips on top of that is `spec.signerName` and each condition's `message` — free
/// text the API server wrote (`impl Bounded for CertificateRequestSnapshot`).
pub(crate) async fn certificate_requests(
    client: &Client,
    deadline: std::time::Duration,
) -> Option<Vec<CertificateRequestSnapshot>> {
    whole_list::<CertificateSigningRequest, _>(Api::all(client.clone()), deadline).await
}

/// **Every ReplicaSet the cluster has** — Waste's *parked at 0 replicas* row
/// ([`crate::rules::ClusterSnapshot::replica_sets`]).
///
/// **Not the owner cache, and the owner cache may not stand in for it** (§ RESOLVING AN OWNER).
/// The argument is a **subset** one and not an emptiness one: that map is selected by *some pod
/// named this as its controller*, and this row asks *which ReplicaSets want zero copies*. A list
/// selected by one predicate cannot answer a question asked by another, however much the two
/// overlap — so the cache would answer *nothing is parked* from whatever happened to be in it.
///
/// **It is deliberately not the stronger claim.** *The cache structurally cannot contain a parked
/// ReplicaSet* was written here and is false (`k8s-admin`, 2026-08-29): a ReplicaSet scaled to 0
/// keeps its pods while they terminate — a grace period, a stuck finalizer, an unreachable node —
/// and every one of those pods still names it as controller. The subset argument survives that
/// case; the emptiness one does not, and a wrong reason is what gets repaired into a wrong fix.
pub(crate) async fn replica_sets(
    client: &Client,
    coverage: &Coverage,
    deadline: std::time::Duration,
) -> Option<Vec<WorkloadSnapshot>> {
    whole_list::<ReplicaSet, _>(scoped(client.clone(), coverage), deadline).await
}

/// **Every Service the cluster has** — Waste's *the 503 nobody can explain*, the selector half
/// ([`crate::rules::ClusterSnapshot::services`]).
pub(crate) async fn services(
    client: &Client,
    coverage: &Coverage,
    deadline: std::time::Duration,
) -> Option<Vec<ServiceSnapshot>> {
    whole_list::<Service, _>(scoped(client.clone(), coverage), deadline).await
}

/// **Every EndpointSlice the cluster has** — the other half of that row, and the reason it does
/// not have to match a selector against every pod in the cluster
/// ([`crate::rules::ClusterSnapshot::endpoint_slices`]).
///
/// **Both halves or neither**: `rules.rs` says the row needs Services *and* slices to be `Some`,
/// because slices missing beside Services present reads as *every Service matches nothing*. That
/// pairing is the report's to enforce and this function cannot — which is why the two are fetched
/// together by [`report_lists`] rather than one at a time by whoever remembers.
pub(crate) async fn endpoint_slices(
    client: &Client,
    coverage: &Coverage,
    deadline: std::time::Duration,
) -> Option<Vec<EndpointSliceSnapshot>> {
    whole_list::<EndpointSlice, _>(scoped(client.clone(), coverage), deadline).await
}

/// **Every PersistentVolumeClaim the cluster has** — Waste's *a disk was reserved for it and no
/// pod ever mounted it* ([`crate::rules::ClusterSnapshot::claims`]).
pub(crate) async fn claims(
    client: &Client,
    coverage: &Coverage,
    deadline: std::time::Duration,
) -> Option<Vec<ClaimSnapshot>> {
    whole_list::<PersistentVolumeClaim, _>(scoped(client.clone(), coverage), deadline).await
}

/// **Every PodDisruptionBudget the cluster has** — the Drain safety report's whole subject
/// ([`crate::rules::ClusterSnapshot::disruption_budgets`]).
///
/// **`None` here is the direction that report may not be wrong in.** A drain check that answered
/// *this node is ready* over budgets it never read is the false green light in front of a
/// destructive operation NOTES § D46 refuses, which is why the field stays `None` rather than
/// empty and the pane draws *not checked*.
pub(crate) async fn disruption_budgets(
    client: &Client,
    coverage: &Coverage,
    deadline: std::time::Duration,
) -> Option<Vec<DisruptionBudgetSnapshot>> {
    whole_list::<PodDisruptionBudget, _>(scoped(client.clone(), coverage), deadline).await
}

/// **The five on-demand lists a report joins, as one value** — what [`report_lists`] answers and
/// what [`Store::reports_fetched`] files.
///
/// **One struct and not five `Store` fields**, for the reason `rules.rs` gives four workload kinds
/// one type: five fields, five setters and five call-site lines are five places a sixth list has
/// to be remembered, and the names here are the snapshot's own so nothing is positional.
///
/// **Every field is independently `None`.** They are fetched together but refused separately — a
/// role may list Services and not PodDisruptionBudgets — so one refusal silences one row and not
/// the other four (NOTES § D129).
///
/// **No `Debug`**, for [`Store`]'s reason: the rule is mechanical, not a per-field judgement call.
#[derive(Clone, Default)]
pub(crate) struct ReportLists {
    pub(crate) replica_sets: Option<Vec<WorkloadSnapshot>>,
    pub(crate) services: Option<Vec<ServiceSnapshot>>,
    pub(crate) endpoint_slices: Option<Vec<EndpointSliceSnapshot>>,
    pub(crate) claims: Option<Vec<ClaimSnapshot>>,
    pub(crate) disruption_budgets: Option<Vec<DisruptionBudgetSnapshot>>,
}

/// **All five, at once, on one deadline rather than five** (§ WHAT A REPORT ASKS FOR).
///
/// **Concurrent and not sequential, and the number is the reason.** This runs on the startup path,
/// *after* `k8rs: watching — …` has gone to stderr and *before* the first watch, so every second
/// spent here is a second the tool looks connected and draws nothing ([`REPORT_FETCH`]). Awaited
/// one after another the five deadlines add up: a cluster that accepts connections and answers
/// none holds the greeting for fifty seconds instead of ten, and the bound that was put here
/// precisely so the run could not hang would have been spent five times over. `tokio::join!` polls
/// them on **one task** — no thread, no spawn, and `futures_util` not needed for it — so the worst
/// case is one [`REPORT_FETCH`] whatever the count grows to. *How many TCP connections hyper opens
/// underneath* is not claimed here: it depends on whether ALPN negotiated h2, and an earlier draft
/// asserted *one connection* without measuring either (`k8s-admin`, 2026-08-29). Nothing above
/// rests on the answer.
///
/// **What is measured is the wall clock**, which is the claim that matters:
/// `the_five_lists_wait_side_by_side_and_not_one_after_another` holds five unanswered fetches
/// against a 200ms deadline and asserts they come back together — 201ms, against 1008ms for the
/// same five awaited in a row.
///
/// **Five requests and not one.** There is no API that lists five kinds in one call, and the
/// browser's `Table` path is not it (invariant 12 is about *columns*, not about this): a report
/// needs `minAvailable`'s resolved counter and `.spec.selector` as fields, and `Table` gives
/// strings for display (todo.md § Phase 5).
///
/// **Once per session, never on a timer** — the region doc's rule, NOTES § D151. A kubelet that
/// starts waiting to join after this has run is not seen until the next connect, which is the
/// ceiling [`Identity`] already states for the three facts beside it.
pub(crate) async fn report_lists(
    client: &Client,
    coverage: &Coverage,
    deadline: std::time::Duration,
) -> ReportLists {
    let (replica_sets, services, endpoint_slices, claims, disruption_budgets) = tokio::join!(
        replica_sets(client, coverage, deadline),
        services(client, coverage, deadline),
        endpoint_slices(client, coverage, deadline),
        claims(client, coverage, deadline),
        disruption_budgets(client, coverage, deadline),
    );
    ReportLists {
        replica_sets,
        services,
        endpoint_slices,
        claims,
        disruption_budgets,
    }
}

// --- WHAT A REPORT ASKS FOR END ---

// --- WHAT A NODE IS USING START ---
//
// **The one call in this file that runs on a timer, and the only one invariant 6 allows to**
// (todo.md § Phase 5, whose first line is *the one thing that cannot be watched*). Everything
// else is a permanent watch or a single fetch a report opens.
//
// **What a usage number is, measured off the objects rather than argued from the API's shape**:
// each entry carries its own `window`, and on the cluster this box deployed to those windows ran
// **10.006s to 20.056s against a configured 15s resolution** — so the value is an average over a
// period the object states and that period is not even constant between two nodes read in one
// answer. What is *not* claimed here is why watching is unavailable: whether `metrics.k8s.io`
// serves a `watch` verb was not read off a cluster in this turn, and the premise this region
// rests on is the box's, not a measurement of this agent's.
//
// **Thirty seconds, only while a report wants it, and it never stops** ([`METRICS_POLL`],
// [`node_usage_poll`]). *Only for what is on screen* is `main.rs`'s `--analysis`, the same gate
// § WHAT A REPORT ASKS FOR's six fetches sit behind, because there is no pane to open yet.
//
// ## Why every state keeps polling, including the two that look final
//
// **It shipped stopping on [`crate::rules::Metrics::NotInstalled`] and
// [`crate::rules::Metrics::Denied`], and that was a defect** (`k8s-admin`, 2026-08-29;
// NOTES § D181 carries the cost table). Those two arms draw *"Install metrics-server if you want
// it"* and *"Ask for permission to list nodes in the metrics.k8s.io API group"* — and ended the
// poll in the same tick. The operator does exactly what the screen asked, and the screen keeps
// saying the thing they just fixed until k8rs is restarted, with nothing on it hinting that a
// restart is what is needed.
// **A pane that instructs an action has to be able to observe that action.** It is D181's own
// argument from the other side: probe-gating was refused for printing *install metrics-server* at
// a cluster that had one, and this version could not even self-correct.
//
// **The rule was also applied to the two cheap answers and not to the expensive one.** Measured:
// a `404` costs 11–36 ms and a `403` 12–35 ms, while the `Silent` the design already polled
// forever costs the full ten seconds of [`REPORT_FETCH`]. Stopping saved the two that were nearly
// free.
//
// **And [`Store::unresolved_owners`]'s rule does not reach here, which is what three readings of
// it in a row got wrong.** NOTES § D151's words are *"a standing 403 would otherwise become one
// refused request **per pod per pass**"* — a count that scales with the cluster and with the event
// rate. **Two requests a minute** is not that: it is a heartbeat, on a timer that was already
// running for two of the four states, behind an opt-in flag, and cheaper than the `Silent` case
// the design polled forever anyway. The security gate's *never retries in a loop* is about
// hammering a refused endpoint as fast as the socket allows — § WHAT A THROTTLE LOOKS LIKE's
// subject, and the k9s failure mode `PRIOR-ART § B3` names — not about a bounded cadence. What
// the gate does ask of a `403` is that it degrade one feature and never the session, and that is
// unchanged: the pane draws its own sentence and nothing else on screen moves.
//
// ## The four states, and what picks between them
//
// [`crate::rules::Metrics`] has four arms because a reader acts differently on each, and this
// call's own answer is the only input:
//
// * **200** is [`crate::rules::Metrics::Read`].
// * **403** is [`crate::rules::Metrics::Denied`] — [`Fault::Refused`], *ask for read access*.
// * **404** is [`crate::rules::Metrics::NotInstalled`] — [`Fault::Gone`], which is already this
//   file's *this server says there is no such thing*. That is a fact somebody stated rather than a
//   failed request read as an absence — *who* stated it is the next section, and it is not always
//   the API server.
// * **everything else** is [`crate::rules::Metrics::Silent`] — a `500`, a body that is not a
//   `NodeMetricsList` (that type is where the shape is enforced), and every answer that never
//   arrives at all.
//
// **`503`, `429` and `504` land on `Silent` by the deadline rather than by their status**, and
// [`node_usage`]'s doc has the measurement: kube retries those three under this call, so none of
// them reaches the classifier, and the commonest *installed and not answering* cluster costs one
// [`REPORT_FETCH`] per poll.
//
// ## What a status line is worth here, in both directions
//
// **kube keeps the *body's* `Status.code` and discards the HTTP status line** whenever the body
// parses as a `Status` (`client/mod.rs:551-559`). [`answer`]'s own doc writes one direction down:
// a refusal whose body is JSON that is **not** a `Status` arrives with its code already gone and
// lands on `Silent` — the harmless miss, because it names nothing to go and install.
//
// **The other direction is this region's to own, because the consequence here is not the usual
// one.** A body carrying `code: 404` makes *any* HTTP status
// [`crate::rules::Metrics::NotInstalled`] and one carrying `Forbidden` makes it `Denied` —
// measured: a `500` whose body is `Status{code:404}` is `NotInstalled`, a `404` whose body is
// `Status{code:403}` is `Denied`, and a `403` whose body is `Status{code:200}` is `Silent`
// (`tester`, 2026-08-29).
//
// **What makes that this region's problem is that this is the first caller whose behaviour the
// classification changes.** § WHAT A REPORT ASKS FOR discards it — every failure there is one
// `None`, so a misread code selects between one outcome — and § WHAT WENT WRONG's other readers
// pick a sentence. Here it picks a sentence **and** decides whether to keep asking
// ([`node_usage_poll`]), so a middlebox that writes its own error bodies can take live usage off
// the screen until k8rs is restarted.
//
// **Not guarded, and that is the choice rather than an oversight.** [`answer`] is the one
// classifier (§ WHAT WENT WRONG) and a second reading of the status line beside it is exactly the
// divergence this file refuses. What is owed instead is that the two shapes which *stop* the poll
// are fed rather than reasoned about, and they are.
//
// ## Why the capability probe is not consulted
//
// **[`Served::capabilities`] is deliberately *not* an input** (NOTES § D181, which holds the whole
// argument and is not restated here). What that costs on a cluster with no metrics-server is one
// `404` every [`METRICS_POLL`], at 11–36 ms each — **not** *one per session*, which is what this
// sentence said while the poll could stop and which deleting that rule made false.
//
// **What it buys was measured on a live cluster, and it is the sentence the four states rest on.**
// metrics-server was deleted and the endpoint polled through the restart: **74 polls, 61 ×
// `503 ServiceUnavailable`, 13 × `200`, and zero `403`**, over two restarts that both recovered
// (`k8s-admin`, 2026-08-29). So a registered aggregated API whose backend is gone answers **`503`**
// — not the `404` that would have been read as *not installed*, and not a `403`. With kube
// retrying `503` for the full deadline that lands on [`crate::rules::Metrics::Silent`] —
// *installed and did not answer* — and `Silent` keeps polling, so the screen recovers on its own
// exactly as the cluster did.
//
// **What that run measured is a *pod* restart, and it is the narrower claim.** No transient `403`
// appeared in those 74 polls. That mattered while the poll could stop — a transient refusal would
// have ended it for good — and it is a smaller fact now that nothing stops: a state this poll
// enters wrongly is a state it leaves again within thirty seconds. An RBAC change, the
// `system:auth-delegator` binding, was **not** measured and is now the same shape as any other:
// the screen follows it, in both directions, without a reconnect.
//
// **A `401` is [`crate::rules::Metrics::Silent`] and not `Denied`**, which is a choice between two
// wrong sentences rather than a right one. An expired login is not a missing permission, so *ask
// for permission to list nodes in the metrics.k8s.io API group* would send the reader to their
// cluster administrator over a `kubectl` login they can renew themselves. **Since NOTES § D187
// that arm names an RBAC verb, a resource and an API group, which makes it a sharper example and
// not a weaker one**: the vaguer wording sent the reader somewhere unhelpful, and this one hands
// their administrator an exact Role rule to add — one that changes nothing, because a `401` is
// not an RBAC answer at all. *Did not answer* is at least true of what happened.
// The reader is not left with only this: every watch is failing at the same moment and
// `main.rs`'s `unreadable` names the expired credential and the program to renew it with.

/// **How often the metrics API is asked** — thirty seconds, the floor todo.md § Phase 5 names.
///
/// **It is a constant and not a knob**, which is the box's own line. A number the reader can turn
/// down is a number somebody sets to one second, and the windows measured on a live cluster were
/// 10s to 20s wide (the region above) — so a poll faster than the interval is a round trip for an
/// average that may not have moved.
///
/// **`MissedTickBehavior::Delay` in [`node_usage_poll`] is what makes *30s+* literally true.**
/// Tokio's default is `Burst`, which schedules the next tick relative to the one that was missed —
/// so a fetch that took the whole of [`REPORT_FETCH`] would be followed immediately by another.
/// With `Delay` the gap between two polls is never less than this, whatever the answer cost.
///
/// **The `+` is the answer's own cost, and on one cluster it is not small** — the period is this
/// constant *plus* however long the fetch took, so it is a floor and not the period
/// (`k8s-admin`, 2026-08-29). Measured per answer:
///
/// | what the cluster answers | the fetch costs | so the period is |
/// |---|---|---|
/// | `200` — a reading | milliseconds | ~30s |
/// | `404` — [`crate::rules::Metrics::NotInstalled`] | 11–36 ms | ~30s |
/// | `403` — [`crate::rules::Metrics::Denied`] | 12–35 ms | ~30s |
/// | `503` — [`crate::rules::Metrics::Silent`] | 10s, the whole of [`REPORT_FETCH`] | **40s** |
///
/// **The one that is 40s is the one that is retried**, and only that row: a cluster whose
/// metrics-server is down is asked once every forty seconds and not every thirty. Nothing on
/// screen depends on the difference — the pane draws the same sentence throughout, and the
/// recovery it is waiting for arrives one period late at worst.
const METRICS_POLL: std::time::Duration = std::time::Duration::from_secs(30);

/// **The one path this region asks for** — every node's usage, cluster-scoped, one request.
///
/// **Written out rather than built from an [`ApiResource`]**, which is § THE BROWSER'S ROWS'
/// pattern and not this one's: `k8s-openapi` ships no `metrics.k8s.io` types (the group is served
/// by an aggregated API server and is not part of the core spec), so there is no `Api<K>` to take
/// a path from and nothing here to disagree with. **Nothing the cluster wrote is interpolated into
/// it** — it is a literal, so [`path_safe`]'s class of failure cannot arise.
const METRICS_NODES: &str = "/apis/metrics.k8s.io/v1beta1/nodes";

/// What the metrics API calls its own answer — checked in [`node_usage`], never assumed
/// (`tester`'s F3, 2026-08-29). [`TABLE_KIND`] is the same field read for the same reason.
const METRICS_KIND: &str = "NodeMetricsList";

/// The group version whose **units** this code has read off a live cluster ([`UsageResponse`]) —
/// checked beside the kind, because a later version that changed them would decode perfectly here
/// and print wrong numbers.
const METRICS_VERSION: &str = "metrics.k8s.io/v1beta1";

/// **The body of a `NodeMetricsList`, cut to the three values a snapshot needs** — and the cut
/// *is* the prune (invariant 6, § THE INGEST GUARD's module doc).
///
/// **Hand-rolled because there is nothing to derive it from.** `k8s-openapi` has no
/// `metrics.k8s.io` module at any feature level; the group is an aggregated API metrics-server
/// serves, and the alternative — `DynamicObject` — would name the resource by hand *and* keep the
/// whole object, which is the prune given up for nothing.
///
/// **`timestamp` and `window` are not named here, so serde never builds them.** `window` is the
/// averaging period the object states, measured at 10.006s–20.056s on a cluster configured for
/// 15s; nothing on screen says it today, and a field with no reader is one nobody can prove
/// ([`Column`] drops discovery's `shortNames` under the same rule).
///
/// # Nothing defaults, and that is the opposite of [`TableResponse`]
///
/// **The decode is the only thing standing between a `200` and a reading**, so every field this
/// type names is required and a body missing one is not a `NodeMetricsList`. While they defaulted,
/// anything that was JSON at all decoded — measured: `{}`, `[]` and a `Status` carrying
/// `code: 403` each came back `Read({})`, which draws no measurement **and no sentence saying
/// why** (`tester`'s F3, 2026-08-29). An authorizing proxy answering `200` with a JSON error body
/// is the ordinary way to reach it, and *the pane going quiet* is exactly what
/// [`crate::rules::Metrics`]'s four arms exist to prevent.
///
/// **[`TableResponse`] defaults for a reason this type does not have**: one decode there covers
/// two shapes the server chooses between, so a missing field is a legal answer. There is one shape
/// here.
///
/// **`Read(∅)` stays reachable and is a different fact** — `{"kind":…,"apiVersion":…,"items":[]}`
/// is a metrics API with nothing to report, which is *this cluster has none* rather than *nobody
/// looked* (NOTES § D129). What is refused is `Read(∅)` from a body that is not one of these.
///
/// **One rule for every deviation, because two near-neighbours behaving oppositely is what
/// `tester` measured next**: a wrongly-typed `usage.cpu` (a number, not a quantity string) lost the
/// whole reading while a *missing* `usage` degraded one node. Both are `Silent` now. A
/// `resource.Quantity` is a string in every Kubernetes API by definition, so a number there means
/// something that is not metrics-server answered — and one entry the schema does not fit is not
/// evidence about the other entries beside it, it is evidence about the writer.
///
/// **Unknown fields are still accepted** (no `deny_unknown_fields`): a metrics-server that adds a
/// field is not a metrics-server that stopped being one, and refusing those would be a pin on a
/// version rather than on a shape.
#[derive(Deserialize)]
#[serde(crate = "k8s_openapi::serde", rename_all = "camelCase")]
struct NodeMetricsList {
    /// The server's own word for what it sent, checked against [`METRICS_KIND`] — [`Table`]'s
    /// `kind` branch reads the same field for the same reason, and a `PodMetricsList` reaching
    /// this decode would otherwise be filed as nodes.
    kind: String,
    /// Checked against [`METRICS_VERSION`], because the **units** are what this code knows off
    /// that group version and no other (the reading is on [`UsageResponse`]). A `v2` that changed
    /// them would decode here perfectly and print wrong numbers.
    api_version: String,
    items: Vec<NodeMetricsResponse>,
}

/// One node's entry in that list.
#[derive(Deserialize)]
#[serde(crate = "k8s_openapi::serde")]
struct NodeMetricsResponse {
    metadata: NodeMetricsName,
    usage: UsageResponse,
}

/// **The one field of a node metrics object's `metadata` that is read**: which node this is.
///
/// **Not [`MetadataResponse`], which is one type over and holds three fields.** Two of them —
/// `namespace` and `uid` — have no reader here, and a struct that names only what the snapshot
/// needs is what makes the decode the prune. The saving is small; the property is not.
///
/// **Required, and the `From` impl below therefore has no nameless entry to drop.** The name is
/// the map's *key* and an entry without one could not be joined to any node — so an object that
/// omits it is not a `NodeMetrics`, which is the type's rule above rather than a second one here.
/// The one place an empty key can still appear is the strip, and `impl Bounded for Metrics` is
/// where it is dropped.
#[derive(Deserialize)]
#[serde(crate = "k8s_openapi::serde")]
struct NodeMetricsName {
    name: String,
}

/// **The two quantities, as the metrics API writes them** — and it writes them in units nothing
/// else in this repo sends.
///
/// **Read off a live cluster rather than off upstream's docs** (todo.md § Phase 5, which exists
/// partly to do this): `usage.cpu` is **nanocores** — `76530604n` — and `usage.memory` is
/// **`Ki`** — `1107584Ki`. `rules::quantity_milli` already maps both suffixes, and `76530604n`
/// comes out 77 milli-cores, which is the column `kubectl top nodes` prints.
///
/// **This API also writes `"0"`, with no suffix at all** — observed on the `PodMetricsList` beside
/// the node one, on an idle container, and **not** on a node, which never idles that far. It is
/// fed to the parser here anyway: `quantity_milli`'s `""` arm had never seen a string from this
/// source, and D29 is about the shapes a check was fed rather than the ones it was likely to meet.
///
/// It is a `String` for [`crate::rules::NodeUsage`]'s reason — a quantity stays the API's own text
/// and the caller that needs a number owns the parse.
#[derive(Deserialize)]
#[serde(crate = "k8s_openapi::serde")]
struct UsageResponse {
    cpu: String,
    memory: String,
}

/// **A body that decoded is always [`crate::rules::Metrics::Read`]** — the other three arms are
/// what [`node_usage`] makes of a call that produced no body, and none of them is reachable from
/// here.
///
/// **Nothing is dropped here and nothing can be**, which is a change: an entry with no name used
/// to be filtered out at this line, and the rule moved up into the decode, where a missing name is
/// one of the deviations that makes a body not a `NodeMetricsList` ([`NodeMetricsName`]). The
/// empty key this file must not build can now only come from the *strip*, and
/// `impl Bounded for Metrics` is the one place that drops it — one guard where there were two,
/// and the one that runs last (`tester`'s F2, 2026-08-29).
impl From<NodeMetricsList> for Metrics {
    fn from(list: NodeMetricsList) -> Self {
        Self::Read(
            list.items
                .into_iter()
                .map(|node| {
                    (
                        node.metadata.name,
                        NodeUsage {
                            cpu: node.usage.cpu,
                            memory: node.usage.memory,
                        },
                    )
                })
                .collect(),
        )
    }
}

/// **What every node is using, or which way the question failed** — one request, classified into
/// [`crate::rules::Metrics`]'s four arms (the region above, which is where the mapping is argued).
///
/// **Not a `Result` and not an `Option`, unlike every other fetch in this file.** § WHAT A REPORT
/// ASKS FOR collapses every failure into one silence because the pane it feeds owes the reader no
/// sentence about *why*. This one does owe one — `screens/analysis.md` § *Live usage* writes four
/// different wordings and four different ways out — so the taxonomy is the return type.
///
/// **The request is built through an invariant-1 reader.** `Request::list` supplies the `GET` and
/// is on the allowlist; a hand-rolled `http::Request` with a verb of our own choosing is what
/// NOTES § D142 caught. `ListParams::default()` sends no `limit`, so the answer is whole — the
/// same reasoning [`whole_list`] carries, and it costs less here because the list is one entry per
/// node.
///
/// **Bounded by the caller's `deadline`, because nothing under it is** — `Config::read_timeout` is
/// `None` in every kube constructor. Unbounded, a middlebox that accepts the connection and
/// answers nothing would hold this poll's stream forever and the *next* poll would never be sent,
/// so the pane would keep drawing a reading that stopped being true (the failure [`REPORT_FETCH`]
/// was added for, one stream along).
///
/// **And the deadline is what ends three status codes as well as the hang**, which was measured
/// here rather than inherited: kube retries **`429`, `503` and `504`** inside a tower layer with
/// no callback, so none of the three ever reaches the classifier below — `500` and `502` come back
/// in a millisecond and those three run to the deadline. That is NOTES § D148's fifteen retries
/// seen from this call, and it is the ordinary shape of the failure this box cares most about: an
/// aggregated metrics API whose backend is down answers **`503`**, so *metrics-server is installed
/// here but did not answer* costs one [`REPORT_FETCH`] every [`METRICS_POLL`] to keep saying. Both
/// halves are pinned by `the_deadline_and_not_a_status_is_what_ends_a_throttled_metrics_api`,
/// which fails if kube's retry layer ever covers a different set.
///
/// **What that costs the cluster is counted, not estimated: 10–11 requests** inside these ten
/// seconds — measured against the stub at this deadline, three retried statuses, and 5–7 at 300ms
/// (`dev-core`, 2026-08-29). It brackets slightly below the eleven-or-twelve D148's constants
/// predict, which is what jitter drawing under its mean looks like. The test beside it asserts
/// *more than one* rather than the figure, at a deadline a suite can afford; the figure is here
/// because it is the number an operator pays every [`METRICS_POLL`] while metrics-server is down.
///
/// **Every string goes through [`ingest`] like every other object in this file** — invariant 9,
/// and the node name is a map *key*, which is the shape `impl Bounded for Metrics` exists for.
///
/// **What is bounded is every field; what is not is the entry count** — [`whole_list`]'s ceiling
/// exactly, named here rather than restated, and one entry per node makes it the smallest of the
/// seven calls that carry it. The response body is collected whole before the decode, so the peak
/// is the body plus the decoded `Vec`, and this repeats every [`METRICS_POLL`] where the six
/// fetches run once — which is why it matters that the list is short.
pub(crate) async fn node_usage(client: &Client, deadline: std::time::Duration) -> Metrics {
    let Ok(asked) = Request::new(METRICS_NODES).list(&ListParams::default()) else {
        return Metrics::Silent;
    };
    match tokio::time::timeout(deadline, client.request::<NodeMetricsList>(asked)).await {
        // **The body says what it is, and it is checked.** [`NodeMetricsList`] has already refused
        // everything not shaped like one; these two refuse the two that *are* — a `PodMetricsList`
        // decoded as nodes, and a group version whose units this code has never read.
        Ok(Ok(list)) if list.kind == METRICS_KIND && list.api_version == METRICS_VERSION => {
            ingest(list)
        }
        Ok(Ok(_)) => Metrics::Silent,
        // **The one classifier, not a second reading of a status code** (§ WHAT WENT WRONG).
        Ok(Err(failure)) => match fault(&failure) {
            Fault::Refused => Metrics::Denied,
            Fault::Gone => Metrics::NotInstalled,
            _ => Metrics::Silent,
        },
        // The deadline. Nothing answered, which is the same thing to say as a `503`.
        Err(_) => Metrics::Silent,
    }
}

/// **[`node_usage`] on a timer, as one more stream for [`drive_watching`]** — the whole of how a
/// poll reaches a store the watch loop holds `&mut`.
///
/// **An [`Update`] and not a task, so nothing here needs a lock.** The store is owned by the pump
/// and mutated only between events; a second task holding it would need a mutex, and a mutex over
/// the store is a second way for a snapshot to be read half-written. `select_all` merges this with
/// the five watches and the closure lands on the same single-threaded loop every event does.
///
/// **The first poll is immediate**: `tokio::time::interval` completes its first tick at once, so a
/// report drawn seconds after connect already carries usage rather than an empty slot for half a
/// minute. Every tick after it is at least [`METRICS_POLL`] apart, which is what
/// `MissedTickBehavior::Delay` buys.
///
/// **It never stops, and every one of the four states keeps polling** — there is no branch here
/// and that is the fix, not an omission (the region above, NOTES § D181). It shipped ending on
/// `NotInstalled` and `Denied`, so a pane that told the operator to install metrics-server could
/// not see them do it. One cadence for four states is also less code than the rule it replaced.
///
/// **So the reader watching the screen sees every direction of travel**: a metrics-server that is
/// restarting comes back, one that is installed starts answering, and a role that is granted the
/// verb starts working — each within one [`METRICS_POLL`], without touching anything.
///
/// **Like the five watches, this stream cannot end**, which is what `select_all` in
/// [`drive_watching`] wants: [`updates`] appends an end marker precisely because a watch that
/// finishes would otherwise be read as live, and there is nothing here that can finish.
pub(crate) fn node_usage_poll(client: Client) -> BoxStream<'static, Update> {
    let mut ticker = tokio::time::interval(METRICS_POLL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    stream::unfold((client, ticker), |(client, mut ticker)| async move {
        ticker.tick().await;
        let metrics = node_usage(&client, REPORT_FETCH).await;
        let update = Box::new(move |store: &mut Store| store.metrics_polled(metrics)) as Update;
        Some((update, (client, ticker)))
    })
    .boxed()
}

// --- WHAT A NODE IS USING END ---

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

/// **One ReplicaSet a pod names as its controller, which the cache cannot answer for**, and why.
///
/// The shape is [`Listing`]'s: facts, not sentences, in namespace-then-name order.
///
/// **It derives `Debug` where [`Store`] deliberately does not**, for [`Listing`]'s reason: an
/// identity and a [`Fault`] are values that never touched a credential.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved {
    /// The ReplicaSet, exactly as the pod's `ownerReference` named it — namespace, name and the
    /// uid the fetch is keyed and checked on.
    pub id: ObjectId,
    /// **Why this reference has no object yet, or `None` for *nothing has asked*.**
    ///
    /// `None` is the ordinary state of every ReplicaSet a new pod names, and the one state that
    /// is an instruction: these are the references the caller fetches. Everything else is what
    /// one fetch came back with, classified once (§ WHAT WENT WRONG) — this field held its own
    /// four-way enum until 2026-08-27, and that enum read a `401` as *k8rs could not ask*.
    pub why: Option<Fault>,
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

impl Store {
    /// **Every ReplicaSet a live pod names as its controller that the cache has no object for**,
    /// one entry per ReplicaSet however many pods name it.
    ///
    /// **Two callers, one list, for [`Store::still_listing`]'s reason.** The fetcher takes the
    /// not-yet-asked entries; a screen shows the rest, so a heading that stayed at the
    /// generated name always has a fact behind it and never a silence.
    ///
    /// **A failure stays in the answer and is therefore not asked again.** A `403` on
    /// `replicasets` is a standing fact about the kubeconfig's role, and a caller that re-read
    /// it as *not asked* would send one refused request per pod per pass — the retry loop the
    /// security gate forbids by name. **Nothing here retries, ever, and the ceiling that names
    /// is a transient [`Fault::Unanswered`]** — a socket that died once leaves the heading at
    /// ReplicaSet for the life of the process. Retry policy belongs to the reconnect box, which
    /// is where per-watch identity arrives; this half is the one that is true without it. The
    /// store-wide `failure` field was the same shape and has since been replaced by one per
    /// watch (NOTES § D162); **this heading's retry has not**, and no box has claimed it.
    pub fn unresolved_owners(&self) -> Vec<Unresolved> {
        let mut found = BTreeMap::new();
        for pod in self.pods.live.values() {
            let Some(uid) = owner_uid(&pod.owner) else {
                continue;
            };
            let why = match self.owners.get(uid) {
                None => None,
                Some(Ok(_)) => continue,
                Some(Err(fault)) => Some(*fault),
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
    /// **An object that comes back under a different uid is [`Fault::Gone`]**, not an answer. The
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
                    Err(Fault::Gone)
                }
            }
            Err(error) => Err(fault(&error)),
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
// [`Store::snapshot`] is `None`, [`Store::troubles`] is empty, and the only honest thing on this
// store is [`Store::still_listing`]. **That is A3 one layer lower**: not a queue whose depth
// could be drawn, just a wait.
//
// **The retry is kept on, deliberately.** `default_retry: false` would put every 429 straight
// onto [`Trouble::failure`] where a screen could name it — but kube's bare `watcher()` restarts
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
// `:35`, `:43`) — typed, and kept typed by [`Trouble::failure`] (NOTES § D145, § D162).
//
// **A fourth variant carries a `Status` and does not carry it that way: `WatchError(Box<Status>)`
// (`watcher.rs:39`).** It holds the `Status` **directly**, not behind `Error::Api`, and it is the
// one a busy cluster produces most — the 410 desync that ends a watch (`:702`) and an in-band 403
// arrive on it. A formatter written from the three-variant list above unwraps `Error::Api`, finds
// nothing, and falls through to a generic message for the commonest watch failure there is, which
// is `PRIOR-ART § C1` exactly. **Four variants carry a `Status`; three of them wrap it.**
//
// **A dead connection, and the two halves of it are not the same story.** `Config` sets
// `connect_timeout` 30 s and `write_timeout` 295 s (`config/mod.rs:418-419`) and leaves
// **`read_timeout` unset** (`:191`, `:273`, `:339`); the connector is a bare
// `HttpConnector::new()` (`client/builder.rs:117`) whose `TcpKeepaliveConfig::default()` is
// all-`None`, so `into_tcpkeepalive()` yields `None` and `set_tcp_keepalive` is never called
// (`hyper-util-0.1.20/src/client/legacy/connect/http.rs:94-98`, `:104-110`, `:842-843`).
// **SO_KEEPALIVE is off on the watch sockets**, so a connection that dies without a FIN or an
// RST — a laptop suspending, a NAT entry expiring, a load balancer dropping an idle flow —
// raises no error at the socket at all. What happens next depends on which call is waiting.
//
// **The watch is bounded, and kube does it above the socket.** `next_with_idle_timeout` wraps
// the stream poll in a `tokio::time::timeout` of `Config::timeout.unwrap_or(290)` plus a 5 s
// margin (`watcher.rs:483`, `:494`) and is used by both watching states (`:589`, `:659`). So a
// severed watch unblocks after **~295 s** and reconnects with nobody at the keyboard — kube
// found a period it could safely use, and it is the watch's own `timeoutSeconds=290` rather
// than a client-wide read deadline. **The timeout arm returns `None`, not `Err`** (`:714`,
// which goes to `State::InitListed` and re-watches from the stored resourceVersion without
// re-listing), so for up to five minutes the store serves a frozen cluster and
// [`Store::troubles`] is **correctly empty**. Stale, silent, self-healing, bounded — and the
// silence is the honest answer, because nothing failed.
//
// **The initial LIST is genuinely unbounded, and that is where `PRIOR-ART § A7` stands.**
// `next_with_idle_timeout` does not wrap `State::InitPage`'s `api.list()`, and no deadline
// reaches the wire either: `to_list_params` copies `timeout` into the `ListParams`
// (`watcher.rs:400`) and `ListParams::populate_qp` never serialises it
// (`kube-core-4.2.0/src/params.rs:94-122`) — `timeoutSeconds` is appended in exactly one place
// in that crate, `:381`, which is the **watch** builder. `ListParams::timeout`'s own doc says
// *"Defaults to 290s"* (`:137-139`) and the query builder one screen away disagrees with it.
// So a LIST against a dead socket blocks forever: [`drive`] waits, [`Store::troubles`] stays
// empty, and [`Store::still_listing`] is the only thing with anything to say. **Not fixable
// from here** — `read_timeout` is client-wide and a healthy watch is idle for long stretches,
// and kube's params doc says clients "should not assume bookmarks are returned at any specific
// interval" (`:329`). That is the *deadline on the first watch sync* box, next in this phase.
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
pub(crate) type Update = Box<dyn FnOnce(&mut Store) + Send>;

/// One `watcher()` stream, ready for [`drive`].
///
/// `of` picks the [`Watch`] the stream feeds — `|store| &mut store.pods` for a Pod watch — and
/// this is the only line in the driver where a kind is named at all.
///
/// **One argument and not two, so the identity cannot disagree with itself** (NOTES § D162). The
/// first draft took the [`Store`] method *and* somewhere to record a failure, and
/// `updates(pods, Store::node, …pods…)` would have compiled. Everything this stream does now
/// goes through the one watch `of` returns: the events, the failure, and the end of it.
///
/// **Both arms produce an [`Update`], so the item type carries no `Result`** — an `Err` is a
/// thing that happened to one watch and is recorded on it, exactly as an event is. That is the
/// same refusal [`drive`]'s old `Err` arm made, moved to the only place that knows *whose*
/// failure it is.
///
/// **The last item is always the end of the stream**, appended with `chain`: `select_all` drops
/// a stream that finishes and says nothing, so the marker has to be inside the stream that is
/// about to finish. It is what keeps a kind that stopped from being read as live.
///
/// # One stream per [`Watch`], for the life of the process
///
/// `of` is a plain `fn` pointer and nothing stops a second `updates(…, |s| &mut s.pods)`. **Do
/// not.** `ended` is deliberately never cleared (NOTES § D162), so the moment a second stream
/// feeds the same watch, `Trouble { kind: Pod, ended: true }` is permanent while pods stream in
/// normally — a banner saying *stopped* about a live watch, which
/// `objects_arriving_again_do_not_take_back_the_end_of_a_watch` pins as the behaviour. **A
/// reconnect resubscribes below `drive`, inside the stream `of` already names, and never by
/// building a second one for the same `of`.**
///
/// **For this file that is free, and it is `connect()` that walks into it.** kube's `watcher()`
/// is `stream::unfold` whose closure returns `Some(..)` unconditionally (`watcher.rs:791-797`),
/// so it provably cannot end and the marker never fires. But the obvious way to add the backoff
/// [`drive`] asks the caller for is `StreamBackoff`, whose doc reads: *"If
/// `Backoff::next_backoff` returns `None` then the backing stream is given up on, and closed"*
/// (`utils/stream_backoff.rs:9-14`) — k9s's `BailOut` inside kube's own utility. That is what
/// `ended` is a defence against, and [`StandingBackoff`] is the policy that never returns
/// `None`.
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
fn updates<K, T>(
    watch: impl Stream<Item = watcher::Result<Event<K>>> + Send + 'static,
    of: fn(&mut Store) -> &mut Watch<T>,
) -> BoxStream<'static, Update>
where
    K: Send + 'static,
    T: Watched + From<K> + Bounded + Send + 'static,
{
    watch
        .map(move |next| match next {
            Ok(event) => {
                let now = Time(Timestamp::now());
                Box::new(move |store: &mut Store| of(store).take(&now, event)) as Update
            }
            Err(failure) => {
                Box::new(move |store: &mut Store| of(store).failure = Some(failure)) as Update
            }
        })
        .chain(stream::once(async move {
            Box::new(move |store: &mut Store| of(store).ended = true) as Update
        }))
        .boxed()
}

/// **Every watch into one store, until they all end** — and **nothing here can make it end
/// early** (NOTES § D162, `PRIOR-ART § B3`).
///
/// **There is no error handling left in this function, and that is the point.** [`updates`]
/// turns an `Err` into an [`Update`] against the watch that raised it, so this loop has no
/// `Result` to unwrap and **no place a `?` could be written** — the failure that killed k9s's
/// reconnector permanently ([#3922](https://github.com/derailed/k9s/issues/3922)) cannot be
/// reintroduced here by an edit, only by rewriting [`updates`]. **There is no retry budget
/// either**: nothing counts failures and nothing returns because there have been enough of
/// them, so a cluster that goes away over lunch is a state on screen and not an exit.
///
/// **`PRIOR-ART § B3`'s rule is *retried forever*, not *retried as fast as the socket allows*,
/// and the second half is the caller's to pay.** kube's own `Error` doc says it
/// (`watcher.rs:26`): *"To avoid constantly looping errors, make sure backoff is applied."*
/// Backoff is **opt-in** — `watcher()` restarts "normally immediately" and `StreamBackoff` is
/// something you wrap it in (`:777-779`) — so a watch the cluster refuses runs
/// `Err → Init → list() → 403` bounded only by round-trip time, which is § WHAT A THROTTLE
/// LOOKS LIKE's own warning pointed back at us. **This function cannot fix it**: it takes an
/// `impl Stream` and never builds a `watcher()`, so **the caller owes a [`Backoff`]** and
/// `connect()`'s box carries it. Not `.default_backoff()`, which a refused watch resets on every
/// `Ok(Init)` and so never slows down at all — [`StandingBackoff`] is that reset silenced, and
/// § CONNECTING has the measurement. It slows down forever and never gives up:
/// `ExponentialBackoff::new` calls `.without_max_times()` (`:930`).
///
/// **And if it returns anyway, the caller does not exit.** Every stream ending is not a reason to
/// stop: `drive` returning `()` means nothing is being watched any more, which is a state to
/// draw — every kind is in [`Store::troubles`] with `ended` — and not a shutdown. A `main` that
/// lets this fall off the end takes the tool down for exactly the reason this box exists to
/// prevent.
///
/// **A failure reaches its own watch and no other.** It cannot open the bootstrap gate — only
/// `InitDone` on a watch that saw its `Init` does that (NOTES § D28) — and it cannot be erased
/// by the four healthy watches beside it, which is the whole of what per-watch identity bought
/// (NOTES § D145).
///
/// **The ceiling `select_all` left is closed rather than inherited.** It still drops a stream
/// that finishes, and the loop still runs on with the rest — but [`updates`] appends an end
/// marker *inside* each stream, so the kind that stopped is recorded as stopped instead of
/// sitting frozen and being read as live. kube documents a `watcher()` stream as recovering on
/// the next poll rather than finishing, which is read off its doc and not off a cluster; that a
/// **real** severed socket comes back with nobody touching the keyboard is the `connect()`
/// box's proof and not this function's (NOTES § D161).
///
/// **It is [`drive_watching`] with nobody watching**, so every test of the pump below is a test
/// of that one too — Rust has no default arguments and this is the shape that costs no call site.
async fn drive(watches: Vec<BoxStream<'static, Update>>, store: &mut Store) {
    drive_watching(watches, store, |_| {}).await;
}

/// **[`drive`], with the store handed to somebody after every update** — the same pump, and the
/// only difference is that a caller can see what changed.
///
/// **`watching` is told *after* the update lands and is given the store whole**, because what an
/// event meant is a question only the store can answer: one `Apply` can add a finding, remove
/// three, or change nothing at all. It takes `&Store` rather than a snapshot so the gate is still
/// the caller's to ask about — [`Store::snapshot`] answers `None` all the way through a bootstrap
/// (NOTES § D28) and [`Store::still_listing`] is what there is to say meanwhile.
///
/// **It cannot end the loop.** The closure returns `()`: there is no `Result` for a `?` to sit
/// on and no `bool` for it to stop on, so the failure this file exists to prevent cannot be
/// reintroduced through the observer either.
///
/// **No clock and no timer** (invariant 7): this fires once per update, and coalescing storms is
/// the caller's, where the frame is. The temporary driver in `main.rs` prints only when the
/// report it renders differs from the last one, which is the cheapest form of it.
pub(crate) async fn drive_watching(
    watches: Vec<BoxStream<'static, Update>>,
    store: &mut Store,
    mut watching: impl FnMut(&Store),
) {
    let mut merged = select_all(watches);
    while let Some(update) = merged.next().await {
        update(store);
        watching(store);
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

// --- WHAT ELSE THE CLUSTER SERVES START ---
//
// **The optional half of the same answer, and it costs nothing to read** (NOTES § Capability
// probe, § D152). The region above turns the discovery result into a sidebar; this one asks it a
// second question — *does this cluster have metrics-server, PodDisruptionBudgets, cert-manager,
// kube-prometheus-stack, a mesh?* — with no request of its own. D152's closing line is the
// promise: a lookup over what `run_aggregated()` already returned, zero extra round trips.
//
// **These group names are written down here, and that is not invariant 12 broken.** That
// invariant is about the *browser*: a sidebar built from a list of kinds is a design failure
// because the cluster already knows the answer. Nothing knows whether cert-manager is worth
// asking about except k8rs, and a probe that could not spell `cert-manager.io` would answer
// nothing at all. What stays true is that no *browsable kind* is named — every string below
// belongs to a feature this repo has decided to build, and § EVERY KIND THE CLUSTER SERVES
// still names none.
//
// **Rule 1 of that section is why this returns what is *there* rather than what is missing.** A
// missing capability is stated and never hidden — the Analysis row stays and reads *needs
// metrics-server — not installed in this cluster* — so the feature is the one that owns the
// sentence, and it can only write it if it can ask. A list of absences would have to be a list of
// everything k8rs might ever want, which is the hard-coded list invariant 12 refuses one layer up.
//
// **Rule 2 is why nothing here is an address.** Presence comes from the API; *where to reach
// Prometheus* comes from the reader and from nowhere else, never from a pod annotation
// (REQUIREMENTS § DevSecOps, SSRF). This function returns no string at all, which is that rule
// made structural rather than remembered.
//
// ## Nothing discovered is not the same fact as nothing installed
//
// **Failure 1 above is a lie waiting for a consumer.** A server too old for the aggregated call
// answers `Ok` with zero groups, and a probe keyed on group presence then reads *absent* for
// every capability — so a cluster with metrics-server, cert-manager and Istio on it is told, once
// per feature, to install the thing it already has. That is invariant 14 broken in the worst
// direction: not jargon, but a plain sentence that is false.
//
// **So the answer is `Option`, and the empty input is the `None`.** It is the distinction
// `ClusterSnapshot`'s six `Option<Vec<_>>` fields already draw — *nobody asked* against *asked and
// there are none* — and **for an unfiltered answer** the check is exact: a working API server
// always serves `v1`, so a discovery answer that named no resource at all did not come from a
// cluster with nothing on it. **That premise is not unconditional**: the trimmed-discovery
// paragraph below says what withdraws it, and what `connect()` owes as a result.
//
// **[`browsable`] refuses to interpret the same emptiness and that is not a contradiction.** An
// empty sidebar is *visible*: the reader sees no rows and knows something is wrong without being
// told. An absent capability is invisible by construction — its whole output is one sentence
// about a thing that is not on the screen — so the same input has to be handled where it is
// consumed, and here it is consumed as prose.
//
// **A group the server names but does not fill is invisible here, and that is the constraint
// `connect()` inherits.** kube builds one pair per entry in a group version's `resources` array
// (`kube-client-4.2.0/src/discovery/parse.rs:94-108`), so a version that came back with an empty
// array contributes no pair at all — and failure 3 above has already discarded `freshness`, the
// one field that would have named it. A registered APIService whose backend is down is exactly
// that shape. So a row missing from `Some(set)` means **no resource of that group was named**,
// never **that group is not registered**: the probe is a floor and not a census.
// [`crate::rules::Metrics::Silent`] is written for the cluster this produces — installed, and not
// answering — so a caller that routes on this set alone sends that reader to `NotInstalled` and
// tells them to install what they already have, which is invariant 14 broken in failure 1's
// direction.
//
// **Nothing in this file draws that distinction, and no version of it has.** An earlier draft of
// this paragraph said `connect()` owned it "out of the direct `list_api_groups_aggregated()`
// call": there is no such call — [`served`] goes through `Discovery::new(..).run_aggregated()`,
// which is where `freshness` is discarded — and a reader who believed the sentence would have
// routed a cluster that *has* metrics-server to `NotInstalled` on the strength of it
// (`k8s-admin`, 2026-08-27). Measured: a `v1beta1.metrics.k8s.io` APIService whose Service does
// not exist produces a banner byte-identical to a cluster with no metrics-server at all. **The
// distinction is owed by the box that first routes on [`Served::capabilities`]** — the freshness
// field is one direct `/apis` call away and this file does not make it — and until that box lands
// the only honest reading of a missing row is the floor above.
//
// **That box has landed, and it discharged the debt by not routing on this set at all**
// (§ WHAT A NODE IS USING). [`crate::rules::Metrics`]'s four arms come off the metrics call's own
// status, so *registered and down* and *not installed there* are told apart by what the aggregator
// answers rather than by a row that is absent for both reasons. Nothing above changes: this
// function is still a floor and not a census, and the next feature to key on a missing row
// inherits the same debt with no precedent for paying it this way.
//
// **A trimmed discovery answer is the same shape and is `connect()`'s to not build.**
// `Discovery::filter`/`exclude` set a mode every group is gated on before it is kept
// (`kube-client-4.2.0/src/discovery/mod.rs:24-29`, applied at `:182` and `:190`), so a discovery
// narrowed to trim the sidebar comes back non-empty, the guard below never fires, and every
// capability reads absent as `Some(∅)` — and `filter` drops the core group with the rest
// (`CORE_GROUP` is `""`, `apigroup.rs:207`), so the *a working server always serves `v1`* reasoning
// the guard rests on stops holding at the same moment. Only an unfiltered answer may reach here.
//
// ## Registered is not running, and the word *installed* is stronger than the fact
//
// **A served group is a floor on what the cluster once had, never proof the product is running.**
// CRDs outlive their operator by design — `helm uninstall` leaves them behind, `istioctl uninstall`
// leaves them without `--purge` — so a cluster whose operator was removed six months ago answers
// this function exactly as one running it does. Measured: cert-manager's CRDs applied with no
// controller ever started still come back `cert-manager.io v1 Current 4`, and
// `kubectl get certificates.cert-manager.io -A` prints *No resources found*
// (`reports/2026-08-26-capability-probe-group-strings.md` § 4).
//
// **The function is right and the sentence a caller writes is what has to be careful.** There is
// nothing better in the discovery answer to read, and asking is still worth it — that is all
// `Some(row)` claims. **[`Capability::Prometheus`] is where the strong word would cost the reader
// something**, because NOTES § Capability probe rule 2 has its feature ask them for an address and
// there is no address for something that is gone; its variant doc below is written in the weak
// word for that reason, and any sentence a later box builds on it has to be too.
// [`Capability::Istio`] is the same shape as a *shipping* configuration, not only a leftover — the
// `remote` profile puts the CRDs here and istiod in another cluster. [`Capability::Metrics`] is
// immune: an APIService is not a CRD and nothing leaves one behind.
//
// ## Why the pairs, and not the sidebar
//
// [`browsable`]'s output is the wrong input for this, twice over, and neither is a preference:
//
// **It has already dropped what nobody can list.** That filter is about opening a row, and a
// capability is not a row — a group whose kinds are all `create`-only is still a capability the
// cluster has. Reading the sidebar would tie a feature's existence to the browser's one verb.
//
// **And it has already been through [`ingest`], which *rewrites* these strings.** [`text`] removes
// a zero-width character rather than replacing it, so `metrics.k8s\u{200b}.io` — a group name no
// CRD may carry but the aggregated parse validates nothing about, which is [`path_safe`]'s reason
// one region down — comes back out of the guard spelled exactly `metrics.k8s.io`. A probe reading
// the stripped word would report metrics-server present on a cluster that has no such group. The
// bytes the server sent are the only ones that may answer this, so the comparison happens before
// the strip and keeps nothing afterwards: [`Capability`] holds no text, and there is nothing here
// for the ingest guard to bound.
//
// ## What each row is keyed on
//
// **`policy` is matched with its kind and everything else by group alone, and on a supported
// server that narrowing is unreachable rather than load-bearing.** It is the spelling NOTES §
// Capability probe uses for this row and no other, and the kind is in the same answer at no cost,
// so the narrower fact is the one taken. What it does *not* buy is a second kind to tell apart:
// `PodSecurityPolicy` left this group at 1.25 and D149's floor is 1.29, and `policy/v1` serves
// exactly one resource at 1.36, so a `--runtime-config` that switches off either the version or
// the resource alone takes the whole group off `/apis` with it — measured both shapes, both
// absent (`reports/2026-08-26-capability-probe-group-strings.md`). On every server k8rs supports
// `("policy", "PodDisruptionBudget")` and `("policy", _)` are the same function.
//
// **What the row is really worth is the opposite of what it looks like: `policy` is a built-in
// every supported server serves**, so it is the one capability whose absence is not a fact about
// what anybody installed — nobody installs PodDisruptionBudgets. `DisruptionBudgets` missing from
// a `Some(set)` has exactly two reachable causes, and a caller should read it as either: the
// answer was **trimmed** before it got here, the shape the trimmed-discovery paragraph above hands
// `connect()` and the one the `is_empty()` guard cannot catch; or an operator **turned the group
// off** with `--runtime-config`, which the paragraph directly above measures taking the whole
// group off `/apis`. Every other row is a group whose presence *is* the product being installed,
// and naming one of its kinds would put a per-kind list here for no gain.
//
// **The three meshes are three variants, where NOTES writes them on one row.** They are one
// *feature* — service-to-service traffic, later — and not one fact: what a reader is told to
// install, and what an operator would have to configure, differs per mesh, and this file freezes
// at the end of Phase 5. Collapsing them now would cost a later box the answer and could not be
// undone by that box.
//
// **Linkerd is two groups because it ships two and neither is the whole install.** `linkerd.io`
// carries `ServiceProfile` and `policy.linkerd.io` the `Server`/`HTTPRoute` family; both come from
// the `linkerd-crds` chart, and either one present means Linkerd. A set is what absorbs the
// overlap.
//
// **Nothing here re-probes.** Discovery is a photograph (the region above) and this is a read of
// the same photograph — a capability that appeared after connect appears when discovery is run
// again, on the triggers that region names, and never on a timer of this function's own.

/// **One optional thing a cluster may have brought with it**, and one feature that turns on if
/// it did (NOTES § Capability probe).
///
/// Every variant is a fact about the *cluster*, never about the reader: a group being served says
/// nothing about whether this kubeconfig may touch it, which is § EVERY KIND THE CLUSTER SERVES'
/// distinction and holds here unchanged. A 403 on the feature's own call is the feature's to
/// report.
///
/// **And the fact is *registered*, which is weaker than *installed*.** CRDs outlive the operator
/// that shipped them, so every variant below whose group is a CRD group is a floor on what the
/// cluster once had — the region above has the mechanism and the measurement. A feature turning on
/// here is licence to *ask*, never a sentence saying the product is running.
///
/// **`Debug` is free of credentials**: the variants carry nothing at all — no address, no name,
/// no string the cluster wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    /// `metrics.k8s.io` — real usage numbers in the Capacity report, [`crate::rules::Metrics`]'s
    /// subject. **Present is what this row can say**, and which of that enum's four arms a reader
    /// is shown is not decidable from this set alone: absent here reaches
    /// [`crate::rules::Metrics::Silent`] as readily as [`crate::rules::Metrics::NotInstalled`] —
    /// the region above says why — and this is the variant where taking the wrong one prints a
    /// false sentence, telling a reader with metrics-server installed to install it. That choice is
    /// the metrics poll's; a probe that answered `None` has said nothing at all.
    Metrics,
    /// `policy/PodDisruptionBudget` — drain safety: what a drain would violate, before it runs.
    DisruptionBudgets,
    /// `cert-manager.io` — the C-series' C4 findings, which have no other source of truth about
    /// a certificate that cert-manager owns.
    CertManager,
    /// `monitoring.coreos.com` — kube-prometheus-stack's CRDs are **registered**, which is not the
    /// same as its operator running. **Where to reach it is still the reader's to type** (rule 2
    /// above), and that is exactly why this row is the one that must not overstate: on a cluster
    /// whose operator was removed and whose CRDs were left behind, the strong word asks the reader
    /// to type an address for something that is gone. *Prometheus may be installed — where should
    /// I look?* is the sentence this supports; *Prometheus is installed* is not.
    Prometheus,
    /// `networking.istio.io` — Istio's traffic API.
    Istio,
    /// `linkerd.io` or `policy.linkerd.io` — Linkerd's CRDs.
    Linkerd,
    /// `cilium.io` — Cilium's CRDs.
    Cilium,
}

/// **Which of [`Capability`]'s rows this cluster serves — or `None` if nothing was discovered at
/// all.**
///
/// Takes the same `(ApiResource, ApiCapabilities)` pairs [`browsable`] does, borrowed rather than
/// consumed so one discovery answer feeds both, and reads only the group and the kind. The verbs
/// are not consulted: a capability is what the cluster has, not what it will let anyone do.
///
/// **`None` is *the discovery answer named nothing*, which is not *this cluster has none of
/// these*.** The region above is why the two cannot share a spelling, and which failure produces
/// it. A caller that flattens `None` into an empty set has put the lie back.
///
/// **`Some(set)` is complete for the answer it was handed, which is not the same as complete for
/// the cluster.** Every row not in the set named no resource in these pairs. **A group whose
/// `resources` array came back empty is invisible to it**, so absent here is never *not
/// registered* — the region above says which caller owns that difference, where the field it needs
/// still exists, and why rule 1's sentence is that caller's to write rather than this function's.
///
/// **`Some(∅)` is not the bare-cluster answer — it is the alarming one.** A cluster exactly as
/// `kind create cluster` left it, nothing installed, answers 51 resources with `policy v1` among
/// them, so a bare cluster is `Some({DisruptionBudgets})`
/// (`reports/2026-08-26-capability-probe-group-strings.md` § 2). Reaching the empty set means
/// `policy` was absent, which on a supported server means the answer was trimmed rather than the
/// cluster being empty — the region's `policy` note seen from the other side, and the same canary.
pub fn capabilities(served: &[(ApiResource, ApiCapabilities)]) -> Option<BTreeSet<Capability>> {
    if served.is_empty() {
        return None;
    }
    Some(
        served
            .iter()
            .filter_map(
                |(resource, _)| match (resource.group.as_str(), resource.kind.as_str()) {
                    ("metrics.k8s.io", _) => Some(Capability::Metrics),
                    ("policy", "PodDisruptionBudget") => Some(Capability::DisruptionBudgets),
                    ("cert-manager.io", _) => Some(Capability::CertManager),
                    ("monitoring.coreos.com", _) => Some(Capability::Prometheus),
                    ("networking.istio.io", _) => Some(Capability::Istio),
                    ("linkerd.io" | "policy.linkerd.io", _) => Some(Capability::Linkerd),
                    ("cilium.io", _) => Some(Capability::Cilium),
                    _ => None,
                },
            )
            .collect(),
    )
}

// --- WHAT ELSE THE CLUSTER SERVES END ---

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
/// **The namespace goes through it too, and it is the *sink* rather than the check**
/// ([`namespace_name`], `k8s-admin` 2026-08-29). This predicate answers *can this word go in a
/// path*, and it says yes to `PAYMENTS`, to `foo.bar` and to eight kilobytes of them — none of
/// which is a namespace, and all of which the API server answers `200` and an empty list for, so
/// the reader gets *nothing is broken* over a namespace that does not exist. What a namespace may
/// look like is [`namespace_name`]'s; this stays where it is, applied to every word that becomes a
/// segment, because a predicate that is applied to every word cannot be the one that was forgotten
/// when the source changed.
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
pub(crate) fn path_safe(word: &str) -> bool {
    word.starts_with(|character: char| character.is_ascii_alphanumeric())
        && word.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '.'
        })
}

/// **The longest a namespace name can be** — 63, because a namespace name is a DNS-1123 *label*
/// and that is the label limit (`apimachinery/pkg/util/validation`, `DNS1123LabelMaxLength`).
///
/// **`pub(crate)` because `main.rs` bounds what it echoes back with it.** A word refused for being
/// eight kilobytes long may not be printed at eight kilobytes to say so.
pub(crate) const NAMESPACE_MAX: usize = 63;

/// **Whether a word is a namespace name** — the check `--namespace` is refused by and the
/// kubeconfig's own namespace is filtered with (`k8s-admin`, 2026-08-29).
///
/// **[`path_safe`] was standing here and it is the wrong predicate**, because it answers a
/// different question: *can this go in a URL path*. Measured against a real API server, three
/// values it accepts are not namespaces and none of them fails loudly — `PAYMENTS` and `foo.bar`
/// both come back `200` with an empty `items`, and an 8 KiB name produces an 8 218-byte header
/// line and 8 241-byte request paths. **An empty list is the one wrong answer this tool must never
/// give**, because the report over it reads *nothing is broken*.
///
/// **The rule is Kubernetes' own and is not invented here**: a namespace name is a DNS-1123 label
/// — lowercase alphanumerics and `-`, starting and ending with an alphanumeric, at most
/// [`NAMESPACE_MAX`] characters (`apimachinery`'s `IsDNS1123Label`, which `ValidateNamespaceName`
/// is built from). It is deliberately **narrower** than [`path_safe`] in every direction rather
/// than a superset of it: no dot, no uppercase, and a length.
///
/// **argv is the first unbounded source a name has ever come from here.** Every other caller of
/// [`path_safe`] is handed a string the API server already bounded; `--namespace` is handed
/// whatever a shell had (the security gate's *sizes are bounded* row).
pub(crate) fn namespace_name(word: &str) -> bool {
    !word.is_empty()
        && word.len() <= NAMESPACE_MAX
        && word.starts_with(|character: char| {
            character.is_ascii_lowercase() || character.is_ascii_digit()
        })
        && word.ends_with(|character: char| {
            character.is_ascii_lowercase() || character.is_ascii_digit()
        })
        && word.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

/// **The longest an object's name can be** — 253, the DNS-1123 *subdomain* limit
/// (`apimachinery/pkg/util/validation`, `DNS1123SubdomainMaxLength`), which is what a pod name is.
pub(crate) const NAME_MAX: usize = 253;

/// **Whether a word typed on the command line may become an object's segment of a URL path** —
/// [`path_safe`] with a length, the way [`namespace_name`] is [`path_safe`]'s rule with one.
///
/// **The bound is the half [`path_safe`] does not have.** argv is unbounded, and eight kilobytes
/// of `a` is `path_safe` and is not a name any cluster has — it is an 8 KiB request path
/// (`k8s-admin`, 2026-08-29, the reason [`namespace_name`] exists beside [`path_safe`]).
///
/// **Deliberately looser than the API's own rule**, which is lowercase-only for a pod and a
/// DNS-1123 *label* for a container. What this closes is the path hole; a name that is merely not
/// a name the cluster has comes back `404` and is told plainly, which is a better sentence than
/// one this file could write.
///
/// **It is here and not beside the log request that reads it** (§ ONE CONTAINER'S LOG), because
/// *what a word may be before it becomes a path segment* is answered in one place in this file
/// and the other two answers are the two above it.
pub(crate) fn object_name(word: &str) -> bool {
    word.len() <= NAME_MAX && path_safe(word)
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

// --- ONE CONTAINER'S LOG START ---
//
// **The most-typed kubectl command there is, and the only read in this file whose input is a
// stream that never has to end** (todo.md § Phase 6, `screens/detail.md` § The logs tab).
// Everything else here decodes an object; this decodes bytes a container wrote, which are
// attacker-controlled text by the same argument invariant 9 makes about a pod name — with the
// difference that there is no `resourceVersion` at which they stop arriving.
//
// **One sanitizer and not a second.** A log line goes through [`text`] with [`FREE_TEXT`]'s cap
// exactly as a termination message does, so the strip, the character-boundary step-back and the
// [`SHORTENED`] marker are the ingest guard's and nothing here re-implements them (NOTES § D146,
// § D154). `screens/detail.md` left it open whether 4096 should be shared literally or as a
// same-valued sibling: it is shared literally, because a *second* constant with the same value is
// how the two come to differ, and the reason the number is right — a census of real captures — is
// the same reason in both places.
//
// **Three bounds, whichever is hit first, and only one of them is load-bearing**
// (`screens/detail.md` § The buffer): [`LOG_BYTES`] is the ceiling that is true in the common
// case and the worst one, [`LOG_LINES`] is what actually evicts when lines are short, and
// [`FREE_TEXT`] cuts one line whatever the other two are doing. **A dropped line is counted out
// loud** — [`LogLines::dropped_line`] is the sentence, and it is here rather than in a printer
// because the Phase 11 pane and the temporary driver are two consumers of one number and a
// second wording is a second thing that can be wrong.
//
// **What bounds an *unterminated* line is [`LINE_READ`] and not the buffer.** The security gate's
// *an endless log line must not be held whole in memory* is about the bytes between two newlines,
// and a container that writes a gigabyte without one is the shape that pays it: every reader that
// accumulates until a newline arrives — `read_line`, `read_until`, and anything written the
// obvious way — allocates the whole gigabyte first and bounds it second. [`read_lines`] stops
// accumulating at [`LINE_READ`] and throws the rest of that line away as it arrives.

/// **The bytes one open log stream retains: 2 MB** (`screens/detail.md` § The buffer).
///
/// **The one figure that is true in the common case and the worst one**, which is why it is the
/// ceiling to defend rather than [`LOG_LINES`]: a line count multiplied by a per-line cap is a
/// worst case nobody chose (5000 × 4096 is ~19.5 MB, and that number was a *product* rather than
/// a decision). It is a **fixed** addition and does not grow with session length — eight minutes
/// or eight days, the same 2 MB, which is the property `PRIOR-ART § A6` lacked when an unmeasured
/// log buffer reached 21.5 GB resident over eight days and invoked the node's OOM killer.
pub(crate) const LOG_BYTES: usize = 2 * 1024 * 1024;

/// **The lines one open log stream retains: 5000, oldest dropped first**
/// (`screens/detail.md` § The buffer).
///
/// **What actually evicts in the ordinary case.** At a generous 256 bytes a line, 5000 lines is
/// about 1.2 MB — comfortably under [`LOG_BYTES`] and enough to scroll back through a crash's
/// run-up. Only when lines run long does the byte ceiling take over: stuffed to [`FREE_TEXT`],
/// 2 MB buys roughly 500 lines and not 5000.
pub(crate) const LOG_LINES: usize = 5_000;

/// **The most of one unterminated line that is ever held**: [`FREE_TEXT`] plus four bytes.
///
/// **The four are what lets [`text`] make the cut rather than this ceiling making it.** That
/// function steps back from the cap to a character boundary — at most three bytes, a UTF-8
/// character being at most four — and it can only do that over bytes it was given. Cut at exactly
/// [`FREE_TEXT`], a multi-byte character straddling the ceiling would be a replacement character
/// this file introduced; cut four later, the step-back lands inside what we hold.
const LINE_READ: usize = FREE_TEXT + 4;

/// How much of the socket is read at once: 8 KiB.
///
/// **Nothing depends on the number** — a line is assembled across as many of these as it takes,
/// and [`a_line_split_across_two_reads_arrives_whole`](tests) is what says so. That is also why it
/// is written as one literal rather than as `8 * 1024`: an arithmetic mutant of a figure no test
/// can observe survives every run of the gate for ever, and a mutant that cannot be killed is a
/// gate that stays red for a reason nobody can act on (`dev-core`'s run, 2026-08-30).
const LOG_READ: usize = 8192;

/// **What one open log stream retains, and what it had to throw away to stay that size**
/// (`screens/detail.md` § When the buffer fills).
///
/// **The drop counter only ever climbs**, for the lifetime of the buffer: a pane that has dropped
/// nothing says nothing, and one that has dropped a line says so from then on, because the lines
/// are gone and turning follow off does not bring them back.
///
/// **Nothing here strips or cuts.** Lines arrive already through [`text`] ([`read_lines`]); this
/// type decides only how many of them are kept.
#[derive(Default)]
pub(crate) struct LogLines {
    lines: std::collections::VecDeque<String>,
    bytes: usize,
    dropped: usize,
}

impl LogLines {
    /// **One line in, and however many out as the two collection bounds need**
    /// ([`LOG_BYTES`], [`LOG_LINES`]).
    ///
    /// **Both are checked after the push and not before**, so the buffer is never left over its
    /// ceiling waiting for the next line to bring it back under.
    ///
    /// **It cannot loop forever on a line larger than [`LOG_BYTES`]** — no such line reaches here,
    /// because [`read_lines`] caps every one at [`FREE_TEXT`] — and the `else` is there for the
    /// edit that changes that rather than for an input that can happen.
    pub(crate) fn push(&mut self, line: String) {
        self.bytes += line.len();
        self.lines.push_back(line);
        while self.lines.len() > LOG_LINES || self.bytes > LOG_BYTES {
            let Some(gone) = self.lines.pop_front() else {
                break;
            };
            self.bytes -= gone.len();
            self.dropped += 1;
        }
    }

    /// What is retained, oldest first.
    pub(crate) fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(String::as_str)
    }

    /// How many lines have been dropped to stay inside the two collection bounds.
    pub(crate) fn dropped(&self) -> usize {
        self.dropped
    }

    /// **Whether anything has arrived at all** — *no logs yet* is drawn from its opposite
    /// (`screens/detail.md` § No logs yet, `PRIOR-ART § E1`).
    ///
    /// **The drop count needs no term here, and that is a property rather than an oversight**: a
    /// line is only ever evicted to make room for one that just arrived, so a buffer that has
    /// dropped something always holds lines. Writing it as `!lines.is_empty() || dropped != 0`
    /// would be a second condition that can only ever agree with the first.
    ///
    /// **The positive question and not `is_empty`, because the negation has to live somewhere a
    /// test can watch it.** Spelled `!held.is_empty()` at the call site, `delete !` survived the
    /// whole gate — the driver's only use of it is a line on stderr, and stderr belongs to the
    /// process (`dev-core`'s run, 2026-08-30). Here,
    /// [`a_buffer_has_arrived_only_after_its_first_line`](tests) kills it.
    ///
    /// **It is here and not counted at the call site** because both surfaces ask it — the pane
    /// that draws `○ no logs yet` and the driver that prints the same sentence — and a count
    /// assembled at a call site is a count the other one can assemble differently.
    pub(crate) fn arrived(&self) -> bool {
        !self.lines.is_empty()
    }

    /// **The sentence a reader gets when lines were lost, and `None` when none were**
    /// (`screens/detail.md` § When the buffer fills, word for word).
    ///
    /// **Silent below one drop, exact at and above it** — nothing is shown until it is true, and
    /// the count is never rounded or bucketed: a beginner counting on this tool to tell the truth
    /// about what it lost gets the real number.
    ///
    /// **Singular is spelled out rather than reached by a plural helper**, because the verb moves
    /// too — *1 line **was** dropped* against *2 lines **were** dropped* — and a helper that only
    /// pluralises the noun would print *1 lines were*.
    pub(crate) fn dropped_line(&self) -> Option<String> {
        match self.dropped {
            0 => None,
            1 => Some("1 line was dropped from the top to keep this pane bounded.".to_string()),
            dropped => Some(format!(
                "{dropped} lines were dropped from the top to keep this pane bounded."
            )),
        }
    }
}

/// **One log line, made safe to hold and safe to print** — the ingest guard's [`text`], and a
/// marker for the bytes [`read_lines`] refused to hold at all.
///
/// **The `overran` half is what keeps the cut honest across a strip.** [`text`] strips first and
/// bounds second, so a line whose first [`LINE_READ`] bytes are mostly control characters can
/// come back under [`FREE_TEXT`] and unmarked — which is right when the whole line was seen
/// (NOTES § D146: 10 MB of `ESC` stores as `""`, unmarked, because nothing showable was lost) and
/// a lie when it was not. Here it was not: the rest of the line was thrown away as it arrived and
/// nobody knows what was in it, so it is marked.
fn log_line(raw: &[u8], overran: bool) -> String {
    let mut line = String::from_utf8_lossy(raw).into_owned();
    text(&mut line, FREE_TEXT);
    if overran && !line.ends_with(SHORTENED) {
        line.push_str(SHORTENED);
    }
    line
}

/// As much of `more` as [`LINE_READ`] still has room for, and whether anything was refused.
fn hold(held: &mut Vec<u8>, more: &[u8], overran: &mut bool) {
    let room = LINE_READ.saturating_sub(held.len()).min(more.len());
    *overran |= room < more.len();
    held.extend_from_slice(&more[..room]);
}

/// **Every line a log stream produces, stripped and capped, handed over one at a time** — the one
/// reader both surfaces are built on (`screens/detail.md` § Printed instead of drawn).
///
/// **`line` returns whether to keep reading**, which is the whole of what a callback needs here
/// and less than a `Stream` would cost: the pane collects into [`LogLines`] and answers `true`
/// forever, and a driver printing to stdout answers `false` the moment the write fails — `head`
/// closing the pipe on a `--follow` run, which without a way to stop would drain a socket nobody
/// is listening to for as long as the container keeps writing.
///
/// **Nothing is buffered between two lines.** A caller that prints and forgets holds no log bytes
/// at all, which is a stronger property than [`LogLines`]' ceiling rather than a weaker one, and
/// it is why the headless surface has no dropped-lines count to print.
///
/// **A stream that broke and a stream that ended are two different facts and this returns both.**
/// `PRIOR-ART § E1` is *a stream ends for many reasons and the viewer says one thing*: swallowed,
/// a connection reset half way through a log prints what arrived and exits `0`, which is a
/// confident wrong answer about a log the reader is debugging from. `Ok(())` is the server
/// closing the body, and the *why* of that is answered by asking the cluster about the object
/// afterwards — the caller's call and not this loop's.
///
/// **What arrived before the break is still handed over**, including a last line with no newline
/// after it: those bytes are real, and the caller says so beside them rather than instead of them.
///
/// **The last line is emitted even with no newline after it.** A log that ends mid-line is what a
/// container killed while writing produces, and dropping it would lose the one line a crash is
/// most likely to be explained by.
pub(crate) async fn read_lines(
    reader: impl AsyncRead,
    mut line: impl FnMut(String) -> bool,
) -> std::io::Result<()> {
    let mut reader = Box::pin(reader);
    let mut chunk = [0_u8; LOG_READ];
    let mut held: Vec<u8> = Vec::new();
    let mut overran = false;
    let mut broke = None;
    loop {
        let read = match reader.read(&mut chunk).await {
            Ok(0) => break,
            Ok(read) => read,
            Err(failed) => {
                broke = Some(failed);
                break;
            }
        };
        let mut rest = &chunk[..read];
        while let Some(end) = rest.iter().position(|byte| *byte == b'\n') {
            // **`split_at` and not `rest[end + 1..]`**, which is the same slice with the index
            // arithmetic taken out of it. Mutated to `end - 1`, that expression leaves the
            // newline in `rest` and the loop never advances — a hang, which the mutation gate
            // reports as a 90-second `TIMEOUT` rather than as a defect, and a test that hangs is
            // a test whose failure nobody reads (`dev-core`, 2026-08-30; the same reasoning
            // `main_tests.rs`'s `watching` gives for its own timeout).
            let (this, after) = rest.split_at(end);
            hold(&mut held, this, &mut overran);
            if !line(log_line(&held, overran)) {
                return Ok(());
            }
            held.clear();
            overran = false;
            // `after` begins with the newline `position` just found, so this cannot panic.
            rest = &after[1..];
        }
        hold(&mut held, rest, &mut overran);
    }
    if !held.is_empty() {
        line(log_line(&held, overran));
    }
    match broke {
        Some(failed) => Err(failed),
        None => Ok(()),
    }
}

/// **Which log to read, in one value** — so the request that goes out and the `kubectl` line the
/// reader is taught cannot disagree about it (invariant 4, `screens/detail.md`).
///
/// **The three names are stripped once, here, and never again.** [`text`] runs at construction
/// rather than at display, because a value sanitised for the screen and a value sent to the
/// cluster that differ are exactly the two records invariant 4 forbids from lying — and both
/// records are built off these fields afterwards.
///
/// **Stripping is not the check.** A name that is about to become part of a URL path is refused
/// by [`object_name`] before it gets here; this bounds and cleans what is printed.
pub(crate) struct LogRequest {
    pub(crate) namespace: String,
    pub(crate) pod: String,
    pub(crate) container: Option<String>,
    pub(crate) previous: bool,
    pub(crate) follow: bool,
}

impl LogRequest {
    /// [`LogRequest`] with the three names cleaned and bounded on the way in.
    pub(crate) fn new(
        namespace: &str,
        pod: &str,
        container: Option<&str>,
        previous: bool,
        follow: bool,
    ) -> Self {
        let named = |word: &str| {
            let mut word = word.to_string();
            text(&mut word, IDENTIFIER);
            word
        };
        Self {
            namespace: named(namespace),
            pod: named(pod),
            container: container.map(named),
            previous,
            follow,
        }
    }

    /// What kube is asked for. **`follow` and `previous` and nothing else**: `tailLines`,
    /// `sinceSeconds` and `limitBytes` are knobs no screen offers, and a request carrying one the
    /// `kubectl` line below does not print is invariant 4's record lying.
    fn params(&self) -> LogParams {
        LogParams {
            container: self.container.clone(),
            follow: self.follow,
            previous: self.previous,
            ..LogParams::default()
        }
    }

    /// **The command a reader could have typed for this exact answer** — the teaching device
    /// (invariant 4, `screens/detail.md`, `screens/once.md` § stdout and stderr are split).
    ///
    /// **Built from the same fields [`LogRequest::params`] is**, so the line cannot describe a
    /// request that was not sent. It is display text: k8rs does not execute it and nothing in it
    /// is fed back into a process (the security gate).
    pub(crate) fn kubectl(&self) -> String {
        let mut line = format!("$ kubectl logs {} -n {}", self.pod, self.namespace);
        if let Some(container) = &self.container {
            line.push_str(&format!(" -c {container}"));
        }
        if self.previous {
            line.push_str(" --previous");
        }
        if self.follow {
            line.push_str(" -f");
        }
        line
    }
}

/// **The bytes of one container's log**, following it when asked (invariant 1, which puts
/// `log_stream` on the allowlist by name).
///
/// **The streaming call and never `Api::logs`, for both modes.** That one answers a `String`: the
/// whole log is on the heap before a bound is applied to it, which is the security gate's *an
/// endless log line must not be held whole in memory* through the front door, and it cannot follow
/// at all. One call site, one bound, and a fetch is a follow that ends.
pub(crate) async fn log_stream(
    client: &Client,
    request: &LogRequest,
) -> Result<impl AsyncBufRead + use<>, kube::Error> {
    Api::<Pod>::namespaced(client.clone(), &request.namespace)
        .log_stream(&request.pod, &request.params())
        .await
}

/// **One pod, read on demand and never watched** — what a log request needs before it can be made
/// at all: which containers there are, and which of them has a previous run to show.
///
/// **Through [`ingest`] like every other object** (invariant 9, § THE INGEST GUARD), so what comes
/// back is a snapshot type and not an API object, and nothing downstream can hold a field the
/// rules do not name.
///
/// **No deadline of its own.** The caller owns it: `--once` is a command in a pipeline and bounds
/// its whole run, and a log follow has no ending to be late for.
pub(crate) async fn pod(
    client: &Client,
    namespace: &str,
    name: &str,
) -> Result<PodSnapshot, kube::Error> {
    Api::<Pod>::namespaced(client.clone(), namespace)
        .get(name)
        .await
        .map(ingest)
}

/// **Which container `kubectl logs` reads when the reader named none** — the first one the pod
/// declares, which is `kubectl`'s own rule and therefore the one a reader already has.
///
/// **Init containers and sidecars are not it.** [`crate::rules::ContainerSnapshot`]s arrive init
/// first and regular after (`rules.rs`'s `container_snapshots`), and `kubectl`'s default is
/// `spec.containers[0]` — so a pod with a migration init container would otherwise default to a
/// log that stopped before the thing the reader is debugging started.
///
/// **`None` is a pod the kubelet has not reported a container for yet**, which is a `Pending` pod
/// and a real state rather than an error: the request then names no container and the API server
/// picks, exactly as `kubectl` with no `-c` does.
pub(crate) fn default_container(pod: &PodSnapshot) -> Option<&ContainerSnapshot> {
    pod.containers
        .iter()
        .find(|container| container.role == ContainerRole::Regular)
}

// --- ONE CONTAINER'S LOG END ---

// --- CONNECTING START ---
//
// **Connecting is a function and not a step in `main`** (NOTES § D16). The Phase 11 `X` switcher
// is this call made a second time after everything from the previous context has been dropped,
// so nothing here is global, `static` or initialised once: a [`Session`] is a value, and
// dropping it drops the client, the discovery answer and the five watch streams together. A
// second [`connect`] beside a live one is two sessions, not a mutated one — which is what makes
// a failed switch able to leave the old session running rather than falling back to nothing.
//
// **Four HTTP round trips before the first watch, and one TLS handshake beside them.**
// `/version` for [`version_note`], the same path a second time for the `Date` header alone
// ([`skew`]), then the aggregated discovery pair `/apis` and `/api` that § EVERY KIND THE CLUSTER
// SERVES prices at two — one answer read twice, for the sidebar and for the capability probe
// (§ WHAT ELSE THE CLUSTER SERVES). The watches' own initial LISTs are the fifth onwards and they
// are not waited for: [`Store::snapshot`] is shut until they land (NOTES § D28) and
// [`Store::still_listing`] is what a screen draws meanwhile.
//
// **The handshake is C2's and it sends no request at all** (§ THE SERVER'S OWN CERTIFICATE,
// NOTES § D178). It is not a fifth call: it opens a connection to the same host and port, reads
// the certificate the server presents, and closes — because that certificate is reachable no other
// way, kube driving its own handshake inside `hyper-rustls` and handing back no connection. It is
// bounded by [`SERVING_PROBE`] and every failure of it is `None`.
//
// **What is *not* here is the CSR list** (§ WHAT A REPORT ASKS FOR). A report's inputs are
// fetched when a report asks for them, so nothing on this path pays for a pane nobody opened.
//
// **The second `/version` is what not growing a second classifier costs**, and it is the choice
// this box had to make (2026-08-28). `Client::apiserver_version` is the only call that decodes
// `Info` *and* turns a refusal into `kube::Error::Api` — kube's `handle_api_errors` is private to
// `client/mod.rs` — while `Client::send` is the only one that hands back the response head a
// `Date` sits in. Reading both off one call means writing that status-to-error mapping again
// here, which is the thing § WHAT WENT WRONG forbids by name: it is the one place a failed call
// is classified, and a second one is how two sentences about the same refusal come to disagree.
// So the header is read by a second GET whose only failure mode is *no measurement at all*, and
// [`Session::version`]'s `Err` stays exactly the value kube handed back.
//
// **Only what cannot be connected *with* is an error.** Reading the kubeconfig and building the
// client are the two steps with no cluster on the other side of them; everything after has a
// server that can refuse, and a refusal there is one feature degraded and not a session that
// failed. That is the security gate's *a 403 degrades that one feature* read structurally: a
// kubeconfig that may not `get /apis` — the `nonResourceURLs` grant NOTES § D160 found missing
// from our own documented role — still watches pods, and the reader still gets the Alerts view
// they came for. So [`Session::version`] and [`Session::served`] are each a `Result` that
// travels, and neither can take the session down.
//
// **Nothing here classifies what failed and nothing here ever will** — § WHAT WENT WRONG is
// the one place that does, and five sites read it: the owner fetch, the two arms of
// [`NotConnected`], each watch's [`Trouble`], and — in `main.rs`, over the two `Result`s a
// session carries — [`Session::version`] and [`Session::served`] (todo.md § Phase 5). The
// `kube` error is handed back exactly as it was received — not flattened into a `String`, not
// wrapped in a message, not matched on — because a generic sentence standing in for a typed
// error is `PRIOR-ART § C1`, and losing nothing here is what let that box be written at all.
//
// **The backoff is applied here because nothing below can apply it** (NOTES § D162,
// `PRIOR-ART § B3`). [`drive`] takes an `impl Stream` and never builds a `watcher()`, so the
// caller owes the policy kube's own doc asks for: *"To avoid constantly looping errors, make
// sure backoff is applied"* (`watcher.rs:26`). Without it a watch the cluster refuses runs
// `Err → Init → list() → 403` at socket speed, which is the security gate's *never retries in a
// loop* broken by omission.
//
// **And `.default_backoff()` does not earn that row either, which is what the first draft of
// this file got wrong** (`k8s-admin`, 2026-08-27). `StreamBackoff` resets the policy on every
// non-error item (`utils/stream_backoff.rs:9-14`, `:88-91`) and a refused `watcher()` emits
// `Ok(Event::Init)` before *every* failure — `State::Empty` returns it with no `.await` in front
// of it and only then makes the request (`watcher.rs:521-527`), whose failure returns straight
// back to `State::Empty` (`:584`). So every `Err` was paid at the policy's **first** step and the
// 30-second ceiling was never approached. Measured on a live cluster off
// `apiserver_request_total`: **one request every 1.2 seconds, 2985 per refused watch per hour**,
// at 0.95% of a core, continuously — 15,320 an hour across the five, and ~122,000 failed
// authentications for a kubeconfig left open overnight. This is NOTES § D162's own sentence about
// `Init` arriving before the request, read correctly and concluded from wrongly.
//
// **[`StandingBackoff`] is that reset silenced and nothing else**, and its own doc has the
// mechanism. What it costs is measured rather than derived, twice.
// `a_refused_watch_asks_less_and_less_often_and_costs_under_130_requests_an_hour` performs the
// exact `reset, next, reset, next, …` sequence `StreamBackoff` performs: **83, 86 and 87 waits
// in the first hour** across three runs, against **3004** for the same test with the reset
// honoured — one percent off the live measurement above, which is the cross-check that the
// simulated sequence is the real one. And the built binary against a local listener that answers
// `403` to everything, which is the whole path with nothing simulated (2026-08-27):
//
// ```text
// reset honoured   /api/v1/pods   48 requests in 60s   gaps 0.80-1.60s, never growing
// reset silenced   /api/v1/pods    8 requests in 130s  gaps 1.34 1.86 6.14 8.49 22.88 39.25 43.75
// ```
//
// `a_refused_watch_of_every_kind_waits_before_it_asks_again` is what fails if the policy is
// dropped from one of the five below — a green suite proved the Pod watch alone until
// 2026-08-27 — though **it cannot tell this policy from `.default_backoff()`**: one delay is the
// most it can afford in wall clock and the two agree on the first one.
//
// **It never gives up, and that is the load-bearing half**: `StreamBackoff` *closes* a stream
// whose backoff returns `None` (`:9-14`), which is k9s's `BailOut` living inside kube's own
// utility. `next` is [`DefaultBackoff`]'s untouched and `ExponentialBackoff::new` builds it
// `.without_max_times()` (`watcher.rs:930`).
//
// **What the ceiling actually is, and the one reset left under it — none of which kube documents
// in one place.** `DefaultBackoff` is `ResetTimerBackoff<ExponentialBackoff>`
// (`watcher.rs:981-988`): 800ms doubling to a `max_delay` of 30s, with jitter that *adds* —
// backon's `with_jitter` is "a random jitter within (0, current_delay)" — so the steady-state
// wait is **30 to 60 seconds**, and an hour measures in the mid-80s of requests rather than the
// 120 the cap alone would allow.
//
// **The reset that is left is a 120-second wall clock, and it is now the recovery path rather
// than a curiosity.** `ResetTimerBackoff` resets the ramp from *inside* `next()`, when more than
// its `reset_duration` has passed since the last delay was handed out
// (`utils/backoff_reset_timer.rs:37-49`), and `DefaultBackoff` sets that to 120s
// (`watcher.rs:983`). A watch that is failing continuously always asks again inside that window,
// so the ceiling stays a ceiling; a watch that came back and stayed up is not calling `next()` at
// all, so the clock runs out and its next trouble starts at 800ms. A watch that flaps faster than
// two minutes keeps its ramp, which is the right answer for one.
//
// **One `updates()` stream per [`Watch`], for the life of the session** (NOTES § D162). The five
// below are the only five; a reconnect happens *inside* one of them, because `ended` is never
// cleared and a second stream feeding the same watch would leave a permanent *stopped* banner
// over a live kind.

/// **The namespace k8rs falls back to when the cluster refuses a cluster-wide pod LIST and the
/// context names none** (NOTES § D5).
///
/// **kubectl's own substitution**, which is what makes it the least surprising answer: kube spells
/// it in `Config::default_namespace` (`config/mod.rs:318-322`) for exactly this hole. It is
/// deliberately *not* what [`kubeconfig_namespace`] returns for a context with no `namespace:` —
/// that one answers `None` on purpose, because a screen opening on a namespace the file never
/// named is a filter the reader cannot see the reason for. Here there is a reason, it is printed,
/// and the alternative is a tool with nothing in it.
pub(crate) const FALLBACK_NAMESPACE: &str = "default";

/// **How much of the cluster this session's watches cover, and why** (NOTES § D5, § D46).
///
/// **Three states and not an `Option<String>`, because the reader is owed the cause and the rules
/// are not.** `--namespace payments` and a `403` on the cluster-wide pod list produce the
/// identical *scope* — [`crate::rules::ClusterSnapshot::namespace_scope`] carries one field for
/// both, and every rule and report that branches on it was written that way — but one of them is
/// something the reader typed a second ago and the other is a permission they may not know they
/// lack. A screen that cannot tell them apart can only stay silent about the second, which is
/// `PRIOR-ART § B4` exactly: a denied permission has to degrade one feature *and say so*.
///
/// **The two namespaced arms are the same fact to [`Identity::namespace_scope`]**, which is
/// [`Coverage::namespace`]'s whole job — the collapse happens once, on the way to the rules, and
/// the cause stays here for whoever draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Coverage {
    /// Every namespace — the ordinary run, and the one the permanent watch was designed for
    /// (invariant 6, NOTES § D28).
    Cluster,
    /// **`--namespace`, or whatever a later namespace picker sets** — the reader's own choice, and
    /// nothing is wrong.
    ///
    /// **It is also where a value that is not a namespace name lands**, narrowed to the context's
    /// namespace or [`FALLBACK_NAMESPACE`] rather than dropped ([`coverage`]). Unreachable today —
    /// `main.rs` refuses one with a sentence, exit 2 — and the arm exists so that a defence
    /// nobody can trip cannot default to the widest scope in the program. It says nothing on
    /// stderr, which is the only honest thing left: the run is scoped to a namespace, and the
    /// header names it.
    Asked(String),
    /// **The cluster-wide pod LIST came back `403`, so this is the context's namespace — or
    /// [`FALLBACK_NAMESPACE`] where the context names none and that guess was checked.**
    ///
    /// **A namespace-scoped user must get a working tool, not an empty one** (NOTES § D5). This
    /// is the arm that earns the security gate's *a 403 degrades that one feature* row: the
    /// feature that degrades is *the whole cluster*, and what replaces it is one namespace rather
    /// than nothing.
    Refused(String),
    /// **Refused cluster-wide *and* in the namespace k8rs had to guess** — the watches still point
    /// at [`FALLBACK_NAMESPACE`], and nothing they read can be trusted.
    ///
    /// **The guess was the defect** (`reports/2026-08-29-namespace-scope-under-a-real-role.md`
    /// § R1). On the ordinary platform-issued kubeconfig — a namespaced `RoleBinding` and a
    /// context that names no namespace — `default` is the one namespace the developer is *not*
    /// granted, so the run k8rs called scoped read nothing at all and said so nowhere. This arm is
    /// what a probe of the guess turns that into: a sentence, rather than a fallback presented as
    /// if it had worked.
    ///
    /// **It still carries the namespace and the watches still ask there**, which is deliberate.
    /// Widening back to the cluster would be the widest scope as a failure default; pointing
    /// nowhere would leave nothing to recover into when the grant arrives, and a grant arriving
    /// mid-run *is* picked up (§ THE DRIVER, measured at R7). What the reader gets meanwhile is
    /// five watch-trouble lines and no health claim.
    Blind(String),
}

impl Coverage {
    /// **The one namespace these watches cover, or `None` for all of them** — the collapse
    /// `rules.rs` reads (NOTES § D46).
    pub(crate) fn namespace(&self) -> Option<&str> {
        match self {
            Coverage::Cluster => None,
            Coverage::Asked(namespace)
            | Coverage::Refused(namespace)
            | Coverage::Blind(namespace) => Some(namespace),
        }
    }
}

/// **What the watches will be built to cover** — `--namespace` if it was given, otherwise
/// whatever a cluster-wide pod LIST says this login is allowed (NOTES § D5, todo.md § Phase 5).
///
/// **One extra round trip at connect, and only on a run that did not name a namespace.** It is a
/// LIST of **one** pod — `limit=1` — because what is being asked is *am I allowed*, and the
/// server decides that before it reads a single object. A reader who typed `--namespace` has
/// already answered the question, so nothing is sent for them.
///
/// # Why a probe and not a fallback on the watch itself
///
/// **A watch cannot change what it is watching.** `watcher()` is handed an `Api` at construction
/// and the stream is built once, for the life of the process (§ THE DRIVER — `ended` is never
/// cleared, so a second stream feeding the same [`Watch`] leaves a permanent *stopped* banner
/// over a live kind). So *fall back after the 403 arrives* means tearing a watch down and
/// standing another one up, and the one thing this file must never do is give a watch a second
/// stream. Deciding before they are built costs one round trip and no machinery at all.
///
/// **Only a `403` narrows the scope, and that is the half worth stating.** A timeout, a `5xx`, a
/// socket that died — [`Fault::Unanswered`] — all leave this at [`Coverage::Cluster`]: narrowing
/// on no evidence would silently hide four fifths of a cluster because a link was slow for a
/// second, and the watches' own retry is the right answer to that. A `401` does not narrow
/// either: the login is expired, not scoped, and every namespace is equally refused.
///
/// **It never retries** (the security gate). One LIST, one answer, and whatever comes back the
/// run continues — there is no loop here and nothing above it asks again.
///
/// **So the answer is a photograph, and that is the ceiling rather than an omission** — the same
/// one [`Identity`] states for the three facts beside it. A role widened while k8rs is running
/// leaves the run scoped until it is restarted; a role *narrowed* is caught by the watches
/// themselves, which start failing and say so ([`Store::troubles`]). The refresh belongs to the
/// phase that has a namespace picker to trigger one from.
///
/// **Bounded by [`REPORT_FETCH`]'s ten seconds, reusing that constant rather than inventing a
/// second number.** The hang it bounds is the same one: nothing under a kube call has a read
/// deadline (`Config::read_timeout` is `None` in every constructor), and this call sits between
/// building the client and starting the first watch — so an unbounded one is a tool that has
/// connected and will never draw. Timing out is [`Fault::Unanswered`] by the paragraph above, so
/// it stays cluster-wide.
///
/// **A namespace that is not a namespace name is refused from either source**
/// ([`namespace_name`], which replaced [`path_safe`] here on 2026-08-29). The asked-for one is
/// already refused, with a sentence, where the reader typed it — `main.rs` — so this arm is
/// defence and not the message; the kubeconfig's is not checked anywhere else, and [`text`] only
/// removes unprintable characters, so `../secrets` would survive it.
///
/// **Defence that fails *narrow*, which is a fix and not a preference** (`k8s-admin`,
/// 2026-08-29). The arm above used to drop an unusable `--namespace` and fall through to the
/// probe, so the one thing a broken narrowing check could produce was the **widest** scope in the
/// program. It now goes to the fallback like any other run that cannot have what it asked for.
///
/// # What the probe asks about, and what its answer is taken to cover
///
/// **It asks about pods and the answer scopes four kinds** — pods, Deployments, StatefulSets and
/// DaemonSets ([`watches`]) — plus the five namespaced report lists ([`report_lists`]). That is an
/// assumption and it is written down rather than left to be discovered (`tester`, 2026-08-29): a
/// role with cluster-wide `list pods` and namespaced `apps/*` gets [`Coverage::Cluster`], an empty
/// Deployment set, and no *check is off* line to explain it.
///
/// **It is defensible and it is still an assumption.** Every rule the Alerts view runs needs pods;
/// a probe per kind is four more round trips on every startup for a role nobody has reported, and
/// the failure it would prevent is already visible — the workload watches under such a role fail
/// and say so ([`Store::troubles`]), which is what `main.rs` suppresses the health claim on.
async fn coverage(
    client: &Client,
    asked: Option<&str>,
    context_namespace: Option<&str>,
) -> Coverage {
    if let Some(namespace) = asked {
        return Coverage::Asked(if namespace_name(namespace) {
            namespace.to_string()
        } else {
            context_scope(context_namespace)
                .unwrap_or(FALLBACK_NAMESPACE)
                .to_string()
        });
    }
    if lists_pods(client, None, REPORT_FETCH).await {
        return Coverage::Cluster;
    }
    // **The guess is checked and the kubeconfig's own namespace is not**, which is the whole of
    // the difference between the two arms below. A context that names `payments` is a fact the
    // reader's own file states; `default` is a word this file invented, and on the ordinary
    // platform-issued kubeconfig it is the one namespace the developer's `RoleBinding` does not
    // cover (R1). One extra round trip, on the one path that already sends one, and only when
    // there was nothing to read a namespace off.
    //
    // **A guess the cluster did not answer in time stays [`Coverage::Refused`] and does not become
    // [`Coverage::Blind`]**, which follows from [`lists_pods`]'s contract rather than being a
    // second rule: `false` there is a refusal and nothing else. `Blind` says *refused there too*,
    // and a timeout is not a refusal — claiming one would be the same wrong sentence this arm
    // exists to stop, facing the other way. The weaker claim is also the one the report corrects
    // for free: if the watch then fails, `main.rs` prints its trouble line and withholds the
    // health claim.
    let Some(named) = context_scope(context_namespace) else {
        return if lists_pods(client, Some(FALLBACK_NAMESPACE), REPORT_FETCH).await {
            Coverage::Refused(FALLBACK_NAMESPACE.to_string())
        } else {
            Coverage::Blind(FALLBACK_NAMESPACE.to_string())
        };
    };
    Coverage::Refused(named.to_string())
}

/// **The context's own namespace, when it is one** — `None` both for a context that named none
/// and for one that named something that is not a namespace name ([`namespace_name`]).
///
/// One function because two arms of [`coverage`] reach for it and the second copy of
/// *filter, then decide what to do with nothing* is where the filter gets dropped.
fn context_scope(context_namespace: Option<&str>) -> Option<&str> {
    context_namespace.filter(|namespace| namespace_name(namespace))
}

/// **Whether this login may list pods** — cluster-wide when `namespace` is `None`, inside one
/// when it is `Some` — the one question [`coverage`] asks a server.
///
/// **`false` is a refusal and nothing else**, which is what keeps the fallback from firing on a
/// bad minute: every other outcome — an answer, a timeout, a dead socket, an expired login — is
/// `true`, because none of them is evidence that this view is not allowed.
/// § WHAT WENT WRONG is what decides which is which, as it is for every other failed call here.
///
/// **`deadline` is a parameter for [`SERVING_PROBE`]'s reason** — a bound nothing can wait out is
/// a bound nothing tests. [`coverage`] passes [`REPORT_FETCH`] at both call sites; only a test
/// passes anything else, and without that the arm that answers a hang could not be reached inside
/// a suite (`k8s-admin`, 2026-08-29: the mutation gate cannot see a joint arm the 200-answer test
/// already kills).
async fn lists_pods(
    client: &Client,
    namespace: Option<&str>,
    deadline: std::time::Duration,
) -> bool {
    let api = match namespace {
        Some(namespace) => Api::<Pod>::namespaced(client.clone(), namespace),
        None => Api::<Pod>::all(client.clone()),
    };
    let asked = tokio::time::timeout(deadline, api.list(&ListParams::default().limit(1))).await;
    match asked {
        Ok(Err(error)) => fault(&error) != Fault::Refused,
        // An answer: this login may read here.
        Ok(Ok(_)) => true,
        // The deadline. A cluster that did not answer in time has said nothing about this login's
        // scope, and narrowing on that would hide a cluster because a link was slow for a second.
        Err(_) => true,
    }
}

/// **One cluster, connected** — everything a session is built from, and nothing that outlives
/// it.
///
/// **No `Debug`, and this is the type the rule is written for** (`docs/security.md` § Token
/// hygiene, NOTES § D164): [`client`](Session::client) holds a `kube::Config`, whose
/// `auth_provider.config` map is a plain `HashMap<String, String>` with a derived `Debug` — that
/// is where the oidc and gcp providers keep their tokens. [`version`](Session::version) and
/// [`served`](Session::served) hold a `kube::Error`, whose `Display` interpolates its source at
/// every hop down to an `exec` plugin's stdout. Both halves of `{:?}` on this type print a
/// credential, so the derive is absent rather than hand-written: with no `Debug` at all, a stray
/// `{:?}` is a compile error instead of a leak.
pub(crate) struct Session {
    /// **The client the rest of the phase fetches with** — the owner ReplicaSets
    /// (§ RESOLVING AN OWNER), the browser's tables (§ THE BROWSER'S ROWS), the metrics poll.
    /// **Cloning it is cheap and every clone is the same service**, which is why the five watches
    /// each took one rather than needing a client of their own: the inner value is a
    /// `tower::buffer::Buffer` whose clone shares one worker — kube's own comment on the field
    /// reads *"`Buffer` for cheap clone"* (`client/mod.rs:88`).
    pub(crate) client: Client,
    /// **What the API server calls itself**, stripped and bounded like any other free text
    /// (invariant 9) — [`version_note`] is what turns it into a sentence, and
    /// [`crate::rules::ClusterSnapshot::server_version`] is where a later box puts it.
    ///
    /// `Err` is *the server did not answer that question*, which on a cluster that is up is a
    /// `nonResourceURL` refusal on `/version` and not a fact about the version.
    pub(crate) version: Result<String, kube::Error>,
    /// **The discovery answer, read for both of the things it says** — or the one error that
    /// cost us both. A 403 on `/apis` is *no sidebar and no capability probe*, and it is not a
    /// reason to stop watching pods.
    pub(crate) served: Result<Served, kube::Error>,
    /// **The five permanent watches, ready for [`drive`]** (invariant 6) — one per [`Watch`],
    /// with the backoff already on them.
    pub(crate) watches: Vec<BoxStream<'static, Update>>,
    /// **The program this kubeconfig logs in with**, or `None` when it carries its credential
    /// itself — [`renewal`] is what reads it and why it is the `command` alone.
    ///
    /// **The one string a [`Fault::Expired`] may be told beside** (NOTES § D19). A `401` is a
    /// short-lived token that ran out mid-session, and the thing a reader needs is *which system
    /// to sign in to again* — which this kubeconfig names and no cloud can be guessed from.
    ///
    /// **It is the reader's own file and not the cluster's**, which is what makes it safe to
    /// print at all: nothing the API server wrote is in it. It is still stripped and bounded
    /// like any other text, because a kubeconfig can be built by a tool as easily as typed.
    pub(crate) renewal: Option<String>,
    /// **What the reader calls this cluster** — the context this session was opened on, which is
    /// `--context` when one was given and the kubeconfig's `current-context` otherwise
    /// ([`kubeconfig_context`]). It is C1's object name
    /// ([`crate::rules::ClusterSnapshot::context`]).
    ///
    /// `None` only for a session built from a client rather than a file, and for a kubeconfig
    /// whose current context is empty — the state C1 already says nothing about
    /// (NOTES § D51).
    pub(crate) context: Option<String>,
    /// **The namespace that context names for itself**, or `None` for a context that names none
    /// — [`kubeconfig_namespace`] is what reads it, and why the two are not one answer.
    ///
    /// **It is the namespace k8rs must start in** (`PRIOR-ART § B1`), and it is *not* itself the
    /// filter on the permanent watch: [`Coverage`] is, and this is one of the two things that
    /// decides it. Alerts is every namespace at once wherever the cluster allows one
    /// (invariant 6, NOTES § D28); what this decides is where a screen that has a namespace
    /// opens, and which namespace the 403 fallback lands on.
    pub(crate) namespace: Option<String>,
    /// **How much of the cluster [`watches`](Session::watches) is actually covering, and why**
    /// ([`Coverage`], NOTES § D5).
    ///
    /// **Not the same field as [`namespace`](Session::namespace) and never derived from it.** That
    /// one is what the kubeconfig says; this one is what the watches were built with, which is the
    /// flag when there was one and the cluster's own answer otherwise. They agree on the ordinary
    /// scoped run and differ on every other — `--namespace` overriding a context that names
    /// another, a context that names one on a cluster the reader can read whole, a fallback to
    /// [`FALLBACK_NAMESPACE`] where the context named nothing at all.
    pub(crate) coverage: Coverage,
    /// **The client certificate this kubeconfig logs in with**, PEM bytes as they sit on disk —
    /// C1's other input ([`kubeconfig_certificate`],
    /// [`crate::rules::ClusterSnapshot::client_certificate`]).
    ///
    /// **The certificate and nothing else out of that block**: never the key, never a token,
    /// never the `exec` plugin's output. A certificate is the public half and is what a server is
    /// shown on every handshake; a key or a token copied into our own types is one `Debug` away
    /// from a backtrace (invariant 8, NOTES § D51) — which is also why [`Session`] has no
    /// `Debug` at all.
    pub(crate) client_certificate: Option<Vec<u8>>,
    /// **How far this machine's clock is from the cluster's, when that is worth saying at all** —
    /// signed, and `None` for everything else. Negative is a laptop *behind* the cluster;
    /// positive is one *ahead* of it.
    ///
    /// **The sign is the one [`crate::rules::age`] already reads.** `age` draws no time at all
    /// when a stamp sits more than five minutes in our own future, which is the laptop that is
    /// behind — so the negative half here is exactly the half that blanks a card, and one
    /// direction means one thing in both files (NOTES § D55, § D69).
    ///
    /// **`None` is four different silences and they are deliberately one field**
    /// (`screens/states.md` § When there is nothing to say): a call that was refused rather than
    /// answered, no `Date` header on the answer, a `Date` that will not parse, and a real skew
    /// inside [`SKEW_ALLOWANCE`]. All four mean *nothing on any screen is different*, and a field
    /// that told them apart would be offering a distinction no renderer is allowed to draw. The
    /// first of them is NOTES § D177's second blocker and was a *reading* until 2026-08-28.
    ///
    /// **It is a number and a direction, never a sentence** — the same split invariant 5 already
    /// makes, where a [`crate::rules::Finding`] carries timestamps and the renderer says
    /// "4 min ago". The two sentences are drawn in `screens/once.md` § When your clock and the
    /// cluster's disagree, and `main.rs` is what spells them.
    ///
    /// **Read once, at connect, and never refreshed** — the ceiling [`Identity`] states for the
    /// three facts beside it. A machine whose clock is fixed while k8rs is open keeps saying so
    /// until the next [`connect`].
    ///
    /// **So a renderer that has a live/disconnected state must drop the line while disconnected,
    /// and this field cannot do it for them** (`screens/states.md` § While disconnected, or while
    /// the login has expired): the reading needs a *live* answer's `Date` to stay honest, and this
    /// one is from the last successful request by construction. Nothing here is that state — the
    /// report `main.rs` prints has per-watch trouble lines and no session-level *disconnected* —
    /// so the suppression is the drawing's, at Phase 9, and it is written here because this field
    /// is what that drawing will read.
    pub(crate) skew: Option<SignedDuration>,
    /// **When the certificate this API server presents stops being accepted** — C2, or
    /// [`Serving::Unread`] when there was no reading to take (§ THE SERVER'S OWN CERTIFICATE,
    /// NOTES § D178).
    ///
    /// **It is the soonest of several samples and never *the cluster's earliest*.** A control
    /// plane can run several API servers behind one address, and this probe cannot ask the load
    /// balancer what else is behind it ([`SERVING_SAMPLES`]).
    ///
    /// **[`Serving::Expired`] draws two different screens and the session does not choose between
    /// them.** With both round trips below also silent it is the message `main.rs` prints *instead
    /// of* a report (`screens/states.md` § Before the TUI ever starts); on a session that read the
    /// cluster fine it is one more replica past its `notAfter`, and it joins the report through
    /// [`Serving::until`] as the ordinary expired line (`screens/once.md` § *A clean tally does
    /// not mean every replica is current*). Which one is `main.rs`'s call, on facts this field
    /// does not have.
    ///
    /// **The answer and not the bytes.** [`CertificateRequestSnapshot::issued`] makes this
    /// argument for the same reason: a certificate is public, but nothing on any screen prints
    /// one, and a field nothing reads is a field that ends up in a `Debug` line (invariant 8).
    ///
    /// **No threshold has been applied**, unlike [`skew`] one field up. A gap between two clocks
    /// does not move; a deadline does, so the *is this worth saying* comparison against
    /// [`CERT_EXPIRY_WARN`] belongs to the renderer, at the instant it draws.
    ///
    /// **Read once at connect and never refreshed** — the ceiling [`Identity`] states for the
    /// three facts beside it, and [`skew`] for itself. A control-plane replica that renews its
    /// certificate while k8rs is open keeps being reported at the old date until the next
    /// [`connect`].
    ///
    /// **`None` for a session built from a client rather than a kubeconfig**, because the
    /// handshake is driven off the `Config` and [`session`] has none — the same place
    /// [`renewal`](Session::renewal) and [`client_certificate`](Session::client_certificate) are
    /// filled from.
    pub(crate) serving_expiry: Serving,
}

/// **What one discovery answer says**, read twice at connect so nothing asks again
/// (§ EVERY KIND THE CLUSTER SERVES, § WHAT ELSE THE CLUSTER SERVES).
///
/// The two are one struct because they come from one call and share its failure: a screen that
/// has kinds always has a capability answer, and one that has neither knows why.
pub(crate) struct Served {
    /// Every kind the browser may offer, in one order — [`browsable`]'s answer, so already
    /// stripped and bounded.
    pub(crate) kinds: Vec<Browsable>,
    /// [`capabilities`]'s answer over the **raw** pairs, taken before [`browsable`] rewrote a
    /// single byte of them (NOTES § D160): a zero-width character removed by [`text`] would
    /// otherwise turn `metrics.k8s\u{200b}.io` into a metrics-server that is not there.
    pub(crate) capabilities: Option<BTreeSet<Capability>>,
}

/// **The two ways there is nothing to connect *with***, each carrying the error it was handed.
///
/// **Neither is interpreted here** and neither is flattened into a `String`:
/// [`NotConnected::fault`] tells `403` from `401` from *nothing answered*, in the one place that
/// classifies anything (§ WHAT WENT WRONG), and a message standing in for a typed error is the
/// failure `PRIOR-ART § C1` catalogues. What this type promises is that the typed value survived
/// the trip.
///
/// **No `Debug`** — both payloads reach a credential through `Display`, [`Session`]'s doc has
/// the chain (`docs/security.md` § Token hygiene).
pub(crate) enum NotConnected {
    /// The kubeconfig could not be read, names no such context, or points at something that
    /// is not usable — [`kubeconfig_fault`] is what tells those three apart.
    ///
    /// **`PRIOR-ART § B1`'s six shapes do not all arrive here, which this said until 2026-08-27**
    /// (`k8s-admin`). Its sixth is an `exec` plugin that is not on the disk, and that is a client
    /// that could not be built — [`Fault::NoCredential`], through the arm below. **Two more of
    /// the six are not failures at all**: a context name containing a space and a context naming
    /// a namespace both connect, measured. So this arm carries three of B1's six, and B1's ★
    /// panic case is genuinely immune.
    Kubeconfig(kube::config::KubeconfigError),
    /// The kubeconfig parsed and no client could be built from it — a certificate that will not
    /// load, a proxy URL that will not parse. **Not** a cluster that is down: nothing here has
    /// sent a request yet.
    ///
    /// **It carries the login program because the file loaded**, which is the whole difference
    /// from the arm above and was got wrong first time round (`tester`, 2026-08-27). The
    /// commonest shape here is an `exec` block whose program is missing or broken, the one fault
    /// in the taxonomy whose fix is on the reader's own machine — and a sentence about it that
    /// cannot name the program has thrown away the only actionable thing it had. [`connect_with`]
    /// computed it and dropped it on a `?`.
    ///
    /// **Its reader is [`NotConnected::fault`]**, which tells `403` from `401` from *nothing
    /// answered* — for this arm, in practice, a login helper that produced nothing
    /// ([`Fault::NoCredential`]) or a client kube could not build at all.
    ///
    /// **An earlier draft said no test could reach this arm without broken TLS material, and that
    /// was wrong** (`k8s-admin`, 2026-08-27). A `user.exec` block whose `command` is missing from
    /// the disk — or present and exiting non-zero — reaches it with no TLS material anywhere, as
    /// `kube::Error::Auth`, before a byte has been sent to the cluster.
    /// `a_credential_plugin_that_never_answers_is_a_client_that_could_not_be_built` is that shape,
    /// and it is the **sixth** the classifier was written for: *this kubeconfig's login helper did
    /// not answer* is neither *refused* nor *not there*, and is [`Fault::NoCredential`].
    Client {
        /// The error kube handed back, exactly as it was received.
        failure: kube::Error,
        /// [`Session::renewal`], for a connection that never became a session — the program the
        /// kubeconfig names, read before the client was built because building it consumes the
        /// `Config`.
        renewal: Option<String>,
    },
}

/// **Connect to one context and hand back everything a session needs**, built fresh
/// (NOTES § D16).
///
/// `None` is the kubeconfig's own current context — the case `k8rs` with no flag is — and
/// `Some(name)` is `--context`, or the Phase 11 picker. Nothing is remembered between calls:
/// call it again for another context and drop the first [`Session`], in that order or the other,
/// and no state of the old cluster's can reach the new one because none of it is anywhere but
/// in the value.
///
/// **The kubeconfig is the only door** (invariant 3, the security gate's *identity and
/// transport*). `Kubeconfig::read` reads `KUBECONFIG` — every path in it, merged by kube's own
/// rules — or `~/.kube/config`, and nothing else. `Config::infer` and `Client::try_default` are
/// what open the in-cluster ServiceAccount path, and both are banned by name in
/// `scripts/security-guard.py`. TLS verification is never
/// disabled here — a kubeconfig that sets `insecure-skip-tls-verify` is honoured, because it is
/// the user's own file, and saying so in the header is `views.rs`'s.
pub(crate) async fn connect(
    context: Option<&str>,
    namespace: Option<&str>,
) -> Result<Session, NotConnected> {
    connect_with(kubeconfig()?, context, namespace).await
}

/// **The reader's kubeconfig, read the one way it is ever read** — `KUBECONFIG`'s paths merged by
/// kube's own rules, or `~/.kube/config`, and nothing else.
///
/// **It exists so that the Phase 11 picker does not become a second reader of the file.** That
/// picker needs the context list ([`contexts`]) *before* it connects, and [`connect`] reads the
/// file and drops it — so without this the screen would call `Kubeconfig::read()` itself and the
/// security gate's *credentials come from the kubeconfig current context and nowhere else* would
/// stop being mechanical, which is the only thing that makes it checkable (the PM's ruling,
/// 2026-08-28). It is here rather than in Phase 11 because this file freezes at the end of
/// Phase 6.
///
/// **Nothing covers this line and that is deliberate.** It reads the ambient machine, so a test
/// over it passes for one reason on a runner with a kubeconfig and another on a runner without —
/// the shape NOTES § D26 refuses, and the whole reason [`connect_with`] exists beside
/// [`connect`]. The mutation gate reports it MISSED and that report is correct.
///
/// **`result_large_err` is expected rather than designed around.** clippy sees a 136-byte
/// [`NotConnected`] over a smaller `Ok`; [`connect`] and [`connect_with`] carry the same `Err`
/// and do not trip it only because a [`Session`] is bigger. Boxing it would reshape the type five
/// call sites read, and narrowing this to `KubeconfigError` would put the
/// `NotConnected::Kubeconfig` wrap back at every caller — which is the duplication this function
/// exists to remove. It is called once, at startup. `expect` rather than `allow` so it expires by
/// itself if the type ever shrinks.
#[expect(
    clippy::result_large_err,
    reason = "one call at startup; boxing reshapes a type five sites read"
)]
pub(crate) fn kubeconfig() -> Result<Kubeconfig, NotConnected> {
    Kubeconfig::read().map_err(NotConnected::Kubeconfig)
}

/// **[`connect`] over a kubeconfig that is already in hand**, which is the same call with the
/// file read taken out of it.
///
/// **The split is exact and not a reimplementation.** `Config::from_kubeconfig` is
/// `Kubeconfig::read()` followed by `ConfigLoader::load(config, context, cluster, user)`
/// (`config/mod.rs:293-296`), and `Config::from_custom_kubeconfig` is that same `load` with the
/// value handed in (`:301-307`) — so `KUBECONFIG`'s multi-path merge, `~/.kube/config`, and every
/// `KubeconfigError` above still happen exactly where they did, one line up.
///
/// **It exists because the test that pins the context argument could not otherwise fail.** Asking
/// [`connect`] for a context no file names is an error on a machine that *has* a kubeconfig and
/// an error on one that does not — for different reasons — so a test written against the
/// ambient file passes on a runner even when the argument is ignored entirely, which is a test
/// that cannot fail (NOTES § D26). Handed a kubeconfig it wrote itself, the same test has a
/// current context to be wrongly loaded, and ignoring the argument turns it green.
pub(crate) async fn connect_with(
    kubeconfig: Kubeconfig,
    context: Option<&str>,
    namespace: Option<&str>,
) -> Result<Session, NotConnected> {
    // **Read before the kubeconfig is moved into the loader**, which is the only place the
    // *current* context is still legible: `KubeConfigOptions` carries what was asked for, and
    // what was asked for is `None` on the ordinary run.
    let named = kubeconfig_context(&kubeconfig, context);
    // Read here for the same reason and through the same [`wanted`], so the name a session
    // reports and the namespace it starts in can never come off two different entries. It is
    // also the namespace the 403 fallback lands on ([`coverage`]).
    let context_namespace = kubeconfig_namespace(&kubeconfig, context);
    let config = Config::from_custom_kubeconfig(
        kubeconfig,
        &KubeConfigOptions {
            context: context.map(str::to_string),
            ..KubeConfigOptions::default()
        },
    )
    .await
    .map_err(NotConnected::Kubeconfig)?;
    // **Read before the client is built, because building it consumes the `Config`** — and
    // read here rather than in [`session`] for the same reason § CONNECTING splits the two: a
    // test may have a client and no kubeconfig at all. The certificate is read in the same place
    // and for the same reason.
    let renewal = renewal(&config.auth_info);
    let client_certificate = kubeconfig_certificate(&config.auth_info);
    // **Not a `?`**: the failure arm needs the login program as much as the success arm does, and
    // a `?` here is exactly how it got lost the first time (`tester`, 2026-08-27).
    //
    // **`client_certificate` was the one field here no test watched travel, and it is watched
    // now** (NOTES § D169, § D179). The straight route is shut: kube's `identity_pem` refuses a
    // certificate with no key, and rustls refuses a key that is not one — the committed PEM with
    // a PEM-shaped placeholder beside it came back
    // `RustlsTls(InvalidPrivateKey("failed to parse private key as RSA, ECDSA, or EdDSA"))`, and a
    // real key may not be committed. So the key is **generated during the test** and nothing is
    // committed, which D169 accepted as a documented limit and D179 then measured the reason for:
    // `openssl` is already a hard dependency of `just check` (`scripts/certs-test.sh`), so a test
    // that shells to it costs the gate nothing. D179 took that route for the field below and not
    // for this one; the namespace box edits this expression, which put the mutant back in the
    // diff, and `the_certificate_a_kubeconfig_logs_in_with_reaches_the_session` kills it.
    // **C2's read of the *kubeconfig*, and it happens here because this is where the `Config`
    // still exists** — building the client consumes it, and the probe needs the same CA and
    // `accept_invalid_certs` the client is about to be built with (§ THE SERVER'S OWN CERTIFICATE,
    // [`trust_only`], which is what keeps the reader's *identity* out of it). **Not a `?` and
    // never one**: every failure is a silence, because only what cannot be connected *with* is an
    // error (§ CONNECTING).
    //
    // **The network half is below the client and not here, which is a correction and not a
    // preference** (`k8s-admin`, 2026-08-28). A kubeconfig `Client::try_from` refuses without
    // sending a packet — a `proxy-url` whose scheme this build cannot speak — used to pay the
    // whole of [`SERVING_PROBE`] first: measured at 10.008 s of a dead terminal before an error
    // that was available in zero (`reports/2026-08-28-c2-c3-against-a-real-api-server.md` § 5).
    let probe = probe(&config);
    // **Both halves are asserted: that the probe runs, and that its answer lands in the field
    // below.** `a_completed_handshake_reads_the_expiry_and_connect_with_files_it` stands up a real
    // TLS server and compares the field with what the wire said, and
    // `a_client_that_cannot_be_built_never_opens_the_certificate_probes_connection` counts the
    // connections on the arm that must not open one.
    //
    // **The first draft of this comment said the handshake test was impossible, and it was
    // wrong** — `openssl` is already a hard `just check` dependency and rustls's server side is
    // compiled in (`tester`, 2026-08-28). Worth keeping as a warning: `delete field
    // serving_expiry from struct Session` is *not* reported MISSED either way, because planting it
    // leaves `serving_expiry` unused and `-D warnings` files the build as `unviable` — the class
    // NOTES § D133 is about, an honest-looking count over a question nobody asked. The test is the
    // gate here, not the run.
    match Client::try_from(config) {
        Ok(client) => {
            // **Before the watches are built, because a watch cannot change what it watches**
            // ([`coverage`] carries the whole argument). On a run that named a namespace this
            // sends nothing at all.
            let coverage = coverage(&client, namespace, context_namespace.as_deref()).await;
            Ok(Session {
                renewal,
                context: named,
                namespace: context_namespace,
                client_certificate,
                serving_expiry: serving_expiry(probe, SERVING_PROBE, SERVING_SAMPLES).await,
                ..session(client, coverage).await
            })
        }
        Err(failure) => Err(NotConnected::Client { failure, renewal }),
    }
}

/// **Which context this session is on**, cleaned and bounded — the name the reader gave, or the
/// one their file already had.
///
/// **`--context` wins, and it is taken *before* the kubeconfig is loaded rather than read back
/// off the `Config`.** kube's `Config` keeps no context name at all, and `KubeConfigOptions`
/// carries only what was asked for — `None` on every ordinary run — so the current context is
/// legible in exactly one place and only until the file is moved into the loader.
///
/// **The name that reaches C1 is the one that was connected with**, so it may not be the file's
/// `current-context` when a flag overrode it. A card saying *your access to `prod` expires*
/// while the run is watching `staging` is a rule telling a reader about a cluster they are not
/// looking at.
///
/// **Stripped and bounded like the login program beside it** ([`renewal`], invariant 9,
/// NOTES § D154): a kubeconfig is written by tooling as often as by hand, and a bidi override in
/// a context name reverses the card it is printed on. Empty becomes `None` — C1 with no context
/// says nothing, which is the state NOTES § D51 already describes.
fn kubeconfig_context(kubeconfig: &Kubeconfig, asked_for: Option<&str>) -> Option<String> {
    let mut name = wanted(kubeconfig, asked_for)?.name.clone();
    text(&mut name, IDENTIFIER);
    (!name.is_empty()).then_some(name)
}

/// **The context entry this run is on** — the one place `--context` beats `current-context`, and
/// the one place a name is turned into an entry.
///
/// **Three readers, so it is written once**: [`kubeconfig_context`] for the name a session
/// reports, [`kubeconfig_namespace`] for the namespace it starts in, and [`contexts`] for the row
/// the picker preselects. A second copy of `asked_for.or(…)` is how a session comes to report one
/// context's name beside another context's namespace.
///
/// **It returns the entry and not the name, which is NOTES § D174's fix for two readers that had
/// come apart.** With `current-context:` naming something the file does not define, the name
/// resolved and the entry did not — so [`kubeconfig_context`] answered `Some("gone")` while
/// [`contexts`] marked no row current, and a header would name a context that is on no row. It is
/// inert only while [`connect_with`] fails first; it stops being inert the moment the Phase 11
/// picker calls [`kubeconfig`] and [`contexts`] *without* connecting, which is why [`kubeconfig`]
/// exists. Resolving through the entry makes the disagreement unrepresentable.
///
/// **The lookup is by the file's own spelling and never by a cleaned one**: `kubeconfig.contexts`
/// is keyed by that spelling, so a `find` against a stripped name would miss exactly the entry
/// whose name was the reason for stripping it.
///
/// **First wins, which is kube's rule and not ours** (`file_loader.rs:63-82`); [`contexts`] says
/// what that costs a duplicate.
///
/// **An entry with no `context:` body is not an entry, which is kube's answer too**
/// (NOTES § D175). `file_loader.rs:70-76` is
/// `find(…).and_then(|named| named.context.clone()).ok_or(LoadContext)`, so `- name: a` on its
/// own is an error there and used to be `Some(entry)` here — one `and_then` between this family
/// and the loader it hands the file to. *Does this context exist* has one reader.
fn wanted<'a>(
    kubeconfig: &'a Kubeconfig,
    asked_for: Option<&str>,
) -> Option<&'a kube::config::NamedContext> {
    let name = asked_for.or(kubeconfig.current_context.as_deref())?;
    kubeconfig
        .contexts
        .iter()
        .find(|entry| entry.name == name)
        .filter(|entry| entry.context.is_some())
}

/// **The namespace the context names for itself** — the one k8rs starts in, and a regression k9s
/// shipped twice (`PRIOR-ART § B1`, #1397 and #1444).
///
/// **Read off the file rather than off `Config::default_namespace`, which is this field with its
/// most useful answer removed.** kube writes `.unwrap_or_else(|| String::from("default"))`
/// (`config/mod.rs:318-322`), so *the context asked for `default`* and *the context asked for
/// nothing* arrive as one string. Those are not one state here: k8rs watches every namespace
/// (invariant 6), so a context that names none has asked for nothing at all, and a screen that
/// opened on `default` because the file was silent would be showing a filter the reader never
/// set. `None` is *the context named none*, the same `None` as everywhere else in this file
/// (NOTES § D129).
///
/// **Stripped, bounded and empty-is-`None`, exactly as [`kubeconfig_context`] treats the name
/// beside it** (invariant 9, NOTES § D154): a `namespace:` is text off a disk file like any other
/// and a bidi override in one reverses whatever line it is drawn on.
fn kubeconfig_namespace(kubeconfig: &Kubeconfig, asked_for: Option<&str>) -> Option<String> {
    namespace_of(wanted(kubeconfig, asked_for)?.context.as_ref())
}

/// **One context entry's `namespace:`, cleaned and bounded** — [`kubeconfig_namespace`] for the
/// entry this run is on, [`Choice::namespace`] for every row the picker draws (NOTES § D174).
///
/// Two callers and one read, so the namespace a session starts in and the namespace its row shows
/// cannot come out differently.
fn namespace_of(context: Option<&kube::config::Context>) -> Option<String> {
    let mut namespace = context?.namespace.clone()?;
    text(&mut namespace, IDENTIFIER);
    (!namespace.is_empty()).then_some(namespace)
}

/// **One context in the reader's kubeconfig, as the startup picker draws it** (NOTES § D116).
///
/// **Nothing here connects to anything.** Every field is off the file, so the whole list — an
/// unreachable context included — is known before the first keypress, and the picker spends no
/// round trip and no failure modal discovering a fact `kube::config::Kubeconfig` already parsed.
///
/// **No `Debug`, for [`Store`]'s own reason**: nothing here holds a credential — a context name,
/// a server address and a label are all display text — but the type is built out of a kubeconfig,
/// and the security gate's rule about that is mechanical rather than a per-field judgement call.
/// [`Tag`] below carries one and says why it may.
#[derive(Clone)]
pub(crate) struct Choice {
    /// **The context's name as it is drawn**, stripped and bounded through the same [`text`] call
    /// [`kubeconfig_context`] makes, so the name on this row and the name a [`Session`] reports
    /// are one string by construction.
    pub(crate) name: String,
    /// **The same name as the *file* spells it — what [`connect`] must be handed to open this
    /// row**, and the one field here that is never drawn.
    ///
    /// **Two fields because they are two strings, and the second pass of the box that added this
    /// list is what found it.** A context whose name carries a zero-width or bidi character is
    /// drawn without it (invariant 9, [`name`](Choice::name)) and looked up *with* it — a
    /// kubeconfig is keyed by its own spelling and [`wanted`] says why that spelling is never
    /// cleaned — so a picker handing its row's `name` back to [`connect`] would draw a row that
    /// cannot be connected to, and answer [`Fault::NoContext`] for a context that is right there
    /// in the file.
    ///
    /// **Deliberately *not* bounded, which is the one place this file's size rule would be the
    /// defect.** [`name`](Choice::name) is cut at [`IDENTIFIER`] because it is drawn; a key cut
    /// at 512 bytes is a key that no longer opens the entry it came from, and it would fail
    /// silently. The bound that applies is the kubeconfig's own: this is a clone of a string kube
    /// has already parsed into memory, so nothing here holds anything the file did not.
    pub(crate) key: String,
    /// **What this row would connect to** — an [`Address`], which is three states because there
    /// are three facts (NOTES § D175).
    ///
    /// **It was an `Option<String>` for two rounds and that was one state short.**
    /// `screens/context.md` renders the absent case as `⚠ cluster undefined` and skips the cursor
    /// over it, which is true of an entry that names no cluster and false of one whose address
    /// k8rs merely cannot read — that row `⏎` opens perfectly well. A second boolean beside
    /// [`shadowed`](Choice::shadowed) was the alternative, and a screen cannot draw a truth handed
    /// to it as two flags that can both be set.
    pub(crate) server: Address,
    /// **An entry earlier in the file already has this name, so nothing can open this one**
    /// (NOTES § D174). The row keeps its own address and its own everything; what it does not
    /// have is a way to be reached, because every lookup — kube's `load_context`, [`wanted`],
    /// `--context` — is a `find` by name and stops at the first.
    ///
    /// **k8rs is the only tool in that reader's terminal that opens the file at all.** `kubectl`
    /// v1.36.3 refuses it outright — client-go converts the list into a map and errors
    /// *"duplicate name … in list"* — while kube-rs resolves silently first-wins
    /// (`file_loader.rs:63-82`), so there is nowhere else the reader can go to find out. What the
    /// row says about it is `screens/`'.
    ///
    /// **[`current`](Choice::current) is the one thing it does *not* keep**: `current-context:`
    /// resolves through the same first-wins `find`, so the row that gets preselected is the entry
    /// above this one even when both spell the name it names.
    pub(crate) shadowed: bool,
    /// **The namespace this context names for itself**, or `None` for a context that names none —
    /// [`namespace_of`] is the read, shared with [`kubeconfig_namespace`] so a row and the
    /// session it opens cannot say two different things.
    ///
    /// **A column `kubectl config get-contexts` has and the picker owes** (NOTES § D174): it is
    /// what decides where a namespaced screen opens, and the reader has no other way to see it
    /// before pressing `⏎`.
    pub(crate) namespace: Option<String>,
    /// **The cluster sets `insecure-skip-tls-verify`.** Honoured, because it is the user's own
    /// file — and said out loud *before* the switch rather than in a header afterwards, which is
    /// the half of the security gate's *identity and transport* row no script can see.
    ///
    /// **True only where this row's own cluster is what `⏎` opens**, which is two conditions and
    /// each closed a defect of its own. **`false` where the entry names no `server:`**
    /// (NOTES § D174): there is no connection to warn about, and `⚠ TLS not verified` on it is a
    /// warning with nothing behind it. **`false` on a [`shadowed`](Choice::shadowed) row**
    /// (NOTES § D175): the connection that row opens is the entry above's, so its own flag
    /// describes a connection nobody makes — and, mirrored, it went quiet while `⏎` opened the
    /// unverified cluster above it.
    ///
    /// **[`Address::Unreadable`] still warns**, which is the one that looks like an exception and
    /// is not: kube hands the raw `server:` to `http::Uri` and connects with it, so the flag
    /// describes a connection that really happens. What k8rs cannot do is *draw* the address.
    pub(crate) insecure: bool,
    /// [`Tag`] — the user's own label, or the provider guessed off the host.
    pub(crate) tag: Tag,
    /// **This is the context the file is already on**, which the picker preselects.
    ///
    /// **Decided here and not upstairs** so the comparison happens beside the one [`wanted`]
    /// makes, over the file's own spelling; a screen comparing its own idea of *current* against
    /// these cleaned names is a second reader that can disagree with this one.
    pub(crate) current: bool,
}

/// **What a row would connect to, and the three answers there are** (NOTES § D175).
///
/// **Three states because there are three facts**, and the screen draws a different thing for
/// each: an address it can show, an entry that names none, and an entry whose address k8rs will
/// not put on the screen because it cannot read it without guessing. The last of those is a row
/// `⏎` opens perfectly well — kube parses the raw `server:` with `http::Uri` and connects — so
/// collapsing it into the second would tell the reader a cluster is undefined when it is not.
///
/// **`Debug` for [`Tag`]'s reason**: every string that can reach here has been through
/// [`address`], which drops the userinfo, and then through [`text`] (`docs/security.md`
/// § Token hygiene).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Address {
    /// The API server this row opens, stripped and bounded — scheme, host, port and path as the
    /// reader wrote them, minus any credential ([`address`]).
    Server(String),
    /// **The entry names no cluster, or that cluster has no `server:` line.** There is nothing
    /// here to connect to, and `screens/context.md` draws it `⚠ cluster undefined`.
    Undefined,
    /// **There is a `server:` and k8rs will not state what it is** — the authority is not a
    /// plausible `host[:port]` ([`host_of`]), or nothing of it survives invariant 9's strip. The
    /// row is still openable; what is missing is a line the reader can trust, and the honest
    /// answer to *am I about to touch production* is not a guess between two readings.
    Unreadable,
}

/// **A context's short label, and which of the two kinds it is** (NOTES § D116).
///
/// **Telling them apart is the type's whole job.** A written tag is a statement by whoever owns
/// the cluster and may say `prod`; a derived one is a guess off a hostname and names the provider
/// and never the environment. A screen that could not tell them apart would present a guess as a
/// statement, which is the exact mistake the tag exists to prevent — *how* the difference is
/// marked is `screens/context.md`'s, and it is marked, not merely coloured.
///
/// **This is the one type in the pair that derives `Debug`, and it is not an oversight**: the
/// whole of it is one short label the reader typed into their own file, or one of four constants
/// this file wrote. Nothing off an API server and nothing out of a `user:` block can reach it, so
/// `{:?}` on it prints what a screen prints (`docs/security.md` § Token hygiene).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Tag {
    /// `contexts[].context.extensions[name=k8rs].extension.tag`, stripped and bounded.
    Written(String),
    /// Guessed: one of `aws`, `gcp`, `azure`, `local`, and never anything else.
    Derived(&'static str),
    /// No extension of ours and nothing the host gave away. The ordinary case on day one, and
    /// not an error state.
    Blank,
}

/// **Every context the kubeconfig names, in file order** — the startup picker's list
/// (NOTES § D116), and the reason that box sits in this phase rather than in Phase 11.
///
/// **It takes `asked_for` for the same reason [`wanted`] does** (NOTES § D175). It hard-coded
/// `None` for one round, so under `k8rs --context b` the picker marked and preselected row `a`
/// while the header said `b` — the disagreement NOTES § D174 closed, arriving back through the
/// one door this function had no parameter to hear about.
///
/// **A lookup and not a parser.** `kube::config::Kubeconfig` has already read the whole file,
/// clusters included, so the join below is over two `Vec`s that are in memory; nothing here opens
/// a file, and no second reader of the kubeconfig is introduced.
///
/// **Every string is stripped and bounded on the way out** (invariant 9, NOTES § D154). A
/// kubeconfig is written by tooling as often as by hand and this picker is the first screen a
/// stranger sees, so a bidi override in a context name would reverse the row it is drawn on
/// before there is a cluster to blame it on.
pub(crate) fn contexts(kubeconfig: &Kubeconfig, asked_for: Option<&str>) -> Vec<Choice> {
    let current = wanted(kubeconfig, asked_for).map(|entry| entry.name.as_str());
    kubeconfig
        .contexts
        .iter()
        .enumerate()
        .map(|(at, named)| {
            // **A second entry under a name already taken can never be opened** (NOTES § D173,
            // corrected by NOTES § D174). Every lookup of a context — kube's `load_context`,
            // [`wanted`], the `--context` argument — is a `find` by name, so the first entry wins
            // and this one is unreachable however the reader picks it.
            //
            // **It keeps everything it has and carries [`Choice::shadowed`] instead.** For one
            // round it was drawn with `server: None`, reusing *there is nothing here to connect
            // to* — and `screens/context.md` renders that as `⚠ cluster undefined`, about a
            // cluster the line above defines. A row that lies about the file is the failure this
            // list exists to avoid.
            let shadowed = kubeconfig
                .contexts
                .iter()
                .position(|entry| entry.name == named.name)
                != Some(at);
            let context = named.context.as_ref();
            let cluster = context.and_then(|context| {
                kubeconfig
                    .clusters
                    .iter()
                    .find(|entry| entry.name == context.cluster)?
                    .cluster
                    .as_ref()
            });
            // **Derived off what the file says, drawn as the cleaned version** — one rule for
            // both of [`derived`]'s arguments (NOTES § D173). [`text`] can only ever *create* a
            // match that the file's own bytes do not have: `amazonaws\u{200b}.com` is not
            // Amazon's and `kind\u{200b}-k8rs` is not a name kind wrote, and a tag invented by
            // our own strip is the guess this column exists to refuse. Blank is the direction it
            // fails in.
            //
            // **One split, two outputs, so what is drawn and what is matched cannot disagree**
            // ([`address`], NOTES § D173).
            // **Three answers, not two** ([`Address`], NOTES § D175): no `server:` line at all,
            // one this file will not state, or the address itself.
            let (server, host) = match cluster.and_then(|cluster| cluster.server.as_deref()) {
                None => (Address::Undefined, String::new()),
                Some(raw) => match address(raw) {
                    None => (Address::Unreadable, String::new()),
                    Some((mut drawn, host)) => {
                        text(&mut drawn, IDENTIFIER);
                        // **A host made only of characters invariant 9 removes leaves nothing to
                        // put on screen**, and `drawn` alone cannot say so — it still carries the
                        // scheme, which is how this drew `https://` for one round. The question
                        // is asked of the host directly rather than by stripping a copy of it:
                        // what [`derived`] matches is still the file's own bytes (NOTES § D173).
                        if host.chars().any(|character| !unprintable(character)) {
                            (Address::Server(drawn), host)
                        } else {
                            (Address::Unreadable, String::new())
                        }
                    }
                },
            };
            let mut name = named.name.clone();
            text(&mut name, IDENTIFIER);
            let tag = match written_tag(context) {
                Some(written) => Tag::Written(written),
                // **A written tag is never a second opinion on a derived one and never the other
                // way round** (NOTES § D116): the heuristic runs only where the file said nothing.
                None if matches!(server, Address::Server(_)) => {
                    derived(&named.name, &host).map_or(Tag::Blank, Tag::Derived)
                }
                None => Tag::Blank,
            };
            Choice {
                current: !shadowed && current == Some(named.name.as_str()),
                // **Only where this row's own cluster is what `⏎` opens** — an entry with no
                // `server:` has no connection to warn about (NOTES § D174), and a shadowed row's
                // `⏎` opens the entry above, so its own flag describes a connection nobody makes
                // (NOTES § D175). [`Address::Unreadable`] is *not* excluded: kube connects with
                // the raw string whatever this file can draw.
                insecure: !shadowed
                    && server != Address::Undefined
                    && cluster
                        .and_then(|cluster| cluster.insecure_skip_tls_verify)
                        .unwrap_or(false),
                namespace: namespace_of(context),
                key: named.name.clone(),
                shadowed,
                tag,
                name,
                server,
            }
        })
        .collect()
}

/// **The tag the person who owns the cluster wrote**, out of the context's own `extensions`
/// (NOTES § D116) — the one tag that may make a claim about an environment, because a human made
/// it.
///
/// **One field of a `serde_json::Value`, and anything else under that name is ignored rather than
/// refused.** A kubeconfig is round-tripped by other tools and `extensions` is the field they
/// round-trip it *through*, so a strict read here would turn somebody else's key into our error.
///
/// **Empty is `None`** for [`kubeconfig_context`]'s reason: a blank column is the ordinary case,
/// and a tag that is present and says nothing is worse than one that is absent.
fn written_tag(context: Option<&kube::config::Context>) -> Option<String> {
    let mut tag = context?
        .extensions
        .as_ref()?
        .iter()
        .find(|extension| extension.name == "k8rs")?
        .extension
        .get("tag")?
        .as_str()?
        .to_string();
    text(&mut tag, IDENTIFIER);
    (!tag.is_empty()).then_some(tag)
}

/// **The provider a server host gives away — never an environment, and never a *place***
/// (NOTES § D116, tightened by NOTES § D173, narrowed again by NOTES § D174).
///
/// **The boundary is the decision and not the heuristic.** A `prod` guessed off a hostname and
/// printed against a cluster that is not prod is the exact mistake the tag was added to prevent,
/// so what is derivable is *who runs the machine* and nothing else. Anything unrecognised is
/// [`Tag::Blank`], and a blank column is the safe answer rather than a missing feature.
///
/// **There is no loopback arm, and that is a reversal of D116's own list** (NOTES § D174).
/// `ssh -L`, `kubectl proxy`, `kubectl port-forward`, Teleport and a corporate mTLS proxy all
/// leave a loopback `server:`, and all of them are how people reach *production* control planes
/// with no public endpoint — so the picker drew `~local` on `prod-eu-via-bastion`, measured.
/// `local` is not a provider: it is a claim about *where* the cluster is, and it is the one word
/// in the set a newcomer reads as *safe to break*. kind, minikube and Docker Desktop still get it
/// from their **name**; `rancher-desktop` and k3s-on-the-node fall to blank, which is the
/// direction this heuristic is built to fail in.
///
/// **Every host arm precedes every name arm** (NOTES § D174). Before, a `gke_` name beat an Azure
/// host and lost to an AWS one, by position alone. A host is what you connect to; a name is what
/// somebody typed, and it is the weaker evidence of the two wherever they disagree.
///
/// **The domain arms anchor at the end of the host and D116's own wording did not**
/// (NOTES § D173). It listed `amazonaws.com` and this file read it as `contains`, so
/// `amazonaws.com.attacker.example` — a DNS name anybody can register — came back `~aws`.
/// [`under`] is the anchor. **Case-insensitive, because DNS is**:
/// `HOST.EKS.AMAZONAWS.COM` is the same machine and used to fall to blank. **And one trailing dot
/// is the DNS root**, so a fully-qualified `…amazonaws.com.` is the same host and used to lose
/// its tag (NOTES § D174).
///
/// **Google is two domains and a name, and none of the three is redundant** (NOTES § D174).
/// `gke.goog` is the DNS-based control-plane endpoint — GA since 2024, and spelled
/// `gke-<hash>.<region>.gke.goog` — which the old bare `gke` substring caught by accident and
/// nothing caught after it was deleted. `googleapis.com` is what `gcloud container fleet
/// memberships get-credentials` writes (`connectgateway.googleapis.com`), and the host arm is the
/// only thing that sees it. The name
/// `gke_<project>_<zone>_<cluster>` is what `gcloud container clusters get-credentials` writes for
/// an IP-endpoint cluster, where the host says nothing at all. **All three are read off Google's
/// documentation and none is measured against a real GKE kubeconfig** — nobody here has one — and
/// if any is wrong the tag falls to blank.
///
/// **A row with no address never reaches here at all**, name arms included: [`contexts`] does not
/// call this where there is no host to read, and that row is one `screens/context.md` dims.
///
/// **The name arms: `kind-` is a prefix, the other two are equality** (NOTES § D173).
/// `starts_with("minikube")` tagged `minikube-prod` `~local`, and a context somebody named that
/// may well be a cloud cluster — the unsafe direction. minikube and Docker Desktop write their
/// context name exactly; kind writes `kind-<cluster>`. **The name arms stay case-sensitive**,
/// unlike the host: these are strings three tools write themselves, all lowercase, and a
/// case-folded `KIND-` is a name a human chose.
fn derived(name: &str, host: &str) -> Option<&'static str> {
    let lowered = host.to_ascii_lowercase();
    // One trailing dot is the DNS root label and names the same host fully qualified. Two is
    // malformed and is left to fall to blank.
    let host = lowered.strip_suffix('.').unwrap_or(&lowered);
    if under(host, "amazonaws.com") {
        return Some("aws");
    }
    if under(host, "azmk8s.io") {
        return Some("azure");
    }
    if under(host, "gke.goog") || under(host, "googleapis.com") {
        return Some("gcp");
    }
    if name.starts_with("gke_") {
        return Some("gcp");
    }
    (name.starts_with("kind-") || name == "minikube" || name == "docker-desktop").then_some("local")
}

/// **Is this host the domain itself, or one under it** — the anchor D116's list of substrings did
/// not have (NOTES § D173).
///
/// A label boundary and not a string one: `amazonaws.com` and `eks.eu-west-1.amazonaws.com` are
/// Amazon's, `amazonaws.com.attacker.example` and `notamazonaws.com` are not. The caller has
/// already lowercased and taken off the root dot.
///
/// **The label before the domain has to be a label**, which it did not until NOTES § D174:
/// `evil..amazonaws.com` and `.amazonaws.com` both ended in `.amazonaws.com` and both drew `~aws`
/// — the same false claim one spelling further along. Neither is a name that resolves, and
/// neither is a name Amazon runs.
fn under(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .and_then(|above| above.strip_suffix('.'))
            .is_some_and(|label| !label.is_empty() && !label.ends_with('.'))
}

/// **One `server:` line, split into the address a screen may draw and the host [`derived`]
/// matches** — one function, because those two must not disagree (NOTES § D173), and `None` when
/// there is no reading of it k8rs is willing to state (NOTES § D175).
///
/// **Two orderings, two different lies, and the fix is neither of them alone** (NOTES § D175).
/// Cutting the authority first and looking for the `@` inside it is RFC 3986's order and every
/// parser's — and on a *malformed* `server:` whose credential contains a `/`, `?` or `#`, the cut
/// lands inside the credential and the whole of it is drawn:
///
/// ```text
/// //admin:aGVsbG8/d29ybGQ=@APISERVER:6443  ->  authority "admin:aGVsbG8", the rest drawn
/// ```
///
/// Taking the **last** `@` instead — which is what this function did for one round, on a ruling
/// that said an unencoded `/` in userinfo made such inputs malformed — fabricates a host on a
/// *conformant* one, because `@` is a `pchar` and is legal unencoded in a path, query or
/// fragment:
///
/// ```text
/// https://host/path/a@b/c        ->  drawn "https://b/c", host "b"
/// //APISERVER/tenant/a@AWSHOST/x ->  drawn "//AWSHOST/x", and the row earns `~aws`
/// ```
///
/// That second line is the one that settles it: `http::Uri` — the parser **kube itself** hands
/// the raw `server:` to (`kube-client-4.2.0/src/config/mod.rs:310-316`) — answers `host="host"`
/// and `host="APISERVER"` for those, so the connection went one place while the row said another,
/// and the `amazonaws.com.attacker.example` class NOTES § D173 closed came back through the path.
///
/// **So the parse is the RFC's, plus one validation.** Cut the authority at the first `/`, `?` or
/// `#`; take the userinfo off at the last `@` *inside that authority*; then require what is left
/// to be a plausible `host[:port]` ([`host_of`]). It is not, on exactly the malformed inputs —
/// `admin:aGVsbG8` has a `:` followed by letters — and when it is not, **there is no address k8rs
/// can state and this answers `None`** rather than picking one of the two readings. No input
/// draws a credential, and none invents a host.
///
/// **Hand-written, and the reason is the parse and not the dependency list** (NOTES § D175;
/// `http` is already in `Cargo.lock` at 1.5.0, which is D143's narrow case, so invariant 10 is
/// the weaker argument and not the one to lean on). `http::Uri` alone reproduces the round-2
/// blocker verbatim — it answers `host=Some("admin")` for the base64 line above — because a URL
/// parser still has to be *told* which reading of an ambiguous `@` to take. The validation is the
/// telling, and it is what no parser does for us.
///
/// **One shape is accepted rather than caught, so nobody re-derives it** (NOTES § D175):
/// `//admin:p@ssw0rd` — a credential with no host at all — leaves `ssw0rd`, which is a plausible
/// single-label host, so part of a password is drawn. It needs a kubeconfig that cannot connect
/// anywhere, and every rule that catches it also rejects real single-label hosts. Its sibling is
/// a credential whose first segment ends in digits — `//admin:123/rest@APISERVER:6443` leaves
/// `admin:123`, a plausible `host:port` — and there the drawn address is *right*, because
/// `http::Uri` reads it the same way and that is where kube connects.
///
/// **The path is kept in what is drawn**, because a proxied cluster is
/// `https://rancher.example/k8s/clusters/c-abc` and an address with its path cut off is not the
/// address the reader connects to. **The port comes off the host and never off the drawn
/// address.**
fn address(server: &str) -> Option<(String, String)> {
    let (scheme, rest) = match server.split_once("://") {
        Some((scheme, rest)) => (Some(scheme), rest),
        None => (None, server),
    };
    // 1. RFC 3986's order: the authority ends at the first `/`, `?` or `#`.
    let (authority, tail) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    // 2. The userinfo is everything to the last `@` **inside that authority**, never past it.
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, after)| after);
    // 3. What is left has to look like somewhere to connect to, or there is nothing to say.
    let host = host_of(authority)?;
    let drawn = match scheme {
        Some(scheme) => format!("{scheme}://{authority}{tail}"),
        None => format!("{authority}{tail}"),
    };
    Some((drawn, host.to_string()))
}

/// **The host inside an authority, or `None` when that authority is not `host[:port]`** — step 3
/// of [`address`] (NOTES § D175), and the whole of what tells a real authority from a credential
/// the first `/` cut in half.
///
/// **A port is digits.** `admin:aGVsbG8` and `admin:hunter` are the shapes a malformed `server:`
/// leaves behind, and both have a `:` followed by letters. An empty port (`host.example:`) is
/// refused for the same reason: it is not a port and it is not a host either.
///
/// **A bracketed IPv6 literal is read as one** and comes back bare, so `[::1]:6443` and `[::1]`
/// are one answer to [`derived`]. Outside brackets a host may not contain a `:` at all, which is
/// what refuses `host:6443:7443` and a bare `::1` somebody forgot to bracket.
///
/// **An empty host is not a host**: `//admin:hunter2@` leaves nothing after the `@`, and *nothing*
/// is not an address to draw.
fn host_of(authority: &str) -> Option<&str> {
    let (host, port) = match authority.strip_prefix('[') {
        Some(literal) => match literal.split_once(']')? {
            (host, "") => (host, None),
            (host, after) => (host, Some(after.strip_prefix(':')?)),
        },
        None => match authority.rsplit_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        },
    };
    let plausible = !host.is_empty()
        && (authority.starts_with('[') || !host.contains(':'))
        && port.is_none_or(|port| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()));
    plausible.then_some(host)
}

/// **The client certificate a kubeconfig logs in with, as PEM bytes** — C1's input, and the one
/// thing off the credential block that is not a credential.
///
/// **Data first, then the path, because that is the order kube itself resolves them in**:
/// `client-certificate-data` *overrides* `client-certificate` (`config/file_config.rs`, the
/// field's own doc), so reading the file when both are present would report on a certificate the
/// connection is not using. Both shapes are real — kind and EKS embed the data, kubeadm and
/// minikube write a path — and a build that read only one would say *no certificate* to half the
/// clusters there are.
///
/// **kube's own decoder, because there is no base64 crate here and there is not going to be**
/// (invariant 10). `k8s_openapi::ByteString` is the type every `Secret` value is decoded through
/// and its `Deserialize` is standard base64; going through it is the same decoder the rest of
/// the product already uses rather than a hand-rolled second one.
///
/// **The certificate and never the key.** Where kube reads this pair at all it wants both —
/// `identity_pem` refuses a certificate with no key and builds no client — but that is kube's
/// business and not ours, and the paragraph below is the shape where it does not read them at
/// all: nothing here reads `client-key`, `client-key-data`, `token` or an `exec` block's output,
/// and a key in one of our own types is one `Debug` away from a backtrace (NOTES § D51,
/// `docs/security.md` § Token hygiene).
///
/// **A certificate with no key is refused, and a complete pair is read even under an `exec`
/// block** — one shape excluded because kube proves it cannot be the identity, one knowingly
/// accepted (the PM's ruling as corrected by `k8s-admin`, 2026-08-28; NOTES § D19 is why a
/// kubeconfig may run a program at all).
///
/// **Excluded: `client-certificate` with no `client-key`.** kube's `identity_pem` answers
/// `(Some(_), None)` with `LoadClientKey(NoBase64DataOrFile)` (`config/file_config.rs:651-661`),
/// so that file is never a live session's TLS identity: either an `exec` block supplied one and
/// kube never opened this path, or `Client::try_from` failed before a byte was sent. It is not an
/// exotic file — it is the residue of every auth migration, an `exec` block added for
/// `aws-iam-authenticator` or `gke-gcloud-auth-plugin` with the old `client-certificate:` line
/// left behind. Measured on the live cluster before this guard existed, k8rs drew a card, a badge
/// and a `1 note` in the tally about a file with no bearing on the login (`k8s-admin`,
/// 2026-08-28) — two of those three are gone anyway, because the same change stopped drawing the
/// `Info` band in the card block (`main.rs`'s `render`, NOTES § D87), and the badge is what is
/// left for this to answer. `kubectl` refuses that same kubeconfig outright:
/// *"client-key-data or client-key must be specified for … to use the clientCert authentication
/// method"*, run here against the file this guard was written for.
///
/// **Accepted: a complete pair shadowed by an `exec` block.** A plugin that returns a **token**
/// falls through to `identity_pem`, so the static pair really is the TLS identity and C1 is
/// exactly right; a plugin that returns `clientCertificateData` supplies its own, and C1's card is
/// then true of the file and not of the session. Those two cannot be told apart without running
/// the plugin, which this design refuses to do to answer a question — and going silent on every
/// `exec` block would trade an over-broad true statement for a **missed expiry**, which is the one
/// failure C1 exists to prevent.
///
/// **The path read is capped at [`CERTIFICATE_BYTES`], and the first draft's reason for leaving it
/// unbounded was measured false** (`tester`, 2026-08-28). That reason was *kube reads the
/// identical bytes one line later*, which holds only while `identity_pem()` is what supplies the
/// TLS identity. When an `exec` plugin supplies one, kube takes the plugin's and never calls it
/// (`client/config_ext.rs:391`), so `client-certificate` is a path **nothing else opens** —
/// `tester` proved it by pointing the field at `/nonexistent/…` beside a working plugin and
/// watching the binary connect anyway, and measured 16.4 GB and an OOM kill on `/dev/zero` in
/// that shape. **Re-measured here on this build rather than taken on report**: a 400 MB file kube
/// never opens cost **408 MB** of peak RSS uncapped and **18.8 MB** capped, against an 18.6 MB
/// baseline with no `client-certificate` line at all — and `/dev/zero`, which has no end to reach,
/// comes back at 18.4 MB. The `-data` read needs no cap: it is a string out of a kubeconfig kube
/// has already parsed into memory.
///
/// **A file over the cap is `None`, the same `None` as *nobody looked*** (NOTES § D129), and
/// nothing on screen pretends otherwise — C1 draws no card, exactly as for a login that carries no
/// certificate. It is refused rather than truncated because a cut file is not a smaller answer, it
/// is a different one: whatever PEM block happens to fall inside the first 64 KiB would be read as
/// this login's certificate, and its date stated as fact.
///
/// **A path that will not read is `None` and not an error**: `Client::try_from` is about to fail
/// on the same file with kube's own typed error, which is the sentence the reader gets
/// (§ WHAT WENT WRONG) — in the one shape above it will not fail at all, and a session that works
/// is not one to print a sentence about. Saying it twice, in two wordings, is the divergence this
/// file is built to avoid. **Nothing distinguishes empty bytes from no certificate here for the
/// same reason**: nothing downstream can tell them apart either, because `expires_at` answers
/// `None` to both.
fn kubeconfig_certificate(auth: &AuthInfo) -> Option<Vec<u8>> {
    use std::io::Read;
    // **A certificate with no key beside it cannot be what authenticated this session**, whoever
    // else is in the file — kube refuses that pair outright (`config/file_config.rs:651-661`), so
    // either an `exec` block supplied the identity and this file was never opened, or no client
    // was built at all. Presence only: neither key field is read, so this costs no token hygiene.
    if auth.client_key.is_none() && auth.client_key_data.is_none() {
        return None;
    }
    if let Some(data) = &auth.client_certificate_data {
        return k8s_openapi::serde_json::from_value::<k8s_openapi::ByteString>(Value::String(
            data.clone(),
        ))
        .ok()
        .map(|decoded| decoded.0);
    }
    let mut pem = Vec::new();
    // **One byte past the cap**, so *exactly at the cap* and *over it* are two answers rather than
    // one — `take(CERTIFICATE_BYTES)` alone cannot tell a file that fits from one that was cut.
    std::fs::File::open(auth.client_certificate.as_ref()?)
        .ok()?
        .take(CERTIFICATE_BYTES + 1)
        .read_to_end(&mut pem)
        .ok()?;
    (pem.len() as u64 <= CERTIFICATE_BYTES).then_some(pem)
}

/// The most a certificate this file reads may be: 64 KiB — a kubeconfig's `client-certificate`
/// **file** ([`kubeconfig_certificate`]) and the leaf a server presents on a handshake
/// ([`serving_expiry`]), the security gate's *sizes are bounded*.
///
/// **One number for both, because both are one certificate and neither is ours.** The second
/// reader arrived with C2 and is measured against the same census below; a second constant beside
/// it would be two answers to *how big may a certificate be*.
///
/// **`pub(crate)` for [`expiry_of`]'s reason** — the boundary is pinned from `main_tests.rs`,
/// which is where a file may read the committed certificates, and only a *real* certificate past
/// the cap can tell this bound from the parser refusing nonsense.
///
/// **Measured rather than picked.** The live kind cluster's admin certificate is **1155 bytes** of
/// PEM and the three committed fixtures are **1196–1220**, so the cap is room for roughly fifty of
/// them — past any chain a client is issued, and small enough that the worst case this function
/// can be pointed at costs 64 KiB instead of everything the machine has.
pub(crate) const CERTIFICATE_BYTES: u64 = 64 * 1024;

/// **The program a kubeconfig logs in with, cleaned and bounded** — the `exec` block's `command`,
/// and nothing else out of that block.
///
/// **`command` alone, and not `command` + `args`, which is a decision** (2026-08-27). What a
/// reader is told is *this login came from `aws`, sign in to it again* — the program names the
/// system to re-authenticate to, which is the whole of what NOTES § D19 asks for. The args are
/// the opposite of helpful: `aws --region eu-west-1 eks get-token --cluster-name prod` mints a
/// token for **k8rs's** use and renews nothing a human needs, so printing it invites somebody to
/// paste a line that cannot fix their problem — and args are a sibling of the `env` values the
/// security gate refuses outright.
///
/// **Never `env`, never the plugin's output.** Those are credentials
/// (`docs/security.md` § Token hygiene); this reads one field of the user's own file.
///
/// **Stripped and bounded like anything else** (invariant 9, NOTES § D146). It is not from the
/// API server, so it is not *untrusted* in that sense — but a kubeconfig is written by tooling as
/// often as by hand, and a `command` carrying a bidi override would rewrite the line it is
/// printed in. An empty one becomes `None`, because a sentence with an empty pair of backticks
/// in it is worse than one that names no program at all.
fn renewal(auth: &AuthInfo) -> Option<String> {
    let mut command = auth.exec.as_ref()?.command.clone()?;
    text(&mut command, IDENTIFIER);
    (!command.is_empty()).then_some(command)
}

/// **Everything [`connect`] does once it has a client** — split off so it can be proven against
/// a client that points at nothing.
///
/// A session is built even when the cluster answers none of these questions: the watches are
/// live either way and their failures are per-watch (NOTES § D162), so a cluster that is down at
/// connect is a screen full of *this is failing* rather than a tool that would not start.
///
/// **Crate-visible for that reason and no other.** [`connect`] needs a kubeconfig and a test may
/// not have one, so this is the seam `main.rs`'s live driver is tested through — a session over
/// a client pointed at a name that cannot resolve is the *cluster is not there* case, whole.
///
/// **The [`Coverage`] is decided *before* this and handed in**, because deciding it needs a
/// kubeconfig's namespace as well as a client and because [`watches`] is built here: a session
/// that chose its own scope would be choosing it from half the inputs.
pub(crate) async fn session(client: Client, coverage: Coverage) -> Session {
    let version = client.apiserver_version().await.map(|info| {
        // The API server's own text, and the first string in this file that did not arrive
        // through [`ingest`] — the module doc's own note about `server_version` (invariant 9).
        let mut version = info.git_version;
        text(&mut version, IDENTIFIER);
        version
    });
    // **Straight after the version and for the header the version call threw away** — the region
    // head prices the second round trip and says why it is not one call.
    let skew = skew(&client).await;
    let served = served(&client).await.map(|pairs| Served {
        // Before [`browsable`], which consumes the pairs and rewrites their strings on the way
        // through (NOTES § D160).
        capabilities: capabilities(&pairs),
        kinds: browsable(pairs),
    });
    let watches = watches(&client, &coverage);
    Session {
        client,
        version,
        served,
        watches,
        skew,
        coverage,
        // **`None` here and filled in by [`connect_with`]**, which is the only caller that has a
        // kubeconfig to read them from. A client is not a file: it carries no context name and no
        // certificate that could be read back off it.
        renewal: None,
        context: None,
        namespace: None,
        client_certificate: None,
        serving_expiry: Serving::Unread,
    }
}

/// **What the API server's own `Date` header says about this machine's clock** —
/// [`Session::skew`], measured rather than guessed.
///
/// **`/version`, because it is the one path already known to be granted.** It is what
/// `Client::apiserver_version` asks for one line above, so a kubeconfig that reaches this call at
/// all reaches this one — and `docs/security.md`'s read-only Role names it outright, in the same
/// `nonResourceURLs` rule NOTES § D160 had to add `/apis` to. This box needs no grant that role
/// did not already carry, which was read off the file rather than assumed.
///
/// **The request is built through an invariant-1 reader**, which is § THE BROWSER'S ROWS' own
/// pattern ([`Fetch`]) with the one builder that fits a single object: `get` is on the allowlist,
/// and it is what supplies the `GET`. A hand-rolled `http::Request` with a verb of our own
/// choosing is what NOTES § D142 caught — a complete DELETE that raised nothing — and
/// `Client::send` is verb-agnostic, which is exactly why `clippy.toml` leaves `Client` off its
/// list and says so.
///
/// **`get` and not `list`, because `list` would send a path no cluster here has answered.**
/// `Request::list` serialises a query string whatever is in it, so `/version` comes out as
/// `/version?`; `Request::get` with no `resourceVersion` is a plain `format!("{url_path}/{name}")`
/// (`request.rs:82-95`), so an empty base and the name `version` produce **`/version`** — byte for
/// byte the path `Client::apiserver_version` builds one line above, and the only spelling of it
/// any live cluster has answered. The trailing `?` would very probably be ignored by a Go mux
/// that routes on `URL.Path` alone; *very probably* is reasoning about the object instead of
/// measuring it, and there is no cluster in this turn to measure against (NOTES § D136).
///
/// **Every failure is the same answer: no measurement.** A refused call, a proxy that strips the
/// header, a machine whose own clock will not read — none of them is an error this file
/// classifies, and § WHAT WENT WRONG stays the one place that classifies anything. That is the
/// whole reason this is an `Option` and not a `Result`: it owes no sentence, so it needs no
/// taxonomy.
///
/// **And *refused* has to be checked for, because `Client::send` does not** (NOTES § D177). It
/// hands back `Ok(Response)` for a `403` or a `500` — the status classification that would have
/// turned one into an error is `handle_api_errors`, which is private and which only
/// `request_text` and its callers reach. Without the line below the sentence above was false:
/// measured, a `kubectl proxy` whose upstream is unreachable manufactures a whole `500` **from
/// its own clock**, and k8rs printed *I could not read the server version* on stderr and the
/// middlebox's clock as the cluster's on stdout, with the reader's own clock correct. **A body
/// this file did not ask for is not the cluster answering**, and reading its `Date` is the one
/// way this probe can state something false rather than nothing.
///
/// **Nothing off this call reaches a screen, so nothing here is stripped** (invariant 9). What
/// travels out is a duration this file computed; the header's own bytes are parsed and dropped,
/// and they are bounded on the way in by [`DATE_BYTES`].
async fn skew(client: &Client) -> Option<SignedDuration> {
    let asked = Request::new("")
        .get(VERSION_NAME, &GetParams::default())
        .ok()?;
    let answered = client.send(asked.map(Body::from)).await.ok()?;
    // The whole of NOTES § D177's second blocker: anything but a 2xx is somebody refusing, and a
    // refusal's clock is not the cluster's.
    if !answered.status().is_success() {
        return None;
    }
    // **The local instant is taken after the answer landed**, so the two clocks are read as close
    // together as the wire allows. What is left between them is one network return trip, which is
    // milliseconds against a threshold of five minutes.
    let local = local_clock()?;
    // `HeaderMap` matches a name without regard to case, and both spellings a real API server
    // sends are fed to it rather than recalled: kind's answers `date:` over HTTP/2 and `Date:`
    // over HTTP/1.1, measured on the wire, and `k8s_tests.rs` sends each one through
    // (`reports/2026-08-28-clock-skew-date-header.md` § 1).
    measure(local, answered.headers().get("date")?.as_bytes())
}

/// The one path [`skew`] reads its header off, spelled as the *name* under an empty base because
/// that is what makes `Request::get` produce `/version` and nothing else. It is
/// `Client::apiserver_version`'s own path (`client/mod.rs:415`), written here because this file
/// asks for it too.
const VERSION_NAME: &str = "version";

/// **The pure half of [`skew`]: one local instant and one `Date` header, subtracted** — no
/// network, so every shape a server can send is a test rather than a cluster.
///
/// **The arithmetic is `Timestamp::duration_since` and never `-`** (NOTES § D54), which is the
/// rule `rules.rs` already keeps for the same subtraction.
///
/// **What makes it total is the parser, not the subtraction**, and the first draft of this comment
/// argued it the wrong way round — off `Timestamp::MAX - MIN` rather than off the path.
/// `parse_timestamp` refuses anything outside `-377705023201..=253402207200` seconds, so the
/// widest gap it can hand over is 6.31×10^11 seconds against an `i64::MAX` of 9.22×10^18. Both
/// halves of that were run: `Wed, 01 Jan 5000` measures and prints a very large number, while
/// `Fri, 31 Dec 9999 23:59:59` is **past the range and comes back silence** — so *the year 9999*
/// is not one answer, and a comment claiming it was had never been fed one
/// (`reports/2026-08-28-clock-skew-date-header.md` § 10).
///
/// **The threshold lives here and in no renderer** (`screens/states.md` § The threshold: five
/// minutes, the same five, both directions). Two renderers are drawn against this field — the
/// report `main.rs` prints today, which `--once` inherits when that flag lands, and the header
/// pointer and banner at Phase 9 — and a threshold each of them remembered separately is the
/// second copy that goes stale.
fn measure(local: Timestamp, date: &[u8]) -> Option<SignedDuration> {
    if date.len() > DATE_BYTES {
        return None;
    }
    let served = DateTimeParser::new().parse_timestamp(date).ok()?;
    let skew = local.duration_since(served);
    // Written as two comparisons rather than `abs()`, which panics on a `SignedDuration` of
    // `i64::MIN` seconds. Unreachable here by the range argument above — and unreachable is not a
    // reason to write the spelling that has a panic in it at all.
    (skew > SKEW_ALLOWANCE || skew < -SKEW_ALLOWANCE).then_some(skew)
}

/// **How far the two clocks may be apart before anything is said** — five minutes, in both
/// directions.
///
/// **The same five `rules::age` blanks a card at, and a second copy only because that one is
/// private to a frozen file.** `rules.rs` freezes at the end of Phase 3, its `SKEW_ALLOWANCE` is
/// not `pub`, and this box may not reach back to make it so (CLAUDE.md § forward-only). The
/// number is not re-argued here: `screens/states.md` § The threshold reuses it deliberately, in
/// both directions, rather than inventing a second one, and NOTES § D69 is where it was chosen.
///
/// **`scripts/twin-guard.py` is what stops the two copies drifting** — [`CERT_EXPIRY_WARN`]'s own
/// doc has the measurement, and this pair is the same shape one rule down: neither file's tests
/// can see the other move.
///
/// **Strictly past it, which is `age`'s own boundary.** `age` blanks when the elapsed time is
/// `< -SKEW_ALLOWANCE`, so a skew of exactly five minutes changes nothing on screen — and a
/// sentence pointing at a screen that looks the same is the warning `screens/states.md` refuses.
const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(5);

/// **The most a `Date` header may be before it is not read at all** — 64 bytes.
///
/// **It bounds what reaches the parser and not what is allocated, which is a narrower claim than
/// the first draft made** (2026-08-28). By the time [`measure`] sees the value, hyper has already
/// read and allocated it: a 100 000-byte `Date` was accepted off the wire and refused here, so
/// the memory was spent either way and the security gate's *sizes are bounded* is held for this
/// header by hyper's own header limits, not by this constant. What this one buys is that no
/// unbounded value is ever handed to a date parser.
///
/// **Measured against the format rather than picked.** HTTP's own preferred form is IMF-fixdate,
/// `Sun, 06 Nov 1994 08:49:37 GMT`, which is **29** bytes; RFC 2822 with a numeric offset is 31.
/// So this is room for twice the longest legal value. It is not decorative: RFC 2822 allows
/// folding whitespace *between* the fields and `http` strips only the leading and trailing kind,
/// so a 68-byte value with 40 spaces after the comma both survives the wire and parses — which is
/// the shape the test feeds. Past the cap there is no sentence and no error
/// (`screens/states.md` § When there is nothing to say).
const DATE_BYTES: usize = 64;

/// **This machine's clock as an instant**, or `None` when it cannot be read as one.
///
/// **Not `main.rs`'s `wall_clock`, and the two are not a copy of each other**: they share two
/// library calls and disagree on everything after. That one owes the reader a plain-language
/// sentence per failure — *this machine's clock is set before 1970* is a thing someone can act on
/// (invariant 14) — while this one owes nobody anything, because a clock that will not read is
/// simply a skew that was not measured. Collapsing them would cost `main.rs` the sentence.
///
/// **`Timestamp::now` does not exist here**: `jiff` arrives through `k8s-openapi` with
/// `default-features = false`, so the seconds come off `SystemTime`. No new dependency, and none
/// is wanted for a subtraction (invariant 10).
fn local_clock() -> Option<Timestamp> {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    Timestamp::new(
        since_epoch.as_secs() as i64,
        since_epoch.subsec_nanos() as i32,
    )
    .ok()
}

/// **Every `(ApiResource, ApiCapabilities)` pair the cluster serves**, by the two-round-trip
/// path, with the legacy one underneath it (§ EVERY KIND THE CLUSTER SERVES).
///
/// **The aggregated answer wins, and `Discovery::run()` is never called.** Measured on a cluster
/// whose metrics-server was down, the two paths disagree: aggregated names `metrics.k8s.io` with
/// `freshness: Stale` and **zero resources**, so kube's `from_v2` drops it and the group reads
/// absent — while the legacy path names the group and then answers **503** for its resources,
/// which `run()`'s in-loop `?` turns into no sidebar at all
/// (`reports/2026-08-26-capability-probe-group-strings.md` § 3). One APIService whose backing pod
/// is crashlooping costing the reader every kind in the cluster is the failure this box was
/// briefed to avoid; a group that reads absent costs them one row.
///
/// **The fallback is for an answer that named *nothing*, never for a group that read stale.** A
/// server below 1.27 answers the aggregated call `Ok` with an empty list rather than an error
/// (failure 1 in the region above), and that is indistinguishable from a cluster with no kinds —
/// so it is the one case worth a second, more expensive question. `discovery::group` per name is
/// `1 + V(g)` round trips each and repeats `/apis` (the region's table), which is more than
/// `run()` would cost; what it buys is the thing `run()` cannot express — **a group that fails
/// answers for itself alone** and the rest of the sidebar survives it.
///
/// **`filter()` is not used, here or anywhere** (NOTES § D160). It sets `DiscoveryMode::Allow`
/// and `CORE_GROUP` is `""`, so a filtered answer silently drops the core group — and with it
/// the *a working server always serves `v1`* premise that [`capabilities`]'s emptiness guard
/// rests on. Narrowing, if it is ever wanted, happens over the pairs after this returns.
async fn served(client: &Client) -> Result<Vec<(ApiResource, ApiCapabilities)>, kube::Error> {
    let aggregated = Discovery::new(client.clone()).run_aggregated().await?;
    let pairs: Vec<(ApiResource, ApiCapabilities)> = aggregated
        .groups()
        .flat_map(ApiGroup::resources_by_stability)
        .collect();
    if pairs.is_empty() {
        let mut legacy = Vec::new();
        for name in group_names(client.list_api_groups().await?) {
            // **The error is kept per group and the group is what is lost.** This is the whole
            // difference from `Discovery::run()`, whose `?` is inside the same loop.
            if let Ok(group) = kube::discovery::group(client, &name).await {
                legacy.extend(group.resources_by_stability());
            }
        }
        return Ok(legacy);
    }
    Ok(pairs)
}

/// **Every group to ask about on the legacy path, the core group first and each one once.**
///
/// **`/apis` does not name the core group** — it is served by `/api` and `kube::discovery::group`
/// takes `""` for it (`ApiGroup::CORE_GROUP`, `apigroup.rs:207`). Leaving it out would drop
/// `v1` — every pod, service and node kind in the sidebar — *and* silently take
/// [`capabilities`]'s emptiness guard with it, because that guard's premise is that a working
/// server always serves `v1`. The stub-server test below proves it is load-bearing rather than
/// defensive: `Pod` arrives through `""` and through nothing else.
///
/// **A name is asked about once.** A conformant `/apis` never repeats a group and never names the
/// core one, so both duplicates come from a server or a proxy that is not conformant — and the
/// cost is not one wasted round trip but a doubled row in the sidebar, because [`browsable`]
/// sorts and deliberately does not de-duplicate (§ EVERY KIND THE CLUSTER SERVES: the same plural
/// under two groups is two real resources). The scan is linear per name against a list the size
/// of a cluster's group count — tens — so nothing here needs a set.
///
/// **Nothing is stripped and nothing may be.** A group name from `/apis` is free text from the
/// API (invariant 9), and it goes straight into a URL — so `text` would be the *wrong* guard
/// here: a name the strip shortened or altered would be asked about under a spelling the server
/// never served. The two places it can go are both closed already: as a URL it is refused by
/// `http`'s own parser and the group is skipped by the loop in [`served`] — `tester` measured all
/// six NOTES § D160 spellings plus `../../../apis/secrets` and a CRLF header injection coming
/// back `Err` on 2026-08-27 — and toward a screen it goes through [`browsable`], which strips and
/// bounds. Do not "fix" this by stripping.
fn group_names(listed: APIGroupList) -> Vec<String> {
    let mut names = vec![ApiGroup::CORE_GROUP.to_string()];
    for group in listed.groups {
        if !names.contains(&group.name) {
            names.push(group.name);
        }
    }
    names
}

/// **kube's own backoff with its one reachable reset taken out**, because with that reset in
/// place the delay never grows and a refused watch retries at a flat ~1.2s forever
/// (§ CONNECTING has the measurement).
///
/// **The mechanism, in four lines of somebody else's crate.** `StreamBackoff` resets the policy
/// on every non-error item (`utils/stream_backoff.rs:88-91`, and its own doc says so at `:9-14`).
/// A refused `watcher()` emits one before every failure: `State::Empty` returns `Ok(Event::Init)`
/// with no `.await` in front of it and *then* makes the request (`watcher.rs:521-527`), and the
/// failed initial LIST returns straight back to `State::Empty` (`:584`). So the stream is
/// `Ok(Init), Err, Ok(Init), Err, …` and every `Err` is paid at `ExponentialBackoff`'s **first**
/// step. This is NOTES § D162's own sentence about `Init` arriving before the request, used for
/// the opposite purpose.
///
/// **Making [`Backoff::reset`] do nothing leaves exactly one reset, and it is the one whose name
/// promises it.** `ResetTimerBackoff` also resets the ramp from *inside* `next()`, when more than
/// its 120 seconds of wall clock have passed since the last delay was handed out
/// (`utils/backoff_reset_timer.rs:37-49`) — that path calls the *inner* policy's `reset` and
/// never this one, so a watch that genuinely recovers and stays up for two minutes still starts
/// its next trouble at the bottom of the ramp — 0.8-1.6s, by the same jitter — rather than at
/// the plateau. **What has to clear that 120-second window is the 60-second edge of the band
/// below, not `max_delay`**: a watch that is still
/// failing always asks again inside it, so the timer never fires under a standing refusal — a 2×
/// margin, not the 4× the constant alone suggests.
///
/// **And the plateau is 30-60 seconds, not 30.** `max_delay` caps the *ramp*, and `backon`
/// applies jitter after the cap as a multiplier — `if cur > max_delay { cur = max_delay }` and
/// then `tmp_cur.saturating_add(tmp_cur.mul_f32(self.rng.f32()))`, which is `× (1 + U(0,1))`
/// (`backon-1.6.0/src/backoff/exponential.rs:216-235`). A reader who takes the constant for the
/// behaviour is wrong by 2×, and every latency this comment claims is read off the band. A live
/// cluster's generation gaps sat at 49-54s and this file's own runs top out at 43.75s: inside
/// the band, nearer its mean than its ceiling.
///
/// **It never returns `None`**, which is the half that is not about politeness: `StreamBackoff`
/// *closes* a stream whose backoff gives up (`utils/stream_backoff.rs:9-14`) — k9s's `BailOut`
/// living inside kube's own utility — and `next` here is `DefaultBackoff`'s, built
/// `.without_max_times()` (`watcher.rs:930`).
#[derive(Default)]
struct StandingBackoff(watcher::DefaultBackoff);

impl Iterator for StandingBackoff {
    type Item = std::time::Duration;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

impl Backoff for StandingBackoff {
    /// Deliberately nothing. The type's doc is the whole reason.
    fn reset(&mut self) {}
}

/// **The five permanent watches** (invariant 6), each with the backoff [`drive`] asks the caller
/// for and each feeding exactly one [`Watch`] (NOTES § D162).
///
/// **The config is [`INITIAL_LIST_PAGE`] and kube's defaults for everything else**, which
/// § THE INITIAL LIST argues one field at a time: the quorum read rather than the watch cache,
/// `ListWatch` rather than the `StreamingList` a 1.33 server answers `403` to, no selectors, and
/// **no `Config::timeout`** — it is one field for both calls, so a timeout short enough to bound
/// the initial LIST would also cap the watch and re-LIST the whole cluster on that period.
///
/// **Cloning the client per watch is cloning a handle**, not opening five connections —
/// [`Session::client`] carries the mechanism and kube's own word for it.
///
/// # Four of the five narrow under a namespace scope and nodes does not
///
/// **`nodes` is cluster-scoped: there is no such thing as a namespaced node list**, so it is
/// `Api::all` under every [`Coverage`]. On the login this box exists for — an ordinary
/// enterprise developer with a namespaced `Role` — that watch is refused, permanently, and
/// nothing here can change it. What changed is what a refusal costs: [`Watch::settled`] counts it
/// as answered, so the four kinds the cluster *is* serving reach the rules, `Store::troubles`
/// names the missing verb and resource, and `analysis.rs` draws its *Reading what a node has
/// needs permission to list nodes* rows where the numbers would have been.
///
/// **The namespace is already a namespace name** ([`namespace_name`]) — [`coverage`] refuses one
/// that is not, from either source, and narrows rather than widens when it does — so what
/// `Api::namespaced` interpolates into `/api/v1/namespaces/{ns}/pods` cannot carry a path segment
/// of its own (the security gate's *names build paths* row).
fn watches(client: &Client, coverage: &Coverage) -> Vec<BoxStream<'static, Update>> {
    let config = watcher::Config::default().page_size(INITIAL_LIST_PAGE);
    vec![
        updates(
            watcher(scoped::<Pod>(client.clone(), coverage), config.clone())
                .backoff(StandingBackoff::default()),
            |store| &mut store.pods,
        ),
        updates(
            // Cluster-scoped, so never narrowed — the paragraph above is the whole reason.
            watcher(Api::<Node>::all(client.clone()), config.clone())
                .backoff(StandingBackoff::default()),
            |store| &mut store.nodes,
        ),
        updates(
            watcher(
                scoped::<Deployment>(client.clone(), coverage),
                config.clone(),
            )
            .backoff(StandingBackoff::default()),
            |store| &mut store.deployments,
        ),
        updates(
            watcher(
                scoped::<StatefulSet>(client.clone(), coverage),
                config.clone(),
            )
            .backoff(StandingBackoff::default()),
            |store| &mut store.stateful_sets,
        ),
        updates(
            watcher(scoped::<DaemonSet>(client.clone(), coverage), config)
                .backoff(StandingBackoff::default()),
            |store| &mut store.daemon_sets,
        ),
    ]
}

/// **One namespaced kind's `Api`, cluster-wide or narrowed to what [`Coverage`] covers** — the
/// only place in [`watches`] that knows about the scope.
///
/// **One function and not a `match` per kind**, because five copies of a two-arm match is five
/// places the scope can be forgotten in one — the shape [`updates`]' single `of` argument already
/// refused once (NOTES § D162). It is a `fn` rather than a closure because each call is a
/// different `K` and a closure cannot be generic.
///
/// **The bound is what keeps `nodes` out.** `Api::namespaced` is only implemented for
/// `Resource<Scope = NamespaceResourceScope>`, so a future edit that routed the node watch
/// through here would not compile rather than build `/api/v1/namespaces/x/nodes`.
fn scoped<K>(client: Client, coverage: &Coverage) -> Api<K>
where
    K: Resource<Scope = NamespaceResourceScope>,
    K::DynamicType: Default,
{
    match coverage.namespace() {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    }
}

// --- CONNECTING END ---

// --- THE SERVER'S OWN CERTIFICATE START ---
//
// **C2 — *the API server's own certificate expires in N days*** (NOTES § Certificate rules,
// § D178). It is the certificate the server presents on every connection, and it is the one C1 is
// not about: C1 reads the reader's kubeconfig, this reads the wire. **The rule's name is not the
// sentence it prints**: that reads *a certificate the API server presented*, because one reading
// off a load-balanced control plane is a sample and not the cluster ([`SERVING_SAMPLES`],
// `screens/once.md` § When the API server's own certificate is running out).
//
// **It is a [`Session`] field and not a snapshot field, which is a ruling and not a shortcut**
// (NOTES § D178). `analysis.rs` and the snapshot types froze at Phase 4 close, so there is no row
// for it to fill and no rule for it to be; what it is instead is the shape
// [`Session::skew`] took one commit earlier — a Phase 5 fact with no snapshot field, read once at
// connect, carried on the session, spelled by `main.rs`. It carries no severity and no place in
// the tally, for the same reason `skew` never has: it names no cluster object. The Certificates
// pane's row is a later phase's box and is in `backlog.md`.
//
// **The trust configuration is kube's own and nothing here relaxes one.**
// `ConfigExt::rustls_client_config` builds a `rustls::ClientConfig` from the same kubeconfig CA
// and the same `accept_invalid_certs` the real client uses, so the security gate's *TLS
// verification is never disabled **by us*** holds structurally rather than by review: there is no
// `dangerous()` call here, no verifier of ours, and no second attempt after a verification
// failure. A kubeconfig that itself sets `insecure-skip-tls-verify` is honoured exactly as it is
// honoured everywhere else — that is the reader's own knob, and reading it is not turning it.
//
// **What it is *not* built from is the reader's identity** ([`trust_only`]): the probe carries no
// token, no client key and no `exec` block, because asking kube for an identity runs the login
// program a second time and a server's certificate needs no login to read.
//
// **Handshakes of our own are what it costs, and there is no first one to read instead.** kube
// drives its own inside `hyper-rustls` and hands back no connection, so the peer certificate is
// reachable only from a handshake we drive — which is why `tokio-rustls` is named in `Cargo.toml`
// at all (NOTES § D178: it adds no compiled code, `hyper-rustls` already links it). It was one
// until 2026-08-28 and is [`SERVING_SAMPLES`] now, for the reason two paragraphs down.
//
// **It reads several and reports the soonest, because one reading off a load balancer is a coin
// flip** ([`SERVING_SAMPLES`]). Measured on three kind control-planes behind one balancer with a
// single replica reissued to twelve days: the same command eight times printed the sentence three
// times and nothing five times, with nothing about the cluster changing between runs
// (`reports/2026-08-28-c2-c3-against-a-real-api-server.md` § 2).
//
// **It cannot fail the session on its own, and one reading of it can end a run that had already
// failed.** § CONNECTING's rule is unchanged — only what cannot be connected *with* is an error —
// and [`Serving::Expired`] is not connected *with* anything: it is *why* a session that read
// nothing read nothing, and `main.rs` is where the two facts are put together
// (`screens/states.md` § Before the TUI ever starts: *a more specific cannot reach the cluster,
// not a fourth kind of failure*). On a session that read the cluster fine it is one more reading
// for the report instead ([`Serving::until`]). Every **other** failure here is
// [`Serving::Unread`] and no sentence, which is C1's own policy read across: `expires_at` answers
// `None` alike for a truncated certificate, a wrong PEM label and an RFC 5280 §4.1.2.5 *no
// well-defined expiry*, and draws no card for any of them
// (`screens/once.md` § No reading at all is one silence, not several).
//
// **The probe runs after the client is built and the [`Probe`] it needs is prepared before**
// (`reports/2026-08-28-c2-c3-against-a-real-api-server.md` § 5). It used to run first, and on a
// kubeconfig `Client::try_from` refuses without touching the network — a `proxy-url` whose scheme
// this build cannot speak — that charged the whole of [`SERVING_PROBE`] to a run that was about
// to print an error it already had: ten seconds of a dead terminal, measured. Splitting it in two
// costs nothing, because [`endpoint`], [`trust_only`] and [`server_name`] all read the `Config`
// that `Client::try_from` consumes and none of them sends a packet.

/// **How long the whole probe gets** — ten seconds over the DNS lookup, the TCP connect and the
/// handshake together.
///
/// **It exists because the failure it bounds is a hang and not a slow answer.** A port that
/// silently drops SYNs costs Linux's own default around two minutes, and a middlebox that accepts
/// the socket and never speaks TLS costs forever — and this runs inside [`connect_with`], before
/// the first screen, so either one is a tool that looks broken while a cluster is fine
/// (`PRIOR-ART § A7`, NOTES § D150).
///
/// **Deliberately shorter than kube's own 30-second `connect_timeout`** (`config/mod.rs:418`),
/// which is what the four calls a session actually needs get — and *only* over the connecting:
/// `read_timeout` is `None` in every kube constructor, so nothing bounds their answers at all
/// ([`REPORT_FETCH`], which is the same hole one region down). This one owes the reader nothing: a
/// link too slow for ten seconds produces exactly the same silence as a handshake that failed, and
/// no screen is different for it.
///
/// **It bounds the whole sampling loop and not one handshake** ([`SERVING_SAMPLES`]), so a slow
/// endpoint cannot multiply it: five handshakes against a listener that accepts and never speaks
/// still cost ten seconds once, which is the number this constant promises.
const SERVING_PROBE: std::time::Duration = std::time::Duration::from_secs(10);

/// **How many handshakes one connect drives** — five, and the answer is the soonest deadline any
/// of them read.
///
/// **One reading is a coin flip on an HA control plane and that was measured, not reasoned.**
/// Three `kube-apiserver` replicas behind one load balancer, one of them reissued to twelve days:
/// eight consecutive runs of the same command printed the certificate sentence **three** times and
/// nothing five times (`reports/2026-08-28-c2-c3-against-a-real-api-server.md` § 2). The balancer
/// picks the replica, and a probe that opens one connection samples one of N.
///
/// **Five, because it is free and because it cannot be enough.** A whole connect — the four HTTP
/// calls a session makes *and* this probe — measured 24–36 ms on the review host, so the extra
/// four handshakes are noise beside it. On three replicas, five independent samples take the
/// chance of missing the one worth warning about from about two in three to about one in eight.
/// **It does not reach zero and is not meant to**: what this reports is *the soonest deadline
/// these samples saw*, never *the cluster's earliest*, and `screens/once.md` § When the API
/// server's own certificate is running out words the sentence for exactly that claim — which is
/// why a bigger number here buys wording nothing, only odds.
const SERVING_SAMPLES: usize = 5;

/// **What the samples saw** — a deadline, a `notAfter` already behind us, or nothing.
///
/// **Three outcomes and not two, because the moment C2 exists for is the moment it went silent**
/// (`screens/once.md` § When the certificate is why nothing came back). rustls *verifies*, so an
/// API server whose certificate has already run out fails the handshake — and a probe with two
/// outcomes collapses that into the same silence as an unparseable address, while it is holding
/// the typed error that says exactly what is wrong. Measured against a real API server three days
/// past its own `notAfter`: `grep -c "API server's own certificate"` over the run was `0` and what
/// the operator got was a wall of *nothing usable came back*
/// (`reports/2026-08-28-c2-c3-against-a-real-api-server.md` § 3), which is `PRIOR-ART § C1` on the
/// one failure this product ships a dedicated probe to diagnose.
///
/// **The other two silences stay one silence.** An address this probe cannot reuse and a
/// `notAfter` that will not parse are still [`Unread`](Serving::Unread) alike — that ruling is
/// unchanged, and the reason it survives is that neither of them is a *typed* fact the way this
/// one is.
///
/// **`Default` is derived for the mutation gate and the default is the honest one**
/// (NOTES § D133). cargo-mutants replaces a function body with `Default::default()`, and a type
/// without one turns that into an `unviable` — which reads exactly like a mutant that was tested.
/// Three of this region's four new functions were filed that way on 2026-08-28, so the body
/// replacement nobody could plant was also the body replacement nobody had to kill. `Unread` is
/// what the type means by nothing, so the derive costs no meaning either.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Serving {
    /// **The soonest `notAfter` any sample read**, over a handshake that completed.
    Until(Timestamp),
    /// **The `notAfter` rustls refused a sample over** — the certificate's real expiry, carried
    /// out of the very handshake that would hand back nothing else
    /// (`CertificateError::ExpiredContext`, `rustls-0.23.43/src/error.rs:382`). Nothing is
    /// invented here and nothing is placeheld: a verifying kubeconfig never receives the bytes,
    /// and it does not need to, because the refusal already named the date.
    ///
    /// **The date is no longer what separates this from [`Until`](Serving::Until) — a completed
    /// handshake is.** That is the whole of the distinction `main.rs` keys its abort on: `Until`
    /// is proof some replica behind this address serves a certificate a verifying client accepts,
    /// and this one is proof of nothing but its own sample.
    Expired(Timestamp),
    /// **No reading**, which is every other way this can come back.
    #[default]
    Unread,
}

impl Serving {
    /// **The deadline, for the renderer that draws one** — `None` only when nothing was read.
    ///
    /// **[`Expired`](Serving::Expired) answers with its date too**, which is the whole of the
    /// second sentence C2 draws: a session that read the cluster fine while the probe met an
    /// already-expired replica gets the *same* trailer line [`Until`](Serving::Until) draws, not a
    /// new one (`screens/once.md` § *A clean tally does not mean every replica is current*).
    pub(crate) fn until(self) -> Option<Timestamp> {
        match self {
            Serving::Until(at) | Serving::Expired(at) => Some(at),
            Serving::Unread => None,
        }
    }

    /// **Two samples folded into one answer** — the soonest deadline wins, and a deadline beats a
    /// typed expiry beats nothing.
    ///
    /// **A date outranks [`Expired`](Serving::Expired), and that ordering is what keeps the
    /// startup message off a cluster that works.** A completed handshake means some replica behind
    /// this address is serving a certificate a verifying client accepts; saying *the certificate
    /// has expired* over the top of that would be `main.rs`'s abort firing on a cluster k8rs can
    /// read.
    fn soonest(self, other: Self) -> Self {
        match (self, other) {
            (Serving::Until(a), Serving::Until(b)) => Serving::Until(a.min(b)),
            (Serving::Until(at), _) | (_, Serving::Until(at)) => Serving::Until(at),
            (Serving::Expired(a), Serving::Expired(b)) => Serving::Expired(a.min(b)),
            (Serving::Expired(at), _) | (_, Serving::Expired(at)) => Serving::Expired(at),
            (Serving::Unread, Serving::Unread) => Serving::Unread,
        }
    }
}

/// **How close the API server's certificate has to be to running out before anything is said** —
/// thirty days.
///
/// **The same thirty `rules.rs` warns C1 at, and a second copy only because that one is private to
/// a frozen file** — exactly [`SKEW_ALLOWANCE`]'s position. `rules.rs` froze at the end of Phase 3
/// and its `CERT_EXPIRY_WARN` is not `pub`, and this box may not reach back to make it so
/// (CLAUDE.md § forward-only). The number is not re-argued here: `screens/once.md` § When the API
/// server's own certificate is running out reuses it deliberately rather than inventing a second
/// distance for a second certificate on the same report, and `rules.rs` § the certificate rules is
/// where it was chosen.
///
/// **What stops the two copies drifting is `scripts/twin-guard.py`, and nothing else does.**
/// Measured on 2026-08-28: setting this to sixty days leaves the whole suite green, because
/// `rules_tests` pins only `rules.rs`'s copy and `main_tests` asks its boundary question in terms
/// of *this* one — so a re-tune of one file and its tests ships one report warning about two
/// certificates at two distances. The guard compares the durations rather than the spellings and
/// runs inside `just check`.
///
/// **It is `pub(crate)` because the renderer applies it, not this file.** Unlike [`SKEW_ALLOWANCE`]
/// — a gap between two clocks, which does not move — an expiry is a deadline and the days left
/// change under a long-running session, so the comparison has to happen at draw time. One constant
/// below every renderer is what stops the report and the Phase 9 banner keeping two.
pub(crate) const CERT_EXPIRY_WARN: SignedDuration = SignedDuration::from_hours(30 * 24);

/// **Everything one sample needs, computed off the `Config` before the client consumes it** —
/// where to connect, what to verify against, and the trust to verify with.
///
/// **It exists so the connecting can happen after `Client::try_from` and the reading of the
/// kubeconfig before it** (§ THE SERVER'S OWN CERTIFICATE's last paragraph). Nothing in
/// [`probe`] touches the network, so a kubeconfig kube refuses outright fails in zero seconds
/// instead of ten.
struct Probe {
    /// [`endpoint`]'s host, already unbracketed.
    host: String,
    /// [`endpoint`]'s port.
    port: u16,
    /// The reader's own trust and nothing that can log in ([`trust_only`]). Shared across the
    /// samples rather than rebuilt per handshake — building it is where the certificate parsing
    /// happens.
    tls: std::sync::Arc<tokio_rustls::rustls::ClientConfig>,
    /// [`server_name`]'s answer: what kube's own connection verifies and SNIs.
    name: ServerName<'static>,
}

/// **The half of the probe that reads the kubeconfig**, or `None` when this `server:` is not
/// somewhere a handshake can be driven a second time.
fn probe(config: &Config) -> Option<Probe> {
    let (host, port) = endpoint(config)?;
    let tls = std::sync::Arc::new(trust_only(config).rustls_client_config().ok()?);
    let name = server_name(config, &host)?;
    Some(Probe {
        host,
        port,
        tls,
        name,
    })
}

/// **When the certificate this API server presents stops being accepted** — [`Serving`], over
/// several samples (§ THE SERVER'S OWN CERTIFICATE).
///
/// **Every failure but one is [`Serving::Unread`].** A handshake that does not complete for any
/// reason other than a `notAfter` behind us, a `server:` this probe cannot turn into somewhere to
/// connect a second time, a certificate past [`CERTIFICATE_BYTES`], and a `notAfter` that will not
/// parse are four different things with one content: k8rs does not know when this certificate runs
/// out. C1's `expires_at` already answers exactly this question the same way. **A refusal rustls
/// spells without a date is on that list too** — [`refused_for_expiry`] has which one and why. The
/// exception is [`Serving::Expired`], whose own doc has the measurement.
///
/// **No threshold is applied here**, which is where this parts company with [`skew`]: a date this
/// hands back is *this is when it expires*, not *this is worth saying*.
/// [`CERT_EXPIRY_WARN`]'s own doc has the reason — a deadline moves against `now` and a clock gap
/// does not.
///
/// **The leaf and only the leaf.** rustls hands back the chain the server sent, whose first entry
/// is the end-entity certificate (RFC 8446 §4.4.2); the intermediates above it are the CA's
/// business and expire on their own schedule.
///
/// **Where it connects and what it verifies are two different names** ([`endpoint`],
/// [`server_name`]), which is the correction `tester` sent this box back for: reading the second
/// off the URL host is a probe that can hand back a *different certificate* than the reader's own
/// connection uses.
///
/// **`config.proxy_url` is a known silence.** kube honours it and this does not — the probe opens
/// its own TCP connection to the `server:` host — so on a kubeconfig behind a proxy the reading is
/// whatever that host answers directly, and usually nothing at all. That is one more
/// [`Serving::Unread`] on the list above rather than a wrong sentence, and closing it means
/// speaking `CONNECT` (or SOCKS5, which this build's kube cannot do either — `Cargo.toml`,
/// `default-features = false`) by hand. It is stated rather than fixed because the fix is a proxy
/// client and the cost of not having one is the silence every other failure here already produces.
///
/// **The deadline and the sample count are parameters, and [`SERVING_PROBE`] and
/// [`SERVING_SAMPLES`] are named at the one call site.** A bound nothing can wait out is a bound
/// nothing tests: the shape it exists for is a listener that accepts and never speaks, and a suite
/// that proved it at the real numbers would spend ten seconds doing so.
///
/// **The samples accumulate outside the timeout on purpose.** Written inside it, a deadline that
/// fires on the last handshake throws away the four readings that already came back — which is the
/// slow endpoint turning a good answer into a silence.
async fn serving_expiry(
    probe: Option<Probe>,
    deadline: std::time::Duration,
    samples: usize,
) -> Serving {
    let Some(probe) = probe else {
        return Serving::Unread;
    };
    let mut seen = Serving::Unread;
    // **One deadline over the lookups, the connects and the handshakes**, because each of them can
    // hang and only the outermost one can bound them all ([`SERVING_PROBE`]).
    let _ = tokio::time::timeout(deadline, async {
        for _ in 0..samples {
            seen = seen.soonest(presented(&probe).await);
        }
    })
    .await;
    seen
}

/// **One handshake, and what it said.**
async fn presented(probe: &Probe) -> Serving {
    let Ok(socket) = tokio::net::TcpStream::connect((probe.host.as_str(), probe.port)).await else {
        return Serving::Unread;
    };
    let connector = TlsConnector::from(std::sync::Arc::clone(&probe.tls));
    match connector.connect(probe.name.clone(), socket).await {
        // Bounded inside [`expiry_of`], which is where the bytes are copied and re-encoded.
        Ok(session) => session
            .get_ref()
            .1
            .peer_certificates()
            .and_then(|chain| chain.first())
            .and_then(|leaf| expiry_of(leaf))
            .map_or(Serving::Unread, Serving::Until),
        Err(refused) => refused_for_expiry(&refused).map_or(Serving::Unread, Serving::Expired),
    }
}

/// **When rustls refused this handshake because the certificate had already run out** — the one
/// failure that is a fact and not a silence, and the fact carries the date
/// (`screens/once.md` § When the certificate is why nothing came back).
///
/// **`tokio-rustls` wraps the typed error in an `io::Error`** —
/// `io::Error::new(ErrorKind::InvalidData, err)` (`tokio-rustls-0.26.4/src/common/mod.rs:115`) —
/// so the fact is reached by downcast and never by matching on a string. Nothing here formats the
/// error: § WHAT WENT WRONG's *select, never format* holds on this path too.
///
/// **`ExpiredContext` and deliberately not the dateless `CertificateError::Expired` beside it.**
/// rustls documents the two as "semantically the same" and they are **not** equal under its own
/// `PartialEq` (`rustls-0.23.43/src/error.rs:376,380,524`), so which one arrives matters — and
/// what decides it is not a coin flip. webpki's `CertExpired { time, not_after }` becomes the
/// context spelling and is the *only* one that means a certificate ran out; the bare `Expired` is
/// where rustls files webpki's `InvalidCertValidity`, which is "notAfter time is earlier than the
/// notBefore time" (`rustls-webpki-0.103.15/src/error.rs:83`) — a certificate nobody could ever
/// have used, not one that has run out, and it carries no date to say anything with. It goes back
/// as one more [`Serving::Unread`], where every other malformed certificate on this path already
/// is. The typed-fact test prints the variant it actually got, and on 2026-08-28 it was
/// `ExpiredContext`; `UnixTime` is whole seconds, which is pinned there too.
fn refused_for_expiry(refused: &std::io::Error) -> Option<Timestamp> {
    use tokio_rustls::rustls::{CertificateError, Error};
    match refused.get_ref()?.downcast_ref::<Error>()? {
        Error::InvalidCertificate(CertificateError::ExpiredContext { not_after, .. }) => {
            Timestamp::from_second(not_after.as_secs() as i64).ok()
        }
        _ => None,
    }
}

/// **Where to open the second connection**, or `None` when this `server:` is not somewhere a
/// handshake can be driven a second time.
///
/// **`https` only.** A kubeconfig pointing at `http://` has no certificate to read and is not a
/// failure to report; the stub server `k8s_tests.rs` builds is exactly that shape.
///
/// **A bracketed IPv6 literal comes back bare**, because that is what both `TcpStream::connect`
/// and rustls's `ServerName` want and what `http::Uri::host` does not give: it keeps the brackets
/// (`[::1]`), and handing that to either is a lookup for a host with no such name. The same
/// unwrapping [`host_of`] does one region up, on the value that has already been parsed.
///
/// **443 is the default and it is HTTPS's, not Kubernetes'.** A `server:` with no port is a URL
/// whose port the scheme decides, which is the same answer kube's own client reaches through
/// `http::Uri`; 6443 is a *kubeadm* convention and writing it here would connect somewhere the
/// client is not.
fn endpoint(config: &Config) -> Option<(String, u16)> {
    let url = &config.cluster_url;
    if url.scheme_str() != Some("https") {
        return None;
    }
    let host = url.host()?;
    let host = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    Some((host.to_string(), url.port_u16().unwrap_or(443)))
}

/// **Which name this handshake is verified against and sends as SNI** — `tls-server-name` where
/// the kubeconfig sets one, and the host it connected to otherwise.
///
/// **It mirrors kube's own connector rather than reasoning about what a name is for**
/// (`kube-client-4.2.0/src/client/config_ext.rs:438`): when `tls_server_name` is set, kube installs
/// a `hyper_rustls::FixedServerNameResolver` built from it, so *its* connection verifies and SNIs
/// that name and not [`endpoint`]'s host. A probe that read the URL host instead would be a second
/// answer to *which server is this*, and this is the file that already refuses to keep two
/// (NOTES § D129).
///
/// **Two failures, and only the second is why this is not a nicety.** `tls-server-name` is exactly
/// what a reader sets when `server:` names an IP the certificate does not cover, so the wrong name
/// verifies against nothing and the report says nothing on an ordinary shape. And an SNI-routing
/// front end can answer two names with two *different* certificates — so the wrong name reads a
/// real expiry off a certificate nobody's kubectl will ever be handed, and the sentence
/// `main.rs::serving_certificate` prints is then confidently about the wrong one (`tester`,
/// 2026-08-28).
///
/// **`None` for a name rustls will not take**, which is the same silence every other failure on
/// this path is: an empty `tls-server-name`, or one that is not a DNS name or an IP literal.
/// **The same trust configuration with nothing that can log in** — the CA to verify against and
/// the reader's own `insecure-skip-tls-verify`, and no credential of any kind.
///
/// **It exists because building the probe's TLS off the whole `Config` *runs the login program*.**
/// `Config::rustls_client_config` asks for a client identity, which goes through kube's
/// `Auth::try_from`, which spawns the `exec` block's command
/// (`kube-client-4.2.0/src/client/auth/mod.rs:344`) — so the first draft of this box started the
/// reader's credential plugin twice per connect: once here and once in `Client::try_from`. On an
/// EKS or OIDC kubeconfig that is a second token round trip, or a second browser window
/// (this box's own second pass, 2026-08-28).
///
/// **Nothing is relaxed by leaving the identity out.** `root_cert` and `accept_invalid_certs` are
/// copied across, so verification is exactly the reader's — the security gate's *TLS verification
/// is never disabled by us*, still structural. What is dropped is the half a *server* certificate
/// has no use for: an API server requests a client certificate and never requires one, which is
/// what makes every token-authenticated `kubectl` in the world work, so the handshake completes
/// without one and the server's certificate arrives before the question is even asked.
///
/// **It is also the smaller claim.** A probe that carries no token, no key and no plugin cannot
/// leak one (invariant 8), and `tls_server_name` is deliberately *not* copied: [`server_name`]
/// reads it off the original.
///
/// **`root_cert_file` is not copied either, and that is a no-op rather than a choice.** kube fills
/// it on the in-cluster path only, which this program has no door into (invariant 3, the security
/// gate's *no in-cluster ServiceAccount path*) — so it is always `None` here, and what it would
/// otherwise switch on is a verifier that re-reads the same CA from disk on rotation.
fn trust_only(config: &Config) -> Config {
    let mut probe = Config::new(config.cluster_url.clone());
    probe.root_cert = config.root_cert.clone();
    probe.accept_invalid_certs = config.accept_invalid_certs;
    probe
}

fn server_name(config: &Config, host: &str) -> Option<ServerName<'static>> {
    let name = config.tls_server_name.clone();
    ServerName::try_from(name.unwrap_or_else(|| host.to_string())).ok()
}

/// **One DER certificate's expiry, answered by the rule's own parser** (NOTES § D129).
///
/// **The wrap is the whole function, and it is what stops a second answer to *when does this
/// certificate expire* existing.** `rules::expires_at` is C1's parser and takes **PEM**; rustls
/// hands back **DER**. Re-parsing the DER here with `x509-parser` is two lines shorter and is the
/// divergence D129 congratulated itself for avoiding — the two would then disagree the day one of
/// them changes, and the one that would not get fixed is this one. What comes free with going
/// through it: the `CERTIFICATE` label check, and RFC 5280 §4.1.2.5's *no well-defined expiry*
/// answering `None` rather than a date in the year 9999 (NOTES § D56).
///
/// **The base64 is `k8s-openapi`'s own**, through the `ByteString` whose `Deserialize` already
/// decodes every `Secret` value and every `client-certificate-data` in this file
/// ([`kubeconfig_certificate`]) — read the other way round. No base64 crate, none wanted for one
/// encode (invariant 10).
///
/// **No line wrapping, which was measured rather than assumed**: `x509-parser`'s PEM reader
/// accepts the body as one line, so the 64-column fold PEM is usually written with buys nothing
/// here. `pem_body_on_one_line_is_a_certificate_this_parser_reads` is what fails if that stops
/// being true.
///
/// **Bounded at [`CERTIFICATE_BYTES`], and here rather than at the handshake**, because this is
/// the function that copies the bytes, base64-encodes them and hands the result to a parser — the
/// security gate's *sizes are bounded*, and the same number and the same rule
/// [`kubeconfig_certificate`] keeps for the certificate on the reader's own disk. It is
/// [`DATE_BYTES`]'s narrower claim: rustls has already read the chain off the wire under the TLS
/// stack's own limits, so what this refuses is three more copies of an unbounded value.
///
/// **`pub(crate)` for one reason: where it can honestly be tested.** `scripts/certs-test.sh`
/// requires every file under `src/` that reads the committed certificates to pin the one instant
/// they are measured from, and `k8s_tests.rs` keeps no such clock — its own `certificate_bytes`
/// doc says so. `main_tests.rs` does pin it, and reads those bytes already, so the positive case
/// is asserted from there against the same instant C1's card is.
pub(crate) fn expiry_of(der: &[u8]) -> Option<Timestamp> {
    // **The length is bound to a name first, and that is not style.** Written inline,
    // `der.len() as u64 > CERTIFICATE_BYTES` makes the flipped comparison a *parse* error —
    // `as u64 <` reads as the start of generic arguments — so the mutation gate files it
    // `unviable` and the boundary this guard exists for is proven by nothing it can see
    // (NOTES § D133: an honest unviable still hides a real question).
    let size = der.len() as u64;
    if size > CERTIFICATE_BYTES {
        return None;
    }
    let encoded = k8s_openapi::serde_json::to_value(k8s_openapi::ByteString(der.to_vec())).ok()?;
    expires_at(
        format!(
            "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
            encoded.as_str()?
        )
        .as_bytes(),
    )
}

// --- THE SERVER'S OWN CERTIFICATE END ---
