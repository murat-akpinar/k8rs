//! **What the screen is looking at** — the whole of it, as data, so that `ui.rs` (Phase 11) can
//! be a pure function of one value and every navigation and filter case is provable with no
//! terminal in the room (todo.md § Phase 10).
//!
//! **Nothing here names a `ratatui` type**, the same rule and the same reason as `theme.rs`
//! (NOTES § D241): `ratatui` is a `[dev-dependencies]` line until Phase 11, and a state file that
//! could not be compiled without a renderer is a state file nobody can test without one. So a
//! selection is a `usize` and an anchor, not a `ListState`; a scroll offset is a `u16`, not a
//! `ScrollbarState`. `screens/widgets.md` § 2 is the mapping from each of these onto the widget
//! that draws it, and it stays on that side of the line.
//!
//! **Nothing here strips a string that came off the API, and that is not an omission**
//! (invariant 9). Every one of those reached this file through `k8s::text` at ingest — the strip is
//! paid once on the way in so no renderer has to remember it, which is what [`crate::rules`]'s and
//! [`crate::analysis`]'s own module docs already promise.
//!
//! **Two classes did *not* arrive that way, and each is handled in one method.** What the user
//! types is [`Input`] — bounded in length, **and** refused a [`crate::k8s::unprintable`]
//! character, reusing the ingest strip's own predicate rather than a second list
//! (NOTES § D246 ruling 5). What the *cluster* says about a mutation is [`Log::outcome`], whose
//! word comes from `ops::Performed::plainly` and therefore from the server: NOTES § D217 measured
//! one of those handing back the whole object that was sent, 4859 bytes on a trivial Deployment,
//! and that is the security gate's *a Secret value never enters the command log* row. It spends
//! [`crate::k8s::text`] itself ([`SAID`]).
//!
//! **It was one method until 2026-09-07 and this paragraph said so** — the second was found by
//! reading who the obvious caller of `outcome` would be (`k8s-admin`, 2026-09-07). That is the
//! whole of invariant 9 inside this file: two calls, two methods, both naming `k8s.rs`'s own
//! guard rather than a second list.
//!
//! **Nothing here sorts a rendered string back into values** (NOTES § D245,
//! `screens/analysis.md` § 3, PRIOR-ART § F1). The browser keeps the order the server sent; the
//! only sort in this file is [`cards`]', over `Severity` and a `Time`, both of which were never
//! strings.

// The renderer that reads all of this is Phase 11's `ui.rs`. Same attribute, same position and
// same accepted blind spot as `theme.rs`'s and `ops.rs`'s (NOTES § D38) — and, like `theme.rs`'s,
// **it does not expire by itself: Phase 11 deletes it by hand.**
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the renderer that reads this state is Phase 11's (todo.md § Phase 11)"
    )
)]

use crate::analysis::Row as ReportRow;
use crate::k8s::{Browsable, Fault, IDENTIFIER, text, unprintable};
use crate::ops::Verdict;
use crate::rules::{
    ContainerSnapshot, ContainerState, Finding, ObjectId, ObjectKind, PodSnapshot, Severity,
    WorkloadSnapshot, age,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use std::borrow::Cow;
use std::cmp::Ordering;

// --- LOADED, EMPTY AND DENIED ARE THREE THINGS START ---

/// **What a pane has, which is three answers and not two** (PRIOR-ART § C2, NOTES § D20).
///
/// *Nothing came back yet* · *it came back and there is nothing in it* · *we were not allowed to
/// look*. Collapsing the third into the second is the one k9s reported most
/// ([#4121](https://github.com/derailed/k9s/issues/4121)), and collapsing the first into the
/// second teaches a reader their cluster is clean while it is still being read
/// (`screens/states.md` § Still loading).
///
/// **`Ready(vec![])` is the empty one and it is a real state**, not a placeholder: `screens/`
/// draws it as `○ nothing is broken`. This type exists so that a renderer cannot reach that
/// sentence by way of a `Vec` that simply has not been filled in yet.
///
/// The same distinction one layer down is [`crate::analysis::Row::NotComputed`] against an empty
/// `Vec` of rows — a report says *did not run* in its body, because a pane that is drawn at all
/// has already loaded. This type is about the pane.
///
/// **[`App`] deliberately holds none of these, and that is the layering rather than an omission.**
/// *Loading* and *denied* are facts about the store, not about what the reader navigated to, so
/// the renderer asks `k8s.rs` and wraps the answer in this on its way to the screen. What this
/// type contributes is that there is one vocabulary for the three, named in the file both
/// renderers read, instead of a `bool` per pane invented twice.
///
/// **There is no `ready() -> Option<&T>` accessor, and its absence is the guard** (NOTES § D246).
/// It collapsed *loading* and *denied* into one `None`, which is #4121 again — and it made the
/// collapse the *ergonomic* call, so a renderer reaches `.ready().map_or(&[], …)` and draws
/// `nothing is broken` over a refusal without ever deciding to. The three-arm `match` is the one
/// that cannot be wrong, and it is three lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pane<T> {
    /// The first answer has not arrived. `screens/states.md` § *Still loading*.
    Loading,
    /// **The cluster refused, and whatever did come back anyway** — the sentence the screen
    /// prints, then the partial answer that sentence is about. Never a `403`, never the word
    /// `RBAC` (invariant 14).
    ///
    /// **The second field is what stops a refusal clearing the screen.** A reader scoped to one
    /// namespace is refused the cluster-wide list and still has that namespace's findings:
    /// `screens/states.md` § *You can only see some namespaces* draws the sentence as a **banner
    /// above the cards**, badge and all, and § *Your login expired* says it in prose — *"stale
    /// data stays visible and stays labelled… k8rs does not clear the screen because it lost its
    /// token"*. A one-field `Denied` could not express *refused, and here is the partial list*,
    /// so the renderer drew the sentence over an empty pane and the reader lost the findings they
    /// already had (2026-09-06).
    ///
    /// **An empty `T` here is *refused, and nothing came back*, which is not [`Pane::Ready`]'s
    /// empty.** It must never reach `nothing is broken`: that is the strongest claim k8rs makes
    /// and a refusal is exactly the moment it cannot be made.
    Denied(String, T),
    /// It came back. An empty `Vec` is *there is nothing*, which is an answer.
    Ready(T),
}

// --- LOADED, EMPTY AND DENIED ARE THREE THINGS END ---

// --- WHAT THE USER TYPES START ---

/// **One line the user typed** — a filter, or the object name a delete asks to have typed back.
///
/// **It is bounded, and the bound is the security gate's row about sizes, not a nicety.** A
/// bracketed paste hands a terminal a whole file in one event; nothing between the keyboard and
/// this buffer has a length opinion, and this is the one string in the file that did not come
/// through `k8s::text`'s bound at ingest. [`crate::k8s::IDENTIFIER`] is reused rather than
/// re-picked so that the longest name a dialog can *ask* for is exactly the longest name this can
/// *hold*: a name stripped to 512 bytes on the way in stays typeable back (NOTES § D146).
///
/// **The same paste is also the invariant-9 event, so the length is not the only thing checked**
/// (NOTES § D246 ruling 5). A bracketed paste of `\u{1b}[2Jweb` is one event carrying an escape
/// sequence into a buffer the renderer draws — `screens/widgets.md` § 7's *an escape sequence in
/// a pod name reaches the terminal and rewrites it*, on the one string `k8s::text` never saw.
/// [`crate::k8s::unprintable`] is reused rather than re-listed, for the same reason the bound is:
/// what the ingest strip removes from a name is exactly what cannot usefully be typed back.
///
/// **Bytes, not characters** — the same unit the bound it borrows is measured in — and a
/// character is never cut in half, because [`Input::push`] refuses a whole `char` or takes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Input(String);

impl Input {
    /// What has been typed so far.
    pub fn text(&self) -> &str {
        &self.0
    }

    /// Whether anything has been typed. A filter that is empty matches everything
    /// ([`Filters::matches`]).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Add one character, **or refuse it silently** — past the bound, or because it is one of the
    /// [`crate::k8s::unprintable`] set (invariant 9). Refusing silently is the right failure for
    /// both: the alternative is a buffer that grows with the paste, and there is nothing a reader
    /// can do about a message saying so.
    pub fn push(&mut self, character: char) {
        if !unprintable(character) && self.0.len() + character.len_utf8() <= IDENTIFIER {
            self.0.push(character);
        }
    }

    /// Backspace — **one `char`, not one byte**, or a multi-byte name is corrupted into something
    /// that can never match the name a delete is asking for.
    pub fn pop(&mut self) {
        self.0.pop();
    }

    /// `esc` on a filter, and what a dialog does when it closes.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

// --- WHAT THE USER TYPES END ---

// --- A CURSOR THAT STAYS ON THE SAME OBJECT START ---

/// **Which row the keys act on, and the identity it is anchored to** — so that a re-fetch which
/// reorders or removes rows cannot silently move the cursor onto a different object.
///
/// **The anchor is the object's `uid` and never its name**, which is [`crate::k8s::Row::uid`]'s
/// own reason: a name deleted and recreated is a different object, and invariant 2's *explicitly
/// selected object* is defeated by a cursor that followed a string. The caller supplies the
/// anchors; this type does not know what a row is.
///
/// **`None` in the key slice is a row with no stable identity to follow**, which is real: a Table
/// fetched with `?includeObject=None` carries no uid at all
/// (`tests/fixtures/table-deployments.json`). Such a row can be selected and cannot be followed,
/// so the cursor falls back to its index — stated here rather than found later.
///
/// **Movement clamps and does not wrap.** `↓` at the last row stays on the last row. A list that
/// wraps sends a reader who is holding a key from the bottom back to the top without their
/// noticing, and there is no mockup in `screens/` that asks for one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Cursor {
    index: usize,
    anchor: Option<String>,
}

impl Cursor {
    /// Which row the keys act on, or **`None` for an empty list** — the state every caller has to
    /// handle and the one an index alone cannot express.
    pub fn selected(&self, keys: &[Option<&str>]) -> Option<usize> {
        (!keys.is_empty()).then(|| self.index.min(keys.len() - 1))
    }

    /// Put the cursor on this row and remember what is there. Out of range is clamped rather than
    /// refused: the caller that computed the index and the list it computed it against can be one
    /// event apart.
    pub fn select(&mut self, index: usize, keys: &[Option<&str>]) {
        if keys.is_empty() {
            self.index = 0;
            self.anchor = None;
            return;
        }
        self.index = index.min(keys.len() - 1);
        self.anchor = keys[self.index].map(str::to_owned);
    }

    /// `↑` / `k`. **Moves from [`Cursor::selected`] and never from the raw index**, so a key press
    /// against a list that shrank since the last [`Cursor::follow`] moves one row from where the
    /// screen is drawing the highlight (NOTES § D246). Reading the field instead left index 49
    /// against three rows: `↑` clamped 48 back to 2 and the reader's first press did nothing.
    pub fn up(&mut self, keys: &[Option<&str>]) {
        self.select(self.selected(keys).unwrap_or(0).saturating_sub(1), keys);
    }

    /// `↓` / `j`.
    pub fn down(&mut self, keys: &[Option<&str>]) {
        self.select(self.selected(keys).unwrap_or(0).saturating_add(1), keys);
    }

    /// **The list changed underneath the cursor** — a re-fetch, a filter, a watch event.
    ///
    /// The anchored object is found and the cursor follows it wherever it moved. When it is
    /// **gone** the index is kept, clamped — which is the only honest answer left: the object the
    /// user selected does not exist, so there is nothing to stay on, and holding position is what
    /// every list in every editor does. The cursor re-anchors on whatever is now there, so the
    /// next `⏎` acts on the object the screen is showing and not on a memory.
    ///
    /// **Called on every rebuild of the list, including a filter keystroke.** A cursor that is
    /// only re-anchored on a re-fetch drifts the moment `/` shortens the list under it.
    pub fn follow(&mut self, keys: &[Option<&str>]) {
        if keys.is_empty() {
            self.index = 0;
            self.anchor = None;
            return;
        }
        let moved_to = self
            .anchor
            .as_deref()
            .and_then(|anchor| keys.iter().position(|key| *key == Some(anchor)));
        self.select(moved_to.unwrap_or(self.index), keys);
    }
}

// --- A CURSOR THAT STAYS ON THE SAME OBJECT END ---

// --- ONE CARD PER OWNER START ---

/// **One card: everything filed under one owner** (NOTES § D3).
///
/// A DaemonSet on forty nodes is one of these and not forty. The identity is
/// [`ObjectId::group_key`]'s — kind, namespace, name, **never the uid** — which `rules.rs`
/// decided there precisely so this file could not hold a second copy that drifts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// What the card is filed under, and its title line — `payments/web`, or the bare `node-3`
    /// for something cluster-scoped (`screens/README.md` § the five rules).
    pub owner: ObjectId,
    /// Everything filed here, in the order `analyze` produced it.
    pub findings: Vec<Finding>,
    /// **How many distinct pods this card is about** — the numerator of *"3 of 40 pods"*.
    ///
    /// The spec is [`Finding::object`]'s and is followed literally: the number of distinct
    /// `object`s in the group whose kind is `Pod`, **distinct over the whole `ObjectId`, uid
    /// included**. `0` is a card about no pods at all — every node card — and it draws no count
    /// (NOTES § D39). **It is not the only reason a card draws none**: see [`Card::count`], which
    /// also drops the fragment for a bare pod and the denominator for a number that cannot be
    /// defended.
    pub affected: usize,
    /// **How many pods that owner is supposed to have**, or nothing at all.
    ///
    /// `None` is not zero and not an error: the denominator comes from the workload watch and not
    /// from the pods (NOTES § D28), so a reader allowed `pods` but not `deployments` has the
    /// numerator and not the total, and the card says `3 pods` instead of `3 of 5 pods`
    /// (`screens/alerts.md`). A node card and a bare-pod card have no workload behind them either.
    pub total: Option<i32>,
}

