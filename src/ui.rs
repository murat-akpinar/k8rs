//! **The frame, and the Alerts pane inside it** — the file that turns a [`crate::views::App`]
//! into a screen (`screens/alerts.md`, `screens/widgets.md`).
//!
//! **It is a pure function of state.** [`draw`] reads a `&App` and a [`Screen`] of borrowed
//! store data and writes cells; it decides nothing [`crate::views`] could decide and holds
//! nothing that outlives the frame. Every `ListState` it hands ratatui is built from a
//! [`crate::views::Cursor`] on the spot and dropped again, which is why the resolved scroll
//! offset is not carried between frames: `ListState` re-derives it from the selection, and the
//! selection is the state (`screens/widgets.md` § 4).
//!
//! **The mapping from `theme.rs` onto ratatui lands here, and nowhere else** (NOTES § D241).
//! `theme.rs` is frozen and names no ratatui type — a role is data, its Catppuccin value and the
//! one of the sixteen it degrades to — so [`ink`] is the one place an [`Ink`] becomes a `Color`
//! and [`mark`] the one place a [`Signal`] becomes text. Nothing below writes a colour of its
//! own, and a severity's colour and its glyph both come from `theme::band`, together, so the two
//! carriers cannot disagree (invariant: colour is never the only one).
//!
//! **Nothing here strips a string that came off the API, and that is not an omission**
//! (invariant 9). Everything reaching this file came through `k8s::text` at ingest, and the one
//! class that did not — what the user typed — is bounded and refused control characters by
//! `views::Input`. A third strip here would be a second opinion about what the first one did.
//! What this file does owe is not *building* a string that escapes that guarantee, which is why
//! every span below is either a literal or a value that arrived stripped.
//!
//! **A modal is drawn by [`draw`] and never by [`content`]**, because it floats over the body
//! region rather than inside the content pane: [`content`] dispatches on
//! [`crate::views::View`] and has one arm per view, and on [`Screen::detail`] before any of them
//! — a detail is open *over* a view, which is what `esc back` means. `screens/widgets.md` § 5's
//! three calls — `Clear`, the block, the content — are [`boxed`]'s, once, for every dialog on
//! `screens/dialogs.md`; [`help`] is the sizing exception that file names, and the only modal
//! that takes the whole body and leaves no sidebar showing to float over.

// Nothing outside `#[cfg(test)]` calls this file yet: the event loop that will is Phase 12's
// `main.rs`. Same attribute, same position and same accepted blind spot as `theme.rs`'s and
// `views.rs`'s (NOTES § D38) — and, like both of those, **it does not expire by itself: the
// turn that wires `main.rs` deletes it by hand.**
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the event loop that draws is Phase 12's (todo.md § Phase 12)"
    )
)]

use crate::analysis::{Badge, Report, Row as ReportRow};
use crate::k8s::{Address, Browsable, Choice, Coverage, Fault, Tag};
use crate::rules::{ContainerSnapshot, Finding, ObjectId, ObjectKind, PodSnapshot, Severity, age};
use crate::theme::{self, Colour, Depth, Ink, Signal};
use crate::views::{
    self, App, Card, Cursor, Filters, Input, NavItem, Offer, Pane, Refused, Tab, Typing, View,
};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Clear, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Table, TableState, Tabs,
};
use std::borrow::Cow;

// --- THE NUMBERS THE MOCKUPS ARE DRAWN TO START ---

/// The floor k8rs draws a layout at. Below it there is one sentence and no layout
/// (`screens/widgets.md` § 8).
const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;

/// The sidebar, **fixed and never proportional** — the labels are fixed-length strings and the
/// eye should find them in the same place after a resize (`screens/widgets.md` § 1).
const SIDEBAR: u16 = 20;

/// The command log strip: two lines inside the frame (`screens/alerts.md` § The height).
const LOG_LINES: u16 = 2;

/// The pad each side of the content pane. The **card region** is the pane less two of these —
/// 53 columns at the 80×24 floor (`screens/alerts.md` § The columns).
const PAD: u16 = 2;

/// Between a name that had to clip and the age beside it (`screens/alerts.md` § The age), and
/// between two browser columns — **which is what keeps a clipped name and the number beside it
/// from being read as one token** (`screens/resources.md` § Browsing every namespace, the clip
/// point). One column would satisfy that rule; two is what every mockup on both screens draws.
const GAP: usize = 2;

/// **The gap between the facts on the at-rest filter line** — `filter: "web"   esc clears it`,
/// read off `screens/widgets.md` § A committed filter is drawn at rest, too. **Three and not
/// [`GAP`]**: that one is the gap between two columns of a table, and this row is a run of
/// independent short facts, which is the same three columns `screens/detail.md`'s own container
/// block puts between a name and the word after it.
const FILTER_GAP: &str = "   ";

/// **The gutter a band's glyph sits in — two columns whether or not there is a glyph**, so a
/// banded row and a plain one start at the same column (`screens/alerts.md`,
/// `screens/resources.md`). One number because it is the same gutter on both screens: a card's
/// identity line and a browser row that a card bleeds through onto have to agree about where the
/// name begins, or `● web` in Alerts and `● web` in the browser are two different indents.
const GUTTER: usize = 2;

/// **The priority plain `kubectl get` prints, and the only one the browser draws.** Anything
/// above it is what `-o wide` adds, and drawing it makes every screen the wide view
/// (`screens/resources.md`, [`crate::k8s::Column::priority`] — the filter is the screen's,
/// because which columns fit is).
const PLAIN: i32 = 0;

/// The evidence is the one thing a card cuts, and it is cut at three wrapped lines. The number
/// is a measurement rather than a taste: at two, rule 10's card stops before the quote it went
/// and fetched (`screens/alerts.md` § How wide a card is, and how tall).
const EVIDENCE_LINES: usize = 3;

/// The centred block an empty or still-loading pane draws its sentences in
/// (`screens/states.md`).
///
/// **Measured off that page's own widest already-drawn line, not chosen** (§ The audit log could
/// not be opened, which states the number and shows the three lines it is read off): `84 pods and
/// 3 nodes checked, none of` is 36 (§ Nothing is broken), `Large clusters take a moment. Findings`
/// is 38 (§ Still loading), and that section's own `No space left on device (os error 28) —` is
/// 39 — the longest the file draws anywhere this block speaks. At 34 the audit sentence needed
/// nine lines against the 13 [`FLOOR`] leaves and was cut with a visible mark; at 39 it is 12 of
/// them and the file's line is drawn whole, which is the check this number exists to pass.
const BLOCK: u16 = 39;

/// **The rows the list — or the calm block — keeps whatever queues above it**
/// (`screens/states.md` § Your clock and a scoped namespace together, which states the arithmetic
/// once for all three banners: the body is 16 rows, this is 3 of them, and everything stacked over
/// the list shares the other 13). The number is
/// [`screens/alerts.md` § How wide a card is, and how tall]'s own — *"`shop/api` gets three rows
/// and that is the floor"* — so a card under a stack of banners is still a card.
const FLOOR: u16 = 3;

/// The browser's own title and the blank under it, which come out of what a caveat leaves rather
/// than out of the table's rows.
const HEADING: u16 = 2;

/// `▸ ` — [`theme::SELECTION`] plus the column that keeps it off the label. Two columns, so a
/// group's row reads `▸  workloads` exactly as `screens/resources.md` draws it.
const MARKER: &str = "▸ ";

/// The centred zone of the header row (`screens/widgets.md` § 1a).
const NAME: &str = "k8rs";

/// **The only thing k8rs shortens on purpose, and it is always visible where it happened**
/// (`screens/widgets.md` § 7). One character on every cut, so the mark a reader learns is one mark
/// — but for the command log strip's, which is [`STRIP_CUT`] and says why.
const CUT: &str = "…";

/// **The command log strip's own cut mark, and the one exception to [`CUT`]** — three periods,
/// because on that strip `…` is already `crate::views::Log`'s running mark, and a command cut for
/// space drew the same trailing character as one still on the wire (`screens/widgets.md` § 7,
/// back-cut 3; NOTES § D266).
const STRIP_CUT: &str = "...";

/// **The three widths a nested dialog box picks from, and the only choice a dialog makes about
/// its own shape** (`screens/widgets.md` § 5, read off every box in `screens/dialogs.md`). The
/// margins are not a second choice: they are whatever centring one of these in the body leaves,
/// which is [`boxed`]'s one `Rect::centered` and never a table of hand-computed rectangles.
///
/// The default, for a `Confirm` whose content fits it (`screens/dialogs.md` § Scale).
const CONFIRM_BOX: u16 = 58;

/// **[`CONFIRM_BOX`] does not fit** — a consequence past [`CONSEQUENCE_LINES`], or the
/// typed-name field (`screens/dialogs.md` § Restart, § Delete). It is close to the real ceiling:
/// the smaller margin cannot drop below 2 without touching the outer frame, and 61 leaves 2.
const CROWDED_BOX: u16 = 61;

/// **A dismiss-only box** — `Refused` and `Gone`, which have no `$ kubectl …` line and no input
/// field, so there is consistently less to fit (`screens/dialogs.md` § The cluster said no,
/// § The object went away).
const DISMISS_BOX: u16 = 54;

/// The margin between a nested box's left border and its text. **Left only, and it is a prefix
/// on each line rather than `Padding` on the block**, so a centred row — the buttons — is centred
/// on the box and not on what is left of it after a pad.
///
/// **Two columns is measured rather than chosen**: at `61 - 2` the restart and delete
/// consequences wrap onto exactly the lines `screens/dialogs.md` draws them on, and at `58 - 2`
/// the scale box holds its 55-column `$ kubectl scale …` line whole, which is the widest single
/// thing any box has to fit.
const MODAL_MARGIN: &str = "  ";

/// **The two lines § Scale draws its consequence on** — and the test for whether a `Confirm` box
/// can stay at [`CONFIRM_BOX`] (`screens/dialogs.md` § Scale, *the box draws these as two lines*).
const CONSEQUENCE_LINES: usize = 2;

/// **The columns the container picker guarantees a name, whatever its state word costs**
/// (`screens/detail.md` § When a container's own state is what does not fit).
///
/// **Twenty, and it is `tui-designer`'s number rather than a round one**: measured against the
/// names that page's own mockups draw — `sidecar-envoy` is 13 and `istio-proxy-metrics` is 19, so
/// the floor clears the longest of them by a column, and a name past it front-cuts the way every
/// name on this product does rather than disappearing. It is a floor and not a width: an ordinary
/// pod, whose state words are `running` and `done`, gives the name far more than this.
///
/// **What it is measured *against* is the failure it prevents**, not the names alone: with the
/// name column taken as the plain remainder, one `CreateContainerConfigError` container — 47
/// columns of translated state — left it at **one** column, and `front(name, 1, "…")` draws
/// nothing at all (`reports/2026-09-18-filter-and-container-picker.md` § 1).
const NAME_FLOOR: usize = 20;

/// **The rows the zero-match sentence is drawn on** (`screens/states.md` § The filter hides every
/// row, which draws it on one).
///
/// **Three and not one, measured against the longest sentence a real screen can build**: the kind
/// plural is the server's word and both filter values are the reader's, so
/// `No validatingadmissionpolicybindings match "payments-web" in a namespace like
/// "openshift-cluster-node-tuning-operator".` is 129 columns — three rows of a 53-column pane —
/// and every character of it is a fact the reader needs. Past that the values are longer than
/// anybody types on purpose, and what is drawn is marked.
const MATCH_LINES: usize = 3;

/// **The two lines the container picker's restart hint is drawn on** — `screens/detail.md`
/// § Choosing a container draws it on exactly two, and the eighteen fixed rows that file counts
/// are counted with both of them. It is a ceiling as well as a drawing: the rows left for the list
/// are what this leaves, so a hint that grew would take the list with it.
const HINT_LINES: usize = 2;

/// **The rows a nested box has between its own borders**, at the 80×24 floor this product is
/// drawn to (`screens/widgets.md` § 5). § Restart's paused variant and § Drain both land on
/// exactly this; it is what bounds what the cluster sent back in a `Refused` box, which is the one
/// string in a dialog that came off the API.
const MODAL_ROWS: usize = 13;

/// The rows a typed-name field takes: its label, and the three the box around it draws
/// (`screens/dialogs.md` § Delete).
const FIELD_ROWS: usize = 4;

/// Between a dialog's two buttons (`screens/dialogs.md`, every box on it).
const BUTTON_GAP: &str = "    ";

/// **How far in from each side of the body the cluster picker's box sits** — read off
/// `screens/context.md` § The picker, whose 62-column box sits three columns in on a 68-column
/// body. **The margin is fixed and the box is not**: a wider terminal widens the name slot and
/// nothing else (that file's § The tag column), which is why this is not one of [`CONFIRM_BOX`]'s
/// three widths.
const PICK_MARGIN: u16 = 3;

/// **A context's tag slot — 12 columns, fixed**, and cut at its edge with no mark: there is nowhere
/// to escape into and read the rest (`screens/context.md` § A 60-character tag).
const TAG: usize = 12;

/// **A context's badge slot — 20 columns, fixed**, the width of `⚠ TLS not verified` and its pad.
const BADGE: usize = 20;

/// **The most the context's name takes in front of the server line** — the name slot's own width
/// on the page `screens/context.md` draws. The line below the list is the *am I about to touch
/// production* line, and a 90-character EKS ARN in front of it would push the address off it.
const LABEL: usize = 20;

/// **The rows the server line may take**, and the rows the box keeps for it: the three § A context
/// defined twice's sentence takes behind a short name, the longest k8rs itself writes there. An EKS
/// endpoint is 72 columns and needs two, and anything longer is cut with a mark. **Every row more
/// is a list row less at the 80×24 floor** — measured with an address at the ingest bound selected
/// over two contexts: three shows both, four shows one, and five leaves no blank row above the box.
const SERVER_ROWS: usize = 3;

/// **The sentence under the picker's list, on the two lines `screens/context.md` breaks it at.**
/// Fixed text written to fit, so it is two literals rather than a wrap that would move with the
/// terminal's width.
const UNCHANGED: [&str; 2] = [
    "k8rs does not change your kubeconfig — it just",
    "talks to the cluster you pick here.",
];

/// **What the slot under the picker's list says when no row is selected and one can be**
/// (`screens/context.md` § No row is both current and landable) — in place of an address, so `⏎`
/// does not look bound. On the page's two lines, for [`UNCHANGED`]'s reason.
const UNSTARTED: [&str; 2] = [
    "k8rs cannot start you on a row here —",
    "pick one with ↑ or ↓.",
];

/// **What the slot says when the list shows rows and none can be landed on** — no key moves the
/// cursor, so the sentence sends the reader to none (NOTES § D264 ruling 16).
const UNREACHABLE: [&str; 2] = [
    "None of these can be reached — every one names a",
    "cluster this kubeconfig does not define.",
];

/// **What the slot says when the kubeconfig has no contexts left in it** — there is no row, dimmed
/// or not, for a key to reach, so the sentence sends the reader to none (`screens/context.md`
/// § The kubeconfig has no contexts at all, NOTES § D264 ruling 18).
const EMPTIED: [&str; 2] = [
    "This kubeconfig has no contexts in it —",
    "there is nothing here to connect to.",
];

/// **The rows the picker's box leaves unspent in the body**: its own two borders and one more, the
/// row [`MODAL_ROWS`] leaves at the 80×24 floor. The list takes whatever else a taller terminal has
/// (NOTES § D264 ruling 9).
const PICK_SPARE: usize = 3;

/// **The one title the frame's outer border ever carries**, and it carries it only while `?` is
/// open (`screens/widgets.md` § 5, `screens/help.md`). A space each side, because that is how
/// `screens/help.md` draws it: `┌ Keys ─…`.
const TITLE: &str = " Keys ";

/// **The key map itself — sixteen lines, which is exactly the body's own budget**
/// (`screens/help.md`, `screens/widgets.md` § 1). It is a literal and not a table assembled from
/// [`crate::views::Tab`] or a key enum: the grouping is *what you are doing* rather than keycode
/// order, the jargon in brackets is the teaching (invariant 14), and both are the screen file's
/// choices rather than anything the code holds. NOTES § D12 fixes the keys; this is the one place
/// they are spelled for a reader, and `screens/help.md` is what it is checked against.
///
/// **Every line is drawn as written, with no wrap** — the widest is well inside the 78 columns
/// the body has at the floor, so nothing here reflows and nothing here is cut. The indents are
/// the mockup's own: two columns for a group, four for a key.
///
/// **The first line starts on the opening quote and not behind a `\` continuation**, which is
/// the one way this literal has already been got wrong: `\` at a line's end strips the newline
/// *and the next line's leading whitespace*, so `Moving around` drew at column 0 while its two
/// sibling headings drew at 2. It was invisible to a test that compared the screen with this
/// constant, and visible the moment the screen was compared with `screens/help.md`.
/// **The three groups run without a blank row between them, and that is paid for and not tidy**
/// (`screens/help.md` § Rules): the body is capped at sixteen rows by `screens/widgets.md` § 1's
/// budget, `s` and `r` each needed a second line, and a verbatim sentence cannot be shortened to
/// fit. The two separators are what bought them. It is denser, the screen file says so, and every
/// full-body mockup on that page is drawn without them.
const HELP: &str = "  Moving around
    ↑ ↓ / j k    move            ⏎     open the selected thing
    tab          next panel      esc   back / close
    X            switch cluster
    [ ]          detail tabs     / n   filter · namespace
  Looking at things (always available)
    l  logs, with the log from before a crash
       in the log tab:  f follow · c container · ⇧p previous
    d  describe — the object and what happened to it
    y  view as YAML
  Changing things (each one asks first, and shows the command)
    s       run more or fewer copies       (scale)
            works on a deployment, a statefulset and a replicaset
    r       restart, at its own pace       (rollout restart)
            works on a deployment, a statefulset and a daemonset
    ctrl-d  delete — you type the name to confirm";

/// **[`HELP`]'s sixteen rows, with a *why not* clause on each key this login has been told it may
/// not use** (`screens/help.md` § *When a key is refused*, NOTES § D23).
///
/// **Each clause is anchored on the row it is for, by that row's own text, and never by counting**
/// (`tester`, 2026-09-12 and 2026-09-13). The first draft rewrote the last three rows of [`HELP`]
/// by arithmetic — correct for today's [`HELP`] and wrong the moment another box changes it: fed a
/// body that had lost those keys it ate `y view as YAML`, the blank line and a read-only sentence,
/// and drew the three mutating keys back — no panic, no failing test. A row that is not the one a
/// clause is for is now never written, and a body with no such row simply has nowhere to land it.
/// **The rows are split on `\n` and not `str::lines`**, which drops a trailing blank row and with
/// it the body's sixteenth.
///
/// **`help` is a parameter for that reason too**: what this does to a key map shaped differently
/// is the thing to be able to test, and a `const` read from inside cannot be fed one. The one
/// product caller passes [`HELP`].
///
/// No other row and no other block changes, and the body stays sixteen rows, so
/// `screens/widgets.md` § 1's budget is untouched. Nothing is refused and nothing is held ⇒ the
/// join is [`HELP`] back, character for character, which is what the *fail open* half of NOTES
/// § D229 ruling 4 looks like from the screen's side.
///
/// **The clause extends the jargon parenthesis for `s` and `r` and opens a new one for `ctrl-d`**,
/// and both `s`'s and `r`'s `(` move left to the 40th character where [`HELP`] has them at the
/// 44th — the screen file's ruling, and forced: `s` now names two verbs, and holding either `(`
/// at the 44th puts that row past the 78-column ceiling. The padding is written into the literals
/// rather than computed, because what it lines up with is [`HELP`]'s own hand-set columns and
/// there is no second budget to derive it from.
///
/// **The verbs come from [`Refused`]'s own permission lists**, so the sentence the reader is asked
/// to hand their cluster owner cannot name fewer grants than the probe asked about — which is the
/// half of the 2026-09-12 `scale` defect that survived a correct probe.
///
/// **The only other thing interpolated is [`Refused::resource`], a `&'static str` from k8rs's own
/// closed set** (NOTES § D246) — never a plural discovery handed back, which is what would put a
/// row past the ceiling as well as past invariant 9.
///
/// **A reason that holds for the whole block rewrites the heading, and while one does the three
/// permission clauses do not draw at all** (`screens/help.md` § *While the call is running*,
/// § *While the link is down…*, § *Under a dead-writes run*; NOTES § D262, § D265 rulings 4 and 5).
/// This screen exists to answer *what may I press*, and it promised `s`, `r` and `ctrl-d` as live
/// while `App::may_mutate` was false — the reader presses one, gets nothing, and no line anywhere
/// says why (PRIOR-ART § G1's *refuses for no visible reason*). **One reason is drawn, in this
/// order**: `held` when it is [`Held::Off`], then `changing`, then `held` when it is
/// [`Held::Paused`]. Off also puts the cause's own sentence in the `s` row and blanks the four
/// rows under it — `r`, `ctrl-d` and both [`SCALE_KINDS`] lines; the other two leave the block's
/// six rows alone, because *what kind does this work on* is not the question either of them
/// answers (`screens/help.md` § While the call is running).
///
/// **The heading governs `s`, `r` and `ctrl-d` at once, where a permission rewrites the rows and
/// leaves the heading.** That is not a layout preference: every one of these reasons refuses all
/// three uniformly and a missing permission never does — one key can be refused while the other
/// two are not — so a reason is one fact with one home and a permission is three.
///
/// **`changing` also rewrites the `X` row, whatever else holds**, because a call in flight is the
/// one reason that refuses a cluster switch too (`crate::views::App::may_switch_cluster`); dead
/// writes, the link and the clock say nothing about `X`.
///
/// **`paused`, not `no`** — `no` is this product's word for a permission this login lacks
/// (invariant 14, `screens/help.md`), and every one of these is a wait or a state of the run.
/// **No clause names the object** a call is running on: *a change is running* is true whichever
/// one it is, and dismissing Help puts the in-flight footer, which does name it, back on screen
/// (`tui-designer`, 2026-09-12).
fn key_map(help: &str, refused: Refused, changing: bool, held: Option<Held>) -> String {
    let resource = refused.resource();
    let heading = match (held, changing) {
        (Some(Held::Off(_)), _) => Some(OFF),
        (_, true) => Some(RUNNING),
        (Some(Held::Paused(why)), false) => Some(why),
        (None, false) => None,
    };
    let mut clauses: Vec<(&str, String)> = Vec::new();
    if changing {
        clauses.push((
            "    X ",
            format!("    X            switch cluster ({RUNNING})"),
        ));
    }
    if let Some(why) = heading {
        clauses.push((CHANGING_HEADING, format!("{CHANGING_HEADING} ({why})")));
    }
    if let Some(Held::Off(why)) = held {
        // **Four blank rows, not two** (`screens/help.md` § Under a dead-writes run): the cause's
        // sentence replaces the `s` row, and `r`, `ctrl-d` **and the two `works on …` lines** go
        // with it. A kind list under a key that is off for the whole run answers a question nobody
        // can reach. The block keeps the six rows it has everywhere else, so the body stays
        // sixteen and `screens/widgets.md` § 1's budget is untouched.
        clauses.push((SCALE_ROW, format!("    {why}")));
        clauses.push((SCALE_KINDS, String::new()));
        clauses.push((RESTART_ROW, String::new()));
        clauses.push((RESTART_KINDS, String::new()));
        clauses.push((DELETE_ROW, String::new()));
    } else if heading.is_none() {
        if refused.scale() {
            clauses.push((
                SCALE_ROW,
                format!(
                    "    s       run more or fewer copies   (scale — {} {resource}/scale)",
                    Refused::SCALE_VERBS.join("+")
                ),
            ));
        }
        if refused.restart() {
            clauses.push((
                RESTART_ROW,
                format!(
                    "    r       restart, at its own pace   (rollout restart — {} {resource})",
                    Refused::RESTART_VERBS.join("+")
                ),
            ));
        }
        if refused.delete() {
            clauses.push((
                DELETE_ROW,
                format!(
                    "    ctrl-d  delete — you type the name to confirm ({} {resource})",
                    Refused::DELETE_VERBS.join("+")
                ),
            ));
        }
    }
    let mut rows: Vec<&str> = help.split('\n').collect();
    for (key, clause) in &clauses {
        if let Some(row) = rows.iter_mut().find(|row| row.starts_with(key)) {
            *row = clause;
        }
    }
    rows.join("\n")
}

/// **The row anchors [`key_map`] finds**, spelled once so no two rewrites can come to disagree
/// about which row is which. `X`'s is written where it is used, because only one rewrite reads it.
const CHANGING_HEADING: &str = "  Changing things";
const SCALE_ROW: &str = "    s ";
const RESTART_ROW: &str = "    r ";
const DELETE_ROW: &str = "    ctrl-d ";

/// **What `s` and `r` work on, and the one place in this file that says it**
/// (`screens/help.md` § Rules). A reader who watched `s` leave the footer because the selected kind
/// has no `/scale` has nowhere else to ask *why* — the row it sat on is gone — so `?` closes the
/// loop, which `(scale)` alone never did (`crate::views::Offer::Act`).
///
/// **Each is `works on ` followed, character for character, by `ops.rs`'s own refusal sentence** —
/// its private `SCALABLE` and `RESTARTABLE`, the text `ops::scalable` and `ops::restartable` put
/// in the `Err` they hand a headless run. **The second copy is deliberate and it is not
/// avoidable**: those constants are private to `ops.rs`, which froze at the end of Phase 7, so
/// nothing here can read them. **Verbatim and never reworded** for exactly that reason —
/// `tester`'s guard compares these two literals with `ops.rs` as text, and a rephrasing is a copy
/// it cannot pin. Change one and the other has to move in the same commit.
///
/// **They double as [`key_map`]'s anchors for those two rows**, which have no key to be found by:
/// the whole line is the anchor, and the two differ in their last word, so neither can match the
/// other's row.
///
/// **`ctrl-d` has no such line**, and that is the screen file's ruling rather than an omission:
/// `ops.rs`'s `DELETABLE` names all six kinds this product ships, so delete is never withheld for
/// a kind's own sake and there is no *why did this vanish* for Help to answer.
const SCALE_KINDS: &str = "            works on a deployment, a statefulset and a replicaset";
const RESTART_KINDS: &str = "            works on a deployment, a statefulset and a daemonset";

/// The heading's clause while writes are dead for the run (`screens/help.md` § Under a dead-writes
/// run).
const OFF: &str = "off for this whole run";

/// The clause a call in flight puts on the heading and on `X` (`screens/help.md` § While the call
/// is running).
const RUNNING: &str = "paused while a change is running";

