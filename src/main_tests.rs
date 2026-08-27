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
fn now() -> Time {
    Time("2026-08-16T12:00:00Z".parse().expect("a fixed timestamp"))
}

/// Four minutes before [`now`] — the `4 min ago` rung of the ladder
/// (`screens/widgets.md` § 1b).
fn four_minutes_ago() -> Time {
    Time("2026-08-16T11:56:00Z".parse().expect("a fixed timestamp"))
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

/// **Severity is the order** — the declaration order of [`Severity`] is severity order and the
/// derived `Ord` is what sorts this report (NOTES § D35). Handed the three bands backwards,
/// the report puts them back: `●` then `▲` then `○`.
#[test]
fn severity_orders_the_report_and_the_summary_names_all_three_bands() {
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
    assert_eq!(symbols, ["●", "▲", "○"], "{report:?}");
    assert!(
        report.ends_with("\n1 critical, 1 warning, 1 note"),
        "{report:?}"
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
    assert_eq!(summary(one(Severity::Info)), "1 note");
    assert_eq!(summary(two(Severity::Info)), "2 notes");
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
        live_report(&k8s::Store::default(), now(), &mut last, None),
        None
    );

    // Four of the five landed and the fifth never opened: still not a cluster anyone may read.
    let mut store = k8s::Store::default();
    the_other_four(&mut store);
    assert_eq!(live_report(&store, now(), &mut last, None), None);
    assert!(
        last.is_empty(),
        "something was recorded as printed while the bootstrap was still running"
    );

    // **And still nothing after something else has been printed.** `last` is what the driver said
    // most recently, so a silent bootstrap has to stay silent *against a non-empty last* too —
    // an empty report is not a report, and printing one would put a blank block on stdout every
    // time a watch re-listed.
    let printed = live_report(&listed(Vec::new()), now(), &mut last, None).expect("a listed store");
    assert!(!printed.is_empty(), "the report is empty: {printed:?}");
    // `None` and not merely *empty*: `Some(String::new())` is a blank block on stdout, which is
    // what the driver would print every time a watch re-listed.
    assert_eq!(
        live_report(&store, now(), &mut last, None),
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

    let first = live_report(&store, now(), &mut last, None).expect("every initial LIST landed");
    println!("{first}");
    assert!(
        first.contains(" pods · "),
        "the live report is not the report `render` draws"
    );
    assert_eq!(
        live_report(&store, now(), &mut last, None),
        None,
        "the same cluster printed twice"
    );

    let crashloop: Pod = serde_json::from_str(
        &std::fs::read_to_string(fixture("crashloop.json")).expect("the fixture reads"),
    )
    .expect("the capture decodes");
    store.pod(&now(), Event::Apply(crashloop));
    let second =
        live_report(&store, now(), &mut last, None).expect("a pod arrived, so the report moved");
    println!("{second}");
    assert!(
        second.contains("broken-crashloop"),
        "a pod that arrived after the bootstrap never reached the report"
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
    let watches = k8s::session(offline())
        .await
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();
    let mut store = k8s::Store::default();
    k8s::drive_watching(watches, &mut store, |_| {}).await;

    let mut last = String::new();
    let failing = live_report(&store, now(), &mut last, None).expect("five watches are failing");
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
    assert_eq!(live_report(&store, now(), &mut last, None), None);

    // …and then every watch delivers a complete answer, which is what a reconnect looks like
    // from in here: the failure clears itself and the report says so without being asked.
    store.pod(&now(), Event::Init);
    store.pod(&now(), Event::InitDone);
    the_other_four(&mut store);
    let recovered = live_report(&store, now(), &mut last, None).expect("the cluster came back");
    println!("{recovered}");
    assert!(
        !recovered.contains("not getting"),
        "the driver still says the cluster is unreadable after every watch delivered: {recovered}"
    );
    assert!(
        recovered.starts_with("0 pods · 0 nodes"),
        "a healthy report starts with something other than the report: {recovered:?}"
    );

    // **And the outage the proof actually watches: one that arrives *after* a good bootstrap.**
    // The store keeps its last complete answer while a watch is down (NOTES § D162), so this is
    // the only shape where both halves are printed at once — the lines on top, the cards they
    // are a warning about underneath, one blank line between.
    let watches = k8s::session(offline())
        .await
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();
    k8s::drive_watching(watches, &mut store, |_| {}).await;
    let stale = live_report(&store, now(), &mut last, None).expect("an outage is news");
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
    let unloadable = live(k8s::connect_with(yaml("{}"), Some("k8rs-tests-no-such-context")).await);
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
    let entry = live(k8s::connect_with(yaml(moved), None).await).await;
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
    let broken = live(k8s::connect_with(yaml(&user), None).await).await;
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
    let mut session = k8s::session(offline()).await;
    session.watches = session
        .watches
        .into_iter()
        .map(|watch| watch.take(2).boxed())
        .collect();

    let stopped = live(Ok(session)).await;

    assert!(
        stopped.contains("every watch has stopped"),
        "a driver whose watches all ended returned {stopped:?} instead of saying so"
    );
}