impl Card {
    /// **The worst thing on the card**, which is what the card is drawn as and what it sorts by.
    ///
    /// [`cards`] never builds an empty one, so the `unwrap_or` is unreachable rather than a
    /// default anybody meets; `Info` is the harmless direction if it ever is.
    pub fn severity(&self) -> Severity {
        self.findings
            .iter()
            .map(|finding| finding.severity)
            .min()
            .unwrap_or(Severity::Info)
    }

    /// **The finding whose event is the most recent *and drawable***, or `None` when nothing here
    /// has a time this reader can use. What the card's right edge is drawn from, and its second
    /// sort key.
    ///
    /// **"Has a timestamp" is not the test; [`Finding::age`] answering is** (NOTES § D246
    /// ruling 4). `rules::age` refuses a stamp more than its skew allowance into the future, so a
    /// card carrying one drew a blank age — and, worse, *one* such finding beside a dozen ordinary
    /// ones suppressed a perfectly drawable age, because this is what [`Card::age`] asks. A laptop
    /// resumed from suspend before NTP catches up reads every finding as future and loses the
    /// whole right-hand column. Filtering on the same call that draws it is what keeps
    /// [`Card::age`] and [`newest_first`] from disagreeing about which findings exist.
    ///
    /// **What that agreement costs, measured rather than reasoned about:** [`Finding::age`]
    /// formats a `String` to answer a yes/no question, and [`newest_first`] asks it inside the
    /// comparator — so [`cards`] over **2000 cards** went **8.5 ms → 16.8 ms** at one finding each
    /// and **46.8 ms → 71.2 ms** at five (release, 2026-09-06). Same order, same named O(n²)
    /// ceiling, and two orders of magnitude under the ~100 ms storm-coalescing window that D3's
    /// short card list never approaches. Should a cluster ever make it approach one, the fix is to
    /// compute this once per card in [`cards`] and sort on the answer — not to re-derive the skew
    /// rule here, which is the copy that would drift from `rules::age`.
    pub fn newest(&self, now: &Time) -> Option<&Finding> {
        self.findings
            .iter()
            .filter(|finding| finding.age(now).is_some())
            .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
    }

    /// **How long ago, or nothing** — and it goes through [`Finding::age`], never the free
    /// [`crate::rules::age`], so this renderer cannot disagree with `--once` about one moment and
    /// cannot swap that function's two same-typed arguments (NOTES § D69).
    pub fn age(&self, now: &Time) -> Option<String> {
        self.newest(now)?.age(now)
    }

    /// **`3 of 5 pods` · `3 pods` · nothing** — the `· n of m pods` fragment of the identity line
    /// (`screens/alerts.md`), spelled in one place because it is drawn in one place.
    ///
    /// **Nothing, for two different reasons, and both are the screen's** (NOTES § D246 ruling 2).
    /// [`Card::affected`] is `0` — that count counts pods, and a node card is about one machine
    /// (NOTES § D39). Or the owner **is** the pod: nothing owns it, so `owner == object` and a
    /// fraction of one pod out of itself is not a fact. `screens/alerts.md` draws that card twice
    /// off committed captures — `default/broken-pending`, `default/broken-hostpath` — with the
    /// identity line ending after the name, and says it twice in prose: *"a bare pod, so there is
    /// no owner and no `n of m`"*. Every kubeadm and kind cluster has a second shape of it, the
    /// mirror pod `kube-system/etcd-…`.
    ///
    /// **`3 pods`** when the denominator was not readable — **and when it cannot be believed**.
    /// [`Card::total`] is `spec.replicas`, which is what the workload was *asked* for and is no
    /// upper bound on what *exists*: one replica, the old pod stuck `Terminating` behind a
    /// finalizer while its replacement cannot pull, is two pod objects at `desired: 1`. So where
    /// `affected > total` the denominator is dropped rather than clamped or printed — the same
    /// move `screens/alerts.md` § *the third form* makes for `unavailableReplicas`, in those
    /// words: *a denominator here would eventually print `2 of 1 pod not answering`*. Clamping
    /// would hide a pod that has a finding, which is the one direction that file forbids outright;
    /// printing it is PRIOR-ART § F2, *a number that cannot be defended*.
    ///
    /// **The unit word is plural at 1**, which `screens/alerts.md` draws itself — `data/migrate-job
    /// ·  1 of 1 pods`. It follows that `1 pods` is still reachable, on an **owned** single-pod
    /// card whose workload could not be read; inventing a singular for it here would put two
    /// spellings on one line, and the wording is `tui-designer`'s to settle in `screens/` first
    /// (NOTES § D246 ruling 2, `backlog.md`).
    pub fn count(&self) -> Option<String> {
        if self.affected == 0 || self.owner.kind == ObjectKind::Pod {
            return None;
        }
        match self.total.and_then(|total| usize::try_from(total).ok()) {
            Some(total) if self.affected <= total => {
                Some(format!("{} of {total} pods", self.affected))
            }
            _ => Some(format!("{} pods", self.affected)),
        }
    }
}

/// **Findings folded into cards, sorted the way the Alerts view draws them** — the one thing
/// standing between Alerts and a 400-line lint report (NOTES § D3).
///
/// `workloads` is the denominator's only source, matched on [`ObjectId::group_key`]; pass an empty
/// slice and no card can read `n of m`, which is exactly the state a reader without `deployments`
/// is in (`screens/alerts.md`). What each card then draws is [`Card::count`]'s — `n pods`, or
/// nothing at all where the owner is a node or the pod itself.
///
/// **The sort is severity first, then recency, and both halves have a trap.**
/// [`Severity`]'s derived `Ord` runs `Critical < Warn < Info`, so *severity descending* on screen
/// is **ascending** here; and `Option<Time>`'s derived `Ord` puts `None` **first**, where
/// `screens/alerts.md` wants ageless cards **last inside their own band** — so both are written
/// out by hand below rather than reached for (NOTES § D69). Ties keep `analyze`'s order, because
/// `sort_by` is stable.
///
/// **`now` is the caller's moment — [`crate::rules::ClusterSnapshot::now`], captured once — and it
/// is here so the sort and the screen agree about which findings have an age at all** (NOTES
/// § D246 ruling 4). Without it this function sorted on a raw timestamp the renderer then refused,
/// so a card whose clock ran ahead drew nothing on its right edge and sorted to the **top** of its
/// band, where `screens/alerts.md` puts ageless cards **last**.
///
/// **Its ceiling, named rather than left to be found: the fold and the denominator lookup are
/// both linear scans**, so this is O(findings × cards) and O(cards × workloads). A `HashMap` is
/// not available for the fold — `ObjectId` deliberately does not derive `Hash`, which is the
/// two-cards bug made a compile error (NOTES § D38) — and D3's whole claim is that the card list
/// is short. If a cluster ever makes it long, the key to build a map on is `group_key()`.
pub fn cards(findings: &[Finding], workloads: &[WorkloadSnapshot], now: &Time) -> Vec<Card> {
    let mut cards: Vec<Card> = Vec::new();
    for finding in findings {
        match cards
            .iter_mut()
            .position(|card| card.owner.group_key() == finding.owner.group_key())
        {
            Some(at) => cards[at].findings.push(finding.clone()),
            None => cards.push(Card {
                owner: finding.owner.clone(),
                findings: vec![finding.clone()],
                affected: 0,
                total: None,
            }),
        }
    }

    for card in &mut cards {
        // **Distinct is the whole `object`, uid included** — a `Vec` and a linear `contains`, not
        // a set of `group_key()`, which answers *which card* rather than *what is counted on it*
        // ([`Finding::object`], NOTES § D39).
        let mut pods: Vec<&ObjectId> = Vec::new();
        for finding in &card.findings {
            if finding.object.kind == ObjectKind::Pod && !pods.contains(&&finding.object) {
                pods.push(&finding.object);
            }
        }
        card.affected = pods.len();
        card.total = workloads
            .iter()
            .find(|workload| workload.id.group_key() == card.owner.group_key())
            .and_then(|workload| workload.desired);
    }

    cards.sort_by(|a, b| {
        a.severity()
            .cmp(&b.severity())
            .then_with(|| newest_first(a, b, now))
    });
    cards
}

