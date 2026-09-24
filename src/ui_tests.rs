//! Tests for [`super`] — the whole frame and the Alerts pane, rendered into a `Buffer` with no
//! terminal in the room. `TestBackend` is the entire reason this file can be tested at all.
//!
//! **The fixtures here are constructed, and that is not a breach of NOTES § D53**, for the reason
//! `views_tests.rs` states one layer down: that rule is about *captures*, and [`Card`] and
//! [`Finding`] are k8rs's own types, produced from captures that are tested where they are
//! decoded. What this file proves is what the renderer does with them once they exist.
//!
//! **The strings that measure things are the screen file's own**, not a shape that happens to be
//! long: `screens/alerts.md` § The columns is what every column number below is read off, and the
//! evidence quotes are the committed capture's bytes as that file prints them.

use super::*;
use crate::k8s::Fault;
use crate::rules::{
    ClusterSnapshot, ContainerSnapshot, ContainerState, Finding, NodeSnapshot, ObjectId,
    ObjectKind, PodSnapshot,
};
use crate::views::{Group, Op};
use k8s_openapi::jiff::Timestamp;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

// --- BUILDING A SCREEN ---

/// A moment, `n` seconds after a fixed epoch, so an age is arithmetic and not a clock read.
fn at(second: i64) -> Time {
    Time(Timestamp::from_second(1_700_000_000 + second).expect("a representable second"))
}

/// The reader's moment: four minutes after every stamp below, which is `4 min ago` on the ladder.
fn now() -> Time {
    at(240)
}

fn id(kind: ObjectKind, namespace: Option<&str>, name: &str) -> ObjectId {
    ObjectId {
        kind,
        namespace: namespace.map(str::to_owned),
        name: name.to_owned(),
        uid: Some("u-1".to_owned()),
    }
}

fn finding(severity: Severity, title: &str, evidence: &str, action: &str) -> Finding {
    Finding {
        severity,
        title: title.to_owned(),
        evidence: evidence.to_owned(),
        action: action.to_owned(),
        kubectl_cmd: None,
        owner: id(ObjectKind::Deployment, Some("payments"), "web"),
        object: id(ObjectKind::Pod, Some("payments"), "web-1"),
        timestamp: Some(at(0)),
    }
}

/// The OOM card `screens/alerts.md` opens with.
fn oom() -> Card {
    Card {
        owner: id(ObjectKind::Deployment, Some("payments"), "web"),
        findings: vec![finding(
            Severity::Critical,
            "Containers exceeded their memory limit and were killed by the kernel (OOMKilled)",
            "limit 256Mi · exit 137 · 47 restarts",
            "raise limits.memory, or find the leak",
        )],
        affected: 3,
        total: Some(5),
    }
}

/// The cordon card, whose age is an `Option` and whose owner is a node.
fn cordon(added: Option<Time>) -> Card {
    let node = id(ObjectKind::Node, None, "node-3");
    Card {
        owner: node.clone(),
        findings: vec![Finding {
            timestamp: added,
            owner: node.clone(),
            object: node,
            ..finding(
                Severity::Warn,
                "This node refuses new pods (cordoned)",
                "2 pods here would still have to move",
                "allow new pods once the work is done",
            )
        }],
        affected: 0,
        total: None,
    }
}

fn app() -> App {
    App::default()
}

/// **A report with nothing in it but its badge** — what the sidebar reads off one. Constructed
/// and not computed, because what the two sidebar tests below measure is that column's
/// arithmetic; every test that draws a *pane* uses a real producer over a committed capture
/// ([`reported`]).
fn badged(value: &str, severity: Severity) -> Report {
    Report {
        title: "what this report answers".to_owned(),
        badge: Some(Badge {
            value: value.to_owned(),
            severity,
        }),
        rows: Vec::new(),
    }
}

/// The browser pane a test that is not about the browser hands over: still loading, which is
/// what a view nobody opened has answered.
static UNOPENED: Pane<crate::k8s::Table> = Pane::Loading;

fn screen<'a>(alerts: &'a Pane<Vec<Card>>, now: &'a Time) -> Screen<'a> {
    Screen {
        depth: Depth::TrueColor,
        vitals: Stripped::of("nodes 3/3"),
        context: Stripped::of("ctx: prod-eu"),
        insecure: false,
        alerts,
        browser: &UNOPENED,
        namespace: None,
        now,
        note: &[],
        kinds: &[],
        reports: &[],
        log: &[],
        refused: Refused::default(),
        writes: Writes::Live,
        clock: None,
        link: Link::Live,
        detail: None,
        contexts: &[],
    }
}

/// **One tab's offset, and zero for the other three** — `App::scroll` is four numbers, one per tab
/// (`screens/widgets.md` § 4), and every test below is about the tab it opens.
fn offsets(tab: Tab, offset: u16) -> [u16; Tab::ALL.len()] {
    let mut scroll = [0; Tab::ALL.len()];
    scroll[tab.at()] = offset;
    scroll
}

/// **Draw, and keep what the frame resolved [`App::scroll`] to** — `ui::draw` takes `&mut App`
/// because [`scrolled`] writes the row it drew back into the state, and the scroll tests are about
/// exactly that value. Every other test in this file is about what is on the screen, so
/// [`render_at`] hands this a copy and they need no `mut` binding of their own.
fn render_into(columns: u16, rows: u16, app: &mut App, screen: &Screen) -> Buffer {
    let mut terminal =
        Terminal::new(TestBackend::new(columns, rows)).expect("a terminal over a test backend");
    terminal
        .draw(|frame| draw(frame, app, screen))
        .expect("a frame");
    terminal.backend().buffer().clone()
}

fn render_at(columns: u16, rows: u16, app: &App, screen: &Screen) -> Buffer {
    render_into(columns, rows, &mut app.clone(), screen)
}

fn render(app: &App, screen: &Screen) -> Buffer {
    render_at(MIN_WIDTH, MIN_HEIGHT, app, screen)
}

/// The buffer as the rows a reader sees, trailing blanks kept — every column assertion below
/// counts them.
fn rows(buffer: &Buffer) -> Vec<String> {
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map_or(" ", ratatui::buffer::Cell::symbol)
                })
                .collect()
        })
        .collect()
}

/// The row a needle is on, so a test names what it is looking for rather than a row number that
/// moves when the frame does.
fn row(buffer: &Buffer, needle: &str) -> String {
    rows(buffer)
        .into_iter()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", rows(buffer).join("\n")))
}

/// [`row`], of the **body** only. **The header carries a `\u{26a0}` of its own under
/// [`Link::Lost`] and [`Link::Expired`]** — `\u{26a0} disconnected, retrying`,
/// `\u{26a0} login expired`, joined from [`Screen::link`] since Phase 12 — so a search for a
/// banner's mark that read the whole frame would measure the header's column and never the
/// banner's. (Under [`Link::Unconnected`] the mark is the *caller's*, inside the context zone, and
/// reads the same way.)
fn body_row(buffer: &Buffer, needle: &str) -> String {
    rows(buffer)
        .into_iter()
        .skip(1)
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no body row holds {needle:?}\n{}", rows(buffer).join("\n")))
}

fn holds(buffer: &Buffer, needle: &str) -> bool {
    rows(buffer).iter().any(|line| line.contains(needle))
}

/// The pane half of a row — the sidebar and the divider dropped — so a column assertion counts
/// from the content pane's own left edge. 22 is `1` border + [`SIDEBAR`] + `1` divider.
fn pane(line: &str) -> String {
    line.chars().skip(usize::from(1 + SIDEBAR + 1)).collect()
}

/// **A card row with the cursor's gutter dropped** — [`MARKER`] where that card is the selected
/// one, two blank columns where it is not (`screens/alerts.md` § The selected card). Every
/// assertion about a card's *own* columns reads this, so the name, the age and the wrap are
/// measured from where the card's text begins whichever card the cursor is on; that the mark is
/// there at all, and that it costs nothing, is
/// [`the_marked_card_is_the_selected_one_and_the_mark_costs_no_column`]'s to say.
///
/// **Card rows only** — a banner's own two blank columns are [`padded`]'s and not a gutter, so this
/// would eat a column of its text.
fn unmarked(line: &str) -> String {
    let row = pane(line);
    match row.strip_prefix(MARKER) {
        Some(rest) => rest.to_owned(),
        None => row.chars().skip(width(MARKER)).collect(),
    }
}

/// The character in one column. **Not a byte slice**: every row here carries box-drawing and
/// severity glyphs, and `&line[21..22]` lands inside one of them.
fn column(line: &str, at: usize) -> char {
    line.chars().nth(at).unwrap_or(' ')
}

// --- THE FRAME ---

/// The 24 rows are spent exactly as `screens/alerts.md` § The height spends them, and the count
/// is the thing to assert: a body one row short is a card the reader never sees.
#[test]
fn the_frame_spends_the_rows_the_way_the_screen_file_does() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let drawn = render(&app(), &screen(&alerts, &now));
    let rows = rows(&drawn);

    assert!(rows[0].starts_with(" nodes 3/3"), "row 0 is the header row");
    assert!(rows[1].starts_with('┌'), "row 1 opens the frame");
    // 16 body rows, then the rule that closes them.
    assert!(
        rows[18].starts_with('├') && rows[18].ends_with('┤'),
        "row 18 is the rule under the body, not {:?}",
        rows[18]
    );
    assert!(rows[19].starts_with('│'), "row 19 is the command log");
    assert!(
        rows[21].starts_with('├'),
        "row 21 is the rule under the log, not {:?}",
        rows[21]
    );
    assert!(rows[22].starts_with('│'), "row 22 is the footer");
    assert!(rows[23].starts_with('└'), "row 23 closes the frame");
}

/// 80 − 2 borders − 20 sidebar − 1 divider = 57, and the divider is a divider on every body row.
#[test]
fn the_sidebar_is_twenty_columns_and_the_pane_takes_the_rest() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let drawn = render(&app(), &screen(&alerts, &now));
    let rows = rows(&drawn);

    assert_eq!(
        column(&rows[1], 21),
        '┬',
        "the top border joins the divider"
    );
    assert_eq!(
        column(&rows[18], 21),
        '┴',
        "and the rule under the body closes it"
    );
    for (nth, line) in rows.iter().enumerate().take(18).skip(2) {
        assert_eq!(column(line, 21), '│', "body row {nth} carries the divider");
    }
}

/// `screens/widgets.md` § 8 — below the floor there is no layout at all.
#[test]
fn below_the_floor_there_is_one_sentence_and_no_frame() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let drawn = render_at(64, 18, &app(), &screen(&alerts, &now));

    assert!(
        holds(
            &drawn,
            "k8rs needs a terminal at least 80×24. This one is 64×18."
        ),
        "{}",
        rows(&drawn).join("\n")
    );
    assert!(!holds(&drawn, "┌"), "and no frame is attempted");
    assert!(!holds(&drawn, "ALERTS"), "and no sidebar");
}

/// **Each half of the floor has to be able to refuse on its own.** A terminal that is small in
/// both directions cannot tell `width < 80 || height < 24` from the same line with `&&`, or from
/// either comparison the wrong way round — measured, all three of those mutants survived a test
/// that only tried 64×18.
#[test]
fn each_half_of_the_floor_refuses_on_its_own() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let screen = screen(&alerts, &now);

    for (columns, lines) in [(79, 24), (80, 23), (79, 23)] {
        let drawn = render_at(columns, lines, &app(), &screen);
        assert!(
            holds(&drawn, "k8rs needs a terminal at least 80×24."),
            "{columns}×{lines} is below the floor:\n{}",
            rows(&drawn).join("\n")
        );
        assert!(!holds(&drawn, "┌"), "{columns}×{lines} draws no frame");
    }

    // And the floor itself is not below itself.
    let drawn = render_at(MIN_WIDTH, MIN_HEIGHT, &app(), &screen);
    assert!(!holds(&drawn, "k8rs needs a terminal"));
    assert!(holds(&drawn, "┌"));
}

/// The one-column pad inside the frame is a pad and not a nudge: a line handed the whole width
/// writes over the frame's own border.
#[test]
fn a_line_inside_the_frame_never_reaches_the_border() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let long =
        "$ kubectl get pods -n a-namespace-with-a-name-long-enough-to-run-past-the-edge --watch";
    let log = [Stripped::of(long), Stripped::of(long)];
    let mut wide = screen(&alerts, &now);
    wide.log = &log;
    let drawn = render(&app(), &wide);
    let rows = rows(&drawn);

    for nth in [19, 20, 22] {
        assert!(rows[nth].starts_with('│'), "row {nth}: {:?}", rows[nth]);
        assert!(rows[nth].ends_with('│'), "row {nth}: {:?}", rows[nth]);
    }

    // **The footer can no longer be the line that runs past the border** — every string
    // `views::App::footer` returns is a literal inside the 76 columns [`indented`] leaves. What
    // it can still get wrong is the pad, and a short line proves nothing about a pad by starting
    // with `│`, so the first key is named at the column it is drawn at.
    assert!(
        rows[22].starts_with("│ ↑↓ move"),
        "the footer lost its one-column pad: {:?}",
        rows[22]
    );
}

/// **`k8rs` is drawn only with two blank columns each side, and the boundary is where the
/// mutants live.** At 80 columns the centred name starts at 38, so a left zone of 35 keeps it and
/// one of 36 loses it; on the other side a context zone of 36 keeps it and one of 37 loses it —
/// the whole zone, `live · admin` included, which is the header's to join and not the caller's
/// (NOTES § D265 ruling 1, todo.md § Phase 12). The name is the only zone that gives way — never
/// the context.
#[test]
fn the_name_needs_two_blank_columns_on_each_side() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let header = |screen: &Screen| rows(&render(&app(), screen))[0].clone();

    let thirty_five = "n".repeat(35);
    let mut roomy = screen(&alerts, &now);
    roomy.vitals = Stripped::of(&thirty_five);
    assert!(
        header(&roomy).contains("k8rs"),
        "a 35-column left zone fits"
    );

    let thirty_six = "n".repeat(36);
    let mut tight = screen(&alerts, &now);
    tight.vitals = Stripped::of(&thirty_six);
    assert!(!header(&tight).contains("k8rs"), "a 36-column one does not");

    let last = format!("ctx: {}", "c".repeat(16));
    let mut roomy = screen(&alerts, &now);
    roomy.context = Stripped::of(&last);
    assert_eq!(width(&format!("{last} · live · admin")), 36);
    assert!(
        header(&roomy).contains("k8rs"),
        "a 36-column context still leaves two blanks"
    );

    let over = format!("ctx: {}", "c".repeat(17));
    let mut tight = screen(&alerts, &now);
    tight.context = Stripped::of(&over);
    let zone = format!("{over} · live · admin");
    assert_eq!(width(&zone), 37);
    let drawn = header(&tight);
    assert!(
        !drawn.contains("k8rs"),
        "and a 37-column one does not: {drawn:?}"
    );
    assert!(
        drawn.ends_with(&zone),
        "the context is never the zone that gives way: {drawn:?}"
    );
}

#[test]
fn the_header_puts_the_name_between_the_two_zones() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let drawn = render(&app(), &screen(&alerts, &now));
    let header = &rows(&drawn)[0];

    assert!(header.starts_with(" nodes 3/3"), "vitals left: {header:?}");
    assert!(
        header.ends_with("ctx: prod-eu · live · admin"),
        "context right, never truncated: {header:?}"
    );
    assert!(header.contains("k8rs"), "the name is centred: {header:?}");
}

/// The name is the first zone to go, and the context is never the one that gives way.
#[test]
fn the_name_is_dropped_when_the_row_fills_up() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut wide = screen(&alerts, &now);
    wide.context = Stripped::of("ctx: a-very-long-context-name-indeed · ns: payments");
    wide.writes = Writes::ReadOnly;
    wide.insecure = true;
    let drawn = render(&app(), &wide);
    let header = &rows(&drawn)[0];

    assert!(!header.contains("k8rs"), "the name went first: {header:?}");
    assert!(
        header.contains("read-only"),
        "and the context stayed: {header:?}"
    );
    assert!(
        header.ends_with("⚠ TLS not verified"),
        "including its last column, which is the one that says so: {header:?}"
    );
}

/// **The right zone never loses its tail, because its tail is what says what you may do here.**
///
/// `screens/widgets.md` § 1a orders the sacrifice — name, then vitals, never the context — and an
/// EKS ARN at 80 columns runs out of row anyway. What gives way then is the context\'s own *name*,
/// from the left, behind a `…` the reader can see; `read-only` and the TLS warning stay put.
/// A right-aligned `Line` handed a zone narrower than itself is clipped at its **tail** by
/// ratatui, which drops exactly those two and leaves a row still reading as complete.
#[test]
fn the_context_elides_its_name_and_never_its_tail() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let arn = "ctx: arn:aws:eks:eu-west-1:123456789012:cluster/production-eu \
               · ns: payments";
    let zone = format!("{arn} · live · read-only · ⚠ TLS not verified");
    let mut screen = screen(&alerts, &now);
    screen.context = Stripped::of(arn);
    screen.writes = Writes::ReadOnly;
    screen.insecure = true;
    assert_eq!(width(&zone), 116, "wider than the row, by half again");

    let drawn = render(&app(), &screen);
    let header = rows(&drawn)[0].clone();
    println!("{header}");

    assert!(
        header.ends_with("read-only · ⚠ TLS not verified"),
        "the tail is intact: {header:?}"
    );
    assert!(
        header.starts_with('…'),
        "and the cut is marked where it happened: {header:?}"
    );
    assert!(
        header.contains("cluster/production-eu"),
        "the end of the name is what tells two clusters apart: {header:?}"
    );
    assert!(!header.contains("k8rs"), "the name went first: {header:?}");
    assert!(
        !header.contains("nodes"),
        "the vitals went second: {header:?}"
    );

    // **And the zone fits the row at every width, not just this one** — the property [`fits`]
    // owes at the other end of a string, and the reason the marker needs no special case of its
    // own: a row with no room for `…` keeps nothing.
    for columns in 0..=width(&zone) {
        let zone = shortened(&zone, columns);
        assert!(
            width(&zone) <= columns,
            "{columns} columns asked for, {} drawn: {zone:?}",
            width(&zone)
        );
    }
}

/// An `App` with a write on the wire for one object — what `screens/dialogs.md` § *While the call
/// is running* is drawn from.
fn changing(namespace: Option<&str>, name: &str) -> App {
    App {
        changing: Some(views::Object::new(
            "deployment",
            namespace.map(str::to_owned),
            name.to_owned(),
            Some("8656c3ec-0f0e-4d0e-9f0b-2a1d3c4b5a69".to_owned()),
        )),
        ..App::default()
    }
}

/// **`changing…` joins the right zone last of all** — after `admin`/`read-only` and after any TLS
/// warning, never ahead of them (`screens/widgets.md` § 1a, `screens/dialogs.md` § *While the call
/// is running*, whose own header line is the first case below).
///
/// **The mark is `theme::CHANGING` and not a second literal**, which is why it is read off the
/// palette here rather than typed again.
///
/// **The healthy case is the same frame with nothing running**, asserted first — a header that
/// appended unconditionally could not pass it.
///
/// **What the mark costs comes out of the centred name and nothing else**, which is § 1a's own
/// order of sacrifice and is unchanged by this box: **twelve** more columns of right zone push
/// `k8rs` off the row — ` · ` is three and `changing…` is nine — and the vitals stay. The first
/// draft of this comment said eleven, reasoned from the mark alone rather than measured off the
/// two rows (`tester`, 2026-09-12; the assertion below was right all along).
#[test]
fn the_header_says_a_change_is_running_and_says_it_last_of_all() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let header = |app: &App, screen: &Screen| rows(&render(app, screen))[0].trim_end().to_owned();

    let plain = screen(&alerts, &now);
    let zone = "ctx: prod-eu · live · admin";
    let quiet = header(&app(), &plain);
    assert!(
        quiet.ends_with(zone),
        "the ordinary header moved: {quiet:?}"
    );
    assert!(
        !quiet.contains(mark(theme::CHANGING)),
        "a mark for a call nobody made: {quiet:?}"
    );

    // **The row `screens/dialogs.md` draws, read out of it** rather than typed here — the same
    // reason [`mockup_dialog`] exists, applied to the one block that is labelled lines and not a
    // frame.
    let expected = labelled(&mockup_in_flight()[0], "header");
    assert_eq!(
        width(&expected),
        width(zone) + 12,
        "the twelve columns this test's own comment counts: {expected:?}"
    );
    let running = header(&changing(Some("payments"), "web"), &plain);
    assert!(
        running.ends_with(&expected),
        "`screens/dialogs.md` draws this row verbatim: {running:?} against {expected:?}"
    );
    assert!(
        quiet.contains("k8rs") && !running.contains("k8rs"),
        "the centred name is what pays for the mark: {quiet:?} then {running:?}"
    );
    assert!(
        running.starts_with(" nodes 3/3"),
        "the vitals gave way before the name had: {running:?}"
    );

    // **Last of all means after the TLS warning too**, which is the segment a caller joining this
    // in itself would most easily have put it in front of.
    let mut guarded = screen(&alerts, &now);
    guarded.writes = Writes::ReadOnly;
    guarded.insecure = true;
    let drawn = header(&changing(None, "node-3"), &guarded);
    assert!(
        drawn.ends_with("read-only · ⚠ TLS not verified · changing…"),
        "the mark jumped ahead of what says what you may do here: {drawn:?}"
    );
}

/// **The zone still gives way from its front, and the mark is in the half that never erodes**
/// (`screens/widgets.md` § 1a, NOTES § D249). The EKS ARN that already overflows the row at 80
/// columns overflows it by nine more with the mark appended, so the cut is real and the tail —
/// `read-only`, the TLS warning and `changing…` — survives it whole.
#[test]
fn a_change_in_flight_survives_the_cut_the_clusters_own_name_does_not() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let arn = "ctx: arn:aws:eks:eu-west-1:123456789012:cluster/production-eu \
               · ns: payments";
    let mut screen = screen(&alerts, &now);
    screen.context = Stripped::of(arn);
    screen.writes = Writes::ReadOnly;
    screen.insecure = true;

    let drawn = rows(&render(&changing(Some("payments"), "web"), &screen))[0].clone();
    println!("{drawn}");
    assert!(
        drawn.starts_with('…'),
        "the cut is marked where it happened: {drawn:?}"
    );
    assert!(
        drawn.ends_with("read-only · ⚠ TLS not verified · changing…"),
        "the tail is intact, mark included: {drawn:?}"
    );
    assert_eq!(
        width(drawn.trim_end()),
        80,
        "the zone still fits the row it is drawn in"
    );
}

/// **`admin` or `read-only` is the header's own word, joined after the caller's segments in every
/// state and not only the startup picker's** (`screens/widgets.md` § 1a, NOTES § D265 ruling 1).
/// It is [`Screen::writes`]' answer, and **the same `read-only` for both causes that kill
/// writes** — the header says what is true now, not why (`screens/help.md` § Under a dead-writes
/// run draws it).
///
/// **Each frame is also asserted not to hold the other word**, so a header that drew both, or one
/// of them whatever the run, fails here and not only a header that drew neither.
///
/// **And the word stays while a key is withheld for another reason** — a lost link, an expired
/// login, a skewed clock (NOTES § D265 ruling 6): `read-only` during a disconnect is when a reader
/// is about to press something. A header that dropped the word whenever the link was not live, or
/// whenever [`withheld`] held, went through every test until this loop (`tester`, R12 and R13).
///
/// **[`Link::Unconnected`] is in the loop for the permission word's sake as much as the connection
/// one**: it is the state with no connection word, and dropping the *permission* word with it
/// would be that same R12 failure one state over.
#[test]
fn the_header_says_what_this_run_may_do_after_the_callers_own_segments() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut seen = 0;
    for (link, state, clock) in [
        (Link::Connecting, Some("connecting…"), None),
        (Link::Live, Some("live"), None),
        (Link::Lost, Some("⚠ disconnected, retrying"), None),
        (Link::Expired, Some("⚠ login expired"), None),
        (Link::Unconnected, None, None),
        (Link::Live, Some("live"), Some(SKEWED)),
    ] {
        for (writes, word, other) in [
            (Writes::Live, "admin", "read-only"),
            (Writes::ReadOnly, "read-only", "admin"),
            (
                Writes::Unaudited("the audit log could not be opened"),
                "read-only",
                "admin",
            ),
        ] {
            let mut screen = screen(&alerts, &now);
            screen.writes = writes;
            screen.link = link;
            screen.clock = clock.map(Stripped::of);
            let drawn = rows(&render(&app(), &screen))[0].clone();
            let what = format!("{writes:?} · {link:?} · clock {clock:?}");
            // **The connection word is the header's too, off [`Screen::link`]** — the caller's
            // `ctx: prod-eu` carries none, so a header that read one out of the string would
            // draw nothing here (todo.md § Phase 12).
            let zone = match state {
                Some(state) => format!("ctx: prod-eu · {state} · {word}"),
                None => format!("ctx: prod-eu · {word}"),
            };
            assert!(drawn.ends_with(&zone), "{what}: {drawn:?}");
            assert!(!drawn.contains(other), "{what}: {drawn:?}");
            // **[`Link::Unconnected`] joins no word at all, and the claim is that *none of the
            // four* appears** — not that the row happens to be short (`screens/widgets.md` § 1a:
            // *not a fifth connection word, and not a blank segment either*). The fault's own word
            // lives in the caller's zone, which this screen's `ctx: prod-eu` does not carry.
            if state.is_none() {
                for never in ["live", "connecting", "disconnected", "login expired"] {
                    assert!(!drawn.contains(never), "{what}: {never:?} in {drawn:?}");
                }
            }
            seen += 1;
        }
    }
    assert_eq!(seen, 18, "a combination stopped being drawn");
}

/// **The tail keeps `screens/widgets.md` § 1a's order in all twelve combinations** — the
/// permission word, then the TLS warning, then `changing…`, each one there exactly when its own
/// fact holds (NOTES § D265 ruling 2). The expected zone is built in that table's order rather
/// than read off a frame, so a warning joined in front of the word, or a mark in front of the
/// warning, fails.
#[test]
fn the_tail_keeps_the_zone_tables_order_whichever_of_its_facts_hold() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut seen = 0;
    for (writes, word) in [
        (Writes::Live, "admin"),
        (Writes::ReadOnly, "read-only"),
        (
            Writes::Unaudited("the audit log could not be opened"),
            "read-only",
        ),
    ] {
        for insecure in [false, true] {
            for running in [false, true] {
                let mut screen = screen(&alerts, &now);
                screen.writes = writes;
                screen.insecure = insecure;
                let app = if running {
                    changing(Some("payments"), "web")
                } else {
                    app()
                };
                let mut zone = format!("ctx: prod-eu · live · {word}");
                if insecure {
                    zone.push_str(" · ⚠ TLS not verified");
                }
                if running {
                    zone.push_str(" · changing…");
                }
                let drawn = rows(&render(&app, &screen))[0].trim_end().to_owned();
                let what = format!("{writes:?} · insecure {insecure} · running {running}");
                assert!(drawn.ends_with(&zone), "{what}: {drawn:?}");
                assert_eq!(
                    drawn.contains("TLS not verified"),
                    insecure,
                    "{what}: {drawn:?}"
                );
                assert_eq!(drawn.contains("changing…"), running, "{what}: {drawn:?}");
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 12, "a combination stopped being drawn");
}

/// **An empty [`Screen::context`] joins nothing** — no leading ` · ` in front of the connection
/// and permission words, whatever follows them (NOTES § D265 ruling 7) — and **the startup
/// picker's zone, which never reads the caller's string, is untouched by one**.
#[test]
fn an_empty_context_joins_nothing_in_front_of_the_tail() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    for (writes, insecure, running, zone) in [
        (Writes::Live, false, false, "live · admin"),
        (
            Writes::ReadOnly,
            true,
            true,
            "live · read-only · ⚠ TLS not verified · changing…",
        ),
    ] {
        let mut screen = screen(&alerts, &now);
        screen.context = Stripped::of("");
        screen.writes = writes;
        screen.insecure = insecure;
        let app = if running {
            changing(Some("payments"), "web")
        } else {
            app()
        };
        let drawn = rows(&render(&app, &screen))[0].trim_end().to_owned();
        assert!(
            drawn.ends_with(&format!("  {zone}")),
            "{writes:?}: {drawn:?}"
        );
        assert!(!drawn.contains("· ·"), "{writes:?}: {drawn:?}");
    }

    let four = contexts_of(FOUR);
    let mut screen = pick_over(&alerts, &now, &[], &four);
    screen.context = Stripped::of("");
    let drawn = rows(&render(&picking(&four, views::Connection::Never), &screen))[0].clone();
    assert!(
        drawn.trim_end().ends_with("  choose a cluster · admin"),
        "{drawn:?}"
    );
}

/// **The header of every link state `screens/states.md` draws is that page's own** — `connecting…`,
/// `⚠ disconnected, retrying` and `⚠ login expired`, each followed by the permission word the page
/// draws after it (NOTES § D265 ruling 6), and `read-only` in its place under `--read-only`. Each
/// frame is drawn in the page's own link state, so a header that drops the word while the link is
/// down fails here.
///
/// **What is compared is the two zones that carry facts, exactly**: the vitals, and the right zone.
/// The caller hands over the page's right zone less its last word, and its vitals. **Nothing is
/// asserted about the centred name**: the page is drawn 70 columns wide and a frame is never drawn
/// below 80, so the two widths cannot say the same thing about it.
#[test]
fn the_header_of_every_link_state_is_the_pages_own() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let zones = |row: &str| {
        let vitals = row
            .trim_start()
            .split("  ")
            .next()
            .unwrap_or_default()
            .to_owned();
        let right = row
            .trim_end()
            .rsplit("  ")
            .next()
            .unwrap_or_default()
            .to_owned();
        (vitals, right)
    };
    let mut found = Vec::new();
    for section in [
        "## Still loading",
        "## The connection dropped",
        "## Your login expired",
    ] {
        let headers: Vec<String> = fenced("states.md", section)
            .into_iter()
            .filter_map(|block| block.first().filter(|row| row.contains("ctx: ")).cloned())
            .collect();
        found.push(headers.len());
        for page in headers {
            assert_eq!(
                page.split("  ")
                    .filter(|zone| !zone.trim().is_empty())
                    .count(),
                2,
                "{section}: {page:?} is not two zones"
            );
            // **The link is the header's own word**, because § *Your login expired* carries a
            // dropped-connection frame of its own since NOTES § D266.
            let link = match &page {
                page if page.contains("disconnected") => Link::Lost,
                page if page.contains("login expired") => Link::Expired,
                page if page.contains("connecting") => Link::Connecting,
                _ => Link::Live,
            };
            let (vitals, zone) = zones(&page);
            let tail = zone
                .strip_suffix(" · admin")
                .unwrap_or_else(|| panic!("{section}: {zone:?} does not end on the word"));
            // **The connection word comes off the page too, and the caller is handed what is in
            // front of it** (todo.md § Phase 12): the header re-derives it from [`Screen::link`],
            // so a header reading it out of the caller's string draws nothing where the page
            // draws `connecting…`, `⚠ disconnected, retrying` or `⚠ login expired`.
            let (head, state) = tail
                .rsplit_once(" · ")
                .unwrap_or_else(|| panic!("{section}: {tail:?} has no connection word"));
            for (writes, word) in [(Writes::Live, "admin"), (Writes::ReadOnly, "read-only")] {
                let mut screen = screen(&alerts, &now);
                screen.vitals = Stripped::of(&vitals);
                screen.context = Stripped::of(head);
                screen.writes = writes;
                screen.link = link;
                let drawn = rows(&render(&app(), &screen))[0].clone();
                println!("{section} · {writes:?}\n{drawn}");
                assert_eq!(
                    zones(&drawn),
                    (vitals.clone(), format!("{head} · {state} · {word}")),
                    "{section} · {writes:?}: {drawn:?}"
                );
                assert!(
                    drawn.starts_with(&format!(" {vitals}")),
                    "{section} · {writes:?} — the vitals lost their pad: {drawn:?}"
                );
            }
        }
    }
    assert_eq!(
        found,
        [1, 1, 5],
        "a link state stopped drawing its header, or grew one"
    );
}

/// **What a caller assembles reaches a cell in its stripped form** (invariant 9, todo.md
/// § Phase 12) — the two header zones, a note paragraph, the clock sentence, the namespace and a
/// command-log line, each of which met no strip on the way in until `views::Stripped`.
///
/// **What is asserted is the substitution and not the removal, because the removal cannot be seen
/// from here** — measured, not reasoned: a frame rendered with the strip turned off draws
/// `pay[2Jments` for `pay\u{202e}\u{1b}[2Jments` exactly as one with it on does, because ratatui's
/// `Buffer` discards a zero-width character as it writes the cell. **So a `TestBackend` assertion
/// about control characters cannot fail**, and one written that way would be a green build proving
/// nothing (NOTES § D26). `screens/widgets.md` § 7's *an escape sequence in a pod name reaches the
/// terminal and rewrites it* is about the write to the real terminal, a layer below anything this
/// file can render into. **That is the other half of why the guarantee is a type**: nothing at this
/// level can catch its absence.
///
/// **A whitespace control character is visible, and that is the one this reads.** `k8s::text` turns
/// a break into a single space rather than deleting it — a boundary deleted glues two words into
/// one — so `pay\nments` draws `pay ments` through the strip and `payments` without it.
///
/// **Three frames, because no single one draws all six**: a list carries the clock banner, a pane
/// that has not answered carries the note, the browser carries the namespace, and the header and
/// the strip carry the rest of each.
#[test]
fn what_a_caller_assembles_reaches_a_cell_stripped() {
    let now = now();
    // **A token no fixture on any of these frames holds**, so the glued form below names this
    // value and not a pod called `payments/web`.
    let crafted = "kra\nken";
    let clean = "kra ken";
    let glued = "kraken";
    let log = [Stripped::of(&format!("$ kubectl get pods -n {crafted}"))];

    // The header's two zones, the command-log strip, and the clock sentence over a list.
    let live = Pane::Ready(vec![oom()]);
    let mut banner = screen(&live, &now);
    banner.vitals = Stripped::of(&format!("nodes {crafted}"));
    banner.context = Stripped::of(&format!("ctx: {crafted}"));
    banner.clock = Some(Stripped::of(&format!("\u{26a0} your clock is {crafted}")));
    banner.log = &log;

    // A pane that has not answered, which is where the caller's paragraphs are drawn.
    let loading = Pane::Loading;
    let note = [Stripped::of(&format!(
        "reading the cluster\u{2026} {crafted}"
    ))];
    let mut waiting = screen(&loading, &now);
    waiting.note = &note;

    // The browser's own title, which is the one place `--namespace` reaches a cell.
    let kinds = [browsable("deployments", true)];
    let ready = Pane::Ready(table("table-deployments"));
    let mut browser = browsing(&ready, &kinds, &now);
    browser.namespace = Some(Stripped::of(crafted));

    let mut seen = 0;
    for (what, app, screen) in [
        ("the header, the clock and the strip", app(), &banner),
        ("a pane that has not answered", app(), &waiting),
        ("the browser's title", opened(), &browser),
    ] {
        let drawn = rows(&render(&app, screen));
        println!("--- {what} ---\n{}\n", drawn.join("\n"));
        for (n, line) in drawn.iter().enumerate() {
            assert!(
                !line.contains(glued),
                "{what}: row {n} glued two words into one, so the break was deleted and not \
                 substituted: {line:?}"
            );
            seen += line.matches(clean).count();
        }
    }
    // **Every field is counted and not merely searched for** — occurrences and not rows, because
    // the header's two zones share one row: vitals and context, the strip, the clock, the note and
    // the browser's title are six, so a field that quietly stopped being drawn fails here instead
    // of passing on a sibling's row.
    assert_eq!(seen, 6, "a caller's string stopped reaching a cell");
}

/// **A vital gives way whole.** `screens/widgets.md` § 1a: *a vital that cannot be read is blank,
/// never guessed* — and half of one is a guess with no marker on it. `nodes 3/3 (40s ago)` clipped
/// to `nodes 3/3 (` reads as a complete count of three ready nodes out of three.
#[test]
fn the_vitals_give_way_whole_and_never_half_a_number() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let stale = "nodes 3/3 (40s ago)";
    let mut screen = screen(&alerts, &now);
    screen.vitals = Stripped::of(stale);
    screen.context = Stripped::of("ctx: prod-eu · ns: payments");
    screen.writes = Writes::ReadOnly;
    screen.insecure = true;
    let zone = "ctx: prod-eu · ns: payments · live · read-only · ⚠ TLS not verified";
    assert_eq!(width(zone), 67, "which leaves 11 columns of left zone");
    assert_eq!(width(stale), 19, "and the vitals want 19");

    let drawn = render(&app(), &screen);
    let header = rows(&drawn)[0].clone();
    assert!(
        header.ends_with(zone),
        "the zone this test measured: {header:?}"
    );
    assert!(!header.contains("nodes"), "no half a vital: {header:?}");

    screen.vitals = Stripped::of("nodes 3/3");
    let drawn = render(&app(), &screen);
    assert!(
        rows(&drawn)[0].contains("nodes 3/3"),
        "and one that fits is drawn"
    );
}

#[test]
fn the_command_log_draws_the_last_two_lines() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = [
        Stripped::of("$ kubectl get certificatesigningrequests"),
        Stripped::of("$ kubectl get pods -A --watch"),
        Stripped::of("$ kubectl get nodes --watch"),
    ];
    let mut with_log = screen(&alerts, &now);
    with_log.log = &log;
    let drawn = render(&app(), &with_log);

    assert!(holds(&drawn, "$ kubectl get pods -A --watch"));
    assert!(holds(&drawn, "$ kubectl get nodes --watch"));
    assert!(
        !holds(&drawn, "certificatesigningrequests"),
        "the strip is two lines, not a scrollback"
    );
}

/// **The feed and the strip, end to end** — the panel is drawn from `views::Log` and not from a
/// list somebody assembled for a test (NOTES § D233, § D257). The three kinds of line meet here:
/// the manifest this run opened with, the read the reader asked for, and the mutation still on
/// the wire with D20's `…` on it.
#[test]
fn the_strip_draws_a_running_mutation_from_the_feed_it_is_given() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut feed = views::Log::default();
    feed.ran("$ kubectl get pods -A --watch".to_owned());
    feed.ran("$ kubectl describe pod web-7d9f4 -n payments".to_owned());
    feed.sent("$ kubectl scale deployment/web --replicas=3 -n payments".to_owned());

    let lines = feed.lines().to_vec();
    let mut with_log = screen(&alerts, &now);
    with_log.log = &lines;
    let drawn = render(&app(), &with_log);
    println!("{}", rows(&drawn).join("\n"));

    assert!(holds(
        &drawn,
        "$ kubectl describe pod web-7d9f4 -n payments"
    ));
    assert!(holds(
        &drawn,
        "$ kubectl scale deployment/web --replicas=3 -n payments   \u{2026}"
    ));
    assert!(
        !holds(&drawn, "get pods -A --watch"),
        "the strip is two lines, not a scrollback"
    );
}

/// **Invariant 4 reached through a layout decision, which is why it took a reviewer to see it.**
/// The strip is 76 columns at the floor — 80 less the frame's two and [`indented`]'s two — and a
/// `Paragraph` with no `Wrap` truncates in silence. A 106-column `get pod` line therefore drew as
/// a **valid, different command**: the same `get pod` without `-o yaml`, which runs, exits 0 and
/// prints a table row instead of the object (`k8s-admin`, 2026-09-07,
/// `reports/2026-09-07-command-log-panel.md`). The line is not wrapped — a wrapped command is a
/// lie (`screens/widgets.md` § 2) — it is cut, behind [`CUT`], the way every other cut on this
/// screen is marked (§ 7).
#[test]
fn a_command_wider_than_the_strip_is_cut_where_the_reader_can_see_it() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    // 106 columns: an 18-column prefix, a 40-character Deployment-generated pod name, ` -n ` and
    // a 14-character namespace put the cut exactly on a token boundary.
    let long = [Stripped::of(
        "$ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst \
         -n payments-prod0 -o yaml --show-managed-fields",
    )];
    assert_eq!(width(long[0].as_str()), 106);
    let mut with_log = screen(&alerts, &now);
    with_log.log = &long;
    let drawn = render(&app(), &with_log);
    let line = row(&drawn, "kubectl get pod");
    println!("{line}");

    assert!(line.contains(STRIP_CUT), "the cut is not marked: {line:?}");
    assert!(
        !line.contains("-n payments-prod0"),
        "the drawn line is a whole, different, working command: {line:?}"
    );
    // **The value gives way inside itself and `-n` is never left naming nothing** — the strip's
    // own rule (`screens/dialogs.md` § The command log's own line): 73 columns before the mark,
    // 62 of them the head and `-n `, so 11 of the namespace.
    assert!(
        line.contains(
            "$ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n payments-pr... "
        ),
        "the head is kept and the value that did not fit gave way inside itself: {line:?}"
    );
}

/// **The one line in `screens/` that does not fit even at the true 76-column floor**, drawn as
/// `screens/detail.md` § A Secret's values, hidden behind an explicit reveal draws it: the flag
/// gives way whole, the mark lands right after `yaml`, and what is left of it is **deliberately** a
/// real command — the `kubectl get -o yaml` a reader already gets by default — which is the trade
/// that section makes and states.
///
/// **The mark is [`STRIP_CUT`], and `screens/detail.md` still draws `…` there** — the page this
/// line is transcribed from predates `screens/widgets.md` § 7's back-cut 3, which gives the strip
/// its own three-period mark because `…` on this strip is the running mark (NOTES § D266). The two
/// pages disagree; this follows § 7 and the running mark, and the drawing is `tui-designer`'s.
#[test]
fn the_yaml_tabs_secret_line_is_cut_exactly_where_the_mockup_draws_it() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let secret = [Stripped::of(&views::yaml_line(
        "secret",
        &ObjectId {
            kind: ObjectKind::Pod,
            namespace: Some("payments".to_owned()),
            name: "db-credentials".to_owned(),
            uid: None,
        },
    ))];
    assert_eq!(
        width(secret[0].as_str()),
        77,
        "77 columns against the strip's 76"
    );
    let mut with_log = screen(&alerts, &now);
    with_log.log = &secret;
    let drawn = render(&app(), &with_log);
    let line = row(&drawn, "kubectl get secret");
    println!("{line}");
    assert!(
        line.contains("$ kubectl get secret db-credentials -n payments -o yaml... "),
        "{line:?}"
    );
    assert!(
        !line.contains("--show-managed"),
        "half a flag still reads as a flag: {line:?}"
    );
}

/// And one that fits is drawn whole, with no marker on it — the mark means *this is not all of
/// it* and a mark on a complete command would be its own lie.
#[test]
fn a_command_that_fits_the_strip_carries_no_mark() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    // 76 columns exactly, which is the whole of the strip at the floor.
    let exact = [Stripped::of(
        "$ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n payments-prod0",
    )];
    assert_eq!(width(exact[0].as_str()), 76);
    let mut with_log = screen(&alerts, &now);
    with_log.log = &exact;
    let drawn = render(&app(), &with_log);
    let line = row(&drawn, "kubectl get pod");
    println!("{line}");
    assert!(line.contains(exact[0].as_str()), "{line:?}");
    assert!(!line.contains(CUT), "{line:?}");
}

/// **The narrow end of [`clipped`], which is what that arm exists for.** One column is room for
/// [`CUT`] and nothing else, and zero is room for nothing at all — so the guard is `<`, not `<=`,
/// and a mutation run found nothing feeding it either side (2026-09-07). It is not reachable
/// through [`command_cut`] at the 80x24 floor, where the strip is 76 columns and below which
/// there is no layout (`screens/widgets.md` § 8); it is reachable through the next caller of a
/// helper this turn made shared, and a guard whose boundary has never been drawn is not one.
#[test]
fn one_column_draws_the_mark_alone_and_zero_draws_nothing() {
    assert_eq!(clipped("kubectl", 1), "\u{2026}");
    assert_eq!(clipped("kubectl", 0), "");
}

/// **The identity cut, every width from whole to nothing** (`screens/widgets.md` § 7, cut 11;
/// NOTES § D266) — its four cases read straight off that section, at the one budget where each
/// begins: whole at 12; the namespace giving way from its front while the `/` and `web` stay
/// whole, down to `…/web` at 5; the name giving way from its front too at 4; and below the two
/// columns `…/` itself takes, the plain front-cut of the whole, which is room for nothing.
///
/// **This used to walk the tail-cut the section replaced** — `payments/…` at 10, `…/w…` at 4 —
/// and a name cut at its tail is where a canary and its stable sibling become one object.
///
/// **Nothing ever comes back wider than it was asked for**, which is the property the whole
/// function exists to keep and the one another branch could silently break — and the bare name and
/// the two degenerate joins are swept beside it.
#[test]
fn the_slash_is_kept_only_while_there_are_two_columns_to_keep_it_in() {
    let web = |columns| name_cut(Some("payments"), "web", columns);
    assert_eq!(web(13), "payments/web");
    assert_eq!(web(12), "payments/web");
    assert_eq!(web(11), "\u{2026}yments/web");
    assert_eq!(web(10), "\u{2026}ments/web");
    assert_eq!(web(6), "\u{2026}s/web");
    assert_eq!(web(5), "\u{2026}/web");
    assert_eq!(web(4), "\u{2026}/\u{2026}b");
    assert_eq!(web(3), "\u{2026}/");
    assert_eq!(web(2), "\u{2026}/");
    assert_eq!(web(1), "");
    assert_eq!(web(0), "");
    assert_eq!(name_cut(None, "node-3", 5), "\u{2026}de-3");
    for columns in 0..=13 {
        for (namespace, name) in [
            (Some("payments"), "web"),
            (None, "node-3"),
            (Some(""), "web"),
            (Some("payments"), ""),
        ] {
            let drawn = name_cut(namespace, name, columns);
            assert!(
                width(&drawn) <= columns,
                "{namespace:?}/{name:?} at {columns} drew {drawn:?}, {} columns",
                width(&drawn)
            );
            assert_eq!(
                drawn.contains('/'),
                namespace.is_some() && columns >= 2,
                "{namespace:?}/{name:?} at {columns}: {drawn:?}"
            );
        }
    }
}

// --- THE SIDEBAR ---

#[test]
fn the_selected_row_carries_the_marker_and_no_other_row_does() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let drawn = render(&app(), &screen(&alerts, &now));
    let rows = rows(&drawn);

    assert!(
        rows.iter().any(|line| line.contains("▸ ALERTS")),
        "ALERTS is selected on startup:\n{}",
        rows.join("\n")
    );
    assert_eq!(
        rows.iter().filter(|line| line.contains('▸')).count(),
        1,
        "exactly one row is marked"
    );
    assert!(
        row(&drawn, "RESOURCES").contains("  RESOURCES"),
        "an unselected row keeps the marker's columns blank"
    );
}

/// Group headers are drawn and never selected — the cursor walks `views::selectable`'s answer,
/// so row 1 of it is the first *group*, not the `RESOURCES` header.
#[test]
fn the_cursor_skips_the_section_headers() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut app = app();
    app.nav.select(1, &[None, None, None]);
    let drawn = render(&app, &screen(&alerts, &now));

    assert!(
        row(&drawn, "workloads").contains("▸  workloads"),
        "the second selectable row is `workloads`, not `RESOURCES`"
    );
    assert!(!row(&drawn, "RESOURCES").contains('▸'));
}

#[test]
fn a_group_lists_its_kinds_only_while_it_is_open() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let kinds = [
        Browsable {
            group: "apps".to_owned(),
            version: "v1".to_owned(),
            kind: "Deployment".to_owned(),
            plural: "deployments".to_owned(),
            namespaced: true,
            verbs: vec!["list".to_owned()],
        },
        Browsable {
            group: "storage.k8s.io".to_owned(),
            version: "v1".to_owned(),
            kind: "StorageClass".to_owned(),
            plural: "storageclasses".to_owned(),
            namespaced: false,
            verbs: vec!["list".to_owned()],
        },
    ];
    let mut with_kinds = screen(&alerts, &now);
    with_kinds.kinds = &kinds;

    let closed = render(&app(), &with_kinds);
    assert!(
        !holds(&closed, "deployments"),
        "every group is closed at startup"
    );

    let mut open = app();
    open.expanded = Some(Group::Workloads);
    let open = render(&open, &with_kinds);
    assert!(
        row(&open, "deployments").contains("     deployments"),
        "a kind sits two columns in from its group"
    );
    assert!(
        !holds(&open, "storageclasses"),
        "and only the open group's kinds are drawn"
    );
}

/// `3 ● 7 ▲` — owners, not pods, and only the bands that have something in them.
#[test]
fn the_alerts_badge_counts_owners_in_the_bands_that_have_one() {
    let now = now();
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0))), cordon(None)]);
    let drawn = render(&app(), &screen(&alerts, &now));
    assert!(
        row(&drawn, "ALERTS").contains("1 ● 2 ▲"),
        "{:?}",
        row(&drawn, "ALERTS")
    );

    let warn_only = Pane::Ready(vec![cordon(None)]);
    let drawn = render(&app(), &screen(&warn_only, &now));
    let alerts = row(&drawn, "ALERTS");
    assert!(alerts.contains("1 ▲"), "{alerts:?}");
    assert!(
        !alerts.contains('●'),
        "no band with nothing in it: {alerts:?}"
    );

    let loading = Pane::Loading;
    let drawn = render(&app(), &screen(&loading, &now));
    let alerts = row(&drawn, "ALERTS");
    assert!(
        !alerts.contains('▲') && !alerts.contains('●'),
        "a pane that has not answered counts nothing: {alerts:?}"
    );
}

/// The badge-glyph rule, both halves: a count draws its band, a duration draws none.
#[test]
fn a_count_badge_takes_a_glyph_and_a_duration_does_not() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let overcommitted = badged("1", Severity::Warn);
    let expiring = badged("30d", Severity::Warn);
    let quiet = Report {
        badge: None,
        ..badged("", Severity::Info)
    };
    let reports = [
        ("capacity", Some(&overcommitted)),
        ("certificates", Some(&expiring)),
        ("drain safety", Some(&quiet)),
    ];
    let mut with_reports = screen(&alerts, &now);
    with_reports.reports = &reports;
    let drawn = render(&app(), &with_reports);

    assert!(
        row(&drawn, "capacity").contains("capacity      1 ▲"),
        "{:?}",
        row(&drawn, "capacity")
    );
    assert!(
        row(&drawn, "certificates").contains("certificates  30d"),
        "a duration says its own unit: {:?}",
        row(&drawn, "certificates")
    );
    assert!(
        !row(&drawn, "drain safety").contains('▲'),
        "and a report with nothing to say draws nothing"
    );
}

// --- THE CARD ---

/// The age is right-aligned and its last column is the card region's — 53 columns in from the
/// pane's left edge, which at 80×24 is the pane's last column less the two-column pad.
#[test]
fn the_age_ends_at_the_card_regions_last_column() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let drawn = render(&app(), &screen(&alerts, &now));
    let line = row(&drawn, "payments/web");

    assert!(
        line.ends_with("4 min ago  │"),
        "two pad columns then the frame: {line:?}"
    );
    assert!(
        line.contains("● payments/web  ·  3 of 5 pods"),
        "the identity line: {line:?}"
    );
}

/// **The selected card is the one ⏎, `r` and `ctrl-d` act on, and it says so** — [`MARKER`] on
/// its identity line and on no other row (`screens/alerts.md` § The selected card, which reversed
/// `screens/widgets.md` § 2's own *"no card carries a selection marker"* once Phase 12's loop made
/// the cursor act).
///
/// **What is asserted is *the mark costs the pane no column*, not that a ▸ is somewhere on the
/// screen.** The gutter it fills is the one every card already left blank, so the same card's
/// glyph, name and age draw in the same columns whether it is the marked one or the one under it —
/// which is why the two frames are compared to each other and not to a literal. A marker paid for
/// out of the card's own width would draw a ▸ just as happily, and rewrap every card by two
/// columns.
#[test]
fn the_marked_card_is_the_selected_one_and_the_mark_costs_no_column() {
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let now = now();
    let keys = [None, None];
    let mut app = app();
    let mut without_the_gutter = Vec::new();

    for chosen in [0, 1] {
        app.content.select(chosen, &keys);
        let drawn = render(&app, &screen(&alerts, &now));
        println!(
            "=== the cursor on card {chosen} ===\n{}",
            rows(&drawn).join("\n")
        );

        let identity = ["payments/web", "node-3"].map(|name| row(&drawn, name));
        for (at, line) in identity.iter().enumerate() {
            let want = match at == chosen {
                true => MARKER.to_owned(),
                false => " ".repeat(width(MARKER)),
            };
            assert!(
                pane(line).starts_with(&want),
                "card {at} with the cursor on card {chosen} drew {:?} in the gutter: {line:?}",
                pane(line).chars().take(width(MARKER)).collect::<String>()
            );
            // The right edge, in both states: the age, the pad, the frame — and nothing between.
            assert!(
                line.ends_with("4 min ago  \u{2502}"),
                "card {at} with the cursor on card {chosen} lost its right edge: {line:?}"
            );
        }
        assert_eq!(
            rows(&drawn)
                .iter()
                .filter(|line| pane(line).contains(MARKER.trim_end()))
                .count(),
            1,
            "a cursor is one fact, and a card is up to twelve rows:\n{}",
            rows(&drawn).join("\n")
        );
        without_the_gutter
            .push(identity.map(|line| pane(&line).chars().skip(width(MARKER)).collect::<String>()));
    }

    assert_eq!(
        without_the_gutter[0], without_the_gutter[1],
        "the gutter is not the only column the mark spends: a card drew differently marked and \
         unmarked, so every card in the pane wrapped at a different width"
    );
}

/// **A card with no age draws none and reserves nothing** — the name gets the whole 51 columns.
///
/// **The name has to be long enough for 51 to be a different answer from 40**, which `node-3` is
/// not: six columns fit either way, so a card that had quietly kept the widest age's 14 columns
/// back would pass. 42 is an ordinary EKS node name, it fits the whole line, and it does not fit
/// beside any age at all (`screens/alerts.md` § The age, and what it costs the name).
#[test]
fn a_card_with_no_age_draws_none_and_keeps_the_whole_line() {
    let now = now();
    let ageless = Pane::Ready(vec![cordon(None)]);
    let drawn = render(&app(), &screen(&ageless, &now));
    let line = row(&drawn, "node-3");

    assert!(line.contains("▲ node-3"), "{line:?}");
    assert!(
        !line.contains("ago"),
        "nothing borrowed from elsewhere: {line:?}"
    );

    let long = "ip-10-0-134-201.eu-west-1.compute.internal";
    assert_eq!(width(long), 42);
    let mut card = cordon(None);
    card.owner.name = long.to_owned();
    let alerts = Pane::Ready(vec![card]);
    let drawn = render(&app(), &screen(&alerts, &now));
    let line = row(&drawn, "ip-10-0-134");

    assert!(
        unmarked(&line).starts_with(&format!("▲ {long}")),
        "the whole name, on a line that reserved nothing: {line:?}"
    );
    // And what follows it is the pad and the frame, nothing else.
    assert!(
        pane(&line).trim_end_matches(['│', ' ']).ends_with(long),
        "the right edge is simply empty: {line:?}"
    );

    let dated = Pane::Ready(vec![cordon(Some(at(0)))]);
    let drawn = render(&app(), &screen(&dated, &now));
    assert!(
        row(&drawn, "node-3").ends_with("4 min ago  │"),
        "the same card with a stamp draws it"
    );
}

/// An ordinary EKS node name is 42 columns: it fits on a card with no age and on no card with
/// one, and when it does not fit the **name** is what gives way.
#[test]
fn a_long_name_clips_and_the_age_is_never_touched() {
    let now = now();
    let long = "ip-10-0-134-201.eu-west-1.compute.internal";
    let mut card = cordon(Some(at(0)));
    card.owner.name = long.to_owned();
    let alerts = Pane::Ready(vec![card]);
    let drawn = render(&app(), &screen(&alerts, &now));
    let line = row(&drawn, "compute.internal");

    assert!(
        line.ends_with("4 min ago  │"),
        "the age keeps its column: {line:?}"
    );
    assert!(
        !line.contains(long),
        "and the 42-column name did not fit beside it: {line:?}"
    );
    // **From its front, marked** — the identity cut (`screens/alerts.md` § The age, and what it
    // costs the name), where a node pool's names differ.
    assert!(
        line.contains("\u{2026}10-0-134-201.eu-west-1.compute.internal  4 min ago"),
        "{line:?}"
    );

    let mut card = cordon(None);
    card.owner.name = long.to_owned();
    let alerts = Pane::Ready(vec![card]);
    let drawn = render(&app(), &screen(&alerts, &now));
    assert!(
        holds(&drawn, long),
        "the same name fits whole on a card with no age"
    );
}

/// `· n of m pods` gives way before the name does: a name half-read still tells you which
/// workload, a fraction with no workload attached tells you nothing.
#[test]
fn the_count_gives_way_before_the_name_clips() {
    let now = now();
    let mut card = oom();
    // `payments/a-deployment-with-a-longer-name` is **40** columns, which is exactly what an age
    // of `4 min ago` leaves: the name alone is the last one that fits whole, and the name plus
    // `  ·  3 of 5 pods` is 16 columns over. A shorter fixture cannot tell `measured + GAP` from
    // `measured * GAP` — measured, that mutant survived a 33-column name.
    card.owner.name = "a-deployment-with-a-longer-name".to_owned();
    let alerts = Pane::Ready(vec![card]);
    let drawn = render(&app(), &screen(&alerts, &now));
    let line = row(&drawn, "a-deployment-with-a-longer");

    assert!(
        !line.contains("3 of 5 pods"),
        "the fraction went first: {line:?}"
    );
    assert!(
        line.contains("payments/a-deployment-with-a-longer-name"),
        "and the whole name stayed: {line:?}"
    );
    assert!(line.ends_with("4 min ago  │"), "{line:?}");
}

/// The evidence is the one thing a card cuts, and the cut is visible.
#[test]
fn the_evidence_is_cut_at_three_lines_and_says_so() {
    let now = now();
    let mut card = oom();
    card.findings[0].evidence = "container nope · image registry.invalid/does-not-exist:v9 · \
         failed to pull and unpack image \
         \"https://registry.invalid/v2/does-not-exist/manifests/v9\": unexpected status from \
         HEAD request, and a great deal more text after it than three lines can hold"
        .to_owned();
    let alerts = Pane::Ready(vec![card]);
    let drawn = render(&app(), &screen(&alerts, &now));

    assert!(
        holds(&drawn, "…"),
        "the cut is marked:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        !holds(&drawn, "a great deal more text"),
        "and what is past the third line is gone"
    );
    assert!(
        holds(&drawn, "container nope"),
        "while the first line is intact"
    );

    // Counted on the screen, not inferred from the marker: the rows between the last line of the
    // title and the action are the evidence, and there are three of them.
    let rows = rows(&drawn);
    let title = rows
        .iter()
        .position(|line| line.contains("(OOMKilled)"))
        .expect("the title's last line");
    let action = rows
        .iter()
        .position(|line| line.contains("→ raise limits.memory"))
        .expect("the action");
    assert_eq!(action - title - 1, 3, "\n{}", rows.join("\n"));
}

/// **The action is never cut**, and neither is the title: only the evidence is.
#[test]
fn the_title_and_the_action_are_drawn_whole() {
    let now = now();
    let mut card = oom();
    card.findings[0].action = "exit 0 says the run ended, not who stopped it — check the pod's \
         events for a Killing line and the node for a memory killer"
        .to_owned();
    let alerts = Pane::Ready(vec![card]);
    let drawn = render(&app(), &screen(&alerts, &now));

    assert!(
        holds(&drawn, "  → exit 0 says the run ended"),
        "the arrow costs the first line two columns:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        holds(&drawn, "node for a memory killer"),
        "and the last word is on screen"
    );
    assert!(
        holds(&drawn, "killed by the kernel (OOMKilled)"),
        "the title is drawn whole too"
    );
}

/// An empty evidence draws no line at all — not a blank one, which is a hole in the card.
#[test]
fn an_empty_evidence_leaves_the_line_out() {
    let now = now();
    let mut card = oom();
    card.findings[0].evidence = String::new();
    let alerts = Pane::Ready(vec![card]);
    let drawn = render(&app(), &screen(&alerts, &now));
    let rows = rows(&drawn);
    let title = rows
        .iter()
        .position(|line| line.contains("(OOMKilled)"))
        .expect("the title's last line");

    assert!(
        rows[title + 1].contains("→ raise limits.memory"),
        "the action follows the title with nothing between:\n{}",
        rows.join("\n")
    );
}

/// Severity is a symbol **and** a colour, never colour alone — and both come from
/// `theme::band`, so the two carriers cannot disagree.
#[test]
fn a_severity_is_a_symbol_and_a_colour() {
    let now = now();
    let alerts = Pane::Ready(vec![oom(), cordon(None)]);
    let drawn = render(&app(), &screen(&alerts, &now));
    let rows = rows(&drawn);

    let critical = rows
        .iter()
        .position(|line| line.contains("payments/web"))
        .expect("the OOM card");
    let warn = rows
        .iter()
        .position(|line| line.contains("node-3"))
        .expect("the cordon card");
    // The gutter: two columns in from the pane's left edge, which is column 22 + 2.
    let gutter = 24;
    assert_eq!(column(&rows[critical], gutter), '●');
    assert_eq!(column(&rows[warn], gutter), '▲');
    assert_eq!(
        drawn.cell((gutter as u16, critical as u16)).unwrap().fg,
        ink(theme::CRITICAL, Depth::TrueColor)
    );
    assert_eq!(
        drawn.cell((gutter as u16, warn as u16)).unwrap().fg,
        ink(theme::WARN, Depth::TrueColor)
    );
}

/// At sixteen colours the same two are still told apart, by the glyph and by an indexed colour —
/// which is what the fallback is for.
#[test]
fn the_bands_survive_a_sixteen_colour_terminal() {
    let now = now();
    let alerts = Pane::Ready(vec![oom()]);
    let mut plain = screen(&alerts, &now);
    plain.depth = Depth::Ansi16;
    let drawn = render(&app(), &plain);

    assert!(
        holds(&drawn, "● payments/web"),
        "the glyph is unconditional"
    );
    assert_eq!(drawn.cell((24, 2)).unwrap().fg, Color::Indexed(1));
}

// --- THE THREE STATES ---

/// *Nothing came back yet* is not *there is nothing* is not *we were not allowed to look*, and
/// none of the three says `403` or the word RBAC.
#[test]
fn loading_empty_and_denied_are_three_different_screens() {
    let now = now();
    let note = [Stripped::of(
        "84 pods and 3 nodes checked, none of them is in trouble right now.",
    )];

    let loading = Pane::Loading;
    let mut still = screen(&loading, &now);
    let reading = [Stripped::of("reading the cluster… 2,140 pods")];
    still.note = &reading;
    let still = render(&app(), &still);
    assert!(holds(&still, "reading the cluster…"));
    assert!(
        !holds(&still, "nothing is broken"),
        "a list that has not answered does not claim to be empty"
    );

    let empty = Pane::Ready(Vec::new());
    let mut clean = screen(&empty, &now);
    clean.note = &note;
    let clean = render(&app(), &clean);
    assert!(holds(&clean, "○  nothing is broken"));
    assert!(holds(&clean, "84 pods and 3 nodes checked"));

    // **A blank line goes between the paragraphs and nowhere else** — not above the first one,
    // which would push the block off its own centre, and not nowhere, which runs two sentences
    // together.
    let two = [
        Stripped::of("84 pods and 3 nodes checked."),
        Stripped::of("Worth a look anyway:"),
    ];
    let mut spaced = screen(&empty, &now);
    spaced.note = &two;
    let spaced = render(&app(), &spaced);
    let drawn = rows(&spaced);
    let first = drawn
        .iter()
        .position(|line| line.contains("84 pods and 3 nodes checked."))
        .expect("the first paragraph");
    let second = drawn
        .iter()
        .position(|line| line.contains("Worth a look anyway:"))
        .expect("the second");
    assert_eq!(
        second - first,
        2,
        "one blank between:\n{}",
        drawn.join("\n")
    );
    let headline = drawn
        .iter()
        .position(|line| line.contains("nothing is broken"))
        .expect("the headline");
    assert_eq!(first - headline, 2, "one blank under the headline, not two");

    // Refused, and nothing came back with it — which is not `Ready(vec![])` and never reaches
    // `nothing is broken`.
    let refused = Pane::Denied(
        "You can't list pods across the whole cluster, so k8rs is showing the namespace your \
         kubeconfig points at: payments."
            .to_owned(),
        Vec::new(),
    );
    let refused = render(&app(), &screen(&refused, &now));

    // A caller with nothing to say still gets a sentence rather than a blank pane — and the
    // fallback is the *loading* pane's alone: an empty list with no note still says only that it
    // is empty, never that it is still reading.
    let silent = render(&app(), &screen(&Pane::Loading, &now));
    assert!(holds(&silent, "reading the cluster…"));
    assert!(!holds(&silent, "nothing is broken"));
    let wordless = render(&app(), &screen(&Pane::Ready(Vec::new()), &now));
    assert!(holds(&wordless, "○  nothing is broken"));
    assert!(!holds(&wordless, "reading the cluster…"));

    for state in [&still, &clean, &refused] {
        println!("{}\n", rows(state).join("\n"));
    }
    assert!(holds(&refused, "You can't list pods across the whole"));
    assert!(!holds(&refused, "nothing is broken"));
    for word in ["403", "RBAC", "forbidden"] {
        for state in [&still, &clean, &refused] {
            assert!(!holds(state, word), "no state prints {word:?}");
        }
    }
}

/// **A card holds every finding filed under one owner, draws the one that decides it, and counts
/// the rest** (`screens/alerts.md` § A card with more than one finding).
///
/// Severity settles this one outright: the memory kill is Critical and the readiness failure a
/// Warning. The right edge belongs to the *newer* of the two and that is not a mismatch — the age
/// answers *when did something last happen to this owner*, which is `Card::age`'s own reading.
#[test]
fn a_second_finding_is_counted_under_the_action_and_never_drawn_beside_it() {
    let now = now();
    let mut card = oom();
    card.findings.push(Finding {
        timestamp: Some(at(200)),
        ..finding(
            Severity::Warn,
            "Running, but not receiving traffic — the readiness check is failing",
            "",
            "check the app's /healthz endpoint",
        )
    });
    let alerts = Pane::Ready(vec![card]);
    let drawn = render(&app(), &screen(&alerts, &now));
    println!("{}", rows(&drawn).join("\n"));

    assert!(
        holds(&drawn, "Containers exceeded their memory limit"),
        "the worst severity is what the four parts are drawn from"
    );
    assert!(
        !holds(&drawn, "readiness check is failing"),
        "and the other one is one ⏎ away, not on the card"
    );
    assert!(
        holds(&drawn, "1 more problem — ⏎ to see"),
        "singular at one"
    );
    // Two columns of card pad, then the two the title and the evidence also indent by — and
    // nothing else, which is what *no glyph precedes it* looks like on a rendered row.
    assert!(
        pane(&row(&drawn, "more problem")).starts_with("    1 more problem — ⏎ to see"),
        "under the title, not under the arrow, and no glyph before it: {:?}",
        row(&drawn, "more problem")
    );
    assert!(
        row(&drawn, "payments/web").ends_with("40s ago  │"),
        "the right edge is the newest drawable event on the card, whichever finding it belongs to"
    );

    // Two hidden findings take the plural, and a card with one finding has no fifth part at all —
    // it is absent, never a blank placeholder.
    let mut three = oom();
    three
        .findings
        .push(finding(Severity::Warn, "second", "", "do a thing"));
    three
        .findings
        .push(finding(Severity::Warn, "third", "", "do another"));
    let alerts = Pane::Ready(vec![three]);
    let drawn = render(&app(), &screen(&alerts, &now));
    assert!(
        holds(&drawn, "2 more problems — ⏎ to see"),
        "plural past one"
    );

    let alone = Pane::Ready(vec![oom()]);
    let drawn = render(&app(), &screen(&alone, &now));
    assert!(!holds(&drawn, "more problem"), "one finding, four parts");
}

/// **Where severity ties, recency breaks it — not `analyze()`'s order.**
///
/// `screens/alerts.md` draws the case: a node carrying both N2 (cordoned) and N3 (running low on
/// memory), both `▲`. N2 is declared before N3 in `rules.rs`, so *first in that order* would draw
/// the cordon — the older and, on this node, the less urgent of the two — and nothing about which
/// rule number fired first is a fact a reader can reconstruct.
///
/// **And *recent* is [`Finding::age`] answering, not a timestamp existing.** A stamp more than the
/// skew allowance into the future draws no age at all, so it cannot be the most recent thing on a
/// card either — the same test `views::Card::newest` applies to the right edge.
#[test]
fn a_tie_on_severity_is_broken_by_the_most_recent_drawable_finding() {
    let now = now();
    let low = |stamp| Finding {
        timestamp: stamp,
        owner: id(ObjectKind::Node, None, "node-3"),
        object: id(ObjectKind::Node, None, "node-3"),
        ..finding(
            Severity::Warn,
            "This node is running low on memory — Kubernetes may start evicting pods to free it up",
            "",
            "free up memory on this node, or move some pods elsewhere",
        )
    };

    let mut recent = cordon(Some(at(0)));
    recent.findings.push(low(Some(at(180))));
    let alerts = Pane::Ready(vec![recent]);
    let drawn = render(&app(), &screen(&alerts, &now));
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        holds(&drawn, "running low on memory"),
        "the newer of the two"
    );
    assert!(
        !holds(&drawn, "refuses new pods"),
        "the cordon is the one behind ⏎: {}",
        rows(&drawn).join("\n")
    );
    assert!(holds(&drawn, "1 more problem — ⏎ to see"));

    // The same pair, with the memory finding stamped ten minutes into the reader's future: it
    // draws no age, so it is not the most recent thing here and the cordon is drawn instead.
    let mut ahead = cordon(Some(at(0)));
    ahead.findings.push(low(Some(at(840))));
    let alerts = Pane::Ready(vec![ahead]);
    let drawn = render(&app(), &screen(&alerts, &now));
    assert!(
        holds(&drawn, "refuses new pods"),
        "a future stamp is not recency"
    );
    assert!(!holds(&drawn, "running low on memory"));
    assert!(
        row(&drawn, "node-3").ends_with("4 min ago  │"),
        "and the right edge is the drawable one: {:?}",
        row(&drawn, "node-3")
    );

    // Neither has a drawable age: the screen file promises nothing about which is shown, so this
    // asserts only that a card still draws a finding and still counts the other.
    let mut neither = cordon(None);
    neither.findings.push(low(None));
    let alerts = Pane::Ready(vec![neither]);
    let drawn = render(&app(), &screen(&alerts, &now));
    assert!(
        holds(&drawn, "refuses new pods") || holds(&drawn, "running low on memory"),
        "a card with no drawable age still draws a finding"
    );
    assert!(holds(&drawn, "1 more problem — ⏎ to see"));
}

/// **A refusal is a banner over the list, never instead of it.**
///
/// `screens/states.md` § *You can only see some namespaces* draws the sentence above the cards,
/// with `3 ● 7 ▲` still on the sidebar and `● payments/web  ·  3 of 5 pods    4 min ago` still on
/// the pane. § *Your login expired* says it in prose: *"Stale data stays visible and stays
/// labelled… k8rs does not clear the screen because it lost its token."*
#[test]
fn a_refusal_is_a_banner_over_the_list_and_does_not_clear_it() {
    let now = now();
    let refused = Pane::Denied(
        "You can't list pods across the whole cluster, so k8rs is showing the namespace your \
         kubeconfig points at: payments."
            .to_owned(),
        vec![oom(), cordon(Some(at(0)))],
    );
    let drawn = render(&app(), &screen(&refused, &now));
    println!("{}", rows(&drawn).join("\n"));
    let lines = rows(&drawn);
    let at_row = |needle: &str| {
        lines
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", lines.join("\n")))
    };

    let banner = at_row("You can't list pods across the whole");
    let card = at_row("payments/web");
    assert!(banner < card, "the banner is above the list, not below it");
    // The sidebar row beside the blank one is not this test's business.
    assert!(
        pane(&lines[card - 1])
            .chars()
            .all(|cell| cell == ' ' || cell == '│'),
        "one blank row between them: {:?}",
        lines[card - 1]
    );
    assert!(
        row(&drawn, "payments/web").ends_with("4 min ago  │"),
        "and the card is drawn whole, right edge included"
    );
    assert!(
        holds(&drawn, "node-3"),
        "every card that did come back, not just the first"
    );
    assert!(
        row(&drawn, "ALERTS").contains("1 ● 1 ▲"),
        "the badge counts what did come back: {:?}",
        row(&drawn, "ALERTS")
    );
    assert!(
        !holds(&drawn, "nothing is broken"),
        "a refusal is the one moment k8rs cannot claim the cluster is clean"
    );
}
/// **One screen `screens/states.md` draws** — the pane side of its body rows, and the footer under
/// them.
///
/// **The pane side and not the whole row**, because the sidebar a mockup draws is that file's
/// illustration of a sidebar and not this box's subject; what the nine states are about is what the
/// content pane says and which keys the footer offers.
struct Mockup {
    /// The footer, frame stripped.
    footer: String,
    /// One entry per body row, pane side, trimmed — blank rows kept, because they are what
    /// separates one paragraph from the next.
    pane: Vec<String>,
}

/// **Every fenced block of one `##` section that draws a whole screen** — a block with no footer is
/// a fragment (the two narrow ones under § *An empty kind*) or a stderr message (§ *Before the TUI
/// ever starts*), and neither is a state this file draws.
///
/// **It reads the file rather than asserting against it**, so
/// [`every_state_draws_the_body_and_the_footer_its_own_mockup_gives_it`] can ask *which sections
/// draw a screen at all* and refuse to leave one untested — where [`fenced`] panics on a section
/// with no block, which is right for its own callers and wrong for a sweep.
fn mockups(section: &str) -> Vec<Mockup> {
    let path = format!("{}/screens/states.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the screen file {path} could not be read: {e}"));
    let mut drawn = Vec::new();
    let mut open: Option<(Vec<String>, Option<String>, bool)> = None;
    for line in text
        .lines()
        .skip_while(|line| *line != section)
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
    {
        let fence = line.starts_with("```");
        match (&mut open, fence) {
            (None, false) => {}
            (None, true) => open = Some((Vec::new(), None, true)),
            (Some((pane, footer, body)), false) => {
                if line.contains("q quit") {
                    *footer = Some(unframed(line));
                } else if line.starts_with('├') {
                    *body = false;
                } else if *body && line.starts_with('│') {
                    // The last cell of the row is the content pane; a row with no sidebar has one
                    // cell and this is still it.
                    let cell = line
                        .trim_end()
                        .trim_end_matches('│')
                        .rsplit('│')
                        .next()
                        .unwrap_or("");
                    pane.push(cell.trim().to_owned());
                }
            }
            (Some(_), true) => {
                let (pane, footer, _) = open.take().expect("open");
                if let Some(footer) = footer {
                    drawn.push(Mockup { footer, pane });
                }
            }
        }
    }
    drawn
}

/// **The paragraphs above the list** — consecutive non-blank rows grouped, each collapsed to single
/// spaces, and the walk stopped at the first card so a mockup that draws a card in outline is not
/// compared with the four lines a real one has.
///
/// **`●` and `▲` end it and `○` does not**: the first two are a card's own severity, and the third
/// is the calm block's headline, which is one of the paragraphs being compared
/// (`screens/README.md` § the five rules, item 4).
fn said_above(pane: &[String]) -> Vec<String> {
    let mut paragraphs: Vec<String> = Vec::new();
    for row in pane.iter().take_while(|row| {
        // **The marker is stripped before the glyph is looked for** — `screens/` now draws the
        // selected card with [`MARKER`] in front of its band (`screens/alerts.md`, 2026-09-24), so
        // a row that starts `▸ ●` is still the first card and still ends the paragraphs above it.
        // Without this the card's own text was swept into them and pushed the banner being measured
        // off the pane (measured: § You can only see some namespaces came back with `▸ ●
        // payments/web · 3 of 5 pods …` appended to its second paragraph).
        let row = row.strip_prefix(MARKER).unwrap_or(row);
        !row.starts_with('●') && !row.starts_with('▲')
    }) {
        let words = row.split_whitespace().collect::<Vec<_>>().join(" ");
        match (words.is_empty(), paragraphs.last_mut()) {
            (true, _) => paragraphs.push(String::new()),
            (false, Some(last)) if !last.is_empty() => {
                last.push(' ');
                last.push_str(&words);
            }
            (false, _) => {
                paragraphs.pop();
                paragraphs.push(words);
            }
        }
    }
    paragraphs.retain(|paragraph| !paragraph.is_empty());
    paragraphs
}

/// **The whole content pane as one run of words** — for a needle that wraps, which [`holds`]
/// cannot see because it reads one row at a time.
fn body_text(drawn: &Buffer) -> String {
    rows(drawn)[2..18]
        .iter()
        .flat_map(|row| {
            pane(row)
                .trim_end_matches('│')
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// **A sentence as [`body_text`] reads one** — every run of whitespace one space — so a sentence
/// handed to the renderer can be searched for across the rows it wrapped onto.
fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// [`said_above`] over a rendered frame — the sixteen body rows, pane side.
fn drawn_above(drawn: &Buffer) -> Vec<String> {
    let pane: Vec<String> = rows(drawn)[2..18]
        .iter()
        .map(|row| pane(row).trim_end_matches('│').trim().to_owned())
        .collect();
    said_above(&pane)
}

/// **The mockup's own paragraphs, as the sentences a caller would hand over** — so a fixture is
/// read out of `screens/states.md` rather than typed beside it, and a mockup that is edited feeds
/// the edit straight into the screen this test draws.
/// **Nothing is stripped on the way through, [`CUT`] included.** `reading the cluster…` and
/// `Retrying…` are sentences with an ellipsis in them and a capped banner ends on the same
/// character, so no rule here can tell the two apart — which is why [`against`] is pointed only at
/// mockups that draw their sentences whole, and the capped ones are
/// [`a_stack_of_caveats_never_takes_the_list_below_its_own_floor`]'s.
fn fed(section: &str, nth: usize, from: usize) -> Vec<Stripped> {
    paragraphs(section, nth, from)
        .iter()
        .map(|line| Stripped::of(line))
        .collect()
}

/// The same paragraphs, still as `String`s — what a `Pane::Denied` reason and a clock sentence are
/// built from, both of which are one string and not a list of them.
fn paragraphs(section: &str, nth: usize, from: usize) -> Vec<String> {
    said_above(&mockups(section)[nth].pane).split_off(from)
}

/// The same paragraphs as the one string a banner is.
fn joined(section: &str, nth: usize, from: usize) -> String {
    paragraphs(section, nth, from).join("\n\n")
}

/// **What every one of the nine owes its own mockup**: the footer byte for byte, the paragraphs
/// above the list word for word, and each mutating key on the screen exactly where the mockup's own
/// footer draws it and nowhere else.
///
/// **Which keys a frame offers is read off that footer and not written here.** Every mockup once
/// withheld `s` and `r`, and this helper asserted it for all of them; § *You can only see some
/// namespaces* now draws both live, because a namespace scope is not a permission (re-ruled
/// 2026-09-12), and a list of withheld keys kept here would have gone on asserting the old page.
///
/// **The paragraphs are compared and not merely searched for.** The half that decides this is the
/// rows the *code* writes — `○  nothing is broken`, `reading the cluster…`, `no jobs in this
/// cluster` — which no fixture hands over and which a wrong screen file therefore fails on. The
/// rest of a paragraph is a caller's sentence read out of the same mockup, so what it proves is
/// narrower and worth saying: that this renderer draws the sentence it was handed, whole, in the
/// order and the position the file draws it, above the list and never instead of it.
fn against(section: &str, nth: usize, app: &App, screen: &Screen) {
    let drawn = render(app, screen);
    println!("{section} [{nth}]\n{}\n", rows(&drawn).join("\n"));
    let mockup = &mockups(section)[nth];
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        mockup.footer,
        "{section} [{nth}] — the footer"
    );
    assert_eq!(
        drawn_above(&drawn),
        said_above(&mockup.pane),
        "{section} [{nth}] — the paragraphs above the list"
    );
    for key in ["s scale", "r restart", "s no scale", "r no restart"] {
        assert_eq!(
            holds(&drawn, key),
            mockup.footer.contains(key),
            "{section} [{nth}] — {key:?} is drawn exactly where the mockup's footer puts it"
        );
    }
}

/// **The nine states of `screens/states.md`, each drawn at 80×24 against its own mockup.**
///
/// **`s scale` and `r restart` are on none of them and are marked `no` on none of them either** —
/// the two halves of that file's own rule, and the distinction `screens/widgets.md` § 2a keeps:
/// **withheld** is the key not being on the line at all, **refused** is the key on the line with
/// `no` before its label, and `no` stays reserved for a `may_i` verdict about this login
/// (NOTES § D261). The ordinary screen that is none of them is
/// [`the_ordinary_screen_is_the_only_one_that_offers_a_mutating_key`], the must-not-fire half.
///
/// **The list is the file's, not this test's, and it is counted in *mockups* and not in sections.**
/// A hand-written list of eight closed over whatever the file happened to hold when it was written,
/// so the sweep was made to ask the file instead — but it asked which `##` *sections* draw a
/// screen, and a `###` subsection is inside its `##` parent as far as [`mockups`] reads. Every
/// frame after the first in a section it had already ticked was therefore invisible to it: when
/// `screens/states.md` grew § *Over a pane with nothing to show yet* under § *Your login expired*,
/// two new screens arrived and the sweep stayed green (`dev-ui`, 2026-09-12). Twenty frames carry
/// a footer today — twenty-two since § *Alerts had already found nothing, and then the link went*
/// (NOTES § D266) — and each one is either drawn here or named below with the test that owns it.
#[test]
fn every_state_draws_the_body_and_the_footer_its_own_mockup_gives_it() {
    let now = now();
    let mut seen: Vec<(&str, usize)> = Vec::new();
    let live_cards = Pane::Ready(vec![oom()]);

    // § Still loading — nothing has arrived, so there is nothing to move across, open or narrow.
    // The count in the sentence is the store's; `reading the cluster…` is `ui::note`'s own.
    let section = "## Still loading";
    let loading = Pane::Loading;
    let reading = fed(section, 0, 0);
    let mut still = screen(&loading, &now);
    still.note = &reading;
    // **The page's own connection word, now that [`header`] joins it from [`Screen::link`]** — this
    // state is the moment before the first answer, which `screens/states.md` draws as
    // `ctx: prod-eu · connecting… · admin`. Drawn at the default [`Link::Live`] the printed frame
    // said `live` under a mockup that says otherwise (`tester`, 2026-09-19). The `nodes 3/3` half
    // of that row is a separate, older gap and is not this box's.
    still.link = Link::Connecting;
    against(section, 0, &app(), &still);
    seen.push((section, 0));

    // § Nothing is broken — an empty Alerts list keeps the cursor keys, because the sidebar's own
    // rows have finished filling in, and withholds the two that need a selection. The headline is
    // the code's; the three paragraphs under it are the caller's.
    let section = "## Nothing is broken";
    let empty = Pane::Ready(Vec::new());
    let counted = fed(section, 0, 1);
    let mut clean = screen(&empty, &now);
    clean.note = &counted;
    against(section, 0, &app(), &clean);
    seen.push((section, 0));

    // § An empty kind in the browser — the one state that keeps `/ filter` and loses the cursor
    // keys with the rows. **The kind is read out of the mockup's own title row**, so
    // `no jobs in this cluster` is composed by `ui::empty` from the file's word and compared with
    // the file's sentence: nothing in this state is a literal this test also wrote.
    let section = "## An empty kind in the browser";
    let titled = said_above(&mockups(section)[0].pane);
    let kinds = [browsable(&titled[0], true)];
    let none = Pane::Ready(crate::k8s::Table::default());
    against(section, 0, &opened(), &browsing(&none, &kinds, &now));
    seen.push((section, 0));

    // § The filter hides every row — the third reason a list can be empty, and the only state on
    // this page whose sentence is about what the *reader* typed rather than about the cluster.
    // **Both panes are handed a list that has rows in it**, because that is the whole of what
    // separates this state from the two above: a filter that hid something. The sentence is
    // `ui::hidden`'s and the footer is the one line on this page that can afford `esc`'s own word.
    let section = "## The filter hides every row";
    let mut typed = app();
    typed.filters.text = filter("prodeu");
    against(section, 0, &typed, &screen(&live_cards, &now));
    seen.push((section, 0));

    let named = said_above(&mockups(section)[1].pane);
    let plural = named[0]
        .split_whitespace()
        .next()
        .expect("the mockup's title row names a kind");
    let deployments = [browsable(plural, true)];
    let listed = Pane::Ready(table("table-deployments"));
    let mut narrowed = browsing(&listed, &deployments, &now);
    narrowed.namespace = Some(Stripped::of("payments"));
    let mut browsing_typed = opened();
    browsing_typed.filters.text = filter("prodeu");
    against(section, 1, &browsing_typed, &narrowed);
    seen.push((section, 1));

    // § The connection dropped — a stale card is still a selected object in principle, and k8rs
    // cannot ask whether a write would be allowed, so the two keys go rather than being marked.
    let section = "## The connection dropped";
    let dropped = Pane::Denied(joined(section, 0, 0), vec![oom()]);
    let mut lost = screen(&dropped, &now);
    lost.link = Link::Lost;
    against(section, 0, &app(), &lost);
    seen.push((section, 0));

    // § Your login expired — the one state on the page that promotes a key off `?`.
    let section = "## Your login expired";
    let timed_out = Pane::Denied(joined(section, 0, 0), vec![oom()]);
    let mut expired = screen(&timed_out, &now);
    expired.link = Link::Expired;
    against(section, 0, &app(), &expired);
    seen.push((section, 0));

    // § Alerts had already found nothing, and then the link went — the two banners above over an
    // empty list, each with the closing line that is true when nothing is under it. **The sentence
    // is the caller's over `Pane::Denied`, as it is over a stale list**; what is this file's is
    // that no `○  nothing is broken` is drawn under either, which
    // [`an_empty_alerts_list_says_nothing_is_broken_only_while_the_link_is_live`] asserts of a
    // `Pane::Ready` too (NOTES § D266).
    for (nth, link) in [(1, Link::Lost), (2, Link::Expired)] {
        let went = Pane::Denied(joined(section, nth, 0), Vec::new());
        let mut empty_and_gone = screen(&went, &now);
        empty_and_gone.link = link;
        against(section, nth, &app(), &empty_and_gone);
        seen.push((section, nth));
    }

    // § Over a pane with nothing to show yet — the same expired login over the two panes that have
    // no card to relabel as stale. **`X` is promoted wherever the link is `Link::Expired`, whatever
    // the pane under it is drawing**, which is what separates it from `s` and `r`: it never acted
    // on a selected object, so *nothing is selected* was never its condition. Each pane otherwise
    // keeps its own shape — Still loading still drops the cursor keys, the empty kind still keeps
    // `/ filter`. **Each frame is fed its own mockup and not the one it resembles**, so an edit to
    // either drawing arrives here rather than being masked by the section it was copied from.
    let waited = fed(section, 3, 0);
    let mut waiting = screen(&loading, &now);
    waiting.note = &waited;
    waiting.link = Link::Expired;
    against(section, 3, &app(), &waiting);
    seen.push((section, 3));

    let named = said_above(&mockups(section)[4].pane);
    let open_kind = [browsable(&named[0], true)];
    let mut bare = browsing(&none, &open_kind, &now);
    bare.link = Link::Expired;
    against(section, 4, &opened(), &bare);
    seen.push((section, 4));

    // § Your computer's clock is off — a banner over a *live* list: the header still reads
    // `live · admin`, so this is neither the connection's reason nor a permission's, and the keys
    // go because an age that reads fresher than it is decides which card somebody acts on first.
    let section = "## Your computer's clock is off";
    let live = Pane::Ready(vec![oom()]);
    let behind = joined(section, 0, 0);
    let mut skewed = screen(&live, &now);
    skewed.clock = Some(Stripped::of(&behind));
    against(section, 0, &app(), &skewed);
    seen.push((section, 0));

    // § Ahead of the cluster — the other direction, and a second sentence rather than the same one
    // with a sign flipped: this one loses no times, it only makes them read large (NOTES § D177).
    let ahead = joined(section, 1, 0);
    let mut fast = screen(&live, &now);
    fast.clock = Some(Stripped::of(&ahead));
    against(section, 1, &app(), &fast);
    seen.push((section, 1));

    // § Nothing is broken, and the clock is still off — the skew joins `ui::note`'s own paragraphs
    // instead of bringing a banner, because *clock skew is drawn in whichever family the rest of
    // the screen is already in*. The calm pane's keys are the calm pane's.
    let calm = fed(section, 2, 1);
    let mut quiet = screen(&empty, &now);
    quiet.note = &calm;
    against(section, 2, &app(), &quiet);
    seen.push((section, 2));

    // § Your clock and a scoped namespace together is the capped frame — its last banner ends on
    // `…`, which [`fed`]'s own doc rules out of [`against`]'s reach. What it draws is the 13-of-16
    // budget rather than a state of its own, and that rule is
    // [`a_stack_of_caveats_never_takes_the_list_below_its_own_floor`]'s — over sentences of its own
    // length, **not this mockup's words**, which no test compares.
    seen.push((section, 3));

    // § You can only see some namespaces — an ordinary `live · admin` session that happens to be
    // scoped, so `s scale` and `r restart` are live: **a namespace scope is not a permission**
    // (re-ruled 2026-09-12). This mockup read `read-only` once and this frame was fed
    // `Writes::ReadOnly` to match it, which is the wiring that section now names as the mistake.
    let section = "## You can only see some namespaces";
    let scoped = Pane::Denied(joined(section, 0, 0), vec![oom()]);
    let partial = screen(&scoped, &now);
    against(section, 0, &app(), &partial);
    seen.push((section, 0));

    // § *The same screen, four ways it can differ* → #### Nodes and deployments both refused — the
    // frame added on 2026-09-24, and **the one frame on this page this sweep visits without
    // comparing**, for the reason `## Before the TUI ever starts` is visited without one further
    // down: there is nothing here that can draw it.
    //
    // **Its footer is `↑↓ move  ⏎ open  / filter  ? all keys  q quit` over a card whose owner
    // is a Deployment**, and nothing in `ui::offered`'s five facts withholds `r` for a refused
    // *workload* watch — while the sibling mockup two screens up insists in as many words that `r
    // restart` stays and rides on *"the connection and the audit log, never on which namespace"*.
    // So either that footer is a rule with no home yet or it is an oversight in a new drawing, and
    // this box does not get to pick: asserting the paragraphs alone would leave the footer
    // untested, and asserting the code's own answer would be pinning the implementation against its
    // own specification (CLAUDE.md § Tests must not lie). Reported instead, and ticked here so the
    // coverage sweep below stays honest about what it has and has not compared.
    seen.push((section, 1));

    // § Nothing broken, and something not checked — the same sentence with no list under it, which
    // is the one screen where silence and *nothing is broken* would look identical. The check that
    // could not run says so beneath the verdict rather than instead of it.
    let unchecked = fed(section, 2, 1);
    let mut said_anyway = screen(&empty, &now);
    said_anyway.note = &unchecked;
    against(section, 2, &app(), &said_anyway);
    seen.push((section, 2));

    // § The audit log could not be opened — `ops::audit_log`'s own sentence over a live list, and
    // the state this phase owns outright (NOTES § D21, § D231).
    let section = "## The audit log could not be opened";
    let dead = dead_log();
    let mut unaudited = screen(&live, &now);
    unaudited.writes = Writes::Unaudited(&dead);
    against(section, 0, &app(), &unaudited);
    seen.push((section, 0));

    // § And when the clock is off at the same time, § And when a namespace is all you can see, too,
    // § And when the login has also expired and § All three at once are the rank, not four states:
    // who stays whole, who is cut and who is absent when banners queue.
    // [`the_audit_sentence_gives_way_first_and_the_pane_s_own_reason_never_does`] draws the last
    // three from the page's own uncut sentences, and
    // [`a_stack_of_caveats_never_takes_the_list_below_its_own_floor`] the first. **Neither compares
    // these mockups word for word, and that is a measurement and not a shortcut**: three of the
    // four are drawn at 70 columns, below the 80×24 floor, and the fourth wraps at 55 where the
    // card region is 53 — so where each one breaks, and at 80 whether the audit sentence is cut at
    // all, is not what the floor draws.
    for nth in 1..=4 {
        seen.push((section, nth));
    }

    // § On a healthy or a still-loading Alerts screen — the two panes that draw no list, and the
    // only two where the audit sentence joins the calm block instead of a banner. **It is passed by
    // `ui::content`'s Alerts arm, not by the caller**, so only `Screen::writes` is set here and the
    // caller's paragraphs are the whole of what each screen is handed.
    // [`the_dead_write_path_says_so_on_a_pane_that_draws_no_list`] owns the order and the blank
    // rows this comparison does not look at.
    let checked = fed(section, 5, 2);
    let mut healthy = screen(&empty, &now);
    healthy.note = &checked;
    healthy.writes = Writes::Unaudited(&dead);
    against(section, 5, &app(), &healthy);
    seen.push((section, 5));

    // Still loading, with the caller's paragraphs read out of § Still loading's own uncut mockup —
    // this one draws the last of them cut, and a fixture fed the cut sentence would compare the
    // mark with itself. The block wraps at `ui::BLOCK` whatever the terminal is, so here the break
    // and the mark are the page's own and are compared.
    let started = fed("## Still loading", 0, 0);
    let mut first_frame = screen(&loading, &now);
    first_frame.note = &started;
    first_frame.writes = Writes::Unaudited(&dead);
    against(section, 6, &app(), &first_frame);
    seen.push((section, 6));

    // § Before the TUI ever starts — the ninth, and its claim is that it draws no footer: those
    // failures print to stderr out of `main.rs` before raw mode is on, and there is no frame to put
    // one in.
    let section = "## Before the TUI ever starts";
    assert!(
        mockups(section).is_empty(),
        "{section} grew a screen with a footer — it prints before there is a frame to draw one in"
    );
    seen.push((section, 0));

    // **The file decides what is left**, not a number written here — and it is asked per *frame*.
    // A `###` subsection lives inside its `##` parent, so a section this test had already ticked
    // could grow a second and a third screen with nothing to notice: that is exactly how
    // § *Over a pane with nothing to show yet* arrived green.
    let path = format!("{}/screens/states.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("the screen file");
    let sections: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("## "))
        .collect();
    assert!(sections.len() >= 9, "screens/states.md lost a section");
    let mut frames = 0;
    for section in sections {
        for nth in 0..mockups(section).len() {
            frames += 1;
            assert!(
                seen.contains(&(section, nth)),
                "{section} [{nth}] draws a screen and nothing in this test visits it"
            );
        }
    }
    // **A derived list asserts it found something** (CLAUDE.md § Tests must not lie): a [`mockups`]
    // that silently stopped parsing would make every loop above it vacuous and this whole sweep a
    // green that proves nothing.
    assert_eq!(
        frames, 25,
        "screens/states.md draws {frames} screens with a footer, not the 25 this sweep was \
         written against — a frame was added or removed and this test has to say so"
    );
}

/// A refusal exactly as `ops::audit_log` returns one, read out of
/// `screens/states.md` § *The audit log could not be opened*'s own mockup.
///
/// **Read and not transcribed, because the transcription was wrong** (`k8s-admin`, 2026-09-12): the
/// literal this file carried said `audit.log ($HOME)` where `ops::Source::clause` has exactly two
/// answers — `from $XDG_STATE_HOME` and `under your home directory` — abbreviated an absolute
/// `path.to_string_lossy()` to `~`, and ended on a full stop `ops::without` does not write.
///
/// **It still cannot be imported.** `ops::without`, `ops::STILL_READS` and `ops::Source` are
/// private to that module and `ops.rs` is frozen; `audit_log` itself reads the real environment, so
/// calling it would either write into the developer's own state directory or need an `unsafe`
/// `set_var` that races `cargo test`'s threads. The screen file is the nearest fixture that is not
/// this file's own words, and `tester`'s capture of both `Source` arms is what checked it.
///
/// **The clause is asserted rather than assumed** (re-ruled 2026-09-12): the page now draws the
/// common refusal, `Permission denied (os error 13)` on a state directory that cannot be written,
/// and with it the `({from})` clause `audit_log` puts between the path and the error. A mockup
/// that dropped the clause again would hand every test here a sentence `ops` cannot return.
fn dead_log() -> String {
    let said = said_above(&mockups("## The audit log could not be opened")[0].pane).remove(0);
    assert!(
        said.starts_with("k8rs could not open its audit log at /")
            && ["(under your home directory): ", "(from $XDG_STATE_HOME): "]
                .iter()
                .any(|clause| said.contains(clause)),
        "the page's audit sentence is not a shape `ops::audit_log` returns: {said:?}"
    );
    said
}

/// **Both mutating keys on one screen, asked separately** ([`Op`]) — `(s, r)`, live exactly where
/// the footer above them draws them.
///
/// **[`offered`] is called once and handed to both**, which is the whole guarantee: the value that
/// drew the line is the value that decides the press, so a key can never be on one and not the
/// other (invariant 2's *unreachable, not merely unbound*).
fn pressable(app: &App, screen: &Screen) -> (bool, bool) {
    let offer = offered(app, screen);
    (
        app.may_mutate(offer, Op::Scale),
        app.may_mutate(offer, Op::Restart),
    )
}

/// **The ordinary screen is the only one that offers a mutating key** — the must-not-fire half of
/// [`every_state_draws_the_body_and_the_footer_its_own_mockup_gives_it`], and every cause that
/// takes the offer away, one at a time.
///
/// **Drawn and live are one fact, asserted together.** `views::App::may_mutate` is handed the same
/// `Offer` the footer was drawn from, so a key that is not on the line cannot be pressed either —
/// invariant 2's *unreachable, not merely unbound*, the bar `--read-only` is held to and the one
/// `screens/states.md` § *The audit log could not be opened* asks for by name.
#[test]
fn the_ordinary_screen_is_the_only_one_that_offers_a_mutating_key() {
    let now = now();
    let live = Pane::Ready(vec![oom()]);
    let ordinary = screen(&live, &now);
    let drawn = render(&app(), &ordinary);
    println!("{}", rows(&drawn).join("\n"));
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
        "a pane with a card in it, the link up, writes live and every time on it trustworthy"
    );
    assert_eq!(
        pressable(&app(), &ordinary),
        (false, true),
        "the ordinary screen refused a key it had just drawn"
    );

    // **And the browser's ordinary screen too, which is a table with rows in it** — the one state
    // § *An empty kind* is the counter-example to. A mutation run caught this missing
    // (2026-09-12): with only the empty table tested, a guard that matched every table drew
    // `/ filter` over a full list of deployments and nothing here noticed.
    let kinds = [browsable("deployments", true)];
    let listed = Pane::Ready(table("table-deployments"));
    let listing = browsing(&listed, &kinds, &now);
    let drawn = render(&opened(), &listing);
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
        "a browser pane with rows in it:\n{}",
        rows(&drawn).join("\n")
    );
    assert_eq!(
        pressable(&opened(), &listing),
        (false, true),
        "the browser refused a key it had just drawn"
    );

    // One cause at a time, each off the ordinary screen above, so nothing here passes by accident
    // of a second one being true.
    let empty = Pane::Ready(Vec::new());
    let nothing_selected = screen(&empty, &now);
    let dead = dead_log();
    let mut dead_log = screen(&live, &now);
    dead_log.writes = Writes::Unaudited(&dead);
    let mut asked = screen(&live, &now);
    asked.writes = Writes::ReadOnly;
    let mut skewed = screen(&live, &now);
    skewed.clock = Some(Stripped::of(
        "⚠ This computer and the cluster disagree about the time by 11 minutes.",
    ));
    let mut lost = screen(&live, &now);
    lost.link = Link::Lost;
    let mut expired = screen(&live, &now);
    expired.link = Link::Expired;

    for (cause, screen) in [
        ("nothing is selected", &nothing_selected),
        ("the audit log would not open", &dead_log),
        ("this login may not change anything", &asked),
        ("no time on the page can be trusted", &skewed),
        ("the connection dropped", &lost),
        ("the login expired", &expired),
    ] {
        let drawn = render(&app(), screen);
        let footer = unframed(&rows(&drawn)[22]);
        for withheld in ["s scale", "r restart", "s no scale", "r no restart"] {
            assert!(!footer.contains(withheld), "{cause}: {footer:?}");
        }
        assert_eq!(
            pressable(&app(), screen),
            (false, false),
            "{cause}: the key was off the line and live behind it"
        );
    }
}

/// **The selected object's *kind* decides which of `s` and `r` is on the line, and a Node's answer
/// is neither** (`screens/widgets.md` § 2a, extended 2026-09-18; NOTES § D261 ruling 8). This is
/// the defect the box named: a node card offered `s scale`, a key `ops::scale` refuses outright
/// and `may_i` was never asked about.
///
/// **Both panes, because they read the kind from two different places** — an Alerts card from its
/// own owner, a browser row from the kind the sidebar has open — and one of them being right says
/// nothing about the other.
///
/// **A kind is a group and a word, and the browser rows say so** (NOTES § D51,
/// `reports/2026-09-18-offer-per-kind-operator-read.md` § 1). The word alone is not an address:
/// `k8s::browsable` keeps the same plural under two groups as two resources, and a stock cluster
/// with no CRD installed already serves `v1 Event` beside `events.k8s.io/v1 Event`. So the last
/// three browser rows are workload plurals under somebody else's group — the shape `ops::scale`
/// would address in `apps/v1` and must therefore never offer a key for.
///
/// **The missing key is missing and not marked**: the whole drawn line is compared, so `s no
/// scale` on a Node fails here exactly as a live `s scale` does. That is the shape rule
/// `screens/widgets.md` § 2a states — refused is the ordinary line plus one word, unsupported is
/// the ordinary line minus one key — and it is asserted here rather than assumed, because the two
/// are only distinguishable if the unsupported one really does lose the key.
///
/// **And the two compose**: a DaemonSet whose `r` this login may not use draws `r no restart` and
/// still no `s` at all, which is the one row where both mechanisms are on the same line.
///
/// **Every Alerts list here holds two cards of *different* kinds, and that is the assertion and
/// not a garnish** (`tester`, 2026-09-18). With a one-card list `shown[0]` and
/// `shown[selected]` are the same card, so the branch's own claim — *the card under the cursor,
/// not the first card in the store* — was asserted by nothing: `shown.first()` in place of the
/// selection left the whole suite green. Each row is drawn twice over the same pair, once with the
/// cursor on the second card and once on the first, so `first()` fails the one and `last()` fails
/// the other, and neither can pass by symmetry.
#[test]
fn a_kind_that_cannot_scale_or_restart_is_not_offered_that_key() {
    let now = now();
    // **Two lines, not four** (`screens/widgets.md` § 2a, 2026-09-24): `s` is withheld from every
    // offer ([`crate::views::SCALE_IS_BUILT`]), so a kind either restarts or it has no mutating key
    // on the line at all. The names say which — `both` and `only_s` described footers no reader can
    // reach any more.
    let restarts = "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit";
    let neither = "↑↓ move  ⏎ open  / filter  ? all keys  q quit";
    // **One Alerts pane, several cards, and the cursor put somewhere in it** — the footer it draws
    // and what that footer leaves pressable, which are one value and are read as one.
    let alerts_footer = |kinds: &[ObjectKind], at: usize| {
        let cards: Vec<Card> = kinds
            .iter()
            .map(|kind| {
                let mut card = oom();
                card.owner = id(kind.clone(), Some("payments"), "web");
                card
            })
            .collect();
        let alerts = Pane::Ready(cards);
        let screen = screen(&alerts, &now);
        let mut app = app();
        // The anchors the pane itself builds — one `None` per card, `alerts`' own literal.
        let anchors: Vec<Option<&str>> = vec![None; kinds.len()];
        app.content.select(at, &anchors);
        let drawn = render(&app, &screen);
        (
            unframed(&rows(&drawn)[22]),
            pressable(&app, &screen),
            rows(&drawn).join("\n"),
        )
    };
    for (kind, expected, live) in [
        (ObjectKind::Deployment, restarts, (false, true)),
        (ObjectKind::StatefulSet, restarts, (false, true)),
        (ObjectKind::DaemonSet, restarts, (false, true)),
        // **A bare ReplicaSet scales and does not restart, so today it has no mutating key at all**
        // — the row that would start passing again on its own if the withholding were reverted
        // without this table.
        (ObjectKind::ReplicaSet, neither, (false, false)),
        (ObjectKind::Node, neither, (false, false)),
        (ObjectKind::Pod, neither, (false, false)),
        (ObjectKind::Job, neither, (false, false)),
        (ObjectKind::CronJob, neither, (false, false)),
        // A CRD, group-qualified as `rules.rs` builds one.
        (
            ObjectKind::Other("Rollout.argoproj.io".to_owned()),
            neither,
            (false, false),
        ),
        // **The `Other` that distinguishes the claim `ui::singular`'s doc makes** (`tester`,
        // 2026-09-18). The row above is refused by `crate::ops` whatever `singular` hands it, so
        // handing the reported word straight through would have passed it. This one is the word
        // `ops::scalable` serves, worn by a kind that is not it — every kind k8rs can operate on
        // has its own `ObjectKind` variant, so an `Other` is by construction not one, and it must
        // not borrow the answer belonging to the name it happens to carry.
        (
            ObjectKind::Other("deployment".to_owned()),
            neither,
            (false, false),
        ),
    ] {
        // **The partner card is always a kind with a different answer**, so a renderer reading the
        // store's first card rather than the selected one draws the partner's line and fails.
        let partner = match expected == restarts {
            true => ObjectKind::Node,
            false => ObjectKind::Deployment,
        };
        let pair = [partner, kind.clone()];
        let (line, keys, frame) = alerts_footer(&pair, 1);
        // **The line a reader would be looking at, printed** — `cargo test -- --nocapture` is the
        // only way this phase can *run* the renderer, because nothing outside `#[cfg(test)]` calls
        // `ui.rs` until Phase 12 wires `main.rs` (this file's own head).
        println!("{kind:?} (second of two)\n  {line}");
        assert_eq!(line, expected, "an Alerts card on a {kind:?}:\n{frame}");
        assert_eq!(
            keys, live,
            "an Alerts card on a {kind:?} — drawn and live disagree"
        );

        // The control: the same two cards the other way round, cursor on the first. A renderer
        // that read the *last* card would pass the row above and fail this one.
        let pair = [pair[1].clone(), pair[0].clone()];
        let (line, keys, frame) = alerts_footer(&pair, 0);
        assert_eq!(
            line, expected,
            "an Alerts card on a {kind:?}, first of two:\n{frame}"
        );
        assert_eq!(
            keys, live,
            "a {kind:?} first of two — drawn and live differ"
        );
    }

    // **The three pairs the review named, spelled out** — the sweep above builds them by rule, and
    // a rule is one edit away from building something else.
    for (pair, expected) in [
        (
            [ObjectKind::Deployment, ObjectKind::Node],
            "↑↓ move  ⏎ open  / filter  ? all keys  q quit",
        ),
        (
            [ObjectKind::Node, ObjectKind::Deployment],
            "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
        ),
        (
            [ObjectKind::Node, ObjectKind::DaemonSet],
            "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
        ),
    ] {
        let (line, _, frame) = alerts_footer(&pair, 1);
        println!("{pair:?} · cursor on 1\n  {line}");
        assert_eq!(
            line, expected,
            "{pair:?} with the cursor on the second:\n{frame}"
        );
    }

    // **The browser, where a kind supporting neither is the common case and not the exception**
    // (`screens/resources.md`): most rows it can reach are a ConfigMap, a Service, a Node.
    //
    // **The group is named on every row, because the kind word alone is not an address**
    // (NOTES § D51, `reports/2026-09-18-offer-per-kind-operator-read.md` § 1). The last three are
    // the regression: the same four workload plurals under somebody else's group — OpenKruise's
    // `apps.kruise.io StatefulSet` beside `apps/v1`'s under one sidebar row, and a CRD called
    // `Deployment` — all of which `ops::scale` would address in `apps/v1` and therefore must not
    // be offered `s` at all.
    let listed = Pane::Ready(table("table-deployments"));
    for (group, plural, expected, live) in [
        ("apps", "deployments", restarts, (false, true)),
        ("apps", "statefulsets", restarts, (false, true)),
        ("apps", "daemonsets", restarts, (false, true)),
        ("apps", "replicasets", neither, (false, false)),
        ("", "pods", neither, (false, false)),
        ("", "nodes", neither, (false, false)),
        ("", "configmaps", neither, (false, false)),
        ("apps.kruise.io", "statefulsets", neither, (false, false)),
        ("apps.kruise.io", "daemonsets", neither, (false, false)),
        ("example.com", "deployments", neither, (false, false)),
    ] {
        let kinds = [browsable_in(group, plural, true)];
        let listing = browsing(&listed, &kinds, &now);
        let drawn = render(&opened(), &listing);
        let shown = format!("{group}/{plural}");
        println!("browsing {shown}\n  {}", unframed(&rows(&drawn)[22]));
        assert_eq!(
            unframed(&rows(&drawn)[22]),
            expected,
            "the browser open on {shown}:\n{}",
            rows(&drawn).join("\n")
        );
        assert_eq!(
            pressable(&opened(), &listing),
            live,
            "the browser open on {shown} — drawn and live disagree"
        );
    }

    // **Refused and unsupported on one line.** `r` is the only mutating key a DaemonSet has, this
    // login may not use it, and `s` is still absent rather than marked: the refusal reaches the key
    // that was asked about and nothing else.
    let no = Some(&crate::ops::Verdict::No);
    let mut card = oom();
    card.owner = id(ObjectKind::DaemonSet, Some("payments"), "web");
    let alerts = Pane::Ready(vec![card]);
    let mut screen = screen(&alerts, &now);
    screen.refused = Refused::of("daemonsets", [no; 2], [no], [None]);
    let drawn = render(&app(), &screen);
    println!(
        "a DaemonSet this login may not restart\n{}",
        rows(&drawn).join("\n")
    );
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "↑↓ move  ⏎ open  r no restart  / filter  ? all keys  q quit",
        "a refused key on a kind that has only that one:\n{}",
        rows(&drawn).join("\n")
    );
}

/// **A namespace-scoped login that may act keeps both keys** — the defect `Pane::Denied` carried
/// when it was read as *we cannot reach the cluster* (`k8s-admin`, 2026-09-12,
/// `reports/2026-09-12-the-nine-states.md` § 3).
///
/// **`Denied` is two facts and only one of them is a refusal.** That type's own doc says so: the
/// second field exists so a refusal does not clear the screen, and the namespace-scoped fallback
/// (NOTES § D5) is the commonest thing that lands in it. A developer with a `RoleBinding` in
/// `payments` — `patch deployments/scale`, `patch deployments`, no cluster-wide `list pods` — has
/// `may_i` answering `Yes` for both keys and a header reading `live · admin`. So does a cluster
/// admin who simply typed `--namespace payments`. Neither may lose the keys, and the fact that
/// takes them away when it is true lives on [`Link`], which a `String` in a pane cannot carry.
#[test]
fn a_namespace_scoped_login_that_may_act_keeps_its_keys() {
    let now = now();
    let scoped = Pane::Denied(
        "Showing only the payments namespace, because --namespace asked for it.".to_owned(),
        vec![oom()],
    );
    let mut developer = screen(&scoped, &now);
    developer.namespace = Some(Stripped::of("payments"));
    let drawn = render(&app(), &developer);
    println!("{}", rows(&drawn).join("\n"));
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
        "a refusal that came back with cards is a selection, not a dead link"
    );
    assert_eq!(
        pressable(&app(), &developer),
        (false, true),
        "a namespace-scoped login could not reach the keys its own grant allows"
    );
    assert!(
        holds(&drawn, "Showing only the payments namespace"),
        "and the banner is still drawn over the list"
    );

    // **Zero rows is zero rows whichever answer holds them**: the same refusal over a kind that
    // came back with nothing promises `⏎ open` over an empty grid unless both arms are read.
    let kinds = [browsable("jobs", true)];
    let refused = Pane::Denied(
        "You can't list jobs across the whole cluster.".to_owned(),
        crate::k8s::Table::default(),
    );
    let nothing = browsing(&refused, &kinds, &now);
    let drawn = render(&opened(), &nothing);
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "/ filter  ? all keys  q quit",
        "a refusal with no rows in it still has nothing to move across:\n{}",
        rows(&drawn).join("\n")
    );
}

/// **`X switch cluster` survives the two expired frames that have nothing else on them** — which
/// are the two the key was promoted for: *"so a reader does not have to hold `aws sso login` in
/// their head while hunting the key map"* (`screens/states.md` § Your login expired).
///
/// **`screens/` now draws both**, in § *Over a pane with nothing to show yet*, and
/// [`every_state_draws_the_body_and_the_footer_its_own_mockup_gives_it`] compares each against its
/// own mockup — these two lines were this box's composition when the file had no drawing of them,
/// and were reported as a gap rather than smuggled in. What is left here is the half a mockup
/// cannot state: the seat the key keeps, and the link that must *not* name a key.
#[test]
fn an_expired_login_keeps_the_key_that_renews_it_on_every_frame() {
    let now = now();
    let quiet = Pane::Ready(Vec::new());
    let loading = Pane::Loading;

    // The state's own mockup, unchanged: the key sits after `⏎ open`.
    let mut cards = screen(&quiet, &now);
    cards.link = Link::Expired;
    assert!(
        unframed(&rows(&render(&app(), &cards))[22]).contains("⏎ open  X switch cluster  / filter"),
        "the key moved out of the seat the mockup gives it"
    );

    // A link that is merely down names no key to press: disconnected is retrying on its own.
    let mut lost = screen(&loading, &now);
    lost.link = Link::Lost;
    assert_eq!(
        unframed(&rows(&render(&app(), &lost))[22]),
        "? all keys  q quit",
        "a dropped connection invented a key for an action k8rs cannot perform"
    );
}

/// **`Link` has no `Default`**, because the only one it could derive is `Live` — the answer that
/// leaves `s` and `r` live over stale cards — and a `..Default::default()` would pick it without
/// anyone deciding to (`k8s-admin`, 2026-09-12, round two).
///
/// **Asked of the type, and answered by the compiler**: an inherent `const` that exists only where
/// `T: Default` shadows the trait's `false`, so re-adding the derive flips the answer and the test
/// binary does not build. The assertions are `const` blocks because the answer *is* a constant,
/// which is also what `clippy::assertions_on_constants` asks for. The second is the probe's own
/// canary — a probe that answered `false` for every type would pass the first for the wrong reason.
#[test]
fn a_link_has_no_default_for_a_screen_to_fall_back_on() {
    struct Probe<T>(std::marker::PhantomData<T>);
    trait Lacks {
        const DEFAULTS: bool = false;
    }
    impl<T> Lacks for Probe<T> {}
    impl<T: Default> Probe<T> {
        const DEFAULTS: bool = true;
    }
    const {
        assert!(
            !Probe::<Link>::DEFAULTS,
            "`Link` derives `Default` again, and its default is the answer that leaves writes live"
        );
        assert!(
            Probe::<Refused>::DEFAULTS,
            "the probe cannot see a `Default` where one exists, so it proves nothing about `Link`"
        );
    }
}

/// **A dead write path says so on the two Alerts panes that draw no list** — a healthy cluster,
/// where Alerts is empty for the whole session, and the first frame of every run, which is
/// `Loading` (`screens/states.md` § On a healthy or a still-loading Alerts screen).
///
/// **The rank is positional** (re-ruled 2026-09-12): the audit sentence sits directly under the
/// calm headline — at the very top on Loading, which has none — and every paragraph the caller
/// handed over follows in the caller's own order, so **whatever the caller put last is what gives
/// way**. The rule before it asked `ui.rs` to drop *Worth a look anyway* first, which it could only
/// find by recognising the caller's words; measured against it, the file's own paragraphs kept
/// *Worth a look* and cut *"will not change anything"* (`k8s-admin`, 2026-09-12, round two).
///
/// **A caller paragraph with no room left draws nothing, not a mark on the one above it** — the
/// banners' own *a share under two rows draws nothing at all*, which is the rank this is stated to
/// be identical to. A block cut as one run of lines put `…` on the end of a whole sentence.
#[test]
fn the_dead_write_path_says_so_on_a_pane_that_draws_no_list() {
    let now = now();
    let dead = dead_log();

    // § Nothing is broken's own paragraphs, verdict excluded: the count, then *Worth a look*.
    let empty = Pane::Ready(Vec::new());
    let handed = fed("## Nothing is broken", 0, 1);
    assert!(
        handed.len() == 2 && handed[1].as_str().starts_with("Worth a look anyway"),
        "the fixture is the page's count and its pointer, in that order: {handed:?}"
    );
    let mut clean = screen(&empty, &now);
    clean.note = &handed;
    clean.writes = Writes::Unaudited(&dead);
    let drawn = render(&app(), &clean);
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        body_text(&drawn).contains(&words(&dead)),
        "the audit sentence is ranked above every caller paragraph and came out cut:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        body_text(&drawn).contains(&words(handed[0].as_str())),
        "the count fits after the audit sentence and was not drawn whole:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        !holds(&drawn, "Worth a look"),
        "the caller's last paragraph is the one that gives way, and it did not:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        !holds(&drawn, "\u{2026}"),
        "a paragraph with no room left put a mark on the whole sentence above it:\n{}",
        rows(&drawn).join("\n")
    );
    let lines = rows(&drawn);
    let at_row = |needle: &str| {
        lines
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", lines.join("\n")))
    };
    // **One blank row at each join and no more.** The needles are each paragraph's own first and
    // last words, not a phrase that happened to sit on a row at one width.
    assert_eq!(
        at_row("could not open its audit log") - at_row("nothing is broken"),
        2,
        "the audit sentence sits directly under the verdict:\n{}",
        lines.join("\n")
    );
    assert_eq!(
        at_row("84 pods") - at_row("cluster still works"),
        2,
        "and the count follows it, one blank row down:\n{}",
        lines.join("\n")
    );

    // **One row left is not a share**: a paragraph needs the blank that separates it and a line of
    // its own. Handed one row, the last paragraph draws nothing — not a row holding only the mark.
    // The audit sentence is 8 lines, the one-line count and its blank are 2, and 11 is the budget
    // under the verdict, so exactly one row is left for the pointer.
    let short = [
        Stripped::of("84 pods checked."),
        Stripped::of("Worth a look anyway."),
    ];
    let mut tight = screen(&empty, &now);
    tight.note = &short;
    tight.writes = Writes::Unaudited(&dead);
    let drawn = render(&app(), &tight);
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        holds(&drawn, "84 pods checked.") && !holds(&drawn, "Worth a look"),
        "the fixture no longer leaves exactly one row for the last paragraph:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        !holds(&drawn, "\u{2026}"),
        "a paragraph handed one row drew the mark and nothing else:\n{}",
        rows(&drawn).join("\n")
    );

    // § Still loading's own paragraphs — no headline, so the sentence is at the very top, and the
    // caller's last paragraph is cut where its share ends.
    let loading = Pane::Loading;
    let reading = fed("## Still loading", 0, 0);
    let mut still = screen(&loading, &now);
    still.note = &reading;
    still.writes = Writes::Unaudited(&dead);
    let drawn = render(&app(), &still);
    println!("{}", rows(&drawn).join("\n"));
    let lines = rows(&drawn);
    let at_row = |needle: &str| {
        lines
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", lines.join("\n")))
    };
    assert!(
        body_text(&drawn).contains(&words(&dead)),
        "the first frame of every run lost the sentence, or cut it:\n{}",
        lines.join("\n")
    );
    assert!(
        at_row("could not open its audit log") < at_row("reading the cluster… 2,140 pods")
            && at_row("reading the cluster… 2,140 pods") < at_row("Large clusters"),
        "the audit sentence first, then the caller's paragraphs in the caller's order:\n{}",
        lines.join("\n")
    );
    assert!(
        !body_text(&drawn).contains("it does not wait") && holds(&drawn, "\u{2026}"),
        "the caller's last paragraph is what gives, and it gives with a mark:\n{}",
        lines.join("\n")
    );
}

/// **The audit sentence is drawn once, and only by the one pane that has no banner to carry it** —
/// the Alerts arm of `ui::content` (re-ruled 2026-09-12).
///
/// **It was appended inside `ui::note`, which seven panes call** (`k8s-admin`, 2026-09-12, round
/// two, probes p07 and p08): the browser's still-loading pane drew it twice — whole in the banner
/// above its title, and again cut inside the block under it — and every detail tab and Analysis
/// report drew it under *reading the cluster…* and dropped it the instant the tab answered,
/// because a loaded tab stacks no banners. A sentence that is there while a tab loads and gone once
/// it has is a flicker, not a statement.
#[test]
fn the_audit_sentence_is_drawn_once_and_only_where_no_banner_carries_it() {
    let now = now();
    let dead = dead_log();
    let opens = "could not open its audit log";
    let starts = |drawn: &Buffer| rows(drawn).iter().filter(|row| row.contains(opens)).count();

    let kinds = [browsable("deployments", true)];
    let loading = Pane::Loading;
    let mut waiting = browsing(&loading, &kinds, &now);
    waiting.writes = Writes::Unaudited(&dead);
    let drawn = render(&opened(), &waiting);
    println!("{}", rows(&drawn).join("\n"));
    assert_eq!(
        starts(&drawn),
        1,
        "the browser's still-loading pane has its banner and must not repeat it below:\n{}",
        rows(&drawn).join("\n")
    );

    let alerts = Pane::Ready(vec![oom()]);
    for tab in Tab::ALL {
        let open = Open::new();
        let detail = open.open();
        let mut reading = screen(&alerts, &now);
        reading.detail = Some(Detailed::Tabs {
            open: &detail,
            from_step: false,
        });
        reading.writes = Writes::Unaudited(&dead);
        let drawn = render(&on(tab), &reading);
        assert_eq!(
            starts(&drawn),
            0,
            "{tab:?} drew the sentence while it loaded and will drop it once it answers:\n{}",
            rows(&drawn).join("\n")
        );
    }

    let reports = [("capacity", None)];
    let loading = Pane::Loading;
    let mut computing = screen(&loading, &now);
    computing.reports = &reports;
    computing.writes = Writes::Unaudited(&dead);
    let drawn = render(&opened_report(0), &computing);
    assert_eq!(
        starts(&drawn),
        0,
        "an Analysis report still computing drew a sentence its computed pane has no seat for:\n{}",
        rows(&drawn).join("\n")
    );
}

/// **The list keeps three rows whatever queues above it, and a sentence past the budget is marked**
/// (`screens/states.md` § Your clock and a scoped namespace together — the body is 16 rows, the
/// list keeps 3, everything stacked over it shares the other 13).
///
/// **Two caveats at the floor used to delete the list the sidebar badge was still counting**, and
/// the sentence had no ceiling at all: a real `$XDG_STATE_HOME` measured 1383 characters against a
/// pane that holds about 880 at the floor (`k8s-admin`, 2026-09-12).
#[test]
fn a_stack_of_caveats_never_takes_the_list_below_its_own_floor() {
    let now = now();
    let live = Pane::Ready(vec![oom()]);
    let dead = dead_log();
    let mut both = screen(&live, &now);
    both.clock = Some(Stripped::of(
        "⚠ This computer and the cluster disagree about the time by 11 minutes (this one is \
         behind), so recent times are missing and older ones can read smaller than they really \
         are.",
    ));
    both.writes = Writes::Unaudited(&dead);
    let drawn = render(&app(), &both);
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        holds(&drawn, "payments/web"),
        "the card the badge is counting was drawn off the screen:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        holds(&drawn, "⚠ This computer"),
        "the first banner is the one that keeps its words"
    );
    assert!(
        body_text(&drawn).contains("reading your cluster still works"),
        "both sentences fit the 13 rows at this width, so neither gives way:\n{}",
        rows(&drawn).join("\n")
    );

    // A sentence far past anything a pane can hold — `ops::audit_log`'s own measured worst case
    // rounded up — still leaves the card and still marks the cut.
    let huge = format!(
        "k8rs could not open its audit log at {} — {}",
        "x".repeat(1200),
        dead
    );
    assert!(
        huge.len() > 1383,
        "the measured worst case is the floor of this fixture"
    );
    let mut flooded = screen(&live, &now);
    flooded.writes = Writes::Unaudited(&huge);
    let drawn = render(&app(), &flooded);
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        holds(&drawn, "payments/web"),
        "an unbounded sentence took the whole list with it:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(holds(&drawn, "\u{2026}"), "and it was cut with no mark");

    // The same value reaching the calm block, which had no cut of its own at all.
    let empty = Pane::Ready(Vec::new());
    let mut calm = screen(&empty, &now);
    calm.writes = Writes::Unaudited(&huge);
    let drawn = render(&app(), &calm);
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        holds(&drawn, "○  nothing is broken"),
        "the verdict survived"
    );
    assert!(
        holds(&drawn, "\u{2026}"),
        "and the paragraph under it was marked"
    );
    assert!(
        rows(&drawn)[2..18]
            .iter()
            .filter(|row| !pane(row).trim_end_matches('│').trim().is_empty())
            .count()
            <= 13,
        "the calm block spent more than the 13 rows the floor leaves it:\n{}",
        rows(&drawn).join("\n")
    );
}

/// **The audit sentence is the first banner to give way, and the other two never give way to feed
/// it** (`screens/states.md` § Your clock and a scoped namespace together, re-ruled 2026-09-12).
///
/// **Every sentence is read out of the page's own uncut mockups**, and the combinations are the
/// three the page draws under § The audit log could not be opened. What is asserted is the rank —
/// who is whole, who is cut, who is absent — and not where a line breaks: those mockups are drawn
/// at 70 and 80 columns with a right pad the card region does not have, so their break points are a
/// fact about the drawing, and this renders at the 80×24 floor.
///
/// **Before the rank, the pane's own reason was the one cut** (`k8s-admin`, 2026-09-12, round two):
/// *"One node check is off"* beside a namespace scope, `aws sso login` and the staleness label
/// beside an expired login, and with all three queued the namespace banner gone with no mark.
#[test]
fn the_audit_sentence_gives_way_first_and_the_pane_s_own_reason_never_does() {
    let now = now();
    let clock = joined("## Your computer's clock is off", 0, 0);
    let scoped = joined("## You can only see some namespaces", 0, 0);
    let expired = joined("## Your login expired", 0, 0);
    let dead = dead_log();
    let audit_opens = "k8rs could not open its audit log";

    // § And when a namespace is all you can see, too — the namespace banner whole, both paragraphs,
    // and the audit sentence started and cut with a mark.
    let denied = Pane::Denied(scoped.clone(), vec![oom()]);
    let mut both = screen(&denied, &now);
    both.writes = Writes::Unaudited(&dead);
    let drawn = render(&app(), &both);
    println!("namespace + audit\n{}\n", rows(&drawn).join("\n"));
    assert!(
        body_text(&drawn).contains(&words(&scoped)),
        "the pane's own reason gave way to the audit sentence:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        body_text(&drawn).contains(audit_opens) && holds(&drawn, "\u{2026}"),
        "the audit sentence had rows to start in and was not drawn, or was cut with no mark:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(holds(&drawn, "payments/web"), "and the list kept its card");

    // § And when the login has also expired — the login banner whole, every paragraph of it, and
    // the audit sentence gets only what is left.
    let timed_out = Pane::Denied(expired.clone(), vec![oom()]);
    let mut lapsed = screen(&timed_out, &now);
    lapsed.link = Link::Expired;
    lapsed.writes = Writes::Unaudited(&dead);
    let drawn = render(&app(), &lapsed);
    println!("login expired + audit\n{}\n", rows(&drawn).join("\n"));
    assert!(
        body_text(&drawn).contains(&words(&expired)),
        "the renewal command or the staleness label gave way to the audit sentence:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(holds(&drawn, "payments/web"), "and the list kept its card");

    // § All three at once — the clock whole, the namespace banner started and marked where its
    // share ends, and the audit sentence absent because nothing is left for it.
    let mut all = screen(&denied, &now);
    all.clock = Some(Stripped::of(&clock));
    all.writes = Writes::Unaudited(&dead);
    let drawn = render(&app(), &all);
    println!("clock + namespace + audit\n{}\n", rows(&drawn).join("\n"));
    assert!(
        body_text(&drawn).contains(&words(&clock)),
        "the clock gave way to something ranked under it:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        body_text(&drawn).contains("You can't list pods across the whole cluster"),
        "the namespace banner vanished beside the clock and the audit sentence:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        !body_text(&drawn).contains(audit_opens),
        "the audit sentence took rows the namespace banner is ranked above it for:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(holds(&drawn, "payments/web"), "and the list kept its card");
}

/// **The card the cursor is on is drawn, whichever card that is, under any stack of banners.**
///
/// **Every index is fed, because index 0 is the one case that hid the defect** (`k8s-admin`,
/// 2026-09-12, round two, probe p06): ratatui's `List` moves its offset to the *selected* item and
/// skips an item whole when it does not fit the region, so trimming only the first card left one
/// `↓` between the reader and an empty pane under a sidebar still counting `1 ● 1 ▲`. At the
/// 3-row floor every card is taller than the region — the cordon card, the shortest, is four lines
/// and its blank — so the cursor on it drew nothing at all.
#[test]
fn the_selected_card_is_drawn_under_a_stack_of_banners_on_every_index() {
    let now = now();
    let cards = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let clock = joined("## Your computer's clock is off", 0, 0);
    let dead = dead_log();
    // The real pair, and one that spends the whole 13 so the list is at its 3-row floor whatever
    // width either sentence wraps to.
    let flood = vec!["x".repeat(53); 20].join(" ");
    for (what, audit) in [
        ("clock and the audit sentence", dead.as_str()),
        ("banners that spend all 13 rows", flood.as_str()),
    ] {
        let mut stacked = screen(&cards, &now);
        stacked.clock = Some(Stripped::of(&clock));
        stacked.writes = Writes::Unaudited(audit);
        for (nth, identity) in ["payments/web", "node-3"].into_iter().enumerate() {
            let mut moved = app();
            moved.content.select(nth, &[None, None]);
            let drawn = render(&moved, &stacked);
            println!(
                "{what}, cursor on card {nth}\n{}\n",
                rows(&drawn).join("\n")
            );
            assert!(
                holds(&drawn, identity),
                "{what}: the cursor is on card {nth} and the pane under the banners is empty:\n{}",
                rows(&drawn).join("\n")
            );
        }
    }
}

/// **The floor holds at its own boundary, and that is where the audit sentence stops being
/// drawable** — two rows is the least a banner can say anything in, a line and the blank under it
/// (`screens/states.md` § Your clock and a scoped namespace together: the body is 16 rows, the list
/// keeps 3, and everything stacked over it shares the other 13).
///
/// **The audit sentence is the third because it is ranked last**, not because it happens to be
/// queued there: the clock and the pane's own reason are handed their rows first and never give
/// any back to it. **Under two rows it draws nothing at all** — no fragment, no dangling mark.
///
/// **One 53-column token per line, so the arithmetic under test is this test's and not the
/// wrapper's**: the card region is 53 columns at the floor (`screens/alerts.md` § The columns) and
/// two of these never share a row. A 4-line clock is 5 rows, a 5-line reason is 6, and 2 are left.
#[test]
fn the_audit_sentence_is_drawn_while_two_rows_are_left_and_absent_below_that() {
    let now = now();
    let filling = |lines: usize| vec!["x".repeat(53); lines].join(" ");
    let audit = "the audit sentence";
    let clock = filling(4);

    let reason = filling(5);
    let denied = Pane::Denied(reason.clone(), vec![oom()]);
    let mut queued = screen(&denied, &now);
    queued.clock = Some(Stripped::of(&clock));
    queued.writes = Writes::Unaudited(audit);
    let drawn = render(&app(), &queued);
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        body_text(&drawn).contains(audit),
        "two rows left is a line and its blank, and the sentence was dropped anyway:\n{}",
        rows(&drawn).join("\n")
    );

    // One line more in the pane's own reason leaves one row, which cannot hold a sentence *and*
    // the blank that separates it from the list. The reason is ranked above the audit sentence, so
    // it is drawn whole, and the audit sentence is the one that goes — whole, with no mark.
    let reason = filling(6);
    let denied = Pane::Denied(reason.clone(), vec![oom()]);
    let mut tighter = screen(&denied, &now);
    tighter.clock = Some(Stripped::of(&clock));
    tighter.writes = Writes::Unaudited(audit);
    let drawn = render(&app(), &tighter);
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        !body_text(&drawn).contains(audit),
        "one row left, and a sentence was drawn into the list's own floor:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        !holds(&drawn, "\u{2026}"),
        "a banner with no share left a dangling mark where it would have been:\n{}",
        rows(&drawn).join("\n")
    );
    assert_eq!(
        rows(&drawn)[2..18]
            .iter()
            .filter(|row| pane(row).trim_start().starts_with('x'))
            .count(),
        4 + 6,
        "the clock and the pane's own reason are both whole, and neither gave a row to the \
         audit sentence:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(holds(&drawn, "payments/web"), "and the floor kept its card");
}

/// **The browser keeps its title and the blank under it as well as the table's three rows** — they
/// come out of what a caveat leaves here, where on the Alerts pane the same rows are the list's own
/// ([`HEADING`]).
///
/// **Counted, not inferred**: a banner that wants twenty lines is drawn at exactly ten, which is
/// the 16 rows less the three the table keeps, less the title and its blank, less the banner's own
/// trailing blank.
#[test]
fn a_banner_over_the_browser_leaves_the_title_and_the_tables_own_floor() {
    let now = now();
    let kinds = [browsable("deployments", true)];
    let listed = Pane::Ready(table("table-deployments"));
    let filling = vec!["x".repeat(53); 20].join(" ");
    let mut flooded = browsing(&listed, &kinds, &now);
    flooded.writes = Writes::Unaudited(&filling);
    let drawn = render(&opened(), &flooded);
    println!("{}", rows(&drawn).join("\n"));
    let banner = rows(&drawn)[2..18]
        .iter()
        .filter(|row| pane(row).trim_start().starts_with('x'))
        .count();
    assert_eq!(
        banner,
        10,
        "the banner spent rows the title and the table are owed:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        holds(&drawn, "deployments"),
        "the title survived the banner"
    );
    assert!(
        holds(&drawn, "NAME"),
        "and so did the table's own header row"
    );
}

/// **The clock line hides while k8rs is not completing requests and the audit line does not**
/// (`screens/states.md` § While disconnected, or while the login has expired, § This sentence does
/// not hide with the clock's).
///
/// A skew is measured off a live response's `Date` header; a state directory that could not be
/// opened is fixed for the run and goes nowhere while the connection is down.
#[test]
fn the_clock_line_hides_with_the_connection_and_the_audit_line_stays() {
    let now = now();
    let dead = dead_log();
    let dropped = Pane::Denied(
        "⚠ Not connected to the cluster right now.".to_owned(),
        vec![oom()],
    );
    for (what, link) in [("disconnected", Link::Lost), ("expired", Link::Expired)] {
        let mut degraded = screen(&dropped, &now);
        degraded.link = link;
        degraded.clock = Some(Stripped::of(
            "⚠ This computer and the cluster disagree about the time.",
        ));
        degraded.writes = Writes::Unaudited(&dead);
        let drawn = render(&app(), &degraded);
        println!("{what}\n{}\n", rows(&drawn).join("\n"));
        assert!(
            !body_text(&drawn).contains("disagree about the time"),
            "{what}: a clock reading kept past the last successful request"
        );
        assert!(
            body_text(&drawn).contains("reading your cluster still works"),
            "{what}: the audit sentence hid with it and would have to reappear from nowhere"
        );
        assert!(
            body_text(&drawn).contains("⚠ Not connected"),
            "{what}: the pane's own sentence"
        );
    }
}

/// **Two caveats stack, most fundamental first** — `screens/states.md` § *Your clock and a scoped
/// namespace together*: the clock line before the namespace line, because a reader who cannot trust
/// *any* time on the page should be told that before being told which *part* of it they can see.
#[test]
fn the_clock_line_is_drawn_above_the_pane_s_own_banner() {
    let now = now();
    let scoped = Pane::Denied(
        "You can't list pods across the whole cluster, so k8rs is showing the namespace your \
         kubeconfig points at: payments."
            .to_owned(),
        vec![oom()],
    );
    let mut both = screen(&scoped, &now);
    both.clock = Some(Stripped::of(
        "⚠ This computer and the cluster disagree about the time by 11 minutes.",
    ));
    let drawn = render(&app(), &both);
    println!("{}", rows(&drawn).join("\n"));
    let lines = rows(&drawn);
    let at_row = |needle: &str| {
        lines
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", lines.join("\n")))
    };
    assert!(
        at_row("⚠ This computer") < at_row("You can't list pods"),
        "the clock line is not above the namespace line"
    );
    assert!(
        at_row("You can't list pods") < at_row("payments/web"),
        "and both are above the list they are about"
    );
}

/// **The column each non-blank banner row starts at, pane side** — the walk stops at the first
/// card, which is where the banners end.
///
/// **It reads the drawn rows and deliberately not [`said_above`]**, which collapses every run of
/// spaces to one: a comparison made through that helper reads identically whether the mark is spent
/// once or redrawn on every line, which is the one thing the caller of this is testing.
fn banner_indents(drawn: &Buffer) -> Vec<usize> {
    rows(drawn)[2..18]
        .iter()
        // The frame's right border is not content, and a row of nothing but spaces ends on it —
        // which reads as a 57-column indent to anything that counts leading spaces.
        .map(|line| pane(line).trim_end_matches('│').to_owned())
        .take_while(|line| {
            // **[`MARKER`] comes off before the band is looked for**, for [`said_above`]'s own
            // reason: the selected card draws `▸ ●` and is still the row a banner ends at.
            let head = line.trim_start();
            let head = head.strip_prefix(MARKER).unwrap_or(head);
            !head.starts_with('●') && !head.starts_with('▲')
        })
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take_while(|glyph| *glyph == ' ').count())
        .collect()
}

/// **A banner that opens with the alarm mark spends it once, and every line under the first begins
/// at the column the text began at** (`screens/states.md` § Rules that hold across every state on
/// this page, `tui-designer` 2026-09-12).
///
/// **The rule is about the paragraph, not about word wrap.** § *The connection dropped* draws three
/// whole sentences this way and not one sentence wrapped across three lines, which is why a
/// renderer that indented only a *continuation* would still be wrong: the mark would come back at
/// the head of the next sentence and read as repeating once per sentence.
///
/// **The unmarked family is the must-not-fire half**, and it is what stops this from passing on a
/// renderer that indents everything: § *You can only see some namespaces* opens with no mark, so
/// there is nothing to hang under and every one of its rows sits flush at the pad.
#[test]
fn a_marked_banner_spends_the_mark_once_and_hangs_the_rest_under_the_text() {
    let now = now();

    let section = "## The connection dropped";
    let dropped = Pane::Denied(joined(section, 0, 0), vec![oom()]);
    let mut lost = screen(&dropped, &now);
    lost.link = Link::Lost;
    let drawn = render(&app(), &lost);
    println!("{}", rows(&drawn).join("\n"));

    // The column a needle starts at on its own row, pane side and counted in characters — `⚠` is
    // three bytes and a byte offset would land inside it. [`body_row`] and not [`row`], because
    // the header carries a `⚠` of its own under this link.
    let at = |needle: &str| {
        let line = pane(&body_row(&drawn, needle));
        let byte = line.find(needle).expect("the row this needle was found by");
        line[..byte].chars().count()
    };
    let opening = at("⚠");
    let head = at("Not connected");
    assert_eq!(
        head,
        opening + 2,
        "the mark and the one space after it, and nothing else, before the first word:\n{}",
        rows(&drawn).join("\n")
    );
    // **The columns are asserted and the line breaks are not.** Where this sentence wraps is a
    // fact about the width it was rendered at — the screen file draws it at the floor's 47-column
    // pane and this renders at 80 — and a test that read the mockup's own break would be asserting
    // the terminal size rather than the rule.
    let hung = banner_indents(&drawn);
    assert_eq!(
        hung.first(),
        Some(&2),
        "the first line sits at the pad, mark and all:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        hung.len() > 1 && hung[1..].iter().all(|indent| *indent == head),
        "every line under the first hangs at the text's column, not the mark's: {hung:?}\n{}",
        rows(&drawn).join("\n")
    );

    // § Your login expired — the half word wrap cannot produce: three paragraphs, each starting a
    // line of its own whatever the width, and each one hanging at the same column. A renderer that
    // indented only a *continuation* draws every one of these back under the mark.
    let section = "## Your login expired";
    let timed_out = Pane::Denied(joined(section, 0, 0), vec![oom()]);
    let mut expired = screen(&timed_out, &now);
    expired.link = Link::Expired;
    let drawn = render(&app(), &expired);
    println!("{}", rows(&drawn).join("\n"));
    let at = |needle: &str| {
        let line = pane(&body_row(&drawn, needle));
        let byte = line.find(needle).expect("the row this needle was found by");
        line[..byte].chars().count()
    };
    let head = at("Your login expired.");
    for sentence in [
        "The cluster still knows",
        "Renew it",
        "What you see below is from",
    ] {
        assert_eq!(
            at(sentence),
            head,
            "{sentence:?} is its own sentence and does not start under the first one's text:\n{}",
            rows(&drawn).join("\n")
        );
    }
    // **Once, and the count is the point** — a renderer that redrew the mark on every sentence
    // would line those three up perfectly and still be wrong, because the mark would then read as
    // repeating rather than opening the paragraph.
    assert_eq!(
        rows(&drawn)
            .iter()
            .skip(1)
            .filter(|line| line.contains('\u{26a0}'))
            .count(),
        1,
        "the mark is spent once per paragraph, not once per sentence:\n{}",
        rows(&drawn).join("\n")
    );

    // § You can only see some namespaces — no mark, so nothing is spent and nothing hangs.
    let section = "## You can only see some namespaces";
    let scoped = Pane::Denied(joined(section, 0, 0), vec![oom()]);
    let partial = screen(&scoped, &now);
    let drawn = render(&app(), &partial);
    println!("{}", rows(&drawn).join("\n"));
    let flush = banner_indents(&drawn);
    assert!(
        flush.iter().all(|indent| *indent == 2),
        "an unmarked banner has no mark to hang under and every row sits at the pad: {flush:?}\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        flush.len() > 2,
        "and this screen draws more than one row of it: {flush:?}"
    );
}

/// **Analysis and an open detail tab leave no mutating key pressable behind a footer that names
/// none** — PRIOR-ART § G2, *read-only enforced per view is a hole per view*, which k9s #3858 is:
/// its XRay view still allowed a delete under read-only.
#[test]
fn a_mode_whose_footer_names_no_mutating_key_leaves_none_live() {
    let now = now();
    let live = Pane::Ready(vec![oom()]);
    let held = Open::new();
    let open = held.open();

    let reporting = App {
        view: View::Analysis(0),
        ..App::default()
    };
    let analysis = screen(&live, &now);
    let mut over = screen(&live, &now);
    over.detail = Some(Detailed::Tabs {
        open: &open,
        from_step: false,
    });

    let card = group();
    let mut stepping = screen(&live, &now);
    stepping.detail = Some(Detailed::Pods(&card));

    for (what, app, asked) in [
        ("Analysis", &reporting, &analysis),
        ("a detail tab", &app(), &over),
        // **The which-pods step is a mode whose own footer names no mutating key**, so it may not
        // leave one pressable behind it either (PRIOR-ART § G2).
        ("the which-pods step", &app(), &stepping),
    ] {
        let offer = offered(app, asked);
        assert_eq!(
            pressable(app, asked),
            (false, false),
            "{what} left a key live that its own footer never names — {offer:?}"
        );
        let (keys, _) = app.footer(
            detailing(asked),
            offer,
            Refused::default(),
            "",
            asked.contexts,
        );
        for withheld in ["s scale", "r restart"] {
            assert!(!keys.contains(withheld), "{what}: {keys:?}");
        }
    }
}

// --- THE BROWSER ---

/// One of the two committed `Table` captures, decoded the way `k8s.rs` decodes a response
/// (`tests/fixtures/`). **Never a hand-written column list** (NOTES § D53): what is proven below
/// is that one code path draws two different kinds, so the kinds have to be the cluster's.
///
/// The ingest *bound* is not applied — the trait that carries it is private to `k8s.rs`, where it
/// is tested. Nothing here is about the strip; `ui.rs` draws what it is handed (invariant 9 is
/// paid at ingest, and `ui.rs`'s module doc says so).
fn table(name: &str) -> crate::k8s::Table {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} could not be read: {e}"));
    let response: crate::k8s::TableResponse = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("fixture {path} is not a Table: {e}"));
    crate::k8s::Table::from(response)
}

/// A filter with something typed into it — one character at a time, the way the keyboard fills it,
/// because [`Input::push`] is where the bound and the control-character refusal live.
fn filter(text: &str) -> Input {
    let mut input = Input::default();
    for character in text.chars() {
        input.push(character);
    }
    input
}

/// A kind as discovery describes it, **in the group a real cluster serves that plural under** —
/// `apps` for the four workload plurals, `example.com` for everything else, which is a CRD and is
/// a coherent thing for a cluster to serve.
///
/// **Three of the four fields are read by the renderer now, and the fixture has to be an object
/// that could exist** (`k8s-admin`, `reports/2026-09-18-offer-per-kind-operator-read.md`; NOTES
/// § D51). It said `plural: "deployments"` beside `group: "example.com"` and `kind: "Whatever"`,
/// and when [`offered`] started reading the kind the answer was *"the right answer to the fixture
/// and the wrong one to the test"* — which was itself the defect: for an `example.com` kind called
/// `Deployment`, drawing no `s scale` is the right answer to **both**, and pinning the opposite as
/// the requirement is exactly what D51 forbids. The group is what makes it honest; the kind is
/// still de-pluralised, which is right for every plural these tests use and for the one reader,
/// which lowercases before matching.
///
/// [`browsable_in`] is the same thing with the group named, for the tests that are *about* it.
fn browsable(plural: &str, namespaced: bool) -> Browsable {
    let group = match plural {
        "deployments" | "statefulsets" | "daemonsets" | "replicasets" => "apps",
        _ => "example.com",
    };
    browsable_in(group, plural, namespaced)
}

/// [`browsable`] with the API group spelled out — **the field that decides whether a mutating key
/// is offered at all**, so a test about that decision names it rather than inheriting it.
fn browsable_in(group: &str, plural: &str, namespaced: bool) -> Browsable {
    let mut kind = plural.strip_suffix('s').unwrap_or(plural).to_owned();
    if let Some(first) = kind.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    Browsable {
        group: group.to_owned(),
        version: "v1".to_owned(),
        kind,
        plural: plural.to_owned(),
        namespaced,
        verbs: vec!["list".to_owned()],
    }
}

/// The Alerts pane a browser test hands over: not the pane under test, and its state must not
/// change a single column of what the browser draws.
static QUIET: Pane<Vec<Card>> = Pane::Ready(Vec::new());

/// The browser open on the first kind, through [`App::open`] rather than by setting the field —
/// so a test cannot reach a view the sidebar could not.
fn opened() -> App {
    let mut app = App::default();
    app.open(NavItem::Kind(0));
    app
}

fn browsing<'a>(
    browser: &'a Pane<crate::k8s::Table>,
    kinds: &'a [Browsable],
    now: &'a Time,
) -> Screen<'a> {
    let mut screen = screen(&QUIET, now);
    screen.browser = browser;
    screen.kinds = kinds;
    screen
}

/// The column each header starts at, in the drawn header row — so a cell assertion names its
/// column instead of a magic offset that moves when a fixture does.
fn columns_at(header: &str, names: &[&str]) -> Vec<usize> {
    let mut from = 0;
    names
        .iter()
        .map(|name| {
            let byte = header[from..]
                .find(name)
                .unwrap_or_else(|| panic!("no header {name:?} after byte {from} in {header:?}"))
                + from;
            from = byte + name.len();
            // **Counted in characters, never bytes** — the same reason [`column`] exists: a row
            // here carries `▸` and a byte offset lands inside it.
            header[..byte].chars().count()
        })
        .collect()
}

/// The row from one column on, counted in characters. `&line[2..]` panics inside `▸`.
fn from(line: &str, at: usize) -> String {
    line.chars().skip(at).collect()
}

/// **The whole of the browser's genericity claim, and both fixtures go through it.** Different
/// kinds, different column counts, different priority splits (8 = 5 + 3 and 9 = 5 + 4), one code
/// path, and nothing in it naming either.
///
/// **`priority: 1` is what `-o wide` adds** and drawing it makes every screen the wide view
/// (`screens/resources.md`). The negative half is the half that matters: a header the server sent
/// and this screen must not show.
#[test]
fn the_columns_are_the_servers_own_and_only_the_ones_plain_kubectl_prints() {
    let now = now();
    for (fixture, plural, plain, wide) in [
        (
            "table-deployments",
            "deployments",
            ["NAME", "READY", "UP-TO-DATE", "AVAILABLE", "AGE"].as_slice(),
            ["CONTAINERS", "IMAGES", "SELECTOR"].as_slice(),
        ),
        (
            "table-pods",
            "pods",
            ["NAME", "READY", "STATUS", "RESTARTS", "AGE"].as_slice(),
            ["IP", "NODE", "NOMINATED NODE", "READINESS GATES"].as_slice(),
        ),
    ] {
        let ready = Pane::Ready(table(fixture));
        let kinds = [browsable(plural, true)];
        let drawn = render(&opened(), &browsing(&ready, &kinds, &now));
        println!("{plural}\n{}\n", rows(&drawn).join("\n"));

        let header = pane(&row(&drawn, plain[1]));
        // Present, and in the server's own order — `columns_at` searches forward, so a header
        // drawn out of order cannot be found — and the first one starts at the selection gutter,
        // which is what puts the header over the cells rather than two columns off them.
        let at = columns_at(&header, plain);
        assert_eq!(at[0], MARKER.chars().count(), "{header:?}");
        for column in wide {
            assert!(
                !holds(&drawn, column),
                "{fixture}: {column:?} is priority 1, which is what `-o wide` adds"
            );
        }
        assert!(holds(&drawn, plural), "the pane says which kind it is");
    }
}

/// **A cell is read at the index of the column it was kept from, never at its place in the kept
/// list** — [`crate::k8s::Row::cells`] is aligned to the *whole* column list.
///
/// **Neither committed capture can catch this**, and that is the reason this test builds a third
/// shape: in both, the `priority: 0` columns are the first five, so the kept indices and their
/// positions in the kept list are the same numbers. A CRD's `additionalPrinterColumns` carry a
/// priority each and in any order, so a non-prefix split is an ordinary shape the pipeline can
/// hand this file (NOTES § D29 — a check is proven only for the shapes it was fed). The columns
/// and rows below are still the committed pods capture's; one column's `priority` is raised, in
/// memory, and no fixture on disk is touched (NOTES § D53).
#[test]
fn a_cell_is_read_at_its_own_columns_index_and_not_at_its_place_in_the_kept_list() {
    let now = now();
    let mut pods = table("table-pods");
    pods.columns[2].priority = 1;
    assert_eq!(pods.columns[2].name, "Status", "the promoted column");
    // Name, Ready, Restarts, Age — indices 0, 1, 3, 4, which is not 0, 1, 2, 3.
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let drawn = render(&opened(), &browsing(&ready, &kinds, &now));
    println!("{}", rows(&drawn).join("\n"));

    assert!(!holds(&drawn, "STATUS"), "Status is priority 1 here");
    let header = pane(&row(&drawn, "RESTARTS"));
    let at = columns_at(&header, &["NAME", "READY", "RESTARTS", "AGE"]);
    let line = pane(&row(&drawn, "kube-system/coredns"));
    for (column, cell) in at.iter().zip(["kube-system/coredns", "1/1", "0", "34h"]) {
        assert!(
            from(&line, *column).starts_with(cell),
            "{cell:?} belongs under column {column}:\n{header}\n{line}"
        );
    }
    assert!(
        !line.contains("Running"),
        "the cell of a dropped column is dropped with it: {line:?}"
    );
}

/// **The `ns:` label is two conditions and not one** (`screens/resources.md` § Rules): discovery's
/// own `namespaced` flag **and** a namespace scope in effect. A namespaced kind browsed with no
/// scope reads the same as a cluster-wide one, and that is the *ordinary* state today.
#[test]
fn the_namespace_label_needs_both_the_flag_and_a_scope() {
    let now = now();
    let ready = Pane::Ready(table("table-deployments"));
    for (namespaced, scope, labelled) in [
        (true, Some("payments"), true),
        (true, None, false),
        (false, Some("payments"), false),
        (false, None, false),
    ] {
        let kinds = [browsable("deployments", namespaced)];
        let mut screen = browsing(&ready, &kinds, &now);
        screen.namespace = scope.map(Stripped::of);
        let drawn = render(&opened(), &screen);
        let title = row(&drawn, "deployments");
        assert_eq!(
            title.contains("ns: payments"),
            labelled,
            "namespaced={namespaced} scope={scope:?}: {title:?}"
        );
        // Never blank and never `ns: -` — absent (`screens/resources.md` § Rules).
        assert!(!title.contains("ns: -"), "{title:?}");
    }
}

/// **A label that does not fit is left out, never half-drawn** — `ns: payme` names a namespace
/// that does not exist, and the header's own rule is that a vital is blank rather than guessed
/// (`screens/widgets.md` § 1a). A 43-character namespace is an ordinary DNS label, and the pane
/// has 53 columns at the floor.
#[test]
fn a_namespace_label_that_does_not_fit_is_left_out_rather_than_clipped() {
    let now = now();
    let long = "platform-integration-staging-eu-west-tenant";
    let ready = Pane::Ready(table("table-deployments"));
    let kinds = [browsable("deployments", true)];
    let mut screen = browsing(&ready, &kinds, &now);
    screen.namespace = Some(Stripped::of(long));
    let drawn = render(&opened(), &screen);
    println!("{}", rows(&drawn).join("\n"));

    let title = row(&drawn, "deployments");
    assert!(title.contains("deployments"), "the kind is still named");
    assert!(
        !title.contains("ns:"),
        "half a namespace names one that does not exist: {title:?}"
    );
    for cut in 4..long.len() {
        assert!(
            !holds(&drawn, &long[..cut]),
            "no prefix of the namespace is drawn either: {:?}",
            &long[..cut]
        );
    }
    // It still fits when the pane has room for it, or this test proves only that the label is
    // never drawn at all.
    let mut room = browsing(&ready, &kinds, &now);
    room.namespace = Some(Stripped::of("payments"));
    assert!(holds(&render(&opened(), &room), "ns: payments"));
}

/// **Where the label stops fitting is one column, and it is the sum in [`heading`] that decides
/// it.** Two data points either side of the edge: a namespace that misses by thirty columns is
/// refused by any arithmetic that happens to be large there, which is what left three mutants of
/// that sum alive on the first run (2026-09-06).
#[test]
fn the_namespace_label_fits_to_the_pane_edge_and_not_one_column_past_it() {
    let now = now();
    // The title's room at the floor: the frame's two borders, the sidebar, its divider and the
    // pane's two pads — 53 columns, `screens/alerts.md` § The columns' own number.
    let room = usize::from(MIN_WIDTH - 2 - SIDEBAR - 1 - 2 * PAD);
    assert_eq!(room, 53, "the floor's pane is not what it was");
    let fits = room - width("deployments") - GAP - width("ns: ");
    let ready = Pane::Ready(table("table-deployments"));
    let kinds = [browsable("deployments", true)];
    for (length, drawn) in [(fits, true), (fits + 1, false)] {
        let namespace = "n".repeat(length);
        let mut screen = browsing(&ready, &kinds, &now);
        screen.namespace = Some(Stripped::of(&namespace));
        let buffer = render(&opened(), &screen);
        let title = row(&buffer, "deployments");
        // **`ns:` at all, not the whole label.** One column past the edge the label is drawn and
        // then *clipped* by the widget, so a test looking for the complete string passes on the
        // very screen it exists to forbid -- which is how the first draft of this test left the
        // sum's mutants alive twice (2026-09-06).
        assert_eq!(
            title.contains("ns:"),
            drawn,
            "a {length}-character namespace with {room} columns to put it in:\n{}",
            rows(&buffer).join("\n")
        );
        if drawn {
            assert!(title.contains(&format!("ns: {namespace}")), "{title:?}");
        }
    }
}

/// **A row carries its namespace exactly when the view does not** (`screens/resources.md`
/// § Browsing every namespace) — the server sends no `NAMESPACE` column either way, so the
/// identity goes in the first cell or nowhere.
///
/// The two captures are the two identity shapes: `table-pods` carries a `PartialObjectMetadata`
/// per row, `table-deployments` was taken with `?includeObject=None` and carries none. **A row
/// whose namespace is genuinely absent draws the bare name, never `None/name` and never a bare
/// slash.**
#[test]
fn an_unscoped_row_is_prefixed_with_its_namespace_and_a_scoped_one_is_not() {
    let now = now();
    let pods = Pane::Ready(table("table-pods"));
    let kinds = [browsable("pods", true)];

    let unscoped = render(&opened(), &browsing(&pods, &kinds, &now));
    println!("{}", rows(&unscoped).join("\n"));
    assert!(
        holds(&unscoped, "kube-system/coredns"),
        "six identical names with a cursor on one of them is invariant 2 defeated in the intent"
    );

    let mut scoped = browsing(&pods, &kinds, &now);
    scoped.namespace = Some(Stripped::of("kube-system"));
    let scoped = render(&opened(), &scoped);
    assert!(
        !holds(&scoped, "kube-system/"),
        "a scoped view already names its one namespace in the title"
    );
    assert!(holds(&scoped, "coredns-589f44"));

    // `?includeObject=None`: no namespace on the row, so the bare name and nothing invented.
    let deployments = Pane::Ready(table("table-deployments"));
    let kinds = [browsable("deployments", true)];
    let bare = render(&opened(), &browsing(&deployments, &kinds, &now));
    assert!(holds(&bare, "broken-owned"));
    for invented in ["None/", "/broken-owned"] {
        assert!(
            !holds(&bare, invented),
            "no namespace is invented: {invented}"
        );
    }
}

/// **The selection marker comes from the same [`Cursor`] the sidebar and the cards use**, and one
/// row carries it.
///
/// The anchor is the row's `uid` (`crate::views::Cursor`), and `table-deployments` was captured
/// with `?includeObject=None` so it carries none at all — the documented shape where the cursor
/// falls back to its index. `↓` therefore lands on the second row by position, which is what a
/// renderer sees either way: `Cursor::selected` reads the anchors' *count*, not their values.
#[test]
fn the_selected_browser_row_carries_the_marker_and_no_other_row_does() {
    let now = now();
    let deployments = table("table-deployments");
    let uids: Vec<Option<String>> = deployments.rows.iter().map(|row| row.uid.clone()).collect();
    let second = deployments.rows[1].cells[0].clone();
    let ready = Pane::Ready(deployments);
    let kinds = [browsable("deployments", true)];

    let mut app = opened();
    let anchors: Vec<Option<&str>> = uids.iter().map(Option::as_deref).collect();
    app.content.down(&anchors);
    let drawn = render(&app, &browsing(&ready, &kinds, &now));
    println!("{}", rows(&drawn).join("\n"));

    assert!(
        pane(&row(&drawn, &second)).starts_with(MARKER),
        "the marker sits on the selected row: {:?}",
        pane(&row(&drawn, &second))
    );
    let marked = rows(&drawn)
        .iter()
        .filter(|line| pane(line).starts_with(MARKER))
        .count();
    assert_eq!(
        marked,
        1,
        "one row, never two:\n{}",
        rows(&drawn).join("\n")
    );
}

/// **A name too long for its column clips, and never fuses onto the number beside it**
/// (`screens/resources.md` § Browsing every namespace, the clip point).
///
/// **The order of sacrifice is the name's**: every other column keeps the width its header and its
/// widest cell asked for, and the name column is the one that gives way — `Min(len(header))`
/// against `Length(natural)`, which is `screens/widgets.md` § 2's own spelling.
#[test]
fn the_name_gives_way_and_leaves_a_blank_column_before_the_next_cell() {
    let now = now();
    let ready = Pane::Ready(table("table-pods"));
    let kinds = [browsable("pods", true)];
    let drawn = render(&opened(), &browsing(&ready, &kinds, &now));

    let header = pane(&row(&drawn, "RESTARTS"));
    let at = columns_at(&header, &["NAME", "READY", "STATUS", "RESTARTS", "AGE"]);
    let line = pane(&row(&drawn, "kube-system/"));
    assert!(
        line.chars().count() > at[1],
        "the row reaches the second column: {line:?}"
    );
    assert!(
        line.chars()
            .take(at[1])
            .collect::<String>()
            .contains("kube-system/"),
        "the identity did not fit whole and is clipped where its column ends: {line:?}"
    );
    assert_eq!(
        column(&line, at[1] - 1),
        ' ',
        "a clipped identity and the number beside it are never read as one token:\n{header}\n{line}"
    );
    // **The numbers keep the width their own content asked for**, which is the other half of the
    // give-way rule: `RESTARTS` is an 8-column header whose widest cell is `3 (34h ago)`, so the
    // column is eleven — not an even share of the pane, and not shrunk to the header.
    assert_eq!(at[4] - at[3], width("3 (34h ago)") + GAP, "{header:?}");
}

/// **Still loading · nothing there · refused** — the same three the Alerts pane has, and none of
/// them says `403` or the word RBAC.
///
/// **An empty list of deployments is not `nothing is broken`**: that sentence is Alerts' claim
/// about the whole cluster, and a kind with no objects in it has no severity at all. **A refusal
/// never reaches the empty sentence either** — *we were not allowed to look* is not *there is
/// nothing*.
#[test]
fn the_browser_has_the_same_three_answers_the_alerts_pane_has() {
    let now = now();
    let kinds = [browsable("deployments", true)];

    let loading = Pane::Loading;
    let still = render(&opened(), &browsing(&loading, &kinds, &now));
    assert!(holds(&still, "reading the cluster…"));
    assert!(!holds(&still, "no deployments"));

    let none = Pane::Ready(crate::k8s::Table::default());
    let bare = render(&opened(), &browsing(&none, &kinds, &now));
    assert!(holds(&bare, "no deployments in this cluster"));
    assert!(
        !holds(&bare, "nothing is broken"),
        "that is the Alerts pane's claim about the cluster, not this kind's row count"
    );
    let mut scoped = browsing(&none, &kinds, &now);
    scoped.namespace = Some(Stripped::of("payments"));
    let scoped = render(&opened(), &scoped);
    assert!(holds(&scoped, "no deployments in payments"));

    // **The kind that outlived discovery**: an empty `kinds` slice under `View::Resources(0)`,
    // which is what a discovery refresh that dropped a kind leaves the view pointing at. There is
    // no plural to put in a sentence, so the pane names the next thing to try instead
    // (`screens/states.md` § An empty kind in the browser, third row).
    let stale = render(&opened(), &browsing(&none, &[], &now));
    assert!(
        holds(&stale, "no longer in the list — pick another kind"),
        "with no kind to name, the pane names the next thing to try:\n{}",
        rows(&stale).join("\n")
    );
    assert!(
        !holds(&stale, "no  in"),
        "never the scoped sentence with a hole where the kind would be"
    );

    let said = "You can't list deployments across the whole cluster, so k8rs is showing the \
                namespace your kubeconfig points at: payments.";
    let refused = Pane::Denied(said.to_owned(), table("table-deployments"));
    let banner = render(&opened(), &browsing(&refused, &kinds, &now));
    assert!(holds(
        &banner,
        "You can't list deployments across the whole"
    ));
    assert!(
        holds(&banner, "broken-owned"),
        "a banner over whatever did come back, never instead of it"
    );
    let empty = Pane::Denied(said.to_owned(), crate::k8s::Table::default());
    let empty = render(&opened(), &browsing(&empty, &kinds, &now));
    assert!(holds(&empty, "You can't list deployments across the whole"));
    assert!(
        !holds(&empty, "no deployments"),
        "refused and nothing came back is not the same screen as there is nothing"
    );

    for state in [&still, &bare, &scoped, &stale, &banner, &empty] {
        println!("{}\n", rows(state).join("\n"));
        for word in ["403", "RBAC", "forbidden"] {
            assert!(!holds(state, word), "no state prints {word:?}");
        }
    }
    // **The title is drawn in every state**, asserted only where the kind's name could come from
    // nowhere else: the two refusals put `deployments` in the banner's own sentence, so the same
    // check there would pass without a title at all.
    for state in [&still, &bare, &scoped] {
        assert!(
            row(state, "deployments").contains("deployments"),
            "a pane that has not answered still says which kind it is about"
        );
    }
}

/// A card filed under the object one browser row **is** — the uid join and nothing else
/// (`screens/resources.md` § Rules). The kind is the owner's, not the browsed row's: what a card
/// is filed under is the controller, and the browser is open on whatever kind that controller is.
fn filed(severity: Severity, kind: ObjectKind, at: &crate::k8s::Row, affected: usize) -> Card {
    let owner = ObjectId {
        kind,
        namespace: at.namespace.clone(),
        name: at.name.clone().expect("a captured row carries a name"),
        uid: at.uid.clone(),
    };
    Card {
        owner: owner.clone(),
        findings: vec![Finding {
            owner,
            ..finding(
                severity,
                "Containers exceeded their memory limit and were killed by the kernel (OOMKilled)",
                "limit 256Mi · exit 137 · 47 restarts",
                "raise limits.memory, or find the leak",
            )
        }],
        affected,
        total: Some(5),
    }
}

/// The browser open on `table-pods` — the one committed capture whose rows carry uids — with an
/// Alerts pane bleeding through onto it.
fn bleeding<'a>(
    browser: &'a Pane<crate::k8s::Table>,
    alerts: &'a Pane<Vec<Card>>,
    kinds: &'a [Browsable],
    now: &'a Time,
) -> Screen<'a> {
    let mut screen = browsing(browser, kinds, now);
    screen.alerts = alerts;
    screen
}

/// The pane's own columns with the frame's right border dropped — what a whole-line `==` has to
/// compare, where every `contains` above can ignore it.
fn only(line: &str) -> String {
    pane(line).trim_end_matches('│').to_owned()
}

/// The column a row's own glyph sits in: the pane's left edge, past the selection marker.
fn gutter() -> usize {
    usize::from(1 + SIDEBAR + 1) + width(MARKER)
}

/// **Wider than the floor, so a row can be told from its neighbour.** `table-pods` is fourteen
/// kube-system pods whose names share a prefix, and at 80 columns the name cell clips them to
/// `kube-system/coredns` — two rows, one string, and an assertion that cannot say which row it
/// found. Nothing about the mark depends on the width; the floor is printed at the end of this
/// file, where the clip is the thing being shown.
fn spacious(app: &App, screen: &Screen) -> Buffer {
    render_at(120, MIN_HEIGHT, app, screen)
}

/// **A row whose object Alerts has a card about is marked, in that card's own band** —
/// `screens/resources.md` § Rules, *so the browser never disagrees with the Alerts view*.
///
/// **The colour and the glyph both come from `theme::band`**, which is what makes the mark
/// survive a monochrome terminal and what keeps this screen from writing a colour of its own.
/// The negative half is the half that matters: every other row of the same capture draws the
/// gutter **blank rather than absent**, which is what keeps the names — and `NAME` over them — in
/// one column whether the row carries a glyph or not.
#[test]
fn a_row_its_card_is_filed_under_is_marked_in_that_cards_band() {
    let now = now();
    let pods = table("table-pods");
    let critical = filed(Severity::Critical, ObjectKind::Deployment, &pods.rows[0], 3);
    let warn = filed(Severity::Warn, ObjectKind::DaemonSet, &pods.rows[3], 2);
    let alerts = Pane::Ready(vec![critical, warn]);
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let drawn = spacious(&opened(), &bleeding(&ready, &alerts, &kinds, &now));
    let lines = rows(&drawn);
    println!("{}", lines.join("\n"));

    let banded = |needle: &str, glyph: char, colour: Colour| {
        let at = lines
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", lines.join("\n")));
        assert_eq!(column(&lines[at], gutter()), glyph, "{:?}", lines[at]);
        assert_eq!(
            drawn
                .cell((gutter() as u16, at as u16))
                .expect("a cell in the pane")
                .fg,
            ink(colour, Depth::TrueColor),
            "the band's colour, never one written in `ui.rs`"
        );
    };
    banded("hdrv5", '●', theme::CRITICAL);
    banded("kindnet-bhzgd", '▲', theme::WARN);

    // **Nobody else**, and the gutter they leave is blank rather than absent: the names line up
    // with the marked ones, and `NAME` lines up with both.
    for quiet in ["lbkj6", "etcd-k8rs", "kindnet-szmvh", "kube-proxy-5d9xj"] {
        assert_eq!(
            column(&row(&drawn, quiet), gutter()),
            ' ',
            "no card is filed under {quiet}"
        );
    }
    let header = pane(&row(&drawn, "RESTARTS"));
    let at = columns_at(&header, &["NAME", "READY", "STATUS", "RESTARTS", "AGE"]);
    assert_eq!(
        at[0],
        MARKER.chars().count() + GUTTER,
        "the header moves over the gutter with the cells: {header:?}"
    );
    for line in ["hdrv5", "etcd-k8rs"] {
        let line = pane(&row(&drawn, line));
        assert_eq!(
            line.chars().nth(at[0]),
            Some('k'),
            "marked or not, the name starts in the same column: {line:?}"
        );
    }
}

/// **A table nobody has a finding in draws exactly what it drew before Alerts bled through** —
/// the whole pane compared cell for cell, not a column counted by hand.
///
/// Three panes that must all produce that frame: no cards at all, cards about objects this kind
/// does not hold, and an Alerts pane that has **not answered yet**. The third is the one with a
/// claim in it — a browser that marked nothing while Alerts was still loading would be saying
/// *no findings here*, which nobody has established.
#[test]
fn a_table_with_no_card_in_it_is_the_frame_that_was_drawn_before() {
    let now = now();
    let pods = table("table-pods");
    let elsewhere = Pane::Ready(vec![oom(), cordon(None)]);
    let loading: Pane<Vec<Card>> = Pane::Loading;
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];

    // **The pane, not the frame**: the sidebar's badge counts every card there is, and two of
    // these panes legitimately have cards in them — about objects this kind does not hold.
    let panes = |buffer: &Buffer| -> Vec<String> { rows(buffer).iter().map(|l| pane(l)).collect() };
    let plain = spacious(&opened(), &bleeding(&ready, &QUIET, &kinds, &now));
    for alerts in [&elsewhere, &loading] {
        let drawn = spacious(&opened(), &bleeding(&ready, alerts, &kinds, &now));
        assert_eq!(
            panes(&drawn),
            panes(&plain),
            "not a single column moves:\n{}",
            rows(&drawn).join("\n")
        );
        assert!(!holds(&drawn, "with problems"), "no row is marked");
    }
    // And a card that *is* filed under a row of this capture moves it, or the comparison above
    // proves only that the fixture is quiet.
    let here = Pane::Ready(vec![filed(
        Severity::Critical,
        ObjectKind::Deployment,
        &table("table-pods").rows[0],
        3,
    )]);
    let marked = spacious(&opened(), &bleeding(&ready, &here, &kinds, &now));
    assert_ne!(panes(&marked), panes(&plain));
}

/// **One line under the table, about the row the cursor is on** (NOTES § D251) — not one per
/// marked row, which would be a second list competing with the table above it.
///
/// **And the count in it is the card's own numerator**: `3 pods with problems` here and
/// `3 of 5 pods` on the card one screen over are the same three objects, drawn from the same
/// field. Both are rendered from one `Card` below, so a second count could not be introduced
/// without this failing.
#[test]
fn the_line_under_the_table_is_about_the_selected_row_and_counts_the_cards_own_pods() {
    let now = now();
    let pods = table("table-pods");
    let uids: Vec<Option<String>> = pods.rows.iter().map(|row| row.uid.clone()).collect();
    let alerts = Pane::Ready(vec![filed(
        Severity::Critical,
        ObjectKind::Deployment,
        &pods.rows[0],
        3,
    )]);
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];

    let on = spacious(&opened(), &bleeding(&ready, &alerts, &kinds, &now));
    println!("{}", rows(&on).join("\n"));
    let said = row(&on, "with problems");
    assert!(
        said.contains("kube-system/coredns-589f44dc88-hdrv5 has 3 pods with problems — ⏎ to see"),
        "the selected row, named the way the table drew it: {said:?}"
    );
    assert_eq!(
        column(&said, gutter()),
        '●',
        "the line's glyph sits under the row's: {said:?}"
    );
    assert_eq!(
        rows(&on)
            .iter()
            .filter(|line| line.contains("with problems"))
            .count(),
        1,
        "one line, never one per marked row"
    );

    // **The cursor moves off it and the line goes with it** — the row stays marked, because the
    // mark is about the object and the line is about the selection.
    let mut moved = opened();
    let anchors: Vec<Option<&str>> = uids.iter().map(Option::as_deref).collect();
    moved.content.down(&anchors);
    let off = spacious(&moved, &bleeding(&ready, &alerts, &kinds, &now));
    assert!(
        !holds(&off, "with problems"),
        "the row the cursor is on has no card:\n{}",
        rows(&off).join("\n")
    );
    assert_eq!(column(&row(&off, "hdrv5"), gutter()), '●');

    // The same card, drawn as a card: one numerator, two screens. Wide, for [`spacious`]' reason
    // one level over — at the floor this name is 39 of the card region's 51 columns and the
    // `· n of m pods` fragment is what gives way (`screens/alerts.md` § The age).
    let card = spacious(&app(), &screen(&alerts, &now));
    assert!(
        holds(&card, "3 of 5 pods"),
        "the Alerts view counts the same objects:\n{}",
        rows(&card).join("\n")
    );
}

/// **A card with no pod count draws the line without one, never `0 pods`** — the two cases
/// `Card::count` already refuses (NOTES § D246 ruling 2): a card about no pods at all, and one
/// whose owner *is* the pod the row shows.
#[test]
fn a_card_that_counts_no_pods_says_so_by_leaving_the_count_out() {
    let now = now();
    let pods = table("table-pods");
    let kinds = [browsable("pods", true)];
    for owner in [
        // A card about no pods at all, which is every node card: nothing here counts pods. The
        // pairing is synthetic — a node's uid is not a pod's — and the code path is the one a
        // marked row in the `nodes` browser takes.
        filed(Severity::Critical, ObjectKind::Node, &pods.rows[0], 0),
        // A bare pod: nothing owns it, so a fraction of one pod out of itself is not a fact.
        filed(Severity::Critical, ObjectKind::Pod, &pods.rows[0], 1),
    ] {
        let alerts = Pane::Ready(vec![owner]);
        let ready = Pane::Ready(table("table-pods"));
        let drawn = spacious(&opened(), &bleeding(&ready, &alerts, &kinds, &now));
        let said = row(&drawn, "has problems");
        println!("{said}");
        assert!(said.contains("hdrv5 has problems — ⏎ to see"), "{said:?}");
        for invented in ["0 pods", "1 pods", "pods with problems"] {
            assert!(
                !holds(&drawn, invented),
                "a count that is not a fact is left out, never printed: {invented}"
            );
        }
        assert_eq!(column(&said, gutter()), '●');
    }
}

/// **A short kind draws the mockup's own spacing**: every row, then one blank, then the line —
/// `screens/resources.md`'s populated mockup has four rows and puts the line two below the last.
///
/// The table is a prefix of the committed capture rather than a shape invented here (NOTES § D53
/// is about editing a capture, and taking four of its rows edits nothing): every capture in this
/// repo is longer than the pane at the floor, so nothing else in this file reaches the case where
/// the table asks for less height than it is offered.
#[test]
fn a_short_kind_puts_the_line_one_blank_row_under_its_last_row() {
    let now = now();
    let mut pods = table("table-pods");
    pods.rows.truncate(4);
    let alerts = Pane::Ready(vec![filed(
        Severity::Critical,
        ObjectKind::Deployment,
        &pods.rows[0],
        3,
    )]);
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let drawn = render(&opened(), &bleeding(&ready, &alerts, &kinds, &now));
    let lines = rows(&drawn);
    println!("{}", lines.join("\n"));

    let last = lines
        .iter()
        // Clipped at the floor, which is the point of drawing this one at the floor.
        .position(|line| line.contains("kindnet-bh"))
        .expect("the fourth row of the capture");
    assert!(
        only(&lines[last + 1]).trim().is_empty(),
        "one blank row between the table and the line: {:?}",
        lines[last + 1]
    );
    assert!(
        lines[last + 2].contains("has 3 pods with problems"),
        "and the line under that, not at the bottom of the pane: {:?}",
        lines[last + 2]
    );
}

/// **The gutter is bought out of the name column's minimum**, so a pane too narrow for its
/// columns still draws `NAME` whole over names it has clipped to nothing.
///
/// Nine `priority: 0` columns want 102 of the floor's 51 — the shape
/// [`more_columns_than_the_pane_can_hold_still_draws_a_frame`] builds, with a mark on it. The
/// first column is then held at exactly its minimum, which is the only width at which that
/// minimum is a fact anyone can see.
#[test]
fn a_pane_too_narrow_for_its_columns_still_draws_the_header_over_the_gutter() {
    let now = now();
    let mut pods = table("table-pods");
    for column in &mut pods.columns {
        column.priority = 0;
    }
    let alerts = Pane::Ready(vec![filed(
        Severity::Critical,
        ObjectKind::Deployment,
        &pods.rows[0],
        3,
    )]);
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let drawn = render(&opened(), &bleeding(&ready, &alerts, &kinds, &now));
    println!("{}", rows(&drawn).join("\n"));

    let header = pane(&row(&drawn, "RESTA"));
    assert!(
        header.starts_with("    NAME"),
        "the gutter and the header both survive the squeeze: {header:?}"
    );
    // Every name is clipped to four columns here, so the marked row is named by its marker.
    assert_eq!(column(&row(&drawn, "▸ ● "), gutter()), '●');
}

/// **At the floor the line is exactly this, name cut and marked and `⏎ to see` intact** — the
/// whole string, because the cut point is arithmetic and an assertion on a substring cannot see
/// it move.
///
/// 57 columns of pane, less the selection marker's 2 and the gutter's 2, less the 36 the sentence
/// needs, leaves the name 17: sixteen columns of it and the [`CUT`] that says the rest is gone.
/// What gives way is the name, never the half that says what to do next
/// (`screens/resources.md` § The line under the table; `⏎` opens the object the rest of it is on).
#[test]
fn the_line_clips_its_name_and_never_the_key_it_names() {
    let now = now();
    let pods = table("table-pods");
    let alerts = Pane::Ready(vec![filed(
        Severity::Critical,
        ObjectKind::Deployment,
        &pods.rows[0],
        3,
    )]);
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let drawn = render(&opened(), &bleeding(&ready, &alerts, &kinds, &now));
    let said = only(&row(&drawn, "with problems"));
    println!("{said:?}");

    assert_eq!(
        said.trim_end(),
        "  ● kube-system/core… has 3 pods with problems — ⏎ to see"
    );
    assert_eq!(
        width("kube-system/core") + width(CUT),
        57 - width(MARKER) - GUTTER - width(" has 3 pods with problems — ⏎ to see"),
        "the name and its mark get what the pane has left and not a column more"
    );
}

/// **Both sides of the boundary** — a name that fills `room` exactly draws whole and unmarked, one
/// column past it draws cut and marked (`screens/resources.md` § The line under the table,
/// rules 2 and 3).
///
/// **The marked side is asserted on the mark and the kept width, never on a longer string being
/// present.** `contains("kube-system/core")` is true of the unmarked screen that rule forbids,
/// which is how one shipped: a whole-string `==` written to match the output says only that the
/// output has not changed since somebody looked at it.
#[test]
fn the_name_is_cut_with_a_visible_mark_one_column_past_where_it_fits() {
    let now = now();
    let pods = table("table-pods");
    let alerts = Pane::Ready(vec![filed(
        Severity::Critical,
        ObjectKind::Deployment,
        &pods.rows[0],
        3,
    )]);
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let view = bleeding(&ready, &alerts, &kinds, &now);

    let name = "kube-system/coredns-589f44dc88-hdrv5";
    let tail = " has 3 pods with problems — ⏎ to see";
    // The narrowest terminal that leaves the name every column it needs and not one more: the
    // frame's furniture — left border, sidebar, divider, right border — then the line itself.
    let exact = 1
        + SIDEBAR
        + 1
        + u16::try_from(width(MARKER) + GUTTER + width(name) + width(tail)).expect("a width")
        + 1;

    let said = only(&row(
        &render_at(exact, MIN_HEIGHT, &opened(), &view),
        "with problems",
    ));
    println!("{said:?}");
    assert_eq!(
        said.trim_end(),
        format!("  ● {name}{tail}"),
        "the name that fits exactly is drawn whole, and nothing marks it"
    );

    let said = only(&row(
        &render_at(exact - 1, MIN_HEIGHT, &opened(), &view),
        "with problems",
    ));
    println!("{said:?}");
    let drawn: String = said
        .trim_end()
        .strip_suffix(tail)
        .expect("`⏎ to see` never gives way")
        .chars()
        .skip(width(MARKER) + GUTTER)
        .collect();
    let kept = drawn
        .strip_suffix(CUT)
        .expect("a cut name carries the mark");
    assert_eq!(
        width(&drawn),
        width(name) - 1,
        "the name still fills the columns it has: {drawn:?}"
    );
    assert_eq!(
        width(kept),
        width(name) - 1 - width(CUT),
        "the mark is paid for out of the name, not added beside it: {kept:?}"
    );
    assert!(
        name.starts_with(kept),
        "the mark is glued to a real prefix of the name: {kept:?}"
    );
}

/// **Rule 4: a line with no room for a name draws none — and no [`CUT`] either**
/// (`screens/resources.md` § The line under the table). A lone `…` standing where the name would
/// be is what the rule's own guard, removed, prints, so the mark is what this asserts on: an
/// assertion that only checks the name is gone passes on the screen rule 4 forbids.
///
/// **The one assertion in this file that calls a browser function instead of going through
/// [`draw`], and it is not a shortcut.** `draw` refuses anything under 80×24 and the content pane
/// is 57 columns at that floor, so no terminal width can starve this line through the frame — the
/// arm is ruled and unreachable from outside, which is exactly why nothing had ever exercised it.
/// The `Rect` here is the whole of what makes `room` zero: the prefix's four columns and the 36
/// the sentence needs, and not one more.
#[test]
fn a_line_with_no_room_for_a_name_draws_neither_the_name_nor_the_mark() {
    let now = now();
    let pods = table("table-pods");
    let card = filed(Severity::Critical, ObjectKind::Deployment, &pods.rows[0], 3);
    let showing = screen(&QUIET, &now);
    let tail = " has 3 pods with problems — ⏎ to see";
    let wide = u16::try_from(width(MARKER) + GUTTER + width(tail)).expect("a width");

    let mut terminal = Terminal::new(TestBackend::new(wide, 1)).expect("a terminal");
    terminal
        .draw(|frame| {
            problems(
                frame,
                Rect::new(0, 0, wide, 1),
                &showing,
                &card,
                "kube-system/coredns-589f44dc88-hdrv5",
            );
        })
        .expect("a frame");
    let said = rows(terminal.backend().buffer()).remove(0);
    println!("{said:?}");

    assert_eq!(
        said,
        format!("  ● {tail}"),
        "nothing is drawn where the name would go"
    );
    assert!(
        !said.contains(CUT),
        "and a mark with no name under it is not a cut, it is a glyph: {said:?}"
    );
}

/// **A refused Alerts pane marks what did come back**, because the badge beside `ALERTS` counts
/// exactly those cards — a sidebar reading `1 ●` over a browser marking nothing is the
/// disagreement `screens/resources.md` § Rules exists to forbid.
#[test]
fn a_refused_alerts_pane_marks_whatever_did_come_back() {
    let now = now();
    let pods = table("table-pods");
    let said = "You can only see the namespaces your kubeconfig points at.";
    let alerts = Pane::Denied(
        said.to_owned(),
        vec![filed(
            Severity::Critical,
            ObjectKind::Deployment,
            &pods.rows[0],
            3,
        )],
    );
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let drawn = spacious(&opened(), &bleeding(&ready, &alerts, &kinds, &now));
    println!("{}", rows(&drawn).join("\n"));

    assert_eq!(column(&row(&drawn, "hdrv5"), gutter()), '●');
    assert!(holds(&drawn, "1 ●"), "and the badge counts the same card");
}

// --- THE ANALYSIS PANE ---
//
// **Every pane below is computed, never built** — `analysis.rs`'s own seven producers over the
// committed captures, driven the way `analysis_tests` drives them (NOTES § D53). A pane written
// out by hand here would prove that the renderer draws what this file typed, which is the one
// thing nobody needs to know; what it has to prove is that one code path draws all seven.
//
// **The loaders are this module's own.** `analysis_tests` has the same ones and they are
// `pub(super)` to its own tree, out of reach from here, and no `lib.rs` exists to share them
// through (invariant 11, NOTES § D50).

/// The instant `analysis_tests` builds its snapshot at, so a capture's own timestamps read the
/// same here as they do one layer down.
fn pinned() -> Time {
    Time(
        "2026-08-23T00:00:00Z"
            .parse()
            .expect("the pin is a timestamp"),
    )
}

fn capture(name: &str) -> serde_json::Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {path} could not be read: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("fixture {path} is not JSON: {e}"))
}

/// `kubectl get -A` answers with `kind: List`, which `k8s_openapi::List<T>` refuses — it wants
/// `PodList` — so the items come out by hand.
fn captured_items<T: k8s_openapi::serde::de::DeserializeOwned>(name: &str) -> Vec<T> {
    capture(name)["items"]
        .as_array()
        .unwrap_or_else(|| panic!("{name}.json has no items array"))
        .iter()
        .map(|value| {
            serde_json::from_value(value.clone())
                .unwrap_or_else(|e| panic!("{name}.json item does not decode: {e}"))
        })
        .collect()
}

fn captured_pod(name: &str) -> PodSnapshot {
    let pod: k8s_openapi::api::core::v1::Pod = serde_json::from_value(capture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
    PodSnapshot::from(pod)
}

/// **One cluster, all seven reports** — the union of the slices `analysis_tests` hands each
/// producer, so no pane below is empty for want of a list nobody fetched. It is one snapshot and
/// not seven because what is being drawn is one code path: seven snapshots would let a pane be
/// interesting for a reason the renderer had nothing to do with.
///
/// Every field is spelled, so a sixteenth on `ClusterSnapshot` cannot arrive here in silence.
fn cluster() -> ClusterSnapshot {
    let mut pods: Vec<PodSnapshot> = [
        // Capacity's limits row, and the four pods that break a naive version of it.
        "overhead",
        "nolimits",
        "podlimit",
        "healthy",
        "healthy-podlevel",
        // Drain safety: the Deployment pods a budget protects live in their own capture.
        "gang",
        "restarts",
        // Posture's host mounts.
        "hostpath",
        "socket",
        "healthy-hostpath",
        // Waste's two pileups.
        "succeeded",
        "failed",
        "evicted",
        "healthy-disk",
        // Restarts: a container serving now that has died often enough to be worth a row.
        "restarts10serving",
        "crashloop",
        "oomserving",
    ]
    .iter()
    .map(|name| captured_pod(name))
    .collect();
    pods.extend(
        captured_items::<k8s_openapi::api::core::v1::Pod>("kube-system-pods")
            .into_iter()
            .map(PodSnapshot::from),
    );
    pods.extend(
        captured_items::<k8s_openapi::api::core::v1::Pod>("healthy-deploy-pods")
            .into_iter()
            .map(PodSnapshot::from),
    );
    let mut nodes: Vec<NodeSnapshot> = captured_items::<k8s_openapi::api::core::v1::Node>("nodes")
        .into_iter()
        .map(NodeSnapshot::from)
        .collect();
    // **One machine promised more than it has.** Every node in the capture has room — a kind
    // cluster nobody has loaded does — so the state `screens/analysis.md` § Capacity opens on is
    // planted, one field moved on the way in (NOTES § D40), and nothing on disk is touched. What
    // the plant is for is the renderer: a pane needs a banded row and a plain one to show that
    // both start in the same column.
    nodes[0].allocatable_cpu = Some("100m".to_owned());
    ClusterSnapshot {
        now: pinned(),
        pods,
        nodes,
        workloads: Vec::new(),
        server_version: Some(
            std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/K8S_VERSION"
            ))
            .expect("the capture stamps the version it came from")
            .trim()
            .to_owned(),
        ),
        context: Some("kind-k8rs".to_owned()),
        // **No kubeconfig certificate here, and that is a guard's ruling and not a gap.**
        // `scripts/certs-test.sh` admits three files as readers of the committed certificates and
        // takes each one's instant from its own `fn now() -> Time`; this file's `now` is the
        // Alerts card ladder's moment, and a second instant in it is exactly what that guard
        // exists to refuse. So the Certificates pane below draws its other row, and C1's own is
        // `analysis_tests`' to prove — what this file is about is the drawing.
        client_certificate: None,
        namespace_scope: None,
        replica_sets: Some(
            captured_items::<k8s_openapi::api::apps::v1::ReplicaSet>("healthy-replicasets")
                .into_iter()
                .map(Into::into)
                .collect(),
        ),
        services: Some(
            captured_items::<k8s_openapi::api::core::v1::Service>("services")
                .into_iter()
                .map(Into::into)
                .collect(),
        ),
        endpoint_slices: Some(
            captured_items::<k8s_openapi::api::discovery::v1::EndpointSlice>("endpointslices")
                .into_iter()
                .map(Into::into)
                .collect(),
        ),
        claims: Some(
            captured_items::<k8s_openapi::api::core::v1::PersistentVolumeClaim>(
                "persistentvolumeclaims",
            )
            .into_iter()
            .map(Into::into)
            .collect(),
        ),
        disruption_budgets: Some(
            captured_items::<k8s_openapi::api::policy::v1::PodDisruptionBudget>(
                "poddisruptionbudgets",
            )
            .into_iter()
            .map(Into::into)
            .collect(),
        ),
        // **The signer moved on the way in** — the D40 plant `analysis_tests` uses for this row,
        // because `csr-pending.json` is a *human* asking for a kubeconfig and the row is about
        // machines. Nothing on disk is touched.
        certificate_requests: Some(
            vec![capture("csr-pending")]
                .into_iter()
                .map(|value| {
                    let mut object: k8s_openapi::api::certificates::v1::CertificateSigningRequest =
                        serde_json::from_value(value)
                            .expect("csr-pending.json is a CertificateSigningRequest");
                    object.spec.signer_name =
                        "kubernetes.io/kube-apiserver-client-kubelet".to_owned();
                    object.into()
                })
                .collect(),
        ),
        metrics: None,
    }
}

/// **The seven, in sidebar order, each from its own producer over [`cluster`]** — and the
/// findings the rule engine returned for that same snapshot, because two of them restate a card
/// a rule already made.
fn reported() -> Vec<(&'static str, Report)> {
    let cluster = cluster();
    let findings = crate::rules::analyze(&cluster);
    vec![
        ("capacity", crate::analysis::capacity(&cluster, &findings)),
        (
            "certificates",
            crate::analysis::certificates(&cluster, &findings),
        ),
        (
            "drain safety",
            crate::analysis::drain_safety(&cluster, &findings),
        ),
        ("posture", crate::analysis::posture(&cluster, &findings)),
        ("restarts", crate::analysis::restarts(&cluster, &findings)),
        ("waste", crate::analysis::waste(&cluster, &findings)),
        ("versions", crate::analysis::versions(&cluster, &findings)),
    ]
}

/// The sidebar as the caller builds it: every report named, every one of them computed.
fn entries<'a>(reports: &'a [(&'static str, Report)]) -> Vec<(&'a str, Option<&'a Report>)> {
    reports
        .iter()
        .map(|(label, report)| (*label, Some(report)))
        .collect()
}

/// An `App` looking at the nth report, with the sidebar cursor on its entry — which is what every
/// frame in `screens/analysis.md` draws.
fn opened_report(nth: usize) -> App {
    let mut app = app();
    app.view = View::Analysis(nth);
    let rows = views::sidebar(&[], 7, None);
    let picks = views::selectable(&rows, |item| item.selectable());
    let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
    let at = picks
        .iter()
        .position(|row| rows[*row] == NavItem::Report(nth))
        .expect("the sidebar names every report");
    app.nav.select(at, &anchors);
    app
}

/// The body of the content pane, one string per line, with the pad kept — every column assertion
/// below counts from the pane's own left edge.
///
/// Rows 2 to 17 are the 16 body lines at the floor: row 0 is the header, row 1 opens the frame and
/// row 18 is the rule under the body, which is what
/// [`the_frame_spends_the_rows_the_way_the_screen_file_does`] asserts for itself.
fn body(buffer: &Buffer) -> Vec<String> {
    rows(buffer)[2..18]
        .iter()
        .map(|line| pane(line).trim_end_matches('\u{2502}').to_owned())
        .collect()
}

/// **The title is the pane's first line, and it is not a row** (`screens/analysis.md` rule 5), so
/// whatever the reader has scrolled to it is still there. Both halves are asserted: the title on
/// the first line before the scroll, and the same line after the cursor has walked the pane to
/// its end.
#[test]
fn the_title_is_on_the_first_line_and_does_not_scroll_with_the_rows() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let reports = reported();
    let entries = entries(&reports);

    for (nth, (label, report)) in reports.iter().enumerate() {
        let mut screen = screen(&alerts, &now);
        screen.reports = &entries;
        let mut app = opened_report(nth);
        let first = render(&app, &screen);
        assert_eq!(
            body(&first)[0].trim_end(),
            format!("  {}", fits(&report.title, 55)),
            "{label} opens on its own heading"
        );

        // The cursor walked to the last answer on the pane, which is what scrolls the list.
        let picks = views::selectable(&report.rows, views::answers);
        let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
        app.content.select(picks.len().saturating_sub(1), &anchors);
        let scrolled = render(&app, &screen);
        assert_eq!(
            body(&scrolled)[0].trim_end(),
            format!("  {}", fits(&report.title, 55)),
            "{label} still says what it is after the reader has scrolled:\n{}",
            rows(&scrolled).join("\n")
        );
    }
}

/// **The gutter is two columns whether or not the row has a glyph** (rule 2), so a banded row and
/// a plain one start at the same column — and the glyph itself comes from `theme::band` and from
/// nothing written in `ui.rs` (rule 1).
#[test]
fn a_row_pays_the_band_gutter_whether_or_not_it_has_a_glyph() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let reports = reported();
    let entries = entries(&reports);
    let (nth, (_, capacity)) = reports
        .iter()
        .enumerate()
        .find(|(_, (label, _))| *label == "capacity")
        .expect("capacity is one of the seven");
    let mut screen = screen(&alerts, &now);
    screen.reports = &entries;
    let drawn = render(&opened_report(nth), &screen);
    println!("{}", rows(&drawn).join("\n"));

    // One banded row and one plain one, taken off the report rather than off the screen: what is
    // asserted is that the *renderer* put them in the same column.
    let banded = capacity
        .rows
        .iter()
        .find_map(|row| match row {
            ReportRow::Answer {
                severity: Some(severity),
                text,
                ..
            } => Some((*severity, text.as_str())),
            _ => None,
        })
        .expect("the corpus overcommits a node");
    let plain = capacity
        .rows
        .iter()
        .find_map(|row| match row {
            ReportRow::Answer {
                severity: None,
                text,
                ..
            } => Some(text.as_str()),
            _ => None,
        })
        .expect("and leaves the others alone");

    // The first line of each, wrapped the way the pane wraps it — a node name plus its numbers
    // is wider than the 51 columns a row has once the gutter is paid.
    let opens = |text: &str| wrapped(text, 51).first().expect("a row has text").clone();
    let with = row(&drawn, &opens(banded.1));
    let without = row(&drawn, &opens(plain));
    let (colour, signal) = theme::band(banded.0);
    assert_eq!(
        column(&pane(&with), PAD.into()),
        mark(signal).chars().next().expect("a band has a mark"),
        "the band's own glyph, in the gutter's first column: {with:?}"
    );
    // **And its colour is that band's too** — both come from `theme::band` together, so the two
    // carriers cannot disagree (`ui.rs`'s own module doc). The cell is read rather than the
    // string: a glyph drawn in the wrong ink prints identically.
    let y = rows(&drawn)
        .iter()
        .position(|line| *line == with)
        .expect("the row is on the screen");
    let x = 1 + SIDEBAR + 1 + PAD;
    assert_eq!(
        drawn
            .cell((x, u16::try_from(y).expect("a row index")))
            .expect("the gutter's first column is inside the frame")
            .fg,
        ink(colour, Depth::TrueColor),
        "the glyph is drawn in its band's colour"
    );
    assert_eq!(
        column(&pane(&with), (PAD + 1).into()),
        ' ',
        "and the column that keeps it off the name"
    );
    // **Not a byte slice**: the row above opens on a glyph, and `&line[..4]` lands inside it.
    let after = |line: &str, columns: usize| -> String { line.chars().skip(columns).collect() };
    assert_eq!(
        pane(&without)
            .chars()
            .take(usize::from(PAD) + GUTTER)
            .collect::<String>(),
        "    ",
        "a row that makes no judgement pays the same two columns: {without:?}"
    );
    assert_eq!(
        pane(&with).chars().nth(usize::from(PAD) + GUTTER),
        pane(&without).chars().nth(usize::from(PAD) + GUTTER),
        "so both names start in the same column"
    );
    assert!(
        after(&pane(&with), usize::from(PAD) + GUTTER).starts_with(&opens(banded.1)),
        "and the row's text follows the gutter: {with:?}"
    );
}

/// **Rows wrap and never clip** (rule 4) — the one place this page differs from an Alerts card,
/// where the name clips and the age keeps its columns. The paragraph asserted here is 63 columns
/// against a 49-column measure, so it cannot survive on one line.
#[test]
fn a_row_wraps_onto_a_second_line_and_nothing_on_the_pane_is_cut() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let reports = reported();
    let entries = entries(&reports);
    let (nth, _) = reports
        .iter()
        .enumerate()
        .find(|(_, (label, _))| *label == "waste")
        .expect("waste is one of the seven");
    let mut screen = screen(&alerts, &now);
    screen.reports = &entries;
    let drawn = render(&opened_report(nth), &screen);
    println!("{}", rows(&drawn).join("\n"));

    let said = "This Service points at nothing. Anything calling it gets a 503.";
    let wrapped = wrapped(said, 49);
    assert!(wrapped.len() > 1, "the fixture sentence is what wraps it");
    for line in &wrapped {
        let drawn_line = pane(&row(&drawn, line));
        assert!(
            drawn_line.starts_with(&format!("      {line}")),
            "every part of it is drawn whole, indented under the row: {drawn_line:?}"
        );
    }
    assert!(
        !holds(&drawn, CUT),
        "and nothing on this page is shortened:\n{}",
        rows(&drawn).join("\n")
    );
}

/// **The cursor lands on an `Answer` and skips a `Prose` and a `NotComputed`** (NOTES § D127) —
/// the variant decides it, never a field.
///
/// It is read off the **scroll**, which is the only thing a selection moves on this pane: the
/// drain pane opens on a `Prose`, so a cursor walking the raw rows and one walking
/// [`views::selectable`]'s answer are one row apart, and at the foot of a list taller than the
/// body that is the difference between the last answer being on screen and being off it.
#[test]
fn the_cursor_lands_only_on_an_answer() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let reports = reported();
    let entries = entries(&reports);
    let (nth, (_, drain)) = reports
        .iter()
        .enumerate()
        .find(|(_, (label, _))| *label == "drain safety")
        .expect("drain safety is one of the seven");
    assert!(
        matches!(drain.rows.first(), Some(ReportRow::Prose(_))),
        "the pane opens on the line that says a drain assumes --ignore-daemonsets"
    );

    let picks = views::selectable(&drain.rows, views::answers);
    assert_eq!(
        picks.first(),
        Some(&1),
        "so the first row the cursor may land on is the second row"
    );
    let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
    let mut screen = screen(&alerts, &now);
    screen.reports = &entries;
    let mut app = opened_report(nth);
    app.content.select(picks.len() - 1, &anchors);
    let drawn = render(&app, &screen);
    println!("{}", rows(&drawn).join("\n"));

    let last = drain.rows.last().expect("the pane has rows").clone();
    let ReportRow::Answer { text, .. } = last else {
        panic!("the drain pane ends on an answer")
    };
    assert!(
        holds(&drawn, fits(&text, 51)),
        "the last answer is what the cursor reached:\n{}",
        rows(&drawn).join("\n")
    );
}

/// **The blank lines are the block structure**: a `NotComputed` stands off from the answers
/// around it and keeps one between its two sentences, while the answers themselves pack — which
/// is `screens/analysis.md` § *Live usage, and the one place a missing metrics-server is said*
/// drawn at the real 53-column region, and § *Capacity* above it, where the node rows follow one
/// another with nothing between.
#[test]
fn a_check_that_could_not_run_stands_off_from_the_answers_and_they_pack() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let reports = reported();
    let entries = entries(&reports);
    let (nth, (_, capacity)) = reports
        .iter()
        .enumerate()
        .find(|(_, (label, _))| *label == "capacity")
        .expect("capacity is one of the seven");
    let (reason, ask_for) = capacity
        .rows
        .iter()
        .find_map(|row| match row {
            ReportRow::NotComputed { reason, ask_for } => Some((reason.as_str(), ask_for.as_str())),
            _ => None,
        })
        .expect("this cluster was never asked what it is using, and the pane says so");

    // The foot of the pane, where the row that could not be computed sits.
    let picks = views::selectable(&capacity.rows, views::answers);
    let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
    let mut app = opened_report(nth);
    app.content.select(picks.len() - 1, &anchors);
    let mut screen = screen(&alerts, &now);
    screen.reports = &entries;
    let drawn = render(&app, &screen);
    let lines = body(&drawn);
    println!("{}", rows(&drawn).join("\n"));

    let at = |needle: &str| {
        let first = wrapped(needle, 53).first().expect("a sentence").clone();
        lines
            .iter()
            .position(|line| line.trim_end() == format!("  {first}"))
            .unwrap_or_else(|| panic!("no line opens {first:?}:\n{}", lines.join("\n")))
    };
    let blank = |nth: usize| lines[nth].trim().is_empty();

    let opens = at(reason);
    let way_out = at(ask_for);
    assert!(
        blank(opens - 1),
        "a blank line above it:\n{}",
        lines.join("\n")
    );
    assert!(
        way_out > opens && blank(way_out - 1),
        "one between the reason and the way out"
    );
    let ends = way_out + wrapped(ask_for, 53).len();
    assert!(blank(ends), "and one under it, before the answers resume");
    assert!(
        !lines[ends + 1].trim().is_empty(),
        "which is one blank line and not two:\n{}",
        lines.join("\n")
    );

    // **And the answers pack.** Two node rows in a row, drawn one under the other.
    let node_rows: Vec<usize> = capacity
        .rows
        .iter()
        .filter_map(|row| match row {
            ReportRow::Answer {
                text,
                detail,
                action,
                ..
            } if detail.is_empty() && action.is_empty() => Some(text.as_str()),
            _ => None,
        })
        .map(|text| {
            let first = wrapped(text, 51).first().expect("a row").clone();
            lines
                .iter()
                .position(|line| line.contains(&first))
                .unwrap_or_else(|| panic!("no line holds {first:?}"))
        })
        .collect();
    assert!(
        node_rows.len() > 1,
        "the pane draws more than one plain node row, or this proves nothing"
    );
    for pair in node_rows.windows(2) {
        assert_eq!(
            pair[1],
            pair[0] + 1,
            "nothing sits between two answers:\n{}",
            lines.join("\n")
        );
    }
}

/// **The `→ ` action sits under its row, its continuation under the words after the arrow, and
/// it wraps at 47 columns** — the last two lines of `screens/analysis.md` § *How a report is
/// drawn*'s column table, which is 49 for the action line and 47 for what follows it.
///
/// **Two actions, because one cannot see its own measure.** A sentence that breaks in the same
/// place at 45, 47 and 49 columns says nothing about which of the three the renderer used, and
/// most of them do; the pair below is the one that brackets it — the first breaks differently at
/// 45, the second at 49. Both are searched for rather than named, and the search asserting it
/// found each is the point (CLAUDE.md § *a derived list asserts it found something*).
#[test]
fn an_action_indents_under_its_row_and_wraps_at_the_columns_the_table_gives_it() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let reports = reported();
    let entries = entries(&reports);
    let narrower = |action: &str| wrapped(action, 45) != wrapped(action, 47);
    let wider = |action: &str| wrapped(action, 49) != wrapped(action, 47);

    for (which, tells) in [
        (
            "a measure two columns narrower",
            &narrower as &dyn Fn(&str) -> bool,
        ),
        ("a measure two columns wider", &wider),
    ] {
        let (nth, label, action) = reports
            .iter()
            .enumerate()
            .find_map(|(nth, (label, report))| {
                report.rows.iter().find_map(|row| match row {
                    ReportRow::Answer { action, .. } if tells(action) => {
                        Some((nth, *label, action.clone()))
                    }
                    _ => None,
                })
            })
            .unwrap_or_else(|| {
                panic!(
                    "no way out on this screen tells 47 columns from {which}, so this test \
                        would pass whatever the renderer measured"
                )
            });
        let mut screen = screen(&alerts, &now);
        screen.reports = &entries;
        let drawn = render(&opened_report(nth), &screen);
        println!("{}", rows(&drawn).join("\n"));

        let mut lines = wrapped(&action, 47).into_iter();
        let opens = lines.next().expect("an action has a first line");
        assert!(
            pane(&row(&drawn, &opens)).starts_with(&format!("      → {opens}")),
            "{label} ({which}): the arrow four columns into the region, the words two further in"
        );
        for line in lines {
            assert!(
                pane(&row(&drawn, &line)).starts_with(&format!("        {line}")),
                "{label} ({which}): a continuation sits under the words, not under the arrow"
            );
        }
    }
}

/// **A report the store has not answered for yet draws the block every waiting pane draws**, and
/// the sidebar still names it — `screens/states.md` § *Still loading* draws seven ANALYSIS
/// entries with a value beside only the one that reads a file on disk.
#[test]
fn a_report_that_is_not_computed_yet_is_named_in_the_sidebar_and_waits_in_the_pane() {
    let now = now();
    let alerts = Pane::Loading;
    let certificates = badged("13d", Severity::Warn);
    let reports = [
        ("capacity", None),
        ("certificates", Some(&certificates)),
        ("drain safety", None),
    ];
    let mut screen = screen(&alerts, &now);
    screen.reports = &reports;
    let drawn = render(&opened_report(0), &screen);
    println!("{}", rows(&drawn).join("\n"));

    assert!(
        holds(&drawn, "reading the cluster…"),
        "the pane says it is still reading, not that there is nothing"
    );
    assert!(
        row(&drawn, "capacity").contains("capacity"),
        "and the sidebar still names the report"
    );
    assert!(
        !row(&drawn, "capacity").contains('▲'),
        "with no value beside it, because there is nothing to count yet"
    );
    assert!(
        row(&drawn, "certificates").contains("13d"),
        "while the one report that reads a file on disk already has its own"
    );
}

// --- MEASURING AND CUTTING ---

#[test]
fn a_word_wider_than_the_line_is_broken_by_character() {
    assert_eq!(wrapped("aa bb cc", 5), ["aa bb", "cc"]);
    assert_eq!(
        wrapped("supercalifragilistic x", 5),
        ["super", "calif", "ragil", "istic", "x"]
    );
    assert_eq!(wrapped("", 5), Vec::<String>::new());
    // A zero-column line cannot loop forever, and does not lose the word.
    assert_eq!(wrapped("hello", 0), ["hello"]);

    // **The space between two words is a column and is counted as one.** Without it `aa bb`
    // would be judged to fit in four and be drawn five wide.
    assert_eq!(wrapped("aa bb", 4), ["aa", "bb"]);
    assert_eq!(wrapped("aa bb", 5), ["aa bb"]);
    // And it is *added*, not multiplied in: `aaa` + ` ` + `bb` is six and fits in six.
    assert_eq!(wrapped("aaa bb", 6), ["aaa bb"]);

    // A word exactly as wide as the line is not a word that has to be broken.
    assert_eq!(wrapped("abcde", 5), ["abcde"]);
    assert_eq!(wrapped("abcdef", 5), ["abcde", "f"]);

    // **And "exactly as wide as the line" has to be fed a word one of whose own *prefixes* is
    // wider than the whole of it, or the break loop\'s `>` cannot be told from `>=`.** U+FE0E
    // asks for the text presentation of a symbol that is two columns by default, so `🀄︎` measures
    // one column where `🀄` measures two: the word fits the line and its prefix does not, so
    // `fits` hands back a strict prefix of a word that never needed breaking. `>` never asks;
    // `>=` asks and splits it (measured 2026-09-06 — every prefix of a ZWJ family emoji measures
    // 2, so that string, the obvious candidate, cannot tell the two apart).
    let narrowed = "\u{2600}\u{1F004}\u{FE0E}";
    assert_eq!(width(narrowed), 2, "the word is exactly the line");
    assert_eq!(
        width(&narrowed[..narrowed.len() - 3]),
        3,
        "and its prefix is not"
    );
    assert_eq!(wrapped(narrowed, 2), [narrowed]);
}

/// **A prefix is measured, never summed** — the defect under the mutant above.
///
/// `Span::width` measures a grapheme cluster whole and the cluster is not the sum of its
/// characters\' widths, in both directions: `☀️` is base + variation selector, one column summed
/// and **two** measured; a ZWJ family emoji is six summed and **two** measured. A `fits` that
/// added a per-character step therefore returned a prefix wider than the columns it was given,
/// and the pane got a line it had no room for — a silent cut, which `screens/widgets.md` § 7
/// forbids by name.
#[test]
fn a_prefix_is_measured_and_never_overruns_the_columns_it_was_given() {
    let sun = "\u{2600}\u{FE0F}";
    assert_eq!(width(sun), 2, "measured whole");
    assert_eq!(
        sun.chars().map(|c| width(&c.to_string())).sum::<usize>(),
        1,
        "summed"
    );

    // 45 plain columns and six of those clusters: 57 measured, 51 summed. Asking for 51 used to
    // hand back all 57 of them, into a 51-column card region.
    let token = format!("{}{}", "x".repeat(45), sun.repeat(6));
    assert_eq!(width(&token), 57);
    assert!(
        width(fits(&token, 51)) <= 51,
        "{:?} is {} columns",
        fits(&token, 51),
        width(fits(&token, 51))
    );
    assert!(
        width(fits("\u{2600}\u{FE0F}a", 1)) <= 1,
        "one column means one column"
    );

    // And the same defect one caller up: nothing `wrapped` emits is wider than the line.
    for line in wrapped(&token, 51) {
        assert!(width(&line) <= 51, "{line:?} is {} columns", width(&line));
    }

    // The other direction — a cluster narrower whole than its characters sum to — is measured
    // whole too, so a word that fits is never broken up.
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F466}";
    assert_eq!(width(family), 2);
    assert_eq!(fits(family, 2), family);
    assert_eq!(wrapped(family, 2), [family]);
}

#[test]
fn fits_never_cuts_a_character_in_half() {
    assert_eq!(fits("payments/web", 8), "payments");
    assert_eq!(fits("née", 2), "né");
    assert_eq!(fits("née", 0), "");
    // Wide characters cost two columns each, so three of them do not fit in five.
    assert_eq!(fits("日本語", 5), "日本");
}

#[test]
fn the_cut_walks_back_to_a_whole_word_before_it_marks() {
    // Three lines and no more: nothing is cut, nothing is marked.
    let short = cut("aa bb cc dd", 5, EVIDENCE_LINES);
    assert_eq!(short, ["aa bb", "cc dd"]);

    let long = cut(
        "aaaa bbbb cccc dddd eeee ffff gggg hhhh",
        10,
        EVIDENCE_LINES,
    );
    // Three, spelled out rather than read off the constant the code uses: a test that asserts
    // against `EVIDENCE_LINES` agrees with whatever that constant becomes.
    assert_eq!(long.len(), 3, "{long:?}");
    assert_eq!(
        EVIDENCE_LINES, 3,
        "and three is the number the screen file measured"
    );
    // **The exact line, not merely a marker on the end of one.** The third line is nine columns
    // and the marker is the tenth, so nothing is given up: a walk-back here would throw away a
    // word the line had room for.
    assert_eq!(long, ["aaaa bbbb", "cccc dddd", "eeee ffff…"]);

    // And where the third line fills the width, the walk-back is what makes room for the marker.
    let full = cut("aaaaa bbbb ccccc dddd eeeee ffff ggggg", 10, EVIDENCE_LINES);
    assert_eq!(full, ["aaaaa bbbb", "ccccc dddd", "eeeee…"]);
    for line in &full {
        assert!(width(line) <= 10, "{full:?}");
    }
}

/// **The space between two words is the caller's** — an analysis row spells its own gap and the
/// wrap keeps it, because normalising one is redrawing a line a lower layer already decided
/// (`screens/analysis.md` § Capacity). What is dropped is only the space a line breaks on.
#[test]
fn a_wrap_keeps_the_spacing_inside_a_line_and_drops_the_one_it_breaks_on() {
    // The row `analysis.rs` builds: the name, three columns, then the numbers.
    let node = "k8rs-worker   0.45 of 12 cpu · 234Mi of 23.1Gi";
    assert_eq!(wrapped(node, 51), [node], "one line, spelled as it arrived");
    let broken = wrapped(node, 30);
    assert_eq!(
        broken,
        ["k8rs-worker   0.45 of 12 cpu ·", "234Mi of 23.1Gi"],
        "the three columns after the name survive the break; the space it broke on does not"
    );
    for line in &broken {
        assert!(width(line) <= 30, "{broken:?}");
    }
    // Leading and trailing whitespace is nobody's alignment.
    assert_eq!(wrapped("  padded  ", 10), ["padded"]);
}

// --- WHAT THE SCREEN ACTUALLY LOOKS LIKE ---

/// Not an assertion about a column — the whole screen, printed, so a reader of the report can
/// compare it with `screens/alerts.md` line by line. `cargo test -- --nocapture`.
#[test]
fn the_alerts_screen_at_the_floor() {
    let now = now();
    let cards = vec![oom(), cordon(Some(at(0))), cordon(None)];
    let alerts = Pane::Ready(cards);
    let capacity = badged("1", Severity::Warn);
    let certificates = badged("30d", Severity::Warn);
    let reports = [
        ("capacity", Some(&capacity)),
        ("certificates", Some(&certificates)),
        ("drain safety", None),
        ("posture", None),
        ("restarts", None),
        ("waste", None),
        ("versions", None),
    ];
    let log = [
        Stripped::of("$ kubectl get pods -A --watch"),
        Stripped::of("$ kubectl get nodes --watch"),
    ];
    let mut screen = screen(&alerts, &now);
    screen.reports = &reports;
    screen.log = &log;

    let drawn = render(&app(), &screen);
    println!("{}", rows(&drawn).join("\n"));
    assert!(holds(&drawn, "▸ ALERTS"));
}

/// The browser, at the floor, for both committed captures and for both namespace scopes — printed
/// so a reader of the report can compare it with `screens/resources.md` line by line.
/// `cargo test -- --nocapture`.
#[test]
fn the_browser_screen_at_the_floor() {
    let now = now();
    let workloads: Vec<Browsable> = ["deployments", "statefulsets", "daemonsets", "pods", "jobs"]
        .into_iter()
        .map(|plural| browsable(plural, true))
        .collect();
    let log = [Stripped::of("$ kubectl get deployments -n payments")];

    for (fixture, at, namespace) in [
        ("table-deployments", 0, Some("payments")),
        ("table-pods", 3, None),
    ] {
        let ready = Pane::Ready(table(fixture));
        let mut screen = browsing(&ready, &workloads, &now);
        screen.namespace = namespace.map(Stripped::of);
        screen.log = &log;

        let mut app = App::default();
        app.open(NavItem::Group(Group::Workloads));
        app.open(NavItem::Kind(at));
        println!("{}\n", rows(&render(&app, &screen)).join("\n"));
    }
}

/// The browser at the floor **with Alerts bleeding through it** — the gutter, the two bands, and
/// the line under the table, at the 80 columns `screens/resources.md` draws its populated mockup
/// at. Printed so a reader of the report can compare it with that mockup line by line.
/// `cargo test -- --nocapture`.
#[test]
fn the_marked_browser_screen_at_the_floor() {
    let now = now();
    let pods = table("table-pods");
    let alerts = Pane::Ready(vec![
        filed(Severity::Critical, ObjectKind::Deployment, &pods.rows[0], 3),
        filed(Severity::Warn, ObjectKind::DaemonSet, &pods.rows[3], 2),
    ]);
    let ready = Pane::Ready(pods);
    let kinds: Vec<Browsable> = ["deployments", "statefulsets", "daemonsets", "pods", "jobs"]
        .into_iter()
        .map(|plural| browsable(plural, true))
        .collect();
    let log = [Stripped::of("$ kubectl get pods -A")];
    let mut screen = bleeding(&ready, &alerts, &kinds, &now);
    screen.log = &log;

    let mut app = App::default();
    app.open(NavItem::Group(Group::Workloads));
    app.open(NavItem::Kind(3));
    println!("{}", rows(&render(&app, &screen)).join("\n"));
}

/// **More columns than the pane has room for, which no committed capture has and every wide CRD
/// can.** The nine columns of `table-pods` all promoted to `priority: 0` want 102 of the 51 the
/// floor gives them.
///
/// What is asserted is that the screen stays a screen: the frame is intact, the header and the
/// rows still line up, and nothing panics. Which columns survive is ratatui's solver, not a rule
/// `screens/` states — the one thing this file promises is that the name is what gives way first,
/// and at this width it has already given everything it has.
#[test]
fn more_columns_than_the_pane_can_hold_still_draws_a_frame() {
    let now = now();
    let mut pods = table("table-pods");
    for column in &mut pods.columns {
        column.priority = 0;
    }
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let drawn = render(&opened(), &browsing(&ready, &kinds, &now));
    println!("{}", rows(&drawn).join("\n"));

    for line in rows(&drawn) {
        assert_eq!(line.chars().count(), usize::from(MIN_WIDTH), "{line:?}");
    }
    assert!(holds(&drawn, "NAME"), "the header is still drawn");
    assert!(
        holds(&drawn, "pods"),
        "the pane still says which kind it is"
    );
}

/// A server that sent rows and no `priority: 0` column at all — nothing in `screens/` draws it,
/// and the only promise is that it is not a panic.
#[test]
fn rows_with_no_plain_column_draw_nothing_and_do_not_panic() {
    let now = now();
    let mut pods = table("table-pods");
    for column in &mut pods.columns {
        column.priority = 1;
    }
    let ready = Pane::Ready(pods);
    let kinds = [browsable("pods", true)];
    let drawn = render(&opened(), &browsing(&ready, &kinds, &now));
    println!("{}", rows(&drawn).join("\n"));
    assert!(
        holds(&drawn, "pods"),
        "the pane still says which kind it is"
    );
    assert!(!holds(&drawn, "NAME"), "no column survived the filter");
    assert!(
        !rows(&drawn)
            .iter()
            .any(|line| pane(line).starts_with(MARKER)),
        "no marker over a row with nothing in it:\n{}",
        rows(&drawn).join("\n")
    );
}

/// **All seven reports, each at the 80×24 floor, printed** — the box's own claim, which is that
/// one code path draws every one of them and no line of `ui.rs` names a report. Not an assertion
/// about a column: a reader of the report compares these with `screens/analysis.md` pane by pane.
/// `cargo test -- --nocapture`.
///
/// What is asserted is only what a printed pane cannot show by itself: that the seven came from
/// seven different producers over one snapshot, and that every one of them drew something.
#[test]
fn every_analysis_pane_at_the_floor() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let log = [Stripped::of("$ kubectl get nodes -o json")];
    let reports = reported();
    let entries = entries(&reports);
    assert_eq!(reports.len(), 7, "seven reports, seven sidebar entries");

    for (nth, (label, report)) in reports.iter().enumerate() {
        let mut screen = screen(&alerts, &now);
        screen.reports = &entries;
        screen.log = &log;
        let drawn = render(&opened_report(nth), &screen);
        println!("=== {label} ===\n{}\n", rows(&drawn).join("\n"));

        assert!(
            !report.rows.is_empty(),
            "{label} has something to say, or this pane proves nothing"
        );
        // **A line of this report's own**, and not merely a pane with something in it.
        //
        // **The row asserted is the first one the cursor lands on, not the first one in the
        // list**, and the difference is real: a `List` scrolls to keep the selected item whole, so
        // a report that opens on a `Prose` followed by an answer as tall as the whole body draws
        // the answer and not the preamble. The drain pane in this corpus is exactly that — one
        // node carrying three stacked problems — and it is a fixture at an extreme rather than
        // the state `screens/analysis.md` draws, where the same preamble sits above a four-line
        // answer and stays on screen.
        let first = report
            .rows
            .iter()
            .find_map(|row| match row {
                ReportRow::Answer { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .or_else(|| {
                report.rows.first().map(|row| match row {
                    ReportRow::Answer { text, .. } | ReportRow::Prose(text) => text.as_str(),
                    ReportRow::NotComputed { reason, .. } => reason.as_str(),
                })
            })
            .expect("a report with rows has a first row");
        let opens = wrapped(first, 51).first().expect("a row has text").clone();
        assert!(
            holds(&drawn, &opens),
            "{label} drew its own first row:\n{}",
            rows(&drawn).join("\n")
        );

        // **What is under the fold, for the panes that have one** — and the same frame is rule 5
        // printed: the heading is still on the first line after the list has scrolled.
        let picks = views::selectable(&report.rows, views::answers);
        let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
        let mut deeper = opened_report(nth);
        deeper
            .content
            .select(picks.len().saturating_sub(1), &anchors);
        let scrolled = render(&deeper, &screen);
        if rows(&scrolled) != rows(&drawn) {
            println!(
                "=== {label}, at the last row the cursor can reach ===\n{}\n",
                rows(&scrolled).join("\n")
            );
        }
    }
}

// --- THE DETAIL TABS ---
//
// **The fixtures are committed captures wherever a pane reads a pod** (NOTES § D53): `oom`,
// `crashloop`, `evicted`, `healthy-sidecar` and `pending` are the five real container shapes
// describe has to draw, and none of them is hand-written JSON. What *is* constructed here is
// `k8s::Happened` and `k8s::LogLines` — k8rs's own types, decoded and proven in `k8s_tests.rs`,
// the same reason `Card` and `Finding` are constructed above.

/// The pane's own columns — the sidebar, the divider and the frame's right border dropped — so an
/// assertion reads as the row a person sees.
fn said(line: &str) -> String {
    pane(line).trim_end_matches(['│', ' ']).to_owned()
}

/// **A moment after every committed capture's own stamps**, so `created 4 days ago` is a real rung
/// of the ladder. [`now`] is four minutes after a *constructed* stamp and is four years *before*
/// these, which draws no age at all — correctly, by [`crate::rules::age`]'s skew guard.
const READING: i64 = 1_787_616_000;

fn later() -> Time {
    Time(Timestamp::from_second(READING).expect("a representable second"))
}

/// A stamp `seconds` before [`later`].
fn ago(seconds: i64) -> Time {
    Time(Timestamp::from_second(READING - seconds).expect("a representable second"))
}

/// One event, in the five fields [`crate::k8s::Happening`] carries.
fn happening(
    at: Option<Time>,
    reason: &str,
    message: &str,
    count: Option<i32>,
    first: Option<Time>,
) -> crate::k8s::Happening {
    crate::k8s::Happening {
        at,
        reason: reason.to_owned(),
        message: message.to_owned(),
        count,
        first,
    }
}

/// The two events `screens/detail.md` § A repeated event measured — the `Unhealthy` that happened
/// 2,383 times over four days, and a `Pulled` that happened once — side by side, so the fourth
/// line and its absence are on one screen.
fn measured() -> crate::k8s::Happened {
    crate::k8s::Happened {
        lines: vec![
            happening(
                Some(ago(180)),
                "Unhealthy",
                "Readiness probe failed: HTTP probe failed with statuscode: 503",
                Some(2383),
                Some(ago(345_600)),
            ),
            happening(
                Some(ago(14_400)),
                "Pulled",
                "Successfully pulled image \"payments/web:2.3.1\"",
                Some(1),
                None,
            ),
        ],
        cut: false,
    }
}

/// **A pod and its containers in `spec` order** — `k8s::PodRead`'s own pairing, rebuilt from the
/// same capture because that type has no constructor outside the frozen `k8s.rs`.
///
/// **`spec.containers[]` then `spec.initContainers[]`, never `status.containerStatuses`'** order:
/// the kubelet sorts that one by name, and reading it instead is what opened `alpha` where
/// `kubectl logs` opens `zeta` (`k8s-admin`, 2026-08-30).
fn declared_by(name: &str) -> (PodSnapshot, Vec<String>) {
    let pod: k8s_openapi::api::core::v1::Pod = serde_json::from_value(capture(name))
        .unwrap_or_else(|e| panic!("{name}.json is not a Pod: {e}"));
    let spec = pod.spec.clone().unwrap_or_default();
    let names = spec
        .containers
        .iter()
        .chain(spec.init_containers.iter().flatten())
        .map(|container| container.name.clone())
        .collect();
    (PodSnapshot::from(pod), names)
}

fn paired<'a>(
    pod: &'a PodSnapshot,
    names: &'a [String],
) -> Vec<(&'a str, Option<&'a ContainerSnapshot>)> {
    names
        .iter()
        .map(|name| {
            (
                name.as_str(),
                pod.containers.iter().find(|held| held.name == *name),
            )
        })
        .collect()
}

/// Everything a detail pane is drawn from, owned, so the borrows in [`Detail`] have somewhere to
/// live for the length of a test.
struct Open<'a> {
    object: ObjectId,
    /// The card this object is filed under, whose findings every tab pins. `None` is the ordinary
    /// tab test: a healthy object the browser opened, which Alerts has no card about.
    card: Option<Card>,
    logs: Pane<Logs<'a>>,
    read: Pane<Described<'a>>,
    yaml: Pane<String>,
    events: Pane<crate::k8s::Happened>,
    secret: bool,
}

impl<'a> Open<'a> {
    /// Nothing has answered — which is the state of every tab the reader is not on.
    fn new() -> Self {
        Open {
            object: id(ObjectKind::Pod, Some("payments"), "web-7d9f4"),
            card: None,
            logs: Pane::Loading,
            read: Pane::Loading,
            yaml: Pane::Loading,
            events: Pane::Loading,
            secret: false,
        }
    }

    fn open(&'a self) -> Detail<'a> {
        Detail {
            object: &self.object,
            card: self.card.as_ref(),
            logs: &self.logs,
            read: &self.read,
            yaml: &self.yaml,
            events: &self.events,
            secret_without_keys: self.secret,
        }
    }
}

fn on(tab: Tab) -> App {
    App {
        tab,
        ..App::default()
    }
}

/// Render one detail pane at the 80×24 floor.
fn detailed(app: &App, open: &Detail) -> Buffer {
    detailed_into(&mut app.clone(), open)
}

/// [`detailed`] over the caller's own [`App`], so the offset the frame resolved survives the call
/// ([`render_into`]).
fn detailed_into(app: &mut App, open: &Detail) -> Buffer {
    let alerts = Pane::Ready(vec![oom()]);
    let now = later();
    let mut screen = screen(&alerts, &now);
    screen.detail = Some(Detailed::Tabs {
        open,
        from_step: false,
    });
    render_into(MIN_WIDTH, MIN_HEIGHT, app, &screen)
}

fn logged(lines: &[&str]) -> crate::k8s::LogLines {
    let mut held = crate::k8s::LogLines::default();
    for line in lines {
        held.push((*line).to_owned());
    }
    held
}

/// **The tab row and the underline under the open tab**, for all four tabs, at the columns
/// `screens/detail.md` draws them — read off that file rather than estimated.
///
/// **The logs mockup is the one that does not agree with its three siblings** and is drawn to the
/// rule the other three share: three columns between labels, and an underline of `label + 2`
/// starting at the `‹`. That mockup draws one column and seven; describe (10 at 9), yaml (6 at 20)
/// and events (8 at 27) all read straight off the rule.
#[test]
fn the_tab_row_marks_the_open_tab_and_underlines_exactly_it() {
    for (tab, row, under) in [
        (
            Tab::Logs,
            "  ‹ logs ›   describe   yaml   events",
            "  ──────",
        ),
        (
            Tab::Describe,
            "  logs   ‹ describe ›   yaml   events",
            "         ──────────",
        ),
        (
            Tab::Yaml,
            "  logs   describe   ‹ yaml ›   events",
            "                    ──────",
        ),
        (
            Tab::Events,
            "  logs   describe   yaml   ‹ events ›",
            "                           ────────",
        ),
    ] {
        let open = Open::new();
        let drawn = detailed(&on(tab), &open.open());
        let rows = rows(&drawn);
        assert_eq!(said(&rows[3]), row, "{tab:?} drew the wrong tab row");
        assert_eq!(
            said(&rows[4]),
            under,
            "{tab:?} underlined the wrong columns"
        );
    }
}

/// The object's name is the row above the tabs, `namespace/name` — [`name`]'s one spelling.
#[test]
fn the_object_is_named_above_the_tab_row() {
    let open = Open::new();
    let drawn = detailed(&on(Tab::Events), &open.open());
    assert_eq!(said(&rows(&drawn)[2]), "  payments/web-7d9f4");
}

/// **A count above one earns the fourth line and a count of one does not** — the two events
/// § A repeated event measured, on one screen so the rule and its absence are both visible. The
/// age column pads to the widest age on the pane, which is what makes the phrases line up.
#[test]
fn an_event_that_repeated_says_how_often_and_one_that_did_not_says_nothing_extra() {
    let mut open = Open::new();
    open.events = Pane::Ready(measured());
    let drawn = detailed(&on(Tab::Events), &open.open());
    println!("{}", rows(&drawn).join("\n"));

    assert_eq!(
        said(&row(&drawn, "the health check failed")),
        "  3 min ago    the health check failed",
        "the age pads to `4 hours ago`, the widest on the pane"
    );
    assert_eq!(
        said(&row(&drawn, "Readiness probe")),
        "  (Unhealthy) Readiness probe failed: HTTP probe failed"
    );
    assert_eq!(
        said(&row(&drawn, "happened")),
        "  happened 2,383 times since 4 days ago"
    );
    assert_eq!(
        said(&row(&drawn, "the image is ready")),
        "  4 hours ago  the image is ready"
    );
    assert!(
        !holds(&drawn, "happened 1 times"),
        "a thing that happened once needs no sentence saying so"
    );
}

/// **An event with no stamp draws no age**, rather than one this file invented — and still pays
/// the column, so the phrases below it line up.
#[test]
fn an_event_with_no_age_draws_none_and_still_pays_the_column() {
    let mut open = Open::new();
    let mut happened = measured();
    happened.lines[0].at = None;
    open.events = Pane::Ready(happened);
    let drawn = detailed(&on(Tab::Events), &open.open());
    assert_eq!(
        said(&row(&drawn, "the health check failed")),
        "               the health check failed",
        "no age, and the phrase still starts where the other one's does"
    );
}

/// **Neither an age nor a phrase drops the first line rather than padding a blank one** — the
/// phrase-less, age-less `BackOff` `screens/detail.md` draws, as its two lines and not three.
#[test]
fn an_event_with_neither_an_age_nor_a_phrase_drops_its_first_line() {
    let mut open = Open::new();
    open.events = Pane::Ready(crate::k8s::Happened {
        lines: vec![happening(
            None,
            "BackOff",
            "Back-off restarting failed container app",
            None,
            None,
        )],
        cut: false,
    });
    let drawn = detailed(&on(Tab::Events), &open.open());
    assert_eq!(
        said(&rows(&drawn)[5]),
        "  (BackOff) Back-off restarting failed container app",
        "the row starts on the first body line, with no blank padding above it"
    );
}

/// **A reason no table names prints its own raw word beside the message, nothing invented** — the
/// fall-through that is the ordinary case rather than a carve-out (NOTES § D198).
#[test]
fn a_reason_the_table_does_not_name_keeps_its_raw_word() {
    let mut open = Open::new();
    open.events = Pane::Ready(crate::k8s::Happened {
        lines: vec![happening(
            Some(ago(180)),
            "FailedScheduling",
            "0/3 nodes are available",
            None,
            None,
        )],
        cut: false,
    });
    let drawn = detailed(&on(Tab::Events), &open.open());
    assert_eq!(
        said(&row(&drawn, "3 min ago")),
        "  3 min ago",
        "no phrase, so the head line is the age alone"
    );
    assert_eq!(
        said(&row(&drawn, "FailedScheduling")),
        "  (FailedScheduling) 0/3 nodes are available"
    );
}

/// **A cut read brings the heading back, and the heading withdraws the newest-first promise** —
/// the one state this pane draws a heading at all, with `k8s::EVENTS_KEPT` interpolated rather
/// than a second copy of the number.
#[test]
fn a_read_the_server_cut_says_so_and_takes_back_newest_first() {
    let mut open = Open::new();
    let mut happened = measured();
    happened.cut = true;
    open.events = Pane::Ready(happened);
    let drawn = detailed(&on(Tab::Events), &open.open());
    assert_eq!(
        said(&rows(&drawn)[5]),
        "  events (the first 500 k8rs was given — there are",
        "the heading is the first thing in the pane"
    );
    assert_eq!(
        said(&rows(&drawn)[6]),
        "  more, and these are not the newest):"
    );

    let mut open = Open::new();
    open.events = Pane::Ready(measured());
    let uncut = detailed(&on(Tab::Events), &open.open());
    assert!(
        !holds(&uncut, "events ("),
        "an uncut list draws no heading — the tab label already says what the pane is"
    );
}

/// **Nothing left is not nothing happened**, and the second sentence is what tells them apart —
/// centred here, because with nothing else on the pane this is a whole-screen calm state.
#[test]
fn an_events_pane_with_nothing_in_it_says_why_it_is_empty() {
    let mut open = Open::new();
    open.events = Pane::Ready(crate::k8s::Happened::default());
    let drawn = detailed(&on(Tab::Events), &open.open());
    assert!(holds(&drawn, "○  none right now"));
    assert!(holds(&drawn, "Kubernetes only keeps events for a"));
    assert!(
        said(&row(&drawn, "○  none right now")).starts_with("           "),
        "the calm headline is centred in the pane, not flush left: {:?}",
        said(&row(&drawn, "○  none right now"))
    );
}

/// **A refusal degrades this one tab and says so in the pane** — never a `403`, never the word
/// RBAC, and **never the empty sentence**: *we were not allowed to look* is not *there is
/// nothing*. This is the one a shared three-answer helper got wrong on 2026-09-06.
#[test]
fn a_refused_events_read_draws_the_sentence_and_not_the_empty_state() {
    let mut open = Open::new();
    open.events = Pane::Denied(
        "k8rs can't read this pod's events. Missing permission: list events in payments."
            .to_owned(),
        crate::k8s::Happened::default(),
    );
    let drawn = detailed(&on(Tab::Events), &open.open());
    assert_eq!(
        said(&rows(&drawn)[5]),
        "  k8rs can't read this pod's events. Missing"
    );
    assert!(holds(&drawn, "permission: list events in payments."));
    assert!(
        !holds(&drawn, "none right now"),
        "a refusal must not draw the empty state"
    );
    assert!(!holds(&drawn, "Kubernetes only keeps events"));
}

/// **The identity line, the containers block and the events under it** — describe's three parts,
/// over the committed OOM capture, at the columns `screens/detail.md` draws them.
#[test]
fn describe_draws_the_identity_line_the_containers_and_the_events() {
    let (pod, names) = declared_by("oom");
    let containers = paired(&pod, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    open.events = Pane::Ready(measured());
    let drawn = detailed(&on(Tab::Describe), &open.open());
    println!("{}", rows(&drawn).join("\n"));

    assert_eq!(
        said(&rows(&drawn)[5]),
        "  Pod · running · created 4 days ago"
    );
    assert_eq!(said(&row(&drawn, "containers")), "  containers");
    assert_eq!(
        said(&row(&drawn, "hog")),
        "    hog   failed",
        "the name pads to the widest declared name plus three"
    );
    assert_eq!(
        said(&row(&drawn, "exceeded")),
        "      container exceeded its memory limit — exit 137,",
        "the restart count rides the last line of the row"
    );
    assert_eq!(
        said(&row(&drawn, "10 restarts")),
        "      10 restarts",
        "a detail row's wrap stays at its own indent, unlike a name row's"
    );
    assert_eq!(said(&row(&drawn, "events (")), "  events (newest first)");
    assert!(holds(&drawn, "the health check failed"));
}

/// **A pod carrying `reason: Evicted` says why it failed**, in the same shape an event's row uses
/// — the phrase, then the raw word.
///
/// **`(Evicted)` carries no message and that is a limit, not a bug**: `status.message` is not a
/// field `rules.rs` holds and that file is frozen, so this build has no sentence to put beside it
/// (`screens/detail.md` draws one; `crate::views::identity` says why it cannot).
#[test]
fn describe_says_why_a_pod_failed_when_it_carries_a_reason() {
    let (pod, names) = declared_by("evicted");
    let containers = paired(&pod, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    let drawn = detailed(&on(Tab::Describe), &open.open());
    assert_eq!(
        said(&rows(&drawn)[5]),
        "  Pod · failed · created 2 days ago"
    );
    assert_eq!(
        said(&row(&drawn, "take back room")),
        "  removed by the node to take back room"
    );
    assert_eq!(said(&row(&drawn, "(Evicted)")), "  (Evicted)");
    // `Error` is in no table, so the container row falls through to its exit code alone.
    assert_eq!(said(&row(&drawn, "exit 137")), "      exit 137");
}

/// **A waiting container says its reason in plain language, not the generic `waiting`** — over the
/// committed CrashLoopBackOff capture, wrapping under its own text.
#[test]
fn describe_translates_a_waiting_container_and_counts_its_restarts() {
    let (pod, names) = declared_by("crashloop");
    let containers = paired(&pod, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    let drawn = detailed(&on(Tab::Describe), &open.open());
    assert_eq!(
        said(&row(&drawn, "quitter")),
        "    quitter   keeps crashing and restarting, 10"
    );
    assert_eq!(
        said(&row(&drawn, "restarts")),
        "      restarts",
        "a name-row continuation indents one pad past the name, not under it"
    );
}

/// **The containers block is `spec` order and not the kubelet's** — `healthy-sidecar` declares
/// `app` then the init container `proxy`, and `status.containerStatuses` reports them the other
/// way round.
#[test]
fn the_containers_block_keeps_the_order_the_author_wrote() {
    let (pod, names) = declared_by("healthy-sidecar");
    let containers = paired(&pod, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    let drawn = detailed(&on(Tab::Describe), &open.open());
    let rows = rows(&drawn);
    let at = |needle: &str| rows.iter().position(|line| line.contains(needle));
    assert!(
        at("  app  ") < at("  proxy"),
        "spec order, not kubelet order"
    );
    assert_eq!(said(&row(&drawn, "proxy")), "    proxy   running");
}

/// **A healthy pod's empty events block is not a broken fetch** — the heading loses its
/// `(newest first)`, and the sentence under it says *why* the list is empty. **Left-flush here**,
/// unlike the tab, because it is a section of a pane rather than the whole of one.
#[test]
fn describe_with_no_events_says_nothing_is_left_rather_than_nothing_happened() {
    let (pod, names) = declared_by("healthy-sidecar");
    let containers = paired(&pod, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    open.events = Pane::Ready(crate::k8s::Happened::default());
    let drawn = detailed(&on(Tab::Describe), &open.open());
    assert_eq!(
        said(&row(&drawn, "none right now")),
        "  ○  none right now",
        "flush under the heading, not centred in the pane"
    );
    let rows = rows(&drawn);
    let heading = rows
        .iter()
        .position(|line| said(line) == "  events")
        .expect("the block still has a heading");
    assert!(
        said(&rows[heading + 1]).contains("none right now"),
        "the heading sits directly above the calm line"
    );
    assert!(holds(&drawn, "Kubernetes only keeps events for a"));
}

/// **`▾` where there is something to pick, and nothing where there is not** — a key that does
/// nothing is a bug this product has already shipped once. `previous log:` starts in the same
/// column either way, which is what both mockups draw.
#[test]
fn the_logs_header_offers_a_picker_only_to_a_pod_that_has_one() {
    for (capture, expected) in [
        (
            "healthy-sidecar",
            "  container: app ▾                    previous log: off",
        ),
        (
            "pending",
            "  container: app                      previous log: off",
        ),
    ] {
        let (pod, names) = declared_by(capture);
        let containers = paired(&pod, &names);
        let read = Described {
            snapshot: &pod,
            containers: &containers,
        };
        let held = logged(&["14:21:58  starting worker pool"]);
        let mut open = Open::new();
        open.logs = Pane::Ready(Logs {
            pod: &read,
            container: "app",
            previous: false,
            held: &held,
        });
        let drawn = detailed(&on(Tab::Logs), &open.open());
        assert_eq!(said(&row(&drawn, "container:")), expected, "{capture}");
    }
}

/// **Silent below one drop, exact at and above it** — and the sentence is pinned above the
/// content, where the gap actually is.
#[test]
fn the_dropped_lines_sentence_appears_only_once_something_was_dropped() {
    let (pod, names) = declared_by("pending");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let short = logged(&["14:23:41  connected to postgres"]);
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &short,
    });
    let quiet = detailed(&on(Tab::Logs), &open.open());
    assert!(!holds(&quiet, "dropped from the top"));

    let mut full = crate::k8s::LogLines::default();
    for nth in 0..crate::k8s::LOG_LINES + 142 {
        full.push(format!("line {nth}"));
    }
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &full,
    });
    let drawn = detailed(&on(Tab::Logs), &open.open());
    assert_eq!(
        said(&row(&drawn, "dropped")),
        "  142 lines were dropped from the top to keep this pane"
    );
    assert!(holds(&drawn, "bounded."));
}

/// **A container that has produced nothing is a state, not a hang** (PRIOR-ART § E1) — and the
/// header line above it still says which container and which run.
#[test]
fn a_container_that_has_written_nothing_says_so_rather_than_hanging() {
    let (pod, names) = declared_by("pending");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let held = crate::k8s::LogLines::default();
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &held,
    });
    let drawn = detailed(&on(Tab::Logs), &open.open());
    assert!(holds(&drawn, "○  no logs yet"));
    assert!(holds(&drawn, "Nothing has been written to this"));
    assert!(holds(&drawn, "container: app"));
    assert!(
        !holds(&drawn, "reading the cluster…"),
        "the stream answered — it answered with nothing"
    );
}

/// **`⇧p` on a container that has never restarted** falls back to the run that does exist and says
/// so — k8rs does not print the API's refusal and does not leave the toggle pointed at nothing.
#[test]
fn asking_for_a_previous_run_that_does_not_exist_says_so_and_falls_back() {
    let (pod, names) = declared_by("healthy-sidecar");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: true,
        held: &held,
    });
    let drawn = detailed(&on(Tab::Logs), &open.open());
    assert_eq!(
        said(&row(&drawn, "⇧p —")),
        "  ⇧p — app hasn't restarted, so there's no previous run"
    );
    assert_eq!(
        said(&row(&drawn, "Showing the current")),
        "       to show. Showing the current run instead."
    );

    // A container that *has* restarted gets no such line — `oom`'s `hog` has 10.
    let (restarted, names) = declared_by("oom");
    let containers = paired(&restarted, &names);
    let read = Described {
        snapshot: &restarted,
        containers: &containers,
    };
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "hog",
        previous: true,
        held: &held,
    });
    let drawn = detailed(&on(Tab::Logs), &open.open());
    assert!(!holds(&drawn, "hasn't restarted"));
    assert!(holds(&drawn, "previous log: on"));
}

/// **A log line keeps its own indentation through a wrap** — a stack frame's leading spaces *are*
/// the line — and a line `k8s::text` already cut carries its marker verbatim rather than being cut
/// a second time here.
#[test]
fn a_log_line_keeps_its_indent_and_a_cut_one_keeps_its_marker() {
    let (pod, names) = declared_by("pending");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let held = logged(&[
        "    at Ledger.post(Ledger.java:214)",
        "14:23:51  {\"level\":\"error\"… (shortened by k8rs)",
        "14:23:52  connecting to postgres://payments-db.svc.cluster.local:5432/payments",
    ]);
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &held,
    });
    let drawn = detailed(&on(Tab::Logs), &open.open());
    assert_eq!(
        said(&row(&drawn, "Ledger.post")),
        "      at Ledger.post(Ledger.java:214)",
        "the line's own four spaces survive"
    );
    assert!(holds(&drawn, "(shortened by k8rs)"));
    // Wider than the 53-column pane, so it takes two rows and loses nothing.
    assert!(holds(&drawn, "14:23:52  connecting to"));
    assert!(holds(
        &drawn,
        "postgres://payments-db.svc.cluster.local:5432"
    ));
}

/// **The yaml pane is the document and nothing else**: no reading margin, the API's own key order,
/// and — the one reversal on this page — `\n` surviving, because here the payload *is* the text
/// (NOTES § D198).
#[test]
fn the_yaml_pane_keeps_the_documents_own_lines_and_its_own_left_edge() {
    let mut open = Open::new();
    open.yaml = Pane::Ready(
        "apiVersion: v1\nkind: ConfigMap\ndata:\n  Corefile: |\n    .:53 {\n        errors\n    }\n"
            .to_owned(),
    );
    let drawn = detailed(&on(Tab::Yaml), &open.open());
    let rows = rows(&drawn);
    println!("{}", rows.join("\n"));
    assert_eq!(
        said(&rows[5]),
        "apiVersion: v1",
        "no two-column reading margin: the pane's left edge is the document's"
    );
    assert_eq!(said(&rows[6]), "kind: ConfigMap");
    assert_eq!(said(&rows[7]), "data:");
    assert_eq!(said(&rows[8]), "  Corefile: |");
    assert_eq!(
        said(&rows[9]),
        "    .:53 {",
        "a block scalar's newlines print as the lines they are"
    );
    assert_eq!(said(&rows[10]), "        errors");
}

/// **A Secret's values arrive masked and are drawn exactly as `k8s::document` wrote them** — this
/// pane adds no masking of its own and takes none away.
#[test]
fn a_secrets_values_are_drawn_as_the_sizes_they_were_masked_to() {
    let mut open = Open::new();
    open.yaml = Pane::Ready(
        "kind: Secret\ndata:\n  username: <hidden — 8 bytes>\n  tls.crt: <hidden — 1,172 bytes>\n"
            .to_owned(),
    );
    let drawn = detailed(&on(Tab::Yaml), &open.open());
    assert_eq!(
        said(&row(&drawn, "username")),
        "  username: <hidden — 8 bytes>"
    );
    assert!(holds(&drawn, "tls.crt: <hidden — 1,172 bytes>"));
}

/// **A Secret with no keys says so** — `data: {}` is drawn as the API returned it, and the
/// sentence under it says what an empty map means rather than leaving it to be interpreted.
#[test]
fn a_secret_with_no_keys_says_it_holds_none_yet() {
    let mut open = Open::new();
    open.yaml = Pane::Ready("kind: Secret\ntype: Opaque\ndata: {}\n".to_owned());
    open.secret = true;
    let drawn = detailed(&on(Tab::Yaml), &open.open());
    assert!(holds(&drawn, "data: {}"));
    assert_eq!(
        said(&row(&drawn, "holds no keys")),
        "  This Secret holds no keys yet."
    );

    let mut open = Open::new();
    open.yaml = Pane::Ready("kind: Secret\ndata:\n  username: <hidden — 8 bytes>\n".to_owned());
    let drawn = detailed(&on(Tab::Yaml), &open.open());
    assert!(
        !holds(&drawn, "holds no keys"),
        "a Secret that has keys is told nothing about having none"
    );
}

/// **A tab that has not answered says so, and never says there is nothing** (PRIOR-ART § C2) — the
/// same wait every other pane on this product draws.
#[test]
fn a_tab_that_has_not_answered_is_not_a_tab_with_nothing_in_it() {
    for tab in Tab::ALL {
        let open = Open::new();
        let drawn = detailed(&on(tab), &open.open());
        assert!(
            holds(&drawn, "reading the cluster…"),
            "{tab:?} drew no waiting state"
        );
        assert!(
            !holds(&drawn, "none right now") && !holds(&drawn, "no logs yet"),
            "{tab:?} turned a wait into an empty answer"
        );
    }
}

/// **The name, the tab row and the underline stay pinned while the body scrolls** — a reader who
/// has scrolled has not lost which object they are on.
#[test]
fn scrolling_a_detail_pane_moves_the_body_and_nothing_above_it() {
    let mut open = Open::new();
    open.yaml = Pane::Ready((0..40).map(|nth| format!("key{nth}: value\n")).collect());
    let still = detailed(&on(Tab::Yaml), &open.open());
    let moved = detailed(
        &App {
            tab: Tab::Yaml,
            scroll: offsets(Tab::Yaml, 3),
            ..App::default()
        },
        &open.open(),
    );
    for nth in 2..5 {
        assert_eq!(
            pane(&rows(&still)[nth]),
            pane(&rows(&moved)[nth]),
            "row {nth} is pinned"
        );
    }
    assert_eq!(said(&rows(&still)[5]), "key0: value");
    assert_eq!(said(&rows(&moved)[5]), "key3: value");
}

/// **Follow pins to the bottom, and a manual offset cannot scroll past the end.** The clamp is
/// this file's because how many rows a pane has depends on the width it is drawn at.
#[test]
fn follow_pins_to_the_last_line_and_a_wild_offset_stops_at_the_end() {
    let (pod, names) = declared_by("pending");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let held = logged(&[
        "line 0", "line 1", "line 2", "line 3", "line 4", "line 5", "line 6", "line 7", "line 8",
        "line 9", "line 10", "line 11", "line 12", "line 13", "line 14",
    ]);
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &held,
    });
    let following = detailed(
        &App {
            tab: Tab::Logs,
            following: true,
            ..App::default()
        },
        &open.open(),
    );
    assert!(
        holds(&following, "line 14"),
        "follow shows the newest line:\n{}",
        rows(&following).join("\n")
    );
    assert!(
        !holds(&following, "line 0 "),
        "and has scrolled past the oldest"
    );
    let mut app = App {
        tab: Tab::Logs,
        scroll: offsets(Tab::Logs, 900),
        ..App::default()
    };
    let wild = detailed_into(&mut app, &open.open());
    assert!(
        holds(&wild, "line 14"),
        "an offset past the end clamps to it rather than drawing a blank pane"
    );
    // **And the clamp is kept, not recomputed and dropped** ([`scrolled`]): left at 900, the next
    // `k` would have spent hundreds of presses walking back to an end the screen was already
    // drawing (`crate::views::App::scroll`).
    assert!(
        app.scroll[Tab::Logs.at()] < 900,
        "the offset the frame clamped on screen went back into the state as 900"
    );
}

/// **The first manual scroll out of follow mode steps up from the tail, not from the top of the
/// buffer** — `screens/widgets.md` § 4, *"follow mode (`f`) pins the offset to the bottom and any
/// manual scroll turns it off"*, which is the sentence that rules the row. `screens/detail.md`
/// § When the buffer fills says the reader's half of it — that turning follow off freezes the
/// *view* and not the stream under it — and names no row of its own.
///
/// **The bottom was the renderer's own local and `App::scroll` never learned it.** Follow pinned
/// the pane to `lines.len() - height` while the field stayed on 0, so `scroll_by(-1)` saturated
/// there and one `k` threw the reader to line 0 of the stream they were tailing — the top, which
/// is the opposite end from the line above the one they were watching.
///
/// **The claim is *one line*, measured off the screen at both ends and not off the geometry that
/// produces it**: a pane height, a block stack and a wrap width all move where the window sits,
/// and none of them may decide whether this test passes.
#[test]
fn the_first_scroll_out_of_follow_steps_up_one_line_from_the_tail() {
    let (pod, names) = declared_by("pending");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    // Long enough that the window at the tail and the window at the top share no line at all —
    // which is what lets *jumped to the top* be an assertion rather than an impression.
    let written: Vec<String> = (0..40).map(|nth| format!("line {nth}")).collect();
    let held = logged(&written.iter().map(String::as_str).collect::<Vec<_>>());
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &held,
    });

    /// The stream lines on screen, oldest first. **It asserts it found some** — every claim below
    /// is about where this window sits, and an empty one satisfies all of them.
    fn shown(drawn: &Buffer) -> Vec<usize> {
        let seen: Vec<usize> = rows(drawn)
            .iter()
            .filter_map(|row| {
                said(row)
                    .trim_start()
                    .strip_prefix("line ")
                    .and_then(|nth| nth.parse().ok())
            })
            .collect();
        assert!(
            !seen.is_empty(),
            "no `line N` of the stream is on screen at all:\n{}",
            rows(drawn).join("\n")
        );
        seen
    }

    let mut app = App {
        tab: Tab::Logs,
        following: true,
        ..App::default()
    };
    let followed = shown(&detailed_into(&mut app, &open.open()));
    assert_eq!(
        followed.last(),
        Some(&39),
        "follow is not showing the newest line, so nothing below is about follow"
    );

    // The reader presses `k`.
    app.scroll_by(-1);
    let after = shown(&detailed_into(&mut app, &open.open()));

    assert_eq!(
        after.first().map(|nth| nth + 1),
        followed.first().copied(),
        "one `k` out of follow did not move the window up exactly one line \
         (0 would be the top of the buffer)"
    );
    assert_eq!(
        after.last().map(|nth| nth + 1),
        followed.last().copied(),
        "the line the reader was watching is not the last one off the bottom"
    );
}

/// **A frame that drew no row writes no row back** — [`crate::views::App::scroll`]'s own promise
/// is *the row the last frame actually drew*, and a pane of zero rows drew none.
///
/// **`last` there is the whole line count**, one past the end rather than a row anybody is
/// looking at, so writing it was this field claiming a frame that never happened. Unreachable
/// through [`draw`] today, where every pane is laid out at a `Min`, and it self-heals on the next
/// real frame; what is kept true here is the claim (`tester`, 2026-09-20).
///
/// **The one-row control is what makes it falsifiable**: the same call at a height of 1 does
/// write back, so the assertion above is not satisfied by a function that never writes at all.
#[test]
fn a_pane_with_no_rows_leaves_the_offset_where_it_was() {
    let lines: Vec<Line> = (0..15)
        .map(|nth| Line::raw(format!("line {nth}")))
        .collect();
    let drawn_at = |height: u16, offset: u16| {
        let mut terminal =
            Terminal::new(TestBackend::new(20, 4)).expect("a terminal over a test backend");
        let mut held = offset;
        let mut at = 0;
        terminal
            .draw(|frame| {
                at = scrolled(
                    frame,
                    Rect::new(0, 0, 20, height),
                    &mut held,
                    true,
                    lines.clone(),
                );
            })
            .expect("a frame");
        (held, at)
    };
    assert_eq!(
        drawn_at(0, 3),
        (3, 15),
        "a pane with no rows wrote one past the last line back as the row the reader is on"
    );
    assert_eq!(
        drawn_at(1, 3),
        (14, 14),
        "a pane with a row did not write the row it drew back, so the case above says nothing"
    );
}

/// **The only field a frame writes through its `&mut App` is the offset it resolved** — `ui::draw`
/// takes the state by `&mut` for [`scrolled`]'s write-back alone, and until now a doc comment was
/// the whole of that bound: the signature is a licence to write any field on the way past
/// (`tester`, 2026-09-20). `App` derives `Clone` and `Eq`, so it can be an assertion instead.
///
/// **It renders through [`detailed_into`] and not [`detailed`]**, because the second hands the
/// frame a `clone()` — over that entry point every field comes back unchanged and this would pass
/// whatever `draw` did.
///
/// **The offset has to have actually moved**, or *nothing else changed* is satisfied by a frame
/// that wrote nothing at all.
#[test]
fn a_frame_writes_nothing_through_its_app_but_the_row_it_drew() {
    let (pod, names) = declared_by("pending");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let written: Vec<String> = (0..40).map(|nth| format!("line {nth}")).collect();
    let held = logged(&written.iter().map(String::as_str).collect::<Vec<_>>());
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &held,
    });

    let mut app = App {
        tab: Tab::Logs,
        following: true,
        ..App::default()
    };
    let before = app.clone();
    let _ = detailed_into(&mut app, &open.open());
    assert_ne!(
        app.scroll, before.scroll,
        "the frame resolved no new offset, so nothing below could catch a stray write"
    );

    let mut only = before.clone();
    only.scroll = app.scroll;
    assert_eq!(
        app, only,
        "a frame wrote a field of App other than the scroll offset it resolved"
    );
}

/// **The `(RawReason)` line is the evidence and is drawn dim; the lines above it are not** — and
/// it is dim for being *last*, so a reason no table names still dims the right line when the
/// block is two lines instead of three.
///
/// **All three block sizes, because *last* is not *third* and it is not *any*** — three lines, two
/// lines, and the one line a pod with no `status.reason` at all draws, where the last line is also
/// the first and must stay in the identity's own ink. That one shape was the unfed one
/// (NOTES § D29): `just mutants-diff` turned `last > 0` into `last >= 0` — always true for a
/// `usize` — and no test could tell the difference, because none of them drew a block of one.
#[test]
fn a_pods_own_reason_is_dim_and_the_identity_line_above_it_is_not() {
    let ink_of = |drawn: &Buffer, needle: &str, at: u16| {
        let y = rows(drawn)
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}"));
        drawn
            .cell((at, u16::try_from(y).expect("a row on screen")))
            .map(|cell| cell.style().fg)
            .expect("a cell")
    };
    let dim = Some(ink(theme::DIM, Depth::TrueColor));
    let text = Some(ink(theme::TEXT, Depth::TrueColor));
    let left = 1 + SIDEBAR + 1 + PAD;

    let (pod, names) = declared_by("evicted");
    let containers = paired(&pod, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    let three = detailed(&on(Tab::Describe), &open.open());
    assert_eq!(ink_of(&three, "Pod · failed", left), text);
    assert_eq!(ink_of(&three, "take back room", left), text);
    assert_eq!(ink_of(&three, "(Evicted)", left), dim);

    // Two lines, not three: `Shutdown` is in no table, so there is no phrase between them.
    let mut two = pod.clone();
    two.reason = Some("Shutdown".to_owned());
    let containers = paired(&two, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &two,
        containers: &containers,
    });
    let drawn = detailed(&on(Tab::Describe), &open.open());
    assert_eq!(ink_of(&drawn, "Pod · failed", left), text);
    assert_eq!(
        ink_of(&drawn, "(Shutdown)", left),
        dim,
        "the evidence is still the dim one when the block lost its middle line"
    );

    // One line, not two: a pod with no `status.reason` has no evidence line under its identity,
    // and the one line it does draw is the identity — never dimmed for being last of one.
    let (quiet, names) = declared_by("healthy-sidecar");
    let containers = paired(&quiet, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &quiet,
        containers: &containers,
    });
    let one = detailed(&on(Tab::Describe), &open.open());
    assert_eq!(
        ink_of(&one, "Pod · running", left),
        text,
        "a block of one line is the identity and not the evidence"
    );
}

/// **The cut heading wraps on describe exactly as it does on the events tab** — the clause it
/// exists to say is its second half, so a hard cut at the pane edge takes off precisely the
/// withdrawal and leaves the promise standing (`k8s-admin`, 2026-09-07: 84 columns into 55).
#[test]
fn describe_wraps_the_cut_heading_rather_than_clipping_the_withdrawal() {
    let (pod, names) = declared_by("oom");
    let containers = paired(&pod, &names);
    let mut happened = measured();
    happened.cut = true;
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    open.events = Pane::Ready(happened);
    let drawn = detailed(&on(Tab::Describe), &open.open());
    println!("{}", rows(&drawn).join("\n"));
    assert_eq!(
        said(&row(&drawn, "events (")),
        "  events (the first 500 k8rs was given — there are",
        "the heading wraps at the pane edge like every other free-text block here"
    );
    assert!(
        holds(&drawn, "and these are not the newest"),
        "the withdrawal is the only reason this heading exists and it has to reach the screen"
    );
}

/// **The withdrawal is pinned above the list, not carried inside it** — it is a claim about the
/// whole read rather than a row of it, which is the rule the logs pane one region up already
/// keeps for its dropped-lines sentence: *a sentence that scrolled away with the content would be
/// pointing at nothing*. Measured scrolling off at offset 3 (`k8s-admin`, 2026-09-07).
#[test]
fn the_cut_withdrawal_stays_on_screen_while_the_events_scroll() {
    let mut happened = measured();
    happened.cut = true;
    for nth in 0..12 {
        happened.lines.push(happening(
            Some(ago(60)),
            "BackOff",
            &format!("Back-off restarting failed container app-{nth}"),
            None,
            None,
        ));
    }
    let mut open = Open::new();
    open.events = Pane::Ready(happened);
    let at = |scroll: u16| {
        detailed(
            &App {
                tab: Tab::Events,
                scroll: offsets(Tab::Events, scroll),
                ..App::default()
            },
            &open.open(),
        )
    };
    let still = at(0);
    println!("{}", rows(&still).join("\n"));
    for scroll in [0, 1, 3, 6, 900] {
        let drawn = at(scroll);
        assert!(
            holds(&drawn, "and these are not the newest"),
            "scroll {scroll} took the withdrawal off the pane:\n{}",
            rows(&drawn).join("\n")
        );
    }
    let moved = at(3);
    println!(
        "=== scrolled three rows in ===\n{}",
        rows(&moved).join("\n")
    );
    for nth in 5..7 {
        assert_eq!(
            pane(&rows(&still)[nth]),
            pane(&rows(&moved)[nth]),
            "row {nth} is pinned"
        );
    }
    assert_ne!(
        pane(&rows(&still)[7]),
        pane(&rows(&moved)[7]),
        "and the list under the heading did move — the blank row between them is the list's \
         first row and scrolls with it, which is what the two mockups draw between them"
    );
}

/// **A waiting reason no table names still says what the kubelet said** — the raw word alone is a
/// dead end for the reader it was left to, and the events table two functions away has never done
/// that (`k8s-admin`, 2026-09-07, on `InvalidImageName`: one of rule 3's seven, and a real one).
#[test]
fn a_waiting_reason_the_table_does_not_name_keeps_the_kubelets_message() {
    let (mut pod, names) = declared_by("oom");
    {
        let held = pod
            .containers
            .first_mut()
            .expect("the capture reports one container");
        held.state = ContainerState::Waiting {
            reason: Some("InvalidImageName".to_owned()),
            message: Some(
                "Failed to apply default image tag \"nginx:latest:\": couldn't parse image \
                 reference"
                    .to_owned(),
            ),
        };
    }
    let containers = paired(&pod, &names);
    let mut open = Open::new();
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    let drawn = detailed(&on(Tab::Describe), &open.open());
    println!("{}", rows(&drawn).join("\n"));
    assert_eq!(
        said(&row(&drawn, "hog")),
        "    hog   InvalidImageName",
        "the raw word is still the state word"
    );
    assert!(
        holds(&drawn, "Failed to apply default image tag"),
        "and the kubelet's own sentence is under it, the way an event's message is"
    );
    assert!(
        holds(&drawn, "10 restarts"),
        "the restart count still rides the last line of the row"
    );
}

/// **A refusal draws the sentence *and* whatever did come back — on all four tabs.** The second
/// field of [`Pane::Denied`] is the whole reason that arm exists, and until 2026-09-07 no test fed
/// a refusal that carried anything to logs, describe or yaml, and fed the events tab an empty one
/// (`tester`). `cargo mutants` cannot see this: it replaces function bodies, not match arms.
#[test]
fn a_refusal_draws_the_sentence_and_the_partial_answer_on_every_tab() {
    let (pod, names) = declared_by("oom");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let held = logged(&["the last line before the token expired"]);
    let mut open = Open::new();
    open.logs = Pane::Denied(
        "k8rs can't read this container's log. Missing permission: get pods/log in payments."
            .to_owned(),
        Logs {
            pod: &read,
            container: "hog",
            previous: false,
            held: &held,
        },
    );
    open.read = Pane::Denied(
        "k8rs can't re-read this pod. Missing permission: get pods in payments.".to_owned(),
        Described {
            snapshot: &pod,
            containers: &containers,
        },
    );
    open.yaml = Pane::Denied(
        "k8rs can't read this pod's YAML. Missing permission: get pods in payments.".to_owned(),
        "kind: Pod\nmetadata:\n  name: web-7d9f4\n".to_owned(),
    );
    open.events = Pane::Denied(
        "k8rs can't read this pod's events. Missing permission: list events in payments."
            .to_owned(),
        measured(),
    );
    for (tab, refusal, came_back) in [
        (Tab::Logs, "pods/log", "before the token expired"),
        (Tab::Describe, "re-read this pod", "Pod · running · created"),
        (Tab::Yaml, "pod's YAML", "name: web-7d9f4"),
        (
            Tab::Events,
            "list events in payments.",
            "the health check failed",
        ),
    ] {
        let drawn = detailed(&on(tab), &open.open());
        println!("=== {tab:?} refused ===\n{}", rows(&drawn).join("\n"));
        assert!(
            holds(&drawn, refusal),
            "{tab:?} drew no refusal sentence:\n{}",
            rows(&drawn).join("\n")
        );
        assert!(
            holds(&drawn, came_back),
            "{tab:?} threw away what did come back:\n{}",
            rows(&drawn).join("\n")
        );
        assert!(
            !holds(&drawn, "none right now") && !holds(&drawn, "no logs yet"),
            "{tab:?} turned a refusal into an empty answer"
        );
    }
}

// --- THE FOOTER AND THE HELP SCREEN ---

/// The footer row of a frame drawn at the floor, the frame's own borders and pad dropped.
///
/// **Not [`said`]**, which drops the sidebar's columns too — the footer runs the full width of
/// the frame and has no sidebar to cut off it.
fn footer_of(buffer: &Buffer) -> String {
    unframed(&rows(buffer)[22])
}

/// A row that runs the frame's full width — the borders and the pad dropped, and no sidebar cut
/// off it, which is the whole difference from [`said`].
fn unframed(line: &str) -> String {
    line.trim_matches('│').trim().to_owned()
}

/// **The footer is drawn from [`views::App`] and from nothing the caller hands over** — one
/// `Screen`, three different footers, which is only possible if `ui.rs` asks `App` for it.
///
/// **The `Screen` is deliberately the Alerts one throughout**: what changes between the arms is
/// the `App`, so a footer that came from the caller could not move.
#[test]
fn the_footer_is_the_apps_answer_and_not_the_callers() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let screen = screen(&alerts, &now);

    for (app, expected) in [
        (
            app(),
            "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
        ),
        (
            App {
                view: View::Analysis(0),
                ..App::default()
            },
            "↑↓ move  ⏎ open  esc back  ? all keys  q quit",
        ),
        (
            App {
                modal: Some(views::Modal::Help),
                ..App::default()
            },
            "? or esc to close",
        ),
    ] {
        assert!(
            footer_of(&render(&app, &screen)).starts_with(expected),
            "{:?} drew {:?}",
            app.view,
            footer_of(&render(&app, &screen))
        );
    }
}

/// **The detail tab decides the footer, and it is `Screen::detail` that says one is open** — the
/// same `App::tab` with and without a detail draws two different lines, which is the whole reason
/// `ui.rs` passes what it reads off `Screen::detail` rather than letting `App` guess.
///
/// **The logs tab is drawn over a pod whose containers are known**, because *how many there are*
/// is the second half of what that line says (`screens/detail.md` § Choosing a container) — over a
/// tab that has not answered yet the same `Tab::Logs` draws no `c container` at all, which is what
/// [`the_logs_footer_offers_the_picker_only_where_there_is_one`] is about.
#[test]
fn a_detail_tab_changes_the_footer_and_only_while_one_is_open() {
    let (pod, names) = declared_by("healthy-sidecar");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &held,
    });
    let open = open;
    for (tab, expected) in [
        (
            Tab::Logs,
            "[ ] tabs  f follow  c container  esc back  ? all keys  q quit",
        ),
        (Tab::Describe, "[ ] tabs  esc back  ? all keys  q quit"),
        (Tab::Yaml, "[ ] tabs  esc back  ? all keys  q quit"),
        (Tab::Events, "[ ] tabs  esc back  ? all keys  q quit"),
    ] {
        assert_eq!(footer_of(&detailed(&on(tab), &open.open())), expected);

        // The same tab with nothing open is the list footer — `Tab` alone must not decide it.
        let alerts = Pane::Ready(vec![oom()]);
        let now = now();
        let closed = render(&on(tab), &screen(&alerts, &now));
        assert_eq!(
            footer_of(&closed),
            "↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit",
            "{tab:?} drew a detail footer with no detail open"
        );
    }
}

/// **`q quit` is right-aligned against [`indented`]'s own right edge, and it is the only
/// right-hand zone any footer has** (`screens/help.md`).
///
/// **The mockup draws it one column further left than this**, because `screens/` draws its
/// right-aligned content two columns short of the page's right edge — the header row of that same
/// mockup is 68 wide inside a 70-wide frame. What is drawn here is the shipped [`header`]'s own
/// rule, flush to the `Rect`, which is what the brief's *align, do not transcribe the gap* settles.
#[test]
fn the_help_footer_puts_the_quit_at_the_right_edge_and_the_rest_at_the_left() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let helping = App {
        modal: Some(views::Modal::Help),
        ..App::default()
    };
    let drawn = render(&helping, &screen(&alerts, &now));
    let footer = &rows(&drawn)[22];

    assert!(
        footer.starts_with("│ ? or esc to close  "),
        "the left zone lost its pad or gained a neighbour: {footer:?}"
    );
    assert!(
        footer.ends_with("q quit │"),
        "`q quit` is not against the pad at the right edge: {footer:?}"
    );
    assert!(
        !footer.contains("all keys"),
        "help pointed at itself: {footer:?}"
    );

    // At a wider terminal the zone moves with the edge — it is a right-aligned zone and not a
    // gap somebody counted.
    let wide = render_at(120, MIN_HEIGHT, &helping, &screen(&alerts, &now));
    let row = &rows(&wide)[22];
    assert!(row.starts_with("│ ? or esc to close"), "{row:?}");
    assert!(row.ends_with("q quit │"), "{row:?}");
}

/// **The mockup's own sixteen rows, read out of `screens/help.md`** — the body of the fenced
/// block between the titled top border and the rule under it, borders stripped and the page's
/// trailing pad dropped.
///
/// **The screen file is the fixture here, which is the point.** A test that compares the drawn
/// screen with [`HELP`] compares the implementation with itself: that is exactly what let a
/// `\` line-continuation swallow `Moving around`'s two-column indent and still go green
/// (2026-09-10). `screens/help.md` is the specification, so it is what the assertion reads.
fn mockup() -> Vec<String> {
    let path = format!("{}/screens/help.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the screen file {path} could not be read: {e}"));
    let body: Vec<String> = text
        .lines()
        .skip_while(|line| !line.starts_with("┌ Keys "))
        .skip(1)
        .take_while(|line| !line.starts_with('├'))
        .map(|line| {
            line.trim_start_matches('│')
                .trim_end_matches('│')
                .trim_end()
                .to_owned()
        })
        .collect();
    assert_eq!(
        body.len(),
        16,
        "screens/help.md's key map is no longer sixteen rows — the body's own budget"
    );
    body
}

/// **The key map is the mockup's sixteen lines, and each of them fits the body it is drawn in**
/// (`screens/help.md`, `screens/widgets.md` § 1).
#[test]
fn the_key_map_is_the_sixteen_lines_the_mockup_draws() {
    let drawn: Vec<String> = HELP.lines().map(str::to_owned).collect();
    assert_eq!(drawn, mockup());
    for line in &drawn {
        assert!(
            width(line) <= usize::from(MIN_WIDTH - 2),
            "{line:?} is {} columns, past the body at the floor",
            width(line)
        );
    }
}

/// **Help is the body region and nothing else** (`screens/widgets.md` § 5): the frame's own
/// border takes the title `Keys`, the sidebar and the divider are gone, and the command log
/// strip behind it keeps showing the real commands the run made.
#[test]
fn help_replaces_the_body_and_leaves_the_rest_of_the_frame_alone() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = [
        Stripped::of("$ kubectl get statefulsets -A --watch"),
        Stripped::of("$ kubectl get daemonsets -A --watch"),
    ];
    let mut screen = screen(&alerts, &now);
    screen.log = &log;
    let helping = App {
        modal: Some(views::Modal::Help),
        ..App::default()
    };
    let drawn = render(&helping, &screen);
    let rows = rows(&drawn);

    assert!(
        rows[1].starts_with("┌ Keys ─"),
        "the title is not on the frame's own border: {:?}",
        rows[1]
    );
    assert!(
        !rows[1].contains('┬'),
        "the sidebar divider survived into the help screen: {:?}",
        rows[1]
    );
    assert_eq!(
        rows[0],
        crate::ui::tests::rows(&render(&app(), &screen))[0],
        "the header is a sibling of the body and must not move when `?` opens"
    );
    assert_eq!(unframed(&rows[19]), "$ kubectl get statefulsets -A --watch");
    assert_eq!(unframed(&rows[20]), "$ kubectl get daemonsets -A --watch");
    assert_eq!(rows.len(), usize::from(MIN_HEIGHT), "the frame is 24 rows");

    // **The sixteen body rows are `screens/help.md`'s own, in order**, each starting at the
    // frame's first inner column — no block of its own, so nothing is indented by a border.
    // Read off the screen file and not off [`HELP`], for the reason [`mockup`] gives.
    for (nth, line) in mockup().iter().enumerate() {
        assert_eq!(
            rows[2 + nth],
            format!("│{line:<width$}│", width = usize::from(MIN_WIDTH - 2)),
            "body row {nth}"
        );
    }

    // Nothing of the Alerts screen underneath survives the `Clear`.
    for leaked in ["ALERTS", "OOMKilled", "▸", "payments/web"] {
        assert!(
            !rows[2..18].iter().any(|row| row.contains(leaked)),
            "{leaked:?} showed through the help screen"
        );
    }
}

/// **The three rows `screens/help.md` § *When a key is refused* draws**, read out of that
/// section's own fenced block: the *Changing things* heading and the three keys under it, all
/// three refused, which is the worst case that file draws.
///
/// **Where a row is, found by its own leading text and never by counting** — [`key_map`]'s own
/// rule (*"never by arithmetic"*), applied to the fixtures that check it.
///
/// **It exists because three tests here did count** (`tester`/`k8s-admin`, 2026-09-18): they held
/// the *Changing things* heading at row 12 and its keys at 13, 14, 15, which was true only while
/// two blank separators sat above it and the block held three rows. `screens/help.md` removed the
/// separators to pay for `s` and `r`'s `works on …` lines, every one of those numbers moved, and
/// the arithmetic had no way to say so — it read a different row and compared it happily.
fn row_at(body: &[String], anchor: &str) -> usize {
    body.iter()
        .position(|row| row.starts_with(anchor))
        .unwrap_or_else(|| panic!("no row starting {anchor:?} in {body:?}"))
}

/// **A second reader rather than a second literal**, for [`mockup`]'s reason: the block is six
/// lines of plain text with no border to strip — the heading, three keys and the two `works on …`
/// lines `s` and `r` gained — and the `Changing things` heading in it is asserted below to be the
/// very row [`mockup`] already returns, so the two readers cannot drift into describing two
/// different screens.
fn mockup_refused() -> Vec<String> {
    let path = format!("{}/screens/help.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the screen file {path} could not be read: {e}"));
    let rows: Vec<String> = text
        .lines()
        .skip_while(|line| !line.starts_with("## When a key is refused"))
        .skip_while(|line| !line.starts_with("```"))
        .skip(1)
        .take_while(|line| !line.starts_with("```"))
        .map(|line| line.trim_end().to_owned())
        .collect();
    assert_eq!(
        rows.len(),
        6,
        "screens/help.md § When a key is refused no longer draws the heading and its five rows"
    );
    rows
}

/// **Every fenced block of one `##` section of a screen file, in the file's own order** — a third
/// reader beside [`mockup`] and [`mockup_dialog`], for the reason both of those exist: a test that
/// compares the drawn line with a constant this file also wrote compares the implementation with
/// itself, and the two agreeing today is what makes the next edit to the screen file drift in
/// silence rather than go red (`tester`, 2026-09-12).
///
/// **The section runs to the next `##` and takes its `###` subsections with it**, which is what
/// `screens/dialogs.md` needs: its own § *Detail tabs and Analysis keep their own footer* sits
/// inside § *While the call is running*.
///
/// **A fence indented under a list item is a block too, with that indent taken off its lines** —
/// `screens/context.md` draws the sentences that stand in for an address inside a bullet, and a
/// reader that saw only column-0 fences skipped them without a word.
fn fenced(file: &str, section: &str) -> Vec<Vec<String>> {
    let path = format!("{}/screens/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the screen file {path} could not be read: {e}"));
    let mut blocks = Vec::new();
    let mut open: Option<(usize, Vec<String>)> = None;
    for line in text
        .lines()
        .skip_while(|line| *line != section)
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
    {
        let indent = line.len() - line.trim_start().len();
        match (&mut open, line.trim_start().starts_with("```")) {
            (None, false) => {}
            (None, true) => open = Some((indent, Vec::new())),
            (Some((at, block)), false) => {
                block.push(line.get(*at..).unwrap_or_default().trim_end().to_owned());
            }
            (Some(_), true) => blocks.push(open.take().expect("open").1),
        }
    }
    assert!(
        !blocks.is_empty(),
        "{path} draws nothing under {section:?} — the heading moved or the section went"
    );
    blocks
}

/// The six fenced blocks of `screens/dialogs.md` § *While the call is running*, in its own
/// order: **0** the three labelled rows the state draws · **1** the namespace giving way for a
/// name kept whole · **2** the name giving way too · **3** a namespace too long on its own · and,
/// under § *The command log's own line*, **4** a running command cut in front of its mark and
/// **5** the same command answered.
fn mockup_in_flight() -> Vec<Vec<String>> {
    let blocks = fenced("dialogs.md", "## While the call is running");
    assert_eq!(
        blocks.iter().map(Vec::len).collect::<Vec<_>>(),
        [3, 1, 1, 1, 1, 1],
        "screens/dialogs.md § While the call is running no longer draws one state, three cuts \
         and two log rows"
    );
    blocks
}

/// One row of [`mockup_in_flight`]'s first block, by the label in its left column. That block is
/// `header` / `log` / `footer` rather than a frame, which is why it needs this and
/// [`mockup_dialog`]'s border strip does not fit it.
fn labelled(block: &[String], label: &str) -> String {
    let row = block
        .iter()
        .find(|row| row.starts_with(label))
        .unwrap_or_else(|| panic!("no {label:?} row in {block:?}"));
    row[label.len()..].trim_start().to_owned()
}

/// The three fenced blocks of `screens/help.md` § *While the call is running*, in its own order:
/// **0** the rewritten `X` row · **1** the rewritten *Changing things* heading and the five rows
/// left unchanged beneath it · **2** the footer, which that state already emptied.
fn mockup_paused() -> Vec<Vec<String>> {
    let blocks = fenced("help.md", "## While the call is running");
    assert_eq!(
        blocks.iter().map(Vec::len).collect::<Vec<_>>(),
        [1, 6, 1],
        "screens/help.md § While the call is running no longer draws two rows and a footer"
    );
    blocks
}

/// **Every kind an operation can be pointed at, singular and plural** — `src/main.rs`'s own
/// `KINDS`, the closed set NOTES § D246 means by *literals in the driver*. Transcribed rather than
/// imported because that list is `dev-core`'s and goes away at Phase 12; the singular is what
/// [`every_clause_names_the_plural_its_own_operation_would_send`] hands `ops`, and the plural is
/// what the column budget is measured over.
const KINDS: [(&str, &str); 6] = [
    ("deployment", "deployments"),
    ("statefulset", "statefulsets"),
    ("daemonset", "daemonsets"),
    ("replicaset", "replicasets"),
    ("pod", "pods"),
    ("node", "nodes"),
];

/// A refusal for each of the three keys, from the one answer that may mark one — every permission
/// the operation needs refused at once, which is the shape the drawn row is the same for.
/// [`an_operation_is_refused_when_any_one_of_its_permissions_is`] in `views_tests.rs` is where the
/// slots are set apart.
fn refusing(scale: bool, restart: bool, delete: bool, resource: &'static str) -> Refused {
    let no = |refused: bool| refused.then_some(&crate::ops::Verdict::No);
    Refused::of(resource, [no(scale); 2], [no(restart)], [no(delete)])
}

/// **Nothing refused draws [`HELP`] back, character for character** — the *fail open* half of
/// NOTES § D229 ruling 4 seen from the screen: a run with no probe at all, a probe still in
/// flight, a `Verdict::Yes` and a `Verdict::CouldNotTell` are one screen, and it is the screen
/// `screens/help.md`'s main mockup draws.
#[test]
fn a_key_map_with_nothing_refused_is_the_mockup_untouched() {
    let could_not_tell = crate::ops::Verdict::CouldNotTell("k8rs could not read it".to_owned());
    for (what, answer) in [
        ("no answer yet", None),
        ("yes", Some(&crate::ops::Verdict::Yes)),
        ("could not tell", Some(&could_not_tell)),
    ] {
        let refused = Refused::of("deployments", [answer; 2], [answer], [answer]);
        assert_eq!(key_map(HELP, refused, false, None), HELP, "{what}");
        assert_eq!(
            key_map(HELP, refused, false, None)
                .lines()
                .collect::<Vec<_>>(),
            mockup(),
            "{what} — and the mockup is the fixture, not HELP"
        );
    }
    assert_eq!(
        key_map(HELP, Refused::default(), false, None),
        HELP,
        "nothing selected"
    );
}

/// **All three refused is `screens/help.md` § *When a key is refused*'s own block, row for row**,
/// on top of the twelve rows of the main mockup that the section says do not change.
///
/// **Both halves are read off the screen file** ([`mockup`], [`mockup_refused`]). Comparing the
/// drawn map with a literal this file also wrote would compare the implementation with itself,
/// which is the defect NOTES § D259 shipped once already.
#[test]
fn all_three_refused_is_the_block_the_screen_file_draws() {
    let base = mockup();
    let clause = mockup_refused();
    let heading = row_at(&base, CHANGING_HEADING);
    assert_eq!(
        clause[0], base[heading],
        "the two readers disagree about which row the Changing things heading is"
    );
    let expected: Vec<String> = base[..heading].iter().chain(&clause).cloned().collect();
    assert_eq!(
        key_map(HELP, refusing(true, true, true, "deployments"), false, None)
            .lines()
            .map(str::to_owned)
            .collect::<Vec<String>>(),
        expected,
        "screens/help.md § When a key is refused"
    );
}

/// **The three keys are independent — one, two or all three, in any combination** — and each row
/// only ever answers for itself (`screens/help.md` § *When a key is refused*). All eight.
#[test]
fn each_refused_row_answers_only_for_its_own_key() {
    let base = mockup();
    let clause = mockup_refused();
    for scale in [false, true] {
        for restart in [false, true] {
            for delete in [false, true] {
                let mut expected = base.clone();
                for (anchor, marked) in [SCALE_ROW, RESTART_ROW, DELETE_ROW]
                    .into_iter()
                    .zip([scale, restart, delete])
                {
                    if marked {
                        expected[row_at(&base, anchor)] = clause[row_at(&clause, anchor)].clone();
                    }
                }
                assert_eq!(
                    key_map(
                        HELP,
                        refusing(scale, restart, delete, "deployments"),
                        false,
                        None
                    )
                    .lines()
                    .map(str::to_owned)
                    .collect::<Vec<String>>(),
                    expected,
                    "s {scale} · r {restart} · ctrl-d {delete}"
                );
            }
        }
    }
}

/// **The body stays sixteen rows and every row stays inside the frame, for every kind and every
/// combination** (`screens/help.md`'s own count, `screens/widgets.md` § 1's budget).
///
/// **The longest plural is what decides it**, which is why every entry of [`KINDS`] is fed and
/// not just the `deployments` the mockup happens to draw. `s` and `r` are **tied**, not one
/// tighter than the other — both are 65 columns plus the plural — so `statefulsets` puts both at
/// 77 against the 78 a body row has at the floor: **one column of headroom** over the six kinds
/// that ship, and **13 columns is the longest plural that fits at all**. Those boundaries are
/// asserted below rather than left to this comment, because a number in a doc comment is what the
/// next person builds on — and this comment's own first draft called `s` the tightest of the
/// three, which it never was.
#[test]
fn no_refused_row_outgrows_the_body_for_any_kind_it_can_name() {
    let mut seen = 0;
    for (_, resource) in KINDS {
        for scale in [false, true] {
            for restart in [false, true] {
                for delete in [false, true] {
                    let drawn = key_map(
                        HELP,
                        refusing(scale, restart, delete, resource),
                        false,
                        None,
                    );
                    let lines: Vec<&str> = drawn.lines().collect();
                    assert_eq!(
                        lines.len(),
                        16,
                        "{resource} — the body is no longer sixteen rows"
                    );
                    for line in lines {
                        assert!(
                            width(line) <= usize::from(MIN_WIDTH - 2),
                            "{line:?} is {} columns, past the body at the floor",
                            width(line)
                        );
                    }
                    seen += 1;
                }
            }
        }
    }
    assert_eq!(seen, 48, "a kind or a combination stopped being measured");

    // **The ceiling from both sides, and what a plural off discovery would do.** `deviceclasses`
    // (13) is the longest plural that fits; `storageclasses` (14) is the shortest that does not;
    // `customresourcedefinitions` (25) draws 90. All three are real API plurals and none is a kind
    // `s` can be pointed at — which is the point of feeding them here rather than to [`KINDS`]:
    // what stops a row overflowing is the closed set in the driver, not the arithmetic.
    let widest = |resource| {
        key_map(HELP, refusing(true, true, true, resource), false, None)
            .lines()
            .map(width)
            .max()
            .expect("a body with rows in it")
    };
    assert_eq!(
        widest("deviceclasses"),
        usize::from(MIN_WIDTH - 2),
        "13 columns is no longer the longest plural that fills the body exactly"
    );
    assert_eq!(
        widest("storageclasses"),
        usize::from(MIN_WIDTH - 1),
        "14 columns is no longer the shortest plural that overflows by one"
    );
    assert_eq!(
        widest("customresourcedefinitions"),
        90,
        "the longest plural in a stock cluster no longer draws 90"
    );
}

/// **The word the clause prints is the word the probe would send** — for every kind an operation
/// can be pointed at, the plural in the drawn row is the one `ops` derives from that kind's own
/// `ApiResource`, never a word this layer spelled (`k8s-admin`, 2026-09-12). Four hand-kept lists
/// of these kinds exist — `ops`'s derived one, `main.rs`'s singulars, [`KINDS`] here and whatever
/// Phase 12 passes — and nothing but this tied the word on screen to the word on the wire, so
/// `""` was only the smallest wrong value: `statefulsets` under a selected Deployment is another,
/// and no type catches either.
///
/// **The tie is through [`KINDS`] and is transitive, which is the whole of what is available
/// here.** `ApiResource::plural` is a `String` and [`Refused::of`] takes a `&'static str`, so the
/// drawn clause cannot literally be fed `ops`'s own word; both are checked against [`KINDS`]'s
/// instead. What that catches is the list drifting — which is the one thing four uncoupled copies
/// of these kinds does. What it cannot catch is Phase 12 handing one call the wrong kind's plural.
///
/// **`ctrl-d`'s row is not covered and cannot be from here.** The only place `delete` derives a
/// plural is `ops::removal`, which is private in a frozen file; making it `pub` for a test is not
/// a trade this box gets to make. Two of the three rows are proven — a reader must not take this
/// test as covering the third.
///
/// **`s`'s row is no longer one of the two, and that is the ruling rather than a gap**
/// (`screens/help.md` § When a key is refused, 2026-09-24): that row carries the fixed *not built
/// yet* sentence in every state, so there is nothing for a refusal to append a plural to, and `s`
/// is withheld before `may_i_in` is ever asked. What is left is `r`'s clause, which is where the
/// drift this test exists to catch would show. The `s` row is asserted *unchanged* below instead,
/// so a clause growing back on it fails here rather than quietly reappearing.
#[test]
fn every_clause_names_the_plural_its_own_operation_would_send() {
    let mut seen = 0;
    for (kind, plural) in KINDS {
        for (operation, resource, row, names) in [(
            "restart",
            crate::ops::restartable(kind),
            "    r ",
            format!(" {plural})"),
        )] {
            // A kind the operation does not reach is never asked and is never *refused* — it is
            // withheld, `screens/states.md`'s own word and its own later box.
            let Ok(resource) = resource else { continue };
            assert_eq!(
                resource.plural, plural,
                "{operation} {kind} — the driver's plural is not the one ops would send"
            );
            let drawn = key_map(HELP, refusing(true, true, true, plural), false, None);
            let drawn = drawn
                .lines()
                .find(|line| line.starts_with(row))
                .unwrap_or_else(|| panic!("no {row:?} row to carry the {operation} clause"));
            assert!(
                drawn.contains(&names),
                "the {operation} clause does not name {names:?}: {drawn:?}"
            );
            seen += 1;
        }
    }
    assert_eq!(
        seen, 3,
        "restart reaches three kinds; a kind stopped being measured"
    );
    // **And `s`'s row takes no clause, whatever this login may not do** — every kind, both
    // refusals, one fixed sentence (`screens/help.md` § When a key is refused).
    for kind in ["deployments", "statefulsets", "replicasets"] {
        let drawn = key_map(HELP, refusing(true, true, true, kind), false, None);
        let row = drawn
            .lines()
            .find(|line| line.starts_with("    s "))
            .expect("the `s` row");
        assert_eq!(
            row.trim_end(),
            "    s       not built yet — there is no way yet to type a copy count",
            "a clause grew back on the `s` row for {kind}"
        );
    }
}

/// **Every rewrite lands on the row it is for, found by that row's own text, wherever the block
/// sits and whatever follows it** (`tester`, 2026-09-12 and 2026-09-13). The input is [`HELP`] with
/// *Changing things* moved to the top and a trailing blank row added: a swap anchored by index
/// writes into *Moving around*, and a `str::lines` split drops the trailing row — each was proven
/// red on a copy. Both halves are fed, a dead-writes swap and all three permission clauses, and
/// both expected blocks are read off `screens/help.md`.
#[test]
fn a_key_map_finds_its_rows_by_their_own_text_wherever_the_block_sits() {
    let rows: Vec<&str> = HELP.split('\n').collect();
    let at = rows
        .iter()
        .position(|row| row.starts_with("  Changing things"))
        .expect("HELP's Changing things heading");
    // **The block's size is derived and never written down**: *Changing things* is HELP's last
    // group, so it runs from its heading to the end — and it grew from four rows to six when `s`
    // and `r` gained their `works on …` lines, which a literal `4` here would have survived by
    // shuffling half a block.
    let block = rows.len() - at;
    let mut moved: Vec<&str> = rows[at..].to_vec();
    moved.extend(&rows[..at]);
    moved.push("");
    let input = moved.join("\n");
    let rest: Vec<String> = moved[block..].iter().map(|row| (*row).to_owned()).collect();

    let off = dead_body(Writes::ReadOnly);
    let heading = off
        .iter()
        .position(|row| row.starts_with("  Changing things"))
        .expect("the page's own heading");
    let why = Writes::ReadOnly.why().expect("a dead cause has a sentence");
    let swapped = key_map(
        &input,
        refusing(true, true, true, "deployments"),
        false,
        Some(Held::Off(why)),
    );
    let expected: Vec<String> = off[heading..heading + block]
        .iter()
        .chain(&rest)
        .cloned()
        .collect();
    assert_eq!(
        swapped.split('\n').collect::<Vec<_>>(),
        expected,
        "the dead-writes swap"
    );

    let refused = key_map(
        &input,
        refusing(true, true, true, "deployments"),
        false,
        None,
    );
    let expected: Vec<String> = mockup_refused().into_iter().chain(rest).collect();
    assert_eq!(
        refused.split('\n').collect::<Vec<_>>(),
        expected,
        "the three permission clauses"
    );
    assert_eq!(
        refused.split('\n').next_back(),
        Some(""),
        "the trailing blank row went"
    );
}

/// **`screens/help.md` § *Under a dead-writes run*, in its own order**: **0** the whole
/// `--read-only` frame, all 24 rows at the real 80-column floor (that section's first bullet), so
/// they compare with a rendered frame as they stand · **1** the two rows `Writes::Unaudited`
/// draws in their place.
fn mockup_dead() -> Vec<Vec<String>> {
    let blocks = fenced("help.md", "## Under a dead-writes run");
    assert_eq!(
        blocks.iter().map(Vec::len).collect::<Vec<_>>(),
        [usize::from(MIN_HEIGHT), 2],
        "screens/help.md § Under a dead-writes run no longer draws one frame and two rows"
    );
    blocks
}

/// The sixteen body rows of a frame, borders and trailing pad off — the shape [`key_map`] answers
/// in, whether the frame was rendered or read off a page.
fn body_of(frame: &[String]) -> Vec<String> {
    frame[2..18]
        .iter()
        .map(|row| {
            row.trim_start_matches('│')
                .trim_end_matches('│')
                .trim_end()
                .to_owned()
        })
        .collect()
}

/// **The body `?` draws under one dead cause, read off the page** — the `--read-only` frame, with
/// the heading and the row under it taken from the second block for `Writes::Unaudited`. Found by
/// the heading's own text, never by a row number.
fn dead_body(writes: Writes) -> Vec<String> {
    let page = mockup_dead();
    let mut body = body_of(&page[0]);
    if let Writes::Unaudited(_) = writes {
        let at = body
            .iter()
            .position(|row| row.starts_with("  Changing things"))
            .expect("the page's own heading");
        assert_eq!(body[at], page[1][0], "the two causes draw one heading");
        body.splice(at..at + 2, page[1].iter().cloned());
    }
    body
}

/// **Under either dead cause `?` is that section's frame, row for row** (`screens/help.md` § Under
/// a dead-writes run, NOTES § D265 rulings 3 and 5): the body, the rules, the command log strip and
/// the footer byte for byte, and the header word for word — the page draws `k8rs` one column left
/// of centre and its right zone two short of the edge, [`against_page_header`]'s reason one screen
/// along. **The two causes differ in one row and only that one**, and the audit sentence
/// [`Writes::Unaudited`] carries is on neither.
///
/// **Under `Live` the same `?` is the main mockup's body**, so a swap that ignored the cause —
/// drawing it always, or never — fails one of the two halves.
#[test]
fn help_under_dead_writes_is_the_frame_the_screen_file_draws() {
    let page = mockup_dead();
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = [
        Stripped::of("$ kubectl get statefulsets -A --watch"),
        Stripped::of("$ kubectl get daemonsets -A --watch"),
    ];
    let helping = App {
        modal: Some(views::Modal::Help),
        ..App::default()
    };
    let spoken = |row: &str| row.split_whitespace().collect::<Vec<_>>().join(" ");
    let audit = "k8rs could not open its audit log at /home/you/.local/state/k8rs/audit.log";

    let mut bodies = Vec::new();
    for writes in [Writes::ReadOnly, Writes::Unaudited(audit)] {
        let mut screen = screen(&alerts, &now);
        screen.log = &log;
        screen.writes = writes;
        let drawn = rows(&render(&helping, &screen));
        println!("--- ? under {writes:?} ---\n{}\n", drawn.join("\n"));
        assert_eq!(
            spoken(&drawn[0]),
            spoken(&page[0][0]),
            "{writes:?} — the header"
        );
        for nth in [1, 18, 19, 20, 21, 22, 23] {
            assert_eq!(drawn[nth], page[0][nth], "{writes:?} — frame row {nth}");
        }
        assert_eq!(body_of(&drawn), dead_body(writes), "{writes:?} — the body");
        assert!(
            !drawn.iter().any(|row| row.contains("/home/you")),
            "{writes:?} — the banner's own sentence reached Help"
        );
        bodies.push(body_of(&drawn));
    }
    assert_eq!(
        bodies[0]
            .iter()
            .zip(&bodies[1])
            .filter(|(read_only, unaudited)| read_only != unaudited)
            .count(),
        1,
        "the two causes differ in more than their one row"
    );

    let mut screen = screen(&alerts, &now);
    screen.log = &log;
    assert_eq!(
        body_of(&rows(&render(&helping, &screen))),
        mockup(),
        "Live — the main mockup's body"
    );
}

/// **Dead writes win outright: no refusal and no call in flight puts `s`, `r` or `ctrl-d` back on
/// the body** (`screens/help.md` § When a key is refused, its last bullet, and § Under a
/// dead-writes run, its last) — for all eight refusals, with and without a call running, under both
/// causes.
///
/// **The whole body is compared row by row** (`tester`, 2026-09-13): the first draft only looked
/// for the absence of words, and a `help` that passed `changing && writes.live()` — so the `X` row
/// stopped pausing under dead writes — went straight through it. **The `X` row pauses while a call
/// runs**: switching cluster is not a write, and dead writes say nothing about it.
#[test]
fn dead_writes_draw_no_mutating_key_whatever_is_refused_or_running() {
    let paused = mockup_paused();
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut seen = 0;
    for writes in [
        Writes::ReadOnly,
        Writes::Unaudited("the audit log could not be opened"),
    ] {
        for running in [false, true] {
            let expected: Vec<String> = dead_body(writes)
                .into_iter()
                .map(|row| {
                    if running && row.starts_with("    X ") {
                        paused[0][0].clone()
                    } else {
                        row
                    }
                })
                .collect();
            for scale in [false, true] {
                for restart in [false, true] {
                    for delete in [false, true] {
                        let mut screen = screen(&alerts, &now);
                        screen.writes = writes;
                        screen.refused = refusing(scale, restart, delete, "deployments");
                        let mut app = if running {
                            changing(Some("payments"), "web")
                        } else {
                            app()
                        };
                        app.modal = Some(views::Modal::Help);
                        let frame = rows(&render(&app, &screen));
                        assert_eq!(
                            body_of(&frame),
                            expected,
                            "{writes:?} · running {running} · s {scale} · r {restart} · ctrl-d \
                             {delete}"
                        );
                        seen += 1;
                    }
                }
            }
        }
    }
    assert_eq!(seen, 32, "a combination stopped being drawn");
}

/// The four fenced blocks of `screens/help.md` § *While the link is down, still connecting, the
/// login has expired, or the clock is off*, in its own order: **0** still connecting · **1** the
/// lost link · **2** the expired login · **3** the clock.
///
/// **The heading is quoted, so a section renamed on the page fails here rather than drifting** —
/// which is how it was caught: `screens/help.md` gained the connecting clause and renamed the
/// heading with it, and [`fenced`] panicked with the old title (`tester`, 2026-09-19).
fn mockup_held() -> Vec<String> {
    let blocks = fenced(
        "help.md",
        "## While the link is down, still connecting, the login has expired, or the clock is off",
    );
    assert_eq!(
        blocks.iter().map(Vec::len).collect::<Vec<_>>(),
        [1, 1, 1, 1],
        "that section no longer draws four headings"
    );
    blocks.into_iter().flatten().collect()
}

/// The sentence a clock out of step puts above the pane — any sentence, since what Help reads is
/// that one is drawn and not what it says.
const SKEWED: &str = "⚠ This computer and the cluster disagree about the time.";

/// **Help pauses *Changing things* for the link and the clock, and for nothing else on the body**
/// (`screens/help.md` § While the link is down, still connecting, the login has expired, or the
/// clock is off, NOTES § D265 ruling 4): the heading rewritten, its three rows and the `X` row as
/// the main mockup draws them, and **no permission clause under any of the four**, for all eight
/// refusals.
///
/// **[`Link::Connecting`] is one of the four now that the page has written its row** (todo.md
/// § Phase 12): it was grouped with [`Link::Live`] while no clause existed, so `?` promised `s`,
/// `r` and `ctrl-d` over a link that had never answered.
#[test]
fn help_pauses_changing_things_while_the_link_or_the_clock_withholds_it() {
    let held = mockup_held();
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let helping = App {
        modal: Some(views::Modal::Help),
        ..App::default()
    };
    let mut seen = 0;
    for (what, link, clock, heading) in [
        ("connecting", Link::Connecting, None, &held[0]),
        ("lost", Link::Lost, None, &held[1]),
        ("expired", Link::Expired, None, &held[2]),
        ("clock", Link::Live, Some(SKEWED), &held[3]),
    ] {
        let expected: Vec<String> = mockup()
            .into_iter()
            .map(|row| {
                if row.starts_with("  Changing things") {
                    heading.clone()
                } else {
                    row
                }
            })
            .collect();
        for scale in [false, true] {
            for restart in [false, true] {
                for delete in [false, true] {
                    let mut screen = screen(&alerts, &now);
                    screen.link = link;
                    screen.clock = clock.map(Stripped::of);
                    screen.refused = refusing(scale, restart, delete, "deployments");
                    let frame = rows(&render(&helping, &screen));
                    if !scale && !restart && !delete {
                        println!("--- ? {what} ---\n{}\n", frame.join("\n"));
                    }
                    assert_eq!(
                        body_of(&frame),
                        expected,
                        "{what} · s {scale} · r {restart} · ctrl-d {delete}"
                    );
                    seen += 1;
                }
            }
        }
    }
    assert_eq!(seen, 32, "a combination stopped being drawn");
}

/// **One reason is drawn, and it is the highest-ranked one that holds** — dead writes, then a call
/// in flight, then the link, then the clock (NOTES § D265 ruling 4, `screens/help.md` § While the
/// link is down…, its first bullet). Each case adds every lower reason underneath the one expected
/// to win, so a rank swapped anywhere draws the wrong heading. Every heading is read off the page.
#[test]
fn help_draws_the_highest_ranked_reason_and_only_that_one() {
    let held = mockup_held();
    let off = dead_body(Writes::ReadOnly);
    let running = &mockup_paused()[1][0];
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let heading = |frame: &[String]| {
        body_of(frame)
            .into_iter()
            .filter(|row| row.starts_with("  Changing things"))
            .collect::<Vec<_>>()
    };
    let off_heading = off
        .iter()
        .find(|row| row.starts_with("  Changing things"))
        .expect("the page's own heading");

    for (what, writes, busy, link, expected) in [
        (
            "dead writes",
            Writes::ReadOnly,
            true,
            Link::Lost,
            off_heading,
        ),
        ("a call in flight", Writes::Live, true, Link::Lost, running),
        // **A link still connecting outranks the clock**, which is the only rank it can be in:
        // `ui::clock` answers `None` whenever the link is not `Live`, so a stale reading never
        // competes with a link reason at all — connecting included (that section's first bullet).
        (
            "still connecting",
            Writes::Live,
            false,
            Link::Connecting,
            &held[0],
        ),
        ("the lost link", Writes::Live, false, Link::Lost, &held[1]),
        (
            "the expired login",
            Writes::Live,
            false,
            Link::Expired,
            &held[2],
        ),
        ("the clock", Writes::Live, false, Link::Live, &held[3]),
    ] {
        let mut screen = screen(&alerts, &now);
        screen.writes = writes;
        screen.link = link;
        screen.clock = Some(Stripped::of(SKEWED));
        let mut app = if busy {
            changing(Some("payments"), "web")
        } else {
            app()
        };
        app.modal = Some(views::Modal::Help);
        let frame = rows(&render(&app, &screen));
        assert_eq!(heading(&frame), std::slice::from_ref(expected), "{what}");
    }
}

/// **Help's heading and [`offered`] cannot disagree** — over every combination of the three
/// run-level facts, on an Alerts screen with a card to act on and nothing running, `s` and `r` are
/// offered exactly when *Changing things* carries no clause (NOTES § D265 ruling 4). Asserted off
/// the drawn row and not off [`withheld`], which both of them read.
///
/// **What it asserts is that the two agree, never which answer they agree on**, which is why every
/// value of [`Link`] belongs in the loop — including [`Link::Unconnected`], for which
/// `screens/help.md` writes no clause and `screens/context.md` § *After `esc dismiss`…* writes the
/// reason: *"`s` and `r` do not appear, and nothing pauses them to get there"*. The count below is
/// the one place that answer is recorded.
#[test]
fn help_says_changing_things_is_open_exactly_when_the_footer_offers_it() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let plain = mockup()
        .into_iter()
        .find(|row| row.starts_with("  Changing things"))
        .expect("the main mockup's heading");
    let helping = App {
        modal: Some(views::Modal::Help),
        ..App::default()
    };
    let mut seen = (0, 0);
    for writes in [
        Writes::Live,
        Writes::ReadOnly,
        Writes::Unaudited("the audit log could not be opened"),
    ] {
        for link in [
            Link::Connecting,
            Link::Live,
            Link::Lost,
            Link::Expired,
            Link::Unconnected,
        ] {
            for clock in [None, Some(SKEWED)] {
                let mut screen = screen(&alerts, &now);
                screen.writes = writes;
                screen.link = link;
                screen.clock = clock.map(Stripped::of);
                let open = body_of(&rows(&render(&helping, &screen))).contains(&plain);
                let act = matches!(offered(&app(), &screen), Offer::Act { .. });
                assert_eq!(
                    open, act,
                    "{writes:?} · {link:?} · clock {clock:?} — Help and the footer disagree"
                );
                if act {
                    seen.0 += 1;
                } else {
                    seen.1 += 1;
                }
            }
        }
    }
    assert_eq!(
        seen,
        (3, 27),
        "three combinations of thirty leave the keys live — a live run on a live link with no \
         skew, and the two under `Link::Unconnected`, where nothing pauses them because nothing \
         survived the switch to be selected (`screens/context.md` § After `esc dismiss`, on a \
         switch that failed with a cluster already live). Unreachable, and for that section's own \
         reason rather than for want of a clause. `Link::Connecting` is no longer one of them: \
         `screens/help.md` has written its row (todo.md § Phase 12)"
    );
}

/// **The refused rows reach the real frame**, not just the string that feeds it — the same body
/// region, the same sixteen rows, and the rest of the frame as untouched as it is with nothing
/// refused (`screens/help.md`, its note under the mockup).
#[test]
fn the_refused_key_map_is_what_the_help_screen_draws() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = [
        Stripped::of("$ kubectl get statefulsets -A --watch"),
        Stripped::of("$ kubectl get daemonsets -A --watch"),
    ];
    let mut unmarked = screen(&alerts, &now);
    unmarked.log = &log;
    let mut refused = screen(&alerts, &now);
    refused.log = &log;
    refused.refused = refusing(true, true, true, "deployments");
    let helping = App {
        modal: Some(views::Modal::Help),
        ..App::default()
    };
    let marked = rows(&render(&helping, &refused));
    // **The screen itself, for a reader of the report to hold beside `screens/help.md`** —
    // `every_footer_on_the_screen_it_belongs_to`'s own move, one screen along.
    // `cargo test -- --nocapture`.
    println!(
        "--- ? with all three keys refused ---\n{}\n",
        marked.join("\n")
    );

    let base = mockup();
    let expected: Vec<String> = base[..row_at(&base, CHANGING_HEADING)]
        .iter()
        .chain(&mockup_refused())
        .cloned()
        .collect();
    for (nth, line) in expected.iter().enumerate() {
        assert_eq!(
            marked[2 + nth],
            format!("│{line:<width$}│", width = usize::from(MIN_WIDTH - 2)),
            "body row {nth}"
        );
    }
    assert_eq!(
        marked.len(),
        usize::from(MIN_HEIGHT),
        "the frame is 24 rows"
    );
    assert!(
        marked[1].starts_with("┌ Keys ─"),
        "the title left the frame's own border: {:?}",
        marked[1]
    );
    assert_eq!(
        unframed(&marked[19]),
        "$ kubectl get statefulsets -A --watch"
    );

    // **Only the body changes.** The header, the log strip and the footer are siblings of the
    // body region (`screens/widgets.md` § 1) and a refusal reaches none of them — asserted against
    // the same frame with nothing refused rather than against a padding this file would have to
    // spell, which is the comparison the claim actually is.
    let plain = rows(&render(&helping, &unmarked));
    for nth in [0, 1, 18, 19, 20, 21, 22, 23] {
        assert_eq!(
            marked[nth], plain[nth],
            "frame row {nth} moved under a refusal"
        );
    }
    assert_ne!(marked[15], plain[15], "the `s` row did not change at all");
}

/// **The title is drawn only while `?` is open** — every other screen's frame is untitled
/// (`screens/widgets.md` § 5), and a border that always said `Keys` would be the same defect
/// seen from the other side.
#[test]
fn the_frame_carries_no_title_when_help_is_closed() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let drawn = render(&app(), &screen(&alerts, &now));
    let top = &rows(&drawn)[1];
    assert!(!top.contains("Keys"), "{top:?}");
    assert!(
        top.starts_with("┌────────────────────┬"),
        "the sidebar's own divider is back on an ordinary frame: {top:?}"
    );
}

/// **The one footer carrying a string this product did not choose the length of**
/// (`screens/dialogs.md` § *While the call is running*, `screens/widgets.md` § 7's identity cut).
///
/// **All four cases are read out of that file, not written here** (`tester`, 2026-09-12, and
/// [`mockup_dialog`]'s own reason one screen along): the short name that draws whole; the
/// namespace giving way from its front for a canary whose own name is kept whole; the name giving
/// way from its front too, once even `…/<name>` does not fit; and a namespace too long on its own.
/// They agreed with this file's own literals byte for byte on the morning they were written, which
/// is exactly what makes a literal drift rather than fail, and the page's cut changed under them
/// twice (NOTES § D266 is the second).
///
/// **The last fixture is `openshift-cluster-node-tuning-operator`**, 38 characters and a namespace
/// a real distribution ships. Under the tail-cut this replaces, *every object in it* drew one
/// byte-identical line; the `/` clause held and the name was gone. That second assertion is
/// [`every_object_in_one_long_namespace_still_draws_its_own_name`], because this test would pass
/// on a line that identified nothing.
///
/// **Both cut lines are asserted at exactly 76 columns** — the ceiling [`indented`] leaves at the
/// 80×24 floor — and no number in `ui.rs` says what `room` is: it is measured off this same
/// footer asked for with an empty name.
///
/// **The bare cluster-scoped name is the one case `screens/dialogs.md` draws no line for**, so it
/// is derived from that file's own short line rather than typed again: same fixed words, the one
/// name swapped. `ui::name` joins `None` to nothing, and `/node-3` is the row a `Some("")` would
/// draw instead.
#[test]
fn the_in_flight_footer_cuts_the_name_and_nothing_else() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let screen = screen(&alerts, &now);
    let mockup = mockup_in_flight();
    let expected = labelled(&mockup[0], "footer");

    let short = footer_of(&render(&changing(Some("payments"), "web"), &screen));
    assert_eq!(short, expected, "screens/dialogs.md's own mockup row");

    for (nth, namespace, name) in [
        (
            1,
            "team-alpha-payments-platform",
            "checkout-worker-service-canary",
        ),
        (
            2,
            "payments",
            "checkout-worker-service-account-token-projector",
        ),
        (3, "openshift-cluster-node-tuning-operator", "tuned"),
    ] {
        let drawn = footer_of(&render(&changing(Some(namespace), name), &screen));
        println!("{drawn}");
        assert_eq!(
            drawn, mockup[nth][0],
            "screens/dialogs.md § While the call is running, its case {nth} block"
        );
        assert_eq!(width(&drawn), 76, "the floor's own ceiling, exactly");
    }

    let node = footer_of(&render(&changing(None, "node-3"), &screen));
    assert_eq!(node, expected.replace("payments/web", "node-3"));
}

/// **The drawn string contains a `/` whenever the full name does** — `screens/dialogs.md`
/// § *While the call is running*'s rules 1–4, which is a hard clause and not a wider budget that
/// makes the old defect less likely (`tui-designer`, 2026-09-12).
///
/// **Why it is a defect and not an ugly cut**: a bare name means cluster-scoped everywhere else in
/// this product (`screens/README.md` § the five rules), so a flat cut that ate the `/` taught a
/// reader that a namespaced Deployment is a Node. `team-alpha-payments-platform/web` drew as
/// `team-alpha-payments-pl…` at the budget this line had before this round.
///
/// **Every namespace length across the boundary is fed**, because the interesting cases are the
/// three columns either side of where the `/` stops surviving an ordinary clip — 1 to 60, which
/// walks case 1 into case 2 into case 3 and out the other side. The bare name is swept beside it:
/// it has no `/` to protect and must not grow one.
///
/// **Which end of the namespace gives way is swept with it** (`k8s-admin`, 2026-09-12; the identity
/// cut, NOTES § D266). Stated as the invariant rather than by re-deriving the branches here: the
/// namespace on screen is **either whole, where the whole line fit, or behind a leading `…`**. An
/// unmarked *prefix* of a namespace is the one thing no case may draw. Behind that mark the
/// object's own name is whole, unless the namespace has already given up the whole of itself and
/// there is still nothing left to give.
#[test]
fn the_cut_never_takes_the_slash_that_says_the_object_is_namespaced() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let screen = screen(&alerts, &now);
    for length in 1..=60 {
        let namespace = "n".repeat(length);
        let drawn = footer_of(&render(&changing(Some(&namespace), "web"), &screen));
        assert!(
            drawn.contains('/'),
            "a namespaced object drew as cluster-scoped at {length}: {drawn:?}"
        );
        let cut = drawn
            .split_once("changing ")
            .and_then(|(_, tail)| tail.rsplit_once(" first"))
            .expect("the in-flight footer's own fixed words")
            .0;
        let (shown, named) = cut.split_once('/').expect("the slash asserted above");
        assert!(
            shown == namespace || shown.starts_with(CUT),
            "an unmarked front of a namespace at {length}: {drawn:?}"
        );
        assert!(
            named == "web" || shown == CUT || shown == namespace,
            "the object's own name gave way while the namespace still had a front at {length}: \
             {drawn:?}"
        );
        assert!(
            width(&drawn) <= 76,
            "{length} — {drawn:?} is {} columns",
            width(&drawn)
        );

        let bare = footer_of(&render(&changing(None, &namespace), &screen));
        assert!(
            !bare.contains('/'),
            "a cluster-scoped object grew a namespace at {length}: {bare:?}"
        );
        assert!(width(&bare) <= 76, "{length} — {bare:?}");
    }
}

/// **Two objects in one namespace too long for the line still draw two different footers** —
/// rule 3's whole reason (`screens/dialogs.md` § *While the call is running*).
///
/// **This is the blue/green collision this box fixed once and then reintroduced, total instead of
/// partial** (`k8s-admin`, 2026-09-12). `openshift-cluster-node-tuning-operator` is 38 characters
/// and real; under the tail-cut, `tuned`, `cluster-node-tuning-operator` and anything else in it
/// all drew `openshift-cluster-node-tuning-o/…` — the same 33 columns, naming nothing. The
/// namespace's front is what a reader can afford to lose; the object's name is what every later
/// command needs typed.
///
/// **The last branch gives the name's front too** (`screens/dialogs.md` § *While the call is
/// running*, case 3, NOTES § D266): once the name alone passes `room − 2` there is nothing left to
/// give, and what is kept is the name's **tail** — where two names from one rollout differ — behind
/// `…/…`, never its head, which is where they are alike.
#[test]
fn every_object_in_one_long_namespace_still_draws_its_own_name() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let screen = screen(&alerts, &now);
    let namespace = "openshift-cluster-node-tuning-operator";
    let drawn = |name: &str| footer_of(&render(&changing(Some(namespace), name), &screen));

    let mut seen: Vec<String> = Vec::new();
    for name in [
        "tuned",
        "tuned-render",
        "cluster-node-tuning-operator",
        "cluster-node-tuning-operator-x",
    ] {
        let line = drawn(name);
        println!("{line}");
        assert!(line.contains('/'), "{name} lost the slash: {line:?}");
        assert!(
            line.contains(&format!("/{name} ")),
            "{name} was cut while the namespace still had a front to lose: {line:?}"
        );
        assert!(
            !seen.contains(&line),
            "{name} drew a line another object in the same namespace already drew: {line:?}"
        );
        assert_eq!(width(&line), 76, "{name} — the floor's own ceiling");
        seen.push(line);
    }

    // **Past `room − 2` the namespace goes whole and the name gives way from its own front**, so
    // what is on screen is the object's tail and not a namespace every object shares.
    let long = drawn("cluster-node-tuning-operator-metrics-reader-binding");
    let other = drawn("cluster-node-tuning-operator-metrics-reader-bindinx");
    println!("{long}");
    assert!(
        long.contains("changing …/…perator-metrics-reader-binding first"),
        "the name kept a head instead of its tail: {long:?}"
    );
    assert_ne!(
        long, other,
        "two names differing in their last character drew one line"
    );
    assert_eq!(width(&long), 76);
}

/// **A detail tab keeps its own footer and loses exactly one word, `q quit`**
/// (`screens/dialogs.md` § *Detail tabs and Analysis keep their own footer, not this line*,
/// `screens/detail.md`'s own sentence above the tab sections).
///
/// The logs tab is the one that shows it, because it is the richest ordinary footer there is and
/// the one the first draft of this box broke hardest: over a call confirmed on Alerts it drew
/// `⏎ open` on a pane with nothing to select, dropped the `esc back` that is the only way out of
/// the tab, and left `[ ] tabs`, `f follow` and `c container` bound and unnamed (`k8s-admin`,
/// 2026-09-12).
#[test]
fn a_detail_tabs_footer_keeps_itself_and_loses_only_the_quit() {
    let (pod, names) = declared_by("healthy-sidecar");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &held,
    });
    let detail = open.open();
    let quiet = footer_of(&detailed(&on(Tab::Logs), &detail));
    assert_eq!(
        quiet,
        "[ ] tabs  f follow  c container  esc back  ? all keys  q quit"
    );

    let mut running = changing(Some("payments"), "web");
    running.tab = Tab::Logs;
    let drawn = footer_of(&detailed(&running, &detail));
    println!("{drawn}");
    assert_eq!(
        format!("{drawn}  q quit"),
        quiet,
        "more than one word gave way"
    );
}

/// **Help over a call in flight loses its right-hand zone, and its body says why**
/// (`screens/help.md` § *While the call is running*): `? or esc to close` alone, `q quit` gone
/// rather than marked `q no quit`, and the two rewritten rows on the screen the reader pressed
/// `?` to read.
///
/// The row is read whole, not through [`footer_of`], because what this is about is the zone at the
/// **right** edge — a trimmed line cannot tell an empty zone from one drawn somewhere else.
///
/// **This is where the threading is proven and not only [`key_map`]**: the flag reaches `ui::help`
/// through `ui::modal`, and a body that paused nothing would pass every test in the region below
/// and still draw four keys it refuses.
#[test]
fn help_over_a_call_in_flight_draws_no_quit_at_the_right_edge() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let screen = screen(&alerts, &now);

    let helping = App {
        modal: Some(views::Modal::Help),
        ..App::default()
    };
    let quiet = rows(&render(&helping, &screen));
    assert!(
        quiet[22].ends_with("q quit │"),
        "help's ordinary footer lost the quit it is the one modal to keep"
    );
    assert!(
        !quiet[2..18].iter().any(|row| row.contains("paused")),
        "a row paused with nothing on the wire"
    );

    let paused = mockup_paused();
    let mut running = changing(Some("payments"), "web");
    running.modal = Some(views::Modal::Help);
    let body = rows(&render(&running, &screen));
    let drawn = body[22].clone();
    println!("{drawn}");
    assert!(
        drawn.starts_with(&format!("│ {}", paused[2][0])),
        "the body of help's own line moved: {drawn:?}"
    );
    assert!(
        !drawn.contains("quit"),
        "a key the running call refuses was promised anyway: {drawn:?}"
    );
    assert!(
        !drawn.contains("changing"),
        "the in-flight line reached a modal that draws its own: {drawn:?}"
    );

    // **The two rewritten rows reach the screen**, which `ui::modal` threading the flag to
    // `ui::help` is the whole of. Anchored on the screen file's own text, never on a row number.
    for block in [&paused[0][0], &paused[1][0]] {
        assert!(
            body[2..18].iter().any(|row| row.contains(block.trim())),
            "screens/help.md's {block:?} never reached the screen"
        );
    }
}

/// **Help's body pauses the four keys the running call refuses, and promises none of them**
/// (`screens/help.md` § *While the call is running*).
///
/// **This is the blocker both reviewers found independently** (2026-09-12). The body drew
/// `X switch cluster`, `s run more or fewer copies`, `r restart, at its own pace` and
/// `ctrl-d delete` exactly as if all four worked, while `App::may_switch_cluster` and
/// `App::may_mutate` were both false: the footer has gone quiet, the operator presses `?` to find
/// out what changed, the map says `s run more or fewer copies`, they press it and nothing
/// happens — PRIOR-ART § G1's *refuses for no visible reason*, arriving through the one screen
/// that exists to answer *what may I press*.
///
/// **Both rewritten rows are read off the screen file**, and the three rows under the heading are
/// cross-checked against [`mockup`]'s own — so the two readers cannot drift into describing two
/// different screens, which is what [`all_three_refused_is_the_block_the_screen_file_draws`]
/// already does for the refusal block.
///
/// **The body is still sixteen rows and every other row is untouched**, which is the whole of
/// `screens/widgets.md` § 1's budget surviving this state.
#[test]
fn help_pauses_the_four_keys_a_running_call_refuses() {
    let base = mockup();
    let paused = mockup_paused();
    let heading = row_at(&base, CHANGING_HEADING);
    assert_eq!(
        &paused[1][1..],
        &base[heading + 1..],
        "the two readers disagree about the rows under the Changing things heading"
    );

    let expected: Vec<String> = base
        .iter()
        .map(|row| {
            if row.starts_with("    X ") {
                paused[0][0].clone()
            } else if row.starts_with("  Changing things") {
                paused[1][0].clone()
            } else {
                row.clone()
            }
        })
        .collect();
    assert_eq!(expected.len(), 16, "the body is no longer sixteen rows");
    assert_eq!(
        expected.iter().filter(|row| row.contains("paused")).count(),
        2,
        "the two rows the screen file rewrites did not both land: {expected:?}"
    );

    let drawn: Vec<String> = key_map(HELP, Refused::default(), true, None)
        .lines()
        .map(str::to_owned)
        .collect();
    for row in &drawn {
        println!("{row}");
    }
    assert_eq!(
        drawn, expected,
        "screens/help.md § While the call is running"
    );
    assert_eq!(
        key_map(HELP, Refused::default(), false, None),
        HELP,
        "nothing running and the map is the mockup untouched"
    );
}

/// **A permission clause and the wait are never reconciled on the same row** (`screens/help.md`
/// § *While the call is running*, its third bullet). While a call is in flight `s`, `r` and
/// `ctrl-d` are inactionable for the wait's reason alone, whatever a probe would otherwise say
/// about any one of them — so the map is the same sixteen rows for all eight combinations, and
/// `no` (a permission this login lacks) never lands beside `paused` (a wait).
#[test]
fn a_call_in_flight_draws_no_permission_clause_on_any_row() {
    let expected = key_map(HELP, Refused::default(), true, None);
    for scale in [false, true] {
        for restart in [false, true] {
            for delete in [false, true] {
                let drawn = key_map(
                    HELP,
                    refusing(scale, restart, delete, "deployments"),
                    true,
                    None,
                );
                assert_eq!(drawn, expected, "s {scale} · r {restart} · ctrl-d {delete}");
                assert!(
                    !drawn.contains("(scale — ")
                        && !drawn.contains("rollout restart — ")
                        && !drawn.contains("(delete "),
                    "a permission clause drew over the wait: {drawn}"
                );
                for line in drawn.lines() {
                    assert!(
                        width(line) <= usize::from(MIN_WIDTH - 2),
                        "{line:?} is {} columns, past the body at the floor",
                        width(line)
                    );
                }
            }
        }
    }
}

/// **Every mode's whole screen, printed, footer row and all** — not an assertion about a column
/// but the eight frames a reader of the report compares with `screens/`, line by line.
/// `cargo test -- --nocapture`.
#[test]
fn every_footer_on_the_screen_it_belongs_to() {
    let now = now();
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let log = [
        Stripped::of("$ kubectl get statefulsets -A --watch"),
        Stripped::of("$ kubectl get daemonsets -A --watch"),
    ];

    let show = |title: &str, buffer: &Buffer| {
        println!("--- {title} ---\n{}\n", rows(buffer).join("\n"));
    };

    // Alerts.
    let mut plain = screen(&alerts, &now);
    plain.log = &log;
    show("Alerts", &render(&app(), &plain));

    // Resources — the browser open on the committed pods capture.
    let kinds: Vec<Browsable> = ["deployments", "statefulsets", "daemonsets", "pods", "jobs"]
        .into_iter()
        .map(|plural| browsable(plural, true))
        .collect();
    let ready = Pane::Ready(table("table-pods"));
    let mut browser = browsing(&ready, &kinds, &now);
    browser.log = &log;
    let mut opened = App::default();
    opened.open(NavItem::Group(Group::Workloads));
    opened.open(NavItem::Kind(3));
    show("Resources", &render(&opened, &browser));

    // Analysis — the capacity report over the committed cluster capture.
    let held = reported();
    let reports = entries(&held);
    let mut analysis = screen(&alerts, &now);
    analysis.reports = &reports;
    analysis.log = &log;
    show("Analysis", &render(&opened_report(0), &analysis));

    // The four detail tabs, each over the committed OOM capture.
    let (pod, names) = declared_by("oom");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let held_lines = logged(&[
        "14:21:58  starting worker pool",
        "14:22:03  allocating 512Mi cache",
    ]);
    let mut open = Open::new();
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &held_lines,
    });
    open.read = Pane::Ready(Described {
        snapshot: &pod,
        containers: &containers,
    });
    open.yaml = Pane::Ready("apiVersion: v1\nkind: Pod\nmetadata:\n  name: web-7d9f4".to_owned());
    open.events = Pane::Ready(measured());
    let detail = open.open();
    for tab in Tab::ALL {
        show(
            &format!("Detail · {}", tab.label()),
            &detailed(&on(tab), &detail),
        );
    }

    // Help, over the Alerts screen it was opened from.
    let helping = App {
        modal: Some(views::Modal::Help),
        ..App::default()
    };
    show("Help", &render(&helping, &plain));

    // **Alerts again, for a login that may not scale or restart the selected card** — the one
    // footer a refusal reaches, and the only mode on this page whose text is not fixed by the
    // mode alone (`screens/widgets.md` § 2a, NOTES § D23).
    let mut denied = screen(&alerts, &now);
    denied.log = &log;
    denied.refused = refusing(true, true, false, "deployments");
    show("Alerts · s and r refused", &render(&app(), &denied));

    // **The in-flight screens** (`screens/dialogs.md` and `screens/help.md`, each § *While the
    // call is running*) — the header's mark and the footer's reason clause, both of that file's
    // cuts, the logs tab keeping its own line less `q quit`, and Help over the same call with no
    // `q quit` and two rows paused.
    show(
        "Alerts · a change in flight",
        &render(&changing(Some("payments"), "web"), &plain),
    );
    let mut helping = changing(
        Some("payments"),
        "checkout-worker-service-account-token-projector",
    );
    show("Alerts · a long name in flight", &render(&helping, &plain));
    show(
        "Alerts · a namespace that has used up the line",
        &render(
            &changing(Some("openshift-cluster-node-tuning-operator"), "tuned"),
            &plain,
        ),
    );
    show(
        "Alerts · and a name that has used up what was left",
        &render(
            &changing(
                Some("openshift-cluster-node-tuning-operator"),
                "cluster-node-tuning-operator-metrics-reader-binding",
            ),
            &plain,
        ),
    );
    let open = Open::new();
    let mut running = changing(Some("payments"), "web");
    running.tab = Tab::Logs;
    show(
        "Detail · logs, with a change in flight",
        &detailed(&running, &open.open()),
    );
    helping.modal = Some(views::Modal::Help);
    show("Help · a change in flight", &render(&helping, &plain));
}

// --- THE DIALOGS ---

/// The scale dialog `screens/dialogs.md` § Scale draws, with the check already answered.
fn scaling() -> views::Dialog {
    views::Dialog {
        verb: "scale",
        object: views::Object::new(
            "deployment",
            Some("payments".to_owned()),
            "web".to_owned(),
            Some("8656c3ec-0f0e-4d0e-9f0b-2a1d3c4b5a69".to_owned()),
        ),
        consequence: "This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies."
            .to_owned(),
        warning: None,
        kubectl: "kubectl --context prod-eu scale deployment/web --replicas=3 -n payments"
            .to_owned(),
        verdict: Some(ACCEPTED),
        asks: None,
        typed: views::Input::default(),
    }
}

fn restarting() -> views::Dialog {
    views::Dialog {
        verb: "restart",
        consequence: "This asks Kubernetes to replace every copy of your app with a new one. How \
                      many stop at the same time is a setting on this deployment — it can be a \
                      few, or all of them at once. A paused deployment will not start until you \
                      resume it."
            .to_owned(),
        kubectl: "kubectl --context prod-eu rollout restart deployment/web -n payments".to_owned(),
        ..scaling()
    }
}

fn deleting() -> views::Dialog {
    let mut typed = views::Input::default();
    for character in "web-7d9f".chars() {
        typed.push(character);
    }
    views::Dialog {
        verb: "delete",
        object: views::Object::new(
            "pod",
            Some("payments".to_owned()),
            "web-7d9f4".to_owned(),
            Some("f0a1b2c3-d4e5-4678-9abc-def012345678".to_owned()),
        ),
        consequence: "This removes the pod. Whatever created it will normally replace it — k8rs \
                      has not checked whether anything did."
            .to_owned(),
        warning: None,
        kubectl: "kubectl --context prod-eu delete pod/web-7d9f4 -n payments".to_owned(),
        verdict: Some(UNCHECKABLE),
        asks: Some("web-7d9f4".to_owned()),
        typed,
    }
}

/// The paused Deployment's warning, `main.rs`'s `while_paused` sentence for a deployment
/// (NOTES § D224).
const PAUSED: &str = "This deployment is paused, so nothing will be replaced until somebody \
                      resumes it with kubectl rollout resume — and the command above will refuse \
                      to run until then.";

/// **`ops.rs`'s own two verdict lines, copied here because they are private there.**
///
/// **They are lower case with no full stop, which is what `ops::Checked::verdict` really
/// returns** — right for the headless surface `main.rs` prints them on, and reshaped for a drawn
/// box by [`spoken`]. The fixtures used to carry the *drawn* form, so the box was built from a
/// string no code path produces and the transformation was never exercised: a test asserting
/// what the implementation happens to hand it, which CLAUDE.md § Tests must not lie forbids by
/// name (both reviewers, 2026-09-10).
///
/// **They are still retyped, and that is a hole with a name on it.** `ops::ACCEPTED` and
/// `ops::UNCHECKABLE` are private consts (`src/ops.rs`), so nothing outside that file can reach
/// them; the PM has the visibility ruling. Until then this pair is a second copy, and the one
/// thing keeping it honest is that `screens/dialogs.md` draws the reshaped form and
/// `the_restart_boxes_…` and `the_delete_boxes_…` compare against the page.
const ACCEPTED: &str = "the cluster checked it first and accepted it";
const UNCHECKABLE: &str = "k8rs did not check this one with the cluster first";

/// **Four rows of warning at [`CROWDED_BOX`]'s own text width** — one row short of [`WORDY`], so
/// the pair of them is the blank row under the consequence appearing and disappearing.
const ROOMY: &str = "The cluster answered this check with a sentence longer than any it really \
                     sends, long enough to take four whole rows of this box and no more than \
                     four of them, which is what this one is for.";

/// **Five rows of warning at [`CROWDED_BOX`]'s own text width** — no operation sends anything
/// like it, and that is the point: the row budget is a total guard on a `views::Dialog` anyone
/// can build, and this is the only shape that crowds the box from below.
const WORDY: &str = "The cluster answered this check with a sentence far longer than any it \
                     really sends, long enough to take five whole rows of this box on its \
                     own and to leave the consequence above it a single row to live in, \
                     which is what this case exists to say.";

/// The webhook refusal `screens/dialogs.md` § The cluster said no quotes.
const DENIED: &str = "admission webhook 'limits.example.com' denied the request: replicas may not \
                      exceed 5 in this namespace";

/// § Delete's second box — the one cluster-scoped object any operation reaches, whose title bar
/// carries no namespace (rule 1).
fn deleting_a_node() -> views::Dialog {
    let mut typed = views::Input::default();
    for character in "node-".chars() {
        typed.push(character);
    }
    views::Dialog {
        object: views::Object::new(
            "node",
            None,
            "node-3".to_owned(),
            Some("c0ffee00-0000-4000-8000-000000000000".to_owned()),
        ),
        consequence: "This asks the cluster to remove its record of node-3, not the machine. \
                      Something attached to it, unread by k8rs, may delay this or act first. Left \
                      alone, its pods are deleted and the machine keeps running until its kubelet \
                      restarts."
            .to_owned(),
        kubectl: "kubectl --context prod-eu delete node/node-3".to_owned(),
        asks: Some("node-3".to_owned()),
        typed,
        ..deleting()
    }
}

fn over(modal: views::Modal) -> App {
    App {
        modal: Some(modal),
        ..App::default()
    }
}

/// **Every dialog, printed whole** — `cargo test -- --nocapture`.
#[test]
fn every_dialog_on_the_screen_it_belongs_to() {
    let now = now();
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let log = [
        Stripped::of("$ kubectl get statefulsets -A --watch"),
        Stripped::of("$ kubectl scale deployment/web --replicas=3 -n payments"),
    ];
    let mut plain = screen(&alerts, &now);
    plain.log = &log;

    let mut paused = restarting();
    paused.warning = Some(PAUSED.to_owned());

    for (title, modal) in [
        ("Scale", views::Modal::Confirm(scaling())),
        ("Restart", views::Modal::Confirm(restarting())),
        ("Restart · paused", views::Modal::Confirm(paused)),
        ("Delete · pod", views::Modal::Confirm(deleting())),
        ("Delete · node", views::Modal::Confirm(deleting_a_node())),
        (
            "Refused",
            views::Modal::Refused {
                sent: false,
                fault: crate::k8s::Fault::Rejected,
                said: Some(DENIED.to_owned()),
            },
        ),
        (
            "Gone · pod",
            views::Modal::Gone {
                object: deleting().object,
                recreated: true,
            },
        ),
        (
            "Gone · deployment",
            views::Modal::Gone {
                object: scaling().object,
                recreated: false,
            },
        ),
    ] {
        println!(
            "--- {title} ---\n{}\n",
            rows(&render(&over(modal), &plain)).join("\n")
        );
    }
}

/// **The nested box out of a drawn frame or out of `screens/dialogs.md`, borders and all** — the
/// same extraction over both, so a comparison between them is a comparison and not a coincidence.
///
/// It anchors on the first `┌` that is not in column 0, which is the frame's own border in both.
/// The delete field's own `┌` is further in and on a later row, so it is never the anchor.
fn nested(rows: &[String]) -> Vec<String> {
    let (top, left) = rows
        .iter()
        .enumerate()
        .find_map(|(y, row)| {
            row.chars()
                .position(|character| character == '┌')
                .filter(|&x| x > 0)
                .map(|x| (y, x))
        })
        .unwrap_or_else(|| panic!("no nested box in\n{}", rows.join("\n")));
    let width = rows[top]
        .chars()
        .skip(left)
        .position(|character| character == '┐')
        .expect("a nested box closes its title bar")
        + 1;
    let mut box_ = Vec::new();
    for row in &rows[top..] {
        let line: String = row.chars().skip(left).take(width).collect();
        let last = line.starts_with('└');
        box_.push(line);
        if last {
            break;
        }
    }
    box_
}

/// **`screens/dialogs.md` is the fixture, which is the point** (`mockup`'s own reason, one screen
/// along). A test that compares the drawn box with a constant this file also wrote compares the
/// implementation with itself; the screen file is the specification, so it is what the assertion
/// reads. The blocks are in the file's own order: 0 scale · **1 scale, check on the wire** ·
/// 2 restart · 3 restart paused · **4 restart, check on the wire** · 5 delete pod ·
/// 6 delete node · 7 the object changed first · 8 refused · 9 gone · 10 drain.
fn mockup_dialog(nth: usize) -> Vec<String> {
    let path = format!("{}/screens/dialogs.md", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the screen file {path} could not be read: {e}"));
    let mut blocks = Vec::new();
    let mut open: Option<Vec<String>> = None;
    for line in text.lines() {
        match (&mut open, line.starts_with("```")) {
            (None, false) => {}
            (None, true) => open = Some(Vec::new()),
            (Some(_), false) => open.as_mut().expect("open").push(line.to_owned()),
            (Some(_), true) => {
                let block = open.take().expect("open");
                // **The *Choosing how many* box is specified and not built**
                // (`screens/dialogs.md` § Choosing how many, before the confirm box, 2026-09-24:
                // *"this section exists so a later box has something to build against, not because
                // any of it is on screen today"*). Nothing in `ui.rs` draws it — `s` is the key
                // that would open it and there is no state to type a number into — so it is
                // skipped here rather than renumbering the eleven boxes this file does draw. The
                // box that builds it takes this filter off in the same turn.
                let unbuilt = block
                    .iter()
                    .any(|line| line.contains("How many do you want?"));
                if !unbuilt && block.first().is_some_and(|line| line.starts_with(" nodes")) {
                    blocks.push(block);
                }
            }
        }
    }
    assert_eq!(
        blocks.len(),
        11,
        "screens/dialogs.md no longer draws eleven screens"
    );
    nested(&blocks[nth])
}

/// The Alerts screen a dialog is opened over, with a real command log under it.
fn opened_over<'a>(alerts: &'a Pane<Vec<Card>>, now: &'a Time, log: &'a [Stripped]) -> Screen<'a> {
    let mut screen = screen(alerts, now);
    screen.log = log;
    screen
}

fn logged_pair() -> [Stripped; 2] {
    [
        Stripped::of("$ kubectl get statefulsets -A --watch"),
        Stripped::of("$ kubectl scale deployment/web --replicas=3 -n payments"),
    ]
}

/// **The box's sentence rows — everything but the `$ kubectl …` line** (NOTES § D278).
///
/// **Four tests counted `…` marks across the whole box and every one of them broke on the same
/// day**, because since `--context` joined every taught command the `$` line carries a mark on
/// every box this product draws ([`ui::command_cut`], [`ui::context_cut`]). What each of them was
/// really asserting is what happened to the *sentences* — was the consequence cut, was the
/// warning spared — and a scan that cannot tell the two apart answers a question nobody asked.
fn sentences(box_: &[String]) -> impl Iterator<Item = &String> {
    box_.iter().filter(|row| !row.contains("$ kubectl"))
}

/// The box a modal draws at the floor, extracted from the real frame.
fn box_of(modal: views::Modal) -> Vec<String> {
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let now = now();
    let log = logged_pair();
    nested(&rows(&render(
        &over(modal),
        &opened_over(&alerts, &now, &log),
    )))
}

/// **§ Scale's two boxes are drawn exactly as `screens/dialogs.md` draws them, every row.**
///
/// **It is § Scale that pins the blank row between the consequence and the verdict**, because
/// § Scale is the only box on that page that keeps one: its consequence is
/// [`ui::CONSEQUENCE_LINES`] rows and it asks for no typed name, which is what `confirm`'s
/// `spare_row` reads. § Restart's four rows and § Delete's field are the two boxes that spend it.
///
/// **It exists because `just mutants-diff` said it was missing** (2026-09-24). Blocks 2, 3 and 4
/// were compared row for row and blocks 0 and 1 were not, so `field == 0` could be flipped to
/// `field != 0` with nothing to notice: § Delete's own consequence is already past
/// `CONSEQUENCE_LINES`, so the `&&` short-circuits there and only § Scale can see the change.
#[test]
fn the_scale_boxes_are_the_screen_files_boxes_row_for_row() {
    let mut pending = scaling();
    pending.verdict = None;
    for (named, dialog, nth) in [
        ("§ Scale", scaling(), 0),
        ("§ Scale, its check still on the wire", pending, 1),
    ] {
        let drawn = box_of(views::Modal::Confirm(dialog));
        let mockup = mockup_dialog(nth);
        // **The row count first, because it is the blank row** — the one this box keeps and the
        // other two spend, and the number `confirm`'s `spare_row` decides.
        assert_eq!(
            drawn.len(),
            mockup.len(),
            "screens/dialogs.md {named}: {} rows against the page's {}\\n{}",
            drawn.len(),
            mockup.len(),
            drawn.join("\\n")
        );
        for (row, (a, b)) in drawn.iter().zip(&mockup).enumerate() {
            // **Rows 2 and 3 are the consequence, and they are compared joined** — the one place
            // this page and the wrapper disagree, reported rather than papered over. § Scale
            // draws the break at the sentence (*"This starts 1 more copy of your app."* /
            // *"Right now: 2 copies. After: 3 copies."*); `wrapped` packs greedily and breaks
            // after *copies.*, which is what the page's own prose says it must — *"that one
            // string wrapped to the box width — a rendering choice, not two fields"*. Both draw
            // the same sentence on the same two rows; only the break moves. Handed to
            // `tui-designer` 2026-09-24.
            if (2..=3).contains(&row) {
                continue;
            }
            assert_eq!(a, b, "screens/dialogs.md {named}, row {row}");
        }
        let said = |rows: &[String]| {
            rows.iter()
                .map(|row| row.trim_matches('\u{2502}').trim().to_owned())
                .collect::<Vec<_>>()
                .join(" ")
        };
        assert_eq!(
            said(&drawn[2..=3]),
            said(&mockup[2..=3]),
            "screens/dialogs.md {named}: the consequence is not the page's sentence"
        );
    }
}

/// **§ Restart's two boxes are drawn exactly as `screens/dialogs.md` draws them, every row.**
///
/// **This is the pair that has no exception**, and it is what the whole geometry rests on: the
/// box width, the left margin, the dropped blank row between consequence and verdict, the wrap
/// points of a 232-column consequence and a 163-column warning, the `$` line and the buttons.
/// Every one of those is a number `ui.rs` derives, and here they land on the screen file's own
/// characters.
#[test]
fn the_restart_boxes_are_the_screen_files_boxes_row_for_row() {
    assert_eq!(
        box_of(views::Modal::Confirm(restarting())),
        mockup_dialog(2),
        "screens/dialogs.md § Restart"
    );

    let mut paused = restarting();
    paused.warning = Some(PAUSED.to_owned());
    assert_eq!(
        box_of(views::Modal::Confirm(paused)),
        mockup_dialog(3),
        "screens/dialogs.md § Restart, its paused Deployment"
    );

    // **And the same box with its check still on the wire** (§ … restart's own box) — ten content
    // rows, the same as the plain box above, with `Checking with the cluster…` in the row the
    // verdict lands in and `Dialog::warning` as unknown as the verdict while this frame is up.
    let mut pending = restarting();
    pending.verdict = None;
    assert_eq!(
        box_of(views::Modal::Confirm(pending)),
        mockup_dialog(4),
        "screens/dialogs.md § Restart, its check still on the wire"
    );
}

/// **The verdict's row is reserved from the first frame, so the box does not move when the
/// cluster answers** (`screens/dialogs.md` § While the check is still on the wire: *"same width,
/// same nine content rows"*).
///
/// **It compares two frames the product drew, not a frame with a drawing.** What the two boxes
/// *say* is the screen file's, asserted against blocks 1 and 4 by the two tests either side of
/// this one; what is asserted here is that the answered box and the pending one are the same
/// shape — a claim no single mockup can carry, because a box that grew by a row would match its
/// own mockup perfectly and still shift a sentence under a reader mid-way through it.
///
/// **The row that holds the verdict is found rather than counted**, so a wrap that moves the
/// consequence onto another line cannot decide whether this passes.
#[test]
fn the_verdict_row_is_reserved_before_the_cluster_answers() {
    for (verdict, dialog) in [(ACCEPTED, scaling()), (ACCEPTED, restarting())] {
        let mut pending = dialog.clone();
        pending.verdict = None;
        assert!(
            pending.waiting() && !dialog.waiting(),
            "the pair is not one waiting box and one answered one"
        );
        let waiting = box_of(views::Modal::Confirm(pending));
        let answered = box_of(views::Modal::Confirm(dialog));
        assert!(
            waiting.last().is_some_and(|row| row.starts_with('└')),
            "the pending box does not close:\n{}",
            waiting.join("\n")
        );
        assert_eq!(
            (waiting.len(), width(&waiting[0])),
            (answered.len(), width(&answered[0])),
            "the box changed shape when the cluster answered:\n{}\n{}",
            waiting.join("\n"),
            answered.join("\n")
        );
        let at = |box_: &[String], needle: &str| {
            box_.iter()
                .position(|row| row.contains(needle))
                .unwrap_or_else(|| panic!("no {needle:?} row in\n{}", box_.join("\n")))
        };
        assert_eq!(
            at(&waiting, &checking()),
            at(&answered, &spoken(verdict)),
            "the sentence the cluster's answer replaces is not drawn in the row it lands in"
        );
    }
}

/// **§ Delete's two boxes, every row but the one between its buttons.**
///
/// **The exception is the gap and nothing else.** `screens/dialogs.md` widens delete's gap to five
/// columns so the pair occupies the same 29 columns § Scale's `[ ⏎ do it ]` pair does; this file
/// spends [`BUTTON_GAP`] between every pair of buttons on every box, which is one rule instead of
/// a width per verb. The buttons themselves, and that the pair is centred, are asserted below.
///
/// **What it is not an exception about is the box** — the node box reaches [`MODAL_ROWS`] exactly
/// with four consequence lines and a typed-name field, which is the measurement that says a `$`
/// row cannot also fit inside a delete box.
#[test]
fn the_delete_boxes_are_the_screen_files_boxes_but_for_the_gap_between_two_buttons() {
    for (nth, dialog) in [(5, deleting()), (6, deleting_a_node())] {
        let drawn = box_of(views::Modal::Confirm(dialog));
        let mockup = mockup_dialog(nth);
        assert_eq!(drawn.len(), mockup.len(), "block {nth} changed height");
        for (n, (drawn, mockup)) in drawn.iter().zip(&mockup).enumerate() {
            if mockup.contains("esc cancel") {
                continue;
            }
            assert_eq!(drawn, mockup, "screens/dialogs.md block {nth}, row {n}");
        }
    }
}

/// **Every box says what the screen file says, wherever the wrap lands** — the words, in order,
/// none dropped and none invented, in a box of the same width and the same height.
///
/// **The wrap points themselves are deliberately not asserted for four of these boxes**, because
/// `screens/dialogs.md` says they are not the specification: *"the wrap points shown are this
/// box's choice of where to break for readability, not a second field"* (§ Restart, of a
/// consequence that is one string). § Scale's — in both of its states — § The cluster said no's
/// and § The object went away's differ from a real wrap at the same width; § Restart's and
/// § Delete's do not, and those are asserted character for character above.
///
/// **The button row is left out of the join** — its gap and its centring are the one difference
/// this file keeps from the page, and it has its own test.
#[test]
fn every_dialog_says_the_words_the_screen_file_says() {
    let mut paused = restarting();
    paused.warning = Some(PAUSED.to_owned());
    let mut pending = scaling();
    pending.verdict = None;
    for (nth, modal) in [
        (0, views::Modal::Confirm(scaling())),
        (1, views::Modal::Confirm(pending)),
        (2, views::Modal::Confirm(restarting())),
        (3, views::Modal::Confirm(paused)),
        (5, views::Modal::Confirm(deleting())),
        (6, views::Modal::Confirm(deleting_a_node())),
        (
            7,
            views::Modal::Refused {
                sent: true,
                fault: crate::k8s::Fault::Conflict,
                said: None,
            },
        ),
        (
            8,
            views::Modal::Refused {
                sent: false,
                fault: crate::k8s::Fault::Rejected,
                said: Some(DENIED.to_owned()),
            },
        ),
        (
            9,
            views::Modal::Gone {
                object: deleting().object,
                recreated: true,
            },
        ),
    ] {
        let drawn = box_of(modal);
        let mockup = mockup_dialog(nth);
        assert_eq!(
            width(&drawn[0]),
            width(&mockup[0]),
            "block {nth} is a different width from the box the screen file draws"
        );
        assert_eq!(
            said_by(&drawn),
            said_by(&mockup),
            "block {nth} does not say what screens/dialogs.md says"
        );

        // **The button row is left out of the join above and asserted here instead**, because it
        // is the one row whose *spacing* this file does not take from the page — the gap and the
        // centring are its own (see the two tests above). Its **words** are still the screen
        // file's, and nothing else on the box asserted them: a mutation run replaced `dismiss`
        // with `Default::default()`, drew a `Refused` and an `Already gone` box with no button at
        // all, and every test here stayed green (2026-09-10).
        let buttons = |box_: &[String]| {
            box_.iter()
                .find(|row| row.contains("esc cancel") || row.contains("esc dismiss"))
                .map(|row| {
                    row.trim_matches('│')
                        .split_whitespace()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| panic!("no button row on\n{}", box_.join("\n")))
        };
        assert_eq!(
            buttons(&drawn),
            buttons(&mockup),
            "block {nth} does not offer the buttons screens/dialogs.md draws"
        );
    }
}

/// The words of every row of a box but its button row, in order — the button row is every box's
/// one difference from the page ([`every_dialog_says_the_words_the_screen_file_says`] says why)
/// and is asserted on its own wherever it matters.
fn said_by(box_: &[String]) -> Vec<String> {
    box_[1..box_.len() - 1]
        .iter()
        .filter(|row| !row.contains("esc cancel") && !row.contains("esc dismiss"))
        .flat_map(|row| {
            row.trim_matches('│')
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

/// **The pending row names the ceiling its check is bounded at, and the number is
/// `ops::CHECK_DEADLINE`'s** (`screens/dialogs.md` § While the check is still on the wire,
/// NOTES § D273 — which moved that number once already).
///
/// **What this pins is the *number*, against the constant and never against `35`.** The sentence
/// around it is the screen file's, and
/// [`the_restart_boxes_are_the_screen_files_boxes_row_for_row`] and
/// [`every_dialog_says_the_words_the_screen_file_says`] assert it against the page — which is why
/// this one reads a fragment: the wording has a specification and the number has a source, and
/// the defect worth catching is a second copy of the source.
///
/// **What it cannot prove is that `ui.rs` did not type `35`**, because both sides read 35 today —
/// no in-process assertion can tell a derived number from a copied one that agrees. It is proven
/// by moving the constant and watching this go red, which is what it is here to do the day
/// somebody moves it for real (NOTES § D273 *The review round* 1 is the last time).
#[test]
fn the_pending_row_names_the_ceiling_the_check_is_bounded_at() {
    let mut pending = scaling();
    pending.verdict = None;
    let drawn = box_of(views::Modal::Confirm(pending));
    let ceiling = format!("up to {} seconds", crate::ops::CHECK_DEADLINE.as_secs());
    assert!(
        drawn.iter().any(|row| row.contains(&ceiling)),
        "the pending box does not name {ceiling:?}, the ceiling its own check is bounded at:\n{}",
        drawn.join("\n")
    );
}

/// **State 1b is one box with two explanation lines, and k8rs's own deadline gets the specific
/// one** (`screens/dialogs.md` § The cluster said no, *1b, when it is k8rs's own deadline that
/// answers*; NOTES § D273).
///
/// **Both readings are asserted, because the claim is that one box says two things** — a test
/// fed only the expiry sentence would pass on a box that had lost the generic one entirely, and
/// the generic sentence is the right answer for every `Unanswered` a socket caused.
///
/// **The quote heading is asserted absent by name.** `What the cluster sent back:` is 1c's, for
/// the one state that can honestly carry the cluster's words; this sentence is k8rs's own, and
/// the heading over it would say the cluster answered when it did not.
///
/// **The full stop is part of the words and not a geometry detail** — `back.` and `back` are
/// different tokens, so the join below carries it (NOTES § D273's last two sections: `ops.rs`'s
/// vocabulary is composable fragments and the terminator is the renderer's).
///
/// **The sentence fed in is written here rather than called**, because `ops::heard_nothing` is
/// private to its own file; its *number* is still read off `ops::CHECK_DEADLINE` and not typed.
#[test]
fn the_unanswered_box_says_how_long_it_waited_where_that_is_what_k8rs_knows() {
    let blocks = fenced("dialogs.md", "## The cluster said no");
    assert_eq!(
        blocks.len(),
        5,
        "screens/dialogs.md § The cluster said no no longer draws a 409, 1a, 1b's two readings \
         and 1c"
    );
    let waited = format!(
        "k8rs waited {} seconds for the cluster to check this change and heard nothing back",
        crate::ops::CHECK_DEADLINE.as_secs()
    );
    let mut both = Vec::new();
    for (nth, said) in [(2, None), (3, Some(waited))] {
        let drawn = box_of(views::Modal::Refused {
            sent: false,
            fault: crate::k8s::Fault::Unanswered,
            said,
        });
        let mockup = nested(&blocks[nth]);
        assert!(
            drawn.last().is_some_and(|row| row.starts_with('└')),
            "the box does not close:\n{}",
            drawn.join("\n")
        );
        // **The title bar is the width and the title in one row**, which is the page's own
        // *same title, same width* said as a comparison instead of as two numbers.
        assert_eq!(
            drawn[0], mockup[0],
            "1b's block {nth} does not draw screens/dialogs.md's title bar"
        );
        assert_eq!(
            drawn.len(),
            mockup.len(),
            "1b's block {nth} is a different height from the box screens/dialogs.md draws:\n{}\n{}",
            drawn.join("\n"),
            mockup.join("\n")
        );
        assert_eq!(
            said_by(&drawn),
            said_by(&mockup),
            "1b's block {nth} does not say what screens/dialogs.md says"
        );
        assert!(
            !drawn.join("\n").contains("What the cluster sent back:"),
            "1b drew 1c's quote heading over a sentence k8rs wrote itself:\n{}",
            drawn.join("\n")
        );
        both.push(drawn);
    }

    // **The button is compared between the two drawn boxes and never against the page**, because
    // its centring is the one thing on any box this file does not take from the screen file
    // ([`the_delete_boxes_are_the_screen_files_boxes_but_for_the_gap_between_two_buttons`]).
    let button = |box_: &[String]| {
        box_.iter()
            .find(|row| row.contains("esc dismiss"))
            .cloned()
            .unwrap_or_else(|| panic!("no button row on\n{}", box_.join("\n")))
    };
    assert_eq!(
        button(&both[0]),
        button(&both[1]),
        "the two readings differ in their button"
    );
    assert_ne!(
        said_by(&both[0]),
        said_by(&both[1]),
        "the deadline reading drew the sentence for a cause k8rs could not name"
    );
}

/// **1b's own sentence came off the API too, so it is bounded like the quote is** — the security
/// gate's *sizes are bounded* row, read at the second place a string `k8s::FREE_TEXT` sized can
/// now reach this box ([`refused`], NOTES § D273).
///
/// **The sibling of [`a_consequence_too_long_for_the_box_gives_way_before_the_buttons_do`], and
/// for the same reason**: [`boxed`] sizes a box to the lines it is handed, so an unbounded
/// explanation is not a long sentence — it is a box taller than the body with `esc dismiss`, its
/// last row, the first thing ratatui clips. Nothing `ops.rs` builds comes near 4096 bytes; the
/// field is a `String` off the wire and this is reachable by construction.
///
/// **The mark is asserted, not just the height**, because a silent clip passes a height check and
/// is what `screens/widgets.md` § 7 bans by name.
#[test]
fn a_refusal_sentence_the_size_the_api_allows_gives_way_before_the_button_does() {
    let said = "word ".repeat(820);
    assert!(
        said.len() > 4000,
        "the fixture stopped being the size k8s::FREE_TEXT allows"
    );
    let drawn = box_of(views::Modal::Refused {
        sent: false,
        fault: crate::k8s::Fault::Unanswered,
        said: Some(said),
    });
    // **Exactly [`MODAL_ROWS`], not *at most* it** — a sentence with more to say than the box has
    // room for spends every row the box has and not one more. `<=` passes on a budget that is one
    // row short as readily as on the right one, and a budget arrived at by arithmetic is where a
    // row goes missing (the same arithmetic one box along, in [`confirm`], has four surviving
    // mutants recorded against it for exactly this reason).
    assert_eq!(
        drawn.len(),
        MODAL_ROWS + 2,
        "the box is {} rows where it has room for {}:\n{}",
        drawn.len(),
        MODAL_ROWS + 2,
        drawn.join("\n")
    );
    assert!(
        drawn.iter().any(|row| row.contains(CUT)),
        "the sentence was clipped in silence:\n{}",
        drawn.join("\n")
    );
    assert!(
        drawn.iter().any(|row| row.contains("[ esc dismiss ]")),
        "a long sentence pushed the way out off the box:\n{}",
        drawn.join("\n")
    );
    assert!(
        drawn.last().is_some_and(|row| row.starts_with('└')),
        "the box does not close:\n{}",
        drawn.join("\n")
    );
}

/// **The three widths, and the margins that are nobody's choice** (`screens/widgets.md` § 5).
///
/// **The margins are asserted as *what centring leaves* and never as numbers off the page.** The
/// screen file measures against a 68-column body and the product's floor gives 78, so the 4/4 it
/// draws is 9/9 here; what has to hold at either width is that the two sides differ by at most
/// one and that nothing chose them.
#[test]
fn a_dialog_picks_one_of_three_widths_and_centring_decides_the_rest() {
    let mut wide = scaling();
    wide.consequence = "This asks the cluster to remove the deployment and every copy of the app \
                        it runs. k8rs has not read what may be attached to it, and something \
                        there may delay this or act first — left alone, nothing is left running."
        .to_owned();
    for (expected, modal) in [
        (CROWDED_BOX, views::Modal::Confirm(scaling())),
        (CROWDED_BOX, views::Modal::Confirm(wide)),
        (CROWDED_BOX, views::Modal::Confirm(restarting())),
        (CROWDED_BOX, views::Modal::Confirm(deleting())),
        (
            DISMISS_BOX,
            views::Modal::Refused {
                sent: false,
                fault: crate::k8s::Fault::Rejected,
                said: None,
            },
        ),
        (
            DISMISS_BOX,
            views::Modal::Gone {
                object: scaling().object,
                recreated: false,
            },
        ),
    ] {
        let alerts = Pane::Ready(vec![oom()]);
        let now = now();
        let log = logged_pair();
        let drawn = rows(&render(&over(modal), &opened_over(&alerts, &now, &log)));
        let box_ = nested(&drawn);
        assert_eq!(
            width(&box_[0]),
            usize::from(expected) + 2,
            "the box is not {expected} columns of interior"
        );

        // The margins are read off the frame rather than chosen: the columns before the box's
        // own `┌` and after its `┐`, inside the frame's borders.
        let row = &box_[0];
        let title = drawn
            .iter()
            .find(|line| line.contains(row.as_str()))
            .expect("the title row");
        let left = title.chars().position(|c| c == '┌').expect("a left corner") - 1;
        let right = usize::from(MIN_WIDTH)
            - 1
            - title
                .chars()
                .position(|c| c == '┐')
                .expect("a right corner")
            - 1;
        assert!(
            left.abs_diff(right) <= 1,
            "the box is off-centre: {left} left, {right} right"
        );
        assert_eq!(
            left + right + width(row),
            usize::from(MIN_WIDTH) - 2,
            "the margins and the box do not fill the body"
        );
    }
}

/// **There is one box width now, and the `$` line is what gives way inside it**
/// (`screens/widgets.md` § 5's table, whose 58 row reads **retired**;
/// `screens/dialogs.md` § Scale). The narrow box used to win wherever the whole command already
/// fit its own `$` room; `--context` is on every taught line unconditionally, so no command any
/// of these boxes draws fits 58 — the choice is not narrower today, it is unreachable, which is
/// why `box_width`'s `fits`/`whole` condition went with the constant (NOTES § D278).
///
/// **What survives the retirement is the claim that was always the point**: the `$` row ends
/// inside the box's own border, **in both states** — a command that needs one cut and one that
/// needs three — because a fix for a clip is one column from reintroducing it.
#[test]
fn the_command_line_is_cut_to_the_one_box_width_and_never_past_its_border() {
    let short = scaling();
    let mut long = scaling();
    long.object = views::Object::new(
        "deployment",
        Some("payments-production".to_owned()),
        "checkout-worker".to_owned(),
        Some("8656c3ec-0f0e-4d0e-9f0b-2a1d3c4b5a69".to_owned()),
    );
    long.kubectl = "kubectl --context prod-eu scale deployment/checkout-worker --replicas=3 \
                    -n payments-production"
        .to_owned();
    assert_eq!(
        short.consequence, long.consequence,
        "the two dialogs differ in their consequence too, so the width says nothing about the \
         command"
    );
    // **One width for both**, and for a dialog that asks for a typed name as well.
    for dialog in [&short, &long, &deleting(), &restarting()] {
        assert_eq!(
            box_width(dialog),
            CROWDED_BOX,
            "a Confirm was drawn at a width the box no longer has"
        );
    }

    // What the reader actually reads, and that it ends inside the box either way.
    let command = |dialog: views::Dialog| {
        let box_ = box_of(views::Modal::Confirm(dialog));
        let row = box_
            .iter()
            .find(|row| row.contains("$ kubectl"))
            .unwrap_or_else(|| panic!("no `$` row in\n{}", box_.join("\n")))
            .clone();
        assert!(
            row.starts_with('│') && row.ends_with('│'),
            "the `$` row overran the box's own border: {row:?}"
        );
        assert_eq!(
            width(&row),
            width(&box_[0]),
            "the `$` row is not the width of the box it is drawn in: {row:?}"
        );
        row.trim_matches('│').trim().to_owned()
    };
    // **One cut: `--replicas=3` goes whole and `-n`'s value shrinks** — the order
    // `screens/dialogs.md` § Scale draws.
    assert_eq!(
        command(short),
        "$ kubectl --context prod-eu scale deployment/web -n paymen…",
        "the scale box is not the line screens/dialogs.md § Scale draws"
    );
    // **Three: the trailing flag, then `-n` bare, then `--context`'s own value** — and the
    // object is named in full at the end of all of it
    // (§ The context flag never disappears without a trace either, step 3).
    assert_eq!(
        command(long),
        "$ kubectl --context p… scale deployment/checkout-worker -n…",
        "the object gave way before the context's value did"
    );
}

/// **The confirm button is dim until [`views::Dialog::armed`], and then it is
/// [`theme::FOCUS`]** (`screens/widgets.md` § 5, `screens/dialogs.md` rule 3) — the ctrl-key-slip
/// guard, seen on the screen rather than asked of the type.
///
/// **`esc cancel` is asserted here only for a dialog whose check has answered**, which is every
/// box `screens/dialogs.md` draws but the two pending ones. The window where it dims is
/// [`views::Dialog::waiting`]'s and has its own test below — and the pair is the point: the
/// delete dialog here has its verdict and an unfinished name, so cancel is live while confirm is
/// not.
#[test]
fn the_confirm_button_is_only_lit_once_the_dialog_is_armed() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = logged_pair();
    let screen = opened_over(&alerts, &now, &log);

    // **The columns are found on the row rather than counted off it** — `x + 15` into
    // `[ esc cancel ]` was this closure's first draft, and a hand-computed offset into a row this
    // file also lays out is the test agreeing with the implementation's arithmetic.
    let lit = |dialog: views::Dialog| {
        let armed = dialog.armed();
        let drawn = render(&over(views::Modal::Confirm(dialog)), &screen);
        let text = rows(&drawn);
        let (y, row) = text
            .iter()
            .enumerate()
            .find(|(_, row)| row.contains("[ delete ]"))
            .unwrap_or_else(|| panic!("no `[ delete ]` on\n{}", text.join("\n")));
        let reversed = |needle: &str| {
            let byte = row
                .find(needle)
                .unwrap_or_else(|| panic!("{needle:?} is not on {row:?}"));
            let x = u16::try_from(row[..byte].chars().count()).expect("a column inside the frame");
            drawn
                .cell((x, u16::try_from(y).expect("a row inside the frame")))
                .expect("a cell the row says is there")
                .modifier
                .contains(ratatui::style::Modifier::REVERSED)
        };
        (armed, reversed("[ delete ]"), reversed("[ esc cancel ]"))
    };

    let half = deleting();
    assert_eq!(
        lit(half),
        (false, false, false),
        "a half-typed name lit the delete button"
    );

    let mut whole = deleting();
    whole.typed = views::Input::default();
    for character in "web-7d9f4".chars() {
        whole.typed.push(character);
    }
    assert_eq!(
        lit(whole),
        (true, true, false),
        "the typed name did not light the button, or it lit `esc cancel` with it"
    );
}

/// **Both buttons dim while the cluster's check is out, and each un-dims on its own**
/// (`screens/dialogs.md` § While the check is still on the wire, ruling 1; `screens/widgets.md`
/// § 5's *"the cancel button beside it is not exempt from the same wait"*). `esc` is inert for
/// exactly that window (NOTES § D214) and the button read live over a press that did nothing.
///
/// **The independence is the half one pending box cannot show**, and a delete is the state that
/// has it: its verdict is `Some` from the first frame (NOTES § D225 ruling 1) while the name is
/// half typed, so `esc cancel` is live because the cluster answered and `[ delete ]` is not
/// because the reader has not finished. A cancel button keyed off [`views::Dialog::armed`] would
/// draw that one dim and be wrong in a state this product reaches on every delete.
#[test]
fn both_buttons_dim_while_the_check_is_out_and_each_undims_on_its_own() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = logged_pair();
    let screen = opened_over(&alerts, &now, &log);

    // The colour of a button's own first column, found on the row rather than counted off it.
    let inked = |dialog: views::Dialog, needle: &str| {
        let drawn = render(&over(views::Modal::Confirm(dialog)), &screen);
        let text = rows(&drawn);
        let (y, row) = text
            .iter()
            .enumerate()
            .find(|(_, row)| row.contains(needle))
            .unwrap_or_else(|| panic!("no {needle:?} on\n{}", text.join("\n")));
        let byte = row.find(needle).expect("the row holds it, found above");
        let x = u16::try_from(row[..byte].chars().count()).expect("a column inside the frame");
        drawn
            .cell((x, u16::try_from(y).expect("a row inside the frame")))
            .expect("a cell the row says is there")
            .fg
    };
    let dim = ink(theme::DIM, Depth::TrueColor);
    let text = ink(theme::TEXT, Depth::TrueColor);
    assert_ne!(
        dim, text,
        "the palette draws the two roles the same, so nothing below can fail"
    );

    let mut pending = scaling();
    pending.verdict = None;
    assert_eq!(
        (
            inked(pending.clone(), "[ esc cancel ]"),
            inked(pending, "[ ⏎ do it ]")
        ),
        (dim, dim),
        "a box waiting on its dry-run drew a button at full weight"
    );

    // The same dialog, answered: both sides un-dim together because a press-only dialog arms on
    // the verdict alone.
    let answered = scaling();
    assert!(answered.armed(), "the answered scale box is not armed");
    assert_eq!(
        (
            inked(answered.clone(), "[ esc cancel ]"),
            inked(answered, "[ ⏎ do it ]")
        ),
        (text, text),
        "the verdict landed and a button stayed dim"
    );

    // A delete with its verdict and half a name: the two answers differ, and that is the claim.
    let half = deleting();
    assert!(
        !half.waiting() && !half.armed(),
        "the delete fixture is not in the one state where the two buttons disagree"
    );
    assert_eq!(
        (
            inked(half.clone(), "[ esc cancel ]"),
            inked(half, "[ delete ]")
        ),
        (text, dim),
        "cancel followed the confirm button's own arming rule instead of the verdict"
    );
}

/// **A dialog floats over the screen it was opened from, and [`help`] is the one that does not**
/// (`screens/widgets.md` § 5). The sidebar and the pane are still there beside the box; nothing
/// of them shows through inside it, which is the `Clear`.
#[test]
fn a_dialog_floats_over_the_screen_and_clears_only_its_own_box() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = logged_pair();
    let drawn = rows(&render(
        &over(views::Modal::Confirm(scaling())),
        &opened_over(&alerts, &now, &log),
    ));

    assert!(
        drawn.iter().any(|row| row.contains("ALERTS")),
        "the sidebar went away under a dialog — that is Help's exception, not every modal's"
    );
    assert!(
        drawn[1].contains('┬') && drawn[18].contains('┴'),
        "the frame lost the sidebar's own junctions: {:?} {:?}",
        drawn[1],
        drawn[18]
    );
    assert_eq!(drawn.len(), usize::from(MIN_HEIGHT), "the frame is 24 rows");
    assert_eq!(
        unframed(&drawn[19]),
        "$ kubectl get statefulsets -A --watch",
        "the command log strip stopped showing real commands behind the box"
    );

    // Nothing of the pane underneath survives inside the box: the divider is drawn before the
    // modal, and its column is inside this box's own columns.
    for row in nested(&drawn) {
        let inside: String = row.chars().skip(1).take(row.chars().count() - 2).collect();
        assert!(
            !inside.contains('│'),
            "the sidebar's divider was ruled through the dialog: {row:?}"
        );
    }
}

/// **A crafted object name cannot grow a title bar** — the security gate's *sizes are bounded*,
/// and invariant 9's own class read at the one place a name is drawn inside a border
/// (`screens/widgets.md` § 7).
///
/// **10 000 characters is past anything an API server accepts** and past `k8s::IDENTIFIER`'s own
/// 512-byte bound; what is asserted is that the frame is still 80×24, that the box is still the
/// width it chose, and that the cut is *marked* — a `Block` left to clip its own title would draw
/// a name flush against the border with nothing to say it was cut.
#[test]
fn a_ten_thousand_character_name_does_not_grow_the_box_it_is_drawn_in() {
    let mut dialog = deleting();
    dialog.object.name = "w".repeat(10_000);
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = logged_pair();
    let drawn = rows(&render(
        &over(views::Modal::Confirm(dialog)),
        &opened_over(&alerts, &now, &log),
    ));

    assert_eq!(drawn.len(), usize::from(MIN_HEIGHT));
    for row in &drawn {
        assert_eq!(width(row), usize::from(MIN_WIDTH), "{row:?}");
    }
    let box_ = nested(&drawn);
    assert_eq!(width(&box_[0]), usize::from(CROWDED_BOX) + 2);
    assert!(
        box_[0].contains(CUT),
        "the title was cut in silence: {:?}",
        box_[0]
    );
}

/// **What the cluster sent back is cut to the rows the box has left** — the one string in any
/// dialog that came off the API, and the reason [`cut`] takes a line cap
/// (NOTES § D217: a `fieldValidation=Strict` rejection hands back the object that was sent).
#[test]
fn a_four_kilobyte_refusal_is_cut_to_the_box_and_says_so() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = logged_pair();
    let said = "denied: ".to_owned() + &"replicas may not exceed five ".repeat(140);
    assert!(said.len() > 4000, "the fixture stopped being oversized");

    let drawn = rows(&render(
        &over(views::Modal::Refused {
            sent: false,
            fault: crate::k8s::Fault::Rejected,
            said: Some(said.clone()),
        }),
        &opened_over(&alerts, &now, &log),
    ));
    assert_eq!(
        drawn.len(),
        usize::from(MIN_HEIGHT),
        "a refusal blew the frame"
    );
    let box_ = nested(&drawn);
    assert!(
        box_.len() <= MODAL_ROWS + 2,
        "the box grew to {} rows on a 4 kB message",
        box_.len()
    );
    assert!(
        box_.iter().any(|row| row.contains(CUT)),
        "4 kB was dropped in silence:\n{}",
        box_.join("\n")
    );
    // And the two sentences the box owns are still both on it — the quote gave way, not them.
    assert!(box_.iter().any(|row| row.contains("Nothing was changed.")));
    assert!(box_.iter().any(|row| row.contains("it stopped this one.")));

    // A refusal the cluster did not explain drops the heading with the quote.
    let quiet = nested(&rows(&render(
        &over(views::Modal::Refused {
            sent: false,
            fault: crate::k8s::Fault::Rejected,
            said: None,
        }),
        &opened_over(&alerts, &now, &log),
    )));
    assert!(
        !quiet.iter().any(|row| row.contains("sent back")),
        "an unexplained refusal still promised something sent back"
    );
}

/// **The verdict's row is reserved before the verdict arrives**, so the box does not grow by one
/// under the reader the moment the cluster answers (`screens/dialogs.md` § Scale — the check is a
/// real round trip for a scale and a restart).
#[test]
fn a_dialog_is_the_same_height_before_and_after_the_check_answers() {
    let mut waiting = scaling();
    waiting.verdict = None;
    assert_eq!(
        box_of(views::Modal::Confirm(waiting)).len(),
        box_of(views::Modal::Confirm(scaling())).len(),
        "the box changed height when the dry-run came back"
    );
}

/// **The typed name keeps its end** — a field shows where the cursor is, which is the opposite
/// end from every other cut in this file ([`shortened`] against [`clipped`]).
#[test]
fn the_typed_field_keeps_the_end_of_a_name_too_long_for_it() {
    let mut dialog = deleting();
    dialog.object.name = "w".repeat(400);
    dialog.asks = Some("w".repeat(400));
    dialog.typed = views::Input::default();
    for character in "abcdefghij".chars().cycle().take(400) {
        dialog.typed.push(character);
    }
    let box_ = box_of(views::Modal::Confirm(dialog));
    let field = box_
        .iter()
        .find(|row| row.contains('_'))
        .expect("the field's own row");
    assert!(
        field.contains("hij_"),
        "the field kept the head and lost the cursor: {field:?}"
    );
    assert!(field.contains(CUT), "the field cut in silence: {field:?}");
    assert_eq!(
        width(field),
        usize::from(CROWDED_BOX) + 2,
        "the field ran past its box"
    );

    // **And a character that is two columns wide, which is what the ASCII above could not see.**
    // `views::Input::push` accepts any printable character up to `k8s::IDENTIFIER` bytes, so a
    // pasted CJK name reaches this field; the padding counted `char`s until 2026-09-10, and one
    // glyph pushed the field's right border a column out while ten pushed it off the box
    // (`tester`). Every row of the box has to stay exactly as wide as the box.
    //
    // **Asserted on buffer cells and not on [`rows`]**, because both string views of a wide glyph
    // lie in opposite directions: [`nested`] slices by `char` and ratatui stores the second cell
    // of a wide character as its own symbol, so a reconstructed row measures one column long per
    // glyph whether or not the renderer drifted. What cannot be faked is *which cell* the box's
    // right border is written into.
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = logged_pair();
    let screen = opened_over(&alerts, &now, &log);
    // The x of every `│` on the row carrying the field's cursor.
    let edges = |dialog: views::Dialog| {
        let drawn = render(&over(views::Modal::Confirm(dialog)), &screen);
        let row = (0..drawn.area.height)
            .find(|y| {
                (0..drawn.area.width)
                    .any(|x| drawn.cell((x, *y)).is_some_and(|c| c.symbol() == "_"))
            })
            .expect("the field's own row");
        (0..drawn.area.width)
            .filter(|x| {
                drawn
                    .cell((*x, row))
                    .is_some_and(|cell| cell.symbol() == "│")
            })
            .collect::<Vec<_>>()
    };

    // Six: the frame's own two, the nested box's two, and the field's two.
    let wanted = edges(deleting());
    assert_eq!(wanted.len(), 6, "the field's row lost a border: {wanted:?}");
    for count in [1, 10, 30] {
        let mut wide = deleting();
        wide.typed = views::Input::default();
        for character in "宽".chars().cycle().take(count) {
            wide.typed.push(character);
        }
        assert_eq!(
            edges(wide),
            wanted,
            "{count} wide character(s) moved a border out of its column"
        );
    }
}

/// **A refusal says what is true of *this* fault, and never a sentence that fits the common
/// case** (`screens/dialogs.md` § The cluster said no, PRIOR-ART § C1).
///
/// **Three shapes, and two of them the box used to get wrong.** `delete` sends no check at all
/// (NOTES § D225 ruling 1), so every delete refusal is post-send — *"This is the check that runs
/// before the real change"* is false of all of them. And invariant 2 names the state where
/// *"Nothing was changed."* is unknowable in those words: a dead socket on a delete leaves the
/// request on the wire with no answer.
#[test]
fn a_refusal_says_what_is_true_of_the_fault_it_carries() {
    for (sent, fault, title, outcome, because) in [
        (
            false,
            Fault::Rejected,
            "The cluster refused this",
            "Nothing was changed.",
            "This is the check that runs before the real change",
        ),
        (
            true,
            Fault::Refused,
            "The cluster refused this",
            "Nothing was changed.",
            "This was the real change, not a check.",
        ),
        (
            true,
            Fault::Unanswered,
            "The cluster never answered",
            "k8rs does not know whether the change was made.",
            "This was the real change, not a check.",
        ),
    ] {
        let drawn = spoken_box(&box_of(views::Modal::Refused {
            sent,
            fault,
            said: None,
        }));
        for wanted in [title, outcome, because] {
            assert!(
                drawn.contains(wanted),
                "sent {sent}: {wanted:?} is not on the box:\n{drawn}"
            );
        }
    }

    // The two sentences are exclusive — a post-send refusal never claims a check stopped it, and
    // a refusal that cannot know never claims nothing changed.
    let post = spoken_box(&box_of(views::Modal::Refused {
        sent: true,
        fault: Fault::Unanswered,
        said: None,
    }));
    assert!(!post.contains("Nothing was changed."), "{post}");
    assert!(!post.contains("check that runs before"), "{post}");
}

/// A box's text as one line — borders dropped and the wrap undone — so an assertion about a
/// sentence is not an assertion about where it happened to break.
fn spoken_box(box_: &[String]) -> String {
    box_.iter()
        .flat_map(|row| {
            row.trim_matches(|c| {
                c == '\u{250c}' || c == '\u{2510}' || c == '\u{2514}' || c == '\u{2518}'
            })
            .trim_matches('\u{2502}')
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// **Only a pod and a replicaset get the hedge** (`screens/dialogs.md` § The object went away):
/// nothing recreates a deployment, a statefulset, a daemonset or a node on its own.
///
/// **And no variant names a successor** (NOTES § D44) — the line this box used to carry,
/// `replaced by web-2c81a 3 seconds ago`, was an inference off a shared `ownerReference` that
/// named the wrong pod whenever the ReplicaSet scaled for another reason.
#[test]
fn already_gone_hedges_only_where_something_puts_one_back() {
    let hedged = spoken_box(&box_of(views::Modal::Gone {
        object: deleting().object,
        recreated: true,
    }));
    assert!(hedged.contains("This pod is already gone"), "{hedged}");
    assert!(hedged.contains("k8rs has not checked whether"), "{hedged}");

    let bare = spoken_box(&box_of(views::Modal::Gone {
        object: scaling().object,
        recreated: false,
    }));
    assert!(bare.contains("This deployment is already gone"), "{bare}");
    assert!(
        bare.contains("Nothing will take its place on its own."),
        "{bare}"
    );
    assert!(
        !bare.contains("has not checked whether"),
        "a deployment was hedged as though something would recreate it: {bare}"
    );

    for box_ in [hedged, bare] {
        assert!(
            !box_.contains("replaced by"),
            "the successor line NOTES § D44 removed is back: {box_}"
        );
    }
}

/// **A consequence longer than the box gives way with a mark, and the buttons never do**
/// — the security gate's *sizes are bounded* read at the one place a dialog's own text could
/// outgrow the 24 rows this product is drawn to.
///
/// **`views::Dialog::consequence` is a `String`**, so this is reachable by construction even
/// though nothing `ops.rs` builds comes near it: the widest real one, a paused Deployment's
/// restart, lands on the budget exactly, which is asserted below rather than assumed. Without the
/// cut the box would simply be taller than the body and ratatui would clip it — with the confirm
/// button, the last row, the first thing to go.
#[test]
fn a_consequence_too_long_for_the_box_gives_way_before_the_buttons_do() {
    let mut dialog = scaling();
    dialog.consequence = "this starts one more copy of your app and here is a sentence that goes \
                          on "
    .repeat(60);
    assert!(
        dialog.consequence.len() > 4000,
        "the fixture stopped being oversized"
    );
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = logged_pair();
    let drawn = rows(&render(
        &over(views::Modal::Confirm(dialog)),
        &opened_over(&alerts, &now, &log),
    ));

    assert_eq!(drawn.len(), usize::from(MIN_HEIGHT), "the frame is 24 rows");
    let box_ = nested(&drawn);
    assert!(
        box_.len() <= MODAL_ROWS + 2,
        "the box grew to {} rows",
        box_.len()
    );
    assert!(
        sentences(&box_).any(|row| row.contains(CUT)),
        "the consequence was clipped in silence:\n{}",
        box_.join("\n")
    );
    // **`$ kubectl` and not `$ kubectl scale`**: since `--context` the verb is past the cut on
    // this fixture, and what this row is about is the `$` line still being in the box at all.
    for kept in ["[ ⏎ do it ]", "[ esc cancel ]", "$ kubectl"] {
        assert!(
            box_.iter().any(|row| row.contains(kept)),
            "{kept:?} was pushed out of the box by a long consequence"
        );
    }

    // **The blank row under the consequence is no longer what this pair counts, and the reason
    // is that the narrow box it belonged to is retired** (NOTES § D278). It used to exist only
    // at `CONFIRM_BOX`; since every taught line carries `--context`, no `Confirm` reaches that
    // width, and the row moved onto the one width that is left — kept where the text above the
    // verdict is short enough to spare it, which is `ui::CONSEQUENCE_LINES` and which neither
    // fixture below is (`screens/widgets.md` § 5, `screens/dialogs.md` § Scale).
    //
    // **What the pair still proves is the row budget itself**, which is why it is kept rather
    // than deleted: four rows of warning and five both land on the ceiling, neither pushes the
    // `$` line or a button off, and the sentences give way in order.
    //
    // **`ops.rs` builds no such dialog**: its one warning is three rows and attaches only to a
    // restart. The budget is a total guard on a `views::Dialog` anyone can construct.
    let narrow = |warning: &str| {
        box_of(views::Modal::Confirm(views::Dialog {
            consequence:
                "This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies."
                    .to_owned(),
            warning: Some(warning.to_owned()),
            ..scaling()
        }))
    };

    let roomy = narrow(ROOMY);
    assert_eq!(
        width(&roomy[0]),
        usize::from(CROWDED_BOX) + 2,
        "there is one box width and this fixture is not drawn at it"
    );
    // **Both land on the ceiling and neither goes past it**, which is what a total guard on an
    // arbitrary `views::Dialog` is for.
    let wordy = narrow(WORDY);
    for (named, box_) in [("four rows", &roomy), ("five rows", &wordy)] {
        assert!(
            box_.len() <= MODAL_ROWS + 2,
            "{named} of warning grew past the ceiling:\n{}",
            box_.join("\n")
        );
        for kept in ["$ kubectl", "[ ⏎ do it ]", "[ esc cancel ]"] {
            assert!(
                box_.iter().any(|row| row.contains(kept)),
                "{named} of warning pushed {kept:?} off the box:\n{}",
                box_.join("\n")
            );
        }
    }
    // **Neither costs a sentence** — the blanks are spent before any text is
    // (`screens/widgets.md` § 5), and both of these fit once they are. What a box has to be to
    // lose a clause is the very long consequence at the top of this test, which is where that
    // half of the ordering is proven.
    for (named, box_) in [("four rows", &roomy), ("five rows", &wordy)] {
        assert!(
            !sentences(box_).any(|row| row.contains(CUT)),
            "{named} of warning cost a sentence a box still had blank rows to spend for:\n{}",
            box_.join("\n")
        );
    }
    // **Five rows is the one that lands on the ceiling exactly**, which is what makes the ceiling
    // a measurement rather than a limit nothing reaches. Four sits one row under it, because the
    // blank row that used to fill that gap belonged to the retired narrow box (NOTES § D278).
    assert_eq!(
        wordy.len(),
        MODAL_ROWS + 2,
        "the wordiest box a `views::Dialog` can carry no longer reaches the ceiling:\n{}",
        wordy.join("\n")
    );
    assert_eq!(
        roomy.len() + 1,
        wordy.len(),
        "the extra row of warning stopped costing a row:\n{}\n{}",
        roomy.join("\n"),
        wordy.join("\n")
    );
}

/// **The widest thing `ops.rs` actually builds lands on the budget exactly, and that is what
/// makes the budget a measurement** (`screens/widgets.md` § 5 — § Restart's paused variant and
/// § Drain both land on [`MODAL_ROWS`], the ceiling itself).
///
/// **Every consequence the shipped operations produce is fed here**, which is NOTES § D29's rule:
/// a check is proven only for the shapes the real pipeline hands it. The three restart sentences
/// are `ops::rollout`'s, the paused warning is the driver's `while_paused`, and the delete
/// sentences are `screens/dialogs.md` § Delete's own bullets.
///
/// **The warning is paired only with the one kind that can carry it**, which is the defect this
/// test found in its own first draft: `ops::paused` reads `/spec/paused`, a StatefulSet and a
/// DaemonSet do not have one, and their dialogs never grow the line (NOTES § D224). A cartesian
/// product over every consequence invented a box the pipeline cannot produce and failed on it.
#[test]
fn every_consequence_the_operations_build_fits_the_box_it_is_drawn_in() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = logged_pair();
    let screen = opened_over(&alerts, &now, &log);

    let scale_up = "This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies.";
    let scale_zero = "This stops all 3 copies of your app — nothing will be left running. Right \
                      now: 3 copies. After: 0 copies.";
    let deployment = "This asks Kubernetes to replace every copy of your app with a new one. How \
                      many stop at the same time is a setting on this deployment — it can be a \
                      few, or all of them at once. A paused deployment will not start until you \
                      resume it.";
    let statefulset = "This asks Kubernetes to replace every copy of your app with a new one, \
                       working down from the highest-numbered copy. How many stop at the same \
                       time, how far down it goes, and whether it waits for you to delete a copy \
                       yourself are all settings on this statefulset.";
    let daemonset = "This asks Kubernetes to replace the copy of your app on each node it runs \
                     on. How many nodes it takes at a time, and whether it waits for you to \
                     delete a copy yourself, are settings on this daemonset.";
    let pod = "This removes the pod. Whatever created it will normally replace it — k8rs has not \
               checked whether anything did.";
    let replicaset = "This removes the replicaset and every pod it manages. Whatever created it \
                      will normally replace it — k8rs has not checked whether anything did.";
    let removed = "This asks the cluster to remove the deployment and every copy of the app it \
                   runs. k8rs has not read what may be attached to it, and something there may \
                   delay this or act first — left alone, nothing is left running.";
    let per_node = "This asks the cluster to remove the daemonset and the copy of the app it \
                    runs on every node. k8rs has not read what may be attached to it, and \
                    something there may delay this or act first — left alone, nothing is left \
                    running.";
    let node = "This asks the cluster to remove its record of node-3, not the machine. Something \
                attached to it, unread by k8rs, may delay this or act first. Left alone, its \
                pods are deleted and the machine keeps running until its kubelet restarts.";

    // **The relations `ops::scale` really builds**, one per arm of its own match: up by one, up by
    // more, down by one, down to zero, down to zero from the only copy, and the count that is
    // already running. `tester` fed these and the three `reverted` delete sentences on
    // 2026-09-10; this is that list, kept rather than described.
    let scale_more = "This starts 3 more copies of your app. Right now: 2 copies. After: 5 \
                      copies.";
    let scale_down = "This stops 1 copy of your app. Right now: 3 copies. After: 2 copies.";
    let scale_only = "This stops the only copy of your app — nothing will be left running. Right \
                      now: 1 copy. After: 0 copies.";
    let scale_same = "This asks for the count web is already running. Right now: 3 copies. \
                      After: 3 copies.";
    let reverted_sts = "This asks the cluster to remove the statefulset and every copy of the \
                        app it runs. k8rs has not read what may be attached to it, and something \
                        there may delay this or act first — left alone, nothing is left running.";

    let mut widest = 0;
    let mut seen = 0;
    for (consequence, asks, pausable) in [
        (scale_up, false, false),
        (scale_more, false, false),
        (scale_down, false, false),
        (scale_only, false, false),
        (scale_same, false, false),
        (reverted_sts, true, false),
        (scale_zero, false, false),
        (deployment, false, true),
        (statefulset, false, false),
        (daemonset, false, false),
        (pod, true, false),
        (replicaset, true, false),
        (removed, true, false),
        (per_node, true, false),
        (node, true, false),
    ] {
        let warnings = if pausable {
            vec![None, Some(PAUSED.to_owned())]
        } else {
            vec![None]
        };
        for warning in warnings {
            let paused = warning.is_some();
            let dialog = views::Dialog {
                consequence: consequence.to_owned(),
                warning,
                asks: asks.then(|| "web".to_owned()),
                ..scaling()
            };
            let drawn = rows(&render(&over(views::Modal::Confirm(dialog)), &screen));
            assert_eq!(drawn.len(), usize::from(MIN_HEIGHT));
            let box_ = nested(&drawn);
            assert!(
                !sentences(&box_).any(|row| row.contains(CUT)),
                "a sentence ops.rs really builds had to be cut:\n{}",
                box_.join("\n")
            );
            assert!(
                box_.len() <= MODAL_ROWS + 2,
                "{consequence:?} (paused: {paused}) drew {} rows",
                box_.len()
            );
            widest = widest.max(box_.len());
            seen += 1;
        }
    }
    assert_eq!(seen, 16, "a consequence stopped being measured");
    assert_eq!(
        widest,
        MODAL_ROWS + 2,
        "nothing the operations build reaches the ceiling any more — the budget stopped being \
         measured against anything"
    );
}
/// **When the box runs out of rows the consequence gives way and the warning never does**
/// (NOTES § D224, `screens/widgets.md` § 5's *dropped — first, before any sentence is cut*).
///
/// **This is a defect this box shipped and had sent back.** The consequence and the warning were
/// wrapped into one block and cut at its end, so a consequence that filled the budget left
/// *"This deployment is paused, so nothing will be replaced…"* — D224's own correction truncated
/// to the half that names no fix, standing over a consequence a reader can half infer anyway.
/// The warning exists because without it the dialog claims copies were replaced when they were
/// not, so it is furniture the budget is counted *around*, like the verdict and the buttons.
///
/// **The consequence here is longer than anything `ops.rs` builds** — the real ones are measured
/// against the budget by `every_consequence_the_operations_build_fits_the_box_it_is_drawn_in`,
/// and none of them is cut. This one is what makes the ordering observable at all.
#[test]
fn a_full_box_cuts_the_consequence_and_never_the_warning_or_the_buttons() {
    // Seven wrapped lines at 59 — one more than the budget a 61-wide box leaves once its
    // three blank rows are gone, so the consequence is what has to give.
    let long = "This asks Kubernetes to replace every copy of your app with a new one. How many \
                stop at the same time is a setting on this deployment, it can be a few or all of \
                them at once, and a paused deployment will not start again until somebody has \
                gone and resumed it by hand with a command that this dialog does not run \
                for you at all, and never has done so. Not once."
        .to_owned();

    let whole = box_of(views::Modal::Confirm(views::Dialog {
        consequence: long.clone(),
        warning: None,
        ..restarting()
    }));
    assert!(
        !sentences(&whole).any(|row| row.contains(CUT)),
        "the fixture stopped landing on the budget exactly:\n{}",
        whole.join("\n")
    );

    let warned = box_of(views::Modal::Confirm(views::Dialog {
        consequence: long,
        warning: Some(PAUSED.to_owned()),
        ..restarting()
    }));
    assert_eq!(
        warned.len(),
        MODAL_ROWS + 2,
        "the box grew past the ceiling instead of cutting:\n{}",
        warned.join("\n")
    );
    // The warning is whole — every one of its three lines, including the one that names the fix.
    for kept in [
        "This deployment is paused, so nothing will be replaced",
        "until somebody resumes it with kubectl rollout resume",
        "the command above will refuse to run until then.",
    ] {
        assert!(
            warned.iter().any(|row| row.contains(kept)),
            "the warning gave way instead of the consequence — {kept:?} is gone:\n{}",
            warned.join("\n")
        );
    }
    // And the consequence is what gave way, visibly.
    assert!(
        warned.iter().any(|row| row.contains(CUT)),
        "nothing was marked, so nothing was cut:\n{}",
        warned.join("\n")
    );
    let marked_at = warned
        .iter()
        .position(|row| row.contains(CUT))
        .expect("a marked row");
    let warning_at = warned
        .iter()
        .position(|row| row.contains("This deployment is paused"))
        .expect("the warning's first row");
    assert!(
        marked_at < warning_at,
        "the mark landed after the warning, so the warning is what was cut:\n{}",
        warned.join("\n")
    );
    // The three rows the reader acts on are still on the box.
    for kept in [
        "[ ⏎ do it ]",
        "[ esc cancel ]",
        "The cluster checked it first",
    ] {
        assert!(
            warned.iter().any(|row| row.contains(kept)),
            "{kept:?} was pushed off the box"
        );
    }
}

/// **A warning *and* a typed-name field together — the case that overruns the box, and the order
/// the two sentences give way in.**
///
/// **`k8s-admin` constructed this and it was reachable**: five rows of warning plus the four a
/// field takes is fifteen rows into a thirteen-row box, and ratatui clips a box that will not fit
/// with the buttons — the last row — the first thing to go. `ops.rs` builds no such dialog today,
/// because its one warning attaches only to a restart and a restart asks for no name; the budget
/// is a total guard on a `views::Dialog` anyone can construct.
///
/// **The order is the whole of it.** Every blank row goes first (`screens/widgets.md` § 5), then
/// the consequence down to its last row, and only then the warning — which is NOTES § D224's
/// correction and the one sentence that may not vanish. Both cuts are marked, the field and the
/// buttons are untouched, and the box lands on the ceiling rather than past it.
#[test]
fn a_warning_over_a_typed_name_field_cuts_in_order_and_still_fits() {
    let crowded = box_of(views::Modal::Confirm(views::Dialog {
        warning: Some(WORDY.to_owned()),
        ..deleting()
    }));
    assert_eq!(
        crowded.len(),
        MODAL_ROWS + 2,
        "the box did not land on the ceiling:\n{}",
        crowded.join("\n")
    );

    let text: Vec<&String> = sentences(&crowded)
        .filter(|row| row.contains(CUT))
        .collect();
    assert_eq!(
        text.len(),
        2,
        "both sentences should carry a mark, one each:\n{}",
        crowded.join("\n")
    );
    // The consequence is down to its one row, and it is the first of the two. Row 0 is the
    // titled border and row 1 is the box's own top blank, which is not one of the four the
    // budget can give up.
    assert!(
        crowded[2].contains("This removes the pod") && crowded[2].contains(CUT),
        "the consequence is not the row that gave way first:\n{}",
        crowded.join("\n")
    );
    // The warning kept four of its five rows, and the row that names the fix is still there.
    assert!(
        crowded[3].contains("The cluster answered this check"),
        "the warning did not start where the consequence stopped:\n{}",
        crowded.join("\n")
    );
    // Nothing the reader acts on gave way. **`delete pod/…` and not `$ kubectl delete pod/…`**:
    // `--context` sits between the two since NOTES § D278, and what this row is about is the
    // object still being named on a line the reader is about to agree to.
    for kept in [
        "delete pod/web-7d9f4",
        "Type the pod's name to confirm:",
        "[ delete ]",
        "[ esc cancel ]",
    ] {
        assert!(
            crowded.iter().any(|row| row.contains(kept)),
            "{kept:?} was pushed off the box:\n{}",
            crowded.join("\n")
        );
    }
}

// --- THE CLUSTER PICKER ---

/// **The rows come out of `k8s::contexts` over a kubeconfig this file wrote** — which row is
/// derived, written, insecure or current is that function's answer, never a literal here.
fn contexts_of(yaml: &str) -> Vec<Choice> {
    crate::k8s::contexts(
        &kube::config::Kubeconfig::from_yaml(yaml).expect("a kubeconfig this file wrote itself"),
        None,
    )
}

/// **`screens/context.md` § The picker's four contexts**: `prod-eu` current with a written tag,
/// `staging` with none, `kind-k8rs` whose tag kind's own name gives away, and `dev-cluster` on an
/// Amazon host with TLS verification off.
///
/// **The hosts are reserved names, and the page's `prod-eu.internal` is not one**:
/// `scripts/security-guard.py` refuses an `https://` address in `src/` whose host could be
/// reached, so `prod-eu` serves `prod-eu.example` and the page's row is read with that one
/// substitution, width kept ([`on_the_page`]). The Amazon host carries no scheme, which is a
/// `server:` kube accepts and the one shape a `~aws` needs here.
const FOUR: &str = "apiVersion: v1\n\
     kind: Config\n\
     current-context: prod-eu\n\
     clusters:\n\
     - {name: prod, cluster: {server: 'https://prod-eu.example:6443'}}\n\
     - {name: staging, cluster: {server: 'https://staging.invalid:6443'}}\n\
     - {name: kind, cluster: {server: 'https://kind.invalid:41234'}}\n\
     - {name: dev, cluster: {server: 'dev.gr7.eu-west-1.eks.amazonaws.com', \
       insecure-skip-tls-verify: true}}\n\
     contexts:\n\
     - {name: prod-eu, context: {cluster: prod, user: u, \
       extensions: [{name: k8rs, extension: {tag: 'aws · prod'}}]}}\n\
     - {name: staging, context: {cluster: staging, user: u}}\n\
     - {name: kind-k8rs, context: {cluster: kind, user: u}}\n\
     - {name: dev-cluster, context: {cluster: dev, user: u}}\n\
     users: [{name: u, user: {token: k8rs-tests-fake-static-token}}]\n";

/// The picker opened over `prod-eu` while it is live — the ordinary `X`.
fn live() -> views::Connection {
    views::Connection::Live(Some("prod-eu".to_owned()))
}

fn picking(rows: &[Choice], connection: views::Connection) -> App {
    App {
        modal: Some(views::Modal::ContextPick(views::Picker::new(
            rows, connection,
        ))),
        ..App::default()
    }
}

/// A picker, the cursor moved `down` rows from where it opened and `typed` into `/`.
fn moved(rows: &[Choice], connection: views::Connection, down: usize, typed: &str) -> App {
    let mut app = picking(rows, connection);
    if let Some(views::Modal::ContextPick(picker)) = &mut app.modal {
        for _ in 0..down {
            picker.down(rows);
        }
        for character in typed.chars() {
            picker.filter.push(character);
        }
    }
    app
}

/// **The row of the page's own mockup that the picker's command log line is on** — so the line is
/// held to the page and not to [`views::GET_CONTEXTS`] itself.
const PAGE_LOG: &str = "$ kubectl config get-contexts";

/// The Alerts screen a picker is drawn over, with `contexts` handed in and the command log ending
/// on the line the picker put there.
fn pick_over<'a>(
    alerts: &'a Pane<Vec<Card>>,
    now: &'a Time,
    log: &'a [Stripped],
    contexts: &'a [Choice],
) -> Screen<'a> {
    let mut screen = screen(alerts, now);
    screen.log = log;
    screen.contexts = contexts;
    screen
}

/// The nested box as drawn on a terminal `height` rows tall, and every row of the frame.
fn picked_at(height: u16, app: &App, contexts: &[Choice]) -> (Vec<String>, Vec<String>) {
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let now = now();
    let log = [
        Stripped::of("$ kubectl get daemonsets -A --watch"),
        Stripped::of(views::GET_CONTEXTS),
    ];
    let screen = pick_over(&alerts, &now, &log, contexts);
    let drawn = rows(&render_at(MIN_WIDTH, height, app, &screen));
    (nested(&drawn), drawn)
}

/// [`picked_at`] at the 80×24 floor.
fn picked(app: &App, contexts: &[Choice]) -> (Vec<String>, Vec<String>) {
    picked_at(MIN_HEIGHT, app, contexts)
}

/// **How many rows the body has in a drawn frame** — counted off the frame, from under its top
/// border down to the first rule, rather than restated.
fn body_rows(frame: &[String]) -> usize {
    frame
        .iter()
        .position(|row| row.starts_with('├'))
        .expect("a frame has a rule under its body")
        - 2
}

/// **A drawn picker row at the width of the page `screens/context.md` is drawn on.**
///
/// The floor's 78-column body is ten columns wider than that page's 68, and the picker's box
/// widens with it (its § The tag column: *a wider terminal only widens that one slot*). So a **list
/// row** gives the ten back out of the end of its name slot — box columns 25 to 35, the name slot
/// being 20 on the page — and **every other row** out of its pad before the right border. Both are
/// asserted to be blank before they are dropped, so this cannot hide a character.
fn at_page_width(row: &str, list: bool) -> String {
    let characters: Vec<char> = row.chars().collect();
    let extra = characters.len() - 62;
    let from = if list {
        25
    } else {
        characters.len() - 1 - extra
    };
    let dropped: String = characters[from..from + extra].iter().collect();
    assert!(
        dropped.chars().all(|c| c == ' ' || c == '─'),
        "the columns the page does not have were not blank in {row:?}: {dropped:?}"
    );
    characters[..from]
        .iter()
        .chain(&characters[from + extra..])
        .collect()
}

/// The page's row with the one substitution [`FOUR`]'s hosts need, width kept.
fn on_the_page(row: &str) -> String {
    row.replacen("prod-eu.internal:6443", "prod-eu.example:6443 ", 1)
}

fn words_of(row: &str) -> Vec<String> {
    row.trim_matches('│')
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// A box's text as one line of words, its borders taken off first — a sentence that wraps is then
/// one string to search, and never broken by the `│` between its two rows.
fn inside(box_: &[String]) -> String {
    words(
        &box_
            .iter()
            .map(|row| row.trim_matches(['│', '┌', '┐', '└', '┘', '─']))
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// **`screens/context.md` § The picker and § Opening at startup, every row of the box** — the four
/// rows, their name, tag and badge columns, the server line of `(current)` with the cursor on it,
/// the two-line sentence and the blank rows between, at the page's width. The buttons are compared
/// by their words, because their centring moves with the width (`the_delete_boxes_…`'s
/// exception). **The footer and the command log line are the page's rows, byte for byte**, so
/// neither [`views::GET_CONTEXTS`] nor the footer can drift from the page without this reddening.
#[test]
fn the_picker_is_the_screen_files_box_both_ways() {
    let four = contexts_of(FOUR);
    for (connection, section) in [
        (live(), "## The picker"),
        (views::Connection::Never, "## Opening at startup"),
    ] {
        let page = &fenced("context.md", section)[0];
        let mockup = nested(page);
        let (drawn, frame) = picked(&picking(&four, connection), &four);
        assert_eq!(drawn.len(), mockup.len(), "{section}: the box's height");
        for (n, row) in mockup.iter().enumerate() {
            let row = on_the_page(row);
            if row.contains("[ ⏎") {
                assert_eq!(
                    words_of(&drawn[n]),
                    words_of(&row),
                    "{section}: the buttons"
                );
            } else {
                assert_eq!(
                    at_page_width(&drawn[n], (2..6).contains(&n)),
                    row,
                    "{section}, row {n}"
                );
            }
        }
        let log = page
            .iter()
            .find(|row| row.starts_with("│ $ "))
            .expect("the page draws a command log");
        assert_eq!(unframed(log), PAGE_LOG, "{section}: the page's command log");
        assert_eq!(
            unframed(&frame[20]),
            unframed(log),
            "{section}: the command log line"
        );
        assert_eq!(
            unframed(&frame[22]),
            unframed(&page[page.len() - 2]),
            "{section}: the footer"
        );
        assert_eq!(
            words_of(&frame[0]),
            words_of(&page[0]),
            "{section}: the header"
        );
    }
}

/// **At startup the header is `choose a cluster` and nothing is drawn behind the box**
/// (`screens/context.md` § Opening at startup) — no vitals, no sidebar, no divider, whatever the
/// caller handed over — **and the failure that picker leads to is drawn over nothing too**, on a
/// drawn frame. **`X`'s picker clears the body behind it as well** (NOTES § D264 ruling 9) and
/// keeps the running app's header; the failure a live switch leads to is a box over the app, the
/// way every other dialog is.
#[test]
fn a_picker_is_drawn_over_an_empty_body_and_only_a_startup_one_over_no_header() {
    let four = contexts_of(FOUR);
    let empty = |frame: &[String], what: &str| {
        assert!(
            !frame[1].contains('┬') && !frame[18].contains('┴'),
            "{what}: a sidebar divider was drawn behind the box"
        );
        for row in &frame[2..18] {
            assert!(
                !row.contains("ALERTS") && !row.contains("payments/web"),
                "{what}: the app showed through behind the box: {row:?}"
            );
        }
    };

    let (_, starting) = picked(&picking(&four, views::Connection::Never), &four);
    let header = format!("{:38}k8rs{:>38}", "", "choose a cluster · admin");
    assert_eq!(starting[0], header, "the startup header");
    empty(&starting, "the startup picker");

    let failed = |before| views::Modal::Unconnected {
        to: Some("staging".to_owned()),
        before,
        sent: true,
        fault: Fault::Refused,
        said: None,
        coverage: Coverage::Cluster,
        renewal: None,
    };
    // **Vitals handed over, and the startup failure draws none of them**, under the page's header.
    let (_, after_startup) = failed_under(
        failed(views::Before::Picking(views::Picker::new(
            &four,
            views::Connection::Never,
        ))),
        &page_header(),
        "nodes 3/3",
    );
    empty(&after_startup, "the startup failure");
    assert!(
        !after_startup[0].contains("nodes") && after_startup[0].ends_with(&page_header()),
        "the startup failure drew vitals, or not the context the caller named: {:?}",
        after_startup[0]
    );

    for connection in [
        live(),
        views::Connection::Dropped(Some("prod-eu".to_owned())),
    ] {
        let (_, switching) = picked(&picking(&four, connection.clone()), &four);
        empty(&switching, "X's picker");
        assert!(
            switching[0].starts_with(" nodes 3/3")
                && switching[0].ends_with("ctx: prod-eu · live · admin"),
            "{connection:?}: {:?}",
            switching[0]
        );
    }
    let (_, dismissing) = failure_box(failed(views::Before::Connected(Some("prod-eu".to_owned()))));
    assert!(
        dismissing[1].contains('┬') && dismissing[2].contains("ALERTS"),
        "the failure of a live switch hid the app it is a box over"
    );

    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut screen = pick_over(&alerts, &now, &[], &four);
    // **No context has been read, so no TLS warning either** — a flag the caller left set is not
    // drawn after `choose a cluster` (`ui::header`'s doc, NOTES § D265 ruling 2).
    screen.insecure = true;
    for writes in [
        Writes::ReadOnly,
        Writes::Unaudited("the audit log could not be opened"),
    ] {
        screen.writes = writes;
        let drawn = rows(&render(&picking(&four, views::Connection::Never), &screen));
        assert!(
            drawn[0].ends_with("choose a cluster · read-only"),
            "{:?}",
            drawn[0]
        );
    }
}

/// A kubeconfig of one current context and the rows under test, so no row below is the one the
/// cursor opens on and each draws exactly as the page's excerpt does.
fn beside(contexts: &str, clusters: &str) -> String {
    format!(
        "apiVersion: v1\n\
         kind: Config\n\
         current-context: anchor\n\
         clusters:\n\
         - {{name: anchor, cluster: {{server: 'https://anchor.example:6443'}}}}\n\
         {clusters}\
         contexts:\n\
         - {{name: anchor, context: {{cluster: anchor, user: u}}}}\n\
         {contexts}\
         users: [{{name: u, user: {{token: k8rs-tests-fake-static-token}}}}]\n"
    )
}

/// **One excerpt row against one drawn row**, at the page's width, character for character.
fn against_excerpt(excerpt: &str, drawn: &str) {
    assert_eq!(at_page_width(drawn, true), excerpt);
}

/// **A 60-character tag clips at the slot's edge, and a right-to-left override is stripped before
/// the tag is a `Span`** (`screens/context.md` § A 60-character tag, § A tag holding a control
/// character) — neither moves the badge column, and the override never reaches a cell.
#[test]
fn a_long_tag_clips_at_twelve_and_an_override_never_reaches_the_screen() {
    let yaml = beside(
        "- {name: aws-prod, context: {cluster: anchor, user: u, extensions: [{name: k8rs, \
           extension: {tag: 'production-us-east-1-payments-platform-blue-green-canary-01x'}}]}}\n\
         - {name: aws-staging, context: {cluster: anchor, user: u, extensions: [{name: k8rs, \
           extension: {tag: \"prod\\u202Ereversed\"}}]}}\n",
        "",
    );
    let rows = contexts_of(&yaml);
    let long = match &rows[1].tag {
        Tag::Written(tag) => tag.clone(),
        other => panic!("the long tag was not read as written: {other:?}"),
    };
    assert_eq!(
        width(&long),
        60,
        "the fixture's tag is not the 60 columns the page names"
    );
    let (box_, _) = picked(&picking(&rows, live()), &rows);
    let blocks = fenced("context.md", "## The tag column");
    against_excerpt(&blocks[1][1], &box_[3]);
    against_excerpt(&blocks[2][1], &box_[4]);
    assert!(
        !box_.iter().any(|row| row.contains('\u{202e}')),
        "the override reached a cell"
    );
}

/// **The EKS fleet `aws eks update-kubeconfig` writes is told apart by its tail**
/// (`screens/context.md` § Three contexts told apart only by their tail, NOTES § D264 ruling 5) —
/// the page's own 80×24 mockup: each name gives way from its front in its slot, the server line's
/// label does too, and every row earns `~aws` off the Amazon host the page draws.
///
/// **The whole frame is the page's, byte for byte, from the top border down** (ruling 21): the box,
/// the blank row either side of it, both log rows — the page's one command and the blank under it —
/// and the footer. **The header is byte for byte through `k8rs`, and its right zone as text**:
/// this mockup ends that zone two columns short of its frame, as most in `screens/` do, and
/// `screens/widgets.md` § 1a's right alignment puts it at the edge.
///
/// **One substitution, width kept**, as [`on_the_page`] makes for [`FOUR`]: the host is written
/// with no scheme — `scripts/security-guard.py` refuses a reachable `https://` host in `src/`, and
/// a schemeless one is how [`FOUR`] earns its `~aws` too — so the page's `https://` becomes blanks
/// at the end of the address.
#[test]
fn a_fleet_of_arn_named_contexts_is_told_apart_by_the_tail_of_each_name() {
    const HOST: &str = "B4E2.eu-west-1.eks.amazonaws.com";
    let page: Vec<String> = fenced("context.md", "## The tag column")[3]
        .iter()
        .map(|row| {
            row.replacen("https://", "", 1)
                .replacen(HOST, &format!("{HOST}        "), 1)
        })
        .collect();
    let fleet: String = ["prod", "staging", "dev"]
        .iter()
        .map(|env| {
            format!(
                "- {{name: 'arn:aws:eks:eu-west-1:111122223333:cluster/payments-{env}', \
                 context: {{cluster: eks, user: u}}}}\n"
            )
        })
        .collect();
    let rows = contexts_of(&format!(
        "apiVersion: v1\n\
         kind: Config\n\
         clusters:\n\
         - {{name: eks, cluster: {{server: '{HOST}'}}}}\n\
         contexts:\n\
         {fleet}\
         users: [{{name: u, user: {{token: k8rs-tests-fake-static-token}}}}]\n"
    ));
    assert!(
        rows.len() == 3
            && rows
                .iter()
                .all(|row| row.tag == Tag::Derived("aws") && !row.current),
        "the fleet does not earn ~aws off its host"
    );
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = [Stripped::of(views::GET_CONTEXTS)];
    let frame = rows_of(&render(
        &moved(&rows, live(), 1, ""),
        &pick_over(&alerts, &now, &log, &rows),
    ));
    assert_eq!(frame.len(), page.len(), "the frame's height");
    let through = page[0].find(NAME).expect("the page's header draws k8rs") + NAME.len();
    assert_eq!(
        frame[0][..through],
        page[0][..through],
        "the header, through k8rs"
    );
    assert_eq!(
        frame[0][through..].trim(),
        page[0][through..].trim(),
        "the header's right zone"
    );
    for (n, (drawn, row)) in frame.iter().zip(&page).enumerate().skip(1) {
        assert_eq!(drawn, row, "row {n} from the top");
    }
}

/// **The unhappy rows § Unhappy states draws, each against its excerpt**: an undefined cluster, a
/// name that strips to nothing on the current row, a duplicate, and an address k8rs will not draw
/// — plus the two sentences that stand in for an address, word for word and hung under their
/// text. The duplicate's sentence wraps a word earlier at the floor than on the page's narrower
/// box, which is why words and not rows.
#[test]
fn every_unhappy_row_is_the_excerpt_the_screen_file_draws() {
    let yaml = beside(
        "- {name: old-cluster, context: {cluster: nowhere, user: u}}\n\
         - {name: \"\\u200B\", context: {cluster: anchor, user: u}}\n\
         - {name: prod-eu, context: {cluster: anchor, user: u}}\n\
         - {name: prod-eu, context: {cluster: anchor, user: u}}\n\
         - {name: weird-proxy, context: {cluster: weird, user: u}}\n",
        "- {name: weird, cluster: {server: '//admin:aGVsbG8/d29ybGQ=@APISERVER:6443', \
         insecure-skip-tls-verify: true}}\n",
    )
    .replace("current-context: anchor", "current-context: \"\\u200B\"");
    let rows = contexts_of(&yaml);
    assert_eq!(
        rows[1].server,
        Address::Undefined,
        "the fixture's undefined row"
    );
    assert!(
        rows[2].name.is_none() && rows[2].current,
        "the fixture's unnamed row"
    );
    assert!(
        rows[4].shadowed && !rows[3].shadowed,
        "the fixture's duplicate"
    );
    assert!(
        rows[5].server == Address::Unreadable && rows[5].insecure,
        "the fixture's unreadable address"
    );
    let blocks = fenced("context.md", "## Unhappy states");

    // The cursor opens on the unnamed row; the excerpts draw none of these selected, so it is moved
    // up onto the anchor. Thirty rows, so all six contexts are on one box.
    let mut app = picking(&rows, live());
    if let Some(views::Modal::ContextPick(picker)) = &mut app.modal {
        picker.up(&rows);
    }
    let (box_, _) = picked_at(30, &app, &rows);
    against_excerpt(&blocks[0][1], &box_[3]);
    against_excerpt(&blocks[1][1], &box_[4]);
    against_excerpt(&blocks[2][1], &box_[6]);
    against_excerpt(&blocks[4][1], &box_[7]);

    for (down, sentence) in [(2, &blocks[3]), (3, &blocks[5])] {
        let (drawn, _) = picked_at(30, &moved(&rows, live(), down, ""), &rows);
        let at = drawn
            .iter()
            .position(|row| row.contains("  —  "))
            .unwrap_or_else(|| panic!("no sentence under the list:\n{}", drawn.join("\n")));
        let under: Vec<&String> = drawn[at..]
            .iter()
            .take_while(|row| !words_of(row).is_empty())
            .collect();
        assert_eq!(
            words(
                &under
                    .iter()
                    .map(|row| row.trim_matches('│'))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            words(&sentence.join(" ")),
            "the sentence that stands in for an address"
        );
        let hang =
            drawn[at].chars().position(|c| c == '—').expect("the joint") + "—  ".chars().count();
        for row in &under[1..] {
            let characters: Vec<char> = row.chars().collect();
            assert!(
                characters[1..hang].iter().all(|c| *c == ' ') && characters[hang] != ' ',
                "a continuation is not hung under the sentence: {row:?}"
            );
        }
    }
}

/// **Two contexts, and neither names a cluster the file defines** — `screens/context.md` § No row
/// is both current and landable's own `[7]` rows.
const UNDEFINED: &str = "- {name: dev-cluster, context: {cluster: dev, user: u}}\n\
     - {name: old-cluster, context: {cluster: old, user: u}}\n";

/// [`UNDEFINED`] alone, `current-context` naming one of them, so no row can be landed on at all.
fn undefined_only() -> Vec<Choice> {
    contexts_of(
        &beside(UNDEFINED, "")
            .replace("current-context: anchor", "current-context: dev-cluster")
            .replace(
                "- {name: anchor, context: {cluster: anchor, user: u}}\n",
                "",
            ),
    )
}

/// **`⏎` is drawn dim, dropped from the footer, and explained in the slot under the list wherever
/// it would do nothing** (NOTES § D264 rulings 2, 16 and 18), each state against the page's own
/// excerpt in § Unhappy states, byte for byte at its width: no row selected with one to find
/// (`[6]`, `current-context` naming a context the file no longer has); no row that can be (`[7]`),
/// both ways the page names — every context undefined, and a filter that shows only undefined
/// rows; a filter that hides every row (`[8]`); and a kubeconfig with no contexts (`[9]`). From
/// `[7]` on, `↑↓ move` leaves the footer too, and **where `/` holds the text that emptied the list,
/// `esc` reads `clear filter`** (ruling 27). Each draws its own sentence and none of the other
/// three. Everywhere else the button is the live one.
///
/// **Drawn thirty rows tall**: at the floor a two-row slot leaves a four-context list three rows
/// and a scrollbar ([`a_long_list_scrolls_under_a_scrollbar_and_the_box_grows_with_the_terminal`]),
/// and the page's `[6]` draws the fourth.
#[test]
fn enter_is_dim_and_unoffered_and_the_slot_says_why_wherever_it_does_nothing() {
    let blocks = fenced("context.md", "## Unhappy states");
    let four = contexts_of(FOUR);
    let dangling = contexts_of(&FOUR.replace("current-context: prod-eu", "current-context: gone"));
    let undefined = undefined_only();
    assert!(
        undefined.len() == 2 && undefined.iter().all(|row| row.server == Address::Undefined),
        "the fixture's undefined rows"
    );
    let filtered = contexts_of(&beside(UNDEFINED, ""));
    let none: Vec<Choice> = Vec::new();
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    for (what, app, rows, excerpt, footer) in [
        (
            "no row selected",
            picking(&dangling, live()),
            &dangling,
            Some(6),
            "↑↓ move  type to filter  esc cancel",
        ),
        (
            "every row undefined",
            picking(&undefined, live()),
            &undefined,
            Some(7),
            "type to filter  esc cancel",
        ),
        (
            "a filter that shows only undefined rows",
            moved(&filtered, live(), 0, "-cluster"),
            &filtered,
            Some(7),
            "type to filter  esc clear filter",
        ),
        (
            "a filter that hides every row",
            moved(&four, live(), 0, "prod-uk"),
            &four,
            Some(8),
            "type to filter  esc clear filter",
        ),
        (
            "a kubeconfig with no contexts",
            picking(&none, live()),
            &none,
            Some(9),
            "type to filter  esc cancel",
        ),
        (
            "the live row",
            picking(&four, live()),
            &four,
            None,
            "↑↓ move  type to filter  ⏎ switch  esc cancel",
        ),
    ] {
        let drawn = render_at(MIN_WIDTH, 30, &app, &pick_over(&alerts, &now, &[], rows));
        let frame = rows_of(&drawn);
        let box_ = nested(&frame);
        let button = frame
            .iter()
            .position(|row| row.contains("[ ⏎ switch ]"))
            .unwrap_or_else(|| panic!("{what}: no ⏎ button:\n{}", frame.join("\n")));
        let x = frame[button]
            .chars()
            .position(|c| c == '[')
            .expect("the button");
        let cell = drawn.cell((x as u16, button as u16)).expect("a cell");
        assert_eq!(
            unframed(&frame[frame.len() - 2]),
            footer,
            "{what}: the footer"
        );
        let Some(excerpt) = excerpt else {
            assert_eq!(
                cell.fg,
                ink(theme::TEXT, Depth::TrueColor),
                "{what}: the button"
            );
            assert!(
                cell.modifier.contains(Modifier::REVERSED),
                "{what}: the button"
            );
            continue;
        };
        assert_eq!(
            cell.fg,
            ink(theme::DIM, Depth::TrueColor),
            "{what}: the button"
        );
        assert!(
            !box_.iter().any(|row| row.contains(MARKER.trim_end())),
            "{what}: a row was marked selected"
        );

        // **Rows above the excerpt's last blank are contexts, found by name, and rows below it are
        // the slot's sentence, found by its first line** — the shape all three excerpts draw.
        let inner = &blocks[excerpt][1..blocks[excerpt].len() - 1];
        let blank = inner
            .iter()
            .rposition(|row| words_of(row).is_empty())
            .expect("the excerpt separates the slot with a blank row");
        for row in inner[..blank]
            .iter()
            .filter(|row| !words_of(row).is_empty())
        {
            let name = words_of(row).remove(0);
            let at = box_
                .iter()
                .position(|drawn| words_of(drawn).first() == Some(&name))
                .unwrap_or_else(|| panic!("{what}: {name} is not drawn:\n{}", box_.join("\n")));
            assert_eq!(at_page_width(&box_[at], true), *row, "{what}: {name}'s row");
        }
        let sentence = &inner[blank + 1..];
        let first = sentence[0].trim_matches('│').trim();
        let at = box_
            .iter()
            .position(|drawn| drawn.contains(first))
            .unwrap_or_else(|| {
                panic!(
                    "{what}: the slot does not say {first:?}:\n{}",
                    box_.join("\n")
                )
            });
        assert!(
            words_of(&box_[at - 1]).is_empty(),
            "{what}: no blank row above the slot"
        );
        for (n, row) in sentence.iter().enumerate() {
            assert_eq!(
                at_page_width(&box_[at + n], false),
                *row,
                "{what}: the slot, row {n}"
            );
        }
        for other in [6, 7, 8, 9].into_iter().filter(|&nth| nth != excerpt) {
            let last = blocks[other][blocks[other].len() - 2]
                .trim_matches('│')
                .trim();
            assert!(
                !inside(&box_).contains(last),
                "{what}: `[{other}]`'s {last:?} was drawn too"
            );
        }
    }

    // **A filter over a picker with no row selected is still *pick one*, not *nothing matches***:
    // the rows it shows are right there, and only a filter that hides them all is about the filter.
    let (box_, _) = picked(&moved(&dangling, live(), 0, "prod"), &dangling);
    let said = inside(&box_);
    assert!(
        said.contains("prod-eu")
            && said.contains(&UNSTARTED.join(" "))
            && !said.contains("No context matches"),
        "{}",
        box_.join("\n")
    );
    // **And a filter over a kubeconfig with no contexts is still about the file**: there was
    // nothing for it to hide.
    let said = inside(&picked(&moved(&none, live(), 0, "prod"), &none).0);
    assert!(
        said.contains(&EMPTIED.join(" ")) && !said.contains("No context matches"),
        "{said}"
    );

    let (box_, frame) = picked(&moved(&four, live(), 0, "prod-uk"), &four);
    assert!(
        !box_
            .iter()
            .any(|row| ["prod-eu", "staging", "kind-k8rs", "dev-cluster"]
                .iter()
                .any(|name| row.contains(name))),
        "a filter that hides every row still drew one:\n{}",
        frame.join("\n")
    );
    let shadowed = contexts_of(&FOUR.replace("name: staging,", "name: prod-eu,"));
    assert!(shadowed[1].shadowed);
    let (box_, frame) = picked(&moved(&shadowed, live(), 1, ""), &shadowed);
    assert_eq!(unframed(&frame[22]), "↑↓ move  type to filter  esc cancel");
    assert!(
        inside(&box_).contains("Every lookup by that name, ⏎ here included, finds it first"),
        "{}",
        box_.join("\n")
    );
}

/// **The box's `esc` button and the footer name `esc` with one word, in every state `/` can be
/// typed into** (NOTES § D264 ruling 31), on all three connections: `clear filter` on both while
/// `/` holds text — over a live row, no row selected, a shadowed row, only undefined rows, no row
/// shown and a kubeconfig with no contexts — and `quit` at startup or `cancel` otherwise when it
/// holds none. **The pair stays one centred run four columns apart** whichever word it carries,
/// six columns wider with `clear filter`.
#[test]
fn the_esc_button_and_the_footer_say_one_word_in_every_state_a_filter_can_be_typed_in() {
    let four = contexts_of(FOUR);
    let dangling = contexts_of(&FOUR.replace("current-context: prod-eu", "current-context: gone"));
    let shadowed = contexts_of(&FOUR.replace("name: staging,", "name: prod-eu,"));
    let undefined = undefined_only();
    let filtered = contexts_of(&beside(UNDEFINED, ""));
    let none: Vec<Choice> = Vec::new();
    for (what, rows, down, filter) in [
        ("the live row", &four, 0, "prod"),
        ("no row selected", &dangling, 0, "prod"),
        ("a shadowed row", &shadowed, 1, "prod"),
        ("every row undefined", &undefined, 0, "cluster"),
        (
            "a filter that shows only undefined rows",
            &filtered,
            0,
            "-cluster",
        ),
        ("a filter that hides every row", &four, 0, "prod-uk"),
        ("a kubeconfig with no contexts", &none, 0, "prod"),
    ] {
        for connection in [
            live(),
            views::Connection::Dropped(Some("prod-eu".to_owned())),
            views::Connection::Never,
        ] {
            for typed in ["", filter] {
                let label = format!("{what}, /{typed:?}, {connection:?}");
                let expected = match (typed.is_empty(), &connection) {
                    (false, _) => "clear filter",
                    (true, views::Connection::Never) => "quit",
                    (true, _) => "cancel",
                };
                let (box_, frame) = picked(&moved(rows, connection.clone(), down, typed), rows);
                let footer = unframed(&frame[usize::from(MIN_HEIGHT) - 2]);
                let (_, said) = footer
                    .rsplit_once("esc ")
                    .unwrap_or_else(|| panic!("{label}: no esc in {footer:?}"));
                assert_eq!(said, expected, "{label}: the footer");

                let row = box_
                    .iter()
                    .find(|row| row.contains("[ ⏎ "))
                    .unwrap_or_else(|| panic!("{label}: no buttons:\n{}", box_.join("\n")));
                let inner = row.trim_matches('│');
                let (start, end) = (
                    inner.find('[').expect("a button"),
                    inner.rfind(']').expect("a button") + 1,
                );
                let (enter, leave) = inner[start..end]
                    .split_once("    ")
                    .unwrap_or_else(|| panic!("{label}: no four-column gap in {row:?}"));
                assert!(
                    enter.starts_with("[ ⏎ ") && enter.ends_with(" ]"),
                    "{label}: {row:?}"
                );
                assert_eq!(
                    leave
                        .strip_prefix("[ esc ")
                        .and_then(|leave| leave.strip_suffix(" ]")),
                    Some(said),
                    "{label}: the button and the footer spell esc two ways in {row:?}"
                );
                let (left, right) = (inner[..start].chars().count(), inner[end..].chars().count());
                assert!(
                    left.abs_diff(right) <= 1,
                    "{label}: the pair is not centred in {row:?}"
                );
                if what == "a filter that hides every row"
                    && !typed.is_empty()
                    && connection == live()
                {
                    println!("{}", frame.join("\n"));
                }
            }
        }
    }
}

/// The rows of a drawn buffer, for a test that also reads its cells.
fn rows_of(buffer: &Buffer) -> Vec<String> {
    rows(buffer)
}

/// **The styles the page says carry a fact**: a written tag bright with no marker, a derived one
/// dim behind `~`, and a row the cursor skips dim throughout — while the unnamed row is drawn like
/// any other (§ A context whose name strips to nothing: *not dimmed*).
#[test]
fn a_guess_is_dim_behind_a_tilde_and_a_statement_is_bright_with_none() {
    let four = contexts_of(FOUR);
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let drawn = render(
        &picking(&four, live()),
        &pick_over(&alerts, &now, &[], &four),
    );
    let fg = |needle: &str| {
        let y = rows(&drawn)
            .iter()
            .position(|row| row.contains(needle))
            .unwrap_or_else(|| panic!("{needle:?} is not drawn"));
        let x = rows(&drawn)[y]
            .chars()
            .collect::<Vec<_>>()
            .windows(needle.chars().count())
            .position(|w| w.iter().collect::<String>() == needle)
            .expect("found above");
        drawn.cell((x as u16, y as u16)).expect("a cell").fg
    };
    assert_eq!(fg("aws · prod"), ink(theme::TEXT, Depth::TrueColor));
    assert_eq!(fg("~local"), ink(theme::DIM, Depth::TrueColor));
    assert_eq!(fg("~aws"), ink(theme::DIM, Depth::TrueColor));
    assert!(
        !holds(&drawn, "~aws · prod"),
        "a written tag was marked as a guess"
    );
    assert_eq!(fg("staging"), ink(theme::TEXT, Depth::TrueColor));

    let yaml = beside(
        "- {name: old-cluster, context: {cluster: nowhere, user: u}}\n\
         - {name: \"\\u200B\", context: {cluster: anchor, user: u}}\n\
         - {name: anchor, context: {cluster: anchor, user: u}}\n",
        "",
    );
    let rows_ = contexts_of(&yaml);
    let drawn = render_at(
        MIN_WIDTH,
        30,
        &picking(&rows_, live()),
        &pick_over(&alerts, &now, &[], &rows_),
    );
    let fg = |needle: &str, nth: usize| {
        let (y, x) = rows(&drawn)
            .iter()
            .enumerate()
            .filter_map(|(y, row)| {
                row.chars()
                    .collect::<Vec<_>>()
                    .windows(needle.chars().count())
                    .position(|w| w.iter().collect::<String>() == needle)
                    .map(|x| (y, x))
            })
            .nth(nth)
            .unwrap_or_else(|| panic!("{needle:?} is not drawn {nth} times"));
        drawn.cell((x as u16, y as u16)).expect("a cell").fg
    };
    assert_eq!(fg("old-cluster", 0), ink(theme::DIM, Depth::TrueColor));
    assert_eq!(fg(views::UNNAMED, 0), ink(theme::TEXT, Depth::TrueColor));
    assert_eq!(
        fg("anchor", 1),
        ink(theme::DIM, Depth::TrueColor),
        "the duplicate"
    );
    assert_eq!(
        fg("anchor", 0),
        ink(theme::TEXT, Depth::TrueColor),
        "the entry it shadows"
    );
}

/// **`⚠ TLS not verified` beats `(current)`** (§ The badge tie-break) — and only on a row that is
/// both: the current row alone still says `(current)`.
#[test]
fn a_tls_warning_takes_the_badge_from_current_and_nothing_else_does() {
    let insecure = FOUR.replace("current-context: prod-eu", "current-context: dev-cluster");
    for (yaml, current, badge, not) in [
        (
            insecure.as_str(),
            "dev-cluster",
            "⚠ TLS not verified",
            "(current)",
        ),
        (FOUR, "prod-eu", "(current)", "⚠ TLS not verified"),
    ] {
        let rows = contexts_of(yaml);
        let (box_, _) = picked(&picking(&rows, live()), &rows);
        let row = box_
            .iter()
            .find(|row| row.contains(&format!("▸ {current}")))
            .unwrap_or_else(|| panic!("{current} is not the selected row:\n{}", box_.join("\n")));
        assert!(row.contains(badge), "{row:?}");
        assert!(!row.contains(not), "{row:?}");
    }
    let rows = contexts_of(&insecure);
    let (box_, _) = picked(&picking(&rows, live()), &rows);
    assert!(
        !box_.iter().any(|row| row.contains("(current)")),
        "(current) was drawn beside the warning that took its slot"
    );
}

/// **One context and `X`: the picker opens and says why nothing can be picked** — the page's
/// sentence, in the place of the one about switching, which two contexts keep — **and a name too
/// long for its row gives way from its front there too** (NOTES § D264 rulings 5 and 11), keeping
/// the tail that tells one EKS cluster from the next.
#[test]
fn one_context_says_it_is_the_only_one_and_two_do_not() {
    let one = |name: &str| {
        contexts_of(&format!(
            "apiVersion: v1\n\
             kind: Config\n\
             current-context: '{name}'\n\
             clusters: [{{name: prod, cluster: {{server: 'https://prod-eu.example:6443'}}}}]\n\
             contexts: [{{name: '{name}', context: {{cluster: prod, user: u}}}}]\n\
             users: [{{name: u, user: {{token: k8rs-tests-fake-static-token}}}}]\n"
        ))
    };
    let prod = one("prod-eu");
    let (box_, _) = picked(&picking(&prod, live()), &prod);
    assert!(
        inside(&box_).contains("prod-eu is the only cluster in your kubeconfig."),
        "{}",
        box_.join("\n")
    );
    assert!(!box_.iter().any(|row| row.contains(UNCHANGED[0])));

    // **The page's own ARN excerpt, both rows byte for byte** (§ The picker, ruling 21): a name
    // narrower than the line is drawn whole and the sentence wraps, at the floor the page names.
    let excerpt = &fenced("context.md", "## The picker")[1];
    let arn = "arn:aws:eks:eu-west-1:111122223333:cluster/payments-prod";
    let rows = one(arn);
    let (box_, _) = picked(&picking(&rows, live()), &rows);
    let at = box_
        .iter()
        .position(|row| row == &excerpt[0])
        .unwrap_or_else(|| panic!("{:?} is not drawn:\n{}", excerpt[0], box_.join("\n")));
    assert_eq!(excerpt.len(), 2, "the page's excerpt");
    assert_eq!(box_[at + 1], excerpt[1], "the sentence's second row");

    let wider =
        "arn:aws:eks:eu-west-1:111122223333:cluster/payments-platform-production-blue-green";
    let rows = one(wider);
    let (box_, _) = picked(&picking(&rows, live()), &rows);
    let said: Vec<&String> = box_
        .iter()
        .filter(|row| row.contains("payments-platform") || row.contains("only cluster"))
        .filter(|row| !row.contains('→') && !row.contains(MARKER))
        .collect();
    assert_eq!(
        said.len(),
        2,
        "the sentence is not two rows:\n{}",
        box_.join("\n")
    );
    let sentence = words(
        &said
            .iter()
            .map(|row| row.trim_matches('│'))
            .collect::<Vec<_>>()
            .join(" "),
    );
    assert!(
        sentence.starts_with(CUT)
            && sentence.ends_with("-production-blue-green is the only cluster in your kubeconfig."),
        "{sentence:?}"
    );

    let four = contexts_of(FOUR);
    let (box_, _) = picked(&picking(&four, live()), &four);
    assert!(!box_.iter().any(|row| row.contains("the only cluster")));
    assert!(box_.iter().any(|row| row.contains(UNCHANGED[1])));
}

/// **More contexts than the box has rows: the list scrolls with the cursor, a scrollbar says so,
/// and the box grows with the terminal** (NOTES § D264 rulings 6 and 9). Measured at the floor and
/// at forty rows, each against the same rule: the box stops one row short of the body, so the
/// list is what the body has left after the box's fixed rows — never `MODAL_ROWS`, which only
/// agrees with that at 24.
#[test]
fn a_long_list_scrolls_under_a_scrollbar_and_the_box_grows_with_the_terminal() {
    let contexts: String = (0..30)
        .map(|n| format!("- {{name: ctx-{n:02}, context: {{cluster: anchor, user: u}}}}\n"))
        .collect();
    let rows = contexts_of(&beside(&contexts, ""));
    for height in [MIN_HEIGHT, 40] {
        for down in [0, 3, 17, 30, 40] {
            let (box_, frame) = picked_at(height, &moved(&rows, live(), down, ""), &rows);
            assert_eq!(frame.len(), usize::from(height));
            // The box's rows: its two borders, and the nine the list does not take — four blanks,
            // the one-row server line, the two-line sentence, the buttons and the blank under them.
            let fixed = 2 + 9;
            let listed = body_rows(&frame) - 1 - fixed;
            assert_eq!(
                box_.len(),
                body_rows(&frame) - 1,
                "{height} rows, ↓ {down}: the box is not one row short of the body:\n{}",
                frame.join("\n")
            );
            let list = &box_[2..2 + listed];
            let selected = if down == 0 {
                "anchor".to_owned()
            } else {
                format!("ctx-{:02}", (down - 1).min(29))
            };
            let at = list
                .iter()
                .position(|row| row.contains(&format!("▸ {selected} ")))
                .unwrap_or_else(|| {
                    panic!(
                        "{height}, ↓ {down}: {selected} is not drawn:\n{}",
                        box_.join("\n")
                    )
                });
            assert_eq!(at, down.min(listed - 1), "{height}, ↓ {down}: the window");

            // **The scrollbar is the list's right-hand column**: its thumb at the top of the track
            // while the window is at the top, and at the bottom once the cursor is on the last row.
            let edge: Vec<char> = list
                .iter()
                .map(|row| {
                    row.chars()
                        .rev()
                        .nth(1)
                        .expect("a column inside the border")
                })
                .collect();
            assert!(
                edge.iter().all(|c| ['█', '║'].contains(c)) && edge.contains(&'█'),
                "{height}, ↓ {down}: no scrollbar down the list: {edge:?}\n{}",
                box_.join("\n")
            );
            if down < listed {
                assert_eq!(
                    edge[0], '█',
                    "{height}, ↓ {down}: the thumb is not at the top"
                );
            }
            if down >= 30 {
                assert_eq!(
                    edge[listed - 1],
                    '█',
                    "{height}, ↓ {down}: the thumb is not at the foot"
                );
                assert_ne!(
                    edge[0], '█',
                    "{height}, ↓ {down}: the thumb still covers the top"
                );
            }
        }
    }

    // **The thumb is as long as the share of the list on screen**, to the nearest cell — twenty of
    // twenty-three rows is seventeen cells of a twenty-row track, with the window at the top and
    // at the foot alike.
    let contexts: String = (0..22)
        .map(|n| format!("- {{name: ctx-{n:02}, context: {{cluster: anchor, user: u}}}}\n"))
        .collect();
    let rows = contexts_of(&beside(&contexts, ""));
    for down in [0, 22] {
        let (box_, frame) = picked_at(40, &moved(&rows, live(), down, ""), &rows);
        let listed = body_rows(&frame) - 1 - (2 + 9);
        let thumb = box_[2..2 + listed]
            .iter()
            .filter(|row| row.chars().rev().nth(1) == Some('█'))
            .count();
        let share = (listed * listed) as f64 / rows.len() as f64;
        assert_eq!(listed, 20, "the fixture's arithmetic");
        assert_eq!(
            thumb,
            share.round() as usize,
            "↓ {down}: the thumb's length"
        );
    }

    // **A list that fits has no scrollbar** — `screens/widgets.md` § 2's *only* when the content is
    // taller than the viewport.
    let four = contexts_of(FOUR);
    for height in [MIN_HEIGHT, 40] {
        let (box_, _) = picked_at(height, &picking(&four, live()), &four);
        assert!(
            !box_
                .iter()
                .any(|row| row.contains('█') || row.contains('║')),
            "{height}: a scrollbar over a list that fits:\n{}",
            box_.join("\n")
        );
    }
}

/// **A context name and a server address at the ingest bound cannot size the picker**: the tag and
/// badge columns hold still, the name gives way from its front, the server line is cut at the rows
/// the longest sentence k8rs writes in its place takes, and **the sentence and the buttons are
/// still on the box** (`tester`, NOTES § D264).
#[test]
fn a_name_and_an_address_at_the_ingest_bound_move_no_column_and_push_nothing_off() {
    let huge = "n".repeat(10_000);
    let yaml = beside(
        &format!(
            "- {{name: {huge}, context: {{cluster: long, user: u, \
             extensions: [{{name: k8rs, extension: {{tag: t}}}}]}}}}\n"
        ),
        &format!(
            "- {{name: long, cluster: {{server: 'https://{}.example:6443'}}}}\n",
            "h".repeat(600)
        ),
    );
    let rows = contexts_of(&yaml);
    let address = match &rows[1].server {
        Address::Server(address) => address.clone(),
        other => panic!("the long address was not read: {other:?}"),
    };
    assert!(
        address.len() >= 512,
        "the fixture's address is {} bytes",
        address.len()
    );
    let (short_box, _) = picked(&picking(&rows, live()), &rows);
    let (box_, frame) = picked(&moved(&rows, live(), 1, ""), &rows);
    assert_eq!(frame.len(), 24);
    assert!(frame.iter().all(|row| row.chars().count() == 80));
    assert_eq!(
        box_.len(),
        short_box.len(),
        "the box changed height under the cursor"
    );
    assert!(box_.len() < body_rows(&frame), "the box outgrew the body");
    let long_row = box_
        .iter()
        .find(|row| row.contains(&format!("▸ {CUT}nnn")))
        .expect("the long row, cut from its front");
    assert_eq!(
        long_row.chars().nth(1 + 2 + 2 + 30 + GAP),
        Some('t'),
        "the tag moved: {long_row:?}"
    );
    // **The slot keeps the rows of the longest sentence k8rs itself writes there, and not one
    // more** — § A context defined twice's, behind the page's own `prod-eu`: drawn whole at the
    // floor in the rows the page gives it, and those are the rows a 512-byte address is cut at.
    // A slot one row taller costs the list that row for nothing, and one shorter cuts k8rs's own
    // sentence.
    let shadowed = contexts_of(&FOUR.replace("name: staging,", "name: prod-eu,"));
    let (duplicate, _) = picked(&moved(&shadowed, live(), 1, ""), &shadowed);
    let sentence: Vec<&String> = duplicate
        .iter()
        .skip_while(|row| !row.contains("  —  "))
        .take_while(|row| !words_of(row).is_empty())
        .collect();
    let page = fenced("context.md", "## Unhappy states")[3].len();
    assert!(
        sentence.len() == page && !sentence.iter().any(|row| row.contains(CUT)),
        "the duplicate's sentence is not the page's {page} rows, whole:\n{}",
        duplicate.join("\n")
    );
    let server = box_.iter().filter(|row| row.contains("hhh")).count();
    assert_eq!(
        server,
        sentence.len(),
        "the server line took {server} rows:\n{}",
        box_.join("\n")
    );
    for kept in [UNCHANGED[0], UNCHANGED[1], "[ ⏎ switch ]", "[ esc cancel ]"] {
        assert!(
            box_.iter().any(|row| row.contains(kept)),
            "{kept:?} was pushed off the box:\n{}",
            frame.join("\n")
        );
    }
}

/// **The failure box a picked context ends in**, as drawn over the Alerts screen at the floor.
fn unconnected_box(
    to: Option<&str>,
    before: views::Before,
    sent: bool,
    fault: Fault,
    coverage: Coverage,
) -> (Vec<String>, Vec<String>) {
    failure_box(views::Modal::Unconnected {
        to: to.map(str::to_owned),
        before,
        sent,
        fault,
        said: None,
        coverage,
        renewal: None,
    })
}

/// **[`failed_under`] the page's own header, with nothing in the vitals zone** — `ctx: staging · ⚠
/// not allowed · admin`, read off § When the new cluster does not work's `[0]`, and never the
/// helper [`screen`]'s `prod-eu · live`: the header names the context that was tried and not the
/// one live before it (§ Opening at startup), and nothing from the old cluster survives the switch
/// (§ What happens on `⏎`), so the page draws no vitals.
fn failure_box(modal: views::Modal) -> (Vec<String>, Vec<String>) {
    failed_under(modal, &page_header(), "")
}

/// **A failure box at the floor under `header` and `vitals`** — so a test can hand vitals over and
/// watch the startup frame drop them.
///
/// **`header` is the zone as a page draws it, `admin` and all, and the caller hands over only
/// what comes before that word**: [`header`] joins it on (NOTES § D265 ruling 1), so a page's zone
/// handed over whole would draw it twice.
///
/// **The link is [`Link::Unconnected`] in every one of these, which is the whole of what a caller
/// owes this frame** (todo.md § Phase 12): the zone handed over already ends in the *fault's* word,
/// and any other value of [`Link`] joins a second connection word behind it — measured, with
/// `Link::Live`, as `ctx: staging · ⚠ not allowed · live · admin` against this page's own row
/// (`tester`, 2026-09-19).
fn failed_under(modal: views::Modal, header: &str, vitals: &str) -> (Vec<String>, Vec<String>) {
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let now = now();
    let log = [
        Stripped::of("$ kubectl get daemonsets -A --watch"),
        Stripped::of(views::GET_CONTEXTS),
    ];
    let mut screen = pick_over(&alerts, &now, &log, &[]);
    screen.context = Stripped::of(header.strip_suffix(" · admin").unwrap_or_else(|| {
        panic!("{header:?} does not end on the word a live run's header joins")
    }));
    screen.vitals = Stripped::of(vitals);
    screen.link = Link::Unconnected;
    let app = App {
        modal: Some(modal),
        ..App::default()
    };
    let drawn = rows(&render(&app, &screen));
    (nested(&drawn), drawn)
}

/// The right zone of the header both *staging said no* mockups draw.
fn page_header() -> String {
    fenced("context.md", "## When the new cluster does not work")[0][0]
        .trim()
        .to_owned()
}

/// **A drawn header against the page's**, which is 70 columns wide: at 80 the centred [`NAME`]
/// fits between the zones and at 70 it does not (`screens/widgets.md` § 1a drops it first), so it
/// is blanked before the rows are compared, and nothing else is.
fn against_page_header(drawn: &str, page: &str) {
    assert_eq!(
        drawn
            .replacen(NAME, &" ".repeat(width(NAME)), 1)
            .trim_start(),
        page.trim_start(),
        "the header"
    );
}

/// The picker a failed first connection goes back to.
fn never(rows: &[Choice]) -> views::Before {
    views::Before::Picking(views::Picker::new(rows, views::Connection::Never))
}

/// **Both *staging said no* boxes are the screen file's, byte for byte** — `Refused` on a watch
/// cluster-wide, over a live switch and over the startup picker: every row of the box, its title
/// and both borders included, and the footer each draws (`screens/context.md` § When the new
/// cluster does not work, NOTES § D264 ruling 1) — **the button row too**, whose odd spare column
/// the page puts on the left, where `screens/dialogs.md`'s two boxes of the same width put it
/// (ruling 19). **The paragraph's next step is the running one** (ruling 23), six rows in the box,
/// and **the header is each mockup's own** ([`against_page_header`]).
#[test]
fn both_refusals_are_the_screen_files_boxes() {
    let four = contexts_of(FOUR);
    let blocks = fenced("context.md", "## When the new cluster does not work");
    for (block, before) in [
        (
            &blocks[0],
            views::Before::Connected(Some("prod-eu".to_owned())),
        ),
        (&blocks[2], never(&four)),
    ] {
        let (drawn, frame) = unconnected_box(
            Some("staging"),
            before,
            true,
            Fault::Refused,
            Coverage::Cluster,
        );
        let mockup = nested(block);
        assert_eq!(drawn.len(), mockup.len(), "the box's height");
        for (n, (drawn, row)) in drawn.iter().zip(&mockup).enumerate() {
            assert_eq!(drawn, row, "row {n}");
        }
        assert_eq!(
            unframed(&frame[22]),
            unframed(&block[block.len() - 2]),
            "the footer"
        );
        against_page_header(&frame[0], &block[0]);
    }
}

/// **The frame `esc` reveals, once a mid-session switch has failed** — `screens/context.md`
/// § *After `esc dismiss`, on a switch that failed with a cluster already live*, whose header and
/// footer are that page's own (todo.md § Phase 12).
///
/// **The claim is that dismissing the box changes nothing about the header**, so the two frames
/// are compared with each other as well as with the page: [`header`] read
/// `crate::views::Modal::Unconnected` until 2026-09-19, which made the fault's word end when the
/// box did — `ctx: staging · ⚠ not allowed · live · admin` the frame after `esc`, two connection
/// words over a cluster k8rs never reached
/// (`reports/2026-09-19-the-strip-and-the-connection-word.md` § M1). Asserted as *the same row*
/// and not as *a row 36 columns wide*: a geometry assertion passes whichever word is in it.
///
/// **`X switch cluster` is on the footer and `↑↓ move` / `⏎ open` are not** — the page's own line,
/// read off the page: nothing survived the switch to move a cursor across, and `X` is the only way
/// out ([`offered`] answers `Nothing { switch: true }`).
///
/// **The body is the caller's and is not asserted here**: its sentence comes in on
/// [`Screen::note`], which the turn that wires `main.rs` writes. What is fed is a note and a
/// command-log line so the frame is the shape the page draws, not a bare one.
///
/// **The mockup is fetched by its own `###` heading and not as the fourth block of the `##`
/// section it sits in** ([`fenced`] runs a section to the next `##`, so a `###` addresses exactly
/// its own fences): an index would have gone on passing against a *different* drawing the day
/// somebody adds a fence above it, and `mockup_held` had its heading renamed under it this same
/// afternoon.
#[test]
fn the_frame_behind_a_dismissed_failure_keeps_the_faults_word_and_the_way_back() {
    let blocks = fenced(
        "context.md",
        "### After `esc dismiss`, on a switch that failed with a cluster already live",
    );
    assert_eq!(blocks.len(), 1, "that section no longer draws one frame");
    let page = &blocks[0];
    let zone = page[0].trim();
    let alerts: Pane<Vec<Card>> = Pane::Loading;
    let now = now();
    // **Nothing survived the switch** (that page's § What happens on `⏎`, step 1): the store was
    // dropped the moment `⏎` was pressed, so the pane is `Loading` and the note is all the body
    // has.
    let note = [Stripped::of("⚠ Not connected to the cluster right now.")];
    let log = [Stripped::of(
        "$ kubectl --context staging get pods -A --watch   → not allowed",
    )];
    let mut screen = screen(&alerts, &now);
    screen.context = Stripped::of(zone.strip_suffix(" · admin").expect("the page's own word"));
    screen.vitals = Stripped::of("");
    screen.note = &note;
    screen.log = &log;
    screen.link = Link::Unconnected;

    let dismissed = rows(&render(&app(), &screen));
    let open = App {
        modal: Some(views::Modal::Unconnected {
            to: Some("staging".to_owned()),
            before: views::Before::Connected(Some("prod-eu".to_owned())),
            sent: true,
            fault: Fault::Refused,
            said: None,
            coverage: Coverage::Cluster,
            renewal: None,
        }),
        ..App::default()
    };
    let with_box = rows(&render(&open, &screen));
    println!("--- the page ---\n{}\n", page.join("\n"));
    println!("--- esc dismiss ---\n{}\n", dismissed.join("\n"));
    assert_eq!(
        with_box[0], dismissed[0],
        "the header changed when the box closed"
    );
    against_page_header(&dismissed[0], &page[0]);
    assert_eq!(
        unframed(&dismissed[22]),
        unframed(&page[page.len() - 2]),
        "the footer"
    );
    // **One connection word in the slot, and it is the fault's** — a header that joined `live`
    // behind it reads as a connected cluster that is also not allowed.
    for never in ["live", "connecting", "disconnected", "login expired"] {
        assert!(
            !dismissed[0].contains(never),
            "{never:?} was joined behind the fault's word: {:?}",
            dismissed[0]
        );
    }
}

/// **A namespace the reader named and one k8rs had to guess take the page's two next steps**, and
/// the same opening reason (`screens/context.md` § The scope changes the next step, not just a
/// number) — the contrast `Coverage::namespace()` alone collapsed. **Each is the page's quote with
/// nothing added**: the reason before it ends in the full stop `failed` writes, and what follows it
/// in the box is the way out, so a stop the driver's sentence does not have would be caught.
#[test]
fn a_named_namespace_and_a_guessed_one_take_the_page_s_two_next_steps() {
    let excerpt = &fenced("context.md", "## When the new cluster does not work")[1];
    let paragraphs: Vec<String> = excerpt
        .split(|row| row.ends_with(':') && !row.starts_with(' '))
        .skip(1)
        .map(|rows| words(&rows.join(" ")))
        .collect();
    assert_eq!(paragraphs.len(), 2, "{excerpt:?}");
    for (coverage, next, namespace) in [
        (
            Coverage::Asked("payments".to_owned()),
            &paragraphs[0],
            "payments",
        ),
        (
            Coverage::Blind("default".to_owned()),
            &paragraphs[1],
            "default",
        ),
    ] {
        let (drawn, _) = unconnected_box(
            Some("staging"),
            never(&[]),
            true,
            Fault::Refused,
            coverage.clone(),
        );
        let said = inside(&drawn);
        assert!(
            said.contains(&format!(
                "needs to `list` and `watch` pods in the namespace {namespace}. {next} Nothing has \
                 connected yet"
            )),
            "{coverage:?}:\n{}",
            drawn.join("\n")
        );
        assert!(!said.contains("across the whole cluster"), "{coverage:?}");
    }
}

/// **Every other fault is the driver's sentence under the title of its class** — *said no*,
/// *did not answer*, *could not be opened* — and the class is whether a request reached a cluster,
/// never the HTTP class (`screens/context.md` § Every other fault has its own sentence, its table)
/// **and never the path the fault came by**: the four that build no client are *could not be
/// opened* on a watch too (NOTES § D264 ruling 17). The
/// sentence is the moved functions' own output, because that is the page's claim about it; the
/// titles are the page's table, row by row. The way out is on every one of them.
#[test]
fn every_fault_is_the_driver_s_sentence_under_the_title_its_request_earns() {
    let four = contexts_of(FOUR);
    let sent = [
        (Fault::Refused, "said no"),
        (Fault::Expired, "said no"),
        (Fault::Rejected, "said no"),
        (Fault::Gone, "said no"),
        (Fault::Conflict, "said no"),
        (Fault::Unanswered, "did not answer"),
        (Fault::Unfinished, "did not answer"),
        (Fault::Kubeconfig, "could not be opened"),
        (Fault::NoContext, "could not be opened"),
        (Fault::BadEntry, "could not be opened"),
        (Fault::NoCredential, "could not be opened"),
    ];
    let unsent = [
        Fault::Kubeconfig,
        Fault::NoContext,
        Fault::BadEntry,
        Fault::NoCredential,
        Fault::Unanswered,
    ];
    let cases = sent
        .iter()
        .map(|&(fault, title)| (true, fault, title))
        .chain(
            unsent
                .iter()
                .map(|&fault| (false, fault, "could not be opened")),
        );
    for (went_out, fault, title) in cases {
        // **Asked as *reach this cluster* wherever the title is *could not be opened*** — the
        // page's table, row by row — **and the running next step after it only for ruling 32's
        // three, and only where a request went out** (NOTES § D264 rulings 23 and 32). Written from
        // the ruling's list and not read off `next_step`, which answers `Unanswered` unsent too.
        let reason = if title != "could not be opened" {
            let asked = format!(
                "{} {}",
                views::watching("pods"),
                views::scope(&Coverage::Cluster)
            );
            capitalised(&views::because(fault, &asked, None, None))
        } else {
            capitalised(&views::because(fault, views::REACH, None, None))
        };
        let has_next =
            went_out && matches!(fault, Fault::Refused | Fault::Unanswered | Fault::Gone);
        let paragraph = if has_next {
            let next = views::next_step(fault, &Coverage::Cluster, "pods", true)
                .expect("ruling 32 lists this fault");
            format!("{reason}. {next}")
        } else {
            format!("{reason}.")
        };
        for (before, way_out) in [
            (
                views::Before::Connected(Some("prod-eu".to_owned())),
                "Nothing is wrong with prod-eu — X takes you back.",
            ),
            (
                never(&four),
                "Nothing has connected yet — esc takes you back to the list to try a different \
                 cluster.",
            ),
        ] {
            let (drawn, _) =
                unconnected_box(Some("staging"), before, went_out, fault, Coverage::Cluster);
            let said = inside(&drawn);
            assert!(
                drawn[0].starts_with(&format!("┌ staging {title} ─")),
                "{fault:?}, sent {went_out}: {:?}",
                drawn[0]
            );
            // **The paragraph ends where the way out begins**, so a next step drawn where none is
            // owed is a red here and not a longer string that still contains the reason.
            assert!(
                said.contains(&words(&format!("{paragraph} {way_out}"))),
                "{fault:?}, sent {went_out}:\n{}",
                drawn.join("\n")
            );
            assert!(drawn.len() <= MODAL_ROWS + 2, "{fault:?}");
        }
    }

    // **Ruling 32's `Gone` in its own bytes, straight after its reason and straight before the way
    // out — and `NoCredential`, sent or not, with nothing between its reason and the way out.**
    for (went_out, fault, title, said) in [
        (
            true,
            Fault::Gone,
            "said no",
            "This server says there is no such thing when k8rs tries to `list` and `watch` pods \
             across the whole cluster. Check the server address this kubeconfig names — as \
             written, it does not lead to a Kubernetes API server Nothing has connected yet",
        ),
        (
            true,
            Fault::NoCredential,
            "could not be opened",
            "The program this kubeconfig logs in with gave k8rs nothing to sign in with. Nothing \
             has connected yet",
        ),
        (
            false,
            Fault::NoCredential,
            "could not be opened",
            "The program this kubeconfig logs in with gave k8rs nothing to sign in with. Nothing \
             has connected yet",
        ),
    ] {
        let (drawn, _) = unconnected_box(
            Some("staging"),
            never(&four),
            went_out,
            fault,
            Coverage::Cluster,
        );
        assert!(
            drawn[0].starts_with(&format!("┌ staging {title} ─")) && inside(&drawn).contains(said),
            "{fault:?}, sent {went_out}:\n{}",
            drawn.join("\n")
        );
    }

    // **The login program is named where the kubeconfig has one** — the `401` on an EKS login the
    // first draft could not name.
    let (drawn, _) = failure_box(views::Modal::Unconnected {
        to: Some("staging".to_owned()),
        before: never(&four),
        sent: true,
        fault: Fault::Expired,
        said: None,
        coverage: Coverage::Cluster,
        renewal: Some("aws-iam-authenticator".to_owned()),
    });
    assert!(inside(&drawn).contains("it comes from `aws-iam-authenticator`, so renew it there."));

    // **The cluster's own words are quoted, and bounded** — four kilobytes of a `Strict` rejection
    // cannot push the way out or the button off the box.
    let (drawn, frame) = failure_box(views::Modal::Unconnected {
        to: Some("staging".to_owned()),
        before: views::Before::Connected(Some("prod-eu".to_owned())),
        sent: true,
        fault: Fault::Rejected,
        said: Some("spec.containers[0].env: ".repeat(170)),
        coverage: Coverage::Cluster,
        renewal: None,
    });
    assert_eq!(drawn.len(), MODAL_ROWS + 2, "{}", frame.join("\n"));
    let said = inside(&drawn);
    assert!(said.contains("and said: spec.containers[0].env:") && said.contains(CUT));
    assert!(said.contains("X takes you back.") && said.contains("[ esc dismiss ]"));
}

/// **Two failed switches in a row, and the second box still names the context that was last
/// live** (NOTES § D264 ruling 15). Each failure is built the way the caller builds it, off
/// [`views::Picker::chosen`] over the picker `X` opens — the first over live `prod-eu`, the second
/// over nothing live — and each says *Nothing is wrong with prod-eu — X takes you back.* under
/// `esc dismiss`, never *Nothing has connected yet*. **A run that never connected says the second,
/// on both attempts**, under `esc back to the list`.
#[test]
fn a_second_failed_switch_in_a_row_still_names_the_context_that_was_last_live() {
    let four = contexts_of(FOUR);
    let fail = |connection: views::Connection, down: usize| {
        let mut picker = views::Picker::new(&four, connection);
        for _ in 0..down {
            picker.down(&four);
        }
        let views::Chosen::Connect { row, before } = picker.chosen(&four) else {
            panic!("⏎ did not connect");
        };
        let (drawn, frame) = unconnected_box(
            Some(views::drawn_name(row)),
            before.clone(),
            true,
            Fault::Refused,
            Coverage::Cluster,
        );
        (before, inside(&drawn), unframed(&frame[22]))
    };

    let mut connection = live();
    for (attempt, down) in [(1, 1), (2, 2)] {
        let (before, said, footer) = fail(connection, down);
        assert!(
            said.contains("Nothing is wrong with prod-eu — X takes you back.")
                && !said.contains("Nothing has connected yet"),
            "switch {attempt} in a row: {said}"
        );
        assert_eq!(footer, "esc dismiss", "switch {attempt} in a row");
        let views::Before::Connected(last) = before else {
            panic!("switch {attempt} in a row went back to a list nothing had connected behind");
        };
        connection = views::Connection::Dropped(last);
    }

    for attempt in [1, 2] {
        let (_, said, footer) = fail(views::Connection::Never, attempt);
        assert!(
            said.contains("Nothing has connected yet — esc takes you back to the list")
                && !said.contains("Nothing is wrong with"),
            "startup attempt {attempt}: {said}"
        );
        assert_eq!(footer, "esc back to the list", "startup attempt {attempt}");
    }
}

/// **A context name past the ingest bound costs the failure box nothing** (NOTES § D264 rulings 5
/// and 25) — the box stays inside its ceiling, the title keeps its outcome, and the way out keeps
/// *X takes you back* on its two rows.
///
/// **The name comes the way the driver's does**: through `k8s::contexts`, and into the box through
/// [`views::Picker::chosen`] on a second attempt, which is where both names in it are read (NOTES
/// § D29). **What is not claimed is which cluster it was**: `k8s::contexts` has already cut the
/// name at its tail, so the front cut keeps little but that mark, and ruling 25 accepts it.
#[test]
fn a_name_at_the_ingest_bound_keeps_the_title_s_outcome_and_the_box_s_way_out() {
    let huge = "arn:aws:eks:eu-west-1:123456789012:cluster/".repeat(12) + "payments-prod";
    let rows = contexts_of(&format!(
        "apiVersion: v1\n\
         kind: Config\n\
         current-context: '{huge}'\n\
         clusters: [{{name: eks, cluster: {{server: 'https://eks.example:6443'}}}}]\n\
         contexts: [{{name: '{huge}', context: {{cluster: eks, user: u}}}}]\n\
         users: [{{name: u, user: {{token: k8rs-tests-fake-static-token}}}}]\n"
    ));
    let name = rows[0].name.clone().expect("the name survives the strip");
    assert!(
        huge.len() > crate::k8s::IDENTIFIER && !name.contains("payments-prod"),
        "the fixture's name was not cut at the ingest bound: {name:?}"
    );
    // **`X` again after a switch to it failed**: nothing is live, and the context last live is it.
    let picker = views::Picker::new(&rows, views::Connection::Dropped(Some(name.clone())));
    let views::Chosen::Connect { row, before } = picker.chosen(&rows) else {
        panic!("⏎ did not connect");
    };
    for (sent, fault, outcome) in [
        (true, Fault::Refused, "said no"),
        (true, Fault::Unanswered, "did not answer"),
        (false, Fault::BadEntry, "could not be opened"),
    ] {
        let (drawn, frame) = unconnected_box(
            Some(views::drawn_name(row)),
            before.clone(),
            sent,
            fault,
            Coverage::Blind("default".to_owned()),
        );
        assert_eq!(frame.len(), 24);
        assert!(
            drawn.len() <= MODAL_ROWS + 2,
            "{fault:?}: {} rows",
            drawn.len()
        );
        let title = drawn[0].trim_end_matches(['─', '┐']).trim_end();
        assert!(
            title.starts_with(&format!("┌ {CUT}")) && title.ends_with(&format!(" {outcome}")),
            "{fault:?}: {title:?}"
        );
        let said = inside(&drawn);
        assert!(
            said.contains("Nothing is wrong with")
                && said.contains(" — X takes you back.")
                && drawn.iter().any(|row| row.contains("[ esc dismiss ]")),
            "{fault:?} lost its way out:\n{}",
            drawn.join("\n")
        );
        let way_out = drawn
            .iter()
            .filter(|row| row.contains("Nothing is wrong") || row.contains("X takes you back"))
            .count();
        assert!(way_out <= 2, "{fault:?}: the way out took {way_out} rows");
    }
}

/// **Every picker screen, printed whole** — `cargo test -- --nocapture` — the switcher, its startup
/// form, the states where `⏎` does nothing — four of them with no row to land on at all — a long
/// list, and the refusals a picked context can end in, a second failed switch in a row among them.
#[test]
fn every_picker_on_the_screen_it_belongs_to() {
    let four = contexts_of(FOUR);
    let dangling = contexts_of(&FOUR.replace("current-context: prod-eu", "current-context: gone"));
    let many: String = (0..9)
        .map(|n| format!("- {{name: ctx-{n}, context: {{cluster: anchor, user: u}}}}\n"))
        .collect();
    let many = contexts_of(&beside(&many, ""));
    let show = |title: &str, app: &App, rows: &[Choice]| {
        println!("--- {title} ---\n{}\n", picked(app, rows).1.join("\n"));
    };
    show("X", &picking(&four, live()), &four);
    show("X · on staging", &moved(&four, live(), 1, ""), &four);
    show("Startup", &picking(&four, views::Connection::Never), &four);
    show("No row selected", &picking(&dangling, live()), &dangling);
    show(
        "Filter hides every row",
        &moved(&four, live(), 0, "prod-uk"),
        &four,
    );
    let undefined = undefined_only();
    show(
        "Every row undefined",
        &picking(&undefined, live()),
        &undefined,
    );
    let filtered = contexts_of(&beside(UNDEFINED, ""));
    show(
        "Filter shows only undefined rows",
        &moved(&filtered, live(), 0, "-cluster"),
        &filtered,
    );
    show("No contexts in the kubeconfig", &picking(&[], live()), &[]);
    show("Ten contexts", &moved(&many, live(), 6, ""), &many);
    // **The failure boxes under the header the page draws for them** — the context that was tried,
    // never `prod-eu · live`. No page draws a header for a `404` or a login program's failure, so
    // those two name the context and the permission alone.
    let failure = |title: &str,
                   header: &str,
                   before: views::Before,
                   sent: bool,
                   fault: Fault,
                   renewal: Option<&str>| {
        let modal = views::Modal::Unconnected {
            to: Some(title.split(' ').next().expect("a name").to_owned()),
            before,
            sent,
            fault,
            said: None,
            coverage: Coverage::Cluster,
            renewal: renewal.map(str::to_owned),
        };
        println!(
            "--- {title} ---\n{}\n",
            failed_under(modal, header, "").1.join("\n")
        );
    };
    let refused = page_header();
    let live_before = || views::Before::Connected(Some("prod-eu".to_owned()));
    failure(
        "staging said no",
        &refused,
        live_before(),
        true,
        Fault::Refused,
        None,
    );
    failure(
        "staging said no · startup",
        &refused,
        never(&four),
        true,
        Fault::Refused,
        None,
    );
    let mut again = views::Picker::new(
        &four,
        views::Connection::Dropped(Some("prod-eu".to_owned())),
    );
    again.down(&four);
    again.down(&four);
    let views::Chosen::Connect { before, .. } = again.chosen(&four) else {
        panic!("⏎ on kind-k8rs did not connect");
    };
    failure(
        "kind-k8rs said no · the second failed switch in a row",
        &refused.replacen("staging", "kind-k8rs", 1),
        before,
        true,
        Fault::Refused,
        None,
    );
    failure(
        "staging said no · 404, the address leads to no API server",
        "ctx: staging · admin",
        live_before(),
        true,
        Fault::Gone,
        None,
    );
    failure(
        "staging could not be opened · the login program gave nothing",
        "ctx: staging · admin",
        never(&four),
        false,
        Fault::NoCredential,
        Some("aws"),
    );
}

// --- WHAT NOTES § D266'S SIX BLOCKERS DRAW ---

/// **Every row of `file`'s `section` fences that holds `needle`**, in the file's order — the one
/// line of an excerpt a surface is compared against, read off the page rather than typed here.
fn excerpt(file: &str, section: &str, needle: &str) -> Vec<String> {
    fenced(file, section)
        .into_iter()
        .flatten()
        .filter(|line| line.contains(needle))
        .collect()
}

/// A drawn or a mockup row as its words — so spacing the page draws at another width is not what
/// is compared.
fn spaced(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('│')
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// **A canary and its stable sibling are two objects on every surface that names one**
/// (`screens/widgets.md` § 7, the identity cut; NOTES § D266). Each surface is drawn at the 80×24
/// floor for both names and compared with the row its own screen file draws for it — so the two
/// frames differing is the file's claim and not only this test's — and a name that fits is drawn
/// whole with no mark.
///
/// **Six call sites, eight rows**: a `Confirm`'s title and `$` line for scale and restart, the
/// *Already gone* body, the in-flight footer, the Alerts card and the detail heading.
#[test]
fn a_canary_and_its_stable_sibling_are_two_objects_on_every_surface_that_names_one() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let plain = screen(&alerts, &now);
    let namespace = "team-alpha-payments-platform";
    let object = |name: &str| {
        views::Object::new(
            "deployment",
            Some(namespace.to_owned()),
            name.to_owned(),
            Some("8656c3ec-0f0e-4d0e-9f0b-2a1d3c4b5a69".to_owned()),
        )
    };
    let title = |box_: &[String]| {
        box_[0]
            .trim_start_matches('┌')
            .trim_end_matches('┐')
            .trim_end_matches('─')
            .trim()
            .to_owned()
    };
    let holding = |box_: &[String], needle: &str| {
        box_.iter()
            .find(|row| row.contains(needle))
            .map(|row| row.trim_matches('│').trim().to_owned())
            .unwrap_or_else(|| panic!("no {needle:?} row in\n{}", box_.join("\n")))
    };

    let titles = excerpt("dialogs.md", "## Scale — confirm with dry-run", " …");
    let commands = excerpt(
        "dialogs.md",
        "## Scale — confirm with dry-run",
        "deployment/…",
    );
    let gone = excerpt(
        "dialogs.md",
        "## The object went away while the dialog was open",
        "…-payments-platform/",
    );
    let footer = excerpt("dialogs.md", "## While the call is running", "changing …m/");
    assert_eq!(
        (titles.len(), commands.len(), gone.len(), footer.len()),
        (4, 4, 2, 1),
        "screens/dialogs.md stopped drawing the canary and the stable pair"
    );

    for (nth, name) in [
        "checkout-worker-service-canary",
        "checkout-worker-service-stable",
    ]
    .into_iter()
    .enumerate()
    {
        let scale = box_of(views::Modal::Confirm(views::Dialog {
            object: object(name),
            kubectl: format!(
                "kubectl --context prod-eu scale deployment/{name} --replicas=3 -n {namespace}"
            ),
            ..scaling()
        }));
        let restart = box_of(views::Modal::Confirm(views::Dialog {
            object: object(name),
            kubectl: format!(
                "kubectl --context prod-eu rollout restart deployment/{name} -n {namespace}"
            ),
            ..restarting()
        }));
        assert_eq!(title(&scale), titles[nth], "the scale title");
        assert_eq!(title(&restart), titles[2 + nth], "the restart title");
        assert_eq!(
            holding(&scale, "$ kubectl"),
            commands[nth],
            "the scale $ line"
        );
        assert_eq!(
            holding(&restart, "$ kubectl"),
            commands[2 + nth],
            "the restart $ line"
        );
        let went = box_of(views::Modal::Gone {
            object: object(name),
            recreated: false,
        });
        assert_eq!(
            holding(&went, "checkout-worker"),
            gone[nth].trim(),
            "the Already gone body"
        );
        // The page draws the canary's footer and says in words that the stable one differs only
        // in its own tail.
        assert_eq!(
            footer_of(&render(&changing(Some(namespace), name), &plain)),
            footer[0].replace("checkout-worker-service-canary", name),
            "the in-flight footer"
        );
    }

    // The card, at both ages the page measures, and the second pair it draws: one namespace too
    // long for the row and two objects in it.
    let cards = excerpt("alerts.md", "## How wide a card is, and how tall", "● …");
    assert_eq!(
        cards.len(),
        4,
        "screens/alerts.md stopped drawing its four identity rows"
    );
    for (nth, (space, name, stamp)) in [
        (namespace, "checkout-worker-service-canary", at(0)),
        (namespace, "checkout-worker-service-stable", at(0)),
        (
            "openshift-cluster-node-tuning-operator",
            "tuned",
            at(240 - 7200),
        ),
        (
            "openshift-cluster-node-tuning-operator",
            "node-tuning-operator",
            at(240 - 7200),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut card = oom();
        card.owner = id(ObjectKind::Deployment, Some(space), name);
        card.findings[0].timestamp = Some(stamp);
        let listed = Pane::Ready(vec![card]);
        let drawn = render(&app(), &screen(&listed, &now));
        // **[`unmarked`], because the page's four loose rows are excerpts and not a pane**: one
        // card on its own has no sibling to be selected over, so the file draws the columns and
        // not the cursor.
        assert_eq!(
            spaced(&unmarked(&row(&drawn, " ago"))),
            spaced(&cards[nth]),
            "the Alerts card, row {nth}"
        );
    }

    let headings = excerpt(
        "detail.md",
        "## The heading, when the name does not fit",
        "…",
    );
    assert_eq!(
        headings.len(),
        4,
        "screens/detail.md stopped drawing its two pairs"
    );
    for (nth, (space, name)) in [
        (namespace, "checkout-worker-service-canary-7d9f4bc86d-x2k9p"),
        (namespace, "checkout-worker-service-stable-7d9f4bc86d-x2k9p"),
        (
            "openshift-cluster-node-tuning-operator",
            "tuned-metrics-reader-canary",
        ),
        (
            "openshift-cluster-node-tuning-operator",
            "tuned-metrics-reader-stable",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut open = Open::new();
        open.object = id(ObjectKind::Pod, Some(space), name);
        let drawn = rows(&detailed(&on(Tab::Events), &open.open()));
        assert_eq!(
            pane(&drawn[2]).trim_matches('│').trim(),
            headings[nth],
            "the detail heading, row {nth}"
        );
    }

    // **A name that fits is not touched on any of them** — the must-not-fire half.
    let short = box_of(views::Modal::Confirm(scaling()));
    let went = box_of(views::Modal::Gone {
        object: scaling().object,
        recreated: false,
    });
    let flight = footer_of(&render(&changing(Some("payments"), "web"), &plain));
    let card = row(&render(&app(), &plain), " ago");
    let mut open = Open::new();
    open.object = id(ObjectKind::Pod, Some("payments"), "web-7d9f4");
    let heading = rows(&detailed(&on(Tab::Events), &open.open()))[2].clone();
    // **The `$` line is not one of the surfaces here any more** (NOTES § D278). This loop is the
    // must-not-fire half of a test about *names* — a short name is not cut on a surface that
    // names it — and since `--context` the `$` line is cut on every box regardless of the name it
    // carries, so it can no longer answer that question either way. Its own must-not-fire is
    // [`the_command_line_is_cut_to_the_one_box_width_and_never_past_its_border`], which asserts
    // the drawn line for a short command and a long one.
    for (surface, drawn, whole) in [
        ("title", title(&short), "Scale payments/web"),
        ("gone", holding(&went, "payments/web"), "payments/web"),
        ("footer", flight, "changing payments/web first"),
        ("card", card, "● payments/web  ·  3 of 5 pods"),
        ("heading", heading, "payments/web-7d9f4"),
    ] {
        assert!(drawn.contains(whole), "{surface}: {drawn:?}");
        assert!(
            !drawn.contains(CUT),
            "{surface} was cut though it fit: {drawn:?}"
        );
    }
}

/// **A discovery plural too long for its row gives way from its front, marked** — and the two
/// pairs every real cluster serves are two rows again (`screens/widgets.md` § 7, cut 10; NOTES
/// § D266). A plural that fits is drawn whole.
#[test]
fn a_kind_too_long_for_the_sidebar_gives_way_from_its_front_and_two_kinds_stay_two() {
    let now = now();
    let kind = |group: &str, plural: &str| Browsable {
        group: group.to_owned(),
        version: "v1".to_owned(),
        kind: "K".to_owned(),
        plural: plural.to_owned(),
        namespaced: false,
        verbs: vec!["list".to_owned()],
    };
    let kinds = [
        kind("", "persistentvolumes"),
        kind("", "persistentvolumeclaims"),
        kind("storage.k8s.io", "storageclasses"),
        kind(
            "admissionregistration.k8s.io",
            "validatingadmissionpolicies",
        ),
        kind(
            "admissionregistration.k8s.io",
            "validatingadmissionpolicybindings",
        ),
    ];
    let alerts = Pane::Ready(vec![oom()]);
    let mut listed = screen(&alerts, &now);
    listed.kinds = &kinds;
    let sidebar = |group: Group| {
        let mut open = app();
        open.expanded = Some(group);
        rows(&render(&open, &listed))
            .iter()
            .map(|line| {
                line.chars()
                    .skip(1)
                    .take(usize::from(SIDEBAR))
                    .collect::<String>()
            })
            .filter(|cell| cell.starts_with("     ") && !cell.trim().is_empty())
            .map(|cell| cell.trim().to_owned())
            .collect::<Vec<_>>()
    };
    let storage = sidebar(Group::Storage);
    let cluster = sidebar(Group::Cluster);
    println!("{storage:?}\n{cluster:?}");
    assert_eq!(
        storage,
        [
            "\u{2026}sistentvolumes",
            "\u{2026}ntvolumeclaims",
            "storageclasses"
        ],
        "the storage group's three rows"
    );
    assert_eq!(
        cluster,
        ["\u{2026}issionpolicies", "\u{2026}policybindings"],
        "the cluster group's two rows"
    );
}

/// **A call still running and the same call answered are two rows, and the command gives way
/// before either mark does** — `screens/dialogs.md` § The command log's own line, both of its rows
/// byte for byte (NOTES § D266). And a command that fits beside its outcome keeps its namespace.
#[test]
fn a_running_call_and_its_answer_are_two_rows_and_the_command_gives_way_first() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let strip = |lines: &[Stripped]| {
        let mut logged = screen(&alerts, &now);
        logged.log = lines;
        unframed(&row(&render(&app(), &logged), "$ kubectl"))
    };
    let page = excerpt(
        "dialogs.md",
        "## While the call is running",
        "$ kubectl rollout restart deployment/payments-api",
    );
    assert_eq!(
        page.len(),
        2,
        "the page stopped drawing its running and answered rows"
    );

    for (command, running, answered) in [
        (
            "$ kubectl rollout restart deployment/payments-api -n payments-production-eu",
            page[0].clone(),
            page[1].clone(),
        ),
        (
            "$ kubectl scale deployment/web --replicas=3 -n payments",
            "$ kubectl scale deployment/web --replicas=3 -n payments   \u{2026}".to_owned(),
            "$ kubectl scale deployment/web --replicas=3 -n payments   → rejected".to_owned(),
        ),
    ] {
        let mut feed = views::Log::default();
        feed.sent(command.to_owned());
        let before = strip(feed.lines());
        feed.outcome("rejected");
        let after = strip(feed.lines());
        println!("{before}\n{after}");
        assert_eq!(before, running, "{command}: while it runs");
        assert_eq!(after, answered, "{command}: once it is answered");
        assert_ne!(
            before, after,
            "{command}: a running call and a rejected one drew one row"
        );
    }
}

/// **`○  nothing is broken` needs the link** (`screens/states.md` § Alerts had already found
/// nothing, and then the link went; NOTES § D266). The same empty list, the same paragraphs,
/// five links: the headline on the live one and on no other. **[`Link::Connecting`] is one of the
/// five** and is the claim its own doc makes — *nothing is broken* is a verdict on a list that has
/// answered, and on that frame none has (todo.md § Phase 12). **So is [`Link::Unconnected`]**, for
/// that page's own reason one file over: *"k8rs has not read this cluster's health even once"*
/// (`screens/context.md` § After `esc dismiss`, on a switch that failed with a cluster already
/// live) — the strongest claim this product makes, on a cluster it never reached.
#[test]
fn an_empty_alerts_list_says_nothing_is_broken_only_while_the_link_is_live() {
    let now = now();
    let empty = Pane::Ready(Vec::new());
    let counted = [Stripped::of(
        "84 pods and 3 nodes checked, none of them is in trouble right now.",
    )];
    for (link, calm) in [
        (Link::Connecting, false),
        (Link::Live, true),
        (Link::Lost, false),
        (Link::Expired, false),
        (Link::Unconnected, false),
    ] {
        let mut quiet = screen(&empty, &now);
        quiet.note = &counted;
        quiet.link = link;
        let drawn = render(&app(), &quiet);
        assert_eq!(
            holds(&drawn, "nothing is broken"),
            calm,
            "{link:?}:\n{}",
            rows(&drawn).join("\n")
        );
    }
}

/// **Each way a check can fail draws the box its audit line names, and a `409` draws its own**
/// (`screens/dialogs.md` § The cluster said no, states 0 and 1a–1c; NOTES § D266).
///
/// **The three classes are `ops::Record::check`'s**, transcribed because that function is private
/// to a frozen file: the four that never left this machine, the two k8rs never heard back about,
/// and the four the cluster answered. Inside a class every fault draws one box; across classes no
/// two boxes are alike; and a `409` draws one box whether it was the check or the real call that
/// met it, with no quote even where the cluster sent words. No box stacks two blank rows.
#[test]
fn each_way_a_check_can_fail_draws_its_own_box_and_a_conflict_draws_another() {
    let said = Some("the object has been modified; please apply your changes".to_owned());
    let classes: [(&str, &[Fault], &str); 3] = [
        (
            "the check never left this machine",
            &[
                Fault::Kubeconfig,
                Fault::NoContext,
                Fault::BadEntry,
                Fault::NoCredential,
            ],
            "This could not be checked",
        ),
        (
            "k8rs does not know whether the check reached the cluster",
            &[Fault::Unanswered, Fault::Unfinished],
            "The check never got an answer",
        ),
        (
            "the check was sent and did not pass",
            &[Fault::Refused, Fault::Rejected, Fault::Expired, Fault::Gone],
            "The cluster refused this",
        ),
    ];
    let refused = |sent: bool, fault: Fault, said: Option<String>| {
        box_of(views::Modal::Refused { sent, fault, said })
    };
    let mut boxes: Vec<Vec<String>> = Vec::new();
    for (audit, faults, title) in classes {
        let first = refused(false, faults[0], None);
        println!("{audit}\n{}", first.join("\n"));
        assert!(first[0].contains(title), "{audit}: {:?}", first[0]);
        for fault in faults {
            assert_eq!(refused(false, *fault, None), first, "{audit}: {fault:?}");
        }
        boxes.push(first);
    }
    let conflict = refused(false, Fault::Conflict, said.clone());
    println!("409\n{}", conflict.join("\n"));
    assert!(
        conflict[0].contains("The object changed first"),
        "{:?}",
        conflict[0]
    );
    assert_eq!(
        refused(true, Fault::Conflict, said),
        conflict,
        "a 409 is one box both ways"
    );
    assert!(
        !spoken_box(&conflict).contains("apply your changes"),
        "a 409 quoted the cluster"
    );
    boxes.push(conflict);

    for (nth, box_) in boxes.iter().enumerate() {
        assert!(
            !boxes[..nth].contains(box_),
            "box {nth} is another state's box:\n{}",
            box_.join("\n")
        );
        let blank = |row: &String| row.trim_matches('│').trim().is_empty();
        assert!(
            !box_
                .windows(2)
                .any(|pair| blank(&pair[0]) && blank(&pair[1])),
            "box {nth} stacks two blank rows:\n{}",
            box_.join("\n")
        );
    }

    // **1a and 1b say what the page says.** Their excerpts carry no frame, so the words are
    // compared and not the borders (the page draws them at another width).
    for (fault, heading) in [
        (Fault::Kubeconfig, "This could not be checked"),
        (Fault::Unanswered, "The check never got an answer"),
    ] {
        let block = fenced("dialogs.md", "## The cluster said no")
            .into_iter()
            .find(|block| block.first().is_some_and(|top| top.contains(heading)))
            .unwrap_or_else(|| panic!("the page lost the {heading:?} box"));
        let page: Vec<String> = block[1..block.len() - 1]
            .iter()
            .map(|row| row.trim().to_owned())
            .collect();
        let drawn = refused(false, fault, None);
        assert_eq!(
            spoken_box(&drawn[1..drawn.len() - 1]),
            spoken_box(&page),
            "{heading}"
        );
    }
}

/// **An answer taller than the pane is cut to it and drawn, wherever the cursor is**
/// (`screens/analysis.md` § A row taller than the pane; NOTES § D266). The committed captures'
/// own drain pane holds one — `k8rs-worker3`, four problem paragraphs — and with the cursor on it
/// the body was blank.
#[test]
fn a_drain_row_taller_than_the_pane_is_cut_to_it_and_the_cursors_row_is_always_drawn() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let reports = reported();
    let listed = entries(&reports);
    let (nth, (_, drain)) = reports
        .iter()
        .enumerate()
        .find(|(_, (label, _))| *label == "drain safety")
        .expect("drain safety is one of the seven");
    let mut quiet = screen(&alerts, &now);
    quiet.reports = &listed;
    let picks = views::selectable(&drain.rows, views::answers);
    let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
    let mut cut = 0;
    for (at, &pick) in picks.iter().enumerate() {
        let ReportRow::Answer { text, action, .. } = &drain.rows[pick] else {
            panic!("the cursor landed on a row that is not an answer");
        };
        let mut open = opened_report(nth);
        open.content.select(at, &anchors);
        let drawn = render(&open, &quiet);
        let body: Vec<String> = rows(&drawn)[4..18].iter().map(|line| pane(line)).collect();
        println!("answer {at}\n{}", body.join("\n"));
        assert!(
            holds(&drawn, text),
            "answer {at}: {text:?} is not on screen"
        );
        let first = action
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            first.is_empty() || holds(&drawn, &first),
            "answer {at}: the action went"
        );
        // Marked exactly where the answer on the cursor is the tall one, and nowhere else.
        let marked = body.iter().any(|line| line.contains(CUT));
        assert_eq!(marked, text.contains("worker3"), "answer {at}: {text:?}");
        cut += usize::from(marked);
    }
    assert!(
        picks.len() >= 3,
        "the corpus stopped drawing its drain answers"
    );
    assert_eq!(
        cut, 1,
        "the tall answer was not the one cut, or was not marked"
    );
}

/// **[`command_cut`] shape by shape** — the four the strip and the `$` line are handed, each at the
/// budget where its rule decides (`screens/widgets.md` § 7, back-cuts 3 and 4; NOTES § D266).
#[test]
fn a_command_gives_way_from_its_last_flag_and_never_from_its_object() {
    let scale = "kubectl scale deployment/web --replicas=3 -n payments";
    // Whole, borrowed.
    assert_eq!(command_cut(scale, 53, CUT), scale);
    // The value gives way inside itself, keeping what fits.
    assert_eq!(
        command_cut(scale, 50, CUT),
        "kubectl scale deployment/web --replicas=3 -n paym\u{2026}"
    );
    // One character of it still beats dropping it — and `-n` with none of it is never the end.
    assert_eq!(
        command_cut(scale, 47, CUT),
        "kubectl scale deployment/web --replicas=3 -n p\u{2026}"
    );
    assert_eq!(
        command_cut(scale, 46, CUT),
        "kubectl scale deployment/web --replicas=3\u{2026}"
    );
    // A flag with its value glued on goes whole.
    assert_eq!(
        command_cut(scale, 41, CUT),
        "kubectl scale deployment/web\u{2026}"
    );
    // The head with its mark is the last whole-word answer; one column less, the name's front.
    assert_eq!(
        command_cut(scale, 29, CUT),
        "kubectl scale deployment/web\u{2026}"
    );
    assert_eq!(
        command_cut(scale, 28, CUT),
        "kubectl scale deployment/\u{2026}eb"
    );
    assert_eq!(
        command_cut(scale, 26, CUT),
        "kubectl scale deployment/\u{2026}"
    );
    // Fixed words wider than the row: the plain character clip, still marked, never wider.
    assert_eq!(
        command_cut(scale, 25, CUT),
        "kubectl scale deployment\u{2026}"
    );
    assert_eq!(command_cut(scale, 0, CUT), "");

    // **The flag that names the cluster survives where `screens/context.md` puts it and nowhere
    // else** — that page draws `$ kubectl --context staging get pods -A --watch`, the flag second
    // and not last, and this is why. [`command_cut`] keeps a prefix: the head, then whole words
    // from the front until one will not fit, so what a narrow row loses is always the line's
    // *end*. Second, `--context staging` outlives every word after it; appended last it is the
    // first thing gone, and a `kubectl` line that no longer says which cluster it was run against
    // is the dishonesty invariant 4 exists to stop. Both marks, because the strip and the `$`
    // line inside a box spend different ones on the same rule.
    //
    // **`main.rs`'s `kubectl` composes exactly this line now** (NOTES § D278) — every read on
    // the strip carries the flag in this position, where until 2026-09-24 this pinned a
    // placement nothing yet produced.
    for mark in [CUT, STRIP_CUT] {
        let front = command_cut("kubectl --context staging get pods -A --watch", 30, mark);
        let back = command_cut("kubectl get pods -A --watch --context staging", 30, mark);
        assert!(
            front.ends_with(mark),
            "the page's line was not cut at 30 columns, so it says nothing about a cut: {front:?}"
        );
        assert!(
            back.ends_with(mark),
            "the appended line was not cut at 30 columns, so it says nothing either: {back:?}"
        );
        assert!(
            front.contains("--context staging"),
            "the page's own placement lost the cluster the command names: {front:?}"
        );
        assert!(
            !back.contains("--context"),
            "a context flag appended last survived a cut, so this test proves nothing: {back:?}"
        );
    }

    // **The `--context` floor is inside [`command_cut`], so the strip gets it too** — the whole
    // reason it is not a wrapper beside it the way `-n`'s floor is
    // (`screens/dialogs.md` § The context flag never disappears without a trace either). The
    // shape that reaches it is a context name long enough that the head alone does not fit,
    // which a GKE kubeconfig produces without trying: `gke_<project>_<zone>_<cluster>`.
    //
    // **Properties and not a literal**, because no mockup draws a *strip* line at this width —
    // what `screens/` rules is the order things give way in, and that is what is asserted.
    //
    // **Two line shapes, because the floor only fires on one of them** (my own second pass,
    // which caught this test over-claiming on the other). A line naming a `kind/name` token
    // has a *head* that runs through it, and it is that head outgrowing the row that reaches
    // [`ui::context_cut`]; a read like `get pods -A` has no such token, so its head is
    // `kubectl` alone, and what gives way is the ordinary walk from the end — which reaches
    // the same place for this flag, cutting inside its value rather than dropping it. The
    // object's own protection is the first shape's; asserting it on the second was asserting
    // a rule `screens/` does not make.
    let gke = "gke_my-project-12345_us-central1_prod-cluster";
    let named = format!("kubectl --context {gke} rollout restart deployment/web -n payments");
    let read = format!("kubectl --context {gke} get pods -n kube-system --watch");
    for mark in [CUT, STRIP_CUT] {
        for columns in [57, 50, 40, 30, 24] {
            for line in [&named, &read] {
                let drawn = command_cut(line, columns, mark);
                assert!(
                    width(&drawn) <= columns,
                    "{columns}: {drawn:?} is {} columns",
                    width(&drawn)
                );
                // **The flag is never one of the words that drop**, so a pasted line either
                // reaches the connected cluster or fails for a missing value — never another.
                assert!(
                    drawn.contains("--context"),
                    "{columns}: the cut line no longer says which cluster it names: {drawn:?}"
                );
            }
        }
        // **The object outlives every character of the context's own name** — the floor's
        // whole point (`screens/dialogs.md` § A context name long enough to need this on its
        // own: *the object never pays for the context's own length*).
        //
        // **At the widths this product draws, and not below them.** `command_room` gives a box
        // 57 and the strip has the terminal's own width less its borders, so 57 is the narrowest
        // row either surface ever asks for at the 80×24 floor. Below it the row is narrower than
        // a bare `--context…` plus the words after it — where the boundary falls depends on the
        // mark, 49 columns with [`CUT`] and 51 with [`STRIP_CUT`] — and [`ui::command_cut`]'s own
        // last resort takes the row: *a cut wider than the row it is drawn in is worse than one
        // that shows less*, which that function documents as unreachable at the floor. The loop
        // above still runs at those widths, because *fits* and *the flag survives* are owed at
        // every width a caller can ask for.
        let drawn = command_cut(&named, 57, mark);
        assert!(
            drawn.contains("deployment/web"),
            "the object paid before the context's value did: {drawn:?}"
        );
    }
    // **A quoted context name is one word**, whatever is inside it — and *where inside it* the
    // quote sits is the framing, not the value (NOTES § D31). Split it wrong and the tail of
    // the name joins the words *after* the flag, which [`ui::context_cut`] holds out of the
    // budget: the cluster's own name pays the whole cut and the object front-cuts beside a
    // surviving fragment of it. Two framings have done that — a space inside, found by my own
    // second pass, and `ops::pasteable`'s own `'\\''` escape, found by `tester` (2026-09-24).
    //
    // **Every value here is built by the producer rather than written out**, so the consumer's
    // fixtures cannot drift from what the quoter really writes — which is how one of the two
    // framings was missing in the first place.
    for (framing, value) in [
        ("bare", "kind-k8rs".to_owned()),
        (
            "a space inside",
            crate::ops::pasteable("prod eu; echo pwned"),
        ),
        ("a quote inside", crate::ops::pasteable("it's prod")),
        ("nothing but a quote", crate::ops::pasteable("'")),
        (
            "a quote last before the close",
            crate::ops::pasteable("prod'"),
        ),
    ] {
        let line = format!("kubectl --context {value} rollout restart deployment/web");
        for mark in [CUT, STRIP_CUT] {
            // **Narrow enough that the head does not fit, wide enough that the floor is what
            // answers.** Below the bare `--context…` plus the words after it, the row is too
            // narrow for this floor at all and [`ui::command_cut`]'s documented last resort takes
            // it — a different rule, and not the one under test. Two columns under the line is
            // inside that band for every framing here, at both marks.
            let columns = width(&line) - 2;
            let drawn = command_cut(&line, columns, mark);
            assert!(
                width(&drawn) <= columns,
                "{framing}: {drawn:?} is {} columns, not {columns}",
                width(&drawn)
            );
            assert!(
                drawn.contains("--context"),
                "{framing}: the cut line no longer says which cluster it names: {drawn:?}"
            );
            // **The object is whole and last.**
            assert!(
                drawn.ends_with("deployment/web"),
                "{framing}: the tail of the context's own value outlived the object: {drawn:?}"
            );
            // **And what stands between the flag and the object is a *marked prefix of the
            // value* and nothing else** — the assertion a mis-split actually fails.
            // `ends_with` alone does not: read short, the value's own tail simply moves into
            // the words held out of the budget, so the object still survives and the line
            // still ends right while reading `--context 'i… \\''s prod'` — a cluster named
            // `'i` with the rest of its name loose after the cut mark. Measured: the naive
            // scan passed `ends_with` on that framing and failed only on another.
            let tail = " rollout restart deployment/web";
            let named = drawn
                .strip_suffix(tail)
                .and_then(|head| head.strip_prefix("kubectl --context"))
                .map(str::trim_start)
                .unwrap_or_else(|| {
                    panic!("{framing}: the words after the flag are not the command: {drawn:?}")
                });
            assert!(
                named == mark
                    || named
                        .strip_suffix(mark)
                        .is_some_and(|kept| value.starts_with(kept)),
                "{framing}: {named:?} is not a marked prefix of {value:?}"
            );
        }
    }

    // Where not one character of the value fits, the flag still stands and the space goes
    // with it.
    assert!(
        command_cut(&named, 50, CUT).contains("--context\u{2026}"),
        "a bare context flag lost its own name too: {:?}",
        command_cut(&named, 50, CUT)
    );

    // A flag with no value keeps the value before it whole, and the strip's own mark is spent.
    let yaml = "$ kubectl get secret db -n payments -o yaml --show-managed-fields";
    assert_eq!(
        command_cut(yaml, 60, STRIP_CUT),
        "$ kubectl get secret db -n payments -o yaml..."
    );
    // Flags before the object's name are kubectl's too. A value glued on with `=` is its own
    // whole word, so the bare word after it is no value and goes whole; a separate value followed
    // by one ends the line whole.
    let since = "kubectl logs --since=1h web";
    assert_eq!(
        command_cut(since, 26, CUT),
        "kubectl logs --since=1h\u{2026}"
    );
    let scoped = "kubectl logs -n payments web";
    assert_eq!(
        command_cut(scoped, 27, CUT),
        "kubectl logs -n payments\u{2026}"
    );
    // The object named as a flag's value is still the object.
    let events = "$ kubectl events --for pod/checkout-worker-service-canary -n payments";
    assert_eq!(
        command_cut(events, 40, STRIP_CUT),
        "$ kubectl events --for pod/...ice-canary"
    );
    for columns in 0..=80 {
        for line in [scale, yaml, events, since, scoped] {
            for mark in [CUT, STRIP_CUT] {
                let drawn = command_cut(line, columns, mark);
                assert!(
                    width(&drawn) <= columns,
                    "{line:?} at {columns} drew {drawn:?}"
                );
            }
        }
    }
}

/// **Inside a `Confirm`'s `$` line, `-n` is the last flag to give way and its value degrades to a
/// bare `-n…` before the flag name does** — `screens/widgets.md` § 7's back-cut 4 (*"4 goes one
/// floor further than 3, for `-n` alone"*), `screens/dialogs.md` § The namespace flag never
/// disappears without a trace, ruling 1.
///
/// **Every line here is one the product really composes** and the widths that matter are
/// [`command_room`] over the two box widths (`reports/2026-09-20-the-four-behaviours.md` § 5's own
/// measurements), not a shape chosen to make the rule visible. The narrower widths below are the
/// floors: they exist to reach the two states a drawn box cannot — the value that has nowhere
/// left to go, and the row too narrow to keep the object at all.
///
/// **The strip's answer to the same line is asserted beside each one**, because the claim is a
/// difference between two cuts: a test that only read [`namespaced_cut`] would pass just as well
/// if [`command_cut`] had been changed underneath it, which is the one thing back-cut 3 forbids.
#[test]
fn a_confirm_line_gives_up_every_other_flag_before_the_namespace() {
    let budget = command_room(CROWDED_BOX);
    let scale = "kubectl scale deployment/checkout-worker --replicas=3 -n payments-production";

    // The measured case: the namespace's value goes to nothing and the flag still stands, where
    // the strip drops `-n payments-production` whole behind one mark.
    assert_eq!(
        namespaced_cut(scale, budget),
        "kubectl scale deployment/checkout-worker --replicas=3 -n…"
    );
    assert_eq!(
        command_cut(scale, budget, CUT),
        "kubectl scale deployment/checkout-worker --replicas=3…",
        "the strip's own line changed, so the difference above is not the one being asserted"
    );

    // Narrower than any box draws: the other flag gives way **whole** first — which a cut that
    // only ever keeps a prefix of the line cannot do — and what is left over is the namespace's.
    assert_eq!(
        namespaced_cut(scale, 50),
        "kubectl scale deployment/checkout-worker -n payme…"
    );

    // **The floor between those two, at the same drawn width**: a name five columns shorter
    // leaves exactly one column after ` -n`, which is a column a space plus a character cannot
    // share — so the bare flag stands and the line comes in **under** its own budget at 56.
    // A row that is exactly full is what every other case here draws, and a rule that only ever
    // has to fit exactly cannot be told from one that fills whatever it is given.
    let short = "kubectl scale deployment/api-gateway-v2 --replicas=3 -n payments-production";
    assert_eq!(
        namespaced_cut(short, budget),
        "kubectl scale deployment/api-gateway-v2 --replicas=3 -n…"
    );

    // Restart and delete reach ruling 1 through the ordinary value cut both lines already share,
    // and are unchanged by it.
    assert_eq!(
        namespaced_cut(
            "kubectl rollout restart deployment/checkout-worker -n payments-production",
            budget
        ),
        "kubectl rollout restart deployment/checkout-worker -n pa…"
    );

    // The name itself front-cuts **and** the bare flag still stands — the case the section calls
    // "past the identity cut's own case 3".
    let canary = "kubectl scale deployment/checkout-worker-service-canary --replicas=3 \
                  -n team-alpha-payments-platform";
    assert_eq!(
        namespaced_cut(canary, budget),
        "kubectl scale deployment/…ckout-worker-service-canary -n…"
    );
    assert!(
        !command_cut(canary, budget, CUT).contains("-n"),
        "the strip already kept the namespace here, so this case proves nothing"
    );

    // Nothing to protect, or no room to protect it in: the strip's own rule, unchanged. A `-n`
    // that is not the second-to-last word is not this flag.
    for (line, columns) in [
        ("kubectl delete node/node-3", 20),
        ("kubectl get secret db -n payments -o yaml", 30),
        // Four columns is the width where ` -n…` would still fit and the object would not: the
        // one row on which keeping the flag means keeping *only* the flag, which says nothing at
        // all about which object this command runs against.
        (
            "kubectl scale deployment/checkout-worker --replicas=3 -n payments-production",
            4,
        ),
        (
            "kubectl scale deployment/checkout-worker --replicas=3 -n payments-production",
            3,
        ),
    ] {
        assert_eq!(
            namespaced_cut(line, columns),
            command_cut(line, columns, CUT),
            "{line:?} at {columns} did not fall back to the strip's own cut"
        );
    }

    // Every answer above is inside the row it was asked for, in both states — the one that fits
    // whole and the ones that gave way.
    for columns in [budget, command_room(CROWDED_BOX), 50, 30, 4, 3] {
        for line in [
            scale,
            short,
            canary,
            "kubectl scale deployment/web --replicas=3 -n payments",
        ] {
            let drawn = namespaced_cut(line, columns);
            assert!(
                width(&drawn) <= columns,
                "{line:?} at {columns} drew {drawn:?}, which is {} columns",
                width(&drawn)
            );
        }
    }
}

/// **A tall answer is cut to the pane with a mark, a short one is not touched, and an answer
/// whose own identity and action outgrow the pane is still drawn** — `screens/analysis.md` § A row
/// taller than the pane, over rows no corpus holds yet (NOTES § D266). The last is no wording
/// `analysis.rs` builds: its detail gives up everything for the mark, and the row is cut down to
/// the pane rather than skipped.
#[test]
fn an_answer_is_never_taller_than_the_pane_and_a_short_one_is_drawn_whole() {
    let now = now();
    let alerts = Pane::Ready(Vec::new());
    let answer = |name: &str, paragraphs: usize, action: String| ReportRow::Answer {
        severity: Some(Severity::Critical),
        text: format!("{name} would never finish draining"),
        detail: (0..paragraphs)
            .map(|nth| {
                format!(
                    "paragraph {nth} says why this node cannot drain, in a sentence long enough \
                     to wrap onto a second line of the pane"
                )
            })
            .collect(),
        action,
        jump: None,
    };
    let fix = || "check this node first".to_owned();
    for (label, rows_, cursor, marked) in [
        (
            "tall, cursor on it",
            vec![answer("node-2", 7, fix()), answer("node-1", 0, fix())],
            0,
            true,
        ),
        (
            "tall below a short one, cursor on it",
            vec![answer("node-1", 0, fix()), answer("node-2", 7, fix())],
            1,
            true,
        ),
        (
            "short, cursor on it",
            vec![answer("node-2", 2, fix()), answer("node-1", 0, fix())],
            0,
            false,
        ),
        (
            "an action taller than the pane",
            vec![answer("node-2", 1, "move every pod off it ".repeat(40))],
            0,
            true,
        ),
    ] {
        let report = Report {
            title: "Which nodes can be drained right now".to_owned(),
            badge: None,
            rows: rows_,
        };
        let reports = [("drain safety", Some(&report))];
        let mut quiet = screen(&alerts, &now);
        quiet.reports = &reports;
        let mut open = app();
        open.view = View::Analysis(0);
        let picks = views::selectable(&report.rows, views::answers);
        let anchors: Vec<Option<&str>> = picks.iter().map(|_| None).collect();
        open.content.select(cursor, &anchors);
        let drawn = render(&open, &quiet);
        let body: Vec<String> = rows(&drawn)[4..18].iter().map(|line| pane(line)).collect();
        println!("{label}\n{}", body.join("\n"));
        let ReportRow::Answer { text, .. } = &report.rows[picks[cursor]] else {
            panic!("{label}: not an answer");
        };
        assert!(
            holds(&drawn, text),
            "{label}: the cursor's answer is not on screen"
        );
        assert!(holds(&drawn, "→ "), "{label}: the action went");
        assert_eq!(
            body.iter().any(|line| line.contains(CUT)),
            marked,
            "{label}: the mark"
        );
    }
}

// --- THE TWO FILTERS, AND THE CONTAINER PICKER ---

/// **A pod whose containers are known**, owned so the borrows in [`Logs`] have somewhere to live.
/// One capture, read the way `k8s::PodRead` pairs it — `spec` order, never the kubelet's.
struct Picked {
    pod: PodSnapshot,
    names: Vec<String>,
}

impl Picked {
    fn of(capture: &str) -> Self {
        let (pod, names) = declared_by(capture);
        Picked { pod, names }
    }

    fn pairs(&self) -> Vec<(&str, Option<&ContainerSnapshot>)> {
        paired(&self.pod, &self.names)
    }

    /// The snapshot's own containers, to build a state no single capture holds — the same move
    /// [`the_picker_counts_restarts_and_names_the_one_worth_pressing_previous_on`] makes for a
    /// restart count, and not an edit to any committed capture (NOTES § D53).
    fn containers_mut(&mut self) -> &mut Vec<ContainerSnapshot> {
        &mut self.pod.containers
    }
}

/// The logs tab open over a pod, which is the only screen the container picker floats above.
fn reading<'a>(
    read: &'a Described<'a>,
    held: &'a crate::k8s::LogLines,
    open: &'a mut Open<'a>,
) -> Detail<'a> {
    open.logs = Pane::Ready(Logs {
        pod: read,
        container: "app",
        previous: false,
        held,
    });
    open.open()
}

/// **One row of a nested modal box, that box's own cell and nothing else** — the outer frame, the
/// sidebar it floats over and the margins each side are not what the box drew, and [`pane`] cuts
/// at a fixed column that lands inside it.
///
/// **It panics rather than answering `""`**, which [`row`] already does for the same reason: a
/// caller asserting that a row does *not* say something would otherwise pass on a row that was
/// never found (CLAUDE.md § A derived list asserts it found something).
fn boxed_row(drawn: &Buffer, needle: &str) -> String {
    let line = row(drawn, needle);
    line.split('│')
        .find(|cell| cell.contains(needle))
        .unwrap_or_else(|| panic!("no box cell of {line:?} holds {needle:?}"))
        .trim_end()
        .to_owned()
}

/// The whole frame as one run of words, borders dropped — for a sentence inside a box that wraps
/// over two rows, which [`holds`] cannot see and [`body_text`] cuts the wrong column out of.
fn boxed_text(drawn: &Buffer) -> String {
    rows(drawn)
        .join(" ")
        .replace('│', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The frame's cursor, which is the terminal's own and is not a cell in the buffer.
fn cursor_at(columns: u16, lines: u16, app: &App, screen: &Screen) -> ratatui::layout::Position {
    let mut terminal =
        Terminal::new(TestBackend::new(columns, lines)).expect("a terminal over a test backend");
    terminal
        .draw(|frame| draw(frame, &mut app.clone(), screen))
        .expect("a frame");
    terminal.get_cursor_position().expect("a cursor position")
}

/// `screens/widgets.md` § A committed filter is drawn at rest, too — **Alerts has no title row to
/// join, so the line opens one of its own above the first card**, and the cards under it really
/// are the ones that survived.
///
/// **`"web"` is not an arbitrary value**: it is what the remaining card draws, which is that
/// section's own rule about a filter value that could not have produced the row beside it.
#[test]
fn a_committed_filter_opens_a_line_of_its_own_above_the_cards_it_left() {
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let now = now();
    let mut app = app();
    app.filters.text = filter("web");
    let drawn = render(&app, &screen(&alerts, &now));
    // Printed for the same reason [`against`] prints its own — a frame nobody has read is a frame
    // nobody has checked, and this one is what `cargo test -- --nocapture` is for.
    println!("{}", rows(&drawn).join("\n"));

    assert_eq!(
        said(&row(&drawn, "filter:")),
        "  filter: \"web\"   esc clears it"
    );
    assert!(
        holds(&drawn, "payments/web"),
        "the card that matches is gone"
    );
    assert!(
        !holds(&drawn, "node-3"),
        "a card the filter does not hold was drawn:\n{}",
        rows(&drawn).join("\n")
    );

    // **The sidebar's own counts are untouched** (`screens/states.md` § The filter hides every
    // row): `/` and `n` narrow which rows the open pane draws, they are not a second read of the
    // cluster, and a badge that moved with a filter would make the two disagree about how much is
    // wrong.
    let quiet = render(&App::default(), &screen(&alerts, &now));
    assert!(
        !holds(&quiet, "filter:"),
        "an unfiltered list said it was narrowed"
    );
    // The sidebar's own cell, which [`said`] cuts away — a sidebar row and a pane row are one
    // line of the buffer.
    let badge = |drawn: &Buffer| {
        row(drawn, "ALERTS")
            .chars()
            .take(usize::from(1 + SIDEBAR))
            .collect::<String>()
    };
    assert_eq!(
        badge(&drawn),
        badge(&quiet),
        "the filter moved the sidebar's badge"
    );
}

/// `screens/widgets.md` § 2b — **`/` matches every part of the card the reader can see, the
/// remedy line included.**
///
/// [`crate::views::Filters::matches`]' contract is *whatever the row shows the reader*, and
/// `ui::lines` draws `Finding::action` behind its `→` on every card there is. A filter that
/// narrowed a list away from a string sitting on screen is the one failure a filter cannot have
/// (`tester`, 2026-09-18, measured on this exact card).
#[test]
fn the_filter_matches_the_remedy_line_the_card_draws() {
    let alerts = Pane::Ready(vec![oom(), cordon(Some(at(0)))]);
    let now = now();

    // Every part of the card, read off the fixture rather than named twice: the identity, the
    // title, the evidence and the action.
    for typed in [
        "payments/web",
        "exceeded their memory",
        "exit 137",
        "raise limits",
        "or find the leak",
    ] {
        let mut app = app();
        app.filters.text = filter(typed);
        let drawn = render(&app, &screen(&alerts, &now));
        assert!(
            holds(&drawn, "payments/web"),
            "/{typed} hid a card whose own screen holds it:\n{}",
            rows(&drawn).join("\n")
        );
        assert!(
            !holds(&drawn, "No problems match"),
            "/{typed} answered with the zero-match sentence:\n{}",
            rows(&drawn).join("\n")
        );
    }

    // And a string no part of the card draws is still hidden — the filter did not stop filtering.
    let mut app = app();
    app.filters.text = filter("raise the dead");
    let drawn = render(&app, &screen(&alerts, &now));
    assert!(holds(&drawn, "No problems match \"raise the dead\"."));
}

/// The security gate's *sizes are bounded* row, on the one sentence whose job is to quote the
/// reader back to themselves (`screens/states.md` § The filter hides every row).
///
/// **`crate::views::Input` allows a whole `k8s::IDENTIFIER` in each field**, because that is the
/// longest name a delete can ask to have typed back — so this sentence can legally be 1,100 bytes
/// and the pane holds about 150. Before [`MATCH_LINES`] it was wrapped and clipped by the `Rect`:
/// mid-token, with no `…` and no closing quote (`tester`, 2026-09-18).
#[test]
fn a_filter_nobody_could_have_typed_on_purpose_is_cut_and_marked() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut app = app();
    app.filters.text = filter(&"z".repeat(crate::k8s::IDENTIFIER));
    assert_eq!(app.filters.text.text().len(), crate::k8s::IDENTIFIER);

    let drawn = render(&app, &screen(&alerts, &now));
    println!("{}", rows(&drawn).join("\n"));
    // The body rows only — the frame, the header and the footer are not the pane.
    let said: Vec<String> = rows(&drawn)[2..18]
        .iter()
        .map(|line| said(line))
        .filter(|line| !line.trim().is_empty())
        .collect();

    assert_eq!(
        said.len(),
        MATCH_LINES,
        "the sentence is not held to its own rows: {said:?}"
    );
    // The sentence still opens with the words that say what happened — the 512-character token is
    // wider than the pane, so [`wrapped`]'s character break puts it on rows of its own.
    assert!(
        said.join(" ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .starts_with("No problems match \"zzz"),
        "{said:?}"
    );
    assert!(
        said[MATCH_LINES - 1].ends_with(CUT),
        "the cut is not marked, which is the silent truncation § 7 forbids: {said:?}"
    );
    for line in &said {
        assert!(
            width(line) <= 57,
            "the sentence ran past the pane at {} columns: {line:?}",
            width(line)
        );
    }
}

/// **The arm no screen can reach still may not say something false** (`k8s-admin`, 2026-09-18).
///
/// `hidden()` is called only where a filter hid rows, so *no filter set at all* cannot arrive —
/// but a `crate::views::Filters::matches` that answered no to everything would land there, and a
/// sentence saying *what was typed* would then be about a reader who typed nothing. The arm is
/// exercised through the function's own vocabulary rather than through a frame, because a frame
/// that reached it would itself be the bug.
#[test]
fn the_unreachable_zero_match_sentence_claims_nothing_nobody_did() {
    // Every sentence this pane can draw, so the unreachable one is read beside the three that are
    // reachable rather than on its own.
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    for (text, namespace, expected) in [
        ("prodeu", "", "No problems match \"prodeu\"."),
        // A namespace the card really is not in — `pay` reaches `payments` and would leave the
        // card on screen, which is not this state at all.
        ("", "shop", "No problems match a namespace like \"shop\"."),
        (
            "prodeu",
            "pay",
            "No problems match \"prodeu\" in a namespace like \"pay\".",
        ),
    ] {
        let mut app = app();
        app.filters.text = filter(text);
        app.filters.namespace = filter(namespace);
        let drawn = render(&app, &screen(&alerts, &now));
        assert!(
            body_text(&drawn).contains(expected),
            "{text:?}/{namespace:?} does not say {expected:?}:\n{}",
            rows(&drawn).join("\n")
        );
    }

    // The fourth, reached directly: it names no value, because there is none to name.
    let mut terminal = Terminal::new(TestBackend::new(MIN_WIDTH, MIN_HEIGHT))
        .expect("a terminal over a test backend");
    let screen = screen(&alerts, &now);
    terminal
        .draw(|frame| {
            hidden(
                frame,
                frame.area(),
                &screen,
                "problems",
                &crate::views::Filters::default(),
            );
        })
        .expect("a frame");
    let said = rows(&terminal.backend().buffer().clone()).join(" ");
    assert!(
        said.contains("No problems match the filter."),
        "the arm nobody reaches still tells the reader what they typed: {said:?}"
    );
    assert!(
        !said.contains("what was typed") && !said.contains("like \"\""),
        "{said:?}"
    );
}

/// `screens/widgets.md` § A committed filter is drawn at rest, too — **the line says a list was
/// narrowed, so it is drawn only where there is a list.**
///
/// A filter set while the first LIST is still in flight drew `filter: "web"   esc clears it` over
/// *reading the cluster…* — a claim about rows that had not arrived, under a footer that names no
/// `esc` because [`Offer::Nothing`] is what that pane answers (`tester`, 2026-09-18).
#[test]
fn the_filter_line_is_not_drawn_over_a_pane_with_no_rows_to_have_narrowed() {
    let now = now();
    let kinds = [browsable("deployments", true)];
    let mut app = opened();
    app.filters.text = filter("web");

    let reading = [Stripped::of("reading the cluster… 2,140 pods")];
    for (what, browser) in [
        ("a pane that has not answered", Pane::Loading),
        (
            "a kind with no objects in it",
            Pane::Ready(crate::k8s::Table::default()),
        ),
    ] {
        let mut screen = browsing(&browser, &kinds, &now);
        screen.note = &reading;
        let drawn = render(&app, &screen);
        assert!(
            !holds(&drawn, "esc clears"),
            "{what} was told its list had been narrowed:\n{}",
            rows(&drawn).join("\n")
        );
    }

    // Alerts answers the same way over a refusal that came back with nothing under it — **and it
    // is asked on an Alerts `App`**, not the browser's: [`opened`] above is on a kind, and asking
    // this of it would have been answered by the browser pane again (caught in this test's own
    // first red run, 2026-09-18).
    let empty = Pane::Denied(
        "k8rs can only see the payments namespace".to_owned(),
        Vec::new(),
    );
    let mut app = App::default();
    app.filters.text = filter("web");
    let drawn = render(&app, &screen(&empty, &now));
    assert!(
        holds(&drawn, "k8rs can only see"),
        "the refusal's own banner is not on screen, so this frame proves nothing:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        !holds(&drawn, "esc clears"),
        "a refusal with no cards under it was told its list had been narrowed:\n{}",
        rows(&drawn).join("\n")
    );
}

/// `screens/widgets.md` § A committed filter is drawn at rest, too — **the hint names the field
/// `esc` reaches first rather than leaving the reader to guess between two**, and says `it` where
/// there is only one thing it could mean.
#[test]
fn the_at_rest_line_names_whichever_field_esc_would_clear() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();

    let mut namespaced = app();
    namespaced.filters.namespace = filter("pay");
    assert_eq!(
        said(&row(
            &render(&namespaced, &screen(&alerts, &now)),
            "esc clears"
        )),
        "  namespace like: \"pay\"   esc clears it",
        "the namespace half is not labelled the way the zero-match sentence words it"
    );

    // **Both set: the hint goes, not the values** — the labels alone leave two real values two
    // columns to share otherwise, and what the reader typed is written down nowhere else.
    let mut both = app();
    both.filters.text = filter("web");
    both.filters.namespace = filter("pay");
    let drawn = render(&both, &screen(&alerts, &now));
    assert_eq!(
        said(&row(&drawn, "filter:")),
        "  filter: \"web\"   namespace like: \"pay\""
    );
    assert!(
        !holds(&drawn, "esc clears"),
        "the hint stayed on a row that cannot afford it:\n{}",
        rows(&drawn).join("\n")
    );

    // `kube` beside `kube-sys` is the pair the report measured as over by one *with* a hint —
    // both whole now, neither cut, because the hint is what gave way. The card it is drawn over
    // is one both halves really match, which is this page's own rule about filter values.
    let owner = id(ObjectKind::Deployment, Some("kube-system"), "kube-dns");
    let matching = Pane::Ready(vec![Card {
        owner: owner.clone(),
        findings: vec![Finding {
            owner,
            ..oom().findings.remove(0)
        }],
        ..oom()
    }]);
    let mut measured = app();
    measured.filters.text = filter("kube");
    measured.filters.namespace = filter("kube-sys");
    let drawn = render(&measured, &screen(&matching, &now));
    assert!(
        holds(&drawn, "kube-system/kube-dns"),
        "the card both halves match is not on screen:\n{}",
        rows(&drawn).join("\n")
    );
    assert_eq!(
        said(&row(&drawn, "filter:")),
        "  filter: \"kube\"   namespace like: \"kube-sys\""
    );
}

/// `screens/widgets.md` § A committed filter is drawn at rest, too — **overflow gives up the
/// value and never the hint, and never the label either.**
///
/// The hint is the only thing on this row that says how to undo the narrowing, so it is fixed; the
/// label is what says *which* field the quoted string belongs to, so a run front-cut whole — which
/// would eat `filter:` first — is not the cut this row makes. What is left is the value, which the
/// reader can still read off the list.
#[test]
fn a_filter_too_wide_for_its_row_gives_up_its_value_and_keeps_the_label_and_the_hint() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let mut app = app();
    // **A filter this long is unrealistic and it still has to match the card it is drawn over** —
    // a value that could not have produced the row beside it is the claim this page forbids
    // everywhere else, so it is a run of the card's own title.
    let title = "Containers exceeded their memory limit and were killed by the kernel";
    app.filters.text = filter(title);
    let drawn = render(&app, &screen(&alerts, &now));
    let line = said(&row(&drawn, "esc clears"));
    println!("{line:?}");

    // **The whole row, not a `contains`** — what the overflow arm decides is *how much* of the
    // value survives, and only an exact row says it. The pane is 57 columns at the floor and
    // [`padded`] leaves 53; the hint and its gap are fixed, the label and its quotes are fixed,
    // and what is left is the value's — so the row fills the pane and gives up not one character
    // more than it has to.
    assert_eq!(
        line,
        "  filter: \"… were killed by the kernel\"   esc clears it"
    );
    assert_eq!(
        width(&line),
        55,
        "the row does not fill the pane, so the value gave up more than the row asked for"
    );

    // **Both fields set: the hint is gone, and the values get the room it was spending.** Each
    // keeps its own label and an equal share of what the two labels and the gap leave.
    let ns = "platform-observability-and-logging-system";
    let owner = id(ObjectKind::Deployment, Some(ns), "web");
    let wide = Card {
        owner: owner.clone(),
        findings: vec![Finding {
            owner,
            ..oom().findings.remove(0)
        }],
        ..oom()
    };
    let alerts = Pane::Ready(vec![wide]);
    app.filters.namespace = filter("platform-observability-and-logging");
    let drawn = render(&app, &screen(&alerts, &now));
    let line = said(&row(&drawn, "filter:"));
    println!("{line:?}");
    assert!(
        !holds(&drawn, "esc clears"),
        "the hint stayed on a row with two values on it: {line:?}"
    );
    assert_eq!(
        line,
        "  filter: \"…the kernel\"   namespace like: \"…nd-logging\""
    );
    // **The exact width, not a ceiling** (`tester`, 2026-09-18): `<= 55` is satisfied by a row
    // that cut both values to one character, which is the one thing the share exists to stop.
    assert_eq!(width(&line), 55, "{line:?}");
}

/// `screens/widgets.md` § A committed filter is drawn at rest, too — **Resources already draws a
/// title row, and the filter joins it as a second row underneath rather than a third segment
/// crammed onto the first.**
#[test]
fn the_filter_line_sits_under_the_browser_title_and_not_on_it() {
    let now = now();
    let kinds = [browsable("deployments", true)];
    let listed = Pane::Ready(table("table-deployments"));
    let mut screen = browsing(&listed, &kinds, &now);
    screen.namespace = Some(Stripped::of("payments"));
    let mut app = opened();
    app.filters.text = filter("broken");
    let drawn = render(&app, &screen);
    let rows = rows(&drawn);

    let title = rows
        .iter()
        .position(|line| said(line).starts_with("  deployments"))
        .expect("the pane still names its kind");
    assert_eq!(
        said(&rows[title + 1]),
        "  filter: \"broken\"   esc clears it",
        "the filter line is not the row under the title:\n{}",
        rows.join("\n")
    );
    assert!(
        !said(&rows[title]).contains("filter:"),
        "the filter was crammed onto the title row"
    );
    assert!(
        holds(&drawn, "broken-rollout") && !holds(&drawn, "coredns"),
        "the table under the line is not the one the filter left:\n{}",
        rows.join("\n")
    );
}

/// `screens/widgets.md` § 2b — **typing replaces the footer and the pane says nothing**, because
/// the same fact is already on the line the reader is typing on. The cursor is the terminal's own,
/// and it sits after the last character of the buffer.
#[test]
fn typing_draws_the_buffer_in_the_footer_with_the_cursor_after_it() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let screen = screen(&alerts, &now);
    let mut app = app();
    app.filters.text = filter("web");
    app.typing = Some(views::Typing::Text);

    let drawn = render(&app, &screen);
    println!("{}", rows(&drawn).join("\n"));
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "filter: web  ⏎ done  esc clear filter"
    );
    assert!(
        !holds(&drawn, "filter: \"web\""),
        "the at-rest line was drawn under a footer already saying it:\n{}",
        rows(&drawn).join("\n")
    );

    // 1 border + 1 pad, then `filter: web` — the cursor is the column after the last character.
    let at = cursor_at(MIN_WIDTH, MIN_HEIGHT, &app, &screen);
    assert_eq!(
        (at.x, at.y),
        (2 + width("filter: web") as u16, 22),
        "the cursor is not at the end of the buffer"
    );

    // Empty, `esc` has nothing to clear and says so — and the cursor is right after the label.
    app.filters.text = Input::default();
    let drawn = render(&app, &screen);
    assert_eq!(unframed(&rows(&drawn)[22]), "filter:   ⏎ done  esc cancel");
    assert_eq!(
        cursor_at(MIN_WIDTH, MIN_HEIGHT, &app, &screen).x,
        2 + width("filter: ") as u16
    );
}

/// `screens/widgets.md` § 2b — **a buffer past the ceiling loses its own front and never the two
/// hints**: `⏎ done  esc clear filter` is what says how to get out, and the cursor stays the last
/// column drawn.
#[test]
fn a_buffer_longer_than_the_line_gives_up_its_front_and_keeps_the_way_out() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let screen = screen(&alerts, &now);
    let mut app = app();
    app.filters.text = filter(&"payments-web-".repeat(20));
    app.typing = Some(views::Typing::Text);

    let line = unframed(&rows(&render(&app, &screen))[22]);
    assert!(
        line.ends_with("  ⏎ done  esc clear filter"),
        "the hints were pushed off the line: {line:?}"
    );
    assert!(
        line.starts_with("filter: …"),
        "the tail gave way, not the front: {line:?}"
    );
    assert!(
        width(&line) <= 76,
        "the footer ran past the frame: {} columns",
        width(&line)
    );
}

/// `screens/detail.md` § Choosing a container — the same list-picker shape as the cluster picker:
/// `▸` on the row that would open, one line per container, a state word instead of a tag column,
/// and the two buttons under it.
#[test]
fn the_container_picker_draws_one_row_per_container_under_the_pods_own_name() {
    let source = Picked::of("neverback");
    let containers = source.pairs();
    let read = Described {
        snapshot: &source.pod,
        containers: &containers,
    };
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let mut app = on(Tab::Logs);
    app.modal = Some(views::Modal::ContainerPick(Cursor::default()));
    let drawn = detailed(&app, &detail);
    let rows = rows(&drawn);
    println!("{}", rows.join("\n"));

    assert!(
        holds(&drawn, "payments/web-7d9f4 — pick a container"),
        "the box does not say which pod it is about:\n{}",
        rows.join("\n")
    );
    // Every container, and the state column starts in one place whatever the names are.
    let mut columns = Vec::new();
    for (nth, (name, status)) in containers.iter().enumerate() {
        let line = boxed_row(&drawn, name);
        let marker = if nth == 0 { "  ▸ " } else { "    " };
        assert!(
            line.starts_with(&format!("{marker}{name}")),
            "{name} is not drawn at the picker's own indent: {line:?}"
        );
        let (word, _) = views::container_state(status.map(|held| &held.state));
        // **`rfind`, because a container may be named after its own state** — `neverback` has one
        // called `done`, and the name comes first on the row.
        columns.push(
            line.rfind(&word)
                .map(|byte| line[..byte].chars().count())
                .unwrap_or_else(|| panic!("{name} draws no state word: {line:?}")),
        );
    }
    assert_eq!(
        columns.len(),
        3,
        "the capture stopped having three containers"
    );
    assert!(
        columns.windows(2).all(|pair| pair[0] == pair[1]),
        "a name pushed the state column: {columns:?}"
    );

    // **The two fixed columns are pinned toward the right edge and the name takes what is left**
    // (`screens/detail.md` § Choosing a container) — which is a fact about *where* they sit, not
    // only that they line up with each other. This capture carries no restart count, so the state
    // word is the last thing on the row and the widest of them ends one gap short of the box: the
    // gap the empty restart column still reserves. A name column that stopped being the remainder
    // would leave every state word stranded in the middle with the same alignment intact.
    let widest = containers
        .iter()
        .max_by_key(|(_, status)| width(&views::container_state(status.map(|held| &held.state)).0))
        .map(|(name, _)| *name)
        .expect("the capture still has containers");
    assert_eq!(
        width(&boxed_row(&drawn, widest)),
        usize::from(CROWDED_BOX) - NAMES_GAP,
        "the state column is not pinned toward the box's right edge: {:?}",
        boxed_row(&drawn, widest)
    );

    assert!(holds(&drawn, "[ ⏎ pick ]") && holds(&drawn, "[ esc cancel ]"));
    assert_eq!(unframed(&rows[22]), "↑↓ move  ⏎ pick  esc cancel");
}

/// `screens/detail.md` § Choosing a container — **the restart count is next to the container that
/// has one, and the line under the list spells out which key does it and on which container.**
#[test]
fn the_picker_counts_restarts_and_names_the_one_worth_pressing_previous_on() {
    let source = Picked::of("init");
    let containers = source.pairs();
    let read = Described {
        snapshot: &source.pod,
        containers: &containers,
    };
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let mut app = on(Tab::Logs);
    app.modal = Some(views::Modal::ContainerPick(Cursor::default()));
    let drawn = detailed(&app, &detail);

    assert!(
        boxed_row(&drawn, "migrate").ends_with("10 restarts"),
        "the restart count is not in its own column: {:?}",
        boxed_row(&drawn, "migrate")
    );
    assert!(
        !boxed_row(&drawn, "  ▸ app").contains("restart"),
        "a container with none was given a count"
    );
    // **The name column is exactly what the marker, the two gaps, the state column and the count
    // leave**, which is what makes this a column layout rather than four slots that happen to fit:
    // the widest row — the one carrying the widest count — ends on the box's own right edge.
    assert_eq!(
        width(&boxed_row(&drawn, "migrate")),
        usize::from(CROWDED_BOX),
        "the row does not fill the box, so the name column is not the remainder: {:?}",
        boxed_row(&drawn, "migrate")
    );
    assert!(
        boxed_text(&drawn).contains(
            "migrate restarted 10 times. ⇧p shows the log from before that restart, if the \
             kubelet still has it."
        ),
        "the hint does not name the container or the key:\n{}",
        rows(&drawn).join("\n")
    );

    // **The container with the *most* restarts, not the first one with any** — the mockup with
    // four containers names `sidecar-envoy` at 3 over `istio-proxy` at 2, and it is second in
    // `spec` order. No capture has two different non-zero counts, so one is built from one that
    // has two equal ones.
    let mut louder = Picked::of("gang");
    let first = louder.names[0].clone();
    let second = louder.names[1].clone();
    for held in &mut louder.pod.containers {
        if held.name == second {
            held.restarts = 5;
        }
    }
    let containers = louder.pairs();
    let read = Described {
        snapshot: &louder.pod,
        containers: &containers,
    };
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let drawn = detailed(&app, &detail);
    assert!(
        boxed_text(&drawn).contains(&format!("{second} restarted 5 times.")),
        "the hint named a container with fewer restarts:\n{}",
        rows(&drawn).join("\n")
    );

    // **And the first of them where two tie** — `gang` as captured has both at three.
    let containers = Picked::of("gang");
    let pairs = containers.pairs();
    let read = Described {
        snapshot: &containers.pod,
        containers: &pairs,
    };
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let drawn = detailed(&app, &detail);
    assert!(
        boxed_text(&drawn).contains(&format!("{first} restarted 3 times.")),
        "a tie did not go to the first container in spec order:\n{}",
        rows(&drawn).join("\n")
    );

    // A pod where nothing has restarted has nothing for the hint to point at, so it draws none.
    let quiet = Picked::of("healthy-sidecar");
    let containers = quiet.pairs();
    let read = Described {
        snapshot: &quiet.pod,
        containers: &containers,
    };
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let drawn = detailed(&app, &detail);
    assert!(
        !boxed_text(&drawn).contains("⇧p shows the log"),
        "a pod with no restarts was given a restart hint:\n{}",
        rows(&drawn).join("\n")
    );
}

/// `screens/detail.md` § The pod disappears while the picker is open — **the picker closes itself
/// and hands back to the logs tab**, which is where the fact already has a screen. It is not a
/// second `Gone`: picking a container is not a pending mutation, so there is nothing to reassure
/// anybody about.
#[test]
fn the_container_picker_is_not_drawn_once_there_is_nothing_left_to_pick() {
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut app = on(Tab::Logs);
    app.modal = Some(views::Modal::ContainerPick(Cursor::default()));

    // The pod went away: the snapshot behind the tab carries no container any more.
    let gone = Picked::of("healthy-sidecar");
    let none: Vec<(&str, Option<&ContainerSnapshot>)> = Vec::new();
    let read = Described {
        snapshot: &gone.pod,
        containers: &none,
    };
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let drawn = detailed(&app, &detail);
    assert!(
        !holds(&drawn, "pick a container"),
        "a picker was drawn over a pod with no containers:\n{}",
        rows(&drawn).join("\n")
    );
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "[ ] tabs  f follow  esc back  ? all keys  q quit",
        "the picker kept a footer of its own over a box nobody can see"
    );

    // And the same before the container list is known at all — the logs pane has not answered.
    let waiting = Open::new();
    let drawn = detailed(&app, &waiting.open());
    assert!(!holds(&drawn, "pick a container"));
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "[ ] tabs  f follow  esc back  ? all keys  q quit"
    );

    // **And a pod that has exactly one container, which is the ordinary case this rule is written
    // for** (`screens/detail.md`: *a single-container pod has nothing to pick, so the picker is not
    // offered at all*). One is not none, and a box listing the container already on screen is the
    // key that does nothing this product has shipped once already.
    let only = Picked::of("pending");
    let containers = only.pairs();
    assert_eq!(
        containers.len(),
        1,
        "the capture is not a single-container pod"
    );
    let read = Described {
        snapshot: &only.pod,
        containers: &containers,
    };
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let drawn = detailed(&app, &detail);
    assert!(
        !holds(&drawn, "pick a container"),
        "a single-container pod was offered a picker:\n{}",
        rows(&drawn).join("\n")
    );
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "[ ] tabs  f follow  esc back  ? all keys  q quit"
    );
}

/// `screens/detail.md` § Choosing a container — **six rows is what this box's own chrome leaves
/// under the 24-row ceiling, and a seventh container is what puts a `Scrollbar` on the list.**
///
/// **The number is derived and not written down**, which is that section's own instruction: what a
/// review checks is how many rows the fixed chrome leaves, not a drawing. Six with the restart
/// hint drawn is what it counts, and this is that count read off the frame.
#[test]
fn the_container_list_stops_at_six_rows_and_says_so_with_a_scrollbar() {
    // `init` is two containers, one of which has restarted — so the hint is drawn and the list's
    // budget is the six that section counts. Five more make seven, which is one past it.
    let mut many = Picked::of("init");
    let template = many
        .pod
        .containers
        .first()
        .cloned()
        .expect("the capture still has a container");
    for nth in 0..5 {
        let mut extra = template.clone();
        extra.name = format!("sidecar-{nth}");
        many.names.push(extra.name.clone());
        many.pod.containers.push(extra);
    }
    assert_eq!(many.names.len(), 7, "the fixture is not seven containers");

    let containers = many.pairs();
    let read = Described {
        snapshot: &many.pod,
        containers: &containers,
    };
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let mut app = on(Tab::Logs);
    app.modal = Some(views::Modal::ContainerPick(Cursor::default()));
    let drawn = detailed(&app, &detail);

    // **A container is counted only where the *list* draws it** (`tester`, 2026-09-18): a name
    // counted anywhere on the frame also counts the restart hint's own sentence, which names one —
    // so this read six today by luck and would read seven the moment the hint's container scrolled
    // out of the list. A list row is the box's margin, the marker column, then the name; the
    // hint's row is the margin and the name.
    let drawn_in_list =
        |name: &str| holds(&drawn, &format!("  ▸ {name}")) || holds(&drawn, &format!("    {name}"));
    let shown = many.names.iter().filter(|name| drawn_in_list(name)).count();
    assert_eq!(
        shown,
        6,
        "the list is not the six rows this box's own chrome leaves:\n{}",
        rows(&drawn).join("\n")
    );
    // **The scrollbar is the widget's, and it is drawn over the list and nothing else** — the
    // hint, the buttons and the blank rows are not what it says there is more of
    // (`screens/widgets.md` § 2: rendered only once the content exceeds the viewport).
    let list_rows: Vec<usize> = rows(&drawn)
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            many.names.iter().any(|name| {
                line.contains(&format!("  ▸ {name}")) || line.contains(&format!("    {name}"))
            })
        })
        .map(|(nth, _)| nth)
        .collect();
    assert_eq!(list_rows.len(), 6, "the list's own rows were not found");
    let track: Vec<usize> = rows(&drawn)
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains('█'))
        .map(|(nth, _)| nth)
        .collect();
    assert!(
        !track.is_empty(),
        "a list taller than its box drew no scrollbar:\n{}",
        rows(&drawn).join("\n")
    );
    // **No row reaches the scrollbar's own column, because it is reserved before the two fixed
    // columns are measured** (`screens/detail.md`, `k8s-admin` 2026-09-18): the widest row was
    // drawing into it, so `10 restarts` came out `10 restart` — the last character silently the
    // widget's paint, which is the unmarked cut § 7 forbids.
    let widest = boxed_row(&drawn, "migrate");
    // The widget's own glyphs are not the row's text; what is asserted is what the row drew.
    let text = widest.trim_end_matches(['█', '║']);
    assert!(
        text.ends_with("10 restarts"),
        "the restart count lost a character to the scrollbar: {widest:?}"
    );
    assert_eq!(
        width(text),
        usize::from(CROWDED_BOX) - 1,
        "the widest row still spends the column the scrollbar is about to paint: {widest:?}"
    );
    assert!(
        track.iter().all(|nth| list_rows.contains(nth)),
        "the scrollbar is drawn beside rows that are not the list — track {track:?}, list \
         {list_rows:?}:\n{}",
        rows(&drawn).join("\n")
    );
    // The whole box still sits inside the 24-row frame it is centred in.
    assert!(
        rows(&drawn)[23].starts_with('└'),
        "the box pushed the frame open"
    );

    // **The window follows the cursor** — `↑`/`↓` reach a seventh container by scrolling the list
    // under them, which is the other half of what a capped list owes (`screens/detail.md`).
    let anchors: Vec<Option<&str>> = containers.iter().map(|(name, _)| Some(*name)).collect();
    let mut at = Cursor::default();
    at.select(anchors.len() - 1, &anchors);
    app.modal = Some(views::Modal::ContainerPick(at));
    let scrolled = detailed(&app, &detail);
    assert!(
        holds(&scrolled, many.names.last().expect("seven names").as_str()),
        "the cursor's own container is not on screen:\n{}",
        rows(&scrolled).join("\n")
    );
    assert!(
        !holds(&scrolled, &format!("  ▸ {}", many.names[0]))
            && !holds(&scrolled, &format!("    {}  ", many.names[0])),
        "the list did not scroll — the first container is still drawn:\n{}",
        rows(&scrolled).join("\n")
    );
}

/// `screens/detail.md` § When a container's own state is what does not fit — **the name never
/// gives way to the state word; the state gives way to the name.**
///
/// **The blocker this closes, measured before it was fixed**
/// (`reports/2026-09-18-filter-and-container-picker.md` § 1): one container in
/// `CreateContainerConfigError` translates to *needs a ConfigMap or Secret that does not exist*,
/// 47 columns, and with the name column taken as the plain remainder that left it at **one** —
/// `front(name, 1, "…")` draws nothing, so two rows had no name on them at all and the restart
/// count was clipped by the box with no mark. A picker whose rows do not name anything is not a
/// picker, and the pod this happens on is exactly the pod somebody opens one for.
#[test]
fn a_state_word_too_wide_gives_way_to_the_name_and_never_the_other_way_round() {
    // The report's own shape: `gang`'s two containers, the second one's state taken from the
    // committed `config` capture. Neither fixture edited; the two combined here.
    let mut pod = Picked::of("gang");
    let waiting = Picked::of("config");
    let state = waiting
        .pod
        .containers
        .first()
        .map(|held| held.state.clone())
        .expect("the capture still has a container");
    let second = pod.names[1].clone();
    for held in pod.containers_mut() {
        if held.name == second {
            held.state = state.clone();
        }
    }
    let containers = pod.pairs();
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    let read = Described {
        snapshot: &pod.pod,
        containers: &containers,
    };
    let detail = reading(&read, &held, &mut open);
    let mut app = on(Tab::Logs);
    app.modal = Some(views::Modal::ContainerPick(Cursor::default()));
    let drawn = detailed(&app, &detail);
    println!("{}", rows(&drawn).join("\n"));

    // **Both names read in full** — neither is 20 columns, so neither meets even the floor.
    for name in &pod.names {
        let line = boxed_row(&drawn, name);
        assert!(
            line.starts_with(&format!("  ▸ {name}")) || line.starts_with(&format!("    {name}")),
            "{name} is not drawn at the picker's own indent: {line:?}"
        );
    }
    // The state that could not fit is cut and marked, not dropped and not guessed at.
    let cut_row = boxed_row(&drawn, &second);
    assert!(
        cut_row.contains(&format!("needs a ConfigMap{CUT}"))
            || cut_row.contains(&format!("needs a ConfigMap or{CUT}")),
        "the long state is not back-cut behind a mark: {cut_row:?}"
    );
    // And no row is wider than the box it is drawn in.
    for name in &pod.names {
        assert!(
            width(&boxed_row(&drawn, name)) <= usize::from(CROWDED_BOX),
            "a row ran past the box: {:?}",
            boxed_row(&drawn, name)
        );
    }

    // **The `Waiting` fall-through keeps the kubelet's own reason, bounded only by
    // `IDENTIFIER`** — a reason no table names reaches the row as-is, so the floor has to hold
    // against a state word far longer than any k8rs writes itself.
    let long = "A".repeat(crate::k8s::IDENTIFIER);
    let mut pod = Picked::of("gang");
    for held in pod.containers_mut() {
        if held.name == second {
            held.state = ContainerState::Waiting {
                reason: Some(long.clone()),
                message: None,
            };
        }
    }
    let containers = pod.pairs();
    let read = Described {
        snapshot: &pod.pod,
        containers: &containers,
    };
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let drawn = detailed(&app, &detail);
    println!("{}", rows(&drawn).join("\n"));
    for name in &pod.names {
        let line = boxed_row(&drawn, name);
        assert!(
            line.contains(name.as_str()),
            "a 512-byte state word erased a name: {line:?}"
        );
        assert!(
            width(&line) <= usize::from(CROWDED_BOX),
            "a 512-byte state word ran the row past the box at {} columns",
            width(&line)
        );
    }
}

/// `screens/detail.md` § When a container's own state is what does not fit — **a state word that
/// exactly fills its column is drawn verbatim, because it is not *wider* than the column.**
///
/// **The divergence is one column and it is not an ellipsis**, which is worth saying because that
/// is what the boundary looks like it should do: measured, `cut` at a text that already fits is a
/// no-op, so cutting one character early shows up instead as the state losing a **leading space**
/// — `wrapped` drops the whitespace a line starts with, and `k8s::text` keeps a leading space at
/// ingest because a space is printable. `crate::views::container_state`'s fall-through hands the
/// kubelet's own `reason` through as the state word, so a reason that begins with one is API text
/// this row draws verbatim or not at all (`k8s-admin` sweep, 2026-09-18).
#[test]
fn a_state_word_that_exactly_fills_its_column_is_drawn_whole() {
    // 20 columns wide including its leading space, which is `NAME_FLOOR` — so the name column
    // lands exactly on its floor and the state column gets exactly this word's own width.
    let reason = " 1234567890123456789";
    assert_eq!(width(reason), NAME_FLOOR);

    let mut pod = Picked::of("gang");
    let second = pod.names[1].clone();
    for held in pod.containers_mut() {
        if held.name == second {
            held.state = ContainerState::Waiting {
                reason: Some(reason.to_owned()),
                message: None,
            };
        }
    }
    let containers = pod.pairs();
    let read = Described {
        snapshot: &pod.pod,
        containers: &containers,
    };
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let mut app = on(Tab::Logs);
    app.modal = Some(views::Modal::ContainerPick(Cursor::default()));
    let drawn = detailed(&app, &detail);
    println!("{}", rows(&drawn).join("\n"));

    let line = boxed_row(&drawn, &second);
    assert!(
        line.contains(reason),
        "a state word that fits was reformatted anyway: {line:?}"
    );
    assert!(
        !line.contains(CUT),
        "a state word that fits was marked as cut: {line:?}"
    );
    // And it sits where the column puts it — one gap left of the restart count, which is what
    // losing its leading space would move.
    assert!(
        line.ends_with(&format!("{reason}   3 restarts")),
        "the state word did not keep its own column: {line:?}"
    );
}

/// `screens/widgets.md` § 7, cut 6 at its second call site — **a container name too wide for its
/// column gives way from its front**, because `istio-proxy` and `istio-proxy-metrics` differ at
/// the tail and a back-cut would draw them identically.
#[test]
fn a_container_name_too_wide_for_its_column_gives_up_its_front() {
    let mut source = Picked::of("healthy-sidecar");
    let long = format!("{}-metrics", "a".repeat(80));
    source.names[1] = long.clone();
    if let Some(container) = source.pod.containers.get_mut(1) {
        container.name = long.clone();
    }
    let containers = source.pairs();
    let read = Described {
        snapshot: &source.pod,
        containers: &containers,
    };
    let held = logged(&["14:21:58  starting worker pool"]);
    let mut open = Open::new();
    let detail = reading(&read, &held, &mut open);
    let mut app = on(Tab::Logs);
    app.modal = Some(views::Modal::ContainerPick(Cursor::default()));
    let drawn = detailed(&app, &detail);

    let line = boxed_row(&drawn, "-metrics");
    assert!(
        line.contains("a-metrics"),
        "the tail of the name gave way instead of its front: {line:?}"
    );
    assert!(
        line.starts_with("    …"),
        "the cut is not marked at the front: {line:?}"
    );
}

// --- PICKING A POD, AND EVERY FINDING PINNED ON EVERY TAB ---

/// One finding about one pod of `payments/web`, with a card's own three sentences.
fn about(pod: &str, severity: Severity, title: &str, evidence: &str) -> Finding {
    Finding {
        object: id(ObjectKind::Pod, Some("payments"), pod),
        ..finding(
            severity,
            title,
            evidence,
            "raise limits.memory, or find the leak",
        )
    }
}

/// **The grouped card `screens/detail.md` § Picking a pod draws** — three pods of a Deployment of
/// five, two killed for memory and one that cannot pull its image.
fn group() -> Card {
    Card {
        owner: id(ObjectKind::Deployment, Some("payments"), "web"),
        findings: vec![
            about(
                "web-7d9f4bc86d-x2k9p",
                Severity::Critical,
                "ran out of memory",
                "limit 256Mi · exit 137",
            ),
            about(
                "web-7d9f4bc86d-m3p1q",
                Severity::Critical,
                "ran out of memory",
                "limit 256Mi · exit 137",
            ),
            about(
                "web-7d9f4bc86d-t8g2r",
                Severity::Warn,
                "image pull failed",
                "registry refused the pull",
            ),
        ],
        affected: 3,
        total: Some(5),
    }
}

/// The which-pods step at the 80×24 floor, drawn over an Alerts list holding the same card.
fn stepping(app: &App, card: &Card) -> Buffer {
    let alerts = Pane::Ready(vec![card.clone()]);
    let now = now();
    let mut screen = screen(&alerts, &now);
    screen.detail = Some(Detailed::Pods(card));
    render(app, &screen)
}

/// The detail tabs open **on one named pod** of a card Alerts holds. Which pod is not decoration:
/// a tab pins the findings whose own `object` is this one (`screens/detail.md` § A pod's own
/// findings, not the whole card's).
fn pinned_on(tab: Tab, card: &Card, pod: &str) -> Buffer {
    let mut open = Open::new();
    open.object = id(ObjectKind::Pod, Some("payments"), pod);
    open.card = Some(card.clone());
    let held = open.open();
    detailed(&on(tab), &held)
}

/// `screens/detail.md` § Picking a pod — **the head row, the block, the blank, and one row per
/// pod**, in that order, with the cursor on a pod and never on a block.
#[test]
fn the_step_names_the_group_and_lists_one_row_per_pod() {
    let card = group();
    let drawn = stepping(&app(), &card);

    // **The owner, `Card::count`'s own fragment, and a tail that never gives way** — the head row
    // is the one place the identity is said, which is why the blocks below leave it out.
    assert_eq!(
        celled(&row(&drawn, "pick a pod")),
        "  payments/web  ·  3 of 5 pods — pick a pod"
    );

    // One row per pod: the marker's two columns, the pod's own band glyph, its name, its own
    // worst finding's title.
    // **Rows fall in name order inside a band**, so `m3p1q` leads `x2k9p` and the cursor starts
    // on it (`screens/detail.md` § Picking a pod, NOTES § D270).
    assert_eq!(
        celled(&row(&drawn, "m3p1q")),
        "▸ ● web-7d9f4bc86d-m3p1q   ran out of memory"
    );
    assert_eq!(
        celled(&row(&drawn, "x2k9p")),
        "  ● web-7d9f4bc86d-x2k9p   ran out of memory"
    );
    // **A pod's glyph is that pod's own severity and not the card's**, or a list of three rows
    // under one red dot says every pod is as bad as the worst.
    assert_eq!(
        celled(&row(&drawn, "t8g2r")),
        "  ▲ web-7d9f4bc86d-t8g2r   image pull failed"
    );

    // **`↑↓` never lands on a block row**, the sidebar's own rule for its section headings: the
    // marker is on a pod, and the blocks above it carry the same two blank columns every
    // unselected row does.
    assert_eq!(
        rows(&drawn)
            .iter()
            .filter(|line| pane(line).starts_with("▸ "))
            .count(),
        1,
        "more than one row wore the cursor\n{}",
        rows(&drawn).join("\n")
    );

    assert_eq!(
        row(&drawn, "↑↓ move").trim_end(),
        "│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                                │"
    );
    assert!(!holds(&drawn, "/ filter"), "the step offered a filter");
}

/// `screens/detail.md` § Picking a pod — **a block on this step draws title, evidence and action
/// and not the identity line**, because the head row two lines up already says the owner and the
/// count it would repeat. This is the one place on the product a block is three parts.
#[test]
fn a_block_on_the_step_leaves_out_the_line_the_head_row_already_says() {
    let card = group();
    let drawn = stepping(&app(), &card);
    let seen = rows(&drawn);

    assert!(holds(&drawn, "limit 256Mi · exit 137"), "no evidence drawn");
    assert!(holds(&drawn, "→ raise limits.memory"), "no action drawn");
    assert_eq!(
        seen.iter()
            .filter(|line| line.contains("payments/web"))
            .count(),
        1,
        "the owner was named twice on a screen that names it once\n{}",
        seen.join("\n")
    );
}

/// `screens/detail.md` § Every finding pinned at the top of every tab — **all four tabs, and the
/// evidence drawn in full.**
///
/// The three-line `…` an Alerts card puts on the same evidence is redeemed here or it points at
/// nothing, so both halves are asserted off one string: cut on the card, whole on every tab.
#[test]
fn every_tab_pins_the_card_and_is_the_one_place_the_evidence_is_not_cut() {
    // **One token and not a phrase**: the assertion is about a *row* holding it, and a phrase
    // long enough to be distinctive is long enough to land on a wrap boundary and be found on
    // neither row.
    let tail = "whereupon-it-gave-up-entirely";
    let long = format!(
        "limit 256Mi · exit 137 · 47 restarts · the controller went on at some considerable \
         length about what it had tried, which node it had tried it on, and how long it waited \
         before it {tail}"
    );
    let card = Card {
        findings: vec![about(
            "web-7d9f4bc86d-x2k9p",
            Severity::Critical,
            "ran out of memory",
            &long,
        )],
        ..group()
    };

    let listed = render(&app(), &screen(&Pane::Ready(vec![card.clone()]), &now()));
    assert!(
        holds(&listed, "…") && !holds(&listed, tail),
        "the card drew the evidence whole, so there is nothing for a tab to redeem"
    );

    for tab in Tab::ALL {
        let drawn = pinned_on(tab, &card, "web-7d9f4bc86d-x2k9p");
        assert!(
            holds(&drawn, "ran out of memory"),
            "{tab:?} lost the reason the object was opened"
        );
        assert!(
            holds(&drawn, tail),
            "{tab:?} cut the evidence the card's own `…` promised was one ⏎ away"
        );
        // **The identity line stays on a tab** — its heading names one pod and never the owner or
        // the count, so nothing there already carries what this line says.
        assert!(
            holds(&drawn, "payments/web  ·  3 of 5 pods"),
            "{tab:?} dropped the block's identity line"
        );
    }
}

/// `screens/detail.md` § A pod's own findings, not the whole card's — **a pod carrying two
/// findings pins both, worst first, and neither carries the marker that points at the stack.**
///
/// **This is the `affected <= 1` guarantee, asserted** (NOTES § D270): where the card is
/// about one pod, every finding the card holds and every finding about that pod are the same set,
/// so § A group of one pod's *"with every finding the card holds … pinned there"* stays true word
/// for word. It is the one shape where the narrowed predicate and the old one agree, which is what
/// makes it the test that must not go red.
#[test]
fn a_pod_carrying_two_findings_pins_both_worst_first_and_neither_counts_the_rest() {
    // **One pod carrying two findings**, which is the only way a stack happens on a tab at all:
    // a finding about a *different* pod of the group does not pin here (`screens/detail.md` § A
    // pod's own findings, not the whole card's). Two and not three, so the whole stack is inside
    // the 13 rows a tab's body has at the floor and the order this asserts is not an artefact of
    // where the pane ran out.
    let card = Card {
        findings: vec![
            about(
                "web-a",
                Severity::Warn,
                "image pull failed",
                "registry said no",
            ),
            about("web-a", Severity::Critical, "ran out of memory", "exit 137"),
        ],
        affected: 1,
        ..group()
    };
    let drawn = pinned_on(Tab::Logs, &card, "web-a");
    let seen = rows(&drawn);
    let at = |needle: &str| {
        seen.iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", seen.join("\n")))
    };

    assert!(
        at("ran out of memory") < at("image pull failed"),
        "the warning was pinned above the critical\n{}",
        seen.join("\n")
    );
    // **The fifth part is the card face's and never a block's** — `N more problems — ⏎ to see`
    // points *at* this stack, and drawing it inside one points the reader at what they are
    // already looking at.
    assert!(
        !holds(&drawn, "more problems"),
        "a pinned block pointed at itself\n{}",
        seen.join("\n")
    );
}

/// `screens/detail.md` § Every finding pinned — **an object Alerts has no card about pins
/// nothing**, which is every healthy row the browser opens `⏎` on.
#[test]
fn an_object_with_no_card_pins_nothing_and_the_tab_starts_where_it_always_did() {
    let open = Open::new();
    let held = open.open();
    for tab in Tab::ALL {
        let drawn = detailed(&on(tab), &held);
        assert!(
            !holds(&drawn, "payments/web  ·"),
            "{tab:?} pinned a card this object has none of"
        );
    }
}

/// `screens/detail.md` § Picking a pod — **the name gives way from its front and the fact from its
/// end**, the two cuts that section names, each behind a visible `…`.
#[test]
fn a_long_pod_name_and_a_long_fact_each_give_way_at_their_own_end() {
    let long = "checkout-worker-service-canary-7d9f4bc86d-x2k9p";
    let card = Card {
        findings: vec![
            about(
                long,
                Severity::Critical,
                "Containers exceeded their memory limit and were killed by the kernel",
                "limit 256Mi",
            ),
            about("web-2", Severity::Critical, "ran out of memory", "exit 137"),
        ],
        affected: 2,
        ..group()
    };
    let drawn = stepping(&app(), &card);

    // **Asserted as the whole row and not as two `contains`** — the name's 30 columns and the
    // fact's 20 are `region − GUTTER − NAMES_GAP − FACT_FLOOR` and the floor itself, and a
    // `contains` pair passes at 29 and 21 too (`just mutants-diff`, 2026-09-18: `+` for `*` in
    // the gutter arithmetic survived exactly that).
    //
    // Two rollouts of one Deployment differ at the *end* of the name, so that is the end that
    // stays — behind one `…`, never a silent clip; the fact keeps its start and says it was cut.
    assert_eq!(
        celled(&row(&drawn, "x2k9p")),
        "▸ ● …ice-canary-7d9f4bc86d-x2k9p   Containers exceeded…"
    );
}

/// `screens/detail.md` § More pods than the pane shows — **the bar's thumb spans the whole list,
/// block rows included: 15 of 41, not 12 of 38.**
///
/// **The thumb and not the track** (`tester`, 2026-09-18). A `Scrollbar`'s track fills its column
/// whatever content length it was handed, so a test that only looks for a glyph is satisfied by
/// both answers and says nothing — rendered at 41 rows and at 38, every row carried one. What
/// separates them is how many cells the thumb covers.
#[test]
fn the_bars_thumb_counts_the_block_rows_as_well_as_the_pods() {
    let sentence = "Nodes without enough memory refused to run this Pod";
    let many: Vec<Finding> = (0..38)
        .map(|nth| {
            about(
                &format!("log-shipper-{nth:03}"),
                Severity::Critical,
                sentence,
                "",
            )
        })
        .collect();
    let card = Card {
        owner: id(ObjectKind::DaemonSet, Some("payments"), "log-shipper"),
        findings: many,
        affected: 38,
        total: Some(40),
    };
    let seen = rows(&stepping(&app(), &card));

    // The block is 2 rows and the blank under it a third, so the list is 41 rows and the pane
    // shows 15 of them — `screens/detail.md`'s own figures for this exact card.
    let thumb = seen.iter().filter(|line| line.contains('█')).count();
    let track = seen
        .iter()
        .filter(|line| line.contains('█') || line.contains('║'))
        .count();
    assert_eq!(
        track,
        15,
        "the bar did not span the list's own pane\n{}",
        seen.join("\n")
    );
    assert_eq!(
        thumb,
        15 * 15 / 41,
        "the thumb counts the pods and not the list: {thumb} cells\n{}",
        seen.join("\n")
    );

    // **Nothing is painted over**: the bar took the margin the rows already reserve, so every row
    // of the box still closes with its own border and none gave up a column.
    for line in seen.iter().filter(|line| line.starts_with('│')) {
        assert!(
            line.ends_with('│'),
            "the bar took the frame's own column: {line:?}"
        );
    }
    assert!(seen.iter().any(|line| line.contains("log-shipper-000")));
}

/// The content pane's own cells of a row — the sidebar, the dividers and the frame's own border
/// dropped — so a column assertion counts from the pane's left edge and stops at its right one.
fn celled(line: &str) -> String {
    pane(line).trim_end_matches('│').trim_end().to_owned()
}

/// The which-pods step and one pinned tab, printed whole, so a reader of the report can compare
/// them with `screens/detail.md` § Picking a pod and § Every finding pinned line by line.
/// `cargo test -- --nocapture`.
#[test]
fn the_which_pods_step_and_a_pinned_tab_at_the_floor() {
    let card = group();
    println!("{}\n", rows(&stepping(&app(), &card)).join("\n"));

    let many: Vec<Finding> = (0..38)
        .map(|nth| {
            about(
                &format!("log-shipper-{nth:03}"),
                Severity::Critical,
                "Nodes without enough memory refused to run this Pod",
                "",
            )
        })
        .collect();
    let daemon = Card {
        owner: id(ObjectKind::DaemonSet, Some("payments"), "log-shipper"),
        findings: many,
        affected: 38,
        total: Some(40),
    };
    println!("{}\n", rows(&stepping(&app(), &daemon)).join("\n"));

    let (pod, names) = declared_by("pending");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let lines = logged(&["14:23:41  listening on :8080", "14:23:44  GET /healthz 200"]);
    let mut open = Open::new();
    // **Opened on a pod the card is actually about**, which is the whole of what a tab pins now:
    // the same `Open::new()` default drew an empty pane here, correctly (§ A pod's own findings,
    // not the whole card's).
    open.object = id(ObjectKind::Pod, Some("payments"), "web-7d9f4bc86d-x2k9p");
    open.card = Some(card.clone());
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &lines,
    });
    let tabbed = detailed(&on(Tab::Logs), &open.open());
    println!("{}", rows(&tabbed).join("\n"));

    // **A canary, because three `println!`s cannot fail** — the same shape
    // [`the_alerts_screen_at_the_floor`] ends on (`tester`, 2026-09-18). One line off each of the
    // three frames: the step's own head row, the head row of the group too big to draw whole, and
    // the block a tab pins.
    assert!(holds(&stepping(&app(), &card), "— pick a pod"));
    assert!(holds(&stepping(&app(), &daemon), "38 of 40 pods"));
    assert!(holds(&tabbed, "payments/web  ·  3 of 5 pods"));
}

/// `screens/detail.md` § More pods than the pane shows — **the step leads with one block and the
/// rest of the pane is the pods**, which is that section's own counted arithmetic: a log-shipper
/// DaemonSet at `38 of 40 pods`, *"one block … spends 2 rows on the block and 1 on the blank
/// separator: 15 − 3 = 12 rows left for pods"*.
///
/// **This is the assertion that fixes the reading** (`dev-ui`, 2026-09-18, PM's to rule): drawn one
/// block per finding instead, the same card measured **one** pod row and 38 copies of one
/// sentence above it — the step's whole job pushed off the pane.
#[test]
fn the_step_leads_with_one_block_and_spends_the_rest_of_the_pane_on_pods() {
    let sentence = "Nodes without enough memory refused to run this Pod";
    let many: Vec<Finding> = (0..38)
        .map(|nth| {
            about(
                &format!("log-shipper-{nth:03}"),
                Severity::Critical,
                sentence,
                "",
            )
        })
        .collect();
    let card = Card {
        owner: id(ObjectKind::DaemonSet, Some("payments"), "log-shipper"),
        findings: many,
        affected: 38,
        total: Some(40),
    };
    let drawn = stepping(&app(), &card);
    let seen = rows(&drawn);

    assert_eq!(
        seen.iter()
            .filter(|line| line.contains("log-shipper-"))
            .count(),
        12,
        "the pane did not spend 15 − 3 rows on pods\n{}",
        seen.join("\n")
    );
    // One block: the sentence appears once as a block and once per drawn pod row, never twice as
    // a block. A row is a block's only when it does not name a pod.
    assert_eq!(
        seen.iter()
            .filter(|line| line.contains(sentence) && !line.contains("log-shipper-"))
            .count(),
        1,
        "the step pinned more than one block\n{}",
        seen.join("\n")
    );
}

/// `screens/alerts.md` § The columns, `screens/detail.md` § Every finding pinned — **a block's band
/// and its age are that block's own finding's, never the card's.**
///
/// Read off the card, a stack of three drew three identical identity lines and put a `▲` finding
/// under a `●` — a line claiming every part of the stack is as bad as its worst (`dev-ui`,
/// 2026-09-18, measured before this was split).
#[test]
fn a_blocks_band_and_age_are_its_own_findings_and_not_the_cards() {
    let card = Card {
        findings: vec![
            Finding {
                timestamp: Some(ago(4 * 24 * 60 * 60)),
                ..about("web-a", Severity::Critical, "ran out of memory", "exit 137")
            },
            Finding {
                timestamp: Some(ago(180)),
                ..about(
                    "web-a",
                    Severity::Warn,
                    "image pull failed",
                    "registry said no",
                )
            },
        ],
        affected: 1,
        ..group()
    };
    let drawn = pinned_on(Tab::Logs, &card, "web-a");
    let banded: Vec<String> = rows(&drawn)
        .iter()
        .filter(|line| line.contains("payments/web  ·"))
        .map(|line| celled(line))
        .collect();

    assert_eq!(
        banded.len(),
        2,
        "two findings pinned {} blocks",
        banded.len()
    );
    assert!(
        banded[0].starts_with("  ● ") && banded[1].starts_with("  ▲ "),
        "a block wore the card's band instead of its own finding's: {banded:?}"
    );
    // **The age is the block's own finding's too.** `Card::age` is the newest drawable across the
    // whole card, which is one answer — so read off the card these two blocks would both have said
    // `3 min ago`, and the four-day-old one would have been dated by the other.
    assert!(
        banded[0].ends_with("4 days ago") && banded[1].ends_with("3 min ago"),
        "a block wore the card's age instead of its own finding's: {banded:?}"
    );
}

/// `screens/widgets.md` § 2 — **the bar appears once the list is taller than the room left for it,
/// never before**, and **no row gives up a column when it does.**
///
/// The bar draws in the two-column right margin the rows already keep — the head row's own — so a
/// fact sized to the exact width the pane leaves is drawn whole whether the list scrolls or not.
/// That is what stops the same finding's title rewrapping between this step and the tab `⏎` opens
/// from it (`k8s-admin`, 2026-09-18).
#[test]
fn the_bar_appears_only_once_the_list_overflows_and_costs_no_row_a_column() {
    // 33 columns: `region(53) − GUTTER(2) − NAMES_GAP(3) − the 15-column names` — the whole of what
    // a row has for its fact, at this pane's own fixed width.
    let title = "nothing scheduled this Pod at all";
    assert_eq!(width(title), 33, "the fixture is the boundary");
    let step = |pods: usize| {
        let findings: Vec<Finding> = (0..pods)
            .map(|nth| {
                about(
                    &format!("log-shipper-{nth:03}"),
                    Severity::Critical,
                    title,
                    "",
                )
            })
            .collect();
        let card = Card {
            owner: id(ObjectKind::DaemonSet, Some("payments"), "log-shipper"),
            findings,
            affected: pods,
            total: Some(40),
        };
        stepping(&app(), &card)
    };
    let barred = |drawn: &Buffer| {
        rows(drawn)
            .iter()
            .any(|line| line.contains('█') || line.contains('║'))
    };

    // The block is 2 rows and the blank under it a third, so 12 pods fill the 15 the step leaves.
    let drawn = step(12);
    assert_eq!(
        celled(&row(&drawn, "log-shipper-011")),
        format!("  ● log-shipper-011   {title}")
    );
    assert!(
        !barred(&drawn),
        "a list that fits drew a bar over it\n{}",
        rows(&drawn).join("\n")
    );

    let drawn = step(13);
    assert!(
        barred(&drawn),
        "a list one row too tall drew no bar\n{}",
        rows(&drawn).join("\n")
    );
    // **`starts_with` and not equality**: the bar's own glyph is in the margin beyond the row's
    // text, which is the point — the fact is still whole at its full 33 columns.
    let narrowed = celled(&row(&drawn, "log-shipper-000"));
    assert!(
        narrowed.starts_with(&format!("▸ ● log-shipper-000   {title}")),
        "the bar cost a row a column it should have taken from the margin: {narrowed:?}"
    );
}

/// `screens/detail.md` § Picking a pod — **the trailing fact is whole where it fits and back-cut
/// where it does not**, at the exact column the floor leaves it.
///
/// A long name caps the name column at `region − GUTTER − NAMES_GAP − FACT_FLOOR`, so the fact has
/// exactly [`FACT_FLOOR`] columns: a 20-column title draws whole and a 21-column one gives way
/// (`just mutants-diff`, 2026-09-18 — `>` for `>=` here survived until this test).
#[test]
fn a_fact_that_exactly_fills_its_column_is_drawn_whole_and_one_column_more_gives_way() {
    let long = "checkout-worker-service-canary-7d9f4bc86d-x2k9p";
    let said = |title: &str| {
        let card = Card {
            findings: vec![about(long, Severity::Critical, title, "limit 256Mi")],
            affected: 1,
            ..group()
        };
        celled(&row(&stepping(&app(), &card), "x2k9p"))
    };

    assert_eq!(
        width("the image is missing"),
        20,
        "the fixture is the boundary"
    );
    assert!(
        said("the image is missing").ends_with("the image is missing"),
        "a fact that fits exactly was cut: {:?}",
        said("the image is missing")
    );
    assert_eq!(width("this image is missing"), 21);
    // **It gives way at a word boundary and not at a character** — `cut` walks back to one, which
    // is the same back-cut the container picker's state word takes, so the column it ends in is
    // not the point; that it ends in a `…` and lost a word is.
    assert!(
        said("this image is missing").ends_with("this image is…"),
        "a fact one column over did not give way: {:?}",
        said("this image is missing")
    );
}

/// `screens/detail.md` § Picking a pod — **rows sort by severity band and then by name, never by
/// recency** (NOTES § D270).
///
/// **Recency is what this asserts is *not* used**: the newer finding arrives first and carries the
/// later name, so a list ordered by recency and a list ordered by arrival both put it first, and
/// only a name order puts it last. Every restart of any pod in a 38-pod group moves that pod's
/// stamp, and a list that re-sorts under a reader mid-scan is one they cannot work through.
#[test]
fn rows_fall_in_name_order_inside_a_band_and_never_in_recency_order() {
    let card = Card {
        findings: vec![
            Finding {
                timestamp: Some(at(180)),
                ..about("web-zulu", Severity::Critical, "newer", "exit 137")
            },
            Finding {
                timestamp: Some(at(0)),
                ..about("web-alpha", Severity::Critical, "older", "exit 137")
            },
            // A different band always wins, whatever the names do.
            Finding {
                timestamp: Some(at(90)),
                ..about("web-aaa", Severity::Warn, "a warning", "nothing much")
            },
        ],
        affected: 3,
        ..group()
    };
    let seen = rows(&stepping(&app(), &card));
    let at_row = |needle: &str| {
        seen.iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", seen.join("\n")))
    };
    assert!(
        at_row("web-alpha") < at_row("web-zulu"),
        "the newer pod sorted above the earlier name\n{}",
        seen.join("\n")
    );
    assert!(
        at_row("web-zulu") < at_row("web-aaa"),
        "a warning sorted above a critical on its name\n{}",
        seen.join("\n")
    );
}

/// `screens/alerts.md` § A card with more than one finding — **recency still orders the blocks a
/// tab pins**, which is the one level down the step's own name order does not reach
/// ([`decides`], `screens/detail.md` § Picking a pod's last clause).
///
/// **Two findings on one pod at one severity is what proves it** — with a band between them the
/// severity key decides and recency is never asked (`just mutants-diff`, 2026-09-18: a `drawable`
/// that always answered `None` survived every other test in this file).
#[test]
fn two_findings_on_one_pod_at_one_severity_pin_newest_first() {
    let card = Card {
        findings: vec![
            Finding {
                timestamp: Some(ago(4 * 24 * 60 * 60)),
                ..about("web-a", Severity::Critical, "the older reason", "exit 137")
            },
            Finding {
                timestamp: Some(ago(180)),
                ..about("web-a", Severity::Critical, "the newer reason", "exit 139")
            },
        ],
        affected: 1,
        ..group()
    };
    let seen = rows(&pinned_on(Tab::Logs, &card, "web-a"));
    let at_row = |needle: &str| {
        seen.iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("no row holds {needle:?}\n{}", seen.join("\n")))
    };
    assert!(
        at_row("the newer reason") < at_row("the older reason"),
        "the older finding pinned above the newer one\n{}",
        seen.join("\n")
    );
}

/// `screens/detail.md` § A pod's own findings, not the whole card's — **a tab pins the findings
/// filed against the pod it is open on, and never the other 37.**
///
/// A DaemonSet card built from a rule that fires once per pod holds one `Finding::object` per pod
/// and one shared `Finding::owner` (NOTES § D3). Opening pod #7 pins pod #7's finding; the other
/// 37 pods' reasons are the which-pods step's own rows, each carrying its own.
///
/// **An owner-level finding pins on every pod's tab**, because it names no pod to prefer — the
/// W1/W2 shape, the second half of the predicate and the one a `object ==` test alone would miss.
#[test]
fn a_tab_pins_the_findings_about_its_own_pod_and_the_owners_own_but_not_the_groups() {
    let owner = id(ObjectKind::DaemonSet, Some("payments"), "log-shipper");
    let mut findings: Vec<Finding> = (0..38)
        .map(|nth| Finding {
            owner: owner.clone(),
            ..about(
                &format!("log-shipper-{nth:03}"),
                Severity::Critical,
                "cannot be scheduled",
                &format!("log-shipper-{nth:03} could not be placed on any node"),
            )
        })
        .collect();
    // The owner-level finding: its own `object` is the DaemonSet, so it names no pod at all.
    findings.push(Finding {
        owner: owner.clone(),
        object: owner.clone(),
        ..about(
            "unused",
            Severity::Warn,
            "fewer pods are running than this DaemonSet asks for",
            "38 of 40 not running",
        )
    });
    let card = Card {
        owner,
        findings,
        affected: 38,
        total: Some(40),
    };

    let drawn = pinned_on(Tab::Logs, &card, "log-shipper-007");
    let seen = rows(&drawn);

    assert!(
        holds(&drawn, "log-shipper-007 could not be placed"),
        "the tab lost the reason its own pod was opened\n{}",
        seen.join("\n")
    );
    assert!(
        holds(&drawn, "fewer pods are running"),
        "an owner-level finding names no pod to prefer and must pin on every pod's tab\n{}",
        seen.join("\n")
    );
    for other in ["log-shipper-000", "log-shipper-006", "log-shipper-008"] {
        assert!(
            !holds(&drawn, &format!("{other} could not be placed")),
            "{other}'s finding was pinned on log-shipper-007's tab\n{}",
            seen.join("\n")
        );
    }
    // **Two blocks, counted** — one per identity line — because *not the other 37* has to be a
    // claim about how many pinned, not only about which three this loop happened to name.
    //
    // **Matched on the owner's name with a space after it**, which the heading
    // `payments/log-shipper-007` does not have. The `·  38 of 40 pods` fragment is not on these
    // lines to match on: [`identity`]'s own give-way order drops the count before the name clips,
    // and this card's age takes the room (`screens/alerts.md` § The age, and what it costs the
    // name) — unchanged by this box, and the reason a `· n of m` needle would have found nothing.
    assert_eq!(
        seen.iter()
            .filter(|line| line.contains("payments/log-shipper "))
            .count(),
        2,
        "the tab pinned a block per pod in the group\n{}",
        seen.join("\n")
    );
}

/// **A block on the logs tab is laid out at the pane's own width and drawn at it** — the one
/// `leads` call site whose caller had already padded, so the block was drawn four columns narrower
/// than it was wrapped for: the age's own `ago` and the title's last word gone, with **no `…`
/// anywhere** (`tester` and `k8s-admin`, 2026-09-18, independently).
///
/// **It lands on exactly the pods this step exists for.** An `ImagePullBackOff` or `Pending` pod
/// has never written a log line, logs is the tab `⏎` lands on, and on them this branch is
/// permanent rather than the moment before a stream starts.
#[test]
fn a_block_over_a_log_that_never_started_is_drawn_at_the_width_it_was_wrapped_for() {
    let card = Card {
        findings: vec![about(
            "web-7d9f4bc86d-x2k9p",
            Severity::Critical,
            "The image could not be pulled from the registry at all",
            "registry refused the pull",
        )],
        affected: 1,
        ..group()
    };
    let (pod, names) = declared_by("pending");
    let containers = paired(&pod, &names);
    let read = Described {
        snapshot: &pod,
        containers: &containers,
    };
    let empty = crate::k8s::LogLines::default();
    let mut open = Open::new();
    open.object = id(ObjectKind::Pod, Some("payments"), "web-7d9f4bc86d-x2k9p");
    open.card = Some(card);
    open.logs = Pane::Ready(Logs {
        pod: &read,
        container: "app",
        previous: false,
        held: &empty,
    });
    let drawn = detailed(&on(Tab::Logs), &open.open());
    let seen = rows(&drawn);

    // **The identity line keeps its whole age.** Cut four columns short it read `1014 days`, which
    // is a different fact, not a shortened one — and nothing said it had been cut.
    let identity = celled(&row(&drawn, "payments/web  ·"));
    assert!(
        identity.ends_with("ago"),
        "the block was drawn narrower than it was laid out for: {identity:?}"
    );
    // **The title is drawn whole**, over as many rows as it wraps to. Four columns short it lost
    // its last word with nothing to say so — the silent cut `screens/widgets.md` § 7 forbids.
    let said = words(
        &seen
            .iter()
            .map(|line| celled(line))
            .collect::<Vec<_>>()
            .join(" "),
    );
    assert!(
        said.contains("The image could not be pulled from the registry at all"),
        "a word went missing with no mark on it\n{}",
        seen.join("\n")
    );
    // And the tab's own sentence is still there, under the block.
    assert!(holds(&drawn, "no logs yet"), "{}", seen.join("\n"));
}

/// `screens/detail.md` § When the stack is taller than a Loading or Empty tab has anything of its
/// own — **a block past the pane's room never erases the tab's own sentence, never cuts itself
/// unmarked, and never puts a row out of reach.**
///
/// **Measured, not reasoned** (`k8s-admin`, 2026-09-18,
/// `reports/2026-09-18-the-which-pods-step.md` § 4): clamped to the whole pane, `still loading` and
/// `none right now` came out byte-identical but for which tab was marked open — which is exactly
/// the collapse PRIOR-ART § C2 is tagged *covered* in this repo for avoiding.
#[test]
fn a_block_taller_than_the_pane_keeps_the_tabs_own_sentence_and_scrolls_for_the_rest() {
    // Two findings on one pod, the second quoting a seven-line `runc` error — a real shape, and
    // 14 rows against the 13 a tab's body has at the floor.
    let tail = "and-then-it-gave-up-entirely";
    let card = Card {
        findings: vec![
            about("web-a", Severity::Critical, "ran out of memory", "exit 137"),
            about(
                "web-a",
                Severity::Critical,
                "The container could not be started at all",
                &format!(
                    "starting container process caused: exec: \"/usr/local/bin/serve\": stat \
                     /usr/local/bin/serve: no such file or directory, unknown, {tail}"
                ),
            ),
        ],
        affected: 1,
        ..group()
    };

    let mut open = Open::new();
    open.object = id(ObjectKind::Pod, Some("payments"), "web-a");
    open.card = Some(card);
    let held = open.open();

    // **The sentence is drawn, in its own words** — this is the property that keeps loading and
    // empty two frames and not one.
    let loading = detailed(&on(Tab::Describe), &held);
    assert!(
        holds(&loading, "reading the cluster…"),
        "the block erased the tab's own sentence\n{}",
        rows(&loading).join("\n")
    );
    // **And a bar says the rest is there**, rather than the block ending mid-quote with no mark.
    assert!(
        rows(&loading)
            .iter()
            .any(|line| line.contains('█') || line.contains('║')),
        "a block past the pane's room drew no scrollbar\n{}",
        rows(&loading).join("\n")
    );

    // **`app.scroll` reaches every line of it** — nothing is discarded to make it fit, only
    // deferred behind a mark the reader can see.
    let scrolled = detailed(
        &App {
            tab: Tab::Describe,
            scroll: offsets(Tab::Describe, 8),
            ..App::default()
        },
        &held,
    );
    assert!(
        holds(&scrolled, tail),
        "the block's own tail was unreachable\n{}",
        rows(&scrolled).join("\n")
    );

    // **The block's own right edge is the width it was wrapped at, bar or no bar.** [`identity`]
    // lays its line out to exactly the region whenever the finding has a drawable age, so a block
    // one column narrower loses `ago` off the end of every one of them — with no `…`, in the only
    // state this path exists for (`k8s-admin`, 2026-09-18, § 8; the bar's column now comes off the
    // pane's own margin, as `pod_pick` already took it).
    // The bar's own glyph sits in the margin past the block's text, which is the point — so it is
    // stripped before the block's own right edge is read.
    let identity = celled(&row(&loading, "payments/web  ·"));
    let block = identity.trim_end_matches(['█', '║']).trim_end();
    assert!(
        block.ends_with("ago"),
        "the bar took its column out of the block: {identity:?}"
    );

    // **Loading and nothing-to-show are two frames of *one* tab.** Compared across two different
    // tabs the marked tab row alone makes them differ, so that comparison would pass with both
    // sentences erased — it is the two `holds` that pin PRIOR-ART § C2, and this is the
    // comparison that adds anything (`k8s-admin`, 2026-09-18).
    let mut empty = Open::new();
    empty.object = id(ObjectKind::Pod, Some("payments"), "web-a");
    empty.card = open.card.clone();
    empty.events = Pane::Ready(crate::k8s::Happened::default());
    let none = detailed(&on(Tab::Events), &empty.open());
    let waiting = detailed(&on(Tab::Events), &held);
    assert!(
        holds(&none, "none right now"),
        "the block erased the empty sentence\n{}",
        rows(&none).join("\n")
    );
    assert!(holds(&waiting, "reading the cluster…"));
    assert_ne!(
        rows(&waiting),
        rows(&none),
        "one tab's still-loading and nothing-to-show came out the same frame"
    );

    // **And the empty sentence is drawn whole** — `views::NO_EVENTS` is two lines at this width,
    // and a fixed three-row reservation spent one on the headline and one on the blank, cutting
    // the half that says *why* with no mark (`k8s-admin`, 2026-09-18, § 10).
    let said = words(
        &rows(&none)
            .iter()
            .map(|line| celled(line))
            .collect::<Vec<_>>()
            .join(" "),
    );
    assert!(
        said.contains(views::NO_EVENTS),
        "the empty sentence lost the half that says why\n{}",
        rows(&none).join("\n")
    );

    // **A refusal is the third state and this box changed nothing about it** — its banner is
    // drawn before any block, off `floor`, and a pinned finding never explains why a read failed.
    let mut denied = Open::new();
    denied.object = id(ObjectKind::Pod, Some("payments"), "web-a");
    denied.card = open.card.clone();
    denied.events = Pane::Denied(
        "you are not allowed to read this. Ask for `get` on events in payments.".to_owned(),
        crate::k8s::Happened::default(),
    );
    let refused = detailed(&on(Tab::Events), &denied.open());
    let told = words(
        &rows(&refused)
            .iter()
            .map(|line| celled(line))
            .collect::<Vec<_>>()
            .join(" "),
    );
    assert!(
        told.contains("Ask for `get` on events in payments."),
        "a block pushed a refusal's own reason behind a scroll\n{}",
        rows(&refused).join("\n")
    );
    assert_ne!(
        rows(&refused),
        rows(&none),
        "denied and empty are one frame"
    );
    assert_ne!(
        rows(&refused),
        rows(&waiting),
        "denied and still loading are one frame"
    );
}

/// **The slot carries whether `⏎` reached these tabs through the which-pods step, and hands it to
/// `crate::views::Detailing` unchanged** (NOTES § D270).
///
/// **Nothing on this screen draws from it and that is the point**: what `esc` out of a tab reached
/// that way should do is the wiring box's to settle with `tui-designer`. It is carried now because
/// `views.rs` freezes at this phase's close and that box is later in the same phase, and without
/// it the router's only options are a private flag in `main.rs` about what `views.rs`'s state
/// means or reopening a frozen file (`k8s-admin`, 2026-09-18).
#[test]
fn the_slot_carries_whether_the_tabs_were_reached_through_the_step() {
    let open = Open::new();
    let held = open.open();
    let alerts = Pane::Ready(vec![oom()]);
    let now = later();

    for from_step in [false, true] {
        let mut screen = screen(&alerts, &now);
        screen.detail = Some(Detailed::Tabs {
            open: &held,
            from_step,
        });
        assert_eq!(
            detailing(&screen),
            Detailing::Tabs {
                containers: containers(&screen).len(),
                from_step,
            },
            "the slot dropped how the tabs were reached"
        );
        // **And the footer is the same line either way** — the behaviour is not this box's.
        let (keys, _) = App::default().footer(
            detailing(&screen),
            views::Offer::Move { switch: false },
            Refused::default(),
            "",
            &[],
        );
        assert!(keys.contains("esc back"), "{keys:?}");
    }
}
