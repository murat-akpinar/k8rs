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
use crate::rules::ObjectKind;

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

/// **The positive half of invariant 9.** A crafted name, message or action reaches the
/// terminal through `println!` with no ratatui in between, so every string read off a
/// `Finding` passes `sanitize` first (`screens/once.md` § The rule that matters most here).
///
/// All five shapes at once: `ESC`, `CR`, `BEL`, a C1 control (`CSI`) and `DEL` — the whole of
/// Unicode's `Cc` category, which is what `char::is_control` answers.
///
/// **And both identity shapes**, because an identity is drawn by one of two arms and a guard
/// is proven only for the shapes it was fed (NOTES § D29): a namespaced object, whose
/// namespace *and* name are printed, and a cluster-scoped one, whose name is printed on its
/// own. A node is as nameable by an attacker as a pod — `kubectl label` is not needed, a
/// kubelet registers under the name it is given.
#[test]
fn no_control_character_from_a_finding_reaches_the_report() {
    let mut f = finding(Severity::Critical, pod_id("pay\rments", "web\u{9b}0"));
    f.title = "Escape \x1b[2J and bell \x07 here".to_string();
    f.evidence = "exit \x7f 137".to_string();
    f.action = "restart \u{85} it".to_string();
    f.timestamp = Some(four_minutes_ago());
    let cluster_scoped = finding(Severity::Warn, node_id("node\x1b[2J\u{9b}-3"));

    let report = render(&[f, cluster_scoped], &nothing_read());

    let survivors: Vec<char> = report
        .chars()
        .filter(|c| c.is_control() && *c != '\n')
        .collect();
    assert!(
        survivors.is_empty(),
        "control characters reached the report: {survivors:?}\n{report:?}"
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

/// **The negative half.** `char::is_control` is the `Cc` category and nothing wider: a Turkish
/// `ğ`, a CJK name and an en dash are ordinary text and come out byte-identical. Nothing here
/// is truncated either — we never cut a string ourselves (`screens/widgets.md` § 7).
#[test]
fn ordinary_and_multibyte_text_passes_through_whole() {
    let mut f = finding(Severity::Critical, pod_id("üretim", "日本語-0"));
    f.title = "Kapasite yetersiz — ğüşiİ".to_string();
    f.evidence = "limit 256Mi · exit 137".to_string();
    f.action = "limitleri artır".to_string();

    assert_eq!(
        render(&[f], &nothing_read()),
        "0 pods · 0 nodes · 0 workloads\n\
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
        "0 pods · 0 nodes · 0 workloads\n\
         \n\
         ▲ shop/api-7\n  \
           Something happened\n  \
           → do this about it\n\
         \n\
         1 warning"
    );
}

/// An evidence made **only** of control characters is the same case one step later: what
/// decides is what would be printed, not what the API sent.
#[test]
fn an_evidence_that_sanitizes_to_nothing_draws_no_line_either() {
    let mut f = finding(Severity::Warn, pod_id("shop", "api-7"));
    f.evidence = "\x07\x1b\r".to_string();

    assert!(
        !render(&[f], &nothing_read()).contains("\n  \n"),
        "a card was drawn with a blank line where its evidence would have been"
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
#[test]
fn nothing_broken_still_says_what_was_read() {
    let read = read(&["oom.json"]);

    assert_eq!(
        render(&[], &read),
        "1 pod · 0 nodes · 0 workloads\n\n○ nothing is broken"
    );
    assert_eq!(
        render(&[], &nothing_read()),
        "0 pods · 0 nodes · 0 workloads\n\n○ nothing is broken"
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

/// **A kind no rule reads is counted and named, never dropped in silence** — the header is
/// where a reader decides whether the report covered what they handed it. Both shapes: a list
/// of them, and a bare one.
#[test]
fn a_kind_no_rule_reads_is_counted_and_named() {
    let input = read(&["services.json", "csr-pending.json"]);

    assert_eq!(
        input.skipped.get("Service").copied(),
        Some(items_in("services.json"))
    );
    assert_eq!(
        input.skipped.get("CertificateSigningRequest").copied(),
        Some(1)
    );
    assert!(input.snapshot.pods.is_empty() && input.snapshot.nodes.is_empty());
    assert_eq!(
        header(&input),
        format!(
            "0 pods · 0 nodes · 0 workloads · {} objects no rule reads \
             (CertificateSigningRequest, Service)",
            items_in("services.json") + 1
        )
    );
}

/// An empty list is not an error and not a skipped kind: nothing was in it.
#[test]
fn an_empty_list_reads_as_nothing_at_all() {
    let input = read(&["persistentvolumeclaims.json"]);

    assert!(input.skipped.is_empty(), "{:?}", input.skipped);
    assert_eq!(
        render(&[], &input),
        "0 pods · 0 nodes · 0 workloads\n\n○ nothing is broken"
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
fn a_crafted_path_comes_back_out_of_the_error_with_no_control_characters() {
    // `ESC`, `CR` and a C1 control, the shapes the strip test already feeds a `Finding`.
    let crafted = fixture("no-such\x1b[2J\r\u{9b}fixture.json");

    let Err(problem) = load(std::slice::from_ref(&crafted), now()) else {
        panic!("a file that is not there is not a snapshot")
    };

    let survivors: Vec<char> = problem.chars().filter(|c| c.is_control()).collect();
    assert!(
        survivors.is_empty(),
        "control characters reached the error: {survivors:?}\n{problem:?}"
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
        "0 pods · 0 nodes · 0 workloads · 4 objects no rule reads ((no kind))"
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

/// **…and it is still three lines when it gets there.** `'\n'.is_control()` is `true`, so a
/// strip run over the *assembled* message instead of over the values that entered it eats
/// k8rs's own line breaks and prints the three sentences as one run-on line — with the two
/// spaces missing where the breaks were. That is what the first thing a new user ever sees
/// looked like until the strip moved to the interpolations ([`sanitize`]).
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
        Ok("1 pod · 0 nodes · 0 workloads\n\n○ nothing is broken".to_string())
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