/// The recency half of [`cards`]' comparator: newer before older, **and no *drawable* age last** —
/// written out because every derived ordering available here gets one of the two backwards, and it
/// asks [`Card::newest`] rather than the raw field so that *no age* means the same thing here as
/// on the screen (NOTES § D246 ruling 4).
fn newest_first(a: &Card, b: &Card, now: &Time) -> Ordering {
    match (
        a.newest(now).and_then(|f| f.timestamp.as_ref()),
        b.newest(now).and_then(|f| f.timestamp.as_ref()),
    ) {
        (Some(left), Some(right)) => right.cmp(left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

// --- ONE CARD PER OWNER END ---

// --- THE TWO FILTERS, AND THE ONE THAT DOES NOT EXIST START ---

/// **What is being filtered out of the current list** — `/` by text, `n` by namespace
/// (NOTES § D12).
///
/// **There is no severity filter and there is no field for one.** Owner grouping made the list
/// short and severity is already the sort order, so the key was deleted rather than rebound
/// (NOTES § D12). It is absent here so that nothing can quietly grow one back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    /// `/` — text anywhere in the row.
    pub text: Input,
    /// `n` — a substring of the namespace, so `pay` reaches `payments`.
    pub namespace: Input,
}

impl Filters {
    /// **Does this row survive both filters?** An empty filter matches everything, which is what
    /// makes *no filter* and *a filter that matches all* the same screen.
    ///
    /// `fields` is whatever the row shows the reader — a card's title and evidence, a table row's
    /// cells. **Case-insensitive, ASCII only**: a beginner types `web`, not `Web`, and full
    /// Unicode case folding would allocate per row per keystroke for a gain nothing in `screens/`
    /// asks for.
    ///
    /// **A row with no namespace is refused by any namespace filter**, never matched by accident.
    /// Nodes and persistent volumes have no namespace to be `payments`, and a cluster-scoped row
    /// surviving `n payments` would be a list that answers a question nobody asked.
    pub fn matches(&self, namespace: Option<&str>, fields: &[&str]) -> bool {
        let namespace_ok = self.namespace.is_empty()
            || namespace.is_some_and(|value| contains_ignoring_case(value, self.namespace.text()));
        let text_ok = self.text.is_empty()
            || fields
                .iter()
                .any(|field| contains_ignoring_case(field, self.text.text()));
        namespace_ok && text_ok
    }
}

/// Substring, ASCII case folded. **Two allocations per call, and [`Filters::matches`] calls it once
/// per field — so the cost is two per *field*, not two per row** (NOTES § D246 ruling 5; the doc
/// here said *per call and no more*, which read as a per-row budget it never was). Measured at
/// **1.12 ms per keystroke over 5000 rows × 5 cells**, which is why it stays the obvious two-
/// `to_ascii_lowercase` version rather than growing a case-folding matcher.
fn contains_ignoring_case(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

// --- THE TWO FILTERS, AND THE ONE THAT DOES NOT EXIST END ---

// --- THE SIDEBAR, BUILT FROM DISCOVERY START ---

/// **The five sidebar groups** — `workloads` `network` `storage` `config` `cluster`
/// (`screens/resources.md`). There is no sixth, and adding one is a `screens/` change first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Workloads,
    Network,
    Storage,
    Config,
    Cluster,
}

impl Group {
    /// In sidebar order, top to bottom, exactly as every mockup in `screens/` draws them.
    pub const ALL: [Group; 5] = [
        Group::Workloads,
        Group::Network,
        Group::Storage,
        Group::Config,
        Group::Cluster,
    ];

    /// The row's text. Lowercase, as drawn.
    pub fn label(self) -> &'static str {
        match self {
            Group::Workloads => "workloads",
            Group::Network => "network",
            Group::Storage => "storage",
            Group::Config => "config",
            Group::Cluster => "cluster",
        }
    }

    /// **Which of the five a kind belongs in — decided in three steps, none of which is a list of
    /// every kind the cluster serves** (invariant 12).
    ///
    /// 1. **The API group names the topic, where it has one.** `apps`, `batch`, `autoscaling` are
    ///    workloads; `networking.k8s.io` and `discovery.k8s.io` are network; `storage.k8s.io` is
    ///    storage; the rest of the built-in groups describe the cluster itself. This is a table of
    ///    **groups**, and a group is a topic — which is what a group is for.
    /// 2. **The core group is the one exception, and it is the only closed one.** `""` predates
    ///    group naming and holds kinds of four different topics at once, so nothing about the
    ///    group can place them and the plurals are named. **It cannot go stale the way a per-kind
    ///    column list does**: a CustomResourceDefinition's group must contain a dot, so no CRD can
    ///    ever land here, and the set of core kinds only changes when Kubernetes itself does. A
    ///    core kind not named below still draws, under `cluster`.
    /// 3. **Anything left is a group this file does not name, and `namespaced` places it.** That
    ///    is usually a CRD — a namespaced custom resource is something somebody deployed into a
    ///    namespace (`workloads`), a cluster-scoped one configures the cluster (`cluster`) — and
    ///    it is why `Rollout.argoproj.io` gets a row without ever having been heard of. **It is
    ///    not only CRDs, which is what this said and it was false** (NOTES § D246 ruling 5):
    ///    Kubernetes serves *aggregated* built-in groups too, and `resource.k8s.io`,
    ///    `apiserverinternal.k8s.io` and `storagemigration.k8s.io` all arrive here. Those three
    ///    land where `namespaced` would have put them anyway; `metrics.k8s.io` did not, and is
    ///    named in step 1's table — its resource is spelled `pods` and is namespaced, so
    ///    `workloads` drew **two adjacent rows both reading `pods`** on every cluster running
    ///    metrics-server.
    ///
    /// `namespaced` is the one field on [`Browsable`] that carries a fact about the kind rather
    /// than about its spelling, which is why the last resort is a **named** bucket rather than a
    /// drop or a junk drawer.
    pub fn of(kind: &Browsable) -> Group {
        match kind.group.as_str() {
            "apps" | "batch" | "autoscaling" => Group::Workloads,
            "networking.k8s.io" | "discovery.k8s.io" | "gateway.networking.k8s.io" => {
                Group::Network
            }
            "storage.k8s.io" => Group::Storage,
            "" => match kind.plural.as_str() {
                "pods" | "podtemplates" | "replicationcontrollers" => Group::Workloads,
                "services" | "endpoints" => Group::Network,
                "persistentvolumes" | "persistentvolumeclaims" => Group::Storage,
                "configmaps" | "secrets" | "serviceaccounts" | "limitranges" | "resourcequotas" => {
                    Group::Config
                }
                _ => Group::Cluster,
            },
            "rbac.authorization.k8s.io"
            | "certificates.k8s.io"
            | "admissionregistration.k8s.io"
            | "apiextensions.k8s.io"
            | "apiregistration.k8s.io"
            | "policy"
            | "coordination.k8s.io"
            | "node.k8s.io"
            | "scheduling.k8s.io"
            | "flowcontrol.apiserver.k8s.io"
            | "authentication.k8s.io"
            | "authorization.k8s.io"
            | "events.k8s.io"
            // Aggregated, not a CRD, and its `pods` is namespaced — so the step-3 last resort put
            // a second row reading `pods` next to the core one (NOTES § D246 ruling 5). What it
            // serves is how much a node or a pod is *using*, which is the cluster's own vitals.
            | "metrics.k8s.io" => Group::Cluster,
            _ if kind.namespaced => Group::Workloads,
            _ => Group::Cluster,
        }
    }
}

/// **One row of the sidebar** — a flat list, because `screens/widgets.md` § 2 draws it as one
/// `List` and the indentation is a rendering detail rather than a tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavItem {
    /// `ALERTS` — a section and a destination at once, and the row selected on startup
    /// (`screens/alerts.md`).
    Alerts,
    /// `RESOURCES` and `ANALYSIS`: drawn, **never selected**. `↑↓` skips them
    /// (`screens/widgets.md` § 2), which is why [`sidebar`] hands the cursor a filtered list.
    Header(&'static str),
    /// One of the five groups. Selectable: `⏎` opens and closes it.
    Group(Group),
    /// One browsable kind, by its index in the discovery list [`sidebar`] was given.
    Kind(usize),
    /// One analysis report, by its index in the report list.
    Report(usize),
}

impl NavItem {
    /// Whether `↑↓` may land here. The two section headers are the only rows that refuse.
    pub fn selectable(self) -> bool {
        !matches!(self, NavItem::Header(_))
    }
}

/// **The sidebar, built from what the cluster said it serves** — never from a list of kinds
/// written here (invariant 12).
///
/// `kinds` is `k8s::browsable()`'s answer, already sorted and already stripped. `reports` is how
/// many analysis reports there are; the sidebar draws them by index because their labels and
/// badges live on the [`crate::analysis::Report`] itself.
///
/// **`expanded` is one group or none**, which is what every mockup draws: `screens/alerts.md`
/// shows all five closed, `screens/resources.md` shows `workloads` open and the other four closed.
/// A second open group is not a state any screen asks for, and one `Option` is how it stays
/// unrepresentable.
///
/// **A group with no kinds under it still draws its row.** A cluster serving nothing in `storage`
/// is a fact about that cluster; a row that vanishes is a reader wondering where the section went.
///
/// **[`NavItem::Kind`] carries the index into `kinds`, not the index within its group**, which is
/// why the `enumerate` is before the `filter` and not after. Swapping the two opens a different
/// kind under every group but the first, and it is invisible from any test that only opens
/// `workloads` (NOTES § D246 ruling 6).
pub fn sidebar(kinds: &[Browsable], reports: usize, expanded: Option<Group>) -> Vec<NavItem> {
    let mut nav = vec![NavItem::Alerts, NavItem::Header("RESOURCES")];
    for group in Group::ALL {
        nav.push(NavItem::Group(group));
        if expanded == Some(group) {
            nav.extend(
                kinds
                    .iter()
                    .enumerate()
                    .filter(|(_, kind)| Group::of(kind) == group)
                    .map(|(at, _)| NavItem::Kind(at)),
            );
        }
    }
    nav.push(NavItem::Header("ANALYSIS"));
    nav.extend((0..reports).map(NavItem::Report));
    nav
}

/// **The rows of a list a cursor may land on**, by index into the original — the adapter between
/// a list that has unselectable rows and a [`Cursor`], which has none.
///
/// Two lists need it, for one reason stated in two places: the sidebar's section headers
/// (`screens/widgets.md` § 2) and an analysis report's `Prose` and `NotComputed` rows
/// (NOTES § D127). Both are *"skipped, exactly as the sidebar's own group headers are"*, which is
/// `analysis.rs`'s own wording, so they share one mechanism rather than two.
pub fn selectable<T>(rows: &[T], may_select: impl Fn(&T) -> bool) -> Vec<usize> {
    rows.iter()
        .enumerate()
        .filter(|(_, row)| may_select(row))
        .map(|(at, _)| at)
        .collect()
}

/// **Whether the cursor may land on this report row** — [`crate::analysis::Row::Answer`] and
/// nothing else (NOTES § D127).
///
/// **The variant decides it, never a field.** Keying on `jump.is_some()` was the first draft's
/// mistake: it skips every counted row the screen advertises `⏎` on, and a report whose rows are
/// all `Prose` would park the highlight on a line that opens nothing
/// (`crate::analysis::Row`'s own doc).
pub fn answers(row: &ReportRow) -> bool {
    matches!(row, ReportRow::Answer { .. })
}

// --- THE SIDEBAR, BUILT FROM DISCOVERY END ---

// --- THE MODAL LAYER START ---

/// **What is open over the screen — one thing, or nothing** (`screens/widgets.md` § 5).
///
/// **The enum makes stacking unrepresentable, and that is the point.** A dialog that can open over
/// a dialog is how a confirmation ends up applying to the wrong object. There is no modal stack
/// and no z-index; `esc` closes exactly one level, and one level is all there is.
///
/// **[`Self::Refused`] and [`Self::Gone`] are their own variants rather than a [`Dialog`] wearing
/// a different message**, and the reason is structural (`screens/widgets.md` § 5): a `Confirm`
/// arms the moment a verdict lands ([`Dialog::armed`]), while both of these are *terminal* — they
/// never arm and they offer only `esc dismiss`. A `Confirm` that could reach either would need
/// `armed()` to stay false forever after a `Some` verdict, which it has no way to express.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Modal {
    /// `?` — the full key map (`screens/help.md`).
    Help,
    /// A mutation waiting for the person at the keyboard (`screens/dialogs.md`).
    Confirm(Dialog),
    /// **A write that did not land, or that k8rs cannot say landed** (`screens/dialogs.md` § The
    /// cluster said no) — the third state is the second half of that sentence and not a weaker
    /// form of the first (invariant 2).
    ///
    /// **It carries the fault and not just the words, because two of the sentences that box used
    /// to print are false for half the cases that reach it.** `ops::Outcome` distinguishes
    /// `NotSent { fault, said }` from `Failed { fault, said }`, and this variant threw both away:
    /// *"Nothing was changed."* and *"This is the check that runs before the real change — it
    /// stopped this one."* were fixed text. `delete` is `checkable: false` (NOTES § D225 ruling 1)
    /// so **no check is ever sent** and every delete refusal is post-send; and invariant 2 names
    /// the state where the first sentence is unknowable — a dead socket on a delete ends in *k8rs
    /// does not know whether the change was made*. A hardcoded sentence standing in for a typed
    /// fault is PRIOR-ART § C1, which this repo lists as the defect to avoid.
    Refused {
        /// **Whether the real call went out** — `ops::Outcome::Failed` rather than `NotSent`. It
        /// is what decides whether a check can be said to have stopped anything, and [`Fault`]
        /// cannot answer it: a `403` comes back from a `dryRun=All` and from a live `DELETE`
        /// alike.
        sent: bool,
        /// **What went wrong, as `k8s.rs` classified it** — never a second vocabulary for the
        /// same errors, and never a sentence built from what the server said.
        fault: Fault,
        /// **The cluster's own words, where it sent any** — `ops::Outcome::said`, which arrived
        /// already stripped and bounded from `k8s::said` (invariant 9).
        ///
        /// **`None` is *it refused and explained nothing*, and the box then draws no quote block
        /// at all** rather than the heading over an empty space.
        ///
        /// **It is still cut at draw time, and that is the security gate's *sizes are bounded*
        /// row and not tidiness**: `k8s::FREE_TEXT` allows 4096 bytes, and a
        /// `fieldValidation=Strict` rejection hands back the whole object that was sent —
        /// 4859 bytes on a trivial Deployment (NOTES § D217). That is eighty wrapped lines into a
        /// box that has room for four.
        said: Option<String>,
    },
    /// **The object stopped existing while the dialog was open** (`screens/dialogs.md` § The
    /// object went away, NOTES § D22).
    Gone {
        /// What the dialog opened on — the box claims this and nothing else.
        object: Object,
        /// **Whether anything normally puts one back** — which of § The object went away's two
        /// sentences this box says. True for a pod and a replicaset, whose creator k8rs has not
        /// read; false for a deployment, a statefulset, a daemonset and a node, which nothing
        /// recreates on its own.
        ///
        /// **A field and not a match on [`Object::kind`] in the renderer**, so `ui.rs` grows no
        /// kind table of its own. **It has no home below this yet, and saying it had was wrong**:
        /// `main.rs`'s `KINDS` carries `singular`, `short` and `namespaced` and nothing about
        /// what recreates a kind, so the caller Phase 12 writes is what decides this — and that
        /// caller is the place a flag belongs if one is ever wanted.
        ///
        /// **Neither sentence names a successor, and that is a repaired defect rather than an
        /// omission** (NOTES § D44): the box used to read `replaced by web-2c81a 3 seconds ago`,
        /// an inference off a shared `ownerReference` that named the wrong pod whenever the
        /// ReplicaSet scaled for another reason. There is no successor-matching anywhere in k8rs
        /// to back one.
        recreated: bool,
    },
}

/// **Which object a modal is about, in the four facts a screen and a safety check need**
/// (`screens/dialogs.md` rule 1).
///
/// **One type for [`Dialog`] and [`Modal::Gone`], because the second is the first after the watch
/// answered** — `screens/dialogs.md` § The object went away's *this box now claims only what the
/// dialog's own identity fields back*.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    /// The kind in the word a manifest spells — `pod`, `deployment`, `node`. What
    /// *"Type the pod's name to confirm"* and *"This pod is already gone"* interpolate.
    ///
    /// **`&'static str`, for [`Dialog::verdict`]'s own reason** (NOTES § D246): every sentence a
    /// dialog draws is k8rs's own, and a `String` here is nothing structurally stopping a kind
    /// word the API sent from being interpolated into one. The six kinds an operation can be
    /// pointed at are literals in the driver.
    pub kind: &'static str,
    /// Its namespace, or `None` for something cluster-scoped. No namespace is drawn where there
    /// is none — the title bar reads the bare `node-3`, never `/node-3`
    /// (`screens/README.md` § the five rules).
    pub namespace: Option<String>,
    /// Its own name — `web`, `web-7d9f4`, `node-3`. The second half of every title bar, and what
    /// a delete asks to have typed back.
    pub name: String,
    // **Private, and the only field here that is** — [`Object::new`] is the one way to set it and
    // it is what refuses `Some("")`.
    /// **The cluster's own name for *this* instance of that name** — what answers *is this still
    /// the same object*, which is the question a dialog left open exists to ask
    /// (NOTES § D22, `ops::Mutation::uid`, `ops::Deleting::uid`).
    ///
    /// **Deliberately not the `resourceVersion`** (NOTES § D228). That field answers *has anything
    /// at all been written*, and it moves on a status-only write by the object's own controller:
    /// measured, a `CrashLoopBackOff` Deployment whose spec never moved wrote 20 times in 99.4 s,
    /// median gap 2.45 s. A modal keyed on it would kill its own confirm button every couple of
    /// seconds on exactly the object an operator opened it for.
    ///
    /// **It comes off the selection and not off a watch, which is what answers the one kind that
    /// has no watch.** Invariant 6 fetches ReplicaSets on demand and never watches them — but
    /// that is `rules.rs`'s owner lookup, and no dialog opens on it. A dialog opens on a selected
    /// row: `rules::ObjectId::uid` in Alerts, `k8s::Row::uid` in the browser, and a browser row
    /// carries the `uid` the same `Table` fetch that drew it carried. What answers *is it still
    /// there* is the pane behind the modal, which for a browsed kind is that kind's own watch —
    /// open, because the view it belongs to is still open under the dialog.
    ///
    /// **`None` is *k8rs cannot tell this object from the next one to hold its name*, and both
    /// guards are then off**: no [`Modal::Gone`] can be raised for it, and `ops::delete` sends no
    /// `preconditions.uid` either (NOTES § D235). What must not happen is a fallback to comparing
    /// *names*, because a name that has gone is exactly when it belongs to somebody else — the
    /// defect NOTES § D22 exists for.
    ///
    /// **Which selections can raise [`Modal::Gone`] at all, said plainly so a Phase 12 wiring
    /// cannot assume a guard that is not there.** The `uid` comes off the selection, but what
    /// turns a dialog into *Already gone* is what is *watching* afterwards, and invariant 6
    /// watches Pods, Nodes and Deployments/StatefulSets/DaemonSets and nothing else. So:
    ///
    /// - **an Alerts card** — raises `Gone` for those five kinds, and **not for a ReplicaSet**,
    ///   which `crate::rules::ObjectId::name`'s own doc confirms is a real Alerts selection (rule
    ///   W1's object is one). Nothing watches ReplicaSets;
    /// - **a browser row** — raises `Gone` for whichever kind's view is open, because that view's
    ///   own watch is running under the dialog.
    ///
    /// **Where `Gone` cannot be raised the operator meets a `409` from `preconditions.uid`
    /// instead** (NOTES § D235) — safe, and the write does not land on the wrong object, but it
    /// is PRIOR-ART § G1's *refuses for no visible reason*. Routing a watch is not this box's.
    uid: Option<String>,
}