/// **Why the *Changing things* block cannot be used as a whole, for the run** — [`withheld`]'s
/// answer, which [`key_map`] words and [`offered`] withholds `s` and `r` for (NOTES § D265 rulings
/// 4 and 5). A call in flight is not one of these: it is `crate::views::App::changing`'s fact, and
/// [`key_map`] ranks it between the two.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    /// **Writes are dead for the whole run**, carrying [`Writes::why`]'s sentence for the `s` row.
    Off(&'static str),
    /// **No write can start for now** — the link or the clock — carrying the heading's clause.
    Paused(&'static str),
}

// --- THE NUMBERS THE MOCKUPS ARE DRAWN TO END ---

// --- WHAT A FRAME IS DRAWN FROM START ---

/// **What the frame needs that [`App`] does not hold** — one borrowed value per frame, built by
/// the caller and dropped with it.
///
/// `App` is what the *user* did; this is what the *store* answered, plus the two header zones,
/// which are assembled where the facts in them live. Nothing here is stored, and nothing here is
/// a decision: every string arrives already worded and already stripped.
///
/// **[`Screen::detail`] is the one field that is deliberately the other way round, and this
/// sentence used to be false because of it** (NOTES § D254). A detail tab carries *typed* values —
/// a `k8s::LogLines`, a `k8s::Happened`, a `PodSnapshot` — and the wording is done here, out of
/// `crate::views`. Handing them over pre-worded would put the wording back in `main.rs`, one layer
/// **above** the file that draws it, where the drawn pane and the temporary driver would each keep
/// their own copy of the same sentence. **Already stripped still holds of every one of them.**
pub struct Screen<'a> {
    /// How much colour the terminal admits to — [`theme::depth`] of `COLORTERM`, read once by
    /// the caller so this file can be proven without an environment (the same reason `Snapshot`
    /// carries its own clock, invariant 5).
    pub depth: Depth,
    /// The header's left zone — `nodes 3/3`, `nodes …`, or empty for a reader who cannot list
    /// nodes. **A vital that cannot be read is blank, never guessed** (`screens/widgets.md`
    /// § 1a), which is a rule about what the caller puts here.
    pub vitals: &'a str,
    /// The header's right zone **up to the connection state**, already joined — `ctx: prod-eu ·
    /// live`, and on a longer row `ctx: prod-eu · ns: payments · live`.
    ///
    /// **Laid out first, and the last zone to give way**: `prod-eu` and `prod-eu-2` differ by one
    /// character. Where the whole row is not enough for it, what gives way is the **front** of
    /// the zone — see [`shortened`], which is where the reason that end and not the other is
    /// written down. So the caller's order inside it is not cosmetic: **the cluster's name goes
    /// first.**
    ///
    /// **What the reader may do here is not in it, and [`header`] joins it on** (NOTES § D265
    /// rulings 1 and 2): `admin` or `read-only` off [`Screen::writes`], `⚠ TLS not verified` off
    /// [`Screen::insecure`], then `changing…` — `screens/widgets.md` § 1a's last three segments, in
    /// its order. A caller holding that join could say `admin` over dead keys.
    pub context: &'a str,
    /// **The connected context's kubeconfig sets `insecure-skip-tls-verify`**, which [`header`]
    /// draws as the zone's TLS warning. Its writer is Phase 12, from the `current` row of
    /// `k8s::contexts(&kubeconfig, context)` (NOTES § D265 ruling 8).
    pub insecure: bool,
    /// The Alerts list, in the three answers a pane has (`crate::views::Pane`) — and a refusal
    /// carries whatever did come back with it, which is what the banner is drawn *over*.
    pub alerts: &'a Pane<Vec<Card>>,
    /// **The browser's answer for the kind [`crate::views::View::Resources`] names**, in the same
    /// three (`screens/resources.md`). One `Table` and not one per kind: the caller fetches the
    /// open kind and nothing else, because a view that is not open is not watched.
    ///
    /// **Nothing here says which kind it is** — that is `app.view`'s index into
    /// [`Screen::kinds`], so *which kind* and *which answer* cannot come apart in this struct the
    /// way `crate::views::View` already refuses to let them (invariant 12).
    pub browser: &'a Pane<crate::k8s::Table>,
    /// **The one namespace everything is scoped to, or `None` for every namespace** — from
    /// `--namespace/-n` or the 403 fallback, never from a kind.
    ///
    /// It is the second of the two conditions the `ns:` label needs, [`Browsable::namespaced`]
    /// being the first: *a namespaced kind browsed with no scope reads the same as a
    /// cluster-wide one* (`screens/resources.md` § Rules). It is not derived from
    /// [`Screen::context`], which already spells the same fact for the header — a string is not a
    /// value, and parsing one back is how two zones start disagreeing.
    pub namespace: Option<&'a str>,
    /// The snapshot's moment, so an age drawn here and an age sorted on in
    /// [`crate::views::cards`] are the same answer (NOTES § D246 ruling 4).
    pub now: &'a Time,
    /// **The paragraphs an empty or still-loading pane draws**, in order, with a blank line
    /// between them. They count what the store actually read — *"84 pods and 3 nodes checked…"*,
    /// *"reading the cluster… 2,140 pods"* — which is a fact about the store and not about the
    /// screen, so the sentence is assembled where the numbers are (`screens/states.md`).
    ///
    /// **Order is the caller's, and it is the rank**: on Alerts the audit sentence is drawn above
    /// every one of these, and when the 13 rows run out it is the *last* paragraph here that gives
    /// way (`screens/states.md` § On a healthy or a still-loading Alerts screen). A pointer the
    /// reader can do without belongs at the end.
    pub note: &'a [String],
    /// Every browsable kind the cluster said it serves, for the sidebar's rows under an open
    /// group. **Never a list written here** (invariant 12).
    pub kinds: &'a [Browsable],
    /// One entry per analysis report, in sidebar order: its label, and the report itself once
    /// there is one.
    ///
    /// **The label is not on [`Report`] on purpose** — that type's own doc says so — and
    /// `views.rs` does not hold one either, so it arrives from the caller until somebody gives it
    /// a home. **The badge is not carried beside it** and is read off [`Report::badge`], so the
    /// sidebar's value and the pane under it are one claim seen twice, the way [`found`] already
    /// makes the Alerts badge and the browser's marks one.
    ///
    /// **`None` is *the store has not answered yet*, and it is the only pane state this level
    /// carries** — there is no [`Pane`] here, because a report is a pure function of the
    /// snapshot and not a fetch of its own: *we were not allowed to look* is already inside the
    /// report as a [`ReportRow::NotComputed`], in that report's own words
    /// (`screens/analysis.md` § What each report needs), and *there is nothing to say* is
    /// already inside it as one [`ReportRow::Prose`] (that file's rule 8). What is left is the
    /// moment before the first LIST returns, which no snapshot can express, and the caller says
    /// it by having no report to hand over.
    ///
    /// **The label survives that moment and the badge does not**, which is what
    /// `screens/states.md` § *Still loading* draws: seven ANALYSIS entries, `certificates  30d`
    /// beside one of them because C1 reads a file on disk, and no value beside the six that need
    /// the cluster.
    ///
    /// **One entry, one pane — and `screens/analysis.md` draws six panes for seven entries**:
    /// Versions is drawn at the foot of the Certificates pane, keeps its own sidebar entry, and
    /// its own `title` is never drawn (that file's head, and § *Certificates and Versions*). This
    /// slice cannot say that yet, and it is not this file's to say: *which panes exist, and which
    /// of them share one*, is `screens/`'s ruling
    /// ([`crate::analysis::Report`]'s own doc), and the labels beside these badges have no home
    /// either. Both are the same missing piece — which entries exist, what each is called, and
    /// which pane it opens — and it is one ruling away, not one field. **Nothing below needs
    /// changing when it lands**: a pane is whatever rows arrive, so two reports sharing one is a
    /// longer `rows` and a title taken from the first, which is exactly what [`analysis`] already
    /// draws.
    pub reports: &'a [(&'a str, Option<&'a Report>)],
    /// The command log, oldest first. The strip draws the last [`LOG_LINES`] of it — display
    /// text, never executed and never fed back into a process (invariant 4).
    pub log: &'a [String],
    /// **Which of the three mutating keys this login has been told it may not use, for the
    /// selected object** (`crate::views::Refused`, `screens/widgets.md` § 2a).
    ///
    /// It is here and not on [`App`] for the same reason everything else on this struct is: a
    /// permission answer is what the *store* answered, not what the user did. [`Refused::default`]
    /// — nothing selected, nothing asked, nothing answered yet — draws every key exactly as a run
    /// with no probe at all does (NOTES § D229 ruling 4).
    pub refused: Refused,
    /// **Whether a write can happen at all in this run** ([`Writes`]) — the session half of
    /// *may a mutating key be offered right now*, where [`Screen::alerts`] and [`Screen::browser`]
    /// are the half that moves frame to frame (`crate::views::Offer`, PM ruling 2026-09-12).
    pub writes: Writes<'a>,
    /// **The sentence a clock more than five minutes out of step with the cluster's puts above the
    /// pane, or `None`** (`screens/states.md` § Your computer's clock is off).
    ///
    /// **It is the caller's sentence because both of the facts in it are the store's** — the size
    /// of the gap and which way it runs — and the two directions are two sentences, because they
    /// break differently (that section's own table, NOTES § D177). This file draws whichever
    /// arrived and picks neither. **The `⚠` is part of the sentence**, as it is in every other
    /// banner this pane draws (`Pane::Denied`'s own): it marks the alarmed, left-flush family, and
    /// the calm family drops it — which is a choice about the screen the sentence lands on, so it
    /// belongs to whoever wrote the sentence and not to [`banner`].
    ///
    /// **It withholds `s` and `r` for as long as it holds**, which is that section's own reason: an
    /// age that reads fresher than it is decides which card somebody reacts to first, and reacting
    /// means pressing one of those two keys on it.
    ///
    /// **Nothing is drawn from it while [`Screen::alerts`] is [`Pane::Loading`] or empty**: there
    /// the sentence is one of [`Screen::note`]'s own paragraphs, because *clock skew is drawn in
    /// whichever family the rest of the screen is already in — it does not bring its own* (that
    /// section, § Nothing is broken, and the clock is still off).
    pub clock: Option<&'a str>,
    /// **What the connection to this cluster is doing** ([`Link`]) — the fact
    /// [`crate::views::Pane`] cannot carry, and the one that decides whether k8rs is in a position
    /// to offer a write at all.
    pub link: Link,
    /// **The object a detail tab is open on, and what each of its four fetches answered** —
    /// `None` when nothing is open (`screens/detail.md`).
    ///
    /// **A detail is drawn *over* whatever view is open, which is why it is an `Option` here and
    /// not a fourth [`View`]**: `esc` goes back to the pane the reader came from, and a view that
    /// had to be re-derived to go back to would be a second place holding where they were.
    pub detail: Option<&'a Detail<'a>>,
    /// **The kubeconfig's contexts, while the picker is open** — [`crate::k8s::contexts`]' answer,
    /// read once when it opened and the same list every `crate::views::Picker` method was handed.
    /// Empty when nothing is picking.
    pub contexts: &'a [Choice],
}

/// **Whether a write can happen at all in this run, and the sentence that says why not**
/// (`screens/states.md` § The audit log could not be opened, NOTES § D21, § D231).
///
/// **One value, and the footer, the header's own `read-only` mark and `?`'s *Changing things* block
/// all read it** (PM ruling, 2026-09-12; NOTES § D265). D21 — *k8rs says so and continues in
/// read-only mode; it does not exit* — has never been true of any binary: a headless run has no
/// *continue* to continue into. A TUI can start, draw, and leave the write keys dead, and this is
/// the value that makes them dead: [`offered`] turns it into a `crate::views::Offer` that is not
/// `Act`, which `crate::views::App::may_mutate` then refuses. **Unreachable, not a banner over live
/// keys** — invariant 2's own bar.
///
/// **`--read-only` is the second cause and belongs here as a second variant, not as a second
/// signal.** `screens/states.md` requires *one signal true for both causes* — the header draws the
/// same `read-only` word either way, because its job is to say what is true now and not why. The
/// header asks [`Writes::permission`]; the footer and `?` ask [`withheld`], which reads
/// [`Writes::why`] — and the one place the two causes differ on screen is that sentence.
///
/// **Not a `bool` beside a `String`**: the sentence belongs to the one cause that has one, and two
/// fields are two facts that can disagree. It is `ops::audit_log`'s own returned sentence, cleaned
/// and bounded where it was built (invariant 9, `ops::named`) and never a second one written here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writes<'a> {
    /// Writes are reachable. Whether *this login* may scale *this object* is
    /// `crate::views::Refused`'s separate answer, and the only one that may mark a key `no`.
    Live,
    /// **`--read-only` was asked for, and that is its only cause.**
    ///
    /// **Not a login that may not change anything**, which this doc named as a second cause until
    /// 2026-09-12 and which does not exist: RBAC has no whole-session answer, only per verb,
    /// resource and namespace, and that answer is `crate::views::Refused`'s. **And never a
    /// namespace scope** — `screens/states.md` § You can only see some namespaces now draws a
    /// scoped `live · admin` session with `s scale` and `r restart` live, and states that the scope
    /// is not a permission. A Phase 12 wire from *namespace fallback* to this variant would take
    /// every mutating key from the commonest non-admin shape there is, which is the defect [`Link`]
    /// was introduced to close.
    ///
    /// **It carries no sentence and drops no banner**: [`header`] says `read-only`, and [`help`]'s
    /// *Changing things* row says the flag was asked for ([`Writes::why`], NOTES § D265). Nothing
    /// in product code constructs it yet — the command line reaches it at Phase 12's flags box.
    ReadOnly,
    /// **[`crate::ops::audit_log`] could not open the log**, carrying the sentence it returned.
    Unaudited(&'a str),
}

impl<'a> Writes<'a> {
    /// **The one question every reader asks**, so a second cause is one arm here and no call site
    /// anywhere.
    fn live(self) -> bool {
        matches!(self, Writes::Live)
    }

    /// **`admin` or `read-only` — the header's word for [`Writes::live`]'s answer**, one word for
    /// both dead causes (`screens/widgets.md` § 1a, NOTES § D265 ruling 1).
    fn permission(self) -> &'static str {
        if self.live() {
            "admin"
        } else {
            mark(theme::READ_ONLY)
        }
    }

    /// **Help's row under *Changing things (off for this whole run)*: why, and what brings changes
    /// back** — one fixed sentence per cause, and never the audit sentence [`Writes::said`]
    /// carries, which is the banner's (`screens/help.md` § Under a dead-writes run, NOTES § D265
    /// ruling 5). `None` while writes are live.
    fn why(self) -> Option<&'static str> {
        match self {
            Writes::Live => None,
            Writes::ReadOnly => {
                Some("k8rs was started with --read-only — quit and start it again without it")
            }
            Writes::Unaudited(_) => {
                Some("k8rs could not open its audit log — fix that, then start k8rs again")
            }
        }
    }

    /// The banner's own sentence, or nothing to draw.
    fn said(self) -> Option<&'a str> {
        match self {
            Writes::Live | Writes::ReadOnly => None,
            Writes::Unaudited(said) => Some(said),
        }
    }
}

/// **What the connection to this cluster is doing** (`screens/states.md` § The connection dropped,
/// § Your login expired, NOTES § D19).
///
/// **It is a session fact and [`crate::views::Pane`] cannot hold it**, which is the defect this
/// type closes (`k8s-admin`, 2026-09-12). `Pane::Denied` is *what this pane was answered*, and its
/// own doc says it is the namespace-scoped fallback as much as the refusal — so reading a `Denied`
/// as *we cannot reach the cluster* took every mutating key away from the commonest non-admin shape
/// there is: a developer with a `RoleBinding` in one namespace, whose `may_i` answers `Yes` and
/// whose header reads `live · admin`. The link is what decides that, and a `String` in a pane
/// cannot say it.
///
/// **No `Default`, on purpose** (`k8s-admin`, 2026-09-12, round two): the only default there is to
/// derive is `Live`, the one answer that leaves `s` and `r` live over stale cards, and a later
/// `..Default::default()` would pick it without anyone deciding to. Every [`Screen`] names its
/// link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Link {
    /// Requests are completing.
    Live,
    /// **The stream is gone and k8rs is retrying.** Stale data stays visible and stays labelled;
    /// what it costs is the two mutating keys, because k8rs cannot ask whether a write would be
    /// allowed and `s no scale` would claim a verdict nobody gave.
    Lost,
    /// **The kubeconfig's short-lived token ran out** — not a 403 and not a dropped socket. It
    /// costs the same two keys and promotes `X switch cluster` onto the footer, because renewing
    /// and reconnecting is *the* next step.
    Expired,
}

/// **What the four detail tabs were answered**, one field per tab (`screens/detail.md`).
///
/// **Typed values and not sentences, which is the opposite of what [`Screen`]'s own doc says
/// about everything above it** (NOTES § D254). Pre-wording these would put the wording back in
/// `main.rs`, one layer above the file that draws them — and `crate::views` is where the shared
/// half of it now lives precisely so that the drawn pane and the temporary driver cannot say one
/// fact two ways.
///
/// **One field per tab, so *which tab is open* and *which content arrived* cannot come apart.**
/// [`crate::views::App::tab`] picks the field; nothing in here claims to be a particular tab, the
/// same way [`Screen::browser`] carries no kind of its own. An enum of contents would let
/// `Tab::Yaml` be open over a `Content::Logs`, which is the disagreement this shape refuses to be
/// able to express.
///
/// **[`Detail::events`] is one fetch with two readers** — the events tab *and* describe's own
/// events block — which is `screens/detail.md`'s own rule: *"One function, two callers, one
/// order — newest first — settled once."* A second field for describe's copy is how the two come
/// to disagree about the order.
pub struct Detail<'a> {
    /// Which object, for the line above the tab row. **An id and not a name**, so the
    /// `namespace/name` spelling is [`name`]'s one answer and not a second one
    /// (`screens/README.md` § the five rules).
    pub object: &'a ObjectId,
    /// The logs tab's stream and everything the header line over it says.
    pub logs: &'a Pane<Logs<'a>>,
    /// **The describe tab's own fresh, unpruned read** — never the watch store, which is pruned
    /// to the fields `rules.rs` names (invariant 6, `screens/detail.md` § The describe tab).
    pub read: &'a Pane<Described<'a>>,
    /// **The yaml tab's document, as [`crate::k8s::Document::yaml`] emitted it** — masked,
    /// stripped by `k8s::clean` and serialised before it got here.
    ///
    /// **A `String` and not a `Document`, which is not the same compromise as pre-wording a
    /// sentence**: YAML emission is `serde_yaml_ng`'s answer and the masking is `k8s.rs`'s, so
    /// nothing k8rs *says* is decided by the caller — and the emitter's own `Err`, which
    /// `screens/detail.md` draws no pane for, stays with the caller that already has an exit code
    /// for it rather than becoming a screen this file invented.
    pub yaml: &'a Pane<String>,
    /// **This object's own events, newest first** — describe's second read and the events tab's
    /// only one ([`crate::k8s::events`]).
    pub events: &'a Pane<crate::k8s::Happened>,
    /// **A `Secret` whose `data` holds no keys** (`screens/detail.md` § A Secret with no keys).
    ///
    /// **It is a fact from the caller because the pane cannot reach it.**
    /// [`crate::k8s::Document`] hands over YAML and nothing else, and re-reading `data: {}` back
    /// out of the rendered document would be exactly the *split a rendered string back into
    /// values* this build refuses (NOTES § D245). The caller reads it off the object it fetched;
    /// the sentence drawn from it is this file's.
    pub secret_without_keys: bool,
}

/// **The object describe draws**, which is the pod plus the one pairing this file must not make
/// for itself (`screens/detail.md` § The describe tab).
///
/// **`k8s::PodRead` is what the caller holds and this is what it hands over**, because that type
/// has no constructor outside `k8s.rs` — which is frozen — and a pane no test can build is a pane
/// nobody has drawn. What is lost is nothing the screen reads: `snapshot` is the same value
/// `PodRead` carries, and `containers` is `PodRead::declared()` zipped with `PodRead::status()`,
/// which keeps **that** type's by-name lookup as the only one and stops a second by-index reading
/// of `status.containerStatuses` growing here.
pub struct Described<'a> {
    /// The pod as the rules see it — the fresh read's snapshot, not the watch store's.
    pub snapshot: &'a PodSnapshot,
    /// **One entry per *declared* container, in `spec` order**, with the kubelet's report on it or
    /// `None` where it has not reported one — a `Pending` pod.
    ///
    /// **The order is `spec.containers[]` then `spec.initContainers[]`, and never
    /// `status.containerStatuses`'**, which the kubelet sorts by name: choosing off the snapshot
    /// opened `alpha` where `kubectl logs` opens `zeta`, and `[web, envoy]` opened the proxy
    /// (`k8s-admin`, 2026-08-30). It is [`crate::k8s::PodRead::declared`]'s answer, carried
    /// rather than re-derived.
    pub containers: &'a [(&'a str, Option<&'a ContainerSnapshot>)],
}

/// **What the logs tab is showing**, which is a stream plus the facts its header line says
/// (`screens/detail.md` § The logs tab).
pub struct Logs<'a> {
    /// **The pod, so the header says what it *has* rather than being told.** Whether there is a
    /// container to pick is how many entries [`Described::containers`] holds, and whether `⇧p`
    /// had a previous run to show is the chosen container's restart count — both read here rather
    /// than carried as bools somebody else computed.
    pub pod: &'a Described<'a>,
    /// The container being read — `kubectl`'s own default where the reader named none
    /// ([`crate::k8s::PodRead::default_container`]).
    pub container: &'a str,
    /// `⇧p` — whether the run *before* the last restart was asked for.
    pub previous: bool,
    /// **The bounded buffer**: at most [`crate::k8s::LOG_BYTES`], [`crate::k8s::LOG_LINES`] lines,
    /// each already cut at `k8s::FREE_TEXT` and stripped on the way in. Nothing here holds a log
    /// line; this borrows the one the caller keeps.
    pub held: &'a crate::k8s::LogLines,
}

impl Screen<'_> {
    /// A foreground in one of `theme.rs`'s roles, at this terminal's depth.
    fn fg(&self, role: Colour) -> Style {
        Style::new().fg(ink(role, self.depth))
    }
}

/// One palette role as a terminal can be told to draw it — **the whole of the `theme.rs` →
/// `ratatui` mapping for colour** (NOTES § D241).
///
/// [`Ink::Default`] becomes `Color::Reset` and not a guess at the user's background: it is the
/// answer for the roles that paint an area, because k8rs does not know the terminal is dark
/// (PRIOR-ART § D2).
fn ink(role: Colour, depth: Depth) -> Color {
    match role.ink(depth) {
        Ink::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
        Ink::Ansi(index) => Color::Indexed(index),
        Ink::Default => Color::Reset,
    }
}

/// The text of a [`Signal::Mark`] — the whole of the mapping for the non-colour half.
///
/// [`Signal::Reverse`] is a style and not a string, so it has no text to give; it is drawn by
/// reversing a span where a screen asks for it, and every caller below wants a mark.
fn mark(signal: Signal) -> &'static str {
    match signal {
        Signal::Mark(text) => text,
        Signal::Reverse => "",
    }
}

/// A band's mark in the [`GUTTER`] it is drawn in — the glyph, then the column that keeps it off
/// the name. Padded rather than concatenated so a band whose mark is wider than one column still
/// leaves the gutter the width every plain row reserves.
fn glyph(signal: Signal) -> String {
    format!("{:<width$}", mark(signal), width = GUTTER)
}

/// **`⚠ TLS not verified`, spelled once** — the picker's badge on a context's row ([`context`]) and
/// the header's segment for the connected one ([`header`]) are one warning, and neither keeps a
/// copy of it (NOTES § D265 ruling 2).
fn unverified() -> String {
    format!("{} TLS not verified", mark(theme::ALARM))
}

/// **The cards a screen may read, which is two of the three answers and not three.**
///
/// A pane that has not answered has nothing to count and nothing to mark — *blank, never guessed*
/// (`screens/widgets.md` § 1a) — and a refused one carries whatever *did* come back, which is what
/// `screens/states.md` § *You can only see some namespaces* draws: the banner, and `3 ● 7 ▲`
/// beside `ALERTS`.
///
/// **One function because the sidebar's badge and the browser's marks are one claim seen twice.**
/// A badge reading `3 ●` over a browser that marks nothing is exactly the disagreement
/// `screens/resources.md` § Rules exists to forbid, and two copies of this match is how it would
/// arrive.
fn found(alerts: &Pane<Vec<Card>>) -> &[Card] {
    match alerts {
        Pane::Ready(cards) | Pane::Denied(_, cards) => cards,
        Pane::Loading => &[],
    }
}

/// **The containers `c` picks between, in `spec` order** — empty where no detail tab is open, and
/// empty where one is open over a pod whose `crate::rules::PodSnapshot` has not reached the store
/// yet (`screens/detail.md` § The logs tab, before the container list is known).
///
/// **One read, so the logs header's `▾`, the footer's `c container` and the picker's own box
/// cannot disagree about whether there is anything to pick.** A pod deleted under an open picker
/// arrives here as the same emptiness a single-container pod does, which is what closes the box
/// (§ The pod disappears while the picker is open) — there is no second fact to carry and no
/// second `Gone` to draw.
fn containers<'a>(screen: &Screen<'a>) -> &'a [(&'a str, Option<&'a ContainerSnapshot>)] {
    match screen.detail.map(|open| open.logs) {
        Some(Pane::Ready(logs) | Pane::Denied(_, logs)) => logs.pod.containers,
        _ => &[],
    }
}

// --- WHAT A FRAME IS DRAWN FROM END ---

// --- THE FRAME START ---

/// **Draw the whole screen.** The one entry point, called once per event by the loop
/// (invariant 7 — there is no frame rate).
pub fn draw(frame: &mut Frame, app: &App, screen: &Screen) {
    let area = frame.area();
    // **`BACKGROUND` and `TEXT` are painted together or not at all** (`theme.rs` § THE PALETTE):
    // at sixteen colours both are the terminal's own and this paints nothing, and at 24-bit a
    // near-white text on an unpainted light background is the failure the pairing exists to
    // avoid.
    frame.render_widget(
        Block::new().style(
            Style::new()
                .fg(ink(theme::TEXT, screen.depth))
                .bg(ink(theme::BACKGROUND, screen.depth)),
        ),
        area,
    );

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        frame.render_widget(
            Paragraph::new(Line::from(too_small(area)).centered()),
            area.centered(Constraint::Min(0), Constraint::Length(1)),
        );
        return;
    }

    let [top, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    header(frame, top, app, screen);

    // **`?` is the one mode where the frame's own border carries a title, and it is the whole
    // of how `Keys` gets onto the screen** (`screens/widgets.md` § 5). Help draws no block of
    // its own: a second `Block::bordered()` over the body would spend two of the sixteen rows
    // the key map needs on a border nobody asked for.
    let helping = app.modal == Some(views::Modal::Help);
    let border = screen.fg(theme::BORDER);
    let mut outer = Block::bordered().border_style(border);
    if helping {
        outer = outer.title(TITLE);
    }
    let inner = outer.inner(rest);
    frame.render_widget(outer, rest);

    // The 24 rows, and there is only one way to spend them: 1 header + 1 top border + 16 body +
    // 1 divider + 2 command log + 1 divider + 1 footer + 1 bottom border
    // (`screens/alerts.md` § The height). Nothing in that list is optional, which is why every
    // row but the body is a `Length`.
    let [body, above_log, log, above_keys, keys] = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(LOG_LINES),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let [nav, split, pane] = Layout::horizontal([
        Constraint::Length(SIDEBAR),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(body);

    // **Before the first connection there is no app frame to float over** (`screens/context.md`
    // § Opening at startup): the sidebar belongs to a frame that is not built until a cluster has
    // been picked, so neither pane — nor the divider below — is drawn under the startup picker or
    // the failure it led to. **`X`'s picker is drawn over an empty body too** (NOTES § D264
    // ruling 9): its box is as wide as the body less three columns a side, and a pane left under
    // it showed through those margins letter by letter.
    let bare = app.connecting_first() || matches!(app.modal, Some(views::Modal::ContextPick(_)));
    if !bare {
        sidebar(frame, nav, app, screen);
        content(frame, pane, app, screen);
    }
    // **Drawn over the body the normal pass just filled, and not instead of it** —
    // `screens/widgets.md` § 5's own draw order. Under it the `Clear` inside [`help`] and
    // [`boxed`] is load-bearing: their lines are shorter than the region they are drawn in, and a
    // `Paragraph`'s style paints past its text while its symbols do not, so without the `Clear`
    // the sidebar and the pane show through beside them. That is checked, and it is what this
    // box's first draft got wrong by drawing no `Clear` at all.
    //
    // **A dialog leaves the sidebar and the pane showing and [`help`] does not**, which is the
    // whole of § 5's *Help has no sidebar or content pane left showing to float over*: every
    // other modal is a small box over a screen that is still there. The mockups on
    // `screens/dialogs.md` draw an empty body behind their boxes because the box is what they
    // are about, not because the screen underneath goes away.
    //
    // **Skipping the pass underneath instead would draw the identical frame, and nothing here
    // claims otherwise** (`tester`, 2026-09-10: the two orders were rendered and diffed byte for
    // byte). Telling them apart needs a spy on [`sidebar`] and [`content`], which asserts a call
    // and not a screen — so this is the spec's order followed, not a behaviour a test defends.
    strip(frame, log, screen);
    footer(frame, keys, app, screen);

    // The two rules and the sidebar's divider are drawn last, over the panes' own edges, so the
    // frame is one shape rather than four blocks that happen to touch.
    rule(frame, rest, above_log.y, border);
    rule(frame, rest, above_keys.y, border);
    // **The divider is the one part of the frame Help does not keep**: it is drawn *after* the
    // panes, so leaving it in would rule a sidebar edge straight down the cleared key map, and
    // `screens/help.md` draws the body as one field with no sidebar left to divide off.
    if !helping && !bare {
        divider(frame, split.x, rest.y, above_log.y, border);
    }

    // **The modal is the last thing drawn, after the frame has been ruled, and that is the fix
    // for a defect this box's first draft shipped** (2026-09-10). The divider runs *through* the
    // body and is drawn over the panes so its junctions land on the borders; a dialog drawn
    // before it came out with a sidebar edge ruled straight down the middle of its own box.
    // Nothing outside the body is touched here, so the header, the log strip and the footer are
    // as untouched as [`help`]'s own note says (`screens/widgets.md` § 1).
    if let Some(open) = &app.modal {
        modal(frame, body, open, app.changing.is_some(), screen);
    }
}

/// **The keys valid right now** — one line, and the only one in the frame with a right-hand
/// zone (`screens/widgets.md` § 2a).
///
/// **Nothing here chooses a word.** [`crate::views::App::footer`] answers with both zones and
/// this places them, so a mode's footer is spelled once, in the file that already knows which
/// keys a mode has. **One string reaching it is not a literal**, and it is the only cut this
/// function makes: the in-flight footer's object name (`screens/dialogs.md` § *While the call is
/// running*, `screens/widgets.md` § 7's identity cut). Every other footer is
/// *curated to fit* — the curation happens in the words, not in a truncation here.
///
/// **`room` is measured off this line's own fixed parts and never restated as a number** (PM
/// ruling, 2026-09-12, and `screens/resources.md` § *When it does not fit* rule 1: *the same
/// measurement already made off the spans about to be drawn, not a sum restated separately*).
/// The same footer is asked for twice — once with an empty name, whose width **is** the fixed
/// prefix and suffix, and once with what is left of the name after [`name_cut`]. The arm is
/// [`crate::views::App::changing`]'s and never the argument's, so the empty first call cannot
/// draw a different line from the one being measured. The second call is reached only with a
/// write on the wire, and every arm that does not interpolate the name — every modal, and the
/// three modes that keep their own footer less `q quit` — hands back what the first call already
/// said, so the pair needs no guard of its own.
///
/// **[`name_cut`] and not [`command_cut`]**: a name is one token — or two joined by one `/` —
/// so there is no word boundary to walk back to, and it is the one cut every surface naming an
/// object shares (NOTES § D266): the `/` kept, and the fronts given up where two names are alike.
///
/// **The right zone is laid out first and takes exactly its own width**, the same shape
/// [`header`] uses for the context: an empty one is a zero-width `Rect` that draws nothing, so
/// the ordinary single-zone footer needs no branch of its own.
fn footer(frame: &mut Frame, area: Rect, app: &App, screen: &Screen) {
    let dim = screen.fg(theme::DIM);
    let row = indented(area);
    let offer = offered(app, screen);
    let picks = screen.detail.map(|_| containers(screen).len());
    let (line, quit) = app.footer(picks, offer, screen.refused, "", screen.contexts);
    let room = usize::from(row.width).saturating_sub(width(&line));
    // **Two arms interpolate one caller-cut string and never both in one frame** — a filter has
    // focus, or a write is on the wire. `s` is a character while a filter is being typed, so a
    // mutation cannot be started from inside one, and `/` is not on the in-flight line.
    let (keys, caret) = match (app.typing, &app.changing) {
        // **The buffer gives way and the two hints never do** (`screens/widgets.md` § 2b): `⏎
        // done  esc clear filter` is what says how to leave, and a buffer that pushed it off the
        // line would answer *what did I type* at the cost of *how do I get out*. Front-cut, so
        // the cursor stays the last column drawn — [`shortened`], the same end a name gives way
        // from and the same end a shell's own line editor keeps.
        (Some(field), _) => {
            let shown = shortened(app.typed().map_or("", Input::text), room);
            let caret = width(field.label()) + width(": ") + width(&shown);
            let line = app
                .footer(picks, offer, screen.refused, &shown, screen.contexts)
                .0;
            (line, Some(caret))
        }
        (None, Some(object)) => (
            app.footer(
                picks,
                offer,
                screen.refused,
                &name_cut(object.namespace.as_deref(), &object.name, room),
                screen.contexts,
            )
            .0,
            None,
        ),
        (None, None) => (line, None),
    };
    let [left, right] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(width(quit) as u16)]).areas(row);
    frame.render_widget(Paragraph::new(Line::styled(quit, dim)), right);
    frame.render_widget(Paragraph::new(Line::styled(keys, dim)), left);
    // **The cursor is the terminal's own and not a drawn character** (`screens/widgets.md` § 2b)
    // — it is the only thing on this frame that says *the keyboard is going here*, and a `_` in a
    // footer is a character a filter could legitimately contain. **Clamped into the row it
    // belongs to**, because a cursor outside its `Rect` is a cursor in somebody else's line.
    if let Some(caret) = caret {
        let at = u16::try_from(caret).unwrap_or(u16::MAX);
        let last = left.x.saturating_add(left.width.saturating_sub(1));
        frame.set_cursor_position((left.x.saturating_add(at).min(last), left.y));
    }
}

