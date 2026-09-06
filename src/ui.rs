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
//! **What it does not draw yet**: the Analysis pane, the detail tabs, the modal layer and the
//! `?` overlay. [`content`] dispatches on [`crate::views::View`] and has two arms built of the
//! three, so an open report draws the Alerts pane until the box that writes its own.

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

use crate::analysis::Badge;
use crate::k8s::Browsable;
use crate::rules::{Finding, Severity};
use crate::theme::{self, Colour, Depth, Ink, Signal};
use crate::views::{self, App, Card, NavItem, Pane, View};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Row, Table, TableState};
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
    /// One entry per analysis report, in sidebar order: its label and its badge.
    ///
    /// **The label is not on [`crate::analysis::Report`] on purpose** — that type's own doc says
    /// so — and `views.rs` does not hold one either, so it arrives from the caller until
    /// somebody gives it a home.
    pub reports: &'a [(&'a str, Option<&'a Badge>)],
    /// The command log, oldest first. The strip draws the last [`LOG_LINES`] of it — display
    /// text, never executed and never fed back into a process (invariant 4).
    pub log: &'a [String],
    /// The footer: the keys valid right now, already spelled. Rebuilt by the caller every frame,
    /// because there is no stored footer (`screens/widgets.md` § 2).
    pub keys: &'a str,
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

/// The command log — **the last two lines, unwrapped**. These are copy-paste text and a wrapped
/// command is a lie (`screens/widgets.md` § 2).
fn strip(frame: &mut Frame, area: Rect, screen: &Screen) {
    let last = screen.log.len().saturating_sub(usize::from(LOG_LINES));
    let lines: Vec<Line> = screen.log[last..]
        .iter()
        .map(|line| Line::styled(line.as_str(), screen.fg(theme::INFO)))
        .collect();
    frame.render_widget(Paragraph::new(Text::from(lines)), indented(area));
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
                    Some((label, badge)) => (1, *label, theme::TEXT, value(*badge, screen)),
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
/// **[`View::Analysis`] draws the Alerts pane, because the Analysis pane is the next box**
/// (todo.md § Phase 11) — which is the behaviour this file already had when it dispatched on
/// nothing at all. It shares an arm with [`View::Alerts`] rather than falling into a `_`, so the
/// box that writes that pane finds the arm it has to split instead of a wildcard that swallowed
/// it.
fn content(frame: &mut Frame, area: Rect, app: &App, screen: &Screen) {
    match app.view {
        View::Resources(nth) => browser(frame, area, app, screen, screen.kinds.get(nth)),
        View::Alerts | View::Analysis(_) => match screen.alerts {
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
        let (colour, signal) = theme::band(Severity::Info);
        lines.push(
            Line::from(vec![
                Span::styled(format!("{}  ", mark(signal)), screen.fg(colour)),
                Span::styled("nothing is broken", screen.fg(theme::TEXT)),
            ])
            .centered(),
        );
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

    let name = name(card);
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
fn name(card: &Card) -> String {
    match &card.owner.namespace {
        Some(namespace) => format!("{namespace}/{}", card.owner.name),
        None => card.owner.name.clone(),
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
    // `screens/resources.md` § The line under the table, rules 2–4: whole and unmarked when it
    // fits; else `room` less [`CUT`]'s own column, with the mark glued to the last character
    // kept and no space before it; and nothing at all where there is no room to mark a cut in.
    let shown = match fits(name, room) {
        whole if whole == name => whole.to_owned(),
        _ if room == 0 => String::new(),
        _ => format!("{}{CUT}", fits(name, room.saturating_sub(width(CUT)))),
    };
    spans.push(Span::styled(
        format!("{shown}{tail}"),
        screen.fg(theme::DIM),
    ));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// --- THE BROWSER END ---

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

/// **Word wrap, with a character break for a token wider than the line.**
///
/// The fallback is not hypothetical: rule 3's evidence carries
/// `"https://registry.invalid/v2/does-not-exist/manifests/v9":`, 58 columns, which is wider than
/// the 51 a card has at the floor. Wrapping alone cannot make that line fit
/// (`screens/alerts.md` § How wide a card is, and how tall).
fn wrapped(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && width(&line) + 1 + width(word) <= columns {
            line.push(' ');
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