impl Object {
    /// **The one way to build one**, because it is the one place [`Self::uid`]'s empty string is
    /// refused.
    ///
    /// **`Some("")` is strictly worse than `None` and had no guard.** `k8s::Row::uid` does not
    /// filter it, `preconditions: { uid: Some("") }` is a `409` no re-read can ever clear, and a
    /// [`Modal::Gone`] check against it flips a healthy object to *Already gone* the instant the
    /// dialog opens. `k8s::owner_uid` already refuses an empty uid one layer down; this is the
    /// same refusal at the other end.
    pub fn new(
        kind: &'static str,
        namespace: Option<String>,
        name: String,
        uid: Option<String>,
    ) -> Self {
        Self {
            kind,
            namespace,
            name,
            uid: uid.filter(|uid| !uid.is_empty()),
        }
    }

    /// The cluster's own name for this instance, or `None` where there is none to trust.
    pub fn uid(&self) -> Option<&str> {
        self.uid.as_deref()
    }
}

/// **What a confirmation dialog holds while it is open** — the drawable half of
/// `ops::Shown` and `ops::Checked`, plus the line being typed.
///
/// **It holds no `ops::Checked` and no `ops::Agreed`, and that is deliberate.** Those are
/// `ops.rs`'s, they are the only route to a mutation, and nothing outside that file can build one
/// — which is invariant 2's *"a confirmation cannot be forged"* made structural rather than
/// remembered (`ops::Agreed`). The `Checked` lives in the task that is awaiting `ops::perform`'s
/// `ask` callback; this is the state the screen draws while that task waits. Phase 12 wires the
/// two, and it passes [`Dialog::typed`] to `ops::Checked::typed`, which is what actually decides.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dialog {
    /// **The operation, in `ops::Operation::verb`'s own spelling** — `scale`, `restart`,
    /// `delete`. The title bar capitalises it and a typed-name dialog's button prints it as it
    /// is: `┌ Scale payments/web ─` over `[ delete ]`.
    ///
    /// **Not a title built by the caller.** A pre-joined title would put rule 1's spelling —
    /// which zone carries the namespace, and that a node has none — one layer *above* the file
    /// that draws it, where the screen and the driver each keep their own copy of it
    /// (NOTES § D254).
    pub verb: &'static str,
    /// **Which object, and the `uid` that says it is still that one** — the title bar, the name a
    /// delete asks to have typed back, and the reason a stale selection can never be confirmed
    /// blindly (`screens/dialogs.md` rule 1, NOTES § D22).
    pub object: Object,
    /// What is about to happen, in plain language — *"This starts 1 more copy of your app. Right
    /// now: 2 copies. After: 3 copies."* **One string, wrapped by the renderer into the box's two
    /// lines**, never two fields: `ops::Mutation::consequence` is one string and a `\n` put here
    /// would not survive `k8s::text` on the way into the record (`screens/dialogs.md`
    /// § *Printed instead of drawn*).
    pub consequence: String,
    /// **The one extra sentence a check can add to the consequence** — today only
    /// *"This deployment is paused, so nothing will be replaced…"*, built by the driver's
    /// `while_paused` off the `bool` `ops::Checked::returned` carries (NOTES § D224).
    ///
    /// **A warning and not a refusal**: the button still arms and `⏎` still does it. Writing the
    /// annotation on a paused Deployment is not destructive — it takes effect the moment somebody
    /// resumes the rollout. What was wrong before this line existed was the dialog claiming the
    /// copies had already been replaced (`screens/dialogs.md` § The paused Deployment).
    ///
    /// **Its own paragraph and not appended to [`Self::consequence`]**, because it is a second
    /// sentence that starts on its own line in the box; joined with a space it would wrap into
    /// the middle of the line above.
    pub warning: Option<String>,
    /// The equivalent kubectl command, for the `$ …` line. **Display text**: k8rs never executes
    /// it and nothing is fed back from it into a process (invariant 4, the security gate's
    /// *the command log is display text*).
    pub kubectl: String,
    /// **What the cluster's check said, or `None` while it is still running.**
    ///
    /// `None` is the whole of *the button is not live yet* (`screens/dialogs.md` rule 3). It is
    /// not *there was no dry-run*: an operation that declines one still answers with a sentence
    /// saying so (`ops::Checked::verdict`, NOTES § D225 ruling 1 — `delete`), so a dialog whose
    /// operation sends no check is `Some` from the moment it opens.
    ///
    /// **`&'static str`, matching `ops::Checked::verdict`'s own return type, and the borrow is
    /// the guard** (NOTES § D246). A `String` here is nothing structurally stopping Phase 11
    /// putting an API-sourced sentence in a dialog — and a `fieldValidation=Strict` rejection
    /// hands back **4859 bytes of the object you sent** in `Status.message` (NOTES § D217), env
    /// values included. These sentences are k8rs's own, all of them, and now they cannot not be.
    pub verdict: Option<&'static str>,
    /// **The object's own name, where it has to be typed back** — `delete` and `drain`, and
    /// nothing else (invariant 2, `ops::Confirm::Type`). `None` is a dialog a deliberate yes
    /// confirms.
    ///
    /// **It is the name a second time and that is not a duplicate of [`Object::name`].** This one
    /// is `ops::Checked::asks`, stripped by `ops::Record::of` and the same value
    /// `ops::Checked::typed` will actually compare against — so [`Self::armed`] reads *this* one
    /// and the title bar draws the other. `k8s::text` can shorten a 600-byte name to
    /// `k8s::IDENTIFIER` on the way in, and a dialog that armed on the unstripped one would light
    /// a button `ops.rs` then refuses.
    pub asks: Option<String>,
    /// What has been typed into it so far. Empty and unused on a press-only dialog.
    pub typed: Input,
}

impl Dialog {
    /// **Whether the confirm button is drawn live** — the dry-run has answered, and for a
    /// typed-name dialog the name matches (`screens/widgets.md` § 5, `screens/dialogs.md` rule 3).
    ///
    /// **This decides how a button looks and it authorises nothing.** The only route to a mutation
    /// is `ops::Checked::pressed` / `ops::Checked::typed` building an `ops::Agreed`, which nothing
    /// outside `ops.rs` can construct. So the worst a bug here can do is draw a live button that
    /// `ops.rs` then refuses — never the reverse.
    ///
    /// **The empty-name guard is `ops::Checked::typed`'s and is repeated rather than skipped.**
    /// `typed == name` holds for `("", "")` — *typing the object name* satisfied by typing
    /// nothing. `ops.rs` refuses it in the one function every dialog routes through; this is the
    /// same refusal on the drawing side, so a name that `k8s::text` stripped to nothing cannot
    /// even light the button up.
    /// **The word the confirm button and the footer both use** — `do it` for a dialog a press
    /// confirms, and the operation's own verb for one that asks for a name.
    ///
    /// **One source, because they sat a frame apart saying different things**: the footer read
    /// `⏎ do it` while the button beside it read `[ delete ]`, which is two words for one action
    /// in one frame.
    pub fn confirm(&self) -> &'static str {
        match self.asks {
            Some(_) => self.verb,
            None => "do it",
        }
    }

    pub fn armed(&self) -> bool {
        self.verdict.is_some()
            && match self.asks.as_deref() {
                None => true,
                Some(name) => !name.is_empty() && self.typed.text() == name,
            }
    }
}

// --- THE MODAL LAYER END ---

// --- THE COMMAND LOG START ---

/// **How many lines are kept**, oldest dropped first.
///
/// The strip draws two of them and nothing scrolls the rest, so this is the security gate's
/// *sizes are bounded* and not a scrollback: a session left open for a week appends one line per
/// tab the reader opens and one per mutation they confirm, and a `Vec` nothing ever prunes grows
/// for as long as the process lives.
///
/// **A hundred is past any burst this can see, counted rather than guessed.** The longest thing
/// that arrives at once is the manifest, and it is **fifteen** lines — `main.rs`'s `command_log`
/// with `--analysis`: three before the report fetches, seven for the fetches themselves, five
/// watches. **Sixteen is its maximum and not its usual value**, reached only on the
/// `Coverage::Refused` / `Coverage::Blind` branch with a context naming no namespace, which is
/// what adds the second scope probe (`k8s-admin`, 2026-09-07,
/// `reports/2026-09-07-command-log-panel.md`; this doc said sixteen flat until then). Everything
/// after that is one line per keypress.
///
/// **What it costs is bounded by what a line can be**, not by this number alone: a name reaching
/// one came through [`crate::k8s::IDENTIFIER`], which cuts at 512 bytes, and an outcome through
/// [`SAID`], so the worst case is a hundred lines of about a kilobyte and every real one is a few
/// kB in total. **That ceiling is only true while every string entering this type carries a
/// bound** — [`Log::outcome`] took an unbounded one until 2026-09-07, which is the hole [`SAID`]
/// closes.
const KEPT: usize = 100;

/// **The gap between a command and what became of it** — three columns, ruled in
/// `screens/widgets.md` § 2 (*the gap before an outcome word is three columns*) and drawn by
/// `screens/states.md` § Your login expired, `screens/context.md`, `screens/detail.md` § The
/// events fetch could not be completed and `screens/dialogs.md` § The object went away.
///
/// **This said *as every mockup that draws one uses* until 2026-09-07 and that was not measured**
/// — six mockups drew an outcome and two of them used two columns (`k8s-admin`, 2026-09-07). One
/// was a mistake and was corrected; the other is `screens/dialogs.md` § The cluster said no,
/// which that section now names as **the** exception, at two, because its command and its verdict
/// already fill the pane it is drawn in. A screen file's acknowledged exception is not a second
/// constant here: the rule is three.
const OUTCOME_GAP: &str = "   ";

/// **D20's `…`** — a call that takes time is a state, and this is the whole of what the line says
/// while it is in that state. [`Log::outcome`] replaces it and never removes it
/// (`screens/dialogs.md` § While the call is running).
const RUNNING: &str = "…";

/// **The longest outcome word kept** — 32 bytes, and this is a security-gate bound rather than a
/// layout one.
///
/// [`Log::outcome`] takes a **short form** — `rejected`, `not sent`, `refused`, `login expired` —
/// and the caller it is written for is `ops::Performed::plainly`, which interpolates
/// `outcome.said()`: **the server's own words**. NOTES § D217 measured a `fieldValidation=Strict`
/// rejection handing back the whole object that was sent, 4859 bytes on a trivial Deployment, and
/// the gate's *a Secret value never enters the command log* row is exactly what an unbounded
/// `said` walks through.
///
/// **Thirteen is the longest any mockup draws** — `login expired`, `screens/states.md`. Double it
/// and round: 32 is past every word `screens/` spells and nothing a cluster's sentence survives.
/// **The bound is not the guard on its own** — a word cut to 32 bytes is still the cluster's
/// words, which is why [`Log::outcome`]'s doc says what the argument is and why the short form is
/// the dialog's to build (NOTES § D233).
///
/// **It is spent through [`crate::k8s::text`] and not a loop written here**, which is the same
/// reuse [`Input`] makes of [`crate::k8s::unprintable`] and for the same reason: that function
/// already decides which characters go, that a whitespace one leaves a boundary behind rather
/// than gluing two words together, that the cut lands on a character boundary, and that a cut
/// says so in plain language attributed to k8rs (NOTES § D146, `screens/widgets.md` § 7). A
/// second opinion about any of those is the second copy that goes stale.
const SAID: usize = 32;