/// **Which footer shape this frame is in** ([`Offer`]) — the one place a screen becomes one, so the
/// line that is drawn and the key that is live cannot disagree: `crate::views::App::may_mutate` is
/// handed this same value.
///
/// **Five facts decide it and none stands in for another** (PM rulings, 2026-09-12;
/// `screens/widgets.md` § 2a, extended 2026-09-18). *Is there anything to act on* is read off the
/// open pane and moves frame to frame. *Is the connection answering* is [`Screen::link`] and
/// nothing else — **never [`Pane::Denied`]**, which is also the namespace-scoped fallback, and
/// reading it as *we cannot reach the cluster* took both mutating keys from a developer whose
/// `RoleBinding` allows them. *Can a write happen in this run* is [`Screen::writes`]. *Can the
/// times on the page be trusted* is [`clock`]. Those three are [`withheld`]'s, and any one of them
/// answering no gives `Move`, with no key marked `no`: that mark is reserved for
/// `crate::views::Refused`.
///
/// **The fifth is *what kind is under the cursor*, and it is the only one that answers per key**
/// — a Node cannot be scaled by anyone, a DaemonSet has no `/scale`, and most of what the browser
/// can reach supports neither. It is read here rather than handed over on [`Screen`] for the
/// reason every other fact on this line is: the value that draws the footer and the value
/// `crate::views::App::may_mutate` refuses a press from are one, so the pane's own highlight
/// decides both. `crate::views::Offer::act` is what turns the kind into the pair, by asking
/// `crate::ops` (invariant 12).
///
/// **`switch` is set wherever the login has expired, whatever the pane is drawing** — an expired
/// login over a still-loading pane is `Nothing { switch: true }` and over an empty kind
/// `Filter { switch: true }`, because `X` never acted on a selected row and *nothing is selected*
/// was never its condition (`screens/states.md` § Over a pane with nothing to show yet).
///
/// **Analysis and an open detail tab return `Move { switch: false }`, and that value is not idle.**
/// Their footers are drawn above it in `crate::views::App::footer` from their own closed lines, so
/// no footer is built from it — but `may_mutate` is handed it, and it is what leaves no mutating
/// key pressable behind a footer that names none (PRIOR-ART § G2).
pub fn offered(app: &App, screen: &Screen) -> Offer {
    let switch = screen.link == Link::Expired;
    // **A mode whose own footer names no mutating key may not leave one pressable behind it**
    // (PRIOR-ART § G2 — *read-only enforced per view is a hole per view*, and k9s #3858 is that
    // hole in its XRay view). Analysis and the detail tabs answer above this value in
    // `crate::views::App::footer`, so what they draw is unchanged; what changes is that
    // `crate::views::App::may_mutate` now says no from them, instead of acting on whatever the
    // Analysis cursor happens to point at the moment Phase 12 binds `s` globally.
    if screen.detail.is_some() {
        return Offer::Move { switch: false };
    }
    // **`/` and `n` are read here as well as in the pane, because the two have to agree about
    // whether there is a row to act on** (`screens/states.md` § The filter hides every row): a
    // footer offering `⏎ open` over a list a filter emptied is the broken promise this value
    // exists to refuse, and `crate::views::App::may_mutate` is handed the same answer.
    // **Only a list that *has* rows can reach [`Offer::Hidden`]** — a kind with no objects in it
    // is [`Offer::Filter`]'s state and says something else entirely, and a healthy Alerts pane is
    // `○ nothing is broken`'s.
    let hidden = |browsing| Offer::Hidden {
        switch,
        namespace: app.filters.clears() == Some(Typing::Namespace),
        browsing,
    };
    let selected: Option<(&str, Cow<'_, str>)> = match app.view {
        View::Analysis(_) => return Offer::Move { switch: false },
        View::Alerts => match screen.alerts {
            Pane::Loading => return Offer::Nothing { switch },
            // **Alerts' own empty pane keeps the cursor keys where the browser's loses them**, and
            // that is each section's own wording rather than a rule derived here: *the sidebar's
            // own rows are still there to move across and open* once its badges have settled
            // (`screens/states.md` § Nothing is broken) against *there is nothing to move a cursor
            // across* (§ An empty kind in the browser).
            //
            // **Both answers are read the same way, and a refusal is not one of the reasons a key
            // is withheld** ([`Link`]): `Pane::Denied` carries whatever did come back, and a
            // namespace-scoped developer's cards are as selectable as anybody's.
            Pane::Ready(cards) | Pane::Denied(_, cards) => {
                let shown = shown_cards(app, cards);
                if !cards.is_empty() && shown.is_empty() {
                    return hidden(false);
                }
                // **The card under the cursor, read the way the pane reads it** — [`alerts`]
                // builds the same anchors over the same filtered list, so the kind asked about
                // here is the object the highlight is on and not the first card in the store. An
                // empty list answers `None`, which is *nothing is selected* and the one state this
                // value could not express before (`crate::views::Offer::Move`).
                //
                // **`get` and not an index**, though `Cursor::selected` clamps into the slice it
                // was handed: a panic on a draw is the one failure this file cannot recover from,
                // and nothing is bought by depending on that clamp twice.
                let anchors: Vec<Option<&str>> = shown.iter().map(|_| None).collect();
                app.content
                    .selected(&anchors)
                    .and_then(|at| shown.get(at))
                    .map(|card| {
                        let (group, kind) = addressed(&card.owner.kind);
                        (group, Cow::Borrowed(kind))
                    })
            }
        },
        View::Resources(at) => match screen.browser {
            Pane::Loading => return Offer::Nothing { switch },
            // **Zero rows is zero rows whichever answer holds them** — a 403 on `list jobs` that
            // came back with nothing promised `⏎ open` over fourteen blank rows until this arm
            // read both (`k8s-admin`, 2026-09-12).
            Pane::Ready(table) | Pane::Denied(_, table) if table.rows.is_empty() => {
                return Offer::Filter { switch };
            }
            Pane::Ready(table) | Pane::Denied(_, table) => {
                if shown_rows(app, table).is_empty() {
                    return hidden(true);
                }
                // **Every row in this pane is of the open kind, so the cursor is not asked** —
                // `Screen::browser` is one `Table` for the kind `View::Resources` names, and *which
                // kind that is* is this index into [`Screen::kinds`] and nothing else (that
                // field's own doc, invariant 12). **Lowercased, not [`Browsable::plural`]**: the
                // word `crate::ops` matches on is the singular a manifest spells, and `plural` is
                // the URL path's.
                //
                // **[`Browsable::group`] goes with it, and dropping it was a defect** (`k8s-admin`,
                // `reports/2026-09-18-offer-per-kind-operator-read.md` § 1; NOTES § D51): the kind
                // word alone is not an address. `k8s::browsable` deliberately keeps the same
                // plural under two groups as two resources, so a sidebar row reading
                // `statefulsets` can be `apps/v1`'s or OpenKruise's — and a stock cluster with no
                // CRD at all already serves two `Event`s. `crate::views::Offer::act` is where the
                // two halves are compared.
                screen.kinds.get(at).map(|kind| {
                    (
                        kind.group.as_str(),
                        Cow::Owned(kind.kind.to_ascii_lowercase()),
                    )
                })
            }
        },
    };
    // **Five reasons, one line, and none of them a `Refused` mark**: nothing selected, the three
    // run-level ones [`withheld`] answers — which [`help`] reads too, so the footer and `?` cannot
    // disagree about whether `s` and `r` can be pressed — and, inside `Act`, the selected kind's
    // own (`crate::views::Offer::act`), which takes one key and leaves the other.
    match selected {
        Some((group, kind)) if withheld(screen).is_none() => Offer::act(group, &kind),
        _ => Offer::Move { switch },
    }
}

/// **The address an [`ObjectKind`] is** — its API group and the singular word, the two halves a
/// manifest spells as `apiVersion:` and `kind:` (NOTES § D51). The spelling only: *which*
/// operations serve that address is [`crate::views::Offer::act`]'s question and `crate::ops`'
/// answer, never a list here (invariant 12).
///
/// **The group is not a second fact about the kind, it is half of the one fact.** Each variant of
/// that enum *is* a (group, kind) pair already — `ObjectKind::from_api` builds it from exactly
/// those two and files anything else under `Other` — so this is that table read the other way, and
/// the two cannot come apart. A `deployment` under any group but `apps` is not the object
/// `ops::scale` addresses.
///
/// **[`ObjectKind::Other`] has no address here and says so with two empty words.** Every kind k8rs
/// can point an operation at has its own variant, so an `Other` is by construction not one of them
/// — and empty is the answer that stays true however the API spelled it, where handing the
/// reported string on would put a name from the cluster into a match it could only pass by
/// accident. Nothing is drawn from this, so no strip is owed either (invariant 9).
///
/// **This is the second place that knows the pairing, and it stays here because the first one is
/// frozen.** `rules::ObjectKind::from_api` builds the enum *from* a group and a kind; the inverse
/// belongs beside it, and `rules.rs` closed at the end of Phase 3 — so rather than reopen a frozen
/// file, the copy lives at the one place that needs it and is pinned as a pair by `tester`'s
/// guard. The two cannot drift silently: a variant added to that enum is a compile error here.
fn addressed(kind: &ObjectKind) -> (&'static str, &'static str) {
    match kind {
        ObjectKind::Deployment => ("apps", "deployment"),
        ObjectKind::StatefulSet => ("apps", "statefulset"),
        ObjectKind::DaemonSet => ("apps", "daemonset"),
        ObjectKind::ReplicaSet => ("apps", "replicaset"),
        ObjectKind::Job => ("batch", "job"),
        ObjectKind::CronJob => ("batch", "cronjob"),
        ObjectKind::Node => ("", "node"),
        ObjectKind::Pod => ("", "pod"),
        ObjectKind::Other(_) => ("", ""),
    }
}

/// **The run-level reason no write can start, whatever is selected** — dead writes, then the link,
/// then the clock ([`Held`], NOTES § D265 ruling 4).
///
/// **One function with two readers, so they cannot disagree**: [`offered`] withholds `s` and `r`
/// whenever it answers, and [`help`] words *Changing things* from the same answer. Before it, Help
/// read only whether writes were dead, and promised all three keys under a lost link, an expired
/// login and a skewed clock (`k8s-admin`,
/// `reports/2026-09-13-read-only-mark-operator-read.md` § 1).
///
/// **The clock is read through [`clock`] and not off the field**, so a reading suppressed for being
/// stale cannot withhold a key it is not on screen to justify — and it is why the clock never
/// competes with the link: [`clock`] answers `None` whenever the link is not `Live`.
fn withheld(screen: &Screen) -> Option<Held> {
    if let Some(why) = screen.writes.why() {
        return Some(Held::Off(why));
    }
    match screen.link {
        Link::Lost => Some(Held::Paused("paused while disconnected, retrying")),
        Link::Expired => Some(Held::Paused("paused — renew your login, then press X")),
        Link::Live => clock(screen)
            .map(|_| Held::Paused("paused — the clocks disagree; quit and start k8rs again")),
    }
}

/// **`?` — the full key map, drawn over the whole body region** (`screens/help.md`,
/// `screens/widgets.md` § 5).
///
/// **[`key_map`] and not [`HELP`], because up to three of those rows answer for what this login
/// may do** — the only part of this screen that is not fixed text.
///
/// **[`withheld`] is what turns *Changing things* off or pauses it** — the answer [`offered`] reads
/// for the footer, so `?` never promises a key the footer withholds (NOTES § D265 rulings 3–5).
///
/// **`Clear` first, then a borderless `Paragraph`, and there is no third call.** ratatui does not
/// clear for you, so without it the sidebar and the pane show through; and the block that would
/// normally follow is the one thing § 5 rules out, because the frame's own outer border already
/// carries the title ([`TITLE`]).
///
/// **The background is repainted with the foreground or neither is**, which is `theme.rs`
/// § THE PALETTE's pairing and the same reason [`draw`]'s base block sets both: `Clear` resets
/// the cells this `Rect` had been painted with, and a near-white key map on whatever the terminal
/// happens to sit on is exactly the failure the pairing exists to avoid.
///
/// **The header, the command log strip and the footer are untouched, and that is the layout's
/// doing rather than this function's**: they are siblings of the body in `screens/widgets.md`
/// § 1, not inside the `Rect` handed here — which is why the log strip behind Help keeps showing
/// the real commands the run made (`screens/help.md`, its note under the mockup).
fn help(frame: &mut Frame, area: Rect, changing: bool, screen: &Screen) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(key_map(HELP, screen.refused, changing, withheld(screen))).style(
            screen
                .fg(theme::TEXT)
                .bg(ink(theme::BACKGROUND, screen.depth)),
        ),
        area,
    );
}

/// `screens/widgets.md` § 8 — below the floor there is no layout, one sentence, and the normal
/// frame back the moment the terminal grows.
fn too_small(area: Rect) -> String {
    format!(
        "k8rs needs a terminal at least {MIN_WIDTH}×{MIN_HEIGHT}. This one is {}×{}.",
        area.width, area.height
    )
}

/// `├────┤` across the full width, over the outer block's own side borders.
fn rule(frame: &mut Frame, area: Rect, y: u16, style: Style) {
    let buffer = frame.buffer_mut();
    for x in area.left()..area.right() {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_symbol("─").set_style(style);
        }
    }
    for (x, symbol) in [(area.left(), "├"), (area.right().saturating_sub(1), "┤")] {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_symbol(symbol);
        }
    }
}

/// The sidebar's own edge, `┬` on the top border down to `┴` on the rule under the body.
fn divider(frame: &mut Frame, x: u16, top: u16, bottom: u16, style: Style) {
    let buffer = frame.buffer_mut();
    for y in top..=bottom {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_symbol("│").set_style(style);
        }
    }
    for (y, symbol) in [(top, "┬"), (bottom, "┴")] {
        if let Some(cell) = buffer.cell_mut((x, y)) {
            cell.set_symbol(symbol);
        }
    }
}

/// One column of pad each side of a line inside the frame — the command log and the footer.
///
/// **`Rect::inner` rather than arithmetic of our own**: a hand-written `x + 1` with the width
/// left alone is a line that writes over the frame's right border, and it is the shape a
/// mutation run walks straight through (measured 2026-09-06: *delete field width* survived).
fn indented(area: Rect) -> Rect {
    area.inner(Margin::new(1, 0))
}

/// **Three zones on one line** (`screens/widgets.md` § 1a): vitals left, `k8rs` centred on the
/// full width, context right.
///
/// **The context is laid out first and keeps its full width wherever the row has one**, because
/// it is what tells you which cluster the next keypress is about. The order of sacrifice is the
/// screen file's: the centred name — the only zone carrying no information — then the vitals,
/// and the context only once both are already gone.
///
/// **Both of the zones that give way give way whole.** A `Paragraph` handed a zone narrower than
/// its line is clipped by ratatui with nothing to show for it, and `nodes 3/3 (40s ago)` clipped
/// to `nodes 3/3 (` reads as a complete count of three ready nodes: *a vital that cannot be read
/// is blank, never guessed*. What the context does instead is [`shortened`].
///
/// **The zone's last three segments are joined here and not by the caller, which is what fixes
/// their place in the order** (`screens/widgets.md` § 1a, NOTES § D265 rulings 1 and 2). The zone
/// is one string joined by ` · `: [`Screen::context`] up to the connection state, then
/// [`Writes::permission`] — in every state, so the header cannot say `admin` over dead keys — then
/// [`unverified`] where [`Screen::insecure`] holds, then `changing…` last of all. A caller that
/// joined any of them itself could put it anywhere in that string; a caller that cannot reach the
/// join cannot. `changing…` is [`crate::views::App::changing`]'s fact rather than the store's,
/// which is the other reason it is not a field on [`Screen`].
///
/// **Being the tail is also why [`shortened`] needs no case for them**: that cut eats the *front*
/// of the zone, so the cluster's name erodes and `read-only`, the TLS warning and `changing…`
/// never do.
///
/// **The startup picker is the one state whose zone is not the caller's** (`screens/context.md`
/// § Opening at startup, `screens/widgets.md` § 1a): no context has been chosen, so the zone reads
/// `choose a cluster` and the one fact already known before any connection — whether writes are
/// on for this run — and neither a TLS warning nor the vitals, because no context has been read.
/// The failure that picker can lead to names the context it tried, which is the caller's ordinary
/// zone again.
fn header(frame: &mut Frame, area: Rect, app: &App, screen: &Screen) {
    let dim = screen.fg(theme::DIM);
    let picking = matches!(&app.modal, Some(views::Modal::ContextPick(picker)) if picker.startup());
    let tls = unverified();
    let segments: Vec<&str> = if picking {
        vec!["choose a cluster", screen.writes.permission()]
    } else {
        [
            Some(screen.context),
            Some(screen.writes.permission()),
            screen.insecure.then_some(tls.as_str()),
            app.changing.is_some().then_some(mark(theme::CHANGING)),
        ]
        .into_iter()
        .flatten()
        .collect()
    };
    let whole = segments
        .into_iter()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    let context = shortened(&whole, usize::from(area.width));
    let [left, zone] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(width(&context) as u16),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(Line::styled(context, dim).right_aligned()),
        zone,
    );

    let room = indented(left);
    let vitals = if !app.connecting_first() && width(screen.vitals) <= usize::from(room.width) {
        screen.vitals
    } else {
        ""
    };
    frame.render_widget(Paragraph::new(Line::styled(vitals, dim)), room);

    let name = width(NAME) as u16;
    let at = area.x + area.width.saturating_sub(name) / 2;
    let used = room.x + width(vitals) as u16;
    if at >= used + 2 && at + name + 2 <= zone.x {
        frame.render_widget(
            Paragraph::new(Line::styled(NAME, screen.fg(theme::ACCENT))),
            Rect::new(at, area.y, name, area.height),
        );
    }
}

/// The context zone at the width the row has, **shortened from the left** when it does not fit,
/// behind a [`CUT`] the reader can see.
///
/// **Which end gives way is a security question and not a layout preference** (PM ruling,
/// 2026-09-06). The tail of this zone is `read-only` and `⚠ TLS not verified` — what the reader
/// believes they are allowed to do, and the one line of the security gate no script can check —
/// while the head is the cluster's name. ratatui clips a right-aligned `Line` at its *tail*, so
/// an EKS ARN at 80 columns silently dropped both and left a row that still read as complete.
/// A shortened name is a fact the reader can see; a missing `read-only` is not.
///
/// **The name gives way from its front, not its back**, because `prod-eu` and `prod-eu-2` differ
/// in their last character (`screens/widgets.md` § 1a) — [`fits`] read from the other end.
fn shortened(text: &str, columns: usize) -> String {
    if width(text) <= columns {
        return text.to_owned();
    }
    front(text, columns, CUT)
}

/// The command log — **the last two lines, unwrapped, and cut behind a [`CUT`] where one does not
/// fit**. These are copy-paste text and a wrapped command is a lie (`screens/widgets.md` § 2).
///
/// **Unwrapped was never a licence to be silently wrong** (PM ruling, 2026-09-07). The strip is
/// 76 columns at the floor — 80 less the frame's two and [`indented`]'s two — and a `Paragraph`
/// with no `Wrap` truncates with no marker at all, so a 106-column `get pod` line drew as a
/// **valid, different command**: the same `get pod` minus `-o yaml`, which runs, exits 0 and
/// prints a table row instead of the object. Not a mangled string a reader would notice — a
/// working one, in the record invariant 4 says may not lie. Three more real cases were measured
/// the same afternoon, including a mutation whose `…` was itself clipped off, so the line never
/// changed when the outcome landed (`k8s-admin`, 2026-09-07,
/// `reports/2026-09-07-command-log-panel.md`).
///
/// **The cut is [`command_cut`], `screens/widgets.md` § 7's back-cut 3**, behind [`STRIP_CUT`]: a
/// flag with its last character sheared off still looks like a flag — `--show-managed-fiel` is
/// not one a reader would notice was wrong — so a flag gives way whole, and the mark lands where
/// the reader can see it.
///
/// **What [`views::Log`] wrote after the command is reserved before the command is cut, and never
/// cut itself** (`screens/dialogs.md` § The command log's own line, NOTES § D266): measured after
/// the cut instead, a running restart and a rejected one drew the same row, and a command that fit
/// lost its namespace to an outcome word.
fn strip(frame: &mut Frame, area: Rect, screen: &Screen) {
    let row = indented(area);
    let last = screen.log.len().saturating_sub(usize::from(LOG_LINES));
    let lines: Vec<Line> = screen.log[last..]
        .iter()
        .map(|line| {
            let (command, outcome) = views::outcome_of(line);
            let room = usize::from(row.width).saturating_sub(width(outcome));
            Line::styled(
                format!("{}{outcome}", command_cut(command, room, STRIP_CUT)),
                screen.fg(theme::INFO),
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)), row);
}

// --- THE FRAME END ---

// --- THE DIALOGS START ---

/// **What is open over the body** (`screens/widgets.md` § 5). One match, so no screen can open a
/// modal this file does not draw, and no modal can be drawn twice in two shapes.
///
/// **`changing` reaches [`help`] and no other box**: Help is the one modal that can be open over a
/// call on the wire (`screens/dialogs.md` § *While the call is running* — it is not a `Modal`, so
/// `?` still opens on top of it). The three dialogs are each built from that call's own before or
/// after, and the picker and its failure open only on an `X` that a running call refuses
/// (`crate::views::App::may_switch_cluster`).
fn modal(frame: &mut Frame, body: Rect, open: &views::Modal, changing: bool, screen: &Screen) {
    match open {
        views::Modal::Help => help(frame, body, changing, screen),
        views::Modal::Confirm(dialog) => confirm(frame, body, dialog, screen),
        views::Modal::Refused { sent, fault, said } => {
            refused(frame, body, *sent, fault, said.as_deref(), screen);
        }
        views::Modal::Gone { object, recreated } => gone(frame, body, object, *recreated, screen),
        // **Nothing to pick is the box not being drawn at all** (`screens/detail.md` § The pod
        // disappears while the picker is open): the pod went away under it, or its snapshot has
        // not landed. What is left on screen is the logs tab [`content`] has already drawn — the
        // stream's own ended marker — which is the one screen that fact already had.
        views::Modal::ContainerPick(at) => {
            if let Some(open) = screen.detail.filter(|_| containers(screen).len() > 1) {
                container_pick(frame, body, at, open, screen);
            }
        }
        views::Modal::ContextPick(picker) => pick(frame, body, picker, screen),
        views::Modal::Unconnected {
            to,
            before,
            sent,
            fault,
            said,
            coverage,
            renewal,
        } => {
            let (outcome, why) =
                failed(*sent, *fault, said.as_deref(), coverage, renewal.as_deref());
            unconnected(frame, body, to.as_deref(), before, outcome, &why, screen);
        }
    }
}

/// **One nested box, centred over the body** — `screens/widgets.md` § 5's three calls, in its
/// order: `Clear`, the block, the content.
///
/// **The width is the caller's only choice and the margins are nobody's.** `Rect::centered` is
/// `Layout` twice — vertical then horizontal — which is the one helper § 5 requires every modal
/// to share, and what it leaves each side is the margin. At the 68 columns that file measures
/// against, [`CONFIRM_BOX`] leaves 4/4, [`CROWDED_BOX`] 3/2 and [`DISMISS_BOX`] 6/6; at the real
/// floor's 78 the same call leaves 9/9, 8/7 and 11/11. **Neither set is written down anywhere in
/// this file**, which is the point of § 5's *not six hand-computed rectangles*.
///
/// **It answers with the box's inner area**, which is where [`pick`] draws its [`scrollbar`].
///
/// **The title is cut here rather than by ratatui.** `Block` clips a title at its own border with
/// nothing to show for it, and a 512-byte object name is a name the API server accepts
/// (`k8s::IDENTIFIER`). A `Confirm`'s title arrives already cut by [`name_cut`] to this same room
/// (NOTES § D266); this clip is the floor under every title, marked. The room is the box's width
/// less the space each side of the title.
fn boxed(
    frame: &mut Frame,
    body: Rect,
    width: u16,
    title: &str,
    lines: Vec<Line>,
    screen: &Screen,
) -> Rect {
    let rows = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let area = body.centered(
        Constraint::Length(width.saturating_add(2)),
        Constraint::Length(rows.saturating_add(2)),
    );
    frame.render_widget(Clear, area);
    // **The background is repainted with the foreground or neither is** — `theme.rs`
    // § THE PALETTE's pairing, and the same reason [`help`] and [`draw`]'s base block set both:
    // `Clear` resets the cells this `Rect` had been painted with.
    let block = Block::bordered()
        .border_style(screen.fg(theme::BORDER))
        .title(Span::styled(
            format!(" {} ", clipped(title, usize::from(width).saturating_sub(2))),
            screen.fg(theme::TEXT),
        ))
        .style(Style::new().bg(ink(theme::BACKGROUND, screen.depth)));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
    inner
}

/// The columns a nested box has for text — its width less [`MODAL_MARGIN`], which is on the left
/// only (`screens/dialogs.md`, measured against every box on it).
fn room(width: u16) -> usize {
    usize::from(width).saturating_sub(MODAL_MARGIN.len())
}

/// Wrapped text at a nested box's own margin — [`indent`] over [`wrapped`], which is what every
/// line inside a dialog but a centred button is.
fn margined<'a>(text: &str, columns: usize, style: Style) -> Vec<Line<'a>> {
    indent(wrapped(text, columns), MODAL_MARGIN, style)
}

/// **How wide a `Confirm`'s box is, and it is the only thing a dialog decides about its shape**
/// (`screens/widgets.md` § 5).
///
/// **Read off [`views::Dialog::consequence`] and [`views::Dialog::asks`], which is what makes it
/// hold still.** Both are fixed the moment the dialog opens; the verdict and the paused warning
/// arrive later and neither may change the width, or the box would resize under the reader while
/// they were deciding — `screens/dialogs.md` § Restart's *the two states of one dialog should not
/// be shaped differently*, which is why its plain box is drawn at the width its paused variant
/// needs.
fn box_width(dialog: &views::Dialog) -> u16 {
    let fits = wrapped(&dialog.consequence, room(CONFIRM_BOX)).len() <= CONSEQUENCE_LINES;
    if dialog.asks.is_none() && fits {
        CONFIRM_BOX
    } else {
        CROWDED_BOX
    }
}

/// **A live confirm button, drawn as [`theme::FOCUS`] asks** — the reversal `screens/widgets.md`
/// § 5 names by name, and `theme.rs`'s own answer for *the thing the keys act on*.
///
/// The role is consulted rather than a `REVERSED` written here, so the file that owns the theme
/// stays the single point of change. Its mark half has no meaning on a button: a glyph drawn into
/// `[ delete ]` would be a second signal for a state the reversal already carries.
fn focused(screen: &Screen) -> Style {
    let plain = screen.fg(theme::TEXT);
    match theme::FOCUS {
        Signal::Reverse => plain.add_modifier(Modifier::REVERSED),
        Signal::Mark(_) => plain,
    }
}

/// **The row of buttons at the foot of a `Confirm`** (`screens/dialogs.md`, every box on it).
///
/// **The confirm button is not live until [`views::Dialog::armed`] says so** — rule 3, and the
/// ctrl-key-slip guard for a delete. `esc cancel` is always live and never dims: a modal never
/// traps the user (`screens/widgets.md` § 5).
///
/// **A typed-name dialog prints its verb where the others print `⏎ do it`** — `[ delete ]`, the
/// operation's own word, because by then the reader has typed a name rather than reached for a
/// key.
fn buttons(dialog: &views::Dialog, screen: &Screen) -> Line<'static> {
    let confirm = match dialog.asks {
        Some(_) => format!("[ {} ]", dialog.verb),
        None => "[ ⏎ do it ]".to_owned(),
    };
    let style = if dialog.armed() {
        focused(screen)
    } else {
        screen.fg(theme::DIM)
    };
    Line::from(vec![
        Span::styled(confirm, style),
        Span::raw(BUTTON_GAP),
        Span::styled("[ esc cancel ]", screen.fg(theme::TEXT)),
    ])
    .centered()
}

/// **The one button on a dismiss-only box** (`screens/dialogs.md` § The cluster said no, § The
/// object went away). It is never armed and never dims — there is nothing left to confirm.
fn dismiss(screen: &Screen) -> Line<'static> {
    Line::styled("[ esc dismiss ]", screen.fg(theme::TEXT)).centered()
}

