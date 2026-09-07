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
//! **What it does not draw yet**: the modal layer and the `?` overlay. [`content`] dispatches on
//! [`crate::views::View`] and now has one arm per view, and on [`Screen::detail`] before any of
//! them — a detail is open *over* a view, which is what `esc back` means.

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
use crate::k8s::Browsable;
use crate::rules::{ContainerSnapshot, Finding, ObjectId, PodSnapshot, Severity, age};
use crate::theme::{self, Colour, Depth, Ink, Signal};
use crate::views::{self, App, Card, NavItem, Pane, Tab, View};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Row, Table, TableState, Tabs};
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
const BLOCK: u16 = 34;

/// `▸ ` — [`theme::SELECTION`] plus the column that keeps it off the label. Two columns, so a
/// group's row reads `▸  workloads` exactly as `screens/resources.md` draws it.
const MARKER: &str = "▸ ";

/// The centred zone of the header row (`screens/widgets.md` § 1a).
const NAME: &str = "k8rs";

/// **The only thing k8rs shortens on purpose, and it is always visible where it happened**
/// (`screens/widgets.md` § 7). One character, two callers — the evidence line and the header —
/// so the mark a reader learns is one mark.
const CUT: &str = "…";

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
    /// The header's right zone, already joined — `ctx: prod-eu · live · admin`, and on a longer
    /// row `ctx: prod-eu · ns: payments · live · read-only · ⚠ TLS not verified`.
    ///
    /// **Laid out first, and the last zone to give way**: `prod-eu` and `prod-eu-2` differ by one
    /// character. Where the whole row is not enough for it, what gives way is the **front** of
    /// this string — see [`shortened`], which is where the reason that end and not the other is
    /// written down. So the caller's order inside it is not cosmetic: **the cluster's name goes
    /// first and what the reader is allowed to do goes last.**
    pub context: &'a str,
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
    /// The footer: the keys valid right now, already spelled. Rebuilt by the caller every frame,
    /// because there is no stored footer (`screens/widgets.md` § 2).
    pub keys: &'a str,
    /// **The object a detail tab is open on, and what each of its four fetches answered** —
    /// `None` when nothing is open (`screens/detail.md`).
    ///
    /// **A detail is drawn *over* whatever view is open, which is why it is an `Option` here and
    /// not a fourth [`View`]**: `esc` goes back to the pane the reader came from, and a view that
    /// had to be re-derived to go back to would be a second place holding where they were.
    pub detail: Option<&'a Detail<'a>>,
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
    header(frame, top, screen);

    let border = screen.fg(theme::BORDER);
    let outer = Block::bordered().border_style(border);
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

    sidebar(frame, nav, app, screen);
    content(frame, pane, app, screen);
    strip(frame, log, screen);
    frame.render_widget(
        Paragraph::new(Line::styled(screen.keys, screen.fg(theme::DIM))),
        indented(keys),
    );

    // The two rules and the sidebar's divider are drawn last, over the panes' own edges, so the
    // frame is one shape rather than four blocks that happen to touch.
    rule(frame, rest, above_log.y, border);
    rule(frame, rest, above_keys.y, border);
    divider(frame, split.x, rest.y, above_log.y, border);
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
fn header(frame: &mut Frame, area: Rect, screen: &Screen) {
    let dim = screen.fg(theme::DIM);
    let context = shortened(screen.context, usize::from(area.width));
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
    let vitals = if width(screen.vitals) <= usize::from(room.width) {
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
    // **Each candidate is measured whole, marker included** — the same rule [`fits`] is written
    // to. Adding `width(CUT)` to the tail's own width is the sum this file has already been
    // caught by once, and a zone one column wider than its `Rect` is clipped at the tail again,
    // which is the whole defect. It is also why the marker gets no guard of its own: a row too
    // narrow for even `…` keeps nothing, which is what an empty start already says.
    let mut kept = String::new();
    for (at, _) in text.char_indices().rev() {
        let candidate = format!("{CUT}{}", &text[at..]);
        if width(&candidate) > columns {
            break;
        }
        kept = candidate;
    }
    kept
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
/// **The cut walks back to a whole word, which is the third of `screens/widgets.md` § 7's three
/// deliberate truncations** (that section, rewritten 2026-09-07, and `screens/detail.md` § The
/// yaml tab, which draws the one line in the whole directory that does not fit even at the true
/// floor). A flag with its last character sheared off still looks like a flag —
/// `--show-managed-fiel` is not one a reader would notice was wrong — so the whole token gives
/// way and the mark lands where the reader can see it.
fn strip(frame: &mut Frame, area: Rect, screen: &Screen) {
    let row = indented(area);
    let last = screen.log.len().saturating_sub(usize::from(LOG_LINES));
    let lines: Vec<Line> = screen.log[last..]
        .iter()
        .map(|line| {
            Line::styled(
                command_cut(line, usize::from(row.width)),
                screen.fg(theme::INFO),
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)), row);
}

// --- THE FRAME END ---

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
            let mut spans = vec![Span::styled(
                format!("{blank:indent$}{label}", blank = ""),
                screen.fg(style),
            )];
            let pad = inside.saturating_sub(indent + width(label) + spanned(&badge));
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
            None => note(frame, area, screen, false),
            Some(report) => analysis(frame, area, app, screen, report),
        },
        View::Alerts => match screen.alerts {
            Pane::Loading => note(frame, area, screen, false),
            Pane::Denied(said, cards) => {
                let rest = banner(frame, area, screen, said);
                alerts(frame, rest, app, screen, cards);
            }
            Pane::Ready(cards) if cards.is_empty() => note(frame, area, screen, true),
            Pane::Ready(cards) => alerts(frame, area, app, screen, cards),
        },
    }
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
fn note(frame: &mut Frame, area: Rect, screen: &Screen, healthy: bool) {
    let mut lines: Vec<Line> = Vec::new();
    if !healthy && screen.note.is_empty() {
        lines.push(Line::styled("reading the cluster…", screen.fg(theme::DIM)));
    }
    if healthy {
        lines.push(calm(screen, "nothing is broken").centered());
        lines.push(Line::default());
    }
    let dim = screen.fg(theme::DIM);
    for (nth, paragraph) in screen.note.iter().enumerate() {
        if nth > 0 {
            lines.push(Line::default());
        }
        lines.extend(
            wrapped(paragraph, usize::from(BLOCK))
                .into_iter()
                .map(|line| Line::styled(line, dim)),
        );
    }
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
/// at 34 columns. The pane is the real ceiling. [`note`]'s own lines are wrapped at `BLOCK`
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
fn banner(frame: &mut Frame, area: Rect, screen: &Screen, said: &str) -> Rect {
    let text = wrapped(said, usize::from(padded(area).width));
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
    let area = padded(area);
    let region = usize::from(area.width);
    let items: Vec<ListItem> = cards
        .iter()
        .map(|card| ListItem::new(Text::from(lines(card, screen, region))))
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
        drawn.extend(indent(cut(&finding.evidence, body), "  ", dim));
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

    let name = name(&card.owner);
    let whole = match card.count() {
        Some(count) => format!("{name}  ·  {count}"),
        None => name.clone(),
    };
    let left = if width(&whole) <= room {
        whole
    } else {
        fits(&name, room).to_owned()
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
/// **Over an [`ObjectId`] and not over a [`Card`]**, because a detail pane's own heading is the
/// same spelling of the same fact and a second one would be a second answer for `node-3`.
fn name(id: &ObjectId) -> String {
    match &id.namespace {
        Some(namespace) => format!("{namespace}/{}", id.name),
        None => id.name.clone(),
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
    let area = match screen.browser {
        Pane::Denied(said, _) => banner(frame, area, screen, said),
        _ => area,
    };
    let [head, _, body] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    // **The title is padded and the table is not**, which is what both mockups draw: the
    // selection marker is the table's own gutter and sits at the pane's left edge, exactly where
    // the sidebar's does (`screens/resources.md`, `screens/widgets.md` § 2's indent rule). A
    // table padded like a card would spend two columns twice over, on the widest thing on the
    // screen.
    heading(frame, padded(head), screen, kind);
    let scoped = scope(kind, screen).is_some();
    match screen.browser {
        Pane::Loading => note(frame, body, screen, false),
        // **A refusal draws whatever did come back and never the empty sentence below**, which is
        // [`banner`]'s rule one line up: *we were not allowed to look* is not *there is nothing*.
        // A refusal that came back with no rows at all draws the banner and an empty grid.
        Pane::Denied(_, table) => grid(frame, body, app, screen, table, scoped),
        Pane::Ready(table) if table.rows.is_empty() => empty(frame, body, screen, kind),
        Pane::Ready(table) => grid(frame, body, app, screen, table, scoped),
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
    // At 34 it breaks the third sentence in half. The pane still bounds it, so one too narrow for
    // the sentence wraps rather than overruns; the two scoped sentences are shorter than `BLOCK`,
    // which [`centred`] keeps as its floor, so their block is the same 34 columns it always was.
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
    let marks: Vec<Option<&Card>> = table.rows.iter().map(|row| about(cards, row)).collect();
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
            let widest = table
                .rows
                .iter()
                .map(|row| row.cells.get(*at).map_or(0, |cell| width(cell) as u16))
                .max()
                .unwrap_or(0);
            Constraint::Length(header.max(widest))
        })
        .collect();

    let rows: Vec<Row> = table
        .rows
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
    let anchors: Vec<Option<&str>> = table.rows.iter().map(|row| row.uid.as_deref()).collect();
    let at = app.content.selected(&anchors);
    // **The line is about the row the cursor is on, not about every marked one** (NOTES § D251):
    // one per marked row would be a second list competing with the table above it.
    let selected = at.and_then(|nth| table.rows.get(nth).zip(marks.get(nth).copied().flatten()));
    let (area, under) = match selected {
        // **Right under the last row, and never off the bottom of the pane.** The table takes the
        // height it needs up to two lines short of the pane, so a short kind draws the mockup's
        // own spacing and a long one still keeps the line that says `⏎ to see`. Under three rows
        // the table gets none and only the line is drawn; no pane the frame lays out is that
        // short, the floor being 80×24 and this body thirteen.
        Some(pair) => {
            let tall = u16::try_from(table.rows.len() + 1).unwrap_or(u16::MAX);
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
        Table::new(rows, widths)
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
    let items: Vec<ListItem> = report
        .rows
        .iter()
        .enumerate()
        .map(|(nth, row)| {
            let above = nth
                .checked_sub(1)
                .and_then(|before| report.rows.get(before));
            ListItem::new(Text::from(drawn(row, above, screen, region)))
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
/// card, where an age is right-aligned and the name clips: there are two zones there and one here.
///
/// **The blank lines are the block structure and nothing else.** A [`ReportRow::Prose`] or a
/// [`ReportRow::NotComputed`] is separated from the answers above it by one blank line — never at
/// the top of the body, and never after a `NotComputed`, which already closed with one. Answers
/// pack, so a list of nodes reads as a list. `above` is the row before this one, which is all that
/// is needed to know which of those it is.
fn drawn<'a>(
    row: &ReportRow,
    above: Option<&ReportRow>,
    screen: &Screen,
    region: usize,
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
            for paragraph in detail {
                lines.extend(indent(wrapped(paragraph, under), "    ", ink));
            }
            // **The action is never cut and its continuation sits under the text after the
            // arrow**, the same as a card's — `views.rs` draws the `→ `, so the value starts at
            // the word (`crate::analysis::Row::Answer::action`).
            let mut action = wrapped(action, after).into_iter();
            if let Some(first) = action.next() {
                lines.push(Line::from(vec![
                    Span::styled("    → ", screen.fg(theme::ACCENT)),
                    Span::styled(first, ink),
                ]));
                lines.extend(indent(action.collect(), "      ", ink));
            }
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
    frame.render_widget(
        Paragraph::new(Line::styled(name(open.object), screen.fg(theme::TEXT))),
        padded(head),
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
        Pane::Loading => note(frame, area, screen, false),
        Pane::Denied(said, held) => {
            let rest = banner(frame, area, screen, said);
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
        Pane::Loading => return note(frame, area, screen, false),
        Pane::Denied(said, read) => (banner(frame, area, screen, said), read),
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
        Pane::Loading => return note(frame, area, screen, false),
        // **A refusal draws whatever did come back and never the empty sentence below.** A read
        // that was refused and answered with nothing draws the banner and an empty pane, which is
        // the browser's own rule one region up.
        Pane::Denied(said, happened) => {
            let rest = banner(frame, area, screen, said);
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
        Pane::Loading => return note(frame, area, screen, false),
        Pane::Denied(said, document) => (banner(frame, area, screen, said), document),
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
    match fits(text, columns) {
        whole if whole == text => Cow::Borrowed(whole),
        _ if columns < width(CUT) => Cow::Borrowed(""),
        _ => Cow::Owned(format!("{}{CUT}", fits(text, columns - width(CUT)))),
    }
}

/// [`clipped`], **walked back to a whole word first** — [`strip`]'s rule, and the one thing that
/// separates it from [`clipped`]'s own (`screens/widgets.md` § 7 names three deliberate
/// truncations; this is the third, the browser's row is [`clipped`] and the evidence line is
/// [`cut`]).
///
/// A command is several tokens and the browser's row is one name, which is the whole of why they
/// differ: there is a word boundary to find here and none there, so a name cuts mid-token behind
/// the mark and a command gives up the token whole. **`--show-managed-fiel` is the case that
/// decides it** — a flag one character short still reads as a flag, where
/// `-o yaml…` visibly is not the end of the line (`screens/detail.md` § The yaml tab).
///
/// **The character break is still the floor**, through [`clipped`]: a single token wider than the
/// row has no space to walk back to, and dropping it whole would draw an empty strip where a
/// marked prefix is what the reader needs.
fn command_cut(line: &str, columns: usize) -> Cow<'_, str> {
    if width(line) <= columns {
        return Cow::Borrowed(line);
    }
    let room = columns.saturating_sub(width(CUT));
    match fits(line, room).rfind(' ') {
        Some(at) => Cow::Owned(format!("{}{CUT}", line[..at].trim_end())),
        None => clipped(line, columns),
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

/// The evidence, wrapped and **cut at [`EVIDENCE_LINES`] with a visible `…`**.
///
/// It is the only unbounded thing on a card — everything else was written by a rule author, and
/// this carries a controller's sentence quoted verbatim (NOTES § D37). The cut walks back to a
/// whole word to make room for the marker, and steps by characters where there is no word
/// boundary to find. The full text is one `⏎` away, which is what makes cutting it legitimate at
/// all (`screens/widgets.md` § 7).
fn cut(text: &str, columns: usize) -> Vec<String> {
    let mut lines = wrapped(text, columns);
    if lines.len() <= EVIDENCE_LINES {
        return lines;
    }
    lines.truncate(EVIDENCE_LINES);
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