/// **What k8rs ran, oldest first** — the strip along the bottom of every screen, and invariant
/// 4's teaching device (`screens/alerts.md` § The height).
///
/// **Three kinds of line, and each is honest about something different** (NOTES § D233,
/// § D257):
///
/// * the **manifest** — the streams this run opened, computed up front by `main.rs`'s
///   `command_log` from what the run is *going* to do (NOTES § D233 ruling 3), and handed over
///   line by line;
/// * a **user-initiated read** — the fetch a detail tab sent, spelled by [`describe_line`],
///   [`events_line`], [`yaml_line`] and [`crate::k8s::LogRequest::kubectl`], appended by the key
///   handler that asked for it. Not instrumentation: this file chose that read and holds its
///   object, its namespace and its container (NOTES § D257);
/// * a **mutation** — appended the instant `ops::ask` answers `ops::Answer::Confirmed`, and
///   never when the dialog opens. `Cancelled`, `Gone` and `Changed` append nothing. The
///   dialog's `$ …` line and this one carry the same text and are **not the same line**: the
///   dialog prints its own before anyone has agreed to anything, and feeding that into here
///   publishes a command that was never run — invariant 4's *neither record may lie*, reached by
///   reusing a string rather than by writing a wrong one (NOTES § D233 ruling 1).
///
/// **Which method a line goes to is decided by whether an outcome is still coming, and never by
/// which of the three it is.** [`Log::ran`] is a line nothing more will be said about;
/// [`Log::sent`] is a line [`Log::outcome`] will finish. **All three kinds reach `sent`** — a
/// mutation on the wire (`screens/dialogs.md` § While the call is running), a **read** that was
/// refused (`screens/detail.md` § The events fetch could not be completed), a **manifest** watch
/// whose token ran out (`screens/states.md` § Your login expired). Splitting on the kind instead —
/// which is what this type did until 2026-09-07, under a method called `started` documented as
/// mutation-only — is a panel three approved mockups cannot be drawn from.
///
/// **One line at a time, said rather than left implied by an `Option`.** `waiting` is a single
/// index because no screen draws two outcomes at once, and because the honest answer to *what
/// became of it* is per call. A single `401` that kills five watches therefore resolves **one**
/// line here and says the rest in the pane above it, which is where `screens/states.md` § Your
/// login expired puts that sentence anyway. Widening it is a screen ruling first, not a `Vec`.
///
/// **What is forbidden is a line implying k8rs saw a call it never saw.** `k8s.rs`'s internals —
/// a watch reconnecting, a report's five fetches, a ReplicaSet resolved behind an owner chain —
/// are invisible from up here and stay inside the manifest, which is why the read side is a
/// manifest rather than a feed (NOTES § D233 ruling 3).
///
/// **Display text.** k8rs never executes a line of this and nothing in it is fed back into a
/// process (the security gate). **Three of the four strings that reach this type were stripped
/// before they got here**, exactly as this file's module doc promises: the manifest arrives
/// through `main.rs`'s own `sanitize`, a read line's names came through `k8s::text` at ingest,
/// and a mutation's line was stripped once by `ops::Record::of`. **The fourth is
/// [`Log::outcome`]'s word, and it is stripped and bounded here**, because the caller that method
/// is written for hands over whatever the cluster said — the one string on this panel that never
/// passed an ingest (NOTES § D217, [`SAID`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    lines: Vec<String>,
    /// **Which line is still waiting for its outcome**, or `None` when nothing is running.
    ///
    /// **An index, and not *the last line*.** Navigation stays free while a call is on the wire
    /// (`screens/dialogs.md` § While the call is running), so a reader who opens a detail tab
    /// meanwhile appends a read line **after** the one still waiting. Rewriting whatever happens
    /// to be last would put a scale's outcome onto a `kubectl logs`, which is the same record
    /// lying this whole type is shaped to prevent.
    ///
    /// **Set by [`Log::sent`] and by nothing else, whichever of the three kinds that line is.**
    waiting: Option<usize>,
}

impl Log {
    /// **The lines, oldest first** — what `ui::Screen::log` borrows and the strip draws the last
    /// two of.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// **A line nothing more will be said about** — a manifest line, and a read that answered.
    ///
    /// **Not *a call that is over by the time it is written down*, which is what this said until
    /// 2026-09-07 and was never true of the manifest**: `command_log` computes those lines
    /// *before* the calls, from what the run is going to do (NOTES § D233 ruling 3). The only
    /// thing true of every line here is that no [`Log::outcome`] is coming for it. A line that
    /// expects one goes to [`Log::sent`], whichever of the three kinds it is.
    ///
    /// **The whole display line, `$ ` and all**, because that is what every builder feeding this
    /// already produces: `main.rs`'s `command_log`, [`crate::k8s::LogRequest::kubectl`], and the
    /// three below.
    pub fn ran(&mut self, line: String) {
        self.push(line);
    }

    /// **A call whose outcome has not arrived** — appended with [`RUNNING`], which
    /// [`Log::outcome`] replaces when the answer comes (NOTES § D20).
    ///
    /// **Any of the three kinds, and that is the whole reason this is not called `started`.** A
    /// mutation on the wire is the case D20 was written for, and it is not the only one: a read
    /// the reader opened can be refused (`screens/detail.md` § The events fetch could not be
    /// completed) and a manifest watch can outlive its token (`screens/states.md` § Your login
    /// expired). Both mockups draw an outcome on a line that is not a mutation, and a method
    /// reserved for mutations is what made them undrawable (`k8s-admin`, 2026-09-07).
    ///
    /// **The caller writes the `$ `**, because `ops::Shown::kubectl` carries none: the same
    /// `format!("$ {}", shown.kubectl)` the headless dialog already prints, so the panel and the
    /// dialog cannot come to spell one command two ways (invariant 4, NOTES § D8).
    ///
    /// **A second `sent` before the first has an outcome leaves the first `…` standing**, and
    /// that is the honest reading of it: k8rs never learned. A `…` that stays claims less than
    /// the line under it, never more.
    pub fn sent(&mut self, line: String) {
        self.push(format!("{line}{OUTCOME_GAP}{RUNNING}"));
        self.waiting = Some(self.lines.len() - 1);
    }

    /// **What became of the call that is running, onto the line it belongs to** — `→ rejected`,
    /// `→ not sent`, `→ login expired` (`screens/dialogs.md`, `screens/states.md` § Your login
    /// expired). The `…` is replaced, never removed.
    ///
    /// **The word is the caller's, and that is a deferral rather than a design.** `screens/` has
    /// spelled a short form for some of what `ops::Outcome` can say and not for all of it, so a
    /// table from one to the other written here would be wording no screen file has agreed to —
    /// and the code matches the screen file, or the screen file changes first. It belongs with the
    /// dialog, which is a later box in this phase and still inside this file's freeze.
    ///
    /// **It is bounded and stripped here all the same** ([`SAID`]). The argument is the *word* —
    /// `rejected`, `refused`, `login expired` — never the cluster's sentence: the obvious caller
    /// is `ops::Performed::plainly`, which carries the server's own words, and NOTES § D217
    /// measured one of those returning the whole object that was submitted. The bound is what
    /// keeps that out of the panel when a caller forgets; it is not permission to pass a
    /// sentence, which is why the paragraph above still stands.
    ///
    /// **Nothing running means nothing written.** An outcome on a line no call is attached to is
    /// the record lying, so this returns having done nothing rather than editing the last line it
    /// can find.
    pub fn outcome(&mut self, said: &str) {
        // **The ingest strip, paid here** — this panel's other three strings each paid it at
        // their own ingest and this one never had one ([`SAID`]).
        let mut said = said.to_owned();
        text(&mut said, SAID);
        // **A word that is empty after that is not an outcome**, and `→ ` with nothing behind it
        // says strictly less than the `…` it would replace. The line keeps its mark and stays
        // resolvable, which is the same reading a second `sent` gets.
        if said.is_empty() {
            return;
        }
        let Some(at) = self.waiting.take() else {
            return;
        };
        // **Only the `…` goes**; [`OUTCOME_GAP`] is already in place and is spelled once.
        //
        // **`strip_suffix` and not a byte count.** Only [`Log::sent`] sets `waiting` and it
        // always appends [`RUNNING`], so the mark is there — but a `truncate` that was ever wrong
        // about that would cut mid-character and panic a terminal in the middle of a frame.
        //
        // **And the index is read off `waiting`, never fallen back to the last line.** That
        // equivalence rests on *no caller ever passing a line that already ends in `…`*, which is
        // an assumption about the caller set and not a property of this type — [`Log::ran`] is
        // `pub` and takes an arbitrary `String`. `tester` found the sequence that separates them
        // on 2026-09-07; `an_outcome_with_nothing_running_writes_nothing` holds it.
        let replaced = self.lines[at]
            .strip_suffix(RUNNING)
            .map(|command| format!("{command}→ {said}"));
        if let Some(replaced) = replaced {
            self.lines[at] = replaced;
        }
    }

    /// Append, and drop from the front once past [`KEPT`].
    fn push(&mut self, line: String) {
        self.lines.push(line);
        let over = self.lines.len().saturating_sub(KEPT);
        if over > 0 {
            self.lines.drain(..over);
            // **The waiting line moves with them, or stops existing.** `checked_sub` is the second
            // case: a burst longer than the whole window evicted the line that was running, and an
            // outcome then has nowhere true to go.
            self.waiting = self.waiting.and_then(|at| at.checked_sub(over));
        }
    }
}

/// `-n payments`, or nothing at all where there is no namespace to name.
///
/// **An empty string is *no namespace*, and the `Option` alone never said so.** This doc claimed
/// `ObjectId::namespace`'s `Option` made the function safe because `-n ""` is a command that does
/// not work — but an `Option` stops a `None`, not a `Some("")`, and `k8s::maybe` does not
/// collapse an emptied string back to `None`, so a namespace that was entirely control characters
/// survives ingest as `Some("")`. What that built was **worse** than the `-n ""` the doc argued
/// about: a bare `-n ` swallows the next token, so `$ kubectl get pod web -n  -o yaml …` reads
/// `-o` as the namespace (`k8s-admin`, 2026-09-07). Unreachable through today's callers; the
/// record invariant 4 says may not lie does not get to depend on that (NOTES § D36).
///
/// **`rules::in_namespace` is not called from here**, though it is the same three lines: it is
/// private, `rules.rs` is frozen, and it carries this identical defect — reusing it would mean
/// inheriting the bug being fixed. The duplication is the PM's box, not this one's.
fn in_namespace(namespace: Option<&str>) -> String {
    match namespace.unwrap_or_default() {
        "" => String::new(),
        namespace => format!(" -n {namespace}"),
    }
}

/// **`$ kubectl describe pod …`** — the describe tab's line (`screens/detail.md` § The describe
/// tab).
///
/// **Two reads and one line**, because `kubectl describe pod` is the one command a reader would
/// have typed for both halves of that pane — the object *and* its events. It is the *equivalent*
/// command and not a transcript of the two calls under it (invariant 4).
///
/// **`pod` is written here and is not the caller's, which is the opposite of [`yaml_line`].**
/// That pane is `ui::Described`, which holds a `PodSnapshot` and can draw nothing else, so the
/// word cannot be wrong while the pane keeps its shape — and `rules::describe` spells it the same
/// way for the same reason. A describe tab over another kind changes that pane's type first, and
/// this line with it.
pub fn describe_line(id: &ObjectId) -> String {
    format!(
        "$ kubectl describe pod {}{}",
        id.name,
        in_namespace(id.namespace.as_deref())
    )
}

/// **`$ kubectl events --for pod/… -n …`** — the events tab's line (`screens/detail.md` § The
/// command log, and the footer).
///
/// **Not `kubectl describe`.** Describe's pane earns that command because it shows two reads
/// folded into one; this pane shows only the second half, so the line it teaches is the command
/// that produces only that half. `kubectl events --for TYPE/NAME` is a real subcommand, stable
/// since kubectl 1.28, built for exactly this question.
///
/// **The equivalence has two named limits and neither is fixable by a longer line**, which is why
/// that screen section states them rather than leaving them for 3am: `kubectl events` sorts
/// oldest first — the reverse of the order this pane draws and the fetch returns, and it has no
/// `--sort-by` to add — and `--for` selects on kind, apiVersion and name where the fetch's
/// selector also carries the `uid`, so a replacement object under one name is a different match to
/// the pane and the same match to the typed line.
///
/// **The resource word is the caller's** — `k8s::events` already takes the kind as an argument
/// for the tab this is, and mapping an `ObjectKind` onto what `kubectl` accepts is API
/// discovery's job and not a table in this file (invariant 12, `rules::get_yaml`'s own doc).
/// **Lowercased by whoever passes it**, or the teaching device prints a form no documentation
/// shows (NOTES § D39).
///
/// **`namespace` is the one the fetch went to, and reading it off the object instead was wrong.**
/// `k8s::events` takes a namespace because a cluster-scoped object's events live in one the
/// *cluster* chose — `default` for a Node — and that file's own doc names this tab as the caller
/// with the question. Built from [`ObjectId::namespace`], a Node produced
/// `$ kubectl events --for node/node-3` with no `-n` at all: on a context scoped to `payments`
/// that answers *No resources found in payments namespace* while the pane above it is showing
/// the events (`k8s-admin`, 2026-09-07). There is exactly one right answer here because there was
/// exactly one request, and this argument is it.
pub fn events_line(resource: &str, id: &ObjectId, namespace: &str) -> String {
    format!(
        "$ kubectl events --for {resource}/{}{}",
        id.name,
        in_namespace(Some(namespace))
    )
}

/// **`$ kubectl get … -o yaml --show-managed-fields`** — the yaml tab's line
/// (`screens/detail.md` § The yaml tab).
///
/// **`--show-managed-fields`, because without it the printed line does not produce what was
/// printed** (invariant 4). `kubectl` has hidden `managedFields` from `get -o yaml` since v1.21
/// and this pane does not — 95 of a pod's 246 lines, 39% of the document (`k8s-admin`,
/// 2026-08-31). Dropping the field to match instead was refused: the pane's only claim is that it
/// is the object.
///
/// **The claim is about the *request*, not about the same bytes back** — the narrowing
/// [`crate::k8s::LogRequest::kubectl`] took on 2026-08-30 and this line needed too. `kubectl get
/// -o yaml` alphabetises where this pane keeps the API's own order, and quotes a timestamp this
/// pane leaves bare. Same object, not the same file.
///
/// **The resource word is the caller's**, [`events_line`]'s reason exactly — and this is the tab
/// that has a second kind to open on today, `screens/detail.md` § A Secret with no keys.
pub fn yaml_line(resource: &str, id: &ObjectId) -> String {
    format!(
        "$ kubectl get {resource} {}{} -o yaml --show-managed-fields",
        id.name,
        in_namespace(id.namespace.as_deref())
    )
}

// **The logs tab's line is not built here, and its absence is the ruling rather than an
// omission.** [`crate::k8s::LogRequest::kubectl`] already spells it, in the file that sends the
// request and off the same fields `LogRequest::params` is built from — so the line cannot describe
// a request that was not sent. A copy here would be the second spelling NOTES § D257 forbids while
// it is asking for the first, and it would be the one that goes stale. The key handler that opens
// the tab holds the `LogRequest` it is about to fetch with; `log.ran(request.kubectl())` is the
// whole of it.

// --- THE COMMAND LOG END ---

// --- WHICH VIEW IS OPEN START ---

