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
/// (NOTES § D17). The text has to name every door to a cluster this build has, because the name
/// promises one and a reader decides here whether it is safe to try against production.
///
/// **The claim outlived two spellings of the sentence that carries it.** It was *this build
/// cannot reach a cluster* until `ops` landed, then *without --once, --live, --logs, --describe,
/// --yaml or ops this build reads files only* until the console did — a bare `k8rs` reaches a
/// cluster with none of those six words on it (`screens/states.md` § The command line's own
/// synopsis). What has not changed is what the test is for: the **list** is complete, so nothing
/// that reaches a cluster is missing from the one page a reader checks.
#[test]
fn no_arguments_is_the_usage_text_and_not_a_report() {
    let Err(problem) = run(&[]) else {
        panic!("no arguments is not a report")
    };

    assert!(problem.starts_with("usage: k8rs "), "{problem}");
    assert!(
        problem.contains(
            "--once, --live, --logs, --describe, --yaml and ops are its other doors to a cluster"
        ),
        "{problem}"
    );
    // **And the console, which is the door with no word on it** — the half that sentence had to
    // be rewritten to admit.
    assert!(
        problem.contains("without one, this build opens a console instead of reading nothing"),
        "{problem}"
    );
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

/// **A word of argv that is not text is refused, and the process does not panic** — the defect
/// this replaced was `std::env::args()`'s own `unwrap`, which ended a run at exit `101` with a
/// Rust backtrace on stderr and no sentence at all ([`crate::command_line`]).
///
/// **Every placement of the bad byte, at every position on the line** (NOTES § D29): the shell
/// hands over whole words and k8rs does not get to choose where inside one the byte sits — a
/// latin-1 filename has it in the middle, a `--` flag has it at the end, and a word can be
/// nothing but bad bytes. The position it is named by is counted from the program name, so a
/// word at argv index `i` after the skip is word `i + 2`.
#[test]
fn a_word_that_is_not_text_is_refused_by_the_position_it_was_typed_at() {
    use std::os::unix::ffi::OsStringExt;

    // `\xff` is never a byte of UTF-8 at all; `\xe9` is a lead byte with its continuations
    // missing, which is what latin-1 `é` and a filename cut at a length limit both look like.
    let bad: [&[u8]; 5] = [
        b"\xff",
        b"\xffhead",
        b"tail\xff",
        b"mid\xffdle",
        b"caf\xe9.json",
    ];
    for bytes in bad {
        for at in 0..3 {
            let mut line: Vec<std::ffi::OsString> =
                vec!["--once".into(), "--analysis".into(), "pods.json".into()];
            line[at] = std::ffi::OsString::from_vec(bytes.to_vec());
            let said = command_line(line.into_iter())
                .expect_err(&format!("{bytes:?} at {at} was read as text"));

            assert!(said.starts_with("k8rs: "), "{said}");
            assert!(
                said.contains(&format!("word {} of the command line", at + 2)),
                "the wrong word was named for {bytes:?} at {at}: {said}"
            );
        }
    }
}

/// **The bytes are the one thing the sentence may not carry**, which is [`crate::as_typed`]'s
/// ruling one layer out (NOTES § D224): a word k8rs could not read is not offered back.
///
/// **`\u{FFFD}` is asserted by name because it is what the rejected design produces.**
/// `to_string_lossy` refuses nothing at all — the run carries on over a path or a flag nobody
/// typed — and if it were ever spelled back into a sentence, a test that only checked the
/// sentence was printable would pass on it: `\u{FFFD}` is not a control character and
/// [`crate::sanitize`] leaves it alone.
///
/// **The first bad word is the one named**, the same way [`crate::mistyped`] names the first
/// flag it does not have: two of them is still one sentence.
#[test]
fn the_refusal_carries_no_part_of_the_word_and_names_the_first_bad_one() {
    use std::os::unix::ffi::OsStringExt;

    let said = command_line(
        [
            std::ffi::OsString::from("--once"),
            std::ffi::OsString::from_vec(b"first\xff".to_vec()),
            std::ffi::OsString::from_vec(b"second\xfe".to_vec()),
        ]
        .into_iter(),
    )
    .expect_err("two words that are not text were read as text");

    assert!(said.contains("word 3 of the command line"), "{said}");
    assert!(!said.contains("word 4"), "both words were named: {said}");
    assert!(
        !said.contains("first") && !said.contains("second"),
        "the word k8rs could not read was quoted back: {said}"
    );
    assert!(
        !said.contains('\u{FFFD}'),
        "the word was converted instead of refused: {said}"
    );
    // Nothing outside k8rs reaches this sentence, so the strip has nothing to take out of it
    // (invariant 9, [`crate::sanitize`]).
    assert_eq!(sanitize(&said), said, "{said}");
}

/// **The healthy half** (CLAUDE.md § Tests must not lie): a line that is text comes through word
/// for word, in order, and nothing about it is refused.
///
/// **The shapes are the ones the shell really produces** (NOTES § D29): no arguments at all,
/// an empty word — `k8rs ""` is a legal command line — a flag, a path, and a name that is
/// multibyte UTF-8, which is text and must not be mistaken for the case above.
#[test]
fn a_command_line_that_is_text_comes_through_word_for_word() {
    assert_eq!(command_line(std::iter::empty()), Ok(vec![]));

    let words = ["--once", "", "-n", "ödeme", "pods.json", "--"];
    assert_eq!(
        command_line(words.iter().map(std::ffi::OsString::from)),
        Ok(words.iter().map(|w| (*w).to_string()).collect::<Vec<_>>())
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

/// **A store whose first LIST has not landed** — `k8s::Store::snapshot` answers `None` for one, so
/// [`vanished`] answers *k8rs cannot tell* and D22's guard is off (NOTES § D289 ruling 1).
///
/// **Every key-press test that is not about that guard is handed this one**, which is what keeps
/// them about the key they press. The tests that *are* about it build a listed store with
/// [`listed`], where an absent `uid` means absent.
fn before_the_list() -> k8s::Store {
    k8s::Store::default()
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
    // **Last-wins, which is `kubectl`'s rule** — it was first-wins, and the flags box that
    // released this flag is the box both parsers' docs had deferred the fix to
    // (`k8s-admin`, 2026-09-24; [`context_arg`], which carries the wrapper it turns on).
    assert_eq!(
        live_context(&args(&["--live", "--context", "a", "--context", "b"])),
        Some(Some("b"))
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

    // **Last-wins on repeats, across the two spellings**, which is `kubectl`'s rule and the one
    // `alias kp='k8rs -n a'` needs to be overridable ([`value_of`]).
    assert_eq!(
        live_namespace(&args(&["--live", "-n", "a", "--namespace", "b"])),
        Some("b")
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

/// **One parser for `--context`, and the console reads it on a line [`live_context`] is silent
/// about** (todo.md § Phase 12's flags box).
///
/// **The two functions had to be split and may not drift**: `live_context` answers `None` for
/// every line without `--once`, `--live` or a verb on it, which is exactly the line that opens a
/// console — so the console needed an answer that function cannot give, and a second parser for
/// it is how this repo has already shipped a silent wrong cluster three times (that function's
/// own doc). Every spelling below goes through both.
#[test]
fn the_context_flag_has_one_parser_and_a_console_line_reads_the_same_answer() {
    let line = |words: &[&str]| -> Vec<String> { words.iter().map(|w| (*w).to_string()).collect() };

    // The line `live_context` says nothing about is the line the console is opened by.
    assert_eq!(live_context(&line(&["--context", "prod-eu"])), None);
    assert_eq!(
        context_arg(&line(&["--context", "prod-eu"])),
        Some(Some("prod-eu"))
    );

    for (words, answer) in [
        (vec!["--context", "kind-k8rs"], Some(Some("kind-k8rs"))),
        (vec!["--context=kind-k8rs"], Some(Some("kind-k8rs"))),
        // `--context=` keeps the empty value rather than quietly becoming the current context.
        (vec!["--context="], Some(Some(""))),
        (vec!["--context"], Some(None)),
        // A flag is never a context name — the second line, behind [`mistyped`]'s sentence.
        (vec!["--context", "--live"], Some(None)),
        // An `=` says the value was meant, so a flag-shaped one after it is kept.
        (vec!["--context=--live"], Some(Some("--live"))),
        // Last-wins, which is `kubectl`'s rule — it was first-wins until the flags box.
        (vec!["--context", "a", "--context", "b"], Some(Some("b"))),
        // A longer flag that merely starts the same way is not this one.
        (vec!["--contextual", "x"], None),
        (vec![], None),
    ] {
        assert_eq!(context_arg(&line(&words)), answer, "{words:?}");
        // **The same line under `--live` answers the same context**, which is what one parser
        // buys: a console and the temporary driver cannot come to connect to two clusters off
        // one spelling.
        let driven: Vec<&str> = std::iter::once("--live")
            .chain(words.iter().copied())
            .collect();
        assert_eq!(
            live_context(&line(&driven)),
            Some(answer.flatten()),
            "{driven:?}"
        );
    }
}

/// **Which lines open the console, and what the four flags on them asked for**
/// (todo.md § Phase 12's flags box).
///
/// **The premise is the defect, measured at the commit before this one**: `main`'s console arm
/// asked `args.is_empty()`, so `k8rs --read-only` fell past it into the file-driven report, read
/// `--read-only` as a path, and dropped the one flag on the line that may not be dropped in
/// silence. Every row here is a line that reached the wrong driver.
#[test]
fn every_console_flag_opens_a_console_and_arrives_as_what_it_asked_for() {
    let line = |words: &[&str]| -> Vec<String> { words.iter().map(|w| (*w).to_string()).collect() };

    // A bare `k8rs` is still a console, with nothing asked for.
    let nothing = line(&[]);
    let bare = opening(&nothing).expect("a bare k8rs opens the console");
    assert!(!bare.read_only, "a bare k8rs is not read-only");
    assert_eq!(bare.context, None, "a bare k8rs names no context");
    assert_eq!(bare.namespace, None, "a bare k8rs names no namespace");

    for (words, read_only, context, namespace) in [
        (vec!["--read-only"], true, None, None),
        (vec!["--context", "prod-eu"], false, Some("prod-eu"), None),
        (vec!["--context=prod-eu"], false, Some("prod-eu"), None),
        (
            vec!["--namespace", "payments"],
            false,
            None,
            Some("payments"),
        ),
        (vec!["--namespace=payments"], false, None, Some("payments")),
        (vec!["-n", "payments"], false, None, Some("payments")),
        (vec!["-n=payments"], false, None, Some("payments")),
        // All three at once, in either order — the scan is over the whole line.
        (
            vec!["--read-only", "--context=prod-eu", "-n", "payments"],
            true,
            Some("prod-eu"),
            Some("payments"),
        ),
        (
            vec!["-n=payments", "--context", "prod-eu", "--read-only"],
            true,
            Some("prod-eu"),
            Some("payments"),
        ),
    ] {
        let typed = line(&words);
        let opened = opening(&typed)
            .unwrap_or_else(|| panic!("{words:?} opened no console, so its flags were dropped"));
        assert_eq!(
            opened.read_only, read_only,
            "{words:?} answered the wrong thing about --read-only"
        );
        assert_eq!(
            opened.context, context,
            "{words:?} would connect to the wrong context"
        );
        assert_eq!(
            opened.namespace, namespace,
            "{words:?} would watch the wrong namespace"
        );
    }
}

/// **`--once` keeps the temporary driver, and so does every other line that is not a console
/// line** (todo.md § Phase 12, NOTES § D17).
///
/// **`--once` is the row that matters**, because it is released: a console opened under it would
/// replace a report on stdout with a screen nothing in a pipeline can read, and the exit code
/// `screens/once.md` sells would go with it.
#[test]
fn a_cluster_flag_a_verb_or_a_file_never_opens_a_console() {
    let line = |words: &[&str]| -> Vec<String> { words.iter().map(|w| (*w).to_string()).collect() };

    // **Each row names the driver it keeps, and not merely *not the console***: a build where
    // [`opening`] answered `None` to everything would pass a test that asserted only the
    // absence — and that build is exactly the one this box replaced.
    for (words, cluster) in [
        (vec!["--once"], true),
        (vec!["--once", "--read-only"], true),
        (
            vec!["--once", "--context", "prod-eu", "-n", "payments"],
            true,
        ),
        (vec!["--live"], true),
        (vec!["--live", "--namespace=payments"], true),
        (vec!["--logs", "--object", "default/web"], true),
        (vec!["--describe", "--object", "default/web"], true),
        (vec!["--yaml", "--object", "default/web"], true),
        // A file, with and without the one flag that is neither the console's nor a cluster's.
        (vec!["pod.json"], false),
        (vec!["--analysis", "pod.json"], false),
        (vec!["--analysis"], false),
        (vec!["-x", "file.json"], false),
    ] {
        let typed = line(&words);
        assert!(
            opening(&typed).is_none(),
            "{words:?} opened a console instead of keeping the driver it named"
        );
        assert_eq!(
            live_context(&typed).is_some(),
            cluster,
            "{words:?} did not keep the driver it named — `live_context` answered the other one"
        );
        // And nothing on these lines is refused on the way, so the driver is really reached.
        assert_eq!(mistyped(&typed), None, "{words:?}");
    }
}

/// **The header's right zone says what the run is scoped to, and not only which cluster**
/// (`screens/widgets.md` § 1a's zone table: *context · namespace scope · connection state · …*).
///
/// **Measured before the fix, against the kind cluster**: `k8rs -n kube-system` drew
/// `ctx: kind-k8rs · live · admin` — the same header a cluster-wide run draws — while the Alerts
/// pane held `1 ● 1 ▲` instead of `25 ● 2 ▲`. A reader who cannot see the scope reads
/// `○ nothing is broken` over one namespace as a statement about the cluster, which is
/// `screens/states.md` § You can only see some namespaces' whole subject.
///
/// **One zone for both causes** (`k8s::Coverage`, NOTES § D46): `--namespace` and the 403 fallback
/// produce the same scope, so they produce the same header and nothing here asks which it was.
#[test]
fn the_header_zone_names_the_namespace_a_scoped_run_covers() {
    assert_eq!(zone(Some("prod-eu"), None), "ctx: prod-eu");
    assert_eq!(
        zone(Some("prod-eu"), Some("payments")),
        "ctx: prod-eu · ns: payments"
    );
    // **The cluster's name comes first and the scope second**, which is not cosmetic: `shortened`
    // eats the *front* of this zone, so what erodes under a narrow terminal is the context and
    // never the scope or the permission word behind it (NOTES § D249).
    assert!(
        zone(Some("prod-eu"), Some("payments")).starts_with("ctx: "),
        "the scope displaced the cluster's name, so the wrong end erodes"
    );
    // **`(unnamed)` is the one thing a `None` context can be** (NOTES § D202) — and it still
    // carries the scope, because the two facts are independent.
    assert_eq!(
        zone(None, None),
        format!("ctx: {}", views::UNNAMED),
        "a context that stripped to nothing lost its slot"
    );
    assert_eq!(
        zone(None, Some("payments")),
        format!("ctx: {} · ns: payments", views::UNNAMED)
    );
    // The label the pane title carries is this same spelling and not a second one
    // (`ui::Screen::namespace`).
    assert!(zone(Some("prod-eu"), Some("payments")).ends_with(&scoped("payments")));
}

/// **`--read-only` does not open the audit log at all**, and that is not NOTES § D21 weakened
/// (PM ruling, todo.md § Phase 12's flags box).
///
/// **The count is the assertion and the answer is not.** `ui::Writes::ReadOnly` reads the same
/// whether the file was opened and then ignored or never opened — but a run that opened it would
/// have created it, set its mode, and printed `ops::audit_log`'s notes about a log it will never
/// write a line to. And a run that opened it and *failed* would carry `Writes::Unaudited`, whose
/// sentence — *fix that, then start k8rs again* — is false advice while the flag stands.
#[test]
fn read_only_never_opens_the_audit_log_and_every_other_run_does() {
    let opens = std::cell::Cell::new(0);

    assert!(
        audit_log_for(true, || {
            opens.set(opens.get() + 1);
            Ok::<(), String>(())
        })
        .is_none(),
        "--read-only was handed an audit log it can never write a line to"
    );
    assert_eq!(opens.get(), 0, "--read-only opened the audit log anyway");

    assert!(
        audit_log_for(true, || {
            opens.set(opens.get() + 1);
            Err::<(), String>("no state directory".to_string())
        })
        .is_none(),
        "--read-only carries the refusal of a log it will never use"
    );
    assert_eq!(
        opens.get(),
        0,
        "--read-only opened the audit log to find out why it would not open"
    );

    assert!(
        matches!(
            audit_log_for(false, || {
                opens.set(opens.get() + 1);
                Ok::<(), String>(())
            }),
            Some(Ok(()))
        ),
        "a run that may write did not get its log"
    );
    assert_eq!(opens.get(), 1, "a run that may write did not open its log");

    // **A log that will not open is still a refusal to carry** — a banner and every write key
    // dead — for the run that could have written.
    assert!(matches!(
        audit_log_for(false, || Err::<(), String>("no state directory".to_string())),
        Some(Err(said)) if said == "no state directory"
    ));
}

/// **A path beside a console flag is refused, never read with the flag silently dropped** —
/// [`mistyped`]'s *a cluster and a file are two inputs* rule, one door over (todo.md § Phase 12's
/// flags box, NOTES § D189).
///
/// **Measured at the commit before this one**: `k8rs --read-only pod.json` passed `mistyped`,
/// missed the console arm for not being a bare `k8rs`, and printed the file's report with
/// `--read-only` gone and nothing on any stream to say so.
///
/// **The sentence names the flag that is on the line and not a mode that is not** (NOTES § D190's
/// class): a run with no `--live` on it may not be told that `--live` reads a cluster.
///
/// **The subject claims membership and nothing else, and two rounds of it were wrong**
/// (`k8s-admin` and `tester`, 2026-09-24, invariant 14). *"--read-only opens the console"* was
/// true of the run and false of the flag — `k8rs --read-only ops delete …` opens none.
/// *"--read-only on its own opens the console"* was true of the flag and false of a line carrying
/// two more flags, which is the row set below that let it ship: every row fed **one** console
/// flag, so nothing here could see the word `on its own` become false.
///
/// **Both halves are rows now**: a line with `ops` on it, and lines with two and three console
/// flags beside the path.
#[test]
fn a_path_beside_a_console_flag_is_refused_rather_than_read_with_the_flag_dropped() {
    let line = |words: &[&str]| -> Vec<String> { words.iter().map(|w| (*w).to_string()).collect() };

    for (words, flag) in [
        (vec!["--read-only", "pod.json"], "--read-only"),
        (vec!["--context", "prod-eu", "pod.json"], "--context"),
        (vec!["--context=prod-eu", "pod.json"], "--context"),
        (vec!["--namespace", "payments", "pod.json"], "--namespace"),
        (vec!["--namespace=payments", "pod.json"], "--namespace"),
        (vec!["-n", "payments", "pod.json"], "-n"),
        (vec!["-n=payments", "pod.json"], "-n"),
        // **Two and three console flags beside the path** — the rows the last round did not
        // have, and the gap a clause claiming *on its own* sailed through
        // (`tester`, 2026-09-24). The flag named is the first on the line, whichever it is.
        (
            vec!["--read-only", "--context", "prod-eu", "pod.json"],
            "--read-only",
        ),
        (
            vec![
                "--namespace=payments",
                "--context=prod-eu",
                "--read-only",
                "pod.json",
            ],
            "--namespace",
        ),
        (
            vec![
                "-n",
                "payments",
                "--read-only",
                "--context=prod-eu",
                "pod.json",
            ],
            "-n",
        ),
        (
            vec![
                "--context",
                "prod-eu",
                "-n=payments",
                "--read-only",
                "pod.json",
            ],
            "--context",
        ),
    ] {
        let problem = mistyped(&line(&words))
            .unwrap_or_else(|| panic!("{words:?} was accepted, so {flag} is dropped in silence"));
        assert_eq!(
            problem,
            format!(
                "k8rs: {flag} belongs to the console, which reads a cluster, so k8rs cannot \
                 also read pod.json — run it with the flag, or with the file, not both\n{USAGE}"
            ),
            "{words:?}"
        );
    }

    // **What is echoed is the flag and never the word that carried it** (the security gate's
    // *sizes are bounded* row). `--context`'s value is the one word on a console line nothing has
    // bounded — `--namespace`'s is checked against `k8s::namespace_name` further up — so a
    // 64 KiB context name must not come back out in the refusal.
    let enormous = format!("--context={}", "c".repeat(65536));
    let problem = mistyped(&line(&[enormous.as_str(), "pod.json"]))
        .expect("a path beside --context is refused");
    assert!(
        problem.starts_with("k8rs: --context belongs to the console, which reads a cluster"),
        "{problem}"
    );
    assert!(
        !problem.contains("cccc"),
        "the refusal printed the context name back at {} bytes",
        problem.len()
    );

    // **The subject may not claim the flag opens a console, because `ops` is a line where it
    // does not** (invariant 14). A reader who forgot `ops` in front of `may-i` is told about a
    // console, so the rule they carry away has to be one that holds everywhere.
    let forgot = mistyped(&line(&["--read-only", "may-i", "get", "pods"]))
        .expect("a bare `may-i` is a path beside a console flag");
    assert!(
        forgot.starts_with("k8rs: --read-only belongs to the console, which reads a cluster"),
        "{forgot}"
    );
    for claim in ["opens the console", "on its own"] {
        assert!(
            !forgot.contains(claim),
            "the subject claims {claim:?}, which `k8rs --read-only ops …` falsifies: {forgot}"
        );
    }
    // And the line that proves the claim: the same flag in front of `ops` opens no console,
    // because [`ops_line`] takes the line before [`mistyped`] or [`opening`] is ever asked.
    let claimed = ops_line(
        &line(&["--read-only", "ops", "delete", "deployment/web"]),
        nowhere,
        unwired,
        unanswered,
    );
    assert!(
        claimed.is_some(),
        "`--read-only ops …` fell through to the console arm, so the clause would be false"
    );

    // **A one-dash word this build does not have is a usage error on a console line too**, which
    // is the sentence `k8rs --once -o json` already gets.
    let problem =
        mistyped(&line(&["--read-only", "-o", "json"])).expect("-o is not a flag k8rs has");
    assert!(
        problem.starts_with("k8rs: -o is not a flag k8rs has"),
        "{problem}"
    );

    // **`k8rs -x file.json` with no console flag on it is still a path** — [`NAMESPACE_SHORT`]'s
    // doc promises it, and the flags box does not take it away.
    assert_eq!(mistyped(&line(&["-x", "file.json"])), None);
    assert_eq!(mistyped(&line(&["pod.json"])), None);
    assert_eq!(mistyped(&line(&["--analysis", "pod.json"])), None);
    // A console line with no path on it is not refused either.
    assert_eq!(mistyped(&line(&["--read-only"])), None);
    assert_eq!(
        mistyped(&line(&["--context", "prod-eu", "-n", "pay"])),
        None
    );

    // **The cluster flags keep their own subject**, which is the whole of why the gate and the
    // sentence are one function ([`cluster_reader`]).
    for (words, subject) in [
        (vec!["--once", "pod.json"], "--once"),
        (vec!["--live", "pod.json"], "--live"),
        (
            vec!["--logs", "--object", "default/web", "pod.json"],
            "--logs",
        ),
    ] {
        let problem = mistyped(&line(&words)).unwrap_or_else(|| panic!("{words:?} was accepted"));
        assert!(
            problem.starts_with(&format!("k8rs: {subject} reads a cluster,")),
            "{words:?} → {problem}"
        );
    }
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
    k8s::drive_watching(watches, Vec::new(), &mut store, |_| {}).await;

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
    k8s::drive_watching(watches, Vec::new(), &mut store, |_| {}).await;
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
    k8s::drive_watching(watches, Vec::new(), store, |_| {}).await;
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
/// **Only five of the eleven use `asked`, and that is deliberate.** The three kubeconfig faults,
/// a login helper that answered nothing, and a `409` about an object rather than about a request
/// all happened somewhere a verb and a resource would be invented rather than reported.
///
/// **All eleven, and it was seven of nine until 2026-08-30** — `NoContext` and `BadEntry` had
/// never been in this list, so the one test whose whole claim is *no two collapse* could not have
/// seen those two collapse (`dev-core`'s own second pass). `k8s::Fault::Unfinished` is the tenth,
/// and it is the one this list matters most for: it is a hair from `Unanswered` — a server that
/// answered nothing against a connection that carried nothing — and the two send a reader to
/// opposite places. `k8s::Fault::Conflict` is the eleventh (NOTES § D213).
#[test]
fn every_fault_gets_its_own_sentence_and_none_of_them_stands_in_for_another() {
    use k8s::Fault::{
        BadEntry, Conflict, Expired, Gone, Kubeconfig, NoContext, NoCredential, Refused, Rejected,
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
        Conflict,
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
         `current-context` line in the file, and any `--context` on the command line"
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

/// **A `--once` report waits for the headings it is about to count** (`analysis.rs`'s capacity
/// row), and for nothing else it is not already waiting for.
///
/// **The second assertion is the one that shipped wrong.** With no fetcher wired, every pass had
/// `0` outstanding by construction and the report printed with two ReplicaSets of one Deployment
/// counted as two workloads.
#[tokio::test]
async fn a_once_report_waits_for_a_heading_that_is_still_on_its_way() {
    let mut listing = k8s::Store::default();
    listing.pod(&now(), Event::Init);
    the_other_four(&mut listing);
    assert!(
        !ready_to_report(&listing, 0),
        "a pass whose pod LIST has not landed printed a report about a cluster it had not read"
    );

    let landed = listed(objects::<Pod>("owned-pods.json"));
    assert!(
        !ready_to_report(&landed, 1),
        "the report printed while a heading was still on its way, which is the count in it \
         being wrong"
    );
    assert!(ready_to_report(&landed, 0));
}

/// **Every unresolved ReplicaSet is asked about, and each of them once** — the caller
/// `k8s.rs` § RESOLVING AN OWNER described for a phase and this file never was.
///
/// **The count is what `--once` waits on**, so it is asserted as well as the traffic: a run that
/// printed while a heading was still in flight is `analysis.rs`'s capacity row counting two
/// ReplicaSets of one Deployment as two workloads.
///
/// **The second pass is the assertion that matters.** An answer can be minutes away
/// (NOTES § D148) and the store has no *pending* state — an unanswered reference reads exactly
/// like a never-asked one — so a caller with no memory sends one `get` per reference per watch
/// event, which is a storm-rate retry loop against a cluster that is already slow.
#[tokio::test]
async fn one_get_per_unresolved_owner_however_many_events_go_past() {
    let mut store = listed(objects::<Pod>("owned-pods.json"));
    let (asking, mut wanted) = tokio::sync::mpsc::unbounded_channel();
    let mut asked = std::collections::BTreeSet::new();

    assert_eq!(
        ask_owners(&store, &mut asked, &asking),
        1,
        "the capture's pod names one ReplicaSet and nothing asked about it"
    );
    let first = wanted.try_recv().expect("one reference on its way");
    println!("{first:?}");
    assert_eq!(
        (first.name.as_str(), first.namespace.as_deref()),
        ("broken-owned-7bdb7645c8", Some("default")),
        "the fetch was pointed at something other than the pod's own controller"
    );

    // Every event of a storm, while the answer is still in flight.
    for _ in 0..50 {
        assert_eq!(
            ask_owners(&store, &mut asked, &asking),
            1,
            "a reference nobody has answered for stopped being counted as outstanding"
        );
    }
    assert!(
        wanted.try_recv().is_err(),
        "one reference was asked about twice, which is one `get` per event per pod"
    );

    // The answer lands, and the run has nothing left to wait for.
    store.owner_fetched(
        &first,
        Ok(objects::<k8s_openapi::api::apps::v1::ReplicaSet>("owned-replicasets.json").remove(0)),
    );
    assert_eq!(ask_owners(&store, &mut asked, &asking), 0);
    assert!(
        asked.is_empty(),
        "the set of references already asked about keeps a uid nothing is waiting on any more, \
         so a process that runs for a month remembers every ReplicaSet a rollout ever made"
    );
    assert!(
        wanted.try_recv().is_err(),
        "an answered reference was re-asked"
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

/// **Every line k8rs shows carries `--context <name>`, immediately after `kubectl`**
/// (`screens/context.md` § What the command log shows, invariant 4, NOTES § D8).
///
/// **Measured before the fix**: `grep -c -- --context` over every `$ kubectl` literal in
/// `src/main.rs` was **0** (`k8s-admin`, 2026-09-24). A reader who pastes
/// `kubectl get pods -A --watch` gets their own `current-context`, which is a different cluster
/// the moment k8rs was started with `--context` — the line teaches a command that reads somewhere
/// else and says nothing about it.
///
/// **Not only the lines after a switch.** `screens/context.md` writes the rule for the switcher,
/// and its reason — *honest, and it teaches the flag that makes `kubectl` safe to use across
/// clusters* — is about the paste reproducing the read, which is every line's problem.
#[test]
fn every_kubectl_line_names_the_context_it_was_read_from() {
    // **The connected context and not the flag**, so a run that typed none still teaches it.
    assert_eq!(kubectl(Some("staging")), "$ kubectl --context staging");
    // **The gap drops the segment whole** — NOTES § D202's third state, a name that stripped to
    // nothing. `--context ` with an empty value is a line that does not run.
    assert_eq!(kubectl(None), "$ kubectl");
    assert_eq!(kubectl(Some("")), "$ kubectl");
    // Invariant 9, on a name argv never touched but a kubeconfig can hold.
    assert_eq!(
        kubectl(Some("prod\u{202e}eu")),
        "$ kubectl --context prodeu",
        "a bidi override in a context name reached the strip that every drawn string meets"
    );

    // **A name made *only* of characters the strip removes is the gap, not an empty flag**
    // (`k8s-admin`'s step-7 pass, 2026-09-24). The emptiness test used to run on the raw
    // name: it passed, `sanitize` then emptied it, and `ops::pasteable` quoted the nothing —
    // `$ kubectl --context ''`, which is the empty-valued flag this function's own doc
    // refuses two lines up and a line that does not run. `ops::context_segment` had the same
    // hole and was fixed by cleaning first; this is that order, so the two taught surfaces
    // cannot disagree on the one input their shared quoting rule exists for.
    //
    // **Every class `k8s::unprintable` covers**, because one of them is not proof of the
    // others (NOTES § D29): a control character, the soft hyphen, the zero-width and bidi
    // ranges, the invisible-operator range, and the byte-order mark.
    for stripped in [
        "\u{202e}",
        "\u{7}",
        "\u{ad}",
        "\u{200b}",
        "\u{200f}",
        "\u{202a}",
        "\u{2060}",
        "\u{206f}",
        "\u{feff}",
        "\u{202e}\u{200b}\u{feff}",
    ] {
        assert_eq!(
            kubectl(Some(stripped)),
            "$ kubectl",
            "{stripped:?} stripped to nothing and was drawn as a flag with no value"
        );
    }
    // And the two spellings of nothing that never reach the strip at all.
    assert_eq!(kubectl(None), "$ kubectl");
    assert_eq!(kubectl(Some("")), "$ kubectl");

    // **A name the shell would not read whole is quoted, and the line stays the command k8rs
    // ran** (invariant 4, `ops::pasteable`, NOTES § D278 ruling 5 — one rule for this surface
    // and for the three taught mutation lines). **Measured against the real binary before it**:
    // `kubectl config rename-context kind-k8rs 'prod eu; echo pwned'` and then `k8rs --once`
    // printed `$ kubectl --context prod eu; echo pwned get --raw /version` — a line the command
    // log exists to have pasted, doing something else when it is. k8rs executes nothing either
    // way; what it may not do is teach a line that is not the one it ran.
    //
    // **The ordinary names stay bare**, so the line is still the one `screens/context.md` draws.
    for plain in [
        "kind-k8rs",
        "staging",
        "gke_my-project_europe-west1_prod",
        "arn:aws:eks:eu-west-1:123456789012:cluster/production-eu",
    ] {
        assert_eq!(
            kubectl(Some(plain)),
            format!("$ kubectl --context {plain}"),
            "an ordinary context name was quoted, so the taught line stopped being the drawn one"
        );
    }
    assert_eq!(
        kubectl(Some("prod eu; echo pwned")),
        "$ kubectl --context 'prod eu; echo pwned'",
        "a context name with shell syntax in it is pasted as syntax"
    );
    // A `'` is the one character single quotes cannot hold, and this is the standard closing,
    // escaping and reopening.
    assert_eq!(
        kubectl(Some("it's prod")),
        r"$ kubectl --context 'it'\''s prod'",
        "a quote inside the name ended the quoting early"
    );
    // **Every shape the allowlist refuses is quoted and not guessed at** — the rule is what is
    // safe bare, not a list of what is dangerous, so a character nobody thought of is quoted.
    for hostile in [
        "a b", "a;b", "a|b", "a&b", "a$b", "a`b", "a>b", "a(b", "a*b", "a\\b", "a\"b", "a#b",
        "a!b", "a~b",
    ] {
        let line = kubectl(Some(hostile));
        assert!(
            line.starts_with("$ kubectl --context '") && line.ends_with('\''),
            "{hostile:?} was left bare on a line meant to be pasted: {line:?}"
        );
    }

    // **Every line, not the first one** — the probe, the version, discovery, the seven reports
    // and the five watches all come out of one head.
    let lines = command_log(
        true,
        &kubectl(Some("staging")),
        &k8s::Coverage::Cluster,
        None,
    );
    assert!(lines.len() >= 12, "the log shrank: {lines:?}");
    for line in &lines {
        assert!(
            line.starts_with("$ kubectl --context staging "),
            "a line k8rs shows would paste into a different cluster: {line:?}"
        );
    }
    // **The position is the spelling `kubectl` accepts**: a global flag goes before the verb, so
    // `kubectl --context staging get …` and never `kubectl get --context staging …`.
    assert!(
        lines
            .iter()
            .any(|line| line == "$ kubectl --context staging get pods -A --watch"),
        "the watch line is not the one `screens/context.md` draws: {lines:?}"
    );
    assert!(
        lines.iter().all(|line| !line.contains("get --context")),
        "the flag landed after the verb, where kubectl does not take it: {lines:?}"
    );

    // And the same log with no context named is byte-identical to what it printed before.
    let bare = command_log(true, &kubectl(None), &k8s::Coverage::Cluster, None);
    assert_eq!(bare.join("\n"), ANALYSIS_LOG);
}

/// **A bare run prints one line per read, in the order the code starts them** — and the
/// `--analysis` run prints the seven a report fetches between discovery and the watches.
#[test]
fn the_command_log_is_every_read_this_run_performs_in_the_order_it_starts_them() {
    let log = |analysis, coverage| command_log(analysis, "$ kubectl", &coverage, None).join("\n");

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
    let scoped = command_log(
        false,
        "$ kubectl",
        &k8s::Coverage::Asked("payments".to_string()),
        None,
    )
    .join("\n");
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

    let reports = command_log(
        true,
        "$ kubectl",
        &k8s::Coverage::Asked("payments".to_string()),
        None,
    );
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
            command_log(false, "$ kubectl", &coverage, Some("payments")).join("\n"),
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
        command_log(false, "$ kubectl", &coverage, context)
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
    let log = command_log(true, "$ kubectl", &crafted, Some("payments"));
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
            for line in command_log(analysis, "$ kubectl", &coverage, None) {
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
    let probe = &command_log(false, "$ kubectl", &k8s::Coverage::Cluster, None)[0];
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
        command_log(true, "$ kubectl", &k8s::Coverage::Cluster, None),
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
        command_log(true, "$ kubectl", &k8s::Coverage::Cluster, None).len(),
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
        command_log(false, "$ kubectl", &k8s::Coverage::Cluster, None),
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
        let attempted = connect_log("$ kubectl", &coverage, context);
        println!("{attempted:?}");
        for analysis in [false, true] {
            assert!(
                command_log(analysis, "$ kubectl", &coverage, context).starts_with(&attempted),
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
    k8s::drive_watching(watches, Vec::new(), &mut store, |_| {}).await;

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
    k8s::drive_watching(watches, Vec::new(), &mut store, |_| {}).await;
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
    // **`--once` runs at a shell, so the flag is the whole next step** — the restart wording is
    // the TUI's, and both contain the flag (NOTES § D264 ruling 23).
    assert!(
        wide.ends_with("or run k8rs in one namespace you can read: --namespace <name>")
            && !wide.contains("start k8rs again"),
        "a run that has already ended was told to quit and start again: {wide:?}"
    );

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
    assert!(
        blind.ends_with("Say which namespace you work in: --namespace <name>")
            && !blind.contains("start k8rs again"),
        "a run that has already ended was told to quit and start again: {blind:?}"
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

/// The command line as the functions under [`crate::command_line`] see it — already text,
/// because everything that was not is refused before any of them is reached.
fn argv(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

/// **One selector for four consumers, in both spellings, last wins** (NOTES § D194,
/// [`value_of`]).
///
/// **Both spellings, for `--context`'s reason**: matching only `--object NAME` lets
/// `--object=NAME` fall through, and a selector that silently selects nothing is worse than one
/// that refuses. **Last wins is written down** because an unwritten tie-break is the one that
/// changes by accident — and because this one moved: it was first-wins until the flags box
/// (`k8s-admin`, 2026-09-24, [`value_of`]).
#[test]
fn the_object_selector_reads_both_spellings_and_takes_the_last() {
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
        Some(Some("second")),
        "a repeated selector did not take the last one, so a correction typed at the end of a \
         recalled command is silently ignored"
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

/// **One flag, one noun.** `--object`'s two refusals said *pod* on a run that named another kind,
/// while every sentence past the connect uses the kind's own singular
/// (`screens/detail.md` § The yaml tab, `read_failed`) — one binary telling a reader two things
/// about the same word (`k8s-admin`, Phase 6 close).
///
/// **`object` and not the word the reader typed**, because at this point in the run there is no
/// cluster and no discovery: `--kind po` and `--kind pods` are both spellings this check cannot
/// turn into a singular, and *names one pods* is a worse sentence than the one it replaces. The
/// kind's own word arrives after `which_kind`, which is where `read_failed` uses it.
#[test]
fn a_selector_refused_on_a_run_that_named_a_kind_does_not_call_it_a_pod() {
    let named = |line: &[&str]| {
        let said = mistyped(&argv(line)).expect("an underscore is not in a name");
        println!("{}", said.lines().next().expect("a first line"));
        said.lines()
            .next()
            .expect("a first line")
            .split(" — ")
            .next()
            .expect("the clause before the rule")
            .to_string()
    };

    assert_eq!(
        named(&["--yaml", "--kind", "node", "--object", "A_B"]),
        "k8rs: --object names one object, written as `<namespace>/<name>` or just `<name>`, and \
         A_B is not one",
    );
    assert_eq!(
        named(&["--yaml", "--kind", "node", "--object", "web/"]),
        "k8rs: --object has nothing after the `/`, so it names no object",
    );
    // **A run that names no kind is a pod run** — `POD` is the default `yaml_run` applies, so the
    // word is not dropped from the surface, only from the lines that cannot know it.
    assert_eq!(
        named(&["--logs", "--object", "A_B"]),
        "k8rs: --object names one pod, written as `<namespace>/<name>` or just `<name>`, and A_B \
         is not one",
    );
    assert_eq!(
        named(&["--yaml", "--object", "web/"]),
        "k8rs: --object has nothing after the `/`, so it names no pod",
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
/// beside a name — `retry` exited `1` with one restart, `keeper` is `running` with none. A list
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
        "k8rs: this pod has 2 containers — retry (failed, 1 restart), keeper (running)\n\
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
        block.contains("retry (failed, 1 restart)"),
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

/// **The picker names the container the reader is looking for**, on the capture the defect was
/// measured on.
///
/// **`neverback.json` is three containers with three different endings** — `broke` exited `1`,
/// `done` exited `0`, `keeper` is still running — and the picker printed *(done)* beside all
/// three of the first two, so the one line whose job is *which log explains this* said the
/// container that failed had finished cleanly (`k8s-admin`, Phase 6 close, against the live pod
/// this capture came from).
#[tokio::test]
async fn the_picker_does_not_call_a_container_that_failed_done() {
    let pod = pod_read("neverback").await;
    let block = container_choice(&pod, None, Some("broke")).expect("three containers");
    println!("{block}");
    assert_eq!(
        block,
        "k8rs: this pod has 3 containers — broke (failed), done (done), keeper (running)\nk8rs: \
         reading broke. Name another with `--container <name>`.",
        "the picker's word for a container is not the word describe gives the same state"
    );
}

/// **What a container is doing, in a word a beginner reads** (invariant 14) — never the API's own
/// `reason`, which is the jargon this product exists to translate.
///
/// **And never a second spelling of [`views::container_state`]'s word.** The picker is the screen
/// where a reader chooses which container's log explains a failed pod, and it printed `done` for a
/// container that exited `1` while describe printed `failed` for the same state — the one word
/// that sends the reader to the wrong log (`k8s-admin`, Phase 6 close).
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
        "keeps crashing and restarting",
        "a waiting container printed the API's own reason code, or one generic word for three \
         different problems"
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
    // **The arm the two functions disagreed on**: a clean exit stays `done` and everything else is
    // `failed`, in the picker as well as in describe.
    assert_eq!(
        doing(Some(&ContainerState::Terminated(
            crate::rules::Terminated {
                reason: Some("Error".to_string()),
                exit_code: 1,
                started_at: None,
                finished_at: None,
                message: None,
            }
        ))),
        "failed",
        "the picker told a reader that the container which failed had finished cleanly"
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
/// **The clause was not enough on its own, and this test is where that shows**
/// ([`as_typed`], 2026-09-05). *"and web (with what cannot print removed) is not one — a name is
/// letters, digits, dashes and dots"* still names a word that satisfies every rule it goes on to
/// state, so the refusal a reader gets no longer quotes it at all. What is left here is
/// [`shown`]\'s own arms, plus the one live refusal that still carries the clause — a word that is
/// *extra*, which [`as_typed`] deliberately does not reach because an extra word is extra whatever
/// it is spelled like.
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

    // And through a sentence a reader actually gets, so the clause reads as English. It is an
    // extra word and not a refused name, because a refused name no longer echoes at all.
    let said = ops(&[
        "ops",
        "scale",
        "deploy/web",
        "3",
        "ex\u{202e}tra",
        "-n",
        "payments",
    ]);
    println!("{}", said.lines().next().expect("a first line"));
    assert!(
        said.contains("do with extra (with what cannot print removed) — it reads"),
        "{said:?}"
    );
    assert!(
        !said.contains('\u{202e}'),
        "the override reached the terminal: {said:?}"
    );

    // **And the sentence the clause was written for no longer needs it**, which is the half a
    // reader of this test would otherwise have to take on trust ([`as_typed`]).
    let name = mistyped(&argv(&["--logs", "--object", "default/we\u{202e}b"]))
        .expect("a bidi override is not in a name");
    println!("{}", name.lines().next().expect("a first line"));
    assert!(
        !name.contains("web (with what cannot print removed)"),
        "a name a cluster would accept was offered back by the sentence refusing it: {name:?}"
    );
    assert!(
        name.contains("the name you typed") && name.contains("does not print"),
        "the reader is not told which word could not be read, or why: {name:?}"
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
/// worth stating rather than discovering: [`views::NO_EVENTS`] goes to stderr, which belongs to
/// the process (§ WATCHING A CLUSTER), so a reversion that printed the wrong line here would still
/// pass. The words are pinned in `views_tests.rs`, by
/// `an_empty_events_read_gets_the_sentence_and_a_full_one_does_not` over [`views::no_events`]
/// (NOTES § D254); what is proven here is that a read which succeeded and found nothing ends the
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
        views::container_state(Some(&ContainerState::Terminated(rules::Terminated {
            reason: reason.map(str::to_string),
            exit_code,
            started_at: None,
            finished_at: None,
            message: None,
        })))
    };
    let waiting = |reason: Option<&str>| {
        views::container_state(Some(&ContainerState::Waiting {
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
        views::container_state(Some(&ContainerState::Running { started_at: None })),
        ("running".to_string(), None)
    );
    assert_eq!(
        views::container_state(None),
        ("not started".to_string(), None)
    );

    // **The log picker is this function's word and not a second spelling of it.** It said `done`
    // about a container that exited `137` while describe said `failed` about the same state, on
    // the one screen where a reader picks which log explains a failed pod (`k8s-admin`, Phase 6
    // close). Every state is checked, not the two that were wrong, because *one word, one reader*
    // is the property and the arms are where a second one grows back.
    for state in [
        Some(ContainerState::Running { started_at: None }),
        Some(ContainerState::Waiting {
            reason: Some("CrashLoopBackOff".to_string()),
            message: None,
        }),
        Some(ContainerState::Waiting {
            reason: None,
            message: None,
        }),
        Some(ContainerState::Terminated(rules::Terminated {
            reason: Some("OOMKilled".to_string()),
            exit_code: 137,
            started_at: None,
            finished_at: None,
            message: None,
        })),
        Some(ContainerState::Terminated(rules::Terminated {
            reason: None,
            exit_code: 0,
            started_at: None,
            finished_at: None,
            message: None,
        })),
        None,
    ] {
        assert_eq!(
            doing(state.as_ref()),
            views::container_state(state.as_ref()).0,
            "the picker and the describe row spell one state two ways again: {state:?}"
        );
    }
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

/// **What the `kubectl` line does not say, said** — the two places where running that command
/// gives a reader something other than what k8rs did (`k8s-admin`, Phase 6 close).
///
/// **A Secret is the one that matters.** `--yaml` masks `data`, `stringData` and every annotation
/// before `k8s::Document` exists and there is no `--reveal` on this surface, and then it printed
/// `kubectl get secret … -o yaml` beside it — the command that prints the base64 values. The
/// security gate's Secrets row holds on the value, which never enters the log; what defeats the
/// masking is the *line*, for the reader who follows the teaching device into a ticket.
///
/// **The namespace is the other.** A cluster-scoped kind drops it, so
/// `--yaml --kind node --object default/k8rs-worker` reads the node and never tells the reader
/// that half of what they typed was eaten.
#[test]
fn the_line_that_does_not_produce_what_was_printed_says_so() {
    assert_eq!(
        caveats(&browsable("", "Secret", "secrets", true), Some("payments")),
        vec![
            "k8rs: a Secret's values are hidden here and shown as their sizes — the command above \
             prints them in full"
        ],
        "the command log handed over what this run had just masked, and said nothing"
    );
    assert_eq!(
        caveats(&browsable("", "Node", "nodes", false), Some("default")),
        vec!["k8rs: a node lives in no namespace, so `default` on this line was not used"],
        "half of what the reader typed was dropped in silence"
    );
    // **Nothing to say is nothing printed.** A namespaced kind used the namespace, and a run that
    // named none had none to drop — a note about a word the reader did not type is the failure
    // this is one line away from.
    assert_eq!(
        caveats(&browsable("", "Pod", "pods", true), Some("payments")),
        Vec::<String>::new()
    );
    assert_eq!(
        caveats(&browsable("", "Node", "nodes", false), None),
        Vec::<String>::new(),
        "a run that named no namespace was told one of its words was dropped"
    );
    // **The kind's own singular and the namespace as it was typed**, both through the strip every
    // other sentence on this surface uses (invariant 9).
    assert_eq!(
        caveats(
            &browsable("k8rs.example.com", "Widget\u{7}", "widgets", false),
            Some("pay\u{7}ments")
        ),
        vec![
            "k8rs: a widget lives in no namespace, so `payments (with what cannot print removed)` \
             on this line was not used"
        ]
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
/// at 80 columns, `views::raw_and_message` cutting at 100 characters, and `dump` padding every
/// line to a column width. Each fails naming its own path.
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
    //
    // **This arm is fed [`AFTER_ONE_STRIP`] and every other one [`FROM_THE_CLUSTER`], because
    // that is the shape the pipeline hands it** (CLAUDE.md § A check is proven only for the input
    // shapes it was fed). [`happening`]'s own doc is the contract — *one event, as `k8s::events`
    // hands it over* — and `k8s::events` hands over an ingested `Happening`, so a message with an
    // `ESC` still in it is a value that path cannot produce. The strip itself is proven at the
    // door, against a stub server, by `k8s_tests.rs`'s
    // `an_events_words_are_stripped_before_anything_can_draw_them`.
    //
    // **It was fed the raw line while `main.rs` carried a second strip of its own**, and that
    // strip did not move with the wording: nothing in `views.rs` strips, because ingest already
    // did (NOTES § D254). What this arm proves is unchanged — that `described` carries the value
    // out whole, cutting nothing, padding nothing inside it and folding nothing.
    let pod = pod_read("healthy").await;
    let described = described(
        &pod,
        Some(&k8s::Happened {
            lines: vec![happening(
                Some("2026-08-30T21:35:41Z"),
                "Unhealthy",
                AFTER_ONE_STRIP,
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

/// One `ops` line, as argv reaches [`main`].
///
/// **The audit log is `/dev/null`, opened for real** ([`ops_line`]'s `audit` parameter): a real
/// `File`, so the driver's own arm is the one under test, and nowhere, so thirty tests do not
/// each leave a state directory behind. The tests that care *which* file it is open it
/// themselves.
fn ops(line: &[&str]) -> String {
    ops_ended(line).said
}

/// The same line, with the exit code it carries (NOTES § D220 ruling 1, [`Ended`]).
fn ops_ended(line: &[&str]) -> Ended {
    let args: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
    ops_line(&args, nowhere, unwired, unanswered)
        .expect("a line beginning with `ops` is the operations driver's")
}

/// **The seam a test hands [`ops_line`]**, standing in for [`ops_performed`] — which builds a
/// runtime and dials the reader's own cluster, and is the one thing in this driver no unit test
/// may reach.
///
/// **It is [`not_wired`]'s own sentence.** Every assertion in this region is about the words
/// *above* the seam, and this stands in for a call this test process must not make — one that
/// dials the reader's own cluster and, for all three operations now, changes something on it.
///
/// **It stopped being *what one operation really prints* when `delete` was wired**, and it is
/// kept because [`not_wired`] is still reachable ([`wired`]'s `None`) and because a sentence that
/// names every parsed word is what these tests need to compare against. Nothing in this region
/// asserts that a real operation prints it.
fn unwired(ready: Ready<'_>) -> Ended {
    Ended::refused(not_wired(
        ready.operation,
        ready.kind,
        ready.name,
        ready.count,
        ready.namespace,
    ))
}

/// An audit log that works, keeps nothing and has nothing to say — a real open, of the one path
/// that is always there.
/// **The same, for the one verb that is not an operation** — standing in for [`may_i_started`],
/// which builds a runtime and dials the reader's own cluster exactly as [`ops_performed`] does.
///
/// **It panics rather than answering**, because every `may-i` line in this file is meant to be
/// refused above the seam: a test that reaches here is a test that would have talked to whatever
/// cluster the machine running it happens to have. The two tests that *want* the seam reached name
/// their own double.
fn unanswered(question: Question) -> Ended {
    panic!(
        "a unit test reached the cluster seam with `{} {}`",
        question.verb, question.resource
    )
}

fn nowhere() -> Result<(std::fs::File, Vec<String>), String> {
    std::fs::OpenOptions::new()
        .append(true)
        .open("/dev/null")
        .map(|log| (log, Vec::new()))
        .map_err(|failed| failed.to_string())
}

/// **A line that is not the subcommand is not the subcommand's to answer** — the two other ways
/// into this driver keep working, and a namespace called `ops` is a namespace.
///
/// **Every flag that owns the next word gets a row, and the value in each of them is the word
/// `ops`.** `-n ops` is an ordinary watch of a namespace an operator plausibly has, and a
/// `position()` over argv would read that word as a subcommand and refuse the run — so a chain
/// that stops covering one flag is a failure rather than a mutant nothing notices.
#[test]
fn only_a_bare_ops_word_is_the_subcommand_and_a_flags_value_is_not() {
    for line in [
        vec!["pod.json"],
        vec!["--once", "--namespace", "payments"],
        vec!["--live", "-n", "ops"],
        vec!["--namespace", "ops", "--once"],
        vec!["--logs", "--object", "ops"],
        vec!["--logs", "--object", "ops/web"],
        vec!["--logs", "--object", "default/web", "--container", "ops"],
        vec!["--live", "--context", "ops"],
        vec!["--yaml", "--object", "web", "--kind", "ops"],
        vec!["ops.json"],
        // **`--read-only` is read inside [`ops_line`] now and must not reach past it**: a
        // read-only watch of a namespace called `ops` is a watch, and refusing it would be the
        // B1 fix breaking the run it was not about.
        vec!["--read-only", "--live", "-n", "ops"],
        vec!["--read-only", "ops.json"],
    ] {
        let args: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
        assert!(
            ops_line(&args, nowhere, unwired, unanswered).is_none(),
            "{line:?} was taken for the operations subcommand"
        );
    }
}

/// **`ops` anywhere but first is a sentence, not a file that does not exist.**
///
/// `k8rs --once ops delete pod/web` is a line somebody types, and falling through would send
/// it to the file reader, which would come back about a file called `--once`
/// (invariant 14 — jargon about a word nobody meant as a filename).
///
/// **`--read-only` is deliberately not one of these rows.** It outranks the word order now, and
/// the test that owns it is below — the rewrite this sentence offers drops every other flag on
/// the line, so a line that carried `--read-only` may never reach it.
#[test]
fn ops_after_the_first_word_says_where_it_belongs_rather_than_falling_through_to_the_files() {
    for line in [
        vec!["--once", "ops", "delete", "pod/web"],
        vec!["--analysis", "ops", "scale", "deploy/web", "3"],
        vec!["pod.json", "ops"],
    ] {
        let said = ops(&line);
        println!("{line:?}\n{said}\n");
        assert!(
            said.starts_with("k8rs: ") && said.contains("has to be the first word"),
            "{line:?} did not say where `ops` belongs: {said:?}"
        );
        assert!(
            said.contains("usage: k8rs ops "),
            "{line:?} was refused without the subcommand's own usage under it: {said:?}"
        );
    }
}

/// **`k8rs ops` with nothing after it is the usage, and the usage teaches the whole surface.**
///
/// **Every operation is named and every one says how it is confirmed**, because that is the one
/// thing a script author cannot guess: there is no flag that means yes, so the line they have to
/// pipe is the thing this text exists to tell them.
#[test]
fn ops_alone_prints_a_usage_that_names_every_operation_and_how_each_is_confirmed() {
    let said = ops(&["ops"]);
    println!("{said}");
    assert!(said.starts_with("usage: k8rs ops "), "{said:?}");
    for operation in &OPERATIONS {
        let synopsis = format!("  ops {} <kind>/<name>", operation.verb);
        assert!(
            said.contains(&synopsis),
            "the usage has no line of its own for {}: {said:?}",
            operation.verb
        );
    }
    assert!(
        said.contains("type the object's own name to confirm"),
        "the usage does not say that one operation needs the name typed: {said:?}"
    );
    assert!(
        said.contains("say yes to confirm"),
        "the usage does not say how the others are confirmed: {said:?}"
    );
    // **The word the usage prints is the word [`ask`] accepts.** [`Operation::confirm`] is a help
    // string and the mechanism is `ops::Mutation::confirm` (NOTES § D225 ruling 2), so nothing in
    // the language keeps this sentence in step with [`YES`] — a script author told to say a word
    // the reader is not listening for cannot delete anything, and would not know why.
    assert!(
        said.contains(&format!("say {YES} to confirm")),
        "the usage tells a script author to say a word `ask` does not accept: {said:?}"
    );
    assert!(
        said.contains("standard input"),
        "the usage does not say where the answer is read from: {said:?}"
    );
    assert!(
        said.contains("no flag that means yes"),
        "the usage does not rule out the flag invariant 2 refuses: {said:?}"
    );
    // **The namespace is not optional and the synopsis may not say it is** (`k8s-admin`,
    // 2026-09-04): brackets mean optional, and it is required for five of the six kinds and
    // refused for the sixth, so a bracketed `[-n <namespace>]` sat directly above a refusal
    // saying it is required.
    assert!(
        !said.contains("[-n <namespace>]") && !said.contains("[--namespace <namespace>]"),
        "the synopsis still offers the namespace as optional: {said:?}"
    );
    assert!(
        said.contains("-n <namespace>"),
        "the synopsis does not say the namespace is part of the line at all: {said:?}"
    );
    assert!(
        said.contains("node"),
        "the usage does not say which kind takes no namespace, so the one line that is not \
         required is undocumented: {said:?}"
    );
}

/// **Every way an `ops` line can be wrong, and the sentence it gets** — one row per refusal the
/// box names, each derived from what the rule is rather than from what the code printed.
///
/// **Every one of them carries the subcommand's usage**, because a refusal that does not say what
/// to type instead has told the reader half of what they came for (invariant 14) — and every one
/// of them is wrong in its *form*, which is what makes the shape the answer (NOTES § D236
/// ruling 4). Each of these readers wrote an operation, an object, a value or a flag that is not
/// a shape `k8rs ops` has.
///
/// **The three exceptions are checked in their own tests**: `--read-only`, where offering the
/// usage would be inviting a retry of the thing that was refused; the well-formed line, which is
/// not a refusal at all; and the refusal about *meaning* — a real operation pointed at a real
/// object it does not apply to — which
/// `a_kind_an_operation_does_not_work_on_is_refused_before_the_audit_log_is_opened` holds and
/// which asserts the opposite of both halves below. **It was two until this turn**, and the third
/// is the one D236 ruling 4 carved out.
///
/// **What is compared is [`ops_usage`] itself and not only its first line**: a refusal that
/// printed the header and dropped the rows would pass the older assertion.
#[test]
fn every_wrong_ops_line_names_what_is_wrong_and_carries_the_usage() {
    for (line, expected) in [
        (
            vec!["ops", "scal", "deploy/web", "3", "-n", "payments"],
            "no operation called scal",
        ),
        (vec!["ops", "scale"], "has to be told which object"),
        (
            vec!["ops", "scale", "web", "3", "-n", "payments"],
            "needs the kind as well as the name",
        ),
        (
            vec!["ops", "scale", "/web", "3", "-n", "payments"],
            "nothing before the `/`",
        ),
        (
            vec!["ops", "scale", "deploy/", "3", "-n", "payments"],
            "nothing after the `/`",
        ),
        (
            vec!["ops", "scale", "widget/web", "3", "-n", "payments"],
            "does not work on a kind called widget",
        ),
        (
            vec!["ops", "scale", "deploy/a b", "3", "-n", "payments"],
            "is not the name of an object",
        ),
        (
            vec!["ops", "scale", "deploy/web", "three", "-n", "payments"],
            "has to be a whole number",
        ),
        (
            vec!["ops", "scale", "deploy/web", "-3", "-n", "payments"],
            "fewer copies than Kubernetes can hold",
        ),
        (
            vec!["ops", "scale", "deploy/web", "3000000000", "-n", "payments"],
            "more copies than Kubernetes can hold",
        ),
        (
            vec!["ops", "scale", "sts/api", "-n", "prod"],
            "also needs the copies",
        ),
        (
            vec!["ops", "restart", "deploy/web", "3", "-n", "payments"],
            "does not know what to do with 3",
        ),
        (
            vec!["ops", "scale", "deploy/web", "3"],
            "will not guess which namespace",
        ),
        (
            vec!["ops", "delete", "node/worker-1", "-n", "payments"],
            "belongs to the whole cluster",
        ),
        (
            vec!["ops", "scale", "deploy/web", "3", "-n"],
            "--namespace needs the name of a namespace",
        ),
        (
            vec!["ops", "scale", "deploy/web", "3", "-n", "PAYMENTS"],
            "is not one — a namespace is lowercase",
        ),
        (
            vec!["ops", "scale", "deploy/web", "3", "--context", "prod"],
            "is not a flag `k8rs ops` has",
        ),
        (
            vec!["ops", "scale", "deploy/web", "3", "-npayments"],
            "is not a flag `k8rs ops` has",
        ),
        (
            vec![
                "ops",
                "scale",
                "deploy/web",
                "3",
                "-n",
                "payments",
                "-n",
                "prod",
            ],
            "names the namespace more than once",
        ),
    ] {
        let said = ops(&line);
        println!("{line:?}\n{said}\n");
        assert!(
            said.starts_with("k8rs: "),
            "{line:?} was refused without the prefix every refusal in this driver has: {said:?}"
        );
        assert!(
            said.contains(expected),
            "{line:?} did not say {expected:?}: {said:?}"
        );
        // **The header is a literal and the rest is compared against [`ops_usage`]**: the second
        // half alone moves with the thing it checks, which is what `cargo mutants` caught one
        // assertion over on the meaning side.
        assert!(
            said.contains("usage: k8rs ops ") && said.ends_with(&ops_usage()),
            "{line:?} is wrong in its form and was refused without the whole synopsis under it: \
             {said:?}"
        );
        // **No refusal offers a complete mutation of a different object** (`k8s-admin`,
        // 2026-09-04). *write it as `k8rs ops scale deploy/web 3 -n payments`* is a runnable
        // scale of another deployment in another namespace, printed at the moment a tired
        // operator is looking for a line to copy. Every other *write it as* in this region is a
        // fragment or a placeholder, and this asserts that stays true of all of them: no refusal
        // names a concrete object k8rs invented.
        for invented in ["deploy/web 3", "web 3", "-n payments"] {
            assert!(
                !said
                    .lines()
                    .next()
                    .expect("a refusal has a first line")
                    .contains(invented),
                "{line:?} offered {invented:?}, a line somebody can paste at an object they did \
                 not name: {said:?}"
            );
        }
    }
}

/// **A line with nothing wrong in it says what k8rs read, and hands it to the seam.**
///
/// **The sentence asserted below is the test double's** ([`unwired`]), which stands in for the
/// call that would dial the reader's own cluster. All three operations are wired now; what this
/// test is about is every word the driver parsed *before* the seam, and the double is what makes
/// them readable.
///
/// **It names the operation, the kind, the name, the namespace and the value**, because *k8rs
/// cannot do that yet* alone would go green against a parser that had read the wrong object — and
/// this is the one line `just e2e` can compare a well-formed invocation against while there is
/// nothing to run. **The value is the one parsed thing it used to leave out** (`k8s-admin`,
/// 2026-09-04), and it is the one that decides how many pods exist, so `scale …/web 3` and
/// `scale …/web 0` printed one identical line.
///
/// **And it promises no later step.** Against NOTES § Operations' *Applies to* column five pairs
/// are outside it permanently — `scale` on daemonset, pod and node, `restart` on replicaset and
/// node — so *the operation itself is a later step* was false for every one of them. This driver
/// deliberately does not carry that matrix (the operation holds it, and `screens/dialogs.md`
/// rule 4 shows it is not even uniform: `restart pod/web` is a delete and does belong), so the
/// sentence is the thing that had to stop claiming.
///
/// **The short spelling and the long one and the plural are one object**, which is the whole
/// reason `deploy/web` is the form the box was written with.
#[test]
fn a_well_formed_line_names_everything_it_read_before_it_reaches_the_seam() {
    for spelling in ["deploy", "deployment", "deployments", "Deployment"] {
        let said = ops(&[
            "ops",
            "scale",
            &format!("{spelling}/web"),
            "3",
            "-n",
            "payments",
        ]);
        println!("{spelling}\n{said}\n");
        assert_eq!(
            said,
            "k8rs: k8rs read this as `scale` on deployment/web in payments, 3 copies — and this \
             build reads the line and does nothing else",
            "{spelling} did not come back as one deployment named web in payments"
        );
    }
    // **Two different counts are two different lines**, which is what `just e2e` compares. It was
    // one line for both.
    let three = ops(&["ops", "scale", "deploy/web", "3", "-n", "payments"]);
    let none = ops(&["ops", "scale", "deploy/web", "0", "-n", "payments"]);
    println!("{three}\n{none}\n");
    assert!(
        three.contains("3 copies") && none.contains("0 copies"),
        "the count k8rs parsed is not in the line it printed: {three:?} / {none:?}"
    );
    assert_ne!(
        three, none,
        "scaling to three and scaling to none printed the same sentence"
    );
    // **No pair is promised an operation that is not coming.** `scale` on a node and `restart` on
    // a replicaset are outside NOTES § Operations' *Applies to* for good.
    for line in [
        vec!["ops", "scale", "node/worker-1", "3"],
        vec!["ops", "restart", "rs/web", "-n", "payments"],
    ] {
        let said = ops(&line);
        println!("{line:?}\n{said}\n");
        assert!(
            !said.contains("later step") && !said.contains("yet"),
            "{line:?} was promised an operation that is never coming: {said:?}"
        );
    }
    // **Every spelling of the namespace flag is one flag** — the two words and the two attached
    // forms — because [`ops_words`] has to take the value out of the line whichever way it was
    // written, and a spelling it missed would leave `payments` standing as a stray word.
    for flag in [
        "-n payments",
        "-n=payments",
        "--namespace payments",
        "--namespace=payments",
    ] {
        let mut line = vec!["ops", "scale", "deploy/web", "3"];
        line.extend(flag.split(' '));
        let said = ops(&line);
        println!("{flag}\n{said}\n");
        assert!(
            said.contains("on deployment/web in payments"),
            "{flag} did not name one namespace: {said:?}"
        );
    }
    // **A kind that belongs to the whole cluster takes no namespace clause and needs no flag** —
    // `screens/dialogs.md` rule 1 gives it the bare name.
    let said = ops(&["ops", "delete", "node/worker-1"]);
    println!("{said}");
    assert_eq!(
        said,
        "k8rs: k8rs read this as `delete` on node/worker-1 — and this build reads the line and \
         does nothing else"
    );
    // Every operation reaches it, so a verb added to the table without a wired arm says so rather
    // than falling through to something else.
    //
    // **A deployment and not a pod, since todo.md 3749.** The seam is now the last thing on the
    // line and not the only thing: `scale` reads its own kind matrix above it (NOTES § D220
    // ruling 7), so `scale pod/web` is answered before the seam and is a different assertion —
    // the one two tests below. A deployment is a kind all three operations accept, which is what
    // this loop is about.
    for operation in &OPERATIONS {
        let mut line = vec!["ops", operation.verb, "deploy/web"];
        if operation.value.is_some() {
            line.push("3");
        }
        line.extend(["-n", "payments"]);
        let said = ops(&line);
        println!("{said}");
        assert!(
            said.contains(&format!(
                "read this as `{}` on deployment/web in payments",
                operation.verb
            )),
            "{:?} did not reach the seam an operation is wired into: {said:?}",
            operation.verb
        );
    }
}

/// **`--read-only` stops an operation before anything about the line matters** (invariant 2,
/// `screens/dialogs.md` rule 6: *under `--read-only` none of this is reachable*).
///
/// **It outranks every other refusal**, which is the only ordering that is not a lie: a run that
/// was told not to write has nothing to say about a misspelled kind, and answering the spelling
/// first would teach a reader to fix the line and try again.
///
/// **No usage under it**, deliberately: the thing to type instead is not an `ops` line.
///
/// **And it outranks the word order, which is the defect this test was widened for**
/// (`k8s-admin`, 2026-09-04). `k8rs --read-only ops delete pod/web -n payments` used to be
/// answered *`ops` has to be the first word — write it as `k8rs ops <operation> <kind>/<name>`*,
/// and that rewrite drops `--read-only`: k8rs told an operator to retype the line without their
/// safety flag. Two paths where one read the flag and one did not is the shape
/// `PRIOR-ART § G2` tags immune, so the flag is read in one place and before the position is.
#[test]
fn read_only_refuses_an_operation_before_anything_else_on_the_line_is_judged() {
    for line in [
        vec!["ops", "delete", "pod/web", "-n", "payments", "--read-only"],
        vec!["--read-only", "ops", "delete", "pod/web", "-n", "payments"],
        vec!["--once", "--read-only", "ops", "scale", "deploy/web", "3"],
        // **All three operations, because `--read-only` is read once and for all of them**
        // (invariant 2): a row for `scale` alone proves the flag for the verb it names. The two
        // `delete` rows are first above — the destructive one is the one it most has to hold for.
        vec![
            "--read-only",
            "ops",
            "restart",
            "deploy/web",
            "-n",
            "payments",
        ],
        vec![
            "ops",
            "restart",
            "deploy/web",
            "-n",
            "payments",
            "--read-only",
        ],
        vec!["--read-only", "pod.json", "ops"],
        vec![
            "ops",
            "--read-only",
            "scale",
            "deploy/web",
            "3",
            "-n",
            "payments",
        ],
        vec!["ops", "--read-only", "widget/nonsense", "-n", "PAYMENTS"],
        vec!["ops", "--read-only"],
    ] {
        let said = ops(&line);
        println!("{line:?}\n{said}\n");
        assert_eq!(
            said,
            "k8rs: --read-only was asked for, so k8rs will not change anything — run it without \
             that flag to use an operation",
            "{line:?} was answered about something other than the flag that forbids it"
        );
    }
}

/// **The count is judged as a number, and the three ways it can be wrong get three sentences**
/// (invariant 14 — a reader told only *that is not valid* about `-1` has to guess which).
///
/// **`i32` is the bound because that is the type `replicas` is** on every workload and on the
/// scale subresource, so the largest count that can be sent is `i32::MAX` and the first one that
/// cannot is one more.
///
/// **Both sentences finish, and they finish the same way.** *the number of copies cannot be less
/// than none, and -3 is* stopped mid-clause where its sibling named the bound, and *less than
/// none* also asks a beginner to read *none* as a number (`k8s-admin` and `tester`, 2026-09-04);
/// the two are one shape now, each naming the end of the range it is about.
#[test]
fn the_number_of_copies_is_refused_for_the_reason_it_is_wrong() {
    let most = i64::from(i32::MAX);
    // **The number comes back now, and it is the number that was typed** (todo.md 3749). While
    // this answered `Option<String>` every accepted word was one indistinguishable `None`, so a
    // parser that read `+7` as `7` and `3` as `0` was green — and the value is the one that
    // decides how many pods exist.
    for (word, count) in [
        ("0", 0),
        ("1", 1),
        ("3", 3),
        (&most.to_string(), i32::MAX),
        ("+7", 7),
        ("-0", 0),
    ] {
        assert_eq!(
            refuse_count(word),
            Ok(count),
            "{word} is a count k8rs can send, as {count}"
        );
    }
    for (word, expected) in [
        (
            "-1",
            "fewer copies than Kubernetes can hold — the fewest it takes is 0",
        ),
        (
            "-99999999999999999999999",
            "fewer copies than Kubernetes can hold — the fewest it takes is 0",
        ),
        (
            &(most + 1).to_string(),
            "more copies than Kubernetes can hold",
        ),
        (
            "99999999999999999999999",
            "more copies than Kubernetes can hold",
        ),
        ("three", "has to be a whole number"),
        ("3.0", "has to be a whole number"),
        ("", "has to be a whole number"),
    ] {
        let Err(said) = refuse_count(word) else {
            panic!("{word:?} was accepted as a count")
        };
        println!("{word:?}\n{said}\n");
        assert!(said.contains(expected), "{word:?}: {said:?}");
    }
    // **Nothing printable left is a clause and not an empty gap** ([`shown`]) — the doubled-space
    // defect `mistyped` already closed for `--object`.
    let said = refuse_count("").expect_err("an empty count is refused");
    assert!(
        said.contains("a value with nothing printable in it"),
        "an empty count printed an empty gap: {said:?}"
    );
}

/// **Invariant 9 applies to argv, and an object name from the command line has been through no
/// ingest guard.**
///
/// `ops.rs`'s `Record::of` strips on the way into the contract; a refusal that quotes the bad
/// argument is written *before* any mutation exists, so [`sanitize`] is the only thing between a
/// crafted name and the terminal. A pod named `; rm -rf ~` is boring; one carrying a bidi override
/// must not rewrite the line the operator is reading.
#[test]
fn a_crafted_word_on_an_ops_line_reaches_the_terminal_with_nothing_unprintable_left() {
    for (line, readable) in [
        // **Six of these rows name the *kind* of word and not the word** ([`as_typed`]): where
        // the strip changed it, quoting it back quotes a word nobody typed, so the sentence
        // costs itself the echo and says what is wrong instead. It was three until 2026-09-05,
        // when the sweep behind
        // [`no_refusal_ever_names_a_word_the_same_sentence_offers_back`] found the other eight
        // sites of the same defect — three of them on this line.
        //
        // **The two rows that still name the word are the two the rule does not reach**: an
        // *extra* word is extra whatever it is spelled like, so [`ops_value`] has nothing to
        // contradict itself about and keeps the echo [`shown`] bounds.
        (
            vec!["ops", "sc\u{1b}[2Jale", "deploy/web", "3", "-n", "payments"],
            "the operation you typed",
        ),
        (
            vec!["ops", "scale", "wid\u{202e}get/web", "3", "-n", "payments"],
            "the kind you typed",
        ),
        (
            vec!["ops", "scale", "deploy/we\u{202e}b", "3", "-n", "payments"],
            "the name you typed",
        ),
        (
            vec!["ops", "scale", "deploy/web", "th\u{7}ree", "-n", "payments"],
            "the number of copies you typed",
        ),
        (
            vec![
                "ops",
                "restart",
                "deploy/web",
                "ex\u{1b}[31mtra",
                "-n",
                "payments",
            ],
            "ex[31mtra",
        ),
        (
            vec!["ops", "scale", "deploy/web", "3", "-n", "pay\u{202e}ments"],
            "the namespace you typed",
        ),
        (
            vec!["ops", "scale", "deploy/web", "3", "--con\u{1b}text"],
            "the flag you typed",
        ),
        (
            vec!["--rea\u{202e}d", "ops", "scale", "deploy/web"],
            "has to be the first word",
        ),
        (
            // A pod named `; rm -rf ~` is boring: the command log is display text and nothing
            // here is fed back into a process (the security gate's own row).
            vec!["ops", "delete", "; rm -rf ~/web"],
            "; rm -rf ~",
        ),
    ] {
        let said = ops(&line);
        println!("{line:?}\n{said}\n");
        // The refusal's own line breaks are the driver's and are stripped before the sweep, the
        // way every other invariant-9 test in this file does it ([`survivors`]); a `\n` inside a
        // *value* is what would forge a second line, and `shown` removed it before it got here.
        let survivors = survivors(&said.replace('\n', ""));
        assert!(
            survivors.is_empty(),
            "{line:?} put something with no printed form on the terminal: {survivors:?}\n{said:?}"
        );
        // **Both halves.** A strip that returned nothing would pass the sweep and leave the
        // reader a sentence that names no word (CLAUDE.md § A derived list asserts it found
        // something) — which is the word itself where the strip left it alone, and the kind of
        // word it was where it did not ([`as_typed`]).
        assert!(
            said.contains(readable),
            "{line:?} was stripped down past the word it is about: {said:?}"
        );
    }
}

/// **A flag this driver does not have is echoed as the word that was typed, and bounded**
/// ([`shown`], not [`sanitize`]) — the two records may not lie about which string they mean
/// (invariant 4), and a word refused for being eight kilobytes long may not be printed at eight
/// kilobytes to say so (the security gate's *sizes are bounded* row).
///
/// **The word a strip changed is not echoed at all, and that rule has its own test**
/// ([`no_refusal_ever_names_a_word_the_same_sentence_offers_back`]). What is left here is the
/// echo of a word the strip left alone and the cut that bounds it — the half [`shown`] still owns.
#[test]
fn a_flag_this_driver_does_not_have_is_echoed_as_typed_and_cut_to_a_length() {
    // Bounded, and the cut is said out loud. `NAME_MAX` characters plus the clause, never the
    // 9000 the reader typed.
    let long = format!("-{}", "a".repeat(9000));
    let said = ops(&["ops", "scale", "deploy/web", "3", &long]);
    println!("{}", &said[..said.find('\n').unwrap_or(said.len())]);
    let first = said.lines().next().expect("a refusal has a first line");
    assert!(
        first.len() < 500,
        "a 9000-character flag word came back whole, at {} bytes: {first:?}",
        first.len()
    );
    assert!(
        first.contains("(shortened by k8rs)"),
        "the cut was silent: {first:?}"
    );
    // `-` and then `NAME_MAX - 1` of the 9000 `a`s: the cut is at `NAME_MAX` characters, which is
    // where every sibling refusal on this line already cuts, and not one character either side.
    assert!(
        first.contains(&"a".repeat(k8s::NAME_MAX - 1))
            && !first.contains(&"a".repeat(k8s::NAME_MAX)),
        "the echo was cut somewhere other than the length every sibling refusal on this line \
         cuts at: {first:?}"
    );
}

/// **No refusal names a word that the same sentence then offers back** ([`as_typed`], invariant
/// 9, invariant 14).
///
/// **Eleven sites, one rule, and argv reaches every one** (`k8s-admin`, 2026-09-05, for the first
/// three; the rest from the sweep the PM asked for, `dev-core`, 2026-09-05). `ops.rs`'s `a_kind`
/// settled it one layer down (NOTES § D224) in the copy no command line can reach, while these
/// went on quoting the stripped word: `dep<U+200B>loyment/web` came back as *k8rs does not work
/// on a kind called deployment — the ones an operation can be pointed at are deployment, …*, one
/// sentence contradicting its own second clause. The operator then retypes a line that looks
/// identical and is refused identically, with nothing in the sentence to break the loop.
///
/// **The sweep is what says eleven, and it was measured against the built binary** — the same
/// zero-width space fed to every refusal in `main.rs` that names a word out of argv, read off
/// what it printed rather than off the source. It is also why this test is no longer about `ops`:
/// five of the sites are on the read path, and a name saying otherwise would be the second copy
/// of a claim that had already gone wrong once.
///
/// **What deliberately does not route through [`as_typed`], with why for each** — the other half
/// of the sweep, because a site nobody named is a site the next reader re-derives:
///
/// - *`--context --li<U+200B>ve`* — *"and `--live` is a flag"* stays true of the cleaned word, so
///   there is no contradiction to fix.
/// - *a word after a complete line* ([`ops_value`], [`may_i_question`]) — *"does not know what to
///   do with `foo`"* is about there being an extra word, which its spelling does not change.
/// - *a file beside a mode* — *"`--once` reads a cluster, so k8rs cannot also read `pod.json`"*,
///   same shape.
/// - *`in_namespace`* — measured, not reasoned: `k8s::drawable` has already stripped the
///   kubeconfig's `namespace:` before it arrives, and [`mistyped`] refuses the argv one above it,
///   so no word this refusal can name was changed on the way in.
/// - *[`which_kind`]'s ambiguous arm, and a kind whose own words cannot build a URL* — both are
///   reached only by a word that **matched** something the cluster serves, which a word with a
///   character that does not print cannot do; it takes the arm below instead.
/// - *[`refuse_count`]'s two range sentences* — reached only by a word that parsed as a number or
///   is all ASCII digits, which is the same door closed.
///
/// **What is asserted is the contradiction and not the wording.** Each row names the fragment that
/// would put the cleaned word where the sentence says k8rs does not have it, and the row passes by
/// that fragment being absent — so a reworded sentence that quotes the word again is still red.
///
/// **The other half is that the sentence still names something** (CLAUDE.md § A derived list
/// asserts it found something): the kind of word, and what is wrong with it.
///
/// **Every row is reported, and the loop does not stop at the first** — it collected its
/// assertions into one failure until 2026-09-05 and did not, which is how rows 2 and 3 were never
/// once seen red (PM, 2026-09-05). A per-row panic makes the second row's evidence cost a second
/// run, and eleven rows make that eleven.
///
/// **A word the strip left alone is still quoted, and the tests for that are already here** —
/// `every_wrong_ops_line_names_what_is_wrong_and_carries_the_usage`'s `scal`, `widget` and
/// `--context` rows, and
/// [`a_flag_this_driver_does_not_have_is_echoed_as_typed_and_cut_to_a_length`], which is why this
/// test does not repeat them.
#[test]
fn no_refusal_ever_names_a_word_the_same_sentence_offers_back() {
    // A zero-width space, because it is the shape with no tell at all: every one of these words
    // prints exactly as the word k8rs does take.
    let kinds = vec![browsable("", "Node", "nodes", false)];
    let refusals = [
        // --- the `ops` line ---
        (
            "ops <operation>",
            ops(&["ops", "sca\u{200b}le", "deploy/web", "3", "-n", "payments"]),
            "called scale",
            "the operation you typed",
        ),
        (
            "ops <kind>/…",
            ops(&["ops", "restart", "dep\u{200b}loyment/web", "-n", "payments"]),
            "called deployment",
            "the kind you typed",
        ),
        (
            "ops …/<name>",
            ops(&["ops", "scale", "deploy/we\u{200b}b", "3", "-n", "payments"]),
            "web (with what cannot print removed)",
            "the name you typed",
        ),
        (
            "ops <copies>",
            ops(&["ops", "scale", "deploy/web", "\u{200b}3", "-n", "payments"]),
            "3 (with what cannot print removed)",
            "the number of copies you typed",
        ),
        (
            "ops --flag",
            ops(&[
                "ops",
                "scale",
                "deploy/web",
                "3",
                "--names\u{200b}pace=payments",
            ]),
            "--namespace=payments (with what cannot print removed)",
            "the flag you typed",
        ),
        (
            "ops -n <namespace>",
            ops(&["ops", "scale", "deploy/web", "3", "-n", "pay\u{200b}ments"]),
            "payments (with what cannot print removed)",
            "the namespace you typed",
        ),
        // The second caller of the same helper, because one shared sentence with two usages is
        // the shape where a fix lands on one of them.
        (
            "ops may-i -n <namespace>",
            ops(&["ops", "may-i", "get", "pods.", "-n", "pay\u{200b}ments"]),
            "payments (with what cannot print removed)",
            "the namespace you typed",
        ),
        // --- the read path, which `mistyped` answers before anything connects ---
        (
            "--object …/<name>",
            mistyped(&argv(&["--logs", "--object", "default/we\u{200b}b"]))
                .expect("a name with a character that does not print is refused"),
            "web (with what cannot print removed)",
            "the name you typed",
        ),
        (
            "--object <namespace>/…",
            mistyped(&argv(&["--logs", "--object", "pay\u{200b}ments/web"]))
                .expect("a namespace with a character that does not print is refused"),
            "payments (with what cannot print removed)",
            "the namespace you typed",
        ),
        (
            "--namespace",
            mistyped(&argv(&["--once", "--namespace", "pay\u{200b}ments"]))
                .expect("a namespace with a character that does not print is refused"),
            "payments (with what cannot print removed)",
            "the namespace you typed",
        ),
        (
            "--container",
            mistyped(&argv(&[
                "--logs",
                "--object",
                "default/web",
                "--container",
                "ap\u{200b}p",
            ]))
            .expect("a container name with a character that does not print is refused"),
            "app (with what cannot print removed)",
            "the container name you typed",
        ),
        (
            "an unknown --flag",
            mistyped(&argv(&["--once", "--name\u{200b}space=payments"]))
                .expect("a flag with a character that does not print is refused"),
            "--namespace=payments (with what cannot print removed) is not a flag",
            "the flag you typed",
        ),
        // The one-dash arm is a *different* branch of the same sentence — `-n` never reaches the
        // `--` check above it — and `-n` is a flag k8rs very much has.
        (
            "an unknown -flag",
            mistyped(&argv(&["--once", "-\u{200b}n", "payments"]))
                .expect("a flag with a character that does not print is refused"),
            "-n (with what cannot print removed) is not a flag",
            "the flag you typed",
        ),
        // --- the one refusal a cluster's own answer produces ---
        (
            "--kind",
            which_kind(&kinds, "no\u{200b}de")
                .expect_err("nothing serves a kind spelled with a gap"),
            "kind named node",
            "the kind you typed",
        ),
    ];
    let mut wrong: Vec<String> = Vec::new();
    for (site, said, quoted, noun) in &refusals {
        println!("{site}\n{said}\n");
        if said.contains(quoted) {
            wrong.push(format!(
                "{site}: named a word k8rs does take as one it does not have ({quoted:?}) — \
                 {said:?}"
            ));
        }
        if !said.contains(noun) || !said.contains("does not print") {
            wrong.push(format!(
                "{site}: the reader is not told which word could not be read, or why \
                 ({noun:?}) — {said:?}"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// **The namespace may be named once and no more, and `k8rs ops` refuses rather than picking
/// one** (`k8s-admin` and `tester`, 2026-09-04; PM ruling).
///
/// **The read path resolves a repeat and this one refuses it, and that is not the two
/// disagreeing.** `kubectl` is last-wins — measured here on `kubectl` v1.36.3, client-side and
/// against no cluster: `kubectl create deployment web --image=nginx --dry-run=client -o yaml -n
/// payments -n prod` prints `namespace: prod`, and so does the same line with `--namespace
/// payments -n prod`. [`value_of`] followed that measurement as of the flags box and takes the
/// last too; the argument this doc used to make — *first-wins would send a mutation to whichever
/// of the two the reader's habit says is the other one* — went with it. What it rested on did
/// not: a read taken against the wrong namespace costs a re-run and a mutation does not, so this
/// path buys its certainty with a refusal rather than with a tie-break. It is also the
/// contradiction two doc comments in this region already rule out: [`ops_words`]' *a word
/// silently skipped is a run doing something other than what was typed*, and [`ops_namespace`]'s
/// refusal to guess a namespace nobody typed. Refusing to guess when none was typed and then
/// guessing when two were is not one rule.
///
/// **Every spelling combination**, because the four spellings are two branches in [`ops_words`]
/// and a count that only saw one of them would take `-n x --namespace=y` silently.
#[test]
fn a_namespace_named_twice_on_an_ops_line_is_refused_rather_than_guessed() {
    for tail in [
        vec!["-n", "payments", "-n", "prod"],
        vec!["--namespace", "payments", "-n", "prod"],
        vec!["-n", "payments", "--namespace=prod"],
        vec!["-n=payments", "--namespace", "prod"],
        vec!["--namespace=payments", "--namespace=prod"],
        vec!["-n", "payments", "-n", "payments"],
    ] {
        let mut line = vec!["ops", "scale", "deploy/web", "3"];
        line.extend(tail.iter().copied());
        let said = ops(&line);
        println!("{tail:?}\n{said}\n");
        assert!(
            said.starts_with("k8rs: ") && said.contains("names the namespace more than once"),
            "{tail:?} was not refused for naming the namespace twice: {said:?}"
        );
        assert!(
            said.contains("usage: k8rs ops "),
            "{tail:?} was refused without the subcommand's own usage under it: {said:?}"
        );
    }
    // One namespace, however it is spelled, is not two — the guard must not fire on the line it
    // is not about.
    for tail in [
        vec!["-n", "payments"],
        vec!["--namespace=payments"],
        vec!["-n=payments"],
    ] {
        let mut line = vec!["ops", "scale", "deploy/web", "3"];
        line.extend(tail.iter().copied());
        let said = ops(&line);
        assert!(
            said.contains("on deployment/web in payments"),
            "{tail:?} named one namespace and was refused for naming two: {said:?}"
        );
    }
}

/// **The dialog `screens/dialogs.md` draws, printed instead** — the identity, the consequence,
/// then the equivalent kubectl command under it.
///
/// **The consequence is above the command and never instead of it** (`screens/dialogs.md`, first
/// line), and the `$` line is display text: k8rs does not execute it.
///
/// **The verdict is not here**, because [`show`] runs before the check goes out — that ordering is
/// the whole reason `ops::perform` has two callbacks (NOTES § D214).
#[test]
fn the_headless_dialog_prints_the_consequence_above_the_command_and_no_verdict() {
    let shown = ops::Shown {
        object: "deployment/web",
        namespace: Some("payments"),
        consequence: "This starts 1 more copy of your app. Right now: 2 copies.  After: 3 copies.",
        kubectl: "kubectl scale deploy/web --replicas=3 -n payments",
    };
    let mut out = Vec::new();
    show(&shown, &mut out).expect("a Vec takes every write");
    let printed = String::from_utf8(out).expect("the dialog is text");
    print!("{printed}");
    assert_eq!(
        printed,
        "deployment/web in payments\n\
         This starts 1 more copy of your app. Right now: 2 copies.  After: 3 copies.\n\
         $ kubectl scale deploy/web --replicas=3 -n payments\n"
    );
    let lines: Vec<&str> = printed.lines().collect();
    assert!(
        lines[1] == shown.consequence && lines[2].starts_with("$ "),
        "the command is not under the consequence: {printed:?}"
    );
    assert!(
        !printed.contains("checked it first"),
        "the dialog printed a verdict it cannot have yet: {printed:?}"
    );

    // **A kind that belongs to the whole cluster gets the bare identity** (rule 1).
    let mut out = Vec::new();
    show(
        &ops::Shown {
            object: "node/worker-1",
            namespace: None,
            consequence: "This removes the node from the cluster.",
            kubectl: "kubectl delete node worker-1",
        },
        &mut out,
    )
    .expect("a Vec takes every write");
    let printed = String::from_utf8(out).expect("the dialog is text");
    print!("{printed}");
    assert!(
        printed.starts_with("node/worker-1\n"),
        "a cluster-scoped object was given a namespace clause: {printed:?}"
    );

    // A stream that refuses every write is a failure the caller is told about, not one it has to
    // notice — nothing may be confirmed on the strength of a dialog nobody saw.
    assert!(
        show(&shown, &mut Closed).is_err(),
        "a dialog that could not be printed came back as though it had been"
    );
}

/// **A writer that refuses every write** — the stream is closed.
struct Closed;

impl std::io::Write for Closed {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }
}

/// **What one headless confirmation did, driven through the real contract** — the outcome, and
/// what the reader was shown on the way to it.
///
/// **`ops::Checked` can be built nowhere but `ops.rs`, which is the whole point of it and of
/// `ops::Agreed` beside it** (NOTES § D225 ruling 2). So a test of [`ask`] cannot hand it one and
/// has to make `ops::perform` build it — which is what these tests did not have to do while `ask`
/// took three loose values.
///
/// **Nothing dials anything.** The call closure answers `Ok(())` for both passes, the audit log
/// is a `Vec`, and the mutation is `checkable: false`, so this is the delete path's own shape:
/// `show`, then the prompt, then the answer.
///
/// **`Outcome::Done` is a confirmation and `Outcome::Cancelled` is everything else**, which is a
/// stronger claim than the `ops::Answer` these tests used to compare: it says the real call went
/// out, and not merely that a value came back.
async fn confirmation(
    confirm: ops::Confirm<'_>,
    typed: &[u8],
    out: &mut impl std::io::Write,
) -> Option<ops::Outcome> {
    let mutation = ops::Mutation {
        context: "kind-k8rs",
        // A reserved host: `scripts/security-guard.py` reads a loopback URL in this tree as a
        // second outbound path and is right to.
        server: "https://k8rs-tests.invalid:41751",
        namespace: Some("payments"),
        object: "pod/web-7d9f4",
        uid: None,
        uid_sent: false,
        consequence: "This removes the pod. Whatever created it will normally replace it — k8rs \
                      has not checked whether anything did.",
        kubectl: "kubectl delete pod/web-7d9f4 -n payments",
        verb: "DELETE",
        path: "/api/v1/namespaces/payments/pods/web-7d9f4",
        version: None,
        checkable: false,
        confirm,
    };
    // **Bytes and not a `&str`**, because one of the rows this drives is input that is not text
    // at all: `read_line` refuses it, and a `&str` parameter could not carry it.
    let mut input = typed;
    ops::perform(
        &mutation,
        || {
            k8s_openapi::jiff::Timestamp::from_second(1_788_438_896)
                .expect("a timestamp inside jiff's range")
        },
        &mut Vec::new(),
        |_| {},
        |checked| std::future::ready(ask(&checked, &mut input, out)),
        |_| std::future::ready(Ok::<(), kube::Error>(())),
        // A `PATCH`'s shape: the cluster's answer says the change is finished. What the other
        // arm looks like is `ops_tests.rs`'s, over the one operation that can produce it.
        |_| ops::Landing::Finished,
    )
    .await
    .outcome
}

/// **One mutation driven through the real contract, with a dialog of the caller's choosing** —
/// the shape [`confirmation`] has, without the headless [`ask`] in the middle.
///
/// **It exists so the replay attack can be written from outside `ops.rs`**, which is where every
/// dialog in this program lives and where the first draft of `ops::Agreed` was breakable.
async fn one_mutation(
    confirm: ops::Confirm<'_>,
    dialog: impl FnOnce(&ops::Checked<()>) -> ops::Answer,
    calls: &std::cell::Cell<usize>,
) -> Option<ops::Outcome> {
    let mutation = ops::Mutation {
        context: "kind-k8rs",
        server: "https://k8rs-tests.invalid:41751",
        namespace: Some("payments"),
        object: "pod/web-7d9f4",
        uid: None,
        uid_sent: false,
        consequence: "This removes the pod.",
        kubectl: "kubectl delete pod/web-7d9f4 -n payments",
        verb: "DELETE",
        path: "/api/v1/namespaces/payments/pods/web-7d9f4",
        version: None,
        checkable: false,
        confirm,
    };
    ops::perform(
        &mutation,
        || {
            k8s_openapi::jiff::Timestamp::from_second(1_788_438_896)
                .expect("a timestamp inside jiff's range")
        },
        &mut Vec::new(),
        |_| {},
        |checked| std::future::ready(dialog(&checked)),
        |_| {
            calls.set(calls.get() + 1);
            std::future::ready(Ok::<(), kube::Error>(()))
        },
        |_| ops::Landing::Finished,
    )
    .await
    .outcome
}

/// **A yes kept from one mutation cannot confirm another** (NOTES § D225 ruling 2, invariant 2) —
/// the attack the first draft of `ops::Agreed` was open to, written from the file every dialog
/// lives in.
///
/// **An enum variant's fields are as public as the enum**, whatever the token struct's own field
/// says, so a `Copy` token with no contents could be destructured out of one `Answer::Confirmed`
/// and re-wrapped for any later mutation: press-confirm one scale, keep what falls out, and every
/// delete after it proceeds with no name typed. `tester` measured a `DELETE` on the wire from
/// exactly this code. Not reachable through today's driver — one operation per process — and
/// reachable at Phase 12, where the console is one process with many dialogs.
///
/// **`ops::Agreed` carries `ops::perform`'s ticket now**, so a token from any other call — even
/// one with the identical `ops::Confirm` — is refused before the real call.
///
/// **A panic and not a `Cancelled`, in a debug build.** A replayed confirmation is the author's
/// error, and the contract reports one the way `ops::Record::of` and `ops::Checked::pressed`
/// already do: `debug_assert` where it can be loud, and the safe direction where it cannot. In
/// release the same path is `Outcome::Cancelled` and nothing is sent; either way the delete does
/// not happen, which is what this test is about.
#[tokio::test]
#[should_panic(expected = "replayed")]
async fn a_confirmation_kept_from_one_mutation_cannot_confirm_another() {
    let stolen = std::cell::RefCell::new(None);
    let calls = std::cell::Cell::new(0);

    // A first mutation a press confirms. The dialog keeps the token and then cancels, so nothing
    // about this call is a change — only the yes survives it.
    let first = one_mutation(
        ops::Confirm::Press,
        |checked| {
            if let ops::Answer::Confirmed(token) = checked.pressed() {
                *stolen.borrow_mut() = Some(token);
            }
            ops::Answer::Cancelled
        },
        &calls,
    )
    .await;
    println!("first: {first:?} · calls {}", calls.get());
    assert_eq!(first, Some(ops::Outcome::Cancelled));
    assert_eq!(calls.get(), 0, "a cancelled mutation sent the change");
    assert!(
        stolen.borrow().is_some(),
        "the probe kept nothing, so it proves nothing: `ops::Answer::Confirmed` no longer yields \
         a token to keep, and this test has to be rewritten rather than deleted"
    );

    // The same yes, returned at a delete that asked for a typed name and was given none.
    let second = one_mutation(
        ops::Confirm::Type("web-7d9f4"),
        |_| ops::Answer::Confirmed(stolen.borrow_mut().take().expect("the kept token")),
        &calls,
    )
    .await;
    println!("second: {second:?} · calls {}", calls.get());
    assert_eq!(
        calls.get(),
        0,
        "a dialog outside ops.rs deleted an object without typing its name"
    );
    assert_eq!(second, Some(ops::Outcome::Cancelled));
}

/// **The confirmation is one line the caller had to supply, and everything else is `Cancelled`**
/// (invariant 2 — there is no flag that means yes, and no default that means yes either).
#[tokio::test]
async fn the_headless_confirmation_takes_yes_and_nothing_else() {
    for (typed, expected) in [
        ("yes\n", ops::Outcome::Done),
        ("yes", ops::Outcome::Done),
        ("yes\r\n", ops::Outcome::Done),
        ("  yes  \n", ops::Outcome::Done),
        ("y\n", ops::Outcome::Cancelled),
        ("YES\n", ops::Outcome::Cancelled),
        ("no\n", ops::Outcome::Cancelled),
        ("\n", ops::Outcome::Cancelled),
        ("", ops::Outcome::Cancelled),
        ("web\n", ops::Outcome::Cancelled),
    ] {
        let mut out = Vec::new();
        let outcome = confirmation(ops::Confirm::Press, typed.as_bytes(), &mut out).await;
        println!("{typed:?} -> {outcome:?}");
        assert!(
            outcome.as_ref() == Some(&expected),
            "{typed:?} was not read as {expected:?}"
        );
    }
}

/// **The destructive half needs the object's own name and nothing else will do** (invariant 2,
/// `screens/dialogs.md` § Delete — the ctrl-key-slip guard).
///
/// **`yes` does not confirm a delete**: a script that says yes to everything cannot delete
/// anything. Which of the two a mutation wants is `ops::Mutation::confirm`'s now and not this
/// driver's, so what these rows drive is the real requirement (NOTES § D225 ruling 2).
///
/// **An empty name confirms nothing, which is the last two rows** (`k8s-admin`, 2026-09-04).
/// `typed.trim() == wanted` held for `("", "")`, so end of input against an object with no name
/// was invariant 2's *typing the object name* satisfied by typing nothing. No argv reaches it —
/// `k8s::object_name("")` is false — but `ops::Record::of`'s strip can empty a name that argv
/// could not. **The guard moved into `ops::Checked::typed` with the mechanism**, which is the one
/// function every dialog routes through rather than the one every dialog has to remember; these
/// rows drive it from the outside, and `ops_tests.rs` asserts it on the method.
#[tokio::test]
async fn a_typed_name_confirmation_takes_the_name_and_not_yes() {
    for (typed, name, expected) in [
        ("web-7d9f4\n", "web-7d9f4", ops::Outcome::Done),
        ("web-7d9f4", "web-7d9f4", ops::Outcome::Done),
        ("yes\n", "web-7d9f4", ops::Outcome::Cancelled),
        ("web\n", "web-7d9f4", ops::Outcome::Cancelled),
        ("web-7d9f5\n", "web-7d9f4", ops::Outcome::Cancelled),
        ("", "web-7d9f4", ops::Outcome::Cancelled),
        ("", "", ops::Outcome::Cancelled),
        ("\n", "", ops::Outcome::Cancelled),
        ("  \n", "", ops::Outcome::Cancelled),
    ] {
        let mut out = Vec::new();
        let outcome = confirmation(ops::Confirm::Type(name), typed.as_bytes(), &mut out).await;
        println!("PROBE {outcome:?} <- typed {typed:?} name {name:?}");
        assert!(
            outcome.as_ref() == Some(&expected),
            "{typed:?} against the name {name:?} was not read as {expected:?}"
        );
    }
}

/// **The verdict reaches the reader before the answer is read**, which is `screens/dialogs.md`
/// rule 3 — the check's verdict is shown *before* the button is live — and it is why `ops::Checked`
/// exists at all.
///
/// **The prompt says how to confirm**, off `ops::Checked::asks` and no longer off a table in this
/// file, because a script author who cannot guess the word cannot write the script.
///
/// **The verdict is the uncheckable one here**, because [`confirmation`] drives a
/// `checkable: false` mutation — which is `delete`'s own shape (NOTES § D225 ruling 1) and is the
/// sentence `screens/dialogs.md` § Delete puts in every box on that page.
#[tokio::test]
async fn the_confirmation_prints_the_verdict_and_says_what_to_type_before_it_reads_anything() {
    for (confirm, expected) in [
        (ops::Confirm::Press, "type yes and press enter"),
        (
            ops::Confirm::Type("web"),
            "type the object's own name and press enter",
        ),
    ] {
        let mut out = Vec::new();
        confirmation(confirm, b"yes\n", &mut out).await;
        let printed = String::from_utf8(out).expect("the prompt is text");
        println!("{printed}");
        assert!(
            printed.starts_with("k8rs did not check this one with the cluster first\n"),
            "the verdict did not reach the reader before the prompt: {printed:?}"
        );
        assert!(printed.contains(expected), "{printed:?}");
        assert!(
            printed.contains("anything else stops it"),
            "the prompt did not say what not answering does: {printed:?}"
        );
        // **The prompt ends its own line** (`k8s-admin`, 2026-09-04). Under `echo yes | k8rs ops
        // …` — NOTES § D218's documented and only scripted form — stdin echoes nothing, so a
        // prompt that leaves the cursor on its line glues [`ending`]'s closing sentence to the
        // back of it: *…anything else stops it: k8rs: the change was made*, measured. That
        // sentence is the only place `recorded: false` is ever reported (NOTES § D220 ruling 1
        // keeps it out of the exit code), so a script grepping stderr for `k8rs: ` was the one
        // reader that could not find it.
        assert!(
            printed.ends_with("anything else stops it:\n"),
            "whatever is printed next lands on the prompt's own line: {printed:?}"
        );
    }
}

/// **Every failure is `Cancelled`, because nobody confirmed anything** — a prompt that could not
/// be printed, and input that is not text.
///
/// **The safe direction is the only one invariant 2 leaves**: a confirmation that defaults to yes
/// when the terminal is gone is the implicit write the invariant exists to prevent.
#[tokio::test]
async fn a_confirmation_that_could_not_be_asked_or_read_is_a_no() {
    // The prompt could not be printed: the reader never saw what they would be agreeing to, and
    // the answer waiting on the pipe is not an answer to a question that was asked.
    assert_eq!(
        confirmation(ops::Confirm::Press, b"yes\n", &mut Closed).await,
        Some(ops::Outcome::Cancelled),
        "a confirmation nobody could read was taken as a yes"
    );
    // Input that is not text — `read_line` refuses it rather than answering with a shorter line.
    let mut out = Vec::new();
    assert_eq!(
        confirmation(
            ops::Confirm::Press,
            &[b'y', b'e', b's', 0xff, b'\n'],
            &mut out
        )
        .await,
        Some(ops::Outcome::Cancelled),
        "input that is not text was read as a yes"
    );
}

/// **The kind table answers for the spellings an operator's hands type**, and for nothing else.
#[test]
fn a_kind_is_known_by_its_own_word_by_kubectls_short_one_or_by_its_plural() {
    for kind in &KINDS {
        for spelling in [
            kind.singular.to_string(),
            kind.short.to_string(),
            format!("{}s", kind.singular),
            kind.singular.to_uppercase(),
        ] {
            let found = known_kind(&spelling)
                .unwrap_or_else(|| panic!("{spelling} names no kind this driver knows"));
            assert_eq!(
                found.singular, kind.singular,
                "{spelling} resolved to the wrong kind"
            );
            assert_eq!(found.namespaced, kind.namespaced);
        }
    }
    for word in ["", "widget", "deployment.apps", "pods/web", "n", "podss"] {
        assert!(
            known_kind(word).is_none(),
            "{word} was accepted as a kind this driver works on"
        );
    }
    // **The one kind in the table that belongs to the whole cluster**, which is what makes both
    // namespace refusals reachable.
    assert!(
        KINDS.iter().any(|kind| !kind.namespaced),
        "no kind in the table is cluster-scoped, so one of the two namespace rules is unreachable"
    );
}

/// **A word with a leading `-` is a flag unless it is a number**, so a count typed with a sign is
/// answered as a count.
#[test]
fn a_signed_number_is_not_read_as_a_flag() {
    for word in ["-3", "-0", "-", "-999999999999999999999999"] {
        assert!(!flag_word(word), "{word} was read as a flag");
    }
    for word in ["-n", "-nginx", "--context", "--read-only", "-o", "-3a"] {
        assert!(flag_word(word), "{word} was not read as a flag");
    }
    for word in ["3", "web", "deploy/web", ""] {
        assert!(!flag_word(word), "{word} was read as a flag");
    }
}

/// **A line k8rs is going to refuse never asks for an audit log** (todo.md 3696).
///
/// `k8rs ops bogus` must not leave a state directory behind: nothing was going to be changed, so
/// nothing needs recording, and a mistyped word is not a reason to write into somebody's `$HOME`.
/// The `FnOnce` is what makes this assertable — the count is of *asks*, not of files.
///
/// **The rows are one per refusal [`ops_run`] can reach**, in its own order, so a check that
/// moves above the parse would be caught here rather than by a directory appearing.
#[test]
fn a_line_k8rs_refuses_never_asks_for_an_audit_log() {
    for line in [
        vec!["ops"],
        vec!["ops", "--wat", "scale", "deploy/web", "3"],
        vec!["ops", "scale", "deploy/web", "3", "-n", "a", "-n", "b"],
        vec!["ops", "bogus", "deploy/web"],
        vec!["ops", "scale"],
        vec!["ops", "scale", "web", "3"],
        vec!["ops", "scale", "widget/web", "3"],
        vec!["ops", "scale", "deploy/web"],
        vec!["ops", "scale", "deploy/web", "three"],
        vec!["ops", "scale", "deploy/web", "3", "4"],
        vec!["ops", "scale", "deploy/web", "3"],
        vec!["ops", "delete", "node/k8rs-worker2", "-n", "payments"],
        // **`--read-only` is deliberately not a row here, and it was one for a round.** It reads
        // as the row that matters most and it is a row that cannot fail: [`ops_line`] refuses the
        // flag above [`ops_run`], and no single move makes it red — with the refusal deleted the
        // flag is still refused as an unknown one by [`ops_words`], and `FnOnce` makes a second
        // open a compile error rather than a failing test. The flag's own tests are above; this
        // list is the lines that reach [`ops_run`] (my own second pass, 2026-09-04).
    ] {
        let asked = std::cell::Cell::new(0u32);
        let args: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
        let sentence = ops_line(
            &args,
            || {
                asked.set(asked.get() + 1);
                nowhere()
            },
            unwired,
            unanswered,
        )
        .expect("a line beginning with `ops` is the operations driver's")
        .said;
        println!("--- {line:?} ---\n{sentence}");
        assert_eq!(
            asked.get(),
            0,
            "{line:?} is refused and asked for an audit log anyway"
        );
    }
}

/// **A line with nothing wrong in it opens the log, once, before it reaches the seam**
/// (NOTES § D21 — a mutation that cannot be recorded does not happen, so the trail is answered
/// for before an operation is).
#[test]
fn a_well_formed_line_opens_the_audit_log_once_before_it_reaches_the_seam() {
    for line in [
        vec!["ops", "scale", "deploy/web", "3", "-n", "payments"],
        vec!["ops", "restart", "sts/db", "-n", "payments"],
        vec!["ops", "delete", "node/k8rs-worker2"],
    ] {
        let asked = std::cell::Cell::new(0u32);
        let args: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
        let sentence = ops_line(
            &args,
            || {
                asked.set(asked.get() + 1);
                nowhere()
            },
            unwired,
            unanswered,
        )
        .expect("a line beginning with `ops` is the operations driver's")
        .said;
        println!("--- {line:?} ---\n{sentence}");
        assert_eq!(
            asked.get(),
            1,
            "{line:?} did not open an audit log exactly once"
        );
        assert!(
            sentence.ends_with("and this build reads the line and does nothing else"),
            "{line:?} did not reach the seam: {sentence}"
        );
    }
}

/// **Something worth saying about the audit log is said, above the sentence about the run**
/// (`ops::audit_log`'s notes — an ignored `$XDG_STATE_HOME`, a log other people can write to).
///
/// **Above and not instead of**, which is the opposite of the refusal below it: a note is a thing
/// that is true *and* the run went on, so the sentence about what k8rs did has to still be there.
/// A note that replaced it would read as though nothing happened, and one printed after it would
/// be read as part of the outcome.
///
/// **Two notes and not one**, because both of `audit_log`'s can be true at once — a relative
/// `$XDG_STATE_HOME` and a `0666` log under `$HOME` — and a driver that joined them onto one line
/// would bury the second.
#[test]
fn something_worth_saying_about_the_audit_log_is_said_above_the_seams_own_sentence() {
    let args: Vec<String> = ["ops", "scale", "deploy/web", "3", "-n", "payments"]
        .iter()
        .map(|word| (*word).to_string())
        .collect();
    let sentence = ops_line(
        &args,
        || {
            nowhere().map(|(log, _)| {
                (
                    log,
                    vec![
                        "k8rs is not keeping its audit log where $XDG_STATE_HOME points"
                            .to_string(),
                        "the audit log at /home/ops/.local/state/k8rs/audit.log can be written to \
                         by other people on this machine"
                            .to_string(),
                    ],
                )
            })
        },
        unwired,
        unanswered,
    )
    .expect("a line beginning with `ops` is the operations driver's")
    .said;
    println!("{sentence}");
    assert_eq!(
        sentence,
        "k8rs: k8rs is not keeping its audit log where $XDG_STATE_HOME points\n\
         k8rs: the audit log at /home/ops/.local/state/k8rs/audit.log can be written to by \
         other people on this machine\n\
         k8rs: k8rs read this as `scale` on deployment/web in payments, 3 copies — and this \
         build reads the line and does nothing else",
        "a note about the audit log was dropped, printed after the outcome, or printed instead \
         of it"
    );
}

/// **A machine that cannot hold the audit log refuses the operation and says why**
/// ([NOTES § D21](../NOTES.md) — k8rs says so and continues read-only; it does not exit and it
/// does not quietly drop the trail).
///
/// **The refusal replaces the seam's sentence rather than being printed beside it.** A run that
/// says both *k8rs could not open its audit log* and *k8rs read this as scale on deployment/web*
/// reads as though the second happened anyway, and the whole point of D21 is that it did not.
///
/// **The sentence is `ops::audit_log`'s own words**, prefixed the way every other refusal on this
/// line is — the two records may not disagree about which one is speaking (invariant 4).
#[test]
fn a_machine_that_cannot_hold_the_audit_log_refuses_the_operation_and_says_why() {
    let args: Vec<String> = ["ops", "scale", "deploy/web", "3", "-n", "payments"]
        .iter()
        .map(|word| (*word).to_string())
        .collect();
    let sentence = ops_line(
        &args,
        || {
            Err(
                "k8rs could not open its audit log at /nope/audit.log: it is not there — every \
                 change k8rs makes is written to that log before it is sent, so k8rs will not \
                 change anything until that is fixed, and reading your cluster still works"
                    .to_string(),
            )
        },
        unwired,
        unanswered,
    )
    .expect("a line beginning with `ops` is the operations driver's")
    .said;
    println!("{sentence}");
    assert_eq!(
        sentence,
        "k8rs: k8rs could not open its audit log at /nope/audit.log: it is not there — every \
         change k8rs makes is written to that log before it is sent, so k8rs will not change \
         anything until that is fixed, and reading your cluster still works",
        "a run that cannot record what it is about to do did not say so, or said it beside the \
         seam's own sentence as though the operation had gone ahead"
    );
}

// --- WHAT AN OPERATION ENDS AS ---
//
// **The driver's half of `scale`** (todo.md 3749): the exit code, the cluster the record names,
// the sentence a connection that never happened gets, and the three lines the headless surface
// prints instead of `screens/dialogs.md`'s box.
//
// **Everything here is a function over values except [`scale_connected`]**, which reads the
// reader's own kubeconfig and dials it. That one is glue and is deliberately eight lines: what it
// calls above and below is tested here, and what it does itself is proven by running the binary.

/// **Exit `0` iff the cluster changed, and `2` for every other ending** (NOTES § D220 ruling 1).
///
/// **The hazard is `echo no | k8rs ops delete … && kubectl get pod`.** Every ops line exited `2`
/// before this box, which read a cancellation as a failure and — the moment an arm was wired —
/// would have read a success as one too.
#[test]
fn only_a_cluster_that_changed_exits_zero() {
    let changed = ending(&ops::Performed {
        outcome: Some(ops::Outcome::Done),
        recorded: true,
    });
    println!("{} · exit {}", changed.said, changed.code);
    assert_eq!(changed.code, 0, "a change that happened did not exit 0");
    assert_eq!(changed.said, "k8rs: the change was made");
    for outcome in [
        Some(ops::Outcome::Cancelled),
        Some(ops::Outcome::Gone),
        Some(ops::Outcome::Changed),
        Some(ops::Outcome::NotSent {
            fault: k8s::Fault::Refused,
            said: None,
        }),
        Some(ops::Outcome::Failed {
            fault: k8s::Fault::Refused,
            said: None,
        }),
        // NOTES § D21's fourth ending: the attempt could not be recorded, so nothing was sent.
        None,
    ] {
        let ended = ending(&ops::Performed {
            outcome,
            recorded: true,
        });
        println!("{} · exit {}", ended.said, ended.code);
        assert_eq!(
            ended.code, 2,
            "an ending that changed nothing exited 0: {:?}",
            ended.said
        );
        assert!(
            ended.said.starts_with("k8rs: "),
            "an ops sentence lost its prefix: {:?}",
            ended.said
        );
    }
}

/// **A change k8rs could not write down still exits `0`, and says so out loud**
/// (NOTES § D220 ruling 1, § D214's fourth lie).
///
/// **Nothing read `ops::Performed::recorded` until this box** — `#[must_use]` is on the struct and
/// not on the field — so *the change was made and k8rs could not write it to the audit log*
/// reached nobody. It is a sentence on stderr and never an exit code: the code answers *did it
/// happen*, and a `2` here makes a script re-run a mutation that already landed.
#[test]
fn a_change_the_log_missed_exits_zero_and_says_the_trail_is_short() {
    let ended = ending(&ops::Performed {
        outcome: Some(ops::Outcome::Done),
        recorded: false,
    });
    println!("{} · exit {}", ended.said, ended.code);
    assert_eq!(ended.code, 0, "a change that happened did not exit 0");
    assert!(
        ended.said.contains("the change was made")
            && ended.said.contains("could not write that to the audit log"),
        "the operator was not told the trail is incomplete: {:?}",
        ended.said
    );
}

/// **A paused deployment is said out loud above the prompt, and nothing else is**
/// (NOTES § D224, invariant 4, invariant 14).
///
/// **The three things it has to say** are that this deployment is paused, that nothing moves until
/// somebody resumes it, and that the `kubectl rollout restart` line k8rs printed one line above
/// will refuse until then — measured: it exits `1` with *can't restart paused deployment (run
/// rollout resume first)* while k8rs printed *the change was made* and exited `0`.
///
/// **`Some(false)` and `None` say nothing at all.** `None` is a check that was never run;
/// `Some(false)` is every kind that has no pause, which is a StatefulSet, a DaemonSet, and the
/// Deployment nobody paused. A line printed for those would be a warning about a state the object
/// is not in, on the surface invariant 2 exists to keep readable.
///
/// **No jargon, and no second copy of the kubectl line k8rs already printed** — the object and the
/// namespace are on screen twice by the time this lands, and a second spelling of a command in
/// this file is the copy that drifts from the one `ops.rs` builds.
#[test]
fn a_paused_deployment_is_named_above_the_prompt_and_nothing_else_is() {
    let paused = while_paused("deployment", Some(&true)).expect("a paused deployment says so");
    println!("{paused}");
    for owed in [
        "This deployment is paused",
        "nothing will be replaced until somebody resumes it",
        "kubectl rollout resume",
        "the command above will refuse to run until then",
    ] {
        assert!(paused.contains(owed), "{owed:?} is not in {paused:?}");
    }
    // **Plain language is the rule and not a preference** (invariant 14): the words a beginner has
    // not met are the ones this sentence exists to avoid.
    for jargon in [
        "spec",
        "annotation",
        "patch",
        "replica",
        "rollout restart",
        "pod",
    ] {
        assert!(
            !paused.to_lowercase().contains(jargon),
            "the paused line uses {jargon:?}: {paused:?}"
        );
    }
    // **The kind is the one the driver resolved**, so nothing here hard-codes the only kind that
    // can currently be paused.
    assert!(
        while_paused("statefulset", Some(&true))
            .expect("the caller decides which kinds can answer true")
            .starts_with("This statefulset is paused"),
        "the sentence hard-codes a kind it was handed"
    );
    // **Nothing is said where nothing is wrong.**
    for checked in [Some(&false), None] {
        assert_eq!(
            while_paused("deployment", checked),
            None,
            "a deployment nobody paused was warned about anyway: {checked:?}"
        );
    }
}

/// One kubeconfig with one context and one cluster, as text kube parses.
fn one_context(server: Option<&str>) -> kube::config::Kubeconfig {
    let cluster = server.map_or(String::new(), |server| format!("\n    server: {server}"));
    serde_yaml_ng::from_str(&format!(
        "apiVersion: v1\n\
         kind: Config\n\
         current-context: kind-k8rs\n\
         contexts:\n\
         - name: kind-k8rs\n  \
           context:\n    \
             cluster: k8rs\n    \
             user: k8rs\n\
         clusters:\n\
         - name: k8rs\n  \
           cluster:{cluster}\n\
         users:\n\
         - name: k8rs\n  \
           user: {{}}\n"
    ))
    .expect("a kubeconfig this test wrote itself")
}

/// **[`two_contexts`] whose *current* row turns TLS verification off** — the one shape that can
/// tell `k8s::contexts(…, Some(key))` from `k8s::contexts(…, None)` by the answer alone
/// (`tester` F4, NOTES § D280 item 5): asked for nothing, `alpha` is current and insecure and
/// `main::tls_unverified` answers `true`; asked for a name the file does not hold, no row is
/// current and it answers `false`.
fn two_contexts_one_unverified() -> kube::config::Kubeconfig {
    serde_yaml_ng::from_str(
        "apiVersion: v1\n\
         kind: Config\n\
         current-context: alpha\n\
         contexts:\n\
         - name: alpha\n  \
           context: { cluster: a, user: k8rs }\n\
         - name: beta\n  \
           context: { cluster: b, user: k8rs }\n\
         clusters:\n\
         - name: a\n  \
           cluster:\n    \
             server: 'https://alpha.invalid:6443'\n    \
             insecure-skip-tls-verify: true\n\
         - name: b\n  \
           cluster: { server: 'https://beta.invalid:6443' }\n\
         users:\n\
         - name: k8rs\n  \
           user: {}\n",
    )
    .expect("a kubeconfig this test wrote itself")
}

/// **A second context in the file, so `--context` has a row to name that is not the file's own
/// current one** — the one shape [`one_context`] cannot make.
fn two_contexts(first: &str, second: &str) -> kube::config::Kubeconfig {
    serde_yaml_ng::from_str(&format!(
        "apiVersion: v1\n\
         kind: Config\n\
         current-context: alpha\n\
         contexts:\n\
         - name: alpha\n  \
           context:\n    \
             cluster: a\n    \
             user: k8rs\n\
         - name: beta\n  \
           context:\n    \
             cluster: b\n    \
             user: k8rs\n\
         clusters:\n\
         - name: a\n  \
           cluster:\n    \
             server: {first}\n\
         - name: b\n  \
           cluster:\n    \
             server: {second}\n\
         users:\n\
         - name: k8rs\n  \
           user: {{}}\n"
    ))
    .expect("a kubeconfig this test wrote itself")
}

/// **Which cluster the audit line names comes off the row k8rs connected to** (NOTES § D220
/// ruling 5) — the `current` row of the same `k8s::contexts` list the connection was chosen from.
///
/// **The blocking half, measured** (`k8s-admin`, 2026-09-24): the function used to take the
/// kubeconfig and ask `k8s::contexts(…, None)` for itself. That is right for an `ops` line, which
/// takes no `--context`, and wrong for the console, which now connects with one — so
/// `k8rs --context beta`, one restart, and `ops::Record::attempt_line` wrote
/// `context beta · server <alpha's URL>`. Invariant 4: **neither record may lie**, and the field
/// that lied is the one `ops::Mutation::server` exists to be the backstop for, because a context
/// name does not identify a cluster.
///
/// **The three `k8s::Address` states become two answers here**, because a log line has one job:
/// an address it can state, or the gap `ops::Record::attempt_line` already spells. Telling
/// *undefined* from *unreadable* apart is a screen's work, on a row somebody is looking at.
///
/// **The userinfo strip is on this path for free** (NOTES § D173), which is the whole reason the
/// server comes from here rather than being remembered a second time on the way into a file that
/// is kept.
#[test]
fn the_server_the_record_names_is_the_one_the_run_connected_to() {
    // **The finding itself**: two clusters in one file, and the answer follows `--context`.
    let two = two_contexts("https://alpha.invalid:6443", "https://beta.invalid:6443");
    assert_eq!(
        current_server(&k8s::contexts(&two, None)),
        "https://alpha.invalid:6443",
        "a run that named no context did not get the file's own current-context"
    );
    assert_eq!(
        current_server(&k8s::contexts(&two, Some("beta"))),
        "https://beta.invalid:6443",
        "the audit line would have named `context beta` beside alpha's server URL, which is \
         invariant 4's neither record may lie"
    );
    // A context the file does not have leaves no row marked current, so there is nothing to name
    // — and the connection under it fails anyway.
    assert_eq!(current_server(&k8s::contexts(&two, Some("gamma"))), "");

    let current =
        |kubeconfig: &kube::config::Kubeconfig| current_server(&k8s::contexts(kubeconfig, None));
    // **A reserved host and not the `127.0.0.1` a real kind writes**, because
    // `scripts/security-guard.py` reads a loopback URL in this tree as a second outbound path and
    // is right to; the port is what carries the point, and `ops_tests.rs`'s own fixture already
    // pays this price for the same reason.
    assert_eq!(
        current(&one_context(Some("https://k8rs-tests.invalid:41751"))),
        "https://k8rs-tests.invalid:41751"
    );
    // **A password in the URL never reaches the log** — `k8s::Address` dropped it before this
    // function saw it, and a second spelling of that strip is what would go stale.
    //
    // **Built rather than written**, the way `k8s_tests.rs` builds its own: a URL with userinfo
    // in it does not match `scripts/security-guard.py`'s reserved-host rule, because the guard
    // reads the whole authority — so the credential half is a separate literal.
    let with_password = format!("https://{}@{}", "root:hunter2", "prod.invalid:6443");
    assert_eq!(
        current(&one_context(Some(&with_password))),
        "https://prod.invalid:6443",
        "a kubeconfig password reached the audit log"
    );
    // An entry that names no cluster, and one whose address k8rs will not state: both are the gap.
    assert_eq!(current(&one_context(None)), "");
    assert_eq!(current(&one_context(Some("https://[oops"))), "");
    // No current context at all — nothing to name, and nothing to invent.
    let empty: kube::config::Kubeconfig =
        serde_yaml_ng::from_str("apiVersion: v1\nkind: Config\n").expect("an empty kubeconfig");
    assert_eq!(current(&empty), "");
}

/// **A run that never reached a cluster says nothing was changed, and then says why**
/// ([`because`], `PRIOR-ART § C1`).
///
/// **The lead clause is what [`live`]'s sibling has no need for.** A watch that cannot connect has
/// changed nothing by definition; an operation is a line somebody typed in order to change
/// something, and the first thing they need is that it did not happen.
#[test]
fn a_scale_that_never_connected_says_nothing_was_changed_and_names_the_reason() {
    let said = no_cluster(&k8s::NotConnected::Client {
        failure: kube::Error::Service(Box::new(
            kube::client::AuthError::UnrefreshableTokenResponse,
        )),
        renewal: Some("aws-iam-authenticator".to_string()),
    });
    println!("{said}");
    assert!(
        said.starts_with("k8rs: nothing was changed — "),
        "an operation that never connected did not say so first: {said:?}"
    );
    assert!(
        said.contains("aws-iam-authenticator"),
        "the login program the kubeconfig names was dropped: {said:?}"
    );
}

/// **All three operations are wired, and each gets what its own line carried** — todo.md
/// § Phase 7's `scale`, `restart` and `delete` boxes.
///
/// **A verb added to [`OPERATIONS`] without an arm answers `None`** rather than falling into
/// somebody else's, which is what stops a fourth operation being performed as a scale — the row
/// below the loop is the one that still exercises it, since none of the three is unwired now.
///
/// **This is [`ops_performed`]'s whole decision**, held apart from it because everything past the
/// decision dials a cluster: the function itself is one call, and what can be wrong about it is
/// the rows below.
#[test]
fn every_operation_in_the_table_gets_its_own_arm_and_a_verb_with_none_gets_nothing() {
    for operation in &OPERATIONS {
        let chosen = wired(operation, Some(3));
        println!("{} → {chosen:?}", operation.verb);
        assert_eq!(
            chosen,
            match operation.verb {
                SCALE => Some(Wired::Scale(3)),
                RESTART => Some(Wired::Restart),
                DELETE => Some(Wired::Delete),
                _ => None,
            },
            "{} is wired to the wrong arm of the seam",
            operation.verb
        );
    }
    // **A verb this build cannot perform reaches [`not_wired`] and never somebody else's arm** —
    // the safety net the loop above can no longer exercise, now that every row of [`OPERATIONS`]
    // is wired. A fourth operation added to the table without an arm here is the case.
    let unknown = Operation {
        verb: "evict",
        value: None,
        confirm: TYPE_THE_NAME,
    };
    assert_eq!(
        wired(&unknown, Some(3)),
        None,
        "an operation with no arm was performed as one that has one"
    );
    // **The derived list says what it found**: a table that stopped holding these three verbs
    // would satisfy every row above by matching none of them.
    assert_eq!(
        OPERATIONS
            .iter()
            .map(|operation| operation.verb)
            .collect::<Vec<_>>(),
        vec![SCALE, RESTART, DELETE],
        "the operations table is no longer the three this phase wires"
    );
    // **The count comes back and is not invented**, which is what stops a scale-to-three being
    // performed as a scale to anything else.
    let scale = operation_named(SCALE).expect("scale is in the table");
    assert_eq!(wired(scale, Some(0)), Some(Wired::Scale(0)));
    // `ops_value` refuses this above the seam, so it cannot arrive from a command line — and if
    // it ever did, it is `not_wired`'s sentence and never a guessed count.
    assert_eq!(wired(scale, None), None);
    // **A delete takes no value either**, so a count on the line changes nothing about it — the
    // driver refuses the extra word above this.
    let delete = operation_named(DELETE).expect("delete is in the table");
    assert_eq!(wired(delete, None), Some(Wired::Delete));
    assert_eq!(wired(delete, Some(3)), Some(Wired::Delete));
    // **A restart takes no value, so a count on the line changes nothing about it** — the driver
    // refuses the extra word above this, and the seam does not carry one either way.
    let restart = operation_named(RESTART).expect("restart is in the table");
    assert_eq!(wired(restart, None), Some(Wired::Restart));
    assert_eq!(wired(restart, Some(3)), Some(Wired::Restart));
}

/// **A kind an operation does not work on is refused before the audit log is opened**
/// (NOTES § D220 ruling 7) — the same rule `k8rs ops bogus` already had, reached through the one
/// door that was not a spelling mistake.
///
/// **What it asserts is that the opener was never called**, and it used to be named for the state
/// directory that opener would have created (`k8s-admin`, 2026-09-04). The two are the same fact
/// today only because `audit_log` is the only thing that makes the directory; the name now says
/// what the assertion can see, and the filesystem half lives in
/// `a_machine_that_cannot_hold_the_audit_log_refuses_the_operation_and_says_why`.
///
/// **And it is the one refusal in this driver that prints no synopsis** (NOTES § D236 ruling 4).
/// Every row below names a real operation and a real object and is told *k8rs does not do that to
/// that kind* — a reader who already has the shape right, so eight lines of it under a complete
/// answer bury the answer (invariant 14). What they get instead is one line saying where the
/// usage is. The form side is
/// `every_wrong_ops_line_names_what_is_wrong_and_carries_the_usage`, which asserts the opposite of
/// both halves.
#[test]
fn a_kind_an_operation_does_not_work_on_is_refused_before_the_audit_log_is_opened() {
    // **The two matrices are genuinely different and the rows say so** (NOTES § Operations): a
    // replicaset scales and does not restart, a daemonset restarts and does not scale. A driver
    // holding one copy of *which kinds* would have to be wrong about one of them.
    for (line, owed) in [
        (
            vec!["ops", "scale", "pod/web", "3", "-n", "payments"],
            "cannot scale a pod",
        ),
        (
            vec!["ops", "scale", "ds/fluentd", "3", "-n", "payments"],
            "cannot scale a daemonset",
        ),
        (
            vec!["ops", "scale", "node/worker-1", "3"],
            "cannot scale a node",
        ),
        (
            vec!["ops", "restart", "rs/web-abc", "-n", "payments"],
            // **And it says what to restart instead** (NOTES § D224) — a replicaset is the one
            // refused kind whose copies an operator would actually want replaced.
            "restarting that deployment is what replaces its copies",
        ),
        (
            vec!["ops", "restart", "node/worker-1"],
            "cannot restart a node",
        ),
        (
            vec!["ops", "restart", "pod/web", "-n", "payments"],
            "if this pod belongs to one, restart that instead",
        ),
    ] {
        let asked = std::cell::Cell::new(0u32);
        let args: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
        let ended = ops_line(
            &args,
            || {
                asked.set(asked.get() + 1);
                nowhere()
            },
            unwired,
            unanswered,
        )
        .expect("a line beginning with `ops` is the operations driver's");
        println!("--- {line:?} ---\n{}", ended.said);
        assert_eq!(
            asked.get(),
            0,
            "{line:?} is refused and asked for an audit log anyway"
        );
        assert_eq!(ended.code, 2, "{line:?} did not exit 2");
        assert!(
            ended.said.contains(owed),
            "{line:?} was refused for something other than its kind: {:?}",
            ended.said
        );
        // **The refusal names what the verb *can* be pointed at** (invariant 14), and the two
        // lists are not the same one.
        let names = if line[1] == "scale" {
            "a deployment, a statefulset and a replicaset"
        } else {
            "a deployment, a statefulset and a daemonset"
        };
        assert!(
            ended.said.contains(names),
            "{line:?} did not say what its verb works on: {:?}",
            ended.said
        );
        // **The answer is not buried** (NOTES § D236 ruling 4). Two claims, because dropping the
        // synopsis and pointing at it are two separate mistakes to make: no line of the eight is
        // printed, and the one line that replaces them is.
        assert!(
            !ended.said.contains("usage: k8rs ops "),
            "{line:?} knew the shape and was handed the synopsis anyway: {:?}",
            ended.said
        );
        assert_eq!(
            ended.said.lines().count(),
            2,
            "{line:?} is a sentence and a pointer, and it is neither: {:?}",
            ended.said
        );
        // **The sentence is spelled out and not read back off [`see_usage`]** (`cargo mutants`,
        // this turn): `ends_with(&see_usage())` moves with the thing it is checking, and a
        // `see_usage` replaced by `"xyzzy"` survived it.
        assert!(
            ended
                .said
                .ends_with("Run `k8rs ops` on its own to see everything it can do."),
            "{line:?} dropped the synopsis without saying where it is: {:?}",
            ended.said
        );
    }
    // **`delete` refuses no kind at all** (NOTES § D225 ruling 3): there is no `ops::deletable`,
    // so every kind the driver can name reaches the seam. It is the opposite claim from the rows
    // above, and a `deletable()` added later to refuse one of them turns this red.
    for kind in &KINDS {
        let object = format!("{}/web", kind.singular);
        let mut line = vec!["ops", DELETE, object.as_str()];
        if kind.namespaced {
            line.extend(["-n", "payments"]);
        }
        let ended = ops_ended(&line);
        println!("{}", ended.said);
        assert!(
            ended.said.contains("read this as `delete`"),
            "{} was refused by a matrix `delete` does not have: {:?}",
            kind.singular,
            ended.said
        );
    }
    // **The kinds each one does work on still reach the seam**, so a refusal that widened by one
    // word is a red build rather than a quieter driver.
    for (verb, kind) in [
        ("scale", "deploy"),
        ("scale", "sts"),
        ("scale", "rs"),
        ("restart", "deploy"),
        ("restart", "sts"),
        ("restart", "ds"),
    ] {
        let object = format!("{kind}/web");
        // `restart` takes no value, and a word after the object is its own refusal.
        let mut line = vec!["ops", verb, &object];
        if verb == SCALE {
            line.push("3");
        }
        line.extend(["-n", "payments"]);
        let ended = ops_ended(&line);
        println!("{}", ended.said);
        assert!(
            ended.said.contains(&format!("read this as `{verb}`")),
            "{verb} {kind} was refused by its own matrix: {:?}",
            ended.said
        );
    }
}

/// **A well-formed line carries an exit code out of the seam and never falls through**
/// (NOTES § D220 ruling 2).
///
/// **The seam's own answer is what decides it**, which is what the double proves: `ops_line`
/// hands back whatever the operation ended as rather than a constant, so a `scale` that succeeds
/// cannot come back as *this was not an ops line* and start a watch on a cluster it just changed.
#[test]
fn the_seams_own_ending_is_what_an_ops_line_comes_back_with() {
    let args: Vec<String> = ["ops", "scale", "deploy/web", "3", "-n", "payments"]
        .iter()
        .map(|word| (*word).to_string())
        .collect();
    let landed = ops_line(
        &args,
        nowhere,
        |_| Ended {
            said: "k8rs: the change was made".to_string(),
            code: 0,
        },
        unanswered,
    )
    .expect("a line beginning with `ops` is the operations driver's");
    println!("{} · exit {}", landed.said, landed.code);
    assert_eq!(landed.code, 0, "a seam that changed the cluster exited 2");
    assert_eq!(landed.said, "k8rs: the change was made");
    // **A note about the audit log is worth saying and is not worth a `2`** — it goes above the
    // seam's sentence and leaves the code alone.
    let noted = ops_line(
        &args,
        || nowhere().map(|(log, _)| (log, vec!["the audit log is 0666".to_string()])),
        |_| Ended {
            said: "k8rs: the change was made".to_string(),
            code: 0,
        },
        unanswered,
    )
    .expect("a line beginning with `ops` is the operations driver's");
    println!("{} · exit {}", noted.said, noted.code);
    assert_eq!(
        noted.code, 0,
        "an audit-log note turned a change into a failure"
    );
    assert_eq!(
        noted.said,
        "k8rs: the audit log is 0666\nk8rs: the change was made"
    );
}

/// **Every other ops line exits `2`**, whatever it was refused for — the whole of [`Ended`]'s
/// second half, over one row per refusal the driver can reach.
#[test]
fn every_refusal_of_an_ops_line_exits_two() {
    for line in [
        vec!["ops"],
        vec!["ops", "--wat", "scale", "deploy/web", "3"],
        vec!["ops", "bogus", "deploy/web"],
        vec!["ops", "scale", "web", "3"],
        vec!["ops", "scale", "deploy/web", "three", "-n", "payments"],
        vec!["ops", "scale", "deploy/web", "3"],
        vec!["ops", "scale", "pod/web", "3", "-n", "payments"],
        vec![
            "--read-only",
            "ops",
            "scale",
            "deploy/web",
            "3",
            "-n",
            "payments",
        ],
        vec!["--once", "ops", "scale", "deploy/web", "3"],
        // A kind the verb does not serve, which is the operation's own refusal and not the
        // driver's (NOTES § D220 ruling 7) — `restart` is wired now, so `deploy/web` is a
        // well-formed line and no longer belongs on this list.
        vec!["ops", "restart", "pod/web", "-n", "payments"],
        vec!["ops", "restart", "deploy/web"],
        // The seam's own refusal: `delete` is not written yet.
        vec!["ops", "delete", "pod/web", "-n", "payments"],
    ] {
        let ended = ops_ended(&line);
        println!("--- {line:?} ---\n{}", ended.said);
        assert_eq!(ended.code, 2, "{line:?} did not exit 2");
    }
}

/// **`ops` is on the list of what reaches a cluster, and the list is the whole list**
/// (todo.md 3749). The sentence around it has now been rewritten twice — *this build cannot
/// reach a cluster*, then *without --once, --live … this build reads files only*, now
/// *--once, --live … and ops are its other doors to a cluster* — and what this test is for has
/// survived all three: it is the sentence a reader checks before deciding whether it is safe to
/// experiment against production, so a door missing from it is the dangerous kind of omission.
#[test]
fn the_usage_says_ops_reaches_a_cluster_too() {
    let Err(problem) = run(&[]) else {
        panic!("no arguments is not a report")
    };
    println!("{problem}");
    let synopsis = problem.lines().next().expect("the usage has a first line");
    // **`ops <operation>` and not `k8rs ops <operation>`**: `--read-only` sits between them since
    // 2026-09-05 (NOTES § D234), and what this row is about is the subcommand being offered at
    // all, not what may precede it.
    assert!(
        synopsis.contains("ops <operation>"),
        "the synopsis offers no way to reach the one subcommand this build has: {synopsis:?}"
    );
    // **That the synopsis offers `--read-only` at all is pinned in `tests/binary.rs`, not here**
    // (NOTES § D234, `tester`, 2026-09-05). This assertion was here for one round and its failure
    // set is a strict *subset* of that one's: it calls [`run`] directly and never sees the
    // process, so a `main` that stops printing what `run` returns leaves it green — measured, the
    // planted `main` passed here and failed there. Two of the three ways the flag can go
    // undiscoverable are invisible from inside the crate, and the e2e test already spawns the
    // binary, so the pin costs one `contains` on an invocation that is paid for either way.
    let last = problem.lines().last().expect("the usage has a last line");
    assert!(
        last.contains("--yaml and ops are its other doors to a cluster"),
        "the line listing what reaches a cluster still leaves `ops` out of it: {last:?}"
    );
    // **Offered *and* explained.** A bracketed flag on a mutating form reads as *and then it
    // scales*; what it does is refuse, and the prose is where that fits without a fourth line
    // (`tests/binary.rs` counts three).
    assert!(
        last.contains("refuses every operation"),
        "the synopsis offers a flag it never says the effect of: {last:?}"
    );
}

/// **A scratch directory that takes itself away again — in `Drop`, so a panicking assertion still
/// cleans up** (NOTES § D185; `ops_tests.rs`'s `Dir` is the same shape one file over, and cannot
/// be shared with it without the `lib.rs` D50 refuses).
///
/// **The process id separates two `cargo test` runs and the thread id separates two callers
/// inside one**, which is the rule [`emptied_list`] already keeps in this file.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn named(what: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "k8rs-{what}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&path).expect("a scratch directory this test owns");
        Scratch(path)
    }

    /// A file under it, opened the way `ops::audit_log` opens the real one.
    fn file(&self, name: &str) -> std::fs::File {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.0.join(name))
            .expect("a scratch file this test owns")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A stub API server for one operation: it logs `METHOD target body` and answers every request
/// with `answer`. The address is built from whatever `:0` gave us, so there is no loopback URL
/// written down anywhere.
async fn ops_stub(
    answer: impl Fn(&str) -> (String, String) + Send + Sync + 'static,
) -> (kube::Client, Requests) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port");
    let address = listener.local_addr().expect("the port it picked");
    let asked: Requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = std::sync::Arc::clone(&asked);
    let answer = std::sync::Arc::new(answer);
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let log = std::sync::Arc::clone(&log);
            let answer = std::sync::Arc::clone(&answer);
            tokio::spawn(async move {
                let mut pending: Vec<u8> = Vec::new();
                loop {
                    let mut chunk = [0_u8; 4096];
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => pending.extend_from_slice(&chunk[..read]),
                    }
                    // A PATCH has a body, so a request does not end at the blank line: the header
                    // says how much more there is.
                    while let Some(end) =
                        pending.windows(4).position(|window| window == b"\r\n\r\n")
                    {
                        let head = String::from_utf8_lossy(&pending[..end]).to_string();
                        let length: usize = head
                            .lines()
                            .find_map(|line| {
                                line.split_once(':')
                                    .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                                    .and_then(|(_, value)| value.trim().parse().ok())
                            })
                            .unwrap_or(0);
                        if pending.len() < end + 4 + length {
                            break;
                        }
                        let body = String::from_utf8_lossy(&pending[end + 4..end + 4 + length])
                            .to_string();
                        pending.drain(..end + 4 + length);
                        let mut words = head.split_whitespace();
                        let asked = format!(
                            "{} {} {body}",
                            words.next().unwrap_or_default(),
                            words.next().unwrap_or_default()
                        )
                        .trim_end()
                        .to_string();
                        let (status, reply) = answer(&asked);
                        log.lock().expect("the log is never poisoned").push(asked);
                        let sent = format!(
                            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\n\
                             content-length: {}\r\n\r\n{reply}",
                            reply.len()
                        );
                        if socket.write_all(sent.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                }
            });
        }
    });
    let client = kube::Client::try_from(kube::Config::new(
        format!("http://{address}")
            .parse()
            .expect("an address the kernel just gave us"),
    ))
    .expect("a client over plain http asks the machine for nothing");
    (client, asked)
}

/// The `Scale` a cluster hands back for `deployment/web` in `payments`.
fn scale_answer(replicas: i32) -> String {
    format!(
        r#"{{"kind":"Scale","apiVersion":"autoscaling/v1","metadata":{{"name":"web",
           "namespace":"payments","uid":"18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11",
           "resourceVersion":"41751"}},"spec":{{"replicas":{replicas}}},
           "status":{{"replicas":{replicas}}}}}"#
    )
}

/// **One `k8rs ops scale`, from a client to an exit code** — [`scaled`] over a real socket, a
/// scripted confirmation and a real audit file.
///
/// **The three printed lines are `screens/dialogs.md` § *Printed instead of drawn*'s** — object
/// and namespace, the consequence, the `$` line, in that order, with the dialog's box removed and
/// nothing reworded for the terminal.
///
/// **The scale is to zero on purpose**, which is the relation that file prints in full: it is the
/// one that stops the app, and it gets no stricter a guard than any other scale.
#[tokio::test]
async fn a_headless_scale_prints_the_dialog_as_three_lines_and_exits_zero() {
    let (client, sent) = ops_stub(|_| ("200 OK".to_string(), scale_answer(3))).await;
    let log = Scratch::named("headless-scale");
    let path = log.0.join("audit.log");
    let audit = log.file("audit.log");
    let mut ready = Ready {
        operation: operation_named(SCALE).expect("scale is in the table"),
        kind: known_kind("deploy").expect("deploy is a kind the driver knows"),
        name: "web",
        count: Some(0),
        namespace: Some("payments"),
        audit,
    };
    let reached = Reached {
        client: &client,
        context: "kind-k8rs",
        server: "https://k8rs-tests.invalid:41751",
    };
    let mut out = Vec::new();

    let ended = scaled(
        &reached,
        &mut ready,
        0,
        || {
            "2026-09-03T12:34:56Z"
                .parse()
                .expect("a fixed timestamp inside jiff's range")
        },
        &mut "yes\n".as_bytes(),
        &mut out,
    )
    .await;

    let printed = String::from_utf8(out).expect("everything k8rs writes is a string");
    println!("{printed}\n--- exit {} ---\n{}", ended.code, ended.said);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines[..3],
        [
            "deployment/web in payments",
            "This stops all 3 copies of your app — nothing will be left running. Right now: 3 \
             copies. After: 0 copies.",
            "$ kubectl --context kind-k8rs scale deployment/web --replicas=0 -n payments",
        ],
        "the headless dialog is not the three lines screens/dialogs.md prints"
    );
    assert!(
        printed.contains("the cluster checked it first and accepted it")
            && printed.contains("type yes and press enter to go ahead"),
        "the verdict and the prompt did not reach the operator: {printed:?}"
    );
    // **`ended.said` is printed after this, and it has a line to land on.** Piped input echoes
    // nothing, so everything on stderr is what [`show`] and [`ask`] wrote — and until 2026-09-04
    // the last of it left the cursor mid-line and the closing sentence arrived glued to the prompt
    // (`k8s-admin`).
    assert!(
        printed.ends_with("anything else stops it:\n"),
        "the closing sentence lands on the back of the prompt: {printed:?}"
    );
    assert_eq!(ended.code, 0, "a scale that landed did not exit 0");
    assert_eq!(ended.said, "k8rs: the change was made");
    let requests = sent.lock().expect("the log is never poisoned").clone();
    assert_eq!(
        requests.len(),
        3,
        "a confirmed scale is a read, a check and a change: {requests:?}"
    );
    let written = std::fs::read_to_string(&path).expect("the audit log this run opened");
    println!("{written}");
    assert_eq!(
        written.lines().count(),
        2,
        "one mutation is one attempt line and one result line: {written:?}"
    );
    assert!(
        written.contains("attempt · deployment/web · context kind-k8rs")
            && written.contains("· the change was made"),
        "the audit log does not hold what happened: {written:?}"
    );
}

/// **Anything but the word `yes` stops it, and the exit code says so** — invariant 2 through the
/// driver, with no flag anywhere that means yes.
#[tokio::test]
async fn a_headless_scale_nobody_confirmed_changes_nothing_and_exits_two() {
    let (client, sent) = ops_stub(|_| ("200 OK".to_string(), scale_answer(2))).await;
    let log = Scratch::named("cancelled-scale");
    let audit = log.file("audit.log");
    let mut ready = Ready {
        operation: operation_named(SCALE).expect("scale is in the table"),
        kind: known_kind("deploy").expect("deploy is a kind the driver knows"),
        name: "web",
        count: Some(3),
        namespace: Some("payments"),
        audit,
    };
    let reached = Reached {
        client: &client,
        context: "kind-k8rs",
        server: "https://k8rs-tests.invalid:41751",
    };
    let mut out = Vec::new();

    let ended = scaled(
        &reached,
        &mut ready,
        3,
        || {
            "2026-09-03T12:34:56Z"
                .parse()
                .expect("a fixed timestamp inside jiff's range")
        },
        &mut "no\n".as_bytes(),
        &mut out,
    )
    .await;

    println!(
        "{}\n--- exit {} ---\n{}",
        String::from_utf8_lossy(&out),
        ended.code,
        ended.said
    );
    assert_eq!(ended.code, 2, "a scale nobody confirmed exited 0");
    assert_eq!(
        ended.said,
        "k8rs: nobody confirmed it, so nothing was changed"
    );
    assert_eq!(
        sent.lock().expect("the log is never poisoned").len(),
        2,
        "a cancelled scale sent the change anyway"
    );
}

/// **A cluster that will not answer the read is a refusal with an exit `2`, and the operator is
/// told which of their two problems it is** — the RBAC or the network (`PRIOR-ART § C1`).
#[tokio::test]
async fn a_headless_scale_the_cluster_refused_says_which_refusal_it_was_and_exits_two() {
    let (client, _) = ops_stub(|_| {
        (
            "403 Forbidden".to_string(),
            r#"{"kind":"Status","apiVersion":"v1","status":"Failure","code":403,
               "reason":"Forbidden","message":"deployments.apps \"web\" is forbidden"}"#
                .to_string(),
        )
    })
    .await;
    let log = Scratch::named("refused-scale");
    let audit = log.file("audit.log");
    let mut ready = Ready {
        operation: operation_named(SCALE).expect("scale is in the table"),
        kind: known_kind("deploy").expect("deploy is a kind the driver knows"),
        name: "web",
        count: Some(3),
        namespace: Some("payments"),
        audit,
    };
    let reached = Reached {
        client: &client,
        context: "kind-k8rs",
        server: "",
    };
    let mut out = Vec::new();

    let ended = scaled(
        &reached,
        &mut ready,
        3,
        || {
            "2026-09-03T12:34:56Z"
                .parse()
                .expect("a fixed timestamp inside jiff's range")
        },
        &mut "yes\n".as_bytes(),
        &mut out,
    )
    .await;

    println!("--- exit {} ---\n{}", ended.code, ended.said);
    assert_eq!(ended.code, 2);
    assert!(
        ended
            .said
            .starts_with("k8rs: k8rs could not read how many copies of deployment/web"),
        "{:?}",
        ended.said
    );
    assert!(
        ended.said.contains("the cluster would not allow it"),
        "a 403 was not told apart from a dead socket: {:?}",
        ended.said
    );
    assert!(
        out.is_empty(),
        "a scale that never had a consequence to state printed one anyway: {:?}",
        String::from_utf8_lossy(&out)
    );
}

/// **What a cluster hands back from a patched Deployment** — enough of one for `DynamicObject` to
/// deserialise, and nothing the driver reads.
fn restart_answer() -> String {
    r#"{"apiVersion":"apps/v1","kind":"Deployment","metadata":{"name":"web",
       "namespace":"payments","uid":"18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11",
       "resourceVersion":"41752"},"spec":{},"status":{}}"#
        .to_string()
}

/// **One `k8rs ops restart`, from a client to an exit code** — [`restarted`] over a real socket, a
/// scripted confirmation and a real audit file (todo.md 3777).
///
/// **The three printed lines are `screens/dialogs.md` § *Printed instead of drawn*'s** — object
/// and namespace, the consequence, the `$` line, in that order, with the dialog's box removed and
/// nothing reworded for the terminal.
///
/// **The `$` line carries no dry-run flag**, because `kubectl rollout restart` has none
/// (NOTES § D223 ruling 4) — so the verdict above it says k8rs checked this, and the line under it
/// never claims the operator's own command could.
///
/// **Two requests and no third**, which is the driver's half of *nothing is read before the patch*
/// (NOTES § D223 ruling 3): a scale opens with a `GET` and this does not.
#[tokio::test]
async fn a_headless_restart_prints_the_dialog_as_three_lines_and_exits_zero() {
    let (client, sent) = ops_stub(|_| ("200 OK".to_string(), restart_answer())).await;
    let log = Scratch::named("headless-restart");
    let path = log.0.join("audit.log");
    let audit = log.file("audit.log");
    let mut ready = Ready {
        operation: operation_named(RESTART).expect("restart is in the table"),
        kind: known_kind("deploy").expect("deploy is a kind the driver knows"),
        name: "web",
        count: None,
        namespace: Some("payments"),
        audit,
    };
    let reached = Reached {
        client: &client,
        context: "kind-k8rs",
        server: "https://k8rs-tests.invalid:41751",
    };
    let mut out = Vec::new();

    let ended = restarted(
        &reached,
        &mut ready,
        || {
            "2026-09-03T12:34:56Z"
                .parse()
                .expect("a fixed timestamp inside jiff's range")
        },
        &mut "yes\n".as_bytes(),
        &mut out,
    )
    .await;

    let printed = String::from_utf8(out).expect("everything k8rs writes is a string");
    println!("{printed}\n--- exit {} ---\n{}", ended.code, ended.said);
    let lines: Vec<&str> = printed.lines().collect();
    assert_eq!(
        lines[..3],
        [
            "deployment/web in payments",
            "This asks Kubernetes to replace every copy of your app with a new one. How many \
             stop at the same time is a setting on this deployment — it can be a few, or all of \
             them at once. A paused deployment will not start until you resume it.",
            "$ kubectl --context kind-k8rs rollout restart deployment/web -n payments",
        ],
        "the headless dialog is not the three lines screens/dialogs.md prints"
    );
    // **A deployment nobody paused is not warned about one** (NOTES § D224) — the line below is
    // the check's answer and not a hedge printed every time.
    assert!(
        !printed.contains("is paused, so nothing will be replaced"),
        "a deployment the cluster said nothing about was called paused: {printed:?}"
    );
    assert!(
        !printed.contains("dry-run"),
        "the taught line offers a flag `kubectl rollout restart` does not have: {printed:?}"
    );
    assert!(
        printed.contains("the cluster checked it first and accepted it")
            && printed.contains("type yes and press enter to go ahead"),
        "the verdict and the prompt did not reach the operator: {printed:?}"
    );
    assert_eq!(ended.code, 0, "a restart that landed did not exit 0");
    assert_eq!(ended.said, "k8rs: the change was made");
    let requests = sent.lock().expect("the log is never poisoned").clone();
    println!("{}", requests.join("\n"));
    assert_eq!(
        requests.len(),
        2,
        "a confirmed restart is a check and a change, and reads nothing first: {requests:?}"
    );
    assert!(
        requests.iter().all(
            |request| request.contains("kubectl.kubernetes.io/restartedAt")
                && !request.contains("kube.kubernetes.io/restartedAt")
        ),
        "the patch carries kube's own annotation key rather than kubectl's: {requests:?}"
    );
    let written = std::fs::read_to_string(&path).expect("the audit log this run opened");
    println!("{written}");
    assert_eq!(
        written.lines().count(),
        2,
        "one mutation is one attempt line and one result line: {written:?}"
    );
    assert!(
        written.contains(
            "kubectl: kubectl --context kind-k8rs rollout restart deployment/web -n payments \
             · call: PATCH \
             /apis/apps/v1/namespaces/payments/deployments/web · resourceVersion not sent"
        ) && written.contains("· no uid was read ·")
            && written.contains("· the change was made"),
        "the audit log does not hold the call that was made: {written:?}"
    );
}

/// **The same Deployment after `kubectl rollout pause`** — [`restart_answer`] with the one field
/// the check's answer is read for (NOTES § D224).
fn paused_answer() -> String {
    r#"{"apiVersion":"apps/v1","kind":"Deployment","metadata":{"name":"web",
       "namespace":"payments","uid":"18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11",
       "resourceVersion":"41752"},"spec":{"paused":true},"status":{}}"#
        .to_string()
}

/// **A paused deployment is said out loud above the prompt, and the operator still decides**
/// (NOTES § D224) — [`while_paused`] wired, over a real socket, which is the half a test of the
/// sentence alone cannot see.
///
/// **The blocker this closes made three records lie at once.** On a real cluster the apiserver
/// accepts this patch, so the consequence promised every copy replaced, the dry-run passed, k8rs
/// printed *the change was made* and exited `0`, and the three pods still had the same names
/// twelve seconds later — while the `kubectl rollout restart` line k8rs had just taught exits `1`.
///
/// **It is a line and not a refusal.** Both requests still go out, the prompt is still asked and
/// the exit code is still `0`: the annotation is written on resume, and what was wrong was the
/// record and not the write.
#[tokio::test]
async fn a_headless_restart_of_a_paused_deployment_says_so_above_the_prompt_and_still_asks() {
    let (client, sent) = ops_stub(|_| ("200 OK".to_string(), paused_answer())).await;
    let log = Scratch::named("paused-restart");
    let audit = log.file("audit.log");
    let mut ready = Ready {
        operation: operation_named(RESTART).expect("restart is in the table"),
        kind: known_kind("deploy").expect("deploy is a kind the driver knows"),
        name: "web",
        count: None,
        namespace: Some("payments"),
        audit,
    };
    let reached = Reached {
        client: &client,
        context: "kind-k8rs",
        server: "https://k8rs-tests.invalid:41751",
    };
    let mut out = Vec::new();

    let ended = restarted(
        &reached,
        &mut ready,
        || {
            "2026-09-03T12:34:56Z"
                .parse()
                .expect("a fixed timestamp inside jiff's range")
        },
        &mut "yes\n".as_bytes(),
        &mut out,
    )
    .await;

    let printed = String::from_utf8(out).expect("everything k8rs writes is a string");
    println!("{printed}\n--- exit {} ---\n{}", ended.code, ended.said);
    let lines: Vec<&str> = printed.lines().collect();
    // **Above the prompt, and under the command it is about** — a reader meets the `$` line and
    // then the reason it will not work.
    assert_eq!(
        lines[3],
        "This deployment is paused, so nothing will be replaced until somebody resumes it with \
         kubectl rollout resume — and the command above will refuse to run until then.",
        "the paused line is not the one under the taught command: {lines:?}"
    );
    assert_eq!(
        lines[2], "$ kubectl --context kind-k8rs rollout restart deployment/web -n payments",
        "the paused line moved above the command it is about: {lines:?}"
    );
    assert!(
        lines[4..]
            .iter()
            .any(|line| line.contains("type yes and press enter")),
        "the prompt did not follow the warning: {lines:?}"
    );
    // **The operator still decides and the exit code does not move.**
    assert_eq!(
        ended.code, 0,
        "a paused deployment turned a restart into an exit 2"
    );
    assert_eq!(ended.said, "k8rs: the change was made");
    assert_eq!(
        sent.lock().expect("the log is never poisoned").len(),
        2,
        "reading the check's answer changed how many requests a restart makes"
    );
}

/// **Anything but the word `yes` stops it, and the exit code says so** — invariant 2 through the
/// driver, over the second operation as well as the first.
#[tokio::test]
async fn a_headless_restart_nobody_confirmed_changes_nothing_and_exits_two() {
    let (client, sent) = ops_stub(|_| ("200 OK".to_string(), restart_answer())).await;
    let log = Scratch::named("cancelled-restart");
    let audit = log.file("audit.log");
    let mut ready = Ready {
        operation: operation_named(RESTART).expect("restart is in the table"),
        kind: known_kind("ds").expect("ds is a kind the driver knows"),
        name: "fluentd",
        count: None,
        namespace: Some("logging"),
        audit,
    };
    let reached = Reached {
        client: &client,
        context: "kind-k8rs",
        server: "https://k8rs-tests.invalid:41751",
    };
    let mut out = Vec::new();

    let ended = restarted(
        &reached,
        &mut ready,
        || {
            "2026-09-03T12:34:56Z"
                .parse()
                .expect("a fixed timestamp inside jiff's range")
        },
        &mut "no\n".as_bytes(),
        &mut out,
    )
    .await;

    let printed = String::from_utf8(out).expect("everything k8rs writes is a string");
    println!("{printed}\n--- exit {} ---\n{}", ended.code, ended.said);
    // **A daemonset's consequence is not a deployment's**, and the driver hands the kind over
    // rather than deciding this itself (NOTES § D220 ruling 4).
    assert!(
        printed.contains(
            "This asks Kubernetes to replace the copy of your app on each node it runs on."
        ),
        "a daemonset was described as a deployment: {printed:?}"
    );
    assert_eq!(ended.code, 2, "a restart nobody confirmed exited 0");
    assert_eq!(
        ended.said,
        "k8rs: nobody confirmed it, so nothing was changed"
    );
    assert_eq!(
        sent.lock().expect("the log is never poisoned").len(),
        1,
        "a cancelled restart sent the change anyway"
    );
}

// --- WHAT A QUESTION READS AS ---
//
// **`ops may-i` is the one `ops` line that changes nothing** (NOTES § D23, § D229), so what it
// owes is a different list from the three above it: no confirmation, no audit line, no state
// directory, and an exit code that tells a yes from a no from *k8rs could not find out*.
//
// **Everything here is a function over values except [`may_i_connected`]**, which is the same
// eight lines of glue [`ops_connected`] is and is proven by running the binary.
//
// **The whole of what the operator review sent back was in this half of the box**
// (NOTES § D230): the matcher one file down was measured correct and every defect was in how
// this driver reads a typed string. So the rows below are the review's own measurements turned
// into assertions — what `/` means, what a missing group means, what a `no` exits with, and
// which review a single question goes to.

/// **`/` is the object's own name and the subresource is a flag** — `kubectl auth can-i`'s
/// meaning of both (NOTES § D230 ruling 1).
///
/// **The first two rows are the measurement that reversed this.** Under a rule with
/// `resourceNames: [only-this-pod]`, `kubectl auth can-i delete pods/only-this-pod` is **yes** and
/// k8rs answered **no**, because the `/` was read as a subresource — one string, two questions,
/// two answers, in two tools this file's own comment claimed shared a spelling
/// (`reports/2026-09-05-may-i-against-a-real-cluster.md` § 3c).
///
/// **Each row changes one part**, so a parse that dropped the group, read the `/` as a
/// subresource again, or took the `.` before the `/` says which.
#[test]
fn a_may_i_line_reads_the_slash_as_the_object_name_and_the_subresource_as_a_flag() {
    for (line, expected) in [
        (
            vec!["may-i", "delete", "pods."],
            ("delete", "", "pods", None, None),
        ),
        (
            vec!["may-i", "delete", "pods./only-this-pod"],
            ("delete", "", "pods", None, Some("only-this-pod")),
        ),
        (
            vec!["may-i", "patch", "deployments.apps"],
            ("patch", "apps", "deployments", None, None),
        ),
        (
            vec!["may-i", "patch", "deployments.apps", "--subresource=scale"],
            ("patch", "apps", "deployments", Some("scale"), None),
        ),
        (
            vec![
                "may-i",
                "patch",
                "deployments.apps/web",
                "--subresource",
                "scale",
            ],
            ("patch", "apps", "deployments", Some("scale"), Some("web")),
        ),
        // The group can carry dots of its own, so the split is on the **first** one.
        (
            vec!["may-i", "create", "pods.something.invented"],
            ("create", "something.invented", "pods", None, None),
        ),
    ] {
        let rest: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
        let words = ops_words(&rest).unwrap_or_else(|refusal| panic!("{refusal}"));
        let question = may_i_question(&words, &rest).unwrap_or_else(|refusal| panic!("{refusal}"));
        println!("{line:?} → {}", may_i_asked(&question));
        assert_eq!(
            (
                question.verb.as_str(),
                question.group.as_str(),
                question.resource.as_str(),
                question.subresource.as_deref(),
                question.name.as_deref(),
            ),
            expected,
            "{line:?} was read as a different question"
        );
    }
}

/// **A resource with no API group on it is refused and never answered `no`**
/// (NOTES § D230 ruling 2).
///
/// **Measured wrong, not reasoned wrong.** `patch deployments` defaulted the group to `""`,
/// matched nothing in the core group and printed *no* under a login that `kubectl auth can-i`
/// said **yes** for — for `deployments`, `deployment`, `deploy`, `pod` and `po` alike
/// (`reports/2026-09-05-may-i-against-a-real-cluster.md` § 3e). The cluster said no such thing.
///
/// **The trailing dot is the core group and the negative row is what makes this a test.** Without
/// `pods.` passing, an implementation that refused every resource word would satisfy the refusals
/// and answer nothing.
#[test]
fn a_resource_with_no_group_is_refused_and_the_core_group_is_a_trailing_dot() {
    for word in ["deployments", "deployment", "deploy", "pods", "pod", "po"] {
        let line = vec!["may-i".to_string(), "patch".to_string(), word.to_string()];
        let words: Vec<&str> = line.iter().map(String::as_str).collect();
        let refusal = may_i_question(&words, &line)
            .err()
            .unwrap_or_else(|| panic!("{word} was answered rather than refused"));
        println!("{word}\n{refusal}");
        assert!(
            refusal.contains("needs the API group as well as the resource"),
            "{word} was refused for the wrong reason: {refusal:?}"
        );
        // **It names the fix**, which is the half that makes a refusal usable at 3am.
        assert!(
            refusal.contains("deployments.apps") && refusal.contains("pods."),
            "{word}'s refusal does not say how to write it: {refusal:?}"
        );
        // **And it is a refusal and not a verdict** — no `no` anywhere in it, which is the
        // measured defect this row closes.
        assert!(
            !refusal.contains("is not allowed to do that"),
            "{word} was still answered as a refusal by the cluster: {refusal:?}"
        );
    }
    let line = vec![
        "may-i".to_string(),
        "patch".to_string(),
        "pods.".to_string(),
    ];
    let words: Vec<&str> = line.iter().map(String::as_str).collect();
    let core = may_i_question(&words, &line).expect("`pods.` names the core group");
    assert_eq!((core.resource.as_str(), core.group.as_str()), ("pods", ""));
}

/// **The question is printed back before the answer is** — so an answer is never read against a
/// question k8rs did not ask.
///
/// **The group, the name and the subresource appear only when the line carried them**, because a
/// printed `deployments.` or `nodes/` is a question nobody asked — and the core group's own
/// trailing dot is a spelling the line requires rather than something a reader means, so it is not
/// echoed either.
#[test]
fn a_question_is_printed_back_in_the_words_it_was_asked_in() {
    for (line, expected) in [
        (
            vec!["may-i", "delete", "nodes."],
            "may this login delete nodes?",
        ),
        (
            vec!["may-i", "delete", "pods./only-this-pod", "-n", "payments"],
            "may this login delete pods/only-this-pod in payments?",
        ),
        (
            vec![
                "may-i",
                "patch",
                "deployments.apps",
                "--subresource=scale",
                "-n",
                "payments",
            ],
            "may this login patch deployments.apps (subresource: scale) in payments?",
        ),
    ] {
        let rest: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
        let words = ops_words(&rest).unwrap_or_else(|refusal| panic!("{refusal}"));
        let question = may_i_question(&words, &rest).unwrap_or_else(|refusal| panic!("{refusal}"));
        let asked = may_i_asked(&question);
        println!("{asked}");
        assert_eq!(asked, expected);
    }
}

/// **A word out of argv is echoed as it printed and never as it was typed** (invariant 9,
/// invariant 4) — [`shown`]'s rule, on the one line in this region that echoes five values.
///
/// A verb with a bidi override in it must not come back looking like a verb somebody typed.
#[test]
fn a_question_says_when_the_words_it_echoes_are_not_the_words_that_were_typed() {
    let line = vec!["may-i", "de\u{202e}lete", "nodes."];
    let rest: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
    let question = may_i_question(&line, &rest).expect("a verb is not checked against a name rule");
    let asked = may_i_asked(&question);
    println!("{asked}");
    assert!(
        !asked.contains('\u{202e}'),
        "a control character reached the screen: {asked:?}"
    );
    assert!(
        asked.contains("(with what cannot print removed)"),
        "the echo is not the word that was judged and does not say so: {asked:?}"
    );
}

/// **Every way a `may-i` line is refused**, each naming what is wrong with it.
///
/// **`-n` and `--subresource` are both checked here.** A namespace narrows the question and a
/// subresource is a different question entirely (NOTES § D230 ruling 1), so a line whose value for
/// either is missing is a question about nothing.
#[test]
fn a_may_i_line_that_is_not_a_question_is_refused_and_says_which_part() {
    for (line, expected) in [
        (vec!["may-i"], "needs a verb and a resource"),
        (vec!["may-i", "delete"], "needs a verb and a resource"),
        (
            vec!["may-i", "delete", "nodes.", "extra"],
            "does not know what to do with",
        ),
        (
            vec!["may-i", "delete", ".apps"],
            "empty verb, resource or object name",
        ),
        (
            vec!["may-i", "delete", "pods./"],
            "empty verb, resource or object name",
        ),
        (vec!["may-i", "", "pods."], "empty verb, resource or object"),
        (vec!["may-i", "delete", "pods.", "-n", "NOPE"], "is not one"),
        (
            vec!["may-i", "delete", "pods.", "-n"],
            "needs the name of a namespace",
        ),
        (
            vec!["may-i", "delete", "pods.", "--subresource"],
            "needs the name of a subresource",
        ),
        (
            vec!["may-i", "delete", "pods.", "--subresource="],
            "needs the name of a subresource",
        ),
    ] {
        let rest: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
        let words = ops_words(&rest).unwrap_or_else(|refusal| panic!("{line:?}: {refusal}"));
        let refusal = may_i_question(&words, &rest)
            .err()
            .unwrap_or_else(|| panic!("{line:?} was read as a question"));
        println!("{line:?}\n{refusal}");
        assert!(
            refusal.contains(expected),
            "{line:?} was refused for the wrong reason: {refusal:?}"
        );
        // Every refusal on an `ops` line carries the usage under it, and this one is no exception.
        assert!(refusal.contains("ops may-i <verb>"));
    }
}

/// **`0` yes · `1` no · `2` k8rs could not find out** (NOTES § D230 ruling 4) — `kubectl auth
/// can-i`'s vocabulary, so a script reads the answer off the code instead of grepping an English
/// sentence invariant 14 will keep rewriting.
///
/// **The `CouldNotTell` row is the one this box exists for** (D229 ruling 4). A refused probe
/// exiting `1` would be a script reading *k8rs could not find out* as the cluster's no, which is
/// why the sentence is asserted beside the code.
#[test]
fn a_question_exits_zero_for_yes_one_for_no_and_two_when_k8rs_could_not_find_out() {
    let question = Question {
        verb: "delete".to_string(),
        group: String::new(),
        resource: "nodes".to_string(),
        subresource: None,
        name: None,
        namespace: None,
    };
    for (verdict, code) in [
        (ops::Verdict::Yes, 0),
        (ops::Verdict::No, 1),
        (
            ops::Verdict::CouldNotTell("the cluster said nothing".to_string()),
            2,
        ),
    ] {
        let ended = may_i_ended(&question, &verdict);
        println!("--- exit {} ---\n{}", ended.code, ended.said);
        assert_eq!(ended.code, code, "{verdict:?} exited the wrong way");
        assert!(
            ended
                .said
                .starts_with("k8rs: may this login delete nodes?\nk8rs: "),
            "the answer is not printed under the question it answers: {:?}",
            ended.said
        );
    }
    // **Neither answer claims the cluster's authority** (NOTES § D230 ruling 6): `Permits::may`
    // over-reports on `resourceNames` by design, and *"the cluster says"* over that was measured
    // against a `kubectl auth can-i` saying the opposite.
    for verdict in [ops::Verdict::Yes, ops::Verdict::No] {
        let said = may_i_ended(&question, &verdict).said;
        assert!(
            !said.contains("the cluster says"),
            "an answer claimed the cluster said something it may not have: {said:?}"
        );
    }
    // **The sentence is the other half of the ruling**: a script reads the code, a person reads
    // this, and neither may take a probe that could not run for the cluster saying no.
    let unsure = may_i_ended(
        &question,
        &ops::Verdict::CouldNotTell("the cluster said nothing".to_string()),
    );
    assert!(
        unsure.said.contains("That is not a no"),
        "a refused probe does not say it is not a refusal: {:?}",
        unsure.said
    );
    assert!(
        may_i_ended(&question, &ops::Verdict::No)
            .said
            .contains("is not allowed to do that"),
        "a cluster that said no was not reported as one"
    );
}

/// **`k8rs ops may-i` opens no state directory and never reaches an operation** — the branch is
/// taken above [`operation_named`], so nothing below it runs.
///
/// **The audit double asserts it was never called**, which is what says a probe writes no audit
/// line (NOTES § D221: the log records mutations, and a question is not one).
#[test]
fn a_may_i_line_opens_no_audit_log_and_performs_no_operation() {
    let line: Vec<String> = ["ops", "may-i", "delete", "nodes.", "extra"]
        .iter()
        .map(|word| (*word).to_string())
        .collect();
    let opened = std::cell::Cell::new(false);
    let performed = std::cell::Cell::new(false);
    let ended = ops_line(
        &line,
        || {
            opened.set(true);
            Err("the log should never have been opened".to_string())
        },
        |_| {
            performed.set(true);
            Ended::refused("an operation should never have been reached".to_string())
        },
        unanswered,
    )
    .expect("a line with `ops` first is this function's");
    println!("--- exit {} ---\n{}", ended.code, ended.said);
    assert!(!opened.get(), "a question opened the audit log");
    assert!(!performed.get(), "a question was performed as an operation");
    assert!(
        ended.said.contains("does not know what to do with"),
        "the refusal is not the question's own: {:?}",
        ended.said
    );
}

/// **`--read-only` permits a question and still refuses every operation** (NOTES § D230
/// ruling 3).
///
/// **Measured backwards before this ruling**: the reader most likely to ask what they are allowed
/// to do was told *"--read-only was asked for, so k8rs will not change anything"*. Invariant 2's
/// subject is the write path, and `ops::may_i` is the first thing in `ops.rs` that is not one —
/// put there for NOTES § D23's mechanical reason, which is exactly the price D23 said it was
/// paying.
///
/// **The three operations are the negative and they are not decoration**: a carve-out that let
/// every `ops` line past would be the flag doing nothing at all.
#[test]
fn read_only_permits_a_question_and_still_refuses_every_operation() {
    // The question reaches the seam, which is where a test with no cluster stops — so what is
    // asserted is that the refusal is *gone*, not that an answer arrived.
    //
    // **Both positions of the flag**, because only one was measured and the other is where the
    // first fix stopped working: `--read-only` before `ops` is the shape the review ran, and after
    // it is the shape [`ops_words`] refuses as an unknown flag unless the carve-out takes it out
    // of the slice first.
    for asking in [
        vec!["--read-only", "ops", "may-i", "delete", "nodes.", "extra"],
        vec!["ops", "may-i", "delete", "nodes.", "extra", "--read-only"],
    ] {
        let asking: Vec<String> = asking.iter().map(|word| (*word).to_string()).collect();
        let ended = ops_line(
            &asking,
            || panic!("a question opens no audit log"),
            |_| panic!("a question performs nothing"),
            unanswered,
        )
        .expect("a line with `ops` on it is this function's");
        println!("--- {asking:?} → exit {} ---\n{}", ended.code, ended.said);
        assert!(
            !ended.said.contains("will not change anything"),
            "a question that changes nothing was refused for not changing anything: {:?}",
            ended.said
        );
        assert!(
            ended.said.contains("does not know what to do with"),
            "the question was not read as one: {:?}",
            ended.said
        );
    }

    for verb in [SCALE, RESTART, DELETE] {
        let line: Vec<String> = [READ_ONLY, "ops", verb, "deploy/web", "3", "-n", "payments"]
            .iter()
            .map(|word| (*word).to_string())
            .collect();
        let ended = ops_line(
            &line,
            || panic!("a refused line opens no audit log"),
            |_| panic!("a refused line performs nothing"),
            unanswered,
        )
        .expect("a line with `ops` on it is this function's");
        println!("--- {verb} → exit {} ---\n{}", ended.code, ended.said);
        assert_eq!(ended.code, 2);
        assert!(
            ended.said.contains("--read-only was asked for"),
            "{verb} was not refused under --read-only: {:?}",
            ended.said
        );
    }
}

/// **`--subresource` belongs to the question and to nothing else** (NOTES § D230 ruling 1).
///
/// [`ops_words`] consumes the flag for every line, because it runs before the verb is known — so
/// without this refusal a mutation carrying it would be performed with a flag silently dropped,
/// which is the shape that function refuses for every other unknown flag.
#[test]
fn only_a_question_takes_the_subresource_flag() {
    for verb in [SCALE, RESTART, DELETE] {
        let line: Vec<String> = [
            "ops",
            verb,
            "deploy/web",
            "3",
            "-n",
            "payments",
            "--subresource=scale",
        ]
        .iter()
        .map(|word| (*word).to_string())
        .collect();
        let ended = ops_line(
            &line,
            || panic!("a refused line opens no audit log"),
            |_| panic!("a refused line performs nothing"),
            unanswered,
        )
        .expect("a line with `ops` on it is this function's");
        println!("--- {verb} → exit {} ---\n{}", ended.code, ended.said);
        assert_eq!(ended.code, 2);
        assert!(
            ended.said.contains("does not take --subresource"),
            "{verb} accepted a flag it does not read: {:?}",
            ended.said
        );
    }
    // **Named twice is refused for the namespace's own reason** — a read may resolve a repeat
    // ([`value_of`], last-wins like `kubectl` since the flags box) because it can be re-run, and
    // a question about permission that asks about the wrong subresource cannot be taken back.
    //
    // **Both spellings, because they are counted on two different lines** (my own second pass,
    // found by `just mutants-diff`): a row with only the attached form left `replace += with *=`
    // surviving on the separate-value branch, and a counter stuck at zero is a flag that can be
    // named as often as you like.
    for tail in [
        vec!["--subresource=scale", "--subresource=status"],
        vec!["--subresource", "scale", "--subresource", "status"],
        vec!["--subresource=scale", "--subresource", "status"],
    ] {
        let mut line = vec!["ops", "may-i", "patch", "deployments.apps"];
        line.extend(tail.iter().copied());
        let line: Vec<String> = line.iter().map(|word| (*word).to_string()).collect();
        let ended = ops_line(
            &line,
            || panic!("a refused line opens no audit log"),
            |_| panic!("a refused line performs nothing"),
            unanswered,
        )
        .expect("a line with `ops` on it is this function's");
        println!("{tail:?}\n{}", ended.said);
        assert!(
            ended.said.contains("names the subresource more than once"),
            "{tail:?}: two subresources were guessed between: {:?}",
            ended.said
        );
    }
}

/// **The usage says the question exists, in both places a reader can find it** — the top-level
/// synopsis, which is the only place a reader learns a mode exists, and `k8rs ops`'s own rows.
///
/// **The synopsis leads with the console, and its last line no longer denies one**
/// (`screens/states.md` § The command line's own synopsis, which is this text's authority and was
/// written for the flags box).
///
/// **Two runs printed the contradiction** (`k8s-admin`, 2026-09-24). `k8rs --read-only | cat`
/// printed a usage with no line for the flag just typed; `k8rs --read-only pod.json` printed the
/// new refusal — *`--read-only` on its own opens the console, which reads a cluster* — directly
/// above *without --once, --live, --logs, --describe, --yaml or ops this build reads files only —
/// it cannot reach a cluster*. One write to stderr, two sentences that cannot both be true.
///
/// **The synopsis is the only place a reader learns a form exists**, which is [`READ_ONLY`]'s own
/// argument for itself, measured at zero mentions before it landed — and it reaches the console
/// as a whole: nothing the binary printed said that typing `k8rs` alone opens anything.
#[test]
fn the_usage_leads_with_the_console_and_stops_denying_it() {
    println!("{USAGE}");
    let synopsis = USAGE.lines().next().expect("the usage has a first line");
    assert!(
        synopsis.starts_with(
            "usage: k8rs [--read-only] [--context <name>] [--namespace <name>]   |   k8rs \
             [--analysis] <file.json>..."
        ),
        "the console form does not lead the synopsis: {synopsis:?}"
    );
    // **Seven alternatives**, counted off the separator the line is built from.
    assert_eq!(
        synopsis.matches("   |   ").count() + 1,
        7,
        "an alternative was added or lost: {synopsis:?}"
    );
    // **The claim the console form makes false, gone** — and the six doors it replaced with a
    // true sentence still named, because a reader needs each of them too.
    assert!(
        !USAGE.contains("this build reads files only"),
        "the synopsis still says a line without the six mode words cannot reach a cluster"
    );
    assert!(
        USAGE.contains(
            "A path on the line is always the file-driven form, and nothing else; without one, \
             this build opens a console instead of reading nothing"
        ),
        "the rewritten trailing sentence is not the one screens/states.md writes: {USAGE}"
    );
    for door in ["--once", "--live", "--logs", "--describe", "--yaml", "ops"] {
        assert!(
            USAGE.contains(door),
            "the synopsis stopped naming {door}, which still reaches a cluster"
        );
    }
    // **`--analysis` is not on the console form**, and this asserts the *text* and not a
    // behaviour (PM ruling, 2026-09-24; backlog.md carries the reasoning). `--analysis` is not a
    // console flag and never opens a console — `k8rs --analysis` alone is the file form with no
    // file — but `k8rs --read-only --analysis` does open one and drops the word, because the
    // console draws the seven panes anyway and refusing a flag it already satisfies costs the
    // reader more than it saves. The assertion this replaced claimed the behaviour and was
    // shaped to pass: `!starts_with("usage: k8rs [--analysis] [--read-only]")` ruled out one
    // spelling of a thing it said it forbade, so `[--read-only] [--analysis] …` sailed through.
    let console_form = synopsis
        .strip_prefix("usage: ")
        .and_then(|line| line.split("   |   ").next())
        .expect("the synopsis has a first alternative");
    assert_eq!(
        console_form, "k8rs [--read-only] [--context <name>] [--namespace <name>]",
        "the console form is not the one screens/states.md writes"
    );
    assert!(
        !console_form.contains("--analysis"),
        "the console form offers a flag the console has nothing to mean: {console_form:?}"
    );
}

/// **`may-i`'s shape is not the operations' shape**, and the synopsis showed only theirs
/// (`k8s-admin`, 2026-09-05): a reader who followed `ops <operation> <kind>/<name>` for a question
/// would write `may-i delete/nodes`.
#[test]
fn the_usage_offers_the_question_in_the_synopsis_and_in_the_ops_rows() {
    println!("{USAGE}");
    assert!(
        USAGE.contains("k8rs ops may-i <verb> <resource>.<group>[/<name>]"),
        "the top-level usage does not say a question exists"
    );
    let usage = ops_usage();
    println!("{usage}");
    assert!(
        usage.contains("ops may-i <verb> <resource>.<group>[/<name>] [--subresource <name>]"),
        "the ops usage does not say how to ask a question"
    );
    // **The two rules that are the question's alone**, in the prose under the rows rather than
    // inside one 250-character row (NOTES § D230 rulings 1 and 2).
    for said in [
        "Spell the API group",
        "pods.",
        "the object's own name",
        "it asks about the whole cluster",
    ] {
        assert!(
            usage.contains(said),
            "the usage does not say {said:?}: {usage}"
        );
    }
    // **Every row is a row.** The one this box added was 250 characters beside five one-liners.
    for line in usage.lines().filter(|line| line.starts_with("  ")) {
        assert!(
            line.chars().count() <= 100,
            "a usage row is a paragraph: {line:?}"
        );
    }
}

// --- THE CONSOLE LOOP ---
//
// **The coalescer is what these are about, and it is the one claim the loop cannot make for
// itself** (todo.md § Phase 12, PRIOR-ART § A5). k9s merged *skip the reconcile cycle when the
// informer data is unchanged* and reverted it a month later: a coalescer that drops the **last**
// event of a burst shows stale data for ever, and it passes every test that only checks that events
// arrived. So the assertion here is made **after the quiet** — the storm is fed, the feed stops,
// the window expires, and what is on the screen then has to be the last event.
//
// **No terminal and no cluster.** [`pump`] is a function over a stream of updates, a channel of
// keys and something to draw into, so the whole loop runs against a `TestBackend` and a
// `stream::iter`; the clock is tokio's paused one, so the 100 ms window costs the suite nothing and
// the frame count is deterministic rather than a race.

/// A console with nothing in it — every run-level fact at the value a fresh connection gives it.
fn bare_console<'a>() -> Console<'a> {
    Console {
        app: views::App::default(),
        log: views::Log::default(),
        depth: theme::Depth::Ansi16,
        writes: ui::Writes::Live,
        insecure: false,
        context: views::Stripped::of("ctx: k8rs"),
        clock: None,
        kinds: Vec::new(),
        contexts: Vec::new(),
        // **Something connected, which is every run that has a store to press a key against** —
        // the startup states are `Connection::Never`, and they are driven from `console()` itself.
        connection: views::Connection::Live(Some("k8rs".to_owned())),
        unconnected: false,
        link: ui::Link::Live,
        opened: None,
        // **What the *last frame* offered, which for a console that has drawn none is nothing** —
        // the same value `console()` starts with, so a test that presses a mutating key has to draw
        // a frame first, exactly as a reader does.
        offer: views::Offer::Nothing { switch: false },
        carried: None,
        // **Armed, which is every run that reached a keyboard** — the refusal `false` buys is its
        // own test ([`ctrl_z_does_nothing_at_all_when_the_resume_could_not_be_armed`]).
        resumable: true,
    }
}

/// **A burst of watch events, then quiet: one frame for the burst, and the last event on it.**
///
/// **The count is the pods LIST's own progress**, so every one of the fourteen events moves what is
/// on the screen and the last one is `14 pods` — a coalescer that dropped it would draw `13`, which
/// is the defect `PRIOR-ART § A5` names and which no *did the events arrive* assertion can see.
///
/// **Two frames and not fourteen**: the first is the still-loading screen the loop owes before
/// anything has happened at all, the second is the whole burst. That is invariant 7's *"a rollout
/// that restarts 200 pods produces one redraw, not 200"* measured rather than asserted about.
#[tokio::test(start_paused = true)]
async fn a_storm_that_goes_quiet_ends_on_the_last_event_and_costs_one_frame() {
    let pods = objects::<Pod>("kube-system-pods.json");
    let counted = pods.len();
    let mut storm: Vec<k8s::Update> = vec![Box::new(|store: &mut k8s::Store| {
        store.pod(&now(), Event::Init);
    })];
    for pod in pods {
        storm.push(Box::new(move |store: &mut k8s::Store| {
            store.pod(&now(), Event::InitApply(pod));
        }));
    }
    // **No `InitDone`**, so the bootstrap gate stays shut and the screen is the still-loading one
    // whose paragraph counts what has landed (NOTES § D28, `screens/states.md` § Still loading).
    let mut updates = Updates {
        merged: futures_util::stream::select_all(vec![futures_util::stream::iter(storm).boxed()]),
        drained: false,
        asked: std::collections::BTreeSet::new(),
        asking: tokio::sync::mpsc::unbounded_channel().0,
    };
    let (pressing, mut keys) = tokio::sync::mpsc::unbounded_channel();
    // **`q` long after the window, so the assertion is made on a screen the storm drew and not on
    // one a keypress forced.** A key draws at once (`Owing::now`), so a `q` sent beside the burst
    // would paint the frame this test is trying to catch the coalescer not painting.
    tokio::spawn(async move {
        tokio::time::sleep(COALESCE * 10).await;
        let _ = pressing.send(Woke::Key(ratatui::crossterm::event::Event::Key(
            ratatui::crossterm::event::KeyEvent::new(
                ratatui::crossterm::event::KeyCode::Char('q'),
                ratatui::crossterm::event::KeyModifiers::NONE,
            ),
        )));
    });

    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))
        .expect("a terminal over a test backend");
    let mut console = bare_console();
    let mut store = k8s::Store::default();
    let at = Watching {
        renewal: None,
        coverage: k8s::Coverage::Cluster,
    };
    let halt = pump(
        &mut console,
        &mut terminal,
        &mut store,
        &mut updates,
        &mut keys,
        &at,
        None,
    )
    .await;
    assert!(matches!(halt, Halt::Quit), "the loop did not end on `q`");

    let drawn: Vec<String> = (0..terminal.backend().buffer().area.height)
        .map(|row| {
            (0..terminal.backend().buffer().area.width)
                .map(|column| terminal.backend().buffer()[(column, row)].symbol())
                .collect()
        })
        .collect();
    let screen = drawn.join("\n");
    // The canary: this is the console's own frame and not an empty buffer that would pass every
    // absence below (CLAUDE.md § A derived list asserts it found something).
    assert!(screen.contains("ALERTS"), "not a console frame:\n{screen}");
    assert!(
        screen.contains(&format!("{counted} pods")),
        "the last event of the burst never reached the screen:\n{screen}"
    );
    assert!(
        !screen.contains(&format!("{} pods", counted - 1)),
        "the screen stopped one event short of the burst:\n{screen}"
    );
    assert_eq!(
        terminal.get_frame().count(),
        2,
        "the burst was drawn once per event instead of once:\n{screen}"
    );
}

/// **A console needs both ends of the terminal, and neither one alone will do**
/// (`screens/context.md`'s own rule for the startup picker, extended to the console it opens in).
///
/// **All four rows, because the process this suite runs in can only ever produce the last one** —
/// `cargo test`'s stdin and stdout are both pipes, so a function that read them itself was one
/// `false` away from opening a console in CI with nothing to catch it (`just mutants-diff`).
#[test]
fn a_console_opens_only_with_a_terminal_at_both_ends() {
    assert!(at_a_keyboard(true, true), "a person at a keyboard");
    assert!(
        !at_a_keyboard(true, false),
        "`k8rs > out.txt` has nothing to draw into"
    );
    assert!(
        !at_a_keyboard(false, true),
        "`… | k8rs` has no keys to read"
    );
    assert!(
        !at_a_keyboard(false, false),
        "a harness, and every CI run — the row this suite is in"
    );
}

/// One object of a committed single-object capture, decoded — [`objects`]'s other shape.
fn object<T: DeserializeOwned>(name: &str) -> T {
    let text = std::fs::read_to_string(fixture(name)).expect("the fixture reads");
    serde_json::from_str(&text).expect("the capture decodes")
}

/// **A store with three cards in it**, every initial LIST landed — three *broken* pods, because a
/// card is a finding and the healthy captures produce none. Each is its own owner, so three
/// captures are three rows for the content cursor to move across.
///
/// **Committed captures and not hand-written objects** (NOTES § D53): these are the same three
/// `rules_tests.rs` reads, and what this file adds is what the *keys* do once cards exist.
fn a_cluster_with_cards() -> k8s::Store {
    listed(vec![
        object::<Pod>("crashloop.json"),
        object::<Pod>("evicted.json"),
        object::<Pod>("image.json"),
        // **The fourth is *owned*, and it is here for [`filed_under`]** — the three above are bare
        // pods, whose card is filed under the pod itself, so *the owner* and *a pod of the card*
        // are the same `ObjectId` and a lookup reading one as the other cannot be told apart (`just
        // mutants-diff` turned that `||` into `&&` and nothing failed).
        objects::<Pod>("owned-pods.json")
            .into_iter()
            .next()
            .expect("the owned capture holds a pod"),
    ])
}

/// One key press, with no modifier.
fn key(code: ratatui::crossterm::event::KeyCode) -> ratatui::crossterm::event::Event {
    ratatui::crossterm::event::Event::Key(ratatui::crossterm::event::KeyEvent::new(
        code,
        ratatui::crossterm::event::KeyModifiers::NONE,
    ))
}

/// One key press with `ctrl` held — `ctrl-c` and `ctrl-d`, the two the map carries.
fn control(code: ratatui::crossterm::event::KeyCode) -> ratatui::crossterm::event::Event {
    ratatui::crossterm::event::Event::Key(ratatui::crossterm::event::KeyEvent::new(
        code,
        ratatui::crossterm::event::KeyModifiers::CONTROL,
    ))
}

/// `Char(c)`, the commonest of the two above.
fn typed(character: char) -> ratatui::crossterm::event::Event {
    key(ratatui::crossterm::event::KeyCode::Char(character))
}

/// **`q` and `ctrl-c` are one answer, and a write on the wire refuses both** (NOTES § D12,
/// `views::App::may_quit`: quitting mid-`PATCH` leaves the audit log holding an attempt with no
/// result). `ctrl-c` is a key and not a signal because raw mode clears `ISIG`.
#[test]
fn q_and_ctrl_c_quit_and_a_write_on_the_wire_refuses_both() {
    let store = a_cluster_with_cards();
    for pressed in [
        typed('q'),
        control(ratatui::crossterm::event::KeyCode::Char('c')),
    ] {
        let mut console = bare_console();
        assert!(
            matches!(keyed(&mut console, pressed.clone(), &store), Did::Quit),
            "{pressed:?} did not quit"
        );

        let mut busy = bare_console();
        busy.app.changing = Some(views::Object::new(
            "deployment",
            Some("payments".to_owned()),
            "web".to_owned(),
            None,
        ));
        assert!(
            matches!(keyed(&mut busy, pressed.clone(), &store), Did::Nothing),
            "{pressed:?} quit out of a write that was still running"
        );
    }
}

/// **`ctrl-z` is answered from every mode and leaves that mode alone** (NOTES § D24): the handover
/// is not a command, so neither a filter with focus nor an open dialog can swallow it — and those
/// two are exactly the states [`pressed`] answers *before* it reaches the key map, which is why a
/// key added to the map would have been unreachable from both.
#[test]
fn ctrl_z_suspends_from_every_mode_and_leaves_that_mode_alone() {
    let store = a_cluster_with_cards();
    let ctrl_z = || control(ratatui::crossterm::event::KeyCode::Char('z'));

    let mut console = bare_console();
    assert!(
        matches!(keyed(&mut console, ctrl_z(), &store), Did::Suspend),
        "`ctrl-z` with no mode open reached nothing"
    );

    // **While a filter has focus every printable key is text, and `ctrl-z` is not one of them**
    // (`screens/widgets.md` § 2b).
    let mut typing = bare_console();
    let _ = keyed(&mut typing, typed('/'), &store);
    assert!(typing.app.typing.is_some(), "`/` did not open the filter");
    assert!(
        matches!(keyed(&mut typing, ctrl_z(), &store), Did::Suspend),
        "`ctrl-z` while typing was swallowed by the filter"
    );
    assert_eq!(
        typing.app.typed().map(views::Input::text),
        Some(""),
        "`ctrl-z` put a `z` in the filter"
    );

    // **And from under a modal, which it leaves open**: `fg` comes back to the screen the reader
    // left, so the suspend is not a `esc`.
    let mut modal = bare_console();
    modal.app.modal = Some(views::Modal::Help);
    assert!(
        matches!(keyed(&mut modal, ctrl_z(), &store), Did::Suspend),
        "`ctrl-z` under Help reached nothing"
    );
    assert_eq!(
        modal.app.modal,
        Some(views::Modal::Help),
        "the suspend closed Help"
    );

    // A bare `z` is not the key — `screens/help.md` binds no `z` at all.
    assert!(
        matches!(keyed(&mut bare_console(), typed('z'), &store), Did::Nothing),
        "a bare `z` suspended the process"
    );
}

/// **A console that could not arm `SIGCONT` does not stop at all** (PM ruling, 2026-09-24,
/// [`may_stop`]): every recovery there is lives under `Woke::Resumed`, so a `ctrl-z` without it
/// hands the terminal back and nothing ever takes it back — `fg` to a cooked tty and a dead
/// keyboard, which is the failure this whole family exists to remove and which the red run for the
/// signal arms measured. Never hand back a terminal you cannot take back.
///
/// **A key that does nothing is honest here**: it is what `ctrl-z` did before this box, and raw
/// mode means the terminal raises nothing either way.
#[test]
fn ctrl_z_does_nothing_at_all_when_the_resume_could_not_be_armed() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    console.resumable = false;
    assert!(
        matches!(
            keyed(
                &mut console,
                control(ratatui::crossterm::event::KeyCode::Char('z')),
                &store
            ),
            Did::Nothing
        ),
        "`ctrl-z` handed the terminal back with nothing armed to take it back"
    );
}

/// **The handover is two inverse sequences and each carries the cursor** (NOTES § D24,
/// PRIOR-ART § D4) — the alternate screen *and* the cursor, which is the half `ratatui::restore`
/// does not carry and the half a suspend cannot get from `Terminal::drop`.
///
/// **It is the sequence that is asserted and not the `ioctl`**: raw mode needs a real tty, these
/// are bytes, and the bytes are what a `fg` that came back to a dead screen would be missing.
///
/// **Nothing here calls [`stopped`]**, which would stop the test binary with `SIGSTOP` and hang the
/// suite until somebody found it — the composition is what a test can hold.
#[test]
fn the_handover_is_two_inverse_sequences_and_each_carries_the_cursor() {
    let (mut given, mut taken) = (Vec::new(), Vec::new());
    handover(&mut given, false).expect("a `Vec` takes the sequence");
    handover(&mut taken, true).expect("a `Vec` takes the sequence");
    let given = String::from_utf8(given).expect("the sequence is text");
    let taken = String::from_utf8(taken).expect("the sequence is text");
    // The canary: an empty sink passes every absence below (CLAUDE.md § A derived list asserts it
    // found something).
    assert!(
        !given.is_empty() && !taken.is_empty(),
        "the handover wrote nothing at all"
    );

    for (direction, wrote, alternate, cursor) in [
        (
            "handing the terminal back",
            &given,
            "\x1b[?1049l",
            "\x1b[?25h",
        ),
        (
            "taking the terminal back",
            &taken,
            "\x1b[?1049h",
            "\x1b[?25l",
        ),
    ] {
        assert!(
            wrote.contains(alternate),
            "{direction} left the alternate screen alone: {wrote:?}"
        );
        assert!(
            wrote.contains(cursor),
            "{direction} left the cursor alone: {wrote:?}"
        );
    }
    // **Inverses, which is the claim a pair of one-way assertions does not make**: neither
    // direction may carry the other's half, or a resume would hide the cursor it just showed.
    assert!(
        !given.contains("\x1b[?1049h") && !given.contains("\x1b[?25l"),
        "handing the terminal back also took it: {given:?}"
    );
    assert!(
        !taken.contains("\x1b[?1049l") && !taken.contains("\x1b[?25h"),
        "taking the terminal back also gave it away: {taken:?}"
    );
}

/// **The hook restores the terminal and *then* runs the one it replaced** (`screens/widgets.md`
/// § 1, invariant 8) — and the stand-in underneath is what makes both halves visible: it stands in
/// for ratatui's own restoring hook, so the list it appends to is the order the panic path ran in.
///
/// **This is the test `tester` F1 sent back, and the shape it sent back is why the seam moved.** A
/// version that only counted the stand-in passed with `chain_panic_hook`'s body emptied — measured,
/// not argued: with no chain installed the stand-in *is* the hook and runs anyway. What cannot pass
/// is an order, so the restoring half is handed in and the order is the claim.
///
/// **The hook is process-global and this binary runs `main_tests`, `ops_tests` and `rules_tests` on
/// N threads with six deliberate panic sites**, so the stand-in records only this thread's panic
/// and forwards every other thread's to the hook it replaced — a stand-in that swallowed them would
/// take away another thread's failure message for as long as this test holds the hook.
#[test]
fn the_panic_hook_restores_the_terminal_before_the_hook_it_chains_prints() {
    let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let (restoring, printing) = (std::sync::Arc::clone(&order), std::sync::Arc::clone(&order));
    let mine = std::thread::current().id();
    let libtest: std::sync::Arc<dyn Fn(&std::panic::PanicHookInfo<'_>) + Send + Sync> =
        std::sync::Arc::from(std::panic::take_hook());
    let forwarding = std::sync::Arc::clone(&libtest);
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().id() == mine {
            printing
                .lock()
                .expect("the order")
                .push("the hook underneath ran");
        } else {
            (*forwarding)(info);
        }
    }));
    chain_panic_hook(move || {
        // **The same thread guard as the stand-in below, and for the same reason**: without it
        // another thread's panic appends here and reddens this test for somebody else's failure.
        if std::thread::current().id() == mine {
            restoring
                .lock()
                .expect("the order")
                .push("the terminal went back");
        }
    });
    let panicked = std::panic::catch_unwind(|| panic!("a canary, not a failure"));
    std::panic::set_hook(Box::new(move |info| (*libtest)(info)));

    assert!(panicked.is_err(), "the canary did not panic");
    assert_eq!(
        *order.lock().expect("the order"),
        ["the terminal went back", "the hook underneath ran"],
        "the panic path did not restore first and forward second"
    );
}

/// **Each half of the pair writes its own direction** (`tester` F2) — the wrappers, not just
/// [`handover`]: swapping the argument in either of them leaves the sequences perfectly inverse and
/// the terminal perfectly wrong.
///
/// **The `ioctl` is the part no suite without a tty can reach**, so it is the part not asserted:
/// `taken_back` answers `Err` here because `enable_raw_mode` has no terminal to work on, and the
/// bytes it wrote before saying so are the claim.
#[test]
fn each_half_of_the_pair_writes_its_own_direction() {
    let (mut back, mut taken) = (Vec::new(), Vec::new());
    handed_back(&mut back).expect("handing a `Vec` back cannot fail: no raw mode was ever on");
    let _ = taken_back(&mut taken);
    let back = String::from_utf8(back).expect("the sequence is text");
    let taken = String::from_utf8(taken).expect("the sequence is text");

    assert!(
        back.contains("\x1b[?1049l") && back.contains("\x1b[?25h"),
        "`handed_back` did not give the screen and the cursor back: {back:?}"
    );
    assert!(
        taken.contains("\x1b[?1049h") && taken.contains("\x1b[?25l"),
        "`taken_back` did not take the screen and the cursor: {taken:?}"
    );
}

/// **A resume hands the terminal back before it takes it, and then repaints** — the order is the
/// whole of `k8s-admin` finding 1 and `tester` F4: `crossterm`'s `enable_raw_mode` returns `Ok(())`
/// and touches no tty while it believes raw mode is on, so after an external `SIGTSTP` — which
/// disabled nothing — a `taken_back` on its own leaves the terminal **cooked** and every key
/// line-buffered, with the frame redrawn over the top of it.
#[test]
fn a_resume_gives_the_terminal_back_before_it_takes_it_and_then_repaints() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))
        .expect("a terminal over a test backend");
    let at = Watching {
        renewal: None,
        coverage: k8s::Coverage::Cluster,
    };
    drawn(&mut terminal, &mut console, &store, &at).expect("the frame draws");
    assert!(
        screened(&terminal).contains("ALERTS"),
        "not a console frame"
    );

    let mut wrote = Vec::new();
    // `Err` here is the `enable_raw_mode` with no tty, the one part of this a suite cannot hold.
    let _ = resumed(&mut wrote, &mut terminal);
    let wrote = String::from_utf8(wrote).expect("the sequence is text");
    let gave = wrote
        .find("\x1b[?1049l")
        .expect("the resume never gave the screen back");
    let took = wrote
        .find("\x1b[?1049h")
        .expect("the resume never took the screen");
    assert!(
        gave < took,
        "the resume took the terminal without giving it back first, so raw mode stayed off: \
         {wrote:?}"
    );

    drawn(&mut terminal, &mut console, &store, &at).expect("the frame draws");
    assert!(
        screened(&terminal).contains("ALERTS"),
        "the frame after a resume was the difference against a screen that is gone:\n{}",
        screened(&terminal)
    );
}

/// **`?` opens the key map and either of the two keys its own footer names closes it**
/// (`screens/help.md`: *`? or esc to close`*).
#[test]
fn help_opens_on_question_and_closes_on_either_key_it_names() {
    let store = a_cluster_with_cards();
    for closing in [typed('?'), key(ratatui::crossterm::event::KeyCode::Esc)] {
        let mut console = bare_console();
        let _ = keyed(&mut console, typed('?'), &store);
        assert_eq!(console.app.modal, Some(views::Modal::Help));
        let _ = keyed(&mut console, closing.clone(), &store);
        assert_eq!(console.app.modal, None, "{closing:?} left Help open");
    }
}

/// **`q` still quits from under Help, and a write on the wire refuses it there too**
/// (`screens/help.md` draws `q quit` on the Help footer beside `? or esc to close`, and its
/// § *While the call is running* refuses the same key until the call returns).
///
/// **This is the pair [`q_and_ctrl_c_quit_and_a_write_on_the_wire_refuses_both`] cannot reach**:
/// that one presses `q` with no modal open, so [`over_modal`]'s own guard was never asked.
/// Measured: replacing it with `false` survived the whole suite (`just mutants-diff`, 2026-09-24).
#[test]
fn q_quits_from_under_help_and_a_write_on_the_wire_refuses_it_there_too() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    console.app.modal = Some(views::Modal::Help);
    assert!(
        matches!(keyed(&mut console, typed('q'), &store), Did::Quit),
        "`q` under the key map did not quit"
    );

    let mut busy = bare_console();
    busy.app.modal = Some(views::Modal::Help);
    busy.app.changing = Some(views::Object::new(
        "deployment",
        Some("payments".to_owned()),
        "web".to_owned(),
        None,
    ));
    assert!(
        matches!(keyed(&mut busy, typed('q'), &store), Did::Nothing),
        "`q` under the key map quit out of a write that was still running"
    );
    assert_eq!(
        busy.app.modal,
        Some(views::Modal::Help),
        "the refused key closed Help instead of being refused"
    );
}

/// **A box that asks only for a press takes no typing, so `⌫` costs it no frame** — and a box
/// that asks for the object's name shortens its buffer (`screens/dialogs.md`: the restart box names
/// `⏎` and `esc` and no `⌫`; only a delete's *type the name* box has a buffer to shorten,
/// invariant 2's typed name).
///
/// **The frame is the half a mutation found**: popping an empty buffer draws the same screen, so
/// what the guard decides is the *answer* — [`Did::Changed`] against [`Did::Nothing`] — and a key
/// that changed nothing must not cost a redraw (invariant 7). Measured: the guard replaced with
/// `true` survived the whole suite (`just mutants-diff`, 2026-09-24).
#[test]
fn a_box_that_asks_only_for_a_press_spends_no_frame_on_backspace() {
    let mut console = bare_console();
    installed(&mut console, Published::Opening(an_open_dialog()));
    installed(
        &mut console,
        Published::Checked {
            verdict: "The cluster checked it first and accepted it.",
            asks: None,
        },
    );
    let answered = over_modal(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Backspace),
        views::Detailing::Closed,
        &[],
        &before_the_list(),
    );
    assert!(
        matches!(answered, Did::Nothing),
        "`⌫` cost a frame on a box with nothing to type"
    );

    // And the same key on the box that does ask shortens what was typed.
    let mut asked = bare_console();
    installed(&mut asked, Published::Opening(an_open_dialog()));
    installed(
        &mut asked,
        Published::Checked {
            verdict: "k8rs did not check this one with the cluster first",
            asks: Some("web".to_owned()),
        },
    );
    for letter in ['w', 'e'] {
        let _ = over_modal(
            &mut asked,
            pressing(ratatui::crossterm::event::KeyCode::Char(letter)),
            views::Detailing::Closed,
            &[],
            &before_the_list(),
        );
    }
    let shortened = over_modal(
        &mut asked,
        pressing(ratatui::crossterm::event::KeyCode::Backspace),
        views::Detailing::Closed,
        &[],
        &before_the_list(),
    );
    assert!(
        matches!(shortened, Did::Changed),
        "`⌫` did not redraw the box it shortened"
    );
    let Some(views::Modal::Confirm(dialog)) = &asked.app.modal else {
        panic!("the box closed on a `⌫`")
    };
    assert_eq!(
        dialog.typed.text(),
        "w",
        "`⌫` did not take the last letter off the typed name"
    );
}

/// **`↑` moves a free-text pane back up and `↓` moves it on**, and either one takes follow
/// mode off (`views::App::scroll_by`, `screens/widgets.md` § 4).
///
/// **Measured, and it is the plainest survivor of the round**: [`Step::lines`] always answering
/// `1`, and the same function with its `-` deleted, both survived the whole suite — every other
/// test that moves a detail pane moves it *down*, where the two answers agree (`just mutants-diff`,
/// 2026-09-24). A reader whose `↑` scrolled down would be the defect.
#[test]
fn the_up_arrow_moves_a_detail_pane_back_up_and_the_down_arrow_moves_it_on() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    console.opened = Some(Opened::Tabs {
        object: ObjectId {
            kind: ObjectKind::Pod,
            namespace: Some("default".to_owned()),
            name: "broken-crashloop".to_owned(),
            uid: None,
        },
        from_step: false,
    });
    let tab = console.app.tab.at();
    console.app.scroll[tab] = 5;
    console.app.following = true;

    let went = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Up),
        &store,
    );
    assert!(matches!(went, Did::Changed), "`↑` drew nothing");
    assert_eq!(
        console.app.scroll[tab], 4,
        "`↑` did not move the pane back up"
    );
    assert!(
        !console.app.following,
        "`↑` left the pane pinned to the tail it was scrolled away from"
    );

    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Down),
        &store,
    );
    assert_eq!(console.app.scroll[tab], 5, "`↓` did not move the pane on");
}

/// **`tab` moves between the two panels and `↑↓` move whichever one has it** (`screens/help.md`:
/// `tab  next panel`, `views::Panel`).
///
/// **The two cursors are asserted separately**, because the defect a shared one would produce is
/// exactly *the reader moved the sidebar and the card under `s` changed with it*.
#[test]
fn tab_moves_the_focus_and_the_arrows_move_the_panel_that_has_it() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    console.kinds = vec![k8s::Browsable {
        group: "apps".to_owned(),
        version: "v1".to_owned(),
        kind: "Deployment".to_owned(),
        plural: "deployments".to_owned(),
        namespaced: true,
        verbs: vec!["list".to_owned()],
    }];
    let anchors = [None, None, None, None, None, None, None, None];

    // Focus starts on the content pane, which is the pane `screens/alerts.md`'s footer is about.
    assert_eq!(console.app.focus, views::Panel::Content);
    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Down),
        &store,
    );
    assert_eq!(console.app.content.selected(&anchors), Some(1));
    assert_eq!(
        console.app.nav.selected(&anchors),
        Some(0),
        "the sidebar moved with the pane"
    );

    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Tab),
        &store,
    );
    assert_eq!(console.app.focus, views::Panel::Sidebar);
    let _ = keyed(&mut console, typed('j'), &store);
    assert_eq!(console.app.nav.selected(&anchors), Some(1));
    assert_eq!(
        console.app.content.selected(&anchors),
        Some(1),
        "the pane moved with the sidebar"
    );
    let _ = keyed(&mut console, typed('k'), &store);
    assert_eq!(console.app.nav.selected(&anchors), Some(0));
}

/// **`/` and `n` open the field they name, every printable key is text while one has focus, and
/// `esc` empties the field before it leaves it** (`screens/widgets.md` § 2b).
#[test]
fn the_two_filters_open_on_their_own_key_and_take_every_printable_one() {
    let store = a_cluster_with_cards();
    for (opens, field) in [('/', views::Typing::Text), ('n', views::Typing::Namespace)] {
        let mut console = bare_console();
        let _ = keyed(&mut console, typed(opens), &store);
        assert_eq!(console.app.typing, Some(field));
        // `q` is a letter while a filter has focus — that section's own example.
        for character in "quit".chars() {
            let _ = keyed(&mut console, typed(character), &store);
        }
        assert_eq!(console.app.typed().map(views::Input::text), Some("quit"));
        let _ = keyed(
            &mut console,
            key(ratatui::crossterm::event::KeyCode::Backspace),
            &store,
        );
        assert_eq!(console.app.typed().map(views::Input::text), Some("qui"));
        // `⏎` commits and keeps what was typed; the filter was live throughout.
        let _ = keyed(
            &mut console,
            key(ratatui::crossterm::event::KeyCode::Enter),
            &store,
        );
        assert_eq!(console.app.typing, None);
        assert!(console.app.filters.any(), "the filter was thrown away");
    }
}

/// **A resize discards every free-text offset** (`screens/widgets.md` § 4: a row measured against
/// one wrap names nothing at another width) **and a keypress does not.**
#[test]
fn a_resize_rewinds_every_tab_offset_and_an_ordinary_key_does_not() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    console.app.scroll = [7, 8, 9, 10];
    let _ = keyed(&mut console, typed('?'), &store);
    assert_eq!(
        console.app.scroll,
        [7, 8, 9, 10],
        "a key that is not a resize rewound the panes"
    );
    let _ = keyed(
        &mut console,
        ratatui::crossterm::event::Event::Resize(120, 40),
        &store,
    );
    assert_eq!(console.app.scroll, [0, 0, 0, 0]);
}

/// **Inside a detail tab the arrows scroll, `[` and `]` move between tabs, and `f` toggles follow**
/// (`screens/detail.md`, `screens/widgets.md` § 4 — and a manual scroll turns follow off, which is
/// `views::App::scroll_by`'s one call so no handler can do half of it).
#[test]
fn a_detail_tabs_keys_scroll_that_tab_and_move_between_the_four() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    console.opened = Some(Opened::Tabs {
        object: ObjectId {
            kind: ObjectKind::Pod,
            namespace: Some("default".to_owned()),
            name: "broken-crashloop".to_owned(),
            uid: None,
        },
        from_step: false,
    });
    console.app.following = true;

    let _ = keyed(&mut console, typed('j'), &store);
    assert_eq!(console.app.scroll[views::Tab::Logs.at()], 1);
    assert!(
        !console.app.following,
        "a manual scroll left follow mode on"
    );

    let _ = keyed(&mut console, typed(']'), &store);
    assert_eq!(console.app.tab, views::Tab::Describe);
    let _ = keyed(&mut console, typed('j'), &store);
    assert_eq!(console.app.scroll[views::Tab::Describe.at()], 1);
    assert_eq!(
        console.app.scroll[views::Tab::Logs.at()],
        1,
        "describe's scroll moved the logs tab's own offset"
    );
    let _ = keyed(&mut console, typed('['), &store);
    assert_eq!(console.app.tab, views::Tab::Logs);

    let _ = keyed(&mut console, typed('f'), &store);
    assert!(console.app.following, "`f` did not turn follow back on");

    // `esc` closes the slot — the caller's, because `App` holds no field for what is open — and
    // every offset goes with it (§ 4: opening the slot again reads everything fresh).
    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Esc),
        &store,
    );
    assert!(console.opened.is_none(), "`esc back` left the tab open");
    assert_eq!(console.app.scroll, [0, 0, 0, 0]);
}

/// **`l`, `d` and `y` open the slot on the selected object at the tab each one names**
/// (`screens/help.md` § Looking at things).
#[test]
fn the_three_reading_keys_open_the_tab_each_one_names() {
    let store = a_cluster_with_cards();
    for (pressed, tab) in [
        ('l', views::Tab::Logs),
        ('d', views::Tab::Describe),
        ('y', views::Tab::Yaml),
    ] {
        let mut console = bare_console();
        let _ = keyed(&mut console, typed(pressed), &store);
        assert!(
            matches!(
                console.opened,
                Some(Opened::Tabs {
                    from_step: false,
                    ..
                })
            ),
            "`{pressed}` opened no detail"
        );
        assert_eq!(console.app.tab, tab);
    }
}

/// **`X` opens the cluster picker and the log says what it read** (`screens/context.md` § What the
/// command log shows).
#[test]
fn x_opens_the_picker_and_logs_the_read_behind_it() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let _ = keyed(&mut console, typed('X'), &store);
    assert!(
        matches!(console.app.modal, Some(views::Modal::ContextPick(_))),
        "`X` opened no picker"
    );
    assert_eq!(
        console.log.lines().last().map(views::Stripped::as_str),
        Some(views::GET_CONTEXTS)
    );
}

/// **A mutating key with nothing selected reaches nothing** — invariant 2's *an explicitly selected
/// object*, and `views::App::may_mutate`'s own answer for a pane with no cards in it.
///
/// **And a view whose rows this wiring does not have reaches nothing either**: the content cursor
/// is one index over whatever the open pane draws, so reading it as a card while the browser is
/// open would act on the object at that index in a *different* list.
#[test]
fn a_mutating_key_reaches_nothing_with_nothing_selected_and_nothing_off_alerts() {
    let empty = k8s::Store::default();
    let mut console = bare_console();
    for pressed in [
        typed('r'),
        control(ratatui::crossterm::event::KeyCode::Char('d')),
    ] {
        assert!(
            matches!(keyed(&mut console, pressed.clone(), &empty), Did::Nothing),
            "{pressed:?} acted on a pane that has answered nothing"
        );
    }

    let store = a_cluster_with_cards();
    let mut browsing = bare_console();
    browsing.app.view = views::View::Resources(0);
    for pressed in [
        typed('r'),
        control(ratatui::crossterm::event::KeyCode::Char('d')),
        typed('l'),
        key(ratatui::crossterm::event::KeyCode::Enter),
    ] {
        assert!(
            matches!(keyed(&mut browsing, pressed.clone(), &store), Did::Nothing),
            "{pressed:?} read the Alerts list while the browser was open"
        );
    }
}

/// **The seven panes the sidebar draws are the seven the report prints** ([`PANES`], the one array
/// both read) — and each is `None` until the store has answered, which is what `screens/states.md`
/// § Still loading draws as a row with no badge beside it.
#[test]
fn the_seven_panes_are_named_once_and_arrive_with_the_first_snapshot() {
    let labels: Vec<&str> = panes(None, &[]).iter().map(|(label, _)| *label).collect();
    assert_eq!(
        labels,
        vec![
            "capacity",
            "certificates",
            "drain safety",
            "posture",
            "restarts",
            "waste",
            "versions"
        ]
    );
    assert!(
        panes(None, &[]).iter().all(|(_, report)| report.is_none()),
        "a pane carried a report before the store had answered"
    );

    // The same seven, computed, once there is a snapshot to compute them from.
    let store = a_cluster_with_cards();
    let snapshot = store.snapshot(now()).expect("every LIST landed");
    let findings = analyze(&snapshot);
    let held = panes(Some(&snapshot), &findings);
    assert_eq!(held.len(), labels.len());
    for (label, report) in &held {
        let report = report
            .as_ref()
            .unwrap_or_else(|| panic!("{label} is empty"));
        assert!(
            !report.title.is_empty(),
            "{label} produced a pane with no title"
        );
    }
}

/// **The TLS warning belongs to the context this run is *on***, and `current` and `insecure` are
/// two facts (`ui::Screen::insecure`, NOTES § D265 ruling 8) — a row that is one without the other
/// puts no warning in the header.
#[test]
fn the_tls_warning_reads_the_current_row_and_only_when_that_row_turns_it_off() {
    let row = |current: bool, insecure: bool| k8s::Choice {
        name: Some("prod-eu".to_owned()),
        key: "prod-eu".to_owned(),
        server: k8s::Address::Server("https://prod-eu.invalid:6443".to_owned()),
        shadowed: false,
        namespace: None,
        insecure,
        tag: k8s::Tag::Blank,
        current,
    };
    assert!(tls_unverified(&[row(true, true)]), "the run's own context");
    assert!(
        !tls_unverified(&[row(true, false)]),
        "the current context verifies TLS and the header claimed otherwise"
    );
    assert!(
        !tls_unverified(&[row(false, true)]),
        "a context nobody is on took the warning into the header"
    );
    assert!(!tls_unverified(&[]), "no contexts, no warning");
}

/// **A console that never reached a cluster says what `--live` says** — one sentence, off the same
/// [`because`], so the two paths cannot come to word one failure differently (`screens/states.md` §
/// Before the TUI ever starts).
#[test]
fn a_console_that_cannot_connect_says_so_in_the_drivers_own_words() {
    let said = no_cluster_to_watch(&k8s::NotConnected::Kubeconfig(
        kube::config::KubeconfigError::CurrentContextNotSet,
    ));
    println!("{said}");
    assert!(
        said.starts_with("k8rs: no cluster to watch — "),
        "the console invented a lead clause of its own: {said:?}"
    );
    // The fault's own wording, which is `views::because`'s and not this file's.
    assert!(
        said.contains("current-context"),
        "the sentence does not name what could not be read: {said:?}"
    );

    // **A cluster that answered nothing names what k8rs was trying to do, and the login program the
    // kubeconfig names** — the two halves `--live`'s own wall carries, from the same call.
    let dead = no_cluster_to_watch(&k8s::NotConnected::Client {
        failure: kube::Error::Service(Box::new(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "nobody is listening",
        ))),
        renewal: Some("aws-iam-authenticator".to_owned()),
    });
    println!("{dead}");
    assert!(
        dead.contains(views::REACH),
        "the sentence does not say what k8rs was trying to do: {dead:?}"
    );
    assert!(
        dead.starts_with("k8rs: no cluster to watch — "),
        "the lead clause moved: {dead:?}"
    );
}

/// **A terminal that stopped taking frames ends the run on a sentence, never a panic** — the reason
/// is the backend's own string, run through [`sanitize`] like every other outside text.
#[test]
fn a_screen_that_cannot_be_drawn_ends_the_run_on_a_sentence() {
    let said = broken_terminal(&std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    println!("{said}");
    assert!(
        said.starts_with("k8rs: the screen could not be drawn — "),
        "the sentence does not say what failed: {said:?}"
    );
    // The library's own reason is kept, because *broken pipe* is a thing a reader can act on.
    assert!(said.len() > "k8rs: the screen could not be drawn — ".len());
    // And a reason carrying a control character is stripped before it can rewrite the terminal that
    // is already in trouble (invariant 9).
    let planted = broken_terminal(&"one\u{1b}[2Jtwo");
    assert!(
        !planted.contains('\u{1b}'),
        "an escape reached the sentence: {planted:?}"
    );
}

/// **A detail tab finds its card by the owner *or* by one of the pods filed under it**
/// ([`filed_under`], `screens/detail.md` § Every finding about this object pinned at the top of
/// every tab) — and finds nothing for an object Alerts has nothing to say about.
#[test]
fn a_card_is_found_by_its_owner_and_by_every_pod_filed_under_it() {
    let store = a_cluster_with_cards();
    let snapshot = store.snapshot(now()).expect("every LIST landed");
    let findings = analyze(&snapshot);
    let cards = views::cards(&findings, &snapshot.workloads, &now());
    assert!(!cards.is_empty(), "the captures produced no card at all");

    // **The card to assert against is one whose owner is *not* one of its own pods** — an owned
    // pod's card is filed under its workload, so the two lookups are two different objects and a
    // predicate that demanded both would find neither.
    let grouped = cards
        .iter()
        .find(|card| card.pods().iter().all(|pod| **pod != card.owner))
        .expect("the owned capture produced no card filed under a workload");
    println!("owner {:?}, pods {:?}", grouped.owner.name, grouped.pods());
    assert!(
        filed_under(&cards, &grouped.owner).is_some(),
        "a card could not be found by its own owner"
    );
    for pod in grouped.pods() {
        assert!(
            filed_under(&cards, pod).is_some(),
            "a pod filed under a card found none: {}",
            pod.name
        );
    }
    let stranger = ObjectId {
        kind: ObjectKind::Pod,
        namespace: Some("default".to_owned()),
        name: "nothing-is-wrong-with-me".to_owned(),
        uid: None,
    };
    assert!(
        filed_under(&cards, &stranger).is_none(),
        "an object with no findings was filed under somebody else's card"
    );
}

/// One watch in trouble, built by hand — [`k8s::Store::troubles`]'s own shape, so the two functions
/// the header is drawn from can be fed a fault without a cluster behind it.
fn in_trouble<'a>(
    kind: ObjectKind,
    failure: Option<&'a watcher::Error>,
    listed: bool,
) -> k8s::Trouble<'a> {
    k8s::Trouble {
        kind,
        listed,
        failure,
        ended: false,
        unfinished: false,
        outstanding: None,
    }
}

/// **`nodes 3/3` counts the ready ones, `nodes …` is a read that has not answered, and a node watch
/// that never listed draws *nothing at all*** (`screens/widgets.md` § 1a: *a vital that cannot be
/// read is blank, never guessed*).
///
/// **The blank one is the row that matters**: `nodes` is cluster-scoped and no namespaced `Role`
/// grants it, so `0/0` is what every scoped run would otherwise print over a cluster it cannot see
/// — the same defect [`header`] already carries this rule for.
#[test]
fn the_header_counts_the_ready_nodes_and_leaves_a_refused_count_blank() {
    let store = identified(
        objects::<Pod>("kube-system-pods.json"),
        objects::<Node>("nodes.json"),
        nearly_out(Some("v1.36.1")),
    );
    let snapshot = store.snapshot(now()).expect("every LIST landed");
    let total = snapshot.nodes.len();
    assert!(total > 0, "the nodes capture is empty");
    let ready = snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.conditions
                .iter()
                .any(|c| c.type_ == "Ready" && c.status == "True")
        })
        .count();

    let drawn = vitals(true, Some(&snapshot), &[]);
    assert_eq!(drawn.as_str(), format!("nodes {ready}/{total}"));

    // Nothing has answered yet: the ellipsis, which is this product's own mark for *in progress*.
    assert_eq!(vitals(true, None, &[]).as_str(), "nodes …");

    // **Nothing connected is blank and not `nodes …`** — that reading is *while connecting*, and a
    // switch that failed is connecting to nothing (`screens/context.md` § After `esc dismiss`,
    // whose left zone is empty).
    assert_eq!(
        vitals(false, Some(&snapshot), &[]).as_str(),
        "",
        "a node count survived the switch that threw the nodes away"
    );

    // The node watch was refused before it listed — blank, and never a measured zero.
    let refused = watcher::Error::InitialListFailed(api_error(403, "Forbidden"));
    assert_eq!(
        vitals(
            true,
            Some(&snapshot),
            &[in_trouble(ObjectKind::Node, Some(&refused), false)]
        )
        .as_str(),
        "",
        "a node count k8rs was refused was printed anyway"
    );
    // A *pod* watch in trouble says nothing about the node count, and a node watch that listed and
    // then broke still has one to show.
    assert_eq!(
        vitals(
            true,
            Some(&snapshot),
            &[in_trouble(ObjectKind::Pod, Some(&refused), false)]
        )
        .as_str(),
        format!("nodes {ready}/{total}")
    );
    assert_eq!(
        vitals(
            true,
            Some(&snapshot),
            &[in_trouble(ObjectKind::Node, Some(&refused), true)]
        )
        .as_str(),
        format!("nodes {ready}/{total}")
    );
}

/// **A refused watch is not a dead link, and that is the whole of this test** (`ui::Link`, whose
/// own doc says a lost link withholds both mutating keys).
///
/// **A namespaced `Role` cannot `list nodes`**, so every scoped run carries a permanently refused
/// node watch. Reading that as *disconnected* would take `s` and `r` from a developer whose
/// `RoleBinding` allows them, for the life of the session — the defect that type exists to close,
/// reached through the other door.
#[test]
fn the_connection_word_reads_the_fault_and_a_refusal_is_not_a_dropped_link() {
    let store = a_cluster_with_cards();
    let snapshot = store.snapshot(now()).expect("every LIST landed");
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

    assert_eq!(linked(true, None, &[]), ui::Link::Connecting);
    assert_eq!(linked(true, Some(&snapshot), &[]), ui::Link::Live);
    // **A switch that failed is no connection at all** (`ui::Link::Unconnected`): the store is
    // empty after `views::App::switched`, which is the shape of a first launch, so a frame that
    // read only the store would say `connecting…` over a cluster that already said no.
    assert_eq!(
        linked(false, None, &[]),
        ui::Link::Unconnected,
        "a connection that failed was described as one that is arriving"
    );
    assert_eq!(
        linked(
            true,
            Some(&snapshot),
            &[in_trouble(ObjectKind::Node, Some(&refused), false)]
        ),
        ui::Link::Live,
        "a refused watch took both mutating keys off a live cluster"
    );
    // **One dropped watch beside four that are answering is not a dropped connection**
    // (NOTES § D285 ruling 1). It read `Lost` here until 2026-09-26, which is the measured defect:
    // the four healthy watches have no row at all, so the only thing left to read was the one
    // stale `Unanswered`.
    assert_eq!(
        linked(
            true,
            Some(&snapshot),
            &[in_trouble(ObjectKind::Pod, Some(&dead), false)]
        ),
        ui::Link::Live,
        "one watch's stale error took both mutating keys off a cluster still delivering"
    );
    assert_eq!(
        linked(
            true,
            Some(&snapshot),
            &[in_trouble(ObjectKind::Pod, Some(&expired), false)]
        ),
        ui::Link::Expired
    );
    // An expired login answers first, whatever else is in trouble beside it — renewing is *the*
    // next step and `X switch cluster` is promoted onto the footer for it.
    assert_eq!(
        linked(
            true,
            Some(&snapshot),
            &[
                in_trouble(ObjectKind::Pod, Some(&dead), false),
                in_trouble(ObjectKind::Node, Some(&expired), false),
            ]
        ),
        ui::Link::Expired
    );
}

/// **`⚠ disconnected, retrying` is *no watch is answering*, and never *a watch is in trouble***
/// (NOTES § D285 ruling 1). `screens/states.md` § The connection dropped draws a claim about the
/// cluster — *"Not connected to the cluster **right now**"* — and it costs `s` and `r` for as long
/// as it stands, so one quiet watch holding a stale error may not produce it.
///
/// **Measured: a cut all five watches resumed from inside 1.6 s left the node and workload watches
/// `Unanswered` on a quiet cluster** — nothing to deliver is nothing to clear on — and the header
/// said `disconnected` for 340 s while pod events flowed
/// (`reports/2026-09-26-the-error-state-pass.md`).
///
/// **Eight shapes and a startup, because neither `any` nor `all` gets them all right**: the fourth
/// is the scoped run a namespaced `Role` produces, where the node watch is refused for the life of
/// the process and *every row carries `Unanswered`* would therefore never be true. The seventh is
/// the one a predicate keyed on `k8s::Trouble::fault` gets backwards, and the eighth is the only
/// case that can tell this arm's *order* from its *condition*.
#[test]
fn the_connection_word_says_disconnected_only_when_no_watch_answers() {
    let store = a_cluster_with_cards();
    let snapshot = store.snapshot(now()).expect("every LIST landed");
    let dead = watcher::Error::WatchFailed(kube::Error::Service(Box::new(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "timed out",
    ))));
    let refused = watcher::Error::InitialListFailed(api_error(403, "Forbidden"));
    // Read through `k8s::Trouble::fault`, the same public path `linked` reads, so the two fixtures
    // are the faults this test claims they are and not whatever they happen to classify as.
    assert_eq!(
        in_trouble(ObjectKind::Pod, Some(&dead), true).fault(),
        Some(k8s::Fault::Unanswered),
        "the dropped-socket fixture is not the fault this test is about"
    );
    assert_eq!(
        in_trouble(ObjectKind::Node, Some(&refused), false).fault(),
        Some(k8s::Fault::Refused),
        "the 403 fixture is not the fault this test is about"
    );

    // **The universe [`WATCHED`] names, pinned against the store itself.** `k8s::Store::troubles`
    // reports only the watches that are not delivering, so what `linked` reads *nothing is
    // arriving* off is *all five named*. Both ways that can go out of step are silent — a kind
    // that stops being reported leaves `linked` unable to say `Lost` at all, a sixth it does not
    // name is a watch whose health stops being counted — and neither shows up in any other
    // assertion here, because every one of them builds its own rows. `Store::stop_waiting` is the
    // one public way to put every watch of an untouched store into trouble at once.
    let mut nothing_landed = k8s::Store::default();
    nothing_landed.stop_waiting();
    let reported: Vec<ObjectKind> = nothing_landed
        .troubles()
        .iter()
        .map(|trouble| trouble.kind.clone())
        .collect();
    assert_eq!(
        reported,
        WATCHED.to_vec(),
        "`k8s::Store::troubles` no longer reports the watches `WATCHED` names"
    );

    // A `fn` and not a closure: the borrow in the rows it builds outlives the call that built them.
    fn each(failure: &watcher::Error) -> Vec<k8s::Trouble<'_>> {
        WATCHED
            .iter()
            .map(|kind| in_trouble(kind.clone(), Some(failure), true))
            .collect()
    }
    let word = |troubles: &[k8s::Trouble<'_>]| linked(true, Some(&snapshot), troubles);

    // 1. All five dropped — a real total drop, and the shape a startup against a dead port ends in.
    assert_eq!(
        word(&each(&dead)),
        ui::Link::Lost,
        "no watch was answering and the header did not say so"
    );
    // **The same, before anything has ever been published** — the dead-port startup, where `Lost`
    // has to beat `connecting…`. Measured on the binary against a released port: the header drew
    // `⚠ disconnected, retrying` at the first frame and still at +15 s.
    assert_eq!(
        linked(true, None, &each(&dead)),
        ui::Link::Lost,
        "a startup that reached nothing said it was still arriving"
    );

    // 2. The measured defect: the pod watch cleared on its next `Apply`, the node and workload
    //    watches had nothing to deliver and kept a stale `Unanswered`.
    let pods_flowing: Vec<k8s::Trouble<'_>> = each(&dead)
        .into_iter()
        .filter(|trouble| trouble.kind != ObjectKind::Pod)
        .collect();
    // **The list is derived, so it says it found something first** (CLAUDE.md § Code phase rules,
    // `write-guard.py`'s `CANARIES`). `linked` answers `Live` for an **empty** slice too, so a
    // filter that threw everything away would pass the assertion below and prove nothing at all —
    // measured, not argued: `tester` replaced the filter with `|_| false` and this test stayed
    // green. What has to be on screen here is *four stale rows outvoted by one absence*, and four
    // is the number that says so.
    assert_eq!(
        pods_flowing.len(),
        WATCHED.len() - 1,
        "an empty list reads `Live` whatever the predicate does, so the four quiet watches this \
         shape is about were filtered away with the pod watch"
    );
    assert_eq!(
        word(&pods_flowing),
        ui::Link::Live,
        "four quiet watches' stale errors outvoted the one that was delivering"
    );

    // 3. The scoped run: a namespaced `Role` cannot `list nodes`, and that is not a connection
    //    state at all — the defect `linked`'s own doc exists to describe.
    assert_eq!(
        word(&[in_trouble(ObjectKind::Node, Some(&refused), false)]),
        ui::Link::Live,
        "a refused watch took both mutating keys off a live cluster"
    );

    // 4. The same scoped run, now really disconnected. A plain *every row is `Unanswered`* never
    //    fires here, which is why the refusal counts as *not answering* rather than as evidence.
    let scoped_and_dropped: Vec<k8s::Trouble<'_>> = WATCHED
        .iter()
        .map(|kind| {
            let failure = if *kind == ObjectKind::Node {
                &refused
            } else {
                &dead
            };
            in_trouble(kind.clone(), Some(failure), *kind != ObjectKind::Node)
        })
        .collect();
    assert_eq!(
        word(&scoped_and_dropped),
        ui::Link::Lost,
        "a scoped run could not say `disconnected` even with every other watch down"
    );

    // 5. And a refusal is never *evidence* of a drop, however many watches carry one: nothing here
    //    is retrying and `Lost`'s *k8rs is retrying* would be the wrong sentence
    //    (`k8s::Fault::standing`).
    assert_eq!(
        word(&each(&refused)),
        ui::Link::Live,
        "five standing refusals were reported as a connection k8rs is retrying"
    );

    // **The rows [`in_trouble`] cannot build**: it hardcodes both markers, and what the two shapes
    // below turn on is exactly the markers. A `fn` returning `'static` because neither carries a
    // `failure` to borrow from.
    fn marked(kind: ObjectKind, ended: bool, unfinished: bool) -> k8s::Trouble<'static> {
        k8s::Trouble {
            kind,
            listed: true,
            failure: None,
            ended,
            unfinished,
            outstanding: None,
        }
    }

    // 6. Every watch wedged and none of them carrying a `failure` — `k8s::Store::stop_waiting`'s
    //    own shape, which is how a run with a budget ends. `Fault::Unfinished` is the only fault
    //    on screen and it is one of the two this arm reads.
    let all_wedged: Vec<k8s::Trouble<'_>> = WATCHED
        .iter()
        .map(|kind| marked(kind.clone(), false, true))
        .collect();
    // `all` is true of an empty iterator, so the count comes first here too.
    assert_eq!(
        all_wedged.len(),
        WATCHED.len(),
        "this shape is about every watch being wedged and one of them is missing"
    );
    assert!(
        all_wedged
            .iter()
            .all(|trouble| trouble.fault() == Some(k8s::Fault::Unfinished)),
        "the wedged rows do not carry the fault this shape is about"
    );
    assert_eq!(
        word(&all_wedged),
        ui::Link::Lost,
        "a run whose every watch was told the waiting is over said the cluster was answering"
    );

    // 7. **A stream that finished is not a watch that is answering**, and it is the row a
    //    predicate keyed on `fault()` gets backwards: `ended` with no `failure` reads `None`
    //    there (`k8s::Trouble::fault`), which is the same answer a healthy watch would give if
    //    healthy watches had rows — and they do not. Nothing will arrive on it again, so the four
    //    dropped watches beside it are the whole of the cluster.
    let finished = marked(ObjectKind::Pod, true, false);
    assert_eq!(
        finished.fault(),
        None,
        "the finished-stream row does not have the shape this case is about"
    );
    let four_dropped_and_one_finished: Vec<k8s::Trouble<'_>> = std::iter::once(finished)
        .chain(
            each(&dead)
                .into_iter()
                .filter(|trouble| trouble.kind != ObjectKind::Pod),
        )
        .collect();
    assert_eq!(
        four_dropped_and_one_finished.len(),
        WATCHED.len(),
        "this shape is about every watch being accounted for and one of them is missing"
    );
    assert_eq!(
        word(&four_dropped_and_one_finished),
        ui::Link::Lost,
        "a stream that had finished was counted as a watch still answering"
    );

    // 8. **The arms are ordered and this is what pins the order.** An expired login answers before
    //    a dropped connection whatever else is on screen, and the sibling test cannot say so: its
    //    `Expired`-beside-a-drop case carries two rows, so `answering` is true there and `Lost`
    //    was never in the running. Here all five are accounted for, so both arms would fire.
    let expired = watcher::Error::WatchError(
        kube::core::Status::failure("expired", "Unauthorized")
            .with_code(401)
            .boxed(),
    );
    let expired_amid_a_drop: Vec<k8s::Trouble<'_>> = WATCHED
        .iter()
        .map(|kind| {
            let failure = if *kind == ObjectKind::Node {
                &expired
            } else {
                &dead
            };
            in_trouble(kind.clone(), Some(failure), true)
        })
        .collect();
    assert_eq!(
        word(&expired_amid_a_drop),
        ui::Link::Expired,
        "a login that ran out read as a connection k8rs is retrying, which sends the reader to \
         the network instead of to `X switch cluster`"
    );
}

/// **The still-loading paragraph counts the LIST that is running and the healthy one counts what
/// was read** (`screens/states.md` § Still loading, § Nothing is broken).
#[test]
fn the_empty_panes_paragraphs_count_what_the_store_actually_read() {
    let waiting = notes(
        true,
        None,
        &[k8s::Listing {
            kind: ObjectKind::Pod,
            so_far: 2140,
            since: None,
        }],
    );
    let said: Vec<&str> = waiting.iter().map(views::Stripped::as_str).collect();
    println!("{said:?}");
    assert!(
        said[0].contains("2,140 pods"),
        "the count is not the LIST's own progress, grouped: {said:?}"
    );
    assert!(said[0].starts_with("reading the cluster…"));
    assert!(
        said[1].contains("this list"),
        "the second paragraph is missing: {said:?}"
    );

    // A LIST of another kind is not the pod count this paragraph is about.
    let other = notes(
        true,
        None,
        &[k8s::Listing {
            kind: ObjectKind::Node,
            so_far: 9,
            since: None,
        }],
    );
    assert!(
        other[0].as_str().contains("0 pods"),
        "a node LIST was counted as pods: {:?}",
        other[0].as_str()
    );

    let store = a_cluster_with_cards();
    let snapshot = store.snapshot(now()).expect("every LIST landed");
    let healthy = notes(true, Some(&snapshot), &[]);
    let said = healthy[0].as_str();
    println!("{said}");
    assert!(
        said.contains(&plural(snapshot.pods.len(), "pod")),
        "the pod count is not the store's: {said:?}"
    );
    assert!(
        said.ends_with("none of them is in trouble right now."),
        "the sentence is not `screens/states.md`'s: {said:?}"
    );

    // **Nothing connected says so and does not claim to be reading** (`screens/context.md`
    // § After `esc dismiss`): the reason is not repeated — the reader just read it in the box they
    // dismissed — and what is here instead is the one key out.
    let stranded = notes(false, None, &[]);
    let said: Vec<&str> = stranded.iter().map(views::Stripped::as_str).collect();
    println!("{said:?}");
    assert_eq!(
        said,
        vec![
            "⚠ Not connected to the cluster right now.",
            "Press X to try again, or pick a different cluster.",
        ],
        "the frame behind a dismissed failure is not the page's"
    );
    // And it says so whatever the store happens to hold, since nothing in the store is this
    // cluster's.
    assert_eq!(
        notes(
            false,
            Some(&snapshot),
            &[k8s::Listing {
                kind: ObjectKind::Pod,
                so_far: 2140,
                since: None,
            }]
        )
        .iter()
        .map(views::Stripped::as_str)
        .collect::<Vec<&str>>(),
        said,
        "a store left over from another cluster was read as this one's progress"
    );
}

/// **What the detail slot is showing, in the terms the keys are decided in** ([`detailing`]) — the
/// step and the tabs are two states and `from_step` is carried through the second (NOTES § D270).
#[test]
fn the_detail_slot_answers_closed_the_step_or_the_tabs() {
    let object = ObjectId {
        kind: ObjectKind::Pod,
        namespace: Some("default".to_owned()),
        name: "broken-crashloop".to_owned(),
        uid: None,
    };
    let mut console = bare_console();
    assert_eq!(detailing(&console), views::Detailing::Closed);

    console.opened = Some(Opened::Pods(object.clone()));
    assert_eq!(detailing(&console), views::Detailing::Pods);

    for from_step in [false, true] {
        console.opened = Some(Opened::Tabs {
            object: object.clone(),
            from_step,
        });
        assert_eq!(
            detailing(&console),
            views::Detailing::Tabs {
                containers: 0,
                from_step
            },
            "the step `⏎` came through was not carried into the tabs"
        );
    }
}

/// A confirmation as [`installed`] builds one — the dialog `show` publishes, with no verdict yet.
fn an_open_dialog() -> views::Dialog {
    views::Dialog {
        verb: RESTART,
        object: views::Object::new(
            "deployment",
            Some("payments".to_owned()),
            "web".to_owned(),
            Some("u-1".to_owned()),
        ),
        consequence: views::Stripped::of("This replaces every copy of your app, one at a time."),
        warning: None,
        kubectl: views::Stripped::of("kubectl rollout restart deployment/web -n payments"),
        verdict: None,
        asks: None,
        typed: views::Input::default(),
    }
}

/// **The box opens with a dead button and the command log gains the line it is about**
/// (`screens/dialogs.md` rule 3, § Scale — which draws the `$` line in the strip under an open
/// dialog), **then the verdict lands in the box that is already open** and arms it
/// ([`views::Dialog::armed`]).
#[test]
fn the_dialog_opens_dead_and_the_verdict_arms_the_box_that_is_already_open() {
    let mut console = bare_console();
    installed(&mut console, Published::Opening(an_open_dialog()));
    let Some(views::Modal::Confirm(dialog)) = &console.app.modal else {
        panic!("`show` opened no confirmation: {:?}", console.app.modal)
    };
    assert!(dialog.waiting(), "the button was live before the check was");
    assert!(!dialog.armed());
    // **The strip carries nothing yet** (`screens/dialogs.md` rule 7): a dialog that is open
    // teaches its command through its own frame, and a line on the strip would put the running mark
    // on a command nobody has agreed to — on a `delete`, on one that has not been sent at all.
    assert!(
        console.log.lines().is_empty(),
        "the strip gained a mutation's line while its dialog was still asking: {:?}",
        console.log.lines()
    );

    installed(
        &mut console,
        Published::Checked {
            verdict: "The cluster checked it first and accepted it.",
            asks: None,
        },
    );
    let Some(views::Modal::Confirm(dialog)) = &console.app.modal else {
        panic!("the verdict closed the box")
    };
    assert!(!dialog.waiting(), "the verdict did not land");
    assert!(dialog.armed(), "the button did not come alive");
    assert_eq!(
        dialog.consequence,
        an_open_dialog().consequence,
        "the verdict replaced the box instead of landing in it"
    );
    // **Still nothing on the strip** — the verdict is the *check* answering, and rule 7 says the
    // line waits for the real call, dry-run included.
    assert!(
        console.log.lines().is_empty(),
        "an answered check put the line on the strip before anyone agreed to it"
    );

    // **The yes is what appends it, with the running mark** — and the command is the box's own.
    // **A store with nothing to say about this dialog's object, and it is the fixture rather than
    // a convenience.** [`over_modal`]'s confirm arm now asks the store whether the selected object
    // is still there ([`vanished`], NOTES § D22, § D289 ruling 1), and
    // [`a_cluster_with_cards`] is a healthy listed store holding four pods and **no Deployment** —
    // so it answers *gone* about [`an_open_dialog`]'s `payments/web`, correctly, about a console
    // that cannot exist: a card filed under a Deployment owner needs that Deployment on its own
    // watch. This test is about the box's key map and not about a cluster, so the store it presses
    // against is one that has not listed. The guard's own five shapes are
    // [`a_yes_on_an_object_that_went_away_is_answered_gone_and_sends_no_command`].
    let store = before_the_list();
    let answered = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Enter),
        &store,
    );
    assert!(matches!(answered, Did::Answered(Reply::Yes(_))));
    let line = console
        .log
        .lines()
        .last()
        .map(views::Stripped::as_str)
        .unwrap_or_default();
    // **The command half is asserted and the gap is not**: `views::OUTCOME_GAP` is that file's own
    // width and restating it here is the second copy that goes stale (`views::outcome_of` is the
    // reader the strip itself uses).
    let (command, running) = views::outcome_of(line);
    assert_eq!(
        command, "$ kubectl rollout restart deployment/web -n payments",
        "the yes did not put the box's own command on the strip"
    );
    assert!(
        running.ends_with('…') && !running.contains('→'),
        "the line went on already answered: {line:?}"
    );
}

/// A confirmation whose check has answered — armed, so `⏎` confirms it (`views::Dialog::armed`).
fn an_armed_dialog() -> views::Dialog {
    views::Dialog {
        verdict: Some("The cluster checked it first and accepted it."),
        ..an_open_dialog()
    }
}

/// **Drive one mutation the way a reader does** — the box opens, the check answers, `⏎` confirms,
/// and `ops` comes back with this outcome. Returns the console so a test can read what the strip
/// and the modal hold at the end.
fn answered_with(outcome: Option<ops::Outcome>) -> Console<'static> {
    // **A store with nothing to say about this dialog's object, and it is the fixture rather than
    // a convenience.** [`over_modal`]'s confirm arm now asks the store whether the selected object
    // is still there ([`vanished`], NOTES § D22, § D289 ruling 1), and
    // [`a_cluster_with_cards`] is a healthy listed store holding four pods and **no Deployment** —
    // so it answers *gone* about [`an_open_dialog`]'s `payments/web`, correctly, about a console
    // that cannot exist: a card filed under a Deployment owner needs that Deployment on its own
    // watch. This test is about the box's key map and not about a cluster, so the store it presses
    // against is one that has not listed. The guard's own five shapes are
    // [`a_yes_on_an_object_that_went_away_is_answered_gone_and_sends_no_command`].
    let store = before_the_list();
    let mut console = bare_console();
    installed(&mut console, Published::Opening(an_armed_dialog()));
    let answered = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Enter),
        &store,
    );
    assert!(
        matches!(answered, Did::Answered(Reply::Yes(_))),
        "the armed box did not confirm"
    );
    settled(
        &mut console,
        Ok(ops::Performed {
            outcome,
            recorded: true,
        }),
    );
    console
}

/// **Every terminal path of `ops::perform` replaces the modal** — the whole of what this wiring
/// owes (NOTES § D272 § 2: nothing in `views.rs` can perform a `Confirm → Refused` transition, and
/// `esc` is inert while a check is out, so a driver that dropped the ball would leave a box nothing
/// can close).
///
/// **Eight endings, and the assertion is *the confirmation is gone*, not *a particular box is
/// drawn*** — that is the claim; which box replaces it is the arm below it.
#[test]
fn every_ending_replaces_the_confirmation_and_says_what_happened() {
    let endings: Vec<(&str, Result<ops::Performed, String>)> = vec![
        (
            "done",
            Ok(ops::Performed {
                outcome: Some(ops::Outcome::Done),
                recorded: true,
            }),
        ),
        (
            "started",
            Ok(ops::Performed {
                outcome: Some(ops::Outcome::Started),
                recorded: true,
            }),
        ),
        (
            "cancelled",
            Ok(ops::Performed {
                outcome: Some(ops::Outcome::Cancelled),
                recorded: true,
            }),
        ),
        (
            "gone",
            Ok(ops::Performed {
                outcome: Some(ops::Outcome::Gone),
                recorded: true,
            }),
        ),
        (
            "changed",
            Ok(ops::Performed {
                outcome: Some(ops::Outcome::Changed),
                recorded: true,
            }),
        ),
        (
            "the check never went out",
            Ok(ops::Performed {
                outcome: Some(ops::Outcome::NotSent {
                    fault: k8s::Fault::Unanswered,
                    said: None,
                }),
                recorded: true,
            }),
        ),
        (
            "the call failed",
            Ok(ops::Performed {
                outcome: Some(ops::Outcome::Failed {
                    fault: k8s::Fault::Refused,
                    said: Some("forbidden".to_owned()),
                }),
                recorded: true,
            }),
        ),
        (
            "nothing could be recorded",
            Ok(ops::Performed {
                outcome: None,
                recorded: false,
            }),
        ),
    ];
    for (what, performed) in endings {
        let mut console = bare_console();
        installed(&mut console, Published::Opening(an_armed_dialog()));
        console.app.changing = Some(an_armed_dialog().object);
        settled(&mut console, performed);
        assert!(
            !matches!(console.app.modal, Some(views::Modal::Confirm(_))),
            "{what} left the confirmation on screen with no key that can close it"
        );
        assert!(
            console.app.changing.is_none(),
            "{what} left the header saying a write was still running"
        );
    }

    // **And no ending leaves the strip's line running for ever** — driven through the yes, because
    // that is the only thing that puts a line there at all (`screens/dialogs.md` rule 7).
    for outcome in [
        None,
        Some(ops::Outcome::Done),
        Some(ops::Outcome::Started),
        Some(ops::Outcome::Gone),
        Some(ops::Outcome::Changed),
        Some(ops::Outcome::NotSent {
            fault: k8s::Fault::Unanswered,
            said: None,
        }),
        Some(ops::Outcome::Failed {
            fault: k8s::Fault::Refused,
            said: Some("forbidden".to_owned()),
        }),
    ] {
        let word = outcome_word(outcome.as_ref());
        let console = answered_with(outcome);
        let last = console
            .log
            .lines()
            .last()
            .map(views::Stripped::as_str)
            .unwrap_or_default()
            .to_owned();
        println!("{word}: {last}");
        // The running mark is private to `views.rs`, so the line is read the way the strip reads
        // it: `views::outcome_of` splits it into the command and whatever followed, and an answered
        // line's tail carries the outcome word (`views::Log::outcome`).
        let (_, after) = views::outcome_of(&last);
        assert!(
            after.contains("→ "),
            "{word} left the command log line running for ever: {last:?}"
        );
    }

    // **A box nobody said yes to leaves the strip empty** — *Cancelled appends nothing*
    // (NOTES § D233 ruling 1), which here is the whole of what `esc` does to the log.
    let store = a_cluster_with_cards();
    let mut cancelled = bare_console();
    installed(&mut cancelled, Published::Opening(an_armed_dialog()));
    let dismissed = keyed(
        &mut cancelled,
        key(ratatui::crossterm::event::KeyCode::Esc),
        &store,
    );
    assert!(matches!(dismissed, Did::Answered(Reply::No)));
    settled(
        &mut cancelled,
        Ok(ops::Performed {
            outcome: Some(ops::Outcome::Cancelled),
            recorded: true,
        }),
    );
    assert!(
        cancelled.log.lines().is_empty(),
        "a cancelled mutation left a command on the strip: {:?}",
        cancelled.log.lines()
    );
}

/// **Which box replaces it, per ending** (`screens/dialogs.md` § The cluster said no, § The object
/// went away) — and `sent` is the field that decides whether a check can be said to have stopped
/// anything, so the two refusal shapes may not collapse into one.
#[test]
fn a_refusal_opens_the_box_that_says_which_side_of_the_call_it_was() {
    let opened = |performed: Result<ops::Performed, String>| {
        let mut console = bare_console();
        installed(&mut console, Published::Opening(an_open_dialog()));
        console.app.changing = Some(an_open_dialog().object);
        settled(&mut console, performed);
        console.app.modal
    };
    assert_eq!(
        opened(Ok(ops::Performed {
            outcome: Some(ops::Outcome::Done),
            recorded: true
        })),
        None,
        "a change that landed left a box over the screen"
    );
    assert!(matches!(
        opened(Ok(ops::Performed {
            outcome: Some(ops::Outcome::NotSent {
                fault: k8s::Fault::Unanswered,
                said: None
            }),
            recorded: true
        })),
        Some(views::Modal::Refused { sent: false, .. })
    ));
    assert!(matches!(
        opened(Ok(ops::Performed {
            outcome: Some(ops::Outcome::Failed {
                fault: k8s::Fault::Refused,
                said: Some("forbidden".to_owned())
            }),
            recorded: true
        })),
        Some(views::Modal::Refused {
            sent: true,
            said: Some(_),
            ..
        })
    ));
    // **The object the box claims is the one the confirmation was about**, carried across the
    // moment the box closed on the yes (`views::App::changing`, NOTES § D20) — read from a modal
    // that is already gone, this drew `Gone` about nothing.
    let gone = opened(Ok(ops::Performed {
        outcome: Some(ops::Outcome::Gone),
        recorded: true,
    }));
    let Some(views::Modal::Gone { object, .. }) = gone else {
        panic!("an object that went away opened no box: {gone:?}")
    };
    assert_eq!(object.name(), "web");
    assert_eq!(object.namespace(), Some("payments"));
}

/// **A key that is not a press is not a key** — crossterm reports a release and a repeat on the
/// terminals that support the kitty protocol, and a router that acted on all three would fire every
/// key twice (`examples/spike_tui.rs`, which reads the same field).
#[test]
fn a_key_release_is_not_a_command() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let released =
        ratatui::crossterm::event::Event::Key(ratatui::crossterm::event::KeyEvent::new_with_kind(
            ratatui::crossterm::event::KeyCode::Char('q'),
            ratatui::crossterm::event::KeyModifiers::NONE,
            ratatui::crossterm::event::KeyEventKind::Release,
        ));
    assert!(
        matches!(keyed(&mut console, released, &store), Did::Nothing),
        "a key *release* quit the console"
    );
    assert!(matches!(keyed(&mut console, typed('q'), &store), Did::Quit));
}

/// **`esc` while a filter has focus empties the field before it leaves it** — narrow to wide, which
/// is `views::Filters::clears`'s order so the footer that names the field and the key that empties
/// it are one answer (`screens/widgets.md` § 2b, `screens/states.md` § The filter hides every row).
#[test]
fn esc_while_typing_clears_the_field_and_only_then_leaves_it() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let _ = keyed(&mut console, typed('/'), &store);
    for character in "web".chars() {
        let _ = keyed(&mut console, typed(character), &store);
    }
    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Esc),
        &store,
    );
    assert_eq!(
        console.app.typed().map(views::Input::text),
        Some(""),
        "the first `esc` left the field open and dropped what was in it, or the other way round"
    );
    assert_eq!(console.app.typing, Some(views::Typing::Text));

    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Esc),
        &store,
    );
    assert_eq!(console.app.typing, None, "the second `esc` did not leave");
}

/// **The cards a pane holds are two of its three answers, and *loading* is not one of them**
/// (`views::Pane`, PRIOR-ART § C2 — collapsing *nothing came back yet* into *there is nothing* is
/// the report k9s got most).
#[test]
fn the_router_reads_the_cards_a_refused_pane_still_has() {
    let store = a_cluster_with_cards();
    let snapshot = store.snapshot(now()).expect("every LIST landed");
    let cards = views::cards(&analyze(&snapshot), &snapshot.workloads, &now());
    assert!(!cards.is_empty());

    assert!(carded_pane(&views::Pane::Loading).is_empty());
    assert_eq!(
        carded_pane(&views::Pane::Ready(cards.clone())).len(),
        cards.len()
    );
    // A refusal carries whatever did come back — the cursor still has those rows to move across.
    assert_eq!(
        carded_pane(&views::Pane::Denied(
            "k8rs is not getting pods".to_owned(),
            cards.clone()
        ))
        .len(),
        cards.len(),
        "a refusal cleared the rows it came back with"
    );
}

/// One frame, drawn into a test backend — the whole of [`drawn`], with no terminal and no cluster.
fn framed(console: &mut Console<'_>, store: &k8s::Store) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))
        .expect("a terminal over a test backend");
    let at = Watching {
        renewal: None,
        coverage: k8s::Coverage::Cluster,
    };
    drawn(&mut terminal, console, store, &at).expect("the frame draws");
    screened(&terminal)
}

/// **What a test backend is holding, as lines** — [`framed`]'s own reading, split out for the test
/// that draws twice into one terminal and asks what was on it in between.
fn screened(terminal: &ratatui::Terminal<ratatui::backend::TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    (0..buffer.area.height)
        .map(|row| {
            (0..buffer.area.width)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// **A resumed terminal is cleared, and the frame after it is sent whole** (NOTES § D24,
/// [`repainted`]) — the two halves of the repaint, and each of them is passed by code that does
/// only the other: a screen cleared without the buffer reset draws nothing at all next frame, and a
/// buffer reset without the clear leaves the shell's scrollback under our cells.
///
/// **This is the claim the real run made fail.** `Terminal::clear` — the obvious call — reads the
/// cursor position off stdin, which the key thread owns, so `fg` ended the run instead of redrawing
/// it (test host, 2026-09-24). A `TestBackend` answers that read from a field and would have
/// passed.
#[test]
fn a_repaint_clears_the_screen_and_makes_the_next_frame_a_whole_one() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))
        .expect("a terminal over a test backend");
    let at = Watching {
        renewal: None,
        coverage: k8s::Coverage::Cluster,
    };
    drawn(&mut terminal, &mut console, &store, &at).expect("the frame draws");
    // The canary: what follows is about a frame that was really there.
    assert!(
        screened(&terminal).contains("ALERTS"),
        "not a console frame:\n{}",
        screened(&terminal)
    );

    repainted(&mut terminal).expect("the repaint");
    assert!(
        !screened(&terminal).contains("ALERTS"),
        "the screen kept the frame a suspend gave away:\n{}",
        screened(&terminal)
    );

    drawn(&mut terminal, &mut console, &store, &at).expect("the frame draws");
    assert!(
        screened(&terminal).contains("ALERTS"),
        "the frame after a repaint was the difference against a screen that is gone:\n{}",
        screened(&terminal)
    );
}

/// **The three frames this wiring can draw, read off the screen** — still loading, cards, and the
/// cluster picker over an empty body (`screens/states.md` § Still loading, `screens/alerts.md`,
/// `screens/context.md`).
///
/// **The picker's rows reach the screen only while it is open**, which is `ui::Screen::contexts`'
/// own contract — *empty when nothing is picking* — and the only way to see that from outside
/// [`drawn`] is to look at the cells.
#[test]
fn the_frames_this_wiring_draws_say_which_state_the_cluster_is_in() {
    let mut console = bare_console();
    console.contexts = vec![k8s::Choice {
        name: Some("prod-eu".to_owned()),
        key: "prod-eu".to_owned(),
        server: k8s::Address::Server("https://prod-eu.invalid:6443".to_owned()),
        shadowed: false,
        namespace: None,
        insecure: false,
        tag: k8s::Tag::Blank,
        current: true,
    }];

    // 1. Nothing has answered: the still-loading screen, and the header says so.
    let waiting = framed(&mut console, &k8s::Store::default());
    println!("{waiting}");
    assert!(waiting.contains("reading the cluster…"), "{waiting}");
    assert!(waiting.contains("nodes …"), "{waiting}");
    assert!(waiting.contains("connecting…"), "{waiting}");
    assert!(waiting.contains("ALERTS"), "{waiting}");
    assert!(
        !waiting.contains("prod-eu"),
        "the picker's rows reached a screen with no picker open:\n{waiting}"
    );

    // 2. The cards, over a live link.
    let store = a_cluster_with_cards();
    let drawn_cards = framed(&mut console, &store);
    println!("{drawn_cards}");
    assert!(
        drawn_cards.contains("broken-"),
        "no card was drawn:\n{drawn_cards}"
    );
    assert!(drawn_cards.contains("live"), "{drawn_cards}");
    assert!(
        !drawn_cards.contains("nothing is broken"),
        "a cluster with three broken pods was called healthy:\n{drawn_cards}"
    );
    assert!(
        !drawn_cards.contains("reading the cluster"),
        "a finished read still said it was reading:\n{drawn_cards}"
    );

    // 3. The picker, whose rows are handed over only now.
    console.app.modal = Some(views::Modal::ContextPick(views::Picker::new(
        &console.contexts,
        views::Connection::Live(Some("prod-eu".to_owned())),
    )));
    let picking = framed(&mut console, &store);
    println!("{picking}");
    assert!(
        picking.contains("prod-eu"),
        "the picker drew no rows:\n{picking}"
    );
}

/// **The two arrows are the one thing a filter does not take, and they move the same way they do
/// outside typing** (`screens/widgets.md` § 2b: *"`↑` / `↓` still move the selection over whatever
/// rows the live-narrowed list is currently showing, exactly as they do outside typing"* — an arrow
/// is never a letter, which is what lets the picker's own list do the same thing).
///
/// **A `ctrl`-anything is neither text nor a command while a filter has focus**: `ctrl-c` puts no
/// `c` in the buffer, and it does not quit either — `views::App::may_quit` is already no while
/// typing (NOTES § D12).
#[test]
fn the_arrows_still_move_the_list_while_a_filter_is_being_typed() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let anchors = [None, None, None, None];
    let _ = keyed(&mut console, typed('/'), &store);

    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Down),
        &store,
    );
    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Down),
        &store,
    );
    assert_eq!(
        console.app.content.selected(&anchors),
        Some(2),
        "`↓` stopped moving the list the moment a filter took focus"
    );
    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Up),
        &store,
    );
    assert_eq!(
        console.app.content.selected(&anchors),
        Some(1),
        "`↑` and `↓` moved the same way"
    );
    assert_eq!(
        console.app.typed().map(views::Input::text),
        Some(""),
        "an arrow was typed into the filter"
    );

    let pressed = keyed(
        &mut console,
        control(ratatui::crossterm::event::KeyCode::Char('c')),
        &store,
    );
    assert!(
        matches!(pressed, Did::Nothing),
        "`ctrl-c` while typing was a command"
    );
    assert_eq!(
        console.app.typed().map(views::Input::text),
        Some(""),
        "`ctrl-c` put a `c` in the filter"
    );
}

/// **`ctrl-d` on a selected object asks for a delete, and `r` on the same one reaches nothing** —
/// the per-key offer (NOTES § D269, `views::Offer::act`): `ops::delete` serves a pod and
/// `ops::restart` does not, so the two keys answer differently over one card and a router that
/// asked one question for both would offer a restart no operation can perform.
///
/// **What comes back is the mutation, not the dialog** — the loop returns it so the `ops::Mutation`
/// and the audit `File` can be built in the frame that runs the loop (NOTES § D232).
#[test]
fn ctrl_d_asks_for_a_delete_on_the_card_under_the_cursor_and_r_reaches_nothing() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    // The first card, whatever the captures ordered it as — its owner is what both keys ask about.
    let cards = carded(&store);
    let card = selected(&console, &cards).expect("the captures produced a selectable card");
    println!("{:?} {:?}", card.owner.kind, card.owner.name);
    // **A frame first, because the key is checked against the line that was drawn**
    // ([`Console::offer`]): a console that has drawn nothing offers nothing.
    let _ = framed(&mut console, &store);

    let asked = keyed(
        &mut console,
        control(ratatui::crossterm::event::KeyCode::Char('d')),
        &store,
    );
    let Did::Mutate(wanted) = asked else {
        panic!("`ctrl-d` on a selected object reached nothing")
    };
    assert_eq!(wanted.verb, DELETE);
    assert_eq!(wanted.name, card.owner.name);
    assert_eq!(wanted.namespace, card.owner.namespace);
    assert!(
        KINDS.iter().any(|known| known.singular == wanted.kind),
        "the kind handed to `ops` is not one the driver resolved: {:?}",
        wanted.kind
    );

    // A pod has no `rollout restart`, so that key is not on the line and cannot be pressed either.
    assert!(
        matches!(keyed(&mut console, typed('r'), &store), Did::Nothing),
        "`r` acted on a kind `ops::restart` does not serve"
    );

    // **And neither key reaches anything while a write is already on the wire** — a second mutation
    // is refused while one is running (`views::App::may_mutate`, D20).
    console.app.changing = Some(views::Object::new("pod", None, "web".to_owned(), None));
    assert!(matches!(
        keyed(
            &mut console,
            control(ratatui::crossterm::event::KeyCode::Char('d')),
            &store
        ),
        Did::Nothing
    ));
}

/// **`⏎` on a card opens the object it is about** (`views::App::pick_pods`: a bare pod's card is
/// the pod itself, so it opens directly rather than through the which-pods step).
#[test]
fn enter_on_a_card_opens_the_object_that_card_is_about() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let cards = carded(&store);
    let card = selected(&console, &cards).expect("a selectable card");
    let expected = card
        .pods()
        .first()
        .map_or_else(|| card.owner.clone(), |id| (*id).clone());

    assert!(matches!(
        keyed(
            &mut console,
            key(ratatui::crossterm::event::KeyCode::Enter),
            &store
        ),
        Did::Changed
    ));
    match &console.opened {
        // A group of two or more opens the step instead, which is the same key's other answer.
        Some(Opened::Pods(owner)) => assert_eq!(*owner, card.owner),
        Some(Opened::Tabs { object, from_step }) => {
            assert_eq!(*object, expected);
            assert!(!from_step, "`⏎` on a card came through the step");
            assert_eq!(
                console.app.tab,
                views::Tab::default(),
                "the slot opened on a tab nobody asked for"
            );
        }
        None => panic!("`⏎` on a card opened nothing"),
    }
}

/// **A store whose workload watch carries the committed Deployments too** — a card filed under a
/// Deployment is the only shape `r restart` can act on, because `ops::restart` serves the three
/// workloads and no pod (NOTES § D269).
fn a_cluster_with_a_workload_card() -> k8s::Store {
    let mut store = a_cluster_with_cards();
    for deployment in objects::<Deployment>("deployments.json") {
        store.deployment(&now(), Event::Apply(deployment));
    }
    store
}

/// **`r` on a kind `ops::restart` serves asks for one** — the other half of the per-key offer,
/// whose negative (`r` on a pod) is asserted beside `ctrl-d`'s positive above.
#[test]
fn r_asks_for_a_restart_on_a_kind_that_has_one() {
    let store = a_cluster_with_a_workload_card();
    let mut console = bare_console();
    let cards = carded(&store);
    let at = cards
        .iter()
        .position(|card| card.owner.kind == ObjectKind::Deployment)
        .expect("the committed Deployments produced no card");
    let owner = cards[at].owner.clone();
    console.app.content.select(at, &vec![None; cards.len()]);
    assert_eq!(
        selected(&console, &cards).map(|card| card.owner.clone()),
        Some(owner.clone()),
        "the cursor is not on the Deployment's card"
    );
    let _ = framed(&mut console, &store);

    let asked = keyed(&mut console, typed('r'), &store);
    let Did::Mutate(wanted) = asked else {
        panic!("`r` on a Deployment reached nothing")
    };
    assert_eq!(wanted.verb, RESTART);
    assert_eq!(wanted.kind, "deployment");
    assert_eq!(wanted.name, owner.name);
}

/// **Keys that belong to a detail tab reach nothing while no tab is open** — `[`, `]` and `f` are
/// the logs tab's own (`screens/detail.md`), and `X` is unbound while any modal is open
/// (`views::App::may_switch_cluster`, NOTES § D16: switching clusters under an open box is how a
/// dialog ends up naming an object on a cluster it was never read from).
#[test]
fn a_tabs_own_keys_and_the_switcher_are_refused_where_they_do_not_belong() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    for pressed in [typed('['), typed(']'), typed('f')] {
        assert!(
            matches!(keyed(&mut console, pressed.clone(), &store), Did::Nothing),
            "{pressed:?} acted with no detail tab open"
        );
    }
    assert_eq!(console.app.tab, views::Tab::default());
    assert!(!console.app.following);

    let _ = keyed(&mut console, typed('?'), &store);
    let _ = keyed(&mut console, typed('X'), &store);
    assert_eq!(
        console.app.modal,
        Some(views::Modal::Help),
        "`X` opened the picker from under an open box"
    );
}

/// **`esc` out of a tab the which-pods step opened goes back to the step, and out of one opened
/// straight off a card goes back to the view** (`views::Detailing::Tabs`'s `from_step`, NOTES §
/// D270 — the fact carried so the router has an answer that is not a private flag).
#[test]
fn esc_out_of_a_tab_goes_back_to_whatever_opened_it() {
    let store = a_cluster_with_cards();
    let object = ObjectId {
        kind: ObjectKind::Pod,
        namespace: Some("default".to_owned()),
        name: "broken-crashloop".to_owned(),
        uid: None,
    };
    let mut console = bare_console();
    console.opened = Some(Opened::Tabs {
        object: object.clone(),
        from_step: true,
    });
    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Esc),
        &store,
    );
    match &console.opened {
        Some(Opened::Pods(owner)) => assert_eq!(*owner, object),
        other => panic!(
            "`esc` did not go back to the step it came through: {other:?}",
            other = other.is_some()
        ),
    }

    console.opened = Some(Opened::Tabs {
        object,
        from_step: false,
    });
    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Esc),
        &store,
    );
    assert!(
        console.opened.is_none(),
        "`esc` out of a tab nobody stepped into went somewhere"
    );
}

/// **A confirmation answers only the keys its own footer names, and only when it names them**
/// (`screens/dialogs.md` § The verdict line, NOTES § D214): `⏎` is dead until the check answers,
/// `esc` is inert for exactly as long, and a press-only box takes no typing at all.
#[test]
fn a_confirmation_refuses_every_key_its_footer_does_not_name() {
    // **A store with nothing to say about this dialog's object, and it is the fixture rather than
    // a convenience.** [`over_modal`]'s confirm arm now asks the store whether the selected object
    // is still there ([`vanished`], NOTES § D22, § D289 ruling 1), and
    // [`a_cluster_with_cards`] is a healthy listed store holding four pods and **no Deployment** —
    // so it answers *gone* about [`an_open_dialog`]'s `payments/web`, correctly, about a console
    // that cannot exist: a card filed under a Deployment owner needs that Deployment on its own
    // watch. This test is about the box's key map and not about a cluster, so the store it presses
    // against is one that has not listed. The guard's own five shapes are
    // [`a_yes_on_an_object_that_went_away_is_answered_gone_and_sends_no_command`].
    let store = before_the_list();
    let mut console = bare_console();
    installed(&mut console, Published::Opening(an_open_dialog()));

    // While the check is out: no key does anything, and the box stays.
    //
    // **`ctrl-z` is in this list and is answered nowhere near the others** (NOTES § D24,
    // `k8s-admin` finding 3): `ops::CHECK_DEADLINE` is 35 s of `CLOCK_MONOTONIC` and that clock
    // runs while a process is stopped, so a suspend over an open confirmation comes back to *"k8rs
    // waited 35 seconds to check this change and nothing came back"* — about a cluster that was
    // never slow. It is the one window [`may_stop`] closes.
    for pressed in [
        key(ratatui::crossterm::event::KeyCode::Enter),
        key(ratatui::crossterm::event::KeyCode::Esc),
        typed('q'),
        control(ratatui::crossterm::event::KeyCode::Char('z')),
    ] {
        assert!(
            matches!(keyed(&mut console, pressed.clone(), &store), Did::Nothing),
            "{pressed:?} acted while the cluster was still being asked"
        );
        assert!(
            matches!(console.app.modal, Some(views::Modal::Confirm(_))),
            "{pressed:?} closed a box whose check had not answered"
        );
    }

    // A press-only dialog takes no typing — its own `asks` is `None`.
    let _ = keyed(&mut console, typed('w'), &store);
    let Some(views::Modal::Confirm(dialog)) = &console.app.modal else {
        panic!("the box went")
    };
    assert_eq!(
        dialog.typed.text(),
        "",
        "a press-only confirmation took a typed name"
    );

    // The verdict lands: `⏎` now confirms, and what it answers with is what the reader typed.
    installed(
        &mut console,
        Published::Checked {
            verdict: "The cluster checked it first and accepted it.",
            asks: None,
        },
    );
    // **And `ctrl-z` works again the instant it answers** (`screens/help.md` § Rules: *"both keys
    // work again the instant it answers"*). This is the other side of the window above, and the
    // only assertion that can tell [`may_stop`]'s `waiting()` from a guard that asked merely *is a
    // box open* — measured: dropping it leaves all 1540 green (`tester`, 2026-09-24).
    assert!(
        matches!(
            keyed(
                &mut console,
                control(ratatui::crossterm::event::KeyCode::Char('z')),
                &store
            ),
            Did::Suspend
        ),
        "`ctrl-z` stayed inert after the check had answered"
    );

    let answered = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Enter),
        &store,
    );
    assert!(
        matches!(answered, Did::Answered(Reply::Yes(_))),
        "`⏎` on an armed box did not confirm"
    );
    assert_eq!(console.app.modal, None, "the box stayed open on the yes");
    assert!(
        console.app.changing.is_some(),
        "the header does not say a write is running"
    );
}

/// **A card about a group of two**, constructed — the committed captures are all single broken
/// pods, so a group is the one shape no fixture can produce and the which-pods step is the one
/// screen that needs it.
///
/// **Constructed and not captured, which is not a breach of NOTES § D53** for `views_tests.rs`'s
/// own reason: that rule is about *captures*, and `Card` and `Finding` are k8rs's own types. What
/// this drives is what the **keys** do once a group exists.
fn a_group_of_two() -> views::Card {
    let owner = ObjectId {
        kind: ObjectKind::Deployment,
        namespace: Some("payments".to_owned()),
        name: "web".to_owned(),
        uid: Some("d-1".to_owned()),
    };
    let pod = |name: &str, uid: &str| ObjectId {
        kind: ObjectKind::Pod,
        namespace: Some("payments".to_owned()),
        name: name.to_owned(),
        uid: Some(uid.to_owned()),
    };
    let finding = |object: ObjectId| Finding {
        severity: Severity::Critical,
        title: "Containers exceeded their memory limit".to_owned(),
        evidence: "limit 256Mi · exit 137".to_owned(),
        action: "raise limits.memory".to_owned(),
        kubectl_cmd: None,
        owner: owner.clone(),
        object,
        timestamp: Some(four_minutes_ago()),
    };
    views::Card {
        owner: owner.clone(),
        findings: vec![finding(pod("web-1", "p-1")), finding(pod("web-2", "p-2"))],
        affected: 2,
        total: Some(3),
    }
}

/// **`⏎` on a group opens the which-pods step, the step's own cursor moves inside it, and the
/// second `⏎` opens the tabs on the pod that was picked** (`screens/detail.md` § Picking a pod,
/// before Detail has one; NOTES § D270).
///
/// **The object the tabs open on is the card's own `ObjectId`, uid and all** — built from the
/// cursor's anchor instead, which is the pod's *name*, `ui::pinned` matches nothing and the tab
/// silently pins no findings at all (`ui::Detail::object`, raised by both Phase 11 reviews).
#[test]
fn a_group_opens_the_step_and_the_step_opens_the_pod_that_was_picked() {
    let cards = vec![a_group_of_two()];
    let pods: Vec<ObjectId> = cards[0].pods().into_iter().cloned().collect();
    let mut console = bare_console();

    let opened = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &cards,
        &before_the_list(),
    );
    assert!(matches!(opened, Did::Changed));
    match &console.opened {
        Some(Opened::Pods(owner)) => assert_eq!(*owner, cards[0].owner),
        _ => panic!("a group of two did not open the which-pods step"),
    }
    assert_eq!(detailing(&console), views::Detailing::Pods);

    // The step's own cursor, not the view's — the card the reader opened is still under that one.
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Down),
        &cards,
        &before_the_list(),
    );
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &cards,
        &before_the_list(),
    );
    match &console.opened {
        Some(Opened::Tabs { object, from_step }) => {
            assert!(from_step, "the tabs do not know they came through the step");
            assert_eq!(
                *object, pods[1],
                "the second row opened the wrong pod's tabs"
            );
            assert_eq!(
                object.uid.as_deref(),
                Some("p-2"),
                "the object was built from the cursor's anchor and not from the card"
            );
        }
        _ => panic!("`⏎` on a picked pod opened no tabs"),
    }

    // And `esc` goes back to the step it came through, not past it.
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Esc),
        &cards,
        &before_the_list(),
    );
    assert!(matches!(console.opened, Some(Opened::Pods(_))));
}

/// **A step stays open while its group still has pods, and closes itself when the group is gone** —
/// [`drawn`]'s own two-part condition (`screens/detail.md` § Picking a pod's *"the same
/// `esc`-shaped return"*, and `reports/2026-09-24-the-console-event-loop.md` § 6, which is where
/// `k8s-admin` found the half this test's second frame is about).
///
/// **Both halves, because either one alone passes a frame that simply always closes the step.** The
/// self-closing half was written for a pod list that emptied under an open step; with `||` where
/// the `&&` is, the same line closes a step the reader is standing in — one frame after `⏎` opened
/// it, so `esc` has nothing to go back to and `⏎` reaches nothing. Measured: `&&` → `||` survived
/// the whole suite (`just mutants-diff`, 2026-09-24).
#[test]
fn a_step_stays_open_while_its_group_has_pods_and_closes_when_it_has_none() {
    let store = a_cluster_with_cards();
    let cards = carded(&store);
    let card = cards.first().expect("the captures produced no card");

    let mut standing = bare_console();
    standing.opened = Some(Opened::Pods(card.owner.clone()));
    let drawn = framed(&mut standing, &store);
    println!("{drawn}");
    assert!(
        matches!(standing.opened, Some(Opened::Pods(_))),
        "the frame closed a step whose group still has {} pod(s)",
        card.pods().len()
    );
    assert_eq!(
        detailing(&standing),
        views::Detailing::Pods,
        "the step the reader is standing in is not what the keys are decided against"
    );

    // The same step over an owner Alerts has no card for — what a pod list emptying under it
    // leaves.
    let mut emptied = bare_console();
    emptied.opened = Some(Opened::Pods(ObjectId {
        kind: ObjectKind::Deployment,
        namespace: Some("default".to_owned()),
        name: "nothing-is-wrong-with-me".to_owned(),
        uid: None,
    }));
    let drawn = framed(&mut emptied, &store);
    println!("{drawn}");
    assert!(
        emptied.opened.is_none(),
        "a step with no group left under it stayed open, so `esc` needs two presses"
    );
    assert_eq!(detailing(&emptied), views::Detailing::Closed);
}

/// One key press as [`pressed`] takes it, with no modifier.
fn pressing(code: ratatui::crossterm::event::KeyCode) -> ratatui::crossterm::event::KeyEvent {
    ratatui::crossterm::event::KeyEvent::new(code, ratatui::crossterm::event::KeyModifiers::NONE)
}

/// **The cluster picker's own keys** (`screens/context.md`): `↑` / `↓` move over the rows the
/// filter leaves, every printable key narrows it, `⌫` widens it again, `⏎` on the row that is
/// already current just closes the box, and `esc` closes it too.
///
/// **`⏎` on a row that would *connect* answers [`Did::Switch`] with that row**, which
/// [`console`]'s own frame turns into a second `connect()` (NOTES § D264 ruling 13).
#[test]
fn the_pickers_own_keys_move_narrow_and_close_it() {
    let row = |name: &str, current: bool| k8s::Choice {
        name: Some(name.to_owned()),
        key: name.to_owned(),
        server: k8s::Address::Server(format!("https://{name}.invalid:6443")),
        shadowed: false,
        namespace: None,
        insecure: false,
        tag: k8s::Tag::Blank,
        current,
    };
    let contexts = vec![row("prod-eu", true), row("staging", false)];
    let mut console = bare_console();
    console.contexts = contexts.clone();
    let opened = |console: &mut Console<'_>| {
        console.app.modal = Some(views::Modal::ContextPick(views::Picker::new(
            &contexts,
            views::Connection::Live(Some("prod-eu".to_owned())),
        )));
    };
    let at = |console: &Console<'_>| match &console.app.modal {
        Some(views::Modal::ContextPick(picker)) => picker.selected(&contexts),
        _ => None,
    };

    opened(&mut console);
    assert_eq!(
        at(&console),
        Some(0),
        "the picker did not open on the current row"
    );
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Down),
        &[],
        &before_the_list(),
    );
    assert_eq!(at(&console), Some(1), "`↓` did not move the picker");
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Up),
        &[],
        &before_the_list(),
    );
    assert_eq!(at(&console), Some(0), "`↑` did not move it back");

    // Typing narrows the list to the row it names, and the cursor gives way to what is shown.
    for character in "stag".chars() {
        let _ = pressed(
            &mut console,
            pressing(ratatui::crossterm::event::KeyCode::Char(character)),
            &[],
            &before_the_list(),
        );
    }
    assert_eq!(
        at(&console),
        Some(1),
        "the filter did not narrow the picker"
    );
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Backspace),
        &[],
        &before_the_list(),
    );
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Backspace),
        &[],
        &before_the_list(),
    );
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Backspace),
        &[],
        &before_the_list(),
    );
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Backspace),
        &[],
        &before_the_list(),
    );
    assert_eq!(at(&console), Some(0), "`⌫` did not widen the list again");

    // `⏎` on the row that is already current is *yes, stay on this cluster*: the box closes.
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &[],
        &before_the_list(),
    );
    assert_eq!(
        console.app.modal, None,
        "`⏎` on the current row left the box open"
    );

    // A row that is not the live one hands the connection out to the caller.
    opened(&mut console);
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Down),
        &[],
        &before_the_list(),
    );
    let answered = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &[],
        &before_the_list(),
    );
    match answered {
        Did::Switch(asked) => {
            assert_eq!(asked.key, "staging");
            assert_eq!(asked.to.as_deref(), Some("staging"));
            assert_eq!(
                asked.before,
                views::Before::Connected(Some("prod-eu".to_owned())),
                "the way out did not name the cluster that was live"
            );
        }
        _ => panic!("`⏎` on a row that would connect reached nothing"),
    }

    // `esc` closes it.
    opened(&mut console);
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Esc),
        &[],
        &before_the_list(),
    );
    assert_eq!(console.app.modal, None, "`esc` did not close the picker");
}

/// **A delete's box asks for the name, and nothing arms it until the whole name is typed**
/// (invariant 2, `views::Dialog::armed`, `screens/dialogs.md` § Delete).
#[test]
fn a_typed_name_dialog_takes_the_name_and_arms_on_nothing_less() {
    let mut console = bare_console();
    let mut dialog = an_open_dialog();
    dialog.verb = DELETE;
    dialog.object =
        views::Object::new("pod", Some("payments".to_owned()), "web-1".to_owned(), None);
    dialog.kubectl = views::Stripped::of("kubectl delete pod/web-1 -n payments");
    // `delete` sends no check, so its verdict is `Some` from the moment the box opens (NOTES § D225
    // ruling 1) — what is left to wait for is the name.
    dialog.verdict = Some("k8rs did not check this one with the cluster first");
    dialog.asks = Some(views::Stripped::of("web-1"));
    installed(&mut console, Published::Opening(dialog));

    for character in "web-".chars() {
        let _ = pressed(
            &mut console,
            pressing(ratatui::crossterm::event::KeyCode::Char(character)),
            &[],
            &before_the_list(),
        );
    }
    let Some(views::Modal::Confirm(held)) = &console.app.modal else {
        panic!("the box went")
    };
    assert_eq!(held.typed.text(), "web-");
    assert!(!held.armed(), "half a name armed the button");
    let dead = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &[],
        &before_the_list(),
    );
    assert!(
        matches!(dead, Did::Nothing),
        "`⏎` confirmed a delete whose name was half typed"
    );

    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Char('1')),
        &[],
        &before_the_list(),
    );
    let Some(views::Modal::Confirm(held)) = &console.app.modal else {
        panic!("the box went")
    };
    assert!(held.armed(), "the whole name did not arm the button");
    let answered = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &[],
        &before_the_list(),
    );
    assert_eq!(
        answered_name(answered).as_deref(),
        Some("web-1"),
        "what the reader typed is not what the operation is answered with"
    );

    // `⌫` takes the name apart again on a box that is still open.
    let mut console = bare_console();
    let mut dialog = an_open_dialog();
    dialog.verdict = Some("k8rs did not check this one with the cluster first");
    dialog.asks = Some(views::Stripped::of("web-1"));
    installed(&mut console, Published::Opening(dialog));
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Char('w')),
        &[],
        &before_the_list(),
    );
    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Backspace),
        &[],
        &before_the_list(),
    );
    let Some(views::Modal::Confirm(held)) = &console.app.modal else {
        panic!("the box went")
    };
    assert_eq!(held.typed.text(), "");

    // And `esc` dismisses it, because its verdict has landed — a cancellation, not a confirmation.
    let dismissed = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Esc),
        &[],
        &before_the_list(),
    );
    assert!(
        matches!(dismissed, Did::Answered(Reply::No)),
        "`esc` did not cancel"
    );
    assert_eq!(console.app.modal, None);
}

/// What a confirmation answered with, or `None` for anything else.
fn answered_name(did: Did) -> Option<String> {
    match did {
        Did::Answered(Reply::Yes(typed)) => Some(typed),
        _ => None,
    }
}

/// **A listed store holding one real capture per kind the allowlist in [`vanished`] names** — all
/// five of invariant 6's watches, so the guard can be asked a question it is able to answer about
/// each of them, and every link of its snapshot chain has something in it.
///
/// **One capture per watch and not one for Deployments alone**, which is what let `snapshot.pods`
/// and `snapshot.nodes` be dropped from that chain with the suite still green (`tester`,
/// 2026-09-26): every case reaching it fed a Deployment, so two thirds of the chain was dead weight
/// to the tests while `ctrl-d` on a running pod depended on it.
fn a_listed_cluster() -> k8s::Store {
    let mut store = listed(objects::<Pod>("kube-system-pods.json"));
    for node in objects::<Node>("nodes.json") {
        store.node(&now(), Event::Apply(node));
    }
    for deployment in objects::<Deployment>("deployments.json") {
        store.deployment(&now(), Event::Apply(deployment));
    }
    for set in objects::<StatefulSet>("statefulsets.json") {
        store.stateful_set(&now(), Event::Apply(set));
    }
    for set in objects::<DaemonSet>("daemonsets.json") {
        store.daemon_set(&now(), Event::Apply(set));
    }
    store
}

/// **A listed store whose Deployment watch never answered** — every other watch listed, then
/// `k8s::Store::stop_waiting` settles the one that did not, which is what makes
/// `k8s::Store::snapshot` publish with an empty workload list while `k8s::Store::troubles`
/// names the kind (`k8s::Store::still_listing` § *A watch that is not coming back is not
/// listing*).
///
/// **The live shape is a watch the cluster refuses**, drawn in
/// `reports/2026-09-26-the-error-state-pass.md` § 1: cards and a header over an empty list. Nothing
/// here can set a `kube::Error` on a watch from outside `k8s.rs`, and `unfinished` reaches
/// `troubles` through the same filter a failure does.
fn a_deployment_watch_that_never_answered() -> k8s::Store {
    let mut store = k8s::Store::default();
    store.pod(&now(), Event::Init);
    store.pod(&now(), Event::InitDone);
    store.node(&now(), Event::Init);
    store.node(&now(), Event::InitDone);
    store.stateful_set(&now(), Event::Init);
    store.stateful_set(&now(), Event::InitDone);
    store.daemon_set(&now(), Event::Init);
    store.daemon_set(&now(), Event::InitDone);
    store.stop_waiting();
    store
}

/// **A listed store whose *Node* watch never answered, and which holds no Deployment** — so a
/// question about a Deployment has a true answer (*absent*) while `k8s::Store::troubles` names a
/// different kind entirely.
///
/// **This is the store a trouble predicate widened to `!store.troubles().is_empty()` gets wrong**
/// (`tester`, 2026-09-26): that spelling passes `==` → `!=` mutation and every case above, and
/// switches D22's guard off for all five kinds the moment one watch anywhere is in trouble.
fn a_node_watch_that_never_answered() -> k8s::Store {
    let mut store = k8s::Store::default();
    store.pod(&now(), Event::Init);
    store.pod(&now(), Event::InitDone);
    store.deployment(&now(), Event::Init);
    store.deployment(&now(), Event::InitDone);
    store.stateful_set(&now(), Event::Init);
    store.stateful_set(&now(), Event::InitDone);
    store.daemon_set(&now(), Event::Init);
    store.daemon_set(&now(), Event::InitDone);
    store.stop_waiting();
    store
}

/// **The object went away while the box was open, so the yes becomes a refusal and no command is
/// sent** (NOTES § D22, § D289 ruling 1; `screens/dialogs.md` § The object went away while the
/// dialog was open).
///
/// **This is `restart`'s only identity guard.** `ops::restart` is an `Api::patch` and `PatchParams`
/// carries no `preconditions` field (`ops::Mutation::uid_sent`), so nothing on the wire can refuse
/// the write — the failure `ops.rs` records as measured is a Deployment deleted and recreated
/// between the dry-run and the yes, leaving the audit line naming a `uid` nothing changed beside a
/// `PATCH` that landed on a different instance.
///
/// **Two groups, because a guard is proven only for the shapes it was fed** (NOTES § D29) and the
/// two groups feed different halves of it.
///
/// **The seven store shapes**: one where the object is *there*, two where it is *gone* — the
/// second of those a store whose trouble is on a **different** kind — and four where k8rs cannot
/// tell, none of which may read as gone: a ReplicaSet, which invariant 6 fetches on demand and
/// never watches; a selection carrying no `uid`; a store whose first LIST has not landed; and a
/// listed store whose own watch for the kind never answered.
///
/// **Then every word the allowlist carries, both ways** — and *both* is what makes each of them a
/// pin rather than a passenger (`tester`, 2026-09-26, three hand-planted mutations that stayed
/// green). A *held* uid pins that kind's link in [`vanished`]'s snapshot chain: drop `.pods` and a
/// held pod reads as gone. An *absent* uid pins the word in the `matches!` itself: take `"pod"` out
/// and the kind stops being watched, so the guard answers *false* — which a held-uid case expects
/// anyway and cannot see. Neither direction covers the other, and `ctrl-d` on a running pod is what
/// the first one costs.
///
/// **Every uid is read off the capture and none is written down** — a re-capture moves all of them,
/// and a literal would let a case pass by agreeing with itself. The lookup panics rather than
/// skipping a kind it cannot find one for (CLAUDE.md § *a derived list asserts it found
/// something*).
///
/// **What each case asserts is the reply and the strip together** (`screens/dialogs.md` rule 7: the
/// strip carries a mutation's line *"the instant the real call actually goes out"*). A `Gone`
/// decided here sends nothing at all, so a line appended anyway would annotate a command that never
/// left — invariant 4's *neither record may lie*.
#[test]
fn a_yes_on_an_object_that_went_away_is_answered_gone_and_sends_no_command() {
    let live = a_listed_cluster();
    let waiting = before_the_list();
    let refusing = a_deployment_watch_that_never_answered();
    let elsewhere = a_node_watch_that_never_answered();
    let snapshot = live
        .snapshot(now())
        .expect("the five captures' LISTs all landed");
    // **One object of each kind, found the way the guard itself maps a kind onto a word** —
    // `ui::addressed`, so this lookup cannot disagree with the allowlist about what `"daemonset"`
    // means. The panic is the *found something* assertion: a capture that stops holding a kind
    // must fail the test rather than quietly drop a row from the loop below.
    let held = |word: &str| {
        snapshot
            .pods
            .iter()
            .map(|pod| &pod.id)
            .chain(snapshot.nodes.iter().map(|node| &node.id))
            .chain(snapshot.workloads.iter().map(|workload| &workload.id))
            .find(|id| ui::addressed(&id.kind).1 == word)
            .unwrap_or_else(|| panic!("no capture holds a {word}, so its rows would prove nothing"))
            .clone()
    };
    // **A `uid` no capture can hold**, so *absent* is a fact about the store and not a spelling
    // accident.
    let absent = || Some("6f1f2a94-0000-0000-0000-000000000000".to_owned());
    let object = |word: &'static str, id: &ObjectId, uid: Option<String>| {
        views::Object::new(word, id.namespace.clone(), id.name.clone(), uid)
    };

    // **One press per shape, and the assertions are one closure** — the two groups below feed it
    // different things and neither may grow a second copy of what *the guard answered right* means.
    let check = |what: &str, asked: views::Object, store: &k8s::Store, gone: bool| {
        let mut console = bare_console();
        let mut dialog = an_open_dialog();
        dialog.object = asked;
        installed(&mut console, Published::Opening(dialog));
        installed(
            &mut console,
            Published::Checked {
                verdict: "the cluster checked it first and accepted it",
                asks: None,
            },
        );
        let answered = pressed(
            &mut console,
            pressing(ratatui::crossterm::event::KeyCode::Enter),
            &[],
            store,
        );
        assert_eq!(
            matches!(answered, Did::Answered(Reply::Gone)),
            gone,
            "{what}: the guard answered the wrong way"
        );
        assert!(
            !gone == matches!(answered, Did::Answered(Reply::Yes(_))),
            "{what}: a yes that is neither a confirmation nor a `Gone`"
        );
        // **The box closes either way and the object travels on `changing`** — `settled` reads it
        // from there to open the *Already gone* box (`views::App::changing`).
        assert_eq!(console.app.modal, None, "{what}: the box stayed open");
        assert!(
            console.app.changing.is_some(),
            "{what}: nothing carried the object across the moment the box closed"
        );
        // **`contains` and not equality, because `views::Log::sent` appends its own running
        // mark** — what is asserted is whether the command reached the strip at all.
        let strip: Vec<&str> = console
            .log
            .lines()
            .iter()
            .map(views::Stripped::as_str)
            .collect();
        assert_eq!(
            strip
                .iter()
                .any(|line| line.contains("rollout restart deployment/web")),
            !gone,
            "{what}: the strip and the call disagree about whether one went out: {strip:?}"
        );
    };

    // --- THE SEVEN STORE SHAPES ---
    let deployment = held("deployment");
    check(
        "the uid the store holds",
        object("deployment", &deployment, deployment.uid.clone()),
        &live,
        false,
    );
    check(
        "a uid the store does not hold",
        object("deployment", &deployment, absent()),
        &live,
        true,
    );
    check(
        "a ReplicaSet, which no watch answers for",
        object("replicaset", &deployment, absent()),
        &live,
        false,
    );
    check(
        "a selection carrying no uid at all",
        object("deployment", &deployment, None),
        &live,
        false,
    );
    check(
        "a store whose first LIST has not landed",
        object("deployment", &deployment, absent()),
        &waiting,
        false,
    );
    check(
        "a listed store whose Deployment watch never answered",
        object("deployment", &deployment, absent()),
        &refusing,
        false,
    );
    // **The trouble is on the Node watch and the question is about a Deployment**, whose own watch
    // listed and holds no such uid — so *gone* is the true answer and a predicate that asks merely
    // *is anything in trouble* gets it wrong (`a_node_watch_that_never_answered`).
    check(
        "a listed store where a different kind's watch never answered",
        object("deployment", &deployment, absent()),
        &elsewhere,
        true,
    );

    // --- EVERY WORD THE ALLOWLIST CARRIES, BOTH WAYS ---
    // Written out rather than read off `vanished`'s own `matches!`, which no test can reach: a word
    // added there and not here is a kind with a guard nothing proves, and that is the hole these
    // rows exist to keep shut.
    for word in ["pod", "node", "deployment", "statefulset", "daemonset"] {
        let id = held(word);
        check(
            &format!("a {word} uid the store holds"),
            object(word, &id, id.uid.clone()),
            &live,
            false,
        );
        check(
            &format!("a {word} uid the store does not hold"),
            object(word, &id, absent()),
            &live,
            true,
        );
    }
}

/// **`/` and `n` reach nothing while a detail pane is drawn over the list they narrow**
/// (`screens/widgets.md` § A filter is kept over anything drawn on top of the list it narrows:
/// *"Detail … has no `/ filter` or `n namespace` on its own closed footer, but none of them touch
/// `App::filters` to get there"*; NOTES § D289 ruling 4).
///
/// **Asserted in both states, because a guard that refuses everywhere passes the refusal half
/// on its own**: with the slot closed both keys take focus, with it open neither does, and a
/// filter already committed is still there afterwards.
///
/// **`screens/detail.md` § The logs tab does give `/` a text search over the pane, and no such
/// search exists** — refusing the key is today's correct behaviour, and building the search is its
/// own box.
#[test]
fn the_two_filter_keys_are_refused_while_a_detail_pane_is_over_the_list() {
    let cards = vec![a_group_of_two()];
    for (key, typing) in [
        (
            ratatui::crossterm::event::KeyCode::Char('/'),
            views::Typing::Text,
        ),
        (
            ratatui::crossterm::event::KeyCode::Char('n'),
            views::Typing::Namespace,
        ),
    ] {
        // **Closed: the key is on the footer and it takes focus** — the half that proves the guard
        // did not simply kill both keys.
        let mut console = bare_console();
        let did = pressed(&mut console, pressing(key), &cards, &before_the_list());
        assert!(
            matches!(did, Did::Changed),
            "{key:?} reached nothing with the list on screen"
        );
        assert_eq!(
            console.app.typing,
            Some(typing),
            "{key:?} did not open its own filter with the list on screen"
        );

        // **Open — the which-pods step and then the tabs, which are the two `Detailing`s that are
        // not `Closed`.** A filter committed before the pane opened is what the reader loses if the
        // key gets through: it is kept over the pane and cleared by nothing here.
        for opened in [
            Opened::Pods(cards[0].owner.clone()),
            Opened::Tabs {
                object: cards[0].owner.clone(),
                from_step: false,
            },
        ] {
            let mut console = bare_console();
            for character in "pay".chars() {
                console.app.filters.text.push(character);
            }
            console.opened = Some(opened);
            let did = pressed(&mut console, pressing(key), &cards, &before_the_list());
            assert!(
                matches!(did, Did::Nothing),
                "{key:?} acted from over the list it narrows"
            );
            assert_eq!(
                console.app.typing, None,
                "{key:?} took focus from over a detail pane"
            );
            assert_eq!(
                console.app.filters.text.text(),
                "pay",
                "{key:?} reached the filter that narrows the list the pane is drawn over"
            );
        }
    }
}

/// **A write on the wire refuses `X` and `q` from under an open box too** (NOTES § D12, § D16:
/// switching clusters mid-`PATCH`, or quitting out of one, leaves the audit log holding an attempt
/// with no result).
#[test]
fn a_write_on_the_wire_refuses_the_switcher_and_quitting_from_anywhere() {
    let mut console = bare_console();
    console.app.changing = Some(views::Object::new(
        "deployment",
        Some("payments".to_owned()),
        "web".to_owned(),
        None,
    ));
    assert!(
        matches!(
            pressed(
                &mut console,
                pressing(ratatui::crossterm::event::KeyCode::Char('X')),
                &[],
                &before_the_list(),
            ),
            Did::Nothing
        ),
        "`X` switched cluster with a write on the wire"
    );
    assert_eq!(console.app.modal, None);

    // The same, from under Help, whose own footer names `q`.
    console.app.modal = Some(views::Modal::Help);
    assert!(
        matches!(
            pressed(
                &mut console,
                pressing(ratatui::crossterm::event::KeyCode::Char('q')),
                &[],
                &before_the_list(),
            ),
            Did::Nothing
        ),
        "`q` quit out of a write that was still running"
    );
}

/// **`esc` dismisses either terminal box, and `⏎` belongs to the refusal box alone** — its footer
/// draws `esc dismiss  ⏎ open`, because the write it reports on is over and the object it was about
/// is still selected underneath; `Gone`'s draws the bare `esc dismiss`, because its object stopped
/// existing (`views::App::footer`'s two arms, `screens/dialogs.md` states 0 and 1c,
/// `screens/widgets.md` § A modal's footer).
///
/// **The `⏎` half is what this test was missing, and `⏎` is the only key either footer names**
/// (NOTES § D289 ruling 3). It looped `['q', '?', 'r', 'X']` — not one of them on a footer — and
/// its doc said the box *"offers only `esc dismiss`"*, which `views.rs` has never drawn:
/// Phase 12's behaviour written down as though it were the specification.
#[test]
fn a_terminal_box_closes_on_esc_and_only_the_refusal_opens_on_enter() {
    let cards = vec![a_group_of_two()];
    let refused = || views::Modal::Refused {
        sent: true,
        fault: k8s::Fault::Refused,
        said: Some("forbidden".to_owned()),
    };
    let gone = || views::Modal::Gone {
        object: views::Object::new(
            "deployment",
            Some("payments".to_owned()),
            "web".to_owned(),
            Some("d-1".to_owned()),
        ),
        recreated: false,
    };

    for (what, modal) in [("the refusal box", refused()), ("the gone box", gone())] {
        let mut console = bare_console();
        console.app.modal = Some(modal);
        // **Not one of these is on either footer** — a modal's footer is a closed, complete list
        // (`screens/widgets.md` § A modal's footer), so each has to reach nothing and leave the box
        // where it is.
        for ignored in ['q', '?', 'r', 'X'] {
            let did = pressed(
                &mut console,
                pressing(ratatui::crossterm::event::KeyCode::Char(ignored)),
                &cards,
                &before_the_list(),
            );
            assert!(
                matches!(did, Did::Nothing),
                "`{ignored}` acted from under {what}"
            );
            assert!(console.app.modal.is_some(), "`{ignored}` dismissed {what}");
        }
        let _ = pressed(
            &mut console,
            pressing(ratatui::crossterm::event::KeyCode::Esc),
            &cards,
            &before_the_list(),
        );
        assert_eq!(
            console.app.modal, None,
            "`esc dismiss` did not dismiss {what}"
        );
    }

    // **`⏎ open` opens the object the refusal was about** — a group of two opens the which-pods
    // step, which is exactly what `⏎` on that card does with no box in the way.
    let mut console = bare_console();
    console.app.modal = Some(refused());
    let opened = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &cards,
        &before_the_list(),
    );
    assert!(
        matches!(opened, Did::Changed),
        "`⏎ open` owed no frame, so the dismissed refusal box would stay on the screen"
    );
    assert_eq!(
        console.app.modal, None,
        "`⏎ open` left the refusal box on the screen"
    );
    match &console.opened {
        Some(Opened::Pods(owner)) => assert_eq!(*owner, cards[0].owner),
        _ => panic!("`⏎ open` did not open the object the refusal was about"),
    }

    // **And the same key reaches nothing under the gone box**, whose footer names no second key
    // because there is no object left to open.
    let mut console = bare_console();
    console.app.modal = Some(gone());
    let did = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &cards,
        &before_the_list(),
    );
    assert!(
        matches!(did, Did::Nothing),
        "`⏎` acted from under the gone box"
    );
    assert!(
        console.app.modal.is_some(),
        "`⏎` dismissed the gone box, whose footer names only `esc`"
    );
    assert!(
        console.opened.is_none(),
        "`⏎` opened an object from under the gone box"
    );
}

/// **`↑` / `↓` reach nothing on a view whose rows this wiring does not have** — the content cursor
/// is one index over whatever the open pane draws, and the browser's rows come with its own `Table`
/// (its own box).
#[test]
fn the_arrows_reach_nothing_on_a_view_this_wiring_cannot_draw_rows_for() {
    let cards = vec![a_group_of_two()];
    let mut console = bare_console();
    console.app.view = views::View::Resources(0);
    let anchors = [None, None];
    for arrow in [
        ratatui::crossterm::event::KeyCode::Down,
        ratatui::crossterm::event::KeyCode::Up,
    ] {
        assert!(
            matches!(
                pressed(&mut console, pressing(arrow), &cards, &before_the_list()),
                Did::Nothing
            ),
            "{arrow:?} moved a cursor over the Alerts list while the browser was open"
        );
    }
    assert_eq!(console.app.content.selected(&anchors), Some(0));
}

/// **The mutation is driven for real, headlessly** — `ops::restart` and `ops::delete` over a client
/// that refuses everything, so each operation's own `show` reaches the screen and the verb decides
/// which command the reader is taught (invariant 4).
///
/// **Nothing is confirmed**: the answer channel is closed, which `mutating` reads as a
/// cancellation, so no real call is ever sent (the same shape `tests/binary.rs` asserts for the
/// headless driver).
#[tokio::test]
async fn each_verb_shows_its_own_command_and_nothing_is_sent_without_an_answer() {
    let scratch = Scratch::named("console-mutating");
    let client = refusing().await;
    for (verb, expected) in [
        // **The fixture's own context is `prod-eu`**, which `mutating` is handed three lines
        // down — every taught command carries it now (NOTES § D278 ruling 5).
        (
            RESTART,
            "kubectl --context prod-eu rollout restart deployment/web",
        ),
        (DELETE, "kubectl --context prod-eu delete deployment/web"),
    ] {
        let mut audit = scratch.file(&format!("audit-{verb}"));
        let wanted = Wanted {
            verb,
            kind: "deployment",
            name: "web".to_owned(),
            namespace: Some("payments".to_owned()),
            uid: None,
        };
        let shown = std::cell::RefCell::new(Vec::new());
        let (answering, answered) = tokio::sync::mpsc::unbounded_channel();
        // Closed before the call, so `ask` reads a cancellation the moment it is reached.
        drop(answering);
        let performed = mutating(
            &client,
            "https://prod-eu.invalid:6443",
            "prod-eu",
            &wanted,
            &now(),
            &mut audit,
            &shown,
            answered,
        )
        .await;
        let published = shown.into_inner();
        let Some(Published::Opening(dialog)) = published.first() else {
            panic!("{verb} published no dialog: {} entries", published.len())
        };
        println!("{verb}: {}", dialog.kubectl.as_str());
        assert_eq!(dialog.verb, verb);
        assert!(
            dialog.kubectl.as_str().starts_with(expected),
            "{verb} taught the wrong command: {:?}",
            dialog.kubectl.as_str()
        );
        // **The title bar's own spelling** — `views::Object::name` is the bare name, because
        // `ui::name` joins it with the namespace; `ops::Shown::object` is the `kind/name` form the
        // `$` line uses, and putting that here drew `payments/deployment/web`.
        assert_eq!(dialog.object.name(), "web");
        assert_eq!(dialog.object.namespace(), Some("payments"));
        assert_eq!(dialog.object.kind, "deployment");
        // Nothing was confirmed, so nothing was sent — whatever the cluster would have said.
        match performed {
            Ok(performed) => assert!(
                !performed.changed(),
                "{verb} changed something nobody confirmed"
            ),
            Err(refusal) => panic!("{verb} was refused before it could ask: {refusal}"),
        }
    }
}

/// **A `Gone` on the answer channel reaches `ops` as `ops::Answer::Gone`, so nothing is sent
/// and the audit log says so** — the second half of D22's guard, and the half `over_modal`'s
/// own test cannot see (NOTES § D22, § D289 ruling 1). `main.rs` decides *is the uid still in
/// the store* and `mutating`'s `ask` closure is what turns that answer into the value
/// `ops::perform` acts on; a `Cancelled` there would look identical on screen and be a different
/// line in the record (invariant 4).
///
/// **`delete` is the verb, because it is the only one that sends no check** (NOTES § D225
/// ruling 1) — so `ask` is reached against a client that can reach nothing at all. A `restart` ends
/// as `Outcome::NotSent` on its own dry-run before the answer is ever read, which proves the
/// dry-run and not this.
#[tokio::test]
async fn a_gone_on_the_answer_channel_stops_the_write_inside_ops() {
    let scratch = Scratch::named("console-gone");
    let client = refusing().await;
    let mut audit = scratch.file("audit");
    let wanted = Wanted {
        verb: DELETE,
        kind: "deployment",
        name: "web".to_owned(),
        namespace: Some("payments".to_owned()),
        uid: Some("u-1".to_owned()),
    };
    let shown = std::cell::RefCell::new(Vec::new());
    let (answering, answered) = tokio::sync::mpsc::unbounded_channel();
    // Queued before the call, so `ask` reads it the moment it is reached — the receiver is alive
    // for as long as `mutating`'s future is.
    answering.send(Reply::Gone).expect("the channel is open");
    let performed = mutating(
        &client,
        "https://prod-eu.invalid:6443",
        "prod-eu",
        &wanted,
        &now(),
        &mut audit,
        &shown,
        answered,
    )
    .await;
    assert_eq!(
        performed,
        Ok(ops::Performed {
            outcome: Some(ops::Outcome::Gone),
            recorded: true,
        }),
        "a `Gone` answer did not stop the write inside `ops`"
    );
    // **The refusal is in the record too** — the security gate's *every attempt, success,
    // failure or refusal, reaches the audit log*. The words are `ops::Record::result_line`'s and
    // this only asserts that the line is the `Gone` one and names the object.
    let written =
        std::fs::read_to_string(scratch.0.join("audit")).expect("the audit log this run opened");
    println!("{written}");
    assert!(
        written.contains("deployment/web"),
        "the audit log does not name the object the guard refused:\n{written}"
    );
    assert!(
        written.lines().count() >= 2,
        "the attempt and its result are not both in the record:\n{written}"
    );
}

/// **What `k8s::text` appends where it cut** — retyped, because `k8s::SHORTENED` is private to that
/// file, and only ever read here for its length.
const SHORTENED_BY_K8RS: &str = "\u{2026} (shortened by k8rs)";

/// **The crafted name this box feeds the dialog path** — every one of [`CRAFTED_SHAPES`], so no row
/// of the loop below is a check nothing can reach, plus ten thousand characters on one line
/// (todo.md § Phase 12, NOTES § D283 ruling 5).
fn crafted_name() -> String {
    format!("web\u{1b}[31m\u{202e}\u{200b}{}", "z".repeat(10_000))
}

/// **Driven through `mutating` for real, `ops` refuses to address such a name and no box opens at
/// all** — which is not what the box predicted and is the stronger answer.
///
/// **So this is the first of two halves rather than the whole proof.** `ops::restart` and
/// `ops::delete` both check `k8s::object_name` *before* `perform` calls `show`, and that predicate
/// allows only ASCII alphanumerics, `-` and `.` up to `k8s::NAME_MAX` — so not one of the three
/// shapes can reach a `views::Dialog` by this route, and `main.rs`'s `settled` throws the sentence
/// away rather than drawing it. What proves the strip at the door the wiring actually uses is
/// [`a_crafted_name_the_wiring_hands_a_dialog_reaches_no_cell`].
#[tokio::test]
async fn a_crafted_name_is_refused_before_any_box_can_open_on_it() {
    let scratch = Scratch::named("console-crafted-refused");
    let client = refusing().await;
    let store = a_cluster_with_cards();
    // **One crafted carrier per row, and the other left clean** (NOTES § D31, § D284 ruling 4).
    // Both fields poisoned in one call proved neither: `object_name` and `namespace_name` each
    // refuse this value, so bypassing one left the test green on the other. The `which` string is
    // the half of `ops::unaddressable`'s sentence that says *which* word was refused.
    for (verb, name, namespace, which) in [
        (
            RESTART,
            crafted_name(),
            "payments".to_owned(),
            "an object's own name",
        ),
        (
            RESTART,
            "web".to_owned(),
            crafted_name(),
            "the name of a namespace",
        ),
        (
            DELETE,
            crafted_name(),
            "payments".to_owned(),
            "an object's own name",
        ),
        (
            DELETE,
            "web".to_owned(),
            crafted_name(),
            "the name of a namespace",
        ),
    ] {
        let what = format!("{verb} on {which}");
        let mut audit = scratch.file(&format!("audit-{verb}-{}", which.len()));
        let wanted = Wanted {
            verb,
            kind: "deployment",
            name,
            namespace: Some(namespace),
            uid: None,
        };
        let shown = std::cell::RefCell::new(Vec::new());
        let (answering, answered) = tokio::sync::mpsc::unbounded_channel();
        drop(answering);
        let performed = mutating(
            &client,
            "https://prod-eu.invalid:6443",
            "prod-eu",
            &wanted,
            &now(),
            &mut audit,
            &shown,
            answered,
        )
        .await;
        let refusal =
            performed.expect_err("a name Kubernetes would not accept was addressed anyway");
        println!("{what} refused: {refusal}");
        assert!(
            refusal.contains(which),
            "{what}: the refusal is about the other word: {refusal}"
        );
        assert!(
            shown.into_inner().is_empty(),
            "{what} published a dialog for a name it refused"
        );
        // **A box is opened first, so `modal.is_none()` has something to disprove** (ruling 4):
        // `settled` sets it to `None` as its second statement on every path, so the assertion was
        // unfalsifiable over a console that never had one.
        let mut console = bare_console();
        installed(&mut console, Published::Opening(an_armed_dialog()));
        // **And the precondition is asserted, which is ruling 4's own class one line earlier**:
        // with the `installed` above deleted, `modal.is_none()` below passes for ever.
        assert!(
            console.app.modal.is_some(),
            "{what}: the box this asserts `settled` closes never opened"
        );
        // **The waiting line is planted, and the product cannot reach this state** — `Log::sent`'s
        // one product caller is `over_modal`'s confirm arm, which never runs when `show` never
        // ran, so on this path `Log::outcome` writes nothing at all and the refusal is silent on
        // screen (backlog.md § From the dialog-strip box, a Phase 12 close blocker). What is
        // planted is the line `outcome` would annotate if there were one.
        console
            .log
            .sent("$ kubectl rollout restart deployment/web -n payments".to_owned());
        settled(&mut console, Err(refusal));
        assert!(console.app.modal.is_none(), "{what} left a box open");
        // The crafted name never entered the console at all — `settled` discards the sentence —
        // so this frame is that discard rather than a strip.
        unhostile(&what, &framed(&mut console, &store), "broken-");
    }
}

/// **The crafted name handed to a dialog through the calls `mutating`'s own `show` makes, and no
/// cell of the frame holds one of its shapes** — invariant 9, NOTES § D283 rulings 1 and 3.
///
/// **Not a hand-built `views::Dialog`**: every hostile value goes through the wiring's own call
/// over it — `views::Object::new` for the name and the namespace (ruling 3), `Stripped::of` for
/// the consequence and the command (ruling 1), and [`installed`]'s `Stripped::of` for `asks`, the
/// one field no constructor could have covered because it is assigned after the box is open. What
/// this route skips, and why it has to, is
/// [`a_crafted_name_is_refused_before_any_box_can_open_on_it`].
///
/// **Both shapes of box, because `asks` is only a typed-name one's** — a press-only box leaves the
/// field `None` and never spends the one strip `installed` owns.
#[test]
fn a_crafted_name_the_wiring_hands_a_dialog_reaches_no_cell() {
    let store = a_cluster_with_cards();
    let crafted = crafted_name();
    for asks in [None, Some(crafted.clone())] {
        let typed_name = asks.is_some();
        let what = if typed_name {
            "a typed-name box"
        } else {
            "a press-only box"
        };
        let mut dialog = an_open_dialog();
        dialog.object = views::Object::new(
            "deployment",
            Some(crafted.clone()),
            crafted.clone(),
            Some("u-1".to_owned()),
        );
        dialog.consequence = views::Stripped::of(&crafted);
        dialog.kubectl = views::Stripped::of(&crafted);
        // **`warning` is fed one even though the console cannot build one** — `mutating`'s `show`
        // sets it `None` and nothing downstream can (backlog.md § From the dialog-strip box, a
        // Phase 12 close blocker).
        //
        // **What earns it its place is not a third copy of the type's proof, it is the row budget's
        // warning arm** (`tester`, 2026-09-26): an oversized hostile warning is the only fixture in
        // the tree that reaches `ui::confirm`'s `marked(warning, columns, kept)` at all, and the
        // printed frame below shows the documented give-way order at the extreme — the consequence
        // down to its one row while the warning keeps nine. With `None` that arm is skipped
        // entirely, in the function NOTES § D284 ruling 3 just edited.
        dialog.warning = Some(views::Stripped::of(&crafted));
        let mut console = bare_console();
        installed(&mut console, Published::Opening(dialog));
        installed(
            &mut console,
            Published::Checked {
                verdict: "k8rs did not check this one with the cluster first",
                asks: asks.clone(),
            },
        );
        let Some(views::Modal::Confirm(dialog)) = &console.app.modal else {
            panic!("{what} did not stay open")
        };
        assert_eq!(
            dialog.asks.is_some(),
            typed_name,
            "{what}: installed dropped the name the box asks for"
        );

        // **Every carrier the box has, named, with what each is bounded to and whether it is
        // *expected* to hold the name at all** — the claim is *no unprintable character survived*
        // and *it is bounded*, and a failure has to say which field let one past.
        //
        // **The `carries` column is NOTES § D284 ruling 4**: an absence assertion passes on an
        // empty value, so a door that silently dropped the name passed this loop whole. Each row
        // that is handed the name asserts it is still there, in its printable half.
        //
        // **`warning` is the sixth carrier** — a field the console cannot fill today, fed above for
        // the row budget's sake, so this row is about the field and not about an empty `Option`.
        for (which, value, cap, carries) in [
            (
                "consequence",
                dialog.consequence.as_str(),
                k8s::FREE_TEXT,
                true,
            ),
            (
                "warning",
                dialog.warning.as_ref().map_or("", views::Stripped::as_str),
                k8s::FREE_TEXT,
                true,
            ),
            ("kubectl", dialog.kubectl.as_str(), k8s::FREE_TEXT, true),
            // **`k8s::FREE_TEXT` and not the 512 a name really carries** — the bound the *type*
            // spends, the 512 being `ops::Record::of`'s and one file down (NOTES § D283 ruling 2).
            // This route hands `installed` the raw name, which is exactly the caller `ops.rs` is
            // not in the room for.
            (
                "asks",
                dialog.asks.as_ref().map_or("", views::Stripped::as_str),
                k8s::FREE_TEXT,
                typed_name,
            ),
            ("object.name", dialog.object.name(), k8s::IDENTIFIER, true),
            (
                "object.namespace",
                dialog.object.namespace().unwrap_or(""),
                k8s::IDENTIFIER,
                true,
            ),
        ] {
            for shape in CRAFTED_SHAPES {
                assert!(
                    !value.contains(shape),
                    "{what}: views::Dialog::{which} kept {shape:?}"
                );
            }
            // **The security gate's *sizes are bounded* row** — the marked cut `k8s::text` appends
            // is what puts a shortened value a little over its own cap.
            assert!(
                value.len() <= cap + SHORTENED_BY_K8RS.len(),
                "{what}: views::Dialog::{which} is {} bytes, past the {cap} it is bounded to",
                value.len()
            );
            assert_eq!(
                value.contains("zzz"),
                carries,
                "{what}: views::Dialog::{which} does not hold the name this route gave it, so \
                 every absence above passes on nothing"
            );
        }

        // **The canary is the box's own last row**: the buttons are what a box that outgrew the
        // body loses first, so a frame without them is not one this may call clean.
        let frame = framed(&mut console, &store);
        unhostile(what, &frame, "[ esc cancel ]");
        // **And the printable half of the same name is on it** — without this the frame says
        // nothing about *this* string: ratatui writes no cell for a zero-width grapheme either way
        // ([`unhostile`]), so the absence above has to sit beside a presence.
        assert!(
            frame.contains("zzz"),
            "{what}: nothing of the crafted name was drawn, so its absence proves nothing:\n{frame}"
        );
        // Printed under `--nocapture`, because *the frame stayed 80×24* is a claim a reader of the
        // report should be able to see rather than take on the assertions above.
        println!("{what}:\n{frame}");
    }
}

/// **A mutating key is refused wherever the footer that was drawn does not offer it** — invariant
/// 2's *unreachable, not merely unbound*, and the four facts a kind cannot see (`ui::offered`'s
/// five; `k8s-admin`, 2026-09-24, `reports/2026-09-24-the-console-event-loop.md` § 2).
///
/// **Each row is drawn and then pressed**, because the value the key answers is the value the frame
/// produced ([`Console::offer`]) — asserting the press alone would be asserting the router against
/// itself.
#[test]
fn a_mutating_key_is_refused_wherever_the_drawn_footer_withholds_it() {
    let store = a_cluster_with_cards();
    let unaudited = String::from("k8rs could not open its audit log");

    // 1. The ordinary frame offers them, which is what makes the four rows below mean something.
    let mut console = bare_console();
    let _ = framed(&mut console, &store);
    assert!(
        matches!(console.offer, views::Offer::Act { .. }),
        "the baseline frame offered no mutating key at all: {:?}",
        console.offer
    );
    assert!(matches!(
        keyed(
            &mut console,
            control(ratatui::crossterm::event::KeyCode::Char('d')),
            &store
        ),
        Did::Mutate(_)
    ));

    // 2. A detail slot open — the tabs' own footer names neither key (PRIOR-ART § G2).
    let mut console = bare_console();
    console.opened = Some(Opened::Tabs {
        object: ObjectId {
            kind: ObjectKind::Pod,
            namespace: Some("default".to_owned()),
            name: "broken-crashloop".to_owned(),
            uid: None,
        },
        from_step: false,
    });
    let _ = framed(&mut console, &store);
    assert_eq!(console.offer, views::Offer::Move { switch: false });

    // 3. Writes are dead for the whole run — `ops::audit_log` could not open the log (NOTES § D21).
    let mut dead = bare_console();
    dead.writes = ui::Writes::Unaudited(&unaudited);
    let _ = framed(&mut dead, &store);
    assert!(
        !matches!(dead.offer, views::Offer::Act { .. }),
        "a run that cannot record a write still offered one: {:?}",
        dead.offer
    );
    for pressed in [
        typed('r'),
        control(ratatui::crossterm::event::KeyCode::Char('d')),
    ] {
        assert!(
            matches!(keyed(&mut dead, pressed.clone(), &store), Did::Nothing),
            "{pressed:?} started a mutation on a run with no audit log"
        );
    }

    // 4. The clock cannot be trusted — an age that reads fresher than it is decides which card
    // somebody reacts to first (`screens/states.md` § Your computer's clock is off).
    let mut skewed = bare_console();
    skewed.clock = Some("⚠ this computer's clock is 20 minutes behind the cluster's".to_owned());
    let _ = framed(&mut skewed, &store);
    assert!(
        !matches!(skewed.offer, views::Offer::Act { .. }),
        "a frame nobody can trust the times on still offered a write: {:?}",
        skewed.offer
    );
    assert!(matches!(
        keyed(&mut skewed, typed('r'), &store),
        Did::Nothing
    ));
}

/// **`ctrl` held with anything the map does not bind is not the plain letter** — `ctrl-d` and
/// `ctrl-c` are the two it has (`screens/help.md`), and the arms below them read `key.code` alone
/// (`k8s-admin`, 2026-09-24, `reports/2026-09-24-the-console-event-loop.md` § 1).
#[test]
fn a_control_combination_the_map_does_not_have_reaches_nothing() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let _ = framed(&mut console, &store);
    for held in ['r', 'l', 'y', 'n', 'f', 'j', 'k', 'q', 'X', '?', '/'] {
        let before = console.app.clone();
        let did = keyed(
            &mut console,
            control(ratatui::crossterm::event::KeyCode::Char(held)),
            &store,
        );
        assert!(
            matches!(did, Did::Nothing),
            "`ctrl-{held}` reached an arm bound to the plain letter"
        );
        assert_eq!(
            console.app, before,
            "`ctrl-{held}` changed the screen's state"
        );
        assert!(console.opened.is_none(), "`ctrl-{held}` opened a detail");
    }
}

/// **The command log's own line ends on a short word and never on a whole sentence** —
/// `views::SAID` is 32 bytes and [`outcome_word`] is what fits in it; `ops::Performed::plainly`
/// measured 56–58 columns of the strip's 76, which left the command 18
/// (`reports/2026-09-24-the-console-event-loop.md` § 3).
#[test]
fn every_ending_ends_the_log_line_on_a_word_the_strip_has_room_for() {
    let endings = [
        None,
        Some(ops::Outcome::Done),
        Some(ops::Outcome::Started),
        Some(ops::Outcome::Cancelled),
        Some(ops::Outcome::Gone),
        Some(ops::Outcome::Changed),
        Some(ops::Outcome::NotSent {
            fault: k8s::Fault::Unanswered,
            said: None,
        }),
        Some(ops::Outcome::Failed {
            fault: k8s::Fault::Refused,
            said: Some("forbidden".to_owned()),
        }),
        // **A `Failed` is three words and not one** (NOTES § D285 ruling 2), and `login expired`
        // is thirteen columns — joint-widest with `changed first`, and the word `views::SAID`'s
        // thirteen is measured off, so it is the one that has to be fed to this.
        Some(ops::Outcome::Failed {
            fault: k8s::Fault::Expired,
            said: None,
        }),
        Some(ops::Outcome::Failed {
            fault: k8s::Fault::Conflict,
            said: Some("the object changed".to_owned()),
        }),
    ];
    for outcome in &endings {
        let word = outcome_word(outcome.as_ref());
        println!("{outcome:?} → {word}");
        assert!(!word.is_empty(), "{outcome:?} has no word");
        // **Thirteen is the longest any mockup draws** (`views::SAID`'s own doc, `login expired`).
        assert!(
            word.chars().count() <= 13,
            "{word:?} is longer than the longest word any mockup draws"
        );
        assert!(
            !word.contains('\n') && word.trim() == word,
            "{word:?} is not one word-shaped fragment"
        );
    }
    // Ten endings, ten words, and no two endings share one — the line says *which* ending.
    let words: std::collections::BTreeSet<&str> =
        endings.iter().map(|o| outcome_word(o.as_ref())).collect();
    assert_eq!(
        words.len(),
        endings.len(),
        "two endings read the same: {words:?}"
    );
}

/// **A refusal that says *rejected* sends the reader somewhere else than the box beside it**
/// (NOTES § D285 ruling 2, invariant 4's *neither record may lie*). [`outcome_word`] mapped every
/// `ops::Outcome::Failed` to `rejected` whatever the fault, so two of the four short forms
/// `views::Log::outcome`'s own doc names — `refused` and `login expired` — could not be produced by
/// any journey: a real `403` on a delete and a real `409` both printed `→ rejected`
/// (`reports/2026-09-26-the-error-state-pass.md`).
///
/// **`Conflict` staying `rejected` is the ruling and not an omission**: `views.rs` defines the
/// vocabulary, and widening it is a screen ruling first.
#[test]
fn the_log_line_says_which_refusal_the_cluster_gave() {
    // Every fault a `Failed` can carry, and the word its line ends on. The three kubeconfig faults
    // cannot reach a call that was sent, and are here because the vocabulary is decided by the
    // fault and not by which of them a caller can produce.
    let words = [
        (k8s::Fault::Refused, "refused"),
        (k8s::Fault::Expired, "login expired"),
        (k8s::Fault::NoCredential, "login expired"),
        (k8s::Fault::Conflict, "rejected"),
        (k8s::Fault::Rejected, "rejected"),
        (k8s::Fault::Gone, "rejected"),
        (k8s::Fault::Unanswered, "rejected"),
        (k8s::Fault::Unfinished, "rejected"),
        (k8s::Fault::Kubeconfig, "rejected"),
        (k8s::Fault::NoContext, "rejected"),
        (k8s::Fault::BadEntry, "rejected"),
    ];
    for (fault, expected) in words {
        let failed = ops::Outcome::Failed {
            fault,
            said: Some("whatever the server said".to_owned()),
        };
        assert_eq!(
            outcome_word(Some(&failed)),
            expected,
            "{fault:?} on the strip sends the reader somewhere the box does not"
        );
        // **The word is read off the fault and never off the sentence beside it** — `said` is the
        // server's own words and is the refusal box's, bounded there (`views::SAID`).
        assert_eq!(
            outcome_word(Some(&ops::Outcome::Failed { fault, said: None })),
            expected,
            "{fault:?} read its word off the server's sentence"
        );
    }

    // **And the fault does not leak into the arm beside it**: nothing was sent at all under a
    // `NotSent`, which is a different event and keeps its own word whatever the fault.
    for (fault, _) in words {
        assert_eq!(
            outcome_word(Some(&ops::Outcome::NotSent { fault, said: None })),
            "not sent",
            "{fault:?} on a call that was never sent read as one the cluster answered"
        );
    }
}

/// **What the two unwired panes actually draw, read off the cells** — the browser and each of the
/// four detail tabs (`reports/2026-09-24-the-console-event-loop.md` § 4 and § 7 items 1 and 2,
/// which asked for exactly this and could not run it).
///
/// **Both say *still reading* and neither says *there is nothing*** (PRIOR-ART § C2): the fetches
/// behind them are their own box, and until it lands the honest frame is the one that has not
/// answered — not an empty pane claiming an answer.
#[test]
fn the_panes_nothing_fetches_yet_say_they_are_still_reading() {
    let store = a_cluster_with_cards();

    let mut browsing = bare_console();
    browsing.kinds = vec![k8s::Browsable {
        group: "apps".to_owned(),
        version: "v1".to_owned(),
        kind: "Deployment".to_owned(),
        plural: "deployments".to_owned(),
        namespaced: true,
        verbs: vec!["list".to_owned()],
    }];
    browsing.app.view = views::View::Resources(0);
    let drawn = framed(&mut browsing, &store);
    println!("{drawn}");
    assert!(drawn.contains("ALERTS"), "not a console frame:\n{drawn}");
    assert!(
        !drawn.contains("nothing is broken"),
        "the browser's own empty pane claimed the cluster is healthy:\n{drawn}"
    );
    assert!(
        !drawn.contains("in trouble right now"),
        "the browser drew the Alerts pane's own health sentence:\n{drawn}"
    );

    // Each of the four tabs, over an object with no read behind it yet.
    for tab in views::Tab::ALL {
        let mut open = bare_console();
        open.app.tab = tab;
        open.opened = Some(Opened::Tabs {
            object: ObjectId {
                kind: ObjectKind::Pod,
                namespace: Some("default".to_owned()),
                name: "broken-crashloop".to_owned(),
                uid: None,
            },
            from_step: false,
        });
        let drawn = framed(&mut open, &store);
        println!("=== {} ===\n{drawn}", tab.label());
        assert!(
            drawn.contains("broken-crashloop"),
            "{} drew no object heading:\n{drawn}",
            tab.label()
        );
        assert!(
            drawn.contains("reading the cluster…"),
            "{} did not say it was still reading:\n{drawn}",
            tab.label()
        );
        assert!(
            !drawn.contains("none right now") && !drawn.contains("no logs yet"),
            "{} turned a read that has not answered into an empty answer:\n{drawn}",
            tab.label()
        );
        // **And it says nothing about the *cluster's* health** — `Screen::note` is the Alerts
        // pane's and `ui::note` draws it from every caller, so a non-empty one lands in here too.
        assert!(
            !drawn.contains("in trouble right now"),
            "{} drew the Alerts pane's own health sentence:\n{drawn}",
            tab.label()
        );
        // The tab row marks the open one, so the frame and `App::tab` cannot come apart.
        assert!(
            drawn.contains(&format!("‹ {} ›", tab.label())),
            "{} is not the marked tab:\n{drawn}",
            tab.label()
        );
    }
}

/// **The strip's own row for every ending, read off the cells at the 80-column floor** —
/// `reports/2026-09-24-the-console-event-loop.md` § 7 item 3, which asked for the drawn row and not
/// the composed `String`.
///
/// **The object is what the line is about, so it is what may never give way** (NOTES § D266,
/// `screens/widgets.md` § 7): with `ops::Performed::plainly`'s whole sentence in the outcome slot
/// the command was cut to 18 columns and `deployment/web` went with it. [`outcome_word`] is the fix
/// and this is the measurement that holds it.
#[test]
fn the_strip_keeps_the_object_the_line_is_about_for_every_ending() {
    // **A store with nothing to say about this dialog's object, and it is the fixture rather than
    // a convenience.** [`over_modal`]'s confirm arm now asks the store whether the selected object
    // is still there ([`vanished`], NOTES § D22, § D289 ruling 1), and
    // [`a_cluster_with_cards`] is a healthy listed store holding four pods and **no Deployment** —
    // so it answers *gone* about [`an_open_dialog`]'s `payments/web`, correctly, about a console
    // that cannot exist: a card filed under a Deployment owner needs that Deployment on its own
    // watch. This test is about the box's key map and not about a cluster, so the store it presses
    // against is one that has not listed. The guard's own five shapes are
    // [`a_yes_on_an_object_that_went_away_is_answered_gone_and_sends_no_command`].
    let store = before_the_list();
    let endings = [
        None,
        Some(ops::Outcome::Done),
        Some(ops::Outcome::Started),
        Some(ops::Outcome::Cancelled),
        Some(ops::Outcome::Gone),
        Some(ops::Outcome::Changed),
        Some(ops::Outcome::NotSent {
            fault: k8s::Fault::Unanswered,
            said: None,
        }),
        Some(ops::Outcome::Failed {
            fault: k8s::Fault::Refused,
            said: Some("forbidden".to_owned()),
        }),
        // **`login expired` is thirteen columns, joint-widest with `changed first`** — measured
        // here rather than reasoned equal to it, because what this test is about is the row a
        // widest word draws (NOTES § D285 ruling 2, `views::SAID`).
        Some(ops::Outcome::Failed {
            fault: k8s::Fault::Expired,
            said: None,
        }),
    ];
    for outcome in endings {
        let word = outcome_word(outcome.as_ref());
        let mut console = bare_console();
        let mut dialog = an_armed_dialog();
        // The command `screens/dialogs.md` § The command log's own line measures its cut against.
        dialog.kubectl = views::Stripped::of(
            "kubectl rollout restart deployment/payments-api -n payments-production-eu",
        );
        installed(&mut console, Published::Opening(dialog.clone()));
        // **Through the yes, because that is what puts the line on the strip** (rule 7).
        let answered = keyed(
            &mut console,
            key(ratatui::crossterm::event::KeyCode::Enter),
            &store,
        );
        assert!(matches!(answered, Did::Answered(Reply::Yes(_))));
        settled(
            &mut console,
            Ok(ops::Performed {
                outcome,
                recorded: true,
            }),
        );
        let drawn = framed(&mut console, &store);
        let row = drawn
            .lines()
            .find(|line| line.contains("rollout restart"))
            .unwrap_or_else(|| panic!("{word}: the strip drew no command row:\n{drawn}"));
        println!("{word}:\n{row}");
        assert!(
            row.contains("deployment/payments-api"),
            "{word}: the object the line is about was cut away:\n{row}"
        );
        assert!(
            row.contains(word),
            "{word}: the outcome word did not reach the strip:\n{row}"
        );
        assert!(
            !row.contains("shortened by k8rs"),
            "{word}: the outcome slot overflowed and k8rs said so on the line:\n{row}"
        );
    }
}
/// **A frame owed soon is never pushed out by one owed later** — [`Owing::owed`]'s `min`, which is
/// the single line that makes this a throttle and not a debounce (PRIOR-ART § A5), and the single
/// line behind [`Owing::now`]'s own clause: *"A keypress made to wait out a coalescing window is a
/// tool that feels broken."*
///
/// **Neither half is reachable from a burst fed at one instant**: every event of such a burst
/// computes the same deadline, so `min` and `max` return the same value and
/// [`a_storm_that_goes_quiet_ends_on_the_last_event_and_costs_one_frame`] cannot tell them apart —
/// measured, `already.max(at)` passed all 1512 tests (`tester`, 2026-09-23).
#[tokio::test(start_paused = true)]
async fn a_frame_owed_soon_is_never_pushed_out_by_one_owed_later() {
    let mut owing = Owing::default();
    owing.changed();
    let burst = owing.0.expect("a watch event owes a frame");

    // A second event of the same storm does not move the deadline the first one set.
    tokio::time::advance(COALESCE / 4).await;
    owing.changed();
    assert_eq!(
        owing.0,
        Some(burst),
        "a later event pushed the deadline out: a storm would then hold the screen for as long as \
         it lasts, which is the unbounded staleness PRIOR-ART § A5 is about"
    );

    // And a key does not wait for it.
    owing.now();
    let pressed = owing.0.expect("a key owes a frame");
    assert!(
        pressed < burst,
        "the key was made to wait out the coalescing window"
    );
    assert!(
        pressed <= tokio::time::Instant::now(),
        "the key's frame is owed in the future rather than at once"
    );
}

/// **A storm that keeps coming is still redrawn inside the window** — the *throttle*, as opposed to
/// the debounce, and the only assertion that reads the **bound** directly (PRIOR-ART § A5,
/// invariant 7).
///
/// [`a_storm_that_goes_quiet_ends_on_the_last_event_and_costs_one_frame`] feeds its whole burst at
/// one instant, so it measures *the last event arrives* and *the burst is one frame* — both of
/// which a debounce also satisfies once the storm stops. What a debounce breaks is the bound: a
/// deadline pushed out by each new event never fires while the storm lasts, so the screen sits on
/// data of unbounded age. This feeds the same events 20 ms apart and asserts a frame landed
/// *during* the storm, which is the clause [`COALESCE`]'s own doc makes: *"the frame lands at most
/// this far behind."*
#[tokio::test(start_paused = true)]
async fn a_storm_that_keeps_coming_is_still_redrawn_inside_the_window() {
    use futures_util::stream::StreamExt;
    const GAP: std::time::Duration = std::time::Duration::from_millis(20);
    let pods = objects::<Pod>("kube-system-pods.json");
    let counted = pods.len();
    let spans = GAP * u32::try_from(counted).expect("the capture is small");
    // **Derived, not chosen**: the still-loading frame `pump` owes before anything has happened at
    // all, plus one per whole [`COALESCE`] window the storm spans.
    let owed =
        1 + usize::try_from(spans.as_millis() / COALESCE.as_millis()).expect("the storm is short");
    assert!(
        owed >= 3,
        "the storm spans under two windows and would prove nothing"
    );

    let feed = futures_util::stream::iter(pods).then(|pod| async move {
        tokio::time::sleep(GAP).await;
        let update: k8s::Update = Box::new(move |store: &mut k8s::Store| {
            store.pod(&now(), Event::InitApply(pod));
        });
        update
    });
    let opening: k8s::Update = Box::new(|store: &mut k8s::Store| {
        store.pod(&now(), Event::Init);
    });
    let mut updates = Updates {
        merged: futures_util::stream::select_all(vec![
            futures_util::stream::iter(vec![opening])
                .chain(feed)
                .boxed(),
        ]),
        drained: false,
        asked: std::collections::BTreeSet::new(),
        asking: tokio::sync::mpsc::unbounded_channel().0,
    };
    let (pressing, mut keys) = tokio::sync::mpsc::unbounded_channel();
    // Long after the storm, for the reason the quiet-storm test sends it late: a key draws at once.
    tokio::spawn(async move {
        tokio::time::sleep(spans * 4).await;
        let _ = pressing.send(Woke::Key(ratatui::crossterm::event::Event::Key(
            ratatui::crossterm::event::KeyEvent::new(
                ratatui::crossterm::event::KeyCode::Char('q'),
                ratatui::crossterm::event::KeyModifiers::NONE,
            ),
        )));
    });

    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))
        .expect("a terminal over a test backend");
    let mut console = bare_console();
    let mut store = k8s::Store::default();
    let at = Watching {
        renewal: None,
        coverage: k8s::Coverage::Cluster,
    };
    let halt = pump(
        &mut console,
        &mut terminal,
        &mut store,
        &mut updates,
        &mut keys,
        &at,
        None,
    )
    .await;
    assert!(matches!(halt, Halt::Quit), "the loop did not end on `q`");

    let drawn: Vec<String> = (0..terminal.backend().buffer().area.height)
        .map(|row| {
            (0..terminal.backend().buffer().area.width)
                .map(|column| terminal.backend().buffer()[(column, row)].symbol())
                .collect()
        })
        .collect();
    let screen = drawn.join("\n");
    // The canary: a console frame and not an empty buffer that would pass everything below.
    assert!(screen.contains("ALERTS"), "not a console frame:\n{screen}");
    assert!(
        screen.contains(&format!("{counted} pods")),
        "the last event of the burst never reached the screen:\n{screen}"
    );
    let frames = terminal.get_frame().count();
    assert!(
        frames >= owed,
        "a {}ms storm drew {frames} frame(s) and owed {owed}: the screen was held for the whole \
         storm instead of landing at most one window behind it",
        spans.as_millis()
    );
}

/// **A crafted object name reaches the cells and the frame still holds** — invariant 9 and
/// `screens/widgets.md` § 7, on the path that actually draws.
///
/// **The names are planted *past* the ingest strip on purpose.** Every string a cluster sends meets
/// `k8s::text` behind `k8s::ingest`'s single door, so a card built here with an escape in it is a
/// stronger input than anything a real watch can deliver — which is the point: if the renderer ever
/// became the thing that had to strip, this is the test that says so. What is asserted is the
/// property the security gate names — *a 50MB annotation or an endless log line must not blow up
/// the renderer* — not a particular cut.
///
/// **Three framings, because one is not the class** (NOTES § D31): the escape as the whole name,
/// the escape inside a name, and a single line far past any pane's width.
#[test]
fn a_crafted_object_name_cannot_rewrite_the_terminal_or_break_the_frame() {
    let store = a_cluster_with_cards();
    for (what, name) in [
        ("an escape alone", "\u{1b}[2J\u{1b}[H".to_owned()),
        (
            "an escape inside a name",
            "web\u{1b}[31m-\u{202e}drowssap\u{200b}1".to_owned(),
        ),
        ("ten thousand columns on one line", "w".repeat(10_000)),
    ] {
        let mut card = a_group_of_two();
        card.owner.name = name.clone();
        for finding in &mut card.findings {
            finding.owner.name = name.clone();
            finding.title = format!("{name} exceeded its memory limit");
            finding.evidence = name.clone();
        }
        let mut console = bare_console();
        console.opened = Some(Opened::Tabs {
            object: card.pods()[0].clone(),
            from_step: false,
        });
        // The frame is built the way the loop builds it, over a store that has answered — the cards
        // it derives are the store's, and this hostile one is drawn through the detail slot's own
        // heading and pinned block instead.
        console.log.ran(format!("$ kubectl get pod {name}"));
        let drawn = framed(&mut console, &store);

        unhostile(what, &drawn, "ALERTS");
    }
}

/// **The characters invariant 9 is about, as a test may plant them** — a real ANSI escape, a
/// right-to-left override and a zero-width space (`k8s::unprintable`, NOTES § D154).
const CRAFTED_SHAPES: [char; 3] = ['\u{1b}', '\u{202e}', '\u{200b}'];

/// **A frame that drew, at the size the product is drawn to, with none of [`CRAFTED_SHAPES`] in a
/// cell** — the security gate's *a 50MB annotation or an endless log line must not blow up the
/// renderer*, asserted as a property rather than as a particular cut.
///
/// **The canary is the caller's and is not optional**: every assertion here but the last passes on
/// an empty buffer, so what proves the frame is the one under test is a string only it draws.
///
/// **The row and cell counts are the half that can fail; the shapes are ratatui's** — measured
/// 2026-09-26 by removing `views::Object::new`'s strip and running the dialog caller below: the
/// escape and the override still reached no cell, because ratatui skips a zero-width grapheme
/// instead of writing one. So a caller proves its *own* strip at the field and reads this for *the
/// renderer did not blow up* — the canary going missing is a box that outgrew the body and lost its
/// last row.
fn unhostile(what: &str, drawn: &str, canary: &str) {
    let rows: Vec<&str> = drawn.lines().collect();
    assert_eq!(rows.len(), 24, "{what}: the frame is not 24 rows");
    for (at, row) in rows.iter().enumerate() {
        assert_eq!(
            row.chars().count(),
            80,
            "{what}: row {at} is not 80 cells: {row:?}"
        );
    }
    for shape in CRAFTED_SHAPES {
        assert!(
            !drawn.contains(shape),
            "{what}: {shape:?} reached the cells:\n{drawn}"
        );
    }
    assert!(
        drawn.contains(canary),
        "{what}: {canary:?} is not on it, so this is not the frame the assertions above were \
         about:\n{drawn}"
    );
}

/// **What the console says instead of drawing, and in which order** ([`before_the_first_frame`]) —
/// the decision `console` used to hold as three `return`s among the calls that produce them, which
/// no test could reach (`tester`, 2026-09-24).
#[tokio::test]
async fn nothing_drawn_yet_answers_the_connection_first_and_the_clock_costs_only_a_sentence() {
    // A kubeconfig that will not load and a client that will not build are one sentence.
    let refused =
        k8s::NotConnected::Kubeconfig(kube::config::KubeconfigError::CurrentContextNotSet);
    let said = before_the_first_frame(Err(&refused), Some(&now()))
        .expect("a connection that never happened ends the run");
    assert!(said.starts_with("k8rs: no cluster to watch — "), "{said:?}");
    // And it does not need a clock to say it.
    assert_eq!(
        before_the_first_frame(Err(&refused), None).as_deref(),
        Some(said.as_str()),
        "a machine with no readable clock lost the reason it could not connect"
    );

    // A session that answered: nothing here ends the run, so the console goes on to draw.
    let session = k8s::session(refusing().await, k8s::Coverage::Cluster).await;
    assert_eq!(
        before_the_first_frame(Ok(&session), Some(&now())),
        None,
        "a live session was refused a frame"
    );
    // **A clock this machine cannot read costs the sentence and not the run** — the claim
    // `console`'s own comment made and nothing checked.
    assert_eq!(
        before_the_first_frame(Ok(&session), None),
        None,
        "an unreadable clock ended a run that had a session"
    );
}

// --- WHICH CLUSTER ---
//
// **One place decides which context is used** (todo.md § Phase 12, NOTES § D116, § D279) — and
// what the box asks for is proven in two halves. The *rule* is [`which_cluster`], a decision over
// values with every row reachable; the *run* — `k8rs --once` with two contexts and no terminal —
// is `tests/binary.rs`'s and the report's, because this suite's own ends are both pipes and so it
// can only ever produce one row of the table (NOTES § D279 ruling 2).

/// **The whole truth table, because three of its four rows are unreachable from a live console**
/// (NOTES § D279 ruling 2, [`at_a_keyboard`]'s own measured hole).
///
/// **Precedence, once**: `--context` beats the picker, the picker beats `current-context`.
#[test]
fn one_function_decides_which_context_is_used_and_it_is_not_three() {
    let with = |opens: Opens<'_>| match opens {
        Opens::With(context) => Ok(context.map(str::to_owned)),
        Opens::Asking => Err("the picker"),
    };

    // `--context` beats the picker, at every row count and whether or not there is a keyboard.
    for rows in [0, 1, 2, 9] {
        for keyboard in [true, false] {
            assert_eq!(
                with(which_cluster(Some("prod-eu"), rows, keyboard)),
                Ok(Some("prod-eu".to_owned())),
                "`--context` gave way to the picker at {rows} rows, keyboard {keyboard}"
            );
        }
    }

    // The picker beats `current-context` — but only with a real choice and a terminal to draw in.
    assert_eq!(
        with(which_cluster(None, 2, true)),
        Err("the picker"),
        "two contexts and a keyboard did not open the picker"
    );
    assert_eq!(
        with(which_cluster(None, 9, true)),
        Err("the picker"),
        "nine contexts and a keyboard did not open the picker"
    );
    // **A picker in a pipeline is a script that hangs forever** (NOTES § D116): ambiguity resolves
    // the way it always did, silently, on `current-context`.
    assert_eq!(
        with(which_cluster(None, 2, false)),
        Ok(None),
        "a run with no terminal opened a picker nobody could answer"
    );
    // One context, or none: there is nothing to ask.
    assert_eq!(with(which_cluster(None, 1, true)), Ok(None));
    assert_eq!(with(which_cluster(None, 0, true)), Ok(None));
}

/// **What this run asked the watches to cover, before anything has answered** ([`asked_for`]) — the
/// value a frame drawn mid-switch reads, and the one a connection that never opened hands its
/// failure box.
#[test]
fn a_run_that_has_not_connected_covers_what_it_asked_for() {
    assert_eq!(
        asked_for(None).namespace(),
        None,
        "`--namespace` was never given"
    );
    assert_eq!(
        asked_for(Some("payments")).namespace(),
        Some("payments"),
        "a scoped run drew a header that reads like a cluster-wide one"
    );
}

/// **The login program never gets the terminal** (NOTES § D279 ruling 6, § D264 ruling 32) —
/// `interactive_mode: Never` on every `exec` block in the value handed to `k8s::connect_with`.
///
/// **Every entry and not the one being connected with**: a switch connects with another context out
/// of the same value, and this is the only reading of the file.
///
/// **The `Always` row is the one that matters.** A kubeconfig written by `aws eks
/// update-kubeconfig` or `kubelogin` carries its own `interactiveMode`, so a pass that only filled
/// in the missing ones would leave exactly the plugins that ask for a password inheriting a
/// raw-mode terminal.
#[test]
fn every_login_program_in_the_file_is_told_not_to_take_the_terminal() {
    let kubeconfig: kube::config::Kubeconfig = serde_yaml_ng::from_str(
        "apiVersion: v1\n\
         kind: Config\n\
         current-context: alpha\n\
         contexts:\n\
         - name: alpha\n  \
           context: { cluster: a, user: asks }\n\
         - name: beta\n  \
           context: { cluster: a, user: quiet }\n\
         clusters:\n\
         - name: a\n  \
           cluster: { server: 'https://a.invalid:6443' }\n\
         users:\n\
         - name: asks\n  \
           user:\n    \
             exec:\n      \
               apiVersion: client.authentication.k8s.io/v1beta1\n      \
               command: aws\n      \
               interactiveMode: Always\n\
         - name: unsaid\n  \
           user:\n    \
             exec:\n      \
               apiVersion: client.authentication.k8s.io/v1beta1\n      \
               command: kubelogin\n\
         - name: quiet\n  \
           user: {}\n",
    )
    .expect("a kubeconfig this test wrote itself");

    // Seen red first: `Always` and the unset one are both on the way in.
    let before: Vec<Option<kube::config::ExecInteractiveMode>> = kubeconfig
        .auth_infos
        .iter()
        .filter_map(|named| named.auth_info.as_ref()?.exec.as_ref())
        .map(|exec| exec.interactive_mode.clone())
        .collect();
    assert_eq!(
        before,
        vec![Some(kube::config::ExecInteractiveMode::Always), None],
        "the fixture did not carry the two shapes this is about"
    );

    let quietened = login_stays_off_the_terminal(kubeconfig);
    let plugins: Vec<&kube::config::ExecConfig> = quietened
        .auth_infos
        .iter()
        .filter_map(|named| named.auth_info.as_ref()?.exec.as_ref())
        .collect();
    assert_eq!(plugins.len(), 2, "the two `exec` users are not both here");
    for exec in plugins {
        assert_eq!(
            exec.interactive_mode,
            Some(kube::config::ExecInteractiveMode::Never),
            "a login program kept the terminal: {:?}",
            exec.command
        );
    }
    // A user with no login program is left exactly as the file wrote it.
    assert!(
        quietened
            .auth_infos
            .iter()
            .any(|named| named.name == "quiet"
                && named
                    .auth_info
                    .as_ref()
                    .is_some_and(|auth| auth.exec.is_none())),
        "the entry with no `exec` block was rewritten"
    );
}

/// **Which picker a key opens** ([`opening_on`], NOTES § D265 ruling 4).
///
/// **`Live` under [`ui::Link::Expired`] has to become `Dropped`**, or `⏎` on the `(current)` row
/// answers `Close` and *renew your login, then press `X`* shuts the picker instead of reconnecting.
#[test]
fn an_expired_login_opens_a_picker_that_can_reconnect_to_the_context_it_is_on() {
    let live = views::Connection::Live(Some("prod-eu".to_owned()));
    assert_eq!(
        opening_on(&live, ui::Link::Expired),
        views::Connection::Dropped(Some("prod-eu".to_owned())),
        "an expired login opened a picker whose `⏎` only closes"
    );
    // Every other link leaves it alone — a live cluster's `(current)` row is *stay here*.
    for link in [
        ui::Link::Live,
        ui::Link::Lost,
        ui::Link::Connecting,
        ui::Link::Unconnected,
    ] {
        assert_eq!(
            opening_on(&live, link),
            live,
            "{link:?} promoted a live connection to dropped"
        );
    }
    // A switch that failed already stored `Dropped`, carrying the context that was last live
    // (NOTES § D264 ruling 15), and nothing here has to tell the two apart.
    let dropped = views::Connection::Dropped(Some("prod-eu".to_owned()));
    assert_eq!(opening_on(&dropped, ui::Link::Unconnected), dropped);
    assert_eq!(
        opening_on(&views::Connection::Never, ui::Link::Connecting),
        views::Connection::Never
    );
}

/// **`X` opens the picker on the link the frame drew** ([`Console::link`]) — and appends the one
/// read behind it (NOTES § D264 ruling 29).
///
/// **The picker under test is the one `X` built, and that is what this test lacked** (`tester` F1,
/// NOTES § D280 item 5). It compared `picker.startup()` against `wanted == Connection::Never`,
/// where `wanted` was `Live` then `Dropped` — the constant `false` in both rows — and then
/// *replaced* `X`'s picker with one it built itself from [`opening_on`], so the D265 ruling 4
/// regression was invisible: planted in a mirror, 1562 tests passed.
///
/// **`⏎` on the `(current)` row is what tells the two variants apart**, because that is the whole
/// of what the ruling is about: a `Live` picker answers `Chosen::Close` there and shuts, and a
/// `Dropped` one connects — so *renew your login, then press `X`* either works or ends where it
/// started (`views::Picker::chosen`).
#[test]
fn x_opens_the_picker_the_header_just_described() {
    let store = a_cluster_with_cards();
    let current = k8s::Choice {
        name: Some("k8rs".to_owned()),
        key: "k8rs".to_owned(),
        server: k8s::Address::Server("https://k8rs.invalid:6443".to_owned()),
        shadowed: false,
        namespace: None,
        insecure: false,
        tag: k8s::Tag::Blank,
        current: true,
    };
    for (link, connects) in [(ui::Link::Live, false), (ui::Link::Expired, true)] {
        let mut console = bare_console();
        console.link = link;
        console.contexts = vec![current.clone()];
        // **Pressed, and then never touched** — every assertion below is about the picker this key
        // built.
        let _ = keyed(&mut console, typed('X'), &store);
        assert!(
            matches!(console.app.modal, Some(views::Modal::ContextPick(_))),
            "`X` opened no picker under {link:?}"
        );
        assert_eq!(
            console.log.lines().last().map(views::Stripped::as_str),
            Some(views::GET_CONTEXTS),
            "`X` did not say what it read"
        );
        // **Never the startup variant**: something has connected, whatever the link says about it.
        match &console.app.modal {
            Some(views::Modal::ContextPick(picker)) => {
                assert!(
                    !picker.startup(),
                    "{link:?} opened the startup picker mid-session"
                );
                assert_eq!(
                    picker.verbs(),
                    ("switch", "cancel"),
                    "{link:?} drew the startup picker's words"
                );
            }
            _ => unreachable!(),
        }
        let answered = pressed(
            &mut console,
            pressing(ratatui::crossterm::event::KeyCode::Enter),
            &[],
            &before_the_list(),
        );
        match (connects, answered) {
            // A live cluster's own row is *yes, stay here*, and the box closes.
            (false, Did::Changed) => assert_eq!(
                console.app.modal, None,
                "{link:?}: `⏎` on the live current row left the picker open"
            ),
            // An expired login's is *try this again*, or the one way back does nothing.
            (true, Did::Switch(asked)) => {
                assert_eq!(asked.key, "k8rs");
                assert_eq!(
                    asked.before,
                    views::Before::Connected(Some("k8rs".to_owned())),
                    "{link:?}: the way out did not name the context that was live"
                );
            }
            (_, answered) => panic!(
                "{link:?}: `⏎` on the current row answered {}",
                match answered {
                    Did::Changed => "Changed",
                    Did::Nothing => "Nothing",
                    Did::Switch(_) => "Switch",
                    _ => "something else",
                }
            ),
        }
    }
}

/// **`⏎` connects with the file's own spelling and draws the stripped one** (NOTES § D264
/// ruling 8, invariant 9) — the two are different strings whenever a context name carries a
/// character that has no printed form, and a picker that handed the drawn name back would answer
/// `k8s::Fault::NoContext` for a context that is right there in the file.
///
/// **Fed through `k8s::contexts` rather than hand-built**, which is the only framing that proves
/// the strip the box owes (CLAUDE.md § A check is proven only for the shapes it was fed) — **and
/// fed every shape, then drawn** (`tester` F3, NOTES § D280 item 5): a bell, an ANSI colour
/// escape, a right-to-left override and a name ten thousand characters long. The first draft fed
/// the bell alone and never called [`framed`], so *the frame stays 80×24* was proven nowhere in
/// the suite — which is the half of invariant 9 that is about the terminal rather than the string.
#[test]
fn a_crafted_context_name_connects_by_its_key_and_is_drawn_stripped() {
    let crafted = "stag\u{7}ing";
    let huge = "g".repeat(10_000);
    let kubeconfig: kube::config::Kubeconfig = serde_yaml_ng::from_str(&format!(
        "apiVersion: v1\n\
         kind: Config\n\
         current-context: alpha\n\
         contexts:\n\
         - name: alpha\n  \
           context: {{ cluster: a, user: k8rs }}\n\
         - name: \"stag\\u0007ing\"\n  \
           context: {{ cluster: a, user: k8rs }}\n\
         - name: \"\\x1b[31mred\"\n  \
           context: {{ cluster: a, user: k8rs }}\n\
         - name: \"pay\\u202Ements\"\n  \
           context: {{ cluster: a, user: k8rs }}\n\
         - name: \"{huge}\"\n  \
           context: {{ cluster: a, user: k8rs }}\n\
         - name: \"prod eu; echo pwned\"\n  \
           context: {{ cluster: a, user: k8rs }}\n\
         clusters:\n\
         - name: a\n  \
           cluster: {{ server: 'https://a.invalid:6443' }}\n\
         users:\n\
         - name: k8rs\n  \
           user: {{}}\n"
    ))
    .expect("a kubeconfig this test wrote itself");
    let contexts = k8s::contexts(&kubeconfig, None);
    assert_eq!(contexts.len(), 6, "the fixture lost a row");
    // **`k8s::drawable` is not a charset allowlist, and this is where that is proven** (NOTES
    // § D281 item 1): it removes characters with no printed form and caps length, so a space and a
    // `;` reach every reader of `Choice::name` intact. The sentence built from one is an
    // instruction to paste, so it quotes — `ops::pasteable`, the one rule, never a third spelling.
    let shell = contexts[5].name.as_deref().expect("a drawable name");
    assert_eq!(
        shell, "prod eu; echo pwned",
        "the strip removed the dangerous half"
    );
    let told = views::run_the_login(Some(shell)).expect("a runnable name");
    println!("{told}");
    assert!(
        told.contains(&crate::ops::pasteable(shell)) && !told.contains("--context prod eu;"),
        "an unquoted name reached a sentence telling the reader to run it: {told:?}"
    );
    // **And a name with nothing left after the strip is told nothing at all** — `(unnamed)` is a
    // shell syntax error and a bare `kubectl version` names a third cluster (NOTES § D281 item 2).
    // **All three shapes of nothing**, because a check is proven only for the shapes it was fed
    // (NOTES § D29): no name at all, a name that strips to nothing, and the empty one the `filter`
    // exists for — unreachable through `k8s::drawable`, which answers `None` for it, and reachable
    // from any other caller.
    assert_eq!(views::run_the_login(None), None);
    assert_eq!(views::run_the_login(Some("\u{202e}")), None);
    assert_eq!(views::run_the_login(Some("")), None);
    assert_eq!(
        contexts[1].key, crafted,
        "the key stopped being the file's own spelling"
    );
    assert_eq!(
        contexts[4].key.chars().count(),
        10_000,
        "the key was bounded, and a key cut at 512 bytes opens no entry"
    );

    let mut console = bare_console();
    console.contexts = contexts.clone();
    console.app.modal = Some(views::Modal::ContextPick(views::Picker::new(
        &contexts,
        views::Connection::Live(Some(views::drawn_name(&contexts[0]).to_owned())),
    )));

    // **Drawn with every one of them on screen** — invariant 9's other half: a crafted name may
    // not rewrite the reader's terminal, and a 10,000-character one may not blow up the frame.
    let picked = framed(&mut console, &k8s::Store::default());
    println!("{picked}");
    let rows: Vec<&str> = picked.lines().collect();
    assert_eq!(rows.len(), 24, "the frame stopped being 24 rows:\n{picked}");
    for (n, row) in rows.iter().enumerate() {
        assert!(
            row.chars().count() <= 80,
            "row {n} ran past 80 columns ({} chars):\n{picked}",
            row.chars().count()
        );
    }
    for (what, character) in [
        ("a bell", '\u{7}'),
        ("an escape", '\u{1b}'),
        ("a right-to-left override", '\u{202e}'),
    ] {
        assert!(
            !picked.contains(character),
            "{what} reached the screen:\n{picked}"
        );
    }
    // The canary: this is the picker's own frame with the crafted rows on it, and not an empty
    // buffer that would pass every absence above (CLAUDE.md § A derived list asserts it found
    // something).
    assert!(
        picked.contains("Switch cluster"),
        "not a picker frame:\n{picked}"
    );
    assert!(
        picked.contains("staging"),
        "the crafted row never drew, so nothing above was tested:\n{picked}"
    );

    let _ = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Down),
        &[],
        &before_the_list(),
    );
    let answered = pressed(
        &mut console,
        pressing(ratatui::crossterm::event::KeyCode::Enter),
        &[],
        &before_the_list(),
    );
    let Did::Switch(asked) = answered else {
        panic!("`⏎` on the crafted row reached nothing")
    };
    assert_eq!(
        asked.key, crafted,
        "the connection was made with the drawn name, which names no context"
    );
    let to = asked.to.expect("a name that does not strip to nothing");
    assert_eq!(to, "staging", "the failure box's title was not stripped");
    assert!(
        !to.chars().any(|character| character.is_control()),
        "a control character reached the dialog: {to:?}"
    );
    // The header's zone is built from the same value and carries none either.
    let zone = zone(Some(&to), None);
    assert!(
        !zone.chars().any(|character| character.is_control()),
        "a control character reached the header: {zone:?}"
    );
    // And so is every command line this connection would teach.
    let taught = kubectl(Some(&asked.key));
    println!("{taught}");
    assert!(
        taught.contains("--context") && !taught.chars().any(|character| character.is_control()),
        "the taught line lost the flag or kept the control character: {taught:?}"
    );
}

/// **The startup picker's `esc` quits the way `q` does** (NOTES § D279 ruling 5): `console()`
/// answers `None`, nothing goes to stderr and the process exits `0`. It is a reader leaving, not a
/// line being refused.
///
/// **And the mid-session picker's `esc` cancels onto the cluster already connected** — the same key
/// and two answers, which is the one thing that differs between the two pickers.
#[test]
fn esc_quits_the_startup_picker_and_only_cancels_the_other_one() {
    let contexts = vec![
        k8s::Choice {
            name: Some("prod-eu".to_owned()),
            key: "prod-eu".to_owned(),
            server: k8s::Address::Server("https://prod-eu.invalid:6443".to_owned()),
            shadowed: false,
            namespace: None,
            insecure: false,
            tag: k8s::Tag::Blank,
            current: true,
        },
        k8s::Choice {
            name: Some("staging".to_owned()),
            key: "staging".to_owned(),
            server: k8s::Address::Server("https://staging.invalid:6443".to_owned()),
            shadowed: false,
            namespace: None,
            insecure: false,
            tag: k8s::Tag::Blank,
            current: false,
        },
    ];
    let mut starting = bare_console();
    starting.contexts = contexts.clone();
    starting.connection = views::Connection::Never;
    starting.app.modal = Some(views::Modal::ContextPick(views::Picker::new(
        &contexts,
        views::Connection::Never,
    )));
    assert!(
        matches!(
            pressed(
                &mut starting,
                pressing(ratatui::crossterm::event::KeyCode::Esc),
                &[],
                &before_the_list(),
            ),
            Did::Quit
        ),
        "`esc` on the startup picker did not end the run"
    );

    let mut switching = bare_console();
    switching.contexts = contexts.clone();
    switching.app.modal = Some(views::Modal::ContextPick(views::Picker::new(
        &contexts,
        views::Connection::Live(Some("prod-eu".to_owned())),
    )));
    assert!(
        matches!(
            pressed(
                &mut switching,
                pressing(ratatui::crossterm::event::KeyCode::Esc),
                &[],
                &before_the_list(),
            ),
            Did::Changed
        ),
        "`esc` mid-session ended the run instead of cancelling"
    );
    assert_eq!(switching.app.modal, None, "the picker stayed open");
}

/// **The two frames nothing is connected behind** (`screens/context.md` § Opening at startup,
/// `screens/widgets.md` § 1a).
///
/// **The startup picker's header says `choose a cluster`, not a context**, and the body behind it
/// is genuinely empty — no sidebar, no vitals, no pane.
///
/// **A switch that failed draws no connection word at all** ([`ui::Link::Unconnected`]): the store
/// is empty after `views::App::switched`, which is the shape of a first launch, so a frame that
/// asked [`linked`] would say `connecting…` over a cluster that has already refused.
#[test]
fn nothing_connected_draws_no_context_at_startup_and_no_connection_word_after_a_failure() {
    let mut starting = bare_console();
    starting.context = views::Stripped::of("");
    starting.connection = views::Connection::Never;
    starting.contexts = vec![k8s::Choice {
        name: Some("prod-eu".to_owned()),
        key: "prod-eu".to_owned(),
        server: k8s::Address::Server("https://prod-eu.invalid:6443".to_owned()),
        shadowed: false,
        namespace: None,
        insecure: false,
        tag: k8s::Tag::Blank,
        current: true,
    }];
    starting.app.modal = Some(views::Modal::ContextPick(views::Picker::new(
        &starting.contexts.clone(),
        views::Connection::Never,
    )));
    let opening = framed(&mut starting, &k8s::Store::default());
    println!("{opening}");
    assert!(
        opening.contains("Choose a cluster"),
        "the startup picker did not draw:\n{opening}"
    );
    assert!(
        opening.contains("choose a cluster · admin"),
        "the header did not say which state this is:\n{opening}"
    );
    assert!(
        !opening.contains("ctx:") && !opening.contains("connecting…"),
        "a run that has picked nothing named a context:\n{opening}"
    );
    assert!(
        !opening.contains("ALERTS"),
        "the app frame drew behind a picker that has nothing behind it:\n{opening}"
    );

    // A switch that failed: the attempted context is still named, and the slot beside it says the
    // connection is not one — **never blank, and never one of `ui::Link`'s four**
    // (`screens/widgets.md` § 1a, NOTES § D280 item 1). This assertion read
    // `after.contains("ctx: staging · admin")` for a round, which pinned the defect: `admin` is
    // the *permission* word that sat there over a live cluster a moment earlier.
    let mut failed = bare_console();
    failed.context = views::Stripped::of(&format!(
        "{} · {}",
        zone(Some("staging"), None),
        not_connected()
    ));
    failed.unconnected = true;
    failed.connection = views::Connection::Dropped(Some("prod-eu".to_owned()));
    let after = framed(&mut failed, &k8s::Store::default());
    println!("{after}");
    assert!(
        after.contains("ctx: staging · ⚠ not connected · admin"),
        "the header's connection slot is not the page's:\n{after}"
    );
    for word in ["connecting…", "live", "disconnected", "login expired"] {
        assert!(
            !after.contains(&format!("connected · {word}")),
            "a second connection word was joined behind the first:\n{after}"
        );
    }
    // **The body says what the state is, and does not claim to be reading anything** — nothing
    // survived the switch, so *reading the cluster…* would be about a store that is not this
    // cluster's (`screens/context.md` § After `esc dismiss`).
    assert!(
        after.contains("⚠ Not connected to the cluster right"),
        "the body did not say nothing is connected:\n{after}"
    );
    assert!(
        !after.contains("reading the cluster…"),
        "a run connected to nothing claimed to be reading one:\n{after}"
    );
    // **The left zone is blank, not `nodes …`** — that reading is *while connecting*.
    assert!(
        !after.contains("nodes"),
        "a node count survived the switch that threw the nodes away:\n{after}"
    );
    // **`X` is the way out and the footer says so** (`ui::offered`, NOTES § D264 ruling 15).
    assert!(
        after.contains("X switch cluster"),
        "the one way out was not offered:\n{after}"
    );
    // The canary: this is a console frame and not an empty buffer that passes every absence above.
    assert!(after.contains("ALERTS"), "not a console frame:\n{after}");
}

/// **A switch that cannot connect becomes the box, and nothing of the old cluster is left behind
/// it** (`screens/context.md` § When the new cluster does not work, § What happens on `⏎` steps 1
/// and 2; NOTES § D16 rulings 1 and 3, § D264 ruling 13).
///
/// **A context the file does not name is the one connection failure a suite with no cluster can
/// produce**: kube resolves the context out of the value it was handed and refuses before a client
/// is built, so nothing here reaches a network (`tests/binary.rs`
/// § `no_test_here_can_reach_a_cluster` is why that matters).
///
/// **The frame in between is asserted too** — the connect is awaited, so without a draw before it
/// the reader watches the old cluster's screen through a switch that has already thrown it away.
#[tokio::test]
async fn a_switch_that_cannot_connect_is_the_box_and_leaves_nothing_of_the_old_cluster() {
    // **The `current` row turns TLS verification off**, so `insecure` below tells a switch that
    // re-read the list *for the context it tried* from one that re-read the file's own current
    // context — two lists that answer differently (`tester` F4).
    let kubeconfig = two_contexts_one_unverified();
    let mut console = bare_console();
    // **`--read-only` is the process's and not the context's** (`screens/context.md` § What happens
    // on `⏎`), so it has to survive `views::App::switched`.
    console.writes = ui::Writes::ReadOnly;
    // Everything the old cluster left behind, so that dropping it is observable rather than
    // asserted about an already-empty console.
    console.app.view = views::View::Resources(0);
    console.app.filters.text.push('w');
    console.opened = Some(Opened::Pods(pod_id("payments", "web-1")));
    console.kinds = vec![browsable("apps", "Deployment", "deployments", true)];
    console.clock = Some("the clocks disagree".to_owned());
    console.insecure = true;
    // **The strip as the running console actually leaves it when `⏎` is pressed** (`k8s-admin`,
    // NOTES § D281): both sites that build `views::Modal::ContextPick` append
    // `views::GET_CONTEXTS` the moment they open it, so the newest line on this frame is always
    // the picker's own local file read and the old cluster's line is history beneath it. A
    // one-line fixture with no `get-contexts` in it pinned a shape no path produces.
    console
        .log
        .ran("$ kubectl --context prod-eu get pods -A --watch".to_owned());
    console.log.ran(views::GET_CONTEXTS.to_owned());
    let mut cluster = nothing_connected(None);
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))
        .expect("a terminal over a test backend");

    switched(
        &mut console,
        &mut terminal,
        &mut cluster,
        &kubeconfig,
        Some("payments"),
        Switching {
            key: "nothing-in-the-file-names-this".to_owned(),
            to: Some("nothing-in-the-file-names-this".to_owned()),
            before: views::Before::Connected(Some("prod-eu".to_owned())),
        },
    )
    .await
    .expect("the frames draw");

    // **The box, built from the fault, the run's scope and the login program**
    // (todo.md § Phase 12).
    match &console.app.modal {
        Some(views::Modal::Unconnected {
            to,
            before,
            sent,
            fault,
            said,
            coverage,
            renewal,
        }) => {
            assert_eq!(to.as_deref(), Some("nothing-in-the-file-names-this"));
            assert_eq!(
                before,
                &views::Before::Connected(Some("prod-eu".to_owned())),
                "the way out did not name the cluster that was live"
            );
            // **Nothing reached a cluster**, so the title is *could not be opened* and no scope is
            // read (NOTES § D264 ruling 17, `ui::failed`).
            assert!(
                !sent,
                "a connection that was never built claimed to have asked"
            );
            assert_eq!(*fault, k8s::Fault::NoContext);
            assert_eq!(said.as_deref(), None);
            assert_eq!(
                coverage.namespace(),
                Some("payments"),
                "the box did not carry the scope this run asked for"
            );
            assert_eq!(renewal.as_deref(), None);
        }
        other => panic!("a switch that failed left {other:?} on screen"),
    }

    // **Nothing of the old cluster survived** (NOTES § D16 ruling 1), and what is not the old
    // cluster's did.
    assert!(
        cluster.session.is_none(),
        "the old session outlived the switch"
    );
    assert!(
        cluster.updates.drained,
        "the old cluster's streams were still being polled"
    );
    assert!(cluster.server.is_empty());
    assert_eq!(console.app.view, views::View::Alerts);
    assert!(
        console.app.filters.text.is_empty(),
        "the old cluster's filter survived"
    );
    assert!(
        console.opened.is_none(),
        "an open detail slot outlived the cluster it was read from"
    );
    assert!(
        console.kinds.is_empty(),
        "the old cluster's sidebar survived"
    );
    assert_eq!(
        console.clock, None,
        "a skew read against the old cluster survived"
    );
    // **The strip keeps the old cluster's last line** (NOTES § D280 item 2,
    // `screens/context.md` § When the new cluster does not work): nothing was sent for the context
    // that failed — `sent` is `false` for every fault this box draws — so there is no line about it
    // to append, and emptying the strip on `⏎` left the reader a blank pane for the length of the
    // connect and a blank one after it failed.
    assert_eq!(
        console
            .log
            .lines()
            .iter()
            .map(views::Stripped::as_str)
            .collect::<Vec<&str>>(),
        vec![
            "$ kubectl --context prod-eu get pods -A --watch",
            views::GET_CONTEXTS,
        ],
        "the strip did not keep what was already there, in the order it was there"
    );
    // **The newest line is the picker's own local read, never a cluster's** — which is what makes
    // the frame honest: the line naming `prod-eu` reads as history under a header naming the
    // context that was tried (`screens/context.md` § When the new cluster does not work).
    assert_eq!(
        console.log.lines().last().map(views::Stripped::as_str),
        Some(views::GET_CONTEXTS)
    );
    assert!(
        !console
            .log
            .lines()
            .iter()
            .any(|line| line.as_str().contains("nothing-in-the-file-names-this")),
        "a line was invented about a context nothing was ever sent to: {:?}",
        console.log.lines()
    );
    assert!(
        !console.insecure,
        "the TLS warning was read off the kubeconfig's current row and not the one that was tried"
    );
    assert_eq!(
        console.writes,
        ui::Writes::ReadOnly,
        "`--read-only` did not outlive `X`"
    );

    // **The header names the context that was chosen and no connection word joins it**
    // (NOTES § D16 ruling 3, `ui::Link::Unconnected`).
    assert!(
        console.unconnected,
        "a failed switch was still a connection"
    );
    assert!(
        console
            .context
            .as_str()
            .contains("nothing-in-the-file-names-this"),
        "the header lost the context the reader chose: {:?}",
        console.context.as_str()
    );
    // **And the connection slot says what the connection is** (NOTES § D280 item 1,
    // `screens/widgets.md` § 1a: *not a fifth connection word, and not a blank segment either*).
    // Without it the zone reads `ctx: … · admin`, where `admin` is the permission word that sat
    // there over a live cluster a moment earlier.
    assert!(
        console.context.as_str().ends_with(&not_connected()),
        "the header's connection slot is blank over a cluster that was never opened: {:?}",
        console.context.as_str()
    );
    // **What a later `X` opens on** (NOTES § D264 ruling 15).
    assert_eq!(
        console.connection,
        views::Connection::Dropped(Some("prod-eu".to_owned())),
        "`X` would no longer name the cluster that was last live"
    );

    // **A frame was drawn between `⏎` and its answer, and it is the new context's**
    // (`screens/context.md` § What happens on `⏎` step 3): `ctx: <new> · connecting…`, over the
    // loading body. **Both halves and no `||`** (`tester` F2): the name alone passes over a frame
    // that said `live`, and an `…` alone is matched by `reading the cluster…` and `nodes …` — the
    // old cluster's own frame, which is exactly what this assertion exists to forbid.
    //
    // **Read off the terminal before the second `switched` below overwrites it.**
    let drawn = screened(&terminal);
    println!("{drawn}");
    assert!(
        drawn.contains("nothing-in-the-file-names-this"),
        "the frame in between did not name the context `⏎` chose:\n{drawn}"
    );
    assert!(
        drawn.contains("connecting…"),
        "the frame in between did not say a connection was in flight:\n{drawn}"
    );
    for stale in ["live", "⚠ not connected"] {
        assert!(
            !drawn.contains(stale),
            "the frame in between said {stale:?} while the connect was still out:\n{drawn}"
        );
    }

    // **A second failure in a row still names the context that was last live** (ruling 15): the
    // way out is read off what had connected, and a run that never did gets the startup picker
    // back instead.
    switched(
        &mut console,
        &mut terminal,
        &mut cluster,
        &kubeconfig,
        None,
        Switching {
            key: "still-not-in-the-file".to_owned(),
            to: Some("still-not-in-the-file".to_owned()),
            before: views::Before::Picking(views::Picker::new(
                &k8s::contexts(&kubeconfig, None),
                views::Connection::Never,
            )),
        },
    )
    .await
    .expect("the frames draw");
    assert_eq!(
        console.connection,
        views::Connection::Never,
        "a run that has never connected was offered a cluster to go back to"
    );
    assert!(
        matches!(
            &console.app.modal,
            Some(views::Modal::Unconnected {
                before: views::Before::Picking(_),
                ..
            })
        ),
        "the startup picker was not what `esc` goes back to"
    );
}

/// **A key pressed the instant a mutation settles reaches its consequence, and the `Halt` that
/// carries it is not dropped** (`k8s-admin`, NOTES § D280 item 3).
///
/// **The arm this is about claimed both halts were unreachable, and the premise was false.** The
/// inner [`pump`] does not return when a mutation completes — it runs [`settled`], sets its
/// `running` slot to `None` and goes on looping — and `settled` takes `views::App::changing`, which
/// is the field `views::App::may_mutate` and `may_switch_cluster` both read. So from that instant
/// `ctrl-d`, `r` and `X` are live again *inside* a call whose caller was throwing their answers
/// away, and the reader pressed the key twice.
///
/// **What is proven here is the reachability, which is the half a test can reach**: `console`'s own
/// frame, where the halt is now carried into the next turn of the loop, needs a terminal and a
/// cluster and is the same hole the three `console` mutants name.
#[test]
fn a_key_pressed_the_moment_a_mutation_settles_is_not_refused() {
    let store = a_cluster_with_cards();
    let mut console = bare_console();
    let cards = carded(&store);
    let card = selected(&console, &cards).expect("the captures produced a selectable card");
    let _ = framed(&mut console, &store);

    // **In flight**: the reader said yes, the box closed, and every mutating key is refused —
    // `views::App::may_mutate` reads `changing` (`screens/dialogs.md` § While the call is running).
    console.app.changing = Some(views::Object::new(
        "pod",
        card.owner.namespace.clone(),
        card.owner.name.clone(),
        None,
    ));
    for pressed in [
        control(ratatui::crossterm::event::KeyCode::Char('d')),
        typed('X'),
    ] {
        assert!(
            matches!(keyed(&mut console, pressed.clone(), &store), Did::Nothing),
            "{pressed:?} was live while a call was on the wire"
        );
    }

    // **Settled** — the call ended, whichever way. `Err` is `ops::restart`'s own refusal and needs
    // no cluster to produce; what matters is that `settled` runs at all.
    settled(&mut console, Err("nothing was sent".to_owned()));
    assert!(
        console.app.changing.is_none(),
        "`settled` left the object in flight"
    );
    let _ = framed(&mut console, &store);

    // **And live again, in the same `pump` call** — which is the whole of why the arm that dropped
    // these halts was wrong.
    assert!(
        matches!(
            keyed(
                &mut console,
                control(ratatui::crossterm::event::KeyCode::Char('d')),
                &store
            ),
            Did::Mutate(_)
        ),
        "a delete asked for after the dialog closed reached nothing"
    );
    // Dismiss the box that key opened, so `X` is not refused for the modal instead.
    let _ = keyed(
        &mut console,
        key(ratatui::crossterm::event::KeyCode::Esc),
        &store,
    );
    assert!(
        matches!(keyed(&mut console, typed('X'), &store), Did::Changed),
        "a switch asked for after the dialog closed opened no picker"
    );
}

/// **A halt a frame cannot act on is parked, and one that can is answered** ([`parked`],
/// NOTES § D281 item 5) — the decision over values that used to be an arm inside [`console`],
/// where `cargo mutants --list` offers 0 mutants against 3482 in the crate and `just picker`
/// cannot reach the window either.
#[test]
fn a_halt_a_mutation_frame_cannot_act_on_is_parked_rather_than_answered() {
    let asked = || {
        Halt::Mutate(Wanted {
            verb: DELETE,
            kind: "pod",
            name: "web-1".to_owned(),
            namespace: Some("payments".to_owned()),
            uid: None,
        })
    };
    // **A mutation's own frame holds the future, the `&mut File` and the strings they borrow
    // (NOTES § D232), so it cannot build a second one** — the key is kept instead of dropped.
    let mut carried = None;
    assert!(matches!(parked(&mut carried, true, asked()), Halt::Carried));
    assert!(
        matches!(carried, Some(Halt::Mutate(_))),
        "the key was answered away instead of parked"
    );

    // **A frame with nothing running answers on the spot**, and parks nothing — a slot filled here
    // would be a keypress replayed one turn late.
    let mut idle = None;
    assert!(matches!(parked(&mut idle, false, asked()), Halt::Mutate(_)));
    assert!(idle.is_none(), "an ordinary frame parked its own answer");

    // Both halts a frame can be asked for, both ways.
    let switching = || {
        Halt::Switch(Switching {
            key: "prod-eu".to_owned(),
            to: Some("prod-eu".to_owned()),
            before: views::Before::Connected(Some("staging".to_owned())),
        })
    };
    let mut slot = None;
    assert!(matches!(
        parked(&mut slot, true, switching()),
        Halt::Carried
    ));
    assert!(matches!(slot, Some(Halt::Switch(_))));
    assert!(matches!(
        parked(&mut None, false, switching()),
        Halt::Switch(_)
    ));
}

/// **A parked halt is the next call's first answer, before a key is read or a frame is drawn**
/// ([`pump`], NOTES § D281 item 5).
///
/// **`q` is queued on purpose**: a [`pump`] that ignored the slot would read it and answer
/// `Halt::Quit`, which is the shape this test tells apart — rather than hanging, which reads like
/// a harness fault.
#[tokio::test(start_paused = true)]
async fn pump_answers_with_what_was_parked_before_it_reads_a_key() {
    let mut console = bare_console();
    let mut store = k8s::Store::default();
    let mut updates = Updates {
        merged: futures_util::stream::select_all(Vec::new()),
        drained: true,
        asked: std::collections::BTreeSet::new(),
        asking: tokio::sync::mpsc::unbounded_channel().0,
    };
    let at = Watching {
        renewal: None,
        coverage: k8s::Coverage::Cluster,
    };
    let (pressing, mut keys) = tokio::sync::mpsc::unbounded_channel();
    let _ = pressing.send(Woke::Key(typed('q')));
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))
        .expect("a terminal over a test backend");
    console.carried = Some(Halt::Switch(Switching {
        key: "prod-eu".to_owned(),
        to: Some("prod-eu".to_owned()),
        before: views::Before::Connected(Some("staging".to_owned())),
    }));

    let halt = pump(
        &mut console,
        &mut terminal,
        &mut store,
        &mut updates,
        &mut keys,
        &at,
        None,
    )
    .await;
    match halt {
        Halt::Switch(asked) => assert_eq!(asked.key, "prod-eu"),
        Halt::Quit => panic!("the parked halt was left in the slot and a key answered instead"),
        _ => panic!("the parked halt did not come back"),
    }
    assert!(console.carried.is_none(), "the slot was not drained");
}

/// **Either key pressed the moment a mutation settles comes back out of the frame, parked** — the
/// regression round one fixed inside [`console`], where nothing could prove it (NOTES § D281
/// item 5; `k8s-admin` and `tester`, 2026-09-26).
///
/// **The window is real and this drives it**: `pump` takes its own `Running` slot to `None` the
/// instant the call settles and goes on reading keys, so `views::App::may_mutate` and
/// `may_switch_cluster` are both live again inside a call whose caller still holds the future and
/// the audit `File`. Reverting [`parked`] to answer the halt puts the drop back, and the reader
/// presses the key twice.
///
/// **Both halts, because one of them was pinned by nothing** (`tester`, round three): with only
/// the `ctrl-d` pass, a plant that read the capture live *on the `Halt::Switch` site alone* left
/// 1567 tests green — the other key's window had no test, and it is the same defect class.
/// [`a_halt_a_mutation_frame_cannot_act_on_is_parked_rather_than_answered`] cannot see it either,
/// being a pure-value test over `parked`'s three arguments with no view of where the second one
/// came from.
///
/// **`X` then `↓` then `⏎`, because `X` alone opens the picker and does not ask for a switch** —
/// `views::Chosen::Connect` needs a row that is not the live current one, which is why the fixture
/// has two. The keys after the settle are ordinary keys; what is under test is only that the halt
/// they finally produce leaves the frame in the slot.
///
/// **`Err` is the mutation's ending**, because what matters is that [`settled`] runs at all — and
/// an `Err` needs no cluster to produce (`ops::restart`'s own refusal).
#[tokio::test(start_paused = true)]
async fn a_key_pressed_the_moment_a_mutation_settles_leaves_the_frame_parked() {
    let row = |name: &str, current: bool| k8s::Choice {
        name: Some(name.to_owned()),
        key: name.to_owned(),
        server: k8s::Address::Server(format!("https://{name}.invalid:6443")),
        shadowed: false,
        namespace: None,
        insecure: false,
        tag: k8s::Tag::Blank,
        current,
    };
    let asking_to_delete = vec![control(ratatui::crossterm::event::KeyCode::Char('d'))];
    let asking_to_switch = vec![
        typed('X'),
        key(ratatui::crossterm::event::KeyCode::Down),
        key(ratatui::crossterm::event::KeyCode::Enter),
    ];
    for (what, presses) in [
        ("a mutating key", asking_to_delete),
        ("a switch", asking_to_switch),
    ] {
        let mut store = a_cluster_with_cards();
        let mut console = bare_console();
        console.contexts = vec![row("k8rs", true), row("staging", false)];
        let cards = carded(&store);
        let card = selected(&console, &cards).expect("the captures produced a selectable card");
        // **In flight**: the reader said yes and the box closed (NOTES § D20).
        console.app.changing = Some(views::Object::new(
            "pod",
            card.owner.namespace.clone(),
            card.owner.name.clone(),
            None,
        ));
        let mut updates = Updates {
            merged: futures_util::stream::select_all(Vec::new()),
            drained: true,
            asked: std::collections::BTreeSet::new(),
            asking: tokio::sync::mpsc::unbounded_channel().0,
        };
        let at = Watching {
            renewal: None,
            coverage: k8s::Coverage::Cluster,
        };
        let (pressing, mut keys) = tokio::sync::mpsc::unbounded_channel();
        // **Long after the call has settled**, so the keys land in the window and not before it: a
        // key queued up front is read first (the `select!` is biased) and refused while `changing`
        // is set.
        tokio::spawn(async move {
            for press in presses {
                tokio::time::sleep(COALESCE * 20).await;
                let _ = pressing.send(Woke::Key(press));
            }
        });
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24))
            .expect("a terminal over a test backend");
        let mut performing: std::pin::Pin<
            Box<dyn Future<Output = Result<ops::Performed, String>>>,
        > = Box::pin(async { Err("nothing was sent".to_owned()) });
        let shown = std::cell::RefCell::new(Vec::new());
        let (answering, _answered) = tokio::sync::mpsc::unbounded_channel();

        let halt = pump(
            &mut console,
            &mut terminal,
            &mut store,
            &mut updates,
            &mut keys,
            &at,
            Some(Running {
                performing: &mut performing,
                shown: &shown,
                answering,
            }),
        )
        .await;

        assert!(
            console.app.changing.is_none(),
            "{what}: the mutation never settled, so this drove the wrong window"
        );
        assert!(
            matches!(halt, Halt::Carried),
            "{what}: the mutating frame answered a key it cannot act on instead of parking it"
        );
        match (what, console.carried) {
            ("a mutating key", Some(Halt::Mutate(wanted))) => {
                assert_eq!(wanted.verb, DELETE);
                assert_eq!(wanted.name, card.owner.name);
            }
            ("a switch", Some(Halt::Switch(asked))) => {
                assert_eq!(asked.key, "staging");
                assert_eq!(
                    asked.before,
                    views::Before::Connected(Some("k8rs".to_owned())),
                    "the way out did not name the cluster that was live"
                );
            }
            (what, parked) => panic!(
                "{what}: the reader's key was dropped — {}",
                match parked {
                    None => "the slot is empty",
                    Some(_) => "the slot holds the other halt",
                }
            ),
        }
    }
}
