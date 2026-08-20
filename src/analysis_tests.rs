use super::*;

use crate::rules::ObjectKind;

// --- BUILDING THE PANES screens/analysis.md DRAWS ---
//
// The claim under test is that `Report` can say what each pane says — the row that has a
// severity and the row that has none, the row the cursor may land on and the line it must
// skip, the row that jumps to a finding, to an object no rule named, and to nothing at all,
// and the section that could not be computed sitting where its answer would have been. No
// report is computed in this box, so every pane below is built by hand from the screen and
// then read back.
//
// **The glyphs are absent on purpose.** `● ▲ ○` belong to `theme.rs`, `→ ` and `⏎` to
// `views.rs`; what a row carries is the band and the sentence (CLAUDE.md § single point of
// change). So is the line break: a row is one line, and where it wraps is measured a layer up.
// `nothing_a_report_carries_spells_a_glyph_or_breaks_its_own_line` is the sweep that holds
// both, over every string in every report this file builds — titles and badges included.
//
// **The wording below is the pre-D128 sketch's and the claim it proves is not.** Four of the
// panes were redrawn after this box landed — Capacity stopped being a table, `● BLOCKS` moved
// into the band, `Worth knowing (not broken):` left the Waste pane, and `1.31 (1) ▲ too far
// behind` turned out to flag a healthy cluster (NOTES § D128). What each builder asserts is
// that the *shape* carries a pane of that kind: a banded row beside an unbanded one, a
// selectable row beside a line the cursor skips, a jump to a finding beside a jump to an
// object beside none at all. Every one of those is still true of the redrawn panes, and no
// assertion here reads a sentence for its own sake. **Each builder is replaced by its report's
// producer as that box lands**, which is where the current wording gets asserted — against a
// snapshot, not against a literal typed out of a mockup.

/// A line that is read and never selected. Every one of these is a line `screens/analysis.md`
/// draws with no `⏎` on it, and after NOTES § D127's correction the *variant* is what says so.
fn prose(text: &str) -> Row {
    Row::Prose(text.to_string())
}

/// **`detail` is a slice of paragraphs and not one string** (NOTES § D129), so the empty case
/// is `&[]` and reads as what it is: a row with nothing indented under it. Every builder below
/// passes its paragraphs in the order the pane draws them.
fn answer(severity: Option<Severity>, text: &str, detail: &[&str], jump: Option<Jump>) -> Row {
    Row::Answer {
        severity,
        text: text.to_string(),
        detail: detail.iter().map(|d| (*d).to_string()).collect(),
        action: String::new(),
        jump,
    }
}

fn object(kind: ObjectKind, namespace: Option<&str>, name: &str) -> ObjectId {
    ObjectId {
        kind,
        namespace: namespace.map(str::to_string),
        name: name.to_string(),
        uid: Some(format!("uid-{name}")),
    }
}

/// **The one identity in the product that carries no uid** — C1's kubeconfig, a file on the
/// reader's own disk that was never an API object (`rules.rs` § the certificate rules,
/// NOTES § D51). It gets its own builder because [`object`] cannot express `uid: None`, and the
/// one place `None` is mandatory is exactly where a derived `Some("uid-…")` went in silently.
fn kubeconfig(context: &str) -> ObjectId {
    ObjectId {
        kind: ObjectKind::Other("kubeconfig".to_string()),
        namespace: None,
        name: context.to_string(),
        uid: None,
    }
}

fn node(name: &str) -> Jump {
    Jump::Object(object(ObjectKind::Node, None, name))
}

/// C1's finding, the one row on these panes a rule already answered (NOTES § D87).
///
/// **Copied field for field off `kubeconfig_certificate_expiring` in `rules.rs`**, at 30 days
/// out so the pane stays the one `screens/analysis.md` draws — the title's sentence, the
/// evidence's `valid until … · …` shape, the action, `kubectl_cmd: None` (no such command
/// exists), `owner == object`, **`uid: None`** and **`timestamp: None`**. The last two are not
/// wording: `ObjectId::uid` names this as the only `None` in the product, and D69 refuses a
/// timestamp because `notAfter` is a deadline — a stamp here draws an age on the one card that
/// must have none.
///
/// It is hand-built because the rule is private to `rules.rs` and this module is its sibling,
/// not its child. That is the same wall the module doc's producer signature answers: a real
/// producer receives this finding from [`crate::rules::analyze`] rather than rebuilding it.
fn kubeconfig_certificate_expiring() -> Finding {
    Finding {
        severity: Severity::Info,
        title: "Your kubeconfig certificate expires in 30 days".to_string(),
        evidence: "valid until 2026-09-16T00:00:00Z · this is the file on your own machine that \
                   proves who you are — nothing in the cluster is broken"
            .to_string(),
        action: "ask whoever gave you access for a new kubeconfig before that date — k8rs cannot \
                 renew it, and after it kubectl stops working for you too"
            .to_string(),
        kubectl_cmd: None,
        owner: kubeconfig("prod-eu"),
        object: kubeconfig("prod-eu"),
        timestamp: None,
    }
}