/// **The typed-name field** — the label, and the box the name is typed into
/// (`screens/dialogs.md` § Delete). Three rows drawn as text rather than a nested `Block`, so the
/// whole dialog is one `Paragraph` and its height is the length of one `Vec`.
///
/// **The tail is what is kept when the typed line is longer than the field** ([`shortened`], the
/// opposite end from [`clipped`]) — a name is typed left to right and the end is where the
/// reader's cursor is. The line cannot be long: `views::Input` bounds it at `k8s::IDENTIFIER`,
/// which is exactly the longest name a dialog can *ask* for.
///
/// **The kind is the label's own word** — *the pod's name*, *the node's name* — which is why
/// `views::Object` carries one.
fn typed_name(dialog: &views::Dialog, columns: usize, screen: &Screen) -> Vec<Line<'static>> {
    let text = screen.fg(theme::TEXT);
    let border = screen.fg(theme::BORDER);
    // **A drawn frame is inset on the right where the text is not.** The text's margin is
    // left-only because that is what puts the consequences on the lines `screens/dialogs.md`
    // wraps them onto; a *border* sharing a column with the box's own border reads as one shape
    // rather than two, and § Delete draws this field with the same margin each side.
    let field = columns.saturating_sub(MODAL_MARGIN.len());
    let rule = "─".repeat(field.saturating_sub(2));
    let room = field.saturating_sub(3);
    // The cursor is a drawn character and not a terminal cursor: a blinking one is a timer, and
    // a timer is a frame rate by another name (invariant 7).
    let line = shortened(&format!("{}_", dialog.typed.text()), room);
    vec![
        Line::styled(
            format!(
                "{MODAL_MARGIN}Type the {}'s name to confirm:",
                dialog.object.kind
            ),
            text,
        ),
        Line::styled(format!("{MODAL_MARGIN}┌{rule}┐"), border),
        Line::from(vec![
            Span::styled(format!("{MODAL_MARGIN}│ "), border),
            // **Padded in display columns and not in `char`s.** `{:<n$}` counts characters, and
            // every other measurement in this file counts columns ([`width`], [`fits`],
            // [`clipped`], [`shortened`]). `views::Input` accepts any printable character up to
            // `k8s::IDENTIFIER` bytes, so one CJK glyph typed into a delete field pushed the
            // field's right border a column out and ten of them pushed it off the box
            // (`tester`, 2026-09-10).
            Span::styled(
                format!("{line}{}", " ".repeat(room.saturating_sub(width(&line)))),
                text,
            ),
            Span::styled("│", border),
        ]),
        Line::styled(format!("{MODAL_MARGIN}└{rule}┘"), border),
    ]
}

/// **A live confirmation — `screens/dialogs.md` § Scale, § Restart and § Delete are one box with
/// one row order**, and what differs between them is which rows they have.
///
/// The order, top to bottom: the consequence, the warning a check added, the dry-run verdict, and
/// then either the `$ kubectl …` line or the typed-name field that takes its place. **The
/// consequence is above the command in all three**, which is this whole page's rule: the
/// consequence is stated in plain language *above* the command, never instead of it.
///
/// **The verdict's row is drawn whether or not the verdict has arrived**, so the box does not
/// grow by a row the moment the cluster answers. For `delete` it is `Some` from the first frame
/// (NOTES § D225 ruling 1 — nothing is sent, so there is nothing to wait for); for `scale` and
/// `restart` it is the real round trip, and the button below it stays dim until then.
///
/// **A typed-name dialog spends the `$` line's rows on the field, and the command is on the log
/// strip beneath either way** (`screens/widgets.md` § 2, invariant 4). It is not a preference:
/// § Delete's own node box already reaches [`MODAL_ROWS`] exactly with four consequence lines and
/// the field, so a `$` row inside it would put the frame two rows past the 24 this product is
/// drawn to.
///
/// **The blank row between the consequence and the verdict belongs to the [`CONFIRM_BOX`] box**
/// (`screens/widgets.md` § 5: kept whenever there is room, dropped first — before any sentence is
/// cut — whenever there is not). A box that had to widen is a box that had no room, so widening
/// is what spends it: § Scale keeps the row, § Restart and § Delete do not.
fn confirm(frame: &mut Frame, body: Rect, dialog: &views::Dialog, screen: &Screen) {
    let width = box_width(dialog);
    let columns = room(width);
    let text = screen.fg(theme::TEXT);

    let consequence = wrapped(&dialog.consequence, columns);
    let warning = dialog
        .warning
        .as_ref()
        .map_or_else(Vec::new, |warning| wrapped(warning, columns));
    let verdict = dialog
        .verdict
        .map_or_else(Vec::new, |verdict| wrapped(&spoken(verdict), columns));
    let field = usize::from(dialog.asks.is_some()) * FIELD_ROWS;

    // **The rows nothing can give up**: the blank under the title bar, the verdict — one row even
    // before it arrives, so the box does not grow when the cluster answers — the `$ kubectl …`
    // line, the typed-name field, and the buttons.
    let hard = 1 + verdict.len().max(1) + 1 + field + 1;

    // **The four blank rows, given up in this order while the box is over [`MODAL_ROWS`]**
    // (`screens/widgets.md` § 5: dropped first, before any sentence is cut). The order is least
    // separation lost first — the row under the consequence is § 5's own, then the one between
    // the command and the field it belongs to, then the one under the verdict; the row above the
    // buttons is the last to go, because a button pressed by mistake is what these boxes exist to
    // prevent.
    let full = hard
        + consequence.len()
        + warning.len()
        + usize::from(width == CONFIRM_BOX)
        + usize::from(field > 0)
        + 2;
    let mut over = full.saturating_sub(MODAL_ROWS);
    let give = |over: &mut usize, wanted: bool| {
        if wanted && *over > 0 {
            *over -= 1;
            false
        } else {
            wanted
        }
    };
    // **`b1` belongs to the [`CONFIRM_BOX`] box** — a box that had to widen is a box that had no
    // room, so widening is what spends it (§ Scale keeps this row, § Restart and § Delete do not).
    let under_text = give(&mut over, width == CONFIRM_BOX);
    let under_command = give(&mut over, field > 0);
    let under_verdict = give(&mut over, true);
    let above_buttons = give(&mut over, true);

    // **Whatever the blank rows could not absorb is what the sentences give up** — `over`, read
    // once the four are decided. **This is deliberately not recomputed from a row budget**: a
    // sum of `hard` and the surviving blanks says the same thing twice, and the mutation gate
    // said so — four mutants of that arithmetic survived, every one of them equivalent, because
    // a cut can only happen once every blank is already gone (2026-09-10).
    //
    // **The consequence gives way first and the warning only when it is down to its last row.**
    // The warning is NOTES § D224's correction — without it the dialog claims copies were
    // replaced when they were not. Every cut is marked.
    let short = over;
    let keep = consequence.len().saturating_sub(short).max(1);
    let short = short.saturating_sub(consequence.len() - keep);
    let said = marked(consequence, columns, keep);
    let kept = warning.len().saturating_sub(short);
    let warning = marked(warning, columns, kept);

    let mut lines = vec![Line::raw("")];
    lines.extend(indent(said, MODAL_MARGIN, text));
    lines.extend(indent(warning, MODAL_MARGIN, text));
    if under_text {
        lines.push(Line::raw(""));
    }
    if verdict.is_empty() {
        lines.push(Line::raw(""));
    } else {
        lines.extend(indent(verdict, MODAL_MARGIN, screen.fg(theme::DIM)));
    }
    if under_verdict {
        lines.push(Line::raw(""));
    }
    // **Every `Confirm` draws the taught command inside its own frame, delete included**
    // (NOTES § D233 ruling 1, `views::Log`). The strip underneath is *not* the same line: it
    // carries a mutation only once `ops::ask` has answered `Confirmed`, so a dialog that left the
    // command to the strip taught it on no surface at all — and the strip was still showing an
    // unrelated earlier command, which at 3am reads as the one about to run.
    lines.push(Line::styled(
        format!(
            "{MODAL_MARGIN}$ {}",
            command_cut(&dialog.kubectl, columns.saturating_sub(2), CUT)
        ),
        screen.fg(theme::INFO),
    ));
    if under_command {
        lines.push(Line::raw(""));
    }
    if dialog.asks.is_some() {
        lines.extend(typed_name(dialog, columns, screen));
    }
    if above_buttons {
        lines.push(Line::raw(""));
    }
    lines.push(buttons(dialog, screen));

    // **`payments/web` and never `deployment/web`** — the title bar answers *which object*, and
    // the kind is already spelled on the `$` line below it (`screens/dialogs.md` rule 1, its own
    // § Scale paragraph). A node has no namespace and gets the bare `node-3`. The room is
    // [`boxed`]'s own, less the verb and its space ([`name_cut`]).
    let verb = format!("{} ", capitalised(dialog.verb));
    let title = format!(
        "{verb}{}",
        name_cut(
            dialog.object.namespace.as_deref(),
            &dialog.object.name,
            usize::from(width).saturating_sub(2 + self::width(&verb)),
        )
    );
    boxed(frame, body, width, &title, lines, screen);
}

/// **The cluster said no** (`screens/dialogs.md` § The cluster said no) — a rejected write is a
/// first-class state and not a toast that vanishes.
///
/// **What the cluster sent back is the one string in any dialog that came off the API**, so it is
/// the one thing here that is bounded at draw time: `k8s::FREE_TEXT` allows 4096 bytes and a
/// `fieldValidation=Strict` rejection hands back the object that was sent (NOTES § D217), which
/// is eighty wrapped lines into a box with room for four. [`cut`] marks what it dropped, which is
/// what keeps this out of `screens/widgets.md` § 7's ban on a silent truncation.
///
/// **The room left is counted from the rows already spent and never from the mockup's own three
/// lines** — the closing sentence wraps to two at this width today and to more at another, and a
/// number copied off the drawing would go stale the first time either sentence changed.
///
/// **A refusal with nothing quoted drops the heading with it.** `What the cluster sent back:` over
/// an empty space says the cluster answered when it did not.
fn refused(
    frame: &mut Frame,
    body: Rect,
    sent: bool,
    fault: &Fault,
    said: Option<&str>,
    screen: &Screen,
) {
    let columns = room(DISMISS_BOX);
    let text = screen.fg(theme::TEXT);
    let dim = screen.fg(theme::DIM);

    // **`sent` and the fault together pick one of six states**, and the check's own three are the
    // three `ops::Record::check` writes to the audit log for the same fault — it never left this
    // machine, k8rs never heard back, or the cluster said no — so the box and the audit line cannot
    // tell one attempt two ways (`screens/dialogs.md` § The cluster said no, NOTES § D266). A `409`
    // is none of them: nothing was refused, the object moved, and the sentence is
    // [`views::because`]'s own two clauses whichever call met it. After the real call, **did the
    // server answer at all** ([`answered`]) decides the other two. Only a state the cluster can
    // have answered in words carries a quote.
    let moved = format!("{} — {}.", capitalised(views::MOVED), views::REREAD);
    let (title, outcome, because, quotes) = match (fault, sent) {
        (Fault::Conflict, _) => (
            "The object changed first",
            "Nothing was changed.",
            moved.as_str(),
            false,
        ),
        (Fault::Kubeconfig | Fault::NoContext | Fault::BadEntry | Fault::NoCredential, false) => (
            "This could not be checked",
            "Nothing was changed.",
            "The check that runs before the real change never left this machine — k8rs could not \
             build a connection from this kubeconfig.",
            false,
        ),
        (Fault::Unanswered | Fault::Unfinished, false) => (
            "The check never got an answer",
            "Nothing was changed.",
            "k8rs does not know whether the check that runs before the real change reached the \
             cluster.",
            false,
        ),
        (Fault::Refused | Fault::Rejected | Fault::Expired | Fault::Gone, false) => (
            "The cluster refused this",
            "Nothing was changed.",
            "This is the check that runs before the real change — it stopped this one.",
            true,
        ),
        (fault, true) if answered(*fault) => (
            "The cluster refused this",
            "Nothing was changed.",
            "This was the real change, not a check.",
            true,
        ),
        // **Invariant 2 names this state in these words.** A dead socket on a `delete` — which
        // sends no check at all (NOTES § D225 ruling 1) — ends with the request on the wire and
        // no answer, and *"Nothing was changed."* over it is a claim k8rs cannot make.
        //
        // **The title claims nothing about the server either** (`screens/dialogs.md` § The cluster
        // said no, state 3): *the change did not go through* is the idiom for a completed failure —
        // *my payment didn't go through* — so it contradicts the line the same box draws under it.
        // That no answer arrived is the whole of what is known, and the title says that and stops.
        (_, true) => (
            "The cluster never answered",
            "k8rs does not know whether the change was made.",
            "This was the real change, not a check.",
            true,
        ),
    };

    let mut lines = vec![Line::raw("")];
    lines.extend(margined(outcome, columns, text));
    lines.push(Line::raw(""));
    let tail: Vec<Line> = margined(because, columns, text)
        .into_iter()
        .chain([Line::raw(""), dismiss(screen)])
        .collect();

    if let Some(said) = said.filter(|said| quotes && !said.is_empty()) {
        // **The heading stopped promising prose** (invariant 14, `screens/dialogs.md` § The
        // cluster said no): a `fieldValidation=Strict` rejection hands back the object that was
        // sent (NOTES § D217), so *the cluster's own words* delivered JSON. *Sent back* is true of
        // either, in every state that carries a quote.
        let heading = margined("What the cluster sent back:", columns, text);
        // The quote's own blank row under it is the `1`.
        let left = MODAL_ROWS.saturating_sub(lines.len() + heading.len() + 1 + tail.len());
        let quoted = cut(said, columns.saturating_sub(2), left);
        // **The heading goes with the quote and never stands over an empty space**, and so does
        // the blank that closes it: one blank row between the outcome and the explanation whether
        // or not a quote sits there, where two used to stack (`screens/dialogs.md` § The cluster
        // said no, *No two blank rows stack*).
        if !quoted.is_empty() {
            lines.extend(heading);
            lines.extend(indent(quoted, "    ", dim));
            lines.push(Line::raw(""));
        }
    }
    lines.extend(tail);
    boxed(frame, body, DISMISS_BOX, title, lines, screen);
}

/// **The object went away while the dialog was open** (`screens/dialogs.md` § The object went
/// away, NOTES § D22) — the watch never stopped running behind the modal, so a dialog knows when
/// the thing it is about stopped existing.
///
/// **The title names the outcome and the body names the object** (that page's rule 1): by the
/// time this opens there is nothing left to confirm, so `Already gone` is what the border says
/// and the identity moves inside where the reader can still see what disappeared.
///
/// **It claims a successor for nothing, and that is a repaired defect** (NOTES § D44). The box
/// used to read `replaced by web-2c81a 3 seconds ago`; the timestamp was real and *"replaced by"*
/// was an inference off a shared `ownerReference` that named the wrong pod whenever the ReplicaSet
/// scaled for another reason at the same moment. There is no successor-matching anywhere in k8rs
/// to back one, so this box claims only what the dialog's own identity fields back.
///
/// **The hedge is the pod's and the replicaset's, word for word from § Delete's own consequence**
/// — reused rather than reworded, because it is the same unverified fact both times: k8rs has
/// read no `ownerReferences` and does not know whether anything will put one back.
fn gone(frame: &mut Frame, body: Rect, object: &views::Object, recreated: bool, screen: &Screen) {
    let columns = room(DISMISS_BOX);
    let text = screen.fg(theme::TEXT);
    let hedge = if recreated {
        "Whatever created it will normally replace it — k8rs has not checked whether anything did."
    } else {
        "Nothing will take its place on its own."
    };

    let mut lines = vec![Line::raw("")];
    lines.extend(margined(
        &format!(
            "This {} is already gone — something else removed it while this was open. {hedge}",
            object.kind
        ),
        columns,
        text,
    ));
    lines.extend([
        Line::raw(""),
        Line::styled(
            format!(
                "    {}",
                name_cut(
                    object.namespace.as_deref(),
                    &object.name,
                    columns.saturating_sub(2),
                )
            ),
            text,
        ),
        Line::raw(""),
    ]);
    lines.extend(margined("Nothing was changed.", columns, text));
    lines.push(Line::raw(""));
    lines.push(dismiss(screen));
    boxed(frame, body, DISMISS_BOX, "Already gone", lines, screen);
}

/// **The cluster picker** (`screens/context.md` § The picker, § Opening at startup) — one box both
/// ways, [`views::Picker::startup`] choosing its title and [`views::Picker::verbs`] its two words,
/// the footer's own.
///
/// **Top to bottom**: the list, the slot under it, the sentence that k8rs does not change the
/// kubeconfig — or, over a file with one context, that it is the only one — and the buttons, each
/// block a blank row from the next.
///
/// **The slot is the selected row's server line, or why `⏎` has nothing to act on** — no row
/// selected, no row that can be, a filter that hides every row, or a kubeconfig with no contexts
/// in it (NOTES § D264 rulings 2, 16 and 18). Its rows are kept for the
/// tallest thing any shown row, or that sentence, would need, so the box does not change height
/// under the cursor ([`box_width`]'s reason, one axis over).
///
/// **The list takes what the body has left** ([`PICK_SPARE`]), scrolls with the cursor where it has
/// fewer rows than the filter shows — its offset derived fresh from the selection every frame and
/// kept nowhere between them, the value a fresh `ListState` of that height would give
/// (`screens/widgets.md` § 4, NOTES § D264 ruling 26) — and says so with a [`scrollbar`].
///
/// **Every string came from [`crate::k8s::contexts`] already stripped and bounded** (invariant 9);
/// what this adds is that none of them can size the box: a name gives way at its slot, a tag at
/// [`TAG`], and the slot at [`SERVER_ROWS`].
fn pick(frame: &mut Frame, body: Rect, picker: &views::Picker, screen: &Screen) {
    let rows = screen.contexts;
    let wide = body.width.saturating_sub(2 * PICK_MARGIN + 2);
    let columns = room(wide);
    // **The two gaps are not written beside each other**: `GAP + GAP` and `GAP * GAP` are both
    // four, and a term a mutation cannot change is a term no test can hold.
    let slot = columns.saturating_sub(GAP + TAG + GAP + BADGE + width(MARKER));
    let text = screen.fg(theme::TEXT);
    let shown = picker.shown(rows);
    let selected = picker.selected(rows);

    let inert = match selected {
        Some(_) => Vec::new(),
        // **The file before the filter**: with no context in it, a filter has nothing to hide.
        None if rows.is_empty() => EMPTIED.map(str::to_owned).to_vec(),
        None if shown.is_empty() => cut(
            &format!("No context matches \"{}\".", picker.filter.text()),
            columns,
            SERVER_ROWS,
        ),
        None if picker.nowhere(rows) => UNREACHABLE.map(str::to_owned).to_vec(),
        None => UNSTARTED.map(str::to_owned).to_vec(),
    };
    let kept = shown
        .iter()
        .filter(|&&at| views::landable(&rows[at]))
        .map(|&at| served(&rows[at], columns).len())
        .fold(inert.len().max(1), usize::max);
    // **One context and `X`: still a picker, and it says why nothing can be picked**
    // (`screens/context.md`: *a key that appears to do nothing is worse than a screen that
    // explains why*). It takes the place of the kubeconfig sentence, which is about a switch there
    // is nothing to make, and a name wider than the whole slot gives way from its front
    // (NOTES § D264 ruling 5) before the sentence wraps.
    let under = match rows {
        [only] => wrapped(
            &format!(
                "{} is the only cluster in your kubeconfig.",
                shortened(views::drawn_name(only), columns)
            ),
            columns,
        ),
        _ => UNCHANGED.map(str::to_owned).to_vec(),
    };
    // The blank above the list, below it, below the slot and below the sentence, the buttons, and
    // the blank under them.
    let fixed = 6 + kept + under.len();
    let most = usize::from(body.height)
        .saturating_sub(PICK_SPARE)
        .saturating_sub(fixed)
        .max(1);
    let listed = shown.len().clamp(1, most);
    let place = selected
        .and_then(|at| shown.iter().position(|&nth| nth == at))
        .unwrap_or_default();
    let first = (place + 1).saturating_sub(listed);

    let mut lines = vec![Line::raw("")];
    lines.extend(
        shown
            .iter()
            .skip(first)
            .take(listed)
            .map(|&at| context(&rows[at], Some(at) == selected, slot, screen)),
    );
    lines.resize(1 + listed, Line::raw(""));
    lines.push(Line::raw(""));
    let said = selected.map_or(inert, |at| served(&rows[at], columns));
    let mut said = indent(said, MODAL_MARGIN, text);
    said.resize(kept, Line::raw(""));
    lines.extend(said);
    lines.push(Line::raw(""));
    lines.extend(indent(under, MODAL_MARGIN, text));
    lines.push(Line::raw(""));
    let (go, leave) = picker.verbs();
    let enter = if picker.inert(rows) {
        screen.fg(theme::DIM)
    } else {
        focused(screen)
    };
    lines.push(
        Line::from(vec![
            Span::styled(format!("[ ⏎ {go} ]"), enter),
            Span::raw(BUTTON_GAP),
            Span::styled(format!("[ esc {leave} ]"), text),
        ])
        .centered(),
    );
    lines.push(Line::raw(""));

    let title = if picker.startup() {
        "Choose a cluster"
    } else {
        "Switch cluster"
    };
    let inner = boxed(frame, body, wide, title, lines, screen);
    let list = Rect {
        y: inner.y + 1,
        height: u16::try_from(listed).unwrap_or(u16::MAX),
        ..inner
    };
    scrollbar(frame, list, shown.len(), first, screen);
}

/// **The container picker** (`screens/detail.md` § Choosing a container, and when there is nothing
/// to choose) — the same nested box every dialog is drawn in, centred over the body.
///
/// **Not [`pick`]'s shape, and that is the section's own ruling**: the cluster picker's box grows
/// with the terminal, this one is capped at [`MODAL_ROWS`] the way `Confirm`, `Refused` and `Gone`
/// are. The number to count is therefore what this box's own chrome leaves under that cap, which
/// is what the list length below is derived from rather than written down — six rows with the
/// restart hint drawn, which is the figure that file counts off its own mockup.
///
/// **Top to bottom**: the list, the sentence that says which container `⇧p` is worth pressing on,
/// and the buttons, each a blank row from the next. The hint is left out where no container has
/// restarted, because there is then nothing for it to point at.
///
/// **The restart count is measured off what is about to be drawn, the name is guaranteed
/// [`NAME_FLOOR`] columns, and the state word takes what those two leave** — the reverse of the
/// order this doc gave before the blocker below was found, and the inline comment beside the
/// arithmetic has said so since (`k8s-admin`, 2026-09-18; NOTES § D216 — nothing mechanical
/// reads a comment).
///
/// **The name gives way only to its own column and never to the state's**: a name past what the
/// row has left front-cuts, `screens/widgets.md` § 7's sixth cut at its second call site, because
/// `istio-proxy` and `istio-proxy-metrics` differ at the tail. A **state word** past what the name
/// leaves back-cuts at a word boundary instead — two directions, two reasons, and
/// `screens/detail.md` § When a container's own state is what does not fit rules which gives way
/// to which: a picker exists to name things.
///
/// **`⏎` is never inert here**: every row is a container and every container can be picked, which
/// is why this box has no dim button and no `/` of its own to clear.
fn container_pick(frame: &mut Frame, body: Rect, at: &Cursor, open: &Detail, screen: &Screen) {
    let rows = containers(screen);
    let columns = room(CROWDED_BOX);
    let text = screen.fg(theme::TEXT);
    let anchors: Vec<Option<&str>> = rows.iter().map(|(name, _)| Some(*name)).collect();
    let selected = at.selected(&anchors);

    // **The state word is [`crate::views::container_state`]'s and the count is
    // [`crate::views::restart_count`]'s** — describe's own two, so one pod cannot be said to be
    // running on one screen and waiting on another.
    let said: Vec<(String, String)> = rows
        .iter()
        .map(|(_, status)| {
            (
                views::container_state(status.map(|held| &held.state)).0,
                views::restart_count(*status),
            )
        })
        .collect();
    // **The container the hint is about is the one with the most restarts, and the first of them
    // where two tie** — that is the signal that makes `⇧p` worth pressing, and naming the
    // selected row instead would put the key on whatever the cursor happens to be resting on.
    let worst = rows
        .iter()
        .filter_map(|(name, status)| status.map(|held| (*name, held.restarts)))
        .filter(|(_, count)| *count > 0)
        .reduce(|best, next| if next.1 > best.1 { next } else { best });
    let hint = worst.map(|(name, count)| {
        let times = match count {
            1 => "once".to_owned(),
            count => format!("{count} times"),
        };
        // **Neither half of this sentence claims more than it knows** (`screens/detail.md`
        // § Choosing a container, `k8s-admin` 2026-09-18). A restart is not a crash — `exitCode:
        // 0` under `restartPolicy: Always` restarts a container that asked to stop — and
        // `--previous` 404s once the kubelet has rotated the old log away, so the log is offered
        // conditionally rather than promised. The same false wording still stands in
        // `screens/help.md` and this file's own logs-tab header; both belong to a box that may
        // touch them.
        let said = |name: &str| {
            format!(
                "{name} restarted {times}. ⇧p shows the log from before that restart, if the \
                 kubelet still has it."
            )
        };
        // **The name is shortened to what [`HINT_LINES`] leaves, and the sentence is cut at
        // `HINT_LINES` besides** — the security gate's *sizes are bounded* row, on this box's own
        // height. `k8s::IDENTIFIER` allows 512 bytes and the API's own 63 is not this renderer's
        // guarantee; a name that pushed this sentence to ten rows would push the list out of the
        // box rather than merely read badly. Cutting the *name* keeps `⇧p` — the half that says
        // what to do — where cutting the sentence would take it.
        let room = (HINT_LINES * columns).saturating_sub(width(&said("")));
        indent(
            cut(&said(&shortened(name, room)), columns, HINT_LINES),
            MODAL_MARGIN,
            text,
        )
    });

    // The blank above the list, the one below it, the buttons and the blank under them — plus the
    // hint and its own blank row wherever there is one.
    let fixed = 4 + hint.as_ref().map_or(0, |hint| hint.len() + 1);
    let most = MODAL_ROWS.saturating_sub(fixed).max(1);
    let listed = rows.len().clamp(1, most);
    let first = (selected.unwrap_or_default() + 1).saturating_sub(listed);

    // **The scrollbar's column is reserved before anything is measured into it** — it is drawn
    // over the box's own last column, which the widest row was already occupying: `10 restarts`
    // drew as `10 restart`, its last character silently the widget's paint
    // (`reports/2026-09-18-filter-and-container-picker.md` § 3). Reserved only when the list
    // actually scrolls, which is the same condition [`scrollbar`] draws on, so a short list is one
    // column wider exactly as before.
    let columns = columns.saturating_sub(usize::from(rows.len() > listed));

    // **The two fixed columns are measured off what is about to be drawn, and the name's is what
    // is left — with a floor.** `crate::views::container_state` translates
    // `CreateContainerConfigError` into *needs a ConfigMap or Secret that does not exist*, 47
    // columns, and its `Waiting` fall-through keeps the kubelet's own reason, bounded only by
    // `k8s::IDENTIFIER`'s 512 bytes. Measured as written, one container in that state took the
    // name column to [`slot`'s old `.max(1)`] and drew two rows with **no name on them at all**,
    // the restart count clipped by the box with no mark
    // (`reports/2026-09-18-filter-and-container-picker.md` § 1). **A picker exists to name
    // things, so the name never gives way to the state; the state gives way to the name**
    // ([`NAME_FLOOR`], `screens/detail.md` § When a container's own state is what does not fit).
    let counted = said.iter().map(|(_, seen)| width(seen)).max().unwrap_or(0);
    let around = width(MARKER) + NAMES_GAP + NAMES_GAP + counted;
    let widest = said.iter().map(|(word, _)| width(word)).max().unwrap_or(0);
    let slot = columns
        .saturating_sub(around + widest)
        .max(NAME_FLOOR.min(columns.saturating_sub(around).max(1)));
    let word = columns.saturating_sub(around + slot);
    // **Back-cut at a word boundary behind one `…`, which is [`cut`] at one line** — the shape
    // the Alerts card's evidence already takes, and legitimate for the same reason: this is a
    // sentence whose tail a reader loses nothing by losing, because `d describe` has the whole of
    // it one `esc` away.
    let said: Vec<(String, String)> = said
        .into_iter()
        .map(|(state, seen)| {
            let state = match width(&state) > word {
                true => cut(&state, word, 1).join(""),
                false => state,
            };
            (state, seen)
        })
        .collect();

    let mut lines = vec![Line::raw("")];
    lines.extend((first..rows.len()).take(listed).map(|nth| {
        let (state, seen) = &said[nth];
        let marker = match Some(nth) == selected {
            true => MARKER.to_owned(),
            false => " ".repeat(width(MARKER)),
        };
        Line::styled(
            format!(
                "{MODAL_MARGIN}{marker}{}{}{seen}",
                slotted(&shortened(rows[nth].0, slot), slot + NAMES_GAP),
                slotted(state, word + NAMES_GAP),
            ),
            text,
        )
    }));
    lines.push(Line::raw(""));
    if let Some(hint) = hint {
        lines.extend(hint);
        lines.push(Line::raw(""));
    }
    lines.push(
        Line::from(vec![
            Span::styled("[ ⏎ pick ]", focused(screen)),
            Span::raw(BUTTON_GAP),
            Span::styled("[ esc cancel ]", text),
        ])
        .centered(),
    );
    lines.push(Line::raw(""));

    // **The title is the pod, cut the way every other surface naming an object cuts one**
    // ([`name_cut`]), with the box's own room measured off what follows it rather than restated.
    let about = " — pick a container";
    let title = format!(
        "{}{about}",
        name_cut(
            open.object.namespace.as_deref(),
            &open.object.name,
            columns.saturating_sub(width(about)),
        )
    );
    let inner = boxed(frame, body, CROWDED_BOX, &title, lines, screen);
    let list = Rect {
        y: inner.y + 1,
        height: u16::try_from(listed).unwrap_or(u16::MAX),
        ..inner
    };
    scrollbar(frame, list, rows.len(), first, screen);
}

