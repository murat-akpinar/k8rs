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
//! **Strings that did *not* arrive that way are what [`Stripped`] and [`Input`] are for, and each
//! is a type rather than a rule.** What the user types is [`Input`] — bounded in length, **and**
//! refused a [`crate::k8s::unprintable`] character, reusing the ingest strip's own predicate rather
//! than a second list (NOTES § D246 ruling 5). What a *caller* builds for the screen is
//! [`Stripped`] — a header zone, a note paragraph, a command-log line — whose only constructor a
//! caller can reach spends [`crate::k8s::text`], so an unstripped one does not compile. What the
//! *cluster* says about a mutation is [`Log::outcome`], whose word comes from
//! `ops::Performed::plainly` and so from the server: NOTES § D217 measured one of those handing
//! back the whole object that was sent, 4859 bytes on a trivial Deployment, and that is the
//! security gate's *a Secret value never enters the command log* row. It spends
//! [`crate::k8s::text`] itself at the far tighter [`SAID`].
//!
//! **It was one method until 2026-09-07 and this paragraph said so** — the second was found by
//! reading who the obvious caller of `outcome` would be (`k8s-admin`, 2026-09-07). **A third came
//! down with the driver's sentences** (NOTES § D264 ruling 1): a namespace typed on the command
//! line reaches [`scope`] and [`next_step`] inside a [`Coverage`] and never met `k8s::text`, and
//! [`sanitize`] is the driver's own strip over it, moved with them. **The later two were calls
//! somebody had to remember to write, which is why Phase 12 gave that class a type of its own**
//! (todo.md § Phase 12) — [`Input`] always was one: [`sanitize`] stays for the driver's documents,
//! and every string a caller *assembles* for `ui::Screen` goes through [`Stripped`] instead.
//! **Not every string that reaches `ui.rs`, which is what this said and is not true** — that type's
//! own doc names the exceptions and `ui::Screen`'s lists them (`k8s-admin`, 2026-09-19).
//!
//! **Nothing here sorts a rendered string back into values** (NOTES § D245,
//! `screens/analysis.md` § 3, PRIOR-ART § F1). The browser keeps the order the server sent; the
//! only sort in this file is [`cards`]', over `Severity` and a `Time`, both of which were never
//! strings.

// The module-wide `dead_code` expectation that stood here until 2026-09-23 is **gone**, with
// `ui.rs`'s, in the turn that gave `ui::draw` a caller (NOTES § D38's accepted blind spot, closed
// where its own comment said it would be). What is left is per item and named: every
// `#[expect(dead_code, …)]` below carries the box that reaches it, so an unwired function is
// reported by the build instead of by a reader.
//
// **Every one of them is wrapped in `cfg_attr(not(test))`, and that is not copied from the module
// attribute it replaced — it is measured.** `theme.rs`'s own comment records that a *module-wide*
// `dead_code` expectation is never reported unfulfilled in this crate; a **per-item** one is, under
// `cargo clippy --all-targets`, because the tests below construct variants the product does not
// reach yet. Unwrapped, they turned a green build into one
// `unfulfilled_lint_expectations` warning each under `-D warnings` (measured 2026-09-23). The
// wrapper keeps the claim where it belongs: *nothing outside a test reaches this*.
//
// **Two numbers lived here and they counted different things** (both measured, 2026-09-24):
// **fourteen** `dead_code` *warnings* were what removing the two module attributes reported — the
// unit rustc counts, so one warning covered `Modal`'s two unreached variants together — and
// **fifteen** per-item `expect`s were what replaced them, thirteen in this file and two in
// `ui.rs`, because an attribute goes on an item and not on a warning. **How many are left is not
// written down, because it is the copy that goes stale every time a box lands** (this one did,
// the turn *the cluster picker is wired* removed five of them). Counted, not recalled:
// `grep -c 'expect($' src/views.rs src/ui.rs`.
//
// **And a reason may not name a box that does not exist** (PM ruling, 2026-09-24, after `tester`
// grepped `todo.md` 4565–4732): Phase 12 has no detail-tab fetch box, no permission-probe box and
// no scale-count box, so those reasons say *no box yet* and name the work instead. The ones that
// cited **the cluster picker is wired** are gone, with that box; what still cites one cites
// **flags from `std::env::args`**, which is in the file.

use crate::analysis::Row as ReportRow;
use crate::k8s::{
    Address, Browsable, Choice, Coverage, FREE_TEXT, Fault, IDENTIFIER, Tag, text, unprintable,
};
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

// --- WHAT A CALLER BUILT START ---

/// **One line of display text that has been through the ingest strip — and the only shape
/// `ui::Screen` will take one in** (invariant 9, `screens/widgets.md` § 7).
///
/// **The guarantee is the private field, not a doc comment.** [`Stripped::of`] is the only way to
/// make one from outside this file and it strips, so a caller *cannot* hand the renderer a
/// right-to-left override. [`Stripped::assembled`] is the inside half and joins values that have
/// each already been through the strip; `Default` is the empty string, which is every strip's own
/// fixed point. That is the bar [`crate::k8s::Table`] already meets on the ingest side — one
/// `k8s::ingest` door, and no `Deserialize` to walk round it — and the bar a paragraph could not
/// meet: `ui.rs`'s module doc claimed every string reaching that file had been stripped while five
/// `ui::Screen` fields and this file's own [`Log`] took a bare `&str` (todo.md § Phase 12).
///
/// **What this type covers is the strings a caller *assembles*, and a reader here must not read it
/// as *everything `ui.rs` draws*** (`k8s-admin`, 2026-09-19). `ui::Screen`'s own doc is the list of
/// what is left over — `ui::Writes::Unaudited`'s sentence, `ui::Screen::reports`' labels, the
/// server's own words inside [`Pane::Denied`], and the four `String`s [`Dialog`] hands the
/// renderer through [`Modal`] — and **why each is safe without one, and which box owes it a type,
/// is written there and not here**, because a reason kept twice is a reason that goes stale in one
/// of the two (CLAUDE.md § A decision is written once). What a reader of this type needs is that
/// the list exists and is not this type's.
///
/// **[`crate::k8s::text`] and not [`sanitize`], which are two transformations and not one.** Every
/// value this wraps is **one line** — a header segment, a note paragraph, a command-log line — so a
/// `\n` that is *removed* glues two words into one while a `\n` that becomes a space does not. That
/// is `k8s::text`'s own split, and `k8s.rs`'s reason for treating an event message as a cell rather
/// than a document (NOTES § D198). It is also the half that **bounds**, which is the security
/// gate's *sizes are bounded* row and not a nicety: nothing between a 50 MB annotation and one of
/// these sentences has a length opinion of its own. [`crate::k8s::FREE_TEXT`] is reused rather than
/// re-picked, because a sentence is what every one of these is — and it is why a value that already
/// came through ingest is unchanged here (`k8s_tests.rs`'s
/// `sanitize_cannot_act_on_anything_the_ingest_strip_left`).
///
/// **A blank line does not survive this, and the asymmetry it leaves is deliberate**
/// (NOTES § D271): `k8s::text` turns a `\n\n` into one space, so `ui::banner`'s split on `"\n\n"`
/// is permanently dead for `ui::Screen::clock` — which `screens/states.md` § *Your computer's clock
/// is off* writes as one paragraph per direction anyway — while it stays live for
/// [`Pane::Denied`], which is not one of these. The clock sentence is k8rs's own; a refusal carries
/// the server's.
///
/// **It owns its string, and that is the borrow deciding the shape rather than a preference.** The
/// strip allocates, so there is no `&str` for a caller to lend: the four scalar fields on
/// `ui::Screen` hold a `Stripped` by value and the two slices borrow one the caller already keeps
/// in a `Vec`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stripped(String);

impl Stripped {
    /// **The only constructor reachable from outside this file**, so there is one answer to *was
    /// this stripped* and it is *yes*.
    pub fn of(value: &str) -> Self {
        let mut value = value.to_owned();
        text(&mut value, FREE_TEXT);
        Self(value)
    }

    /// **A line joined from parts this file has already stripped** — the command log's resolved
    /// line, whose command came out of [`Log::push`] and whose word came out of
    /// [`crate::k8s::text`] at [`SAID`].
    ///
    /// **Private, so [`Stripped::of`] is still the only door from outside this file** — which is
    /// where a string nobody has stripped comes from. What this skips is the *bound*, and skipping
    /// it is the whole point: `k8s::text` over an already-stripped line can only shorten it, and
    /// `→ rejected` is longer than the [`RUNNING`] mark it replaces, so re-spending it cut the
    /// outcome off a line that fitted while it was still running (`tester`, 2026-09-19).
    ///
    /// **What the gate's *sizes are bounded* row is about is a caller's unbounded string, and one
    /// cannot reach this**: every part joined here was bounded where it entered — the line at
    /// [`crate::k8s::FREE_TEXT`] by [`Log::push`], the word at [`SAID`].
    fn assembled(value: String) -> Self {
        Self(value)
    }

    /// What it holds — safe to print by construction.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// **So a reader may ask what a line says without unwrapping it.** Comparison is not construction
/// and opens nothing: the only ways a `Stripped` on the left came to exist are [`Stripped::of`]
/// and, inside this file, [`Stripped::assembled`] over values that had already been through it.
impl PartialEq<&str> for Stripped {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

// --- WHAT A CALLER BUILT END ---

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

/// **Which of the two filters `/` and `n` is being typed into** (`screens/widgets.md` § 2b).
///
/// **One at a time, and there is nothing here that can say *both*.** `n` pressed while `/` already
/// has focus is a printable character like every other one and lands in the buffer that already
/// has it — that section's own rule — so a second focus is not a state to model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typing {
    /// `/` — [`Filters::text`].
    Text,
    /// `n` — [`Filters::namespace`].
    Namespace,
}

impl Typing {
    /// **The word after `esc` that names the field it clears** — `screens/help.md`'s own
    /// `/ n   filter · namespace` row, so the key map, the typing footer and the zero-match
    /// footer spell one field one way (`screens/widgets.md` § 2b).
    pub fn label(self) -> &'static str {
        match self {
            Typing::Text => "filter",
            Typing::Namespace => "namespace",
        }
    }

    /// **The label drawn in front of the buffer, live and at rest** — and it is not
    /// [`Self::label`], which is the half this was learned the hard way
    /// (`k8s-admin`, `reports/2026-09-18-filter-and-container-picker.md` § 4).
    ///
    /// **`namespace like:`, never a bare `namespace:`.** Drawn one row under the browser's own
    /// title, a bare `namespace:` reads as a second *scope* — the server-side `ns: payments` that
    /// title already carries — rather than a substring match over rows already in hand, and no
    /// operator has a reason to tell the two apart from two labels stacked a row apart. The
    /// zero-match sentence had the fix already (*"a namespace like `pay`"*), so the label takes
    /// the same word rather than growing a third spelling.
    pub fn prompt(self) -> &'static str {
        match self {
            Typing::Text => "filter",
            Typing::Namespace => "namespace like",
        }
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
    /// **Which pods this card is about** — the list [`Card::affected`] is the length of, and the
    /// list `screens/detail.md` § *Picking a pod, before Detail has one* draws one row of each.
    ///
    /// **One scan, because there is one rule about what *distinct* means here** (NOTES § D39,
    /// § D270): the number of distinct `object`s in the group whose kind is `Pod`,
    /// **distinct over the whole `ObjectId`, uid included** — a `Vec` and a linear `contains`, not
    /// a set of `group_key()`, which answers *which card* rather than *what is counted on it*.
    /// [`cards`] sets `affected` from this and the which-pods step lists it; a second scan in the
    /// renderer is how a count and the rows under it come to disagree.
    ///
    /// **In `analyze`'s order and not the step's** — which pod row comes first is severity, then
    /// recency, and that needs the moment this method is not given ([`recency`]).
    pub fn pods(&self) -> Vec<&ObjectId> {
        let mut pods: Vec<&ObjectId> = Vec::new();
        for finding in &self.findings {
            if finding.object.kind == ObjectKind::Pod && !pods.contains(&&finding.object) {
                pods.push(&finding.object);
            }
        }
        pods
    }

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
        // **The scan is [`Card::pods`]'s and is not repeated here** — the count and the rows the
        // which-pods step draws are the same list seen twice (NOTES § D270).
        let counted = card.pods().len();
        card.affected = counted;
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
    recency(
        a.newest(now).and_then(|f| f.timestamp.as_ref()),
        b.newest(now).and_then(|f| f.timestamp.as_ref()),
    )
}

/// **Newer before older, and *no drawable age* last** — the recency half of every
/// severity-then-recency sort this product draws, spelled once.
///
/// **`Option<Time>`'s derived `Ord` puts `None` first and `screens/alerts.md` wants an ageless row
/// last inside its own band**, so it is written out rather than reached for (NOTES § D69) — and it
/// is `pub` because the Alerts card list is not the only list on that rule: the which-pods step
/// orders its pod rows the same way, one level down, and *"a reader who sees one order here and a
/// different one on the object's own Detail a keypress later would have learned nothing to rely
/// on"* (`screens/detail.md` § Picking a pod).
///
/// **What a caller passes is a *drawable* stamp and not a raw one**, which is the trap
/// [`Card::newest`] already carries: a stamp past `rules::age`'s skew allowance draws no age, so
/// sorting on it puts a card with a blank right edge at the top of its band (NOTES § D246
/// ruling 4). This function cannot check that — it is handed the answer.
pub fn recency(left: Option<&Time>, right: Option<&Time>) -> Ordering {
    match (left, right) {
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
        namespace_ok && holds(&self.text, fields)
    }

    /// **Is anything being filtered at all?** What decides whether the pane draws a `filter: "web"`
    /// line over the list it narrowed (`screens/widgets.md` § A committed filter is drawn at rest,
    /// too).
    pub fn any(&self) -> bool {
        self.clears().is_some()
    }

    /// **Which field the next `esc` empties, or `None` when there is nothing left to empty** —
    /// [`App::escape`]'s own text-before-namespace order, read rather than restated.
    ///
    /// **Three screens word a key off this and none of them counts the fields itself**: the
    /// at-rest line's `esc clears filter`, the zero-match footer's `esc clear filter`
    /// (`screens/states.md` § The filter hides every row) and `escape` itself. A footer naming a
    /// field `esc` is not about to clear is the *promised key that does nothing* this product
    /// already forbids, one word over.
    pub fn clears(&self) -> Option<Typing> {
        if !self.text.is_empty() {
            Some(Typing::Text)
        } else if !self.namespace.is_empty() {
            Some(Typing::Namespace)
        } else {
            None
        }
    }
}