// --- READING A ROW BACK ---
//
// A pane is asserted through these, never through the literal it was built from: what the
// test claims is that the shape carries the fact, not that a `String` round-tripped.
//
// **They answer for an `Answer` and panic on anything else**, which is now a claim and not a
// convenience: a line the cursor cannot reach has no band, no explanation and nowhere to go, so
// a test asking one of these questions about a `Prose` or a `NotComputed` is asking the wrong
// row — an unselectable line is asserted as the variant it is, never read through here.
//
// **Exhaustive on purpose, catch-all nowhere.** A `_` arm would swallow whatever variant a
// later report box adds; naming both makes it a compile error in five places at once.

fn severity_of(row: &Row) -> Option<Severity> {
    match row {
        Row::Answer { severity, .. } => *severity,
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so it carries no band")
        }
    }
}

fn text_of(row: &Row) -> &str {
    match row {
        Row::Answer { text, .. } => text,
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so it has no `text` to read back")
        }
    }
}

fn detail_of(row: &Row) -> &[String] {
    match row {
        Row::Answer { detail, .. } => detail,
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so it has no explanation under it")
        }
    }
}

fn action_of(row: &Row) -> &str {
    match row {
        Row::Answer { action, .. } => action,
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so there is nothing to do about it")
        }
    }
}

fn jump_of(row: &Row) -> Option<&Jump> {
    match row {
        Row::Answer { jump, .. } => jump.as_ref(),
        Row::Prose(_) | Row::NotComputed { .. } => {
            panic!("this row is not an answer, so `⏎` never lands on it")
        }
    }
}

/// Every string a report carries, **title and badge included**. The sweep reads this rather
/// than the rows directly, so the exhaustive `match` makes a new [`Row`] variant a compile
/// error here instead of a silently unswept string (`tester` planted a `●` in `Report::title`
/// and the whole suite stayed green).
fn strings_of(report: &Report) -> Vec<&str> {
    let mut out = vec![report.title.as_str()];
    out.extend(report.badge.iter().map(|badge| badge.value.as_str()));
    for row in &report.rows {
        match row {
            Row::Answer {
                text,
                detail,
                action,
                ..
            } => {
                out.push(text.as_str());
                out.extend(detail.iter().map(String::as_str));
                out.push(action.as_str());
            }
            Row::Prose(text) => out.push(text.as_str()),
            Row::NotComputed { reason, ask_for } => out.extend([reason.as_str(), ask_for.as_str()]),
        }
    }
    out
}