/// **A list with more rows than it is drawn in, marked down its right-hand column**
/// (`screens/widgets.md` § 2: a `Scrollbar`, drawn **only** once the content is taller than the
/// viewport — a permanent one in a short list is noise). `first` is the first row drawn.
///
/// **No arrows at the ends**: the picker's list is three rows at the 80×24 floor, and two of those
/// spent on `▲`/`▼` would leave a one-row track whose thumb cannot say where the window is.
fn scrollbar(frame: &mut Frame, area: Rect, content: usize, first: usize, screen: &Screen) {
    let rows = usize::from(area.height);
    if content <= rows {
        return;
    }
    // **The state counts the places the window can start at, not the rows**, so a window on its
    // last row puts the thumb on the track's last cell: counted in rows, the thumb stopped eight
    // cells short of the foot of a 20-row list.
    let mut state = ScrollbarState::new(content - rows + 1)
        .position(first)
        .viewport_content_length(rows);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .style(screen.fg(theme::BORDER)),
        area,
        &mut state,
    );
}

/// **One context's row: name, tag, badge** (`screens/context.md` § The tag column) — the name slot
/// flexible, the other two fixed, so a 90-character name never moves the tag.
///
/// **The name gives way from its front and the tag from its tail** (NOTES § D264 rulings 5 and
/// 24): an EKS fleet shares a long prefix and differs at the end, and a written tag is read from
/// its front. No one cut tells every naming scheme apart; the server line under the list does.
///
/// **A written tag is bright with no marker and a derived one is dim behind `~`** — the tilde is
/// what survives a monochrome terminal and a paste into a chat, so the difference is carried by a
/// character and not by colour alone (NOTES § D116). **A row the cursor skips, or cannot open as
/// itself, is dim throughout**: a written tag on it is dimmed with the rest.
///
/// **The badge is one of four, in this order**: `⚠ duplicate name`, `⚠ cluster undefined`,
/// `⚠ TLS not verified`, `(current)`. The TLS warning beats `(current)` because the cursor's own
/// position already said which row is current (§ The badge tie-break); a shadowed row carries
/// neither of those two, `k8s::contexts` having cleared both (NOTES § D175).
fn context<'a>(row: &Choice, selected: bool, slot: usize, screen: &Screen) -> Line<'a> {
    let dimmed = row.shadowed || row.server == Address::Undefined;
    let style = screen.fg(if dimmed { theme::DIM } else { theme::TEXT });
    let guessed = matches!(row.tag, Tag::Derived(_));
    let badge = if row.shadowed {
        format!("{} duplicate name", mark(theme::ALARM))
    } else if row.server == Address::Undefined {
        format!("{} cluster undefined", mark(theme::ALARM))
    } else if row.insecure {
        unverified()
    } else if row.current {
        "(current)".to_owned()
    } else {
        String::new()
    };
    let marker = if selected {
        MARKER.to_owned()
    } else {
        " ".repeat(width(MARKER))
    };
    let gap = " ".repeat(GAP);
    let name = shortened(views::drawn_name(row), slot);
    let line = Line::from(vec![
        Span::styled(
            format!("{MODAL_MARGIN}{marker}{}{gap}", slotted(&name, slot)),
            style,
        ),
        Span::styled(
            slotted(&views::drawn_tag(&row.tag), TAG),
            screen.fg(if dimmed || guessed {
                theme::DIM
            } else {
                theme::TEXT
            }),
        ),
        Span::styled(format!("{gap}{}", slotted(&badge, BADGE)), style),
    ]);
    // **The fill and the mark together**, as the sidebar and the browser draw a selection:
    // `PANEL` degrades to nothing at sixteen colours and `▸` is what carries it there.
    if selected {
        line.style(Style::new().bg(ink(theme::PANEL, screen.depth)))
    } else {
        line
    }
}

/// `text` in exactly `columns` — cut between characters where it is longer, padded where it is
/// shorter. **No mark**: a tag and a badge clip at their slot's edge like a cell does
/// (`screens/context.md` § A 60-character tag), and [`fits`] is the one cut that never lands inside
/// a character. A name reaches here already [`shortened`] to the slot, so nothing of it is cut.
fn slotted(text: &str, columns: usize) -> String {
    let kept = fits(text, columns);
    format!("{kept}{}", " ".repeat(columns.saturating_sub(width(kept))))
}

/// **The line under the list for one row** — `name  →  address`, or `name  —  ` and the sentence
/// that stands in for an address (`screens/context.md` § A context defined twice, § A server
/// address k8rs will not guess at). Continuations hang under the text, as both of that file's
/// sentences draw, and the name gives way from its front at [`LABEL`] (NOTES § D264 ruling 5).
///
/// **A shadowed row's sentence beats its address**: what a lookup by its name opens is an entry
/// above it, so its own address describes a connection nobody makes — and the sentence claims
/// nothing about that entry, which can be broken too (NOTES § D264 ruling 3). **A row whose
/// cluster is undefined has no line at all**, and the cursor never lands on one to ask.
fn served(row: &Choice, columns: usize) -> Vec<String> {
    let name = views::drawn_name(row);
    let (joint, said) = match &row.server {
        _ if row.shadowed => (
            "—",
            Cow::Owned(format!(
                "another context earlier in this file is also named {name}. Every lookup by that \
                 name, ⏎ here included, finds it first, never this row."
            )),
        ),
        Address::Server(address) => ("→", Cow::Borrowed(address.as_str())),
        Address::Unreadable => (
            "—",
            Cow::Borrowed(
                "k8rs found a server address here it cannot read safely, so nothing is shown \
                 instead of a guess.",
            ),
        ),
        Address::Undefined => return Vec::new(),
    };
    let lead = format!("{}  {joint}  ", shortened(name, LABEL));
    let hang = " ".repeat(width(&lead));
    let room = columns.saturating_sub(width(&lead));
    cut(&said, room, SERVER_ROWS)
        .into_iter()
        .enumerate()
        .map(|(nth, line)| format!("{}{line}", if nth == 0 { &lead } else { &hang }))
        .collect()
}

/// **Why a picked context did not connect, in the driver's own words** (NOTES § D264 ruling 1) —
/// the title's outcome, and the one paragraph under it.
///
/// **Whether a request reached a cluster decides both** (`screens/context.md` § Every other fault
/// has its own sentence). A connection that sent nothing *could not be opened* and is asked as
/// [`views::REACH`], `--live`'s framing, with no scope — **and so are the four faults that never
/// put a request in front of a cluster, whatever path they came by**: a `NoCredential` on a watch
/// mid-session built no client either (NOTES § D264 ruling 17). A pods watch that never listed is
/// asked as [`views::watching`] in [`views::scope`] — the scope folded into what was asked, which
/// is where `because`'s arms put `asked`. Of those, the ones the cluster answered *said no* and the
/// rest *did not answer*: [`answered`], the question [`refused`] asks.
///
/// **[`views::next_step`] follows the reason only where a request went out, asked as a running
/// k8rs** (NOTES § D264 rulings 23 and 32) — so an `Unanswered` whose client was never built draws
/// none, which is the sentence `--once` prints for it (§ Every other fault's table, ruling 28).
fn failed(
    sent: bool,
    fault: Fault,
    said: Option<&str>,
    coverage: &Coverage,
    renewal: Option<&str>,
) -> (&'static str, String) {
    let local = matches!(
        fault,
        Fault::Kubeconfig | Fault::NoContext | Fault::BadEntry | Fault::NoCredential
    );
    let (outcome, reason, next) = if !sent || local {
        let reason = views::because(fault, views::REACH, renewal, None);
        ("could not be opened", reason, None)
    } else {
        // **Pods, because the permanent pods watch is the one request a fresh connection makes**
        // (`screens/context.md` § Every other fault has its own sentence).
        let asked = format!("{} {}", views::watching("pods"), views::scope(coverage));
        let outcome = if answered(fault) {
            "said no"
        } else {
            "did not answer"
        };
        (
            outcome,
            views::because(fault, &asked, renewal, said),
            views::next_step(fault, coverage, "pods", true),
        )
    };
    let reason = capitalised(&reason);
    let paragraph = match next {
        Some(next) => format!("{reason}. {next}"),
        None => format!("{reason}."),
    };
    (outcome, paragraph)
}

/// **Did the server answer at all?** These five are answers; the rest are *nothing came back* or a
/// failure on this machine before anything was sent. [`Fault`]'s to answer and asked in one place,
/// so [`refused`] and [`failed`] cannot sort one fault two ways.
fn answered(fault: Fault) -> bool {
    matches!(
        fault,
        Fault::Rejected | Fault::Expired | Fault::Refused | Fault::Gone | Fault::Conflict
    )
}

/// **A picked context that did not connect** (`screens/context.md` § When the new cluster does not
/// work) — the same box mid-session and at startup, where only the way out and the button differ.
///
/// **Every word inside it but the way out is [`failed`]'s**, and the way out is the page's two
/// sentences: *nothing is wrong with* the context that was last live, or *nothing has connected
/// yet* — which [`views::Before`] it is, read off what has connected in this run
/// (NOTES § D264 ruling 15).
///
/// **Bounded however long the names and the cluster's words are**: both names give way from the
/// front, the title's at whatever leaves its outcome whole and the way out's at what keeps that
/// sentence to two rows; the paragraph takes the rows [`MODAL_ROWS`] has left and is cut with a
/// mark past them — the one thing in it that is not k8rs's own is [`Fault::Rejected`]'s quote.
fn unconnected(
    frame: &mut Frame,
    body: Rect,
    to: Option<&str>,
    before: &views::Before,
    outcome: &str,
    why: &str,
    screen: &Screen,
) {
    let columns = room(DISMISS_BOX);
    let text = screen.fg(theme::TEXT);
    let way_out = match before {
        views::Before::Connected(from) => {
            let lead = "Nothing is wrong with ";
            let from = from.as_deref().unwrap_or(views::UNNAMED);
            format!(
                "{lead}{} — X takes you back.",
                shortened(from, columns.saturating_sub(width(lead)))
            )
        }
        views::Before::Picking(_) => {
            "Nothing has connected yet — esc takes you back to the list to try a different \
             cluster."
                .to_owned()
        }
    };
    let way_out = wrapped(&way_out, columns);
    // The blank under the title, under the paragraph and under the way out, the button, and the
    // blank under it.
    let paragraph = cut(why, columns, MODAL_ROWS.saturating_sub(5 + way_out.len()));

    let mut lines = vec![Line::raw("")];
    lines.extend(indent(paragraph, MODAL_MARGIN, text));
    lines.push(Line::raw(""));
    lines.extend(indent(way_out, MODAL_MARGIN, text));
    lines.push(Line::raw(""));
    lines.push(Line::styled(format!("[ esc {} ]", before.leave()), text).centered());
    lines.push(Line::raw(""));

    let tail = format!(" {outcome}");
    let to = to.unwrap_or(views::UNNAMED);
    let title = format!(
        "{}{tail}",
        shortened(
            to,
            usize::from(DISMISS_BOX).saturating_sub(2 + width(&tail))
        )
    );
    boxed(frame, body, DISMISS_BOX, &title, lines, screen);
}

/// **One of `ops.rs`'s verdict lines as a dialog draws it** — its first letter raised and a full
/// stop added (`screens/dialogs.md` § Scale, § Delete).
///
/// **`ops.rs` keeps the one literal and this is the only place it is reshaped.** Those constants
/// are lower case with no stop because that is right for the headless surface `main.rs` prints
/// them on; the drawn box wants a sentence. Until this existed, the fixtures in `ui_tests.rs`
/// carried a capitalised copy that no code path could produce — a test asserting a string the
/// product does not build, which CLAUDE.md § Tests must not lie forbids by name (both reviewers,
/// 2026-09-10).
///
/// **The product's own name is never raised**, which is why this is not [`capitalised`] alone:
/// `k8rs did not check this one with the cluster first` starts with [`NAME`], and *"K8rs"* is a
/// word this product never spells. `screens/dialogs.md` draws it lower case.
fn spoken(line: &str) -> String {
    let raised = if line.starts_with(NAME) {
        line.to_owned()
    } else {
        capitalised(line)
    };
    if raised.ends_with('.') {
        raised
    } else {
        format!("{raised}.")
    }
}

/// A verb with its first letter raised, for a title bar — `scale` is `ops::Operation::verb`'s own
/// spelling and `Scale payments/web` is `screens/dialogs.md`'s — and the first word of a sentence
/// [`failed`] builds.
///
/// **ASCII, and that is a fact about the input rather than an assumption**: every verb this is
/// handed is one of the literals the driver holds, and every sentence starts on a word
/// `views::because` wrote, never a word the API sent.
fn capitalised(verb: &str) -> String {
    let mut characters = verb.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

// --- THE DIALOGS END ---

// --- THE SIDEBAR START ---

/// **One `List`, flat, with the section headers drawn and never selected**
/// (`screens/widgets.md` § 2).
///
/// The rows come from [`views::sidebar`] — which is where invariant 12's join between the
/// cluster's kinds and k8rs's five sections lives (NOTES § D248) — and the cursor walks
/// [`views::selectable`]'s answer, so `↑↓` skips `RESOURCES` and `ANALYSIS` without this file
/// knowing which rows those are.
fn sidebar(frame: &mut Frame, area: Rect, app: &App, screen: &Screen) {
    let rows = views::sidebar(screen.kinds, screen.reports.len(), app.expanded);
    let picks = views::selectable(&rows, |item| item.selectable());
    // `Cursor::selected` needs one anchor slot per selectable row and reads none of them here:
    // following an object is the key handler's business, and drawing only needs the index.
    let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
    let at = app
        .nav
        .selected(&anchors)
        .and_then(|nth| picks.get(nth))
        .copied();

    let inside = usize::from(area.width).saturating_sub(width(MARKER));
    let items: Vec<ListItem> = rows
        .iter()
        .map(|item| {
            let (indent, label, style, badge) = match *item {
                NavItem::Alerts => (0, "ALERTS", theme::TEXT, tally(screen)),
                NavItem::Header(name) => (0, name, theme::DIM, Vec::new()),
                NavItem::Group(group) => (1, group.label(), theme::TEXT, Vec::new()),
                NavItem::Kind(nth) => (
                    3,
                    screen
                        .kinds
                        .get(nth)
                        .map_or("", |kind| kind.plural.as_str()),
                    theme::TEXT,
                    Vec::new(),
                ),
                NavItem::Report(nth) => match screen.reports.get(nth) {
                    Some((label, report)) => (
                        1,
                        *label,
                        theme::TEXT,
                        value(report.and_then(|report| report.badge.as_ref()), screen),
                    ),
                    None => (1, "", theme::TEXT, Vec::new()),
                },
            };
            // **A kind's plural is the one label here the cluster chose**, so it is the one that
            // gives way — from its front, where `persistentvolumes` and `persistentvolumeclaims`
            // are alike (`screens/widgets.md` § 7, cut 10; NOTES § D266). A kind row carries no
            // badge, so its room is the column less its indent.
            let label = match item {
                NavItem::Kind(_) => Cow::Owned(shortened(label, inside.saturating_sub(indent))),
                _ => Cow::Borrowed(label),
            };
            let mut spans = vec![Span::styled(
                format!("{blank:indent$}{label}", blank = ""),
                screen.fg(style),
            )];
            let pad = inside.saturating_sub(indent + width(&label) + spanned(&badge));
            spans.push(Span::raw(" ".repeat(pad)));
            spans.extend(badge);
            ListItem::new(Line::from(spans))
        })
        .collect();

    let mut state = ListState::default().with_selected(at);
    frame.render_stateful_widget(
        List::new(items)
            .highlight_symbol(MARKER)
            // **The fill and the mark together** — `PANEL` degrades to nothing at sixteen
            // colours on purpose, and `▸` is what carries the selection there (`theme.rs`).
            .highlight_style(Style::new().bg(ink(theme::PANEL, screen.depth))),
        area,
        &mut state,
    );
}

/// `3 ● 7 ▲` — **owners, not pods** (`screens/alerts.md`), and only the bands that have
/// something in them, so the badge never claims a count it did not find.
///
/// Which pane states it counts at all is [`found`]'s, so this badge and the browser's `●` marks
/// cannot answer differently.
fn tally<'a>(screen: &Screen) -> Vec<Span<'a>> {
    let cards = found(screen.alerts);
    let mut spans = Vec::new();
    for severity in [Severity::Critical, Severity::Warn] {
        let count = cards
            .iter()
            .filter(|card| card.severity() == severity)
            .count();
        if count == 0 {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        let (colour, signal) = theme::band(severity);
        spans.push(Span::styled(
            format!("{count} {}", mark(signal)),
            screen.fg(colour),
        ));
    }
    spans
}

/// A report's badge — `capacity  1 ▲`, `certificates  30d`.
///
/// **The glyph is the unit of a count and nothing else** (`screens/widgets.md` § 2, the
/// badge-glyph rule): `1` counts nothing and `1 ▲` counts one warning, while `30d` already
/// states its own unit and `30d ▲` would invent a second reading of it. The discriminator is the
/// value's own spelling, because [`Badge`] deliberately carries a count and a duration in one
/// field.
fn value<'a>(badge: Option<&'a Badge>, screen: &Screen) -> Vec<Span<'a>> {
    let Some(badge) = badge else {
        return Vec::new();
    };
    let (colour, signal) = theme::band(badge.severity);
    let style = screen.fg(colour);
    let mut spans = vec![Span::styled(badge.value.as_str(), style)];
    if !badge.value.is_empty() && badge.value.chars().all(|digit| digit.is_ascii_digit()) {
        spans.push(Span::styled(format!(" {}", mark(signal)), style));
    }
    spans
}

// --- THE SIDEBAR END ---

// --- THE CONTENT PANE START ---

/// **Which pane the content area is**, and inside each one **three answers, three screens**
/// (PRIOR-ART § C2): *nothing came back yet* is not *there is nothing* is not *we were not
/// allowed to look*. The three-arm match is what makes the second unreachable from the other two,
/// and each pane repeats it rather than sharing a two-state helper that could collapse them.
///
/// **[`View::Analysis`] has two answers and not three**, which is the one place this file's
/// three-answer rule genuinely does not apply: a report is computed from the snapshot rather than
/// fetched, so *we were not allowed to look* and *there is nothing to say* are both inside the
/// report already ([`Screen::reports`]). What is left is *nothing came back yet*, and it draws
/// [`note`] — the same block the other two panes draw while they wait, because it is the same
/// wait.
fn content(frame: &mut Frame, area: Rect, app: &App, screen: &Screen) {
    // **A detail is drawn over the view, not instead of one** ([`Screen::detail`]): the view
    // underneath is still what `esc` goes back to, so it is not cleared and not consulted.
    if let Some(open) = screen.detail {
        detail(frame, area, app, screen, open);
        return;
    }
    match app.view {
        View::Resources(nth) => browser(frame, area, app, screen, screen.kinds.get(nth)),
        View::Analysis(nth) => match screen.reports.get(nth).and_then(|(_, report)| *report) {
            None => note(frame, area, screen, false, None),
            Some(report) => analysis(frame, area, app, screen, report),
        },
        View::Alerts => match screen.alerts {
            // **The two states that draw the centred block draw no banner over it**, because there
            // the caveat is one of [`Screen::note`]'s own paragraphs instead: *clock skew is drawn
            // in whichever family the rest of the screen is already in — it does not bring its own*
            // (`screens/states.md` § Nothing is broken, and the clock is still off).
            //
            // **The audit sentence is the exception, and it is passed here rather than appended
            // inside [`note`]** (`screens/states.md` § On a healthy or a still-loading Alerts
            // screen, re-ruled 2026-09-12): this is the only pane that draws the block with no
            // [`caveats`] above it, so without it a healthy cluster — empty all session — and the
            // first frame of every run said nothing about a dead write path. It is read off
            // [`Screen::writes`] and not left to the caller, because the caller is Phase 12's and
            // NOTES § D21 is a rule nothing would enforce otherwise.
            Pane::Loading => note(frame, area, screen, false, screen.writes.said()),
            Pane::Denied(said, cards) => {
                let rest = caveats(frame, area, screen, Some(said), FLOOR);
                alerts(frame, rest, app, screen, cards);
            }
            // **`○  nothing is broken` is a claim about the cluster now, so it needs the link**
            // (`screens/states.md` § Alerts had already found nothing, and then the link went,
            // NOTES § D266) — structural for [`clock`]'s reason. With the link down an empty list
            // falls through to the arm below and draws what it has, which is no card and no claim;
            // the sentence that page draws is the caller's, over `Pane::Denied`, as it is for a
            // stale list.
            Pane::Ready(cards) if cards.is_empty() && screen.link == Link::Live => {
                note(frame, area, screen, true, screen.writes.said());
            }
            Pane::Ready(cards) => {
                let rest = caveats(frame, area, screen, None, FLOOR);
                alerts(frame, rest, app, screen, cards);
            }
        },
    }
}

/// **The sentences that are true of the screen rather than of the list under it**, stacked above
/// whatever the pane answered.
///
/// **A rank, not a draw order that happens to cut the last one** (`screens/states.md` § Your clock
/// and a scoped namespace together, re-ruled 2026-09-12): the clock, then the pane's own reason,
/// then the audit sentence — and each is handed what the ones above it left, so **the audit
/// sentence is the first to give way and neither of the other two ever gives way to feed it**. It
/// is the one fact on the screen with a second carrier: the footer already withholds `s` and `r`,
/// and the header will read `read-only`. The clock and the pane's reason — which namespace, which
/// check is off, what command renews a login — are said nowhere else. The order before this put the
/// audit sentence second, and the pane's own reason was the one cut: *"One node check is off"*
/// beside a namespace scope, `aws sso login` beside an expired login, and with all three queued the
/// namespace banner gone with no mark (`k8s-admin`, 2026-09-12, round two).
///
/// **One function because both panes stack them**, and because a second copy is how the browser and
/// Alerts come to draw one run's caveats in two different orders.
fn caveats(frame: &mut Frame, area: Rect, screen: &Screen, said: Option<&str>, keep: u16) -> Rect {
    let mut rest = area;
    let mut left = usize::from(area.height.saturating_sub(keep));
    for sentence in [clock(screen), said, screen.writes.said()]
        .into_iter()
        .flatten()
    {
        // **Two rows is the least a banner can say anything in** — one marked line and the blank
        // under it. Below that the banner draws nothing at all, not a fragment and not a dangling
        // mark. **Only the audit sentence reaches this in practice, and that is arithmetic, not a
        // guard**: the clock's two sentences are four or five rows at the floor, so the pane's own
        // reason is always handed at least eight.
        if left < 2 {
            break;
        }
        let before = rest.height;
        rest = banner(frame, rest, screen, sentence, left);
        // **Saturating, because a renderer may not panic on arithmetic.** [`banner`] clamps its own
        // height to the `Rect` it was handed as well as to this budget, so a wider one than was
        // granted is not reachable today — and a subtraction that only happens not to underflow is
        // one edit away from taking the terminal with it.
        left = left.saturating_sub(usize::from(before.saturating_sub(rest.height)));
    }
    rest
}

/// **The cards `/` and `n` leave** — the one place the Alerts list is narrowed, read by the pane
/// and by [`offered`] alike (`screens/widgets.md` § 2b).
///
/// **`pub` because the key router is the other caller** ([`offered`]'s reason, one function over):
/// `crate::views::Cursor::follow` is documented as taking the anchors *the pane draws*, on every
/// filter keystroke, and this is the only thing that produces them. A router that could not call
/// it would either move the cursor over the unfiltered list — the highlight and `⏎` on different
/// objects, which is invariant 2's *explicitly selected object* defeated — or re-derive the
/// filter in `main.rs`, which is the second copy of a shared read NOTES § D103 exists about.
///
/// **What `/` matches is what a card's own fields say, and that is *not* the whole of what the
/// card draws** (`k8s-admin`, 2026-09-18). Matched: the identity line as [`name`] spells it, and
/// the title, the evidence **and the remedy** of every finding filed under that owner. **Not
/// matched, and each for a reason:** the `· 3 of 5 pods` fragment and the `4 min ago` age, which
/// [`identity`] composes at draw time out of counts and a clock rather than reading off the card
/// — so `/3 of 5` finds nothing, measured. Making them matchable means matching a string this
/// file builds per frame against a moment that moves, which is a filter whose answer changes
/// while nobody types.
///
/// **[`shown_rows`] is not this rule at a different type**, and the two docs say so separately
/// on purpose: it matches every cell the server sent, including the `priority: 1` columns
/// [`grid`] does not draw.
///
/// **The remedy is the half this function missed, and the omission had a justification that was
/// itself the defect** (`tester`, 2026-09-18). This doc read *not a finding's `kubectl` line,
/// neither of which is on screen for the reader to have been typing at* — true of
/// `Finding::kubectl_cmd`, which no card draws, and false of [`crate::rules::Finding::action`],
/// which [`lines`] draws on **every** card behind its `→`. Measured: a card showing
/// `→ raise limits.memory, or find the leak` answered `/raise limits` with *No problems match
/// "raise limits"* — a list narrowed away from a string the reader could read on screen, which is
/// the one thing a filter may not do.
///
/// **`n` is the owner's namespace and not a string in the fields**, so a node card is refused by
/// any namespace filter rather than matched by a name that happens to hold one.
pub fn shown_cards<'a>(app: &App, cards: &'a [Card]) -> Vec<&'a Card> {
    cards
        .iter()
        .filter(|card| {
            let named = name(card.owner.namespace.as_deref(), &card.owner.name);
            let mut fields = vec![named.as_str()];
            for finding in &card.findings {
                fields.push(&finding.title);
                fields.push(&finding.evidence);
                fields.push(&finding.action);
            }
            app.filters
                .matches(card.owner.namespace.as_deref(), &fields)
        })
        .collect()
}

/// **The table rows `/` and `n` leave** — [`shown_cards`]'s other half, over the browser's own
/// answer.
///
/// **`pub` for [`shown_cards`]'s reason** — the key router needs the anchors the pane draws.
///
/// **Every cell the server printed, which is more than the reader sees** — the `priority: 1`
/// columns [`grid`] drops are matched too, so `/worker3` finds a pod by the node it runs on with
/// no `NODE` column on screen (measured, `k8s-admin` 2026-09-18). That is deliberate and it is
/// **not** [`shown_cards`]' rule: a table row *is* the object, every cell of it came from the
/// object's own printer, and a column the printer marked wide is still that object's — where a
/// card is a composition this file makes and only the parts it was handed can be matched. The two
/// docs are separate because the rules are.
///
/// `n` is [`crate::k8s::Row::namespace`], so a cluster-scoped row is refused by a namespace
/// filter here exactly as a node card is one pane over.
pub fn shown_rows<'a>(app: &App, table: &'a crate::k8s::Table) -> Vec<&'a crate::k8s::Row> {
    table
        .rows
        .iter()
        .filter(|row| {
            let cells: Vec<&str> = row.cells.iter().map(String::as_str).collect();
            app.filters.matches(row.namespace.as_deref(), &cells)
        })
        .collect()
}

