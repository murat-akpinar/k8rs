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
        refused: Refused::default(),
        writes: Writes::Live,
        clock: None,
        link: Link::Live,
        detail: None,
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
    guarded.context = "ctx: prod-eu · read-only · ⚠ TLS not verified";
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
               · ns: payments · live · read-only · ⚠ TLS not verified";
    let mut screen = screen(&alerts, &now);
    screen.context = arn;

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
    let long = [
        "$ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst \
         -n payments-prod0 -o yaml --show-managed-fields"
            .to_owned(),
    ];
    assert_eq!(width(&long[0]), 106);
    let mut with_log = screen(&alerts, &now);
    with_log.log = &long;
    let drawn = render(&app(), &with_log);
    let line = row(&drawn, "kubectl get pod");
    println!("{line}");

    assert!(line.contains(CUT), "the cut is not marked: {line:?}");
    assert!(
        !line.contains("-n payments-prod0"),
        "the drawn line is a whole, different, working command: {line:?}"
    );
    assert!(
        line.contains("$ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n\u{2026}"),
        "the head is kept and the token that did not fit gave way whole: {line:?}"
    );
}

/// **The one line in `screens/` that does not fit even at the true 76-column floor**, drawn byte
/// for byte as `screens/detail.md` § A Secret's values, hidden behind an explicit reveal draws it:
/// the flag gives way whole, the mark lands right after `yaml`, and what is left of it is
/// **deliberately** a real command — the `kubectl get -o yaml` a reader already gets by default —
/// which is the trade that section makes and states.
#[test]
fn the_yaml_tabs_secret_line_is_cut_exactly_where_the_mockup_draws_it() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let secret = [views::yaml_line(
        "secret",
        &ObjectId {
            kind: ObjectKind::Pod,
            namespace: Some("payments".to_owned()),
            name: "db-credentials".to_owned(),
            uid: None,
        },
    )];
    assert_eq!(width(&secret[0]), 77, "77 columns against the strip's 76");
    let mut with_log = screen(&alerts, &now);
    with_log.log = &secret;
    let drawn = render(&app(), &with_log);
    let line = row(&drawn, "kubectl get secret");
    println!("{line}");
    assert!(
        line.contains("$ kubectl get secret db-credentials -n payments -o yaml\u{2026}"),
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
    let exact = [
        "$ kubectl get pod checkout-api-canary-7d9f4bc86d-x2k9pqrst -n payments-prod0".to_owned(),
    ];
    assert_eq!(width(&exact[0]), 76);
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

/// **The same narrow end for [`name_cut`], and it is a column tighter** — the `…/` it glues on is
/// two columns where [`clipped`]'s mark is one, so its guard is `>`, not `>=`, and below two
/// columns there is no room to say *namespaced* at all and the ordinary clip is what draws.
///
/// **Found by a mutation run, exactly as its sibling above was** (2026-09-12): `>=` survived every
/// test in this file and underflowed a column budget one column in. The product's own room here is
/// 33 at the 80×24 floor, so this is a guard on a helper rather than a state anything draws — and
/// a guard whose boundary has never been drawn is not one.
///
/// **Both of rule 3's branches are walked here**, because the boundary between them is arithmetic
/// on the same budget: at 13 columns `web` still fits and it is the namespace's front that goes,
/// at 3 there is room for neither and the name is cut too.
///
/// **Nothing ever comes back wider than it was asked for**, which is the property the whole
/// function exists to keep and the one another branch could silently break.
#[test]
fn the_slash_is_kept_only_while_there_are_two_columns_to_keep_it_in() {
    assert_eq!(name_cut("payments/web", 12), "payments/web");
    assert_eq!(name_cut("payments/web", 10), "payments/\u{2026}");
    assert_eq!(name_cut("payments/web", 9), "\u{2026}ents/web");
    assert_eq!(name_cut("payments/web", 6), "\u{2026}s/web");
    assert_eq!(name_cut("payments/web", 5), "\u{2026}/web");
    assert_eq!(name_cut("payments/web", 4), "\u{2026}/w\u{2026}");
    assert_eq!(name_cut("payments/web", 3), "\u{2026}/\u{2026}");
    assert_eq!(name_cut("payments/web", 2), "\u{2026}/");
    assert_eq!(name_cut("payments/web", 1), "\u{2026}");
    assert_eq!(name_cut("payments/web", 0), "");
    for columns in 0..=13 {
        for name in ["payments/web", "node-3", "/web", "payments/"] {
            let drawn = name_cut(name, columns);
            assert!(
                width(&drawn) <= columns,
                "{name:?} at {columns} drew {drawn:?}, {} columns",
                width(&drawn)
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
    for row in pane
        .iter()
        .take_while(|row| !row.starts_with('●') && !row.starts_with('▲'))
    {
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
fn fed(section: &str, nth: usize, from: usize) -> Vec<String> {
    said_above(&mockups(section)[nth].pane).split_off(from)
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
/// a footer today, and each one is either drawn here or named below with the test that owns it.
#[test]
fn every_state_draws_the_body_and_the_footer_its_own_mockup_gives_it() {
    let now = now();
    let mut seen: Vec<(&str, usize)> = Vec::new();

    // § Still loading — nothing has arrived, so there is nothing to move across, open or narrow.
    // The count in the sentence is the store's; `reading the cluster…` is `ui::note`'s own.
    let section = "## Still loading";
    let loading = Pane::Loading;
    let reading = fed(section, 0, 0);
    let mut still = screen(&loading, &now);
    still.note = &reading;
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

    // § The connection dropped — a stale card is still a selected object in principle, and k8rs
    // cannot ask whether a write would be allowed, so the two keys go rather than being marked.
    let section = "## The connection dropped";
    let dropped = Pane::Denied(fed(section, 0, 0).join("\n\n"), vec![oom()]);
    let mut lost = screen(&dropped, &now);
    lost.link = Link::Lost;
    against(section, 0, &app(), &lost);
    seen.push((section, 0));

    // § Your login expired — the one state on the page that promotes a key off `?`.
    let section = "## Your login expired";
    let timed_out = Pane::Denied(fed(section, 0, 0).join("\n\n"), vec![oom()]);
    let mut expired = screen(&timed_out, &now);
    expired.link = Link::Expired;
    against(section, 0, &app(), &expired);
    seen.push((section, 0));

    // § Over a pane with nothing to show yet — the same expired login over the two panes that have
    // no card to relabel as stale. **`X` is promoted wherever the link is `Link::Expired`, whatever
    // the pane under it is drawing**, which is what separates it from `s` and `r`: it never acted
    // on a selected object, so *nothing is selected* was never its condition. Each pane otherwise
    // keeps its own shape — Still loading still drops the cursor keys, the empty kind still keeps
    // `/ filter`. **Each frame is fed its own mockup and not the one it resembles**, so an edit to
    // either drawing arrives here rather than being masked by the section it was copied from.
    let waited = fed(section, 1, 0);
    let mut waiting = screen(&loading, &now);
    waiting.note = &waited;
    waiting.link = Link::Expired;
    against(section, 1, &app(), &waiting);
    seen.push((section, 1));

    let named = said_above(&mockups(section)[2].pane);
    let open_kind = [browsable(&named[0], true)];
    let mut bare = browsing(&none, &open_kind, &now);
    bare.link = Link::Expired;
    against(section, 2, &opened(), &bare);
    seen.push((section, 2));

    // § Your computer's clock is off — a banner over a *live* list: the header still reads
    // `live · admin`, so this is neither the connection's reason nor a permission's, and the keys
    // go because an age that reads fresher than it is decides which card somebody acts on first.
    let section = "## Your computer's clock is off";
    let live = Pane::Ready(vec![oom()]);
    let behind = fed(section, 0, 0).join("\n\n");
    let mut skewed = screen(&live, &now);
    skewed.clock = Some(&behind);
    against(section, 0, &app(), &skewed);
    seen.push((section, 0));

    // § Ahead of the cluster — the other direction, and a second sentence rather than the same one
    // with a sign flipped: this one loses no times, it only makes them read large (NOTES § D177).
    let ahead = fed(section, 1, 0).join("\n\n");
    let mut fast = screen(&live, &now);
    fast.clock = Some(&ahead);
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
    let scoped = Pane::Denied(fed(section, 0, 0).join("\n\n"), vec![oom()]);
    let partial = screen(&scoped, &now);
    against(section, 0, &app(), &partial);
    seen.push((section, 0));

    // § Nothing broken, and something not checked — the same sentence with no list under it, which
    // is the one screen where silence and *nothing is broken* would look identical. The check that
    // could not run says so beneath the verdict rather than instead of it.
    let unchecked = fed(section, 1, 1);
    let mut said_anyway = screen(&empty, &now);
    said_anyway.note = &unchecked;
    against(section, 1, &app(), &said_anyway);
    seen.push((section, 1));

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
        frames, 20,
        "screens/states.md draws {frames} screens with a footer, not the 20 this sweep was \
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
        "↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit",
        "a pane with a card in it, the link up, writes live and every time on it trustworthy"
    );
    assert!(
        app().may_mutate(offered(&app(), &ordinary)),
        "the ordinary screen refused the key it had just drawn"
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
        "↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit",
        "a browser pane with rows in it:\n{}",
        rows(&drawn).join("\n")
    );
    assert!(
        opened().may_mutate(offered(&opened(), &listing)),
        "the browser refused the key it had just drawn"
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
    skewed.clock = Some("⚠ This computer and the cluster disagree about the time by 11 minutes.");
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
        assert!(
            !app().may_mutate(offered(&app(), screen)),
            "{cause}: the key was off the line and live behind it"
        );
    }
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
    developer.namespace = Some("payments");
    let drawn = render(&app(), &developer);
    println!("{}", rows(&drawn).join("\n"));
    assert_eq!(
        unframed(&rows(&drawn)[22]),
        "↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit",
        "a refusal that came back with cards is a selection, not a dead link"
    );
    assert!(
        app().may_mutate(offered(&app(), &developer)),
        "a namespace-scoped login could not reach the key its own grant allows"
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
        handed.len() == 2 && handed[1].starts_with("Worth a look anyway"),
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
        body_text(&drawn).contains(&words(&handed[0])),
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
        "84 pods checked.".to_owned(),
        "Worth a look anyway.".to_owned(),
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
        reading.detail = Some(&detail);
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
    both.clock = Some(
        "⚠ This computer and the cluster disagree about the time by 11 minutes (this one is \
         behind), so recent times are missing and older ones can read smaller than they really \
         are.",
    );
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
    let clock = fed("## Your computer's clock is off", 0, 0).join("\n\n");
    let scoped = fed("## You can only see some namespaces", 0, 0).join("\n\n");
    let expired = fed("## Your login expired", 0, 0).join("\n\n");
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
    all.clock = Some(&clock);
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
    let clock = fed("## Your computer's clock is off", 0, 0).join("\n\n");
    let dead = dead_log();
    // The real pair, and one that spends the whole 13 so the list is at its 3-row floor whatever
    // width either sentence wraps to.
    let flood = vec!["x".repeat(53); 20].join(" ");
    for (what, audit) in [
        ("clock and the audit sentence", dead.as_str()),
        ("banners that spend all 13 rows", flood.as_str()),
    ] {
        let mut stacked = screen(&cards, &now);
        stacked.clock = Some(&clock);
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
    queued.clock = Some(&clock);
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
    tighter.clock = Some(&clock);
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
        degraded.clock = Some("⚠ This computer and the cluster disagree about the time.");
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
    both.clock = Some("⚠ This computer and the cluster disagree about the time by 11 minutes.");
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
            let head = line.trim_start();
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
    let dropped = Pane::Denied(fed(section, 0, 0).join("\n\n"), vec![oom()]);
    let mut lost = screen(&dropped, &now);
    lost.link = Link::Lost;
    let drawn = render(&app(), &lost);
    println!("{}", rows(&drawn).join("\n"));

    // The column a needle starts at on its own row, pane side and counted in characters — `⚠` is
    // three bytes and a byte offset would land inside it.
    let at = |needle: &str| {
        let line = pane(&row(&drawn, needle));
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
    let timed_out = Pane::Denied(fed(section, 0, 0).join("\n\n"), vec![oom()]);
    let mut expired = screen(&timed_out, &now);
    expired.link = Link::Expired;
    let drawn = render(&app(), &expired);
    println!("{}", rows(&drawn).join("\n"));
    let at = |needle: &str| {
        let line = pane(&row(&drawn, needle));
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
            .filter(|line| line.contains('\u{26a0}'))
            .count(),
        1,
        "the mark is spent once per paragraph, not once per sentence:\n{}",
        rows(&drawn).join("\n")
    );

    // § You can only see some namespaces — no mark, so nothing is spent and nothing hangs.
    let section = "## You can only see some namespaces";
    let scoped = Pane::Denied(fed(section, 0, 0).join("\n\n"), vec![oom()]);
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
    over.detail = Some(&open);

    for (what, app, asked) in [
        ("Analysis", &reporting, &analysis),
        ("a detail tab", &app(), &over),
    ] {
        let offer = offered(app, asked);
        assert!(
            !app.may_mutate(offer),
            "{what} left a key live that its own footer never names — {offer:?}"
        );
        let (keys, _) = app.footer(asked.detail.is_some(), offer, Refused::default(), "");
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
    let log = ["$ kubectl get nodes -o json".to_owned()];
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
    let alerts = Pane::Ready(vec![oom()]);
    let now = later();
    let mut screen = screen(&alerts, &now);
    screen.detail = Some(open);
    render(app, &screen)
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
            scroll: 3,
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
    let wild = detailed(
        &App {
            tab: Tab::Logs,
            scroll: 900,
            ..App::default()
        },
        &open.open(),
    );
    assert!(
        holds(&wild, "line 14"),
        "an offset past the end clamps to it rather than drawing a blank pane"
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
                scroll,
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
            "↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit",
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
/// `ui.rs` passes `screen.detail.is_some()` rather than letting `App` guess.
#[test]
fn a_detail_tab_changes_the_footer_and_only_while_one_is_open() {
    let open = Open::new();
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
            "↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit",
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
        "$ kubectl get statefulsets -A --watch".to_owned(),
        "$ kubectl get daemonsets -A --watch".to_owned(),
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
/// **A second reader rather than a second literal**, for [`mockup`]'s reason: the block is four
/// lines of plain text with no border to strip, and the `Changing things` heading in it is
/// asserted below to be the very row [`mockup`] already returns, so the two readers cannot drift
/// into describing two different screens.
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
        4,
        "screens/help.md § When a key is refused no longer draws the heading and its three keys"
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
fn fenced(file: &str, section: &str) -> Vec<Vec<String>> {
    let path = format!("{}/screens/{file}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the screen file {path} could not be read: {e}"));
    let mut blocks = Vec::new();
    let mut open: Option<Vec<String>> = None;
    for line in text
        .lines()
        .skip_while(|line| *line != section)
        .skip(1)
        .take_while(|line| !line.starts_with("## "))
    {
        match (&mut open, line.starts_with("```")) {
            (None, false) => {}
            (None, true) => open = Some(Vec::new()),
            (Some(block), false) => block.push(line.trim_end().to_owned()),
            (Some(_), true) => blocks.push(open.take().expect("open")),
        }
    }
    assert!(
        !blocks.is_empty(),
        "{path} draws nothing under {section:?} — the heading moved or the section went"
    );
    blocks
}

/// The three fenced blocks of `screens/dialogs.md` § *While the call is running*, in its own
/// order: **0** the three labelled rows the state draws · **1** the ordinary cut · **2** the cut
/// that keeps the `/` an ordinary one would have eaten.
fn mockup_in_flight() -> Vec<Vec<String>> {
    let blocks = fenced("dialogs.md", "## While the call is running");
    assert_eq!(
        blocks.iter().map(Vec::len).collect::<Vec<_>>(),
        [3, 1, 1],
        "screens/dialogs.md § While the call is running no longer draws one state and two cuts"
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
/// **0** the rewritten `X` row · **1** the rewritten *Changing things* heading and the three rows
/// left unchanged beneath it · **2** the footer, which that state already emptied.
fn mockup_paused() -> Vec<Vec<String>> {
    let blocks = fenced("help.md", "## While the call is running");
    assert_eq!(
        blocks.iter().map(Vec::len).collect::<Vec<_>>(),
        [1, 4, 1],
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
        assert_eq!(key_map(HELP, refused, false), HELP, "{what}");
        assert_eq!(
            key_map(HELP, refused, false).lines().collect::<Vec<_>>(),
            mockup(),
            "{what} — and the mockup is the fixture, not HELP"
        );
    }
    assert_eq!(
        key_map(HELP, Refused::default(), false),
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
    assert_eq!(
        clause[0], base[12],
        "the two readers disagree about which row the Changing things heading is"
    );
    let expected: Vec<String> = base[..12].iter().chain(&clause).cloned().collect();
    assert_eq!(
        key_map(HELP, refusing(true, true, true, "deployments"), false)
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
                for (nth, marked) in [scale, restart, delete].into_iter().enumerate() {
                    if marked {
                        expected[13 + nth] = clause[1 + nth].clone();
                    }
                }
                assert_eq!(
                    key_map(HELP, refusing(scale, restart, delete, "deployments"), false)
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
                    let drawn = key_map(HELP, refusing(scale, restart, delete, resource), false);
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
        key_map(HELP, refusing(true, true, true, resource), false)
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
#[test]
fn every_clause_names_the_plural_its_own_operation_would_send() {
    let mut seen = 0;
    for (kind, plural) in KINDS {
        for (operation, resource, row, names) in [
            (
                "scale",
                crate::ops::scalable(kind),
                "    s ",
                format!("{plural}/scale"),
            ),
            (
                "restart",
                crate::ops::restartable(kind),
                "    r ",
                format!(" {plural})"),
            ),
        ] {
            // A kind the operation does not reach is never asked and is never *refused* — it is
            // withheld, `screens/states.md`'s own word and its own later box.
            let Ok(resource) = resource else { continue };
            assert_eq!(
                resource.plural, plural,
                "{operation} {kind} — the driver's plural is not the one ops would send"
            );
            let drawn = key_map(HELP, refusing(true, true, true, plural), false);
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
        seen, 6,
        "scale reaches three kinds and restart three; a kind stopped being measured"
    );
}

/// **A key map that no longer draws the mutating keys loses nothing to a refusal** — the rows are
/// found by their own key, never by counting from the end (`tester`, 2026-09-12).
///
/// **This is the `--read-only` swap simulated, not built**: `screens/help.md` says that swap is
/// owed and not landed, and building it is its own box. What this box owes is that landing it
/// cannot silently eat three rows of a neighbouring block, which is what the arithmetic version
/// did — and it did it without a panic and without failing a test, which is why the guard is here
/// and not in the box that makes the change.
#[test]
fn a_key_map_over_a_help_that_lost_those_keys_rewrites_nothing() {
    let read_only = "  Looking at things (always available)
    l  logs, with the log from before a crash
    d  describe — the object and what happened to it
    y  view as YAML

  read-only mode — nothing can be changed from here";
    for (kind, plural) in KINDS {
        assert_eq!(
            key_map(read_only, refusing(true, true, true, plural), false),
            read_only,
            "{kind} — a refusal rewrote a row that is not its key's"
        );
    }
}

/// **The refused rows reach the real frame**, not just the string that feeds it — the same body
/// region, the same sixteen rows, and the rest of the frame as untouched as it is with nothing
/// refused (`screens/help.md`, its note under the mockup).
#[test]
fn the_refused_key_map_is_what_the_help_screen_draws() {
    let alerts = Pane::Ready(vec![oom()]);
    let now = now();
    let log = [
        "$ kubectl get statefulsets -A --watch".to_owned(),
        "$ kubectl get daemonsets -A --watch".to_owned(),
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

    let expected: Vec<String> = mockup()[..12]
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
/// (`screens/dialogs.md` § *While the call is running*, `screens/widgets.md` § 7's fourth
/// deliberate truncation).
///
/// **All three cases are read out of that file, not written here** (`tester`, 2026-09-12, and
/// [`mockup_dialog`]'s own reason one screen along): the short name that draws whole, the
/// ordinary cut, and rule 3's — the namespace giving way from its front so the object's own name
/// survives whole. They agreed with this file's own literals byte for byte on the morning they
/// were written, which is exactly what makes a literal drift rather than fail, and rule 3's line
/// changed under them two days later.
///
/// **Rule 3's fixture is `openshift-cluster-node-tuning-operator`**, 38 characters and a namespace
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
            "payments",
            "checkout-worker-service-account-token-projector",
        ),
        (2, "openshift-cluster-node-tuning-operator", "tuned"),
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
/// **Which end of the namespace gives way is swept with it, and that is rule 3's reversal**
/// (`k8s-admin`, 2026-09-12). Stated as the invariant rather than by re-deriving the branches
/// here: the namespace on screen is **either whole — rule 2, where the `/` survived a plain clip
/// and nothing of it was cut — or behind a leading `…`**. An unmarked *prefix* of a namespace is
/// the one thing no rule may draw, and it is exactly what the first draft of rule 3 drew.
/// Behind that mark the object's own name is whole, unless the namespace has already given up the
/// whole of itself and there is still nothing left to give.
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
/// **The last branch is stated rather than claimed distinct**: once the name alone passes
/// `room − 2` there is nothing left to give, and two names sharing a 30-character head genuinely
/// do draw alike — what rule 3 promises there is only that the head kept is the **name's** and
/// not the namespace's, which is the half that decides whether the line names an object at all.
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

    // **Past `room − 2` the namespace goes whole and the name is cut at its own tail**, so what is
    // on screen is still the object's head and not a namespace every object shares.
    let long = drawn("cluster-node-tuning-operator-metrics-reader-binding");
    println!("{long}");
    assert!(
        long.contains("changing …/cluster-node-tuning-operator-m… first"),
        "the namespace kept a front the name needed: {long:?}"
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
    let open = Open::new();
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
    assert_eq!(
        &paused[1][1..],
        &base[13..16],
        "the two readers disagree about the three rows under the Changing things heading"
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

    let drawn: Vec<String> = key_map(HELP, Refused::default(), true)
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
        key_map(HELP, Refused::default(), false),
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
    let expected = key_map(HELP, Refused::default(), true);
    for scale in [false, true] {
        for restart in [false, true] {
            for delete in [false, true] {
                let drawn = key_map(HELP, refusing(scale, restart, delete, "deployments"), true);
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
        "$ kubectl get statefulsets -A --watch".to_owned(),
        "$ kubectl get daemonsets -A --watch".to_owned(),
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
        kubectl: "kubectl scale deployment/web --replicas=3 -n payments".to_owned(),
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
        kubectl: "kubectl rollout restart deployment/web -n payments".to_owned(),
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
        kubectl: "kubectl delete pod/web-7d9f4 -n payments".to_owned(),
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

/// **Four rows of warning at [`CONFIRM_BOX`]'s own text width** — one row short of [`WORDY`], so
/// the pair of them is the blank row under the consequence appearing and disappearing.
const ROOMY: &str = "The cluster answered this check with a sentence longer than any it really \
                     sends, long enough to take four whole rows of this box and no more than \
                     four of them, which is what this one is for.";

/// **Five rows of warning at [`CONFIRM_BOX`]'s own text width** — no operation sends anything
/// like it, and that is the point: the row budget is a total guard on a `views::Dialog` anyone
/// can build, and this is the only shape that crowds a 58-wide box from below.
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
        kubectl: "kubectl delete node/node-3".to_owned(),
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
        "$ kubectl get statefulsets -A --watch".to_owned(),
        "$ kubectl scale deployment/web --replicas=3 -n payments".to_owned(),
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
/// reads. The blocks are in the file's own order: 0 scale · 1 restart · 2 restart paused ·
/// 3 delete pod · 4 delete node · 5 refused · 6 gone · 7 drain.
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
                if block.first().is_some_and(|line| line.starts_with(" nodes")) {
                    blocks.push(block);
                }
            }
        }
    }
    assert_eq!(
        blocks.len(),
        8,
        "screens/dialogs.md no longer draws eight screens"
    );
    nested(&blocks[nth])
}

/// The Alerts screen a dialog is opened over, with a real command log under it.
fn opened_over<'a>(alerts: &'a Pane<Vec<Card>>, now: &'a Time, log: &'a [String]) -> Screen<'a> {
    let mut screen = screen(alerts, now);
    screen.log = log;
    screen
}

fn logged_pair() -> [String; 2] {
    [
        "$ kubectl get statefulsets -A --watch".to_owned(),
        "$ kubectl scale deployment/web --replicas=3 -n payments".to_owned(),
    ]
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
        mockup_dialog(1),
        "screens/dialogs.md § Restart"
    );

    let mut paused = restarting();
    paused.warning = Some(PAUSED.to_owned());
    assert_eq!(
        box_of(views::Modal::Confirm(paused)),
        mockup_dialog(2),
        "screens/dialogs.md § Restart, its paused Deployment"
    );
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
    for (nth, dialog) in [(3, deleting()), (4, deleting_a_node())] {
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
/// **The wrap points themselves are deliberately not asserted for three of these boxes**, because
/// `screens/dialogs.md` says they are not the specification: *"the wrap points shown are this
/// box's choice of where to break for readability, not a second field"* (§ Restart, of a
/// consequence that is one string). § Scale's, § The cluster said no's and § The object went
/// away's differ from a real wrap at the same width; § Restart's and § Delete's do not, and those
/// two are asserted character for character above.
///
/// **The button row is left out of the join** — its gap and its centring are the one difference
/// this file keeps from the page, and it has its own test.
#[test]
fn every_dialog_says_the_words_the_screen_file_says() {
    let words = |box_: &[String]| {
        box_[1..box_.len() - 1]
            .iter()
            .filter(|row| !row.contains("esc cancel") && !row.contains("esc dismiss"))
            .flat_map(|row| {
                row.trim_matches('│')
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let mut paused = restarting();
    paused.warning = Some(PAUSED.to_owned());
    for (nth, modal) in [
        (0, views::Modal::Confirm(scaling())),
        (1, views::Modal::Confirm(restarting())),
        (2, views::Modal::Confirm(paused)),
        (3, views::Modal::Confirm(deleting())),
        (4, views::Modal::Confirm(deleting_a_node())),
        (
            5,
            views::Modal::Refused {
                sent: false,
                fault: crate::k8s::Fault::Rejected,
                said: Some(DENIED.to_owned()),
            },
        ),
        (
            6,
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
            words(&drawn),
            words(&mockup),
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
        (CONFIRM_BOX, views::Modal::Confirm(scaling())),
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

/// **The confirm button is dim until [`views::Dialog::armed`], and then it is
/// [`theme::FOCUS`]** (`screens/widgets.md` § 5, `screens/dialogs.md` rule 3) — the ctrl-key-slip
/// guard, seen on the screen rather than asked of the type.
///
/// **`esc cancel` never dims**, because a modal never traps the user.
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
        box_.iter().any(|row| row.contains(CUT)),
        "the consequence was clipped in silence:\n{}",
        box_.join("\n")
    );
    for kept in ["[ ⏎ do it ]", "[ esc cancel ]", "$ kubectl scale"] {
        assert!(
            box_.iter().any(|row| row.contains(kept)),
            "{kept:?} was pushed out of the box by a long consequence"
        );
    }

    // **The narrow box has the same budget, and it is the only one the blank row under the
    // consequence lives in.** A mutation run flipped `1 + spacer` to `1 - spacer` and nothing
    // noticed (2026-09-10): every case above is 61 wide, where that row does not exist. The pair
    // below is that row being counted — four rows of warning and it stays, five and it goes, and
    // both land inside the ceiling.
    //
    // **`ops.rs` builds no such dialog**: its one warning is three rows and attaches only to a
    // restart, whose consequences are four and five rows and so are never 58 wide. The budget is
    // a total guard on a `views::Dialog` anyone can construct.
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
        usize::from(CONFIRM_BOX) + 2,
        "the fixture stopped being the narrow box it is about"
    );
    assert_eq!(roomy.len(), MODAL_ROWS + 2, "{}", roomy.join("\n"));
    // Four rows of warning: the blank under the consequence is still there, between the last
    // warning row and the verdict.
    let verdict_at = |box_: &[String]| {
        box_.iter()
            .position(|row| row.contains("checked it first"))
            .expect("the verdict's row")
    };
    assert!(
        roomy[verdict_at(&roomy) - 1]
            .trim_matches('│')
            .trim()
            .is_empty(),
        "the blank row under the consequence was already gone at four rows:\n{}",
        roomy.join("\n")
    );

    let wordy = narrow(WORDY);
    assert_eq!(wordy.len(), MODAL_ROWS + 2, "{}", wordy.join("\n"));
    assert!(
        !wordy[verdict_at(&wordy) - 1]
            .trim_matches('│')
            .trim()
            .is_empty(),
        "the blank row survived a box that had no room for it:\n{}",
        wordy.join("\n")
    );
    // Nothing was cut to buy it — the blank goes first, before any sentence
    // (`screens/widgets.md` § 5).
    assert!(
        !wordy.iter().any(|row| row.contains(CUT)),
        "a sentence gave way before the blank row did:\n{}",
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
                !box_.iter().any(|row| row.contains(CUT)),
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
        !whole.iter().any(|row| row.contains(CUT)),
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

    let text: Vec<&String> = crowded.iter().filter(|row| row.contains(CUT)).collect();
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
    // Nothing the reader acts on gave way.
    for kept in [
        "$ kubectl delete pod/web-7d9f4",
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