/// **`/` over whatever a row shows the reader** — [`Filters::matches`]' text half, and the cluster
/// picker's whole filter ([`Picker::shown`]), so the two panes cannot come to disagree about what
/// a typed filter matches (NOTES § D264 ruling 7).
fn holds(filter: &Input, fields: &[&str]) -> bool {
    filter.is_empty()
        || fields
            .iter()
            .any(|field| contains_ignoring_case(field, filter.text()))
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
/// and no z-index; one level is all `esc` can ever have to close.
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
    /// **`c` on the logs tab — which container to read** (`screens/detail.md` § Choosing a
    /// container, and when there is nothing to choose).
    ///
    /// **A [`Cursor`] and not a [`Picker`]**, because the two pickers differ in everything but the
    /// word: this one has no filter, no shadowed row, no badge and nothing it cannot land on — a
    /// container is always pickable — so what is left is *which row*, which is what `Cursor`
    /// already is. Its anchor is the container's own name, which is unique inside one pod and
    /// stable for its life.
    ///
    /// **The list is not in here**, for [`Picker`]'s reason one box over: it is the selected pod's
    /// `crate::rules::PodSnapshot` containers, in `spec` order, handed to the renderer as
    /// `crate::ui::Described::containers` and never copied here.
    ///
    /// **Nothing in this variant says the pod still exists, and the screen is what closes it**
    /// (§ The pod disappears while the picker is open). A pod deleted under an open picker leaves
    /// no container to pick, and a box with no rows is not drawn and its footer is not offered —
    /// there is no second `Gone` for it, because picking a container is not a pending mutation
    /// and has nothing to reassure anybody about.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "`c` opens it off the pod read, and **no box fetches the four detail tabs \
                      yet** — Phase 12 has none; the tabs draw `Loading` until one exists"
        )
    )]
    ContainerPick(Cursor),
    /// **The cluster picker — on `X`, and by itself at startup** (`screens/context.md`,
    /// NOTES § D16, § D116). One list and one key map both ways; [`Picker::startup`] is the whole
    /// of the difference, and it is read off what has connected rather than off who opened it
    /// (NOTES § D264 ruling 4).
    ContextPick(Picker),
    /// **A picked context that did not connect** (`screens/context.md` § When the new cluster does
    /// not work). **It never falls back** (NOTES § D16 ruling 3): the reader asked for this context
    /// and is told why not, on the context they asked for.
    ///
    /// **It carries what the driver's own sentences read, and no sentence** (NOTES § D264
    /// ruling 1): the reason is [`because`]'s, the scope [`scope`]'s and the next step
    /// [`next_step`]'s, the same three a `--once` run that could not read pods prints.
    Unconnected {
        /// The picked context's name as drawn — [`Choice::name`], `None` being [`UNNAMED`].
        to: Option<String>,
        /// What was behind the attempt: what `esc` goes back to, and what the way out names.
        before: Before,
        /// **Whether a request went out** — `false` for a `k8s::NotConnected`, whose client was
        /// never built, and `true` for a pods watch that never listed. It decides the title and
        /// what k8rs is said to have asked for: *reach this cluster*, the driver's framing for a
        /// connection that sent nothing, against `list` and `watch` pods. **Four faults are the
        /// first framing whatever this says** — `Kubeconfig`, `NoContext`, `BadEntry` and
        /// `NoCredential` never reach a cluster, even on a watch (NOTES § D264 ruling 17).
        sent: bool,
        /// **What went wrong, as `k8s.rs` classified it** — `NotConnected::fault`, or the pods
        /// watch's own `Trouble::fault`.
        fault: Fault,
        /// **The cluster's own words about the failure**, where it sent any — `Trouble::said`,
        /// already stripped and bounded (invariant 9). Only [`Fault::Rejected`]'s sentence reads
        /// it, and it is still cut at draw time: `k8s::FREE_TEXT` allows 4096 bytes.
        said: Option<String>,
        /// **Where the watches asked** — the whole [`Coverage`], never its collapsed
        /// `namespace()`, because a namespace the reader named and one k8rs had to guess take
        /// opposite next steps (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R1).
        coverage: Coverage,
        /// **The program this kubeconfig logs in with** — `NotConnected::renewal` or
        /// `Session::renewal`, already stripped and bounded.
        renewal: Option<String>,
    },
}

/// **`(unnamed)` — a context whose name strips to nothing**, drawn in the picker's name slot and in
/// the header's `ctx:` alike, so the two cannot disagree about whether a context is in use
/// (`screens/context.md` § A context whose name strips to nothing, NOTES § D202). `pub` because the
/// header is joined by the caller.
pub const UNNAMED: &str = "(unnamed)";

/// **A row's name as the picker draws it** — [`UNNAMED`] for one that stripped to nothing — and so
/// also what `/` matches (NOTES § D264 ruling 7).
pub fn drawn_name(row: &Choice) -> &str {
    row.name.as_deref().unwrap_or(UNNAMED)
}

/// **A row's tag as the picker draws it**: a written one as written, a derived one behind the `~`
/// that marks it a guess (`screens/context.md` § Two kinds of tag), and so also what `/` matches.
pub fn drawn_tag(tag: &Tag) -> Cow<'_, str> {
    match tag {
        Tag::Written(tag) => Cow::Borrowed(tag),
        Tag::Derived(tag) => Cow::Owned(format!("~{tag}")),
        Tag::Blank => Cow::Borrowed(""),
    }
}

/// **What was behind a context that did not connect** — the one difference between the two boxes
/// `screens/context.md` § When the new cluster does not work draws.
///
/// **[`Picker::chosen`] hands one back with the row**, read off the picker's own [`Connection`], so
/// a caller that builds the failure box from it names a cluster as fine only when one really was
/// live in this run (NOTES § D264 rulings 4 and 15).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Before {
    /// **A context has connected in this run** — the one live when `⏎` was pressed or, after a
    /// switch that already failed, the last one that was: its name as drawn (`None` is
    /// [`UNNAMED`]). Nothing of it is kept ([`App::switched`]); the name is what *"X takes you
    /// back"* is about, and `X` does, because that context is on the picker it opens.
    Connected(Option<String>),
    /// **Nothing has connected in this run** — the picker `esc` reopens, on the row that was tried,
    /// over the same list it was handed. **The caller keeps this value across [`App::switched`]**,
    /// which clears the modal it came out of.
    Picking(Picker),
}

impl Before {
    /// **The word after `esc`, on the button and on the footer both**, so the two cannot spell one
    /// key two ways ([`Dialog::confirm`]'s reason).
    pub fn leave(&self) -> &'static str {
        match self {
            Before::Connected(_) => "dismiss",
            Before::Picking(_) => "back to the list",
        }
    }
}

/// **What is connected while the picker is open** — the two facts NOTES § D264 ruling 4 reads, in
/// the three combinations they can hold. *Nothing live, but something connected earlier* is a real
/// state, and *live, but nothing ever connected* is not one, so this is not two `bool`s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    /// **Nothing has connected in this run** — before the first `⏎`, between it and its answer,
    /// and after every failure since. The startup picker: `⏎ connect`, `esc quit`.
    Never,
    /// **A context connected earlier in this run and none is live now** — a switch failed, or has
    /// not answered yet. **It carries the context that was last live**, its name as drawn, so a
    /// second and third failed switch in a row still name it (NOTES § D264 ruling 15).
    Dropped(Option<String>),
    /// **The `(current)` row is the live context** — its name as drawn. The rows must then be
    /// `k8s::contexts` asked for that context, so its row is the one marked current.
    Live(Option<String>),
}

/// **The picker's own state: which row, what `/` typed, and what is connected behind it**
/// (`screens/context.md` § The picker).
///
/// **The list is not in here**, for [`App`]'s own reason: it is [`crate::k8s::contexts`]' answer
/// over the kubeconfig — every string already stripped and bounded there — read once when the
/// picker opens and handed unchanged to every method below and to `ui::Screen::contexts` until it
/// closes. That one read is what keeps a row's place meaning the same row.
///
/// **The row is its place in that list and never its name**: a shadowed row shares its name with
/// the entry above it (NOTES § D174), which is also why this is not a [`Cursor`], whose anchor is a
/// string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picker {
    connection: Connection,
    at: Option<usize>,
    /// `/` — narrows the list to rows whose drawn name or drawn tag holds it.
    pub filter: Input,
}

/// **What `⏎` means on the row the picker is on** (`screens/context.md`).
pub enum Chosen<'a> {
    /// `(current)`, while it is live — *yes, stay on this cluster*, and the picker's job is done.
    Close,
    /// **Nothing happens and the picker stays open** — a shadowed row, whose sentence is already
    /// on screen, or no row selected at all (NOTES § D264 ruling 2).
    Stay,
    /// **Connect to this row** (NOTES § D264 ruling 8): the caller connects with [`Choice::key`],
    /// the file's own spelling, and draws [`Choice::name`].
    Connect {
        /// The row `⏎` was pressed on.
        row: &'a Choice,
        /// What the failure box, if it comes to one, goes back to.
        before: Before,
    },
}

impl Picker {
    /// **Opened on `(current)`, and on no row when `(current)` cannot be landed on** — or is not in
    /// the file at all — so `⏎` with no other keypress connects only where the silent default
    /// would have, and never to whatever happens to be first (NOTES § D116, § D264 ruling 3).
    pub fn new(rows: &[Choice], connection: Connection) -> Self {
        Self {
            connection,
            at: rows.iter().position(|row| row.current && landable(row)),
            filter: Input::default(),
        }
    }

    /// **Nothing has connected in this run**, so `⏎` connects, `esc` quits, and nothing is behind
    /// the picker (`screens/context.md` § Opening at startup).
    pub fn startup(&self) -> bool {
        self.connection == Connection::Never
    }

    /// The rows `/` leaves, in file order — the ones the cursor skips included, because they are
    /// still drawn.
    pub fn shown(&self, rows: &[Choice]) -> Vec<usize> {
        (0..rows.len())
            .filter(|&at| {
                let row = &rows[at];
                holds(&self.filter, &[drawn_name(row), &drawn_tag(&row.tag)])
            })
            .collect()
    }

    /// **The row the keys act on**, or `None` — none was ever selected, or none the filter shows
    /// can be landed on. A selection the filter hid gives way to the first shown row it may land
    /// on, derived rather than re-anchored, so a filter keystroke needs no second call.
    pub fn selected(&self, rows: &[Choice]) -> Option<usize> {
        self.within(&self.landing(rows))
    }

    /// `↑` — the last row it may land on when nothing is selected. Clamps at the top, as
    /// [`Cursor`] does.
    pub fn up(&mut self, rows: &[Choice]) {
        let landing = self.landing(rows);
        self.at = match self.within(&landing) {
            Some(from) => Some(
                landing
                    .iter()
                    .copied()
                    .rfind(|&at| at < from)
                    .unwrap_or(from),
            ),
            None => landing.last().copied().or(self.at),
        };
    }

    /// `↓` — the first row it may land on when nothing is selected. Clamps at the bottom.
    pub fn down(&mut self, rows: &[Choice]) {
        let landing = self.landing(rows);
        self.at = match self.within(&landing) {
            Some(from) => Some(
                landing
                    .iter()
                    .copied()
                    .find(|&at| at > from)
                    .unwrap_or(from),
            ),
            None => landing.first().copied().or(self.at),
        };
    }

    /// `⏎`.
    pub fn chosen<'a>(&self, rows: &'a [Choice]) -> Chosen<'a> {
        let Some(row) = self
            .selected(rows)
            .map(|at| &rows[at])
            .filter(|row| !row.shadowed)
        else {
            return Chosen::Stay;
        };
        let before = match &self.connection {
            Connection::Live(_) if row.current => return Chosen::Close,
            Connection::Live(name) | Connection::Dropped(name) => Before::Connected(name.clone()),
            Connection::Never => Before::Picking(self.clone()),
        };
        Chosen::Connect { row, before }
    }

    /// **`⏎` would do nothing** — the button draws dim and the footer does not offer it
    /// (NOTES § D264 ruling 2). [`Picker::chosen`]'s own answer, so the three cannot disagree.
    pub fn inert(&self, rows: &[Choice]) -> bool {
        matches!(self.chosen(rows), Chosen::Stay)
    }

    /// **No row the list shows can be landed on** — every one names a cluster the file does not
    /// define, `/` left only such rows, or the list shows no row at all: a filter that hides every
    /// row, or a kubeconfig with no contexts left in it — so neither `↑`/`↓` nor `⏎` has anywhere
    /// to go, and neither is offered (NOTES § D264 rulings 16 and 18).
    pub fn nowhere(&self, rows: &[Choice]) -> bool {
        self.landing(rows).is_empty()
    }

    /// **The word after `⏎` and the word after `esc`** — `connect`/`quit` at startup,
    /// `switch`/`cancel` on `X` — for the buttons and the footer both, so one key is never spelled
    /// two ways in one frame. **While `/` holds text `esc` clears it first** ([`App::escape`]), so
    /// its word is `clear filter` on either picker (NOTES § D264 rulings 27 and 31).
    pub fn verbs(&self) -> (&'static str, &'static str) {
        let (go, leave) = if self.startup() {
            ("connect", "quit")
        } else {
            ("switch", "cancel")
        };
        if self.filter.is_empty() {
            (go, leave)
        } else {
            (go, "clear filter")
        }
    }

    /// The shown rows the cursor may land on, in file order.
    fn landing(&self, rows: &[Choice]) -> Vec<usize> {
        let mut shown = self.shown(rows);
        shown.retain(|&at| landable(&rows[at]));
        shown
    }

    /// [`Picker::selected`] over a landing list already read.
    fn within(&self, landing: &[usize]) -> Option<usize> {
        let at = self.at?;
        if landing.contains(&at) {
            Some(at)
        } else {
            landing.first().copied()
        }
    }
}