/// **Which of the three the content pane is showing** (NOTES § The three views).
///
/// **The index is carried here rather than kept in a second field**, so *which view* and *which
/// kind* cannot come apart. Two places holding one fact and disagreeing is the defect class this
/// repo has paid most for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// The default. k8rs never opens on a pod list (`screens/alerts.md`).
    #[default]
    Alerts,
    /// The browser, showing one kind — by its index in the discovery list.
    ///
    /// **The index is into the discovery list of the *current connection*, and it survives
    /// nothing else** (NOTES § D246). Discovery is read once at connect (`k8s.rs`, the
    /// re-discovery comment — no box schedules a re-read), so within one connection the list
    /// cannot move under this number. A **cluster switch** is the startup path run again
    /// (NOTES § D16 ruling 4), which rebuilds discovery and therefore this whole value; resetting
    /// it there is Phase 11's wiring and is in `backlog.md`, not a second field here.
    Resources(usize),
    /// One analysis report, by its index.
    Analysis(usize),
}

/// **The four detail tabs** (`screens/detail.md`). `[` and `]` move between them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Logs,
    Describe,
    Yaml,
    Events,
}

impl Tab {
    /// In the order they are drawn, left to right.
    pub const ALL: [Tab; 4] = [Tab::Logs, Tab::Describe, Tab::Yaml, Tab::Events];

    /// `]`. **Clamps at the last tab rather than wrapping** — the row is drawn as a row, and a
    /// reader holding `]` should arrive at the right-hand end and stay there, not reappear on the
    /// left. `[` is the same, at the other end.
    pub fn next(self) -> Tab {
        Tab::ALL[(self.at() + 1).min(Tab::ALL.len() - 1)]
    }

    /// `[`.
    pub fn previous(self) -> Tab {
        Tab::ALL[self.at().saturating_sub(1)]
    }

    /// **The word the tab row draws, which is the whole of what one tab is called**
    /// (`screens/detail.md`). It is beside [`Tab::ALL`] because a label written where the row is
    /// drawn is a fifth thing to keep in step with this list.
    pub fn label(self) -> &'static str {
        match self {
            Tab::Logs => "logs",
            Tab::Describe => "describe",
            Tab::Yaml => "yaml",
            Tab::Events => "events",
        }
    }

    /// Where this tab sits in the row. **Derived from [`Tab::ALL`] rather than written as a
    /// number**, so a fifth tab moves the clamp with it instead of silently pinning `]` at the
    /// fourth.
    pub fn at(self) -> usize {
        Tab::ALL
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or_default()
    }
}

/// **Everything the screen is looking at.** `ui.rs` draws this and nothing else, which is the
/// whole of todo.md § Phase 10's goal: a renderer that is a pure function of one value cannot rot
/// into a renderer that remembers.
///
/// **The lists themselves are not in here.** Findings, tables and reports are re-derived from the
/// store every frame; what this holds is what the *user* did — where the cursor is, what is
/// filtered, what is open. A state struct that also cached the data would be a second copy of the
/// store with its own staleness.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct App {
    /// What the content pane is showing.
    pub view: View,
    /// Which group of the sidebar is open, or none.
    pub expanded: Option<Group>,
    /// Where the sidebar cursor is, over the **selectable** rows only ([`selectable`]).
    pub nav: Cursor,
    /// Where the content pane's cursor is — a card, a table row, or a report's answer.
    pub content: Cursor,
    /// `/` and `n`.
    pub filters: Filters,
    /// Which detail tab is open, when something is open.
    pub tab: Tab,
    /// **The free-text panes' own scroll offset** — logs, yaml, describe **and events**. Lists and
    /// tables do not use it: ratatui keeps a selection in view by itself, so their scrolling is
    /// [`App::content`]'s (`screens/widgets.md` § 4).
    ///
    /// **Events was not on that list until Phase 11 drew it** (`screens/detail.md` § More events
    /// than the pane): that pane is a `Paragraph` with an offset like the other three, not a list
    /// with a cursor, so it is this field and not a fifth one.
    ///
    /// **Nothing here clamps it to the end of the content, and that is the renderer's job** — the
    /// number of lines a pane has depends on the width it is drawn at, which this file has no
    /// business knowing (`ui::scrolled`).
    pub scroll: u16,
    /// **Follow mode, the log tab's `f`** — the offset is pinned to the bottom while it is on, and
    /// any manual scroll turns it off. The standard `tail -f` behaviour, and the only way a stream
    /// and a scrollbar coexist without fighting (`screens/widgets.md` § 4).
    pub following: bool,
    /// What is open over the screen, or nothing.
    pub modal: Option<Modal>,
    /// **The object a mutation is on the wire for**, or `None` when nothing is running
    /// (`ops::perform` has been given its yes and has not returned). The header's `changing…`
    /// (`theme::CHANGING`), what refuses `q` below, and the name the in-flight footer says it is
    /// waiting on.
    ///
    /// **An [`Object`] and not a `bool` beside a `String`** (PM ruling, 2026-09-12). The footer's
    /// reason clause has to name the object, and the only honest source for that name is the one
    /// the dialog was about; two fields are two facts that can disagree, which is the defect class
    /// this repo has paid most for. It is [`Modal::Confirm`]'s own [`Dialog::object`], carried
    /// across the moment the modal closes.
    pub changing: Option<Object>,
}

/// **Which of the three mutating keys this login has been told it may not use**
/// (`screens/widgets.md` § 2a, `screens/help.md` § *When a key is refused*, NOTES § D23).
///
/// **It is not a field on [`App`].** `App` is what the *user* did; this is what the *store*
/// answered, which is [`crate::ui::Screen`]'s own half — the same reason `detail` is passed into
/// [`App::footer`] rather than held here (NOTES § D259).
///
/// **A key can need more than one permission, and `s` does** — [`Self::SCALE_VERBS`] and the two
/// beside it are each operation's own count, and [`Self::of`] takes exactly that many answers for
/// it. Refused when *any* of them came back [`Verdict::No`], lit when none did.
///
/// **Only [`Verdict::No`] marks a key, and the one constructor is what makes that structural
/// rather than remembered** (NOTES § D229 ruling 4). [`Verdict::Yes`], [`Verdict::CouldNotTell`]
/// and *no answer yet* all draw the ordinary unmarked key, byte for byte. *No answer yet* is two
/// facts at once and neither may mark: a probe still in flight — including the first frame of
/// every run, before `may_i_in` has replied at all — and an operation the selected kind does not
/// support, which is never asked (`ops::scalable` reaches no DaemonSet, `ops::restartable` no bare
/// ReplicaSet, and neither reaches a Pod or a Node). A key the kind cannot act on is **withheld**,
/// which is `screens/states.md`'s own word and its own later box; it is never *refused*.
///
/// **The fields are private for that reason and not for tidiness.** A caller that could write
/// `Refused { scale: true, .. }` could dim a permitted key from a probe that never answered, which
/// is the one thing D229 ruling 4 forbids outright.
///
/// **[`Verdict::CouldNotTell`]'s sentence is dropped here and never drawn**, so this file gains no
/// third class of unstripped string over the two its module doc names.
///
/// **[`Self::resource`] is a `&'static str` for [`Object::kind`]'s own reason** (NOTES § D246):
/// every sentence `?` draws is k8rs's own, and a `String` here is nothing structurally stopping a
/// resource word the API sent from being interpolated into one. The kinds an operation can be
/// pointed at are literals in the driver.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refused {
    resource: &'static str,
    scale: bool,
    restart: bool,
    delete: bool,
}