/// **`filter: "web"   esc clears it`** — the one line that says a list is narrowed once the cursor
/// has left the footer (`screens/widgets.md` § A committed filter is drawn at rest, too). It
/// returns the pane left under it, the way [`banner`] does, and takes nothing at all where there
/// is nothing to say.
///
/// **Not drawn while a filter is being typed**, because the footer is already saying it live, and
/// not drawn over the zero-match sentence, which already names what was typed in a whole sentence
/// — both are that section's own rulings, and the second is why this takes a `Rect` back rather
/// than being folded into [`caveats`].
///
/// **Calm: dim, and none of the four reserved glyphs** (`screens/README.md` § the five rules,
/// item 4). A narrowed list is neither a severity nor a connection problem.
///
/// **`esc clears it` while one field is set, and no hint at all once both are** — measured, and
/// the opposite of this row's first draft (`reports/2026-09-18-filter-and-container-picker.md`
/// § 4): the labels alone cost 10 and 18 columns, so a hint naming which field `esc` reaches
/// first leaves two real values two columns to share on a 53-column row. **The hint is
/// recoverable by reopening `/` or `n`, whose own footer names `esc` again; what the reader typed
/// is written down nowhere else**, so the hint is what goes.
///
/// **The namespace half is labelled `namespace like:`** ([`Typing::prompt`]), because a bare
/// `namespace:` one row under the title's own `ns: payments` reads as a second scope rather than
/// a substring filter.
fn narrowed(frame: &mut Frame, area: Rect, app: &App, screen: &Screen) -> Rect {
    if app.typing.is_some() || !app.filters.any() {
        return area;
    }
    let set: Vec<(Typing, &Input)> = [
        (Typing::Text, &app.filters.text),
        (Typing::Namespace, &app.filters.namespace),
    ]
    .into_iter()
    .filter(|(_, held)| !held.is_empty())
    .collect();
    // **The hint is what gives way, and it gives way before either value does** — measured, not
    // reasoned (`reports/2026-09-18-filter-and-container-picker.md` § 4). `filter: ""` and
    // `namespace like: ""` cost 10 and 18 columns before either holds a character; with the gap
    // between them and `esc clears filter` and its own gap, two real values are left fighting
    // over **two** columns of the 53 this row has — `/ kube` beside `n kube-sys`, eight typed
    // characters, already over by one with the hint on the line. **What the reader typed is
    // written down nowhere else; `/` or `n` reopens typing on the field that holds it, and the
    // footer there names `esc` again.** `?` is *not* a second carrier and this comment claimed it
    // was: `screens/help.md`'s own row reads `esc   back / close` and says nothing about a filter
    // (`k8s-admin`, 2026-09-18). The hint goes because it is the half that can be got back. It
    // shows only while a single field is set, where there is one thing `esc` could mean and
    // `esc clears it` needs to name nothing further.
    let hint = match set.len() {
        1 => Some("esc clears it"),
        _ => None,
    };
    let [top, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    let row = padded(top);
    let room = usize::from(row.width)
        .saturating_sub(hint.map_or(0, |hint| width(hint) + FILTER_GAP.len()));
    // **What gives way after that is the *value*, and never the label that says which field it
    // is.** A run front-cut whole would eat `filter:` first and leave a quoted string belonging to
    // nobody, which is worse than the overflow it was fixing.
    //
    // **Silent below the threshold and exact at it**, the rule this page already states for the
    // dropped-lines line: whole while the row holds them, and only then each value cut to an equal
    // share of what the labels, their quotes and the gaps leave. `kube` beside `kube-sys` fits
    // with the hint gone; only a value nobody types on purpose reaches the share.
    let quoted = |field: &Typing, text: &str| format!("{}: \"{text}\"", field.prompt());
    let whole: Vec<String> = set
        .iter()
        .map(|(field, held)| quoted(field, held.text()))
        .collect();
    let joined = whole.join(FILTER_GAP);
    let values = if width(&joined) <= room {
        joined
    } else {
        let fixed: usize = set
            .iter()
            .map(|(field, _)| width(&quoted(field, "")))
            .sum::<usize>()
            + FILTER_GAP.len() * (set.len() - 1);
        let each = (room.saturating_sub(fixed) / set.len()).max(width(CUT));
        set.iter()
            .map(|(field, held)| quoted(field, &shortened(held.text(), each)))
            .collect::<Vec<String>>()
            .join(FILTER_GAP)
    };
    let said = match hint {
        Some(hint) => format!("{values}{FILTER_GAP}{hint}"),
        None => values,
    };
    frame.render_widget(
        Paragraph::new(Line::styled(said, screen.fg(theme::DIM))),
        row,
    );
    rest
}

/// **A filter that hides every row is a third reason a list can be empty** — not `○ nothing is
/// broken`, which is a verdict on the cluster, and not an empty kind, which is a fact about the
/// kind (`screens/states.md` § The filter hides every row).
///
/// **It names what was typed**, because the reader typed it a keystroke ago and may have mistyped
/// it, and it draws **none of the four reserved glyphs**: it carries no severity and it is not a
/// connection or trust problem (`screens/README.md` § the five rules, item 4).
///
/// **The noun is the pane's own**: `problems` on Alerts, which has no one kind to name and which
/// `screens/resources.md` already calls problems on this same product, and the kind's own plural
/// in the browser.
///
/// **The filter text needs no strip here** — it is [`crate::views::Input`]'s, which refused every
/// control character and bounded the length on the way in (invariant 9), and it is the only class
/// of string on this screen that never came off the API at all.
fn hidden(frame: &mut Frame, area: Rect, screen: &Screen, noun: &str, filters: &Filters) {
    // **Three sentences and no fourth** (`screens/states.md` § The filter hides every row). *No
    // filter set at all* is not one of them and cannot reach here — a list with no filter over it
    // has not been narrowed, so both callers ask [`shown_cards`]/[`shown_rows`] first — and the
    // `(true, true)` arm is written out rather than folded into the namespace one, where it would
    // have printed `a namespace like ""` if anything ever did reach it (`k8s-admin`, 2026-09-18).
    // **Its own sentence names no value, because there is none to name**: a
    // `crate::views::Filters::matches` that answered no to everything would land here, and *what
    // was typed* would then be a claim about a reader who typed nothing.
    let said = match (filters.text.is_empty(), filters.namespace.is_empty()) {
        (false, true) => format!("No {noun} match \"{}\".", filters.text.text()),
        (true, false) => format!(
            "No {noun} match a namespace like \"{}\".",
            filters.namespace.text()
        ),
        (false, false) => format!(
            "No {noun} match \"{}\" in a namespace like \"{}\".",
            filters.text.text(),
            filters.namespace.text()
        ),
        (true, true) => format!("No {noun} match the filter."),
    };
    let dim = screen.fg(theme::DIM);
    // **The measure is the pane, as [`empty`]'s is**: this is one sentence of dim text in the slot
    // a card or a table row would have taken, not the several paragraphs [`BLOCK`] is set for.
    //
    // **And it is cut at [`MATCH_LINES`], which is the security gate's *sizes are bounded* row and
    // not tidiness** (`tester`, 2026-09-18). Both values here are a whole
    // [`crate::k8s::IDENTIFIER`] — 512 bytes each, which `crate::views::Input` allows because a
    // delete can legitimately ask for that much. Wrapped and handed to [`centred`], such a filter
    // filled the pane and was clipped by the `Rect` **mid-token, with no `…` and no closing
    // quote**: the silent truncation `screens/widgets.md` § 7 forbids by name, on the one sentence
    // whose whole job is to say exactly what the reader typed. [`cut`] is the same marked cut the
    // card's evidence, the at-rest line and the picker's restart hint already make.
    let lines = cut(&said, usize::from(area.width), MATCH_LINES)
        .into_iter()
        .map(|line| Line::styled(line, dim).centered())
        .collect();
    centred(frame, area, lines);
}

/// **The rows a banner may spend over a pane that has content of its own** — everything but
/// [`FLOOR`]. One function because five panes draw a refusal over something, and a budget computed
/// per pane is how two of them come to disagree about what a list is owed.
fn floor(area: Rect) -> usize {
    usize::from(area.height.saturating_sub(FLOOR))
}

/// **The clock sentence, or nothing while k8rs is not completing requests** (`screens/states.md`
/// § While disconnected, or while the login has expired).
///
/// **Structural rather than the caller's to remember**, because the caller is Phase 12's and will
/// meet that section long after this one: a skew is measured off a live response's `Date` header,
/// and one kept from the *last* successful request is exactly the guess *a vital that cannot be
/// read is blank, never guessed* refuses. **[`Writes::Unaudited`] does not hide with it** and that
/// is stated in the same section: a state directory that could not be opened is fixed for the run
/// and goes nowhere while the connection is down, so a sentence that had hidden itself would have
/// to reappear from nowhere with no event to explain it.
fn clock<'a>(screen: &Screen<'a>) -> Option<&'a str> {
    screen.clock.filter(|_| screen.link == Link::Live)
}

/// The centred block an empty or still-loading pane draws (`screens/states.md`).
///
/// **`○  nothing is broken` is this file's line and the paragraphs under it are the caller's**,
/// because the glyph is `theme.rs`'s and the counts in the sentences are the store's. The
/// loading screen has no headline of its own for the same reason it has no glyph: *still
/// reading* is not a severity.
///
/// **A caller with nothing to say still gets a sentence**, and that is the whole point of this
/// pane having three states: an empty block would put *loading* and *there is nothing* back on
/// one screen (PRIOR-ART § C2). `reading the cluster…` is `screens/states.md`'s own line without
/// the count it has no source for, which is the same rule the header's vitals obey — blank, not
/// guessed.
///
/// **`first` is a sentence ranked above every paragraph the caller handed over, and only
/// [`content`]'s Alerts arm passes one** — the audit sentence, on the one pane that draws this
/// block with no [`caveats`] above it (`screens/states.md` § On a healthy or a still-loading Alerts
/// screen, re-ruled 2026-09-12). Every other caller either stacks its banners already or has no
/// seat for the sentence once it answers, and appending it here for all seven drew it twice on the
/// browser and made it flicker on every detail tab (`k8s-admin`, 2026-09-12, round two).
fn note(frame: &mut Frame, area: Rect, screen: &Screen, healthy: bool, first: Option<&str>) {
    let mut lines: Vec<Line> = Vec::new();
    let mut budget = usize::from(area.height.saturating_sub(FLOOR));
    if healthy {
        lines.push(calm(screen, "nothing is broken").centered());
        lines.push(Line::default());
        budget = budget.saturating_sub(lines.len());
    }
    let measure = usize::from(BLOCK);
    let waiting = ["reading the cluster…".to_owned()];
    let handed = if !healthy && screen.note.is_empty() {
        &waiting[..]
    } else {
        screen.note
    };
    // **The rank is positional, and each paragraph is handed what the ones above it left** — the
    // banners' own rule in [`caveats`], so *whatever the caller put last is what gives way* and a
    // paragraph with no room left draws nothing rather than a mark on the whole sentence above it.
    // One run of lines cut as a block put `…` on the end of a finished sentence, and one row more
    // drew a line holding nothing but the mark. **Still the same 13-of-16 cap the banner path
    // keeps**: `centred` clips whatever it is handed with no mark of its own.
    let mut text: Vec<String> = Vec::new();
    for paragraph in first.into_iter().chain(handed.iter().map(String::as_str)) {
        let gap = usize::from(!text.is_empty());
        let share = budget.saturating_sub(text.len());
        if share <= gap {
            break;
        }
        if gap == 1 {
            text.push(String::new());
        }
        text.extend(marked(wrapped(paragraph, measure), measure, share - gap));
    }
    let dim = screen.fg(theme::DIM);
    lines.extend(text.into_iter().map(|line| Line::styled(line, dim)));
    centred(frame, area, lines);
}

/// **`○  nothing is broken`, `○  no logs yet`, `○  none right now`** — the product's own calm
/// headline, in the one place its glyph is read off `theme.rs` (`screens/states.md`
/// § Nothing is broken).
///
/// **One function because three panes draw it and the glyph is a severity's**, so a second copy
/// would be a second reader of `theme::band` — the thing [`found`] already refuses one file up.
///
/// **It does not centre itself**: describe draws this same headline left-flush under a heading,
/// because there it is a section of a pane and not the whole of one.
fn calm<'a>(screen: &Screen, headline: &str) -> Line<'a> {
    let (colour, signal) = theme::band(Severity::Info);
    Line::from(vec![
        Span::styled(format!("{}  ", mark(signal)), screen.fg(colour)),
        Span::styled(headline.to_owned(), screen.fg(theme::TEXT)),
    ])
}

/// The calm headline, a blank, and the sentence that says *why* — `screens/states.md`'s own shape,
/// where **the second paragraph is the point of the screen** and not decoration.
fn calmly(frame: &mut Frame, area: Rect, screen: &Screen, headline: &str, said: &str) {
    let mut lines = vec![calm(screen, headline).centered(), Line::default()];
    lines.extend(set(said, usize::from(BLOCK), screen.fg(theme::DIM)));
    centred(frame, area, lines);
}

/// A short block of lines in the middle of a pane — the shape every *nothing to draw* screen
/// takes (`screens/states.md`). One function because [`note`] and [`empty`] draw the same block
/// with different sentences in it, and two copies of a centring calculation is how two panes
/// start putting their sentence in different places.
///
/// **[`BLOCK`] is a floor, not a ceiling.** It is the measure several paragraphs of prose want,
/// and a caller whose line is legitimately wider — `screens/states.md` § *An empty kind in the
/// browser*, one line of dim text that the file draws unbroken — would otherwise have it clipped
/// at `BLOCK` columns. The pane is the real ceiling. [`note`]'s own lines are wrapped at `BLOCK`
/// before they arrive, so nothing about the Alerts block moves.
fn centred(frame: &mut Frame, area: Rect, lines: Vec<Line>) {
    let widest = lines.iter().map(Line::width).max().unwrap_or(0);
    let widest = u16::try_from(widest).unwrap_or(u16::MAX);
    let block = area.centered(
        Constraint::Length(widest.max(BLOCK).min(area.width)),
        Constraint::Length((lines.len() as u16).min(area.height)),
    );
    frame.render_widget(Paragraph::new(Text::from(lines)), block);
}

/// **We were not allowed to look**, in the words the screen prints — never a `403` and never the
/// word RBAC (invariant 14, `crate::views::Pane::Denied`). It sits where a banner sits, at the
/// top of the pane, because a reader who has to scroll to learn the list is incomplete has
/// already believed it.
///
/// **A banner over the list, never instead of it.** Whatever did come back is drawn underneath by
/// the same [`alerts`] the ready pane uses — `screens/states.md` § *You can only see some
/// namespaces* draws the sentence above the cards, and § *Your login expired* states the rule:
/// *"Stale data stays visible and stays labelled… k8rs does not clear the screen because it lost
/// its token."* Nothing came back is an empty list and draws nothing, which is not the same
/// screen as `nothing is broken` and must never become it.
///
/// **It returns the pane that is left under it**, so the caller draws its own list there — cards
/// for Alerts, a table for the browser. A banner that also drew the list could only ever serve
/// one pane, and the sentence is the half both share.
///
/// **A banner that opens with the alarm mark spends it once, and everything under it hangs at the
/// column the text started on** (`screens/states.md` § Rules that hold across every state on this
/// page, `tui-designer` 2026-09-12). It is a rule about the *paragraph* and not about word wrap:
/// § *The connection dropped* and § *Your login expired* both draw three whole sentences that way,
/// so a renderer that indented only a wrapped continuation would put the mark back at the head of
/// the next sentence and make it read as repeating once per sentence. The unmarked family —
/// § *You can only see some namespaces*, the audit sentence — hangs by nothing, which is the same
/// arithmetic with no mark to pay for.
fn banner(frame: &mut Frame, area: Rect, screen: &Screen, said: &str, most: usize) -> Rect {
    // **`most` is rows, and one of them is the trailing blank** — so the text gets one fewer, and
    // a sentence past that budget is cut at a word boundary behind a visible [`CUT`], the same
    // rule the card's evidence line and the command log already follow (`screens/widgets.md` § 7,
    // `screens/states.md` § Your clock and a scoped namespace together). Nothing here is bounded
    // by the sentence's own length: `ops::audit_log` measured 1383 characters against a pane that
    // holds about 880 at the floor, and an unbounded banner took the whole list with it
    // (`k8s-admin`, 2026-09-12).
    let columns = usize::from(padded(area).width);
    // **The mark is read off `theme.rs` and not spelled again here** ([`theme::ALARM`], the one
    // place `⚠` is written): the sentence is still the caller's and still carries its own mark —
    // what this file owns is the column everything after the first line starts at.
    let alarm = format!("{} ", mark(theme::ALARM));
    let hang = if said.starts_with(&alarm) {
        width(&alarm)
    } else {
        0
    };
    // **Plain `saturating_sub` and no floor under it**: with no mark this is `columns` exactly, so
    // an unmarked banner is drawn at the width it was drawn at before this hang existed.
    let mut text = paragraphed(said, columns.saturating_sub(hang));
    // **Blank rows stay blank** — a paragraph separator carrying trailing spaces is a row that
    // reads empty and measures wide, and [`marked`] would then hang [`CUT`] off one.
    let under = " ".repeat(hang);
    for line in text.iter_mut().skip(1).filter(|line| !line.is_empty()) {
        line.insert_str(0, &under);
    }
    let text = marked(text, columns, most.saturating_sub(1).max(1));
    // The banner, then the blank row that separates it from the first card — the same blank a
    // card puts between itself and the next one.
    let height = u16::try_from(text.len() + 1)
        .unwrap_or(u16::MAX)
        .min(area.height);
    let [top, rest] =
        Layout::vertical([Constraint::Length(height), Constraint::Min(0)]).areas(area);
    let lines: Vec<Line> = text
        .into_iter()
        .map(|line| Line::styled(line, screen.fg(theme::TEXT)))
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)), padded(top));
    rest
}

/// The card region: the pane less a two-column pad each side — 53 columns at the floor
/// (`screens/alerts.md` § The columns). [`indented`]'s reason for `Rect::inner`, at two columns.
fn padded(area: Rect) -> Rect {
    area.inner(Margin::new(PAD, 0))
}

/// **One `ListItem` per card, each one several `Line`s tall**, with a blank line between cards —
/// half the design (`screens/alerts.md`).
///
/// **No card carries a selection marker**, which is what every mockup draws (`theme::SELECTION`
/// is the sidebar's today). The `ListState` is here for the other half of its job: it keeps the
/// selected card in view, so `↓` reaches a tall card's action rather than scrolling past it
/// (`screens/widgets.md` § 4).
fn alerts(frame: &mut Frame, area: Rect, app: &App, screen: &Screen, cards: &[Card]) {
    // **A filter that hid every card is its own screen and not an empty list**
    // (`screens/states.md` § The filter hides every row). A list that was empty before the filter
    // was typed is not this: there was nothing for `/` to hide, so whatever that pane already
    // said — `○ nothing is broken`, or a refusal's banner over no cards — is still the true one.
    let left = shown_cards(app, cards);
    if left.is_empty() && !cards.is_empty() {
        return hidden(frame, padded(area), screen, "problems", &app.filters);
    }
    let cards = left;
    // **And no line over a pane with no card under it either** — a refusal that came back with
    // nothing has no list for a filter to have narrowed, so saying one was narrowed would be a
    // claim about rows that are not there (the same rule [`browser`] applies one pane over).
    let area = match cards.is_empty() {
        true => area,
        false => narrowed(frame, area, app, screen),
    };
    let area = padded(area);
    let region = usize::from(area.width);
    let mut drawn: Vec<Vec<Line>> = cards
        .iter()
        .map(|card| lines(card, screen, region))
        .collect();
    // **A card taller than the room left is cut down, never dropped.** ratatui's `List` skips an
    // item whole when it does not fit — `ratatui_widgets::list::rendering::get_items_bounds` breaks
    // on `height_from_offset + item.height() > max_height`, read off the crate — so under a stack
    // of banners at the floor the sidebar went on counting a card nobody could see (`k8s-admin`,
    // 2026-09-12). What is kept is what `screens/states.md` draws in exactly that case: the
    // identity line and its title, *"trimmed to its title… the full text is one `⏎` away"*
    // (§ Your clock and a scoped namespace together). No [`CUT`] mark, because the card is not a
    // sentence with its end shaved off — every part of it is whole, and the parts that did not fit
    // are behind a key the footer names.
    //
    // **Every card, not the first**: `List` scrolls its offset to the *selected* item before it
    // measures, so trimming card 0 alone left the cursor on card 1 skipped whole at the floor — one
    // `↓` from an empty pane (`k8s-admin`, 2026-09-12, round two). A card no taller than the region
    // is untouched by this.
    for card in &mut drawn {
        card.truncate(usize::from(area.height).max(1));
    }
    let items: Vec<ListItem> = drawn
        .into_iter()
        .map(|card| ListItem::new(Text::from(card)))
        .collect();
    let anchors: Vec<Option<&str>> = cards.iter().map(|_| None).collect();
    let mut state = ListState::default().with_selected(app.content.selected(&anchors));
    frame.render_stateful_widget(List::new(items), area, &mut state);
}

/// **One card, in the four parts and only this order**: who · what happened · the evidence ·
/// what to do — then, when the card holds more than one finding, the fifth part that counts the
/// rest, and the blank line that separates it from the next card
/// (`screens/alerts.md` § What each part is, § A card with more than one finding).
///
/// **A card holds every finding filed under one owner and draws the one that decides it**
/// ([`decides`]), so the sentence under the glyph is the sentence that glyph is about. The rest
/// are one `⏎` away, where the whole group is pinned (`screens/detail.md`).
fn lines<'a>(card: &Card, screen: &Screen, region: usize) -> Vec<Line<'a>> {
    let mut drawn = vec![identity(card, screen, region)];
    let body = region.saturating_sub(2);
    let Some(finding) = decides(card, screen.now) else {
        return drawn;
    };

    let text = screen.fg(theme::TEXT);
    let dim = screen.fg(theme::DIM);
    drawn.extend(indent(wrapped(&finding.title, body), "  ", text));
    // **Left out when there is none, never drawn blank** — a blank line is a hole in the middle
    // of a card (`crate::rules::Finding::evidence`).
    if !finding.evidence.is_empty() {
        drawn.extend(indent(
            cut(&finding.evidence, body, EVIDENCE_LINES),
            "  ",
            dim,
        ));
    }
    // **The action is never cut**: a fix the reader cannot finish reading is not a fix. Its
    // continuations indent under the text rather than under the arrow.
    let mut action = wrapped(&finding.action, region.saturating_sub(4)).into_iter();
    if let Some(first) = action.next() {
        drawn.push(Line::from(vec![
            Span::styled("  → ", screen.fg(theme::ACCENT)),
            Span::styled(first, text),
        ]));
        drawn.extend(indent(action.collect(), "    ", text));
    }
    // **The fifth part: one line, a plain count, no glyph** (`screens/alerts.md` § A card with
    // more than one finding). It is dim like the evidence — a pointer, not an instruction — and
    // it has no empty form: a single-finding card draws no placeholder. A glyph here would ask
    // whose severity it carries, when the answer is on the line above and cannot be worse.
    if card.findings.len() > 1 {
        let hidden = card.findings.len() - 1;
        let problems = if hidden == 1 { "problem" } else { "problems" };
        drawn.push(Line::styled(
            format!("  {hidden} more {problems} — ⏎ to see"),
            dim,
        ));
    }
    drawn.push(Line::default());
    drawn
}

/// **Which of a card's findings the four parts above are drawn from** — the card's own severity,
/// which is the worst present, and where more than one finding shares it, the most recent
/// (`screens/alerts.md` § A card with more than one finding).
///
/// The two keys are the list's own — severity, then recency — applied one level down. **What this
/// replaces is *first in `analyze`'s order***: true of the code and invisible to a reader, because
/// which rule number fired first is not a fact anyone reading the screen can reconstruct.
///
/// **Recent is [`Finding::age`] answering, not a timestamp existing** — the same test
/// [`crate::views::Card::newest`] applies to the right edge (NOTES § D246 ruling 4), so a stamp
/// past the skew allowance cannot win a tie it would then draw no age for. Where recency does not
/// resolve it either, the screen file promises nothing and this draws the first at that severity.
///
/// **A hidden finding can never be worse than the drawn one**, which is what lets [`lines`]' count
/// of the rest carry no severity of its own.
fn decides<'a>(card: &'a Card, now: &Time) -> Option<&'a Finding> {
    let worst = card.severity();
    let at_worst = || {
        card.findings
            .iter()
            .filter(move |finding| finding.severity == worst)
    };
    at_worst()
        .filter(|finding| finding.age(now).is_some())
        .max_by(|a, b| a.timestamp.cmp(&b.timestamp))
        .or_else(|| at_worst().next())
}

/// **The identity line: `● name  ·  n of m pods` and the age, exactly one line, never wrapped.**
///
/// The age is laid out **first**, at the width the ladder gave it, right-aligned so its last
/// column is the card region's; the name takes what is left, minus a two-column gap. The
/// give-way order is the screen's and not this file's: `· n of m pods` goes before the name
/// clips, and the age is never touched (`screens/alerts.md` § The age, and what it costs the
/// name).
///
/// **A run of ageless cards is this rule applied more than once**, not a second layout: nothing
/// here looks at a neighbouring card, so an ageless one gets the whole 51 columns and its name
/// ends where the name ends.
fn identity<'a>(card: &Card, screen: &Screen, region: usize) -> Line<'a> {
    let (colour, signal) = theme::band(card.severity());
    let age = card.age(screen.now);
    let measured = age.as_deref().map_or(0, width);
    let body = region.saturating_sub(GUTTER);
    let room = if measured == 0 {
        body
    } else {
        body.saturating_sub(measured + GAP)
    };

    let named = name(card.owner.namespace.as_deref(), &card.owner.name);
    let whole = match card.count() {
        Some(count) => format!("{named}  ·  {count}"),
        None => named,
    };
    // **A name that does not fit gives way by [`name_cut`], never a silent clip** — the clip drew
    // two Deployments one card and dropped the `/` with the name (NOTES § D266).
    let left = if width(&whole) <= room {
        whole
    } else {
        name_cut(card.owner.namespace.as_deref(), &card.owner.name, room)
    };

    let mut spans = vec![
        Span::styled(glyph(signal), screen.fg(colour)),
        Span::styled(left.clone(), screen.fg(theme::TEXT)),
    ];
    if let Some(age) = age {
        spans.push(Span::raw(
            " ".repeat(body.saturating_sub(width(&left) + measured)),
        ));
        spans.push(Span::styled(age, screen.fg(theme::DIM)));
    }
    Line::from(spans)
}

/// `payments/web`, or a bare `node-3` for something cluster-scoped — `None` is not `""`, which
/// would draw as `/node-3` (`screens/README.md` § the five rules).
///
/// **Over the two fields and not over a type**, because the three things that spell this are a
/// [`Card`]'s owner, a detail pane's own heading and a dialog's title bar — a [`ObjectId`] for
/// two of them and a [`views::Object`] for the third. A second joiner beside this one is a second
/// answer for `node-3`.
fn name(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(namespace) => format!("{namespace}/{name}"),
        None => name.to_owned(),
    }
}

/// Wrapped lines under a fixed prefix, which is how a card's body indents.
fn indent<'a>(text: Vec<String>, prefix: &'static str, style: Style) -> Vec<Line<'a>> {
    text.into_iter()
        .map(|line| Line::styled(format!("{prefix}{line}"), style))
        .collect()
}

// --- THE CONTENT PANE END ---

// --- THE BROWSER START ---