/// **Whether the cursor may land on a row** — every row but one whose cluster the file does not
/// define, which has nowhere for `⏎` to go (§ A context whose cluster the file does not define). A
/// shadowed row is landed on whatever its entry says, because the sentence it shows when selected
/// is the one thing that row is for (§ A context defined twice).
pub fn landable(row: &Choice) -> bool {
    row.shadowed || row.server != Address::Undefined
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the caller that re-reads an object and compares this before confirming is \
                      v0.4's `edit`; nothing in v0.1 reads it back (NOTES § D22)"
        )
    )]
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

    /// **Whether the cluster's check is still out** — the one state in which *no* key inside a
    /// confirmation is live, `esc` included (`screens/dialogs.md` § The verdict line: *"for
    /// `scale` and `restart`, `esc` is inert for as long as a real round trip to the cluster
    /// takes"*, NOTES § D214's *"`esc` is inert until the verdict arrives"*).
    ///
    /// **One predicate, two readers, because they were two answers.** [`App::footer`] has drawn
    /// `waiting for the cluster` — naming neither key — since the dialogs were drawn (`1f687fc`,
    /// 2026-09-12), while [`App::escape`]'s catch-all arm took the dialog and dropped it: `esc`
    /// closed a confirmation the footer had just said nothing could be pressed on, with the check
    /// still on the wire behind it.
    ///
    /// **It is [`Self::verdict`] alone and not [`Self::armed`]**, which is the difference between
    /// *the cluster has not answered* and *you have not typed the name yet*. The second is a
    /// dialog waiting on the reader, and `esc cancel` is exactly the key that should work there.
    /// **`delete` is in that second state from its first frame and is never in this one** — it
    /// sends no check, so its verdict is `Some` before the box is drawn (NOTES § D225 ruling 1).
    pub fn waiting(&self) -> bool {
        self.verdict.is_none()
    }

    /// **Whether the confirm button is drawn live** — the dry-run has answered, and for a
    /// typed-name dialog the name matches (`screens/widgets.md` § 5, `screens/dialogs.md` rule 3).
    /// **It is two questions and [`Self::waiting`] is only the first of them**, which is why the
    /// cancel button beside it un-dims off that one alone (`screens/dialogs.md` § While the check
    /// is still on the wire, ruling 1).
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
/// process (the security gate). **Every line here is a [`Stripped`], and that is what changed in
/// Phase 12** (todo.md § Phase 12): three of the four strings that reach this type *were* stripped
/// before they got here — the manifest through `main.rs`'s own `sanitize`, a read line's names
/// through `k8s::text` at ingest, a mutation's line through `ops::Record::of` — but all three were
/// promises held by the **caller**, and a fourth caller with a fourth string would have been
/// nobody's failed test. [`Log::push`] is the single door now, so the type holds what the callers
/// were trusted for.
///
/// **[`Log::outcome`]'s word keeps its own strip all the same**, at the much tighter [`SAID`]: the
/// caller that method is written for hands over whatever the cluster said, and NOTES § D217
/// measured one of those returning the whole 4859-byte object that was submitted. A line-length
/// bound is not a word-length bound, and the row it defends is *a Secret value never enters the
/// command log*.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log {
    lines: Vec<Stripped>,
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
    pub fn lines(&self) -> &[Stripped] {
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
        // **[`OUTCOME_GAP`] is measured from what the line ends in *after* the strip**, and
        // composing before [`Log::push`] meant it was not: `k8s::text` substitutes one space for a
        // whitespace control character rather than deleting it (NOTES § D198), so a caller's line
        // ending in `\n`, `\t` or `\r` drew a four-column gap where `screens/widgets.md` § 2 rules
        // three (`tester`, 2026-09-19). **`trim_end` is `char::is_whitespace`, which is a *wider*
        // set than what `k8s::text` turns into a space, and the first draft of this comment claimed
        // they were the same split** (`k8s-admin`, 2026-09-19; NOTES § D154): that strip applies
        // `is_whitespace` only to characters [`crate::k8s::unprintable`] already answers for, so
        // NBSP `\u{a0}`, `\u{2028}`, `\u{2003}` and `\u{3000}` are whitespace it deliberately
        // **keeps** and this call removes. The behaviour is right either way — three columns for
        // every spelling, measured
        // (`the_gap_before_an_outcome_is_three_columns_whatever_the_line_ends_in`) — and what it
        // costs is that [`Log::ran`] and this method now differ over a line ending in one of those
        // four: `ran` keeps the character, `sent` trims it. A gap `screens/widgets.md` § 2 rules at
        // three columns is the thing being defended; a trailing NBSP no kubectl line has a use for
        // is not.
        self.push(format!("{}{OUTCOME_GAP}{RUNNING}", line.trim_end()));
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
        //
        // **And the result is assembled, never re-bound.** Both halves are already stripped — the
        // command by [`Log::push`], the word by the [`SAID`] strip above — so a second
        // [`Stripped::of`] could only *shorten*, and `→ {said}` is longer than the mark it
        // replaces: a line that fitted while it was running came back cut, with the cut landing on
        // the outcome. `yyy…   →… (shortened by k8rs)` is an arrow pointing at k8rs's own
        // shortening mark, which reads as an outcome and is not one (`tester`, 2026-09-19). The
        // honest failure one line up — a line already past [`crate::k8s::FREE_TEXT`] never
        // resolves at all ([`Log::push`]) — is the other direction and stays.
        let replaced = self.lines[at]
            .as_str()
            .strip_suffix(RUNNING)
            .map(|command| Stripped::assembled(format!("{command}→ {said}")));
        if let Some(replaced) = replaced {
            self.lines[at] = replaced;
        }
    }

    /// Append, and drop from the front once past [`KEPT`].
    ///
    /// **The strip is here and not at any of the three entry points above**, so a fourth builder
    /// cannot arrive without it ([`Stripped`]). It is a no-op on everything today's callers hand
    /// over, which is the point: what it removes is what a *future* caller would have forgotten.
    ///
    /// **It also bounds, and the one thing that costs is worth writing down**: a line past
    /// `k8s::FREE_TEXT` ends in the shortening mark instead of [`RUNNING`], so [`Log::outcome`]'s
    /// `strip_suffix` finds nothing and that line never resolves. Unreachable through today's
    /// builders — every name in a kubectl line came through ingest at `k8s::IDENTIFIER`, and a
    /// handful of them is nowhere near 4096 bytes — and the failure is the honest direction: a line
    /// that keeps no outcome claims less than it knows, never more.
    fn push(&mut self, line: String) {
        self.lines.push(Stripped::of(&line));
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "private to the three lines below, which are logged when the read each names \
                  is issued — and no box issues those reads yet"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "logged when the read it names is issued, and no box issues it yet — Phase 12 \
                  has no detail-tab fetch box"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "logged when the read it names is issued, and no box issues it yet — Phase 12 \
                  has no detail-tab fetch box"
    )
)]
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "logged when the read it names is issued, and no box issues it yet — Phase 12 \
                  has no detail-tab fetch box"
    )
)]
pub fn yaml_line(resource: &str, id: &ObjectId) -> String {
    format!(
        "$ kubectl get {resource} {}{} -o yaml --show-managed-fields",
        id.name,
        in_namespace(id.namespace.as_deref())
    )
}

/// **`$ kubectl config get-contexts`** — the line the picker puts on the strip when it opens,
/// either way (`screens/context.md` § Opening at startup). **Never `kubectl config use-context`**,
/// which edits the kubeconfig k8rs never writes (NOTES § D16 ruling 2).
pub const GET_CONTEXTS: &str = "$ kubectl config get-contexts";

/// **A strip line split into the command and what [`Log`] wrote after it** — [`OUTCOME_GAP`] and
/// [`RUNNING`], or the gap and `→ ` an outcome — and `""` after a line nothing was written after.
///
/// **It is here because this type spells that suffix and nothing else may**: the strip reserves the
/// second half before it cuts the first, and a copy of the gap and the two marks in `ui.rs` is the
/// second spelling that goes stale (`screens/dialogs.md` § The command log's own line, NOTES §
/// D266). **The last outcome arrow is the one it splits at**, so what it hands back as the tail is
/// the shortest there is, which [`SAID`] bounds.
pub fn outcome_of(line: &str) -> (&str, &str) {
    let running = format!("{OUTCOME_GAP}{RUNNING}");
    let at = if line.ends_with(&running) {
        Some(line.len() - running.len())
    } else {
        line.rfind(&format!("{OUTCOME_GAP}→ "))
    };
    at.map_or((line, ""), |at| line.split_at(at))
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
    /// (NOTES § D16 ruling 4), which rebuilds discovery and therefore this whole value — and
    /// [`App::switched`] is what puts the view back on Alerts, not a second field here.
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

/// **Which of the two panels the keys are moving in** (`screens/help.md`: `tab  next panel`).
///
/// **It is the one piece of state `screens/widgets.md` § 3 does not name, and `tab` cannot be
/// bound without it** (PM ruling, 2026-09-23, recorded because the brief did not decide it): `↑↓`
/// and `⏎` mean *the sidebar's rows* or *the pane's rows* and the two cursors are already separate
/// fields — what was missing was which of them a key is talking to. **What it does not do is
/// change what anything draws**: the sidebar marks its own selected row either way, and the
/// content pane's selection has never been marked at all, so *which panel has focus* is not
/// visible on screen. That is a screen question and it is `screens/`'s to answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Panel {
    /// The sidebar — `ALERTS`, the browsable kinds, the reports.
    Sidebar,
    /// **The content pane, which is where a run starts.** `screens/alerts.md`'s footer is that
    /// pane's — `↑↓ move  ⏎ open  s scale` all act on the object under its cursor — so a reader
    /// who presses `↓` before anything else is moving through the cards.
    #[default]
    Content,
}

impl Panel {
    /// `tab`. **Two panels, so there is no order to get wrong**; a third would make this a list.
    pub fn next(self) -> Panel {
        match self {
            Panel::Sidebar => Panel::Content,
            Panel::Content => Panel::Sidebar,
        }
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
    /// **Which pod of a group the which-pods step is on** (`screens/detail.md` § Picking a pod,
    /// before Detail has one).
    ///
    /// **Its own field and not [`App::content`]'s**: the step is drawn *over* a view whose cursor
    /// is still on the card the reader opened, and `esc` goes back to it — borrowing that cursor
    /// would move the card underneath while a list of its pods is on screen.
    ///
    /// **Anchored on the pod's name and not on its uid**, which is the one place this file departs
    /// from [`Cursor`]'s own rule and for [`Modal::ContainerPick`]'s reason: a pod deleted and
    /// recreated under this list is a row that left and a row that arrived, which is what the
    /// section's own *"one pod vanishing removes one row"* asks [`Cursor::follow`] to do.
    ///
    /// **What makes that safe is the store's key and not anything true of names.** [`Card::pods`]
    /// is distinct over the whole `ObjectId`, uid included, so two rows *could* share a name — a
    /// StatefulSet `web-0` stuck `Terminating` behind a finalizer beside its replacement, the
    /// shape [`Card::count`]'s own doc already cites — and both would anchor `"web-0"`, leaving
    /// the second unreachable. That shape does not arrive: `k8s.rs` keys the watch store on
    /// `(namespace, name)`, so two live pod objects of one name cannot coexist in it
    /// (`k8s-admin`, 2026-09-18, `reports/2026-09-18-the-which-pods-step.md` § 5). **If that key
    /// ever gains the uid, this anchor has to gain it too**, and that sentence is the whole reason
    /// this paragraph is here rather than a claim about names being unique.
    pub pods: Cursor,
    /// `/` and `n`.
    pub filters: Filters,
    /// **Which filter is being typed into, or `None` for browsing** (`screens/widgets.md` § 2b).
    ///
    /// **It is a mode and not a widget.** While it is `Some`, every printable key is text — `s`,
    /// `q`, `X` and `?` included — so the three `may_*` questions below all answer no through it,
    /// and the footer is replaced rather than curated. The buffer being typed into is
    /// [`Filters::text`] or [`Filters::namespace`] itself, never a third copy that would have to be
    /// written back: that is what makes the list narrow live, one keystroke at a time.
    ///
    /// **`↑` and `↓` are the exception, and the footer not naming them is not an unbinding**
    /// (`screens/widgets.md` § 2b: *"`↑` / `↓` still move the selection over whatever rows the
    /// live-narrowed list is currently showing, exactly as they do outside typing"*). An arrow is
    /// never a letter, which is what lets the picker's own list do the same thing; the typing
    /// footer spends its room on the two keys that get the reader *out*, which is § 2a's curation
    /// rule and not a claim that nothing else is bound. **Said here because the key router is
    /// Phase 12's and the footer is the only other place this could be read off.**
    ///
    /// **`⏎` is `self.typing = None` and has no method of its own**: committing keeps what was
    /// typed and changes nothing else — the filter was already live while it was being typed, so
    /// there is no second pass over the rows and no cursor to move ([`Cursor::follow`] has been
    /// following the narrowed list throughout).
    pub typing: Option<Typing>,
    /// Which detail tab is open, when something is open.
    pub tab: Tab,
    /// **Which panel `↑↓` and `⏎` are talking to** ([`Panel`]) — `tab` is what moves it.
    pub focus: Panel,
    /// **The free-text panes' own scroll offsets — one per tab, indexed by [`Tab::at`]** — logs,
    /// yaml, describe **and events**. Lists and tables do not use them: ratatui keeps a selection
    /// in view by itself, so their scrolling is [`App::content`]'s (`screens/widgets.md` § 4).
    ///
    /// **Four numbers and not one, which is `screens/widgets.md` § 4's own ruling** — *"one offset
    /// per tab, not one shared by all four"*. A `Paragraph` has no `ListState` to isolate one
    /// tab's scrolling from another's the way a list gets for free, so this product keeps four
    /// numbers instead: yaml at row 400, `]` to a three-row describe pane and back, and yaml is
    /// still at 400 rather than at whatever describe's own body clamped a shared number down to
    /// (NOTES § D272 § 1, `tester`'s measured case — the defect storing the clamp created).
    ///
    /// **A resize discards all four, and so does closing the detail slot and opening it again** —
    /// that section's own two rules, both of them the key handler's to carry out
    /// ([`Self::rewound`])
    /// because neither a resize nor an `esc` is a fact this file can see. The row an offset names
    /// is a row of *wrapped, rendered* text, so it means nothing at a width it was not measured at
    /// (PRIOR-ART § D3), and a re-opened tab is reading a buffer that was fetched again from
    /// scratch.
    ///
    /// **Events was not on that list until Phase 11 drew it** (`screens/detail.md` § More events
    /// than the pane): that pane is a `Paragraph` with an offset like the other three, not a list
    /// with a cursor, so it is this field and not a fifth one.
    ///
    /// **Nothing here clamps it to the end of the content, and that is the renderer's job** — the
    /// number of lines a pane has depends on the width it is drawn at, which this file has no
    /// business knowing (`ui::scrolled`).
    ///
    /// **So this holds the row the last frame actually drew, written back by `ui::scrolled`, and
    /// that is what makes it an offset a key can move from.** The clamp used to be computed and
    /// thrown away: while [`Self::following`] pinned the pane to the bottom this field kept
    /// whatever it held when the tab opened — 0 — so the first [`App::scroll_by`] out of follow
    /// saturated to the top of the buffer rather than stepping one line up from the tail, and an
    /// offset past the end stayed past the end no matter how often the screen clamped it.
    ///
    /// **The rule is `screens/widgets.md` § 4's** — *"follow mode (`f`) pins the offset to the
    /// bottom and any manual scroll turns it off"* — and a bottom this field does not hold is a
    /// bottom the scroll that turns follow off cannot start from. `screens/detail.md` § When the
    /// buffer fills says the reader's half of the same fact, that turning follow off freezes the
    /// *view* and not the stream under it; it names no row, so the row is this one's to keep.
    ///
    /// **It is the same reason a `ListState` is handed to its widget by `&mut`** — that section's
    /// *"a selection that moves can never leave it pointing at the wrong window"* — applied to the
    /// four offsets on this product that are stored between frames.
    pub scroll: [u16; Tab::ALL.len()],
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
/// which is `screens/states.md`'s own word: it is off the line entirely, and never *refused*.
/// [`Offer::Act`] is where that half is decided, and it asks the same two `ops` functions this
/// paragraph names rather than keeping a list of its own.
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "built from `ops::may_i_in`'s answers, and no box wires that probe yet — \
                      the console draws every key unmarked (NOTES § D229 ruling 4's fail-open)"
        )
    )]
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

