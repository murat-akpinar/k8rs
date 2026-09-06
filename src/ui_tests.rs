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
use crate::rules::{Finding, ObjectId, ObjectKind};
use crate::views::Group;
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

/// The browser pane a test that is not about the browser hands over: still loading, which is
/// what a view nobody opened has answered.
static UNOPENED: Pane<crate::k8s::Table> = Pane::Loading;

fn screen<'a>(alerts: &'a Pane<Vec<Card>>, now: &'a Time) -> Screen<'a> {
    Screen {
        depth: Depth::TrueColor,
        vitals: "nodes 3/3",
        context: "ctx: prod-eu · live · admin",
        alerts,
        browser: &UNOPENED,
        namespace: None,
        now,
        note: &[],
        kinds: &[],
        reports: &[],
        log: &[],
        keys: "↑↓ move  ⏎ open  s scale  r restart  l logs  ? all keys  q quit",
    }
}

fn render_at(columns: u16, rows: u16, app: &App, screen: &Screen) -> Buffer {
    let mut terminal =
        Terminal::new(TestBackend::new(columns, rows)).expect("a terminal over a test backend");
    terminal
        .draw(|frame| draw(frame, app, screen))
        .expect("a frame");
    terminal.backend().buffer().clone()
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

fn holds(buffer: &Buffer, needle: &str) -> bool {
    rows(buffer).iter().any(|line| line.contains(needle))
}

/// The pane half of a row — the sidebar and the divider dropped — so a column assertion counts
/// from the content pane's own left edge. 22 is `1` border + [`SIDEBAR`] + `1` divider.
fn pane(line: &str) -> String {
    line.chars().skip(usize::from(1 + SIDEBAR + 1)).collect()
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
    let log = [long.to_owned(), long.to_owned()];
    let mut wide = screen(&alerts, &now);
    wide.log = &log;
    wide.keys = long;
    let drawn = render(&app(), &wide);
    let rows = rows(&drawn);

    for nth in [19, 20, 22] {
        assert!(rows[nth].starts_with('│'), "row {nth}: {:?}", rows[nth]);
        assert!(rows[nth].ends_with('│'), "row {nth}: {:?}", rows[nth]);
    }
}

/// **`k8rs` is drawn only with two blank columns each side, and the boundary is where the
/// mutants live.** At 80 columns the centred name starts at 38, so a left zone of 35 keeps it and
/// one of 36 loses it; on the other side a context of 36 keeps it and one of 37 loses it. The
/// name is the only zone that gives way — never the context.
#[test]
fn the_name_needs_two_blank_columns_on_each_side() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let header = |screen: &Screen| rows(&render(&app(), screen))[0].clone();

    let thirty_five = "n".repeat(35);
    let mut roomy = screen(&alerts, &now);
    roomy.vitals = &thirty_five;
    assert!(
        header(&roomy).contains("k8rs"),
        "a 35-column left zone fits"
    );

    let thirty_six = "n".repeat(36);
    let mut tight = screen(&alerts, &now);
    tight.vitals = &thirty_six;
    assert!(!header(&tight).contains("k8rs"), "a 36-column one does not");

    let last = format!("ctx: {}", "c".repeat(31));
    let mut roomy = screen(&alerts, &now);
    roomy.context = &last;
    assert_eq!(width(&last), 36);
    assert!(
        header(&roomy).contains("k8rs"),
        "a 36-column context still leaves two blanks"
    );

    let over = format!("ctx: {}", "c".repeat(32));
    let mut tight = screen(&alerts, &now);
    tight.context = &over;
    assert_eq!(width(&over), 37);
    let drawn = header(&tight);
    assert!(
        !drawn.contains("k8rs"),
        "and a 37-column one does not: {drawn:?}"
    );
    assert!(
        drawn.ends_with(&over),
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
    wide.context =
        "ctx: a-very-long-context-name-indeed · ns: payments · read-only · ⚠ TLS not verified";
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
               · ns: payments · live · read-only · ⚠ TLS not verified";
    let mut screen = screen(&alerts, &now);
    screen.context = arn;
    assert_eq!(width(arn), 116, "wider than the row, by half again");

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
    for columns in 0..=width(arn) {
        let zone = shortened(arn, columns);
        assert!(
            width(&zone) <= columns,
            "{columns} columns asked for, {} drawn: {zone:?}",
            width(&zone)
        );
    }
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
    screen.vitals = stale;
    screen.context = "ctx: prod-eu · ns: payments · live · read-only · ⚠ TLS not verified";
    assert_eq!(
        width(screen.context),
        67,
        "which leaves 11 columns of left zone"
    );
    assert_eq!(width(stale), 19, "and the vitals want 19");

    let drawn = render(&app(), &screen);
    let header = rows(&drawn)[0].clone();
    assert!(!header.contains("nodes"), "no half a vital: {header:?}");

    screen.vitals = "nodes 3/3";
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
        "$ kubectl get certificatesigningrequests".to_owned(),
        "$ kubectl get pods -A --watch".to_owned(),
        "$ kubectl get nodes --watch".to_owned(),
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
    let overcommitted = Badge {
        value: "1".to_owned(),
        severity: Severity::Warn,
    };
    let expiring = Badge {
        value: "30d".to_owned(),
        severity: Severity::Warn,
    };
    let reports = [
        ("capacity", Some(&overcommitted)),
        ("certificates", Some(&expiring)),
        ("drain safety", None),
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
        pane(&line).starts_with(&format!("  ▲ {long}")),
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
    let line = row(&drawn, "ip-10-0-134");

    assert!(
        line.ends_with("4 min ago  │"),
        "the age keeps its column: {line:?}"
    );
    assert!(
        !line.contains(long),
        "and the 42-column name did not fit beside it: {line:?}"
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
    let note = ["84 pods and 3 nodes checked, none of them is in trouble right now.".to_owned()];

    let loading = Pane::Loading;
    let mut still = screen(&loading, &now);
    let reading = ["reading the cluster… 2,140 pods".to_owned()];
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
        "84 pods and 3 nodes checked.".to_owned(),
        "Worth a look anyway:".to_owned(),
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

/// A kind as discovery describes it. **Only `plural` and `namespaced` are read by the renderer**,
/// which is the point — the rest is what a URL is built from, one layer down.
fn browsable(plural: &str, namespaced: bool) -> Browsable {
    Browsable {
        group: "example.com".to_owned(),
        version: "v1".to_owned(),
        kind: "Whatever".to_owned(),
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
        screen.namespace = scope;
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
    screen.namespace = Some(long);
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
    room.namespace = Some("payments");
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
        screen.namespace = Some(&namespace);
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
    scoped.namespace = Some("kube-system");
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
    scoped.namespace = Some("payments");
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
    let short = cut("aa bb cc dd", 5);
    assert_eq!(short, ["aa bb", "cc dd"]);

    let long = cut("aaaa bbbb cccc dddd eeee ffff gggg hhhh", 10);
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
    let full = cut("aaaaa bbbb ccccc dddd eeeee ffff ggggg", 10);
    assert_eq!(full, ["aaaaa bbbb", "ccccc dddd", "eeeee…"]);
    for line in &full {
        assert!(width(line) <= 10, "{full:?}");
    }
}

// --- WHAT THE SCREEN ACTUALLY LOOKS LIKE ---

/// Not an assertion about a column — the whole screen, printed, so a reader of the report can
/// compare it with `screens/alerts.md` line by line. `cargo test -- --nocapture`.
#[test]
fn the_alerts_screen_at_the_floor() {
    let now = now();
    let cards = vec![oom(), cordon(Some(at(0))), cordon(None)];
    let alerts = Pane::Ready(cards);
    let capacity = Badge {
        value: "1".to_owned(),
        severity: Severity::Warn,
    };
    let certificates = Badge {
        value: "30d".to_owned(),
        severity: Severity::Warn,
    };
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
        "$ kubectl get pods -A --watch".to_owned(),
        "$ kubectl get nodes --watch".to_owned(),
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
    let log = ["$ kubectl get deployments -n payments".to_owned()];

    for (fixture, at, namespace) in [
        ("table-deployments", 0, Some("payments")),
        ("table-pods", 3, None),
    ] {
        let ready = Pane::Ready(table(fixture));
        let mut screen = browsing(&ready, &workloads, &now);
        screen.namespace = namespace;
        screen.log = &log;
        screen.keys = "↑↓ move  ⏎ open  s scale  r restart  ctrl-d delete  / filter";

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
    let log = ["$ kubectl get pods -A".to_owned()];
    let mut screen = bleeding(&ready, &alerts, &kinds, &now);
    screen.log = &log;
    screen.keys = "↑↓ move  ⏎ open  s scale  r restart  ctrl-d delete  / filter";

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