/// **Every kind the cluster serves, drawn from the server's own `Table` and nothing else**
/// (`screens/resources.md`, invariant 12). Nothing below names a kind, reads one, or branches on
/// one: the columns, their headers and their order are the API server's answer, which is what
/// makes a CRD display correctly with no line written for it.
///
/// The title is drawn in every state — a pane that has not answered yet still has to say which
/// kind it is about — and the three answers below it are the same three [`content`] gives the
/// Alerts pane, for the same reason.
fn browser(frame: &mut Frame, area: Rect, app: &App, screen: &Screen, kind: Option<&Browsable>) {
    // **Above the title, where this pane already puts its own banner** — a caveat about the run or
    // the clock is not about the kind named on that row ([`caveats`]). **The floor it keeps is two
    // rows wider than the Alerts pane's**, because the title and the blank under it come out of
    // what is left here and out of the list's own rows there.
    let denial = match screen.browser {
        Pane::Denied(said, _) => Some(said.as_str()),
        _ => None,
    };
    let area = caveats(frame, area, screen, denial, FLOOR + HEADING);
    let [head, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    // **The title is padded and the table is not**, which is what both mockups draw: the
    // selection marker is the table's own gutter and sits at the pane's left edge, exactly where
    // the sidebar's does (`screens/resources.md`, `screens/widgets.md` § 2's indent rule). A
    // table padded like a card would spend two columns twice over, on the widest thing on the
    // screen.
    heading(frame, padded(head), screen, kind);
    let scoped = scope(kind, screen).is_some();
    let table = match screen.browser {
        Pane::Loading => None,
        Pane::Ready(table) | Pane::Denied(_, table) => Some(table),
    };
    let left = table
        .map(|table| shown_rows(app, table))
        .unwrap_or_default();
    // **The filter hid every row of a kind that has them** (`screens/states.md` § The filter hides
    // every row) — which is never the same state as a kind that genuinely has none, and never
    // reached while the pane has not answered.
    let vanished = table.is_some_and(|table| !table.rows.is_empty()) && left.is_empty();
    // **The filter line joins the title row it belongs with — and only over rows it could have
    // narrowed.** The zero-match state draws none, which is that section's own ruling: its
    // sentence already names what was typed, and a banner above it would be the same two facts
    // said twice. **A pane with nothing under it draws none either**, which is the same rule read
    // the other way (`tester`, 2026-09-18): a filter set while the first LIST is still in flight
    // drew `filter: "web"   esc clears it` over *reading the cluster…*, claiming a list had been
    // narrowed when no list had arrived — and under a footer that names no `esc`, because
    // `Offer::Nothing` is what a pane with nothing to show answers.
    let rest = if vanished || left.is_empty() {
        rest
    } else {
        narrowed(frame, rest, app, screen)
    };
    let [_, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(rest);
    match screen.browser {
        _ if vanished => hidden(frame, padded(body), screen, plural(kind), &app.filters),
        Pane::Loading => note(frame, body, screen, false, None),
        // **A refusal draws whatever did come back and never the empty sentence below**, which is
        // [`banner`]'s rule one line up: *we were not allowed to look* is not *there is nothing*.
        // A refusal that came back with no rows at all draws the banner and an empty grid.
        Pane::Denied(_, table) => grid(frame, body, app, screen, table, &left, scoped),
        Pane::Ready(table) if table.rows.is_empty() => empty(frame, body, screen, kind),
        Pane::Ready(table) => grid(frame, body, app, screen, table, &left, scoped),
    }
}

/// **The pane title: the kind, and `ns: payments` beside it under two conditions and not one**
/// (`screens/resources.md` § Rules) — discovery's own `namespaced` flag, **and** a namespace scope
/// actually in effect. A namespaced kind browsed with no scope reads the same as a cluster-wide
/// one, which is the ordinary state of the browser today and not an edge case.
///
/// **A label that does not fit is left out, never half-drawn.** `ns: payme` names a namespace that
/// does not exist, and the header's rule for a vital is this file's rule for every fact it draws:
/// blank, never guessed (`screens/widgets.md` § 1a). The kind itself clips at the pane edge like
/// any other string (§ 7) — it is the row's subject, so a shortened one still reads as itself.
fn heading(frame: &mut Frame, area: Rect, screen: &Screen, kind: Option<&Browsable>) {
    let plural = plural(kind);
    let mut spans = vec![Span::styled(plural.to_owned(), screen.fg(theme::TEXT))];
    if let Some(namespace) = scope(kind, screen) {
        let label = format!("ns: {namespace}");
        if width(plural) + GAP + width(&label) <= usize::from(area.width) {
            spans.push(Span::raw(" ".repeat(GAP)));
            spans.push(Span::styled(label, screen.fg(theme::DIM)));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// **The namespace this view is scoped to, or `None`** — one condition per fact, never a list of
/// kinds (`screens/resources.md` § Rules). It decides the title's label *and* whether a row is
/// drawn `namespace/name`, so the two can never answer differently.
fn scope<'a>(kind: Option<&Browsable>, screen: &Screen<'a>) -> Option<&'a str> {
    kind.filter(|kind| kind.namespaced).and(screen.namespace)
}

/// The word the sidebar drew for this kind — **the server's own plural**, never one derived from
/// the kind name ([`Browsable::plural`]). Empty where the view's index has outlived the discovery
/// list it points into, which is the sidebar's own answer for the same case.
fn plural(kind: Option<&Browsable>) -> &str {
    kind.map_or("", |kind| kind.plural.as_str())
}

/// **An empty list of deployments is not `nothing is broken`** — that is the Alerts pane's claim
/// and the strongest one k8rs makes, and a kind with no objects in it is an ordinary answer with
/// no severity at all.
///
/// **The three sentences are `screens/states.md` § An empty kind in the browser's own table** —
/// one row per reason the pane can be empty, and that file decides their wording. The first two
/// name the scope, because *no deployments* means two different things depending on whether one
/// namespace or the whole cluster was looked at. The third has no kind to name — the stale-index
/// case [`plural`] describes — so it names the next thing to try instead, which is that file's
/// closing rule: no state is a dead end.
fn empty(frame: &mut Frame, area: Rect, screen: &Screen, kind: Option<&Browsable>) {
    let plural = plural(kind);
    let said = match (plural, scope(kind, screen)) {
        ("", _) => "no longer in the list — pick another kind".to_owned(),
        (plural, Some(namespace)) => format!("no {plural} in {namespace}"),
        (plural, None) => format!("no {plural} in this cluster"),
    };
    let dim = screen.fg(theme::DIM);
    // **The measure is the pane, not [`BLOCK`]** — every row of that table is one line of dim
    // text, and `BLOCK` is the width the *Alerts* pane's several paragraphs of prose are set to.
    // At `BLOCK` it breaks the third sentence in half — 41 columns against 39. The pane still
    // bounds it, so one too narrow for the sentence wraps rather than overruns; the two scoped
    // sentences are shorter than `BLOCK`, which [`centred`] keeps as its floor, so their block is
    // `BLOCK` wide and centred in the pane.
    let lines = wrapped(&said, usize::from(area.width))
        .into_iter()
        .map(|line| Line::styled(line, dim).centered())
        .collect();
    centred(frame, area, lines);
}

/// **The header row and one row per object, both from `columnDefinitions`** — and, where Alerts
/// bleeds through onto it, a two-column gutter down the left of the names and one line under the
/// table about the row the cursor is on (`screens/widgets.md` § 2, `screens/resources.md`).
///
/// **The `priority: 0` filter is the whole of the column choice**, and the indices it kept are
/// what every cell is then read at: [`crate::k8s::Row::cells`] is aligned to the *whole* column
/// list, so a filtered header over unfiltered cell indices puts `Ready` under `AGE` the moment a
/// server sends a priority-1 column anywhere but the end.
///
/// **The widths are `screens/widgets.md` § 2's, and which column gives way follows from them.**
/// Every column but the first is `Length(natural)` — the wider of its header and its widest
/// cell — and the first is `Min(len(header))`, so leftover space grows the name column and a pane
/// too narrow shrinks that one first: ratatui holds a `Min` at or above its minimum more strongly
/// than it holds a `Length` at its length. That is the mockup's own order of sacrifice — the
/// numbers keep their columns, the name clips — expressed as two constraints rather than a
/// measurement per kind. **The gutter is bought out of the same column**: it widens the first
/// constraint's minimum by [`GUTTER`] and nothing else, so the marks cost the names two columns
/// and cost the numbers none.
///
/// **The cost is linear in rows and every row is walked twice** — once for the column widths,
/// once to build the cells — which is what a column layout computed from content costs. Measured
/// on the pods capture cycled to length, debug build, 2026-09-06: 2.3 ms/frame at 100 rows,
/// 7.4 at 1 000, 30 at 5 000, 117 at 20 000. Drawing only the visible window would halve it and
/// no more, because ratatui walks the whole table itself for its column count; nothing in
/// `screens/` asks for the other half, so it is not spent here.
fn grid(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    screen: &Screen,
    table: &crate::k8s::Table,
    rows: &[&crate::k8s::Row],
    scoped: bool,
) {
    let kept: Vec<(usize, String)> = table
        .columns
        .iter()
        .enumerate()
        .filter(|(_, column)| column.priority == PLAIN)
        .map(|(at, column)| (at, column.name.to_uppercase()))
        .collect();
    // **No column the reader was meant to see is nothing to draw**, and that includes the
    // selection marker: a lone `▸` in a blank pane claims a selected row whose every cell
    // was filtered away. Unreachable from a real API server — every printer, built-in and CRD,
    // emits `Name` at priority 0 — and cheaper to state than to reason about.
    if kept.is_empty() {
        return;
    }

    // **The join, and the whole of it**: a card is filed under an owner, a row *is* an object, and
    // both carry the same uid. Two rows with no uid are not the same object, so the `?` is what
    // makes `None == None` not a match — `table-deployments` was captured with
    // `?includeObject=None` and every row in it lands there.
    let cards = found(screen.alerts);
    let marks: Vec<Option<&Card>> = rows.iter().map(|row| about(cards, row)).collect();
    // **A table nobody has a finding in spends no columns on an empty gutter**, so an unmarked
    // kind draws exactly the columns it drew before Alerts bled through at all. Once one row is
    // marked every row reserves the gutter, marked or not, or the names step in and out by two.
    let gutter = if marks.iter().any(Option::is_some) {
        GUTTER
    } else {
        0
    };

    let widths: Vec<Constraint> = kept
        .iter()
        .enumerate()
        .map(|(nth, (at, header))| {
            let header = width(header) as u16;
            if nth == 0 {
                return Constraint::Min(header + (gutter as u16));
            }
            let widest = rows
                .iter()
                .map(|row| row.cells.get(*at).map_or(0, |cell| width(cell) as u16))
                .max()
                .unwrap_or(0);
            Constraint::Length(header.max(widest))
        })
        .collect();

    let drawn: Vec<Row> = rows
        .iter()
        .zip(&marks)
        .map(|(row, card)| {
            Row::new(kept.iter().enumerate().map(|(nth, (at, _))| {
                let cell = row.cells.get(*at).map_or("", String::as_str);
                if nth > 0 {
                    return Line::raw(cell);
                }
                let mut spans = Vec::new();
                if gutter > 0 {
                    // **The `●` is a `Span` prepended to the first `Cell`, never a column of its
                    // own** (`screens/widgets.md` § 2, the finding-marker row) — a column would
                    // take [`GAP`] beside it and put three blanks between the glyph and the name
                    // every mockup draws one blank in. An unmarked row pushes the same two
                    // columns of nothing, which is what keeps the two aligned.
                    spans.push(match card {
                        Some(card) => {
                            let (colour, signal) = theme::band(card.severity());
                            Span::styled(glyph(signal), screen.fg(colour))
                        }
                        None => Span::raw(" ".repeat(GUTTER)),
                    });
                }
                spans.push(Span::raw(identify(row, cell, scoped)));
                Line::from(spans)
            }))
        })
        .collect();

    // The anchor is the row's `uid` and never its name (`crate::views::Cursor`) — `None` on every
    // row of a Table fetched with `?includeObject=None`, which the cursor falls back to its index
    // for rather than following a string.
    let anchors: Vec<Option<&str>> = rows.iter().map(|row| row.uid.as_deref()).collect();
    let at = app.content.selected(&anchors);
    // **The line is about the row the cursor is on, not about every marked one** (NOTES § D251):
    // one per marked row would be a second list competing with the table above it.
    let selected = at.and_then(|nth| {
        rows.get(nth)
            .copied()
            .zip(marks.get(nth).copied().flatten())
    });
    let (area, under) = match selected {
        // **Right under the last row, and never off the bottom of the pane.** The table takes the
        // height it needs up to two lines short of the pane, so a short kind draws the mockup's
        // own spacing and a long one still keeps the line that says `⏎ to see`. Under three rows
        // the table gets none and only the line is drawn; no pane the frame lays out is that
        // short, the floor being 80×24 and this body thirteen.
        Some(pair) => {
            let tall = u16::try_from(rows.len() + 1).unwrap_or(u16::MAX);
            let [top, _, line, _] = Layout::vertical([
                Constraint::Length(tall.min(area.height.saturating_sub(2))),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .areas(area);
            (top, Some((line, pair)))
        }
        None => (area, None),
    };
    let mut state = TableState::default().with_selected(at);
    frame.render_stateful_widget(
        Table::new(drawn, widths)
            .header(
                Row::new(kept.iter().enumerate().map(|(nth, (_, header))| {
                    // The header moves over the gutter with the cells, or `NAME` sits two columns
                    // left of the names under it. A table with no gutter pads by nothing, which
                    // is the same string and needs no branch of its own.
                    let pad = if nth == 0 { gutter } else { 0 };
                    format!("{blank:pad$}{header}", blank = "")
                }))
                .style(screen.fg(theme::DIM)),
            )
            .column_spacing(GAP as u16)
            .highlight_symbol(MARKER)
            // The same pairing the sidebar uses: the fill and the mark together, because `PANEL`
            // degrades to nothing at sixteen colours and `▸` is what carries the selection there.
            .row_highlight_style(Style::new().bg(ink(theme::PANEL, screen.depth))),
        area,
        &mut state,
    );

    if let Some((line, (row, card))) = under {
        let cell = kept
            .first()
            .and_then(|(at, _)| row.cells.get(*at))
            .map_or("", String::as_str);
        problems(frame, line, screen, card, &identify(row, cell, scoped));
    }
}

/// **The first cell as it is drawn** — `namespace/name` where the view carries no namespace scope,
/// and the bare name everywhere else.
///
/// **The server sends no `NAMESPACE` column for an unscoped list**, and `kubectl -A` prepends one
/// client-side (`screens/resources.md` § Browsing every namespace). Without it, fourteen
/// `kube-root-ca.crt` rows are fourteen identical strings with a cursor resting on one of them,
/// which satisfies invariant 2's *explicitly selected object* in the letter and defeats it in the
/// intent. A scoped view already names its one namespace in the title, and a row that carries
/// none never grows one.
///
/// One function because [`problems`] names the selected row and has to name it the way the table
/// drew it — `web` under `ns: payments`, `payments/web` with no scope in effect.
fn identify<'a>(row: &'a crate::k8s::Row, cell: &'a str, scoped: bool) -> Cow<'a, str> {
    match &row.namespace {
        Some(namespace) if !scoped => Cow::Owned(format!("{namespace}/{cell}")),
        _ => Cow::Borrowed(cell),
    }
}

/// **The card this row's object is the owner of, or nothing at all** — the whole of *Alerts bleed
/// through* (`screens/resources.md` § Rules, NOTES § D251).
///
/// **The uid and never the name**: a name deleted and recreated is a different object
/// ([`crate::k8s::Row::uid`]), and this is the same anchor the cursor already follows two lines
/// down. Two rows carrying no uid are not the same object, which is what the `?` says — rule C1's
/// kubeconfig certificate is a card whose owner has none, and `?includeObject=None` is a whole
/// table of rows that do not.
///
/// **A card is filed under an *owner*** (NOTES § D3), so what this marks is the object Alerts has
/// a card *about* — the Deployment, the node, the bare pod. A pod that has a finding but is owned
/// by a Deployment is not one of those: Alerts draws no card for it, and marking its row would
/// promise a card that `⏎` could not open.
///
/// **Its ceiling, named rather than left to be found: a linear scan per row**, so the frame pays
/// O(rows × cards). Both terms are bounded by the same claim [`crate::views::cards`] makes about
/// its own O(n²) — the card list is short, which is the whole of D3 — and if a cluster ever makes
/// it long the key to build a map on is the uid this already reads.
fn about<'a>(cards: &'a [Card], row: &crate::k8s::Row) -> Option<&'a Card> {
    let uid = row.uid.as_deref()?;
    cards
        .iter()
        .find(|card| card.owner.uid.as_deref() == Some(uid))
}

/// **`● web has 3 pods with problems — ⏎ to see`** — one line under the table, about the selected
/// row (`screens/resources.md` § The line under the table, NOTES § D251).
///
/// **The count is [`crate::views::Card::affected`], the same objects `● web · 3 of 5 pods` counts
/// one screen over**, and whether a pod count is a fact at all is asked of
/// [`crate::views::Card::count`] rather than re-derived here. Its `None` has **two** causes and
/// they are not one condition: a node card counts no pods at all, `affected == 0`
/// (NOTES § D39), and a bare pod's card is `owner.kind == Pod`, because nothing owns it
/// (NOTES § D246 ruling 2). A line reading `0 pods`, or one counting a row against itself, is
/// what re-deriving either here would eventually print.
///
/// **Dim, with the glyph in the card's own band** — the shape [`lines`]' fifth part already has:
/// a pointer to where the detail is, not an instruction. **The name is what gives way** when the
/// sentence does not fit, never `⏎ to see`, which is the one half that says what to do next —
/// **and the cut carries [`CUT`]**. The whole name being one `⏎` away is what makes cutting it
/// legitimate; it is not what makes it silent, and a bare `kube-system/cored` reads as an object
/// that exists (`screens/widgets.md` § 7, the second of the two places this product cuts on
/// purpose).
fn problems(frame: &mut Frame, area: Rect, screen: &Screen, card: &Card, name: &str) {
    let tail = match card.count() {
        Some(_) => format!(" has {} pods with problems — ⏎ to see", card.affected),
        None => " has problems — ⏎ to see".to_owned(),
    };
    let (colour, signal) = theme::band(card.severity());
    let mut spans = vec![
        // The selection marker's own columns, so this `●` lands in the column the row's `●` is in.
        Span::raw(" ".repeat(width(MARKER))),
        Span::styled(glyph(signal), screen.fg(colour)),
    ];
    // **Measured off the spans that will be drawn, never restated as a sum of the same two
    // constants** — the indent is what [`spanned`] says it is, so it cannot drift from the two
    // lines above it.
    let room = usize::from(area.width).saturating_sub(spanned(&spans) + width(&tail));
    // `screens/resources.md` § The line under the table, rules 2–4, which is [`clipped`]'s whole
    // contract: whole and unmarked when it fits; else the mark glued to the last character kept,
    // with no space before it; and nothing at all where there is no room to mark a cut in.
    let shown = clipped(name, room);
    spans.push(Span::styled(
        format!("{shown}{tail}"),
        screen.fg(theme::DIM),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// --- THE BROWSER END ---

// --- THE ANALYSIS PANE START ---

/// **One report, and one code path for all seven** (`screens/analysis.md` § *How a report is
/// drawn*, invariant 12's spirit one file over). Nothing below names a report, reads its label or
/// branches on one: a pane is a title and a list of [`ReportRow`]s, and a report with nothing to
/// say says so in its own words as one [`ReportRow::Prose`] — which is that section's rule 8 and
/// the reason no per-report sentence lives here.
///
/// **The title is not a row, so it does not scroll** (rule 5): it is laid out above the list, the
/// way the browser's own heading is, and whatever the reader has scrolled to, the sentence that
/// says what they are looking at is still on the first line. It clips at the pane edge like every
/// other one-line fact k8rs draws (`screens/widgets.md` § 7); the rows below it never do (rule 4).
///
/// **The cursor lands on [`ReportRow::Answer`] and nothing else** (NOTES § D127) — through
/// [`views::selectable`] and [`views::answers`], which is the sidebar's own mechanism for its
/// section headers, so the two lists skip their unselectable rows the same way. No row carries a
/// selection marker, exactly as no card does: the two-column band gutter is the glyph's, and a
/// `▸` in it would be a fourth thing that column can mean.
fn analysis(frame: &mut Frame, area: Rect, app: &App, screen: &Screen, report: &Report) {
    let [head, _, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(Line::styled(report.title.as_str(), screen.fg(theme::TEXT))),
        padded(head),
    );
    let body = padded(body);
    let region = usize::from(body.width);
    let height = usize::from(body.height);
    let items: Vec<ListItem> = report
        .rows
        .iter()
        .enumerate()
        .map(|(nth, row)| {
            let above = nth
                .checked_sub(1)
                .and_then(|before| report.rows.get(before));
            ListItem::new(Text::from(drawn(row, above, screen, region, height)))
        })
        .collect();
    let picks = views::selectable(&report.rows, views::answers);
    // One anchor slot per selectable row and none of them read, the same as the sidebar's: which
    // row the cursor is on is the state, and following an object is the key handler's business.
    let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
    let at = app
        .content
        .selected(&anchors)
        .and_then(|nth| picks.get(nth))
        .copied();
    let mut state = ListState::default().with_selected(at);
    frame.render_stateful_widget(List::new(items), body, &mut state);
}

/// **One row, as the lines it draws** — and the whole of the grammar
/// (`screens/analysis.md` § *How a report is drawn*), at the widths that section's table gives at
/// the 80×24 floor: 51 columns of row text after the gutter, 49 for a `detail` and for the `→ `
/// action, 47 for an action continuation. Every one of them is the region less a constant, so a
/// wider terminal widens all four together.
///
/// **The gutter is [`GUTTER`] columns whether or not there is a glyph** (rule 2), which is why a
/// row with no band draws spaces there rather than nothing: a banded row and a plain one start at
/// the same column, and the eye reads a straight left edge of names. The glyph and its colour come
/// from `theme::band` and from nowhere else (rule 1) — no string on this page contains one, and a
/// wrapped row's continuation indents under its own text so it can never be read as a second,
/// unbanded row.
///
/// **A row wraps and never clips** (rule 4). This is the one place the page differs from an Alerts
/// card, where an age is right-aligned and the name gives way: there are two zones there and one
/// here.
///
/// **The blank lines are the block structure and nothing else.** A [`ReportRow::Prose`] or a
/// [`ReportRow::NotComputed`] is separated from the answers above it by one blank line — never at
/// the top of the body, and never after a `NotComputed`, which already closed with one. Answers
/// pack, so a list of nodes reads as a list. `above` is the row before this one, which is all that
/// is needed to know which of those it is.
///
/// **An answer is never taller than `height`, the rows the pane has** (`screens/analysis.md` § A
/// row taller than the pane, NOTES § D266): ratatui skips a `List` item that does not fit, so a
/// drain row of four problem paragraphs blanked the whole body on the committed captures. The
/// identity and the action are never cut; the paragraphs between them are [`marked`] to what they
/// leave — [`cut`]'s own rule, over lines wrapped a paragraph at a time. Where the identity and the
/// action alone outgrow the pane the row is cut down to it, [`alerts`]' rule, so the cursor's row
/// is on screen whatever it holds.
fn drawn<'a>(
    row: &ReportRow,
    above: Option<&ReportRow>,
    screen: &Screen,
    region: usize,
    height: usize,
) -> Vec<Line<'a>> {
    let ink = screen.fg(theme::TEXT);
    let dim = screen.fg(theme::DIM);
    // **The grammar's own table, each measure the one above it less [`PAD`]**: 51 columns of row
    // text after the gutter, 49 for a `detail` and for the `→ ` action, 47 for an action
    // continuation, at the 80×24 floor (`screens/analysis.md` § *How a report is drawn*). Named
    // and subtracted in one place, so the four cannot drift apart and a wider terminal widens
    // all of them.
    let step = usize::from(PAD);
    let says = region.saturating_sub(GUTTER);
    let under = says.saturating_sub(step);
    let after = under.saturating_sub(step);
    let space = match above {
        None => false,
        Some(ReportRow::NotComputed { .. }) => false,
        Some(_) => true,
    };
    let mut lines: Vec<Line> = Vec::new();
    match row {
        ReportRow::Answer {
            severity,
            text,
            detail,
            action,
            jump: _,
        } => {
            let band = match severity {
                Some(severity) => {
                    let (colour, signal) = theme::band(*severity);
                    Span::styled(glyph(signal), screen.fg(colour))
                }
                // **A row that makes no judgement is still a row** (`crate::analysis::Row::Answer`
                // — `None` is not a fourth band), so it pays the gutter and draws nothing in it.
                None => Span::raw(" ".repeat(GUTTER)),
            };
            let mut text = wrapped(text, says).into_iter();
            lines.push(Line::from(vec![
                band,
                Span::styled(text.next().unwrap_or_default(), ink),
            ]));
            lines.extend(indent(text.collect(), "  ", ink));
            // **One element per paragraph, and no blank line between them** — two adjacent
            // paragraphs are what Capacity's flagged node draws (the measurement, then what the
            // numbers mean) and what a drain row folds a second reason into, and both are drawn
            // solid (`screens/analysis.md` §§ Capacity, A node that would throw away files).
            let detail: Vec<String> = detail
                .iter()
                .flat_map(|paragraph| wrapped(paragraph, under))
                .collect();
            // **The action is never cut and its continuation sits under the text after the
            // arrow**, the same as a card's — `views.rs` draws the `→ `, so the value starts at
            // the word (`crate::analysis::Row::Answer::action`).
            let mut action = wrapped(action, after).into_iter();
            let room = height.saturating_sub(lines.len() + action.len());
            lines.extend(indent(marked(detail, under, room), "    ", ink));
            if let Some(first) = action.next() {
                lines.push(Line::from(vec![
                    Span::styled("    → ", screen.fg(theme::ACCENT)),
                    Span::styled(first, ink),
                ]));
                lines.extend(indent(action.collect(), "      ", ink));
            }
            lines.truncate(height.max(1));
        }
        // **No gutter and no band**: a line the cursor cannot reach cannot be acted on, so it
        // starts at the region's own left edge and is dim, like every other line on this screen
        // that is context rather than an answer — the sidebar's section headers, [`note`]'s
        // paragraphs, [`empty`]'s sentence.
        ReportRow::Prose(said) => {
            if space {
                lines.push(Line::default());
            }
            lines.extend(set(said, region, dim));
        }
        // **The reason, then the way out, and both are the pane's answer** — so they are drawn in
        // the same ink as an answer and not dimmed away. A report that names the check without
        // naming the way out is the half a reader cannot act on
        // (`crate::analysis::Row::NotComputed::ask_for`), and the blank between them is what keeps
        // the two sentences from reading as one paragraph.
        ReportRow::NotComputed { reason, ask_for } => {
            if space {
                lines.push(Line::default());
            }
            lines.extend(set(reason, region, ink));
            lines.push(Line::default());
            lines.extend(set(ask_for, region, ink));
            lines.push(Line::default());
        }
    }
    lines
}

// --- THE ANALYSIS PANE END ---

// --- THE DETAIL TABS START ---

/// The columns between two labels in the tab row (`screens/detail.md`, measured off the describe,
/// yaml and events mockups — the marked tab widens, so the labels after it move).
const TAB_GAP: usize = 3;

/// The marks around the open tab. **Text and not only a colour**, which is `theme.rs`'s own rule
/// that colour is never the only carrier of a state.
const MARKED: (&str, &str) = ("‹ ", " ›");

/// Between the widest container name in describe's block and the word after it — three columns,
/// which is what that mockup draws (`sidecar-envoy` at 13, `failed` at 16) and what the headless
/// surface already pads to.
const NAMES_GAP: usize = 3;

/// **The object's name, the tab row, its underline, and the open tab's pane**
/// (`screens/detail.md`).
///
/// **The first three rows are pinned and only the body scrolls**, which is what that file's
/// scrolled mockups draw: *"The object's name, the tab row and its underline stay pinned — drawn
/// above the scrolling `Paragraph`, not inside it."* A reader who has scrolled has not lost which
/// object they are looking at or which tab they are on.
fn detail(frame: &mut Frame, area: Rect, app: &App, screen: &Screen, open: &Detail) {
    let [head, row, under, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    // **Cut by [`name_cut`] and not clipped by the `Paragraph`**, which drew two pods of one
    // rollout as one heading with no mark (`screens/detail.md` § The heading, when the name does
    // not fit).
    let head = padded(head);
    frame.render_widget(
        Paragraph::new(Line::styled(
            name_cut(
                open.object.namespace.as_deref(),
                &open.object.name,
                usize::from(head.width),
            ),
            screen.fg(theme::TEXT),
        )),
        head,
    );
    tabs(frame, row, under, app, screen);
    match app.tab {
        Tab::Logs => logs(frame, body, app, screen, open.logs),
        Tab::Describe => describe(frame, body, app, screen, open),
        Tab::Yaml => yaml(frame, body, app, screen, open),
        Tab::Events => events(frame, body, app, screen, open.events),
    }
}

/// **The tab row and the underline under the open one**, drawn together because they are one
/// layout: the underline's column is the sum of the labels before it, and two functions computing
/// that sum is two answers to one question.
///
/// **The widget is `Tabs`, which is what `screens/widgets.md` § 2 names** — with no padding and a
/// [`TAB_GAP`] divider, so the row is exactly `logs   describe   yaml   ‹ events ›`.
///
/// **The underline is `label + 2` columns wide and starts at the `‹`**, which is the three
/// mockups' own arithmetic read off them rather than estimated (describe: 10 columns at 9; yaml:
/// 6 at 20; events: 8 at 27). **The logs mockup disagrees with all three** — 7 columns, and one
/// space after `›` where the others draw three — and is drawn from the rule the other three share
/// rather than special-cased; `tui-designer` owes that mockup a correction.
fn tabs(frame: &mut Frame, row: Rect, under: Rect, app: &App, screen: &Screen) {
    let titles: Vec<String> = Tab::ALL
        .iter()
        .map(|tab| match *tab == app.tab {
            true => format!("{}{}{}", MARKED.0, tab.label(), MARKED.1),
            false => tab.label().to_owned(),
        })
        .collect();
    let at = app.tab.at();
    frame.render_widget(
        Tabs::new(titles.clone())
            .select(at)
            .divider(" ".repeat(TAB_GAP))
            .padding("", "")
            .style(screen.fg(theme::DIM))
            .highlight_style(screen.fg(theme::ACCENT)),
        padded(row),
    );
    // **Measured off the same strings the row was built from**, so a longer label cannot move the
    // row and leave the underline behind.
    let start: usize = titles
        .iter()
        .take(at)
        .map(|title| width(title) + TAB_GAP)
        .sum();
    let rule = "─".repeat(
        titles
            .get(at)
            .map_or(0, |title| width(title))
            .saturating_sub(2),
    );
    frame.render_widget(
        Paragraph::new(Line::styled(
            format!("{}{rule}", " ".repeat(start)),
            screen.fg(theme::ACCENT),
        )),
        padded(under),
    );
}

/// **A detail pane's body — the one scrolling widget all four tabs draw**
/// (`screens/widgets.md` § 4: no cap, no *N more* line, a `Paragraph` with an offset).
///
/// **The offset is clamped here and not in `views.rs`**, because how many rows a pane has depends
/// on the width it is drawn at and that file names no widget ([`crate::views::App::scroll`]).
///
/// **Every line handed here is already one row**, wrapped by [`wrapped`] on the way in and never
/// by ratatui's own `Wrap`. Two wrapping algorithms in one pane is two answers to *how tall is
/// this*, and the offset is computed from the taller-or-shorter of them — so a followed stream
/// pins to a bottom that is not the bottom. One algorithm, and the count is `len`.
fn scrolled(frame: &mut Frame, area: Rect, offset: u16, follow: bool, lines: Vec<Line>) {
    let last = lines.len().saturating_sub(usize::from(area.height));
    let at = if follow {
        last
    } else {
        usize::from(offset).min(last)
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines)).scroll((u16::try_from(at).unwrap_or(u16::MAX), 0)),
        area,
    );
}

/// **One line of a document or a stream, wrapped without losing its own indentation.**
///
/// [`wrapped`] drops the whitespace a line *starts* with, which is right for a card's sentence and
/// wrong here: leading whitespace is meaningful in the two panes that draw text nobody at k8rs
/// composed — a YAML document's structure, and the indent on a stack frame in a log line
/// (`screens/detail.md` § The yaml tab, § A line longer than the cap; `screens/widgets.md` § 2's
/// rule that `yaml` and `logs` do not wrap-trim). The indent goes back on the first row; a
/// continuation starts at the pane's own edge, which is what wrapping to a pane means.
fn kept<'a>(text: &str, columns: usize, style: Style) -> Vec<Line<'a>> {
    let lead: String = text.chars().take_while(|c| c.is_whitespace()).collect();
    let body = &text[lead.len()..];
    if body.is_empty() {
        return vec![Line::styled(text.to_owned(), style)];
    }
    let mut rows = wrapped(body, columns.saturating_sub(width(&lead)));
    if let Some(first) = rows.first_mut() {
        first.insert_str(0, &lead);
    }
    rows.into_iter()
        .map(|line| Line::styled(line, style))
        .collect()
}

/// **A block whose first row is indented by `step` and whose continuations are indented by
/// `under`** — describe's container rows (`screens/detail.md` § The describe tab).
///
/// **The two indents are separate because that file draws them differently, for a reason.** A
/// **name** row goes deeper on the wrap — `sidecar-envoy   keeps crashing and` /
/// `      restarting, 12 restarts` — because a continuation starting in the name's own column
/// would read as the next container. A **detail** row does not — `container exceeded its memory
/// limit —` / `exit 137, 4 restarts` — because it is already visibly subordinate to the name
/// above it, and going deeper again would suggest a third level that does not exist.
fn hanging<'a>(
    text: &str,
    region: usize,
    step: usize,
    under: usize,
    style: Style,
) -> Vec<Line<'a>> {
    let mut rows = wrapped(text, region.saturating_sub(step)).into_iter();
    let Some(first) = rows.next() else {
        return Vec::new();
    };
    let mut lines = vec![Line::styled(format!("{}{first}", " ".repeat(step)), style)];
    lines.extend(
        wrapped(
            &rows.collect::<Vec<_>>().join(" "),
            region.saturating_sub(under),
        )
        .into_iter()
        .map(|line| Line::styled(format!("{}{line}", " ".repeat(under)), style)),
    );
    lines
}

/// Wrapped lines in one style — the shape every free-text block in this file draws.
fn set<'a>(text: &str, columns: usize, style: Style) -> Vec<Line<'a>> {
    wrapped(text, columns)
        .into_iter()
        .map(|line| Line::styled(line, style))
        .collect()
}

/// **The logs tab** (`screens/detail.md` § The logs tab), in the three answers a fetch has
/// (PRIOR-ART § C2).
///
/// **The match is written out here and in each of the three panes below rather than shared**,
/// which is [`content`]'s own rule and not a missed extraction: a helper that hands a refusal's
/// partial answer to the same arm that draws the ready one lets *we were not allowed to look*
/// become *there is nothing*. Measured on 2026-09-06 — one was written, and the refused events
/// pane drew `○  none right now` under its own banner.
fn logs(frame: &mut Frame, area: Rect, app: &App, screen: &Screen, pane: &Pane<Logs>) {
    match pane {
        Pane::Loading => note(frame, area, screen, false, None),
        Pane::Denied(said, held) => {
            let rest = banner(frame, area, screen, said, floor(area));
            stream(frame, rest, app, screen, held);
        }
        Pane::Ready(held) => stream(frame, area, app, screen, held),
    }
}

/// **The header line, whatever k8rs has to admit about the buffer, and the stream under both.**
///
/// **The header and the two admissions are pinned above the scrolling half**, which is what
/// § When the buffer fills draws: the dropped-lines line *"replaces the blank row above the
/// content"*, and it says how many lines are gone from the top of what is left — a sentence that
/// scrolled away with the content would be pointing at nothing.
fn stream(frame: &mut Frame, area: Rect, app: &App, screen: &Screen, logs: &Logs) {
    let area = padded(area);
    let region = usize::from(area.width);
    let text = screen.fg(theme::TEXT);
    let dim = screen.fg(theme::DIM);

    // **`previous log: on ` is padded to the width of `off`** so both start in the same column,
    // which is what both mockups draw (column 28 of a 47-column pane, measured).
    let toggle = format!(
        "previous log: {:<3}",
        if logs.previous { "on" } else { "off" }
    );
    // **`▾` only where there is something to pick** — a single-container pod is not offered a
    // picker, and a key that does nothing is a bug this product has already shipped once.
    let picker = if logs.pod.containers.len() > 1 {
        " ▾"
    } else {
        ""
    };
    let named = format!("container: {}{picker}", logs.container);
    let mut top = vec![Line::from(vec![
        Span::styled(named.clone(), text),
        Span::raw(" ".repeat(region.saturating_sub(width(&named) + width(&toggle)))),
        Span::styled(toggle, dim),
    ])];
    // **`⇧p` with no previous run to show**: k8rs does not print the API's refusal and does not
    // leave the toggle pointed at nothing (`crate::views::no_previous_run`, whose sentence the
    // headless surface prints too — the prefix is all that differs).
    let restarts = logs
        .pod
        .containers
        .iter()
        .find(|(name, _)| *name == logs.container)
        .and_then(|(_, status)| *status)
        .map_or(0, |status| status.restarts);
    if let Some(said) = views::no_previous_run(logs.container, restarts, logs.previous) {
        let mut wrapped = wrapped(&said, region.saturating_sub(width("⇧p — "))).into_iter();
        top.push(Line::from(vec![
            Span::styled("⇧p — ", screen.fg(theme::ACCENT)),
            Span::styled(wrapped.next().unwrap_or_default(), text),
        ]));
        top.extend(indent(wrapped.collect(), "     ", text));
    }
    top.push(Line::default());
    if let Some(said) = logs.held.dropped_line() {
        top.extend(set(&said, region, dim));
        top.push(Line::default());
    }

    let height = u16::try_from(top.len())
        .unwrap_or(u16::MAX)
        .min(area.height);
    let [pinned, body] =
        Layout::vertical([Constraint::Length(height), Constraint::Min(0)]).areas(area);
    frame.render_widget(Paragraph::new(Text::from(top)), pinned);

    // **Nothing has arrived is a state, not a hang** (PRIOR-ART § E1) — and it is not the same
    // screen as a stream that ended.
    if !logs.held.arrived() {
        calmly(
            frame,
            body,
            screen,
            "no logs yet",
            "Nothing has been written to this container's log yet.",
        );
        return;
    }
    let lines: Vec<Line> = logs
        .held
        .lines()
        .flat_map(|line| kept(line, region, text))
        .collect();
    scrolled(frame, body, app.scroll, app.following, lines);
}

/// **The describe tab: the object, then what happened to it** — two reads, one pane
/// (`screens/detail.md` § The describe tab).
fn describe(frame: &mut Frame, area: Rect, app: &App, screen: &Screen, open: &Detail) {
    let (area, read) = match open.read {
        Pane::Loading => return note(frame, area, screen, false, None),
        Pane::Denied(said, read) => (banner(frame, area, screen, said, floor(area)), read),
        Pane::Ready(read) => (area, read),
    };
    let area = padded(area);
    let region = usize::from(area.width);
    let text = screen.fg(theme::TEXT);
    let dim = screen.fg(theme::DIM);
    let mut lines: Vec<Line> = Vec::new();
    // **The identity block: `Pod · running · created 3 days ago`, and the pod's own reason
    // under it** — the raw word and the message in the same shape an event's row uses, which
    // is that file's point: one layout for *a word that explains a state*.
    //
    // **The `(RawReason) message` line is dim and the lines above it are not** — it is the
    // evidence, the role a card's quoted line already has. **Dimmed for being *last* and not for
    // being third**: a `status.reason` no table names produces two lines rather than three, and
    // keying on the count would draw the evidence in the identity line's own ink.
    let said = views::identity(read.snapshot, screen.now);
    let last = said.len().saturating_sub(1);
    for (nth, line) in said.into_iter().enumerate() {
        let ink = if nth == last && last > 0 { dim } else { text };
        lines.extend(set(&line, region, ink));
    }
    if !read.containers.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled("containers", dim));
        let column = read
            .containers
            .iter()
            .map(|(name, _)| width(name))
            .max()
            .unwrap_or(0)
            + NAMES_GAP;
        for (name, status) in read.containers {
            let status = *status;
            let (word, detail) = views::container_state(status.map(|held| &held.state));
            // **The restart count goes on the last line of the row**, which is the detail line
            // where there is one and the state word where there is not.
            let counted = views::restarts(status);
            let step = usize::from(PAD);
            let (first, under) = match &detail {
                None => (format!("{name:<column$}{word}{counted}"), None),
                Some(said) => (
                    format!("{name:<column$}{word}"),
                    Some(format!("{said}{counted}")),
                ),
            };
            // **A wrapped row continues under its own text and never under its name**, which
            // is `screens/detail.md`'s own `sidecar-envoy   keeps crashing and` /
            // `      restarting, 12 restarts` — a continuation at the name's column would read
            // as a second container.
            // **The name row's wrap goes one pad deeper and the detail row's stays flat**, which
            // is what that section draws; [`hanging`] carries why.
            lines.extend(hanging(&first, region, step, step * 2, text));
            if let Some(said) = under {
                lines.extend(hanging(&said, region, step * 2, step * 2, dim));
            }
        }
    }
    lines.push(Line::default());
    lines.extend(block(open.events, screen, region));
    scrolled(frame, area, app.scroll, false, lines);
}