/// **What the detail slot is showing, as far as the keys are concerned** — the input [`App`]
/// cannot hold, because *what is open over the view* is `crate::ui::Screen::detail`'s and this
/// file cannot see it ([`App::escape`]'s own reason for taking it).
///
/// **One value and not a count beside a flag** (NOTES § D270). A container count and
/// *the which-pods step is open* as two fields can both be set, which is a screen nothing can
/// draw — the same reason [`Modal`] is one enum and not a row of `Option`s, and the defect class
/// this repo has paid most for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Detailing {
    /// **Nothing is open over the view**, so the view's own keys are the live ones.
    #[default]
    Closed,
    /// **One object's four tabs**, over a pod k8rs knows `containers` of. `0` and `1` are both
    /// *there is nothing to pick*: a pod whose snapshot has not landed and a pod that only ever
    /// has one draw the same screen, because guessing is what the header's own vitals refuse.
    ///
    /// **`from_step` is whether `⏎` reached these tabs through the which-pods step**, and it is
    /// carried rather than drawn from (NOTES § D270). Nothing on this screen reads it yet: what
    /// `esc` out of a tab reached that way should do is the wiring box's to settle with
    /// `tui-designer`. **It is here now because `views.rs` freezes at this phase's close and that
    /// box is later in the same phase** (`todo.md` § Phase 12), and without it the router's only
    /// options are a private flag in `main.rs` about what this file's state means — D103's second
    /// copy — or reopening a frozen file. [`crate::ui::Detail`] is not the discriminator: its
    /// `card` is `Some` both for a tab reached through the step and for one reached by `⏎`
    /// straight onto a bare-pod card.
    Tabs { containers: usize, from_step: bool },
    /// **Which pod of a group to open, before Detail has one**
    /// (`screens/detail.md` § Picking a pod, before Detail has one). It carries no count: what
    /// the step is about is the card, and the card is the renderer's.
    Pods,
}

/// **Whether `c` has more than one answer, which is the whole of *is there anything to pick***
/// (`screens/detail.md` § Choosing a container, and when there is nothing to choose).
///
/// **One predicate, four readers, and the fourth is the key router** — [`App::footer`]'s logs
/// line, its container-picker line, [`App::escape`]'s dropping of a picker that has nothing left
/// in it, and the caller that has to decide what a press just meant.
///
/// **`pub` because [`App::escape`] cannot hand its answer back** — see that method's own doc:
/// `modal` is `None` after the press either way, so the fact has to be read *before* it
/// (`k8s-admin`, 2026-09-18).
pub fn picking(open: Detailing) -> bool {
    matches!(open, Detailing::Tabs { containers, .. } if containers > 1)
}

/// **Fail open, in one line: a probe may never be the reason a permitted action is marked**
/// (NOTES § D229 ruling 4). Everything but a `No` — a yes, a review k8rs could not get an answer
/// out of, and a question that was never asked — is the ordinary key.
///
/// **One `No` anywhere in the operation's answers refuses it**, which is the other half of the
/// same rule read across a set: an operation the login cannot complete is not lit because most of
/// what it needs was granted. An empty set — an operation with nothing asked — is lit, which is
/// `any`'s own answer and is the *not asked* case above.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "private to `Refused::of`, and no box wires the `ops::may_i_in` probe it is \
                  built from"
    )
)]
fn refuses(answers: &[Option<&Verdict>]) -> bool {
    answers
        .iter()
        .any(|answer| matches!(answer, Some(Verdict::No)))
}

/// **Which shape the list footer is in** — `screens/states.md`'s own four, the two `switch` adds to
/// them, and [`Offer::Act`], the ordinary one that is none of them. It is the input
/// [`App::footer`] did not take, and the reason it could not be on [`App`]: *which answer the open
/// pane came back with* is [`crate::ui::Screen`]'s half, not what the reader navigated to
/// (NOTES § D259 ruling 5).
///
/// **It answers one question — may a mutating key be offered right now — and three different facts
/// hide behind it** (PM ruling, 2026-09-12; `screens/widgets.md` § 2a, extended 2026-09-18). *There
/// is nothing to act on* is about the pane's content and moves frame to frame; *writes are off for
/// this whole run* is a property of the session ([`crate::ui::Writes`]); *this kind has no such
/// operation* is a property of the object under the cursor and of nothing else ([`Offer::Act`]).
/// They are separate values and this is where they meet, because they end in the same drawn line.
///
/// **Neither of them is a [`Refused`] mark, and that is the distinction this type exists to keep**
/// (`screens/widgets.md` § 2a, `screens/help.md` § *When a key is refused*): **withheld** is the
/// key not being on the line at all, **refused** is the key on the line with `no` before its label.
/// `no` is `may_i`'s answer about *this login's grant* — a dropped connection, an empty pane, a
/// dead audit log and a Node's missing `/scale` asked nobody, so marking a key `no` for any of them
/// would claim a verdict nobody gave. Only [`Offer::Act`] can carry a mark.
///
/// **One value, and the one shape inside it that varies per key is [`Offer::Act`]'s** — because
/// these are the closed set `screens/states.md` draws and nothing may invent another:
/// [`crate::ui::offered`] is the one place a screen becomes one of them, and [`App::may_mutate`] is
/// handed the same value, per key ([`Op`]) — so a key that is not on the line cannot be pressed
/// either, which is the bar `--read-only` is held to (invariant 2). Every other shape offers
/// neither key, which is why only `Act` carries the pair.
/// **`switch` is the one key this page promotes off `?`, and it is a second fact rather than a
/// fifth shape** (`screens/states.md` § Your login expired). Which keys the *pane* offers and
/// whether *switching cluster is the next step* are independent: the file draws their `Move`
/// combination and states the reason — *"so a reader does not have to hold `aws sso login` in
/// their head while hunting the key map"* — which is at its strongest on the two frames with
/// nothing else on them at all, where an expired login used to lose the key outright
/// (`k8s-admin`, 2026-09-12).
///
/// **The `Nothing` and `Filter` rows it produces were this box's composition first and are the
/// page's now** — § *Over a pane with nothing to show yet* draws both, with `X switch cluster`
/// where § *Your login expired* already put it, after the cursor keys and before `/ filter`. They
/// are literals rather than a built string, so every line this footer can draw is spelled at
/// compile time (NOTES § D259 ruling 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// **Nothing has arrived yet** — nothing to move a cursor across, open, or narrow
    /// (`screens/states.md` § Still loading).
    Nothing {
        /// `X switch cluster`, promoted onto the line.
        switch: bool,
    },
    /// **A list that came back with no rows.** `/ filter` stays — a pane-level control, not an
    /// operation on an object — and the cursor keys go with the rows
    /// (§ An empty kind in the browser). **Whichever [`Pane`] answer holds those rows**: a 403 on
    /// `list jobs` that came back with nothing is as empty as a `Ready` that did, and promising
    /// `⏎ open` over fourteen blank rows is the same broken promise either way (`k8s-admin`,
    /// 2026-09-12).
    Filter {
        /// `X switch cluster`, promoted onto the line.
        switch: bool,
    },
    /// **A list with rows in it that `/` or `n` narrowed to none** (`screens/states.md` § The
    /// filter hides every row). **Not [`Offer::Filter`]**: that one is a kind that genuinely has
    /// no objects, and the two say different things to the reader and offer different keys.
    ///
    /// **It is the one footer on this page that can afford `esc`'s own word**, and it affords it
    /// out of keys that have nothing to act on — **but not the same keys on both panes, which is
    /// what this doc said before [`Offer::Hidden::browsing`] was added under it** (`k8s-admin`,
    /// 2026-09-18; NOTES § D216 — nothing mechanical reads a comment). `s scale` and `r restart`
    /// go from both, because nothing is selected. `↑↓ move` and `⏎ open` go from the **browser
    /// only**; Alerts keeps them, and the field below carries the reason. `esc clear filter`
    /// arrives in the room that leaves, in the picker's own wording for the same state rather
    /// than a second vocabulary for it.
    Hidden {
        /// `X switch cluster`, promoted onto the line — **read by the browser's arm and not by
        /// Alerts'**, which has no room left for it (see the literals in [`App::footer`]).
        switch: bool,
        /// **`esc` is about to clear the namespace rather than the text** —
        /// [`Filters::clears`]'s answer, so this line names the field that key really empties.
        namespace: bool,
        /// **Which pane is empty, because the two do not offer the same keys**
        /// (`screens/states.md` § The filter hides every row, `k8s-admin`, 2026-09-18).
        ///
        /// Alerts keeps `↑↓ move` and `⏎ open` — § *Nothing is broken*'s own reasoning, that the
        /// sidebar's rows are still there to move across and that Alerts is where a reader lands;
        /// the browser drops them, § *An empty kind in the browser*'s own, that the cursor is
        /// already inside one kind's table and an empty table is nothing to move across. **A
        /// mistyped filter would otherwise leave the screen with the most wrong in the cluster
        /// offering fewer keys than the clean one.**
        browsing: bool,
    },
    /// **Rows to move across and open, and no mutating key** — the states `screens/widgets.md`
    /// § 2a's closed mode list groups as *ordinary, mutations withheld*: nothing is selected, the
    /// link is down or the login has expired, no time on the page can be trusted, or writes are
    /// off for this run. **It is also what Analysis and an open detail tab answer**, whose own
    /// footers name neither mutating key and must not leave one pressable behind them
    /// (PRIOR-ART § G2).
    Move {
        /// `X switch cluster`, promoted onto the line.
        switch: bool,
    },
    /// **The ordinary footer**, the one line a [`Refused`] mark can reach — and the only one
    /// [`App::may_mutate`] says yes to.
    ///
    /// **Which of `s` and `r` are on it is the selected object's *kind*, and that is a third cause
    /// of *withheld* beside the run-level ones above** (`screens/widgets.md` § 2a): a Node cannot
    /// be scaled by anyone and a DaemonSet has no `/scale` subresource. It is not a permission
    /// question — it is true whatever this login may do — so `may_i_in` is never asked and the key
    /// is never marked `no`; `s no scale` on a Node would claim a verdict nobody was asked to give
    /// (NOTES § D261 ruling 8). **The key is simply not on the line**, which is the move `c
    /// container` already makes on a single-container pod (`screens/detail.md` § Choosing a
    /// container). The two stay different shapes and never the same shape twice: refused is the
    /// ordinary line **plus** one word per key, unsupported is the ordinary line **minus** one key.
    ///
    /// **Both `false` is a real state and is not *nothing is selected***. A Node under the cursor
    /// is selected: `⏎ open` opens it and `ctrl-d` acts on it (`ops::delete` serves a node), and
    /// only `s` and `r` have nothing to reach. It draws the same line [`Offer::Move`] does, which
    /// is § 2a's own rule rather than a collision — every cause of *withheld* draws alike, and
    /// nothing on screen may tell one from another.
    ///
    /// **`?` is unchanged by any of this** (`screens/help.md`): Help is the exhaustive map of the
    /// product and lists `s` and `r` whatever is selected, so nothing here reaches [`Refused`]'s
    /// other reader.
    Act {
        /// `s` — [`crate::ops::scalable`] serves this kind.
        scalable: bool,
        /// `r` — [`crate::ops::restartable`] serves this kind.
        restartable: bool,
    },
}

/// **Whether there is anywhere for a reader to type a target count yet** — `false`, and that is the
/// whole of why `s scale` reaches no footer (`screens/help.md` § Rules and `screens/dialogs.md`
/// § Choosing how many, before the confirm box, both 2026-09-24).
///
/// **A named `false` rather than a deleted call**: the box that draws that step turns this into
/// `true` and nothing else moves, and one `grep` finds every consequence of the ruling.
const SCALE_IS_BUILT: bool = false;

/// **Which mutating key is being asked about.** The two had one answer until the kind became part
/// of it, and they no longer do: `s` is live on a Deployment and dead on a DaemonSet in the same
/// frame ([`Offer::Act`]).
///
/// **`ctrl-d delete` is the third arm, and it was not one until the key was bound** (2026-09-23,
/// the `main.rs` wiring box — which is the box this type's own doc said would add it). It is on
/// neither list footer — D259 took it off both for width — so nothing draws from it; what it
/// decides is whether the press reaches anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// `s scale`.
    ///
    /// **Nothing asks this yet, and the reason is a screen that has not been drawn**: a scale needs
    /// a target count, and no file in `screens/` says how the reader enters one — there is no
    /// mockup of the step between pressing `s` and the confirmation box, and [`Typing`] has no
    /// state for a number. So the key reaches nothing rather than inventing the step
    /// (`main.rs`'s router, 2026-09-23; reported to the PM as the ruling that unblocks it).
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "`screens/dialogs.md` § Choosing how many specifies the step and marks it \
                      *not built*; no box builds it yet ([`SCALE_IS_BUILT`])"
        )
    )]
    Scale,
    /// `r restart`.
    Restart,
    /// **`ctrl-d delete`, which every kind an [`Offer::Act`] can be about supports** —
    /// `ops::delete` serves all six including a node, so unlike the two above there is no kind
    /// question left for this one to ask. What is left is the run-level half every mutating key
    /// shares, and that is [`App::may_mutate`]'s own three conditions.
    ///
    /// **The kind gate still exists and it is the caller's**: `ops::delete` refuses a kind it does
    /// not serve, and `main.rs`'s driver resolves the word against its own `KINDS` before building
    /// a mutation at all — so a ConfigMap under the browser's cursor answers `true` here and
    /// reaches nothing, which is the same place a kind with no `/scale` reaches.
    Delete,
}

