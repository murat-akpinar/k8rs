//! Tests for the temporary driver — the loader, the report, and the control-character strip
//! that invariant 9 owes this file two phases before Phase 5's ingest strip exists.
//!
//! Everything here is a pure function over values: `load` over the committed captures,
//! `render` over `Finding`s, `stdout_failure` over the error a write returns. Nothing captures
//! stdout or watches a process exit, because nothing needs to: every decision `main` makes is in
//! one of those functions, and what is left there is argv, the choice of stream and the exit
//! code — which is `tests/binary.rs`'s.

use super::*;

// `main.rs` itself never names a kind, only matches on the strings the API sends.
use crate::rules::{ContainerState, ObjectKind};

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// A fixed moment, so a card's age is the same string on every run (invariant 5, NOTES § D18).
/// `main` reads the real clock; a test may not, or the ladder rung moves under it.
///
/// **It is the instant `scripts/certs-test.sh` pins the committed certificates against**, which
/// this file joined when the live driver started printing the Certificates pane (NOTES § D169):
/// C1's row and its badge are a subtraction between that pin and those bytes, and a file
/// measuring them from an instant nothing compares is what that guard exists to refuse. It was
/// `2026-08-16T12:00:00Z` while this file was only about card ages.
fn now() -> Time {
    Time("2026-08-23T00:00:00Z".parse().expect("a fixed timestamp"))
}

/// Four minutes before [`now`] — the `4 min ago` rung of the ladder
/// (`screens/widgets.md` § 1b).
fn four_minutes_ago() -> Time {
    Time("2026-08-22T23:56:00Z".parse().expect("a fixed timestamp"))
}

/// Read nothing: the snapshot a report about findings alone is rendered against.
fn nothing_read() -> Input {
    load(&[], now(), false).expect("no paths is not a failure")
}

fn read(names: &[&str]) -> Input {
    let paths: Vec<String> = names.iter().map(|n| fixture(n)).collect();
    load(&paths, now(), false).unwrap_or_else(|e| panic!("{names:?} did not load: {e}"))
}

/// The bytes of a pod capture, for the stub server that has to answer a `get` with them.
fn pod_body(name: &str) -> String {
    std::fs::read_to_string(fixture(&format!("{name}.json")))
        .unwrap_or_else(|e| panic!("{name}.json does not read: {e}"))
}

/// How many objects a `kind: List` capture actually holds — read out of the file rather than
/// transcribed, because the count belongs to the cluster that produced it and moves on the
/// next `just fixtures` (`src/rules_tests.rs` § What the capture itself says).
fn items_in(name: &str) -> usize {
    let text = std::fs::read_to_string(fixture(name)).expect("the fixture reads");
    let doc: Value = serde_json::from_str(&text).expect("the fixture is JSON");
    let items = doc["items"]
        .as_array()
        .expect("the fixture is a List")
        .len();
    assert!(
        items > 0,
        "{name} holds no items — it stopped being the fixture this test needs"
    );
    items
}

/// **A committed `kind: List` capture with its items taken out**, written where [`load`] can
/// read it back — and the emptiness belongs to this test rather than to a file.
///
/// `poddisruptionbudgets.json` and `persistentvolumeclaims.json` were the only two empty Lists
/// in the corpus, and the test below read one of them *because* it was empty.
/// `scripts/broken.yaml` fills both on the next capture trip, at which point that test would
/// have had no input at all and both its assertions would have failed — found by running the
/// binary, not by reading the test (NOTES § D129's second blocker, `tester`'s finding).
///
/// **A test whose subject is emptiness owns its emptiness.** The source is still a committed
/// capture, so this is not hand-written JSON (CLAUDE.md § fixtures come from real cluster
/// captures): what is removed is the array, and [`items_in`] asserts there was one to remove —
/// otherwise a source that quietly became empty would make this helper a no-op and the test a
/// tautology.
fn emptied_list(name: &str) -> String {
    assert!(items_in(name) > 0, "{name} had nothing to empty");
    let text = std::fs::read_to_string(fixture(name)).expect("the fixture reads");
    let mut doc: Value = serde_json::from_str(&text).expect("the fixture is JSON");
    doc["items"] = Value::Array(Vec::new());
    // The process id separates two `cargo test` runs and the thread id separates two callers
    // inside one — `cargo test` runs tests as threads, so the pid alone would let a second
    // test emptying this same source delete the file this one is about to read. The caller
    // removes it the moment `load` has read it, so nothing survives the test either way.
    let path = std::env::temp_dir().join(format!(
        "k8rs-empty-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, doc.to_string()).expect("a temp file this test owns");
    path.to_string_lossy().into_owned()
}

/// The seven labels [`reports`] prints, in the order the sidebar lists them — named here so a
/// producer that stopped being printed is a red test rather than a pane nobody misses.
///
/// **`[restarts]` was missing until its box landed**, which made the claim above untrue of it:
/// the list only fails on a pane that *leaves*, so one that never joined is a producer this file
/// would not have missed either.
const PANES: [&str; 7] = [
    "[capacity]",
    "[certificates]",
    "[drain safety]",
    "[posture]",
    "[restarts]",
    "[waste]",
    "[versions]",
];

fn finding(severity: Severity, object: ObjectId) -> Finding {
    Finding {
        severity,
        title: "Something happened".to_string(),
        evidence: "the numbers that prove it".to_string(),
        action: "do this about it".to_string(),
        kubectl_cmd: None,
        owner: object.clone(),
        object,
        timestamp: None,
    }
}

fn pod_id(namespace: &str, name: &str) -> ObjectId {
    ObjectId {
        kind: ObjectKind::Pod,
        namespace: Some(namespace.to_string()),
        name: name.to_string(),
        uid: None,
    }
}

fn node_id(name: &str) -> ObjectId {
    ObjectId {
        kind: ObjectKind::Node,
        namespace: None,
        name: name.to_string(),
        uid: None,
    }
}

// --- THE STRIP, WHICH IS WHY THIS BOX EXISTS ---

/// **What a printed string kept that has no printed form of its own** — the sweep all three
/// strip tests run.
///
/// It asks [`crate::k8s::unprintable`], which is what [`crate::sanitize`] asks, because the
/// defect this region exists for was a *second spelling* of that predicate: these tests filtered
/// on the narrower `Cc` category by hand while the ingest guard had already widened, so every
/// one of them was green over a U+202E they were written to catch (NOTES § D154). A test that
/// spells the class out itself is how the next widening comes back green for the same reason.
///
/// **The caller strips the report's own line breaks before calling this**, rather than an
/// exclusion hidden in here: a `\n` in a *value* forges a second card and must be caught, and
/// `the_usage_text_keeps_its_three_lines` is what holds the other half.
fn survivors(printed: &str) -> Vec<char> {
    printed
        .chars()
        .filter(|c| crate::k8s::unprintable(*c))
        .collect()
}

/// **The positive half of invariant 9.** A crafted name, message or action reaches the
/// terminal through `println!` with no ratatui in between, so every string read off a
/// `Finding` passes `sanitize` first (`screens/once.md` § The rule that matters most here).
///
/// All five `Cc` shapes at once — `ESC`, `CR`, `BEL`, a C1 control (`CSI`) and `DEL` — **and
/// one from each of the five things [`crate::k8s::unprintable`] adds beyond `Cc`**: the soft
/// hyphen, the zero-width block (U+200B), the bidi marks (U+200E), the overrides (U+202E) and
/// the word joiner (U+2060), plus U+FEFF. Every field carries one, because a per-field
/// judgement call is how one of six gets forgotten, and because a bidi override is what makes
/// `prod\u{202e}dc-web` read as *prodcd-web* in a list nobody can then search (NOTES § D154).
///
/// **And both identity shapes**, because an identity is drawn by one of two arms and a guard
/// is proven only for the shapes it was fed (NOTES § D29): a namespaced object, whose
/// namespace *and* name are printed, and a cluster-scoped one, whose name is printed on its
/// own. A node is as nameable by an attacker as a pod — `kubectl label` is not needed, a
/// kubelet registers under the name it is given.
#[test]
fn nothing_unprintable_from_a_finding_reaches_the_report() {
    let mut f = finding(
        Severity::Critical,
        pod_id("pay\r\u{200b}ments", "web\u{9b}\u{202e}0"),
    );
    f.title = "Escape \x1b[2J and bell \x07 h\u{ad}ere".to_string();
    f.evidence = "exit \x7f 1\u{feff}37".to_string();
    f.action = "restart \u{85} i\u{2060}t".to_string();
    f.timestamp = Some(four_minutes_ago());
    let cluster_scoped = finding(Severity::Warn, node_id("node\x1b[2J\u{9b}\u{200e}-3"));

    let report = render(&[f, cluster_scoped], &nothing_read());

    // The driver's own line breaks are structure, not values ([`survivors`]).
    let survivors = survivors(&report.replace('\n', ""));
    assert!(
        survivors.is_empty(),
        "characters with no printed form reached the report: {survivors:?}\n{report:?}"
    );
    // And the strip removed only those: a `sanitize` that returned nothing at all would
    // satisfy the assertion above (CLAUDE.md § A derived list asserts it found something).
    assert!(
        report.contains("payments/web0"),
        "the identity was stripped away with the escape: {report:?}"
    );
    assert!(
        report.contains("Escape [2J and bell  here"),
        "the title lost more than its control characters: {report:?}"
    );
    assert!(
        report.contains("exit  137") && report.contains("restart  it"),
        "the evidence or the action lost more than its control characters: {report:?}"
    );
    assert!(
        report.contains("▲ node[2J-3\n"),
        "the cluster-scoped identity is drawn by the other arm of `name`, and it did not \
         come out clean: {report:?}"
    );
}

/// **The same invariant over the seven analysis reports, which nothing outside `#[cfg(test)]` had
/// ever rendered.** `analysis_tests`' own `pane` strips nothing, so until this box every string in
/// every report was unexercised — and **Posture's row text is a `hostPath.path` verbatim and
/// whole**, not a value inside a sentence, so a crafted path arrived at the terminal as an escape
/// sequence (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 8).
///
/// **Four framings, because a guard is proven only for the framing it was written for**
/// (NOTES § D31): the whole of a row's `text` (the host path), a value inside a sentence (the
/// namespace in Posture's `in {namespace}`), a value inside a *row's* sentence built by a
/// different producer (the node name on Capacity's row), and **three values joined into one row's
/// `text` by a fourth** (the pod's namespace, its name and its container's, on a Restarts row).
///
/// **`restarts.json` is in the input for that last one, and its absence was the gap**
/// (NOTES § D29). The pane draws nothing on a cluster of `healthy-hostpath` and nodes, so none of
/// its three untrusted interpolations ever reached this sweep, and a guard that is never fed the
/// shape is not a guard for it.
#[test]
fn nothing_unprintable_from_a_report_reaches_the_terminal() {
    // `restarts.json` second, so the plants below still land on `healthy-hostpath`'s pod at 0.
    let mut input = read(&["healthy-hostpath.json", "restarts.json", "nodes.json"]);
    let pod = &mut input.snapshot.pods[0];
    assert_eq!(pod.host_path_mounts.len(), 1, "the capture mounts one path");
    // `ESC`, `CR`, `BEL`, `DEL` and a C1 control — the whole `Cc` category — **and one of the
    // invisible characters `Cc` does not hold, per value**, in the middle of a value rather
    // than as the whole of one ([`survivors`], NOTES § D154).
    pod.host_path_mounts[0].path = "/var\x1b[2J/lo\u{9b}\u{202e}g".to_string();
    pod.id.namespace = Some("pay\r\u{200b}ments".to_string());
    input.snapshot.nodes[0].id.name = "node\x07-\x7f\u{feff}1".to_string();
    // The pod has to be on the node whose name is crafted, or Capacity's row never names it.
    pod.node = Some(input.snapshot.nodes[0].id.name.clone());

    // **The Restarts row, whose text is three untrusted values joined by that producer's own
    // `separator`.** All three are crafted, because the row builds them in one `format!` and a
    // per-field judgement call is how one of the three gets forgotten.
    let restarting = &mut input.snapshot.pods[1];
    assert_eq!(
        restarting.containers.len(),
        1,
        "the capture carries the one restarting container"
    );
    restarting.id.namespace = Some("sh\x1b\u{ad}op".to_string());
    restarting.id.name = "che\u{9b}\u{2060}ckout".to_string();
    restarting.containers[0].name = "ap\x07\u{200e}p".to_string();
    // The capture was taken after this file's pin, so its own `startedAt` sits four days in the
    // future and `rules::age` declines it — which is the pane's *no age yet* state, not its row.
    // This test is about the strip and not about the ladder, so the start moves onto the pin.
    restarting.containers[0].state = ContainerState::Running {
        started_at: Some(four_minutes_ago()),
    };

    let printed = reports(&input.snapshot, &analyze(&input.snapshot));
    println!("{printed}");

    let survivors = survivors(&printed.replace('\n', ""));
    assert!(
        survivors.is_empty(),
        "characters with no printed form reached the terminal: {survivors:?}\n{printed:?}"
    );
    // **And only those were removed** — a `sanitize` that returned nothing at all satisfies the
    // assertion above (CLAUDE.md § A derived list asserts it found something).
    assert!(
        printed.contains("/var[2J/log"),
        "the path is a row's whole text, and it came back short: {printed:?}"
    );
    assert!(
        // **The em dash is the delimiter the producer writes after the value, not decoration**,
        // and what this proves is that the namespace arrived whole: a strip that returns it
        // short fails here — measured, `in pay` when the strip truncates at the first control
        // character, `in payents —` when it drops the character after one.
        //
        // **It does not prove the strip stopped at the value's end**, and that is the path
        // assertion's half rather than a gap: the plant's two control characters are adjacent,
        // so a strip that consumes one character past each eats the second control instead of a
        // letter and hands this namespace back whole — measured, and it is the path above, whose
        // crafted value has printable text between its controls, that comes back `/var2J/log`
        // and fails.
        //
        // The full stop this looked for before does not occur in the output at all: Posture's
        // read-only sentence carries a clause after the namespace when the mounting pod runs
        // outside `kube-system`, and a crafted namespace never is it
        // (`screens/analysis.md` § Posture).
        printed.contains("in payments \u{2014}"),
        "the namespace enters a sentence and came back short: {printed:?}"
    );
    assert!(
        printed.contains("node-1   "),
        "the node name enters another producer's sentence and came back short: {printed:?}"
    );
    assert!(
        printed.contains("shop/checkout \u{b7} container app"),
        "three values joined into one Restarts row, and one came back short: {printed:?}"
    );
}

/// **An empty action draws no line at all**, the same convention [`card`] follows for an empty
/// evidence — never a `→ ` with nothing after it, which is a hole in the middle of a pane
/// ([`analysis::Row::Answer::action`]). Both halves in one report, because *drawn for everything*
/// and *drawn for nothing* are the two ways this goes wrong and one assertion cannot see both.
#[test]
fn a_row_with_nothing_to_do_draws_no_arrow_and_one_with_something_does() {
    let printed = pane(
        "test",
        &analysis::Report {
            title: "What each node promised, and what it has".to_string(),
            badge: None,
            rows: vec![
                analysis::Row::Answer {
                    severity: Some(Severity::Warn),
                    text: "node-2   over".to_string(),
                    detail: vec!["one of them is killed.".to_string()],
                    action: "move some pods to another node".to_string(),
                    jump: None,
                },
                analysis::Row::Answer {
                    severity: None,
                    text: "node-1   fine".to_string(),
                    detail: Vec::new(),
                    action: String::new(),
                    jump: None,
                },
            ],
        },
    );

    assert_eq!(
        printed.lines().collect::<Vec<&str>>(),
        [
            "[test]",
            "  What each node promised, and what it has",
            "  ▲ node-2   over",
            "      one of them is killed.",
            "      → move some pods to another node",
            "    node-1   fine",
        ],
        "the flagged row carries its glyph and its way out; the row with nothing to do carries \
         neither, and nothing is drawn where nothing was said"
    );
}

/// **The flag, and that it is not read as a path.** Seven panes under the cards when it is passed,
/// none when it is not — and the cards themselves are the same either way, because the driver
/// prints one report under the other rather than folding the reports into the findings.
#[test]
fn the_analysis_flag_adds_every_pane_and_is_not_a_file() {
    let paths = [fixture("nodes.json"), fixture("kube-system-pods.json")];
    let plain = run(&paths).expect("the captures load");
    let with_reports = run(&[paths[0].clone(), ANALYSIS.to_string(), paths[1].clone()])
        .expect("the flag is not a path — a run that tried to open it would be an Err naming it");

    for pane in PANES {
        assert!(!plain.contains(pane), "{pane} is drawn without asking");
        assert!(
            with_reports.contains(pane),
            "{pane} is missing: {with_reports}"
        );
    }
    assert!(
        with_reports.starts_with(&plain),
        "the cards are unchanged and the panes are under them"
    );
    // The order is the sidebar's, and it is asserted rather than assumed.
    let mut at = 0;
    for pane in PANES {
        let found = with_reports[at..].find(pane).expect("checked above");
        at += found;
    }

    // **The flag on its own is the usage**, not a run over no files at all.
    assert_eq!(
        run(&[ANALYSIS.to_string()]),
        Err(USAGE.to_string()),
        "a flag is not an input"
    );
}

/// **The negative half.** [`crate::k8s::unprintable`] answers for characters with no printed
/// form and nothing wider: a Turkish `ğ`, a CJK name and an en dash are ordinary text and come
/// out byte-identical. Nothing here is truncated either — we never cut a string ourselves
/// (`screens/widgets.md` § 7).
#[test]
fn ordinary_and_multibyte_text_passes_through_whole() {
    let mut f = finding(Severity::Critical, pod_id("üretim", "日本語-0"));
    f.title = "Kapasite yetersiz — ğüşiİ".to_string();
    f.evidence = "limit 256Mi · exit 137".to_string();
    f.action = "limitleri artır".to_string();

    assert_eq!(
        render(&[f], &nothing_read()),
        "0 pods · 0 nodes\n\
         \n\
         ● üretim/日本語-0\n  \
           Kapasite yetersiz — ğüşiİ\n  \
           limit 256Mi · exit 137\n  \
           → limitleri artır\n\
         \n\
         1 critical"
    );
}

// --- THE CARD ---

/// An empty evidence is drawn by **leaving the line out**, never as a blank line in the middle
/// of a card ([`Finding::evidence`]). `no_node_accepted_it` is the first rule that produces one.
#[test]
fn an_empty_evidence_draws_no_line_at_all() {
    let mut f = finding(Severity::Warn, pod_id("shop", "api-7"));
    f.evidence = String::new();

    assert_eq!(
        render(&[f], &nothing_read()),
        "0 pods · 0 nodes\n\
         \n\
         ▲ shop/api-7\n  \
           Something happened\n  \
           → do this about it\n\
         \n\
         1 warning"
    );
}

/// An evidence made **only** of unprintable characters is the same case one step later: what
/// decides is what would be printed, not what the API sent.
///
/// **The third framing D31 asks for — the whole of a value rather than a substring of one** —
/// and the zero-width character is in it because that is the framing where the old predicate
/// did not merely leak a character, it drew a card line made of nothing. Asserted as the whole
/// report rather than as *no blank line*, because a line holding one U+200B is not blank and
/// the weaker form was green over it.
#[test]
fn an_evidence_that_sanitizes_to_nothing_draws_no_line_either() {
    let mut f = finding(Severity::Warn, pod_id("shop", "api-7"));
    f.evidence = "\x07\x1b\r\u{200b}\u{feff}".to_string();

    assert_eq!(
        render(&[f], &nothing_read()),
        "0 pods · 0 nodes\n\
         \n\
         ▲ shop/api-7\n  \
           Something happened\n  \
           → do this about it\n\
         \n\
         1 warning"
    );
}

/// A finding whose event has no moment draws **no age suffix** — the empty right edge
/// ([`Finding::timestamp`]), and the same answer the Alerts card owes it.
#[test]
fn a_finding_with_no_moment_draws_no_age() {
    let f = finding(Severity::Warn, node_id("node-3"));

    // Line 0 is the header, line 1 the blank under it, line 2 the card's own first line —
    // and the header is full of `·`, so the assertion has to be about the card and not the
    // report.
    let report = render(&[f], &nothing_read());
    assert_eq!(report.lines().nth(2), Some("▲ node-3"), "{report:?}");
}

/// …and one that has a moment draws the ladder's exact string, from the same ladder both
/// screens read (`screens/widgets.md` § 1b, NOTES § D68). A node also proves the
/// cluster-scoped identity: `node-3`, never `/node-3`.
#[test]
fn a_finding_with_a_moment_draws_the_ladders_own_words() {
    let mut f = finding(Severity::Warn, node_id("node-3"));
    f.timestamp = Some(four_minutes_ago());

    let report = render(&[f], &nothing_read());
    assert_eq!(
        report.lines().nth(2),
        Some("▲ node-3 · 4 min ago"),
        "{report:?}"
    );
}

// --- THE REPORT AROUND THE CARDS ---

/// *Read nothing* and *found nothing* must not print the same three lines
/// (`screens/once.md` § When nothing is broken).
///
/// **This is the requirement the workload count was dropped against**, and it is D121's whole
/// purpose: the count was one of two mechanisms serving it, and the assertions below are the
/// half that has to keep holding without it.
#[test]
fn nothing_broken_still_says_what_was_read() {
    let read = read(&["oom.json"]);

    assert_eq!(render(&[], &read), "1 pod · 0 nodes\n\n○ nothing is broken");
    assert_eq!(
        render(&[], &nothing_read()),
        "0 pods · 0 nodes\n\n○ nothing is broken"
    );
}

/// **Severity is the order, and the `Info` band is not in this block at all** — the declaration
/// order of [`Severity`] is severity order and the derived `Ord` is what sorts what is left
/// (NOTES § D35), and NOTES § D87 is why there is nothing to sort in the third band: `Info` on a
/// rule *means* the finding lives in a report rather than in Alerts, and this block is Alerts.
///
/// **The finding is not dropped, it is drawn elsewhere** — `--analysis`'s panes, which
/// [`certificates_draws_c1s_row_and_the_sidebar_badge`] reads. Handed all three bands backwards,
/// the report puts two of them back and passes the third on.
#[test]
fn severity_orders_the_report_and_the_info_band_is_not_in_it() {
    let findings = vec![
        finding(Severity::Info, node_id("node-3")),
        finding(Severity::Warn, pod_id("shop", "api-7")),
        finding(Severity::Critical, pod_id("payments", "web-0")),
    ];

    let report = render(&findings, &nothing_read());
    let symbols: Vec<&str> = report
        .lines()
        .filter_map(|l| l.split(' ').next().filter(|s| ["●", "▲", "○"].contains(s)))
        .collect();
    assert_eq!(
        symbols,
        ["●", "▲"],
        "an `Info` finding was drawn as a card above the tally, so C1 prints twice in one run — \
         once here and once as the Certificates pane row (NOTES § D87): {report:?}"
    );
    assert!(
        report.ends_with("\n1 critical, 1 warning"),
        "the tally names a band whose cards are not drawn: {report:?}"
    );
    assert!(
        !report.contains("node-3"),
        "the `Info` finding's object reached the card block: {report:?}"
    );
}

/// **A run whose only findings are `Info` says nothing is broken here**, because nothing in this
/// block is (NOTES § D2, § D87). It is not silence about the finding: `--analysis` draws it, and
/// whether `--once` should print the reports for exactly this reason is that box's question.
#[test]
fn a_report_of_nothing_but_notes_is_a_report_with_no_alerts_in_it() {
    let only = vec![finding(Severity::Info, node_id("node-3"))];
    assert_eq!(
        render(&only, &nothing_read()),
        "0 pods · 0 nodes\n\n○ nothing is broken",
        "a block with no alerts in it drew a card, an empty tally, or both"
    );
}

/// The summary names **only** the bands that have something in them: a report that prints
/// `0 critical` is claiming a count it did not find.
#[test]
fn the_summary_leaves_out_the_bands_that_are_empty() {
    let one = |severity| vec![finding(severity, pod_id("shop", "api-7"))];
    let two = |severity| {
        vec![
            finding(severity, pod_id("shop", "api-7")),
            finding(severity, pod_id("shop", "api-8")),
        ]
    };
    let summary = |findings: Vec<Finding>| {
        render(&findings, &nothing_read())
            .lines()
            .next_back()
            .expect("a report has lines")
            .to_string()
    };

    assert_eq!(summary(one(Severity::Critical)), "1 critical");
    assert_eq!(summary(two(Severity::Critical)), "2 critical");
    assert_eq!(summary(one(Severity::Warn)), "1 warning");
    assert_eq!(summary(two(Severity::Warn)), "2 warnings");
    // **The third band is not empty here, it is not this block's** (NOTES § D87): the last line
    // of a report whose only finding is an `Info` is the *no alerts* line, never `1 note`.
    assert_eq!(summary(one(Severity::Info)), "○ nothing is broken");
    assert_eq!(summary(two(Severity::Info)), "○ nothing is broken");
    // And a band that is drawn is not silenced by one that is not.
    assert_eq!(
        summary(vec![
            finding(Severity::Info, node_id("node-3")),
            finding(Severity::Warn, pod_id("shop", "api-7")),
        ]),
        "1 warning"
    );
}

// --- THE LOADER ---

/// `kubectl get -A` answers with `kind: List` and the `kind` sits on each item. Both captures
/// land in the field their kind belongs to, and neither leaks into another.
#[test]
fn a_list_document_lands_in_the_field_its_kind_belongs_to() {
    let nodes = read(&["nodes.json"]);
    assert_eq!(nodes.snapshot.nodes.len(), items_in("nodes.json"));
    assert!(nodes.snapshot.pods.is_empty() && nodes.snapshot.workloads.is_empty());
    assert!(nodes.skipped.is_empty(), "{:?}", nodes.skipped);

    let pods = read(&["kube-system-pods.json"]);
    assert_eq!(pods.snapshot.pods.len(), items_in("kube-system-pods.json"));
    assert!(pods.snapshot.nodes.is_empty() && pods.snapshot.workloads.is_empty());

    // **All four workload kinds, one at a time.** They share a snapshot field, so a kind
    // whose arm went missing lands in `skipped` and the count still looks plausible beside a
    // sibling that works — which is why each is read on its own and `skipped` is asserted
    // empty with it (NOTES § D28: the workload watch is Deployments, StatefulSets and
    // DaemonSets; ReplicaSets are fetched on demand and decode the same way).
    for capture in [
        "deployments.json",
        "statefulsets.json",
        "daemonsets.json",
        "rollout-replicasets.json",
    ] {
        let input = read(&[capture]);
        assert_eq!(
            input.snapshot.workloads.len(),
            items_in(capture),
            "{capture} did not land in workloads"
        );
        assert!(
            input.skipped.is_empty(),
            "{capture} was counted as a kind no rule reads: {:?}",
            input.skipped
        );
        assert!(input.snapshot.pods.is_empty() && input.snapshot.nodes.is_empty());
    }
}

/// A bare document carries its own top-level `kind` — `kubectl get pod -o json`'s answer, and
/// the shape most of the pod fixtures are in.
#[test]
fn a_bare_document_lands_in_the_field_its_kind_belongs_to() {
    let input = read(&["oom.json"]);

    assert_eq!(input.snapshot.pods.len(), 1);
    assert!(input.snapshot.nodes.is_empty() && input.snapshot.workloads.is_empty());
    let text = std::fs::read_to_string(fixture("oom.json")).expect("the fixture reads");
    let doc: Value = serde_json::from_str(&text).expect("the fixture is JSON");
    assert_eq!(
        input.snapshot.pods[0].id.name,
        doc["metadata"]["name"]
            .as_str()
            .expect("the capture names it"),
        "the pod that came out is not the pod that went in"
    );
}

/// **A committed capture rewritten to a kind nothing dispatches on**, in a temp file this test
/// owns — [`emptied_list`]'s mechanism, for the same reason and with the same care: what is
/// asserted below is [`take`]'s fallback arm, which never decodes the body under the `kind`, so
/// the object itself is not what makes the input honest. The source is a capture and the one
/// field moved is named here.
///
/// **The corpus holds no unread kind any anymore, which is what this box changed**: the driver
/// now reads all eleven kinds in it — Pod, Node, the four workload kinds, and the five on-demand
/// lists the reports join. Without this the test would have had no input at all and would have
/// gone green over an assertion about nothing (the defect [`emptied_list`] records).
fn under_a_kind_nothing_reads(name: &str, kind: &str) -> String {
    let text = std::fs::read_to_string(fixture(name)).expect("the fixture reads");
    let mut doc: Value = serde_json::from_str(&text).expect("the fixture is JSON");
    let was = doc["kind"].as_str().expect("the capture names its kind");
    assert_ne!(
        was, kind,
        "{name} is already a {kind}, so nothing was moved"
    );
    doc["kind"] = Value::String(kind.to_string());
    let path = std::env::temp_dir().join(format!(
        "k8rs-unread-{kind}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::write(&path, doc.to_string()).expect("a temp file this test owns");
    path.to_string_lossy().into_owned()
}

/// **A kind no rule reads is counted and named, never dropped in silence** — the header is
/// where a reader decides whether the report covered what they handed it.
#[test]
fn a_kind_no_rule_reads_is_counted_and_named() {
    let one = under_a_kind_nothing_reads("healthy.json", "ConfigMap");
    let two = under_a_kind_nothing_reads("healthy.json", "Secret");
    let input = load(&[one.clone(), two.clone()], now(), false).expect("both documents read");
    let _ = std::fs::remove_file(&one);
    let _ = std::fs::remove_file(&two);

    assert_eq!(input.skipped.get("ConfigMap").copied(), Some(1));
    assert_eq!(input.skipped.get("Secret").copied(), Some(1));
    assert!(input.snapshot.pods.is_empty() && input.snapshot.nodes.is_empty());
    assert_eq!(
        header(&input),
        "0 pods · 0 nodes · 2 objects no rule reads (ConfigMap, Secret)",
        "named, sorted, and counted — and this clause is now the only thing separating a file \
         of objects no rule reads from a file of nothing, which is what NOTES § D121 was \
         narrowed to when the workload count went"
    );
}

/// **The five lists a report joins are `None` until one of their objects is read, and `Some` the
/// moment one is** — the distinction the reports key on (NOTES § D129): *nobody looked* against
/// *looked and found nothing*, which is the difference between Waste saying **not checked** and
/// Waste saying **nothing is going to waste**.
#[test]
fn the_on_demand_lists_are_nobody_looked_until_one_of_their_objects_is_read() {
    let nothing = nothing_read().snapshot;
    assert!(
        nothing.services.is_none()
            && nothing.endpoint_slices.is_none()
            && nothing.claims.is_none()
            && nothing.disruption_budgets.is_none()
            && nothing.certificate_requests.is_none()
            && nothing.replica_sets.is_none(),
        "a run that was handed no such file looked at none of them"
    );

    let read_one = read(&["services.json"]).snapshot;
    assert_eq!(
        read_one.services.as_ref().map(Vec::len),
        Some(items_in("services.json")),
        "and the list it was handed is the list it has"
    );
    assert!(
        read_one.endpoint_slices.is_none() && read_one.claims.is_none(),
        "one list arriving says nothing about the four beside it"
    );

    // **A ReplicaSet lands in both fields**: `workloads` for the W-rules, `replica_sets` for the
    // Waste row that counts the ones parked at zero.
    let sets = read(&["healthy-replicasets.json"]).snapshot;
    assert_eq!(
        sets.replica_sets.as_ref().map(Vec::len),
        Some(items_in("healthy-replicasets.json"))
    );
    assert_eq!(sets.workloads.len(), items_in("healthy-replicasets.json"));

    // And every other kind the reports join, so none of the five is wired to the wrong field.
    let joined = read(&[
        "endpointslices.json",
        "persistentvolumeclaims.json",
        "poddisruptionbudgets.json",
        "csr-pending.json",
    ])
    .snapshot;
    assert_eq!(
        (
            joined.endpoint_slices.as_ref().map(Vec::len),
            joined.claims.as_ref().map(Vec::len),
            joined.disruption_budgets.as_ref().map(Vec::len),
            joined.certificate_requests.as_ref().map(Vec::len),
        ),
        (
            Some(items_in("endpointslices.json")),
            Some(items_in("persistentvolumeclaims.json")),
            Some(items_in("poddisruptionbudgets.json")),
            Some(1),
        ),
        "the last is a bare document rather than a `kind: List`, which is the other shape `take` \
         is handed"
    );
    assert!(
        joined.services.is_none(),
        "and nothing filled a list nobody handed it"
    );
}

/// An empty list is not an error and not a skipped kind: nothing was in it.
///
/// **The input is [`emptied_list`]'s and not a fixture that happens to be empty** — see there
/// for why, and for what running the binary found.
#[test]
fn an_empty_list_reads_as_nothing_at_all() {
    let path = emptied_list("services.json");
    let input =
        load(std::slice::from_ref(&path), now(), false).expect("an empty list is not an error");
    std::fs::remove_file(&path).expect("the temp file this test wrote");

    // Every Service the source holds would have been counted as a kind no rule reads — the
    // test one above asserts exactly that, off the same file — so `skipped` being empty here
    // is the array's absence and nothing else. How many there are belongs to the cluster and
    // is deliberately not written down (`items_in`).
    assert!(input.skipped.is_empty(), "{:?}", input.skipped);
    assert_eq!(
        render(&[], &input),
        "0 pods · 0 nodes\n\n○ nothing is broken"
    );

    // **And the list stays *nobody looked* over a file that says this cluster has none** — the
    // one place the fixture path and the live fetch answer differently, and since 2026-08-29 a
    // ruling rather than an open question ([`take`]'s doc, which carries the measurement). `load`
    // iterates `.items[]`, so an empty envelope calls `take` zero times and `get_or_insert_with`
    // never runs; the envelope's own `kind` is `List` and names no resource, so there is no field
    // to file it under. `k8s::services` answers the same cluster `Some(vec![])` because it asked
    // for Services by name.
    assert_eq!(
        input.snapshot.services, None,
        "an empty envelope filed *looked and found nothing* over a document whose `kind` is \
         `List` — which of the six lists did it think it was?"
    );
}

/// **The panes redraw and the lists behind three of them do not, so the driver says so**
/// ([`lists_were_read`], NOTES § D46).
///
/// **Both halves are asserted, because only one of them is load-bearing and it is not the
/// number.** *How old* is what a reader checks; *it does not refresh* is what stops them trusting
/// a `ready to drain` verdict computed against a budget from before the change they just made.
#[test]
fn the_panes_say_how_old_the_lists_behind_them_are_and_that_they_do_not_refresh() {
    // **Each of the six alone, and the pane it is drawn in named — not the other two.** A check
    // is proven only for the shapes it was fed (NOTES § D29): fed only `services.json` this
    // passed with five of the six terms unexercised, and `just mutants-diff` turned one `||` into
    // `&&` with nothing objecting (2026-08-29). Each capture below holds exactly one of the six.
    for (capture, pane) in [
        ("healthy-replicasets.json", "waste"),
        ("services.json", "waste"),
        ("endpointslices.json", "waste"),
        ("persistentvolumeclaims.json", "waste"),
        ("poddisruptionbudgets.json", "drain safety"),
        ("csr-pending.json", "machines waiting to join"),
    ] {
        let said = lists_were_read(
            &read(&[capture]).snapshot,
            &now(),
            Some(&four_minutes_ago()),
        )
        .unwrap_or_else(|| panic!("a run that read {capture} did not say when it had read"));
        assert!(
            said.contains(pane),
            "{capture} feeds {pane} and the line does not name it: {said:?}"
        );
        for other in ["waste", "drain safety", "machines waiting to join"] {
            assert!(
                other == pane || !said.contains(other),
                "{capture} feeds only {pane}, and the line also claimed {other} had been read: \
                 {said:?}"
            );
        }
    }

    let read = read(&["services.json"]).snapshot;
    let said = lists_were_read(&read, &now(), Some(&four_minutes_ago()))
        .expect("a run that read a list says when");

    assert!(
        said.contains("4 min ago"),
        "the age is not on the one ladder every other age in this tool is drawn on \
         (`rules::age`): {said:?}"
    );
    assert!(
        said.contains("does not read them again"),
        "the line named a moment and never said the lists stop there, which is the half that \
         keeps a stale `ready to drain` from being believed: {said:?}"
    );
    println!("{said}");
}

/// **The shape a real read-only login actually produces: five lists and no CSR list.**
///
/// Upstream's built-in `view` ClusterRole grants all five namespaced kinds and grants
/// `certificatesigningrequests` in neither `view` nor `edit` (`k8s.rs` § WHAT A REPORT ASKS FOR),
/// so this is the ordinary cluster-wide read-only principal and not an exotic one. An
/// unconditional list of three panes said *machines waiting to join read their lists 4 min ago*
/// directly above a certificates pane saying **not checked** — two sentences on one screen that
/// cannot both be true (`k8s-admin`, 2026-08-29).
#[test]
fn a_login_refused_only_the_joining_machines_list_is_not_told_that_one_was_read() {
    let view_role = read(&[
        "healthy-replicasets.json",
        "services.json",
        "endpointslices.json",
        "persistentvolumeclaims.json",
        "poddisruptionbudgets.json",
    ])
    .snapshot;
    assert!(
        view_role.certificate_requests.is_none(),
        "this fixture set was supposed to leave the CSR list unread, and the shape is the whole \
         point of the test"
    );

    let said = lists_were_read(&view_role, &now(), Some(&four_minutes_ago()))
        .expect("five lists came back, so there is a reading to date");
    println!("{said}");
    assert!(
        said.contains("waste") && said.contains("drain safety"),
        "the two panes whose lists did come back are not named: {said:?}"
    );
    assert!(
        !said.contains("machines waiting to join"),
        "the line claims a list this login was refused, over a pane that says *not checked* two \
         lines below it: {said:?}"
    );
}

/// **And the mirror image**, so the filter is proven in both directions rather than by a name
/// that happens to sort last: a login with the CSR list and nothing else names only that.
#[test]
fn a_login_that_read_only_the_joining_machines_list_names_only_that_pane() {
    let said = lists_were_read(
        &read(&["csr-pending.json"]).snapshot,
        &now(),
        Some(&four_minutes_ago()),
    )
    .expect("one list came back");
    println!("{said}");
    assert!(
        said.contains("machines waiting to join")
            && !said.contains("waste")
            && !said.contains("drain safety"),
        "the line names panes whose lists were never read: {said:?}"
    );
}

/// **A run that read nothing claims no reading** — `None` is *nobody looked* and the panes
/// already draw *not checked* for it (NOTES § D129).
///
/// Without this the test above passes with the line hard-coded, and a `--live` refused all six
/// lists would print *"read 4 min ago"* over six refusals.
#[test]
fn a_run_that_was_refused_every_list_does_not_claim_to_have_read_one() {
    assert_eq!(
        lists_were_read(&nothing_read().snapshot, &now(), Some(&four_minutes_ago())),
        None,
        "a run that read nothing named a moment it read at"
    );
}

/// **A clock this machine could not read loses the number and keeps the warning.**
///
/// [`wall_clock`] can fail — a machine set before 1970 — and the line's two facts are not equally
/// important: dropping the whole caveat to lose the age would trade the load-bearing half for the
/// decoration.
#[test]
fn a_clock_that_could_not_be_read_still_warns_that_the_lists_are_frozen() {
    let said = lists_were_read(&read(&["services.json"]).snapshot, &now(), None)
        .expect("the warning does not depend on the clock");
    assert!(
        said.contains("earlier in this run") && said.contains("does not read them again"),
        "a clock that could not be read took the whole caveat with it: {said:?}"
    );
}

/// **And it is printed, above the panes, on the live path** — the whole point being that a reader
/// sees it beside the verdict it qualifies rather than in a doc comment.
#[test]
fn the_live_report_prints_the_reading_line_above_the_panes() {
    let mut store = listed(Vec::new());
    store.reports_fetched(k8s::ReportLists {
        disruption_budgets: Some(Vec::new()),
        ..Default::default()
    });
    let mut last = String::new();
    let printed = live_report(
        &store,
        now(),
        &mut last,
        true,
        false,
        &AtConnect {
            lists_read_at: Some(four_minutes_ago()),
            ..Default::default()
        },
    )
    .expect("a bootstrapped store draws a report");

    let line = printed
        .lines()
        .find(|line| line.contains("does not read them again"))
        .expect("the reading line is printed");
    // The assembled block, as the driver hands it to stdout — this is the run the report quotes.
    println!(
        "{}",
        printed
            .lines()
            .skip_while(|l| !l.contains("does not read them again"))
            .take(4)
            .collect::<Vec<_>>()
            .join("\n")
    );
    let at = printed.find(line).expect("it is in the report");
    let panes = printed.find("[capacity]").expect("the panes are printed");
    assert!(
        at < panes,
        "the caveat is printed under the panes it qualifies, where a reader meets the verdict \
         first"
    );

    // **And a run that read nothing prints no such line**, off the same driver — the negative
    // that keeps this from passing on a line that is always emitted.
    let mut last = String::new();
    let quiet = live_report(
        &listed(Vec::new()),
        now(),
        &mut last,
        true,
        false,
        &AtConnect::default(),
    )
    .expect("a bootstrapped store draws a report");
    assert!(
        !quiet.contains("does not read them again"),
        "a run that fetched nothing told the reader when it had read: {quiet}"
    );
}

/// A path that does not exist is exit 2 and a sentence naming the file, never a panic
/// (NOTES § D17).
#[test]
fn a_path_that_does_not_exist_is_an_error_and_names_itself() {
    let missing = fixture("no-such-fixture.json");
    let Err(problem) = load(std::slice::from_ref(&missing), now(), false) else {
        panic!("a file that is not there is not a snapshot")
    };

    assert!(problem.contains(&missing), "{problem}");
}

/// **A crafted path comes back out of the error clean.** argv is as untrusted as the API — a
/// shell glob expands whatever the directory is named — and a file does not have to exist for
/// its name to reach the screen. The strip is applied to the path as it *enters* the sentence,
/// not to the finished sentence ([`sanitize`]), so this is where it has to hold.
///
/// Both halves, as ever: nothing controlling survives, **and** the readable part of the name
/// still does. A `sanitize` that returned nothing would pass the first assertion and leave the
/// user an error that names no file (CLAUDE.md § A derived list asserts it found something).
#[test]
fn a_crafted_path_comes_back_out_of_the_error_with_nothing_unprintable_left() {
    // `ESC`, `CR`, a C1 control and a bidi override — the shapes the strip test already feeds
    // a `Finding`. A directory really can be named with one, and a glob really will expand it.
    let crafted = fixture("no-such\x1b[2J\r\u{9b}\u{202e}fixture.json");

    let Err(problem) = load(std::slice::from_ref(&crafted), now(), false) else {
        panic!("a file that is not there is not a snapshot")
    };

    // No `\n` is stripped first: this error is one line, and a break in the path would forge
    // a second ([`survivors`]).
    let survivors = survivors(&problem);
    assert!(
        survivors.is_empty(),
        "characters with no printed form reached the error: {survivors:?}\n{problem:?}"
    );
    assert!(
        problem.contains("no-such[2Jfixture.json"),
        "the path was stripped away along with the escape: {problem:?}"
    );
}

/// So is a file that is not JSON. `K8S_VERSION` is a committed one-line text file, which is
/// exactly the mistake a user makes with a shell glob.
#[test]
fn a_file_that_is_not_json_is_an_error_and_names_itself() {
    let path = fixture("K8S_VERSION");
    let Err(problem) = load(std::slice::from_ref(&path), now(), false) else {
        panic!("a version string is not a snapshot")
    };

    assert!(
        problem.contains(&path) && problem.contains("not JSON"),
        "{problem}"
    );
}

/// **A `kind` that is present and is not text is not a missing field.** Four shapes reach the
/// same arm and one label has to be true of all four ([`take`]): no `kind` at all, `{"kind":42}`,
/// the top-level array `kubectl get … -o json | jq '.items'` produces, and a bare `null`. Only
/// the first was ever fed, and the message it wrote — *no kind field* — was false for the other
/// three (NOTES § D29).
///
/// They are handed to [`take`] the way [`load`] hands them over: the `items` lookup there only
/// fires on an object that has one, so each of these arrives whole.
///
/// **Crafted, not captured**, for the same reason as the malformed Pod below: no cluster hands
/// out a document with no kind, and this is the shape a user's own edited file has.
#[test]
fn a_document_with_nothing_that_names_a_kind_does_not_claim_the_field_is_missing() {
    let mut input = nothing_read();
    for text in [
        r#"{"metadata":{"name":"web-0"}}"#,
        r#"{"kind":42}"#,
        r#"[{"kind":"Pod"}]"#,
        "null",
    ] {
        let doc: Value = serde_json::from_str(text).expect("the crafted document is JSON");
        take(doc, &mut input).unwrap_or_else(|e| panic!("{text} is not a failure: {e}"));
    }

    assert_eq!(
        input.skipped,
        BTreeMap::from([("(no kind)".to_string(), 4)]),
        "the four shapes did not come out under one label that is true of all of them"
    );
    assert_eq!(
        header(&input),
        "0 pods · 0 nodes · 4 objects no rule reads ((no kind))"
    );
}

/// A document whose kind we claim to understand and whose body will not decode is exit 2
/// naming the kind — never a snapshot quietly missing an object ([`load`] § `Err` is the
/// exit-2 path).
///
/// **The one input in this file that is not a capture**, and it has to be: no cluster hands
/// out a malformed Pod, so there is nothing to capture (CLAUDE.md § Fixtures come from real
/// cluster captures — this is not a fixture, it is the shape a user's own edited file has).
#[test]
fn a_document_of_a_known_kind_that_will_not_decode_is_an_error_naming_the_kind() {
    let doc: Value = serde_json::from_str(r#"{"kind":"Pod","spec":{"containers":"web-0"}}"#)
        .expect("the crafted document is JSON");
    let mut input = nothing_read();

    let Err(problem) = take(doc, &mut input) else {
        panic!("a Pod whose containers are a string is not a Pod")
    };

    assert!(problem.contains("a Pod did not decode"), "{problem:?}");
}

/// Several paths make **one** snapshot — that is the whole point of taking more than one.
#[test]
fn several_paths_make_one_snapshot() {
    let input = read(&["oom.json", "nodes.json", "deployments.json"]);

    assert_eq!(input.snapshot.pods.len(), 1);
    assert_eq!(input.snapshot.nodes.len(), items_in("nodes.json"));
    assert_eq!(input.snapshot.workloads.len(), items_in("deployments.json"));
    assert_eq!(input.snapshot.now, now(), "the clock is the caller's");
}

/// The clock `main` hands the rules is the wall clock, in seconds — not milliseconds read as
/// seconds, and not the epoch, both of which arrive as a `Time` that compiles and dates every
/// card wrong (invariant 5, [`Finding::timestamp`]).
#[test]
fn the_wall_clock_reads_the_wall_clock() {
    let read = wall_clock().expect("this machine's clock reads");

    let repo_era: k8s_openapi::jiff::Timestamp =
        "2026-08-16T00:00:00Z".parse().expect("a fixed timestamp");
    // **The upper bound is the half that carries the milliseconds claim.** A millis-for-seconds
    // read lands ~56 000 years out, and what stops that today is somebody else's ceiling —
    // `jiff`'s `UnixEpochSeconds` maximum, `253402300799`, which makes `Timestamp::new` error and
    // the `expect` above fire. That is real and it is silent: the day the ceiling widens, or the
    // clock is only a few times wrong rather than a thousand, the lower bound alone passes.
    let no_later_than: k8s_openapi::jiff::Timestamp =
        "2100-01-01T00:00:00Z".parse().expect("a fixed timestamp");
    assert!(
        read.0 > repo_era && read.0 < no_later_than,
        "the clock read {read:?}, which is not a moment this program is running in"
    );
}

// --- WHAT `main` IS A WRAPPER AROUND ---

/// No arguments is not a crash and not an empty report: it is the usage text and exit 2
/// (NOTES § D17). The text has to say this build cannot reach a cluster, because the name
/// promises one.
#[test]
fn no_arguments_is_the_usage_text_and_not_a_report() {
    let Err(problem) = run(&[]) else {
        panic!("no arguments is not a report")
    };

    assert!(problem.starts_with("usage: k8rs "), "{problem}");
    assert!(problem.contains("cannot reach a cluster"), "{problem}");
}

/// **…and it is still three lines when it gets there.** A line break is unprintable by
/// [`crate::sanitize`]'s predicate, so a strip run over the *assembled* message instead of
/// over the values that entered it eats k8rs's own line breaks and prints the three sentences
/// as one run-on line — with the two spaces missing where the breaks were. That is what the
/// first thing a new user ever sees looked like until the strip moved to the interpolations
/// ([`sanitize`]).
#[test]
fn the_usage_text_keeps_its_three_lines() {
    let Err(problem) = run(&[]) else {
        panic!("no arguments is not a report")
    };

    let lines: Vec<&str> = problem.lines().collect();
    assert_eq!(lines.len(), 3, "{problem:?}");
    assert!(
        !lines[0].contains("Each file holds"),
        "the usage text was joined into one run-on line: {problem:?}"
    );
    assert!(
        lines.iter().all(|l| !l.trim().is_empty()),
        "a usage line came out blank: {problem:?}"
    );
}

/// The whole path `main` wraps, over a committed capture: read the file, run the rules,
/// render. The healthy pod is the one whose report does not move with the clock — every other
/// fixture carries an age, and `run` reads the real one.
#[test]
fn a_healthy_capture_runs_end_to_end_and_reports_nothing_broken() {
    assert_eq!(
        run(&[fixture("healthy.json")]),
        Ok("1 pod · 0 nodes\n\n○ nothing is broken".to_string())
    );
}

/// **A reader that closed the pipe costs nothing; a write that failed for any other reason
/// costs the report.** `println!` panicked on both — exit 101 and a backtrace, a code D17's
/// table does not have — and `head`, or `less` quit on the first page, is the pipeline working
/// (`screens/once.md` § Colour and symbols sells `| less`). The other arm is
/// `k8rs > findings.txt` onto a full disk, where silence would leave a truncated report looking
/// like a whole one ([`stdout_failure`]).
///
/// **Both errors are the ones a real write returns**, by errno rather than by name, so neither
/// shape is invented (NOTES § D29). The exit codes themselves are `tests/binary.rs`'s: no unit
/// test can watch a process exit.
#[test]
fn a_closed_pipe_costs_nothing_and_any_other_failed_write_costs_a_sentence() {
    let closed_pipe = std::io::Error::from_raw_os_error(32);
    assert_eq!(
        closed_pipe.kind(),
        std::io::ErrorKind::BrokenPipe,
        "EPIPE is 32 here, or this test is feeding the other arm"
    );
    assert_eq!(
        stdout_failure(&closed_pipe),
        None,
        "`head` closing the pipe was reported as a failure"
    );

    let disk_full = std::io::Error::from_raw_os_error(28);
    let sentence = stdout_failure(&disk_full).expect("a report cut in half is not a success");
    assert!(sentence.starts_with("k8rs: "), "{sentence:?}");
    assert!(
        sentence.contains(&disk_full.to_string()),
        "the reason the write failed did not reach the user: {sentence:?}"
    );
}

/// A failure keeps the program's name on it, so a line in a CI log says who wrote it.
#[test]
fn a_failure_names_k8rs_and_the_file_that_stopped_it() {
    let missing = fixture("no-such-fixture.json");
    let Err(problem) = run(std::slice::from_ref(&missing)) else {
        panic!("a file that is not there is not a report")
    };

    assert!(
        problem.starts_with("k8rs: ") && problem.contains(&missing),
        "{problem}"
    );
}

// --- WATCHING A CLUSTER ---
//
// **The cluster is the one thing not synthesised here, because there is none.** What these feed
// [`live_report`] is a [`k8s::Store`] driven by hand through the same events a watch delivers —
// the shape `k8s_tests.rs` § THE DRIVER already proves the store lands — so what is tested here
// is only this file's half: when a report is printed at all, and when the same cluster is not
// printed twice.

use kube::runtime::watcher::{self, Event};

/// Every object of a committed `kind: List` capture, decoded.
fn objects<T: DeserializeOwned>(name: &str) -> Vec<T> {
    let text = std::fs::read_to_string(fixture(name)).expect("the fixture reads");
    let doc: Value = serde_json::from_str(&text).expect("the fixture is JSON");
    doc["items"]
        .as_array()
        .expect("the fixture is a List")
        .iter()
        .map(|item| serde_json::from_value(item.clone()).expect("the capture decodes"))
        .collect()
}

/// **A store whose five initial LISTs have all landed**, with the capture's pods on the pod
/// watch and nothing on the other four — an empty cluster is a real answer and the gate
/// (NOTES § D28) opens on `InitDone`, not on objects.
fn listed(pods: Vec<Pod>) -> k8s::Store {
    let mut store = k8s::Store::default();
    store.pod(&now(), Event::Init);
    for pod in pods {
        store.pod(&now(), Event::InitApply(pod));
    }
    store.pod(&now(), Event::InitDone);
    the_other_four(&mut store);
    store
}

/// The four watches these tests carry no objects on, each opened and closed. Written out rather
/// than looped: one `Store` method per API type is four different `fn` items, and that is
/// exactly the per-watch identity NOTES § D162 bought.
fn the_other_four(store: &mut k8s::Store) {
    store.node(&now(), Event::Init);
    store.node(&now(), Event::InitDone);
    store.deployment(&now(), Event::Init);
    store.deployment(&now(), Event::InitDone);
    store.stateful_set(&now(), Event::Init);
    store.stateful_set(&now(), Event::InitDone);
    store.daemon_set(&now(), Event::Init);
    store.daemon_set(&now(), Event::InitDone);
}

/// **Nothing is printed until every initial LIST has landed** (NOTES § D28).
///
/// A rule cannot tell a short list from a small cluster, so a report drawn mid-bootstrap says
/// *none of the 3 nodes have that label* about a 200-node cluster. The driver's answer is
/// silence, and the screen that replaces it draws [`k8s::Store::still_listing`] instead.
#[test]
fn a_bootstrap_that_has_not_finished_prints_nothing_at_all() {
    let mut last = String::new();
    assert_eq!(
        live_report(
            &k8s::Store::default(),
            now(),
            &mut last,
            false,
            false,
            &AtConnect::default()
        ),
        None
    );

    // Four of the five landed and the fifth never opened: still not a cluster anyone may read.
    let mut store = k8s::Store::default();
    the_other_four(&mut store);
    assert_eq!(
        live_report(
            &store,
            now(),
            &mut last,
            false,
            false,
            &AtConnect::default()
        ),
        None
    );
    assert!(
        last.is_empty(),
        "something was recorded as printed while the bootstrap was still running"
    );

    // **And still nothing after something else has been printed.** `last` is what the driver said
    // most recently, so a silent bootstrap has to stay silent *against a non-empty last* too —
    // an empty report is not a report, and printing one would put a blank block on stdout every
    // time a watch re-listed.
    let printed = live_report(
        &listed(Vec::new()),
        now(),
        &mut last,
        false,
        false,
        &AtConnect::default(),
    )
    .expect("a listed store");
    assert!(!printed.is_empty(), "the report is empty: {printed:?}");
    // `None` and not merely *empty*: `Some(String::new())` is a blank block on stdout, which is
    // what the driver would print every time a watch re-listed.
    assert_eq!(
        live_report(
            &store,
            now(),
            &mut last,
            false,
            false,
            &AtConnect::default()
        ),
        None,
        "a bootstrap with nothing wrong printed something after an earlier report"
    );
}

/// **The first complete answer prints, the same one again does not, and a change prints again.**
///
/// The middle claim is the whole of why this function exists: a watch delivers an event per
/// object per change and almost none of them move a finding, so a driver that printed on every
/// event would bury the one that did. The third is what the reconnect proof reads
/// (NOTES § D161) — a cluster that comes back is a report that appears with nobody touching the
/// keyboard.
#[test]
fn the_same_cluster_prints_once_and_a_changed_one_prints_again() {
    let mut store = listed(objects::<Pod>("kube-system-pods.json"));
    let mut last = String::new();

    let first = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect::default(),
    )
    .expect("every initial LIST landed");
    println!("{first}");
    assert!(
        first.contains(" pods · "),
        "the live report is not the report `render` draws"
    );
    assert_eq!(
        live_report(
            &store,
            now(),
            &mut last,
            false,
            false,
            &AtConnect::default()
        ),
        None,
        "the same cluster printed twice"
    );

    let crashloop: Pod = serde_json::from_str(
        &std::fs::read_to_string(fixture("crashloop.json")).expect("the fixture reads"),
    )
    .expect("the capture decodes");
    store.pod(&now(), Event::Apply(crashloop));
    let second = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect::default(),
    )
    .expect("a pod arrived, so the report moved");
    println!("{second}");
    assert!(
        second.contains("broken-crashloop"),
        "a pod that arrived after the bootstrap never reached the report"
    );
}

/// **A listed store that also knows the three facts no watch carries** (`k8s::Identity`,
/// NOTES § D169), with the committed nodes on the node watch.
///
/// The nodes arrive as `Apply` rather than `InitApply` because [`listed`] has already closed
/// every initial LIST — which is a real shape and not a shortcut: a node object arriving after
/// the bootstrap is what the watch delivers for the rest of the session.
fn identified(pods: Vec<Pod>, nodes: Vec<Node>, identity: k8s::Identity) -> k8s::Store {
    let mut store = listed(pods);
    store.identify(identity);
    for node in nodes {
        store.node(&now(), Event::Apply(node));
    }
    store
}

/// **The context and certificate of a reader whose login is nearly out** — the committed
/// `expiring-client` certificate, whose dates `scripts/make-certs.sh` pins and
/// `scripts/certs-test.sh` asserts, so it cannot expire out from under [`now`].
fn nearly_out(server_version: Option<&str>) -> k8s::Identity {
    k8s::Identity {
        server_version: server_version.map(str::to_string),
        context: Some("kind-k8rs".to_string()),
        client_certificate: Some(certificate("expiring-client")),
        // Every namespace — the scope every test that is not about scoping runs under, and the
        // one the committed captures were taken with.
        namespace_scope: None,
    }
}

/// **Whole days between [`now`] and the committed `expiring-client` certificate's `notAfter`** —
/// `scripts/certs-test.sh` asserts both ends of that subtraction and prints this same number in
/// its own summary line, so it is a figure a guard pins rather than one transcribed off a run.
const EXPIRES_IN_DAYS: u32 = 13;

/// **`--analysis` is honoured beside `--live` too, and it is still a flag** (NOTES § D169).
///
/// Same rule as the file path's [`the_analysis_flag_adds_every_pane_and_is_not_a_file`], and the
/// same spelling: the cards are unchanged and the seven panes go under them. Until this box the
/// flag was accepted and silently dropped in this mode, so **no** report had ever been drawn off
/// a cluster — and the two below have shapes only a cluster reaches.
#[test]
fn the_panes_are_drawn_live_only_when_the_flag_is_passed() {
    let store = identified(
        objects::<Pod>("kube-system-pods.json"),
        objects::<Node>("nodes.json"),
        nearly_out(Some("v1.36.1")),
    );

    let mut last = String::new();
    let plain = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect::default(),
    )
    .expect("every LIST landed");
    for pane in PANES {
        assert!(
            !plain.contains(pane),
            "{pane} is drawn on a live run that did not ask for it: {plain}"
        );
    }

    let mut last = String::new();
    let panes = live_report(&store, now(), &mut last, true, false, &AtConnect::default())
        .expect("every LIST landed");
    for pane in PANES {
        assert!(
            panes.contains(pane),
            "{pane} is missing from a live run: {panes}"
        );
    }
    // **The two reports differ by exactly one line and it is a ruling, not the cards moving**:
    // C1's expiring band prints as a trailer only on a run with no Certificates pane to draw it
    // as a row (`screens/once.md` § When your own login is running out). Everything above that
    // line is the same report, which is what this assertion is about.
    let cards = plain
        .strip_suffix(LOGIN_EXPIRING)
        .expect("this store's kubeconfig is nearly out, so the bare report ends on C1's trailer")
        .trim_end();
    assert!(
        panes.starts_with(cards),
        "the cards moved when the panes were asked for — the reports go under them, exactly as \
         the file path prints them: {panes}"
    );
    assert!(
        !panes.contains(LOGIN_EXPIRING),
        "the trailer and the Certificates row drew the same fact twice on one page: {panes}"
    );
}

/// **Versions draws the control plane and what it measured against it** — the shapes the binary
/// had never printed, because the driver hard-coded `server_version` to `None`
/// (NOTES § D169).
///
/// **Three shapes here and one before**: with no version the whole pane is one `NotComputed`, and
/// that is the only one any run of the binary had ever produced. With one it is the heading, the
/// control-plane line counting the kubelets it could compare, and either the machines that are
/// behind or the sentence that closes a pane that flagged nobody.
///
/// **The behind half comes from a control plane ahead of the committed nodes, not from an edited
/// capture** (NOTES § D53). The nodes are the fixture verbatim at `v1.36.1`; what moves is the
/// string the API server answered with, which is exactly what a cluster looks like between the
/// control-plane upgrade and the node one.
#[test]
fn versions_draws_the_control_plane_line_and_the_machines_behind_it() {
    let nodes = || objects::<Node>("nodes.json");
    let pane_of = |identity| {
        let store = identified(Vec::new(), nodes(), identity);
        let mut last = String::new();
        let printed = live_report(&store, now(), &mut last, true, false, &AtConnect::default())
            .expect("every LIST landed");
        let at = printed.find("[versions]").expect("the pane is drawn");
        printed[at..].to_string()
    };

    // **The state every earlier run of the binary was in**: nobody read a version, so nothing on
    // the pane can be measured against one.
    let unread = pane_of(k8s::Identity::default());
    assert!(
        unread.contains("k8rs could not read it"),
        "a run with no control-plane version did not say so: {unread}"
    );

    let matching = pane_of(nearly_out(Some("v1.36.1")));
    assert!(
        matching.contains("Control plane v1.36.1 · 4 of 4 kubelets match"),
        "the control-plane line is not drawn from the version the server answered with, or the \
         four committed nodes never reached the report: {matching}"
    );
    assert!(
        matching.contains("Every machine is running the same version as the control plane."),
        "a pane that flagged nobody did not close on the sentence that says so: {matching}"
    );

    // **A control plane four releases ahead of its machines** — N4's own window is three
    // (NOTES § D81), so every one of them is a row.
    let behind = pane_of(nearly_out(Some("v1.40.0")));
    assert!(
        behind.contains("Control plane v1.40.0 · 0 of 4 kubelets match"),
        "the count did not move with the control plane's version: {behind}"
    );
    assert!(
        behind.contains("k8rs-control-plane") && behind.contains("k8rs-worker3"),
        "the machines too far behind the control plane are not drawn: {behind}"
    );
    assert!(
        !behind.contains("Nothing to do."),
        "a pane that flagged four machines still closed on `nothing to do`: {behind}"
    );
}

/// **Certificates draws C1's row and the sidebar badge** — the pane's only row a reader can open
/// a finding from, and the product's only duration badge (NOTES § D169, § D87).
///
/// **Neither had ever been printed by the binary.** C1's two inputs are the kubeconfig's context
/// name and its client certificate, and the driver hard-coded both to `None`, so every run this
/// repo has made drew the same pane: the CSR row that could not be checked, and nothing else.
///
/// **The badge is the expiring band's only route to a reader who has not opened the pane**, since
/// `Severity::Info` keeps C1 off Alerts — so a pane that draws the row and drops the badge is a
/// silent finding, which is why both are asserted here rather than one standing in for the other.
///
/// **The number is arithmetic and not a transcription** ([`EXPIRES_IN_DAYS`]): the committed
/// certificate's `notAfter` is pinned by `scripts/make-certs.sh` and [`now`] is the instant
/// `scripts/certs-test.sh` measures it from, so the two spellings — `13 days` in the row's
/// sentence and `13d` in the badge — are one subtraction seen twice, in two implementations that
/// NOTES § D129 requires to agree.
#[test]
fn certificates_draws_c1s_row_and_the_sidebar_badge() {
    let printed = |identity| {
        let store = identified(Vec::new(), Vec::new(), identity);
        let mut last = String::new();
        let printed = live_report(&store, now(), &mut last, true, false, &AtConnect::default())
            .expect("every LIST landed");
        let at = printed.find("[certificates]").expect("the pane is drawn");
        let end = printed[at..].find("[drain safety]").expect("the next pane");
        printed[at..at + end].to_string()
    };

    // **The state every earlier run of the binary was in**: no kubeconfig reached the snapshot,
    // so the one finding about the reader's own machine could not exist.
    let silent = printed(k8s::Identity::default());
    assert_eq!(
        silent.lines().next(),
        Some("[certificates]"),
        "a pane with no certificate to report on drew a badge: {silent}"
    );
    assert!(
        !silent.contains("kubeconfig certificate"),
        "C1's row was drawn for a run that never read a kubeconfig: {silent}"
    );

    let pane = printed(nearly_out(Some("v1.36.1")));
    assert_eq!(
        pane.lines().next(),
        Some(format!("[certificates] {EXPIRES_IN_DAYS}d").as_str()),
        "the sidebar badge is missing or is not C1's countdown — it is the expiring band's only \
         route to a reader who has not opened this pane (NOTES § D87): {pane}"
    );
    assert!(
        pane.contains(&format!(
            "▲ Your kubeconfig certificate expires in {EXPIRES_IN_DAYS} days"
        )),
        "C1's row is missing, or is not drawn in the band this pane gives it: {pane}"
    );
    assert!(
        pane.contains("this is the file on your own machine that proves who you are"),
        "the row lost the rule's own evidence, so the reader is told a certificate expires and \
         not which one: {pane}"
    );
    assert!(
        pane.contains("→ ask whoever gave you access for a new kubeconfig"),
        "the row has no way out on it: {pane}"
    );
}

/// **Which cluster a run watches, or that it watches none.**
///
/// `--live` is what turns this driver into a cluster reader at all, and `--context` beside it is
/// how the machine running the reconnect proof names a cluster that is not its current one.
#[test]
fn live_is_the_flag_that_names_a_cluster_and_context_names_which_one() {
    let args = |line: &[&str]| -> Vec<String> { line.iter().map(|a| (*a).to_string()).collect() };

    assert_eq!(live_context(&args(&["pod.json"])), None);
    assert_eq!(live_context(&args(&["--analysis", "pod.json"])), None);
    assert_eq!(live_context(&args(&[])), None);
    // `--context` without `--live` is not a live run: the file path is what this driver reads.
    assert_eq!(live_context(&args(&["--context", "kind-k8rs"])), None);

    assert_eq!(live_context(&args(&["--live"])), Some(None));
    assert_eq!(
        live_context(&args(&["--live", "--context", "kind-k8rs"])),
        Some(Some("kind-k8rs"))
    );
    // **The spelling `kubectl` and every GNU tool accept.** Matching only the separated form let
    // this fall through to the kubeconfig's current context in silence, which for this flag is
    // watching a different cluster than the one the reader named (`tester`, 2026-08-27).
    assert_eq!(
        live_context(&args(&["--live", "--context=kind-k8rs"])),
        Some(Some("kind-k8rs"))
    );
    // **A `--context` with nothing after it is the current context here, and never reaches
    // here.** [`mistyped`] refuses it before this function is called (2026-08-30), so what this
    // asserts is that the second line still holds if the first one is ever moved: this function
    // alone answers *no context was named*, not *the context named `--live`*.
    assert_eq!(live_context(&args(&["--live", "--context"])), Some(None));
    assert!(mistyped(&args(&["--live", "--context"])).is_some());
    // **A flag is never a context name.** Both of these used to come back as the context called
    // `--live` / `--analysis`, and the truth arrived later as a kubeconfig error about a name
    // nobody typed.
    assert_eq!(live_context(&args(&["--context", "--live"])), Some(None));
    assert_eq!(
        live_context(&args(&["--live", "--context", "--analysis", "pod.json"])),
        Some(None)
    );
    // An `=` says the value was meant, so a flag-shaped one after it is kept.
    assert_eq!(
        live_context(&args(&["--live", "--context=--analysis"])),
        Some(Some("--analysis"))
    );
    // First-wins, which is the opposite of `kubectl`'s last-wins — stated because it was stated
    // nowhere, and the real flag box (Phase 12) should follow `kubectl` rather than this.
    assert_eq!(
        live_context(&args(&["--live", "--context", "a", "--context", "b"])),
        Some(Some("a"))
    );
    // A longer flag that merely starts the same way is not this one.
    assert_eq!(
        live_context(&args(&["--live", "--contextual", "x"])),
        Some(None)
    );
    // `--context=` with nothing after the `=` is passed through empty rather than quietly
    // becoming the current context: the connect below it answers *no such context*, which is the
    // loud version of the same mistake.
    assert_eq!(
        live_context(&args(&["--live", "--context="])),
        Some(Some(""))
    );
}

/// **Which namespace a run watches**, in every spelling of the two flags that say so
/// (NOTES § D5).
///
/// **`None` here is not *the whole cluster*** — it is *do not narrow at this end*, and `k8s.rs`
/// decides the rest off what the cluster answers a cluster-wide pod list with.
#[test]
fn namespace_names_the_one_namespace_this_run_watches_in_either_spelling() {
    let args = |line: &[&str]| -> Vec<String> { line.iter().map(|a| (*a).to_string()).collect() };

    assert_eq!(live_namespace(&args(&[])), None);
    assert_eq!(live_namespace(&args(&["--live"])), None);
    assert_eq!(live_namespace(&args(&["--live", "--analysis"])), None);

    for line in [
        vec!["--live", "--namespace", "payments"],
        vec!["--live", "--namespace=payments"],
        vec!["--live", "-n", "payments"],
        vec!["--live", "-n=payments"],
        // Beside every other flag, in either order — the scan is over the whole line.
        vec![
            "--live",
            "--analysis",
            "--context",
            "prod",
            "-n",
            "payments",
        ],
        vec!["-n", "payments", "--live"],
    ] {
        assert_eq!(
            live_namespace(&args(&line)),
            Some("payments"),
            "{line:?} did not name the namespace it asked for, so the run is wider than the \
             reader said"
        );
    }

    // First-wins on repeats, [`live_context`]'s rule and not `kubectl`'s last-wins — written
    // down because an unwritten tie-break is the one that changes by accident.
    assert_eq!(
        live_namespace(&args(&["--live", "-n", "a", "--namespace", "b"])),
        Some("a")
    );
    // A longer flag that merely starts the same way is not this one.
    assert_eq!(
        live_namespace(&args(&["--live", "--namespaces", "x"])),
        None
    );
    // `-nginx` is deliberately not `-n ginx`: taking the attached shorthand would make a word
    // somebody plausibly types into a silent wrong scope. **`None` here is not the whole
    // answer** — a line carrying it never reaches this function, because [`mistyped`] refuses it
    // first (`a_namespace_joined_to_the_short_flag_is_refused_rather_than_dropped`). Until
    // 2026-08-29 nothing refused it and the run went cluster-wide, which is the silent wider
    // scope this spelling was rejected to avoid.
    assert_eq!(live_namespace(&args(&["--live", "-nginx"])), None);
    // Nothing after the flag is `None` here, and refused by [`mistyped`] before it is used.
    assert_eq!(live_namespace(&args(&["--live", "--namespace"])), None);
}

/// **A `--namespace` with nothing usable after it is refused, and `--context`'s is not**
/// (NOTES § D5, and [`mistyped`]'s own doc for the difference).
///
/// **The three shapes are one sentence apiece and they are the realistic ones.** `-n "$NS"` with
/// `NS` unset is the flag at the end of a line; `-n ""` is the same variable quoted; a flag in
/// the value position is `--namespace --analysis`. All three used to leave the run watching
/// **every** namespace — silently *wider* than the reader asked for, which is the opposite of
/// what the flag is for and has nothing on screen to notice it by.
///
/// **The fourth shape is the one that is not merely wrong but unsafe**: a value with a `/` or a
/// `..` in it is interpolated into `/api/v1/namespaces/{ns}/pods`.
#[test]
fn a_namespace_flag_with_nothing_usable_after_it_is_refused() {
    let line = |words: &[&str]| -> Vec<String> { words.iter().map(|w| (*w).to_string()).collect() };

    for missing in [
        vec!["--live", "--namespace"],
        vec!["--live", "-n"],
        vec!["--live", "--namespace="],
        vec!["--live", "-n="],
    ] {
        let problem = mistyped(&line(&missing)).unwrap_or_else(|| {
            panic!("{missing:?} was accepted, so the run watches every namespace instead of one")
        });
        assert_eq!(
            problem,
            format!("k8rs: --namespace needs the name of a namespace\n{USAGE}"),
            "{missing:?}"
        );
    }

    // **The last six are the shapes `path_safe` accepted and a namespace name does not**
    // (`k8s::namespace_name`, measured against a real API server on 2026-08-29). `PAYMENTS` and
    // `foo.bar` both come back `200` with an empty `items`, so the reader was shown
    // *nothing is broken* over a namespace that does not exist; the 64-character one is the
    // length bound, and argv is the first unbounded source a name has ever come from here.
    let too_long = "a".repeat(64);
    let enormous = "b".repeat(8192);
    for bad in [
        vec!["--live", "--namespace", "--analysis"],
        vec!["--live", "-n", "--live"],
        vec!["--live", "--namespace=../secrets"],
        vec!["--live", "-n", "kube system"],
        vec!["--live", "--namespace", "a/b"],
        vec!["--live", "--namespace=kube-system?watch=true"],
        vec!["--live", "--namespace", "PAYMENTS"],
        vec!["--live", "--namespace", "foo.bar"],
        vec!["--live", "--namespace", "-leading"],
        vec!["--live", "--namespace", "trailing-"],
        vec!["--live", "--namespace", too_long.as_str()],
        vec!["--live", "--namespace", enormous.as_str()],
    ] {
        let problem = mistyped(&line(&bad))
            .unwrap_or_else(|| panic!("{bad:?} was accepted as a namespace name"));
        assert!(
            problem.starts_with("k8rs: --namespace needs the name of a namespace, and ")
                && problem.contains("is not one")
                && problem.contains("usage: k8rs "),
            "{bad:?} → {problem}"
        );
        // **What is echoed is bounded** (`k8s::NAMESPACE_MAX`). A value refused *for* being eight
        // kilobytes long, printed back at eight kilobytes, is the same unbounded thing one line
        // later (the security gate's *sizes are bounded* row).
        let first = problem
            .lines()
            .next()
            .expect("the refusal has a first line");
        // **One sentence plus at most one namespace-name's worth of echo.** The cap is derived
        // from the bound it is about (`k8s::NAMESPACE_MAX`) rather than picked, so widening the
        // sentence by a clause does not quietly widen what may be echoed with it.
        assert!(
            first.chars().count() <= 200 + k8s::NAMESPACE_MAX,
            "{bad:?} was echoed back at {} characters: {first:.120}",
            first.chars().count()
        );
        let value = bad.last().expect("every line here ends in its value");
        assert!(
            value.chars().count() <= k8s::NAMESPACE_MAX || !first.contains(*value),
            "a value refused for its length was echoed back whole: {} characters",
            value.chars().count()
        );
    }

    // A control character in the value never reaches the terminal (invariant 9).
    let crafted = mistyped(&line(&["--live", "--namespace=pay\u{1b}[2Jments"]))
        .expect("an escape sequence is not a namespace name");
    println!("{crafted}");
    assert!(
        !crafted.contains('\u{1b}'),
        "an escape sequence in argv reached the terminal: {crafted:?}"
    );

    // Real namespaces, in every spelling, are not refused — and neither is a line with no
    // namespace flag on it at all.
    let longest = "a".repeat(63);
    for good in [
        vec!["--live", "--namespace", "payments"],
        vec!["--live", "--namespace=kube-system"],
        vec!["--live", "-n", "payments"],
        vec!["--live", "-n=default"],
        // Digits are a namespace name and a leading one is legal — `team-2`, `2048`.
        vec!["--live", "-n", "2048"],
        // The boundary itself, not one past it: a bound that refused a legal name would be the
        // same defect facing the other way.
        vec!["--live", "--namespace", longest.as_str()],
        vec!["--live"],
        vec!["--analysis", "pod.json"],
    ] {
        assert_eq!(mistyped(&line(&good)), None, "{good:?}");
    }
}

/// **A namespace joined to the short flag is refused, not dropped** (`k8s-admin` § R10 and
/// `tester`, both independently, 2026-08-29).
///
/// **The measured failure is the one the flag's own doc rejects the spelling to avoid.**
/// `k8rs --live -npayments` did not scope anything: the word is not a `--` word, so [`mistyped`]
/// never looked at it and it fell through as a stray positional — and the run went **cluster-wide
/// with no line on screen**. Refusing to *read* the attached form and refusing to *accept* it are
/// two different things, and only the second closes the hole.
///
/// **The `=` spellings are not this shape and must stay accepted**, which is the assertion that
/// keeps the refusal from swallowing the flag it is guarding.
#[test]
fn a_namespace_joined_to_the_short_flag_is_refused_rather_than_dropped() {
    let line = |words: &[&str]| -> Vec<String> { words.iter().map(|w| (*w).to_string()).collect() };

    for attached in [
        vec!["--live", "-npayments"],
        // The word the doc names: `-nginx` would silently mean the namespace `ginx`.
        vec!["--live", "-nginx"],
        vec!["--live", "-n../secrets"],
        vec!["-npayments", "--live"],
    ] {
        let problem = mistyped(&line(&attached)).unwrap_or_else(|| {
            panic!("{attached:?} was accepted, so the run is wider than the reader asked for")
        });
        assert!(
            problem.contains("write it as `-n <name>`") && problem.contains("usage: k8rs "),
            "{attached:?} was refused without naming the spelling that works: {problem}"
        );
        // **Nothing of the value is echoed.** What is wrong is the spelling, and the value is the
        // one word on this line with no bound on it.
        assert!(
            !problem.contains("payments") && !problem.contains("ginx"),
            "{attached:?} echoed the value back: {problem}"
        );
    }

    // **A file whose name begins `-n` is now a usage error**, which is the price of refusing the
    // prefix at all and is written down rather than discovered ([`NAMESPACE_SHORT`]'s doc).
    assert!(
        mistyped(&line(&["-notes.json"])).is_some(),
        "the refusal does not cover a path that begins with the flag, so the doc that says it \
         does is wrong in the direction that matters"
    );

    for spelled in [
        vec!["--live", "-n", "payments"],
        vec!["--live", "-n=payments"],
        vec!["--live", "--namespace", "payments"],
        // Not the short flag at all — a longer flag that merely starts the same way.
        vec!["--live", "--namespace=payments"],
        // Nor is a path that does not begin with it.
        vec!["--analysis", "pod.json"],
    ] {
        assert_eq!(
            mistyped(&line(&spelled)),
            None,
            "{spelled:?} was refused by the attached-form check, which is the flag itself"
        );
    }
}

/// **A mistyped flag is a usage error, not a missing file.**
///
/// `k8rs --live=true` used to be read as a path and came back
/// `--live=true: No such file or directory (os error 2)` — errno jargon about a file nobody
/// named, for a flag the usage line advertises (invariant 14). The wording of a *real* file
/// error is not this box's; the wording of *this* one is, because this box is what made the
/// typo plausible.
#[test]
fn a_word_that_starts_like_a_flag_and_is_not_one_is_a_usage_error() {
    let line = |words: &[&str]| -> Vec<String> { words.iter().map(|w| (*w).to_string()).collect() };

    for typo in ["--live=true", "--LIVE", "--analyse", "--contxt=prod"] {
        let problem = mistyped(&line(&[typo])).unwrap_or_else(|| {
            panic!("{typo} was read as a path, so its error will name a file nobody typed")
        });
        assert!(
            problem.starts_with(&format!("k8rs: {typo} is not a flag k8rs has")),
            "{problem}"
        );
        assert!(
            problem.contains("usage: k8rs "),
            "the sentence names the mistake and then does not say what the flags are: {problem}"
        );
    }

    // **The live line is checked too**, which is the half that was missing: the guard lived
    // inside the file-reading path, so `--live --contxt=prod` watched the current context and
    // said nothing at all about the typo.
    assert!(mistyped(&line(&["--live", "--contxt=prod"])).is_some());

    // **A flag where the context name should be is refused, not swallowed** (`k8s-admin`,
    // 2026-08-27). `live_context` turned it into `None` = the current context and the run watched
    // the wrong cluster in silence — measured as `k8rs --live --context --live` connecting to
    // `kind-review` with a normal banner. The realistic form is `--context "$CTX"` with `CTX`
    // unset. Both words are known flags, so this can only be seen as a pair.
    for swallowed in [
        vec!["--live", "--context", "--live"],
        vec!["--live", "--context", "--analysis", "pod.json"],
        vec!["--context", "--live"],
    ] {
        let problem = mistyped(&line(&swallowed)).unwrap_or_else(|| {
            panic!("{swallowed:?} was accepted, so the run watches a cluster nobody named")
        });
        assert!(
            problem.starts_with("k8rs: --context needs the name of a context, and --")
                && problem.contains("usage: k8rs "),
            "{problem}"
        );
    }
    // An `=` says the value was meant, so it is not this mistake. **`--context` with nothing
    // after it is now its own refusal** and no longer the current context — the sentence differs
    // by one clause, which is what this asserts
    // ([`the_flags_this_build_accepts_and_the_ones_it_now_names_instead_of_dropping`]).
    assert_eq!(mistyped(&line(&["--live", "--context=--analysis"])), None);
    assert_eq!(
        mistyped(&line(&["--live", "--context"])),
        Some(format!(
            "k8rs: --context needs the name of a context\n{USAGE}"
        ))
    );

    // Every flag this build has, in both modes, and in both spellings — none of them is a typo.
    for good in [
        vec!["--analysis", "pod.json"],
        vec!["--live"],
        vec!["--live", "--context", "kind-k8rs"],
        vec!["--live", "--context=kind-k8rs"],
        // One dash is not the shape this refuses: it is a path like any other, and *no such
        // file* is the true thing to say about it.
        vec!["-live"],
        vec!["pod.json"],
    ] {
        assert_eq!(mistyped(&line(&good)), None, "{good:?}");
    }
    assert!(
        run(&line(&["-live"])).is_err_and(|problem| problem.contains("No such file")),
        "a one-dash word stopped being read as a path"
    );
    // The flag this build does have still works, and a file beside it is still read.
    assert!(run(&[ANALYSIS.to_string(), fixture("healthy.json")]).is_ok());
}

/// **A runtime that would not start names the reason the operating system gave**, exactly as a
/// failed write does.
///
/// **The arm had no test because the sentence was inline in `main`** (`tester`, 2026-08-27) —
/// `tokio::runtime::Builder::build` cannot be made to fail on demand, so nothing could reach it
/// and it was quietly throwing its `std::io::Error` away with a `_`. Moved into a function over a
/// value, it is assertable like every other decision `main` makes.
///
/// **The io error is untrusted text like any other** ([`sanitize`]): a reason carrying a control
/// character must not reach the terminal, which is the same guard `stdout_failure` is under.
#[test]
fn a_runtime_that_would_not_start_says_what_the_machine_said() {
    let real = runtime_failure(&std::io::Error::from(std::io::ErrorKind::OutOfMemory));
    println!("{real}");
    assert!(
        real.starts_with("k8rs: this machine would not start the runtime a cluster needs — ")
            && real.len()
                > "k8rs: this machine would not start the runtime a cluster needs — ".len(),
        "the reason the machine gave was dropped, which is the generic string this box is \
         about: {real:?}"
    );

    let crafted = runtime_failure(&std::io::Error::other("too many\u{1b}[2Jopen files"));
    println!("{crafted:?}");
    assert!(
        crafted.contains("too many[2Jopen files"),
        "the reason was not carried through: {crafted:?}"
    );
    assert!(
        !crafted.contains('\u{1b}'),
        "an escape sequence in an io error's own text reached the terminal (invariant 9): \
         {crafted:?}"
    );
}

/// **[`live`] under `--live`, which is the mode that has no happy ending** — the sentence it came
/// back with.
///
/// **The `expect` is an assertion and not a convenience.** `None` is *`--once` ran and reported*,
/// and `--live` reaching it would mean the mode that must never stop had stopped with an exit
/// code of `0` — every test below would then have failed on the unwrap rather than passing on a
/// sentence nobody read.
///
/// **And the timeout is an assertion too.** `--live` has no deadline of its own by design
/// (NOTES § D150), so every caller cuts its streams with `take` to make the run end at all — a
/// change that merges one uncut stream into the pump makes all three of them hang instead of
/// fail, which the mutation gate reported as a 90-second `TIMEOUT` rather than a defect
/// (2026-08-30). A test that hangs is a test whose failure nobody reads.
async fn watching(connected: Result<k8s::Session, k8s::NotConnected>, analysis: bool) -> String {
    tokio::time::timeout(
        std::time::Duration::from_secs(20),
        live(connected, analysis, None),
    )
    .await
    .expect("--live never came back, and every caller here cuts its streams so that it must")
    .expect("--live has no ending that is not a sentence")
}

/// A client pointed at a name RFC 6761 reserves so that it can never resolve — the same double
/// `k8s_tests.rs` § CONNECTING builds, written twice because a test helper cannot cross from one
/// `*_tests` module to another (invariant 11 keeps `mod tests` private to its own product file).
fn offline() -> kube::Client {
    kube::Client::try_from(kube::config::Config::new(
        "http://k8rs.invalid"
            .parse()
            .expect("a URL this file wrote itself"),
    ))
    .expect("a client over plain http asks the machine for nothing")
}

/// **An outage is a printed line and so is the recovery** — the pair the reconnect proof reads
/// (NOTES § D161).
///
/// **Silence cannot carry it.** A watch that dies and comes back leaves the cluster exactly as it
/// was, so the rendered cards are the same text and a driver that only printed those would print
/// nothing for the outage and nothing again on the recovery — the same output a permanently dead
/// watch gives. What is asserted here is the two changes: the failure appears, and it goes away
/// on its own when the watch delivers again.
///
/// **The failures are real ones from a real client**, not a store poked into shape: five watches
/// against a name that cannot resolve, driven through the same `drive` the binary runs.
#[tokio::test]
async fn a_watch_that_stops_delivering_is_a_line_in_the_report_and_so_is_its_recovery() {
    use futures_util::stream::StreamExt;
    let watches = k8s::session(offline(), k8s::Coverage::Cluster)
        .await
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();
    let mut store = k8s::Store::default();
    k8s::drive_watching(watches, &mut store, |_| {}).await;

    let mut last = String::new();
    let failing = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect::default(),
    )
    .expect("five watches are failing");
    println!("{failing}");
    for kind in ["pods", "nodes", "Deployments", "StatefulSets", "DaemonSets"] {
        assert!(
            failing.contains(&format!("not getting {kind} from this cluster")),
            "a cluster that answers nothing said nothing about {kind}: {failing}"
        );
    }
    // **Jargon only inside backticks**, which is a narrowing of this assertion and not a
    // weakening of it (2026-08-27). It read `!failing.contains("watch")`, and the classifier box
    // made that unpassable: the security gate requires a refusal to name the missing verb, and
    // `watch` *is* the verb — quoted, because it is something to type into a `Role` rather than
    // English. What invariant 14 actually owes is that the sentence a reader has to understand
    // never needs it, and that is what is checked now: everything outside backticks.
    let english = prose(&failing);
    assert!(
        !english.contains("watch"),
        "the sentence a reader has to understand uses the word `watch` outside a quoted verb: \
         {english}"
    );

    // The same store again is not news…
    assert_eq!(
        live_report(
            &store,
            now(),
            &mut last,
            false,
            false,
            &AtConnect::default()
        ),
        None
    );

    // …and then every watch delivers a complete answer, which is what a reconnect looks like
    // from in here: the failure clears itself and the report says so without being asked.
    store.pod(&now(), Event::Init);
    store.pod(&now(), Event::InitDone);
    the_other_four(&mut store);
    let recovered = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect::default(),
    )
    .expect("the cluster came back");
    println!("{recovered}");
    assert!(
        !recovered.contains("not getting"),
        "the driver still says the cluster is unreadable after every watch delivered: {recovered}"
    );
    assert!(
        recovered.starts_with("0 pods · 0 nodes"),
        "a healthy report starts with something other than the report: {recovered:?}"
    );
    // **And the claim comes back on with them**, which is the half the assertion under `stale`
    // cannot carry alone: a driver that never said `nothing is broken` at all would satisfy that
    // one for free, so the population is pinned from both sides ([`health`]).
    assert!(
        recovered.contains("○ nothing is broken"),
        "five watches delivered a complete answer and the cluster was still not called healthy: \
         {recovered:?}"
    );

    // **And the outage the proof actually watches: one that arrives *after* a good bootstrap.**
    // The store keeps its last complete answer while a watch is down (NOTES § D162), so this is
    // the only shape where both halves are printed at once — the lines on top, the cards they
    // are a warning about underneath, one blank line between.
    let watches = k8s::session(offline(), k8s::Coverage::Cluster)
        .await
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();
    k8s::drive_watching(watches, &mut store, |_| {}).await;
    let stale = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect::default(),
    )
    .expect("an outage is news");
    println!("{stale}");
    let (unreadable, cards) = stale
        .split_once("\n\n")
        .unwrap_or_else(|| panic!("the two halves are not separated by a blank line: {stale:?}"));
    assert_eq!(
        unreadable.lines().count(),
        5,
        "the top half is not five lines, so the halves ran together: {stale:?}"
    );
    assert!(
        unreadable.lines().all(|line| line.starts_with("▲ k8rs")),
        "{stale:?}"
    );
    // **Neither half of the line may be false under a 403** (`k8s-admin`, 2026-08-27), and a
    // refusal is indistinguishable from an outage from in here. `right now` is a lie about a
    // permission problem and `out of date` is a lie about a list that is empty rather than stale.
    assert!(
        !unreadable.contains("right now") && !unreadable.contains("out of date"),
        "the degraded line makes a claim that is false for a standing refusal: {unreadable:?}"
    );
    assert!(
        cards.starts_with("0 pods · 0 nodes"),
        "the cards under the warning are not the report: {cards:?}"
    );
    // **A trouble line and a health claim may never stand in one report** ([`health`],
    // [`Input::watch_trouble`], the PM's ruling of 2026-08-29). [`render`] is fed that flag by
    // hand everywhere else, so this is the only place it is *derived* from a real store — and
    // the only place that can catch it being read off the wrong side of `troubles`.
    assert!(
        !cards.contains("nothing is broken"),
        "a report whose first five lines say the cluster could not be read went on to call it \
         healthy: {stale:?}"
    );
}

/// A loopback server that answers **every** request with a `403` — the failure shape
/// [`offline`] cannot produce and this test needs.
///
/// **`offline`'s unresolvable name is a `Fault::Unanswered`**, which `k8s::Watch::settled`
/// deliberately leaves out (NOTES § D28's *do not blank on a blip*): the watch stays pending,
/// [`k8s::Store::snapshot`] answers `None`, and a report with no cards in it cannot show what a
/// header left out. A refusal is the standing failure that counts as *answered*
/// ([`k8s::Fault::standing`], NOTES § D184), so it is the one shape where a never-listed watch
/// and a rendered header exist at the same time.
///
/// **The framing is the one `k8s_tests.rs` § THE LEGACY DISCOVERY FALLBACK, AGAINST A SERVER
/// already runs on** — read until a blank line, answer, keep the connection — written twice
/// because a test helper cannot cross from one `*_tests` module to another (invariant 11),
/// exactly as [`offline`] is. A response per read instead would desynchronise on the keepalive
/// connection kube retries a refused watch over, and a hyper protocol error classifies as
/// `Unanswered` — the shape this helper exists to avoid.
///
/// **What it deliberately does not carry, so nobody reads more fidelity into it than it has:
/// `details`, and the sentence a real refusal puts in `message`.** A refused `list` sends
/// `services is forbidden: User "…" cannot list resource "services" in API group "" at the
/// cluster scope` with `details: {group, kind}` beside it; this sends `forbidden` and no
/// `details` field at all. Nothing on this path reads either one today, so it costs nothing
/// here — but the read-only `ClusterRole` box still open in this phase plans to render
/// `status.message`, and the day it lands this stub prints `forbidden`, this test stays green,
/// and the sentence the reader would actually be shown is untested (`k8s-admin`, 2026-08-30).
/// **A server that answers every request with an empty list** — the shortest cluster that lets
/// all five watches finish their initial LIST, so the bootstrap gate opens and there is a report
/// (NOTES § D28).
///
/// **It is [`refusing`]'s opposite and is written beside it for that reason.** That one proves
/// what `--once` does when the cluster will not show it anything; this one proves what it does
/// when the cluster shows it everything there is, which is nothing. Between them they are the two
/// exit codes.
///
/// **One body for all five kinds, because an `ObjectList` with no items is shape-compatible with
/// every one of them** — `items` is what kube decodes, and `[]` decodes into `Vec<Pod>` and
/// `Vec<Node>` alike. The `resourceVersion` is there because a watch answer with none is
/// `Fault::Unanswered` (`k8s::Fault`), which would keep the gate shut and test the wrong thing.
///
/// **The watch kube opens after each initial LIST is accepted and never answered**, which is what
/// a real watch over a cluster where nothing is happening does. Not refused: a refusal is a
/// `k8s::Fault` of its own and would print its own trouble line.
///
/// **Answering that watch with the `List` body above — which this stub did until 2026-08-30 — is
/// not a watch stream, so kube records a watch failure and the report grows a `▲ k8rs is not
/// getting pods from this cluster` line.** The doc here used to say the failure landed too late to
/// matter, *"by then `InitDone` has landed on all five, the gate is open, and `--once` has
/// stopped"*, and `tester` measured both halves of that false against a request-logging listener
/// with this stub's wire behaviour: in three runs of six the pods watch was answered **before** the
/// fifth LIST was — `40.75ms WATCH pods` against `40.82ms LIST daemonsets` — so which side of the
/// gate it lands on is the machine's timing and not a property of the design; and in the other
/// three all five had listed first and the trouble line came out anyway, because **a socket
/// answered is not a store updated**. The same shape flaked `tests/binary.rs` 5 runs in 20.
///
/// **In *this* module it was not that race, and what it was is worse.** The two numbers above are
/// a whole process running uncut streams; here the streams are cut, so the failure never landed
/// *at all* — 10 runs of both tests over this client with the watch answered, 0 trouble lines
/// (`dev-core`, 2026-08-30, on a copy of the tree). It was one event away the whole time:
/// [`driven`] reading a third event instead of two turned every run into **all five** watches
/// carrying *"▲ k8rs is not getting … nothing usable came back when k8rs tried to `list` and
/// `watch` …"* — deterministically, not at some rate — and
/// `analysis_under_once_puts_the_panes_under_the_cards_and_without_it_there_are_none` still passed
/// over that report, because what it asserts is pane headings. **A stub that models a broken
/// cluster and a suite that cannot see it is the loaded gun**, and the next test written here is
/// what pulls the trigger.
///
/// **The mechanism is `tests/binary.rs` § `Watches::HeldOpen`'s and deliberately not a second one**
/// — written twice only because a helper cannot cross from a private `mod tests` into `tests/`
/// (invariant 11), the same reason the body above is. What that file carries and this one does not
/// is its `Cut` variant, and the reason is the cost of the fix: a held-open watch has **no** third
/// event, so a test here that reads past `InitDone` — one counting reports, one waiting for a
/// re-list — hangs instead of failing. Nothing here reads that far today. The first one that needs
/// to takes `Cut` with it rather than reverting this.
async fn emptied() -> kube::Client {
    empty_lists_from(None).await
}

/// [`emptied`], with one URL held back and counted — the shape a metrics-server slower than the
/// pod LIST has (`reports/2026-08-30-once-flag-against-a-live-cluster.md` § 4d).
///
/// **`held` is matched against the whole request head**, so a caller passes the path fragment it
/// wants delayed and nothing here has to parse HTTP. It is the request head and not the body
/// because every call on this path is a `GET`.
///
/// **The counter is how *not sending a request* is asserted at all.** A poll merged into the
/// watch loop and a fetch awaited at connect produce the same report; what tells them apart is
/// that one of them asks twice. Nothing else about a request k8rs did not need is visible from
/// inside the process.
async fn emptied_but_slow_on(
    held: &'static str,
    by: std::time::Duration,
) -> (kube::Client, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let client = empty_lists_from(Some((held, by, std::sync::Arc::clone(&asked)))).await;
    (client, asked)
}

async fn empty_lists_from(
    held: Option<(
        &'static str,
        std::time::Duration,
        std::sync::Arc<std::sync::atomic::AtomicUsize>,
    )>,
) -> kube::Client {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port");
    let address = listener.local_addr().expect("the port it picked");
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let held = held.clone();
            tokio::spawn(async move {
                let body = r#"{"apiVersion":"v1","kind":"List",
                    "metadata":{"resourceVersion":"1"},"items":[]}"#;
                let sent = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\n\r\n{body}",
                    body.len()
                );
                let mut pending = String::new();
                loop {
                    let mut chunk = [0_u8; 2048];
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => pending.push_str(&String::from_utf8_lossy(&chunk[..read])),
                    }
                    // A LIST is a GET with no body, so a request ends at the blank line.
                    while let Some(end) = pending.find("\r\n\r\n") {
                        let head: String = pending.drain(..end + 4).collect();
                        // **The watch, accepted and never answered** — [`emptied`]'s doc carries
                        // the measurement that put this here. Nothing is written back, so hyper
                        // never puts a second request on this connection and the loop stays a
                        // queue; the socket blocks on its next `read` until the process exits.
                        //
                        // **Before the delay below, so a watch is never counted as one of the
                        // requests k8rs chose to send.** [`emptied_but_slow_on`]'s counter is
                        // there to tell a fetch from a poll, and a watch is neither.
                        if head.contains("watch=true") {
                            continue;
                        }
                        if let Some((held, by, counted)) = held.as_ref()
                            && head.contains(held)
                        {
                            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                            tokio::time::sleep(*by).await;
                        }
                        if socket.write_all(sent.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                }
            });
        }
    });
    kube::Client::try_from(kube::config::Config::new(
        format!("http://{address}")
            .parse()
            .expect("an address the kernel just gave us"),
    ))
    .expect("a client over plain http asks the machine for nothing")
}

async fn refusing() -> kube::Client {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port");
    let address = listener.local_addr().expect("the port it picked");
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                // **The `code` is the field that reaches `Fault::Refused`**: `k8s::answer`
                // matches on it first and falls through to `reason` only for a code it does not
                // name. Re-run here with this body, with the `reason` field deleted, and with an
                // empty body — an empty one is not JSON, so kube's own fallback rebuilds a
                // `Status` around the HTTP code (`k8s::answer`'s § table) — and all three
                // classify identically. The `reason` is here because a real API server sends one,
                // not because this path reads it.
                let body = r#"{"kind":"Status","apiVersion":"v1","status":"Failure",
                    "message":"forbidden","reason":"Forbidden","code":403}"#;
                let sent = format!(
                    "HTTP/1.1 403 Forbidden\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\n\r\n{body}",
                    body.len()
                );
                let mut pending = String::new();
                loop {
                    let mut chunk = [0_u8; 2048];
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => pending.push_str(&String::from_utf8_lossy(&chunk[..read])),
                    }
                    // A LIST is a GET with no body, so a request ends at the blank line.
                    while let Some(end) = pending.find("\r\n\r\n") {
                        pending.drain(..end + 4);
                        if socket.write_all(sent.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                }
            });
        }
    });
    kube::Client::try_from(kube::config::Config::new(
        format!("http://{address}")
            .parse()
            .expect("an address the kernel just gave us"),
    ))
    .expect("a client over plain http asks the machine for nothing")
}

/// The five watches driven against [`refusing`] over a store whose successful LISTs — if it has
/// any; one caller hands in none — have already been handed in, and the report [`live_report`]
/// then draws.
///
/// **The failures are real ones from a real client**, the sentence the reconnect proof one test
/// up writes about its own — five watches against a listener that refuses everything, driven
/// through the same pump the binary runs. What the caller hands in by hand is the *successful*
/// LIST that makes a watch stale rather than blank, which is what [`listed`] does everywhere else
/// in this module; a failure cannot be poked in at all, because `watcher::Event` has no arm for
/// one.
async fn refused_over(store: &mut k8s::Store) -> String {
    use futures_util::stream::StreamExt;
    let watches = k8s::session(refusing().await, k8s::Coverage::Cluster)
        .await
        .watches
        .into_iter()
        // `Ok(Init)` and then the refused initial LIST — kube emits the first without awaiting
        // anything (`k8s::StandingBackoff` documents the pair), so two is one failure per watch
        // and no backoff is ever waited on.
        .map(|watch| watch.take(2).boxed())
        .collect();
    k8s::drive_watching(watches, store, |_| {}).await;
    let mut last = String::new();
    live_report(store, now(), &mut last, false, false, &AtConnect::default())
        .expect("five refused watches are news whatever else the store holds")
}

/// **A watch that listed once and then broke has stale data; a watch that never listed has
/// none, and only the second is unreadable** ([`Input::unreadable`], NOTES § D184).
///
/// **Both shapes have to stand in one store or the *discrimination* is never asserted, and the
/// store has to be built both ways round or the discriminator is never told from a coincidence
/// of it.** Every other test here drives all five watches into the same state, and against a
/// store like that [`live_report`]'s `never_listed` and its complement differ only in size:
/// deleting the `!` puts all five in [`Input::unreadable`] and the header comes out empty, which
/// the neighbour above catches on its `0 pods · 0 nodes`. So the mutation is caught there **as a
/// blank**, and the thing the field exists to prevent — a count printed for a list nobody read —
/// is what no store in this file could produce. One arrangement is still not enough: with Pod the
/// only kind that listed *and* the only kind carrying objects, `!trouble.listed` and
/// `trouble.kind != Pod` select the same four watches. The mirror — nodes listed, pods never —
/// is what tells them apart, and it is the only case in this file that reaches
/// `header`'s `read(&ObjectKind::Pod) == false` *from a store*: the one other test that blanks
/// the pod vital sets `Input::unreadable` on the value by hand and never goes through
/// [`live_report`] at all (`tester`, 2026-08-30, D29).
///
/// **The counts are the captures' own and are asserted with `==`**, the convention
/// `a_health_claim_is_never_made_over_a_watch_that_could_not_be_read` already uses two screens
/// down. A `0` proves nothing here — it is textually what a blanked vital would print — and
/// `contains` would not have separated the two halves either: `"10 pods".contains("0 pods")` is
/// true, and `!header.contains("node")` fails on a correct namespace-scoped header
/// (`ns: node-pool · 1 pod`) and on the context field `header`'s own doc reserves.
///
/// **The order is the whole setup, and a cluster reaches it in two steps rather than one.**
/// `InitDone` clears a watch's failure (`k8s::Watch::take`), so the successful LIST has to land
/// *before* the refusal. A plain namespaced `Role` is not what produces that: measured against a
/// real one, it leaves the pod watch healthy and out of `troubles()` altogether, with only
/// `nodes` in there and `listed: false`
/// (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R2). What produces it is a
/// **pods-only** `Role` first, so four kinds are refused before they ever list, and *then* that
/// `Role` narrowed or the token expiring, so pods refuse on a later re-list. Both steps are
/// ordinary — a pods-and-logs-only `Role` is common, and Kubernetes does not re-authorize a watch
/// already in flight, so the refusal can only land on the next one (NOTES § D162).
#[tokio::test]
async fn a_watch_that_never_listed_is_unreadable_and_one_that_listed_and_broke_is_merely_stale() {
    let mut pods_read = k8s::Store::default();
    pods_read.pod(&now(), Event::Init);
    for pod in objects::<Pod>("kube-system-pods.json") {
        pods_read.pod(&now(), Event::InitApply(pod));
    }
    pods_read.pod(&now(), Event::InitDone);

    let mut nodes_read = k8s::Store::default();
    nodes_read.node(&now(), Event::Init);
    for node in objects::<Node>("nodes.json") {
        nodes_read.node(&now(), Event::InitApply(node));
    }
    nodes_read.node(&now(), Event::InitDone);

    for (mut store, shapes, expected) in [
        (
            pods_read,
            vec![
                (ObjectKind::Pod, true),
                (ObjectKind::Node, false),
                (ObjectKind::Deployment, false),
                (ObjectKind::StatefulSet, false),
                (ObjectKind::DaemonSet, false),
            ],
            "14 pods",
        ),
        (
            nodes_read,
            vec![
                (ObjectKind::Pod, false),
                (ObjectKind::Node, true),
                (ObjectKind::Deployment, false),
                (ObjectKind::StatefulSet, false),
                (ObjectKind::DaemonSet, false),
            ],
            "4 nodes",
        ),
    ] {
        let report = refused_over(&mut store).await;
        println!("{report}");

        // **The store really is carrying both shapes, and which watch is which**, asserted before
        // the report is read: without it the header below could be right for the wrong reason.
        //
        // **What it does not catch is a stub that stopped refusing.** `troubles()` filters on
        // `failure.is_some() || ended` and `listed` is `complete`, so neither field can see the
        // fault class — answering `500` instead of `403` leaves this vector byte-identical
        // (measured, `tester` 2026-08-30). It is still non-vacuous: an empty `troubles()`, or the
        // `true` sitting on the wrong row, fails it outright.
        let held: Vec<(ObjectKind, bool)> = store
            .troubles()
            .iter()
            .map(|trouble| (trouble.kind.clone(), trouble.listed))
            .collect();
        println!("{held:?}");
        assert_eq!(
            held, shapes,
            "the store does not hold one listed-then-broken watch beside four that never listed, \
             so nothing below can tell the two apart"
        );

        // **This is what catches a stub that stopped refusing**, and it is the reason the panic
        // is spelled out rather than an `unwrap`: a fault that is not *standing* leaves the four
        // watches unsettled, [`k8s::Store::snapshot`] answers `None`, and there is no card block
        // to split off at all.
        let cards = report
            .split_once("\n\n")
            .unwrap_or_else(|| panic!("a refused watch published no snapshot at all: {report:?}"))
            .1;
        // **The stale kind keeps its measured count and the never-listed kind is left out** — the
        // first half is `screens/widgets.md` § 1a's *stale vitals stay visible*, the second is the
        // defect [`Input::unreadable`] exists for, and one `==` is what asserts both at once.
        assert_eq!(
            cards.lines().next().expect("the cards begin with a header"),
            expected,
            "a vital was blanked after it had been read, or printed as a measured-looking count \
             for a list nobody was allowed to read: {cards:?}"
        );
    }
}

/// **A report with nothing but trouble lines in it ends on the last trouble line.**
///
/// The shape is a kubeconfig granting none of the five kinds: every watch is refused before it
/// lists, so no vital may be printed, there are no cards, no health claim may be made, and
/// [`render`] correctly answers `""`. [`live_report`] pushed that empty block anyway, with a
/// blank-line separator in front of it, and [`run`]'s own `\n` turned the one trailing newline
/// into two (`tester`, 2026-08-30) — the exact shape [`render`]'s trailer comment says it
/// prevents, reintroduced one layer up.
#[tokio::test]
async fn a_report_that_is_only_trouble_lines_does_not_end_in_a_blank_line() {
    let mut store = k8s::Store::default();
    let report = refused_over(&mut store).await;
    println!("{report:?}");
    assert_eq!(
        report.lines().count(),
        5,
        "a report with no readable vital, no card and no claim in it grew a line that is not a \
         trouble line: {report:?}"
    );
    assert!(
        !report.ends_with('\n'),
        "a report with no readable vital, no card and no claim in it did not end on its last \
         trouble line, so [`run`]'s own newline makes two blank ones: {report:?}"
    );
}

/// A `Status` the API server would send, wrapped exactly as kube wraps one — the same double
/// `k8s_tests.rs` § RESOLVING AN OWNER builds, written twice because a test helper cannot cross
/// from one `*_tests` module to another (invariant 11).
/// **The English of a line, with everything inside backticks taken out** — what invariant 14
/// is owed, once the security gate's *name the verb and the resource* has put RBAC verbs on the
/// screen.
///
/// **It asserts the backticks are balanced, because otherwise it degrades in silence**
/// (`tester`, 2026-08-27, CLAUDE.md § *a derived list asserts it found something*). With an odd
/// count `step_by(2)` keeps the wrong halves, so prose slides into the discarded side and the
/// assertion passes over a line nobody checked.
fn prose(line: &str) -> String {
    let parts: Vec<&str> = line.split('`').collect();
    assert!(
        parts.len() % 2 == 1,
        "the line has an odd number of backticks, so splitting on them keeps the wrong halves \
         and any assertion over the result is meaningless: {line:?}"
    );
    parts.into_iter().step_by(2).collect()
}

fn api_error(code: u16, reason: &str) -> kube::Error {
    kube::Error::Api(
        kube::core::Status::failure("refused", reason)
            .with_code(code)
            .boxed(),
    )
}

/// **The three things a watch can be failing of, said apart** — the box's own clause, and the
/// defect `PRIOR-ART § C1` catalogues if any two of them come out as one sentence.
///
/// **The line was true of all three before this and said nothing about which** (`k8s-admin`,
/// 2026-08-27): `unreadable` read `failure.is_some()` and stopped there, so a reader whose
/// kubeconfig is not allowed to see pods and a reader whose cluster is down got the same words.
/// Both halves of the frame around the clause still have to hold for every one of them, which is
/// what the `right now` / `out of date` assertions carry forward.
///
/// **The `403` names a verb and a resource because the security gate requires it**, and the
/// resource is the plural a `Role` spells: `statefulsets`, not `StatefulSets`.
#[test]
fn a_refusal_an_expired_login_and_a_dead_cluster_are_three_different_lines() {
    let refused = watcher::Error::InitialListFailed(api_error(403, "Forbidden"));
    let expired = watcher::Error::WatchError(
        kube::core::Status::failure("expired", "Unauthorized")
            .with_code(401)
            .boxed(),
    );
    let dead = watcher::Error::WatchFailed(kube::Error::Service(Box::new(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "timed out",
    ))));
    let line = |failure, kind, renewal| {
        let lines = unreadable(
            &[k8s::Trouble {
                kind,
                listed: false,
                failure: Some(failure),
                ended: false,
                unfinished: false,
                outstanding: None,
            }],
            renewal,
            None,
            false,
        );
        let [line] = lines.as_slice() else {
            panic!("one trouble did not make one line: {lines:?}")
        };
        println!("{line}");
        line.clone()
    };

    let refusal = line(&refused, ObjectKind::StatefulSet, None);
    assert!(
        refusal.contains("the role this kubeconfig uses needs to `list` and `watch` statefulsets"),
        "a refusal does not name the verb and the resource the security gate asks for: \
         {refusal:?}"
    );

    let timeout = line(&expired, ObjectKind::Pod, Some("aws"));
    assert!(
        timeout.contains("no longer accepts this login") && timeout.contains("`aws`"),
        "an expired login is not told apart from a refusal, or does not name the program this \
         kubeconfig logs in with (NOTES § D19): {timeout:?}"
    );
    assert!(
        !timeout.contains("the role"),
        "an expired login reads as a missing permission, which sends a beginner to their \
         platform team over a timeout: {timeout:?}"
    );

    let outage = line(&dead, ObjectKind::Node, Some("aws"));
    assert!(
        outage.contains("nothing usable came back"),
        "a dead cluster does not say so: {outage:?}"
    );
    assert!(
        !outage.contains("the role") && !outage.contains("login"),
        "a cluster that is down is reported as a permission or credential problem: {outage:?}"
    );

    // The three are three, not two that happen to differ from a third.
    assert_ne!(refusal, timeout);
    assert_ne!(timeout, outage);
    assert_ne!(refusal, outage);

    // And every one of them keeps the frame that has to be true of all three.
    for line in [&refusal, &timeout, &outage] {
        assert!(
            !line.contains("right now") && !line.contains("out of date"),
            "the line claims something a standing refusal makes false: {line:?}"
        );
    }
}

/// **The two things this tool says about itself, and neither may be false on the shape it does
/// not name.**
///
/// **`right now` and `out of date` were both lies under a 403** (`k8s-admin`, 2026-08-27): a
/// refusal is not *right now*, it is until somebody edits RBAC, and nothing **is** shown about
/// that kind — the list is empty, not stale. The clause the classifier added is now what tells
/// them apart; this frame still has to be true of both.
///
/// **`ended` gets the heavier glyph.** *Will not change again* is the most severe thing this tool
/// can say about itself and it was wearing `▲`, the same mark as the merely-degraded line. This
/// branch had no test at all before: no stream a test can build ends, because kube's `watcher()`
/// cannot, and `Watch::ended` is private to `k8s.rs` — which is why `unreadable` takes the
/// troubles rather than the store.
///
/// **The `failure: None` shape is the one place a fallback string is allowed**, which is the
/// second box's own rule read the right way round: *nothing was ever said about why* is printed
/// only for the case it actually describes — a stream that finished carrying no error at all.
#[test]
fn what_the_driver_says_about_itself_is_true_of_a_refusal_and_of_an_outage() {
    let trouble = |kind, ended| k8s::Trouble {
        kind,
        listed: false,
        failure: None,
        ended,
        unfinished: false,
        outstanding: None,
    };
    let degraded = unreadable(&[trouble(ObjectKind::Node, false)], None, None, false);
    let [degraded] = degraded.as_slice() else {
        panic!("one trouble did not make one line: {degraded:?}")
    };
    println!("{degraded}");
    assert!(
        degraded.starts_with("▲ k8rs is not getting nodes from this cluster"),
        "{degraded:?}"
    );
    assert!(
        !degraded.contains("right now") && !degraded.contains("out of date"),
        "the degraded line claims something a standing refusal makes false: {degraded:?}"
    );

    let stopped = unreadable(&[trouble(ObjectKind::Pod, true)], None, None, false);
    let [stopped] = stopped.as_slice() else {
        panic!("one trouble did not make one line: {stopped:?}")
    };
    println!("{stopped}");
    assert!(
        stopped.starts_with("● k8rs has stopped receiving pods from this cluster"),
        "a watch that will never deliver again wears the warning glyph, not the severe one: \
         {stopped:?}"
    );
    for line in [degraded, stopped] {
        assert!(
            line.contains("nothing was ever said about why"),
            "a stream that carried no error got a sentence about something else: {line:?}"
        );
        // **Jargon only inside backticks** (invariant 14). `list` and `watch` are RBAC verbs a
        // reader has to type into a `Role`, so a refusal names them literally — but the English
        // around them may never need them, and this is what fails if the frame borrows one.
        let english = prose(line);
        assert!(
            !english.contains("watch"),
            "the sentence a reader has to understand uses the word `watch` outside a quoted \
             verb: {english:?}"
        );
    }
}

/// **The line about a kind the run stopped waiting for states NOTES § D150's two numbers and
/// never a cause** (`k8s::Trouble::outstanding`, [`read_so_far`]).
///
/// **The shape that caught the defect is the second one, and it had no test at all**
/// (`k8s-admin`, 2026-09-03; NOTES § D29 — a check is proven only for the shapes it was fed).
/// Every wedge test used `so_far == 0`, and the negative that guards D150 holds *pods*, which is
/// the one kind [`out_of_time`] exempts — so nothing in the suite could see a **non-pod LIST that
/// was still moving at the deadline**. `k8rs --once -n payments` against a 2 000-node cluster is
/// exactly that: pods land in a second, nodes is cluster-scoped whatever the scope is, and at the
/// deadline the nodes LIST holds 1 500 objects with a stamp from this millisecond. The line for
/// it said *it is the cluster, or the network in between, that has gone quiet* — a verdict about
/// a cluster that was working, in the one direction D150 forbids anything here to guess.
///
/// **`--once` may not promise a retry either.** [`ONCE`] prints these one instant before
/// `stop.abort()`, so *It keeps asking* is false of every line on that run — not only of the
/// kind that ran out of time, which is why `stopping` and not `unfinished` picks that tail.
#[test]
fn the_line_about_a_kind_the_run_ran_out_on_states_the_two_numbers_and_never_a_cause() {
    let ran_out = |so_far, since| k8s::Trouble {
        kind: ObjectKind::Node,
        listed: false,
        failure: None,
        ended: false,
        unfinished: true,
        outstanding: Some(k8s::Listing {
            kind: ObjectKind::Node,
            so_far,
            since,
        }),
    };
    let one = |troubles: &[k8s::Trouble<'_>]| {
        let lines = unreadable(troubles, None, Some(&now()), true);
        let [line] = lines.as_slice() else {
            panic!("one trouble did not make one line: {lines:?}")
        };
        println!("{line}");
        line.clone()
    };

    // **The LIST that was still moving** — the shape the suite could not see.
    let moving = one(&[ran_out(1500, Some(four_minutes_ago()))]);
    assert!(
        moving.contains("1500 read so far, the last one 4 min ago"),
        "the two facts D150 hands a reader are missing, so a slow cluster and a dead one read \
         the same: {moving:?}"
    );
    // **No verdict, in either direction.** These are the words a cause would arrive in.
    for guess in [
        "gone quiet",
        "nothing is wrong with this login",
        "never answered",
        "accepted the request",
    ] {
        assert!(
            !moving.contains(guess),
            "a LIST that was still moving at the deadline was given a cause ({guess:?}), which \
             is the verdict NOTES § D150 exists to refuse: {moving:?}"
        );
    }
    assert!(
        moving.contains("this run ran out of time"),
        "the line does not say what actually happened: {moving:?}"
    );

    // **The wedge**: the same line, and the number is what differs.
    let wedged = one(&[ran_out(0, Some(four_minutes_ago()))]);
    assert!(
        wedged.contains("0 read so far") && !wedged.contains("the last one"),
        "`0 read so far` carried an age, and `k8s::Listing::since` is stamped by the `Init` that \
         opens the watch, so there is no *one* for it to be about: {wedged:?}"
    );
    assert_ne!(
        moving, wedged,
        "a LIST holding 1 500 objects and one holding none printed the same line, which is the \
         whole of what this box had to fix"
    );

    // **A real failure behind it keeps its own reason, ahead of the numbers**
    // (`k8s::Trouble::fault`): *check the address* is an action, and the counts are not.
    let dark = watcher::Error::WatchFailed(kube::Error::Service(Box::new(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "timed out",
    ))));
    let mut with_reason = ran_out(0, None);
    with_reason.failure = Some(&dark);
    let retried = one(&[with_reason]);
    assert!(
        retried.contains("nothing usable came back") && retried.contains("0 read so far"),
        "thirty seconds of a dead connection lost either its reason or its numbers: {retried:?}"
    );

    // **Jargon only inside backticks** (invariant 14), the rule the two older tails are held to.
    for line in [&moving, &wedged] {
        let english = prose(line);
        assert!(
            !english.contains("watch") && !english.contains("list"),
            "the sentence a reader has to understand uses an RBAC verb outside a quoted one: \
             {english:?}"
        );
    }
}

/// **A run that is ending may not promise it keeps asking** — the tail [`ONCE`] prints one
/// instant before `stop.abort()` (`k8s-admin`, 2026-09-03).
///
/// **It is `stopping` and not `unfinished` that picks it**, which is the finding: the wedge tail
/// only read correctly because `k8s::Store::stop_waiting` is unreachable outside `--once`, so
/// `unfinished` was doubling as a mode signal. A watch that **listed and then broke** is not
/// unfinished, gets the ordinary tail, and on a `--once` run that tail was a promise the process
/// was about to break.
///
/// **The same trouble, both modes, asserted against each other** — a `--live` run must keep the
/// retry sentence, because there a retry really is what happens next.
#[test]
fn a_run_that_is_about_to_exit_does_not_promise_it_keeps_asking() {
    let refused = watcher::Error::InitialListFailed(api_error(403, "Forbidden"));
    let broke = |listed| {
        vec![k8s::Trouble {
            kind: ObjectKind::Node,
            listed,
            failure: Some(&refused),
            ended: false,
            unfinished: false,
            outstanding: None,
        }]
    };
    for listed in [true, false] {
        let watching = unreadable(&broke(listed), None, Some(&now()), false);
        let stopping = unreadable(&broke(listed), None, Some(&now()), true);
        println!("--live  {}", watching[0]);
        println!("--once  {}", stopping[0]);
        assert!(
            watching[0].contains("It keeps asking"),
            "a screen somebody is watching stopped saying the tool is still trying: {watching:?}"
        );
        assert!(
            !stopping[0].contains("It keeps asking"),
            "a run one instant from exiting told the reader it keeps asking: {stopping:?}"
        );
        // **The reason survives the shorter tail**, which is what the line is for.
        assert!(
            stopping[0].contains("the role this kubeconfig uses needs to"),
            "dropping the promise dropped the refusal with it: {stopping:?}"
        );
    }
}

/// **Ten faults, ten sentences, and no two of them the same** — the second box's whole claim
/// (`PRIOR-ART § C1`), checked as a set rather than one at a time.
///
/// **A generic message may never stand in for an error we were handed.** The failure that rule
/// exists for is not a badly worded sentence; it is *one* sentence covering several errors, which
/// looks fine in every review and sends a reader to the wrong place at 3am. Two faults collapsing
/// into one string is what fails here, whichever two.
///
/// **Only five of the ten use `asked`, and that is deliberate.** The three kubeconfig faults and
/// a login helper that answered nothing all happened before anything was asked of any cluster, so
/// a sentence naming a verb and a resource there would be inventing one.
///
/// **All ten, and it was seven of nine until 2026-08-30** — `NoContext` and `BadEntry` had never
/// been in this list, so the one test whose whole claim is *no two collapse* could not have seen
/// those two collapse (`dev-core`'s own second pass). `k8s::Fault::Unfinished` is the tenth, and
/// it is the one this list matters most for: it is a hair from `Unanswered` — a server that
/// answered nothing against a connection that carried nothing — and the two send a reader to
/// opposite places.
#[test]
fn every_fault_gets_its_own_sentence_and_none_of_them_stands_in_for_another() {
    use k8s::Fault::{
        BadEntry, Expired, Gone, Kubeconfig, NoContext, NoCredential, Refused, Rejected,
        Unanswered, Unfinished,
    };
    let all = [
        Kubeconfig,
        NoContext,
        BadEntry,
        NoCredential,
        Rejected,
        Expired,
        Refused,
        Gone,
        Unfinished,
        Unanswered,
    ];

    // **Every framing a caller can hand `asked`, and not only the one that reads well**
    // (`tester`, 2026-08-27, NOTES § D29). The `Gone` arm was `there is nothing to {asked}`,
    // which wants a noun, and it was fed `` `get /apis` `` alone — the single framing where that
    // passes. Three of the four callers supply a verb phrase, and every one of them is here.
    let framings = [
        "`get /version`",
        "`get /apis`",
        "`list` and `watch` pods",
        "reach this cluster",
    ];
    // **The server's own sentence, in the two states it has**: one it wrote and one it did not.
    // Every claim below has to hold in both, because a caller supplies whichever it was handed.
    let wrote = "container \"app\" in pod \"broken-config\" is waiting to start: \
                 CreateContainerConfigError";
    for renewal in [None, Some("aws")] {
        for asked in framings {
            for server in [None, Some(wrote)] {
                let said: Vec<String> = all
                    .iter()
                    .map(|fault| because(*fault, asked, renewal, server))
                    .collect();
                for line in &said {
                    println!("{renewal:?}  {server:?}  {line}");
                }
                let distinct: std::collections::BTreeSet<&String> = said.iter().collect();
                assert_eq!(
                    distinct.len(),
                    all.len(),
                    "two faults print the same sentence, which is the generic handler growing \
                     back: {said:#?}"
                );
                for line in &said {
                    assert!(
                        !line.is_empty() && !line.contains("``"),
                        "a sentence is empty or carries an empty pair of backticks: {line:?}"
                    );
                }
                // The three arms that read `asked` must actually contain it, whichever framing
                // arrives. That is the cheap half; the grid below is the half that catches a
                // frame that reads wrongly.
                for fault in [Refused, Rejected, Gone, Unanswered, Unfinished] {
                    let line = because(fault, asked, renewal, server);
                    assert!(
                        line.contains(asked),
                        "`{fault:?}` dropped what k8rs was trying to do: {line:?}"
                    );
                }
            }
        }
    }

    // **Exactly one arm reads what the server said, and the other nine are byte-identical with
    // and without it.** That is the claim `because`'s own doc makes and the one that would rot
    // silently: a `403`'s message names a user and a verb where *the role this kubeconfig uses
    // needs to …* names the fix, and a `404`'s repeats a name the reader just typed.
    for fault in all {
        let quiet = because(fault, "`get /apis`", None, None);
        let told = because(fault, "`get /apis`", None, Some(wrote));
        match fault {
            Rejected => {
                assert!(
                    told.contains(wrote) && !told.contains("fault in k8rs"),
                    "the one fault whose diagnosis is the server's own sentence either dropped \
                     it or kept blaming k8rs over the top of it: {told:?}"
                );
                assert_ne!(quiet, told);
            }
            other => assert_eq!(
                quiet, told,
                "`{other:?}` started quoting the server, which puts a username or a name the \
                 reader just typed in place of the sentence written for it: {told:?}"
            ),
        }
    }

    // **The refusal names the verb and the resource** — the security gate's own words — and for
    // a `nonResourceURL` that means a path, because its `Status` carries no group and no kind
    // (NOTES § D160).
    assert_eq!(
        because(Refused, "`get /apis`", None, None),
        "the role this kubeconfig uses needs to `get /apis`"
    );
    // **And it never claims which verb is missing.** A watch is two verbs, and a `Role` granting
    // `list` without `watch` is ordinary — measured as printing *not allowed to `list` and
    // `watch` pods* while the LIST had just succeeded (`k8s-admin`, 2026-08-27).
    for asked in ["`get /apis`", "`list` and `watch` pods"] {
        let line = because(Refused, asked, None, None);
        assert!(
            !line.contains("not allowed"),
            "the refusal claims a state this code cannot know — which of two verbs was \
             refused: {line:?}"
        );
    }
    // **And the expiry is not a refusal.** Telling a beginner *you are not allowed* when their
    // login timed out sends them to their platform team for nothing (NOTES § D19).
    let expired = because(Expired, "`get /apis`", Some("aws"), None);
    assert!(
        !expired.contains("needs") && expired.contains("`aws`"),
        "{expired:?}"
    );
    // **And it promises nothing about restarting** (`tester`, 2026-08-27). kube re-runs the
    // `exec` plugin as its cached credential ages out — 25 executions against 22 requests over a
    // ten-second run — so the ordinary exec kubeconfig recovers on its own once the login is
    // repaired, and *restart k8rs* is a true problem answered with the wrong errand. The other
    // shape, a token with no `expirationTimestamp`, genuinely does need one; the sentence has to
    // be true of both, so it names neither.
    for renewal in [None, Some("aws")] {
        let line = because(Expired, "`get /apis`", renewal, None);
        assert!(
            !line.contains("afresh") && !line.contains("restart") && !line.contains("start k8rs"),
            "the expired-login sentence tells the reader to restart, which is false for the \
             exec kubeconfig it was written for: {line:?}"
        );
    }
    assert!(
        !because(Expired, "`get /apis`", None, None).contains('`'),
        "a kubeconfig with no login program to name printed backticks around nothing"
    );
    // **The program is named where there is one and the sentence still works where there is
    // not.** Both shapes are ordinary: a static token in the file has no program behind it.
    assert!(because(NoCredential, "", Some("aws"), None).contains("(`aws`)"));
    assert!(!because(NoCredential, "", None, None).contains('`'));
}

/// **The four sentences that carry `asked`, in all four framings a caller can supply, written
/// out** — sixteen literals, because nothing weaker can fail.
///
/// **A suffix check does not catch this, and that is measured** (2026-08-27). The broken `Gone`
/// frame was *there is nothing to {asked}* and the fixed one is *when k8rs tries to {asked}*;
/// both end in `` to {asked} ``, so an `ends_with` assertion over the grid stayed **green**
/// against the defect it was written for. What separates them is that the first wants a noun
/// where every caller but one supplies a verb phrase — *there is nothing to `list` and `watch`
/// pods* — and no predicate over a string can see that.
///
/// **So the sentences are literals and a reworded frame reddens this test**, which forces the one
/// thing that does work: somebody reads the four framings side by side. It is the shape
/// `tests/binary.rs` already uses for the whole report, for the same reason.
///
/// **The four framings are the four callers**, not an invented set: `` `get /version` `` and
/// `` `get /apis` `` from [`greeting`], `` `list` and `watch` <resource> `` from [`unreadable`],
/// and *reach this cluster* from [`live`].
#[test]
fn the_three_sentences_that_name_what_was_asked_read_in_all_four_framings() {
    use k8s::Fault::{Gone, Refused, Rejected, Unanswered, Unfinished};
    let grid = [
        (
            Refused,
            "`get /version`",
            "the role this kubeconfig uses needs to `get /version`",
        ),
        (
            Refused,
            "`get /apis`",
            "the role this kubeconfig uses needs to `get /apis`",
        ),
        (
            Refused,
            "`list` and `watch` pods",
            "the role this kubeconfig uses needs to `list` and `watch` pods",
        ),
        (
            Refused,
            "reach this cluster",
            "the role this kubeconfig uses needs to reach this cluster",
        ),
        (
            Gone,
            "`get /version`",
            "this server says there is no such thing when k8rs tries to `get /version`",
        ),
        (
            Gone,
            "`get /apis`",
            "this server says there is no such thing when k8rs tries to `get /apis`",
        ),
        (
            Gone,
            "`list` and `watch` pods",
            "this server says there is no such thing when k8rs tries to `list` and `watch` pods",
        ),
        (
            Gone,
            "reach this cluster",
            "this server says there is no such thing when k8rs tries to reach this cluster",
        ),
        (
            Rejected,
            "`get /version`",
            "this cluster would not accept the request k8rs made to `get /version` — that is a \
             fault in k8rs, and nothing is wrong with the cluster or with this login",
        ),
        (
            Rejected,
            "`get /apis`",
            "this cluster would not accept the request k8rs made to `get /apis` — that is a \
             fault in k8rs, and nothing is wrong with the cluster or with this login",
        ),
        (
            Rejected,
            "`list` and `watch` pods",
            "this cluster would not accept the request k8rs made to `list` and `watch` pods — \
             that is a fault in k8rs, and nothing is wrong with the cluster or with this login",
        ),
        (
            Rejected,
            "reach this cluster",
            "this cluster would not accept the request k8rs made to reach this cluster — that is \
             a fault in k8rs, and nothing is wrong with the cluster or with this login",
        ),
        (
            Unanswered,
            "`get /version`",
            "nothing usable came back when k8rs tried to `get /version`",
        ),
        (
            Unanswered,
            "`get /apis`",
            "nothing usable came back when k8rs tried to `get /apis`",
        ),
        (
            Unanswered,
            "`list` and `watch` pods",
            "nothing usable came back when k8rs tried to `list` and `watch` pods",
        ),
        (
            Unanswered,
            "reach this cluster",
            "nothing usable came back when k8rs tried to reach this cluster",
        ),
        // **The arm that carries no cause at all, and that is the assertion.** Nothing came
        // back and nothing said why, so any explanation is a guess — NOTES § D148's missing
        // keepalive hides a dead socket behind a quiet server, and NOTES § D150 refuses to call a
        // LIST that is still moving *hung*. An earlier draft said *nothing is wrong with this
        // login: it is the cluster, or the network in between, that has gone quiet* and was both.
        (
            Unfinished,
            "`get /version`",
            "the request k8rs made to `get /version` had not been answered",
        ),
        (
            Unfinished,
            "`get /apis`",
            "the request k8rs made to `get /apis` had not been answered",
        ),
        (
            Unfinished,
            "`list` and `watch` pods",
            "the request k8rs made to `list` and `watch` pods had not been answered",
        ),
        (
            Unfinished,
            "reach this cluster",
            "the request k8rs made to reach this cluster had not been answered",
        ),
    ];
    for (fault, asked, expected) in grid {
        let line = because(fault, asked, None, None);
        println!("{line}");
        assert_eq!(
            line, expected,
            "`{fault:?}` has been reworded — read all four framings of it above before updating \
             this literal, because three of the four callers supply a verb phrase and one \
             supplies a path"
        );
    }

    // **The one arm that reads what the server said, written out in all four framings too**
    // (`k8s::said`). It is the sentence a live cluster produced, and it is here as a literal for
    // the reason the twenty above are: the difference between quoting the server and blaming
    // k8rs is a difference no predicate over a string can see.
    let wrote = "container \"app\" in pod \"broken-config\" is waiting to start: \
                 CreateContainerConfigError";
    let quoted = [
        (
            "`get /version`",
            "this cluster would not accept the request k8rs made to `get /version`, and said: \
             container \"app\" in pod \"broken-config\" is waiting to start: \
             CreateContainerConfigError",
        ),
        (
            "`get /apis`",
            "this cluster would not accept the request k8rs made to `get /apis`, and said: \
             container \"app\" in pod \"broken-config\" is waiting to start: \
             CreateContainerConfigError",
        ),
        (
            "`list` and `watch` pods",
            "this cluster would not accept the request k8rs made to `list` and `watch` pods, and \
             said: container \"app\" in pod \"broken-config\" is waiting to start: \
             CreateContainerConfigError",
        ),
        (
            "reach this cluster",
            "this cluster would not accept the request k8rs made to reach this cluster, and \
             said: container \"app\" in pod \"broken-config\" is waiting to start: \
             CreateContainerConfigError",
        ),
    ];
    for (asked, expected) in quoted {
        let line = because(Rejected, asked, None, Some(wrote));
        println!("{line}");
        assert_eq!(
            line, expected,
            "the rejected call's sentence has been reworded — it is the only one that quotes the \
             cluster, and what it may not do again is claim the fault is k8rs's over the top of \
             an explanation the server gave"
        );
    }
}

/// A session assembled by hand, because the two failures that matter here need a server that
/// answers `403` and there is none in this repo's tests.
///
/// Every field is `pub(crate)` and this is the crate, which is the same seam `k8s_tests.rs`
/// § CONNECTING uses from the other side.
fn saying(
    version: Result<String, kube::Error>,
    served: Result<k8s::Served, kube::Error>,
    renewal: Option<&str>,
) -> k8s::Session {
    k8s::Session {
        client: offline(),
        version,
        served,
        watches: Vec::new(),
        renewal: renewal.map(str::to_string),
        // The startup line is about what the *cluster* answered; neither of these is a question
        // the cluster was asked, and `k8s_tests.rs` § CONNECTING is where they are proven.
        context: None,
        namespace: None,
        // The startup line this function feeds prints the scope's *why* beside the greeting,
        // and `Cluster` is the arm that has nothing to say ([`scoped_because`]); the arm that
        // does is asserted in its own test.
        coverage: k8s::Coverage::Cluster,
        client_certificate: None,
        // The clock line is stdout's, beside the findings, and this function builds the session
        // the *startup* line is read off — `screens/once.md` § When your clock and the cluster's
        // disagree keeps the two streams apart. The certificate sentence beside it is stdout's for
        // the same reason.
        skew: None,
        serving_expiry: k8s::Serving::Unread,
    }
}

/// **The one sentence that says *why* a run is scoped**, and the two arms that say nothing
/// (NOTES § D5, `PRIOR-ART § B4`, the security gate's Authorization row).
///
/// **The refusal is the only arm with anything to say.** `--namespace payments` is a choice the
/// reader made a second ago and the header already prints `ns: payments`; explaining it back to
/// them is noise. The fallback is the opposite — nobody asked for it, the reader may not know
/// their role is namespaced, and the string they need is the one to hand to whoever owns the
/// cluster.
///
/// **What the security gate asks for is in the assertion**: the missing verb and the resource,
/// named, and a way out. The frame is [`because`]'s, so this sentence and the one a refused watch
/// gets cannot come apart.
#[tokio::test]
async fn a_run_that_was_scoped_by_a_refusal_says_so_and_one_that_was_asked_does_not() {
    let scoped = |coverage: k8s::Coverage| k8s::Session {
        coverage,
        ..saying(
            Ok("v1.36.1".to_string()),
            Err(api_error(403, "Forbidden")),
            None,
        )
    };

    assert_eq!(
        scoped_because(&scoped(k8s::Coverage::Cluster), false),
        None,
        "a run that reads the whole cluster explained a scope it does not have"
    );
    assert_eq!(
        scoped_because(&scoped(k8s::Coverage::Asked("payments".to_string())), false),
        None,
        "a reader who typed --namespace was told what --namespace does"
    );

    let said = scoped_because(
        &scoped(k8s::Coverage::Refused("payments".to_string())),
        false,
    )
    .expect("a run nobody asked to narrow narrowed in silence");
    println!("{said}");
    assert_eq!(
        said,
        "the role this kubeconfig uses needs to `list` pods across the whole cluster — so k8rs \
         is watching one namespace instead: payments. Pass --namespace <name> for a different \
         one, or ask for cluster-wide read access"
    );

    // **The guess that was refused too says so, rather than presenting itself as a scope**
    // (`k8s::Coverage::Blind`, `reports/2026-08-29-namespace-scope-under-a-real-role.md` § R1).
    // The old sentence claimed k8rs *is watching* `default` over a namespace it had just been
    // refused in, and the report under it printed a header and a health claim.
    let blind = scoped_because(&scoped(k8s::Coverage::Blind("default".to_string())), false)
        .expect("a run that could read nothing at all said nothing about it");
    println!("{blind}");
    assert_eq!(
        blind,
        "the role this kubeconfig uses needs to `list` pods across the whole cluster — and this \
         kubeconfig names no namespace, so k8rs tried default and was refused there too. Pass \
         --namespace <name> to say which namespace you work in"
    );
    assert!(
        !blind.contains("is watching one namespace instead"),
        "a scope that read nothing was presented as one that worked: {blind}"
    );

    // **And under `--once` that one arm goes quiet, because [`pods_unread`] is about to say it
    // with the scope and the action in it** (`k8s-admin`, 2026-08-30). Measured, the reader got
    // one fact in two sentences with two different verb sets: `list` pods across the whole
    // cluster here, `list` and `watch` pods there. **Only that arm** — `Refused` is the run that
    // works, so this is the only line explaining the header, and losing it loses the sentence.
    assert_eq!(
        scoped_because(&scoped(k8s::Coverage::Blind("default".to_string())), true),
        None,
        "a --once run that ends on the refusal said it here first, in different words"
    );
    assert_eq!(
        scoped_because(
            &scoped(k8s::Coverage::Refused("payments".to_string())),
            true
        ),
        Some(said.clone()),
        "a --once run that reports fine lost the only line saying why its header names one \
         namespace"
    );

    // Invariant 9: the namespace came off argv or a kubeconfig, and neither is ours. Both arms
    // that print one, because a strip on one of two interpolations is a strip on neither.
    for crafted in [
        scoped_because(
            &scoped(k8s::Coverage::Refused("pay\u{1b}[2Jments".to_string())),
            false,
        ),
        scoped_because(
            &scoped(k8s::Coverage::Blind("pay\u{1b}[2Jments".to_string())),
            false,
        ),
    ] {
        let crafted = crafted.expect("both narrowed arms always say something");
        assert!(
            !crafted.contains('\u{1b}'),
            "an escape sequence in a namespace reached the terminal: {crafted:?}"
        );
        // The readable part survives: a strip that returned nothing would pass the line above
        // and leave a sentence naming no namespace at all (CLAUDE.md § a derived list asserts it
        // found something). `[2J` is printable — only the `ESC` goes.
        assert!(
            crafted.contains("pay[2Jments"),
            "the strip took the namespace with it: {crafted:?}"
        );
    }
}

/// **The startup line names what it could not read and why**, per question, and the session
/// starts anyway.
///
/// **Both of these are `Result`s that travel** (`k8s.rs` § CONNECTING): a kubeconfig that may not
/// `get /apis` still watches pods, so the refusal is a clause and never an exit. Until 2026-08-27
/// both clauses were fixed strings — *"the server would not say which version it is"* — which is
/// true of a refusal, an expiry and a dead socket alike and useful for none of them.
///
/// **`get /version` and `get /apis` are `nonResourceURL`s.** NOTES § D160 measured the `Status`
/// for one: an empty `details`, so there is no group and no kind a sentence could name, and the
/// path is the only true subject. That is also the grant a `ClusterRole` has to spell, which is
/// the one our own documented read-only role was missing.
///
/// **It runs on a tokio runtime for the client alone**: [`saying`] builds one, and a
/// `kube::Client` is a `tower::buffer::Buffer` whose clone needs a spawned worker. Nothing under
/// test here is asynchronous.
#[tokio::test]
async fn the_startup_line_says_which_question_failed_and_why() {
    let refused = || api_error(403, "Forbidden");
    let expired = || api_error(401, "Unauthorized");

    let both = greeting(&saying(Err(refused()), Err(refused()), None));
    for clause in &both {
        println!("{clause}");
    }
    let both = both.join(" · ");
    assert!(
        both.contains(
            "could not read the server version (the role this kubeconfig uses needs to \
                       `get /version`)"
        ),
        "{both:?}"
    );
    assert!(
        both.contains("the role this kubeconfig uses needs to `get /apis`"),
        "the discovery refusal does not name the path, which is the only thing its `Status` \
         gives it (NOTES § D160): {both:?}"
    );
    assert!(
        both.contains("cannot show you what is in it or tell which add-ons it has"),
        "the reader is not told what a refused `/apis` costs them, in words that need no \
         glossary (invariant 14): {both:?}"
    );

    // **An expired login is a different startup line**, and it names the program to sign in to.
    let stale = greeting(&saying(Err(expired()), Err(expired()), Some("aws"))).join(" · ");
    println!("{stale}");
    assert!(
        stale.contains("no longer accepts this login") && stale.contains("`aws`"),
        "{stale:?}"
    );
    assert_ne!(
        stale, both,
        "an expired login and a refusal print the same startup line"
    );

    // **A cluster that answers nothing is a third**, and the shape a test can reach for real.
    let dead = greeting(&saying(
        Err(kube::Error::Service(Box::new(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "timed out",
        )))),
        Err(api_error(500, "InternalError")),
        None,
    ))
    .join(" · ");
    println!("{dead}");
    assert!(dead.contains("nothing usable came back"), "{dead:?}");
    assert!(
        !dead.contains("the role") && !dead.contains("login"),
        "a cluster that is down is reported as a permission or credential problem: {dead:?}"
    );

    // And the healthy line is still the healthy line: the failure clauses are additions, not a
    // rewrite of what a working connection says.
    let well = greeting(&saying(
        Ok("v1.34.0".to_string()),
        Ok(k8s::Served {
            kinds: Vec::new(),
            capabilities: None,
        }),
        None,
    ));
    println!("{}", well.join(" · "));
    assert_eq!(
        well,
        vec![
            "server v1.34.0".to_string(),
            "0 kinds".to_string(),
            "discovery named nothing at all".to_string(),
        ]
    );
}

/// **`server ` with nothing after it is never printed** — an absent, blank or all-stripped
/// `gitVersion` costs the clause instead.
///
/// **Four shapes arrive blank and a test that feeds one proves nothing about the other three**
/// (NOTES § D29). A `/version` that answers `200` with no `gitVersion` at all deserialises to the
/// empty string; a gateway can answer with `""` written out; a value that is only spaces is
/// blank without being empty, and [`sanitize`] leaves it exactly as it was, because a space is
/// printable; and a value made entirely of characters invariant 9 strips is empty only *after*
/// [`sanitize`]. The last two are what make the guard `trim` on the *stripped* value rather than
/// `is_empty` on the raw one, and the spaces shape is the one that turned this test red when the
/// first draft only checked `is_empty`. A real kube-apiserver always sets the field; a proxy or
/// gateway in front of one is where all four come from.
///
/// **Silence and not a failure sentence.** The call was answered, so *could not read the server
/// version* would be a false claim about a request that succeeded — and the clause has nothing
/// true left to say.
#[tokio::test]
async fn a_server_version_that_is_blank_costs_the_clause_and_not_the_line() {
    let served = || {
        Ok(k8s::Served {
            kinds: Vec::new(),
            capabilities: None,
        })
    };
    let line =
        |version: &str| greeting(&saying(Ok(version.to_string()), served(), None)).join(" · ");

    // The positive: a server that answered says which one it is, and nothing changed for it.
    let named = line("v1.34.0");
    println!("k8rs: watching — {named}");
    assert!(
        named.starts_with("server v1.34.0 · "),
        "the healthy greeting stopped naming the server: {named}"
    );

    // **What is decided on is what prints** (`k8s-admin`, 2026-09-03). The guard tests the
    // trimmed value, so the clause has to carry the trimmed value too — the first draft kept the
    // untrimmed one on the grounds that trimming invents text the cluster did not send, and that
    // does not hold: `k8s::session` has already run `text(&mut version, IDENTIFIER)` over it. All
    // the split bought was `server  v1.36.1  ·`.
    let padded = line("  v1.36.1  ");
    println!("k8rs: watching — {padded}");
    assert!(
        padded.starts_with("server v1.36.1 · "),
        "the clause was decided on the trimmed string and printed the untrimmed one: {padded:?}"
    );

    for (shape, version) in [
        // Absent and written as `""` are one shape here, not two: `serde` gives the empty
        // string for both, and this is the value `k8s::session` hands on.
        ("no `gitVersion`, or one written as `\"\"`", ""),
        ("a `gitVersion` that is only spaces", "   "),
        (
            "a `gitVersion` invariant 9 strips to nothing",
            "\u{200e}\u{7}\u{feff}",
        ),
    ] {
        let said = line(version);
        println!("{shape}: k8rs: watching — {said}");
        assert!(
            !said.contains("server "),
            "{shape} printed `server` with nothing after it: {said:?}"
        );
        assert!(
            !said.contains("could not read the server version"),
            "{shape} was reported as a failed call, and the call was answered: {said:?}"
        );
        assert_eq!(
            said, "0 kinds · discovery named nothing at all",
            "{shape} cost more than its own clause, or left an empty one behind: {said:?}"
        );
    }

    // **The line never comes back empty**, whatever the version did: discovery answers on the
    // same session and always says something.
    assert!(
        !line("").is_empty(),
        "a blank version emptied the whole startup line"
    );
}

/// **A connection that never happened says which of the two ways it failed**, and the one that
/// can name the login program does — the sentence `main` turns into exit 2.
///
/// **The `Client` arm had no test at all through `live` and that is why F1 shipped**
/// (`tester`, 2026-08-27). `k8s_tests.rs` proved `problem.fault() == NoCredential` and this file
/// only ever called `live` with a kubeconfig error and an `Ok(session)`, so the arm whose
/// sentence actually changed was reached by nothing — and `--in-diff` cannot flag a line no test
/// runs. `connect_with` computed the login program and dropped it on a `?`, `live` passed `None`,
/// and the `{named}` slot in [`because`]'s `NoCredential` arm could never be filled.
///
/// **The kubeconfig arm's assertion is not the old one either.** Its sentence is byte-identical
/// to what `live` returned before this change, so on its own it was green against the pre-change
/// code — a test that cannot fail (NOTES § D26). What pins the change is the pair: two arms, two
/// different sentences, and the second naming a program the first has no way to know.
#[tokio::test]
async fn a_connection_that_never_happened_says_which_way_it_failed() {
    let yaml = |user: &str| {
        kube::config::Kubeconfig::from_yaml(&format!(
            "apiVersion: v1\n\
             kind: Config\n\
             current-context: demo\n\
             clusters: [{{name: demo, cluster: {{server: 'https://k8rs-tests.invalid:6443'}}}}]\n\
             contexts: [{{name: demo, context: {{cluster: demo, user: demo}}}}]\n\
             users: [{{name: demo, user: {user}}}]\n"
        ))
        .expect("a kubeconfig this file wrote itself")
    };

    // **The context**: the file read perfectly and does not name what was asked for.
    let unloadable = watching(
        k8s::connect_with(yaml("{}"), Some("k8rs-tests-no-such-context"), None).await,
        false,
    );
    let unloadable = unloadable.await;
    println!("{unloadable}");
    assert_eq!(
        unloadable,
        "k8rs: no cluster to watch — this kubeconfig has no such context — check the \
         `--context` you gave, or the `current-context` line in the file"
    );

    // **An entry**: file fine, context fine, and the certificate it names is not on the disk.
    // Measured against a live server as printing *the kubeconfig could not be read* — a sentence
    // that sends the reader to `cat` a file with nothing wrong with it (`k8s-admin`,
    // 2026-08-27).
    let moved = "{client-certificate: /nonexistent/k8rs-tests/client.crt, \
                 client-key: /nonexistent/k8rs-tests/client.key}";
    let entry = watching(k8s::connect_with(yaml(moved), None, None).await, false).await;
    println!("{entry}");
    assert!(
        entry.contains("this kubeconfig loaded, and something it points at did not"),
        "a certificate path that moved still reads as an unreadable kubeconfig: {entry:?}"
    );
    assert_ne!(
        entry, unloadable,
        "a broken entry and a missing context print the same sentence, and they are two \
         different things to go and fix"
    );

    // **The login program**: the file loaded, and the program it names is not on the disk. The
    // whole point of the arm, and the whole point of naming it.
    let helper = "/nonexistent/k8rs-tests-no-such-credential-plugin";
    let user = format!(
        "{{exec: {{apiVersion: client.authentication.k8s.io/v1beta1, command: {helper}}}}}"
    );
    let broken = watching(k8s::connect_with(yaml(&user), None, None).await, false).await;
    println!("{broken}");
    assert!(
        broken.contains(&format!("(`{helper}`)")),
        "the sentence does not name the login program, so the one fault whose fix is on the \
         reader's own machine says nothing about what to fix: {broken:?}"
    );
    assert!(
        broken.starts_with("k8rs: no cluster to watch — the program this kubeconfig logs in with"),
        "{broken:?}"
    );
    assert_ne!(
        broken, unloadable,
        "a broken login program and a kubeconfig that would not load print the same sentence, \
         and they are fixed in two entirely different places"
    );
}

/// **A cluster that never answers still starts the driver, and the driver says why it stopped.**
///
/// Every initial LIST failed here, so nothing was ever printed — but *what is asserted* is the
/// ending, because stdout belongs to the process and a test cannot read it back. That the report
/// is withheld while a bootstrap is unfinished is
/// [`a_bootstrap_that_has_not_finished_prints_nothing_at_all`]'s, one layer down, where it is a
/// value and not a stream.
///
/// **What this one pins is that `live` comes back with a sentence** rather than falling off the
/// end quietly — `main` turns that into exit 2, and a driver that returned silently would look
/// exactly like a clean shutdown of a tool that is supposed to keep watching.
///
/// **The streams are cut after two items each** because a real one never ends: kube's `watcher()`
/// cannot finish (`k8s.rs` § THE DRIVER) and the backoff under it never gives up, so a live
/// `drive` in a test would hang for as long as the test harness let it.
#[tokio::test]
async fn a_cluster_that_never_answers_prints_nothing_and_says_why_it_stopped() {
    use futures_util::stream::StreamExt;
    let mut session = k8s::session(offline(), k8s::Coverage::Cluster).await;
    session.watches = session
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();

    let stopped = watching(Ok(session), false).await;

    assert!(
        stopped.contains("every watch has stopped"),
        "a driver whose watches all ended returned {stopped:?} instead of saying so"
    );
}

// --- THE COMMAND LOG ---
//
// **The teaching device outside the TUI** (invariant 4, `screens/once.md` § stdout and stderr are
// split on purpose). Until this box the screen drew two `$ kubectl …` lines for the live path and
// no code emitted either (NOTES § D189), so a reader was promised the commands and given none.
//
// **The expected blocks below are written out whole, in the order the screen fixes**, rather than
// asserted line by line: what this log has to be right about is *which reads, and in which group*,
// and a per-line `contains` passes for a log that prints the watches above discovery. They are the
// fenced blocks of `screens/once.md` § stdout and stderr are split on purpose and § Under
// `--analysis`, transcribed — a literal, for the reason [`EXPIRING`] is one.
//
// **Inside the last two groups there is no wire order to assert, and these tests do not claim
// one.** The seven report reads share one `tokio::join!` and the five watch LISTs go out together
// the moment the loop starts polling; measured against a logging stub, neither arrives in its
// declaration order and neither is stable ([`command_log`]'s own doc has the readings). What is
// fixed, and what is asserted here, is the order the lines are *printed* in.
//
// **What no test in this file can see is that they reach stderr**, because a test cannot read its
// own process's stream back (this file's own module doc). `command_log` is a function precisely so
// the content is assertable here; that `live` writes it is proven by running the binary.

/// Every read a bare `k8rs --once` or `k8rs --live` performs, in the order it starts them.
const BARE_LOG: &str = "\
$ kubectl get --raw '/api/v1/pods?limit=1'
$ kubectl get --raw /version
$ kubectl api-resources --verbs=list
$ kubectl get pods -A --watch
$ kubectl get nodes --watch
$ kubectl get deployments -A --watch
$ kubectl get statefulsets -A --watch
$ kubectl get daemonsets -A --watch";

/// The same run under `--namespace payments`: four of the five watches follow the scope, `nodes`
/// does not because there is no namespaced node list to ask for, and **the scope probe is absent
/// altogether** — typing the flag answers the question it exists to ask, so no request is sent.
const SCOPED_LOG: &str = "\
$ kubectl get --raw /version
$ kubectl api-resources --verbs=list
$ kubectl get pods -n payments --watch
$ kubectl get nodes --watch
$ kubectl get deployments -n payments --watch
$ kubectl get statefulsets -n payments --watch
$ kubectl get daemonsets -n payments --watch";

/// The same run under `--analysis`: seven more reads, printed after discovery and before the five
/// watches, in the order `k8s.rs`'s own `tokio::join!` *lists* them — which is where they start,
/// not the order they come back in ([`command_log`]).
const ANALYSIS_LOG: &str = "\
$ kubectl get --raw '/api/v1/pods?limit=1'
$ kubectl get --raw /version
$ kubectl api-resources --verbs=list
$ kubectl get certificatesigningrequests
$ kubectl get replicasets -A
$ kubectl get services -A
$ kubectl get endpointslices -A
$ kubectl get persistentvolumeclaims -A
$ kubectl get poddisruptionbudgets -A
$ kubectl top nodes
$ kubectl get pods -A --watch
$ kubectl get nodes --watch
$ kubectl get deployments -A --watch
$ kubectl get statefulsets -A --watch
$ kubectl get daemonsets -A --watch";

/// **A bare run prints one line per read, in the order the code starts them** — and the
/// `--analysis` run prints the seven a report fetches between discovery and the watches.
#[test]
fn the_command_log_is_every_read_this_run_performs_in_the_order_it_starts_them() {
    let log = |analysis, coverage| command_log(analysis, &coverage, None).join("\n");

    let bare = log(false, k8s::Coverage::Cluster);
    println!("{bare}");
    assert_eq!(
        bare, BARE_LOG,
        "the command log is not the block `screens/once.md` § stdout and stderr are split on \
         purpose draws"
    );

    let with_reports = log(true, k8s::Coverage::Cluster);
    println!("{with_reports}");
    assert_eq!(
        with_reports, ANALYSIS_LOG,
        "the command log is not the block `screens/once.md` § Under `--analysis` draws"
    );

    // **The seven go *between* discovery and the watches, and that is the ordering claim** — a
    // log that printed them after the five would satisfy every `contains` in this file.
    let lines: Vec<&str> = with_reports.lines().collect();
    let at = |needle: &str| {
        lines
            .iter()
            .position(|line| line.contains(needle))
            .unwrap_or_else(|| panic!("`{needle}` is not in the command log: {with_reports}"))
    };
    assert!(
        at("api-resources") < at("certificatesigningrequests")
            && at("top nodes") < at("pods -A --watch"),
        "the seven report reads are not between discovery and the watches: {with_reports}"
    );

    // **Discovery carries `--verbs=list`, because that is the filter the greeting counts
    // through** (`k8s::browsable`'s `supports_operation(verbs::LIST)`). Without it the reader is
    // told `62 kinds` two lines above a command that prints 69 and concludes the tool is off by
    // seven (`k8s-admin`, 2026-09-03).
    assert!(
        bare.contains("$ kubectl api-resources --verbs=list"),
        "discovery's line does not select the kinds the greeting counted: {bare}"
    );
}

/// **`--namespace` narrows four of the five watches and five of the seven report reads, and
/// nothing else** — the same split the reads themselves make, because `nodes` and
/// `certificatesigningrequests` are cluster-scoped and `kubectl top nodes` is about machines.
#[test]
fn a_scoped_run_narrows_exactly_the_reads_that_are_narrowed() {
    let scoped = command_log(false, &k8s::Coverage::Asked("payments".to_string()), None).join("\n");
    println!("{scoped}");
    assert_eq!(
        scoped, SCOPED_LOG,
        "the scoped command log is not the block `screens/once.md` draws"
    );
    assert!(
        scoped.contains("$ kubectl get nodes --watch") && !scoped.contains("nodes -n"),
        "a namespace flag was put on the node watch, and there is no namespaced node list to \
         ask for: {scoped}"
    );

    let reports = command_log(true, &k8s::Coverage::Asked("payments".to_string()), None);
    for line in &reports {
        println!("{line}");
    }
    assert_eq!(
        reports
            .iter()
            .filter(|line| line.contains("-n payments"))
            .count(),
        9,
        "the scope did not land on exactly the four watches and five report lists that follow \
         it: {reports:?}"
    );
    for bare in [
        "$ kubectl get certificatesigningrequests",
        "$ kubectl top nodes",
        "$ kubectl get nodes --watch",
    ] {
        assert!(
            reports.iter().any(|line| line == bare),
            "`{bare}` is cluster-scoped and lost its bare spelling on a scoped run: {reports:?}"
        );
    }

    // **The other three arms of `Coverage` narrow the same way**, because the reads do: a scope
    // k8rs fell back to is still the scope every watch below it points at. What they do *not*
    // share with `Asked` is the probe line above them — the cluster-wide `LIST` was really sent
    // and really refused, which is how they became these arms at all.
    for coverage in [
        k8s::Coverage::Refused("payments".to_string()),
        k8s::Coverage::Blind("payments".to_string()),
    ] {
        assert_eq!(
            command_log(false, &coverage, Some("payments")).join("\n"),
            format!("$ kubectl get --raw '/api/v1/pods?limit=1'\n{SCOPED_LOG}"),
            "a scope k8rs fell back to printed a cluster-wide command log while the watches were \
             narrowed, or lost the refused probe that put it in this arm"
        );
    }
}

/// **Which probe lines print, over every shape `k8s::coverage` can leave behind** — the branch
/// `screens/once.md` fixed on 2026-09-03 and the one nothing fed before it (NOTES § D29).
///
/// **The pair is the input, not [`k8s::Coverage`] alone, and row 6 is why.** `coverage` decides
/// the second probe on *the context's own namespace, filtered through `namespace_name`* — so a
/// context that itself names `default` produces `Refused("default")` with **one** request sent,
/// byte-identical in the enum to the fallback case that sent **two**. A log written off the enum
/// alone would print a request k8rs never made, which is invariant 4's *neither record may lie*.
///
/// **Row 7 is the filter half of that.** `context_scope` drops a context namespace that is not a
/// namespace name, so k8rs falls back and probes — and a log that read the raw field would go
/// quiet exactly where a request went out.
#[test]
fn the_scope_probe_prints_once_twice_or_not_at_all_and_the_context_decides_which() {
    let probes = |coverage: k8s::Coverage, context: Option<&str>| -> Vec<String> {
        command_log(false, &coverage, context)
            .into_iter()
            .take_while(|line| line.contains("/pods?limit=1"))
            .collect()
    };
    const WIDE: &str = "$ kubectl get --raw '/api/v1/pods?limit=1'";
    const FALLBACK: &str = "$ kubectl get --raw '/api/v1/namespaces/default/pods?limit=1'";

    for (row, coverage, context, expected) in [
        (
            "`--namespace` typed: the question is answered before it is asked",
            k8s::Coverage::Asked("payments".to_string()),
            None,
            vec![],
        ),
        (
            "`--namespace` typed beside a context that names one: still nothing sent",
            k8s::Coverage::Asked("payments".to_string()),
            Some("shop"),
            vec![],
        ),
        (
            "answered cluster-wide: one request, no fallback needed",
            k8s::Coverage::Cluster,
            None,
            vec![WIDE],
        ),
        (
            "refused, and the context names where to look instead: no second request",
            k8s::Coverage::Refused("payments".to_string()),
            Some("payments"),
            vec![WIDE],
        ),
        (
            "refused, nothing in the file to fall back to: the guess is probed too",
            k8s::Coverage::Refused("default".to_string()),
            None,
            vec![WIDE, FALLBACK],
        ),
        (
            "refused there as well: the same two requests, and neither answered",
            k8s::Coverage::Blind("default".to_string()),
            None,
            vec![WIDE, FALLBACK],
        ),
        (
            "the trap: a context that itself names `default`, so only one request went out",
            k8s::Coverage::Refused("default".to_string()),
            Some("default"),
            vec![WIDE],
        ),
        (
            "a context namespace that is not a namespace name is no fallback at all",
            k8s::Coverage::Refused("default".to_string()),
            Some("Not A Namespace"),
            vec![WIDE, FALLBACK],
        ),
    ] {
        let drawn = probes(coverage, context);
        println!("{row}\n    {drawn:?}");
        assert_eq!(
            drawn, expected,
            "{row}: the command log claims a set of requests `k8s::coverage` did not send"
        );
    }
}

/// **It is display text and it is stripped** — a namespace is argv or a kubeconfig, and this is a
/// place it reaches a terminal (invariant 9, the security gate).
#[test]
fn the_command_log_is_stripped_display_text_and_nothing_executes_it() {
    let crafted = k8s::Coverage::Refused("pay\u{1b}[2Jments".to_string());
    let log = command_log(true, &crafted, Some("payments"));
    for line in &log {
        println!("{line}");
        assert!(
            line.starts_with("$ kubectl "),
            "a line in the command log is not a kubectl command: {line}"
        );
        assert!(
            !line.chars().any(k8s::unprintable),
            "an escape sequence out of a kubeconfig reached the command log: {line:?}"
        );
    }
    assert!(
        log.iter()
            .any(|line| line.contains("-n pay[2Jments --watch")),
        "the namespace was dropped rather than stripped: {log:?}"
    );
}

/// **No line teaches a request storm** — the blocker `k8s-admin` found and the PM measured
/// (2026-09-03).
///
/// **`--chunk-size` is a page size, not a `limit`.** `kubectl get pods -A --chunk-size=1` pages to
/// completion: 41 pods cost 41 sequential requests and 6.3 s where `get --raw
/// '/api/v1/pods?limit=1'` costs one. Printing it on line 1 of every unscoped run would teach the
/// reader that k8rs listed every pod one at a time to find out whether it may look at pods —
/// `PRIOR-ART § A2`'s pathological case, from the tool whose invariant 6 is *watch, never
/// poll-list*.
///
/// **And it is the whole log, not just the probe.** `k8s::whole_list` sends no `limit` while
/// `kubectl get` defaults to `--chunk-size=500`, so exactness of that kind would owe the report
/// lines a `--chunk-size=0` each; they are bare instead, and consistency here is the flag being
/// absent everywhere.
#[test]
fn no_line_in_the_command_log_pages_a_whole_cluster_one_object_at_a_time() {
    for coverage in [
        k8s::Coverage::Cluster,
        k8s::Coverage::Asked("payments".to_string()),
        k8s::Coverage::Refused("default".to_string()),
        k8s::Coverage::Blind("default".to_string()),
    ] {
        for analysis in [false, true] {
            for line in command_log(analysis, &coverage, None) {
                assert!(
                    !line.contains("--chunk-size"),
                    "a page-size flag is back in the command log, and on the probe it turns one \
                     request into one per object: {line}"
                );
            }
        }
    }
    // The probe's own spelling, positively: exact, single-quoted so the `?` survives a shell, and
    // the same `get --raw` shape `/version` two lines under it already uses.
    let probe = &command_log(false, &k8s::Coverage::Cluster, None)[0];
    println!("{probe}");
    assert_eq!(
        probe, "$ kubectl get --raw '/api/v1/pods?limit=1'",
        "the probe line is not the one request `k8s::lists_pods` actually sends"
    );
}

/// **The lines reach the stream, one per line, in order** — the half of this log no assertion
/// about `command_log` can reach.
///
/// **It exists because the mutation gate found the hole**: `replace log_to with ()` survived, and
/// it survived honestly — every other test here reads the `Vec<String>` and none of them could
/// tell a writer that writes from one that does nothing (2026-09-03). `log_to` takes a
/// `&mut impl Write` precisely so a test can hand it something it can read back; until this test
/// nothing did, and `live` could have been printing nothing at all.
///
/// **A failed write costs nothing and must not stop the run.** stderr closed under the tool —
/// `k8rs --once --analysis 2>/dev/null` on a shell that then exits — is not a reason to abandon a
/// report, so the writes are `let _ =` and this asserts that a refusing writer is survived rather
/// than propagated.
#[test]
fn the_command_log_reaches_the_stream_one_line_at_a_time() {
    let mut written = Vec::new();
    log_to(
        &mut written,
        command_log(true, &k8s::Coverage::Cluster, None),
    );
    let written = String::from_utf8(written).expect("the command log is text");
    print!("{written}");
    assert_eq!(
        written,
        format!("{ANALYSIS_LOG}\n"),
        "the lines that reached the stream are not the lines the log is made of, or lost their \
         newlines"
    );
    assert_eq!(
        written.lines().count(),
        command_log(true, &k8s::Coverage::Cluster, None).len(),
        "a line was dropped or doubled on the way out"
    );

    // **A writer that refuses every write** — the stream is closed, and the run carries on.
    struct Closed;
    impl std::io::Write for Closed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        }
    }
    log_to(
        &mut Closed,
        command_log(false, &k8s::Coverage::Cluster, None),
    );
}

/// **The wall and the ordinary run spell the connect reads the same way, because it is one
/// function** — [`connect_log`], which `live` prints alone on the run `certificate_is_why` ends.
///
/// **A wall prints what it attempted and no more.** On that path the report lists and the five
/// watches never start, so a log naming them would be a list of reads that never happened — the
/// lie this box exists to remove. What it may claim is exactly the prefix below.
#[test]
fn the_wall_prints_the_reads_that_happened_and_the_run_prints_them_the_same_way() {
    for (coverage, context) in [
        (k8s::Coverage::Cluster, None),
        (k8s::Coverage::Asked("payments".to_string()), None),
        (k8s::Coverage::Refused("default".to_string()), None),
        (k8s::Coverage::Blind("default".to_string()), Some("shop")),
    ] {
        let attempted = connect_log(&coverage, context);
        println!("{attempted:?}");
        for analysis in [false, true] {
            assert!(
                command_log(analysis, &coverage, context).starts_with(&attempted),
                "the wall and the run spell the connect reads differently, which is two \
                 sentences that can disagree about one request"
            );
        }
        assert!(
            attempted.ends_with(&[
                "$ kubectl get --raw /version".to_string(),
                "$ kubectl api-resources --verbs=list".to_string(),
            ]),
            "the wall's log does not end on discovery, so it names a read that never ran: \
             {attempted:?}"
        );
        assert!(
            !attempted.iter().any(|line| line.contains("--watch")),
            "the wall named a watch, and on that path no watch is ever started: {attempted:?}"
        );
    }
}

// --- THE CLOCK LINE ---
//
// **Both sentences are `screens/once.md` § When your clock and the cluster's disagree verbatim**,
// and they are written out here as literals rather than built from the same `format!` the product
// uses: a test that composes the string the way the code does passes for any wording, including
// the wrong one. What is asserted is the sentence a reader sees.
//
// **The threshold is not tested here and cannot be.** `Some` already means *past five minutes* —
// `k8s.rs`'s `measure` is where that is decided and `src/k8s_tests.rs` § WHAT THE `DATE` HEADER
// SAYS ABOUT THIS MACHINE'S CLOCK is where it is pinned, on both sides of the boundary.

/// The sentence an eleven-minute gap with this machine behind gets, as `screens/once.md` draws it.
const BEHIND: &str = "This computer and the cluster disagree about the time by 11 minutes (this \
                      one is behind), so recent times are missing and older ones can read smaller \
                      than they really are.";

/// The sentence a nine-minute gap with this machine ahead gets, as `screens/once.md` draws it.
const AHEAD: &str = "This computer and the cluster disagree about the time by 9 minutes (this one \
                     is ahead), so times can read larger than they really are.";

/// **Two directions, two sentences, and neither hedges** (`screens/states.md` § Two directions,
/// two sentences, because they break differently).
///
/// **The asymmetry is the point and is easy to lose.** Behind, `rules::age` does *two* things —
/// blanks what is younger than the gap, prints everything older short by it — so the sentence
/// names both; ahead it does one, and the sentence names one. A behind sentence that promised
/// only a blank is what NOTES § D177 reversed, and it was wrong in the direction that matters: 16
/// of 32 cards printed an age underneath it.
///
/// **Neither assigns fault**, which the pair before them did. k8rs measures a gap between two
/// clocks; with a middlebox thirty minutes fast between a laptop and an API server that agreed to
/// the second, *"your computer's clock is 29 minutes behind"* sent the reader to fix a machine
/// that was right (D177).
#[test]
fn the_two_directions_get_the_two_sentences_they_were_drawn_with() {
    assert_eq!(
        clock(Some(SignedDuration::from_mins(-11))).as_deref(),
        Some(BEHIND),
        "the behind sentence is not the one `screens/once.md` draws"
    );
    assert_eq!(
        clock(Some(SignedDuration::from_mins(9))).as_deref(),
        Some(AHEAD),
        "the ahead sentence is not the one `screens/once.md` draws"
    );
}

/// **Nothing measured is nothing printed** — the four silences [`k8s::Session::skew`] collapses
/// into one `None` (a refusal, no header, a header that will not parse, a gap inside the
/// allowance) arrive here as that one `None`, and this end of the pipe cannot tell them apart
/// because no renderer is allowed to (`screens/once.md` § The two cases that print nothing).
#[test]
fn a_clock_nothing_measured_prints_nothing() {
    assert_eq!(clock(None), None);
    assert_eq!(
        render(&[], &nothing_read()),
        "0 pods · 0 nodes\n\n○ nothing is broken",
        "the file-driven path has no cluster to have answered, so the report is byte-for-byte \
         the one it printed before this box"
    );
}

/// **Whole minutes, rounded to the nearest, and the floor stays out of reach of the singular.**
///
/// **The rounding is not decoration.** A `Date` has one-second resolution and is stamped before
/// the response is read, so a true offset of exactly 1800 s reaches this function as 1799-and-a-
/// bit — floored, the built binary printed **29 minutes** while `chronyc tracking` said 30.0
/// (`reports/2026-08-28-clock-skew-date-header.md` § 4), and two numbers disagreeing at 3am is the
/// doubt this line removes. `rules::age`'s floor is right for elapsed time, which genuinely
/// floors; a gap between two clocks does not.
///
/// **The floor is 5 and `1 minute` is unreachable**, which is what lets [`clock`] call [`plural`]
/// without ever drawing its singular: `Some` starts strictly past five minutes, so the smallest
/// input is 301 s and the smallest count is 5.
///
/// It is the *renderer* that rounds. The measurement stays whole in [`k8s::Session::skew`] for the
/// header pointer Phase 9 draws off the same field.
#[test]
fn the_magnitude_rounds_to_the_nearest_minute_and_never_reads_one() {
    let count = |seconds: i64| {
        let sentence = clock(Some(SignedDuration::from_secs(seconds))).expect("past the allowance");
        sentence
            .split(" minute")
            .next()
            .and_then(|head| head.rsplit(' ').next())
            .expect("the sentence names a count")
            .to_string()
    };

    // The two the middlebox actually served, floored to 29 and 14 before this was rounding.
    assert_eq!(count(-1799), "30", "1799s is thirty minutes to any reader");
    assert_eq!(count(-899), "15", "899s is fifteen minutes to any reader");
    // Half a minute rounds away from zero, and the second below it does not.
    assert_eq!(count(-330), "6");
    assert_eq!(count(-329), "5");
    // Both directions round the same way — `signum` is what carries that.
    assert_eq!(count(1799), "30");
    assert_eq!(count(329), "5");
    // The floor: one second past the allowance is the smallest reading there is, and it is 5.
    for seconds in [-301, 301] {
        assert_eq!(
            count(seconds),
            "5",
            "the smallest reading past the threshold is five minutes, so `plural` can never be \
             asked for `1 minute` — {seconds}s"
        );
    }
}

/// **A clock far enough out to be nonsense still gets a true sentence, never a panic** — the
/// year-9999 `Date` `src/k8s_tests.rs` measures, carried all the way to the line a reader sees.
///
/// **The number is ugly and is deliberately not dressed up here.** `screens/once.md` draws
/// minutes and nothing else, and inventing a second unit — *2 days*, *8 years* — would be this
/// file writing wording that belongs to `screens/`. NOTES § D177 upheld it: once a refusal can no
/// longer feed the measurement, a number this size can only come from an API server whose clock
/// really is unset, and it says so better than any cap. What matters is that the sentence stays
/// grammatical and stays true — the two clocks really are that far apart.
#[test]
fn an_absurd_clock_is_still_one_true_sentence() {
    assert_eq!(
        // The same reading `k8s_tests.rs`'s far-future test measures off `Wed, 01 Jan 5000`,
        // carried through to the line it becomes.
        clock(Some(SignedDuration::from_mins(-1_563_827_760))).as_deref(),
        Some(
            "This computer and the cluster disagree about the time by 1563827760 minutes (this \
             one is behind), so recent times are missing and older ones can read smaller than \
             they really are."
        )
    );
}

/// **Last, after the findings, on both paths through the report** (`screens/once.md` § When your
/// clock and the cluster's disagree).
///
/// The two paths are the reason this is not one assertion: a report with cards ends at the tally
/// and one without ends at `○ nothing is broken`, and the early return the second used to take is
/// exactly how a line added to the first goes missing from the second — which is the report a
/// reader on a healthy cluster sees, and the one where a blank time is most confusing.
#[test]
fn the_clock_line_comes_last_whether_or_not_anything_is_broken() {
    let mut input = read(&["oom.json"]);
    input.skew = Some(SignedDuration::from_mins(-11));

    assert_eq!(
        render(&[], &input),
        format!("1 pod · 0 nodes\n\n○ nothing is broken\n\n{BEHIND}")
    );
    assert_eq!(
        render(
            &[finding(Severity::Critical, pod_id("payments", "web-0"))],
            &input
        ),
        format!(
            "1 pod · 0 nodes\n\n● payments/web-0\n  Something happened\n  the numbers that prove \
             it\n  → do this about it\n\n1 critical\n\n{BEHIND}"
        )
    );
}

/// **What a session measured reaches the report the session prints**, which is the whole of what
/// this driver owes the box: `k8s.rs` reads the header, [`live_report`] carries the number,
/// [`clock`] spells it.
///
/// **And it lands under the watch-trouble lines, which is this driver's own layout and not a
/// `screens/` rule** (the PM's ruling, 2026-08-28). Three different lines are easy to conflate
/// here and only two of them exist today:
///
/// - the **watch-trouble** line, `▲ k8rs is not getting …` — per watch, in the report's own `● ▲`
///   vocabulary, written by [`unreadable`], drawn in no `screens/` file, and placed *above* the
///   block since before this box;
/// - the **clock** line, which this box adds at the end of the block;
/// - the **completeness notice**, `One node check is off: …`, which rides on [`Input::skipped`],
///   is empty on every live path, and arrives with the namespace-scoping box. `screens/once.md`
///   § Stacked with a check that could not run puts the clock line *above* that one — a rule about
///   a line this file cannot yet produce, so citing it for the order below would be borrowing a
///   sentence that governs something else.
///
/// So what is asserted is the layout as it actually is: trouble lines, cards, clock line last. It
/// is pinned because the clock line is the new thing in it and the pre-existing order is what it
/// must not disturb.
///
/// **The store is bootstrapped and *then* broken**, because that is the one shape where both are
/// printed at once: five failing watches publish no snapshot at all, so there would be no report
/// for a clock line to sit under.
#[tokio::test]
async fn a_measured_clock_reaches_the_live_report_and_sits_under_what_it_qualifies() {
    use futures_util::stream::StreamExt;
    let mut store = listed(Vec::new());
    let watches = k8s::session(offline(), k8s::Coverage::Cluster)
        .await
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();
    k8s::drive_watching(watches, &mut store, |_| {}).await;

    let mut last = String::new();
    let report = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect {
            skew: Some(SignedDuration::from_mins(9)),
            ..Default::default()
        },
    )
    .expect("a bootstrapped store with a watch in trouble is news");
    println!("{report}");

    assert!(
        report.ends_with(AHEAD),
        "the clock line is last, after the findings — got:\n{report}"
    );
    assert!(
        report.contains("not getting"),
        "this shape is meant to carry a watch-trouble line as well, and it has none — got:\n\
         {report}"
    );
    assert!(
        report.find("not getting") < report.find(AHEAD),
        "a watch-trouble line fell below the clock line — [`unreadable`] puts them above the \
         block and this box may not have moved them — got:\n{report}"
    );
    assert_eq!(
        report.matches("disagree about the time").count(),
        1,
        "one measurement is one sentence — got:\n{report}"
    );
}

// --- THE SERVING CERTIFICATE LINE ---
//
// **C2, the second trailer fact** (`screens/once.md` § When the API server's own certificate is
// running out, NOTES § D178). It is a [`k8s::Session`] field rather than a `Finding` — it names no
// cluster object, so it carries no severity and no place in the tally, the same way the clock line
// never has.
//
// **Both sentences are written out here as literals**, for [`BEHIND`]'s reason: a test that
// composes the string the way the product does passes for any wording, the wrong one included.
//
// **The dates are the committed certificates' own**, measured from [`now`] — the instant
// `scripts/certs-test.sh` pins and asserts both ends of. So the day counts below are figures a
// guard holds rather than numbers transcribed off a run, and all three bands of this line are
// drawn from the three files C1 already uses.

/// One committed certificate's **DER**, which is what rustls hands back off a handshake and what
/// [`k8s::expiry_of`] is written to take. `x509-parser` is the crate `rules.rs` parses with, so
/// nothing new is named to undo a PEM wrapper here.
fn der(name: &str) -> Vec<u8> {
    let pem = certificate(name);
    let (_, block) = x509_parser::pem::parse_x509_pem(&pem)
        .unwrap_or_else(|e| panic!("{name} is not a PEM certificate: {e}"));
    block.contents
}

/// One committed certificate's **PEM**, which is the shape a kubeconfig carries it in and the one
/// C1 parses (`rules::expires_at`, [`k8s::Session::client_certificate`]).
///
/// **One read for both shapes**, so a test about the wire form and a test about the file form can
/// never be looking at two different files.
fn certificate(name: &str) -> Vec<u8> {
    let path = format!(
        "{}/tests/fixtures/certs/{name}.crt.pem",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read(&path).unwrap_or_else(|e| panic!("certificate {path} does not read: {e}"))
}

/// **Whole days between [`now`] and the committed `expired-client` certificate's `notAfter`** —
/// the sibling of [`EXPIRES_IN_DAYS`], and asserted by `scripts/certs-test.sh` in the same list.
const EXPIRED_DAYS_AGO: u32 = 14;

/// The sentence a certificate inside the window gets, as `screens/once.md` draws it.
const EXPIRING: &str = "A certificate the API server presented — not your kubeconfig's — \
                        expires in 13 days (valid until 2026-09-05T00:00:00Z). Once it runs out, \
                        kubectl and everything else stop being able to reach this cluster until \
                        someone on the control plane renews it — not something k8rs can do.";

/// The sentence a certificate that has already run out gets, as `screens/once.md` draws it.
///
/// **It says *a* cluster and not *this* one, and that is the article doing the work.** This report
/// reached the reader, so this cluster was plainly reachable a moment ago; a sentence naming it
/// beside a claim that it cannot be reached would contradict the page it is printed on.
const EXPIRED: &str = "A certificate the API server presented — not your kubeconfig's — expired \
                       14 days ago (was valid until 2026-08-09T00:00:00Z). When that happens, \
                       kubectl and everything else stop being able to reach a cluster until \
                       someone on the control plane renews its certificate — not something k8rs \
                       can do.";

/// **A real certificate's DER goes in and C1's own answer comes back** — the positive
/// [`k8s::expiry_of`] cannot have in `k8s_tests.rs`, because reading these bytes puts a file under
/// `scripts/certs-test.sh`'s rule that it pins the instant they are measured from, and that file
/// keeps no such clock.
///
/// **It is asserted against `rules::expires_at` over the PEM, not against a date typed here.**
/// What the wrap has to be is *the same answer as the rule's own parser* (NOTES § D129) — a
/// literal would still pass the day the two came apart, which is the whole failure being guarded.
///
/// **It is also where the one-line PEM body is measured rather than assumed.** `expiry_of` writes
/// the base64 as a single line instead of folding it at 64 columns; that `x509-parser` reads it is
/// a fact about the crate, and this is what fails if it stops being one.
#[test]
fn pem_body_on_one_line_is_a_certificate_this_parser_reads() {
    for name in ["expiring-client", "healthy-client", "expired-client"] {
        let path = format!(
            "{}/tests/fixtures/certs/{name}.crt.pem",
            env!("CARGO_MANIFEST_DIR")
        );
        let pem = std::fs::read(&path).expect("the committed certificate reads");
        let by_the_rule = rules::expires_at(&pem).expect("the committed certificate has an expiry");
        assert_eq!(
            k8s::expiry_of(&der(name)),
            Some(by_the_rule),
            "{name}: the DER off a handshake and the PEM off the disk are the same certificate, \
             and the wrap answered something else — a second parser for *when does this expire* \
             is exactly what NOTES § D129 refuses"
        );
        println!("{name}: DER through the wrap reads {by_the_rule}");
    }
}

/// **The two bands get the two sentences they were drawn with**, byte for byte, from the committed
/// certificates and the pinned instant.
#[test]
fn the_two_bands_get_the_sentences_screens_once_draws() {
    assert_eq!(
        serving_certificate(k8s::expiry_of(&der("expiring-client")), &now()).as_deref(),
        Some(EXPIRING),
        "the expiring sentence is not the one `screens/once.md` draws"
    );
    assert_eq!(
        serving_certificate(k8s::expiry_of(&der("expired-client")), &now()).as_deref(),
        Some(EXPIRED),
        "the expired sentence is not the one `screens/once.md` draws"
    );
    assert!(
        EXPIRING.contains(&format!("expires in {EXPIRES_IN_DAYS} days"))
            && EXPIRED.contains(&format!("expired {EXPIRED_DAYS_AGO} days ago")),
        "the day counts in the two literals above are no longer the ones \
         `scripts/certs-test.sh` pins, so this test measures a different certificate than C1 does"
    );
    for sentence in [EXPIRING, EXPIRED] {
        // **The noun phrase claims a sample and not a cluster** (`screens/once.md` § When the API
        // server's own certificate is running out, rewritten 2026-08-28). *"The API server's own
        // certificate"* is a definite, singular claim drawn from one connection out of N: eight
        // consecutive runs against three replicas behind one balancer, with one replica reissued
        // to twelve days, printed it on three of the eight
        // (`reports/2026-08-28-c2-c3-against-a-real-api-server.md` § 2). Sampling narrows that
        // window and cannot close it, so the words have to survive the miss.
        assert!(
            sentence.starts_with("A certificate the API server presented"),
            "the definite singular is back, and this reading is one sample from a control plane \
             that may be several API servers: {sentence}"
        );
        assert!(
            sentence.contains("— not your kubeconfig's —"),
            "the clause that tells this certificate from C1's is missing, and the two can print \
             on the same report: {sentence}"
        );
        assert!(
            !sentence.contains('⚠'),
            "`● ▲ ○` is this report's whole vocabulary and a fourth symbol arrives with no legend"
        );
    }
    assert!(
        EXPIRED.contains("reach a cluster") && EXPIRING.contains("reach this cluster"),
        "the expired sentence named *this* cluster beside a claim it cannot be reached, on a \
         report that reached the reader"
    );
}

/// **A healthy control plane says nothing at all**, and the boundary is the same thirty days C1
/// warns the reader's own certificate at.
///
/// **`healthy-client` is the committed negative** — 354 days out, which is the ordinary state of a
/// working cluster and the one a `210 days` line would sit on every single run.
#[test]
fn a_certificate_outside_the_window_prints_nothing() {
    assert_eq!(
        serving_certificate(k8s::expiry_of(&der("healthy-client")), &now()),
        None,
        "a healthy serving certificate drew a line, which is noise on every run of every healthy \
         cluster"
    );
    assert_eq!(
        serving_certificate(None, &now()),
        None,
        "nothing was read and something was printed"
    );

    // The boundary itself, from both sides. The `notAfter` is *inside* the window at exactly
    // thirty days out — C1's own reading of RFC 5280 §4.1.2.5 — so the drawn side is `<=`.
    let at = |offset: SignedDuration| {
        serving_certificate(now().0.checked_add(offset).ok(), &now()).is_some()
    };
    assert!(
        at(k8s::CERT_EXPIRY_WARN),
        "exactly thirty days out drew nothing, and C1 reports its own certificate there"
    );
    assert!(
        !at(k8s::CERT_EXPIRY_WARN + SignedDuration::from_secs(1)),
        "a second past the window drew a line"
    );
}

/// **`less than a day` and never `0 days`** — the most urgent thing this line ever says, and the
/// one a truncating division would print as zero. `rules::in_days` makes the same call for C1.
#[test]
fn the_last_day_is_words_and_not_a_zero() {
    let hours = |n: i64| {
        serving_certificate(
            now().0.checked_add(SignedDuration::from_hours(n)).ok(),
            &now(),
        )
        .expect("inside the window")
    };
    assert!(
        hours(1).contains("expires in less than a day"),
        "an hour left printed a count: {}",
        hours(1)
    );
    assert!(
        hours(-1).contains("expired less than a day ago"),
        "an hour past printed a count: {}",
        hours(-1)
    );
    assert!(hours(25).contains("expires in 1 day"), "{}", hours(25));
    assert!(hours(49).contains("expires in 2 days"), "{}", hours(49));

    // **`notAfter` itself is still valid, so the deadline exactly is *expires* and not *expired***
    // — RFC 5280 §4.1.2.5, and C1's own reading of it one file over. The two sentences send a
    // reader to two different places: one is *go and ask someone*, the other is *this is already
    // broken*, and at the instant the clock reads `notAfter` only the first is true.
    let at_the_deadline =
        serving_certificate(Some(now().0), &now()).expect("the deadline is inside");
    assert!(
        at_the_deadline.contains("expires in less than a day"),
        "a certificate is valid *through* its `notAfter`, and the deadline itself was reported \
         as already run out: {at_the_deadline}"
    );
    assert!(
        hours(-1).contains("(was valid until"),
        "the past tense did not follow the expired branch: {}",
        hours(-1)
    );
    assert!(
        at_the_deadline.contains("(valid until"),
        "the present tense did not follow the expiring branch: {at_the_deadline}"
    );
}

/// **The trailer order is clock, then this** (`screens/once.md` § Stacked with the other trailer
/// lines), on both paths through the block — after a tally, and after `○ nothing is broken`.
///
/// **The clean-cluster case is the one this matters most for.** `○ nothing is broken` reads as
/// permission to look away, and a certificate days from taking the whole cluster down is exactly
/// the fact that permission would hide.
#[test]
fn the_certificate_line_comes_after_the_clock_line_whether_or_not_anything_is_broken() {
    let mut input = read(&["oom.json"]);
    input.skew = Some(SignedDuration::from_mins(-11));
    input.serving_expiry = k8s::expiry_of(&der("expiring-client"));

    let clean = render(&[], &input);
    assert_eq!(
        clean,
        format!("1 pod · 0 nodes\n\n○ nothing is broken\n\n{BEHIND}\n\n{EXPIRING}")
    );

    let broken = render(
        &[finding(Severity::Critical, pod_id("payments", "web-0"))],
        &input,
    );
    assert!(
        broken.ends_with(&format!("1 critical\n\n{BEHIND}\n\n{EXPIRING}")),
        "the two trailer lines are not clock-then-certificate under a tally: {broken}"
    );

    // Alone, with nothing measured about the clock: the certificate line does not depend on it.
    input.skew = None;
    assert!(
        render(&[], &input).ends_with(&format!("○ nothing is broken\n\n{EXPIRING}")),
        "the certificate line went missing when there was no clock line above it"
    );
}

/// **The report says what it covered, and a namespace-scoped one says which namespace**
/// (`screens/once.md` § When a check could not run, NOTES § D5).
///
/// **The header is where a reader decides whether to trust the rest**, and a report pasted into a
/// ticket as *nothing is broken* over one namespace of forty is the reason this line exists.
///
/// **Both causes print identically**, because the scope is identical: `--namespace` and the 403
/// fallback are one field by the time a snapshot exists (NOTES § D46), and *why* is said once on
/// stderr by the driver that decided it ([`scoped_because`]).
#[test]
fn a_scoped_report_says_which_namespace_it_covered() {
    let mut input = read(&["oom.json"]);
    assert_eq!(
        header(&input),
        "1 pod · 0 nodes",
        "an unscoped report grew a scope clause"
    );

    input.snapshot.namespace_scope = Some("payments".to_string());
    assert_eq!(
        header(&input),
        "ns: payments · 1 pod · 0 nodes",
        "the scope is missing, or it landed after the counts it is a count *of*"
    );

    // Invariant 9: the namespace has been through argv or a kubeconfig, and neither is ours.
    input.snapshot.namespace_scope = Some("pay\u{1b}[2Jments".to_string());
    let crafted = header(&input);
    println!("{crafted}");
    assert!(
        !crafted.contains('\u{1b}'),
        "an escape sequence in a namespace reached the terminal: {crafted:?}"
    );
}

/// **`nothing is broken` is never printed over a watch that could not be read** — the blocker
/// this round exists for (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R1, § R4,
/// § R10; the PM's ruling of 2026-08-29).
///
/// **Five measured shapes reached the claim, and four of them printed a trouble line first**: a
/// namespaced `Role` whose context names no namespace, `--namespace` on a namespace the role is
/// refused, `--namespace` on one that does not exist, a role with `get` and no `list`, and a
/// cluster-wide reader that cannot list nodes. **Before the box the tool hung on *loading*, which
/// was useless; after it the tool said the cluster was healthy, which is worse.**
///
/// **The guard is at the root and covers all five with one branch** ([`health`]), and it is read
/// off the same troubles the lines above the cards are drawn from — so a trouble line and a health
/// claim cannot appear in one report, by construction.
///
/// **A vital that was never read is left out and never printed as a measured zero**
/// (`screens/widgets.md` § 1a, and [`Input::unreadable`]). `nodes` is cluster-scoped and cannot be
/// granted by a namespaced `Role`, so `0 nodes` was printed on *every* successful scoped run.
///
/// **Stale is not the same as never read**, which is the other half of that rule one line further
/// down in `widgets.md` — a watch that listed and then went down keeps its count.
#[test]
fn a_health_claim_is_never_made_over_a_watch_that_could_not_be_read() {
    let mut input = read(&["oom.json"]);
    input.snapshot.pods.clear();
    input.snapshot.namespace_scope = Some("payments".to_string());

    // The measured shape: the pod and node watches were refused, so neither vital was ever read.
    input.unreadable = vec![ObjectKind::Pod, ObjectKind::Node];
    input.watch_trouble = true;
    let blind = render(&[], &input);
    println!("{blind}");
    assert!(
        !blind.contains("nothing is broken"),
        "the cluster was called healthy over a scope that read nothing: {blind:?}"
    );
    assert!(
        !blind.contains("0 pods") && !blind.contains("0 nodes"),
        "a vital nobody was allowed to read was printed as a measured zero: {blind:?}"
    );
    assert!(
        blind.starts_with("ns: payments\n"),
        "the header stopped saying what the report covered: {blind:?}"
    );

    // A cluster-wide reader refused only `nodes` — R5's shape. The pod count is real and stays;
    // the node count is a guess and goes; the claim goes with it.
    input.unreadable = vec![ObjectKind::Node];
    input.snapshot.namespace_scope = None;
    let no_nodes = render(&[], &input);
    println!("{no_nodes}");
    assert_eq!(
        no_nodes, "0 pods",
        "a run that could not list nodes still printed a node count or a health claim"
    );

    // **Stale, not unread**: every watch listed and then one went down. `widgets.md` is explicit
    // — stale vitals stay visible — and the claim goes, because a pod that broke after the watch
    // stopped was never seen.
    input.unreadable = Vec::new();
    let stale = render(&[], &input);
    println!("{stale}");
    assert_eq!(
        stale, "0 pods · 0 nodes",
        "a stale count was blanked as if it had never been read"
    );

    // Nothing wrong with any watch: the claim is back, in both scopes.
    input.watch_trouble = false;
    assert_eq!(
        render(&[], &input),
        "0 pods · 0 nodes\n\n○ nothing is broken"
    );
    input.snapshot.namespace_scope = Some("payments".to_string());
    let scoped = render(&[], &input);
    println!("{scoped}");
    assert!(
        scoped.contains("○ nothing is broken in payments"),
        "a claim over one namespace was made about the whole cluster: {scoped:?}"
    );

    // Invariant 9: the namespace reaches the claim as well as the header.
    input.snapshot.namespace_scope = Some("pay\u{1b}[2Jments".to_string());
    let crafted = render(&[], &input);
    assert!(
        !crafted.contains('\u{1b}') && crafted.contains("in pay[2Jments"),
        "the namespace reached the claim unstripped, or was stripped away entirely: {crafted:?}"
    );

    // **Findings do not bring the claim back and never suppressed it**: the guard is on the claim
    // alone, so a report with cards under an unreadable watch still prints its cards.
    input.watch_trouble = true;
    input.unreadable = vec![ObjectKind::Node];
    let carded = render(
        &[finding(Severity::Critical, pod_id("payments", "web-0"))],
        &input,
    );
    println!("{carded}");
    assert!(
        carded.contains("1 critical") && !carded.contains("nothing is broken"),
        "the cards or the tally went with the claim: {carded:?}"
    );
}

/// **A check that is switched off and says nothing looks exactly like a check that passed**
/// (`screens/once.md` § When a check could not run, `screens/states.md`
/// § You can only see some namespaces).
///
/// **It prints in both cases and that is the whole point**: a report with findings is no more
/// complete than an empty one when the same check was off, and `○ nothing is broken` is the
/// strongest claim k8rs makes.
///
/// **Last of the trailer, under the clock line and under the certificate line** — the order
/// `screens/once.md` § Stacked with the other trailer lines fixes, and the slot [`render`]'s own
/// comment has reserved for it since before this file could draw it.
#[test]
fn a_namespace_scope_says_which_node_check_is_off_and_says_it_last() {
    const OFF: &str = "One node check is off: spotting a node someone started emptying and did \
                       not finish needs every pod in the cluster.";

    let mut input = read(&["oom.json"]);
    assert_eq!(
        check_switched_off(None),
        None,
        "a report that covered the whole cluster said a check was off"
    );
    assert!(
        !render(&[], &input).contains("One node check is off"),
        "the line was drawn over an unscoped run"
    );

    input.snapshot.namespace_scope = Some("payments".to_string());
    let clean = render(&[], &input);
    println!("{clean}");
    assert_eq!(
        clean,
        format!("ns: payments · 1 pod · 0 nodes\n\n○ nothing is broken in payments\n\n{OFF}"),
        "`nothing is broken` was printed over a scoped cluster with no note that a check was off"
    );

    let broken = render(
        &[finding(Severity::Critical, pod_id("payments", "web-0"))],
        &input,
    );
    assert!(
        broken.ends_with(&format!("1 critical\n\n{OFF}")),
        "the line did not follow a tally: {broken}"
    );

    // Under both of the other two trailer lines, which is the order `screens/once.md` fixes.
    input.skew = Some(SignedDuration::from_mins(-11));
    input.serving_expiry = k8s::expiry_of(&der("expiring-client"));
    let stacked = render(&[], &input);
    println!("{stacked}");
    assert!(
        stacked.ends_with(&format!("{BEHIND}\n\n{EXPIRING}\n\n{OFF}")),
        "the trailer is not clock, certificate, then the check that could not run: {stacked}"
    );
}

/// **It carries no severity and appears in no tally** (NOTES § D178): it names no cluster object,
/// so there is no band for it to be counted in.
#[test]
fn the_certificate_line_is_in_no_tally_and_carries_no_symbol() {
    let mut input = read(&["oom.json"]);
    input.serving_expiry = k8s::expiry_of(&der("expired-client"));
    let report = render(
        &[finding(Severity::Critical, pod_id("payments", "web-0"))],
        &input,
    );
    assert!(
        report.contains("\n1 critical\n"),
        "the tally counted the certificate line: {report}"
    );
    assert!(
        !report.contains(&format!("● {EXPIRED}")) && !report.contains(&format!("▲ {EXPIRED}")),
        "the sentence was drawn as a card: {report}"
    );
}

/// **What a session read reaches the report the session prints, and it is the same string the
/// file path draws** — one sentence, two renderers, which is the rule NOTES § D177 was written
/// about.
#[tokio::test]
async fn a_read_certificate_reaches_the_live_report_as_the_same_sentence() {
    let expiry = k8s::expiry_of(&der("expiring-client"));
    let store = identified(
        objects::<Pod>("kube-system-pods.json"),
        objects::<Node>("nodes.json"),
        nearly_out(Some("v1.36.1")),
    );

    let mut last = String::new();
    let live = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect {
            serving_expiry: expiry,
            ..Default::default()
        },
    )
    .expect("every LIST landed");
    // **C2 is no longer the last line here and that is the trailer order, not a regression**:
    // this store's identity is [`nearly_out`], so the same report also carries C1's own trailer
    // underneath it (`screens/once.md` § Stacked with the other trailer lines). The claim below
    // is the one this test has always made — one reading, one sentence, in the slot the file
    // fixes — with the neighbour that arrived under it named rather than assumed away.
    assert!(
        live.ends_with(&format!("{EXPIRING}\n\n{LOGIN_EXPIRING}")),
        "the certificate line is not in the trailer slot `screens/once.md` gives it: {live}"
    );
    assert_eq!(
        live.matches("A certificate the API server presented")
            .count(),
        1,
        "one reading is one sentence: {live}"
    );

    let mut input = read(&["oom.json"]);
    input.serving_expiry = expiry;
    let file = render(&[], &input);
    assert!(
        file.ends_with(EXPIRING) && live.contains(EXPIRING),
        "the two renderers drew two different sentences, which is the defect class D177 named"
    );

    let mut last = String::new();
    let unread = live_report(
        &store,
        now(),
        &mut last,
        false,
        false,
        &AtConnect::default(),
    )
    .expect("every LIST landed");
    assert!(
        !unread.contains("A certificate the API server presented"),
        "a session that read nothing printed a sentence anyway: {unread}"
    );
}

// --- THE LOGIN CERTIFICATE LINE ---
//
// **C1's expiring band, the third trailer fact** (`screens/once.md` § When your own login is
// running out, NOTES § D87, § D188). Unlike C2 it *is* a `Finding` — it names the reader's own
// kubeconfig — but `Severity::Info` keeps it out of the card block, and until this box the only
// reader it had was the Certificates pane behind `--analysis`. So a default run told the reader
// the control plane's credential was running out and never told them their own was, which is the
// one credential on the page they can renew without asking anybody.
//
// **The sentence is written out as a literal**, for [`BEHIND`]'s and [`EXPIRING`]'s reason: a test
// that composes the string the way the product does passes for any wording, the wrong one
// included.
//
// **The dates are the committed `expiring-client` certificate's own**, measured from [`now`] —
// the instant `scripts/certs-test.sh` pins — so the day count is a figure a guard holds and not
// one transcribed off a run. It is the same certificate, the same instant and the same subtraction
// the Certificates pane draws its `13 days` row and its `13d` badge from, which is what makes
// "one fact, one place" assertable rather than asserted.

/// The sentence a kubeconfig certificate inside the window gets, as `screens/once.md` draws it.
const LOGIN_EXPIRING: &str = "Your kubeconfig certificate — the file on your own machine that \
                              proves who you are, not anything in the cluster — expires in 13 \
                              days (valid until 2026-09-05T00:00:00Z). Once it runs out the \
                              cluster stops accepting it, so kubectl stops working for you too — \
                              ask whoever gave you access for a new kubeconfig before that date, \
                              because k8rs cannot renew it.";

/// **The expiring band gets the sentence `screens/once.md` draws**, byte for byte.
#[test]
fn the_expiring_band_gets_the_sentence_screens_once_draws() {
    let drawn = login_certificate(k8s::expiry_of(&der("expiring-client")), &now())
        .expect("the committed certificate is inside the window");
    println!("{drawn}");
    assert_eq!(
        drawn, LOGIN_EXPIRING,
        "the trailer is not the sentence `screens/once.md` § When your own login is running out \
         draws"
    );
    assert!(
        LOGIN_EXPIRING.contains(&format!("expires in {EXPIRES_IN_DAYS} days")),
        "the day count in the literal above is no longer the one `scripts/certs-test.sh` pins, so \
         this test measures a different certificate than the Certificates pane does"
    );
    assert!(
        LOGIN_EXPIRING.starts_with("Your kubeconfig certificate"),
        "the sentence no longer opens on the one referent a reader could have, which is the whole \
         reason it carries no `— not your kubeconfig's —` clause of its own"
    );
    assert!(
        !LOGIN_EXPIRING.contains('⚠'),
        "`● ▲ ○` is this report's whole vocabulary and a fourth symbol arrives with no legend"
    );

    // **The cluster is what refuses, and the reader is told k8rs cannot fix it** — both from
    // `k8s-admin`'s reading of 2026-09-03. *"kubectl and k8rs both stop letting you log in"* put
    // the refusal in the tools, and the beginner invariant 14 is written for reads that as
    // *kubectl is broken*; the missing *k8rs cannot renew it* is the clause C1's own card and both
    // of C2's bands already carry, and without it that reader goes hunting for a key.
    assert!(
        LOGIN_EXPIRING.contains("the cluster stops accepting it"),
        "the sentence puts the refusal in the tools rather than in the cluster: {LOGIN_EXPIRING}"
    );
    assert!(
        LOGIN_EXPIRING.contains("k8rs cannot renew it"),
        "the one clause that stops a reader hunting for a key this tool does not have is missing, \
         and its three sibling sentences all carry it: {LOGIN_EXPIRING}"
    );
}

/// **Three silences, and the middle one is the point** — outside the window, already expired, and
/// nothing read at all.
///
/// **`expired-client` draws nothing here because it is already a card.** Past the deadline C1 is
/// `Severity::Critical`, which the block above the tally draws like every other finding
/// (NOTES § D87); a trailer beside it would be the same fact twice, which is the duplicate this
/// whole box exists to prevent rather than create.
#[test]
fn a_login_outside_the_window_or_already_gone_prints_nothing() {
    assert_eq!(
        login_certificate(k8s::expiry_of(&der("healthy-client")), &now()),
        None,
        "a healthy kubeconfig certificate drew a line, which is noise on every run of every \
         working login"
    );
    assert_eq!(
        login_certificate(k8s::expiry_of(&der("expired-client")), &now()),
        None,
        "the expired band drew a trailer line as well as the `Critical` card the block above \
         already draws it as"
    );
    assert_eq!(
        login_certificate(None, &now()),
        None,
        "nothing was read and something was printed"
    );

    // Both boundaries, from both sides. The `notAfter` is *inside* the window at exactly thirty
    // days out — C1's own reading of RFC 5280 §4.1.2.5 — and it is still inside it at the
    // deadline itself, which is where the card takes over.
    let at = |offset: SignedDuration| {
        login_certificate(now().0.checked_add(offset).ok(), &now()).is_some()
    };
    assert!(
        at(k8s::CERT_EXPIRY_WARN),
        "exactly thirty days out drew nothing, and C1 reports its own certificate there"
    );
    assert!(
        !at(k8s::CERT_EXPIRY_WARN + SignedDuration::from_secs(1)),
        "a second past the window drew a line"
    );
    assert!(
        at(SignedDuration::ZERO),
        "the deadline itself is inside the window — the certificate is valid *through* `notAfter`"
    );
    assert!(
        !at(SignedDuration::from_secs(-1)),
        "a second past the deadline drew a trailer line beside the card that already says it"
    );
}

/// **`less than a day` and never `0 days`** — the most urgent thing this line ever says, and the
/// one a truncating division would print as zero. [`in_days`] is shared with C2's line.
#[test]
fn the_last_day_of_a_login_is_words_and_not_a_zero() {
    let hours = |n: i64| {
        login_certificate(
            now().0.checked_add(SignedDuration::from_hours(n)).ok(),
            &now(),
        )
        .expect("inside the window")
    };
    let last = hours(1);
    println!("{last}");
    assert!(
        last.contains("expires in less than a day"),
        "an hour left printed as a day count: {last}"
    );
    assert!(
        hours(24).contains("expires in 1 day") && hours(48).contains("expires in 2 days"),
        "the day count is not singular at one day, or does not advance"
    );
}

/// **A default run puts one trailer line on the report, in the order `screens/once.md` fixes** —
/// clock, then the certificate the *cluster* presented, then this, then the check that could not
/// run.
#[test]
fn the_login_line_is_last_but_one_in_the_trailer_and_is_no_card() {
    const OFF: &str = "One node check is off: spotting a node someone started emptying and did \
                       not finish needs every pod in the cluster.";
    let mut input = read(&["oom.json"]);
    input.snapshot.client_certificate = Some(certificate("expiring-client"));

    let alone = render(&[], &input);
    println!("{alone}");
    assert!(
        alone.ends_with(LOGIN_EXPIRING),
        "a run whose only trailer fact is the reader's own login did not print it: {alone}"
    );
    assert_eq!(
        alone.matches("Your kubeconfig certificate").count(),
        1,
        "one certificate is one sentence: {alone}"
    );

    // The negative: the committed healthy certificate is 354 days out and says nothing.
    let mut healthy = read(&["oom.json"]);
    healthy.snapshot.client_certificate = Some(certificate("healthy-client"));
    assert!(
        !render(&[], &healthy).contains("Your kubeconfig certificate"),
        "a login that is fine drew a line on a report it has nothing to say about"
    );

    // Under both of the lines that come before it and above the one that comes after.
    input.skew = Some(SignedDuration::from_mins(-11));
    input.serving_expiry = k8s::expiry_of(&der("expiring-client"));
    input.snapshot.namespace_scope = Some("payments".to_string());
    let stacked = render(&[], &input);
    println!("{stacked}");
    assert!(
        stacked.ends_with(&format!(
            "{BEHIND}\n\n{EXPIRING}\n\n{LOGIN_EXPIRING}\n\n{OFF}"
        )),
        "the trailer is not clock, the cluster's certificate, this login, then the check that \
         could not run: {stacked}"
    );

    // **No card, no band, no tally entry** — the reasons C2's own line already states, and the
    // reason the block above may not draw `Severity::Info` at all (NOTES § D87).
    let counted = render(
        &[finding(Severity::Critical, pod_id("payments", "web-0"))],
        &input,
    );
    assert!(
        counted.contains("\n1 critical\n"),
        "the tally counted the login line: {counted}"
    );
    assert!(
        !counted.contains(&format!("▲ {LOGIN_EXPIRING}"))
            && !counted.contains(&format!("● {LOGIN_EXPIRING}")),
        "the sentence was drawn as a card: {counted}"
    );
}

/// **One fact, one place: `--analysis` draws the pane row and the trailer stays silent.**
///
/// Printing both would be the same fact twice in two shapes on one page — what NOTES § D188
/// opened this box to stop, not something to reintroduce for C1 (`screens/once.md` § When your own
/// login is running out).
#[test]
fn the_pane_wins_under_analysis_and_the_trailer_does_not_print_twice() {
    let printed = |analysis| {
        let store = identified(Vec::new(), Vec::new(), nearly_out(Some("v1.36.1")));
        let mut last = String::new();
        live_report(
            &store,
            now(),
            &mut last,
            analysis,
            false,
            &AtConnect::default(),
        )
        .expect("every LIST landed")
    };

    let bare = printed(false);
    println!("{bare}");
    assert!(
        bare.ends_with(LOGIN_EXPIRING),
        "the trailer is missing from the run that has no pane to draw it: {bare}"
    );
    assert!(
        !bare.contains("[certificates]"),
        "a run with no `--analysis` drew a pane: {bare}"
    );

    let with_panes = printed(true);
    assert!(
        !with_panes.contains(LOGIN_EXPIRING),
        "the trailer printed under `--analysis`, where the Certificates pane already draws the \
         same fact as a row: {with_panes}"
    );
    assert_eq!(
        with_panes
            .matches(&format!(
                "Your kubeconfig certificate expires in {EXPIRES_IN_DAYS} days"
            ))
            .count(),
        1,
        "the reader is told about one certificate more than once: {with_panes}"
    );
    assert!(
        with_panes.contains(&format!("[certificates] {EXPIRES_IN_DAYS}d")),
        "the pane that is supposed to be winning is not drawn at all: {with_panes}"
    );
}

/// **The trailer is muted only where the pane really draws the row, and never merely because a
/// flag was passed** — [`drawn_as_a_row`], and the defect this test used to be blind to.
///
/// **It asserted absence and never presence, so it passed on a run where nobody was told
/// anything** (`k8s-admin`, 2026-09-03 — D26's class). The pane's row needs C1's `Finding`, and
/// `rules::kubeconfig_certificate_expiring` opens on `snapshot.context.as_deref()?` while
/// [`login_certificate`] deliberately needs no context. So with no context and `--analysis` the
/// old condition muted a trailer nothing replaced: the run with *more* reporting said *less*, in
/// front of a credential about to lock the reader out.
///
/// **That shape is reachable and was measured, not imagined.** `k8s::kubeconfig_context` is
/// `drawable(...)`, which strips per invariant 9, so a context named entirely in control
/// characters answers `None` and still connects (NOTES § D202, closed one box ago).
///
/// **Every row asserts both halves**, which is the whole repair: exactly one of the trailer and
/// the pane row is drawn, and never neither.
#[test]
fn the_trailer_is_muted_only_where_the_pane_really_draws_the_row() {
    let page = |analysis: bool, context: Option<&str>| -> (String, String) {
        let mut input = read(&["oom.json"]);
        input.snapshot.client_certificate = Some(certificate("expiring-client"));
        input.snapshot.context = context.map(str::to_string);
        input.analysis = analysis;
        let findings = analyze(&input.snapshot);
        (
            render(&findings, &input),
            reports(&input.snapshot, &findings),
        )
    };

    for (row, analysis, context, trailer_expected) in [
        (
            "no flag, and a context: the trailer is the only reader",
            false,
            Some("prod-eu"),
            true,
        ),
        (
            "the flag and a context: the pane row replaces it",
            true,
            Some("prod-eu"),
            false,
        ),
        (
            "no flag, no context: C1 never fired, the trailer still tells them",
            false,
            None,
            true,
        ),
        (
            "the flag but no context: no row exists, so muting would tell them nothing at all",
            true,
            None,
            true,
        ),
    ] {
        let (report, panes) = page(analysis, context);
        let trailer = report.contains(LOGIN_EXPIRING);
        let drawn = analysis && panes.contains("Your kubeconfig certificate expires in");
        println!("{row}\n    trailer={trailer} pane row={drawn}");
        assert_eq!(
            trailer, trailer_expected,
            "{row}: the trailer is on the wrong side of this run"
        );
        assert!(
            trailer || drawn,
            "{row}: neither the trailer nor a pane row told the reader their login is running \
             out — a run said less than the run with less reporting"
        );
        assert!(
            !(trailer && drawn),
            "{row}: the same fact printed twice in two shapes on one page"
        );
    }
}

// --- WHEN THE CERTIFICATE IS WHY NOTHING CAME BACK ---
//
// **F2's half in this file: the one certificate reading that is not a trailer line.**
// `screens/states.md` § Before the TUI ever starts draws the message; `screens/once.md` § When the
// certificate is why nothing came back has why it replaces three generic ones rather than joining
// them.
//
// **The trap this region exists to hold shut is that it must never *cause* a failure.** That
// section calls it "a more specific *cannot reach the cluster*, not a fourth kind of failure", and
// on a load-balanced control plane k8rs's probe can meet an expired replica while the client is
// being served by a healthy one. So both sides are tested: expired-and-everything-broken prints
// it, and expired-but-readable runs normally.

/// The message `screens/states.md` draws, byte for byte — indent, wrapping, blank lines and both
/// of its numbers. Written out as a literal rather than composed from the product's own format
/// string, because a test that composes the string the way the code does passes for any wording,
/// the wrong one included.
///
/// **The drawing's own instants, not this file's [`now`].** The block pins *3 days ago* against
/// *2026-08-25T00:00:00Z*, and those are two spellings of one value: a literal that kept the
/// screen's words and substituted the file's clock would be asserting a sentence nobody drew, and
/// would not notice the two halves disagreeing — the defect class this pair is most exposed to
/// (NOTES § D177).
const CERTIFICATE_IS_WHY: &str = "k8rs: the certificate the API server presented expired 3 days ago

  Not your kubeconfig's — the API server's own, and it ran out on
  2026-08-25T00:00:00Z. That is why nothing about this cluster
  could be read this run: kubectl and anything else that connects
  to it the normal way is refused too, until someone on the
  control plane renews it — not something k8rs can do.

  If this cluster runs more than one API server behind a load
  balancer, trying again may reach one that still works.";

/// The `notAfter` [`CERTIFICATE_IS_WHY`] names, and the instant it is three days behind.
fn expired_at() -> Timestamp {
    "2026-08-25T00:00:00Z".parse().expect("a fixed timestamp")
}

fn three_days_later() -> Time {
    Time("2026-08-28T00:00:00Z".parse().expect("a fixed timestamp"))
}

/// **A session that read nothing, and a certificate that says why, is one sentence instead of a
/// wall** — and `main` turns it into exit 2.
///
/// **The wall is what was measured, on a real API server three days past its own `notAfter` with a
/// verifying kubeconfig**: `grep -c "API server's own certificate"` over the run was `0`, and what
/// printed instead was *nothing usable came back when k8rs tried to `get /version`*, worded
/// identically for `/apis` and for the pods watch
/// (`reports/2026-08-28-c2-c3-against-a-real-api-server.md` § 3).
///
/// **`k8s::session(offline(), …)` is the shape and not a stand-in.** Its two errors are a real
/// resolver failure against a name RFC 6761 reserves, so the `Unanswered` this asserts on is
/// classified from a genuine error rather than one this file built to be classified.
///
/// **It is asserted through [`live`] and not only through [`certificate_is_why`]**, because what
/// has to be true is that the run *stops here*: the greeting below this point is the wall.
///
/// **The streams are cut after two items each so that the failure is a failure and not a hang.**
/// A real watch never ends (`k8s.rs` § THE DRIVER), so a `live` missing this check would sit here
/// until the harness killed it — measured, this test ran past sixty seconds against the code
/// before the fix. Cut, the same code comes back with *every watch has stopped* and the assertion
/// below reads it in milliseconds.
#[tokio::test]
async fn an_expired_certificate_on_a_session_that_read_nothing_is_the_message_and_not_the_wall() {
    use futures_util::stream::StreamExt;
    // **The wording, against a fixed clock** — `live` below reads the real one, so the byte-for-
    // byte assertion has to be made where both instants are pinned. Same session shape, and the
    // `Unanswered` on both calls is a real resolver failure rather than one built to be
    // classified.
    let mut fixed = k8s::session(offline(), k8s::Coverage::Cluster).await;
    fixed.serving_expiry = k8s::Serving::Expired(expired_at());
    let drawn = certificate_is_why(&fixed, &three_days_later());
    println!("{}", drawn.clone().unwrap_or_default());
    assert_eq!(
        drawn.as_deref(),
        Some(CERTIFICATE_IS_WHY),
        "the run did not print `screens/states.md`'s sentence"
    );

    let mut session = k8s::session(offline(), k8s::Coverage::Cluster).await;
    session.serving_expiry = k8s::Serving::Expired(expired_at());
    session.watches = session
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();

    // **`live` reads the real clock, so what is asserted here is that the run *stops* here** —
    // the greeting below this point is the wall. The date is pinned; the age beside it is
    // whatever today makes it, and [`CERTIFICATE_IS_WHY`] above already holds every word.
    let said = watching(Ok(session), false).await;
    println!("{said}");
    assert!(
        said.starts_with("k8rs: the certificate the API server presented expired ")
            && said.contains(&format!("it ran out on\n  {}. That is why", expired_at())),
        "the run did not print `screens/states.md`'s sentence: {said:?}"
    );
    assert!(
        !said.contains("nothing usable came back"),
        "the generic wording printed beside the specific one: {said:?}"
    );
}

/// **k8rs never refuses to start on a cluster it could otherwise read** — the invariant this whole
/// change is fenced by (`k8s-admin`, 2026-08-28).
///
/// **Three sessions that must all run normally**, each a shape a real cluster produces:
///
/// * **The version answered.** An HA control plane where the probe met the expired replica and the
///   client did not. A typed expiry that ended the session by itself would take this cluster down.
/// * **Both calls refused with `403`.** The `nonResourceURLs` shape NOTES § D160 measured: a
///   kubeconfig whose role may not `get /version` or `get /apis` and lists pods perfectly well.
///   *Refused* is not *nothing came back*, and reading it as such would turn a documented
///   least-privilege role into a tool that will not start.
/// * **No typed expiry at all**, which is every ordinary run.
/// * **One call answered and the other one silent, both ways round.** The same HA control plane
///   as the first case, caught mid-failover: `get /version` came back from a healthy replica while
///   `get /apis` reached the expired one, or the reverse. The condition is an **and** for exactly
///   this — one answer from this address is proof the cluster can be read, and either half alone
///   would refuse a start on a run that works. Measured as a surviving `&&` → `||` mutant on
///   2026-08-28, with the three cases above all passing under it.
#[tokio::test]
async fn a_cluster_that_can_still_be_read_is_never_refused_a_start() {
    let expired = |mut session: k8s::Session| {
        session.serving_expiry = k8s::Serving::Expired(expired_at());
        session
    };
    let refused = || api_error(403, "Forbidden");
    // **Real `Unanswered`s, taken off a session rather than built to be classified**: a resolver
    // failure against a name RFC 6761 reserves.
    let dead = k8s::session(offline(), k8s::Coverage::Cluster).await;
    let (no_version, no_apis) = (dead.version, dead.served);
    let discovered = || {
        Ok(k8s::Served {
            kinds: Vec::new(),
            capabilities: None,
        })
    };

    assert_eq!(
        certificate_is_why(
            &expired(saying(Ok("v1.36.1".to_string()), Err(refused()), None)),
            &three_days_later()
        ),
        None,
        "the probe met an expired replica while the client was reading the cluster, and k8rs \
         refused to start — which turns a diagnostic into an outage on a working cluster"
    );
    assert_eq!(
        certificate_is_why(
            &expired(saying(Err(refused()), Err(refused()), None)),
            &three_days_later()
        ),
        None,
        "a kubeconfig whose role lacks the `nonResourceURLs` grant was told its cluster's \
         certificate has expired, and refused a start it makes today (NOTES § D160)"
    );
    assert_eq!(
        certificate_is_why(
            &k8s::session(offline(), k8s::Coverage::Cluster).await,
            &three_days_later()
        ),
        None,
        "a cluster that is simply not there was told the certificate expired, on a probe that \
         read nothing at all"
    );
    assert_eq!(
        certificate_is_why(
            &expired(saying(Ok("v1.36.1".to_string()), no_apis, None)),
            &three_days_later()
        ),
        None,
        "`get /version` answered and `get /apis` did not, and k8rs refused to start — one answer \
         from this address is proof the cluster can be read"
    );
    assert_eq!(
        certificate_is_why(
            &expired(saying(no_version, discovered(), None)),
            &three_days_later()
        ),
        None,
        "the same shape the other way round: discovery answered, `get /version` did not, and the \
         run was refused anyway"
    );
}

/// **The other half of the same ruling: a probe that met an expired replica while the session read
/// the cluster fine prints the ordinary expired trailer, where it used to print nothing.**
///
/// **It is one sentence and not a second one** (`screens/once.md` § *A clean tally does not mean
/// every replica is current*). [`k8s::Serving::Expired`] carries a date now, so it composes from
/// what the report already draws — asserted here by feeding [`EXPIRED`]'s own certificate through
/// both readings and demanding the same bytes, which is the check that catches a second wording
/// growing beside the first (NOTES § D177).
///
/// **`○ nothing is broken` is the case the ruling names**, because that is the screen where a
/// control-plane replica already past its `notAfter` is the only thing on the page worth saying.
#[test]
fn an_expired_replica_on_a_readable_cluster_draws_the_ordinary_expired_line() {
    let at = k8s::expiry_of(&der("expired-client")).expect("the committed certificate has a date");
    let drawn = |reading: k8s::Serving| {
        let mut input = read(&["oom.json"]);
        input.serving_expiry = reading.until();
        render(&[], &input)
    };
    let typed = drawn(k8s::Serving::Expired(at));
    println!("{typed}");
    assert!(
        typed.ends_with(&format!("○ nothing is broken\n\n{EXPIRED}")),
        "a replica whose certificate has already expired printed nothing at all: {typed}"
    );
    assert_eq!(
        typed,
        drawn(k8s::Serving::Until(at)),
        "the typed refusal and a completed handshake drew two different sentences about one \
         `notAfter`, which is the second wording D177 refuses"
    );
    assert!(
        !drawn(k8s::Serving::Unread).contains(EXPIRED),
        "a probe that read nothing printed a certificate sentence anyway"
    );
}

/// **A session that read nothing without a typed expiry still gets the wall, and that is the
/// unchanged half** — the message is a rename of a failure, never an extra one.
///
/// It is the negative for the test above it: same session, same silence, one field different.
///
/// **The streams are cut after two items each** for
/// [`a_cluster_that_never_answers_prints_nothing_and_says_why_it_stopped`]'s reason: a real watch
/// never ends, so a `live` that gets past the check under test would hang the suite rather than
/// fail it. That the positive above needs no such cut is itself the point — it returns before the
/// watches are ever driven.
#[tokio::test]
async fn without_a_typed_expiry_the_same_dead_session_still_says_nothing_usable_came_back() {
    use futures_util::stream::StreamExt;
    let mut session = k8s::session(offline(), k8s::Coverage::Cluster).await;
    session.watches = session
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();

    let said = watching(Ok(session), false).await;
    println!("{said}");
    assert!(
        !said.contains("the certificate the API server presented expired"),
        "a cluster that is merely unreachable was told its certificate has expired: {said:?}"
    );
}

/// **A certificate past [`k8s::CERTIFICATE_BYTES`] is not read, and one at the cap is** — the
/// security gate's *sizes are bounded*, over the value a server chooses.
///
/// **Padded rather than invented, because trailing bytes are tolerated and that was measured.**
/// `x509-parser` reads the DER `SEQUENCE` and ignores what follows it — `healthy-client`'s 843
/// bytes with sixteen zeroes after them parse to the same instant — so a padded real certificate
/// is a *valid* input that is only refused by the bound. A run of zeroes would be refused by the
/// parser whatever the cap said, which is a test that cannot fail.
#[test]
fn a_served_certificate_past_the_cap_is_not_read_and_one_at_the_cap_is() {
    let cap = k8s::CERTIFICATE_BYTES as usize;
    let real = der("healthy-client");
    assert!(
        real.len() < cap,
        "the committed certificate is {} bytes, which is not under the {cap}-byte cap this test \
         pads up to",
        real.len()
    );
    let padded = |size: usize| {
        let mut der = real.clone();
        der.resize(size, 0);
        k8s::expiry_of(&der)
    };
    assert_eq!(
        padded(cap),
        k8s::expiry_of(&real),
        "a certificate exactly at the cap was refused, so the bound is off by one and a real \
         chain could be dropped for being the size it is"
    );
    assert_eq!(
        padded(cap + 1),
        None,
        "a certificate one byte past the cap was copied, base64-encoded and parsed — the value \
         is whatever the server chose to send"
    );
}

// --- ONE REPORT AND OUT ---
//
// **`--once` is the shape v0.0.1 ships** (NOTES § D10, § D17, `screens/once.md`): connect, print
// one report, exit — `0` if it ran and reported whether or not anything was broken, `2` if it
// could not run, and never `1`.
//
// **What is asserted here is the ending and the argv, because stdout belongs to the process.**
// `live` writes the report with `writeln!(std::io::stdout(), …)`, which the harness does not
// capture, so a test can read what `live` *returned* and what [`live_report`] would have drawn
// over the same store — the split § WATCHING A CLUSTER above already works under. The two
// process-level halves — which stream the report lands on, and what the process exits with — are
// `tests/binary.rs`'s.

/// **`--once` is a cluster flag, and every flag that qualifies one applies to it unchanged.**
///
/// **`--live` is asserted beside it in each case**, because the failure this guards is not *the
/// flag does nothing* — it is *the flag does something slightly different*, and one mode's answer
/// read on its own cannot show that. `--context` and `--namespace` come out of the same two
/// functions for both, which is the point: there is one cluster path and `--once` is a stopping
/// point on it, not a second one (`screens/once.md` § What `--once` does not do).
#[test]
fn once_reaches_the_cluster_path_and_carries_context_and_namespace_the_way_live_does() {
    let args = |line: &[&str]| -> Vec<String> { line.iter().map(|a| (*a).to_string()).collect() };

    assert!(once_wanted(&args(&["--once"])));
    assert!(once_wanted(&args(&["--analysis", "--once"])));
    assert!(!once_wanted(&args(&["--live"])));
    assert!(!once_wanted(&args(&["pod.json"])));
    // Not a prefix match: a word that merely starts like the flag is not the flag.
    assert!(!once_wanted(&args(&["--once=true"])));

    // The file-driven path is untouched: no cluster flag, no cluster.
    assert_eq!(live_context(&args(&["pod.json"])), None);
    assert_eq!(live_context(&args(&["--analysis", "pod.json"])), None);

    for mode in ["--once", "--live"] {
        assert_eq!(
            live_context(&args(&[mode])),
            Some(None),
            "{mode} did not reach the cluster path"
        );
        assert_eq!(
            live_context(&args(&[mode, "--context", "kind-k8rs"])),
            Some(Some("kind-k8rs")),
            "{mode} dropped the context it was pointed at"
        );
        assert_eq!(
            live_context(&args(&[mode, "--context=kind-k8rs"])),
            Some(Some("kind-k8rs")),
            "{mode} dropped an attached context"
        );
        for spelling in [
            vec![mode, "--namespace", "payments"],
            vec![mode, "--namespace=payments"],
            vec![mode, "-n", "payments"],
            vec![mode, "-n=payments"],
        ] {
            assert_eq!(
                live_namespace(&args(&spelling)),
                Some("payments"),
                "{spelling:?} did not scope the run"
            );
        }
    }
    // Both together is a cluster run with a stopping point, not a usage error and not a file run.
    assert_eq!(live_context(&args(&["--once", "--live"])), Some(None));
    assert!(once_wanted(&args(&["--once", "--live"])));
}

/// **Every refusal `--live` gets for a bad line, `--once` gets too — and a file beside either is
/// now one of them.**
///
/// **The file is the case this box added.** `k8rs --once pod.json` used to read the cluster and
/// say nothing whatever about the file the reader had named ([`live_context`] answers the cluster
/// and drops the path), which is the silent-wrong-input shape [`mistyped`] already refuses three
/// other ways round. It is refused for `--live` as well, because it is one rule about one
/// ambiguity.
///
/// **The negatives are the half that makes it a test.** A value that follows `--context`,
/// `--namespace` or `-n` is that flag's and not a file, and the file-driven path — which has no
/// cluster flag on it at all — must still read every path it is given.
#[test]
fn a_file_beside_a_cluster_flag_is_refused_and_a_flags_own_value_is_not_a_file() {
    let args = |line: &[&str]| -> Vec<String> { line.iter().map(|a| (*a).to_string()).collect() };
    let refused = |line: &[&str]| mistyped(&args(line));

    for mode in ["--once", "--live"] {
        let said = refused(&[mode, "pod.json"]).unwrap_or_else(|| {
            panic!("{mode} beside a file was accepted, so the file was read by nothing")
        });
        println!("{said}");
        assert!(
            said.contains("pod.json"),
            "the refusal does not name the file that was ignored: {said:?}"
        );
        // **The sentence names the mode that is on the line**, which is stricter than the
        // literal that stood here: it read `--once and --live read a cluster` for every mode,
        // and once `--logs` became a third cluster flag that was a sentence naming two flags
        // the run did not have (NOTES § D190's class, `dev-core` 2026-08-30).
        assert!(
            said.starts_with(&format!("k8rs: {mode} reads a cluster")),
            "{said:?}"
        );
        assert!(said.contains("usage: k8rs "), "{said:?}");
        // The path is refused wherever on the line it sits, including in front of the flag.
        assert!(refused(&["pod.json", mode]).is_some(), "{mode}");
        assert!(
            refused(&[mode, "--analysis", "a.json", "b.json"]).is_some(),
            "{mode}"
        );

        // A value is not a file. All four spellings, because the two that attach the value with
        // `=` are one word and the two that do not are two, and only the second shape can be
        // mistaken for a path.
        for line in [
            vec![mode],
            vec![mode, "--analysis"],
            vec![mode, "--context", "kind-k8rs"],
            vec![mode, "--context=kind-k8rs"],
            vec![mode, "--namespace", "payments"],
            vec![mode, "--namespace=payments"],
            vec![mode, "-n", "payments"],
            vec![mode, "-n=payments"],
            vec![mode, "--context", "kind-k8rs", "-n", "payments"],
        ] {
            assert_eq!(
                refused(&line),
                None,
                "{line:?} was refused, and every word in it is a flag or a flag's own value"
            );
        }
    }

    // **A typo is the more specific complaint about the same line**, so it is the one printed.
    let typo = refused(&["--once", "--anaylsis", "pod.json"])
        .expect("a word that starts like a flag and is not one is a usage error");
    println!("{typo}");
    assert!(
        typo.contains("--anaylsis is not a flag k8rs has"),
        "a mistyped flag beside a file was reported as the file: {typo:?}"
    );

    // **The file-driven path is untouched**: with no cluster flag there is no ambiguity, and
    // every one of these is a path this build still reads.
    for line in [
        vec!["pod.json"],
        vec!["--analysis", "pod.json"],
        vec!["a.json", "b.json"],
    ] {
        assert_eq!(mistyped(&args(&line)), None, "{line:?}");
    }
    // `--once` itself is a flag k8rs has, which is what the unknown-flag arm would deny it —
    // and `--once=true` is not, for [`LIVE`]'s reason: an `=` form nothing accepts used to fall
    // through as a path and come back `--live=true: No such file or directory`.
    assert_eq!(mistyped(&args(&["--once"])), None);
    let attached = mistyped(&args(&["--once=true"])).expect("--once takes no value");
    println!("{attached}");
    assert!(
        attached.starts_with("k8rs: --once=true is not a flag k8rs has"),
        "{attached:?}"
    );
}

/// **Three shapes of the flag line that were wrong in the same way: a word this build says
/// nothing about** (`k8s-admin`, `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 6).
///
/// **`--read-only` is accepted and does nothing** (`screens/once.md` § What `--once` does not do,
/// which says exactly that of the version this build ships). It was refused with exit `2`, so a
/// reader following the screen spec learned k8rs has no such flag and would learn otherwise a
/// release later. [`READ_ONLY`]'s doc carries what Phase 7 owes it.
///
/// **`--context` with nothing usable after it is refused, in both modes.** `k8rs --once
/// --context` exited **0** on the current cluster while `k8rs --once --namespace` exits `2` ten
/// lines away in the same function — so `k8rs --once --context "$CTX" && kubectl apply -f prod/`
/// with `CTX` unset was a green light about the wrong cluster, silently, which is the class this
/// file already refuses `--context --live` for.
///
/// **`-o json` said two false things in one sentence.** *"--once and --live read a cluster, so
/// k8rs cannot also read json"*: `-o` was skipped without a word — the unknown-flag check only
/// tests `--` words — and `json` fell through as a stray positional. `screens/once.md` lists
/// `-o json` by name as a shape readers will try.
///
/// **Both modes for all three**, because each is one rule about one line and a rule that held for
/// one of two modes is the second rule this driver would then have (NOTES § D189).
#[test]
fn the_flags_this_build_accepts_and_the_ones_it_now_names_instead_of_dropping() {
    let args = |line: &[&str]| -> Vec<String> { line.iter().map(|a| (*a).to_string()).collect() };
    let refused = |line: &[&str]| mistyped(&args(line));

    for mode in ["--once", "--live"] {
        // **Accepted**, and it reaches the cluster path unchanged: there is no write path for it
        // to guard yet and refusing it teaches the wrong thing.
        assert_eq!(
            refused(&[mode, "--read-only"]),
            None,
            "{mode} --read-only is refused, and `screens/once.md` says v0.0.1 accepts it"
        );
        assert_eq!(live_context(&args(&[mode, "--read-only"])), Some(None));
        assert_eq!(
            refused(&[mode, "--read-only", "--analysis", "-n", "payments"]),
            None,
            "{mode}"
        );

        // **`--context` with nothing usable after it.** Three spellings of nothing, and the one
        // shape an `=` says was meant is not one of them.
        for nothing in [
            vec![mode, "--context"],
            vec![mode, "--context", ""],
            vec![mode, "--context="],
        ] {
            let said = refused(&nothing).unwrap_or_else(|| {
                panic!(
                    "{nothing:?} was accepted, so this run connects to whatever cluster the \
                        kubeconfig is currently pointing at and says nothing about it"
                )
            });
            println!("{said}");
            assert!(
                said.starts_with("k8rs: --context needs the name of a context"),
                "{nothing:?}: {said:?}"
            );
            assert!(said.contains("usage: k8rs "), "{said:?}");
        }
        assert_eq!(
            refused(&[mode, "--context=--live"]),
            None,
            "{mode} --context=--live is refused, and an `=` says the value was meant \
             (`live_context`)"
        );
        assert_eq!(refused(&[mode, "--context", "kind-k8rs"]), None, "{mode}");

        // **A one-dash word this build does not have is named, not dropped.**
        let output = refused(&[mode, "-o", "json"])
            .expect("-o is not a flag k8rs has and a run that drops it says nothing true");
        println!("{output}");
        assert!(
            output.starts_with("k8rs: -o is not a flag k8rs has"),
            "{output:?}"
        );
        assert!(
            !output.contains("cannot also read json"),
            "the value of a flag k8rs does not have was reported as a file the reader named: \
             {output:?}"
        );
        // The two one-dash words that *are* real stay real, and the one refused for its own
        // reason keeps its own sentence.
        assert_eq!(refused(&[mode, "-n=payments"]), None, "{mode}");
        assert_eq!(refused(&[mode, "-n", "payments"]), None, "{mode}");
        let attached = refused(&[mode, "-npayments"]).expect("-npayments is refused");
        assert!(
            attached.contains("has to be separate from -n"),
            "{attached:?}"
        );
    }

    // **The file-driven path is untouched.** With no cluster flag there is no ambiguity, and
    // `k8rs -x file.json` stays a path exactly as `NAMESPACE_SHORT`'s doc promises.
    assert_eq!(mistyped(&args(&["-x", "pod.json"])), None);
    assert_eq!(mistyped(&args(&["-o", "pod.json"])), None);
}

/// **A `--once` run that reached the cluster and printed a report ends at exit `0` — whether or
/// not anything was broken** (NOTES § D17, `screens/once.md` § Exit codes).
///
/// **A cluster with nothing in it is the case that separates the two halves of that sentence.**
/// Findings do not change the exit code, so *nothing broken* and *thirteen things broken* end the
/// same way — and a driver that only ever reported on a broken cluster would never have run this
/// line. `k8rs` is a report, not a linter: a beginner who sees `$?` = 1 concludes the tool failed.
///
/// **It also proves the run *ends*.** The watches under it never stop — kube's `watcher()` cannot
/// finish and `k8s::StandingBackoff` never gives up — so nothing but the stopping point this box
/// added can return this call, and a `--live` in its place hangs until the harness kills it. That
/// is why the streams are **not** cut with `take` the way § WATCHING A CLUSTER's cut theirs.
///
/// **The deadline is ten seconds and is not what is being measured** — five empty lists off a
/// loopback listener land in milliseconds. It is here so a machine under load reports a wrong
/// sentence rather than hanging a CI run.
///
/// **`None` on its own does not say a report was printed, so it is not asserted on its own**
/// (`tester`, 2026-08-30). It is the same `None` for one report, for six and for none at all —
/// stdout belongs to the process — which left the box's own title unasserted here and green over
/// a binary that printed six. **How many** is `tests/binary.rs`'s, which counts headers on a real
/// stdout. **That there is one at all** is this test's, and it is proved by driving the same
/// listener and asking [`live_report`] what the store it leaves behind renders: `None` plus *this
/// store is a report* is *this run printed a report*, and the third case is excluded.
#[tokio::test]
async fn a_once_run_that_reported_ends_by_itself_and_has_no_sentence_to_return() {
    let session = k8s::session(emptied().await, k8s::Coverage::Cluster).await;

    let ending = live(Ok(session), false, Some(in_a_moment(10_000))).await;

    assert_eq!(
        ending, None,
        "a --once run that printed a report came back with a sentence, which `main` turns into \
         exit 2 — a report on stdout and a failure code is the tool calling its own answer a \
         failure"
    );
    // **The half `None` cannot carry**: the same five watches, driven here, and what the run
    // would have handed `writeln!`.
    let store = driven(emptied().await).await;
    let printed = live_report(
        &store,
        now(),
        &mut String::new(),
        false,
        // The store a `--once` run reaches, read the way that run reads it.
        true,
        &AtConnect::default(),
    )
    .expect("the store a --once run reaches is a report, or `None` above means it printed none");
    println!("{printed}");
    assert!(
        printed.contains("0 pods · 0 nodes"),
        "the run ended without a sentence and over a store that renders no header, which is a \
         --once that reported nothing and exited 0: {printed:?}"
    );
}

/// A budget of `n` milliseconds starting now — [`live`]'s parameter since [`ONCE_DEADLINE`]
/// became the moment the **whole run** has to be over by rather than a fresh window for the watch
/// loop ([`cluster_run`], [`Budget`]).
fn in_a_moment(milliseconds: u64) -> Budget {
    let whole = std::time::Duration::from_millis(milliseconds);
    Budget {
        whole,
        ends_at: tokio::time::Instant::now() + whole,
    }
}

/// The store five watches leave behind once their initial LISTs have landed or settled — what
/// [`live`]'s own closure is handed, built the one way a test can reach it.
///
/// **`take(2)` is what ends streams that cannot end.** kube's `watcher()` retries forever, so an
/// uncut stream would leave `k8s::drive_watching` running until the harness gave up; two events
/// is `Init` plus `InitDone` for a list with nothing in it, and a refusal's `Err` plus the `Init`
/// that follows it.
///
/// **It is also what hid the stub defect [`emptied`]'s doc records**: over that listener before
/// 2026-08-30, event three of every watch was a failure, and this cut stopped one event short of
/// it every time.
async fn driven(client: kube::Client) -> k8s::Store {
    use futures_util::stream::StreamExt;
    let watches = k8s::session(client, k8s::Coverage::Cluster)
        .await
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();
    let mut store = k8s::Store::default();
    k8s::drive_watching(watches, &mut store, |_| {}).await;
    assert!(
        store.still_listing().is_empty(),
        "the bootstrap gate did not open, so this store is not the one a --once run reports over"
    );
    store
}

/// **A cluster that will not show k8rs its pods is exit `2` and one sentence, not a report**
/// (`screens/once.md` § Exit codes puts *not allowed to list pods* in the `2` row; § When the
/// certificate is why nothing came back: *one specific sentence and a non-zero exit, never a list
/// of every symptom*).
///
/// **Measured before it was written.** Against a real cluster reached with a credential it would
/// not accept, this printed five `▲ k8rs is not getting … from this cluster` lines **on stdout**
/// and exited **0** — so `k8rs --once && echo all good` printed *all good* about a cluster k8rs
/// had never been shown, and `k8rs --once > findings.txt` left a file of symptoms where a report
/// belongs (`dev-core`, 2026-08-30, against `kind-k8rs`).
///
/// **Pods and not any refused watch**, which is the half that keeps the ordinary run working: a
/// namespaced `Role` cannot grant cluster-scoped `nodes`, so `nodes` is refused on a run that is
/// otherwise perfectly good (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R2), and
/// an exit `2` for that would fail every scoped run there is. [`pods_refused`] is where that line
/// is drawn and the test beside this one is where it is asserted.
#[tokio::test]
async fn a_once_run_that_was_never_shown_a_pod_is_one_sentence_and_not_a_wall_of_symptoms() {
    let session = k8s::session(refusing().await, k8s::Coverage::Cluster).await;

    let refused = live(Ok(session), false, Some(in_a_moment(10_000)))
        .await
        .expect("a cluster that refused the pod watch has no report and must say so");

    println!("{refused}");
    assert!(
        refused.starts_with(
            "k8rs: this cluster did not show k8rs its pods, and every finding starts there"
        ),
        "{refused:?}"
    );
    // **The verb and the resource**, which is what the security gate requires of a refusal and
    // what a reader puts in a `Role`.
    assert!(
        refused.contains("`list` and `watch` pods"),
        "the sentence does not name what k8rs was refused, so there is nothing to go and grant: \
         {refused:?}"
    );
    // **One kind, not five.** The four other refused watches are not walked through one at a
    // time — that is `--live`'s screen and the thing `--once` exists not to print.
    for other in ["nodes", "Deployments", "StatefulSets", "DaemonSets"] {
        assert!(
            !refused.contains(other),
            "the refusal listed {other} as well, which is the wall of symptoms `--once` exists \
             not to print: {refused:?}"
        );
    }
}

/// **Which refusal ends a `--once` run and which one is just a line above the cards.**
///
/// **The discrimination is the whole test, so both sides are built from one store each and the
/// two stores differ only in which watch listed.** Pods refused is the run that has nothing to
/// report; nodes refused with pods read is the ordinary namespaced-`Role` run, which reports and
/// exits `0` with `analysis.rs`'s *needs permission to list nodes* rows where the numbers would
/// be. A predicate that keyed on *any* unlisted watch passes the first and fails the second.
///
/// **A watch that listed and then broke is not a refusal either** — `k8s::Trouble::listed` is the
/// field that tells stale from never-read, and stale pods are still a report.
#[test]
fn only_a_pod_watch_that_never_listed_ends_the_run_and_a_stale_one_does_not() {
    let trouble = |kind: ObjectKind, listed: bool| k8s::Trouble {
        kind,
        listed,
        ended: false,
        failure: None,
        unfinished: false,
        outstanding: None,
    };

    let stops = pods_unread(
        &[trouble(ObjectKind::Pod, false)],
        &k8s::Coverage::Cluster,
        None,
    )
    .expect("a pod watch that never listed is the end of the run");
    println!("{stops}");
    assert!(stops.contains("nothing to report"), "{stops:?}");
    // `failure: None` is the stream that ended without saying why — [`unreadable`]'s clause, so
    // the two cannot drift into two ways of saying one thing.
    assert!(
        stops.ends_with("nothing was ever said about why"),
        "{stops:?}"
    );
    // **And it invents no next step for it** — the fallback [`because`] refuses, in the one arm
    // that has no typed fault to build one from.
    assert!(!stops.contains("Ask whoever"), "{stops:?}");

    for reported in [
        vec![trouble(ObjectKind::Node, false)],
        vec![trouble(ObjectKind::Deployment, false)],
        vec![
            trouble(ObjectKind::Node, false),
            trouble(ObjectKind::DaemonSet, false),
        ],
        // Pods listed once and then broke: stale, and stale is a report.
        vec![trouble(ObjectKind::Pod, true)],
        vec![
            trouble(ObjectKind::Pod, true),
            trouble(ObjectKind::Node, false),
        ],
        vec![],
    ] {
        assert_eq!(
            pods_unread(&reported, &k8s::Coverage::Cluster, None),
            None,
            "a run whose pods were read was ended anyway: {:?}",
            reported
                .iter()
                .map(|t| (&t.kind, t.listed))
                .collect::<Vec<_>>()
        );
    }
}

/// **The block a run with no pods ends on: what k8rs asked for, what happened, and what to do —
/// and the scope is in the first of those** (`screens/states.md` § Before the TUI ever starts,
/// `screens/once.md` § Exit codes: *one text, both paths*).
///
/// **The shape is copied and three of its parts are not** (`k8s-admin`, 2026-08-30). That block
/// says *ask for one of the two roles in the README* and there is no README until Phase 13, so
/// `docs/security.md`'s `k8rs-readonly` is what is named. It offers `--namespace <name>` as the
/// way out, which is a door a reader who typed `--namespace` has already walked through, so the
/// action is chosen per scope. And **the scope has to be in the sentence**: measured under
/// `--namespace kube-system`, the namespace appeared nowhere in the whole run, and without it the
/// reader cannot tell whether to ask for a `Role` or a `ClusterRole` — which is the entire
/// content of the request they are about to make.
///
/// **Four scopes and two faults, because the pipeline produces all of them** (NOTES § D29). A
/// `403` cluster-wide is the plain case; a `403` inside a namespace is the ordinary scoped run;
/// `k8s::Coverage::Blind` is the namespace k8rs had to *guess* and was refused in, where telling
/// the reader to pass `--namespace` is the action rather than a spent door; and
/// `k8s::Fault::Unanswered` is the unreachable cluster the deadline now routes here.
#[test]
fn the_block_a_run_with_no_pods_ends_on_names_the_scope_and_a_next_step_that_fits_it() {
    // The two real `watcher::Error`s the classifier reads, built the way § WATCHING A CLUSTER
    // builds them: a `403` off a `Status`, and a transport failure that answered nothing.
    let refused = watcher::Error::InitialListFailed(api_error(403, "Forbidden"));
    let dark = watcher::Error::WatchFailed(kube::Error::Service(Box::new(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "timed out",
    ))));
    fn unread(failure: &watcher::Error) -> Vec<k8s::Trouble<'_>> {
        vec![k8s::Trouble {
            kind: ObjectKind::Pod,
            listed: false,
            ended: false,
            failure: Some(failure),
            unfinished: false,
            outstanding: None,
        }]
    }

    let wide = pods_unread(&unread(&refused), &k8s::Coverage::Cluster, None)
        .expect("a pod watch that never listed ends the run");
    println!("{wide}");
    assert!(
        wide.contains("What k8rs asked for: pods across the whole cluster"),
        "the block does not say where k8rs looked, so the reader cannot tell whether to ask for \
         a Role or a ClusterRole: {wide:?}"
    );
    assert!(
        wide.contains(
            "What happened: the role this kubeconfig uses needs to `list` and `watch` \
                       pods"
        ),
        "{wide:?}"
    );
    assert!(
        wide.contains("`k8rs-readonly` in the k8rs docs") && wide.contains("--namespace <name>"),
        "a cluster-wide refusal is offered neither the role nor the narrower run: {wide:?}"
    );
    // **The README is not cited**, because there is not one until Phase 13.
    assert!(!wide.contains("README"), "{wide:?}");

    let scoped = pods_unread(
        &unread(&refused),
        &k8s::Coverage::Asked("kube-system".to_string()),
        None,
    )
    .expect("a pod watch that never listed ends the run");
    println!("{scoped}");
    assert!(
        scoped.contains("What k8rs asked for: pods in the namespace kube-system"),
        "the namespace that was refused is named nowhere, which is what the reader has to put in \
         the request: {scoped:?}"
    );
    assert!(
        scoped.contains("read pods in kube-system"),
        "the action names no namespace either: {scoped:?}"
    );
    // **The spent door.** A reader who typed `--namespace` is not told to type it.
    assert!(
        !scoped.contains("--namespace <name>"),
        "a reader who already scoped the run was offered scoping it: {scoped:?}"
    );

    // **`Blind` is the arm where that door is *not* spent** — the namespace was k8rs's guess.
    let blind = pods_unread(
        &unread(&refused),
        &k8s::Coverage::Blind("default".to_string()),
        None,
    )
    .expect("a pod watch that never listed ends the run");
    println!("{blind}");
    assert!(
        blind.contains("k8rs had to guess default") && blind.contains("--namespace <name>"),
        "the one scope the reader did not choose was told to choose a different one: {blind:?}"
    );

    // **Nothing answered is not a permission problem and is not given a role to ask for.**
    let unreachable = pods_unread(&unread(&dark), &k8s::Coverage::Cluster, None)
        .expect("a pod watch that never listed ends the run");
    println!("{unreachable}");
    assert!(
        unreachable.contains("What happened: nothing usable came back"),
        "{unreachable:?}"
    );
    assert!(
        unreachable.contains("Check the server address"),
        "an unreachable cluster is given no address to check: {unreachable:?}"
    );
    assert!(
        !unreachable.contains("k8rs-readonly"),
        "a cluster nobody could reach was blamed on RBAC: {unreachable:?}"
    );
}

/// **`--once` cannot run without a cluster either, and it says the same sentence `--live` does.**
///
/// **One text for both modes** (`screens/once.md` § Exit codes: *failures print the same
/// plain-language stderr messages the TUI prints before it ever enters raw mode — one text, both
/// paths*). The assertion is `assert_eq!` against the `--live` answer rather than a substring,
/// because a second sentence for the same fault is exactly what would go unnoticed.
///
/// **Three startup failures, not one**, and they are the three a kubeconfig can produce before
/// anything is sent: a context that is not in the file, an entry pointing at a certificate that
/// is not on disk, and a login program that is not installed. Each is a different `k8s::Fault`,
/// so a mode that swallowed one would still pass on the others.
#[tokio::test]
async fn a_once_run_that_could_not_start_returns_the_same_sentence_live_returns() {
    let yaml = |user: &str| {
        kube::config::Kubeconfig::from_yaml(&format!(
            "apiVersion: v1\nkind: Config\n\
             current-context: demo\n\
             clusters: [{{name: demo, cluster: {{server: 'https://k8rs-tests.invalid:6443'}}}}]\n\
             contexts: [{{name: demo, context: {{cluster: demo, user: demo}}}}]\n\
             users: [{{name: demo, user: {user}}}]\n"
        ))
        .expect("a kubeconfig this file wrote itself")
    };
    let helper = "/nonexistent/k8rs-tests-no-such-credential-plugin";
    let plugin = format!(
        "{{exec: {{apiVersion: client.authentication.k8s.io/v1beta1, command: {helper}}}}}"
    );
    let moved = "{client-certificate: /nonexistent/k8rs-tests/client.crt, \
                 client-key: /nonexistent/k8rs-tests/client.key}";

    for (what, context, user) in [
        ("a context that is not in the file", Some("no-such"), "{}"),
        ("an entry that points at nothing", None, moved),
        ("a login program that is not there", None, plugin.as_str()),
    ] {
        let once = live(
            k8s::connect_with(yaml(user), context, None).await,
            false,
            Some(in_a_moment(10_000)),
        )
        .await
        .unwrap_or_else(|| panic!("{what}: --once exited 0 over a cluster it never reached"));
        println!("{once}");
        assert!(
            once.starts_with("k8rs: no cluster to watch — "),
            "{what}: {once:?}"
        );
        assert_eq!(
            Some(once),
            live(
                k8s::connect_with(yaml(user), context, None).await,
                false,
                None
            )
            .await,
            "{what}: --once and --live say two different things about one fault"
        );
    }
}

/// **A cluster nobody can reach is told apart from a slow one, and the deadline is where that
/// used to stop being true** (`k8s-admin`,
/// `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 5, `PRIOR-ART § C1`).
///
/// **Measured before it was written.** Against an endpoint with nothing listening, `--once` spent
/// thirty seconds, wrote nothing to stdout and said *this cluster has not finished answering
/// after 30 seconds … Run it again: counts that have moved mean it is slow* — while
/// `k8s::Store::troubles` held `k8s::Fault::Unanswered` on all five watches and `--live` over the
/// identical endpoint printed the typed line inside the first second. The deadline arm read
/// `k8s::Store::still_listing` and never the troubles, so the one actionable thing k8rs held —
/// *check the address* — was the one thing it did not say.
///
/// **It is not a refusal and that is why the gate never opens.** A `403` *settles* the watch and
/// the gate opens without it (the test above); `Unanswered` is D28's *do not blank on a blip* —
/// the retry may well work, so nothing settles, `k8s::Store::snapshot` answers `None` forever and
/// a `--once` with no deadline would sit there until somebody killed it.
///
/// **The deadline is what is being measured here**, so it is short: 300 ms against a name RFC
/// 6761 reserves so that it can never resolve. The streams are uncut for the reason the test
/// above leaves them uncut — a `take` would end them and test the wrong ending.
///
/// **What it must not do is print a report.** Nothing is on stdout, because a bootstrap that has
/// not landed is a partial list and a partial list reads exactly like a small healthy cluster
/// (NOTES § D28).
#[tokio::test]
async fn a_cluster_that_answers_nothing_names_the_fault_instead_of_calling_it_slow() {
    let session = k8s::session(offline(), k8s::Coverage::Cluster).await;

    let gave_up = live(Ok(session), false, Some(in_a_moment(300)))
        .await
        .expect("a run that never got an answer has nothing to report and must say so");

    println!("{gave_up}");
    assert!(
        gave_up.starts_with(
            "k8rs: this cluster did not show k8rs its pods, and every finding starts there"
        ),
        "a cluster nothing answered for was reported as one that is merely taking a while, and \
         the typed fault k8rs was holding never reached the reader: {gave_up:?}"
    );
    assert!(
        gave_up.contains("What happened: nothing usable came back"),
        "the reason is not the one the store held: {gave_up:?}"
    );
    assert!(
        gave_up.contains("Check the server address"),
        "the reader is left with no address to check, which is the one action this failure has: \
         {gave_up:?}"
    );
    // **Neither of the two sentences it is not.** A refusal is a role to ask for and a slow
    // cluster is a run to repeat; this is a third thing and may borrow the words of neither.
    assert!(
        !gave_up.contains("has not finished answering"),
        "a cluster nobody could reach was called slow: {gave_up:?}"
    );
    assert!(
        !gave_up.contains("k8rs-readonly"),
        "a cluster nobody could reach was blamed on RBAC: {gave_up:?}"
    );
    assert!(
        !gave_up.contains("every watch has stopped"),
        "a run that timed out was reported as a run whose watches ended, and they are two \
         different things to go and look at: {gave_up:?}"
    );
}

/// **A store with one kind still inside its first LIST and the rest of the cluster read** — the
/// shape a wedged watch has, built without a socket so the decision below can be asserted rather
/// than inferred.
///
/// **Empty LISTs, because what is under test is the kind that is *missing*.** An `Init` followed
/// by an `InitDone` is a complete answer of zero objects, which is exactly what `k8s::Watch`
/// treats as listed — and no capture is loaded, so nothing here can pass by accident on a card
/// somebody else's fixture happened to draw.
fn read_everything_but(wedged: ObjectKind) -> k8s::Store {
    use kube::runtime::watcher::Event;
    let mut store = k8s::Store::default();
    // **The wedged kind gets its `Init` and nothing after it**, which is the shape a live wedge
    // has: kube emits `Init` and then hangs inside `api.list()` (`k8s.rs` § THE DRIVER). Leaving
    // the watch untouched instead would be *a stream that has not been polled yet*, which is a
    // different state and not the one under test.
    store.pod(&now(), Event::<Pod>::Init);
    if wedged != ObjectKind::Pod {
        store.pod(&now(), Event::InitDone);
    }
    store.node(&now(), Event::<Node>::Init);
    if wedged != ObjectKind::Node {
        store.node(&now(), Event::InitDone);
    }
    store.deployment(&now(), Event::<Deployment>::Init);
    store.deployment(&now(), Event::InitDone);
    store.stateful_set(&now(), Event::<StatefulSet>::Init);
    store.stateful_set(&now(), Event::InitDone);
    store.daemon_set(&now(), Event::<DaemonSet>::Init);
    store.daemon_set(&now(), Event::InitDone);
    store
}

/// **Which of the three answers a run that ran out of time gets** ([`out_of_time`]) — and the
/// third one is the box (`k8s::Fault::Unfinished`).
///
/// **Pods keep both of their old answers and that is the constraint, not a side effect.** A pod
/// LIST that is merely slow is NOTES § D150's two facts — *8 000 read so far, the last one 2s
/// ago* is how a reader tells a big cluster from a dead one — and it is only readable while that
/// LIST is still counted as running. Settling it to open the gate would publish an empty pod list
/// and throw the counts away, so [`out_of_time`] answers for pods *before* anything is settled.
///
/// **`None` is the one that changed.** Pods landed and some other kind did not: that used to be
/// [`too_slow`] as well — thirty seconds, zero bytes on stdout, exit `2` — while the same store
/// with a `403` on the same kind printed the whole report and exited `0`.
#[test]
fn a_run_that_ran_out_of_time_keeps_the_counts_for_pods_and_publishes_for_everything_else() {
    let budget = std::time::Duration::from_secs(30);

    // Nothing has landed at all: pods are still listing, so the two facts are what there is.
    let nothing = k8s::Store::default();
    let waiting = out_of_time(&nothing, &k8s::Coverage::Cluster, Some(now()), budget, None)
        .expect("a run whose pods never landed has no report in it");
    println!("{waiting}");
    assert!(
        waiting.contains("still reading pods (0 read so far)"),
        "the counts NOTES § D150 hands the reader went missing: {waiting:?}"
    );

    // Pods landed and nodes did not: the case the box is about.
    let wedged = read_everything_but(ObjectKind::Node);
    assert_eq!(
        out_of_time(&wedged, &k8s::Coverage::Cluster, Some(now()), budget, None),
        None,
        "a run that had read every pod in the cluster was ended with nothing on stdout because \
         one other kind had not answered — which is what a `403` on that same kind does not do"
    );

    // And the pod watch is what decides, not *any* watch: the same store with pods held back
    // goes the other way.
    let held = read_everything_but(ObjectKind::Pod);
    assert!(
        out_of_time(&held, &k8s::Coverage::Cluster, Some(now()), budget, None)
            .is_some_and(|said| said.contains("still reading pods")),
        "a run whose pods never landed published a report about a cluster it was never shown"
    );
}

/// **The report a wedged kind used to cost the whole of** — zero bytes and exit `2`, where the
/// same store with a `403` on the same kind printed everything
/// (`reports/2026-08-30-once-flag-against-a-live-cluster.md` § 3 vs § 4c).
///
/// **The two halves are asserted in one test on purpose.** Before [`k8s::Store::stop_waiting`],
/// [`live_report`] over this store answered `None`: `k8s::Store::snapshot` was shut and a wedged
/// kind was not a `k8s::Trouble` either, so there was no card **and** no line. The fix is only a
/// fix if both arrive.
#[test]
fn a_kind_the_run_ran_out_on_reaches_the_report_where_it_used_to_cost_the_whole_of_it() {
    let mut wedged = read_everything_but(ObjectKind::Node);
    let mut last = String::new();
    assert_eq!(
        live_report(
            &wedged,
            now(),
            &mut last,
            false,
            true,
            &AtConnect::default()
        ),
        None,
        "the gate was open before anybody said the waiting was over, so the assertion below is \
         about nothing"
    );

    wedged.stop_waiting();
    let report = live_report(
        &wedged,
        now(),
        &mut last,
        false,
        true,
        &AtConnect::default(),
    )
    .expect("a wedged kind cost the entire report, where a refused one costs two rules");
    println!("{report}");
    assert!(
        report.contains("▲ k8rs never finished reading nodes from this cluster"),
        "the kind the run never read is named nowhere in the report it is missing from: \
         {report:?}"
    );
    // **The numbers, and no cause** (NOTES § D150) — the line's own test one file up covers the
    // three shapes; what is asserted here is that they survive the trip through `live_report`.
    assert!(
        report.contains("0 read so far") && report.contains("this run ran out of time"),
        "the report says the kind is missing and not how far it got: {report:?}"
    );
}

/// **A wedged kind ends a `--once` run the way a refused one does: a report, and exit `0`**
/// (`k8s::Fault::Unfinished`, todo.md § Phase 6).
///
/// **The endpoint is the one the original measurement used** — a nodes URL that accepts the
/// connection and never answers, with the rest of the cluster replying normally. That is what
/// makes `k8s::Store::troubles` empty and `k8s::Store::still_listing` name one kind: there is no
/// error anywhere, which is the whole difficulty.
///
/// **`None` is exit `0` in this driver and there is no other way to reach it** — the report went
/// to stdout, which a test cannot read back (the reason [`out_of_time`] and [`live_report`] are
/// asserted directly above). What this adds is that the whole path runs: connect, five watches,
/// the deadline, the store told to stop waiting, the write.
#[tokio::test]
async fn a_once_run_whose_nodes_never_answered_still_reports_and_exits_zero() {
    let (client, _) =
        emptied_but_slow_on("/api/v1/nodes", std::time::Duration::from_secs(30)).await;

    // **Two seconds and not the 300–500 ms its two neighbours use.** Those end at the deadline
    // with nothing landed; this one has to get four LISTs in and a whole report rendered before
    // the deadline is the thing being measured, and a machine running sixteen mutants in parallel
    // is where a tight budget turns a passing gate into a flaky one.
    let ended = live(
        Ok(k8s::session(client, k8s::Coverage::Cluster).await),
        false,
        Some(in_a_moment(2_000)),
    )
    .await;

    println!("{ended:?}");
    assert_eq!(
        ended, None,
        "a nodes endpoint that accepted the connection and never answered cost the whole report \
         and a non-zero exit, where a `403` on the same watch costs two rules and exits 0 — so \
         `k8rs --once && deploy` still flips on which failure the cluster is in"
    );
}

/// **A LIST that is genuinely just slow keeps the sentence D150 wrote for it** — the negative
/// half of the test above, and what stops *name the typed fault first* from swallowing the case
/// it was not for.
///
/// **Only the pod LIST is held**, which is `reports/2026-08-30…` § 4a exactly: it was accepted
/// and never answered, so no failure is recorded, `k8s::Store::troubles` is empty and there is
/// nothing typed to report. What is left is the two numbers and the one action that separates
/// *slow* from *hung*, which is the split NOTES § D150 refuses to make for the reader.
///
/// **The rest of the cluster answers, and it has to.** A listener that held *everything* would
/// hang inside `k8s::session` before this deadline exists to be tested — the shape
/// [`cluster_run`] bounds and this test does not — so the connection is fast and one URL is slow,
/// which is also the only way `still_listing` names one kind rather than five.
#[tokio::test]
async fn a_list_that_is_only_slow_still_gets_the_two_facts_and_no_verdict() {
    let (client, _) = emptied_but_slow_on("/api/v1/pods", std::time::Duration::from_secs(30)).await;

    let gave_up = live(
        Ok(k8s::session(client, k8s::Coverage::Cluster).await),
        false,
        Some(in_a_moment(500)),
    )
    .await
    .expect("a run whose LISTs never landed has nothing to report and must say so");

    println!("{gave_up}");
    // **The frame, not the number.** The budget here is a fraction of a second so the test is
    // one; that the seconds are the caller's is [`too_slow`]'s own test, over [`ONCE_DEADLINE`].
    assert!(
        gave_up.starts_with("k8rs: this cluster has not finished answering after"),
        "a LIST with no failure behind it was reported as something k8rs had a typed error for: \
         {gave_up:?}"
    );
    assert!(
        gave_up.contains("still reading pods (0 read so far)"),
        "the counts D150 hands the reader are missing: {gave_up:?}"
    );
    // **And `0 read so far` carries no age**: `k8s::Listing::since` is stamped by the `Init` that
    // opens the watch, so there is no *last one* for the clause to be about.
    assert!(
        !gave_up.contains("the last one"),
        "a LIST that has read nothing claimed a last one, which is a screen lying about progress \
         (`k8s::Watch::settled`, invariant 14): {gave_up:?}"
    );
}

/// **`--once --analysis` prints the seven panes under the cards, and `--once` alone does not**
/// (NOTES § D188).
///
/// **It is the only reader three shipped rules have.** N4, N5 and C1's expiring band return
/// `Severity::Info` and nothing else, and [`render`]'s card block filters that band out — so
/// without this flag those three rules run on every live report and reach no screen at all.
///
/// **Asserted over the store a `--once` run actually leaves behind**, and the store it was
/// asserted over until 2026-08-30 was not one (`tester`). That one was driven against
/// [`refusing`], where a real `--once` returns at [`pods_unread`] before [`live_report`] is
/// called at all — so the doc said `--once` and the test proved `--live --analysis`. It is
/// [`emptied`] now, which is the listener the run that reaches the panes actually has under it.
/// The pane text is the assertion because it is the thing the flag decides; the stream it lands
/// on is `tests/binary.rs`'s half.
#[tokio::test]
async fn analysis_under_once_puts_the_panes_under_the_cards_and_without_it_there_are_none() {
    let store = driven(emptied().await).await;

    let panes = live_report(
        &store,
        now(),
        &mut String::new(),
        true,
        // `--once --analysis`, so the lines are that mode's.
        true,
        &AtConnect::default(),
    )
    .expect("a cluster with nothing in it is still a report");
    println!("{panes}");
    let plain = live_report(
        &store,
        now(),
        &mut String::new(),
        false,
        // The store a `--once` run reaches, read the way that run reads it.
        true,
        &AtConnect::default(),
    )
    .expect("a cluster with nothing in it is a report with or without the flag");

    assert_ne!(
        panes, plain,
        "--analysis changed nothing about the report, so the flag is not read on this path"
    );
    for heading in ["[versions]", "[capacity]", "[certificates]"] {
        assert!(
            panes.contains(heading),
            "the {heading} pane is missing under --analysis: {panes}"
        );
        assert!(
            !plain.contains(heading),
            "the {heading} pane was drawn without --analysis, which buries the cards the run \
             exists to show: {plain}"
        );
    }
}

/// **`--once --analysis` waits for what each node is using; `--live` polls for it** (NOTES § D188,
/// `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 4d).
///
/// **Measured before it was written.** With `/apis/metrics.k8s.io` three seconds slower than the
/// pod LIST, `--once --analysis` printed *"What each node is actually using is not shown. That
/// number comes from metrics-server, and k8rs does not read it. Nothing to ask for"* — in the
/// same run whose greeting on stderr said `{Metrics, DisruptionBudgets}`, so k8rs's own discovery
/// had found the API it was telling the reader it does not read. Without the delay the same
/// command printed the `using …` rows. The poll is a sixth stream merged into the watch loop and
/// the loop's stopping point is the *five watches'* gate, which does not cover it; `--live`
/// reprints a moment later and `--once` has no moment later.
///
/// **What is asserted is the wait, because the number itself is on the process's stdout.** The
/// mode that stops must not return before the metrics endpoint has answered — that is exactly the
/// race, stated as a thing a test can see. The negative is the same listener with no `--analysis`
/// at all, which must not wait for a number no pane will draw.
#[tokio::test]
async fn once_waits_for_what_each_node_is_using_and_asks_for_it_exactly_once() {
    use std::sync::atomic::Ordering::SeqCst;
    let held = std::time::Duration::from_millis(400);
    let slow = || emptied_but_slow_on("/apis/metrics.k8s.io", held);

    let (client, asked) = slow().await;
    let started = tokio::time::Instant::now();
    let ending = live(
        Ok(k8s::session(client, k8s::Coverage::Cluster).await),
        true,
        Some(in_a_moment(10_000)),
    )
    .await;
    let waited = started.elapsed();

    assert_eq!(ending, None, "the --analysis run did not reach its report");
    assert!(
        waited >= held,
        "--once --analysis ended in {waited:?}, before the {held:?} metrics answer could land — \
         so Capacity printed `k8rs does not read it` about an API k8rs had already asked for, \
         and this mode has no later pass to correct it"
    );
    // **Once, not twice.** The poll stream is *not* merged under this mode, and the only way that
    // shows from inside the process is the request k8rs did not send: a poll whose first tick is
    // immediate would ask a second time for a number the fetch above already has.
    assert_eq!(
        asked.load(SeqCst),
        1,
        "--once asked metrics-server {} times; the fetch at connect and the poll merged into the \
         watch loop are both running",
        asked.load(SeqCst)
    );

    // **The negative: no `--analysis`, no pane, nothing to ask for.** It is also what makes the
    // wait above a fact about the flag rather than about a listener that is slow at everything —
    // the same connection, built the same way, comes back well inside the delay.
    let (client, unasked) = slow().await;
    let started = tokio::time::Instant::now();
    let ending = live(
        Ok(k8s::session(client, k8s::Coverage::Cluster).await),
        false,
        Some(in_a_moment(10_000)),
    )
    .await;
    let plain = started.elapsed();

    assert_eq!(ending, None, "the plain run did not reach its report");
    assert!(
        plain < held,
        "a --once with no --analysis waited {plain:?} for metrics-server, which draws no pane it \
         could go in"
    );
    assert_eq!(
        unasked.load(SeqCst),
        0,
        "a run that draws no Capacity pane asked metrics-server for the numbers anyway"
    );
}

/// **Which mode asks metrics-server for the numbers on a timer, and which asks once**
/// ([`polls_node_usage`], NOTES § D181, § D188).
///
/// **The requirement and not the expression.** `--live` redraws for as long as it runs, so a
/// metrics-server that is restarting, being installed, or being granted the verb starts showing
/// up without the reader touching anything — that is what the poll is for, and it is the row a
/// deleted `!` would silently take away. `--once` has no later pass, so it reads the same number
/// once at connect instead ([`once_waits_for_what_each_node_is_using_and_asks_for_it_exactly_once`]
/// measures that half against a listener). Neither mode asks at all without the flag, because
/// nothing would draw the answer.
///
/// **It is a function so that a test can reach it**: the poll stream cannot end, so a test that
/// drove `--live --analysis` to a conclusion would be waiting for one that cannot come. The
/// mutation gate is what said so — the condition spelled inline had no assertion behind it.
#[test]
fn only_a_run_that_keeps_redrawing_asks_metrics_server_again() {
    assert!(
        polls_node_usage(true, false),
        "--live --analysis stopped polling, so a metrics-server that comes back, is installed, \
         or is granted the verb never shows up on a screen somebody is watching (NOTES § D181)"
    );
    assert!(
        !polls_node_usage(true, true),
        "--once --analysis merged a poll into a loop it stops before the second tick of, and it \
         already read the number once at connect"
    );
    assert!(
        !polls_node_usage(false, false),
        "--live with no --analysis asks metrics-server every thirty seconds for a pane it does \
         not draw"
    );
    assert!(
        !polls_node_usage(false, true),
        "--once with no --analysis asks for a number nothing prints"
    );
}

/// **[`ONCE_DEADLINE`] bounds the run and not the watch loop inside it** ([`cluster_run`],
/// `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 5).
///
/// **Measured before it was written**: an unroutable endpoint took **140 seconds** and one that
/// accepted TCP and then said nothing was still going at 75, because `k8s::connect` sits ahead of
/// the deadline and kube's `read_timeout` default is `None`. A connection that never finishes is
/// the whole of that shape, so it is what is handed over here — `std::future::pending`, which
/// needs no address and cannot resolve, race or depend on a network.
///
/// **The sentence is [`too_slow`]'s empty arm**, which had no way to be reached until this call
/// existed: there is no store, so no kind to name and no count to compare on a second run.
///
/// **`--live` is asserted to have no such bound**, which is the half that keeps this a `--once`
/// decision: a screen somebody is looking at may wait forever (NOTES § D150), so the same
/// pending connection is still pending when the test stops waiting for it.
#[tokio::test]
async fn a_connection_that_never_finishes_ends_a_once_run_and_leaves_live_waiting() {
    let never = || std::future::pending::<Result<k8s::Session, k8s::NotConnected>>();

    let budget = std::time::Duration::from_millis(200);

    let gave_up = tokio::time::timeout(budget * 10, cluster_run(never(), false, Some(budget)))
        .await
        .expect("--once did not come back inside ten times its own budget, so it bounds nothing")
        .expect("a run that never reached the cluster has nothing to report and must say so");

    println!("{gave_up}");
    assert!(
        gave_up.starts_with("k8rs: this cluster has not finished answering after"),
        "{gave_up:?}"
    );
    assert!(
        gave_up.contains("check the server address this kubeconfig names"),
        "a run that never connected is left with nothing to do: {gave_up:?}"
    );
    assert!(
        !gave_up.contains("still reading"),
        "a run with no store named a kind it was reading: {gave_up:?}"
    );

    assert!(
        tokio::time::timeout(budget * 10, cluster_run(never(), false, None))
            .await
            .is_err(),
        "--live gave up on a connection, and a screen somebody is watching may wait (D150)"
    );
}

/// **What a run that ran out of time says: the two facts, and no verdict** (NOTES § D150).
///
/// **`k8s.rs` refuses to pick a threshold between *slow* and *hung*** — any number that called
/// the twentieth round trip of a 10 000-pod cluster a hang would call a working cluster broken —
/// so this sentence may not contain one either. What it carries is how much each unfinished LIST
/// has decoded and when the last of it arrived, which is what moves for a slow cluster and does
/// not for a dead one, plus the one action that tells them apart.
///
/// **The number of seconds is the caller's**, so the sentence cannot drift from the deadline that
/// produced it.
///
/// **Four shapes, because the pipeline produces four** (NOTES § D29): a LIST with a count and a
/// stamp, a LIST with a stamp and nothing read, a machine whose clock would not read, and a list
/// with nothing in it at all — which is [`cluster_run`]'s deadline, not [`live`]'s.
///
/// **`0 read so far, the last one 30s ago` was the ordinary reading and it was a lie**
/// (`k8s-admin` and `tester`, independently, 2026-08-30). `k8s::Listing::since` is stamped by the
/// `Init` that *opens* the watch, so a whole first round trip reports `0` beside a moving age —
/// measured on an unreachable cluster as `12s, 12s, 10s, 12s, 10s`, five numbers walking forward
/// while nothing arrived, pointing D150's *counts that have moved mean it is slow* separator at
/// the wrong answer. *One* has nothing to bind to when nothing came (invariant 14), so the clause
/// is gone in that shape and kept in the other.
#[test]
fn the_sentence_a_run_out_of_time_prints_carries_the_two_facts_and_no_verdict() {
    let waited = ONCE_DEADLINE;
    let listing = |kind: ObjectKind, so_far: usize, since: Option<Time>| k8s::Listing {
        kind,
        so_far,
        since,
    };

    let ordinary = too_slow(
        &[
            listing(ObjectKind::Pod, 4500, Some(four_minutes_ago())),
            listing(ObjectKind::DaemonSet, 0, Some(now())),
        ],
        Some(now()),
        waited,
    );
    println!("{ordinary}");
    assert!(
        ordinary.starts_with("k8rs: this cluster has not finished answering after 30 seconds"),
        "the sentence does not say how long it waited, so the reader cannot tell a slow run from \
         a wrong flag: {ordinary:?}"
    );
    assert!(
        ordinary.contains("still reading pods (4500 read so far, the last one 4 min ago)"),
        "the count and the age are what separate slow from hung, and one of them is missing: \
         {ordinary:?}"
    );
    // **A stamped LIST that has read nothing keeps the count and loses the age.** The `Init` that
    // opened the watch is not a *last one*.
    assert!(
        ordinary.contains("DaemonSets (0 read so far)"),
        "only the first unfinished LIST was named: {ordinary:?}"
    );
    assert!(
        !ordinary.contains("DaemonSets (0 read so far, the last one"),
        "a LIST that has read nothing claimed a last one, which is the age that moves while \
         nothing arrives: {ordinary:?}"
    );
    assert!(
        ordinary.contains("Run it again"),
        "the sentence gives the reader nothing to do: {ordinary:?}"
    );
    // **No verdict.** The words `k8s.rs` declined to say about a cluster it cannot measure.
    for verdict in ["hung", "broken", "dead", "too slow", "timed out"] {
        assert!(
            !ordinary.contains(verdict),
            "the sentence calls the cluster {verdict}, which is the threshold k8s.rs refuses to \
             invent: {ordinary:?}"
        );
    }

    // **A clock that would not read costs the age and keeps the counts** — no reading is printed
    // as no reading, never as a guess.
    let clockless = too_slow(
        &[listing(ObjectKind::Pod, 4500, Some(four_minutes_ago()))],
        None,
        waited,
    );
    println!("{clockless}");
    assert!(
        clockless.contains("still reading pods (4500 read so far)"),
        "{clockless:?}"
    );
    assert!(
        !clockless.contains("ago"),
        "an age was printed for a machine that could not read its own clock: {clockless:?}"
    );

    // **A LIST with no stamp at all** — `Listing::since` is `None` before the loop's first poll.
    let unstamped = too_slow(&[listing(ObjectKind::Node, 0, None)], Some(now()), waited);
    println!("{unstamped}");
    assert!(
        unstamped.contains("still reading nodes (0 read so far)"),
        "{unstamped:?}"
    );

    // **Nothing still listing is [`cluster_run`]'s deadline** — the connection itself ran out of
    // budget, so there is no store, no kind and no count. *Run it again and see whether the
    // counts moved* is advice about numbers that were never printed, so that arm says the one
    // thing that is both true and actionable instead.
    let empty = too_slow(&[], Some(now()), waited);
    println!("{empty}");
    assert!(
        !empty.contains("still reading"),
        "the sentence claims to be reading something and names nothing: {empty:?}"
    );
    assert!(
        !empty.contains("counts that have moved"),
        "a run that printed no counts told the reader to compare them: {empty:?}"
    );
    assert!(
        empty.contains("check the server address this kubeconfig names"),
        "a run that got nothing at all is left with nothing to do: {empty:?}"
    );
}

// --- ONE OBJECT'S LOG ---
//
// **The command line is where most of this box's decisions are, so most of this region is about
// argv** (NOTES § D194): which object, which container, and the two switches. The rest is the
// four decisions [`logs_run`] makes over a pod it has already fetched — which container, whether
// there is a previous run, what a followed stream ended for, and what a fetched log looks like on
// stdout — each a function over values for the reason `main`'s own doc gives.
//
// **What is *not* here is the printing**, and it cannot be: "stdout belongs to the process and a
// test cannot read it back" (§ WATCHING A CLUSTER). [`dump`] is written against a `Write` so that
// the one part with a shape can be asserted; the rest is `tests/binary.rs`'s.

/// A server that answers the requests a run about one object makes — the pod, its log, and its
/// events — with whatever the caller says.
///
/// **A second stub and not the one `k8s_tests.rs` has**, for [`offline`]'s reason: invariant 11
/// keeps each `mod tests` private to its own product file, so a helper cannot cross. It is
/// deliberately the smaller thing — one status, two bodies, no request log — because what is under
/// test here is [`logs_run`]'s answer and not the wire, which is proven one file down.
async fn answers(status: &'static str, pod: String, log: &'static str) -> (kube::Client, Requests) {
    answers_that_may_cut_the_log(status, pod, log, false).await
}

/// [`answers`], with the **log** path answering on a status of its own — a pod that reads
/// perfectly and a log request the server will not accept.
///
/// **The only shape a `400` on `pods/log` actually arrives in, and one status for the whole
/// server cannot spell it**: the pod has to be readable or [`logs_run`] ends before it asks for a
/// log at all, and the sentence under test is the one printed after it asks. Measured on a live
/// kind cluster before it was a test — `default/broken-config`, whose container is waiting on a
/// ConfigMap that does not exist (`k8s-admin`, 2026-09-03).
async fn answers_but_refuses_the_log(
    pod: String,
    refusal: (&'static str, String),
) -> (kube::Client, Requests) {
    served(
        "200 OK",
        pod,
        refusal,
        Some(("200 OK", no_event_list())),
        false,
    )
    .await
}

/// [`answers`], with the whole of the events answer the caller's — **its status as well as its
/// body, and `None` for a server that never answers that path at all**.
///
/// **The status has to travel with the body, and one status for the whole server is the defect
/// this replaces.** `screens/detail.md` puts the entire difference between *this pod has no
/// events* and *k8rs could not find out* in the exit code, and with a single status no test could
/// spell *the pod read was served and the events read was refused* — so all three of
/// [`describe_run`]'s endings were unreachable and the claim could have rotted in silence
/// (`tester`, 2026-08-31). `k8s_tests.rs`'s own `stub` made the same change on 2026-08-29 for the
/// same reason, and its doc is where that is argued.
///
/// **`None` is *accepted and never answered*, which is not the same as refused or closed.** A
/// dropped socket is a connection error; a held one is a cluster that took the request and said
/// nothing, which is the only thing that reaches [`describe_run`]'s timeout arm.
async fn answers_with_events(
    pod: String,
    events: Option<(&'static str, String)>,
) -> (kube::Client, Requests) {
    served("200 OK", pod, ("200 OK", String::new()), events, false).await
}

/// The empty `EventList` [`answers`] hands back when the caller did not choose one.
fn no_event_list() -> String {
    serde_json::json!({
        "apiVersion": "v1", "kind": "EventList",
        "metadata": { "resourceVersion": "1" }, "items": [],
    })
    .to_string()
}

/// [`answers`], and a log body that stops before the `content-length` it declared — a cluster
/// whose connection was severed part way through streaming one.
///
/// **A wrapper rather than a fourth argument at five call sites**, and the same server
/// underneath: what varies is one header, and two copies of forty lines of HTTP is forty lines
/// that can come apart (CLAUDE.md § Code phase rules).
async fn answers_that_may_cut_the_log(
    status: &'static str,
    pod: String,
    log: &'static str,
    cut: bool,
) -> (kube::Client, Requests) {
    served(
        status,
        pod,
        (status, log.to_string()),
        Some((status, no_event_list())),
        cut,
    )
    .await
}

/// The one server the three wrappers above are, and the only place this file writes HTTP.
///
/// **`events` carries its own status because the events path is the one that has to disagree with
/// the others** ([`answers_with_events`]), and `None` there is a request that is read, logged and
/// then never answered.
///
/// **`log` carries its own for the same reason** ([`answers_but_refuses_the_log`]): a `400` on
/// `pods/log` only exists after a pod read that succeeded, so a single status for the whole server
/// could not spell it and the arm that prints it was unreachable.
async fn served(
    status: &'static str,
    pod: String,
    log: (&'static str, String),
    events: Option<(&'static str, String)>,
    cut: bool,
) -> (kube::Client, Requests) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port");
    let address = listener.local_addr().expect("the port it picked");
    let asked: Requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log_of = std::sync::Arc::clone(&asked);
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let pod = pod.clone();
            let log = log.clone();
            let events = events.clone();
            let log_of = std::sync::Arc::clone(&log_of);
            tokio::spawn(async move {
                let mut pending = String::new();
                loop {
                    let mut chunk = [0_u8; 2048];
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => pending.push_str(&String::from_utf8_lossy(&chunk[..read])),
                    }
                    while let Some(end) = pending.find("\r\n\r\n") {
                        let request: String = pending.drain(..end + 4).collect();
                        log_of
                            .lock()
                            .expect("the log is never poisoned")
                            .push(request.split_whitespace().nth(1).unwrap_or("/").to_string());
                        let log_asked_for = request.contains("/log?");
                        let (status, body) = if log_asked_for {
                            (log.0, log.1.clone())
                        } else if request.contains("/events?") {
                            match &events {
                                Some((status, body)) => (*status, body.clone()),
                                // **Held open and never written to.** Returning would drop the
                                // socket, which is a connection error and not a wait; parking the
                                // task keeps the connection alive with no answer on it, which is
                                // the shape a deadline exists for.
                                None => std::future::pending().await,
                            }
                        } else {
                            (status, pod.clone())
                        };
                        // A body that stops before its declared length is what a severed
                        // connection looks like to the client, and it is the only way this
                        // server can produce one.
                        let declared = match cut && log_asked_for {
                            true => body.len() + 50,
                            false => body.len(),
                        };
                        let sent = format!(
                            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\n\
                             content-length: {declared}\r\n\r\n{body}"
                        );
                        if socket.write_all(sent.as_bytes()).await.is_err() {
                            return;
                        }
                        if cut && log_asked_for {
                            return;
                        }
                    }
                }
            });
        }
    });
    let client = kube::Client::try_from(kube::config::Config::new(
        format!("http://{address}")
            .parse()
            .expect("an address the kernel just gave us"),
    ))
    .expect("a client over plain http asks the machine for nothing");
    (client, asked)
}

/// Every path [`answers`] was asked for, in order.
type Requests = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

/// A session over a client of the caller's choosing, with a namespace the context names.
fn session_over(client: kube::Client, namespace: Option<&str>) -> k8s::Session {
    k8s::Session {
        client,
        namespace: namespace.map(str::to_string),
        ..saying(
            Ok("v1.36.1".to_string()),
            Err(api_error(403, "Forbidden")),
            None,
        )
    }
}

/// The command line, as `main` sees it.
fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

/// **One selector for four consumers, in both spellings, first wins** (NOTES § D194,
/// [`value_of`]).
///
/// **Both spellings, for `--context`'s reason**: matching only `--object NAME` lets
/// `--object=NAME` fall through, and a selector that silently selects nothing is worse than one
/// that refuses. **First wins is written down** because an unwritten tie-break is the one that
/// changes by accident.
#[test]
fn the_object_selector_reads_both_spellings_and_takes_the_first() {
    assert_eq!(
        object_arg(&argv(&["--logs", "--object", "payments/web"])),
        Some(Some("payments/web"))
    );
    assert_eq!(
        object_arg(&argv(&["--logs", "--object=payments/web"])),
        Some(Some("payments/web"))
    );
    assert_eq!(
        object_arg(&argv(&[
            "--logs", "--object", "first", "--object", "second"
        ])),
        Some(Some("first")),
        "a repeated selector is not first-wins, so the tie-break is whatever the loop happens to do"
    );
    assert_eq!(
        object_arg(&argv(&["--logs", "--object"])),
        Some(None),
        "the flag with nothing after it has to be its own answer, or a refusal cannot tell it \
         from the flag being absent"
    );
    assert_eq!(object_arg(&argv(&["--once"])), None);
    assert_eq!(
        container_arg(&argv(&["--container=app"])),
        Some(Some("app")),
        "the container flag does not read the spelling every flag beside it reads"
    );
}

/// **`--namespace` still reads exactly as it did**, now that it and the two new flags are one
/// parser ([`value_of`]) — the shapes `tests/binary.rs` refuses, read here as values.
#[test]
fn the_namespace_flag_reads_the_same_four_ways_it_always_did() {
    assert_eq!(
        namespace_arg(&argv(&["--live", "--namespace", "payments"])),
        Some(Some("payments"))
    );
    assert_eq!(
        namespace_arg(&argv(&["--live", "--namespace=payments"])),
        Some(Some("payments"))
    );
    assert_eq!(
        namespace_arg(&argv(&["--live", "-n", "payments"])),
        Some(Some("payments"))
    );
    assert_eq!(
        namespace_arg(&argv(&["--live", "-n=payments"])),
        Some(Some("payments"))
    );
    assert_eq!(namespace_arg(&argv(&["--live", "-n"])), Some(None));
    assert_eq!(namespace_arg(&argv(&["--live"])), None);
}

/// **An object is split on its *first* slash**, so a name with a slash in it stays a name with a
/// slash in it and is refused rather than quietly read as a shorter one (the security gate's
/// *names build paths* row).
#[test]
fn an_object_is_split_on_the_first_slash_and_not_the_last() {
    assert_eq!(
        split_object("payments/web-7d9f4"),
        (Some("payments"), "web-7d9f4")
    );
    assert_eq!(split_object("web-7d9f4"), (None, "web-7d9f4"));
    assert_eq!(
        split_object("payments/web/oops"),
        (Some("payments"), "web/oops"),
        "splitting on the last slash hands `oops` to the name check and reads a pod nobody named"
    );
}

/// **The namespace on the selector beats `--namespace`**, because it is the more specific of the
/// two things the reader typed — and everything else on the line reaches [`Asked`] as written.
#[test]
fn a_log_run_reads_its_object_its_container_and_its_two_switches() {
    let args = argv(&[
        "--logs",
        "--object",
        "payments/web-7d9f4",
        "--container",
        "app",
        "--previous",
        "--follow",
        "--namespace",
        "elsewhere",
    ]);
    let whole = asked(&args).expect("a line with --logs and --object is a log run");

    assert_eq!(whole.namespace, Some("payments"));
    assert_eq!(whole.name, "web-7d9f4");
    assert_eq!(whole.container, Some("app"));
    assert!(whole.previous);
    assert!(whole.follow);

    let line = argv(&["--logs", "--object", "web-7d9f4", "--namespace", "payments"]);
    let bare = asked(&line).expect("a log run");
    assert_eq!(
        bare.namespace,
        Some("payments"),
        "a selector with no namespace in it did not fall through to --namespace"
    );
    assert_eq!(bare.container, None);
    assert!(!bare.previous);
    assert!(!bare.follow);

    assert!(
        asked(&argv(&["--once"])).is_none(),
        "a run with no --logs on it was read as a log run"
    );
}

/// **A log run is a cluster run**, so `--context` reaches it and a path beside it is refused —
/// both of which [`live_context`] answering `Some` is what buys.
#[test]
fn a_log_run_is_a_cluster_run_and_reads_the_context_flag() {
    assert_eq!(
        live_context(&argv(&["--logs", "--object", "web", "--context", "prod"])),
        Some(Some("prod")),
        "--logs did not reach the context flag, so the reconnect-proof machine cannot point it \
         anywhere but its current context"
    );
    assert_eq!(
        live_context(&argv(&["--logs", "--object", "web"])),
        Some(None),
        "a log run was read as a file run, so it would try to open `--object` as a path"
    );
    assert!(
        mistyped(&argv(&["--logs", "--object", "web", "pod.json"])).is_some(),
        "a file beside a log run was silently ignored"
    );
}

/// **Every shape of the two new flags that names nothing usable is refused before anything
/// connects** — the same rule `--context` and `--namespace` are already under.
///
/// **The value checks matter more here than anywhere else on this line.** `--object`'s value is
/// *two* words that end up inside a request path, and it is the only place in this build where a
/// name comes from argv rather than from an API server that already bounded it.
#[test]
fn a_log_run_that_names_nothing_usable_is_refused() {
    for line in [
        vec!["--logs"],
        vec!["--logs", "--object"],
        vec!["--logs", "--object="],
        vec!["--logs", "--object", ""],
        vec!["--logs", "--object", "--follow"],
        vec!["--logs", "--object", "../secrets"],
        vec!["--logs", "--object", "payments/web/oops"],
        vec!["--logs", "--object", "PAYMENTS/web"],
        vec!["--logs", "--object", "payments/web?watch=true"],
        vec!["--logs", "--object", "web", "--container"],
        vec!["--logs", "--object", "web", "--container", ""],
        vec!["--logs", "--object", "web", "--container", "a/b"],
        vec!["--object", "payments/web"],
    ] {
        let refused = mistyped(&argv(&line))
            .unwrap_or_else(|| panic!("{line:?} was accepted, so k8rs went and asked a cluster"));
        assert!(
            refused.starts_with("k8rs: ") && refused.contains("usage: k8rs "),
            "{line:?} was refused without the usage under it: {refused:?}"
        );
    }
    assert_eq!(
        mistyped(&argv(&[
            "--logs",
            "--object",
            "payments/web-7d9f4",
            "--container",
            "app",
            "--previous",
            "--follow",
        ])),
        None,
        "an ordinary log run was refused"
    );
    // **A mistyped flag is the more specific complaint about the same line**, so it wins over
    // *`--logs` and `--object` go together* — which would otherwise tell a reader who typed
    // `--lgos` to add the flag they had just tried to type.
    let typo = mistyped(&argv(&["--lgos", "--object", "default/web"]))
        .expect("a flag k8rs does not have is refused");
    assert!(
        typo.starts_with("k8rs: --lgos is not a flag k8rs has"),
        "{typo:?}"
    );
}

/// **The two halves of a selector are two rules, and they get two sentences** — a namespace is a
/// DNS-1123 *label* and a pod name is a *subdomain*.
///
/// **Found by running the binary, not by a test.** `--object PAYMENTS/web` came back *"a name is
/// letters, digits, dashes and dots, up to 253 characters"*, which is true of nothing that is
/// wrong with `PAYMENTS`: what is wrong is that a namespace may not be uppercase, and the reader
/// was sent to check the wrong half (NOTES § D190's class, `dev-core` 2026-08-30).
#[test]
fn a_selectors_namespace_and_its_name_are_refused_for_their_own_reasons() {
    let namespace = mistyped(&argv(&["--logs", "--object", "PAYMENTS/web"]))
        .expect("an uppercase namespace is not a namespace");
    assert!(
        namespace.starts_with("k8rs: the namespace in --object needs the name of a namespace")
            && namespace.contains("lowercase"),
        "the left half was refused with the rule for the right half: {namespace:?}"
    );

    let name = mistyped(&argv(&["--logs", "--object", "payments/web oops"]))
        .expect("a space is not in a name");
    assert!(
        name.starts_with("k8rs: --object names one pod"),
        "the right half was refused with the rule for the left half: {name:?}"
    );

    // The namespace sentence is `--namespace`'s own, so the two cannot drift apart.
    let flag = mistyped(&argv(&["--live", "--namespace", "PAYMENTS"]))
        .expect("an uppercase namespace is not a namespace");
    assert_eq!(
        flag.replace("--namespace needs", "the namespace in --object needs"),
        namespace.replace("PAYMENTS/web", "PAYMENTS"),
        "the two places a namespace is refused say two different things about one rule"
    );
}

/// **A file beside `--logs` is refused, and the sentence names `--logs`** — not the two flags the
/// run did not have.
#[test]
fn a_file_beside_a_log_run_is_refused_and_the_sentence_names_the_flag_that_is_there() {
    let said = mistyped(&argv(&["--logs", "--object", "default/web", "pod.json"]))
        .expect("a file beside a cluster flag is refused");
    assert!(
        said.starts_with("k8rs: --logs reads a cluster, so k8rs cannot also read pod.json"),
        "the refusal named a flag this run does not have: {said:?}"
    );
}

/// **A name refused for being enormous is not echoed back at that size** — the security gate's
/// *sizes are bounded* row, which the `--namespace` refusal beside this one already obeys.
#[test]
fn a_refused_object_is_not_echoed_back_whole() {
    let refused = mistyped(&argv(&["--logs", "--object", &"a".repeat(9000)]))
        .expect("a name that long is not a name any cluster has");
    let first = refused
        .lines()
        .next()
        .expect("the refusal has a first line");
    assert!(
        first.chars().count() < 500,
        "the refusal echoed {} characters of a value it refused for being too long",
        first.chars().count()
    );
}

/// **One pod, fetched the way [`logs_run`] fetches it** — through [`k8s::pod`] and off a socket,
/// so what these tests are handed is the shape the pipeline produces and not one assembled here
/// (NOTES § D29, which is the rule the `stream_ended` test below broke).
///
/// **It has to go through the wire because `k8s::PodRead` has no other door**: the `spec` order
/// and the default-container annotation are read inside `k8s.rs` and nothing outside it may hold
/// a `Pod` (invariant 6). That is a property worth paying a socket for.
async fn pod_read(capture: &str) -> k8s::PodRead {
    let (client, _) = answers("200 OK", pod_body(capture), "").await;
    k8s::pod(&client, "default", "web")
        .await
        .map_err(|failure| k8s::fault(&failure))
        .expect("the stub answered the get")
}

/// **The container the log is read from, and the sentence when the reader named one that is not
/// there** (`screens/detail.md` § Choosing a container).
///
/// **`gang.json` and not `healthy-sidecar`**, because it is the capture whose two orders
/// disagree: `spec [trigger, bystander]` against `status [bystander, trigger]`. Choosing off the
/// snapshot opened `bystander` where `kubectl logs` opens `trigger` (`k8s-admin`, 2026-08-30).
#[tokio::test]
async fn the_container_is_the_one_named_the_first_declared_one_or_a_sentence() {
    let pod = pod_read("gang").await;

    assert_eq!(
        which_container(&pod, Some("bystander")).expect("the pod has a container by that name"),
        Some("bystander"),
        "a container the reader named by hand was not the one chosen"
    );
    assert_eq!(
        which_container(&pod, None).expect("the pod declares two containers"),
        Some("trigger"),
        "the default is the container that happens to sort first, so `--logs` on `[web, envoy]` \
         reads the proxy"
    );
    assert_eq!(
        which_container(&pod, Some("typo")).expect_err("no such container"),
        "k8rs: this pod has no container named typo — it has trigger, bystander",
        "a misspelled container was refused without the list the reader has to retype from, or \
         the list is not in the order the pod declares them in"
    );

    // **A `Pending` pod is not the no-container case any more, and that was the whole of B2.**
    // It declares its container in `spec` and the request names it, so the API server is never
    // asked to guess and never answers `400`.
    let pending = pod_read("pending").await;
    assert!(
        pending.snapshot.containers.is_empty(),
        "`pending.json` no longer has an empty `containerStatuses`, so this proves nothing"
    );
    assert_eq!(
        which_container(&pending, None).expect("a pending pod is a state and not an error"),
        Some("app"),
        "a pod the kubelet has reported no container for named none, so a multi-container one in \
         the same state is refused by the API server and the reader is told the network failed"
    );
    assert!(
        which_container(&pending, Some("nope"))
            .expect_err("there is no container by that name")
            .contains("it has app"),
        "a misspelled container on a pending pod was refused without the list, which the pod's \
         own `spec` has"
    );
}

/// **A `Pod` that declares no container at all is answered without an empty list** — the arm no
/// cluster can reach, fed because a sentence nobody has read is a sentence that reads wrongly.
///
/// **The body is hand-built and could not be a capture.** The API server refuses a pod that
/// declares no container (NOTES § D156, ruling 1), so no cluster serves this shape; what makes
/// the arm exist at all is that `k8s_openapi` types `spec` as an `Option`.
#[tokio::test]
async fn a_pod_that_declares_no_container_is_refused_without_an_empty_list() {
    let (client, _) = answers(
        "200 OK",
        r#"{"apiVersion":"v1","kind":"Pod","metadata":{"name":"web","namespace":"default"}}"#
            .to_string(),
        "",
    )
    .await;
    let read = k8s::pod(&client, "default", "web")
        .await
        .map_err(|failure| k8s::fault(&failure))
        .expect("the stub answered the get");

    assert_eq!(read.declared().len(), 0);
    assert_eq!(
        which_container(&read, None).expect("no container is a state and not an error"),
        None,
        "a pod with nothing to name named something"
    );
    assert_eq!(
        which_container(&read, Some("app")).expect_err("there is nothing to name"),
        "k8rs: this pod declares no container at all, so there is no app to read",
        "the refusal printed the list-of-none sentence, so a reader gets `it has ` with nothing \
         after it"
    );
    assert_eq!(
        container_choice(&read, None, None),
        None,
        "a pod with nothing to choose from was offered a picker"
    );
}

/// **The headless picker: what there was to choose from, and what was chosen** — and it is silent
/// in the two cases the screen is silent in.
///
/// **Silent on a single-container pod** is the screen's own invariant one layer up: it does not
/// offer the picker at all, because a key that does nothing is a bug already shipped once here.
/// **Silent when the reader named one**, because they know.
///
/// **`neverrules.json` is the capture that can fail this.** Its `spec` is `[retry, keeper]` and
/// its `status` is `[keeper, retry]`, and the two containers differ in *both* the things drawn
/// beside a name — `retry` is `done` with one restart, `keeper` is `running` with none. A list
/// built from `spec` and a status read by *index* would print each container's row against the
/// other one's name, and every order-blind capture in this repo passes that.
#[tokio::test]
async fn the_container_block_is_drawn_only_when_there_was_something_to_choose() {
    let pod = pod_read("neverrules").await;
    let chosen = which_container(&pod, None).expect("the pod declares two containers");

    let block =
        container_choice(&pod, None, chosen).expect("two containers is something to choose");
    assert_eq!(
        block,
        "k8rs: this pod has 2 containers — retry (done, 1 restart), keeper (running)\n\
         k8rs: reading retry. Name another with `--container <name>`.",
        "the block a reader gets is not what the container list and the choice say — a row drawn \
         against the wrong name is a status read by index"
    );
    assert_eq!(
        container_choice(&pod, Some("retry"), chosen),
        None,
        "the picker was drawn for a reader who had already named a container"
    );

    let alone = pod_read("crashloop").await;
    assert_eq!(alone.declared().len(), 1, "`crashloop.json` declares one");
    assert_eq!(
        container_choice(&alone, None, Some("quitter")),
        None,
        "a pod with one container was offered a choice, which is a key that does nothing"
    );
    assert_eq!(
        container_choice(&pod, None, None),
        None,
        "a pod that declares no container was told which one is being read"
    );
}

/// **A container the kubelet has not reported on is listed with what it is and not with a state
/// it never had** — the picker's list comes from `spec` now, so a `Pending` pod has rows the
/// snapshot has nothing to say about.
#[tokio::test]
async fn a_container_the_kubelet_has_not_reported_on_says_so_rather_than_guessing() {
    let pending = pod_read("pending").await;
    // `pending.json` declares one container, so the picker itself stays silent; `doing` is where
    // the answer lives and it is asserted directly.
    assert_eq!(doing(None), "not started");
    assert_eq!(
        doing(Some(&ContainerState::Waiting {
            reason: None,
            message: None
        })),
        "waiting",
        "a container the kubelet says is waiting was drawn as one it has not reported on"
    );
    assert_eq!(
        container_choice(&pending, None, Some("app")),
        None,
        "a single-container pod was offered a picker"
    );
}

/// **A restart count is shown beside a container that has one**, because that is the signal that
/// makes `--previous` worth typing (`screens/detail.md`) — and both the singular and the plural
/// are drawn from a committed capture rather than from a count written here.
#[tokio::test]
async fn a_container_that_has_restarted_says_so_beside_its_state() {
    let one = pod_read("neverrules").await;
    let block = container_choice(&one, None, Some("retry")).expect("two containers");
    assert!(
        block.contains("retry (done, 1 restart)"),
        "the singular is missing or mis-pluralised, and it is the whole reason a reader reaches \
         for --previous: {block:?}"
    );

    let several = pod_read("gang").await;
    let block = container_choice(&several, None, Some("trigger")).expect("two containers");
    assert_eq!(
        block,
        "k8rs: this pod has 2 containers — trigger (running, 3 restarts), bystander (running, 3 \
         restarts)\nk8rs: reading trigger. Name another with `--container <name>`.",
        "the plural is missing or mis-pluralised: {block:?}"
    );
}

/// **What a container is doing, in a word a beginner reads** (invariant 14) — never the API's own
/// `reason`, which is the jargon this product exists to translate.
#[test]
fn a_containers_state_is_a_word_and_never_the_reason_code() {
    assert_eq!(
        doing(Some(&ContainerState::Running { started_at: None })),
        "running"
    );
    assert_eq!(
        doing(Some(&ContainerState::Waiting {
            reason: Some("CrashLoopBackOff".to_string()),
            message: None,
        })),
        "waiting",
        "a waiting container printed the API's own reason code where the screen says one word"
    );
    // **The third arm, fed** — `screens/detail.md` draws a finished init container as `done`, and
    // without this the word could be anything.
    assert_eq!(
        doing(Some(&ContainerState::Terminated(
            crate::rules::Terminated {
                reason: Some("Completed".to_string()),
                exit_code: 0,
                started_at: None,
                finished_at: None,
                message: None,
            }
        ))),
        "done"
    );
    // **The fourth arm is a container the pod declares and the kubelet has not reported on**,
    // which the picker can reach now that its list comes from `spec` (`k8s::PodRead`).
    assert_eq!(
        doing(None),
        "not started",
        "a container nobody has reported on was given a state the kubelet never claimed"
    );
}

/// **`--previous` on a container that never restarted says so and falls back** —
/// `screens/detail.md`'s own words, and k8rs does not print the API's refusal.
///
/// **The count is looked up by name and not by index.** On `neverrules.json` the two lists are in
/// opposite orders and the counts differ, so a lookup by position reads the other container's
/// restarts — and turns `--previous` on where there is nothing to serve, or off where there is.
#[tokio::test]
async fn previous_on_a_container_that_never_restarted_says_so_and_falls_back() {
    let pod = pod_read("neverrules").await;
    let chosen = which_container(&pod, None).expect("the pod declares two containers");
    assert_eq!(chosen, Some("retry"));
    assert_eq!(
        pod.status("retry").map(|container| container.restarts),
        Some(1),
        "`neverrules.json`'s first declared container no longer has a restart, so the negative \
         half of this test proves nothing"
    );
    assert_eq!(
        no_previous_run(&pod, chosen, true),
        None,
        "a container that has restarted was told it has no previous run, which is the one log a \
         crash loop needs"
    );

    assert_eq!(
        pod.status("keeper").map(|container| container.restarts),
        Some(0)
    );
    assert_eq!(
        no_previous_run(&pod, Some("keeper"), true).as_deref(),
        Some(
            "k8rs: keeper hasn't restarted, so there's no previous run to show. Showing the \
             current run instead."
        ),
        "the sentence the screen promises is not the one the driver prints"
    );
    assert_eq!(
        no_previous_run(&pod, Some("keeper"), false),
        None,
        "a run that did not ask for the previous log was told about it anyway"
    );

    // **A container the kubelet has not reported on has not restarted either**, and the fallback
    // is what keeps `--previous` off a `Pending` pod, where the API server has nothing to serve.
    let pending = pod_read("pending").await;
    let unstarted = which_container(&pending, None).expect("a pending pod declares its container");
    assert_eq!(pending.status("app"), None);
    assert!(
        no_previous_run(&pending, unstarted, true).is_some(),
        "`--previous` on a container that has never run was sent to the cluster"
    );
}

/// **Why a followed stream ended is answered only where there is an honest answer**
/// (`PRIOR-ART § E1`, `screens/detail.md`).
///
/// **The deleted pod is the one case the screen asks for by name.** Every other ending — the
/// container stopped writing, a middlebox timed out, the connection broke — is three facts this
/// driver cannot tell apart, and one sentence for all three is E1's own failure wearing the other
/// coat.
///
/// **The shape the pipeline actually produces is a pod that is still there** (NOTES § D29). The
/// stream ends when the container dies and the object outlives it by its grace period, so the
/// re-read *succeeds*: measured twice, deleting four seconds into a follow, the pod still carried
/// a `deletionTimestamp` at t+1 and t+2 at `grace 1s` and at t+1..t+6 at `kubectl`'s default
/// (`k8s-admin`, 2026-08-30). The version of this test before that hand-built a `Fault::Gone`
/// nobody had seen the pipeline produce, and the marker had **never fired** on an ordinary
/// delete — a green gate over an unreachable screen.
///
/// **`stuck.json` is a committed capture of exactly that**: a real pod carrying a real
/// `deletionTimestamp`, read through `k8s::pod` off a socket.
#[tokio::test]
async fn a_stream_says_the_pod_is_gone_and_says_nothing_about_any_other_ending() {
    let deleting = pod_read("stuck").await;
    assert!(
        deleting.snapshot.deletion_timestamp.is_some(),
        "`stuck.json` no longer carries a `deletionTimestamp`, so this test proves nothing"
    );
    assert_eq!(
        stream_ended(Some(Ok(&deleting.snapshot))),
        Some("--- stream ended: pod deleted ---"),
        "the ordinary delete — the object still there inside its grace period — drew no marker, \
         so `screens/detail.md`'s deleted-pod screen is unreachable"
    );

    let alive = pod_read("gang").await;
    assert!(alive.snapshot.deletion_timestamp.is_none());
    assert_eq!(
        stream_ended(Some(Ok(&alive.snapshot))),
        None,
        "a stream that ended while the pod is still there claimed the pod was deleted"
    );

    // The `404` half stays: a re-read late enough that the object has been collected.
    assert_eq!(
        stream_ended(Some(Err(k8s::Fault::Gone))),
        Some("--- stream ended: pod deleted ---"),
        "a pod already collected by the time of the re-read left the reader wondering whether \
         the connection dropped"
    );
    // And a re-read that did not answer inside its own deadline says nothing at all.
    assert_eq!(
        stream_ended(None),
        None,
        "a re-read that timed out was reported as the pod being deleted"
    );
    for other in [
        k8s::Fault::Refused,
        k8s::Fault::Unanswered,
        k8s::Fault::Expired,
        k8s::Fault::Rejected,
    ] {
        assert_eq!(
            stream_ended(Some(Err(other))),
            None,
            "a {other:?} on the re-read was reported as the pod being deleted"
        );
    }
}

/// **A fetched log is the dropped-lines sentence and then the lines**, in that order, because that
/// is literally where the gap is (`screens/detail.md` § When the buffer fills).
///
/// **The sentence is payload and goes to stdout with the lines** — a reader piping this somewhere
/// needs to know it arrived short (`screens/once.md` § stdout and stderr are split on purpose).
#[test]
fn a_fetched_log_prints_what_was_lost_above_what_was_kept() {
    let mut held = k8s::LogLines::default();
    held.push("connected to postgres".to_string());
    held.push("allocating 240MB cache".to_string());

    let mut out = Vec::new();
    dump(&held, &mut out).expect("a Vec never refuses a write");
    assert_eq!(
        String::from_utf8(out).expect("k8rs writes UTF-8"),
        "connected to postgres\nallocating 240MB cache\n",
        "a pane that dropped nothing printed something about dropping"
    );

    let mut lost = k8s::LogLines::default();
    for line in 0..=k8s::LOG_LINES {
        lost.push(format!("line {line}"));
    }
    let mut out = Vec::new();
    dump(&lost, &mut out).expect("a Vec never refuses a write");
    let printed = String::from_utf8(out).expect("k8rs writes UTF-8");
    assert_eq!(
        printed.lines().next(),
        Some("1 line was dropped from the top to keep this pane bounded."),
        "the dropped-lines sentence is missing or is not at the top of the content"
    );
    assert_eq!(
        printed.lines().nth(1),
        Some("line 1"),
        "the sentence is there and the lines under it are not"
    );
}

/// **A kubeconfig that will not connect ends a log run the same way it ends a watch** — one
/// sentence, exit `2`, nothing on stdout.
#[tokio::test]
async fn a_log_run_that_cannot_connect_says_so_and_never_asks_for_a_log() {
    let refused = logs_run(
        std::future::ready(Err(k8s::NotConnected::Kubeconfig(
            kube::config::KubeconfigError::CurrentContextNotSet,
        ))),
        &Asked {
            verb: Verb::Logs,
            namespace: Some("payments"),
            name: "web-7d9f4",
            container: None,
            kind: None,
            previous: false,
            follow: false,
        },
    )
    .await
    .expect("a run that could not connect has no happy ending");

    assert!(
        refused.starts_with("k8rs: no cluster to watch — "),
        "a log run over a kubeconfig that will not load said something else: {refused:?}"
    );
}

/// **The three answers a cluster can give about the pod, and the three different sentences they
/// get** — this is where a log run ends before a single log byte is asked for.
///
/// **A `404` on one named object gets its own sentence and not [`because`]'s.** That function's
/// `Gone` arm is written for a *kind* the server does not serve — *there is no such thing when
/// k8rs tries to …* — which is true and unhelpful about a pod name somebody just typed.
#[tokio::test]
async fn a_pod_that_is_not_there_and_a_pod_that_is_refused_get_different_sentences() {
    let about = |name| Asked {
        verb: Verb::Logs,
        namespace: Some("payments"),
        name,
        kind: None,
        container: None,
        previous: false,
        follow: false,
    };
    let status = |code: u16, reason: &str| {
        serde_json::json!({
            "apiVersion": "v1", "kind": "Status", "status": "Failure",
            "reason": reason, "code": code, "message": "no",
        })
        .to_string()
    };

    let gone = session_over(
        answers("404 Not Found", status(404, "NotFound"), "")
            .await
            .0,
        None,
    );
    assert_eq!(
        logs_run(std::future::ready(Ok(gone)), &about("web-7d9f4")).await,
        Some(
            "k8rs: there is no pod named web-7d9f4 in payments — check the name and the namespace"
                .to_string()
        ),
        "a pod that is not there was reported as something other than a pod that is not there"
    );

    let refused = session_over(
        answers("403 Forbidden", status(403, "Forbidden"), "")
            .await
            .0,
        None,
    );
    let sentence = logs_run(std::future::ready(Ok(refused)), &about("web-7d9f4"))
        .await
        .expect("a refused pod ends the run");
    assert!(
        sentence.contains(
            "the role this kubeconfig uses needs to get the pod web-7d9f4 in \
                           payments"
        ),
        "a refusal did not name the verb and the object the reader has to ask for: {sentence:?}"
    );

    let answering = session_over(
        answers("200 OK", pod_body("healthy-sidecar"), "hello\n")
            .await
            .0,
        None,
    );
    assert_eq!(
        logs_run(std::future::ready(Ok(answering)), &about("healthy-sidecar")).await,
        None,
        "a log that was read and printed did not end the run happily, so `k8rs --logs` exits 2 \
         on a working cluster"
    );
}

/// **The pod reads and the log request is refused: the reader is shown what the server said, not
/// an apology from k8rs.**
///
/// **Measured on a live four-node kind cluster before it was a test** (`k8s-admin`, 2026-09-03).
/// `k8rs --once` cards `default/broken-config` CRITICAL — *Container needs a ConfigMap or Secret
/// that does not exist* — and the obvious next thing anybody does is ask for that pod's log:
///
/// ```text
/// before  k8rs: this cluster would not accept the request k8rs made to get pods/log in default
///               — that is a fault in k8rs, and nothing is wrong with the cluster or with this
///               login
/// after   k8rs: this cluster would not accept the request k8rs made to get pods/log in default,
///               and said: container "app" in pod "broken-config" is waiting to start:
///               CreateContainerConfigError
/// ```
///
/// **The before is false twice over.** Nothing is wrong with k8rs — the container genuinely has
/// not started — and the server had already written the most useful sentence available, naming
/// the same root cause the card names. `PRIOR-ART § C1` in the region written to close it, and
/// `k8s::Fault::Rejected` was the first pass over the same defect: it fixed the category and
/// still dropped the message.
///
/// **Two shapes, both off the same cluster** (NOTES § D29): a container waiting on a ConfigMap
/// that is not there, and one waiting on an image that will not pull. The second was found by
/// feeding the fix rather than by reasoning about it — `default/broken-image` answers *container
/// "nope" in pod "broken-image" is waiting to start: trying and failing to pull image*, which is
/// plain English the reader can act on and which the old sentence also threw away.
///
/// **The two shapes this verb cannot reach are named rather than left implied.** A `--container`
/// naming something the pod does not declare is refused by [`which_container`] before any request
/// goes out, and `--previous` against a container that has never restarted is turned off by
/// [`no_previous_run`] — both measured on the same cluster, and neither reaches a `400`.
#[tokio::test]
async fn a_log_the_server_refuses_prints_what_the_server_said_about_it() {
    let refusal = |message: &str| {
        serde_json::to_string(&serde_json::json!({
            "apiVersion": "v1", "kind": "Status", "status": "Failure",
            "reason": "BadRequest", "code": 400, "message": message,
        }))
        .expect("a json object serialises")
    };
    // The two sentences the live cluster wrote, verbatim.
    for (message, pod) in [
        (
            "container \"app\" in pod \"broken-config\" is waiting to start: \
             CreateContainerConfigError",
            "broken-config",
        ),
        (
            "container \"nope\" in pod \"broken-image\" is waiting to start: trying and failing \
             to pull image",
            "broken-image",
        ),
    ] {
        let (client, _) = answers_but_refuses_the_log(
            pod_body("healthy-sidecar"),
            ("400 Bad Request", refusal(message)),
        )
        .await;
        let sentence = logs_run(
            std::future::ready(Ok(session_over(client, None))),
            &Asked {
                verb: Verb::Logs,
                namespace: Some("default"),
                name: "healthy-sidecar",
                container: Some("app"),
                kind: None,
                previous: false,
                follow: false,
            },
        )
        .await
        .expect("a refused log ends the run");

        assert_eq!(
            sentence,
            format!(
                "k8rs: this cluster would not accept the request k8rs made to get pods/log in \
                 default, and said: {message}"
            ),
            "the one sentence that says why this log cannot be read was replaced by an apology, \
             so the reader is sent to look for a fault in k8rs while {pod} sits waiting"
        );
        assert!(
            !sentence.contains("nothing is wrong with the cluster"),
            "k8rs blamed itself over the top of an explanation the server gave: {sentence:?}"
        );
    }
}

/// **The object read the three verbs share carries the server's sentence too**, so the fix is
/// where they all pass through and not on the one path the finding named ([`read_failed`]).
///
/// **`--logs`, `--describe` and `--yaml` open with the same `get`** (§ ONE OBJECT'S OWN STORY),
/// so a `400` there had the same defect on all three and one row proves all three. **It could not
/// be produced against a live cluster** — every `400` that four-node kind cluster was seen to
/// answer came back on `pods/log`, and nothing there makes a `get pod` malformed — so it is fed
/// here instead, and that is the honest split rather than a claim about a surface nobody
/// exercised.
///
/// **A `404` still gets its own sentence and never [`because`]'s**, which is the negative: that
/// arm returns before the fault is ever spelled, so a message is not printed there and must not
/// start being.
#[tokio::test]
async fn a_read_the_server_refuses_carries_its_sentence_on_every_verb_that_makes_it() {
    let refusal = |code: u16, reason: &str, message: &str| {
        serde_json::to_string(&serde_json::json!({
            "apiVersion": "v1", "kind": "Status", "status": "Failure",
            "reason": reason, "code": code, "message": message,
        }))
        .expect("a json object serialises")
    };
    let about = Asked {
        verb: Verb::Logs,
        namespace: Some("payments"),
        name: "web-7d9f4",
        kind: None,
        container: None,
        previous: false,
        follow: false,
    };

    let (client, _) = answers(
        "400 Bad Request",
        refusal(400, "BadRequest", "the server rejected this request"),
        "",
    )
    .await;
    assert_eq!(
        logs_run(std::future::ready(Ok(session_over(client, None))), &about).await,
        Some(
            "k8rs: this cluster would not accept the request k8rs made to get the pod web-7d9f4 \
             in payments, and said: the server rejected this request"
                .to_string()
        ),
        "the read every one-object verb opens with threw away what the server said, so the \
         defect the log path had is still on `--describe` and `--yaml`"
    );

    let (client, _) = answers(
        "404 Not Found",
        refusal(404, "NotFound", "pods \"web-7d9f4\" not found"),
        "",
    )
    .await;
    assert_eq!(
        logs_run(std::future::ready(Ok(session_over(client, None))), &about).await,
        Some(
            "k8rs: there is no pod named web-7d9f4 in payments — check the name and the namespace"
                .to_string()
        ),
        "a `404` started quoting a server sentence that only repeats the name the reader just \
         typed, in place of the one written for it"
    );
}

/// **A followed stream asks the cluster why it ended and a fetch does not** — which is the whole
/// of `PRIOR-ART § E1`'s *offer resume*, and the only part of it a unit test can see.
///
/// **Counted requests, because the marker itself goes to stdout** and "stdout belongs to the
/// process and a test cannot read it back" (§ WATCHING A CLUSTER). A follow is three requests —
/// the pod, the log, the pod again — and a fetch is two: a fetch ended because the log ended,
/// which is what a fetch is, so asking would be a round trip for an answer nobody needs.
#[tokio::test]
async fn a_followed_stream_asks_why_it_ended_and_a_fetch_does_not() {
    let ran = |follow| async move {
        let (client, asked) = answers("200 OK", pod_body("healthy-sidecar"), "hello\n").await;
        assert_eq!(
            logs_run(
                std::future::ready(Ok(session_over(client, None))),
                &Asked {
                    verb: Verb::Logs,
                    namespace: Some("default"),
                    name: "healthy-sidecar",
                    container: Some("app"),
                    kind: None,
                    previous: false,
                    follow,
                }
            )
            .await,
            None
        );
        asked.lock().expect("the log is never poisoned").clone()
    };

    let followed = ran(true).await;
    assert_eq!(
        followed
            .iter()
            .filter(|path| path.contains("/log?"))
            .count(),
        1,
        "a follow asked for the log more than once: {followed:?}"
    );
    assert!(
        followed.last().is_some_and(|last| !last.contains("/log?")),
        "a followed stream that ended never asked the cluster whether the pod is still there, so \
         a pod deleted mid-follow reads as a dropped connection: {followed:?}"
    );
    assert!(
        followed.iter().any(|path| path.contains("follow=true")),
        "`--follow` did not reach the request: {followed:?}"
    );

    let fetched = ran(false).await;
    assert_eq!(
        fetched.len(),
        followed.len() - 1,
        "a fetch asked the cluster the same number of questions a follow does, so either the \
         re-read is missing from one or it is a round trip the other does not need: \
         {fetched:?} against {followed:?}"
    );
    assert!(
        !fetched.iter().any(|path| path.contains("follow")),
        "a fetch asked to follow: {fetched:?}"
    );
}

/// **A value the echo cannot show as it was judged says so** (invariant 4's *neither record may
/// lie*, invariant 9, NOTES § D31's framing class one layer up).
///
/// **`web` is a perfectly good name**, so a reader shown *"and web is not one"* is sent to fix
/// something that looks correct (`tester`, 2026-08-30). The check runs on the raw value and has
/// to; what may not happen is the sentence quietly naming a different string.
///
/// **Every arm is fed** (NOTES § D29): unchanged, stripped, cut, both, and nothing left.
#[test]
fn a_refused_value_is_echoed_as_what_was_judged_or_says_it_is_not() {
    assert_eq!(
        shown("PAYMENTS", k8s::NAMESPACE_MAX),
        "PAYMENTS",
        "an ordinary value picked up a clause about a change nobody made"
    );
    assert_eq!(
        shown("we\u{202e}b", k8s::NAME_MAX),
        "web (with what cannot print removed)",
        "the reader is sent to fix `web`, which is a name any cluster would accept"
    );
    assert_eq!(
        shown(&"a".repeat(k8s::NAMESPACE_MAX + 1), k8s::NAMESPACE_MAX),
        format!("{} (shortened by k8rs)", "a".repeat(k8s::NAMESPACE_MAX)),
        "a value refused for its length echoed at the legal length, which is a legal name"
    );
    assert_eq!(
        shown(
            &format!("{}\u{7}", "a".repeat(k8s::NAMESPACE_MAX + 1)),
            k8s::NAMESPACE_MAX
        ),
        format!(
            "{} (with what cannot print removed) (shortened by k8rs)",
            "a".repeat(k8s::NAMESPACE_MAX)
        ),
        "a value that was both stripped and cut admitted to only one of the two"
    );
    for nothing in ["", "\u{200b}\u{202e}\u{7}"] {
        assert_eq!(
            shown(nothing, k8s::NAMESPACE_MAX),
            "a value with nothing printable in it",
            "an empty echo leaves `and  is not one` — a doubled space naming nothing"
        );
    }

    // And through the sentence a reader actually gets, so the clause reads as English.
    let said = mistyped(&argv(&["--logs", "--object", "default/we\u{202e}b"]))
        .expect("a bidi override is not in a name");
    println!("{}", said.lines().next().expect("a first line"));
    assert!(
        said.contains("and web (with what cannot print removed) is not one"),
        "{said:?}"
    );
    assert!(
        !said.contains('\u{202e}'),
        "the override reached the terminal: {said:?}"
    );
}

/// **An empty half of `--object` costs the clause rather than printing an empty one**
/// (invariant 14).
///
/// **`--object web/` is a trailing slash off tab completion** and came back *"and  is not one"* —
/// nothing named, and a doubled space where the value would have been (`k8s-admin`, 2026-08-30).
/// `--object /web` is the same defect on the other side and was never reported; both are one
/// check now, because a fix for the reported half only is a second sentence that can drift.
#[test]
fn an_empty_half_of_a_selector_is_named_by_its_position_and_not_echoed() {
    for (line, expected) in [
        (
            "web/",
            "k8rs: --object has nothing after the `/`, so it names no pod — write it as \
             `<namespace>/<name>`",
        ),
        (
            "/web",
            "k8rs: --object has nothing before the `/`, so it names no namespace — write it as \
             `<namespace>/<name>`, or leave the `/` off to use the current namespace",
        ),
        (
            "/",
            "k8rs: --object has nothing before the `/`, so it names no namespace — write it as \
             `<namespace>/<name>`, or leave the `/` off to use the current namespace",
        ),
    ] {
        let said = mistyped(&argv(&["--logs", "--object", line])).expect("an empty half");
        let first = said.lines().next().expect("a first line");
        println!("{first}");
        assert_eq!(first, expected, "`--object {line}` names nothing");
        assert!(said.contains("usage: k8rs "), "{said:?}");
    }

    // The two halves that *are* there still get their own rules, unchanged.
    assert!(
        mistyped(&argv(&["--logs", "--object", "payments/web"])).is_none(),
        "an ordinary selector was refused"
    );
}

/// **A `Pending` pod's log request names the container the pod declares** — B2 end to end, off a
/// socket, because what the API server answers depends on the query string and nothing below the
/// request can see it.
///
/// **A request naming no container is a `400` on any pod with more than one**, and until
/// 2026-08-30 that came back *nothing usable came back when k8rs tried to get pods/log* — a
/// network sentence for a fault on this side of the wire, on the everyday
/// injected-pod-that-cannot-schedule (`k8s-admin`).
#[tokio::test]
async fn a_pending_pods_log_request_names_the_container_the_pod_declares() {
    let (client, asked) = answers("200 OK", pod_body("pending"), "").await;
    let ending = logs_run(
        std::future::ready(Ok(session_over(client, None))),
        &Asked {
            verb: Verb::Logs,
            namespace: Some("default"),
            name: "broken-pending",
            container: None,
            kind: None,
            previous: false,
            follow: false,
        },
    )
    .await;
    assert_eq!(ending, None, "a pending pod's log run failed: {ending:?}");

    let asked = asked.lock().expect("the record is never poisoned").clone();
    let log = asked
        .iter()
        .find(|path| path.contains("/log?"))
        .unwrap_or_else(|| panic!("no log was asked for: {asked:?}"));
    assert!(
        log.contains("container=app"),
        "the request named no container, so a multi-container pod in this state is answered 400 \
         and the reader is told the network failed: {log:?}"
    );
}

/// **A container that has written nothing says so, and one that has written something does not**
/// (`screens/detail.md` § No logs yet, `PRIOR-ART § E1`).
///
/// **A `bool` and not a count**, which is what this function exists to make testable: the two arms
/// of [`logs_run`] used to compare a counter to zero, and every arithmetic mutant of that counter
/// survived the gate because nothing readable depends on the number (`dev-core`, 2026-08-30).
#[test]
fn a_log_with_nothing_in_it_is_a_state_and_a_log_with_something_is_not() {
    assert_eq!(
        nothing_written(false),
        Some("k8rs: nothing has been written to this container's log yet"),
        "a container that has produced nothing was left looking like a hang"
    );
    assert_eq!(
        nothing_written(true),
        None,
        "a log that arrived was reported as empty"
    );
}

/// **A log that stopped arriving is not a log that ended** — `PRIOR-ART § E1`, on the driver's
/// side of it.
///
/// **The exit code is the assertion.** What arrived is on stdout either way; what differs is
/// whether `k8rs --logs … > half.txt && grep -q panic half.txt || echo clean` is told the file is
/// the whole log. Swallowed, this run exits `0` with half a log in it.
#[tokio::test]
async fn a_log_that_stopped_arriving_is_not_reported_as_a_log_that_ended() {
    let (client, _) = answers_that_may_cut_the_log(
        "200 OK",
        pod_body("healthy-sidecar"),
        "connected to postgres\n",
        true,
    )
    .await;
    let cut = logs_run(
        std::future::ready(Ok(session_over(client, None))),
        &Asked {
            verb: Verb::Logs,
            namespace: Some("default"),
            name: "healthy-sidecar",
            container: Some("app"),
            kind: None,
            previous: false,
            follow: false,
        },
    )
    .await
    .expect("a log that stopped arriving does not end the run happily");

    assert!(
        cut.starts_with(
            "k8rs: the log stopped arriving before it ended, so what is above is not all of it — "
        ),
        "{cut:?}"
    );
}

/// **A namespace that is not a namespace never reaches a request, wherever it came from** — the
/// security gate's *names build paths* row.
///
/// **The kubeconfig is the source that nothing else checks.** [`mistyped`] refuses `--object`'s
/// namespace and `--namespace`'s; `k8s::kubeconfig_namespace` strips and bounds the context's own
/// and never asks whether it is a namespace, so `namespace: ../secrets` in the reader's own file
/// is the one way a word that is not a name reaches this path.
#[tokio::test]
async fn a_namespace_that_is_not_a_namespace_never_reaches_a_request() {
    let (client, asked) = answers("200 OK", pod_body("healthy-sidecar"), "hello\n").await;
    let crafted = session_over(client, Some("../secrets"));

    let refused = logs_run(
        std::future::ready(Ok(crafted)),
        &Asked {
            verb: Verb::Logs,
            namespace: None,
            name: "healthy-sidecar",
            container: Some("app"),
            kind: None,
            previous: false,
            follow: false,
        },
    )
    .await
    .expect("a namespace that is not one ends the run");

    assert!(
        refused.starts_with("k8rs: ../secrets is not a namespace"),
        "{refused:?}"
    );
    assert_eq!(
        asked.lock().expect("the log is never poisoned").len(),
        0,
        "a request went out with `../secrets` in its path"
    );
}

/// **The fallback reaches the *request*, and not only the sentence beside it** — the whole point of
/// saying *there's no previous run* is that k8rs then asks for the one that exists.
///
/// **Proved on the wire, because nothing else can prove it.** [`no_previous_run`] is the sentence
/// and it is asserted on its own above; deleting the `previous = false` line that follows it in
/// [`logs_run`] left that test green (`dev-core`'s red run, 2026-08-30) while the request went out
/// with `previous=true` and a real cluster answered `400`. This reads the query string.
#[tokio::test]
async fn previous_on_a_container_that_never_restarted_asks_for_the_run_that_exists() {
    let (client, asked) = answers("200 OK", pod_body("healthy-sidecar"), "hello\n").await;
    let run = |previous| Asked {
        verb: Verb::Logs,
        namespace: Some("default"),
        name: "healthy-sidecar",
        kind: None,
        container: Some("app"),
        previous,
        follow: false,
    };

    assert_eq!(
        logs_run(
            std::future::ready(Ok(session_over(client.clone(), None))),
            &run(true)
        )
        .await,
        None,
        "the run did not end happily"
    );
    let log_request = |asked: &Requests| {
        asked
            .lock()
            .expect("the log is never poisoned")
            .iter()
            .find(|path| path.contains("/log?"))
            .cloned()
            .expect("a log run asks for a log")
    };
    assert!(
        !log_request(&asked).contains("previous"),
        "`--previous` on a container with no restarts still went out as `previous=true`, so the \
         cluster answers 400 about a request the reader was just told k8rs would not make: {}",
        log_request(&asked)
    );

    // The negative half: a container that *has* restarted keeps the switch, or the fallback above
    // is a `--previous` that never works.
    let (crashed, crashed_asked) = answers("200 OK", pod_body("crashloop"), "boom\n").await;
    assert_eq!(
        logs_run(
            std::future::ready(Ok(session_over(crashed, None))),
            &Asked {
                verb: Verb::Logs,
                namespace: Some("default"),
                name: "broken-crashloop",
                kind: None,
                container: Some("quitter"),
                previous: true,
                follow: false,
            }
        )
        .await,
        None
    );
    assert!(
        log_request(&crashed_asked).contains("previous=true"),
        "`--previous` on a container with ten restarts was dropped, so the one log a crash loop \
         needs is unreachable: {}",
        log_request(&crashed_asked)
    );
}

/// **A run that named no namespace anywhere looks where `kubectl logs` would look** — the
/// context's own namespace, and `default` under that.
#[tokio::test]
async fn a_log_run_with_no_namespace_falls_back_to_the_context_and_then_to_default() {
    let status = serde_json::json!({
        "apiVersion": "v1", "kind": "Status", "status": "Failure",
        "reason": "NotFound", "code": 404, "message": "no",
    })
    .to_string();
    let bare = |namespace| Asked {
        verb: Verb::Logs,
        namespace,
        name: "web-7d9f4",
        kind: None,
        container: None,
        previous: false,
        follow: false,
    };

    let context = session_over(
        answers("404 Not Found", status.clone(), "").await.0,
        Some("from-the-context"),
    );
    assert_eq!(
        logs_run(std::future::ready(Ok(context)), &bare(None)).await,
        Some(
            "k8rs: there is no pod named web-7d9f4 in from-the-context — check the name and the \
             namespace"
                .to_string()
        ),
        "a run that named no namespace ignored the one its own context names"
    );

    let neither = session_over(answers("404 Not Found", status, "").await.0, None);
    assert_eq!(
        logs_run(std::future::ready(Ok(neither)), &bare(None)).await,
        Some(
            "k8rs: there is no pod named web-7d9f4 in default — check the name and the namespace"
                .to_string()
        ),
        "a run with no namespace anywhere did not look where `kubectl logs` looks"
    );
}

// --- ONE OBJECT'S OWN STORY ---
//
// **Most of this box is argv and one printed block**, so most of this region is [`mistyped`] and
// [`described`] — the two places every decision `--describe` and `--yaml` make can be read by a
// test. The wire is `k8s_tests.rs`'s and the run itself is `tests/binary.rs`'s: stdout belongs to
// the process and a test cannot read it back (§ WATCHING A CLUSTER).
//
// **The one assertion that is a *pair* is the exit code**, because `screens/detail.md` puts the
// whole difference between *this pod has no events* and *k8rs could not find out* there — both
// print byte-identical stdout, so a test that reads only the block cannot tell them apart and
// neither can a reader.

/// A pod read the three describe tests share.
async fn described_pod(capture: &str) -> k8s::PodRead {
    pod_read(capture).await
}

/// One event, as `k8s::events` hands it over.
fn happening(at: Option<&str>, reason: &str, message: &str) -> k8s::Happening {
    repeatedly(at, reason, message, None, None)
}

/// [`happening`], with the two fields a repeated event carries — `count` and when it was first
/// seen (`screens/detail.md` § A repeated event).
fn repeatedly(
    at: Option<&str>,
    reason: &str,
    message: &str,
    count: Option<i32>,
    first: Option<&str>,
) -> k8s::Happening {
    let stamp = |at: &str| Time(at.parse().expect("a fixed timestamp"));
    k8s::Happening {
        at: at.map(stamp),
        reason: reason.to_string(),
        message: message.to_string(),
        count,
        first: first.map(stamp),
    }
}

/// **The three verbs are one flag each, and a line carrying two is refused rather than ranked**
/// (the box's ruling 7, `screens/detail.md`'s one open question).
///
/// **`--once --live` stays ranked and that is the contrast**: those two are two *breadths* of one
/// read and the narrower is obviously meant, where three verbs are equally narrow — so picking one
/// prints a payload the reader did not ask for and gives no sign of it.
///
/// **The sentence names the ones that collided**, which is the whole of what a reader has to fix.
#[test]
fn two_verbs_over_one_object_are_refused_and_the_sentence_names_them() {
    let refused = mistyped(&argv(&["--logs", "--yaml", "--object", "default/web"]))
        .expect("two verbs over one object is a usage error");
    assert!(
        refused.starts_with("k8rs: --logs and --yaml each print a different thing"),
        "{refused:?}"
    );
    // **The first line only** — [`USAGE`] under it names all three by design, so a `contains`
    // over the whole message could never fail.
    assert!(
        !refused
            .lines()
            .next()
            .is_some_and(|first| first.contains("--describe")),
        "the sentence named a flag that is not on the line: {refused:?}"
    );

    // **All three pairs and not the one that happened to be written**, which is NOTES § D29's
    // rule about the shapes a guard was fed: only `--logs --yaml` and the triple were covered
    // (`tester`, 2026-08-31).
    for (line, named) in [
        (
            ["--logs", "--describe"],
            "k8rs: --logs and --describe each print",
        ),
        (
            ["--describe", "--yaml"],
            "k8rs: --describe and --yaml each print",
        ),
    ] {
        let mut words = line.to_vec();
        words.extend(["--object", "default/web"]);
        let refused = mistyped(&argv(&words)).expect("two verbs over one object is a usage error");
        assert!(refused.starts_with(named), "{words:?}: {refused:?}");
    }

    let three = mistyped(&argv(&[
        "--logs",
        "--describe",
        "--yaml",
        "--object",
        "default/web",
    ]))
    .expect("three verbs over one object is a usage error");
    assert!(
        three.starts_with("k8rs: --logs, --describe and --yaml each print"),
        "{three:?}"
    );

    // **The contrast, and it is asserted rather than described**: two breadths of one read are
    // still accepted, so this refusal is about verbs and not about *two flags*.
    assert_eq!(mistyped(&argv(&["--once", "--live"])), None);
    for one in [["--logs"], ["--describe"], ["--yaml"]] {
        let mut line = one.to_vec();
        line.extend(["--object", "default/web"]);
        assert_eq!(
            mistyped(&argv(&line)),
            None,
            "{line:?} was refused, and it names one verb"
        );
    }
}

/// **Every shape of `--kind` that names nothing usable is refused before anything connects** — the
/// same three-shapes-of-nothing rule `--context` and `--namespace` are already under.
///
/// **`--describe --kind` naming anything but a pod is refused too, and offline** — the sentence
/// names the one value it takes, so there is nothing a cluster could add to it
/// (`screens/detail.md`).
///
/// **The pairing sentence names the verb that is on the line**, which is the defect NOTES § D190
/// is about wearing the third verb's coat: *`--logs` and `--object` go together* is a message that
/// is not true of a run with no `--logs` on it.
#[test]
fn the_kind_flag_is_refused_for_nothing_and_for_a_describe_that_is_not_a_pod() {
    for line in [
        vec!["--yaml", "--object", "default/web", "--kind"],
        vec!["--yaml", "--object", "default/web", "--kind="],
        vec!["--yaml", "--object", "default/web", "--kind", ""],
    ] {
        let refused = mistyped(&argv(&line))
            .unwrap_or_else(|| panic!("{line:?} was accepted, so k8rs went and asked a cluster"));
        assert!(
            refused.starts_with("k8rs: --kind needs the name of a kind")
                && refused.contains("usage: k8rs "),
            "{line:?}: {refused:?}"
        );
    }

    let described = mistyped(&argv(&[
        "--describe",
        "--object",
        "default/web",
        "--kind",
        "secret",
    ]))
    .expect("--describe reads a pod and nothing else");
    assert!(
        described.starts_with(
            "k8rs: --describe only knows how to read a pod right now — containers and events \
             don't mean the same thing on a Secret. --kind pod is the only value it accepts"
        ),
        "{described:?}"
    );
    // **Both spellings of the flag are real**, for [`CONTEXT`]'s reason: matching only
    // `--kind NAME` lets `--kind=NAME` fall through to the default kind, which for `--yaml` is a
    // pod — a selector that silently selects the wrong thing.
    for spelled in [
        vec!["--describe", "--object", "default/web", "--kind", "pod"],
        vec!["--describe", "--object", "default/web", "--kind=pod"],
        vec!["--yaml", "--object", "default/web", "--kind=secret"],
    ] {
        assert_eq!(
            mistyped(&argv(&spelled)),
            None,
            "{spelled:?} was refused, and every flag on it is one k8rs has"
        );
    }
    // **`--kind` beside a verb that does not read it is accepted rather than refused**, which is
    // `--context` without `--live`'s own documented rule: it cannot point the run at something the
    // reader did not name.
    assert_eq!(
        mistyped(&argv(&[
            "--logs",
            "--object",
            "default/web",
            "--kind",
            "secret"
        ])),
        None
    );

    for verb in ["--describe", "--yaml"] {
        let alone = mistyped(&argv(&[verb])).expect("a verb with no object is half an instruction");
        assert!(
            alone.starts_with(&format!("k8rs: {verb} and --object go together")),
            "the sentence named a flag this run does not have: {alone:?}"
        );
    }
}

/// **All three verbs reach the cluster path and all three read `--context`** — a run read as a
/// file run would try to open `--object` as a path.
///
/// **A file beside any of them is refused and the sentence names the verb that is there**, which
/// is the same NOTES § D190 shape the pairing sentence above is under.
#[test]
fn the_two_new_verbs_are_cluster_runs_and_a_file_beside_one_names_it() {
    for verb in ["--describe", "--yaml"] {
        assert_eq!(
            live_context(&argv(&[verb, "--object", "web", "--context", "prod"])),
            Some(Some("prod")),
            "{verb} did not reach the context flag"
        );
        assert_eq!(live_context(&argv(&[verb, "--object", "web"])), Some(None));
        let said = mistyped(&argv(&[verb, "--object", "default/web", "pod.json"]))
            .expect("a file beside a cluster flag is refused");
        assert!(
            said.starts_with(&format!(
                "k8rs: {verb} reads a cluster, so k8rs cannot also read pod.json"
            )),
            "{said:?}"
        );
    }
}

/// **Which of the three a line asked for, and what it asked it about** — [`asked`] over all three
/// verbs, and the kind that only one of them reads.
#[test]
fn a_line_names_one_verb_one_object_and_at_most_one_kind() {
    let read = |words: &[&str]| {
        let args = argv(words);
        let asked = asked(&args).expect("a verb beside an object is a run");
        (
            asked.verb,
            asked.namespace.map(str::to_string),
            asked.name.to_string(),
            asked.kind.map(str::to_string),
        )
    };
    assert_eq!(
        read(&["--describe", "--object", "payments/web"]),
        (
            Verb::Describe,
            Some("payments".to_string()),
            "web".to_string(),
            None
        )
    );
    assert_eq!(
        read(&["--yaml", "--object", "db", "--kind", "secret"]),
        (
            Verb::Yaml,
            None,
            "db".to_string(),
            Some("secret".to_string())
        )
    );
    assert_eq!(
        read(&["--logs", "--object", "web"]).0,
        Verb::Logs,
        "the verb a log run is read as changed"
    );
    // **A line with no verb is not a run about an object at all**, whatever else is on it.
    assert!(asked(&argv(&["--once"])).is_none());
}

/// **One joiner, and the separator before the last item is the caller's** — the two sentences that
/// use it do not agree on the comma, and *"--logs, and --yaml"* is not English.
#[test]
fn a_list_of_things_reads_as_a_sentence() {
    assert_eq!(joined(&[] as &[String], " and "), "");
    assert_eq!(joined(&["one"], " and "), "one");
    assert_eq!(joined(&["one", "two"], " and "), "one and two");
    assert_eq!(joined(&["one", "two"], ", and "), "one, and two");
    assert_eq!(
        joined(&["one", "two", "three"], ", or "),
        "one, two, or three"
    );
}

/// **The block `--describe` puts on stdout: the identity line, the containers, and the events**
/// (`screens/detail.md` § Printed instead of drawn — describe).
///
/// **The containers block is the picker's own list**, so the order is declared-then-init, the word
/// is [`doing`]'s, and a restart count appears only where there is one — the same rule
/// [`container_choice`] draws it by, because a second wording for one fact is the drift this file
/// keeps refusing.
///
/// **The age is [`age`]'s ladder** and not a second spelling of it, so `created 4 min ago` is the
/// same string a card's right edge draws.
#[tokio::test]
async fn the_described_block_is_the_pod_its_containers_and_its_events() {
    // **`healthy-retry` and not `healthy-sidecar`, because it is the capture that reaches every
    // arm of the row** — two declared containers so the padding has something to align, one
    // regular and one init so the order rule is `spec` and not `status`, one `running` and one
    // `done` so both state words are drawn, and `restartCount` 0 beside 3 so the restart clause
    // is proven *and* proven absent. The sidecar capture carries `0` on both, so the
    // `", {} restarts"` arm ran in no test at all until this one (`tester`, 2026-08-31).
    let pod = described_pod("healthy-retry").await;
    let happened = k8s::Happened {
        lines: vec![
            happening(Some("2026-08-22T23:56:00Z"), "Unhealthy", "probe failed"),
            happening(Some("2026-08-19T00:00:00Z"), "Evicted", "the node was full"),
        ],
        cut: false,
    };
    let block = described(&pod, Some(&happened), &now());
    println!("{block}");

    // **The second line of each event is the raw word and the controller's message, always**
    // (NOTES § D198). `Unhealthy` keeps its phrase *and* its message, because the same reason word
    // covers a liveness probe and a readiness probe and only the message says which; `Evicted` has
    // no phrase in the events table, so its row is the age alone over the raw word and the
    // message — never an invented sentence.
    //
    // **The whole block and not a line at a time.** `starts_with("Pod · ")` plus
    // `contains(" · created ")` is satisfied by a line with the state word dropped entirely, and
    // `starts_with("  {name}")` is satisfied by a row with the state and the restart count gone —
    // which is everything the row is *for* (`tester`, 2026-08-31). Every character here is
    // derived: `Running` lowercased, `2026-08-20T22:42:55Z` against [`now`] on the ladder's
    // 48-hour rung, `spec` order, [`doing`]'s two words, the widest name plus three, and the
    // widest age plus two. **A re-capture that moves any of them reddens this on purpose.**
    assert_eq!(
        block,
        "Pod · running · created 2 days ago\n\
         \n\
         containers:\n\
         \x20\x20app           running\n\
         \x20\x20wait-for-db   done, 3 restarts\n\
         \n\
         events (newest first):\n\
         \x20\x204 min ago   the health check failed\n\
         \x20\x20\x20\x20(Unhealthy) probe failed\n\
         \x20\x204 days ago\n\
         \x20\x20\x20\x20(Evicted) the node was full"
    );
}

/// **No events prints no heading, and a read that failed prints byte-identical stdout to a read
/// that found nothing** — which is exactly why the exit code carries that difference
/// (`screens/detail.md`: *the only thing that can carry the difference*).
#[tokio::test]
async fn a_pod_with_no_events_and_one_whose_events_could_not_be_read_print_the_same_block() {
    let pod = described_pod("healthy-sidecar").await;
    let nothing = described(&pod, None, &now());
    let empty = described(&pod, Some(&k8s::Happened::default()), &now());

    assert_eq!(
        nothing, empty,
        "a failed events read and an empty one print different stdout, so the exit code is no \
         longer the only thing telling them apart"
    );
    assert!(
        !nothing.contains("events"),
        "an empty events section was dressed up with a heading over nothing: {nothing:?}"
    );
}

/// **A read that found no events says why on stderr, and one that found some says nothing** —
/// `screens/detail.md` § No events at all, whose whole point is the *second* sentence: *nothing
/// left* and *nothing happened* are different facts wearing the same empty list, and only one of
/// them is true of a pod that has been up for a week.
#[tokio::test]
async fn a_read_that_found_no_events_says_why_and_one_that_found_some_says_nothing() {
    let said = no_events(&k8s::Happened::default()).expect("an empty list has something to say");
    assert!(
        said.contains("Kubernetes only keeps events for a while") && said.contains("none are left"),
        "the sentence says the list is empty without saying why it can be: {said:?}"
    );
    assert_eq!(
        no_events(&k8s::Happened {
            lines: vec![happening(None, "Pulled", "")],
            cut: false,
        }),
        None,
        "a pod whose events were printed was told it has none"
    );
}

/// **A cut list withdraws the *newest first* claim in the heading, because that is where the claim
/// is** (`k8s::Happened::cut`, `k8s::EVENTS_KEPT`).
///
/// A `limit` returns the cluster's own order and not the newest, so a list that was cut is neither
/// all of them nor the newest of them — and the words that promise otherwise are the words that
/// have to go.
#[tokio::test]
async fn a_cut_events_list_stops_claiming_to_be_the_newest() {
    let pod = described_pod("healthy-sidecar").await;
    let cut = described(
        &pod,
        Some(&k8s::Happened {
            lines: vec![happening(Some("2026-08-22T23:56:00Z"), "Unhealthy", "no")],
            cut: true,
        }),
        &now(),
    );
    println!("{cut}");

    assert!(
        !cut.contains("newest first"),
        "a list the server had more of still called itself the newest: {cut:?}"
    );
    assert!(
        cut.contains("there are more, and these are not the newest"),
        "a cut list said nothing about being cut: {cut:?}"
    );
}

/// **An unknown kind is a spelling mistake and an ambiguous one names both spellings** — the two
/// sentences `screens/detail.md` writes, over the one ambiguity a cluster really has.
///
/// **`events` is that ambiguity**: `core/v1` and `events.k8s.io/v1` are different resources and
/// `browsable()` keeps both, so a bare word cannot pick one without silently reading the wrong
/// thing.
#[test]
fn a_kind_no_cluster_serves_and_one_two_resources_answer_to_get_their_own_sentences() {
    let kinds = [
        k8s::Browsable {
            group: String::new(),
            version: "v1".to_string(),
            kind: "Event".to_string(),
            plural: "events".to_string(),
            namespaced: true,
            verbs: vec!["list".to_string()],
        },
        k8s::Browsable {
            group: "events.k8s.io".to_string(),
            version: "v1".to_string(),
            kind: "Event".to_string(),
            plural: "events".to_string(),
            namespaced: true,
            verbs: vec!["list".to_string()],
        },
    ];

    let unknown = which_kind(&kinds, "widget").expect_err("no cluster serves widgets");
    assert_eq!(
        unknown,
        "k8rs: this cluster does not serve a kind named widget — check the spelling"
    );

    let both = which_kind(&kinds, "events").expect_err("two resources answer to `events`");
    assert_eq!(
        both,
        "k8rs: --kind events matches two things this cluster serves — the original one, and the \
         one events.k8s.io adds. Say which: --kind 'events.' for the original one, or --kind \
         'events.events.k8s.io' for the other"
    );

    assert_eq!(
        which_kind(&kinds, "events.")
            .expect("the trailing dot names the core group")
            .group,
        ""
    );

    // **A name refused for being enormous is echoed at the length the *requirement* names, and
    // `< 500` was a comfort figure that a cap of 400 also passed** (`tester`, 2026-08-31). The
    // security gate's row is *the echo is cut to what a name could have been*, so the number is
    // [`k8s::NAME_MAX`] and it is read off the constant rather than transcribed — both directions,
    // because a cap of ten satisfies an upper bound and tells the reader nothing.
    let huge = which_kind(&kinds, &"a".repeat(9000)).expect_err("no cluster serves that");
    assert!(
        !huge.contains(&"a".repeat(k8s::NAME_MAX + 1)),
        "the echo is longer than a kind's name could ever be: {} characters",
        huge.chars().count()
    );
    assert!(
        huge.contains(&"a".repeat(k8s::NAME_MAX)),
        "the echo was cut shorter than a name may be, so a reader cannot recognise what they \
         typed: {huge:?}"
    );
}

/// **The count in the ambiguity sentence is a word as far as a cluster plausibly goes and a digit
/// past that** — `screens/detail.md` writes *"matches two things"*, and the arms above and below it
/// had no test at all until this one (`tester`, 2026-08-31).
///
/// **`for the other` is the two-item shape and only that**, because *the other* naming two things
/// is not a sentence — so the three-item form spells out what each choice is for.
#[test]
fn the_number_of_things_a_kind_word_matches_is_a_word_until_it_stops_being_readable_as_one() {
    let serving = |groups: &[&str]| -> Vec<k8s::Browsable> {
        groups
            .iter()
            .map(|group| k8s::Browsable {
                group: (*group).to_string(),
                version: "v1".to_string(),
                kind: "Widget".to_string(),
                plural: "widgets".to_string(),
                namespaced: true,
                verbs: vec!["list".to_string()],
            })
            .collect()
    };
    let said = |groups: &[&str]| {
        which_kind(&serving(groups), "widgets").expect_err("more than one thing answers to it")
    };

    let three = said(&["", "a.io", "b.io"]);
    println!("{three}");
    assert!(
        three.starts_with(
            "k8rs: --kind widgets matches three things this cluster serves — the original one, \
             the one a.io adds, and the one b.io adds. Say which: --kind 'widgets.' for the \
             original one, --kind 'widgets.a.io' for the one a.io adds, or --kind 'widgets.b.io' \
             for the one b.io adds"
        ),
        "{three:?}"
    );
    assert!(
        !three.contains("for the other"),
        "three things were offered with *the other* naming two of them: {three:?}"
    );

    // **Four is where the word gives way to the digit**, which is the arm the comment beside it
    // used to claim started at three.
    let four = said(&["", "a.io", "b.io", "c.io"]);
    assert!(
        four.starts_with("k8rs: --kind widgets matches 4 things this cluster serves"),
        "{four:?}"
    );
}

/// **`--describe` reads the pod, then that pod's events, and nothing else** — two round trips, one
/// `kubectl` line (invariant 4: the command log shows the *equivalent* command, not the two calls
/// underneath it).
///
/// **The uid is on the events request**, which is what stops a pod deleted and recreated under one
/// name inheriting the dead one's events.
#[tokio::test]
async fn a_describe_run_reads_the_pod_then_its_own_events() {
    let (client, asked) = answers_with_events(
        pod_body("healthy-sidecar"),
        Some((
            "200 OK",
            serde_json::json!({
            "apiVersion": "v1", "kind": "EventList",
            "metadata": { "resourceVersion": "1" },
            "items": [{
                "apiVersion": "v1", "kind": "Event",
                "metadata": { "name": "web.1", "namespace": "default",
                              "creationTimestamp": "2026-08-22T23:56:00Z" },
                "involvedObject": { "kind": "Pod", "name": "healthy-sidecar" },
                "reason": "Unhealthy", "message": "probe failed", "type": "Warning",
                "lastTimestamp": "2026-08-22T23:56:00Z",
            }],
            })
            .to_string(),
        )),
    )
    .await;
    let ended = describe_run(
        std::future::ready(Ok(session_over(client, None))),
        &Asked {
            verb: Verb::Describe,
            namespace: Some("default"),
            name: "healthy-sidecar",
            kind: None,
            container: None,
            previous: false,
            follow: false,
        },
        &now(),
        OBJECT_READ,
    )
    .await;
    let paths = asked.lock().expect("the log is never poisoned").clone();
    println!("{paths:?}");

    assert_eq!(
        ended, None,
        "a describe over a cluster that answered did not exit 0: {ended:?}"
    );
    assert_eq!(paths.len(), 2, "describe made {} requests", paths.len());
    assert_eq!(paths[0], "/api/v1/namespaces/default/pods/healthy-sidecar");
    assert!(
        paths[1].starts_with("/api/v1/namespaces/default/events?")
            && paths[1].contains("involvedObject.name%3Dhealthy-sidecar")
            && paths[1].contains("involvedObject.kind%3DPod"),
        "the events fetch did not name this object: {:?}",
        paths[1]
    );
    assert!(
        paths[1].contains("involvedObject.uid%3D"),
        "the events fetch carried no uid, so a recreated pod inherits the dead one's events: {:?}",
        paths[1]
    );
}

/// **The three endings a describe can have once the pod has been read, told apart by the exit code
/// and by nothing else** — `screens/detail.md` calls that code *the only thing that can carry the
/// difference*, because all three print the identical block on stdout.
///
/// **None of the three was reachable until [`served`] let the events path have its own status**
/// (`tester`, 2026-08-31): one status for the whole server cannot spell *the pod was served and
/// the events were refused*, so the family's central claim had no test under it at all.
///
/// **The refusal names the missing verb and the resource**, which the security gate asks of every
/// `403`, and it is [`because`]'s own frame so it cannot drift from the one a refused watch gets.
///
/// **The timeout arm runs against a server that takes the request and never answers**, which is
/// the only shape that reaches it — a dropped socket is a connection error and lands in the arm
/// above. The deadline is the test's for [`describe_run`]'s stated reason.
///
/// **What the first arm pins is the exit code and not the sentence beside it**, and the limit is
/// worth stating rather than discovering: [`NO_EVENTS`] goes to stderr, which belongs to the
/// process (§ WATCHING A CLUSTER), so a reversion that printed the wrong line here would still
/// pass. The words are pinned one test up, by
/// [`a_read_that_found_no_events_says_why_and_one_that_found_some_says_nothing`] over
/// [`no_events`]; what is proven here is that a read which succeeded and found nothing ends the
/// run *happily*, which is the half `screens/detail.md` says only the code can carry.
#[tokio::test]
async fn the_three_endings_of_a_describe_are_told_apart_by_the_exit_code() {
    let about = |name| Asked {
        verb: Verb::Describe,
        namespace: Some("default"),
        name,
        kind: None,
        container: None,
        previous: false,
        follow: false,
    };
    let ran = |events, deadline| async move {
        let (client, asked) = answers_with_events(pod_body("healthy-sidecar"), events).await;
        let ended = describe_run(
            std::future::ready(Ok(session_over(client, None))),
            &about("healthy-sidecar"),
            &now(),
            deadline,
        )
        .await;
        let paths = asked.lock().expect("the log is never poisoned").len();
        (ended, paths)
    };

    // **A read that succeeded and found nothing is exit `0`** — the calm half of the pair, and
    // the events were really asked for, which is what the second request says.
    assert_eq!(
        ran(Some(("200 OK", no_event_list())), OBJECT_READ).await,
        (None, 2),
        "a pod whose events were read and were empty did not end happily, so `k8rs --describe` \
         exits 2 on a healthy pod"
    );

    // **A read that was refused is exit `2`, with the verb and the resource named.**
    let refused = serde_json::json!({
        "apiVersion": "v1", "kind": "Status", "status": "Failure",
        "reason": "Forbidden", "code": 403, "message": "no",
    })
    .to_string();
    assert_eq!(
        ran(Some(("403 Forbidden", refused)), OBJECT_READ).await,
        (
            Some("k8rs: the role this kubeconfig uses needs to list events in default".to_string()),
            2
        ),
        "a refused events read did not name the verb and the resource the reader has to be granted"
    );

    // **A read that never finished is exit `2` as well, and says so as a wait rather than as a
    // refusal.** 50ms and not [`OBJECT_READ`]: ten real seconds here is ten more on every one of
    // the mutation gate's runs.
    let deadline = std::time::Duration::from_millis(50);
    assert_eq!(
        ran(None, deadline).await,
        (
            Some(format!(
                "k8rs: this cluster has not answered for the events of the pod \
                 healthy-sidecar in default after {} seconds",
                deadline.as_secs()
            )),
            2
        ),
        "a cluster that took the events request and never answered was reported as something \
         other than a wait"
    );
}

/// **A pod that is not there ends a describe before the events are asked for**, with the same
/// sentence a log run gives — one wording, because it is one read.
#[tokio::test]
async fn a_describe_of_a_pod_that_is_not_there_asks_for_no_events() {
    let status = serde_json::json!({
        "apiVersion": "v1", "kind": "Status", "status": "Failure",
        "reason": "NotFound", "code": 404, "message": "no",
    })
    .to_string();
    let (client, asked) = answers("404 Not Found", status, "").await;
    let ended = describe_run(
        std::future::ready(Ok(session_over(client, None))),
        &Asked {
            verb: Verb::Describe,
            namespace: Some("payments"),
            name: "ghost",
            kind: None,
            container: None,
            previous: false,
            follow: false,
        },
        &now(),
        OBJECT_READ,
    )
    .await;

    assert_eq!(
        ended,
        Some(
            "k8rs: there is no pod named ghost in payments — check the name and the namespace"
                .to_string()
        )
    );
    assert_eq!(
        asked.lock().expect("the log is never poisoned").len(),
        1,
        "the events were asked for over a pod that is not there"
    );
}

/// **A kind that lives in no namespace is not sent to check one** — *check the name and the
/// namespace* is advice about a word that is not on the line for `--kind node`, and advice a
/// reader cannot act on is a sentence that should not have been printed.
#[test]
fn a_cluster_scoped_kind_is_named_without_a_namespace_it_does_not_have() {
    let status = serde_json::json!({
        "apiVersion": "v1", "kind": "Status", "status": "Failure",
        "reason": "NotFound", "code": 404, "message": "no",
    })
    .to_string();
    let gone: kube::Error = kube::Error::Api(Box::new(
        serde_json::from_str(&status).expect("a Status this file wrote"),
    ));

    assert_eq!(
        read_failed(&gone, "node", "ghost", None, None),
        "k8rs: there is no node named ghost — check the name"
    );
    assert_eq!(
        read_failed(&gone, "secret", "ghost", Some("payments"), None),
        "k8rs: there is no secret named ghost in payments — check the name and the namespace"
    );
    assert_eq!(
        no_answer("node", "ghost", None, 10),
        "k8rs: this cluster has not answered for the node ghost after 10 seconds"
    );
    assert_eq!(
        no_answer("pod", "web", Some("payments"), 10),
        "k8rs: this cluster has not answered for the pod web in payments after 10 seconds"
    );
}

/// A session whose discovery answer is the caller's, over a client of the caller's choosing —
/// [`session_over`] with the half `--yaml` reads and the log verbs do not.
fn serving(client: kube::Client, kinds: Vec<k8s::Browsable>) -> k8s::Session {
    k8s::Session {
        client,
        namespace: None,
        ..saying(
            Ok("v1.36.1".to_string()),
            Ok(k8s::Served {
                kinds,
                capabilities: None,
            }),
            None,
        )
    }
}

/// One kind, as discovery described it and `k8s::browsable` kept it.
fn browsable(group: &str, kind: &str, plural: &str, namespaced: bool) -> k8s::Browsable {
    k8s::Browsable {
        group: group.to_string(),
        version: "v1".to_string(),
        kind: kind.to_string(),
        plural: plural.to_string(),
        namespaced,
        verbs: vec!["list".to_string()],
    }
}

/// **`--yaml` reads the one object `--kind` and `--object` name, and reads it once** — one round
/// trip, no watch, no `Table` (`screens/detail.md` § The yaml tab).
///
/// **The path is the assertion, because the document itself is stdout's** and a test cannot read
/// the process's own stream back (§ WATCHING A CLUSTER). What the document *says* — the masking,
/// the key order, the strip — is `k8s_tests.rs`'s, over the same function.
#[tokio::test]
async fn a_yaml_run_reads_the_one_object_its_kind_names() {
    let secret = serde_json::json!({
        "apiVersion": "v1", "kind": "Secret", "type": "Opaque",
        "metadata": { "name": "db-credentials", "namespace": "payments" },
        "data": { "username": "YWRtaW4=" },
    })
    .to_string();
    let (client, asked) = answers("200 OK", secret, "").await;
    let ended = yaml_run(
        std::future::ready(Ok(serving(
            client,
            vec![browsable("", "Secret", "secrets", true)],
        ))),
        &Asked {
            verb: Verb::Yaml,
            namespace: Some("payments"),
            name: "db-credentials",
            kind: Some("secret"),
            container: None,
            previous: false,
            follow: false,
        },
    )
    .await;
    let paths = asked.lock().expect("the log is never poisoned").clone();

    assert_eq!(
        ended, None,
        "a yaml run over a cluster that answered: {ended:?}"
    );
    assert_eq!(
        paths,
        ["/api/v1/namespaces/payments/secrets/db-credentials"],
        "the document was read from somewhere else, or read more than once"
    );
}

/// **A kind that lives in no namespace is read without one, and its `kubectl` line has no `-n`** —
/// `k8s::Fetch::table`'s own rule, reached from here so the two cannot disagree.
///
/// **The path is again the assertion**: `/api/v1/nodes/k8rs-worker2` and not
/// `/api/v1/namespaces/default/nodes/…`, which is a path no server answers.
#[tokio::test]
async fn a_cluster_scoped_kind_is_read_without_a_namespace() {
    let node = serde_json::json!({
        "apiVersion": "v1", "kind": "Node", "metadata": { "name": "worker" },
    })
    .to_string();
    let (client, asked) = answers("200 OK", node, "").await;
    let ended = yaml_run(
        std::future::ready(Ok(serving(
            client,
            vec![browsable("", "Node", "nodes", false)],
        ))),
        &Asked {
            verb: Verb::Yaml,
            namespace: Some("payments"),
            name: "worker",
            kind: Some("node"),
            container: None,
            previous: false,
            follow: false,
        },
    )
    .await;

    assert_eq!(ended, None, "{ended:?}");
    assert_eq!(
        asked.lock().expect("the log is never poisoned").clone(),
        ["/api/v1/nodes/worker"],
        "a cluster-scoped kind was read under a namespace, which is a path no server answers"
    );
}

/// **A cluster that would not say what it serves cannot be asked which kind `--kind` means, and
/// the sentence names that refusal rather than the object** — the same `get /apis` clause
/// [`greeting`] prints, because it is the same refusal on the same path (NOTES § D160).
///
/// **Nothing is read.** A run that cannot resolve the kind has no path to ask for, so the refusal
/// arrives before a single request — which is what the empty request log below says.
#[tokio::test]
async fn a_yaml_run_over_a_cluster_that_would_not_say_what_it_serves_reads_nothing() {
    let (client, asked) = answers("200 OK", "{}".to_string(), "").await;
    let refused = yaml_run(
        std::future::ready(Ok(k8s::Session {
            client,
            namespace: None,
            ..saying(
                Ok("v1.36.1".to_string()),
                Err(api_error(403, "Forbidden")),
                None,
            )
        })),
        &Asked {
            verb: Verb::Yaml,
            namespace: Some("payments"),
            name: "db-credentials",
            kind: Some("secret"),
            container: None,
            previous: false,
            follow: false,
        },
    )
    .await
    .expect("a run that cannot resolve its kind ends");

    assert!(
        refused.starts_with(
            "k8rs: this cluster would not say what kinds it serves, so k8rs cannot tell which \
             one --kind means — "
        ) && refused.contains("get /apis"),
        "the refusal did not name the call that was refused: {refused:?}"
    );
    assert!(
        asked.lock().expect("the log is never poisoned").is_empty(),
        "a run that could not resolve its kind still went and asked for an object"
    );
}

/// **A kind the cluster does not serve ends the run before anything is asked for**, with
/// [`which_kind`]'s sentence — the seam between the two is what this proves, since that function's
/// own words are asserted beside it.
#[tokio::test]
async fn a_yaml_run_for_a_kind_no_cluster_serves_asks_for_nothing() {
    let (client, asked) = answers("200 OK", "{}".to_string(), "").await;
    let refused = yaml_run(
        std::future::ready(Ok(serving(
            client,
            vec![browsable("", "Pod", "pods", true)],
        ))),
        &Asked {
            verb: Verb::Yaml,
            namespace: Some("payments"),
            name: "x",
            kind: Some("widget"),
            container: None,
            previous: false,
            follow: false,
        },
    )
    .await
    .expect("a kind nothing serves ends the run");

    assert_eq!(
        refused,
        "k8rs: this cluster does not serve a kind named widget — check the spelling"
    );
    assert!(
        asked.lock().expect("the log is never poisoned").is_empty(),
        "a run that could not resolve its kind still went and asked for an object"
    );
}

/// **`--kind` defaults to a pod**, so `--yaml --object payments/web` reads the pod — the default
/// the flag's own doc states, proven at the seam rather than only at [`k8s::kind_named`].
#[tokio::test]
async fn a_yaml_run_with_no_kind_reads_a_pod() {
    let pod = serde_json::json!({
        "apiVersion": "v1", "kind": "Pod", "metadata": { "name": "web", "namespace": "payments" },
    })
    .to_string();
    let (client, asked) = answers("200 OK", pod, "").await;
    assert_eq!(
        yaml_run(
            std::future::ready(Ok(serving(
                client,
                vec![
                    browsable("", "Pod", "pods", true),
                    browsable("", "Secret", "secrets", true),
                ],
            ))),
            &Asked {
                verb: Verb::Yaml,
                namespace: Some("payments"),
                name: "web",
                kind: None,
                container: None,
                previous: false,
                follow: false,
            },
        )
        .await,
        None
    );
    assert_eq!(
        asked.lock().expect("the log is never poisoned").clone(),
        ["/api/v1/namespaces/payments/pods/web"],
        "a run that named no kind read something other than a pod"
    );
}

/// **What a container's row says, state by state** — the four the picker already had, and the two
/// diagnoses describe adds because there is no card in the same output to carry them
/// (`screens/detail.md` § The describe tab, `k8s-admin` 2026-08-31).
///
/// **`done` for a clean exit and `failed` for anything else** is the finding: three containers
/// that exited 1, 0 and 255 all printed `done`, and `done` is a false statement about the third.
///
/// **`waiting` printed alike for three different problems** is the other half — `ImagePullBackOff`,
/// `CrashLoopBackOff` and `CreateContainerConfigError` are one word to the old code and three
/// different things to do.
#[test]
fn a_containers_row_says_what_happened_and_not_one_word_for_every_ending() {
    let stopped = |reason: Option<&str>, exit_code| {
        container_state(Some(&ContainerState::Terminated(rules::Terminated {
            reason: reason.map(str::to_string),
            exit_code,
            started_at: None,
            finished_at: None,
            message: None,
        })))
    };
    let waiting = |reason: Option<&str>| {
        container_state(Some(&ContainerState::Waiting {
            reason: reason.map(str::to_string),
            message: None,
        }))
    };

    // **A clean exit is the healthy case and keeps the calm word**, with no second line at all.
    assert_eq!(stopped(Some("Completed"), 0), ("done".to_string(), None));
    assert_eq!(stopped(None, 0), ("done".to_string(), None));

    // **The one translated reason is invariant 14's own worked example**, and the exit code is
    // there whether or not a phrase was.
    assert_eq!(
        stopped(Some("OOMKilled"), 137),
        (
            "failed".to_string(),
            Some("container exceeded its memory limit — exit 137".to_string())
        )
    );
    // **Everything else falls through to the exit code alone, never a guessed word** — both
    // shapes `k8s-admin` measured on one pod.
    assert_eq!(
        stopped(Some("Error"), 1),
        ("failed".to_string(), Some("exit 1".to_string()))
    );
    assert_eq!(
        stopped(None, 255),
        ("failed".to_string(), Some("exit 255".to_string()))
    );

    // **A waiting container says which problem it is**, in a phrase derived from the card
    // `rules.rs` already draws for the same state.
    for (reason, said) in [
        ("CrashLoopBackOff", "keeps crashing and restarting"),
        ("ImagePullBackOff", "cannot get its image"),
        ("ErrImagePull", "cannot get its image"),
        (
            "CreateContainerConfigError",
            "needs a ConfigMap or Secret that does not exist",
        ),
    ] {
        assert_eq!(
            waiting(Some(reason)),
            (said.to_string(), None),
            "{reason} still prints one generic word"
        );
    }
    // **A momentary sandbox is not dressed up as a problem**, and a reason no table names falls
    // through to its own raw word rather than to a guess.
    assert_eq!(
        waiting(Some("ContainerCreating")),
        ("not started".to_string(), None)
    );
    assert_eq!(
        waiting(Some("PodInitializing")),
        ("not started".to_string(), None)
    );
    assert_eq!(
        waiting(Some("InvalidImageName")),
        ("InvalidImageName".to_string(), None)
    );
    assert_eq!(waiting(None), ("waiting".to_string(), None));

    assert_eq!(
        container_state(Some(&ContainerState::Running { started_at: None })),
        ("running".to_string(), None)
    );
    assert_eq!(container_state(None), ("not started".to_string(), None));

    // **The log picker's own wording is untouched** — [`doing`]'s doc argues correctly that the
    // jargon card is one keypress away *there*, and this is a second reader of one state rather
    // than a second spelling of one sentence.
    assert_eq!(
        doing(Some(&ContainerState::Waiting {
            reason: Some("CrashLoopBackOff".to_string()),
            message: None
        })),
        "waiting"
    );
    assert_eq!(
        doing(Some(&ContainerState::Terminated(rules::Terminated {
            reason: Some("OOMKilled".to_string()),
            exit_code: 137,
            started_at: None,
            finished_at: None,
            message: None
        }))),
        "done"
    );
}

/// **A pod carrying `status.reason` says why, and one without it prints exactly what it did
/// before** (`screens/detail.md` § The pod's own reason).
///
/// **Measured, not imagined**: a pod carrying `reason: Evicted` printed `Pod · failed · created 8
/// days ago` and never said why — a `Failed` that tells a reader nothing any other `Failed` would
/// not (`k8s-admin`, 2026-08-31).
///
/// **`status.message` is not on the snapshot and this build cannot print it**, so `(Evicted)`
/// stands alone under the phrase. That is the honest half rather than a sentence invented to fill
/// the line, and it is the PM's boxed snapshot field that completes it.
#[tokio::test]
async fn a_pod_with_a_reason_of_its_own_says_why_under_the_identity_line() {
    let mut pod = described_pod("healthy-retry").await;
    assert!(
        !described(&pod, None, &now()).contains('('),
        "a pod with no status.reason grew a reason block"
    );

    pod.snapshot.reason = Some("Evicted".to_string());
    let block = described(&pod, None, &now());
    println!("{block}");
    let lines: Vec<&str> = block.lines().collect();
    assert_eq!(lines[0], "Pod · running · created 2 days ago");
    assert_eq!(lines[1], "removed by the node to take back room");
    assert_eq!(lines[2], "(Evicted)");

    // **Anything the table does not name falls through to its raw word beside the message**, the
    // same safe fallback every table on this surface uses.
    pod.snapshot.reason = Some("NodeAffinity".to_string());
    let other = described(&pod, None, &now());
    assert_eq!(other.lines().nth(1), Some("(NodeAffinity)"), "{other:?}");
}

/// **An event that happened more than once says how many times and over how long**
/// (`screens/detail.md` § A repeated event, NOTES § D198's `count` half).
///
/// **The count is where the information is.** *The health check failed 4 minutes ago* and *it has
/// failed 2,383 times since 4 days ago* are different diagnoses of one pod, and the kubelet bumps
/// `count` on one Event rather than creating another — which is why distinct events stay
/// single-digit and this line is the only place the number can appear.
///
/// **Exact and comma-grouped, never rounded**, through the one separator `k8s::grouped` is.
#[tokio::test]
async fn an_event_that_happened_many_times_says_how_many_and_over_how_long() {
    let pod = described_pod("healthy-retry").await;
    let block = |line| {
        described(
            &pod,
            Some(&k8s::Happened {
                lines: vec![line],
                cut: false,
            }),
            &now(),
        )
    };

    let many = block(repeatedly(
        Some("2026-08-22T23:56:00Z"),
        "Unhealthy",
        "Readiness probe failed: HTTP probe failed with statuscode: 503",
        Some(2_383),
        Some("2026-08-19T00:00:00Z"),
    ));
    println!("{many}");
    let tail: Vec<&str> = many.lines().rev().take(3).collect();
    assert_eq!(
        tail,
        [
            "    happened 2,383 times since 4 days ago",
            "    (Unhealthy) Readiness probe failed: HTTP probe failed with statuscode: 503",
            "  4 min ago  the health check failed",
        ],
        "the repeated line is missing, unrounded, or not under the message"
    );

    // **Two is the boundary the screen's rule sits on** — *the line only appears when `count` is
    // more than one* — and it was the one number no case fed, so `< 2` and `<= 2` were the same
    // test until this ran (`dev-core`'s gate, 2026-08-31).
    let twice = block(repeatedly(
        Some("2026-08-22T23:56:00Z"),
        "Unhealthy",
        "probe failed",
        Some(2),
        Some("2026-08-19T00:00:00Z"),
    ));
    assert!(
        twice.contains("    happened 2 times since 4 days ago"),
        "a thing that happened twice was reported as happening once: {twice:?}"
    );

    // **Silent at one and silent with no count at all** — a thing that happened once needs no
    // sentence saying it happened once.
    for once in [Some(1), None] {
        let single = block(repeatedly(
            Some("2026-08-22T23:56:00Z"),
            "Unhealthy",
            "probe failed",
            once,
            Some("2026-08-19T00:00:00Z"),
        ));
        assert!(
            !single.contains("happened"),
            "an event with count {once:?} claimed to have repeated: {single:?}"
        );
    }

    // **The count without a span is still worth printing**, because it is the half that says
    // *how bad*; a span this file guessed would not be.
    let undated = block(repeatedly(
        Some("2026-08-22T23:56:00Z"),
        "Unhealthy",
        "probe failed",
        Some(12),
        None,
    ));
    assert!(
        undated.contains("    happened 12 times\n") || undated.ends_with("    happened 12 times"),
        "an event with no first stamp lost its count, or grew a span from nowhere: {undated:?}"
    );
}

/// **An event the API let through with no reason, no message or no stamp still draws a row a
/// reader can read** — the shapes `Event`'s own schema allows, all three of which this file will
/// meet the first time a controller emits a sparse one.
///
/// **`()` in front of a message is a word invented out of a field that was not there**, and a row
/// of spaces above the message is what padding a blank age to the column width produces. Both are
/// unhappy-path shapes rather than measured ones: nothing on the fixture cluster emitted them,
/// which is why they are pinned here rather than left to be found.
#[tokio::test]
async fn a_sparse_event_draws_no_empty_brackets_and_no_blank_row() {
    let pod = described_pod("healthy-retry").await;
    let block = |line| {
        described(
            &pod,
            Some(&k8s::Happened {
                lines: vec![line],
                cut: false,
            }),
            &now(),
        )
    };

    let nameless = block(happening(
        Some("2026-08-22T23:56:00Z"),
        "",
        "the node was full",
    ));
    println!("{nameless}");
    assert!(
        nameless.ends_with("    the node was full") && !nameless.contains("()"),
        "an event with no reason drew empty brackets: {nameless:?}"
    );

    // **No stamp and no phrase leaves nothing for the first line**, so there is no first line —
    // never a row of spaces.
    let bare = block(happening(None, "FailedMount", "secret not found"));
    println!("{bare}");
    let tail: Vec<&str> = bare.lines().rev().take(2).collect();
    assert_eq!(
        tail,
        [
            "    (FailedMount) secret not found",
            "events (newest first):"
        ],
        "an undated event with no phrase drew a blank row above its message"
    );

    // **A stamp with no phrase keeps the age and nothing else**, with no padding left hanging.
    let dated = block(happening(
        Some("2026-08-22T23:56:00Z"),
        "FailedMount",
        "secret not found",
    ));
    assert!(
        dated.contains("\n  4 min ago\n    (FailedMount) secret not found"),
        "the age line was padded past its own text: {dated:?}"
    );
}

/// **One flag, one notion of what `pod` is spelled as** — `k8s::kind_named` lowercases and matches
/// the plural as well as the kind, so a raw `!= "pod"` in [`mistyped`] refused
/// `--describe --kind pods`: the spelling `kubectl get pods` teaches, turned down with a sentence
/// about Secrets (`k8s-admin`, 2026-08-31).
#[test]
fn describe_accepts_every_spelling_of_pod_that_the_kind_resolver_would() {
    for spelled in ["pod", "pods", "Pod", "PODS"] {
        assert_eq!(
            mistyped(&argv(&[
                "--describe",
                "--object",
                "default/web",
                "--kind",
                spelled
            ])),
            None,
            "--describe refused {spelled:?}, which --yaml accepts and kubectl teaches"
        );
    }
    for wrong in ["secret", "secrets", "podmetrics"] {
        let refused = mistyped(&argv(&[
            "--describe",
            "--object",
            "default/web",
            "--kind",
            wrong,
        ]))
        .unwrap_or_else(|| panic!("--describe accepted {wrong:?}"));
        assert!(
            refused.starts_with("k8rs: --describe only knows how to read a pod right now"),
            "{wrong:?}: {refused:?}"
        );
    }
}

/// **The printed line produces what was printed** (invariant 4). `kubectl` has hidden
/// `managedFields` from `get -o yaml` since v1.21 and this pane does not, so without
/// `--show-managed-fields` the teaching line describes a *different, shorter* document — measured
/// at 95 of a pod's 246 lines, 39% of it (`k8s-admin`, 2026-08-31).
///
/// **A function rather than a `writeln!` inline**, for `k8s::LogRequest::kubectl`'s reason: stderr
/// belongs to the process and a test cannot read it back, which is what left this unproven.
#[test]
fn the_yaml_teaching_line_asks_for_the_document_that_was_printed() {
    assert_eq!(
        kubectl_get("secret", "db-credentials", Some("payments")),
        "$ kubectl get secret db-credentials -n payments -o yaml --show-managed-fields"
    );
    // **A kind that lives in no namespace carries no `-n`**, the same rule every other sentence
    // about one object on this surface follows.
    assert_eq!(
        kubectl_get("node", "k8rs-worker2", None),
        "$ kubectl get node k8rs-worker2 -o yaml --show-managed-fields"
    );
}

// --- ONE LINE, OUT OF EVERY PATH TEXT LEAVES BY ---
//
// **Sanitising for the screen and emitting for a consumer are two different jobs, and this is the
// second one** (todo.md § Phase 6, `PRIOR-ART § D1`). The strip is proven elsewhere — this asks the
// other question: does anything a *printer* did to a value survive into what leaves the process?
// A wrap, a pad, a cut, a second strip: the reader who redirects `--logs` to a file or pipes
// `--once` into `grep` gets whatever the pane did, and none of it is in the object.
//
// **The paths were enumerated off `main.rs`'s writes rather than off a list, and the first count
// here was wrong** — *four places* against a measured **seven `writeln!` and one `write!`**
// (`k8s-admin`, 2026-08-31): the file-driven report, the live report, the log dump's
// dropped-lines sentence *and* its lines, the `--follow` arm, [`stream_ended`]'s end-of-stream
// marker, the describe block, and the document. Plus the `k8rs: …` sentence every failing run
// ends on and the `$ kubectl …` line invariant 4 owes.
//
// **Two of those eight carry no cluster text and are named rather than fed**: [`stream_ended`]'s
// marker and the dropped-lines sentence are `&'static str` this file wrote, which is what
// `LogLines::dropped_line`'s own doc is about. `--yaml` is proven in `k8s_tests.rs`, where the
// tree it re-reads as lives; every other one is here.
//
// **Two of them cannot be a `contains`, and are asserted whole instead**: the followed log line and
// the fetched one *are* the payload, so an addition anywhere on the line is a byte the container
// did not write.

/// **A line as a cluster wrote it**: an `ESC` at the front, brackets that are data and not markup,
/// a bidi override late in the sentence, and **163 characters** — 165 bytes — with a space either
/// side of column 80 and of column 120. (The count is the constant's own, measured, not the length
/// of the sentence it reads as: `AFTER_ONE_STRIP` is 161 after the two characters are removed, and
/// the sibling in `k8s_tests.rs` is a different string at 158.)
const FROM_THE_CLUSTER: &str = "\u{1b}[2Jallocating 240MB of cache [accounts] for the accounts \
                                table, which is one sentence long enough to cross both an 80 \
                                column and a 120 column bo\u{202e}undary twice over";

/// **The same line after the one transformation this repo documents** — the ingest strip, which
/// removes a character with no printed form of its own and changes nothing else (NOTES § D146,
/// § D154).
///
/// **Written out rather than computed**, so it says what the requirement is and not what the code
/// returned (CLAUDE.md § Tests must not lie). Neither planted character is whitespace, so `text`'s
/// substitution and `sanitize`'s removal produce this same string — which is
/// `sanitize_cannot_act_on_anything_the_ingest_strip_left`'s subject, one file down.
const AFTER_ONE_STRIP: &str = "[2Jallocating 240MB of cache [accounts] for the accounts table, \
                               which is one sentence long enough to cross both an 80 column and a \
                               120 column boundary twice over";

/// **One line, out of every path text leaves k8rs by, carrying exactly one transformation.**
///
/// **This is not a guard waiting for a subject, and calling it one is how it gets deleted as noise
/// in Phase 11** (`k8s-admin`, 2026-08-31). It fires on four things, three of which exist today:
/// a value cut short, a value padded from the inside, a value stripped a second time, and a value
/// folded across lines. Only the last has no producer yet — `column` pads to a width and never
/// cuts, and `serde_yaml_ng`'s emitter does not fold a long scalar (measured: a 155-character
/// scalar comes back on one line, `dev-core` 2026-08-31) — and it is fed its own boundary anyway,
/// so it needs nothing added on the day one arrives.
///
/// **Three of the four were run**, each against a planted producer: `card` wrapping the evidence
/// at 80 columns, `raw_and_message` cutting at 100 characters, and `dump` padding every line to a
/// column width. Each fails naming its own path.
///

#[tokio::test]
async fn one_line_comes_out_of_every_emit_path_with_one_transformation_on_it() {
    assert!(
        FROM_THE_CLUSTER.chars().count() > 130,
        "the line does not reach a second wrap boundary, so this guard is not fed what it is for"
    );

    // **`--once`**, through the report the file-driven run and the live run both print.
    let mut broken = finding(Severity::Critical, pod_id("payments", "web"));
    broken.evidence = FROM_THE_CLUSTER.to_string();
    let once = render(&[broken], &nothing_read());

    // **`--analysis`**, where Posture prints a `hostPath` as a row's whole text rather than as a
    // value inside a sentence — the framing that has no delimiter to hide a cut behind.
    let mut input = read(&["healthy-hostpath.json", "nodes.json"]);
    input.snapshot.pods[0].host_path_mounts[0].path = FROM_THE_CLUSTER.to_string();
    let analysis = reports(&input.snapshot, &analyze(&input.snapshot));

    // **`--logs`**, both arms, through the one reader that produces a line for either
    // (`k8s::read_lines`).
    let mut held = k8s::LogLines::default();
    let mut followed = Vec::new();
    // **`LogSocket::over` and not the bytes**, because `k8s::log_stream` hands back a socket
    // nothing outside `k8s.rs` can decode itself and this constructor is `#[cfg(test)]` — the
    // hole a review found by rewriting the `--follow` arm to read the stream by hand
    // (`k8s-admin`, 2026-08-31).
    k8s::read_lines(k8s::LogSocket::over(FROM_THE_CLUSTER.as_bytes()), |line| {
        followed.push(line.clone());
        held.push(line);
        true
    })
    .await
    .expect("a slice never fails a read");
    let mut fetched = Vec::new();
    dump(&held, &mut fetched).expect("a Vec never refuses a write");
    let fetched = String::from_utf8(fetched).expect("k8rs writes UTF-8");

    // **`--describe`**, where a controller's sentence is the second line of an event's block.
    let pod = pod_read("healthy").await;
    let described = described(
        &pod,
        Some(&k8s::Happened {
            lines: vec![happening(
                Some("2026-08-30T21:35:41Z"),
                "Unhealthy",
                FROM_THE_CLUSTER,
            )],
            cut: false,
        }),
        &now(),
    );

    for (path, printed) in [
        ("--once", &once),
        ("--analysis", &analysis),
        ("--logs (fetched)", &fetched),
        ("--describe", &described),
        // **The `k8rs: …` sentence every failing run ends on**, spelled once by `about`.
        (
            "the stderr sentence",
            &about("pod", FROM_THE_CLUSTER, Some("payments")),
        ),
        // **The command log, which is the one line a reader copies and runs** (invariant 4).
        //
        // **In the `qualified` slot and not the `name` one, which is where this was wrong.**
        // `--object`'s name goes through `k8s::object_name` before any of this, so a name can
        // never carry an `ESC` and feeding one there tested a case production forbids;
        // `qualified` is built from `Browsable::kind` and `Browsable::group`, which are the
        // cluster's words (`k8s-admin`, 2026-08-31). `Fetch::table` now refuses a kind that is
        // not `path_safe`, so this slot is guarded upstream too — and this assertion is about
        // what the *printer* does to whatever reaches it, which is a different question and the
        // one the box asked.
        (
            "the kubectl line",
            &kubectl_get(FROM_THE_CLUSTER, "web", Some("payments")),
        ),
    ] {
        println!("--- {path} ---\n{printed}");
        assert!(
            printed.contains(AFTER_ONE_STRIP),
            "{path} did not carry the line out whole: something wrapped it, cut it, padded inside \
             it or stripped it a second time — {printed:?}"
        );
    }

    // **The two log arms are the payload itself**, so they are asserted whole rather than found
    // inside something.
    assert_eq!(
        followed,
        vec![AFTER_ONE_STRIP.to_string()],
        "the followed log line is not the container's line with one strip on it"
    );
    assert_eq!(
        fetched,
        format!("{AFTER_ONE_STRIP}\n"),
        "the fetched log is not the container's line with one strip and the newline that ends it"
    );
}
