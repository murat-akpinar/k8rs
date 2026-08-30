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
    load(&[], now()).expect("no paths is not a failure")
}

fn read(names: &[&str]) -> Input {
    let paths: Vec<String> = names.iter().map(|n| fixture(n)).collect();
    load(&paths, now()).unwrap_or_else(|e| panic!("{names:?} did not load: {e}"))
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
    let input = load(&[one.clone(), two.clone()], now()).expect("both documents read");
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
    let input = load(std::slice::from_ref(&path), now()).expect("an empty list is not an error");
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
    let Err(problem) = load(std::slice::from_ref(&missing), now()) else {
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

    let Err(problem) = load(std::slice::from_ref(&crafted), now()) else {
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
    let Err(problem) = load(std::slice::from_ref(&path), now()) else {
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
            &AtConnect::default()
        ),
        None
    );

    // Four of the five landed and the fifth never opened: still not a cluster anyone may read.
    let mut store = k8s::Store::default();
    the_other_four(&mut store);
    assert_eq!(
        live_report(&store, now(), &mut last, false, &AtConnect::default()),
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
        &AtConnect::default(),
    )
    .expect("a listed store");
    assert!(!printed.is_empty(), "the report is empty: {printed:?}");
    // `None` and not merely *empty*: `Some(String::new())` is a blank block on stdout, which is
    // what the driver would print every time a watch re-listed.
    assert_eq!(
        live_report(&store, now(), &mut last, false, &AtConnect::default()),
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

    let first = live_report(&store, now(), &mut last, false, &AtConnect::default())
        .expect("every initial LIST landed");
    println!("{first}");
    assert!(
        first.contains(" pods · "),
        "the live report is not the report `render` draws"
    );
    assert_eq!(
        live_report(&store, now(), &mut last, false, &AtConnect::default()),
        None,
        "the same cluster printed twice"
    );

    let crashloop: Pod = serde_json::from_str(
        &std::fs::read_to_string(fixture("crashloop.json")).expect("the fixture reads"),
    )
    .expect("the capture decodes");
    store.pod(&now(), Event::Apply(crashloop));
    let second = live_report(&store, now(), &mut last, false, &AtConnect::default())
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
    let path = format!(
        "{}/tests/fixtures/certs/expiring-client.crt.pem",
        env!("CARGO_MANIFEST_DIR")
    );
    k8s::Identity {
        server_version: server_version.map(str::to_string),
        context: Some("kind-k8rs".to_string()),
        client_certificate: Some(
            std::fs::read(&path)
                .unwrap_or_else(|e| panic!("certificate {path} does not read: {e}")),
        ),
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
    let plain = live_report(&store, now(), &mut last, false, &AtConnect::default())
        .expect("every LIST landed");
    for pane in PANES {
        assert!(
            !plain.contains(pane),
            "{pane} is drawn on a live run that did not ask for it: {plain}"
        );
    }

    let mut last = String::new();
    let panes = live_report(&store, now(), &mut last, true, &AtConnect::default())
        .expect("every LIST landed");
    for pane in PANES {
        assert!(
            panes.contains(pane),
            "{pane} is missing from a live run: {panes}"
        );
    }
    assert!(
        panes.starts_with(&plain),
        "the cards moved when the panes were asked for — the reports go under them, exactly as \
         the file path prints them: {panes}"
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
        let printed = live_report(&store, now(), &mut last, true, &AtConnect::default())
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
        let printed = live_report(&store, now(), &mut last, true, &AtConnect::default())
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
    // A `--context` with nothing after it is the current context, not a crash.
    assert_eq!(live_context(&args(&["--live", "--context"])), Some(None));
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
    // An `=` says the value was meant, and `--context` with nothing after it is the current
    // context on purpose — neither is this mistake.
    assert_eq!(mistyped(&line(&["--live", "--context=--analysis"])), None);
    assert_eq!(mistyped(&line(&["--live", "--context"])), None);

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
    let failing = live_report(&store, now(), &mut last, false, &AtConnect::default())
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
        live_report(&store, now(), &mut last, false, &AtConnect::default()),
        None
    );

    // …and then every watch delivers a complete answer, which is what a reconnect looks like
    // from in here: the failure clears itself and the report says so without being asked.
    store.pod(&now(), Event::Init);
    store.pod(&now(), Event::InitDone);
    the_other_four(&mut store);
    let recovered = live_report(&store, now(), &mut last, false, &AtConnect::default())
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
    let stale = live_report(&store, now(), &mut last, false, &AtConnect::default())
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
    live_report(store, now(), &mut last, false, &AtConnect::default())
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
            }],
            renewal,
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
    };
    let degraded = unreadable(&[trouble(ObjectKind::Node, false)], None);
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

    let stopped = unreadable(&[trouble(ObjectKind::Pod, true)], None);
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

/// **Six faults, six sentences, and no two of them the same** — the second box's whole claim
/// (`PRIOR-ART § C1`), checked as a set rather than one at a time.
///
/// **A generic message may never stand in for an error we were handed.** The failure that rule
/// exists for is not a badly worded sentence; it is *one* sentence covering several errors, which
/// looks fine in every review and sends a reader to the wrong place at 3am. Two faults collapsing
/// into one string is what fails here, whichever two.
///
/// **Only two of the six use `asked`, and that is deliberate.** A kubeconfig that would not load
/// and a login helper that answered nothing happened before anything was asked of any cluster, so
/// a sentence naming a verb and a resource there would be inventing one.
#[test]
fn every_fault_gets_its_own_sentence_and_none_of_them_stands_in_for_another() {
    use k8s::Fault::{Expired, Gone, Kubeconfig, NoCredential, Refused, Unanswered};
    let all = [Kubeconfig, NoCredential, Expired, Refused, Gone, Unanswered];

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
    for renewal in [None, Some("aws")] {
        for asked in framings {
            let said: Vec<String> = all
                .iter()
                .map(|fault| because(*fault, asked, renewal))
                .collect();
            for line in &said {
                println!("{renewal:?}  {line}");
            }
            let distinct: std::collections::BTreeSet<&String> = said.iter().collect();
            assert_eq!(
                distinct.len(),
                all.len(),
                "two faults print the same sentence, which is the generic handler growing back: \
                 {said:#?}"
            );
            for line in &said {
                assert!(
                    !line.is_empty() && !line.contains("``"),
                    "a sentence is empty or carries an empty pair of backticks: {line:?}"
                );
            }
            // The three arms that read `asked` must actually contain it, whichever framing
            // arrives. That is the cheap half; the grid below is the half that catches a frame
            // that reads wrongly.
            for fault in [Refused, Gone, Unanswered] {
                let line = because(fault, asked, renewal);
                assert!(
                    line.contains(asked),
                    "`{fault:?}` dropped what k8rs was trying to do: {line:?}"
                );
            }
        }
    }

    // **The refusal names the verb and the resource** — the security gate's own words — and for
    // a `nonResourceURL` that means a path, because its `Status` carries no group and no kind
    // (NOTES § D160).
    assert_eq!(
        because(Refused, "`get /apis`", None),
        "the role this kubeconfig uses needs to `get /apis`"
    );
    // **And it never claims which verb is missing.** A watch is two verbs, and a `Role` granting
    // `list` without `watch` is ordinary — measured as printing *not allowed to `list` and
    // `watch` pods* while the LIST had just succeeded (`k8s-admin`, 2026-08-27).
    for asked in ["`get /apis`", "`list` and `watch` pods"] {
        let line = because(Refused, asked, None);
        assert!(
            !line.contains("not allowed"),
            "the refusal claims a state this code cannot know — which of two verbs was \
             refused: {line:?}"
        );
    }
    // **And the expiry is not a refusal.** Telling a beginner *you are not allowed* when their
    // login timed out sends them to their platform team for nothing (NOTES § D19).
    let expired = because(Expired, "`get /apis`", Some("aws"));
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
        let line = because(Expired, "`get /apis`", renewal);
        assert!(
            !line.contains("afresh") && !line.contains("restart") && !line.contains("start k8rs"),
            "the expired-login sentence tells the reader to restart, which is false for the \
             exec kubeconfig it was written for: {line:?}"
        );
    }
    assert!(
        !because(Expired, "`get /apis`", None).contains('`'),
        "a kubeconfig with no login program to name printed backticks around nothing"
    );
    // **The program is named where there is one and the sentence still works where there is
    // not.** Both shapes are ordinary: a static token in the file has no program behind it.
    assert!(because(NoCredential, "", Some("aws")).contains("(`aws`)"));
    assert!(!because(NoCredential, "", None).contains('`'));
}