impl Offer {
    /// **The ordinary footer for the object under the cursor** — the one place the kind question is
    /// asked, so no second caller can answer it differently.
    ///
    /// **The answer is [`crate::ops::scalable`]'s and [`crate::ops::restartable`]'s, never a list
    /// written here** (invariant 12). Those two are what the operation itself matches on, so a
    /// footer offering `s` on a kind `ops::scale` then refuses would be a second opinion about the
    /// same three words — and the defect this box closed was exactly that, one step further on: a
    /// Node card offering `s scale`.
    ///
    /// **A kind is two fields and not one, and a manifest spells both** — `kind: Deployment` *and*
    /// `apiVersion: apps/v1` (NOTES § D51, PRIOR-ART § F4: *`pods` is not a key; `apps/v1
    /// deployments` is*). `kind` here is the singular word — `deployment`, never `deploy` and
    /// never `Deployment` (`screens/dialogs.md` § Scale) — and `group` is the half that says
    /// *whose* `deployment`. **It needs no CRD to matter**: a stock cluster serves `v1 Event` and
    /// `events.k8s.io/v1 Event`, two resources whose kind word is the same, and OpenKruise's
    /// `apps.kruise.io/v1beta1 StatefulSet` sits beside `apps/v1 StatefulSet` under one sidebar
    /// row reading `statefulsets` (`reports/2026-09-18-offer-per-kind-operator-read.md` § 1).
    ///
    /// **So the resource `ops` hands back has to be the one the reader is looking at**, and that
    /// is what the group comparison is: those two answer with an `ApiResource` of their own —
    /// `apps/v1`, the crate's declaration and not a literal — so a kind word that matches under a
    /// group that does not is refused here, with no second list of groups written down. A word
    /// neither serves, `""` included, is a kind with no mutating key on its line.
    ///
    /// **Their `Err` is a whole sentence and it is dropped here**, because this asks a yes/no
    /// question: the sentence is the headless driver's, printed on a line somebody typed.
    pub fn act(group: &str, kind: &str) -> Offer {
        Offer::Act {
            // **`s` is withheld here, for every kind, every login and every run**
            // ([`SCALE_IS_BUILT`], `screens/help.md` § Rules and `screens/widgets.md` § 2a: *"`s
            // scale` is never part of it: `s` is withheld from `Offer::Act` … so this line never
            // has it to curate away in the first place"*). Scale's confirm box has always assumed a
            // target count already exists — `--replicas=3` in every mockup — and the step that
            // produces one is `screens/dialogs.md` § Choosing how many, before the confirm box,
            // marked there **specified, not built**.
            //
            // **Withheld and not refused, which is the distinction this type exists to keep**
            // (`screens/help.md` § When a key is refused): the key is off the line entirely, the
            // way a kind with no `/scale` already takes it off, so no verdict about it is claimed
            // and `?` says *not built yet* instead.
            //
            // **`crate::ops::scalable` is still asked** — it is the answer this goes back to when
            // the count step lands, and one word turns it back on.
            scalable: SCALE_IS_BUILT
                && crate::ops::scalable(kind).is_ok_and(|served| served.group == group),
            restartable: crate::ops::restartable(kind).is_ok_and(|served| served.group == group),
        }
    }

    /// **Whether this line carries that key** — [`App::may_mutate`]'s last condition, and the whole
    /// of *a key that is not on the line cannot be pressed either*.
    ///
    /// **Both enums are matched exhaustively and neither has a catch-all**, which is not tidiness:
    /// the shapes are listed so a new [`Offer`] is a compile error here, and the ops are listed
    /// *inside* that arm so a new [`Op`] is one too. A `_` over the pair took both — it was safe
    /// for a new `Offer`, which would fall through to *withheld*, and silently wrong for a new
    /// `Op`, which would be withheld **everywhere** with a green build while [`Op`]'s own doc
    /// promised the opposite (`k8s-admin`, 2026-09-18).
    fn offers(self, op: Op) -> bool {
        match self {
            Offer::Act {
                scalable,
                restartable,
            } => match op {
                Op::Scale => scalable,
                Op::Restart => restartable,
                // **Every kind that can be an `Act` can be deleted** ([`Op::Delete`]) — the pair
                // above is about the two subresource-shaped operations and says nothing about this
                // one, which is why reading it for `ctrl-d` withheld the key on the one kind
                // `ops::delete` was written for (`main.rs`'s router, 2026-09-23).
                Op::Delete => true,
            },
            Offer::Nothing { .. }
            | Offer::Filter { .. }
            | Offer::Hidden { .. }
            | Offer::Move { .. } => false,
        }
    }
}

impl App {
    /// **`q` — refused while a write is in flight, and not a command at all while a filter is
    /// being typed** (NOTES § D12, `screens/dialogs.md` § *While the call is running*,
    /// `screens/widgets.md` § 2b). Quitting mid-`PATCH` would leave the audit log holding an
    /// attempt with no result; `q` typed into a filter is the letter `q`, which is why the typing
    /// footer names `⏎` and `esc` and nothing else.
    ///
    /// **The two are not the same answer wearing one word.** A write in flight *refuses* the key
    /// and says so; typing has not refused anything, it has taken the whole keyboard as text —
    /// which is why the caller asks this before routing a key rather than drawing anything from
    /// it.
    pub fn may_quit(&self) -> bool {
        self.changing.is_none() && self.typing.is_none()
    }

    /// **`X` — unbound while a modal is open, while a write is in flight, and while a filter is
    /// being typed** (NOTES § D12, § D16, `screens/widgets.md` § 2b). Switching clusters under an
    /// open confirmation is how a dialog ends up naming an object on a cluster it was never read
    /// from; `X` is also an ordinary capital letter, and a reader typing one into `/` is not asking
    /// to leave the cluster.
    pub fn may_switch_cluster(&self) -> bool {
        self.modal.is_none() && self.changing.is_none() && self.typing.is_none()
    }

    /// **The buffer `/` or `n` has focus on, or `None` while nothing is being typed** — the one
    /// place [`Self::typing`] becomes a field, so the footer's label, `esc`'s word and the keys
    /// that edit it cannot pick different halves of [`Filters`] (`screens/widgets.md` § 2b).
    pub fn typed(&self) -> Option<&Input> {
        match self.typing? {
            Typing::Text => Some(&self.filters.text),
            Typing::Namespace => Some(&self.filters.namespace),
        }
    }

    /// [`Self::typed`], for the keys that edit it — every printable one through [`Input::push`]
    /// and `⌫` through [`Input::pop`].
    pub fn typed_mut(&mut self) -> Option<&mut Input> {
        match self.typing? {
            Typing::Text => Some(&mut self.filters.text),
            Typing::Namespace => Some(&mut self.filters.namespace),
        }
    }

    /// **A switch was made, and nothing the reader did on the old cluster survives it in [`App`]**
    /// (`screens/context.md` § What happens on `⏎` step 5; NOTES § D16 ruling 3). The view is
    /// Alerts again — the old sidebar's indices mean nothing on the new cluster — and every cursor,
    /// filter, tab and scroll is gone.
    ///
    /// **[`Log`] is not emptied here, and that is a correction rather than an omission**
    /// (NOTES § D280 item 2). It was, on `⏎`, before the outcome was known — so the strip drew
    /// blank for as long as `connect_with` was out, on every switch, and a switch that then failed
    /// had nothing to put there at all. `screens/context.md` § When the new cluster does not work
    /// rules that the strip **keeps what was already there** until the new context has connected:
    /// `sent` is `false` for every fault that box can draw, so no line about the new context was
    /// ever true, and inventing one is the outcome-word fiction `screens/dialogs.md` § The object
    /// went away already refused. The caller empties it on success, beside the new context's first
    /// line.
    ///
    /// **It is not the whole of a switch.** The session and any open detail stream are the
    /// caller's — `ui::Screen::detail` is not a field here — and Phase 12's wiring drops them
    /// beside this call (NOTES § D264 ruling 13).
    ///
    /// Called on `⏎`, before the connection answers: a switch that then fails stays on the context
    /// that was chosen, and so does this.
    pub fn switched(&mut self) {
        *self = App::default();
    }

    /// **Nothing has connected behind what is open** — the startup picker, or the failure it led to
    /// (`screens/context.md` § Opening at startup: *behind the modal, genuinely nothing*). The
    /// frame then draws no sidebar, no pane and no vitals.
    pub fn connecting_first(&self) -> bool {
        match &self.modal {
            Some(
                Modal::ContextPick(picker)
                | Modal::Unconnected {
                    before: Before::Picking(picker),
                    ..
                },
            ) => picker.startup(),
            _ => false,
        }
    }