impl Refused {
    /// **Every permission `s` needs, as the verb of each, in the order `ops.rs` performs them.**
    /// The resource half of each is [`Self::resource`] with `/scale` after it.
    ///
    /// **It is two, and this type modelled it as one until 2026-09-12.** `ops::scale` calls
    /// `get_scale` before `patch_scale` — `src/ops.rs`'s own `SCALE` region comment says it in as
    /// many words, *"`Api::get_scale` reads the count the consequence sentence is built from and
    /// `Api::patch_scale` changes it"* — so `s` needs `get` *and* `patch` on `<plural>/scale`.
    /// Measured on a cluster (`k8s-admin`, 2026-09-12) with a login granted exactly `patch` on
    /// `deployments/scale`: the probe answered yes, the key drew lit, and the operation then
    /// failed with `cannot get resource "deployments/scale"`. Both halves were wrong — a key lit
    /// that cannot be used, and a clause naming a grant that does not work.
    pub const SCALE_VERBS: [&'static str; 2] = ["get", "patch"];

    /// Every permission `r` needs. **One, measured sufficient end to end** on the same cluster: a
    /// login granted only `patch deployments` performed a restart.
    pub const RESTART_VERBS: [&'static str; 1] = ["patch"];

    /// Every permission `ctrl-d` needs. **One, measured sufficient end to end**: a login granted
    /// only `delete deployments` performed a delete.
    pub const DELETE_VERBS: [&'static str; 1] = ["delete"];

    /// **The one place a [`Verdict`] becomes a mark on a key**, which is the whole of the
    /// conversion (NOTES § D229 ruling 4).
    ///
    /// **An operation is refused when *any* permission it needs came back [`Verdict::No`], and lit
    /// when none did** — so each argument is an array of answers, one slot per permission, and its
    /// length is [`Self::SCALE_VERBS`]'s, [`Self::RESTART_VERBS`]'s or [`Self::DELETE_VERBS`]'s
    /// own. **The count is the type, not the arity**, which is the defect this replaced: `scale`'s
    /// second permission was silently assumed not to exist because the constructor took one
    /// argument for it. An operation that gains a permission is one edit to the constant above and
    /// a compile error at every caller; v0.2's cordon and drain bring their own counts the same
    /// way.
    ///
    /// `resource` is the API plural of the selected object's kind — `deployments` — for the clause
    /// `?` draws; the footer never names it (`screens/widgets.md` § 2a).
    ///
    /// **It is read only where something is refused, and a caller that refuses something owes a
    /// real one.** Nothing here can supply a missing plural and nothing here may swallow the
    /// refusal for want of one — the footer's `s no scale` is true with or without a resource
    /// word, and dropping the mark to protect the `?` clause would hide a refusal the login
    /// actually has. [`Self::default`] — nothing selected, nothing asked — is the one shape where
    /// an empty `resource` is right, and it marks nothing.
    pub fn of(
        resource: &'static str,
        scale: [Option<&Verdict>; Self::SCALE_VERBS.len()],
        restart: [Option<&Verdict>; Self::RESTART_VERBS.len()],
        delete: [Option<&Verdict>; Self::DELETE_VERBS.len()],
    ) -> Self {
        Self {
            resource,
            scale: refuses(&scale),
            restart: refuses(&restart),
            delete: refuses(&delete),
        }
    }

    /// The plural the clause `?` draws names — `get+patch deployments/scale`.
    pub fn resource(self) -> &'static str {
        self.resource
    }

    /// `s`.
    pub fn scale(self) -> bool {
        self.scale
    }

    /// `r`.
    pub fn restart(self) -> bool {
        self.restart
    }

    /// `ctrl-d`. **Never reaches a footer, refused or not** — D259 took that key off both list
    /// footers for width before this existed, so it is marked only where it is drawn, in
    /// `screens/help.md`.
    pub fn delete(self) -> bool {
        self.delete
    }
}

/// **Fail open, in one line: a probe may never be the reason a permitted action is marked**
/// (NOTES § D229 ruling 4). Everything but a `No` — a yes, a review k8rs could not get an answer
/// out of, and a question that was never asked — is the ordinary key.
///
/// **One `No` anywhere in the operation's answers refuses it**, which is the other half of the
/// same rule read across a set: an operation the login cannot complete is not lit because most of
/// what it needs was granted. An empty set — an operation with nothing asked — is lit, which is
/// `any`'s own answer and is the *not asked* case above.
fn refuses(answers: &[Option<&Verdict>]) -> bool {
    answers
        .iter()
        .any(|answer| matches!(answer, Some(Verdict::No)))
}

impl App {
    /// **`q` — refused while a write is in flight**, and only then (NOTES § D12,
    /// `screens/dialogs.md` § *While the call is running*). Quitting mid-`PATCH` would leave the
    /// audit log holding an attempt with no result.
    pub fn may_quit(&self) -> bool {
        self.changing.is_none()
    }

    /// **`X` — unbound while a modal is open, and while a write is in flight** (NOTES § D12,
    /// § D16). Switching clusters under an open confirmation is how a dialog ends up naming an
    /// object on a cluster it was never read from.
    pub fn may_switch_cluster(&self) -> bool {
        self.modal.is_none() && self.changing.is_none()
    }

    /// **A second mutation is refused while one is running** (`screens/dialogs.md` § *While the
    /// call is running*). Navigation stays free; this is only about opening another dialog.
    pub fn may_mutate(&self) -> bool {
        self.modal.is_none() && self.changing.is_none()
    }

    /// **The footer — the keys valid right now, and the right-aligned zone beside them**
    /// (`screens/widgets.md` § 2a). The left string is what the reader reads; **`?` is the only
    /// mode that can fill the right one, and even it does not always** — over a call in flight the
    /// `q quit` it normally keeps is refused, and the zone is empty there too (below).
    ///
    /// **One function, so no call site can spell a footer of its own.** `ui.rs` draws what this
    /// returns and `main.rs` never touches it, which is why [`crate::ui::Screen`] carries no
    /// `keys` field: a footer that has one home cannot be got wrong in a second one.
    ///
    /// **A footer is a curated subset and never an exhaustive one** — `? all keys` is what
    /// completes it (`screens/README.md` rule 2). `l logs` and `ctrl-d delete` are bound on both
    /// lists and named by neither footer; `⇧p previous` and `/ search` are bound in the logs tab
    /// and its footer names neither. **The pair `? all keys  q quit` is drawn last and never gives
    /// way — with the one named exception `screens/widgets.md` § 2a now carries in the rule
    /// itself**: while a call is running, every footer it reaches loses `q quit` at least, and
    /// Alerts and Resources lose the whole line, pair included. Both halves of that exception are
    /// below — the `strip_suffix` and the guard arm — and this sentence used to state only the
    /// half that was still true, which nothing mechanical could see (`k8s-admin`, 2026-09-12;
    /// NOTES § D216 — `rustfmt` and the tests cannot read a comment).
    /// The key set itself is NOTES § D12's and nothing here adds to it.
    ///
    /// **`detail` is passed in because [`App`] cannot know it**: *whether* a detail tab is open
    /// is `ui::Screen::detail`'s fact and *which* tab it would be is [`App::tab`]'s, so each
    /// arrives from its one home and the two cannot come apart the way a second `open: Tab`
    /// field here would let them.
    ///
    /// **A modal answers for its own footer and the mode underneath is not asked**
    /// (`screens/dialogs.md`, which draws one under each box). These are the *closed local sets*
    /// `screens/widgets.md` § 2a names: nothing global is offered beside them, because under an
    /// open confirmation there is nothing global to offer.
    ///
    /// **A confirmation says which keys are live *now*, and the same rule applies to both arms.**
    /// A footer is § 2a's *the keys valid right now*, so it may not name `⏎` while the button is
    /// dim: a typed-name dialog reads `type the name to enable` until [`Dialog::armed`], and a
    /// press-only dialog reads `waiting for the cluster` until its dry-run answers. **Only the
    /// typed-name arm had that shape**, and the press-only arm named a key that did nothing for
    /// as long as a real round trip takes (NOTES § D214 — `esc` is inert then too, which is why
    /// neither key is offered).
    ///
    /// **The confirm word is [`Dialog::confirm`]'s**, so the footer and the button beside it
    /// cannot spell one action two ways.
    ///
    /// **The separator is two spaces, as every other footer in the product spells it**
    /// (`screens/widgets.md` § 2a, the file that owns the footer). `screens/dialogs.md` draws
    /// three under its boxes; one product cannot have two answers for the gap between two keys.
    ///
    /// **`refused` arrives the same way `detail` does and for the same reason** — it is what the
    /// *store* answered, not what the user did (NOTES § D259, [`Refused`]). It reaches exactly one
    /// arm: a marked key stays on the line and gains the word `no` before its label, which is one
    /// of the four fixed strings `screens/widgets.md` § 2a counts, never a string built here.
    /// `ctrl-d` is not on this line to mark, and Analysis and the detail tabs name neither `s` nor
    /// `r`, so a refusal cannot show on them.
    ///
    /// **`changing` is the one string in any footer this product did not choose the length of** —
    /// the selected object's own name, already spelled `payments/web` and already cut to the room
    /// this line has left (`screens/dialogs.md` § *While the call is running*,
    /// `screens/widgets.md` § 7's fourth deliberate truncation). Both of those are `ui::name` and
    /// `ui::name_cut`'s, because measuring columns needs `Span::width` and this file may name no
    /// `ratatui` type (NOTES § D241); a `chars().count()` here would be a second measurement of
    /// the same thing, disagreeing on the first wide character (PM ruling, 2026-09-12).
    ///
    /// **The arm is chosen by [`App::changing`] and never by the argument.** A caller that hands
    /// over an empty name cannot turn the in-flight footer off — which is what lets `ui::footer`
    /// ask for this same line with `""` to measure its own fixed parts, rather than restate their
    /// width as a number that goes stale the first time a word here changes.
    ///
    /// **That replacement reaches Alerts and Resources and no other mode**
    /// (`screens/dialogs.md` § *Detail tabs and Analysis keep their own footer, not this line*).
    /// Those two are the only ordinary footers naming `s` and `r`, which a call in flight makes
    /// inactionable and which neither `no` nor silence can honestly say — so the whole line goes.
    /// **Every other mode keeps its own footer and loses exactly one word, `q quit`**: nothing
    /// else on a detail tab's or Analysis's line is made false by a call on the wire, and the
    /// first draft of this box replaced them too — drawing `⏎ open` over a logs pane with nothing
    /// to select, dropping the `esc back` that is the only way out of the tab, and leaving
    /// `[ ] tabs`, `f follow` and `c container` bound and unnamed (`k8s-admin`, 2026-09-12).
    ///
    /// **One word is structural here, not a promise.** The drop is a [`str::strip_suffix`] of
    /// that word and its own two-space separator off a `&'static str`, so a mode cannot lose a
    /// second one however the literals above are edited; what guarantees the word is there to
    /// strip is `views_tests::the_anchor_pair_ends_every_ordinary_footer`, which says so for
    /// every mode.
    pub fn footer(
        &self,
        detail: bool,
        refused: Refused,
        changing: &str,
    ) -> (Cow<'static, str>, &'static str) {
        // **`Help` is the one modal that keeps `q quit` — except over a call in flight, when the
        // key it names is refused** (`screens/help.md` § *While the call is running*,
        // `screens/widgets.md` § 2a). It is dropped and not marked `q no quit`: the `no` this
        // product spells beside a key is a permission this login lacks, and a call finishing is a
        // wait. The moment it returns, `q` is back.
        //
        // **Every other modal answers here unchanged, and the combination is unreachable rather
        // than merely undrawn.** `Confirm` closes when the yes is given, which is *before*
        // `changing` is set; `Refused` and `Gone` open from what the call answered, which is
        // *after* it is cleared. Phase 12's wiring is what makes that true — so these arms are
        // stated to answer first on purpose: a modal drawing its own closed set over a call it
        // cannot have coexisted with is the one honest thing to draw if the wiring ever slips,
        // and it is `screens/widgets.md` § 2a's rule for a modal either way.
        match &self.modal {
            Some(Modal::Help) => {
                let quit = if self.changing.is_some() {
                    ""
                } else {
                    "q quit"
                };
                return (Cow::Borrowed("? or esc to close"), quit);
            }
            Some(Modal::Confirm(dialog)) => {
                let keys = match (dialog.armed(), dialog.asks.is_some()) {
                    (true, _) => Cow::Owned(format!("⏎ {}  esc cancel", dialog.confirm())),
                    (false, true) => Cow::Borrowed("type the name to enable  esc cancel"),
                    // **No verdict yet, so neither key is live** — the dry-run is a real round
                    // trip for a scale and a restart, and `esc` is inert until it answers
                    // (NOTES § D214). `delete` never sits here: it sends no check, so its verdict
                    // is `Some` from the first frame (NOTES § D225 ruling 1).
                    (false, false) => Cow::Borrowed("waiting for the cluster"),
                };
                return (keys, "");
            }
            // **Two dismiss-only screens, and only one of them offers a second key.** `Refused`
            // reports on a write that was already sent, so the object it was about is still
            // selected underneath and `⏎` still opens it; `Gone` is the one state where it is
            // not, because the object stopped existing (`screens/dialogs.md` § The object went
            // away).
            Some(Modal::Refused { .. }) => return (Cow::Borrowed("esc dismiss  ⏎ open"), ""),
            Some(Modal::Gone { .. }) => return (Cow::Borrowed("esc dismiss"), ""),
            None => {}
        }
        // **Exhaustive on both enums on purpose**: a fifth tab or a fourth view is a compile
        // error here rather than a screen that quietly draws the wrong keys.
        let keys = match (detail, self.tab, self.view) {
            (true, Tab::Logs, _) => "[ ] tabs  f follow  c container  esc back  ? all keys  q quit",
            (true, Tab::Describe | Tab::Yaml | Tab::Events, _) => {
                "[ ] tabs  esc back  ? all keys  q quit"
            }
            (false, _, View::Analysis(_)) => "↑↓ move  ⏎ open  esc back  ? all keys  q quit",
            // **One line replaces the whole footer, and only on the two modes that name `s` and
            // `r`** — `screens/dialogs.md` § *While the call is running* and its § *Detail tabs
            // and Analysis keep their own footer, not this line*, NOTES § D20. The guard sits
            // inside this match and not above it so that *which mode is which* is decided in one
            // place; the unguarded arm below keeps the match exhaustive.
            //
            // **`?` reads `? keys` and `q quit` is gone outright**, which is the room the reason
            // clause is spending: `q` is not merely unshown here, it is refused — quitting
            // mid-`PATCH` would leave the audit log holding an attempt with no result. `? keys`
            // rather than the bare `?` this line drew first, because `⏎ open  ?  ·` reads at a
            // glance as `⏎ open?` and every other key on every footer carries a label. `↑↓ move`
            // and `⏎ open` stay, because *navigation stays free* is the one thing this state
            // promises and dropping them to buy the name more room would hide it.
            (false, _, View::Alerts | View::Resources(_)) if self.changing.is_some() => {
                return (
                    Cow::Owned(format!(
                        "↑↓ move  ⏎ open  ? keys  ·  changing {changing} first"
                    )),
                    "",
                );
            }
            // **The four rows of `screens/widgets.md` § 2a's own table, as four literals**
            // (NOTES § D259 ruling 4): the one footer a refusal can reach still spells every state
            // of itself at compile time.
            (false, _, View::Alerts | View::Resources(_)) => {
                match (refused.scale(), refused.restart()) {
                    (false, false) => {
                        "↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit"
                    }
                    (true, false) => {
                        "↑↓ move  ⏎ open  s no scale  r restart  / filter  ? all keys  q quit"
                    }
                    (false, true) => {
                        "↑↓ move  ⏎ open  s scale  r no restart  / filter  ? all keys  q quit"
                    }
                    (true, true) => {
                        "↑↓ move  ⏎ open  s no scale  r no restart  / filter  ? all keys  q quit"
                    }
                }
            }
        };
        // **The three modes that keep their own footer lose one word and no more**
        // (`screens/dialogs.md` § *Detail tabs and Analysis keep their own footer, not this
        // line*). `q quit` is dropped, not marked `q no quit`: the `no` this product spells
        // beside a key is a permission this login lacks, and a call finishing is a wait — the
        // same one-word drop Help's own footer already makes in this state, and the header's
        // `changing…` plus `? all keys` are where the reason lives.
        let keys = match self.changing {
            Some(_) => keys.strip_suffix("  q quit").unwrap_or(keys),
            None => keys,
        };
        (Cow::Borrowed(keys), "")
    }

    /// **`esc` — closes exactly one level, always** (`screens/widgets.md` § 5, and those are its
    /// words). A modal never traps the user; with no modal open it backs out of a filter.
    ///
    /// **The two filters are two levels, so one press clears one of them** (NOTES § D246). **Text
    /// first because it is the inner one** — `/` narrows whatever `n` already scoped, so backing
    /// out goes narrow to wide, which is what *one level* means for two filters that nest. Scoped
    /// to `n payments` and narrowed with `/web`, one `esc` used to jump all the way back to every
    /// namespace in the cluster: one press undoing two.
    pub fn escape(&mut self) {
        if self.modal.take().is_some() {
            return;
        }
        if self.filters.text.is_empty() {
            self.filters.namespace.clear();
        } else {
            self.filters.text.clear();
        }
    }

    /// **`⏎` on the sidebar** — a group opens or closes, anything else becomes the view.
    ///
    /// **The content cursor is reset only when the view actually changed**, which is not the same
    /// as *a key was pressed*. Expanding a sidebar group leaves the reader's place in the table
    /// they were reading — the group is a disclosure triangle, not a destination — and re-opening
    /// the kind already open leaves it too. Resetting on every press was this method's first
    /// draft and it threw away the cursor of the pane it had not touched.
    ///
    /// A [`NavItem::Header`] cannot be passed here by a cursor that only ever walks
    /// [`selectable`]'s answer; it is matched anyway, doing nothing, because *unreachable* and
    /// *ignored* draw the same screen and only one of them needs a `panic!`.
    pub fn open(&mut self, item: NavItem) {
        let before = self.view;
        match item {
            NavItem::Alerts => self.view = View::Alerts,
            NavItem::Group(group) => {
                self.expanded = (self.expanded != Some(group)).then_some(group);
            }
            NavItem::Kind(at) => self.view = View::Resources(at),
            NavItem::Report(at) => self.view = View::Analysis(at),
            NavItem::Header(_) => {}
        }
        if self.view != before {
            self.content = Cursor::default();
        }
    }

    /// **A manual scroll, which turns follow mode off** — the two halves are one action and are
    /// written once so no key handler can do the first and forget the second
    /// (`screens/widgets.md` § 4).
    pub fn scroll_by(&mut self, lines: i16) {
        self.following = false;
        self.scroll = self.scroll.saturating_add_signed(lines);
    }
}

// --- WHICH VIEW IS OPEN END ---

// --- THE WORDING A DETAIL TAB DRAWS START ---
//
// **Every string in this region used to be built in `main.rs`, and it could not stay there**
// (NOTES § D254). The temporary driver's `--describe` and Phase 11's drawn tabs are two consumers
// of one wording, which is the argument `k8s::Happening::plainly` and `k8s::LogLines::dropped_line`
// are already in `k8s.rs` for — and that file is frozen, so the shared half lands here instead.
// **This file freezes at the close of Phase 11**, so it is here now or it is two permanent copies.
//
// **Wording only: nothing here lays anything out.** The padding, the indents and the wrapping are
// the surface's — `main.rs` pads with its own `column`/`widest`, `ui.rs` with its own
// `// --- MEASURING AND CUTTING ---`. What is shared is the sentence, because a second sentence
// for one fact is the defect this repo pays most for.
//
// **Nothing here strips**, exactly as this file's own module doc promises: every field read below
// came through `k8s::text` at ingest — `Bounded for Happening` strips `reason` and `message`,
// `Bounded for PodSnapshot` strips `phase` and `reason`, and a container name came through
// `k8s::IDENTIFIER` in `PodRead::of`. A second strip here would be a second opinion about what the
// first one did.

/// **The plain-language phrase for a state word, or `None` for one no table names.**
///
/// **A short list per surface and a fall-through, never a guess.** A reason with no phrase prints
/// as its own raw word beside the controller's message ([`raw_and_message`]) — which is strictly
/// more informative than an invented sentence and cannot be false, the discipline NOTES § D198
/// generalised from `BackOff` to everything.
pub fn phrase(
    table: &'static [(&'static str, &'static str)],
    reason: &str,
) -> Option<&'static str> {
    table
        .iter()
        .find(|(word, _)| *word == reason)
        .map(|(_, said)| *said)
}

/// **`(Evicted) The node was low on resource: ephemeral-storage.`** — the raw API word and the
/// controller's verbatim message, which is the second line of every *word that explains a state*
/// block on this product: a pod's own reason, a container's, and an event's
/// (`screens/detail.md` — *three separate inventions here would be three things to keep agreeing*).
///
/// **The message is never replaced and never summarised** (NOTES § D37, § D198). A missing one
/// costs the space and nothing else: `(Evicted)` alone is what a pod prints today, because
/// `status.message` is not a field `rules.rs` carries and that file is frozen.
pub fn raw_and_message(reason: &str, message: Option<&str>) -> String {
    let said = message.unwrap_or_default();
    // **An empty reason draws no empty brackets.** The API allows an Event with no `reason`, and
    // `()` in front of a message is a word this file invented out of a field that was not there.
    match reason {
        "" => said.to_owned(),
        word => format!("({word}) {said}").trim_end().to_owned(),
    }
}

/// **The only `status.reason` this build translates** (`screens/detail.md` § The pod's own reason).
///
/// **One entry, because one is what has been measured.** Anything else the field can hold falls
/// through to its raw word beside the message, which is the safe fallback every table here uses.
pub const POD_REASONS: &[(&str, &str)] = &[("Evicted", "removed by the node to take back room")];

/// **The only terminated reason this build translates** — invariant 14's own worked example
/// (`CLAUDE.md`: `OOMKilled` reads *container exceeded its memory limit*, not the raw word).
///
/// **Everything else falls through to the exit code alone, never a guessed word**
/// (`screens/detail.md`). `Error`, `ContainerCannotRun` and the empty string a real container can
/// carry — `k8s-admin` measured `reason=Error, exit=1` and a bare `exit=255` with nothing in
/// `reason` on one pod — say no more than the number already does.
pub const STOPPED_REASONS: &[(&str, &str)] =
    &[("OOMKilled", "container exceeded its memory limit")];

/// **The waiting reasons this build translates**, each phrase derived from the card `rules.rs`
/// already draws for the same state rather than invented beside it — rule 1's *keeps crashing*,
/// rule 3's *image is not usable, so the container never started*, rule 4's *needs a ConfigMap or
/// Secret that does not exist*.
///
/// **The other five of rule 3's seven image reasons are not here**, and that is a limit rather
/// than a decision: `UNUSABLE_IMAGE` is private to the frozen `rules.rs`, so the two
/// `screens/detail.md` names by name are the two spelled here and `InvalidImageName` and its
/// siblings fall through to their own raw word — which is honest and is what the fall-through is
/// for.
pub const WAITING_REASONS: &[(&str, &str)] = &[
    ("CrashLoopBackOff", "keeps crashing and restarting"),
    ("ImagePullBackOff", "cannot get its image"),
    ("ErrImagePull", "cannot get its image"),
    (
        "CreateContainerConfigError",
        "needs a ConfigMap or Secret that does not exist",
    ),
];

/// **What one container's row says** — the word after its name, and the line under it where there
/// is one (`screens/detail.md` § The describe tab).
///
/// **One `match` and not two.** The picker's own wording was a second `match` over the same value
/// and the two disagreed on the arm that matters most: measured on `default/broken-neverback`,
/// three containers that exited `1`, `0` and `255` printed *(done)* in the picker and `failed`
/// in describe (`k8s-admin`, Phase 6 close). Of the two spellings it was the calm one that was
/// wrong, and wrong in the direction that sends a reader to the log of a container that is fine.
///
/// **`done` is not renamed to `failed` before it earns the word.** A clean `exit 0` is the healthy
/// case and stays `done`.
///
/// **A momentary `ContainerCreating` stays the calm `not started`** rather than being dressed up
/// as a problem — it is the ordinary first second of every pod.
///
/// **A waiting reason no table names falls through to its raw word *and the kubelet's message
/// under it*, which is the events table's rule and not a second one** (`screens/detail.md`
/// § The describe tab: *"the same safe-fallback rule the events table below uses, stated once and
/// reused rather than invented twice"*). The raw word appears exactly once on the row either way —
/// there, where an event's row spells it `(Reason) message`, this one already carries it in the
/// state column, so [`raw_and_message`] would print it twice and would put a bare `(Reason)` and
/// the restart count on a line of their own when there is no message. Shipped throwing the
/// message away, measured on `InvalidImageName` (`k8s-admin`, 2026-09-07).
pub fn container_state(state: Option<&ContainerState>) -> (String, Option<String>) {
    match state {
        Some(ContainerState::Running { .. }) => ("running".to_owned(), None),
        Some(ContainerState::Terminated(stopped)) if stopped.exit_code == 0 => {
            ("done".to_owned(), None)
        }
        Some(ContainerState::Terminated(stopped)) => {
            let said = stopped
                .reason
                .as_deref()
                .and_then(|reason| phrase(STOPPED_REASONS, reason))
                .map_or(String::new(), |phrase| format!("{phrase} — "));
            (
                "failed".to_owned(),
                Some(format!("{said}exit {}", stopped.exit_code)),
            )
        }
        Some(ContainerState::Waiting { reason, message }) => match reason.as_deref() {
            // The kubelet has taken the pod and is making the sandbox: nothing is wrong yet.
            Some("ContainerCreating" | "PodInitializing") => ("not started".to_owned(), None),
            Some(word) => match phrase(WAITING_REASONS, word) {
                Some(said) => (said.to_owned(), None),
                // **The fall-through keeps the kubelet's own sentence**, which is the half that
                // was being thrown away: `InvalidImageName` alone is a dead end, and the sentence
                // under it is the one that names the image that will not parse.
                None => (
                    word.to_owned(),
                    message
                        .as_deref()
                        .filter(|said| !said.is_empty())
                        .map(str::to_owned),
                ),
            },
            None => ("waiting".to_owned(), None),
        },
        // **A container the pod declares and the kubelet has not reported on** — a `Pending` pod.
        None => ("not started".to_owned(), None),
    }
}

/// **`, 3 restarts`, or nothing at all** — the one spelling of a fact two surfaces draw
/// (`screens/detail.md` says outright they are one rule).
///
/// **`restartCount` is an `i32` the API server never sets below zero**; a negative one is not a
/// count and is drawn as none rather than as its absolute value. **A container the kubelet has not
/// reported on has no count at all**, which is not a zero it chose.
pub fn restarts(status: Option<&ContainerSnapshot>) -> String {
    match status
        .map(|container| usize::try_from(container.restarts).unwrap_or(0))
        .unwrap_or(0)
    {
        0 => String::new(),
        1 => ", 1 restart".to_owned(),
        counted => format!(", {counted} restarts"),
    }
}

/// **`Pod · running · created 3 days ago`, and the pod's own reason under it** — describe's
/// opening block, as the lines it says (`screens/detail.md` § The describe tab, § The pod's own
/// reason).
///
/// **Each part is dropped rather than guessed at when the field is absent** — a pod with no
/// `phase` says `Pod`, never `Pod · unknown`, which would be a reading nothing took
/// (`screens/states.md` § When there is nothing to say).
///
/// **`status.phase` still comes first when there is a reason**, because the two answer different
/// questions — *what state is it in* and *why* — and a reader who only wants the first still gets
/// it on one short line.
///
/// **`status.message` is not on `PodSnapshot` and this build cannot say it.** `rules.rs` is
/// frozen, so the sentence the mockup draws beside `(Evicted)` waits for the snapshot field the
/// PM has boxed; what is here is the half that is reachable.
pub fn identity(pod: &PodSnapshot, now: &Time) -> Vec<String> {
    let mut first = vec!["Pod".to_owned()];
    if let Some(phase) = &pod.phase {
        first.push(phase.to_lowercase());
    }
    if let Some(created) = pod
        .creation_timestamp
        .as_ref()
        .and_then(|created| age(now, created))
    {
        first.push(format!("created {created}"));
    }
    let mut said = vec![first.join(" · ")];
    if let Some(reason) = &pod.reason {
        if let Some(phrase) = phrase(POD_REASONS, reason) {
            said.push(phrase.to_owned());
        }
        said.push(raw_and_message(reason, None));
    }
    said
}

/// **`happened 2,383 times since 4 days ago`, and `None` for something that happened once**
/// (`screens/detail.md` § A repeated event).
///
/// **Both numbers where both are known, not both or neither.** The count without the span is
/// *a lot*, of unknown recency; the span without the count is *still going*, of unknown severity.
/// An event whose first stamp did not survive says the count alone rather than a span this file
/// guessed.
///
/// **Exact, with a comma at the thousand, never rounded** — the discipline
/// [`crate::k8s::LogLines::dropped_line`] already keeps for a number a reader is counting on, and
/// [`crate::k8s::grouped`] is the one spelling of the separator.
///
/// **Silent at `1`**, because a thing that happened once needs no sentence saying so. The numbers
/// are measured and not invented (`k8s-admin`, 2026-08-31: a real readiness probe on an 8-day
/// cluster, `count` 2,383, first seen 4 days before the last).
pub fn repeated(line: &crate::k8s::Happening, now: &Time) -> Option<String> {
    let counted = usize::try_from(line.count?).unwrap_or(0);
    if counted < 2 {
        return None;
    }
    let since = line
        .first
        .as_ref()
        .and_then(|first| age(now, first))
        .map_or(String::new(), |span| format!(" since {span}"));
    Some(format!(
        "happened {} times{since}",
        crate::k8s::grouped(counted)
    ))
}

/// **What a pod with no events is told** (`screens/detail.md` § No events at all).
///
/// **Two facts wearing one empty list, and only one of them is *nothing happened*.** Kubernetes
/// keeps events for a while and then drops them, so a pod up for a week has almost certainly
/// outlived every event it ever had — and saying only *nothing happened* would be true the day it
/// started and false a week later, in the one case a reader has no other way to check.
///
/// **The `k8rs: ` prefix is not in it**, because it is the stderr convention and not part of the
/// sentence: the pane draws this under `○  none right now` and the driver prefixes it.
pub const NO_EVENTS: &str = "Kubernetes only keeps events for a while, and this pod has run \
                             long enough that none are left.";

/// **[`NO_EVENTS`] when the read found nothing, and `None` when it found something.**
///
/// **The emptiness is decided here and not at the call site.** Spelled as a `match` guard in the
/// driver, both `happened.lines.is_empty() -> true` and `-> false` survived the mutation gate:
/// the only thing that depended on the answer there was a line on stderr, and stderr belongs to
/// the process (`dev-core`'s run, 2026-08-31).
pub fn no_events(happened: &crate::k8s::Happened) -> Option<&'static str> {
    happened.lines.is_empty().then_some(NO_EVENTS)
}