/// **The three sentences that carry `asked`, in all four framings a caller can supply, written
/// out** — twelve literals, because nothing weaker can fail.
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
    use k8s::Fault::{Gone, Refused, Unanswered};
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
    ];
    for (fault, asked, expected) in grid {
        let line = because(fault, asked, None);
        println!("{line}");
        assert_eq!(
            line, expected,
            "`{fault:?}` has been reworded — read all four framings of it above before updating \
             this literal, because three of the four callers supply a verb phrase and one \
             supplies a path"
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
        scoped_because(&scoped(k8s::Coverage::Cluster)),
        None,
        "a run that reads the whole cluster explained a scope it does not have"
    );
    assert_eq!(
        scoped_because(&scoped(k8s::Coverage::Asked("payments".to_string()))),
        None,
        "a reader who typed --namespace was told what --namespace does"
    );

    let said = scoped_because(&scoped(k8s::Coverage::Refused("payments".to_string())))
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
    let blind = scoped_because(&scoped(k8s::Coverage::Blind("default".to_string())))
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

    // Invariant 9: the namespace came off argv or a kubeconfig, and neither is ours. Both arms
    // that print one, because a strip on one of two interpolations is a strip on neither.
    for crafted in [
        scoped_because(&scoped(k8s::Coverage::Refused(
            "pay\u{1b}[2Jments".to_string(),
        ))),
        scoped_because(&scoped(k8s::Coverage::Blind(
            "pay\u{1b}[2Jments".to_string(),
        ))),
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
    let unloadable = live(
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
    let entry = live(k8s::connect_with(yaml(moved), None, None).await, false).await;
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
    let broken = live(k8s::connect_with(yaml(&user), None, None).await, false).await;
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

    let stopped = live(Ok(session), false).await;

    assert!(
        stopped.contains("every watch has stopped"),
        "a driver whose watches all ended returned {stopped:?} instead of saying so"
    );
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
    let path = format!(
        "{}/tests/fixtures/certs/{name}.crt.pem",
        env!("CARGO_MANIFEST_DIR")
    );
    let pem =
        std::fs::read(&path).unwrap_or_else(|e| panic!("certificate {path} does not read: {e}"));
    let (_, block) = x509_parser::pem::parse_x509_pem(&pem)
        .unwrap_or_else(|e| panic!("{path} is not a PEM certificate: {e}"));
    block.contents
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
        &AtConnect {
            serving_expiry: expiry,
            ..Default::default()
        },
    )
    .expect("every LIST landed");
    assert!(
        live.ends_with(EXPIRING),
        "the certificate line is not last on the live path: {live}"
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
        file.ends_with(EXPIRING) && live.ends_with(EXPIRING),
        "the two renderers drew two different sentences, which is the defect class D177 named"
    );

    let mut last = String::new();
    let unread = live_report(&store, now(), &mut last, false, &AtConnect::default())
        .expect("every LIST landed");
    assert!(
        !unread.contains("A certificate the API server presented"),
        "a session that read nothing printed a sentence anyway: {unread}"
    );
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
    let said = live(Ok(session), false).await;
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

    let said = live(Ok(session), false).await;
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