    /// **A second mutation is refused while one is running** (`screens/dialogs.md` § *While the
    /// call is running*). Navigation stays free; this is only about opening another dialog.
    ///
    /// **A mutating key is live exactly where the footer draws it** (`screens/states.md`, NOTES
    /// § D21): [`Offer::Act`] is the one shape whose line can carry `s` or `r` at all, so this is
    /// handed the same value [`crate::ui::offered`] gave the footer rather than re-deriving one.
    /// **Which of the two that line actually carries is `Act`'s own pair** — a Node is an `Act`
    /// with neither, and it is still an `Act` because an object *is* selected. A dead
    /// audit log and an empty pane both land here as *not `Act`*, which is invariant 2's
    /// *unreachable, not merely unbound* — the bar `--read-only` is held to — instead of a banner
    /// over live keys.
    /// **And no key is a command while a filter is being typed** (`screens/widgets.md` § 2b):
    /// `s` and `r` are letters a filter can legitimately hold, the typing footer names neither,
    /// and *a key that is not on the line cannot be pressed either* is this method's whole job.
    ///
    /// **It is asked per key, because the two keys no longer have one answer** ([`Op`]): an
    /// `Offer::Act` on a DaemonSet draws `r restart` and no `s`, and a press of `s` there must
    /// reach nothing. One question and one answer — a caller that asked this and then narrowed it
    /// with a second condition of its own would be the second place a key's liveness is decided.
    pub fn may_mutate(&self, offer: Offer, op: Op) -> bool {
        self.modal.is_none() && self.changing.is_none() && self.typing.is_none() && offer.offers(op)
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
    /// of the nine fixed strings `screens/widgets.md` § 2a counts across its two tables, never a
    /// string built here. **A mark reaches only a key the selected kind actually has**
    /// ([`Offer::Act`]): a key that is off the line was never asked about and has no verdict to
    /// draw, so a refusal carried here for it changes nothing — which is what keeps *refused* and
    /// *unsupported* two shapes rather than one. `ctrl-d` is not on this line to mark, and Analysis
    /// and the detail tabs name neither `s` nor `r`, so a refusal cannot show on them.
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
    ///
    /// **`offer` is `screens/states.md`'s own input and it reaches one arm, the same way `refused`
    /// does** ([`Offer`]). The lines under it are `Nothing`, `Filter` and `Move` each with and
    /// without `X switch cluster` and `Hidden` in its six, byte for byte, as literals — the same
    /// reason the rows below them are nine (NOTES § D259 ruling 4): the one footer these states can
    /// reach still spells every state of itself at compile time. Analysis
    /// and the detail tabs draw their own closed lines above this, which is `screens/widgets.md`
    /// § 2a's closed mode list; the `Offer` they are handed still decides `App::may_mutate`.
    ///
    /// **`contexts` is the picker's list, handed in for `detail`'s reason** — which rows it holds
    /// is the caller's, and whether `⏎` does anything on the picker depends on them
    /// ([`Picker::inert`]). Empty everywhere else, and read by that one arm.
    pub fn footer(
        &self,
        open: Detailing,
        offer: Offer,
        refused: Refused,
        cut: &str,
        contexts: &[Choice],
    ) -> (Cow<'static, str>, &'static str) {
        // **Typing answers before every other arm, because while a filter has focus the ordinary
        // key set is not valid at all** (`screens/widgets.md` § 2b — *the footer is fully
        // replaced, not curated*). It is the in-flight line's move for the opposite reason: there
        // the keys are real and inactionable, here they are not keys.
        //
        // **The combination with any arm below is unreachable rather than merely undrawn**, and
        // this is stated for the reason the modal arms below state the same thing: `?`, `X`, `s`
        // and `⏎`-into-Detail are all characters while a filter has focus, so nothing can open
        // over a typing session — and if Phase 12's wiring ever slips, the honest line to draw is
        // the one that says how to get out of the mode the reader is in.
        if let Some(field) = self.typing {
            // **`esc`'s second word follows the field with focus, and drops to `cancel` once that
            // field's own buffer is empty** — the picker's own rule, on the field the reader is
            // looking at rather than on a global order (NOTES § D264 rulings 27 and 31).
            let leave = match self.typed().is_some_and(Input::is_empty) {
                true => "cancel".to_owned(),
                false => format!("clear {}", field.label()),
            };
            return (
                Cow::Owned(format!("{}: {cut}  ⏎ done  esc {leave}", field.prompt())),
                "",
            );
        }
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
                let keys = match dialog.armed() {
                    true => Cow::Owned(format!("⏎ {}  esc cancel", dialog.confirm())),
                    // **No verdict yet, so neither key is live** — the dry-run is a real round
                    // trip for a scale and a restart, and `esc` is inert until it answers
                    // ([`Dialog::waiting`], NOTES § D214). `delete` never sits here: it sends no
                    // check, so its verdict is `Some` from the first frame (NOTES § D225
                    // ruling 1).
                    //
                    // **Asked before the typed-name arm and not beside it**, so the one dialog
                    // that could hold both — a check on the wire *and* a name to type, which
                    // `drain` is in v0.2 — cannot offer `esc cancel` on a press [`App::escape`]
                    // would refuse. The pair `(false, false)` this replaced reached the same arm
                    // by arithmetic: an unarmed dialog with no name field can only be a waiting
                    // one. It said nothing about a dialog that has both.
                    false if dialog.waiting() => Cow::Borrowed("waiting for the cluster"),
                    false => Cow::Borrowed("type the name to enable  esc cancel"),
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
            // **The picker's closed set, both ways** (`screens/widgets.md` § 2a's mode list,
            // `screens/context.md`): `esc` reads `quit` at startup, where there is no cluster
            // behind it to cancel back onto, and `⏎` is dropped — not dimmed — where it would do
            // nothing (NOTES § D264 ruling 2) — and `↑↓` with it where no row can be landed on at
            // all, a list that shows none included (rulings 16 and 18). `esc`'s word is the box
            // button's own, `clear filter` while the filter holds text ([`Picker::verbs`],
            // ruling 31).
            //
            // **`type to filter` and not `/ filter`, because there is no such key here**
            // (`screens/context.md` § The picker, rewritten 2026-09-24: *"There is no dedicated key
            // that opens it, unlike `/` on Alerts and Resources … every printable character — `/`
            // included, for an ARN context name's own tail — narrows the list the instant it is
            // typed, and the footer says `type to filter` rather than naming a key that does not
            // exist"*). A footer that named `/` would name a key the box does not have, which is
            // `screens/widgets.md` § 2a's own closed-set rule read the other way round.
            Some(Modal::ContextPick(picker)) => {
                let (go, leave) = picker.verbs();
                let keys = if picker.nowhere(contexts) {
                    format!("type to filter  esc {leave}")
                } else if picker.inert(contexts) {
                    format!("↑↓ move  type to filter  esc {leave}")
                } else {
                    format!("↑↓ move  type to filter  ⏎ {go}  esc {leave}")
                };
                return (Cow::Owned(keys), "");
            }
            // **The container picker's closed set** (`screens/detail.md` § Choosing a container).
            // Every row can be landed on — a container is always pickable — so `⏎` is never inert
            // here and there is no `/` on this box to clear.
            Some(Modal::ContainerPick(_)) if picking(open) => {
                return (Cow::Borrowed("↑↓ move  ⏎ pick  esc cancel"), "");
            }
            // **Nothing left to pick, so the box is not drawn and it offers nothing** — the pod
            // went away under the open picker, or its snapshot has not landed yet (§ The pod
            // disappears while the picker is open, § The logs tab, before the container list is
            // known). What the reader is looking at is the logs tab, so the logs tab's own footer
            // is what falls through below. **No second way out is named**: `esc` closes this modal
            // exactly as it closes the tab, which is what makes one word honest for both.
            Some(Modal::ContainerPick(_)) => {}
            // **`esc` alone, and `X` is not on it** — `X` cannot fire under a modal (NOTES § D16
            // ruling 1), so the body's *"X takes you back"* is read after dismissing.
            Some(Modal::Unconnected { before, .. }) => {
                return (Cow::Owned(format!("esc {}", before.leave())), "");
            }
            None => {}
        }
        // **Exhaustive on both enums on purpose**: a fifth tab or a fourth view is a compile
        // error here rather than a screen that quietly draws the wrong keys.
        let keys = match (open, self.tab, self.view) {
            // **The which-pods step's closed set, and it is the Analysis line word for word**
            // (`screens/detail.md` § Picking a pod: *"four words this product already uses …
            // assembled, not invented"*). **No `/` filter**, which that section refuses by name:
            // a picker over one card's own pods is already narrower than the list D3 was written
            // to shrink. The tab is not on it either — there is no tab to be on until a pod is
            // chosen.
            (Detailing::Pods, _, _) => "↑↓ move  ⏎ open  esc back  ? all keys  q quit",
            // **`c container` is on this line only where there is more than one answer**
            // (`screens/detail.md` § Choosing a container: *a key that does nothing is a bug
            // already shipped once here*). One container, and a pod whose snapshot has not
            // reached the store yet, draw the same line — k8rs does not know of a second
            // container in either case, and guessing is what the header's own vitals refuse.
            (Detailing::Tabs { .. }, Tab::Logs, _) if picking(open) => {
                "[ ] tabs  f follow  c container  esc back  ? all keys  q quit"
            }
            (Detailing::Tabs { .. }, Tab::Logs, _) => {
                "[ ] tabs  f follow  esc back  ? all keys  q quit"
            }
            (Detailing::Tabs { .. }, Tab::Describe | Tab::Yaml | Tab::Events, _) => {
                "[ ] tabs  esc back  ? all keys  q quit"
            }
            (Detailing::Closed, _, View::Analysis(_)) => {
                "↑↓ move  ⏎ open  esc back  ? all keys  q quit"
            }
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
            (Detailing::Closed, _, View::Alerts | View::Resources(_))
                if self.changing.is_some() =>
            {
                return (
                    Cow::Owned(format!("↑↓ move  ⏎ open  ? keys  ·  changing {cut} first")),
                    "",
                );
            }
            // **The lines `screens/states.md` draws, as literals** ([`Offer`]).
            (Detailing::Closed, _, View::Alerts | View::Resources(_)) => match offer {
                // **`s` and `r` are on none of the lines before [`Offer::Act`] and are never
                // marked `no` on one** ([`Offer`]): nothing here asked `may_i` anything.
                Offer::Nothing { switch: false } => "? all keys  q quit",
                Offer::Nothing { switch: true } => "X switch cluster  ? all keys  q quit",
                Offer::Filter { switch: false } => "/ filter  ? all keys  q quit",
                Offer::Filter { switch: true } => "X switch cluster  / filter  ? all keys  q quit",
                // **Six more literals** (`screens/states.md` § The filter hides every row). Two
                // things vary and a third is deliberately absent. `esc`'s own word follows the
                // field it is about to clear — the picker's wording for this state, not a second
                // vocabulary. **Alerts keeps the cursor keys and the browser drops them**, each
                // following its own zero-row precedent ([`Offer::Hidden::browsing`]).
                //
                // **And `X switch cluster` is not on Alerts' two** — `screens/states.md` § The
                // filter hides every row carries both the arithmetic and the reason, and this
                // file keeps no second copy of either (CLAUDE.md § A decision is written once).
                Offer::Hidden {
                    namespace: false,
                    browsing: false,
                    ..
                } => "↑↓ move  ⏎ open  / filter  esc clear filter  ? all keys  q quit",
                Offer::Hidden {
                    namespace: true,
                    browsing: false,
                    ..
                } => "↑↓ move  ⏎ open  / filter  esc clear namespace  ? all keys  q quit",
                Offer::Hidden {
                    switch: false,
                    namespace: false,
                    browsing: true,
                } => "/ filter  esc clear filter  ? all keys  q quit",
                Offer::Hidden {
                    switch: true,
                    namespace: false,
                    browsing: true,
                } => "X switch cluster  / filter  esc clear filter  ? all keys  q quit",
                Offer::Hidden {
                    switch: false,
                    namespace: true,
                    browsing: true,
                } => "/ filter  esc clear namespace  ? all keys  q quit",
                Offer::Hidden {
                    switch: true,
                    namespace: true,
                    browsing: true,
                } => "X switch cluster  / filter  esc clear namespace  ? all keys  q quit",
                Offer::Move { switch: false } => "↑↓ move  ⏎ open  / filter  ? all keys  q quit",
                Offer::Move { switch: true } => {
                    "↑↓ move  ⏎ open  X switch cluster  / filter  ? all keys  q quit"
                }
                // **Nine literals, of which [`SCALE_IS_BUILT`] currently makes four unreachable**
                // (NOTES § D259 ruling 4): the one footer a refusal can reach still spells every
                // state of itself at compile time. `screens/widgets.md` § 2a draws eight states
                // today and none of them carries `s` — the four below that do are the rows that
                // come back the day the count step lands, and they are kept *here*, beside the
                // constant that suppresses them, so that flipping it is one edit and cannot leave a
                // key live on a line that does not name it.
                Offer::Act {
                    scalable: true,
                    restartable: true,
                } => match (refused.scale(), refused.restart()) {
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
                },
                // **A kind with only one of them is a shorter line, never a `no` on the one it
                // lacks** ([`Offer::Act`]): a bare ReplicaSet scales and does not restart, a
                // DaemonSet restarts and does not scale. The missing key was never asked about, so
                // there is no verdict to draw and the refusal that is still readable here is the
                // one belonging to the key that is on the line.
                Offer::Act {
                    scalable: true,
                    restartable: false,
                } => match refused.scale() {
                    false => "↑↓ move  ⏎ open  s scale  / filter  ? all keys  q quit",
                    true => "↑↓ move  ⏎ open  s no scale  / filter  ? all keys  q quit",
                },
                Offer::Act {
                    scalable: false,
                    restartable: true,
                } => match refused.restart() {
                    false => "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
                    true => "↑↓ move  ⏎ open  r no restart  / filter  ? all keys  q quit",
                },
                // **Neither, and the line is [`Offer::Move`]'s own** — a Node, a bare Pod, a
                // ConfigMap, which is most of what the browser can reach. Two states drawing one
                // line is what § 2a asks for and not a collision: every cause of *withheld* draws
                // alike.
                Offer::Act {
                    scalable: false,
                    restartable: false,
                } => "↑↓ move  ⏎ open  / filter  ? all keys  q quit",
            },
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

    /// **`esc` — closes exactly one level** (`screens/widgets.md` § 5, and those are its words).
    /// With no modal open it backs out of a filter.
    ///
    /// **That section says *always* and then names the one exception itself** — a confirmation
    /// whose dry-run has not answered ([`Dialog::waiting`]), which it points at
    /// `screens/dialogs.md` § While the check is still on the wire for, over NOTES § D214. The
    /// footer this file draws beside that box has named no key since `1f687fc`, so the two halves
    /// of the screen finally say the same thing. **It is not a modal trapping the reader**: the
    /// check is a real round trip and it ends, the box arms, and the same press cancels it — what
    /// is refused is closing a confirmation while the cluster is still being asked whether it
    /// would be allowed.
    ///
    /// **The two filters are two levels, so one press clears one of them** (NOTES § D246). **Text
    /// first because it is the inner one** — `/` narrows whatever `n` already scoped, so backing
    /// out goes narrow to wide, which is what *one level* means for two filters that nest. Scoped
    /// to `n payments` and narrowed with `/web`, one `esc` used to jump all the way back to every
    /// namespace in the cluster: one press undoing two.
    ///
    /// **The picker is the one modal with a level inside it, and the one place `esc` can end the
    /// run** (`screens/context.md`). A typed `/` is the inner level and goes first — the same
    /// narrow-to-wide rule as the two filters, and at startup the difference between clearing a
    /// typo and quitting. With nothing typed, `esc` on the startup picker **quits**: nothing is
    /// connected to go back to, so the answer is `true` and the modal is left as it was for the
    /// loop to exit over. `esc` on the startup failure reopens the picker it came from — two
    /// presses, never a dead end.
    /// **While a filter is being typed `esc` acts on the field that has focus, and that is a
    /// different rule from the one above rather than an exception to it** (`screens/widgets.md`
    /// § 2b). The buffer `/` or `n` opened is not empty → it is emptied, the list re-widens and
    /// typing stays open on it; it is already empty → typing closes and the *other* field is left
    /// exactly as it was. The reader is looking at one field and `esc`'s own word beside it names
    /// that one; a global order here would clear a field that is not on screen.
    /// **A detail tab open over the list is `esc back`, and it clears no filter** — the filter is
    /// exactly what `screens/widgets.md` § 2b draws surviving that press, because Detail is drawn
    /// *over* a view and going back is not going somewhere else. `containers` is that fact, the
    /// same value [`App::footer`] is handed and for the same reason: *whether* a tab is open is
    /// `crate::ui::Screen::detail`'s and this file cannot see it (`k8s-admin`, 2026-09-18 — this
    /// method had the `None`-modal arm and not the fact, so `esc` out of Detail cleared the filter
    /// the page promises survives it).
    ///
    /// **A container picker with nothing left to pick is not a level, and this is where it stops
    /// existing.** The box is not drawn once the pod's containers have gone
    /// (`screens/detail.md` § The pod disappears while the picker is open) and the footer under it
    /// is the logs tab's — so a press this method spent closing an invisible modal was a key that
    /// visibly did nothing, which is the defect that whole section exists to avoid. It is dropped
    /// here rather than counted, and the press goes on to do what the drawn footer promises.
    ///
    /// **What this method cannot tell the caller afterwards, and the caller must therefore ask
    /// before pressing** (`k8s-admin`, 2026-09-18). Both rows leave [`App::modal`] `None`:
    ///
    /// - a **drawn** picker was cancelled — its footer said `esc cancel`, so the detail tab
    ///   behind it stays open;
    /// - an **undrawn** one was dropped — the footer the reader was looking at was the logs tab's
    ///   own `esc back`, so the tab closes.
    ///
    /// `modal.is_some()` before the call is true in both, so it is not the discriminator either.
    /// [`picking`] is, and it is `pub` for exactly this: **ask it before the press**, and close
    /// the detail tab only where it answered `false`. A caller that read `modal` instead closes
    /// the tab out from under a picker the reader had just cancelled.
    #[must_use = "`true` is the startup picker's `esc`, which ends the run"]
    pub fn escape(&mut self, open: Detailing) -> bool {
        if matches!(self.modal, Some(Modal::ContainerPick(_))) && !picking(open) {
            self.modal = None;
        }
        if self.typing.is_some() {
            match self.typed_mut() {
                Some(buffer) if !buffer.is_empty() => buffer.clear(),
                _ => self.typing = None,
            }
            return false;
        }
        match self.modal.take() {
            Some(Modal::ContextPick(mut picker)) if !picker.filter.is_empty() => {
                picker.filter.clear();
                self.modal = Some(Modal::ContextPick(picker));
            }
            Some(Modal::ContextPick(picker)) if picker.startup() => {
                self.modal = Some(Modal::ContextPick(picker));
                return true;
            }
            Some(Modal::Unconnected {
                before: Before::Picking(picker),
                ..
            }) => self.modal = Some(Modal::ContextPick(picker)),
            // **A confirmation whose check has not answered is the one modal `esc` does not
            // close** ([`Dialog::waiting`], `screens/dialogs.md` § The verdict line, NOTES
            // § D214). Its footer names no key at all, and this arm was taking the dialog and
            // dropping it anyway — the reader pressed the key the box did not offer and the box
            // went, with the dry-run still on the wire behind it. Put back, not left `None`.
            Some(Modal::Confirm(dialog)) if dialog.waiting() => {
                self.modal = Some(Modal::Confirm(dialog));
            }
            Some(_) => {}
            // **A detail tab is open, so this press is `esc back` and the filters are untouched**
            // (`screens/widgets.md` § 2b). The caller closes the tab; `App` holds no field for it.
            // **The which-pods step is the same press for the same reason** — it is drawn over the
            // view, not instead of one, so `esc` goes back to a list whose filter is still the one
            // the reader typed (`screens/detail.md` § Picking a pod).
            None if open != Detailing::Closed => {}
            // **Narrow to wide, and the order is [`Filters::clears`]'s so the footer that names
            // the field and the key that empties it are one answer** (`screens/states.md` § The
            // filter hides every row).
            None => match self.filters.clears() {
                Some(Typing::Text) => self.filters.text.clear(),
                Some(Typing::Namespace) => self.filters.namespace.clear(),
                None => {}
            },
        }
        false
    }

    /// **`⏎` on an Alerts card or a marked browser row — the which-pods step, or the object
    /// itself** (`screens/detail.md` § A group of one pod, or none at all; NOTES § D270).
    ///
    /// **The predicate is here and not in the key router** for [`picking`]'s own reason: one
    /// predicate, several readers — the Alerts card and the browser's own `⏎ to see` line reach
    /// the same step — and a router that re-derives it is D103's second copy. A node card
    /// (`affected == 0`) rendered a step with **zero rows** under a footer promising `↑↓ move  ⏎
    /// open`, which is *"a key that does nothing is a bug already shipped once here"*, the rule
    /// that page itself cites (`tester`, 2026-09-18).
    ///
    /// **Both halves of [`Card::count`]'s `None` and the group of one**: a node card counts no
    /// pods, a bare pod's card is the pod itself, and `affected == 1` leaves one candidate — all
    /// three open the object directly and none of them is a group.
    ///
    /// **The cursor is reset in the same call, which is the whole reason this is a method.** It is
    /// the only cursor on this product with no reset rule of its own: [`App::open`] resets
    /// [`App::content`] on a view change, and nothing resets [`App::pods`] between two cards of
    /// one view. Landing on row 7 of card A, `esc`, then opening card B left [`Cursor::follow`]
    /// missing A's anchor and falling back to `select(self.index)` — **row 7 of a different
    /// group, with `⏎` armed on it** (`tester`, 2026-09-18, measured).
    #[must_use = "the caller opens the step on `true` and the card's own object on `false`"]
    pub fn pick_pods(&mut self, card: &Card) -> bool {
        let group = card.count().is_some() && card.affected >= 2;
        if group {
            self.pods = Cursor::default();
        }
        group
    }

    /// **`⏎` on the sidebar** — a group opens or closes, anything else becomes the view.
    ///
    /// **The content cursor is reset only when the view actually changed**, which is not the same
    /// as *a key was pressed*. Expanding a sidebar group leaves the reader's place in the table
    /// they were reading — the group is a disclosure triangle, not a destination — and re-opening
    /// the kind already open leaves it too. Resetting on every press was this method's first
    /// draft and it threw away the cursor of the pane it had not touched.
    ///
    /// **Both filters go with the cursor, and for the same reason** (`screens/widgets.md` § 2b):
    /// `web` typed for Alerts' cards means nothing against a ConfigMap table, and `pay` typed for
    /// one kind's rows is not a claim about the next kind the sidebar opens. **A list drawn *over*
    /// another one is not this** — Detail, Help, every dialog and the container picker keep both
    /// fields, because none of them is a different list and none of them comes through here.
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
            self.filters = Filters::default();
            // **The session goes with the buffer it was typing into.** Leaving it open would put
            // focus on a field that had just been emptied under it, on a list it was never about
            // — and every printable key would still be text, on a screen whose footer says
            // otherwise (`k8s-admin`, 2026-09-18).
            self.typing = None;
        }
    }

    /// **A manual scroll, which turns follow mode off** — the two halves are one action and are
    /// written once so no key handler can do the first and forget the second
    /// (`screens/widgets.md` § 4).
    ///
    /// **It moves from the row the last frame drew and not from a number nobody was looking at**,
    /// which is [`App::scroll`]'s write-back and is what makes the first `k` out of follow the
    /// line above the one the reader was watching. **`f` off is the same fact from the other end
    /// and the key handler gets it for nothing**: the offset already *is* the tail
    /// (`screens/widgets.md` § 4, *"follow mode (`f`) pins the offset to the bottom"*), so
    /// clearing [`App::following`] leaves the pane where it stood rather than somewhere else —
    /// the row `screens/detail.md` § When the buffer fills leaves unnamed when it says turning
    /// follow off freezes the *view* and not the stream under it.
    /// The toggle itself is the `main.rs` wiring box's; there is no method for it here yet.
    ///
    /// **It moves the open tab's own offset and never a shared one** ([`App::scroll`],
    /// `screens/widgets.md` § 4): which of the four is [`App::tab`]'s answer, read here rather
    /// than passed in, so a key handler cannot scroll one tab's body by another's number.
    pub fn scroll_by(&mut self, lines: i16) {
        self.following = false;
        let at = self.tab.at();
        self.scroll[at] = self.scroll[at].saturating_add_signed(lines);
    }

    /// **Every free-text offset back to the top of its own tab** — a resize, and the detail slot
    /// closing and opening again (`screens/widgets.md` § 4, `screens/detail.md` § Switching tabs
    /// keeps your place, which says what this does *not* cover: `[` and `]`).
    ///
    /// **It is the key handler's because neither cause is visible here.** A resize is a terminal
    /// event and *the slot was closed* is `crate::ui::Screen::detail`'s, so this file can only
    /// offer the one operation both of them need — and offering it once is what keeps the two
    /// callers from each zeroing a different subset of the four.
    ///
    /// **[`Self::following`] is deliberately untouched**: a followed pane ignores its stored
    /// offset every frame, so a reader who re-opens a log lands pinned to the tail rather than at
    /// a zero pretending to be one (that section's last paragraph).
    pub fn rewound(&mut self) {
        self.scroll = [0; Tab::ALL.len()];
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
    match restart_count(status) {
        counted if counted.is_empty() => counted,
        counted => format!(", {counted}"),
    }
}

/// **`3 restarts`, or nothing at all** — the same count [`restarts`] glues onto the end of a
/// sentence, without the comma that glues it there.
///
/// **One source and two spellings, rather than two counts.** The container picker draws this in a
/// column of its own (`screens/detail.md` § Choosing a container) — `running        3 restarts` —
/// where a leading comma is punctuation attached to nothing, and describe draws it at the end of a
/// line where it is not.
pub fn restart_count(status: Option<&ContainerSnapshot>) -> String {
    match status
        .map(|container| usize::try_from(container.restarts).unwrap_or(0))
        .unwrap_or(0)
    {
        0 => String::new(),
        1 => "1 restart".to_owned(),
        counted => format!("{counted} restarts"),
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

// --- WHY A CALL DID NOT WORK START ---
//
// **The words for a failed call, moved down out of the driver whole** (NOTES § D264 ruling 1,
// the move NOTES § D254 made for the detail tabs). `main.rs`'s `--once` and `--live` and the
// picker's failure box are consumers of one vocabulary, and a second set of sentences for the same
// eleven faults sent a namespace-scoped developer to ask for `default` — so the box draws these,
// byte for byte what the driver prints.
//
// **Wording only**, as the region above it: how the sentences are laid out — `What happened:` on
// stderr, one flowing paragraph in a 54-column box — is each surface's.

/// **`--namespace`, the flag a next step below tells the reader to type** — the driver's own
/// spelling, moved with the sentences that name it.
pub const NAMESPACE: &str = "--namespace";

/// **Which context k8rs connects to** — the console's (`crate::main`'s `opening`) and, on the
/// temporary driver, `--live`'s and `--once`'s. One spelling for both, read by one parser
/// (`crate::main`'s `context_arg`).
///
/// **It lives here for [`NAMESPACE`]'s reason** (NOTES § D264 ruling 14): [`run_the_login`] names
/// it in a sentence `ui.rs` draws, and `ui.rs` cannot reach `main.rs` — so a copy there would be
/// the second spelling of a flag, which is how a next step comes to teach a flag the parser no
/// longer has. CLAUDE.md's flag count already greps this file.
///
/// **Released and not scaffolding, since the flags box** (todo.md § Phase 12). It arrived in the
/// driver early, for `--live`, because the machine that runs the reconnect proof does not have to
/// be the machine whose current context is the test cluster; what that bought was the muscle memory
/// already being right when the console came to read it.
///
/// **What it picks is not only the connection**: `k8s::contexts` is handed the same value, so the
/// `current` row the header's TLS warning is read off is the row k8rs is actually on
/// (`crate::ui::Screen::insecure`, NOTES § D265 ruling 8). Reading one and not the other is the
/// disagreement NOTES § D174 closed one door over.
pub const CONTEXT: &str = "--context";

/// **What a connection that sent nothing was trying to do** — a `k8s::NotConnected`, whose client
/// was never built, framed for [`because`] (NOTES § D264 ruling 1).
pub const REACH: &str = "reach this cluster";

/// **What a watch asks for, in the words a `Role` spells** — `` `list` and `watch` pods `` — framed
/// for [`because`]. `resource` is the API's own plural.
pub fn watching(resource: &str) -> String {
    format!("`list` and `watch` {resource}")
}

/// **Strip the characters that have no printed form out of a string that came from outside
/// this file** — the guard invariant 9 owes every printer, and this is the first one
/// (`screens/widgets.md` § 7).
///
/// `println!` has no ratatui between it and the terminal, so an escape sequence in a pod name
/// arrives as an escape sequence and rewrites the user's screen.
///
/// **What counts as such a character is [`k8s::unprintable`](crate::k8s::unprintable)'s answer and
/// is not restated here** (NOTES § D154, CLAUDE.md § Single point of change). `main.rs` carried its
/// own narrower spelling until 2026-08-22, and the day the ingest guard widened and this one did
/// not, `k8rs some-pod.json` printed a row that reads *prodcd* for a pod named
/// `prod\u{202e}dc` — the hole is this path's alone, because it builds its snapshot off
/// `rules.rs`'s `From` impls and never meets [`k8s::Store`](crate::k8s::Store). **A second
/// spelling is what the fix refuses**: the two files are modules of one crate, so this one calls
/// the predicate.
///
/// **Removed, not replaced.** "Stripped" is the word in both invariant 9 and § 7, and a
/// substituted space is a character the API did not send — a second lie in the record
/// invariant 4 says may not lie. **Nothing is truncated here either**: the multi-byte path is
/// where `String::truncate` panics, and § 7 forbids it outright.
///
/// **Where it is applied is the whole rule, and it is mechanical: on a value as it enters a
/// message, never on the finished message.** A `\n` *from the cluster* still dies here, and
/// must — it would forge a second card. Phase 5's ingest strip supersedes this by applying the
/// same rule one layer earlier, cleaning the text as it arrives.
///
/// **It is a no-op on anything `k8s::text` produced, and it is the only strip several live inputs
/// ever meet.** Two claims, both measured, and neither of them is *the live path does not need
/// this* — a first draft of this paragraph said that and it was false
/// (`k8s-admin`, 2026-08-31).
///
/// **No-op on ingested text**: [`k8s::text`](crate::k8s::text) removes or substitutes for every
/// character [`k8s::unprintable`](crate::k8s::unprintable) answers for, so a value that came off
/// the API holds nothing left for a second pass to find — 18 717 strings of every committed
/// capture through both, 0 changed (`k8s_tests.rs`'s
/// `sanitize_cannot_act_on_anything_the_ingest_strip_left`). That is the box's question answered:
/// one string, one transformation.
///
/// **Where the driver applies it, and why never to a document, is `main.rs`'s to say**, beside
/// the import that brings it there (NOTES § D264 ruling 14). Here it is the strip for text that
/// never met `k8s::text` on the way in: a namespace the reader typed reaches [`scope`] and
/// [`next_step`] through [`Coverage`], and nothing else stands between it and the terminal.
pub fn sanitize(text: &str) -> String {
    text.chars().filter(|c| !unprintable(*c)).collect()
}

/// **A `409`'s two clauses — [`because`]'s, and the refused box's own** (`screens/dialogs.md` § The
/// cluster said no, state 0). That box already says *Nothing was changed.* as its outcome, so it
/// joins these two without the middle clause this file's sentence carries; one spelling of each, so
/// the two surfaces cannot come to describe a conflict differently (NOTES § D266).
pub const MOVED: &str = "something else changed this object while k8rs was working on it";

/// The second of [`MOVED`]'s pair.
pub const REREAD: &str = "reading it again shows what it looks like now";

/// **One plain clause: why a call did not work** — the caller supplies the subject, this
/// supplies the reason (invariant 14, `PRIOR-ART § C1`).
///
/// **A generic sentence may never stand in for an error we were handed**, which is the whole of
/// this function's reason to exist. k9s tells these apart internally and still shows
/// `Ruroh? 'v1/pods' command not found` when a credential expires; every site on the cluster path
/// that has a fault in hand — the connection, the version, the discovery answer, each watch, and
/// the picker's failure box — routes through here.
///
/// **The claim is *the cluster path* and not *every site in `main.rs`*, because two typed errors
/// live outside it** and are named where they are: an io error from a failed stdout write
/// ([`stdout_failure`](crate::stdout_failure)) and one from a runtime that would not start
/// ([`runtime_failure`](crate::runtime_failure)). Both print the standard library's own reason
/// through [`sanitize`]. The first draft of this line said *every site in this driver* and the
/// runtime arm was throwing its error away with a `_` (`tester`, 2026-08-27) — an overclaim and a
/// defect in one sentence, which is the second box read literally.
///
/// **Which of the driver's sites are on that path is `main.rs`'s to say**, beside its import of
/// this function.
///
/// **`asked` is what k8rs was trying to do, already spelled the way it should read.** Only the
/// caller knows, so `` `get /apis` ``, `` `list` and `watch` pods `` and *reach this cluster*
/// arrive as display text carrying their own backticks. That is what makes a refusal name the
/// missing verb and resource, which the security gate requires — and what lets the one refusal
/// that has neither, a `nonResourceURL` on `/apis`, name a **path** instead: its measured
/// `Status` carries an empty `details`, so a sentence built from `details.group`/`details.kind`
/// would be empty (NOTES § D160).
///
/// **`renewal` is [`k8s::Session::renewal`](crate::k8s::Session::renewal)** — the program the
/// reader's *own kubeconfig* names, already stripped and bounded by `k8s.rs`'s ingest guard. It is
/// never the cluster's text and never the login program's output, which is a credential
/// (`docs/security.md` § Token hygiene).
///
/// **Nothing here formats the error we were handed**: [`k8s::Fault`](Fault) carries no string at
/// all, and `said` is one named field selected by [`k8s::said`](crate::k8s::said) — never a
/// `Display`, which walks down to an `exec` plugin's stdout (`docs/security.md` § Token hygiene).
///
/// **`said` is the server's own sentence about this call, already stripped and bounded by
/// `k8s.rs`'s ingest guard**, and `None` where the server sent none or where nothing was ever sent
/// to a server. **Exactly one arm reads it and the rest ignore it on purpose**
/// ([`k8s::Fault::Rejected`](Fault::Rejected)): for every other fault this function's own sentence
/// is the better one and was written to be — a `403`'s message names a user and a verb where *the
/// role this kubeconfig uses needs to …* names the fix, and a `404`'s repeats a name the reader
/// just typed. The rejected call is the one where k8rs has nothing of its own to say.
pub fn because(fault: Fault, asked: &str, renewal: Option<&str>, said: Option<&str>) -> String {
    // The program named, or not named, without changing the sentence around it.
    let named = renewal.map_or(String::new(), |program| format!(" (`{program}`)"));
    match fault {
        // **Three sentences where there was one constant over all fifteen of
        // `KubeconfigError`'s variants**
        // (`k8s-admin`, 2026-08-27). *"…or names no such context"* was printed for a
        // `client-certificate` path that had moved and for a cluster entry with no `server:`,
        // and in both the file read fine and the context was there — a generic string standing
        // in for a typed error, which is this box's whole subject, through a door it had not
        // been looked at through.
        Fault::Kubeconfig => {
            "the kubeconfig itself could not be read — it is missing, unreadable, or not valid \
             YAML"
                .to_string()
        }
        Fault::NoContext => {
            "this kubeconfig has no such context — check the `current-context` line in the \
             file, and any `--context` on the command line"
                .to_string()
        }
        // **It does not say *which* entry, and that is the honest limit of a `Fault`.** The
        // variant names the class and the words are the caller's; naming the field would mean
        // carrying kubeconfig text on the type, which is the property that keeps every other
        // sentence in this function free of anything a cluster wrote.
        Fault::BadEntry => {
            "this kubeconfig loaded, and something it points at did not — a certificate file it \
             names, a `server:` line, or a cluster one of its contexts refers to"
                .to_string()
        }
        Fault::NoCredential => format!(
            "the program this kubeconfig logs in with{named} gave k8rs nothing to sign in with"
        ),
        // **The one place a renewal is worth naming** (NOTES § D19): a login minted by a helper
        // ran out mid-session, and what the reader needs is which system to sign in to again —
        // not a cloud guessed from the server URL.
        //
        // **It promises nothing about restarting, and that is measured** (`tester`, 2026-08-27).
        // kube re-runs the `exec` plugin as its cached credential falls out of its own window —
        // 25 plugin executions against 22 requests over a ten-second run — so for the ordinary
        // exec kubeconfig the watch recovers on its own the moment the login is repaired, and
        // *restart k8rs* would be D19's own failure wearing the other face: a true problem
        // answered with the wrong errand.
        //
        // **One shape reaches this arm where *renew it there* is true but incomplete**, and it is
        // narrower than *a plugin that fails mid-session* — that one has been produced since, and
        // it lands in [`k8s::Fault::NoCredential`] rather than here (NOTES § D167). What is left
        // is a plugin whose credential carries **no `expirationTimestamp` from the start**:
        // `Auth::try_from` matches `(Some(token), None) => Ok(Self::Bearer(token))`
        // (`auth/mod.rs:364-367`), so it is a static header with no `RefreshableToken` behind it.
        // Nothing ever re-runs the plugin, no `AuthError` is ever raised, and the server simply
        // answers `401` — so renewing the login where it comes from is necessary and not
        // sufficient, because k8rs also has to be restarted to pick the new token up.
        //
        // **The sentence stays as it is**: it is true of both, and the shape that needs the extra
        // half is the PM's to box rather than this arm's to guess at.
        Fault::Expired => match renewal {
            Some(program) => format!(
                "this cluster no longer accepts this login — it comes from `{program}`, so \
                 renew it there"
            ),
            None => "this cluster no longer accepts this login — this kubeconfig needs a new one"
                .to_string(),
        },
        // **It names what the role needs, not what the kubeconfig is not allowed to do**
        // (`k8s-admin`, 2026-08-27). A watch is two verbs and [`k8s::Trouble`] cannot say which
        // of them was refused — measured through a forwarder that passed `list` and answered
        // only `?watch=true` with a real `403`: the LIST **succeeded**, forty pods printed, and
        // the line beside them said *not allowed to `list` and `watch` pods*. A `Role` granting
        // `list` and omitting `watch` is an ordinary hand-written Role, and the operator adds a
        // verb that was never missing.
        //
        // **Collapsing `InitialListFailed` and `WatchStartFailed` into one [`k8s::Fault`] is
        // right** — that is what one classifier means — so the fix is the frame: *the role needs
        // both of these* is true whichever was refused, where *is not allowed to* is a claim
        // about current state this code cannot make. The security gate asks a refusal to name
        // the missing verb and resource; this names the verbs and the resource without
        // pretending to know which one is absent.
        //
        // **`needs to {asked}` and not `needs {asked}`**, so one verb phrase serves this arm and
        // the two below it: the grid test reddened on *needs reach this cluster* the moment the
        // frame changed, which is what twelve literals are for.
        Fault::Refused => format!("the role this kubeconfig uses needs to {asked}"),
        // **`when k8rs tries to …` and not `there is nothing to …`** (`tester`, 2026-08-27).
        // The old frame wanted a noun where every caller supplies a verb phrase, so it read
        // *there is nothing to `list` and `watch` pods* — and it was only ever fed the one
        // framing where that passes, `` `get /apis` `` (NOTES § D29, in a function whose own doc
        // is about framings). This frame takes all four.
        Fault::Gone => {
            format!("this server says there is no such thing when k8rs tries to {asked}")
        }
        // **The server's own words where it wrote any, because for this fault they are the
        // diagnosis** (`k8s::said`). Measured on a live kind cluster, `--logs` against the pod
        // `--once` had just carded CRITICAL: the API server answered *container "app" in pod
        // "broken-config" is waiting to start: CreateContainerConfigError* — the same root cause
        // the card names — and k8rs replaced it with the self-accusation below
        // (`k8s-admin`, 2026-09-03). `k8s::Fault::Rejected` was this defect's first pass and
        // fixed only the category; this is the message.
        //
        // **Quoted verbatim rather than re-explained, which is NOTES § D37's rule and not an
        // exemption from invariant 14.** Rules 3, 4 and 10 already put the runtime's own message
        // on the card word for word, and the card for this very pod carries the plain-language
        // reading beside the kubelet's own line: *Container needs a ConfigMap or Secret that does
        // not exist (CreateContainerConfigError)* over *configmap "…" not found*. The jargon word
        // is kept **and** explained, on the surface built to explain it.
        //
        // **[`WAITING_REASONS`] is reachable from here and is still not reused, which is
        // the question this box had to answer.** (It stood in `main.rs` until the detail tabs
        // needed the same words and it moved down, NOTES § D254; the argument below did not move
        // with it then, because it is about this call site and not about where the table lives.)
        // Reaching it is not the obstacle — the obstacle is that its phrases are a paraphrase of
        // the cards, not the cards' words, and for one of the two states a live cluster produced
        // they and the server disagree outright: the API server writes
        // *trying and failing to pull image* where that table writes
        // *cannot get its image* (`default/broken-image`, 2026-09-03). Printing both in one
        // sentence is two spellings of one condition, which is the defect this repo has paid most
        // for; keying off the message's trailing word to pick one would be scraping free text the
        // API server never promised the shape of. **And this function has no container in scope
        // anyway** — fourteen callers, one of which is a log request — so the reason would have to
        // travel from a pod read that happened a round trip earlier and may already be stale.
        //
        // **So the choice is the cluster's sentence or none, and the cluster's says what is
        // wrong.** What is *not* closed by that is the reader who runs only `--logs` and never
        // sees the card; `screens/detail.md` has no state for a refused log request at all, and
        // that is the screen's gap to fill rather than this line's to guess at.
        //
        // **`and said:` attributes it.** The words after it are the server's and the reader has
        // to be able to tell; nothing else in this function quotes anybody.
        Fault::Rejected => match said {
            Some(said) => format!(
                "this cluster would not accept the request k8rs made to {asked}, and said: {said}"
            ),
            // **The honest fallback, and it stays as it was.** With no message there is nothing
            // to go on but the code, and a `400` is a request this side built — so *the reader
            // has nothing to fix here* remains the only thing that can be said.
            //
            // **No shape produced so far enters it, and that is a measurement and not a
            // guarantee.** Both `400`s a live four-node kind cluster answered for `--logs`
            // carried a message (`default/broken-config`, `default/broken-image`, 2026-09-03),
            // and a `400` whose body is not a `Status` at all loses its code inside kube and
            // lands in `k8s::Fault::Unanswered` instead (`k8s::answer`). What is *not* claimed is
            // that no server ever sends a `Status` with an empty `message`: the field is
            // `#[serde(default)]`, nothing was measured that does it, and the arm is here for
            // exactly that.
            None => format!(
                "this cluster would not accept the request k8rs made to {asked} — that is a \
                 fault in k8rs, and nothing is wrong with the cluster or with this login"
            ),
        },
        // **The one arm that names no verb, because a `409` is not about what was asked** — it
        // is about the object having moved between the read and the write (NOTES § D213). It is
        // the only fault whose fix is *k8rs reads it again*, so the sentence says what the reader
        // will see happen rather than sending them anywhere.
        Fault::Conflict => format!("{MOVED} — nothing was changed, and {REREAD}"),
        Fault::Unanswered => format!("nothing usable came back when k8rs tried to {asked}"),
        // **The one arm with no cause in it, and that is the arm** (`k8s::Fault::Unfinished`).
        // Nothing came back and nothing said why, so every sentence that would explain it is a
        // guess: NOTES § D148's missing keepalive makes a socket that died mid-LIST look exactly
        // like a server that went quiet, and NOTES § D150 refuses to call a LIST that is still
        // moving *hung*. An earlier draft said *nothing is wrong with this login: it is the
        // cluster, or the network in between, that has gone quiet* and was both — a cause the
        // taxonomy cannot see and a verdict D150 forbids (`k8s-admin`, 2026-09-03).
        //
        // **What a `--once` reader gets instead is the two numbers**, and they are `main.rs`'s
        // `unreadable`'s, not this function's: `k8s::Trouble::outstanding` travels beside the
        // fault for exactly that.
        Fault::Unfinished => {
            format!("the request k8rs made to {asked} had not been answered")
        }
    }
}

/// **Where k8rs looked, in the reader's words** — the fact that decides whether a `Role` or a
/// `ClusterRole` is what they go and ask for.
///
/// **It reads [`Coverage::namespace`], the collapse**, because the *place* is the same whichever
/// way k8rs arrived at it; what differs is the next step, which is [`next_step`]'s and reads the
/// whole [`Coverage`].
pub fn scope(coverage: &Coverage) -> String {
    match coverage.namespace() {
        None => "across the whole cluster".to_string(),
        Some(namespace) => format!("in the namespace {}", sanitize(namespace)),
    }
}

/// **What the reader can do about a watch that never listed**, or `None` where there is no honest
/// answer (`screens/context.md` § The scope changes the next step, not just a number).
///
/// **Three faults have a next step, and inventing one for the rest is the fallback [`because`]
/// refuses** — which three is NOTES § D264 ruling 32's. An expired login already carries its own
/// action inside [`because`], and a stream that ended without saying why has no honest one.
///
/// **The refusal's answer is per [`Coverage`] and never per `Coverage::namespace()`**
/// (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R1): *or run k8rs in one namespace*
/// is a spent door for a reader who already typed `--namespace`, and asking for access to a
/// namespace k8rs guessed sends the reader after one they never chose. `resource` is the API's own
/// plural, as [`watching`] takes it.
///
/// **`running` is whether k8rs is already up**, which changes only the two arms that end in the
/// flag (NOTES § D264 ruling 23): the picker's failure box passes `true`, `--once` passes `false`.
pub fn next_step(
    fault: Fault,
    coverage: &Coverage,
    resource: &str,
    running: bool,
) -> Option<String> {
    match fault {
        Fault::Refused => Some(match coverage {
            Coverage::Cluster if running => format!(
                "Ask whoever runs this cluster for a role that may read {resource} in every \
                 namespace — `k8rs-readonly` in the k8rs docs is that role — or quit and start \
                 k8rs again in one namespace you can read: {NAMESPACE} <name>"
            ),
            Coverage::Cluster => format!(
                "Ask whoever runs this cluster for a role that may read {resource} in every \
                 namespace — `k8rs-readonly` in the k8rs docs is that role — or run k8rs in one \
                 namespace you can read: {NAMESPACE} <name>"
            ),
            // **The one arm where the namespace was not the reader's choice**, so the door the
            // arm below has already spent is the door this one has to open
            // (`k8s::Coverage::Blind`).
            Coverage::Blind(namespace) if running => format!(
                "This kubeconfig names no namespace, so k8rs had to guess {} and was refused \
                 there too. Quit and start k8rs again in the namespace you work in: {NAMESPACE} \
                 <name>",
                sanitize(namespace)
            ),
            Coverage::Blind(namespace) => format!(
                "This kubeconfig names no namespace, so k8rs had to guess {} and was refused \
                 there too. Say which namespace you work in: {NAMESPACE} <name>",
                sanitize(namespace)
            ),
            Coverage::Asked(namespace) | Coverage::Refused(namespace) => format!(
                "Ask whoever runs this cluster for a role that may read {resource} in {} — the \
                 same rules as `k8rs-readonly` in the k8rs docs, granted in one namespace \
                 instead of all of them",
                sanitize(namespace)
            ),
        }),
        Fault::Unanswered => Some(
            "Check the server address this kubeconfig names, and that this machine can reach it"
                .to_string(),
        ),
        Fault::Gone => Some(
            "Check the server address this kubeconfig names — as written, it does not lead to a \
             Kubernetes API server"
                .to_string(),
        ),
        _ => None,
    }
}

/// **What a reader whose login program answered nothing can do about it** — run the same login
/// themselves, in their own terminal, where it can prompt (`screens/context.md` § When the new
/// cluster does not work).
///
/// **It reverses half of NOTES § D264 ruling 32, and the thing that changed is not the wording.**
/// That ruling gave [`Fault::NoCredential`] no next step because the program's own diagnosis went
/// to the terminal's stderr and getting it back was Phase 12's terminal handover to do. Phase 12
/// did that turn and ruled the other way: an `exec` program gets `interactive_mode: Never`, so its
/// stderr is gone **for good** rather than merely unread (NOTES § D279 ruling 6, § D280 item 3).
/// That closes the door ruling 32 was leaning on and opens a plainer one — `kubectl` runs the same
/// program against the same kubeconfig entry, in a terminal that can take a password, a device code
/// or a hardware key.
///
/// **`version`, and not because it needs less permission — nothing the reader can name does**
/// (NOTES § D281 item 3). The login program runs *before* any request is sent, so every command
/// exercises it equally; the only real choice is which failure stays confusable. `get ns` was
/// wrong for exactly the case its own rationale claimed to cover: `namespaces` is cluster-scoped,
/// so a namespaced `Role` grants `list namespaces` as little as `list pods -A`
/// (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R1, the platform-issued kubeconfig
/// this whole `Coverage` fallback exists for), and that reader would have read *namespaces is
/// forbidden at the cluster scope* and concluded the opposite of what was true. `version` asks
/// about `/version`, the `nonResourceURL` `docs/security.md`'s own `k8rs-readonly` Role names and
/// the one path k8rs's connect log already leans on — narrower, though not guaranteed
/// ([NOTES § D160](../NOTES.md)).
///
/// **What proves the login worked does not need the request to succeed**: a `403` on `version`
/// still means the plugin handed the cluster a credential it could check, which is the one
/// question this box asks — never whether that credential can list anything.
///
/// **It is not [`next_step`]'s, and that is not a filing decision**: that function is shared with
/// `--once` through `crate::main`'s `pods_unread`, answers per [`Coverage`], and takes no context
/// name — and the name is the whole of what makes this command runnable.
///
/// **The name is quoted by [`crate::ops::pasteable`] and by nothing else** (NOTES § D278 ruling 5,
/// § D281 item 1). [`Choice::name`] is `k8s::drawable`, which removes characters with no printed
/// form and caps length — **it is not a charset allowlist**, so a space and a `;` both survive.
/// Interpolated raw, this drew `--context needs login` beside the strip's own
/// `--context 'needs login'` in one frame, and for `prod eu; echo pwned` it handed the reader a
/// shell injection *as an instruction to paste*. This surface is worse than the command log for
/// exactly that reason: the strip is text a reader may paste, this is a sentence telling them to.
///
/// **A name that strips to nothing gets no next step at all** (NOTES § D281 item 2). `(unnamed)`
/// is not a wrong command but an unrunnable one — `kubectl --context (unnamed)` is a shell syntax
/// error — and dropping just the flag is worse, because `kubectl version` then teaches a command
/// against the reader's *current* context, a third cluster. `crate::main`'s `kubectl` rules the
/// same input the same way, dropping the segment whole: *a line that does not run has nothing to
/// teach*. A dead end beats a wrong errand.
///
/// **It carries its own full stop**, unlike [`next_step`]'s sentences, because it is two sentences
/// and the second is the instruction: `crate::ui::failed` joins a next step with one space and adds
/// no stop of its own, so *Then try again* would otherwise run into the way-out paragraph.
pub fn run_the_login(context: Option<&str>) -> Option<String> {
    // **The *stripped* value decides whether there is a name**, which is `crate::main`'s `kubectl`
    // own order: a name made only of characters [`sanitize`] removes is empty here and quoting the
    // nothing would produce the empty-valued flag the paragraph above refuses.
    let name = context.map(sanitize).filter(|name| !name.is_empty())?;
    Some(format!(
        "Run it yourself first: `kubectl {CONTEXT} {} version`. Then try again.",
        crate::ops::pasteable(&name)
    ))
}

// --- WHY A CALL DID NOT WORK END ---

#[cfg(test)]
#[path = "views_tests.rs"]
mod tests;