/// **The heading over an events list, with no punctuation of its own** (`screens/detail.md`
/// § The events tab, § More events than k8rs was given).
///
/// **The heading carries the cut, because the heading is where the claim is.** *newest first* is
/// not true of a list the server stopped at [`crate::k8s::EVENTS_KEPT`] — a `limit` returns the
/// cluster's own storage order, not the newest — so the words that promise it are the words that
/// have to be withdrawn.
///
/// **The bound is interpolated and not written out**, because a second copy of a number is the
/// copy that goes stale — the reason `scripts/twin-guard.py` exists one layer up.
///
/// **The trailing colon is the caller's and is not here.** `screens/detail.md` draws describe's
/// own block heading without one and the events tab's withdrawal with one, and the headless
/// surface has always printed one; a colon baked in would make one of the three wrong.
pub fn events_heading(happened: &crate::k8s::Happened) -> String {
    if happened.cut {
        return format!(
            "events (the first {} k8rs was given — there are more, and these are not the newest)",
            crate::k8s::EVENTS_KEPT
        );
    }
    if happened.lines.is_empty() {
        return "events".to_owned();
    }
    "events (newest first)".to_owned()
}

/// **`⇧p` on a container that has never restarted**, or `None` when there is nothing to say
/// (`screens/detail.md` § No logs yet, no previous run).
///
/// k8rs does not print the API's refusal and does not leave `previous log: on` pointed at
/// nothing — it says so in one sentence and falls back to the run that does exist.
///
/// **The prefix is the caller's**: the pane draws `⇧p — ` in front of this and the driver draws
/// `k8rs: `, which is the same split [`NO_EVENTS`] makes for the same reason.
///
/// **`restarts` is the count the kubelet reported for that container** — read through
/// `k8s::PodRead::status`, which is the one by-name lookup, rather than taken by index. **A
/// negative one is not a count**: *has restarted* is `> 0` here exactly as it is in the display
/// sites ([`restarts`] clamps with `usize::try_from`, `rules.rs` asks `> 0`), so a value no API
/// server produces cannot make two screens say *no restarts* while this one says it has restarted.
/// It was `!= 0` and did exactly that until 2026-09-07 (`k8s-admin`).
pub fn no_previous_run(container: &str, restarts: i32, previous: bool) -> Option<String> {
    if !previous || restarts > 0 {
        return None;
    }
    Some(format!(
        "{container} hasn't restarted, so there's no previous run to show. Showing the current \
         run instead."
    ))
}

// --- THE WORDING A DETAIL TAB DRAWS END ---

#[cfg(test)]
#[path = "views_tests.rs"]
mod tests;