/// **Describe's own events block: the heading, then the three answers under it**
/// (`screens/detail.md` § No events at all).
///
/// **It is the events *tab*'s list under a heading, and the heading is where the claim is**
/// ([`crate::views::events_heading`]) — so a cut read withdraws *newest first* here exactly as it
/// does one tab over, from the same function.
///
/// **Left-flush and not centred**, which is the one thing this block does differently from the
/// tab: it is a section inside a longer pane rather than the whole of one, so *`○ none right
/// now`* sits under the heading rather than in the middle of the screen.
fn block<'a>(events: &Pane<crate::k8s::Happened>, screen: &Screen, region: usize) -> Vec<Line<'a>> {
    let dim = screen.fg(theme::DIM);
    let heading = match events {
        Pane::Ready(happened) => views::events_heading(happened),
        // A read that has not answered and one that was refused both have a block to head;
        // neither may claim an order it has not seen.
        _ => "events".to_owned(),
    };
    // **The heading wraps like every other free-text block in this function.** A cut read's
    // heading is 84 columns and the pane is 55, and a hard cut takes off exactly the withdrawal —
    // the clause the heading exists to say (`k8s-admin`, 2026-09-07).
    let mut lines = set(&heading, region, dim);
    match events {
        Pane::Loading => lines.push(Line::styled("reading the cluster…", dim)),
        Pane::Denied(said, _) => lines.extend(set(said, region, screen.fg(theme::TEXT))),
        // **`crate::views::no_events` decides the emptiness, not this call site** — the driver
        // asks the same function, and a `lines.is_empty()` spelled at each of them is two places
        // that can come to differ about what an empty read means.
        Pane::Ready(happened) => match views::no_events(happened) {
            Some(said) => {
                lines.push(calm(screen, "none right now"));
                lines.push(Line::default());
                lines.extend(set(said, region, dim));
            }
            None => lines.extend(rows(happened, screen, region)),
        },
    }
    lines
}

/// **The events tab** (`screens/detail.md` § The events tab) — the same list describe reads,
/// filling the pane instead of a block in it.
fn events(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    screen: &Screen,
    pane: &Pane<crate::k8s::Happened>,
) {
    let happened = match pane {
        Pane::Loading => return note(frame, area, screen, false, None),
        // **A refusal draws whatever did come back and never the empty sentence below.** A read
        // that was refused and answered with nothing draws the banner and an empty pane, which is
        // the browser's own rule one region up.
        Pane::Denied(said, happened) => {
            let rest = banner(frame, area, screen, said, floor(area));
            return rows_into(frame, rest, app, screen, happened);
        }
        Pane::Ready(happened) => happened,
    };
    // **Empty is centred here and left-flush in describe**, because with nothing else sharing the
    // pane this is a whole-screen calm state like *nothing is broken*.
    if let Some(said) = views::no_events(happened) {
        calmly(frame, area, screen, "none right now", said);
        return;
    }
    rows_into(frame, area, app, screen, happened);
}

/// The events list itself, under whatever was drawn above it — and, on a cut read, under the
/// heading this function pins there ([`crate::views::events_heading`]).
fn rows_into(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    screen: &Screen,
    happened: &crate::k8s::Happened,
) {
    let area = padded(area);
    let region = usize::from(area.width);
    // **No heading in the ordinary case — the tab label already says what the pane is** — and
    // exactly one case brings it back: a read the server cut, which is the one state whose
    // heading exists to *withdraw* the newest-first promise the absence of a heading would
    // otherwise imply.
    //
    // **It is pinned above the scrolling half**, for [`stream`]'s reason on the sentence one
    // region up: it is a claim about the whole read rather than a row of it, and a sentence that
    // scrolled away with the content would be pointing at nothing. It scrolled off at offset 3
    // (`k8s-admin`, 2026-09-07). **No row is reserved when there is no heading** — `top` is empty,
    // so the pinned half is zero rows tall and the list starts where it always did.
    //
    // **The heading's own rows pin and the blank row under it does not**, which is what that
    // section's two mockups draw between them: the unscrolled one separates the heading from the
    // first event, the scrolled one has the heading directly above a row, and the prose counts
    // *"the ten rows left once the heading takes its own two"*. The blank is the list's first
    // row, so it goes with the list.
    let mut top: Vec<Line> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    if happened.cut {
        top.extend(set(
            &format!("{}:", views::events_heading(happened)),
            region,
            screen.fg(theme::DIM),
        ));
        lines.push(Line::default());
    }
    lines.extend(rows(happened, screen, region));
    let height = u16::try_from(top.len())
        .unwrap_or(u16::MAX)
        .min(area.height);
    let [pinned, body] =
        Layout::vertical([Constraint::Length(height), Constraint::Min(0)]).areas(area);
    frame.render_widget(Paragraph::new(Text::from(top)), pinned);
    scrolled(frame, body, app.scroll, false, lines);
}

/// **One event's rows, and the whole of the grammar** (`screens/detail.md` § The events tab):
/// the age and the plain-language phrase, the raw word beside the controller's verbatim message
/// under it, and the *happened N times* line where the count is more than one.
///
/// **The age column pads to the widest age actually on the pane plus [`GAP`]**, so the phrases
/// line up — and a
/// row with neither an age nor a phrase **drops its first line rather than drawing a row of blank
/// padding**, which is `main.rs`'s own rule for the same list reached one function away.
///
/// **Every string here comes from `crate::views`**, not from a second table: `plainly` is
/// `k8s.rs`'s, `raw_and_message` and `repeated` are `views.rs`', and a reason no table names falls
/// through to its own raw word beside its message with nothing invented (NOTES § D198, § D254).
fn rows<'a>(happened: &crate::k8s::Happened, screen: &Screen, region: usize) -> Vec<Line<'a>> {
    let text = screen.fg(theme::TEXT);
    let dim = screen.fg(theme::DIM);
    let ages: Vec<String> = happened
        .lines
        .iter()
        .map(|line| {
            line.at
                .as_ref()
                .and_then(|at| age(screen.now, at))
                .unwrap_or_default()
        })
        .collect();
    let column = ages.iter().map(|at| width(at)).max().unwrap_or(0) + GAP;
    let mut lines: Vec<Line> = Vec::new();
    for (happening, at) in happened.lines.iter().zip(&ages) {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        // **The phrase is wrapped after the age column and its continuations return to the pane's
        // own left edge**, which is what the scrolled mockup draws — no hanging indent.
        let mut said = wrapped(
            happening.plainly().unwrap_or_default(),
            region.saturating_sub(column),
        )
        .into_iter();
        let first = said.next().unwrap_or_default();
        if !(at.is_empty() && first.is_empty()) {
            lines.push(Line::from(vec![
                Span::styled(format!("{at:<column$}"), dim),
                Span::styled(first, text),
            ]));
            lines.extend(set(&said.collect::<Vec<_>>().join(" "), region, text));
        }
        lines.extend(set(
            &views::raw_and_message(&happening.reason, Some(&happening.message)),
            region,
            dim,
        ));
        if let Some(said) = views::repeated(happening, screen.now) {
            lines.extend(set(&said, region, dim));
        }
    }
    lines
}

/// **The yaml tab: the object exactly as the API returned it** (`screens/detail.md`
/// § The yaml tab).
///
/// **No two-column reading margin, unlike every other tab on this screen.** The pane's own left
/// edge *is* the document's, so the YAML's indentation is the only indentation drawn — a margin
/// on top of it would misrepresent the structure this pane exists to get right.
///
/// **Nothing here strips, and here that is load-bearing rather than merely true** (NOTES § D198):
/// `k8s::clean` already removed everything `unprintable` refuses **except** `\n` and `\t`, which
/// on this one pane print as themselves. A second strip here would collapse a ConfigMap's
/// 20-line `Corefile` onto one line and call it the object.
fn yaml(frame: &mut Frame, area: Rect, app: &App, screen: &Screen, open: &Detail) {
    let (area, document) = match open.yaml {
        Pane::Loading => return note(frame, area, screen, false, None),
        Pane::Denied(said, document) => (banner(frame, area, screen, said, floor(area)), document),
        Pane::Ready(document) => (area, document),
    };
    let region = usize::from(area.width);
    let mut lines: Vec<Line> = document
        .lines()
        .flat_map(|line| kept(line, region, screen.fg(theme::TEXT)))
        .collect();
    // **`data: {}` is drawn exactly as the API returned it — there is nothing to mask because
    // there is nothing there** — and the sentence under it says so in a reader's words rather
    // than leaving an empty map to be interpreted.
    if open.secret_without_keys {
        lines.push(Line::default());
        lines.extend(indent(
            wrapped(
                "This Secret holds no keys yet.",
                region.saturating_sub(usize::from(PAD)),
            ),
            "  ",
            screen.fg(theme::DIM),
        ));
    }
    scrolled(frame, area, app.scroll, false, lines);
}

// --- THE DETAIL TABS END ---

// --- MEASURING AND CUTTING START ---

/// How many columns a string occupies, measured the way ratatui measures it — wide CJK included.
/// `str::len` is bytes and `chars().count()` is not width; both draw a misaligned age column.
fn width(text: &str) -> usize {
    Span::raw(text).width()
}

/// The prefix of `text` that fills `columns` without passing them, **cut between characters and
/// never inside one** — it stops at the first character that would take the prefix over.
/// `String::truncate` slices bytes and panics in the middle of a multi-byte name
/// (`screens/widgets.md` § 7).
///
/// **The prefix is measured, never summed, and that is the whole of this function.** [`width`]
/// measures a grapheme cluster whole and a cluster is not the sum of its characters: `☀️` is base
/// plus variation selector, **one** column summed and **two** measured, and a ZWJ family emoji is
/// six summed and two measured. Adding a per-character step therefore handed back a prefix wider
/// than the columns asked for — 57 into a 51-column card region, measured — which is the silent
/// cut `screens/widgets.md` § 7 forbids by name (2026-09-06).
fn fits(text: &str, columns: usize) -> &str {
    let mut end = 0;
    for (at, character) in text.char_indices() {
        let next = at + character.len_utf8();
        if width(&text[..next]) > columns {
            break;
        }
        end = next;
    }
    &text[..end]
}

/// `text` at the width it has, **with [`CUT`] where it had to give way** — the head is kept and
/// the tail is what goes, which is the opposite end from [`shortened`] and for the opposite
/// reason: a command reads left to right and its first words are the ones that say what it is.
///
/// **Three answers and the third is the one that is easy to miss.** Whole and unmarked when it
/// fits; the head plus the mark when it does not; and **nothing at all** where the row is too
/// narrow to draw even the mark, because a cut with no mark on it is the silent truncation this
/// exists to stop (`screens/widgets.md` § 7, `screens/resources.md` § The line under the table).
///
/// **The mark's own column comes off the budget before the head is measured**, never added to the
/// head's width afterwards — [`shortened`]'s comment is about the same sum, and it is the one
/// this file has already been caught by once.
fn clipped(text: &str, columns: usize) -> Cow<'_, str> {
    tailed(text, columns, CUT)
}

/// [`clipped`] behind any mark — [`command_cut`]'s floor, which is the one caller whose mark is
/// not always [`CUT`] ([`STRIP_CUT`]).
fn tailed<'a>(text: &'a str, columns: usize, mark: &str) -> Cow<'a, str> {
    match fits(text, columns) {
        whole if whole == text => Cow::Borrowed(whole),
        _ if columns < width(mark) => Cow::Borrowed(""),
        _ => Cow::Owned(format!("{}{mark}", fits(text, columns - width(mark)))),
    }
}

/// **`mark` and the longest tail of `text` that fits `columns` with it** — [`shortened`]'s cut
/// from the front, always marked, and nothing at all where not one character fits beside the mark.
///
/// **Each candidate is measured whole, marker included** — the same rule [`fits`] is written to.
/// Adding the mark's width to the tail's own is the sum this file has already been caught by once,
/// and a zone one column wider than its `Rect` is clipped at the tail again, which is the whole
/// defect. It is also why the marker gets no guard of its own: a row too narrow for even the mark
/// keeps nothing, which is what an empty start already says.
fn front(text: &str, columns: usize, mark: &str) -> String {
    let mut kept = String::new();
    for (at, _) in text.char_indices().rev() {
        let candidate = format!("{mark}{}", &text[at..]);
        if width(&candidate) > columns {
            break;
        }
        kept = candidate;
    }
    kept
}

/// **A `kubectl` line at the width it has — `screens/widgets.md` § 7's back-cuts 3 and 4**, the
/// command log strip's own command and a `Confirm`'s `$` line, one rule and the mark the only thing
/// that differs ([`STRIP_CUT`] on the strip, [`CUT`] in a box).
///
/// **The object is what the line is about, so it is the one thing that never gives way whole**
/// (NOTES § D266). The head runs through the `kind/name` word, or through every word before the
/// first flag where there is none — `kubectl scale deployment/web`, `$ kubectl events --for
/// pod/web`, `$ kubectl get pod web-7d9f4` — and what gives way is what comes after it, from the
/// end, one whole word at a time: a flag one character short still reads as a flag
/// (`--show-managed-fiel`), so a flag goes whole. **A flag's own value is the one word cut inside**
/// (`-n payments-product...`), because dropping it whole leaves `-n` naming no namespace at all
/// (`screens/dialogs.md` § The command log's own line) — and so a flag that takes a value is never
/// where the line ends on its own. A value glued on with `=` is part of its flag's word.
///
/// **Where even the head and its mark do not fit, the object's name gives way from its front**,
/// behind the mark and never its `kind/` (`screens/dialogs.md` § When the object's own name does
/// not fit), with nothing after it: `kubectl scale deployment/…eckout-worker-service-canary`. The
/// character clip is the floor under that, for a head whose fixed words alone are wider than the
/// row — never reached at the 80×24 floor, and a cut wider than its row is worse than one that
/// shows less.
fn command_cut<'a>(line: &'a str, columns: usize, mark: &str) -> Cow<'a, str> {
    if width(line) <= columns {
        return Cow::Borrowed(line);
    }
    let room = columns.saturating_sub(width(mark));
    let mut words = Vec::new();
    let mut at = 0;
    for word in line.split(' ') {
        words.push((at, word));
        at += word.len() + 1;
    }
    let flag = |word: &str| word.starts_with('-');
    // A flag whose value is the next word rather than glued on with `=`.
    let valued = |word: &str| flag(word) && !word.contains('=');
    // The head runs through the `kind/name` word wherever it sits — `kubectl events` names its
    // object as `--for`'s value — and otherwise through the words before the first flag.
    let head = words
        .iter()
        .position(|(_, word)| !flag(word) && word.contains('/'))
        .map(|nth| nth + 1)
        .or_else(|| words.iter().position(|(_, word)| flag(word)))
        .unwrap_or(words.len());
    let head_end = words[..head].last().map_or(0, |(at, word)| at + word.len());
    let mut kept = None;
    if width(&line[..head_end]) <= room {
        kept = Some(head_end);
        for (nth, &(at, word)) in words.iter().enumerate().skip(head) {
            let end = at + word.len();
            let takes = valued(word) && words.get(nth + 1).is_some_and(|(_, next)| !flag(next));
            if width(&line[..end]) <= room {
                if !takes {
                    kept = Some(end);
                }
                continue;
            }
            // `nth` is past the head's first word whenever `word` is not a flag, so `nth - 1` is.
            if !flag(word) && valued(words[nth - 1].1) {
                let inside = fits(&line[..end], room).len();
                if inside > at {
                    kept = Some(inside);
                }
            }
            break;
        }
    }
    if let Some(at) = kept {
        return Cow::Owned(format!("{}{mark}", line[..at].trim_end()));
    }
    let object = line[..head_end].rfind([' ', '/']).map_or(0, |at| at + 1);
    let (fixed, name) = line[..head_end].split_at(object);
    match columns
        .checked_sub(width(fixed))
        .map(|left| front(name, left, mark))
    {
        Some(name) if !name.is_empty() => Cow::Owned(format!("{fixed}{name}")),
        _ => tailed(line, columns, mark),
    }
}

/// **The identity cut — `screens/widgets.md` § 7's eleventh deliberate cut, one rule at six call
/// sites** (NOTES § D266): a `Confirm`'s title, the *Already gone* body, the in-flight footer, the
/// Alerts card's identity row and the detail heading. [`command_cut`] reaches the same end for the
/// `$` line's own `kind/name` word.
///
/// **Two objects that share almost their whole name differ at its end**, so the end is what stays:
/// `…m/checkout-worker-service-canary` and `…m/checkout-worker-service-stable`, where a tail-cut
/// drew both as `checkout-worker-servi…`. In that section's order: whole where it fits; else the
/// **namespace** gives way from its front behind one `…` — [`shortened`]'s cut, so the header and
/// this cannot disagree about which end of a name identifies it — with the `/` and the name whole;
/// `…/<name>` where none of the namespace fits; and only where even that is too wide does the
/// **name** give way from its front too, `…/…<tail>`. A bare cluster-scoped name is the plain
/// front-cut, with no `/` to keep.
///
/// **`columns` too narrow for `…/` itself draws the plain front-cut of the whole**, because a cut
/// wider than the row it is drawn in is worse than one that shows less — a guard on a helper, not
/// a state any of the six draws at the 80×24 floor.
fn name_cut(namespace: Option<&str>, object: &str, columns: usize) -> String {
    let whole = name(namespace, object);
    if width(&whole) <= columns {
        return whole;
    }
    let Some(namespace) = namespace else {
        return shortened(object, columns);
    };
    let kept = columns
        .checked_sub(width(object) + 1)
        .map(|room| shortened(namespace, room))
        .unwrap_or_default();
    if !kept.is_empty() {
        return format!("{kept}/{object}");
    }
    match columns.checked_sub(width(CUT) + 1) {
        Some(room) => format!("{CUT}/{}", shortened(object, room)),
        None => shortened(&whole, columns),
    }
}

/// **Word wrap, with a character break for a token wider than the line.**
///
/// The fallback is not hypothetical: rule 3's evidence carries
/// `"https://registry.invalid/v2/does-not-exist/manifests/v9":`, 58 columns, which is wider than
/// the 51 a card has at the floor. Wrapping alone cannot make that line fit
/// (`screens/alerts.md` § How wide a card is, and how tall).
///
/// **The space *between* two words is the caller's and is kept verbatim.** An analysis row is one
/// string that reads left to right — `k8rs-worker   0.45 of 12 cpu · 234Mi of 23.1Gi`, three
/// columns after the name — and a wrap that normalised runs of spaces would quietly redraw a line
/// `analysis.rs` had already spelled (`screens/analysis.md` § Capacity, its rule 3: this file
/// never splits a rendered string back into values, and rejoining one differently is the same
/// mistake from the other end). Single-spaced text is unaffected, which is every other caller.
/// What is still dropped is the space a line *breaks* on, at both ends: a continuation starts at
/// its word.
fn wrapped(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut left = text;
    while let Some(at) = left.find(|character: char| !character.is_whitespace()) {
        let (gap, tail) = left.split_at(at);
        // **The word runs to the first whitespace *after* its first character**, so it is never
        // empty, `left` is strictly shorter every turn, and this loop cannot spin. Stated where it
        // is relied on, and written with no arithmetic in it on purpose: a word end computed as an
        // offset plus a head is one `+` away from being 0, and a renderer that spins takes the
        // terminal with it.
        let end = tail
            .char_indices()
            .skip(1)
            .find(|(_, character)| character.is_whitespace())
            .map_or(tail.len(), |(at, _)| at);
        let (word, next) = tail.split_at(end);
        left = next;
        if !line.is_empty() && width(&line) + width(gap) + width(word) <= columns {
            line.push_str(gap);
            line.push_str(word);
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        let mut rest = word;
        while width(rest) > columns {
            let head = fits(rest, columns);
            // Nothing fits at all — a zero-column line, or one column against a wide character.
            // Emitting the word whole beats looping forever.
            if head.is_empty() {
                break;
            }
            lines.push(head.to_owned());
            rest = &rest[head.len()..];
        }
        line.push_str(rest);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Text, wrapped and **cut at `most` lines with a visible `…`**.
///
/// **Two callers and one rule.** A card's evidence is cut at [`EVIDENCE_LINES`], which is a
/// measurement (`screens/alerts.md` § How wide a card is, and how tall); a `Refused` dialog's
/// quoted cluster message is cut at whatever rows the box has left, which is arithmetic
/// ([`refused`]). Both are the only unbounded thing on the screen they are drawn on — everything
/// else there was written by a rule author, and these carry the cluster's own sentence verbatim
/// (NOTES § D37, § D217).
///
/// The cut walks back to a whole word to make room for the marker, and steps by characters where
/// there is no word boundary to find. The full text is one `⏎` away, which is what makes cutting
/// it legitimate at all (`screens/widgets.md` § 7).
fn cut(text: &str, columns: usize, most: usize) -> Vec<String> {
    marked(wrapped(text, columns), columns, most)
}

/// **One sentence that is several paragraphs, wrapped, with the blank row the screen file puts
/// between them** — [`wrapped`] collapses every run of whitespace, so a banner built on it alone
/// drew four paragraphs as one block of prose.
///
/// **Two of the nine states need it**: `screens/states.md` § *Your login expired* separates the
/// explanation, the renewal command and the staleness note, and § *You can only see some
/// namespaces* separates the fallback from the check it switched off — a section that file titles
/// *"The second paragraph is the point of this screen"*. A `crate::views::Pane::Denied` carries one
/// `String`, so the break is spelled in it the way it is spelled anywhere else, as a blank line.
fn paragraphed(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split("\n\n").filter(|part| !part.trim().is_empty()) {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        lines.extend(wrapped(paragraph, columns));
    }
    lines
}

/// [`cut`]'s own second half, over lines somebody else wrapped — **`most` of them, with [`CUT`]
/// where the rest were dropped**.
///
/// Split out because [`confirm`] wraps two strings into one block — a consequence and the warning
/// a check added — and a budget that covered only the first would let the second run past the box
/// (`screens/widgets.md` § 5: the blank row goes first, *before any sentence is cut*).
fn marked(mut lines: Vec<String>, columns: usize, most: usize) -> Vec<String> {
    if lines.len() <= most {
        return lines;
    }
    // **A budget of nothing still leaves the mark.** `truncate(0)` leaves no last line to put
    // [`CUT`] on, so the whole text disappeared with nothing to say it had — the *silent* cut
    // `screens/widgets.md` § 7 bans by name. One marked row is the floor, which is what lets
    // every caller here hand over a budget it computed rather than one it had to clamp.
    if most == 0 {
        return vec![CUT.to_owned()];
    }
    lines.truncate(most);
    if let Some(last) = lines.last_mut() {
        let room = columns.saturating_sub(width(CUT));
        let mut kept = last.trim_end();
        if width(kept) > room {
            let head = fits(kept, room);
            kept = match head.rfind(' ') {
                Some(at) => &kept[..at],
                None => head,
            };
        }
        *last = format!("{}{CUT}", kept.trim_end());
    }
    lines
}

/// The columns a row of spans occupies.
fn spanned(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

// --- MEASURING AND CUTTING END ---

#[cfg(test)]
#[path = "ui_tests.rs"]
mod tests;