/// The rows the cursor may land on, in order — the `Answer`s, by their text.
fn selectable(report: &Report) -> Vec<&str> {
    report
        .rows
        .iter()
        .filter_map(|row| match row {
            Row::Answer { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

// --- CAPACITY ---

/// `screens/analysis.md` § Capacity, cluster-wide.
fn capacity() -> Report {
    Report {
        title: "What each node promised, and what it has".to_string(),
        badge: Some(Badge {
            value: "1".to_string(),
            severity: Severity::Warn,
        }),
        rows: vec![
            prose("NODE  PROMISED  USABLE  IN USE"),
            answer(
                None,
                "node-1  7.4 cpu  8 cpu  2.1 cpu",
                &[],
                Some(node("node-1")),
            ),
            answer(
                Some(Severity::Warn),
                "node-2  9.1 cpu  8 cpu  3.4 cpu",
                &[],
                Some(node("node-2")),
            ),
            answer(
                None,
                "node-3  1.2 cpu  8 cpu  0.4 cpu",
                &[],
                Some(node("node-3")),
            ),
            prose("node-2 has promised more CPU than it has. Nothing new can start there."),
            answer(None, "No CPU/memory limit: 34 workloads", &[], None),
            prose("(needs metrics-server for the IN USE column)"),
        ],
    }
}

#[test]
fn capacity_draws_one_warned_node_among_three_and_a_row_that_judges_nothing() {
    let report = capacity();

    assert_eq!(report.rows.len(), 7, "seven lines are drawn in the pane");

    // The table: one node over its allocatable, two under. The `▲` sits beside `9.1 cpu` on
    // screen; where the glyph lands is `views.rs`'s, the band is the row's.
    assert_eq!(severity_of(&report.rows[1]), None);
    assert_eq!(severity_of(&report.rows[2]), Some(Severity::Warn));
    assert_eq!(severity_of(&report.rows[3]), None);

    // The old rule 9 is a count, not an alarm: a row with no severity at all, which is why
    // `severity` is an `Option` rather than a fourth band.
    assert_eq!(severity_of(&report.rows[5]), None);
    assert_eq!(
        text_of(&report.rows[5]),
        "No CPU/memory limit: 34 workloads"
    );

    // The badge is a count and its band, and it carries no symbol.
    let badge = report
        .badge
        .expect("the capacity badge is drawn when the check ran");
    assert_eq!(badge.value, "1");
    assert_eq!(badge.severity, Severity::Warn);
}

#[test]
fn the_cursor_lands_on_the_rows_the_screen_offers_enter_on_and_skips_the_lines_it_does_not() {
    // The pane the whole `Row::Prose` split exists for. `screens/analysis.md` draws
    // `↑↓ move  ⏎ open` under it, and three of its seven lines are read-only: a column
    // header, the sentence under the table, and the metrics-server parenthetical.
    let report = capacity();
    assert_eq!(
        selectable(&report),
        vec![
            "node-1  7.4 cpu  8 cpu  2.1 cpu",
            "node-2  9.1 cpu  8 cpu  3.4 cpu",
            "node-3  1.2 cpu  8 cpu  0.4 cpu",
            "No CPU/memory limit: 34 workloads",
        ],
        "the header, the sentence and the parenthetical are not rows `⏎` may land on"
    );

    // **The pair that makes `jump.is_some()` the wrong test** (NOTES § D127): a selectable row
    // with no destination recorded, and an unselectable line that also has none. Keying a
    // cursor on the field would have skipped the first — the one the screen prints
    // `— ⏎ to list` on — in a file frozen by Phase 9.
    assert!(
        matches!(&report.rows[5], Row::Answer { jump: None, .. }),
        "`No CPU/memory limit: 34 workloads` is selectable and its destination is undecided"
    );
    assert!(
        matches!(&report.rows[6], Row::Prose(_)),
        "`(needs metrics-server …)` is not a row at all, and that is a different fact"
    );
}

/// `screens/analysis.md` § Capacity when you can only see one namespace.
fn capacity_scoped_to_one_namespace() -> Report {
    Report {
        title: "What each node promised, and what it has".to_string(),
        badge: None,
        rows: vec![
            Row::NotComputed {
                reason: "Not checked here. Adding up what a node has promised needs every pod \
                         on it, and you can only see payments — so every number would come out \
                         too low."
                    .to_string(),
                ask_for: "Ask for cluster-wide read access, or drop the --namespace flag if you \
                          set one."
                    .to_string(),
            },
            prose("Still counted, from what you can see:"),
            answer(None, "No CPU/memory limit: 6 workloads", &[], None),
        ],
    }
}

#[test]
fn a_switched_off_section_sits_where_its_answer_would_have_been_and_the_rest_still_answers() {
    let report = capacity_scoped_to_one_namespace();

    assert_eq!(report.rows.len(), 3);

    // The report is not empty and is not an error: one section off, two rows still true.
    let Row::NotComputed { reason, ask_for } = &report.rows[0] else {
        panic!("the promised/usable table is the section that switches off");
    };
    assert!(
        reason.contains("Not checked here"),
        "it names the check that is off"
    );
    assert!(
        !reason.contains("403") && !reason.contains("RBAC"),
        "it does not say 403 or RBAC (screens/analysis.md)"
    );
    // Both causes, because the screen cannot tell a --namespace flag from a 403 fallback.
    assert!(ask_for.contains("read access"));
    assert!(ask_for.contains("--namespace"));

    // The limits row keeps counting, and counts what is in scope.
    assert_eq!(text_of(&report.rows[2]), "No CPU/memory limit: 6 workloads");

    // Nothing is drawn where nothing was computed: the three node rows are **gone**, not
    // filled with dashes, so no row on this pane names a node at all.
    assert!(
        !strings_of(&report).iter().any(|s| s.contains("node-")),
        "there is no table here, so there is no row to put a `—` in — and no line of any \
         kind names a node, which asserting over the `Answer`s alone would not have caught"
    );
}

/// The state `screens/widgets.md` § 1a says the sidebar cannot draw: the check **ran** and
/// found nothing overcommitted, so nothing is badged — the same blank
/// [`capacity_scoped_to_one_namespace`] draws for the opposite reason.
fn capacity_with_nothing_overcommitted() -> Report {
    Report {
        title: "What each node promised, and what it has".to_string(),
        badge: None,
        rows: vec![
            prose("NODE  PROMISED  USABLE  IN USE"),
            answer(
                None,
                "node-1  7.4 cpu  8 cpu  2.1 cpu",
                &[],
                Some(node("node-1")),
            ),
            answer(None, "No CPU/memory limit: 34 workloads", &[], None),
        ],
    }
}

#[test]
fn a_check_that_ran_and_found_nothing_is_not_a_check_that_did_not_run() {
    let ran = capacity_with_nothing_overcommitted();
    let did_not = capacity_scoped_to_one_namespace();

    assert_eq!(ran.rows.len(), 3, "the pane still draws its table");

    // **Both badges are blank, and that is the point.** The sidebar has room for a number and
    // not for a reason, so it cannot be the thing that tells these two apart
    // (`screens/widgets.md` § 1a). An earlier draft badged the first `0`; it carried nothing
    // the body does not already say, and the sidebar has no room for the reason either way.
    assert_eq!(ran.badge, None);
    assert_eq!(did_not.badge, None);

    // The discriminator is in the body, in the one place the screen has to print it anyway.
    assert!(
        !ran.rows
            .iter()
            .any(|r| matches!(r, Row::NotComputed { .. })),
        "a report that ran says nothing about what it could not do"
    );
    assert!(
        did_not
            .rows
            .iter()
            .any(|r| matches!(r, Row::NotComputed { .. })),
        "the badge has no room for a sentence, so the body carries it"
    );
}

/// The three `detail` lengths `screens/analysis.md` § Capacity draws, in one report — the pane
/// NOTES § D129 widened this field for. **This builder is post-D128 and the three above are
/// not**: it is the row as the screen draws it now, `<promised> of <usable> cpu · <promised> of
/// <usable> GiB`, the band first and never mid-line, the measurement in `detail` and never in
/// `text`.
///
/// - the flagged node draws **two** indented paragraphs — what it is using, then what the
///   numbers mean;
/// - a healthy node draws **one**, the measurement alone, because there is nothing to explain;
/// - and on a cluster with no metrics-server there is **no** measurement to draw, so the row
///   has none at all and the pane says why once, under the node rows, in a `Row::NotComputed`.
fn capacity_as_the_screen_draws_it_now() -> Report {
    Report {
        title: "What each node promised, and what it has".to_string(),
        badge: Some(Badge {
            value: "1".to_string(),
            severity: Severity::Warn,
        }),
        rows: vec![
            Row::Answer {
                severity: Some(Severity::Warn),
                text: "node-2   6.2 of 8 cpu · 30 of 16 GiB".to_string(),
                detail: vec![
                    "using 3.4 cpu and 12 GiB".to_string(),
                    "Almost twice the memory is promised as node-2 has. If these pods use what \
                     they asked for, one of them is killed."
                        .to_string(),
                ],
                action: "move a workload off, or ask for less".to_string(),
                jump: Some(node("node-2")),
            },
            answer(
                None,
                "node-1   7.4 of 8 cpu · 11 of 16 GiB",
                &["using 2.1 cpu and 6 GiB"],
                Some(node("node-1")),
            ),
            answer(
                None,
                "node-3   1.2 of 8 cpu · 3 of 16 GiB",
                &[],
                Some(node("node-3")),
            ),
            answer(
                None,
                "34 workloads have no memory or CPU limit",
                &["Nothing stops one taking a whole node."],
                None,
            ),
        ],
    }
}

#[test]
fn a_row_carries_as_many_indented_paragraphs_as_the_pane_draws_under_it() {
    // **The test `detail: String` cannot pass** (NOTES § D129). One string could hold both of
    // node-2's paragraphs only with a `\n` in it, and the sweep at the foot of this file —
    // `nothing_a_report_carries_spells_a_glyph_or_breaks_its_own_line`, which reaches every
    // paragraph of every report — refuses that over this very report. This layer cannot see the
    // pane's width, so it may not be the layer that breaks a line.
    let report = capacity_as_the_screen_draws_it_now();

    assert_eq!(
        report
            .rows
            .iter()
            .map(|row| detail_of(row).len())
            .collect::<Vec<_>>(),
        vec![2, 1, 0, 1],
        "the flagged node draws a measurement and an explanation, a healthy node the \
         measurement alone, and a node with no metrics-server neither"
    );

    // **Order is the claim, not just the count.** The measurement comes first and the sentence
    // that interprets it second; swapped, the reader meets the consequence before the number it
    // is about, and a `Vec` is exactly what makes that assertable.
    let flagged = detail_of(&report.rows[0]);
    assert!(
        flagged[0].starts_with("using "),
        "the measurement is the first paragraph: {:?}",
        flagged[0]
    );
    assert!(
        flagged[1].contains("killed"),
        "the explanation is the second, and it says what happens: {:?}",
        flagged[1]
    );

    // **Empty is `&[]` and is drawn by leaving the line out** — not by an element that is the
    // empty string, which would draw the blank line [`Finding::evidence`]'s convention refuses.
    assert!(detail_of(&report.rows[2]).is_empty());
    assert!(
        report
            .rows
            .iter()
            .all(|row| detail_of(row).iter().all(|p| !p.is_empty())),
        "no paragraph is the empty string — absence is length, never a blank element"
    );

    // The band is the first thing on the row and never inside it: `6.2 of 8 cpu` carries no
    // glyph, and neither does any paragraph under it. Swept for the whole report below.
    assert_eq!(severity_of(&report.rows[0]), Some(Severity::Warn));
    assert_eq!(severity_of(&report.rows[1]), None);
}

// --- DRAIN SAFETY ---

/// `screens/analysis.md` § Drain safety.
fn drain_safety() -> Report {
    Report {
        title: "If you drained each node, what happens?".to_string(),
        badge: None,
        rows: vec![
            answer(None, "node-1  ok  18 pods move", &[], Some(node("node-1"))),
            Row::Answer {
                severity: Some(Severity::Critical),
                text: "node-2  BLOCKS  never finishes".to_string(),
                detail: vec![
                    "payments/web wants at least 5 copies and has exactly 5. Draining would take \
                     one away, so it waits forever."
                        .to_string(),
                ],
                action: "run one more copy, or relax the disruption budget first".to_string(),
                jump: Some(node("node-2")),
            },
            answer(
                Some(Severity::Warn),
                "node-3  2 pods nothing would restart",
                &["(started by hand, no Deployment)"],
                Some(node("node-3")),
            ),
        ],
    }
}

#[test]
fn drain_safety_carries_the_blocked_nodes_explanation_and_its_way_out() {
    let report = drain_safety();

    assert_eq!(report.rows.len(), 3, "one row per node");
    assert_eq!(
        severity_of(&report.rows[0]),
        None,
        "a node that drains cleanly is not a finding"
    );
    assert_eq!(severity_of(&report.rows[1]), Some(Severity::Critical));
    assert_eq!(severity_of(&report.rows[2]), Some(Severity::Warn));

    let Row::Answer { detail, action, .. } = &report.rows[1] else {
        unreachable!("built as an answer above");
    };
    assert_eq!(detail.len(), 1, "this row draws one indented paragraph");
    assert!(
        detail[0].contains("waits forever"),
        "the explanation says what a drain would do"
    );
    assert_eq!(
        action,
        "run one more copy, or relax the disruption budget first"
    );

    // The screen draws exactly one `→` line on this pane: empty is drawn by leaving the line
    // out, never by drawing a blank one, and the contrast is what proves it — an assertion
    // that only read the two empty strings back would be reading the builder, not the pane.
    assert_eq!(
        report
            .rows
            .iter()
            .filter(|row| !action_of(row).is_empty())
            .count(),
        1,
        "only the blocked node has something to do about it"
    );

    // Every row goes somewhere, and no rule fired for any of them: a drain verdict is a
    // report's answer, not a finding.
    for row in &report.rows {
        assert!(
            matches!(jump_of(row), Some(Jump::Object(_))),
            "a drain row jumps to its node"
        );
    }
}

/// `screens/analysis.md` § *What each report needs*: drain safety without a cluster-wide pod
/// list and PodDisruptionBudgets is **not computed, full stop** — the whole report is the one
/// row. It is the report whose partial answer is worst of the three, because *"18 pods move,
/// node-1 is ok"* is a green light for an operation that then hangs on a pod nobody could see.
fn drain_safety_not_computed() -> Report {
    Report {
        title: "If you drained each node, what happens?".to_string(),
        badge: None,
        rows: vec![Row::NotComputed {
            reason: "Not checked here. Working out whether a drain finishes needs every pod on \
                     every node, and the rules that say how many copies must stay up — and a \
                     half-answer here would call a node safe that is not."
                .to_string(),
            ask_for: "Ask for cluster-wide read access, or drop the --namespace flag if you set \
                      one."
                .to_string(),
        }],
    }
}

// --- WASTE ---
//
// **Three rows below carry `jump: None` because no destination is recorded for them yet**,
// which is what [`Row::Answer::jump`] now means and all it means. `47 pods`, `12 replicasets`
// and `9 pods` each stand for a *set* of objects, as do Capacity's `No CPU/memory limit: 34
// workloads — ⏎ to list` and Certificates' `2 kubelets waiting to join` — five counted rows
// across three panes. [`Jump`] has a case for one object and a case for one finding; a set is
// neither, and what `⏎` opens for one is unanswered (NOTES § D127). They are `Answer`s, so the
// cursor still reaches them, which is the half the first draft got wrong.

/// `screens/analysis.md` § Waste.
fn waste() -> Report {
    let service = object(
        ObjectKind::Other("Service".to_string()),
        Some("shop"),
        "api-svc",
    );
    let claim = object(
        ObjectKind::Other("PersistentVolumeClaim".to_string()),
        Some("data"),
        "pgdata-old",
    );
    Report {
        title: "Things that cost you something for nothing".to_string(),
        badge: None,
        rows: vec![
            answer(
                Some(Severity::Critical),
                "shop/api-svc  matches no pod",
                &["This Service points at nothing. Anything calling it gets a 503."],
                Some(Jump::Object(service)),
            ),
            answer(
                Some(Severity::Warn),
                "data/pgdata-old  reserved, unused, 100Gi",
                &[],
                Some(Jump::Object(claim)),
            ),
            answer(
                Some(Severity::Warn),
                "47 pods  Evicted / Completed",
                &[],
                None,
            ),
            answer(
                Some(Severity::Info),
                "12 replicasets  parked at 0 replicas",
                &[],
                None,
            ),
            prose("Worth knowing (not broken):"),
            answer(
                Some(Severity::Info),
                "9 pods mount a path from the node",
                &[],
                None,
            ),
        ],
    }
}

/// The other side of [`Report::rows`]'s line: the check **ran** and had nothing to say, which
/// is an empty `Vec` and never a [`Row::NotComputed`]. Waste is where it happens —
/// `screens/analysis.md` § *What each report needs* has it *run unchanged* under any scope,
/// because its rows are per-object facts rather than sums, so a cluster with nothing wasteful
/// in it draws a pane with no rows at all.
fn waste_with_nothing_wasted() -> Report {
    Report {
        title: "Things that cost you something for nothing".to_string(),
        badge: None,
        rows: Vec::new(),
    }
}

#[test]
fn waste_spans_all_three_bands_and_its_first_row_jumps_to_an_object_no_rule_named() {
    let report = waste();

    assert_eq!(report.rows.len(), 6);
    assert_eq!(
        selectable(&report).len(),
        5,
        "`Worth knowing (not broken):` is a heading and the other five are rows"
    );
    assert_eq!(
        report
            .rows
            .iter()
            .filter(|row| matches!(row, Row::Answer { .. }))
            .map(severity_of)
            .collect::<Vec<_>>(),
        vec![
            Some(Severity::Critical),
            Some(Severity::Warn),
            Some(Severity::Warn),
            Some(Severity::Info),
            Some(Severity::Info),
        ],
        "the 503 first, `Info` under the heading"
    );

    // The Service is the case the whole report exists for: nothing is broken enough for a
    // rule to have fired, so there is no finding to jump to — only the object.
    let Some(Jump::Object(id)) = jump_of(&report.rows[0]) else {
        panic!("the Service row jumps to the Service, and no finding names it");
    };
    assert_eq!(id.kind, ObjectKind::Other("Service".to_string()));
    assert_eq!(id.namespace.as_deref(), Some("shop"));
    assert_eq!(id.name, "api-svc");
    // **Every paragraph is swept, not the first**, which is the assertion `Vec<String>` made
    // possible to get wrong: a row's explanation may now arrive in more than one, and a check
    // that reads element 0 passes over whatever the second one says (NOTES § D129).
    assert!(
        detail_of(&report.rows[0]).iter().any(|p| p.contains("503")),
        "the explanation is what makes this row readable at 3am"
    );

    // `Worth knowing (not broken):` is a heading — read, never selected.
    assert!(
        matches!(&report.rows[4], Row::Prose(text) if text == "Worth knowing (not broken):"),
        "a heading is prose, not an answer with its band left off"
    );
}

#[test]
fn a_report_that_could_not_run_is_one_row_and_a_report_with_nothing_to_say_is_no_rows() {
    let could_not = drain_safety_not_computed();
    let nothing_to_say = waste_with_nothing_wasted();

    // **Neither pane has a cursor**, which is the state NOTES § D127's unselectable
    // `NotComputed` made reachable. It leads the test because it is a fact about the panes
    // rather than about what they say, and because it is what Phase 9 meets first: a reflex
    // `ListState::select(Some(0))` parks the highlight on the *could not run* line of the one
    // and points at a row the other does not have. Neither pane draws `⏎ open` ([`Row`]'s doc).
    assert!(
        selectable(&could_not).is_empty(),
        "a lone `NotComputed` is a line, not a row `⏎` may land on"
    );
    assert!(
        selectable(&nothing_to_say).is_empty(),
        "a report with no rows has no row to select either"
    );

    // The lone `NotComputed`: the whole report, not a section of one, and it still names the
    // check and the way out — the two halves the variant makes mandatory.
    assert_eq!(could_not.rows.len(), 1);
    let Row::NotComputed { reason, ask_for } = &could_not.rows[0] else {
        panic!("drain safety without a cluster-wide read is not computed at all");
    };
    assert!(
        !reason.contains("403")
            && !reason.contains("RBAC")
            && !reason.contains("PodDisruptionBudget"),
        "the reason is in plain language, and `PodDisruptionBudget` is the jargon this \
         report's own rows spell as `disruption budget`: {reason}"
    );
    assert!(
        ask_for.contains("read access") && ask_for.contains("--namespace"),
        "one sentence covering both causes, as Capacity's does"
    );

    // The empty `Vec`: nothing to say is not the same as nothing to show for it.
    assert!(nothing_to_say.rows.is_empty());
    assert!(
        !nothing_to_say.title.is_empty(),
        "the pane still has its heading — an empty report is a pane, not a blank screen"
    );

    // Neither badges, so — again — the badge cannot tell them apart and the body must.
    assert_eq!(could_not.badge, None);
    assert_eq!(nothing_to_say.badge, None);
}

// --- CERTIFICATES AND VERSIONS ---

/// `screens/analysis.md` § Certificates and Versions.
fn certificates() -> Report {
    Report {
        title: "What expires, soonest first".to_string(),
        badge: Some(Badge {
            value: "30d".to_string(),
            severity: Severity::Warn,
        }),
        rows: vec![
            answer(
                Some(Severity::Warn),
                "your kubeconfig certificate  30 days",
                &["After that, kubectl stops working for you until it is renewed."],
                Some(Jump::Finding(Box::new(kubeconfig_certificate_expiring()))),
            ),
            answer(
                Some(Severity::Info),
                "API server certificate  210 days",
                &[],
                None,
            ),
            answer(
                Some(Severity::Critical),
                "2 kubelets waiting to join  pending CSR",
                &["Two nodes cannot join until someone approves them."],
                None,
            ),
            prose("Versions:  control plane 1.34 · kubelets 1.34 (2) · 1.31 (1) too far behind"),
        ],
    }
}

#[test]
fn the_certificate_row_jumps_to_the_finding_a_rule_already_made() {
    let report = certificates();

    assert_eq!(report.rows.len(), 4);
    assert_eq!(severity_of(&report.rows[0]), Some(Severity::Warn));
    assert_eq!(severity_of(&report.rows[1]), Some(Severity::Info));
    assert_eq!(severity_of(&report.rows[2]), Some(Severity::Critical));

    // C1 is a rule and its answer is a `Finding`, carried whole so the detail view can draw
    // it without a registry to look it up in.
    let Some(Jump::Finding(finding)) = jump_of(&report.rows[0]) else {
        panic!("the kubeconfig row jumps to C1's finding");
    };
    assert_eq!(finding.severity, Severity::Info);
    assert!(finding.title.contains("expires in 30 days"));
    assert_eq!(
        finding.kubectl_cmd, None,
        "no kubectl command shows this, which is why C1 exists"
    );

    // **The two fields a hand-written finding gets wrong.** `rules.rs` builds C1's `ObjectId`
    // with no uid — the only `None` in the product — and no timestamp, because `notAfter` is a
    // deadline rather than the moment anything happened (NOTES § D69). A stamp here draws
    // *"365 days ago"* on the one card that must draw no age at all.
    assert_eq!(
        finding.object.uid, None,
        "a kubeconfig is a file on a laptop and never had a uid"
    );
    assert_eq!(
        finding.owner, finding.object,
        "there is nothing above a file"
    );
    assert_eq!(finding.timestamp, None, "and so this card carries no age");

    // The badge is a duration here and a count on Capacity — one field, because the sidebar
    // draws both as the same right-aligned span.
    let badge = report
        .badge
        .expect("certificates badges its soonest expiry");
    assert_eq!(badge.value, "30d");
    assert_eq!(badge.severity, Severity::Warn);

    // The `Versions:` summary is prose: it is read, never selected.
    assert!(
        matches!(&report.rows[3], Row::Prose(text) if text.starts_with("Versions:")),
        "the versions summary is a line, not a row the cursor stops on"
    );
}

// --- THE RESTART ROW ---

/// Phase 4's later box: a container that keeps dying between its restarts draws nothing from
/// rules 1, 2, 5 or 6, so the row exists precisely because there is no finding
/// (NOTES § D101). Its title and its home are `tui-designer`'s to settle; what is asserted
/// here is only the shape's half of the claim.
fn restarts() -> Report {
    Report {
        title: "Containers that keep dying and coming back".to_string(),
        badge: None,
        rows: vec![answer(
            None,
            "payments/web-7d9f4 · retry  47 restarts  this run 3 min",
            &[],
            Some(Jump::Object(object(
                ObjectKind::Pod,
                Some("payments"),
                "web-7d9f4",
            ))),
        )],
    }
}

#[test]
fn the_restart_row_jumps_to_a_pod_and_never_to_a_finding() {
    let report = restarts();
    let row = &report.rows[0];

    let Some(Jump::Object(id)) = jump_of(row) else {
        panic!("there is no finding here — that is the whole reason the row exists");
    };
    assert_eq!(id.kind, ObjectKind::Pod);
    assert_eq!(id.name, "web-7d9f4");

    // The container is not broken right now, so the row makes no judgement.
    assert_eq!(severity_of(row), None);

    // It may not re-spell how the last run ended: `ending` and `exit_meaning` are private to
    // `rules.rs` and a raw `exit 137` here is the defect D85 exists to prevent.
    assert!(
        !text_of(row).contains("exit "),
        "no exit code is spelled in a report row"
    );
}

// --- THE SHAPE ITSELF ---

/// Every `Report` this file builds, named. What each one *draws* is its own test's claim; the
/// rules below hold over all of them.
fn every_report() -> Vec<(&'static str, Report)> {
    vec![
        ("capacity", capacity()),
        (
            "capacity, as the screen draws it now",
            capacity_as_the_screen_draws_it_now(),
        ),
        (
            "capacity, one namespace",
            capacity_scoped_to_one_namespace(),
        ),
        (
            "capacity, nothing overcommitted",
            capacity_with_nothing_overcommitted(),
        ),
        ("drain safety", drain_safety()),
        ("drain safety, not computed", drain_safety_not_computed()),
        ("waste", waste()),
        ("waste, nothing wasted", waste_with_nothing_wasted()),
        ("certificates and versions", certificates()),
        ("restarts", restarts()),
    ]
}

#[test]
fn every_pane_the_screen_draws_is_expressible() {
    // **What this covers, exactly**: the four panes `screens/analysis.md` sketches — Capacity
    // (drawn twice, cluster-wide and namespace-scoped), Drain safety, Waste, Certificates — plus
    // the states its § *What each report needs* table gives them and the restart row Phase 4
    // adds later.
    // **Two of Family C's six reports are not here and neither absence is a shape defect** —
    // `Versions` is drawn at the foot of the Certificates pane rather than as a pane of its
    // own, and `Posture` (todo.md) has no sketch at all. Both are `screens/`'s to answer.
    for (name, report) in every_report() {
        assert!(!report.title.is_empty(), "{name} has a pane heading");
        // Invariant 14 reaches every string here: a report never titles itself with its own
        // sidebar label, which is the jargon the heading exists to replace.
        assert!(
            !report.title.eq_ignore_ascii_case(name),
            "{name}'s heading is a sentence, not its name"
        );
    }
    // **Exactly one report here draws no rows**, and it is the one whose whole claim is that
    // an empty `Vec` is a legal report. Naming it is what keeps this loop honest: a builder
    // that quietly stopped returning rows would show up as a second name in this list rather
    // than as a sweep that passed over nothing.
    assert_eq!(
        every_report()
            .iter()
            .filter(|(_, report)| report.rows.is_empty())
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        vec!["waste, nothing wasted"]
    );
}

#[test]
fn nothing_a_report_carries_spells_a_glyph_or_breaks_its_own_line() {
    for (name, report) in every_report() {
        for s in strings_of(&report) {
            // No theme symbol reaches this file (CLAUDE.md § single point of change), and `⏎`
            // is `views.rs`'s footer and its `— ⏎ to list` suffix — the one glyph the screen
            // prints *inside* a row, which is why it is easiest to leave in by accident.
            assert!(
                !s.contains(['\u{25cf}', '\u{25b2}', '\u{25cb}', '\u{2192}', '\u{23ce}']),
                "{name} spells a glyph that belongs to theme.rs or views.rs: {s}"
            );
            // A row is one line before wrapping. A newline in it is a wrap this layer made,
            // and it is the layer that cannot see the pane's width — the row-height accounting
            // it breaks is Phase 9's (PRIOR-ART § D3).
            assert!(
                !s.contains(['\n', '\r']),
                "{name} breaks a line the renderer has not measured: {s}"
            );
        }
    }
}
