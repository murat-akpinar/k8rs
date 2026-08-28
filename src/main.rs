//! k8rs — the temporary driver, and the first code that shows a `Finding`.
//!
//! It reads Kubernetes objects out of JSON files named on the command line, runs the rule
//! engine over them and prints what is broken. **It cannot reach a cluster**: `k8s.rs` is
//! Phase 5, which is where `--once` and the v0.0.1 release therefore sit. Until then this is
//! how the rules are exercised for real (CLAUDE.md § Running it).
//!
//! The output is `screens/once.md`'s card, minus the two things that need a later phase —
//! owner grouping and its `3 of 5 pods` count, and recency as a second sort key (Phase 10).
//!
//! **The third divergence is gone: the `Info` band is not drawn in the card block and not counted
//! in the tally.** D121 added it because *a driver whose whole job is to show what `analyze`
//! returned may not drop one of them*, and that held for as long as no `Info` could be produced —
//! the file path hard-codes all three of their inputs to `None`. The first live run that filled
//! them printed C1 twice, once as a card and once as the pane row, and NOTES § D87 is explicit
//! that `Severity::Info` *means* a report rather than Alerts. So `analyze`'s `Info` findings are
//! not dropped, they are drawn where D87 sends them — the panes under `--analysis` ([`reports`]).
//! It may not invent a third format: one `rules.rs`, one set of strings.
//!
//! **`--analysis` prints `analysis.rs`'s seven reports under the cards**, which is what makes them
//! runnable at all before `views.rs` exists: until this flag nothing outside `#[cfg(test)]` had
//! ever rendered a `Report`, so invariant 9's strip was unexercised for every string in all of
//! them ([`pane`]). The five lists those reports join — Services, EndpointSlices, PVCs,
//! PodDisruptionBudgets and CertificateSigningRequests — are read here from whatever files are
//! named, so a pane's *not checked* state is still reachable by simply not naming one ([`take`]).
//!
//! **It is honoured beside `--live` too, and that is the door three of the seven had no way
//! through** (NOTES § D169). Their principal shapes are about things a `k8rs pod.json` run does
//! not have: Versions needs a control plane to have a version, C1's row and sidebar badge are
//! about the reader's kubeconfig, and Capacity's `using …` paragraphs need a metrics API — the
//! first two arrive with this flag, the third when the metrics box fills its field. Both modes
//! call [`reports`], so there is one arrangement of the seven and not two.

// A module no `mod` line reaches is not in the crate at all, so `rules.rs` is declared the
// moment it exists rather than when something calls it (NOTES § D34).
mod analysis;
mod k8s;
mod rules;

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;

use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::certificates::v1::CertificateSigningRequest;
use k8s_openapi::api::core::v1::{Node, PersistentVolumeClaim, Pod, Service};
use k8s_openapi::api::discovery::v1::EndpointSlice;
use k8s_openapi::api::policy::v1::PodDisruptionBudget;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
use k8s_openapi::jiff::{SignedDuration, Timestamp};
use k8s_openapi::serde::de::DeserializeOwned;
use k8s_openapi::serde_json::{self, Value};
use rules::{ClusterSnapshot, Finding, ObjectId, ObjectKind, Severity, analyze};
use std::collections::BTreeMap;

/// **stdout is the findings, stderr is everything else** (`screens/once.md` § stdout and
/// stderr are split on purpose), and the two exit codes are `0` and `2` — never `1`, which is
/// reserved so a future `--exit-code` has somewhere to go (NOTES § D17).
///
/// Every decision is in a function over values that is tested — [`run`] for what to report,
/// [`live_context`] for which of the two this run is, [`live`] for what a cluster prints,
/// [`stdout_failure`] for what a failed write costs, [`runtime_failure`] for what a runtime that
/// would not start says — and what is left here is argv, the choice of stream, starting the
/// runtime, and calling `exit`.
///
/// **`runtime_failure` joined that list on 2026-08-27** and the sentence above is why: its arm was
/// spelled inline here, so nothing could reach it, and it was throwing an io error away with a
/// `_` while the line beside it named one (`tester`).
fn main() {
    use std::io::Write;
    let args: Vec<String> = std::env::args().skip(1).collect();
    // **Before the mode is chosen, because a mistyped flag is wrong in both of them.**
    let problem = match mistyped(&args) {
        Some(sentence) => sentence,
        None => match live_context(&args) {
            // **The live run has no happy ending to return**: it prints as it goes and comes back
            // only with the sentence that says why it stopped (§ WATCHING A CLUSTER).
            Some(context) => match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime.block_on(async {
                    live(k8s::connect(context).await, analysis_wanted(&args)).await
                }),
                Err(failed) => runtime_failure(&failed),
            },
            None => match run(&args) {
                // `writeln!`, never `println!`: Rust masks `SIGPIPE`, so `println!`
                // *panics* when the write fails, and a reader that closed the pipe is
                // `head` doing its job. That printed a backtrace and exited 101 — a code
                // D17's table does not have ([`stdout_failure`]).
                Ok(report) => match writeln!(std::io::stdout(), "{report}") {
                    Ok(()) => return,
                    Err(failed) => match stdout_failure(&failed) {
                        Some(sentence) => sentence,
                        None => return,
                    },
                },
                // Printed as it was handed over. Everything in it that came from outside was
                // stripped where it entered the sentence, and everything else is ours
                // ([`sanitize`]).
                Err(problem) => problem,
            },
        },
    };
    // **A write to stderr that fails is dropped, here and nowhere else**: there is no third
    // stream to report it on, and a program that panics while saying why it is unhappy has
    // replaced one bad ending with a worse one.
    let _ = writeln!(std::io::stderr(), "{problem}");
    std::process::exit(2);
}

/// The sentence a failed write to **stdout** costs, or `None` when it costs nothing.
///
/// **The two cases are not the same failure and are deliberately not collapsed.** `BrokenPipe`
/// is the reader closing the pipe — `head`, or `less` quit on the first page, which
/// `screens/once.md` § Colour and symbols sells as a way to read this report — so it is the
/// pipeline working: exit `0`, silently. Anything else is the report arriving cut in half,
/// `k8rs > findings.txt` onto a full disk, and a truncated report that claims success is worse
/// than the panic this replaced: exit `2`, with a sentence on stderr (NOTES § D17).
fn stdout_failure(error: &std::io::Error) -> Option<String> {
    match error.kind() {
        std::io::ErrorKind::BrokenPipe => None,
        // The reason is the standard library's string, which is outside text like any other
        // ([`sanitize`]).
        _ => Some(format!(
            "k8rs: the report could not be written — {}",
            sanitize(&error.to_string())
        )),
    }
}

/// **What a runtime that would not start says**, naming the reason the operating system gave.
///
/// **A function because it is a decision, which is the same reason [`stdout_failure`] is one**
/// (`main`'s own doc): what is left in `main` is argv, the choice of stream, the runtime a cluster
/// needs and calling `exit`. Inline, this arm was unreachable from any test — and it was throwing
/// an identical `std::io::Error` away with a `_` while its neighbour named one
/// (`tester`, 2026-08-27), which is the same *generic string over a typed error* the cluster path
/// was just rid of.
///
/// **The reason is the standard library's string, so it is outside text like any other**
/// ([`sanitize`]) — `EMFILE` here reads *too many open files*, which is a thing a reader can act
/// on, where *would not start* alone is not.
fn runtime_failure(error: &std::io::Error) -> String {
    format!(
        "k8rs: this machine would not start the runtime a cluster needs — {}",
        sanitize(&error.to_string())
    )
}

/// The three lines a run with no arguments gets. **Three, and `tests/binary.rs` counts them**:
/// the file-driven form, the live one, and what the first of them still cannot do — a usage that
/// named only half the binary would be the driver lying about itself.
///
/// **The synopsis is one printed line written across two source lines**, and the `\` that joins
/// them keeps the three spaces before it: `scripts/width-guard.py` refuses a source line past 100
/// columns and `cargo fmt` will not wrap a string literal — it pulls the whole `const` back onto
/// one line however this is indented, so the break has to be inside the literal.
const USAGE: &str = "usage: k8rs [--analysis] <file.json>...   |   \
     k8rs --live [--analysis] [--context <name>]\n\
     Each file holds Kubernetes objects as JSON: one object, or a list of them.\n\
     Without --live this build reads files only — it cannot reach a cluster.";

/// **The one flag this driver has**, and it is scaffolding like the driver itself: `analysis.rs`'s
/// seven reports are whole-cluster answers rather than per-object cards, so they are a second
/// report under the first rather than more findings in it. Phase 9 draws them in panes and this
/// goes away with the rest of the temporary main (NOTES § D34).
///
/// **A flag and not the default**, because the default output is what `tests/binary.rs` pins as
/// the report on stdout — and because a driver that printed seven panes for every `k8rs pod.json`
/// would bury the cards it exists to show.
///
/// **One meaning in both modes** (NOTES § D169): it was accepted and ignored beside `--live`
/// until the reports that need a cluster had nowhere else to be drawn — Versions has a
/// control-plane version only when there is a control plane, and C1's row and badge are about a
/// kubeconfig no file path has. A flag that is honoured in one mode and silently dropped in the
/// other is the second rule this driver would then have.
const ANALYSIS: &str = "--analysis";

/// **Whether the seven panes were asked for**, read the same way for both modes because it is
/// one flag ([`ANALYSIS`]).
fn analysis_wanted(args: &[String]) -> bool {
    args.iter().any(|arg| arg == ANALYSIS)
}

/// The report, or the sentence that goes to stderr instead. `Err` is the whole of exit 2.
fn run(args: &[String]) -> Result<String, String> {
    let wanted = analysis_wanted(args);
    // Everything that is not a flag is a path. A word that *looks* like a flag and is not one
    // never gets here — [`mistyped`] refuses it for both modes at once — and a file really named
    // `--analysis` is not a shape this scaffolding owes an escape hatch to.
    let paths: Vec<String> = args
        .iter()
        .filter(|arg| *arg != ANALYSIS)
        .cloned()
        .collect();
    if paths.is_empty() {
        return Err(USAGE.to_string());
    }
    // The clock is read once, here, and handed to the rules as a field — never called from
    // inside one (invariant 5, NOTES § D18). Phase 5's `k8s.rs` reads it in the same place.
    let input = wall_clock()
        .and_then(|now| load(&paths, now))
        .map_err(|problem| format!("k8rs: {problem}"))?;
    let findings = analyze(&input.snapshot);
    let mut out = render(&findings, &input);
    if wanted {
        out.push('\n');
        out.push_str(&reports(&input.snapshot, &findings));
    }
    Ok(out)
}

/// **The clock the report is read against**, called once per report and handed on as a value —
/// never from inside a rule (invariant 5, NOTES § D18).
///
/// **It is no longer the only clock call in the program, and that is deliberate.** `k8s.rs`'s
/// `local_clock` reads the same machine at the moment the API server's `Date` header comes back,
/// because a skew is the gap between two clocks at *one* instant and a value sampled here would
/// be sampled minutes away from the answer it is compared against ([`k8s::Session::skew`]). The
/// two share two library calls and nothing else: this one owes the reader a plain-language
/// sentence per failure, that one owes nobody anything, because a clock that will not read is a
/// skew that was not measured.
///
/// `jiff` arrives through `k8s-openapi` with `default-features = false`, so `Timestamp::now`
/// — which is behind `std` — does not exist here. The seconds come off `SystemTime` instead;
/// no new dependency, and none is wanted for a subtraction (invariant 10).
fn wall_clock() -> Result<Time, String> {
    let since_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        // Neither arm invents a moment. A snapshot whose `now` is the epoch would age every
        // finding by 56 years, which is the failure invariant 5 exists to prevent.
        .map_err(|_| "this machine's clock is set before 1970".to_string())?;
    Timestamp::new(
        since_epoch.as_secs() as i64,
        since_epoch.subsec_nanos() as i32,
    )
    .map(Time)
    .map_err(|e| {
        format!(
            "this machine's clock is not a usable time — {}",
            sanitize(&e.to_string())
        )
    })
}

/// **Strip the characters that have no printed form out of a string that came from outside
/// this file** — the guard invariant 9 owes every printer, and this is the first one
/// (`screens/widgets.md` § 7).
///
/// `println!` has no ratatui between it and the terminal, so an escape sequence in a pod name
/// arrives as an escape sequence and rewrites the user's screen.
///
/// **What counts as such a character is [`k8s::unprintable`]'s answer and is not restated
/// here** (NOTES § D154, CLAUDE.md § Single point of change). This file carried its own
/// narrower spelling until 2026-08-22, and the day the ingest guard widened and this one did
/// not, `k8rs some-pod.json` printed a row that reads *prodcd* for a pod named
/// `prod\u{202e}dc` — the hole is this path's alone, because it builds its snapshot off
/// `rules.rs`'s `From` impls and never meets [`k8s::Store`]. **A second spelling is what the
/// fix refuses**: the two files are modules of one crate, so this one calls the predicate.
///
/// **Removed, not replaced.** "Stripped" is the word in both invariant 9 and § 7, and a
/// substituted space is a character the API did not send — a second lie in the record
/// invariant 4 says may not lie. **Nothing is truncated here either**: the multi-byte path is
/// where `String::truncate` panics, and § 7 forbids it outright.
///
/// **Where it is applied is the whole rule, and it is mechanical: on a value as it enters a
/// message, never on the finished message.** Every fragment that came from outside passes
/// through it at the `format!` that interpolates it — a [`Finding`]'s fields, a path off argv,
/// an error string from `serde_json`, the standard library or `jiff`. Every literal in this
/// file is ours and stays whole, which is what the other half buys: a line break is
/// unprintable by the predicate above, so a strip over the assembled message ate [`USAGE`]'s
/// own line breaks and printed three sentences as one. A `\n` *from the cluster* still dies
/// here, and must — it would forge a second card. Phase 5's ingest strip supersedes this by
/// applying the same rule one layer earlier, cleaning the text as it arrives.
fn sanitize(text: &str) -> String {
    text.chars().filter(|c| !k8s::unprintable(*c)).collect()
}

// --- WHAT WAS READ START ---

/// One run's input: the snapshot the rules get, and what was read but handed to no rule.
///
/// The second half is here because *read nothing* and *found nothing* must not print the same
/// line. Handed `tests/fixtures/*.json` this reads Services and a CertificateSigningRequest
/// among the pods; each is counted and named in the header rather than dropped where nobody
/// can see it.
struct Input {
    snapshot: ClusterSnapshot,
    /// Kinds no rule in `rules.rs` reads, counted by kind. Sorted, so the header is the same
    /// text twice for the same input.
    skipped: BTreeMap<String, usize>,
    /// **How far this machine's clock is from the cluster's, when there is something to say** —
    /// [`k8s::Session::skew`], and [`clock`] is what spells it.
    ///
    /// **Always `None` on the file-driven path, and that is the honest answer rather than a gap.**
    /// A `.json` on disk has no API server to have answered with a `Date`, so there is no evidence
    /// either way — which is the same silence a live cluster whose header was stripped produces
    /// (`screens/states.md` § When there is nothing to say).
    skew: Option<SignedDuration>,
}

/// Read every path into one snapshot, or say which file stopped it.
///
/// A document carrying an `items` array is a `kind: List` — `kubectl get -A`'s answer — and
/// each item is dispatched on its own `kind`; anything else is dispatched whole. `Err` is the
/// exit-2 path (NOTES § D17): a file that will not read, will not parse, or holds an object of
/// a kind we claim to understand and does not decode.
fn load(paths: &[String], now: Time) -> Result<Input, String> {
    let mut input = Input {
        // No cluster answered, so nothing measured this machine's clock ([`Input::skew`]).
        skew: None,
        snapshot: ClusterSnapshot {
            now,
            pods: Vec::new(),
            nodes: Vec::new(),
            workloads: Vec::new(),
            // A fixture path is not a kubeconfig: there is no server to ask for a version, no
            // context name and no client certificate, so C1 and N4 correctly say nothing.
            server_version: None,
            context: None,
            client_certificate: None,
            // Every namespace, as far as this input knows — the files are what they are.
            namespace_scope: None,
            // **`None` and not `Some(vec![])`, and the difference is the whole point of the
            // `Option`** (NOTES § D129): *nobody looked* is what has happened until a file
            // holding one of these kinds is named, and an empty `Vec` would tell a report that
            // nothing is wasted and nothing is waiting to join, which is the reassuring wrong
            // answer. [`take`] turns each of them into a `Some` the moment it reads one; no
            // rule in `rules.rs` reads any of these, and on a real cluster the Phase 5 fetch
            // fills them when a pane opens.
            replica_sets: None,
            services: None,
            endpoint_slices: None,
            claims: None,
            disruption_budgets: None,
            certificate_requests: None,
            // Same `None`, one field over and for the same reason: a fixture path has no
            // metrics API to probe, so *k8rs did not ask* is what happened.
            metrics: None,
        },
        skipped: BTreeMap::new(),
    };
    for path in paths {
        // A path is argv, an `io::Error` names the file back, and a `serde_json::Error` quotes
        // the input it choked on: all three are outside text and are stripped as they enter the
        // sentence ([`sanitize`]). `take`'s error was already assembled under that rule.
        let named = sanitize(path);
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("{named}: {}", sanitize(&e.to_string())))?;
        let doc: Value = serde_json::from_str(&text)
            .map_err(|e| format!("{named}: not JSON — {}", sanitize(&e.to_string())))?;
        let docs = match doc.get("items").and_then(Value::as_array).cloned() {
            Some(items) => items,
            None => vec![doc],
        };
        for doc in docs {
            take(doc, &mut input).map_err(|e| format!("{named}: {e}"))?;
        }
    }
    Ok(input)
}

/// File one object under the snapshot field its kind belongs to, or count it as unread.
///
/// **`(no kind)` is the label both shapes get**, because one lookup answers both and the
/// reader's question — *what kind was this?* — has the same answer either way: a document with
/// no `kind` field, and one whose `kind` is not text (`{"kind":42}`, a top-level array, a bare
/// `null`). Saying *no kind field* over a document that has one is the same lie, one size down,
/// as a report that misnames what it read.
///
/// **The five on-demand lists arrive here too, and `None` still means *nobody looked***
/// (NOTES § D129). A cluster answers them on a fetch when a report's pane opens; this driver
/// answers them from whatever files were named, so the field stays `None` until one such object
/// is read and becomes `Some` — an empty `Vec` only where a capture held a `kind: List` with
/// nothing in it. That is the same distinction the fetch draws, and it is what lets Waste say
/// *nothing is going to waste* over the lists it read and *not checked* over the ones it did not.
///
/// **A ReplicaSet lands in two fields on purpose.** `workloads` is the permanent watch's, which
/// the W-rules read; `replica_sets` is the list Waste's *parked at 0 replicas* row is counted
/// from ([`crate::rules::ClusterSnapshot::replica_sets`]). One object read once fills both,
/// because on a real cluster both would hold it.
fn take(doc: Value, input: &mut Input) -> Result<(), String> {
    let snapshot = &mut input.snapshot;
    let kind = doc
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("(no kind)")
        .to_string();
    match kind.as_str() {
        "Pod" => snapshot.pods.push(decode::<Pod>(doc, &kind)?.into()),
        "Node" => snapshot.nodes.push(decode::<Node>(doc, &kind)?.into()),
        "Deployment" => snapshot
            .workloads
            .push(decode::<Deployment>(doc, &kind)?.into()),
        "StatefulSet" => snapshot
            .workloads
            .push(decode::<StatefulSet>(doc, &kind)?.into()),
        "ReplicaSet" => {
            let set: rules::WorkloadSnapshot = decode::<ReplicaSet>(doc, &kind)?.into();
            snapshot
                .replica_sets
                .get_or_insert_with(Vec::new)
                .push(set.clone());
            snapshot.workloads.push(set);
        }
        "DaemonSet" => snapshot
            .workloads
            .push(decode::<DaemonSet>(doc, &kind)?.into()),
        "Service" => snapshot
            .services
            .get_or_insert_with(Vec::new)
            .push(decode::<Service>(doc, &kind)?.into()),
        "EndpointSlice" => snapshot
            .endpoint_slices
            .get_or_insert_with(Vec::new)
            .push(decode::<EndpointSlice>(doc, &kind)?.into()),
        "PersistentVolumeClaim" => snapshot
            .claims
            .get_or_insert_with(Vec::new)
            .push(decode::<PersistentVolumeClaim>(doc, &kind)?.into()),
        "PodDisruptionBudget" => snapshot
            .disruption_budgets
            .get_or_insert_with(Vec::new)
            .push(decode::<PodDisruptionBudget>(doc, &kind)?.into()),
        "CertificateSigningRequest" => snapshot
            .certificate_requests
            .get_or_insert_with(Vec::new)
            .push(decode::<CertificateSigningRequest>(doc, &kind)?.into()),
        _ => *input.skipped.entry(kind).or_default() += 1,
    }
    Ok(())
}

/// The `kind` is the document's own string and the error quotes the document's own values back,
/// so both are stripped here rather than at the printer ([`sanitize`]).
///
/// **Neither strip can be made to fire by an input today, and both stay**: the match above
/// only reaches this with one of its six literals, and `serde_json` `Debug`-escapes the values
/// it quotes (`invalid type: string "web\u{1b}[2J"` is seven printable characters, not an
/// escape) — while refusing a raw control character inside a JSON string in the first place.
/// The rule is mechanical, not a per-field judgement call; a seventh match arm or a different
/// deserializer is exactly how the judgement call gets made wrong.
fn decode<T: DeserializeOwned>(doc: Value, kind: &str) -> Result<T, String> {
    serde_json::from_value(doc).map_err(|e| {
        format!(
            "a {} did not decode — {}",
            sanitize(kind),
            sanitize(&e.to_string())
        )
    })
}

// --- WHAT WAS READ END ---

// --- THE REPORT START ---

/// The whole report as one string: what was read, the cards, what was found.
///
/// **Severity and nothing else orders it** (NOTES § D35 — the declaration order *is* the
/// order). `sort_by_key` is stable, so `analyze`'s own order survives inside a band; recency
/// is Phase 10's second key and is deliberately not here.
fn render(findings: &[Finding], input: &Input) -> String {
    let mut lines = vec![header(input), String::new()];
    // **The `Info` band is not drawn here, because this block is Alerts** (NOTES § D87, § D2):
    // `Severity::Info` on a rule already *means* this finding lives in a report rather than in
    // Alerts, which is how N4 and N5 use it and how C1's expiring band reaches the Certificates
    // pane at all. Filtering once, above the `is_empty` check and above [`tally`], is what stops
    // the count and the cards disagreeing — the tally counted the band only because the cards
    // were drawn, and both follow this one line (NOTES § D121's third divergence, now closed).
    //
    // **It was unreachable until this box.** The file path hard-codes C1's two inputs and the
    // control-plane version to `None`, so nothing outside a live run could return an `Info` at
    // all; the first live run with the fields filled printed C1 twice, once as a card above the
    // tally and once as the pane row (`k8s-admin`, 2026-08-28).
    let mut order: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.severity != Severity::Info)
        .collect();
    if order.is_empty() {
        lines.push("○ nothing is broken".to_string());
    } else {
        order.sort_by_key(|f| f.severity);
        for finding in &order {
            lines.push(card(finding, &input.snapshot.now));
            lines.push(String::new());
        }
        lines.push(tally(&order));
    }
    // **Last, after the findings, and on both paths through the block above**
    // (`screens/once.md` § When your clock and the cluster's disagree): it is *how much of this
    // report can you trust*, read at the moment the reader is deciding whether to look away, and
    // `○ nothing is broken` is as much a claim about times as a card is. The early return this
    // replaced is why it is an `else` and not a second exit.
    //
    // **Last *of this block*, which puts it above `--analysis`'s panes rather than under them**
    // (2026-08-28, a choice `screens/once.md` does not make because it draws no panes). What the
    // sentence qualifies is the times on the cards, and it belongs against them rather than at
    // the bottom of seven whole-cluster reports the reader may never scroll to.
    if let Some(clock) = clock(input.skew) {
        lines.push(String::new());
        lines.push(clock);
    }
    lines.join("\n")
}

/// **The one sentence that says the times in this report cannot be trusted**, or `None` when
/// there is nothing to say.
///
/// **Two directions, two sentences, and they are drawn rather than composed here**
/// (`screens/states.md` § Two directions, two sentences, because they break differently). Both are
/// `screens/once.md` § When your clock and the cluster's disagree verbatim, re-wrapped.
///
/// **Neither names a culprit, and the pair this replaced did** (NOTES § D177). What k8rs measures
/// is a *difference between two clocks*; *"your computer's clock is 11 minutes behind"* is a
/// verdict about whose is wrong, and with a middlebox thirty minutes fast between two clocks that
/// agreed to the second, it sent the reader to fix a correct laptop. The sentence states the gap
/// and the direction and stops there.
///
/// **And the behind half does not only blank.** With this machine behind by `S`, an event of true
/// age `A` gives `elapsed = A - S`, so `rules::age` refuses only while `A < S - 5min` and
/// everything older prints a number the whole gap too small — 16 of 32 cards carried an age under
/// the old *"times are blank"* sentence, and a twenty-minute-old crash read `9 min ago`. So the
/// behind sentence names both failures and the ahead one names the single failure it has.
///
/// **No `⚠`.** This report's vocabulary is `● ▲ ○` and nothing else; the console's pointer borrows
/// a glyph from its `⚠ disconnected` family, and this stream has never used that family
/// (`screens/once.md` § Colour and symbols).
///
/// **It does not touch the exit code.** A clock being off is a fact about the data, not a failure
/// to run (`screens/once.md` § Exit codes).
///
/// **The threshold is not re-checked here.** `Some` already means *past five minutes, in one
/// direction or the other* — [`k8s::Session::skew`] is where that is decided, once, for every
/// renderer this field will ever have.
///
/// **Whole minutes, rounded to the nearest — deliberately *not* `rules::age`'s floor.** That
/// ladder floors because elapsed time genuinely does: an event four and a half minutes old *has*
/// been four minutes. A gap between two clocks has not. A `Date` header carries whole seconds and
/// is stamped before the response is read, so a true offset of exactly 1800 s arrives here as
/// 1799-and-a-bit: floored, the built binary printed **29 minutes**
/// (`reports/2026-08-28-clock-skew-date-header.md` § 4) while the reader's next command,
/// `chronyc tracking`, says 30.0. Two numbers disagreeing at 3am is a minute of doubt this line
/// exists to remove.
///
/// **The floor of the count is 5 and it can never be `1`.** `Some` starts strictly past five
/// minutes, so the smallest reading is 301 s, which rounds to 5 — [`plural`] therefore never draws
/// the singular here, and is called anyway rather than hard-coding an `s`, because one place
/// spells a count in this file.
///
/// **The direction comes off the duration and the magnitude off `|seconds|`**, so the rounding
/// never touches a sign and a `Some(0)` this contract forbids still prints rather than panicking.
/// The cast is exact on every target this repo builds for —
/// CI's four and the gnu host beside them are all 64-bit — and on a 32-bit one it could only
/// truncate a reading no server can produce.
fn clock(skew: Option<SignedDuration>) -> Option<String> {
    let skew = skew?;
    // **The magnitude first, so the rounding never has a sign to carry.** Rounding half up over
    // `|seconds|` is the same arithmetic in both directions, in integers — no float in a number a
    // reader will compare against `chronyc`. The first draft multiplied by `signum` instead, and
    // the mutation gate caught what that costs: `30 * signum` and `30 / signum` agree on every
    // reachable input, so the test could not tell them apart — and the division panics on a skew
    // of zero, which is a shape this function's contract forbids but its signature allows.
    let seconds = skew.as_secs().unsigned_abs();
    let count = plural(((seconds + 30) / 60) as usize, "minute");
    Some(if skew.is_negative() {
        // This machine is behind: `rules::age` blanks what is younger than the gap and prints
        // everything older short by it, so the sentence names both.
        format!(
            "This computer and the cluster disagree about the time by {count} (this one is \
             behind), so recent times are missing and older ones can read smaller than they \
             really are."
        )
    } else {
        format!(
            "This computer and the cluster disagree about the time by {count} (this one is \
             ahead), so times can read larger than they really are."
        )
    })
}

/// What the report covered — the first line, so an empty report cannot be mistaken for a
/// clean cluster (`screens/once.md` § When nothing is broken).
///
/// **No workload count, and its removal narrows NOTES § D121 rather than reversing it.** D121
/// added two things for one purpose — *read nothing* and *found nothing* may not print the same
/// line — and the second, `N objects no rule reads (Kind, Kind)`, does that job better than the
/// count ever did, because it names what was read and not understood. What the count cost was a
/// noun: it said `16 workloads` for the controller objects read while Capacity's row says
/// `34 workloads` for the pod owners with no limit, and `34 of 16` is not defensible to a
/// reader. `workload` now means one thing in this product — one distinct owner identity
/// ([`crate::rules::ObjectId::group_key`]) — and it is said in one place, Capacity's row.
fn header(input: &Input) -> String {
    let snapshot = &input.snapshot;
    let mut parts = vec![
        plural(snapshot.pods.len(), "pod"),
        plural(snapshot.nodes.len(), "node"),
    ];
    if !input.skipped.is_empty() {
        let kinds: Vec<String> = input.skipped.keys().map(|k| sanitize(k)).collect();
        parts.push(format!(
            "{} no rule reads ({})",
            plural(input.skipped.values().sum(), "object"),
            kinds.join(", ")
        ));
    }
    parts.join(" · ")
}

/// One finding, four lines at most: what it is about · what happened · what proves it · what
/// to do. The evidence line is **left out** when there is none, never drawn blank
/// ([`Finding::evidence`]), and the age suffix is left out the same way when the finding
/// carries no moment ([`Finding::age`]).
fn card(finding: &Finding, now: &Time) -> String {
    let age = match finding.age(now) {
        Some(age) => format!(" · {}", sanitize(&age)),
        None => String::new(),
    };
    let mut lines = vec![
        format!(
            "{} {}{age}",
            symbol(finding.severity),
            name(&finding.object)
        ),
        format!("  {}", sanitize(&finding.title)),
    ];
    // The sanitized value decides, not the raw one: an evidence made only of control
    // characters would otherwise draw the blank line [`Finding::evidence`] forbids.
    let evidence = sanitize(&finding.evidence);
    if !evidence.is_empty() {
        lines.push(format!("  {evidence}"));
    }
    lines.push(format!("  → {}", sanitize(&finding.action)));
    lines.join("\n")
}

/// `● ▲ ○` — the severity, carried by the symbol alone so the report survives `| less`, a CI
/// log and a reader who does not tell red from green (`screens/once.md` § Colour and symbols).
fn symbol(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "●",
        Severity::Warn => "▲",
        Severity::Info => "○",
    }
}

/// `payments/web`, or a bare name for a cluster-scoped object — `None` is not `""`, which
/// would draw as `/node-3` ([`ObjectId::namespace`]).
fn name(id: &ObjectId) -> String {
    match &id.namespace {
        Some(namespace) => format!("{}/{}", sanitize(namespace), sanitize(&id.name)),
        None => sanitize(&id.name),
    }
}

/// `1 critical, 2 warnings` — only the bands that have something in them, so a report never
/// claims a count it did not find. Called only with a non-empty list, so it is never empty.
fn tally(findings: &[&Finding]) -> String {
    let count = |wanted: Severity| findings.iter().filter(|f| f.severity == wanted).count();
    let mut parts = Vec::new();
    let critical = count(Severity::Critical);
    if critical > 0 {
        // "critical" is the adjective the screen prints, and it does not take an `s`.
        parts.push(format!("{critical} critical"));
    }
    let warnings = count(Severity::Warn);
    if warnings > 0 {
        parts.push(plural(warnings, "warning"));
    }
    // **No `note` count, because there is no `Info` card above it to count** — [`render`] filters
    // the band out of this block entirely (NOTES § D87). Counting a band whose cards are not drawn
    // is the half-way house the other way round: a summary naming something the reader can see no
    // evidence for. `screens/once.md`'s own tally has two bands and so does this one now.
    parts.join(", ")
}

/// `1 pod` / `3 pods`. `rules.rs` spells the same thing for the age ladder in its own
/// `counted`, which is private to that file and stays there while `rules.rs` is frozen — no
/// link, because a private item is not one. The two merge when the header does, in Phase 5.
fn plural(n: usize, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

// --- THE REPORT END ---

// --- THE ANALYSIS REPORTS START ---

/// **The seven reports, in the order the sidebar lists them** (`screens/analysis.md`) — printed
/// under the cards when `--analysis` is passed.
///
/// **This is the first time anything outside `#[cfg(test)]` has rendered a `Report`**, and until
/// it existed invariant 9 was unexercised for all of them: `analysis_tests`' own `pane` is a
/// reading aid that strips nothing, and Posture's row text is a `hostPath.path` **verbatim and
/// whole**, so a crafted path arrived at the terminal as an escape sequence. The strip is
/// [`pane`]'s, below.
///
/// **The label beside each pane is this file's, not the report's.** [`analysis::Report`] carries
/// no sidebar name on purpose — which pane a report is drawn in is `screens/`'s ruling — so the
/// driver spells its own seven, and `Versions` is drawn beside `certificates` on the real screen
/// rather than as a pane of its own.
fn reports(snapshot: &ClusterSnapshot, findings: &[Finding]) -> String {
    [
        (
            "capacity",
            analysis::capacity as fn(&ClusterSnapshot, &[Finding]) -> analysis::Report,
        ),
        ("certificates", analysis::certificates),
        ("drain safety", analysis::drain_safety),
        ("posture", analysis::posture),
        ("restarts", analysis::restarts),
        ("waste", analysis::waste),
        ("versions", analysis::versions),
    ]
    .into_iter()
    .map(|(name, produce)| pane(name, &produce(snapshot, findings)))
    .collect::<Vec<String>>()
    .join("\n")
}

/// **One report as this driver prints it** — the sidebar label and badge, the pane heading, and
/// one block per row.
///
/// **Every string here came from `analysis.rs`, and every one is stripped as it enters the line
/// it is printed on** — never over the finished block, which is the rule that ate [`USAGE`]'s own
/// line breaks the first time it was written the other way round (NOTES § D122). The `Report`
/// surface is treated as outside wholesale, exactly as the [`Finding`] surface is in [`card`]:
/// these are k8rs's own sentences, but every one of them interpolates a name, a path or a message
/// the API sent, and a per-field judgement call is how one field gets forgotten.
///
/// **The badge draws no glyph here, and that is not the sidebar's rule being broken.** A count
/// badge carries its band as a glyph because `capacity  1` has lost what the number was of
/// (`screens/widgets.md` § 2); this driver has no sidebar, and the label is on the same line, so
/// there is nothing for a glyph to disambiguate. The row glyphs are [`symbol`]'s, shared with the
/// cards above.
fn pane(name: &str, report: &analysis::Report) -> String {
    let mut lines = vec![
        match &report.badge {
            Some(badge) => format!("[{name}] {}", sanitize(&badge.value)),
            None => format!("[{name}]"),
        },
        format!("  {}", sanitize(&report.title)),
    ];
    for row in &report.rows {
        match row {
            // **Every field named, and no `..`** — the rule `analysis_tests`' own `strings_of`
            // states for the same shape: under a `..` a new string field on this variant compiles
            // here and is silently never printed, and only a new *variant* is an error. The two
            // that hold no text are named and dropped.
            analysis::Row::Answer {
                severity,
                text,
                detail,
                action,
                jump: _,
            } => {
                lines.push(format!(
                    "  {} {}",
                    severity.map_or(" ", symbol),
                    sanitize(text)
                ));
                lines.extend(
                    detail
                        .iter()
                        .map(|paragraph| format!("      {}", sanitize(paragraph))),
                );
                if !action.is_empty() {
                    lines.push(format!("      → {}", sanitize(action)));
                }
            }
            // Read and never selected, so it carries no glyph and nothing is indented under it.
            analysis::Row::Prose(text) => lines.push(format!("  {}", sanitize(text))),
            // Two sentences, always both: a report that names the check without naming the way
            // out is the half a reader cannot act on ([`analysis::Row::NotComputed`]).
            analysis::Row::NotComputed { reason, ask_for } => {
                lines.push(format!("  {}", sanitize(reason)));
                lines.push(format!("  {}", sanitize(ask_for)));
            }
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

// --- THE ANALYSIS REPORTS END ---

// --- WATCHING A CLUSTER START ---
//
// **The other half of this temporary driver: the same report, off a cluster instead of a file**
// (NOTES § D34). It exists so `k8s.rs`'s `connect()` can be *run* — a watch that reconnects on
// its own is provable no other way, and the proof is a binary somebody leaves running while the
// node it is watching goes away and comes back (NOTES § D161).
//
// **It is not `--once` and it is not a TUI.** `--once` is a later box with an exit code and a
// format `screens/once.md` fixes; this prints the same card `render` already draws, again,
// whenever the answer changes. Nothing here draws, nothing here reads a key, and all of it goes
// away with the rest of the temporary `main` at Phase 12.
//
// **stdout is the findings and stderr is everything else**, which is the split the file-driven
// path already keeps (`screens/once.md`). So `k8rs --live > findings.txt` collects reports and
// the connection's own story stays on the terminal.

/// What every flag on this line starts with, and the whole test for *is this a path?*
const FLAG: &str = "--";

/// The `--live` flag.
const LIVE: &str = "--live";

/// The context `--live` connects to, when the run names one. **The real `--context` flag is
/// Phase 12's** — this is the same spelling so the muscle memory transfers, and it is here at
/// all because the machine that runs the reconnect proof does not have to be the machine whose
/// current context is the test cluster.
const CONTEXT: &str = "--context";

/// **Which cluster this run watches, or `None` when it reads files.**
///
/// Three answers in one: `None` is the file-driven path this driver had before, `Some(None)` is
/// `--live` on the kubeconfig's own current context, and `Some(Some(name))` is `--context name`.
/// The nesting is the same shape [`k8s::connect`] takes, so nothing translates between them.
///
/// **Both spellings, because the wrong one silently watched the wrong cluster.** `--context=NAME`
/// is what GNU getopt and `kubectl` accept, and matching only `--context NAME` let the other form
/// fall through to the kubeconfig's *current* context with no message at all — which, for a flag
/// whose whole job is to point the reconnect proof at a cluster that is not the current one, is
/// the worst available failure (`tester`, 2026-08-27).
///
/// **`--context` with nothing after it at all is still the silent-wrong-cluster failure, and
/// this is where it is written down** (`k8s-admin`, 2026-08-27). `mistyped` refuses a *flag* in
/// the value position, so `k8rs --live --context --live` is caught — but `k8rs --live --context`
/// with nothing following, which is what `--context $CTX` unquoted becomes when `CTX` is unset,
/// falls through to `Some(None)` and watches the current cluster in silence. `--context ""` is
/// refused because an empty string is a value that was meant and is not a context. **The class
/// this function's guards close is a flag in the value position and not the class as a whole**;
/// the rest is Phase 12's real flag parsing, where an option that requires a value can say so.
///
/// **A value that starts with `--` never becomes a context name here, and the sentence about it
/// is [`mistyped`]'s.** `--context --live` used to mean *the context named `--live`*, and the
/// truth arrived a moment later as a kubeconfig error about a context nobody typed. The `filter`
/// below stopped that and then swallowed it instead — `k8rs --live --context --live` connected to
/// the current context and said nothing, which is the same silent-wrong-cluster failure through
/// the other door (`k8s-admin`, 2026-08-27) — so the refusal is `mistyped`'s, which runs first and
/// has somewhere to print. The `filter` stays as the second line: this function alone must not be
/// able to answer *the context named `--live`*. `--context=--live` is not refused: an `=` says the
/// value was meant.
///
/// **A repeated `--context` is first-wins.** `kubectl` is last-wins and the real `--context` flag
/// — Phase 12's, not this scaffolding's — should follow `kubectl` rather than this. It is stated
/// here because it was stated nowhere, and an unwritten tie-break is the one that changes by
/// accident.
///
/// **`--live` wins over anything else on the line.** A path beside it is ignored rather than
/// read: the two inputs are a cluster and a file, and a run that silently merged them would
/// print a report about neither.
fn live_context(args: &[String]) -> Option<Option<&str>> {
    if args.iter().all(|arg| arg != LIVE) {
        return None;
    }
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        // `--context=NAME`. Written as two strips rather than one `"--context="` literal, so the
        // flag is spelled once in this file.
        if let Some(attached) = arg
            .strip_prefix(CONTEXT)
            .and_then(|rest| rest.strip_prefix('='))
        {
            return Some(Some(attached));
        }
        // `--context NAME`. **`--context` with nothing usable after it is the current context,
        // which is a hole and not a decision** — the doc above says which and why Phase 12 is
        // where it closes.
        if arg == CONTEXT {
            return Some(
                rest.next()
                    .map(String::as_str)
                    .filter(|value| !value.starts_with(FLAG)),
            );
        }
    }
    Some(None)
}

/// **The sentence a word that looks like a flag and is not one gets**, or `None` when every
/// flag on the line is one this build has.
///
/// **It runs before the mode is chosen, and that is the whole point.** Put inside the
/// file-reading path — where it was first written — it never ran for a live run, so
/// `k8rs --live --contxt=prod` connected to the kubeconfig's *current* context and said nothing
/// about the typo: the same silent-wrong-cluster failure [`live_context`] accepts `--context=`
/// to avoid, arriving through the other door. Found by running the binary, not by a test.
///
/// **A `--` word is never a path.** `k8rs --live=true` used to be read as one and came back
/// `--live=true: No such file or directory (os error 2)` — errno jargon about a file nobody
/// named (invariant 14). The cost is that a file genuinely called `--x` cannot be read, which is
/// an escape hatch this scaffolding already declined to owe anybody.
///
/// **A flag that is real but useless in this mode is *not* refused** — `--context` without
/// `--live`. It cannot point the run at something the reader did not name, which is the failure
/// this guard is for, and Phase 12's real flag parsing is where a tighter answer belongs.
/// **`--analysis` is no longer one of them**: it is honoured in both modes as of NOTES § D169,
/// so the list that used to name it is one shorter rather than one longer.
///
/// **A flag where `--context`'s value should be *is* refused**, and this is where that sentence
/// lives because [`live_context`] has nowhere to print one. The realistic form is
/// `k8rs --live --context "$CTX"` with `CTX` unset, and swallowing it watches the current cluster
/// in silence (`k8s-admin`, 2026-08-27). Both words are known flags, so it takes the pair rather
/// than the word: no single-argument check can see it.
fn mistyped(args: &[String]) -> Option<String> {
    if let Some(pair) = args
        .windows(2)
        .find(|pair| pair[0] == CONTEXT && pair[1].starts_with(FLAG))
    {
        return Some(format!(
            "k8rs: {CONTEXT} needs the name of a context, and {} is a flag\n{USAGE}",
            sanitize(&pair[1])
        ));
    }
    let known = |arg: &String| {
        arg == ANALYSIS
            || arg == LIVE
            || arg == CONTEXT
            || arg
                .strip_prefix(CONTEXT)
                .is_some_and(|rest| rest.starts_with('='))
    };
    args.iter()
        .find(|arg| arg.starts_with(FLAG) && !known(arg))
        .map(|mistyped| {
            format!(
                "k8rs: {} is not a flag k8rs has\n{USAGE}",
                sanitize(mistyped)
            )
        })
}

/// **The report to print now, or `None` when there is nothing new to say.**
///
/// **Two silences and they are different facts.** Before every initial LIST has landed
/// [`k8s::Store::snapshot`] answers `None` and so does this — a rule cannot tell a short list
/// from a small cluster (NOTES § D28), so nothing is printed rather than something wrong. After
/// that, `None` means the report is the same text it was last time: a watch delivers an event
/// per object per change and almost none of them move a finding, so printing every time would
/// bury the one that did.
///
/// **Comparing the rendered text is the whole of the change detection**, and it is deliberately
/// the cheapest thing that works: the age line moves with the clock, so a card that says nothing
/// new can still reprint when it crosses a rung of the ladder. That is a driver being noisy,
/// which costs a line; the alternative — comparing findings and ignoring the clock — is a driver
/// that goes quiet, which costs the proof this mode exists for.
///
/// **The clock is the caller's** (invariant 5, NOTES § D18): one instant per pass, handed in.
///
/// **`renewal` travels with it for the same reason the clock does**: it is a fact about the
/// reader's kubeconfig, read once at connect ([`k8s::Session::renewal`]), and this function has
/// no session to reach it from. It is what a `401` on a watch — the ordinary EKS/GKE/AKS
/// mid-session failure NOTES § D19 is about — is named beside.
///
/// **`analysis` is [`ANALYSIS`] and the panes go under the cards, spelled exactly as [`run`]
/// spells it** (NOTES § D169) — one `\n` between the two blocks, and [`reports`] shared rather
/// than a second arrangement of the same seven. Two of them draw a shape here that no file path
/// can reach: Versions needs a control plane to have a version, and C1's row and badge are about
/// a kubeconfig, which a `k8rs pod.json` run has none of.
fn live_report(
    store: &k8s::Store,
    now: Time,
    last: &mut String,
    renewal: Option<&str>,
    analysis: bool,
    skew: Option<SignedDuration>,
) -> Option<String> {
    let mut report = unreadable(&store.troubles(), renewal);
    match store.snapshot(now) {
        Some(snapshot) => {
            let input = Input {
                snapshot,
                // Nothing was read that no rule reads: a watch carries the five kinds `k8s.rs`
                // watches and nothing else, so the header's second half has nothing to say.
                skipped: BTreeMap::new(),
                // **Measured once, at connect, and the same for every report this session
                // prints** ([`k8s::Session::skew`]) — so a session that says it lands on the
                // first report and stays, and one that has nothing to say never starts.
                skew,
            };
            let findings = analyze(&input.snapshot);
            if !report.is_empty() {
                report.push(String::new());
            }
            let mut block = render(&findings, &input);
            if analysis {
                block.push('\n');
                block.push_str(&reports(&input.snapshot, &findings));
            }
            report.push(block);
        }
        // Still bootstrapping and nothing wrong with it: the silence is the answer.
        None if report.is_empty() => return None,
        // Bootstrapping and failing. The lines above are all there is to say, and saying them is
        // the whole point — a driver that prints nothing here is indistinguishable from one that
        // is merely slow.
        None => {}
    }
    let report = report.join("\n");
    if report == *last {
        return None;
    }
    *last = report.clone();
    Some(report)
}

/// **One plain clause: why a call did not work** — the caller supplies the subject, this
/// supplies the reason (invariant 14, `PRIOR-ART § C1`).
///
/// **A generic sentence may never stand in for an error we were handed**, which is the whole of
/// this function's reason to exist. k9s tells these apart internally and still shows
/// `Ruroh? 'v1/pods' command not found` when a credential expires; every site on the cluster path
/// — the connection, the version, the discovery answer, each watch — routes through here, so
/// there is nowhere on it for a fallback to grow.
///
/// **The claim is *the cluster path* and not *this driver*, because two typed errors live outside
/// it** and are named where they are: an io error from a failed stdout write ([`stdout_failure`])
/// and one from a runtime that would not start ([`main`]). Both print the standard library's own
/// reason through [`sanitize`]. The first draft of this line said *every site in this driver* and
/// the runtime arm was throwing its error away with a `_` (`tester`, 2026-08-27) — an overclaim
/// and a defect in one sentence, which is the second box read literally.
///
/// **`asked` is what k8rs was trying to do, already spelled the way it should read.** Only the
/// caller knows, so `` `get /apis` ``, `` `list` and `watch` pods `` and *reach this cluster*
/// arrive as display text carrying their own backticks. That is what makes a refusal name the
/// missing verb and resource, which the security gate requires — and what lets the one refusal
/// that has neither, a `nonResourceURL` on `/apis`, name a **path** instead: its measured
/// `Status` carries an empty `details`, so a sentence built from `details.group`/`details.kind`
/// would be empty (NOTES § D160).
///
/// **`renewal` is [`k8s::Session::renewal`]** — the program the reader's *own kubeconfig* names,
/// already stripped and bounded by `k8s.rs`'s ingest guard. It is never the cluster's text and
/// never the login program's output, which is a credential
/// (`docs/security.md` § Token hygiene).
///
/// **Nothing here formats the error we were handed, and that is structural rather than a rule to
/// keep**: [`k8s::Fault`] carries no string at all, so there is nothing in scope to interpolate.
fn because(fault: k8s::Fault, asked: &str, renewal: Option<&str>) -> String {
    // The program named, or not named, without changing the sentence around it.
    let named = renewal.map_or(String::new(), |program| format!(" (`{program}`)"));
    match fault {
        // **Three sentences where there was one constant over all fifteen of
        // `KubeconfigError`'s variants**
        // (`k8s-admin`, 2026-08-27). *"…or names no such context"* was printed for a
        // `client-certificate` path that had moved and for a cluster entry with no `server:`,
        // and in both the file read fine and the context was there — a generic string standing
        // in for a typed error, which is this box's whole subject, through a door it had not
        // been looked at through.
        k8s::Fault::Kubeconfig => {
            "the kubeconfig itself could not be read — it is missing, unreadable, or not valid \
             YAML"
                .to_string()
        }
        k8s::Fault::NoContext => {
            "this kubeconfig has no such context — check the `--context` you gave, or the \
             `current-context` line in the file"
                .to_string()
        }
        // **It does not say *which* entry, and that is the honest limit of a `Fault`.** The
        // variant names the class and the words are the caller's; naming the field would mean
        // carrying kubeconfig text on the type, which is the property that keeps every other
        // sentence in this file free of anything a cluster wrote.
        k8s::Fault::BadEntry => {
            "this kubeconfig loaded, and something it points at did not — a certificate file it \
             names, a `server:` line, or a cluster one of its contexts refers to"
                .to_string()
        }
        k8s::Fault::NoCredential => format!(
            "the program this kubeconfig logs in with{named} gave k8rs nothing to sign in with"
        ),
        // **The one place a renewal is worth naming** (NOTES § D19): a login minted by a helper
        // ran out mid-session, and what the reader needs is which system to sign in to again —
        // not a cloud guessed from the server URL.
        //
        // **It promises nothing about restarting, and that is measured** (`tester`, 2026-08-27).
        // kube re-runs the `exec` plugin as its cached credential falls out of its own window —
        // 25 plugin executions against 22 requests over a ten-second run — so for the ordinary
        // exec kubeconfig the watch recovers on its own the moment the login is repaired, and
        // *restart k8rs* would be D19's own failure wearing the other face: a true problem
        // answered with the wrong errand.
        //
        // **One shape reaches this arm where *renew it there* is true but incomplete**, and it is
        // narrower than *a plugin that fails mid-session* — that one has been produced since, and
        // it lands in [`k8s::Fault::NoCredential`] rather than here (NOTES § D167). What is left
        // is a plugin whose credential carries **no `expirationTimestamp` from the start**:
        // `Auth::try_from` matches `(Some(token), None) => Ok(Self::Bearer(token))`
        // (`auth/mod.rs:364-367`), so it is a static header with no `RefreshableToken` behind it.
        // Nothing ever re-runs the plugin, no `AuthError` is ever raised, and the server simply
        // answers `401` — so renewing the login where it comes from is necessary and not
        // sufficient, because k8rs also has to be restarted to pick the new token up.
        //
        // **The sentence stays as it is**: it is true of both, and the shape that needs the extra
        // half is the PM's to box rather than this arm's to guess at.
        k8s::Fault::Expired => match renewal {
            Some(program) => format!(
                "this cluster no longer accepts this login — it comes from `{program}`, so \
                 renew it there"
            ),
            None => "this cluster no longer accepts this login — this kubeconfig needs a new one"
                .to_string(),
        },
        // **It names what the role needs, not what the kubeconfig is not allowed to do**
        // (`k8s-admin`, 2026-08-27). A watch is two verbs and [`k8s::Trouble`] cannot say which
        // of them was refused — measured through a forwarder that passed `list` and answered
        // only `?watch=true` with a real `403`: the LIST **succeeded**, forty pods printed, and
        // the line beside them said *not allowed to `list` and `watch` pods*. A `Role` granting
        // `list` and omitting `watch` is an ordinary hand-written Role, and the operator adds a
        // verb that was never missing.
        //
        // **Collapsing `InitialListFailed` and `WatchStartFailed` into one [`k8s::Fault`] is
        // right** — that is what one classifier means — so the fix is the frame: *the role needs
        // both of these* is true whichever was refused, where *is not allowed to* is a claim
        // about current state this code cannot make. The security gate asks a refusal to name
        // the missing verb and resource; this names the verbs and the resource without
        // pretending to know which one is absent.
        //
        // **`needs to {asked}` and not `needs {asked}`**, so one verb phrase serves this arm and
        // the two below it: the grid test reddened on *needs reach this cluster* the moment the
        // frame changed, which is what twelve literals are for.
        k8s::Fault::Refused => format!("the role this kubeconfig uses needs to {asked}"),
        // **`when k8rs tries to …` and not `there is nothing to …`** (`tester`, 2026-08-27).
        // The old frame wanted a noun where every caller supplies a verb phrase, so it read
        // *there is nothing to `list` and `watch` pods* — and it was only ever fed the one
        // framing where that passes, `` `get /apis` `` (NOTES § D29, in a function whose own doc
        // is about framings). This frame takes all four.
        k8s::Fault::Gone => {
            format!("this server says there is no such thing when k8rs tries to {asked}")
        }
        k8s::Fault::Unanswered => format!("nothing usable came back when k8rs tried to {asked}"),
    }
}

/// **What this tool is not being given, one plain line each** — empty when all five are
/// delivering.
///
/// **The reconnect proof reads off this and not off silence** (NOTES § D161). A watch that dies
/// and comes back leaves the cluster exactly as it was, so the rendered report is the same text
/// and the driver would print *nothing* for the whole outage and nothing again on recovery —
/// which is also what a permanently dead watch prints. With these lines the outage is a change
/// and the recovery is a change back, and the proof is two printed blocks rather than an absence.
///
/// **It takes the troubles and not the store**, so both branches are reachable: `Watch::ended` is
/// private to `k8s.rs` and no stream a test can build sets it, but [`k8s::Trouble`] is `pub` with
/// `pub` fields and is the only thing this function reads.
///
/// **The store keeps answering while a watch is down, and that is deliberate** (NOTES § D162):
/// what it holds is the last complete answer, so the cards below these lines are still a real
/// answer to a question asked earlier. **The line above them stopped promising that on
/// 2026-08-27** — the paragraph below has why — and the difference these lines exist for is
/// unchanged: *something is wrong and it says so* rather than *something is wrong and it does
/// not*.
///
/// **The error itself is never printed, not even its text.** `Display` on a `kube` error
/// interpolates its source down to an `exec` plugin's stdout (`docs/security.md` § Token
/// hygiene), so what is read here is [`k8s::Trouble::fault`] and a kind — the same *select, never
/// format* rule [`k8s::Trouble::failure`] states, kept by never having the text in scope.
///
/// **The sentence has to be true of a refusal as well as an outage, and until 2026-08-27 it could
/// not tell them apart** (`k8s-admin`). The first draft read *"cannot read {kind} from this
/// cluster right now — what is shown about them may be out of date"*, and under a 403 neither
/// half held: it is not *right now*, it is until somebody edits RBAC, and nothing **is** shown
/// about that kind — the list is empty, not stale. The frame that replaced it is true of both —
/// *not getting* covers a refusal and an outage, *it keeps asking* is the retry said out loud,
/// *nothing here about them can be trusted* is true of an empty list and of a stale one — and it
/// is kept, because a `Fault` sharpens the frame rather than replacing it.
///
/// **What the classifier added is the middle clause** (todo.md § Phase 5): the refusal now names
/// the verb and the resource the security gate asks for, the expired login names the program to
/// sign in to again, and *nothing answered* stops wearing the same words as *you are not
/// allowed*. That clause is [`because`]'s and the rest of the line is this function's.
///
/// **`list` and `watch` appear inside backticks and nowhere else** (invariant 14). They are RBAC
/// verbs, not English, and the sentence around them is readable by someone who has never seen
/// one: a reader who does not know what a watch is still learns that k8rs is not getting pods and
/// that this kubeconfig is not allowed to have them. The plural is the API's own — `statefulsets`
/// and not `StatefulSets` — because it is what a `Role` has to spell.
///
/// **`ended` gets `●` and not `▲`.** It is the most severe thing this tool can say about itself —
/// nothing about that kind will ever change again — and it was wearing the warning glyph while
/// the merely-degraded line wore the same one. The reuse of the severity glyphs for a second axis
/// is a real collision and is `tui-designer`'s to settle when `views.rs` lands (backlog); giving
/// the terminal state the heavier of the two is free and correct either way.
fn unreadable(troubles: &[k8s::Trouble<'_>], renewal: Option<&str>) -> Vec<String> {
    troubles
        .iter()
        .map(|trouble| {
            // The word the reader scans, and the plural a `Role` spells. They differ for three of
            // the five, which is why one match hands back both rather than two matches drifting.
            let (kind, resource) = match trouble.kind {
                ObjectKind::Pod => ("pods", "pods"),
                ObjectKind::Node => ("nodes", "nodes"),
                ObjectKind::Deployment => ("Deployments", "deployments"),
                ObjectKind::StatefulSet => ("StatefulSets", "statefulsets"),
                ObjectKind::DaemonSet => ("DaemonSets", "daemonsets"),
                // Unreachable: `Store::troubles` answers for the five watched kinds and no
                // others. A word rather than a panic, because a driver is not the place to
                // discover that.
                _ => ("some objects", "them"),
            };
            let why = match trouble.fault() {
                Some(fault) => because(fault, &format!("`list` and `watch` {resource}"), renewal),
                // `ended` with no failure: the stream finished and never said why. The only
                // honest clause, and the one thing a fallback string is allowed to describe.
                None => "nothing was ever said about why".to_string(),
            };
            if trouble.ended {
                format!(
                    "● k8rs has stopped receiving {kind} from this cluster: {why}. What is shown \
                     about them will not change again"
                )
            } else {
                format!(
                    "▲ k8rs is not getting {kind} from this cluster: {why}. It keeps asking, and \
                     until that works nothing here about them can be trusted"
                )
            }
        })
        .collect()
}

/// **What one connection had to say for itself**, in the order a reader asks it — who this
/// server is, how much of it k8rs can see, and what it has that the tool can use.
///
/// **Two of the three are `Result`s that travel** (§ CONNECTING): a refusal on `/version` or
/// `/apis` degrades that one feature and never the session, so each is a clause here rather than
/// a reason to stop. Both are typed errors, so both name *what* failed and *why*
/// (`PRIOR-ART § C1`); neither is ever a generic sentence.
///
/// **`get /apis` and not `list apis`.** That refusal is the `nonResourceURL` one NOTES § D160
/// measured on a cluster without the default `system:discovery` binding, and its `Status` carries
/// an **empty `details`** — no group and no kind — so the path is the only true subject a
/// sentence about it can have.
///
/// **A function so that both failures can be asserted.** `live` writes this to stderr and a test
/// cannot read the process's own stream back, which is what left the two clauses unproven while
/// they were inline (2026-08-27).
fn greeting(session: &k8s::Session) -> Vec<String> {
    let renewal = session.renewal.as_deref();
    let mut said = vec![match &session.version {
        Ok(version) => format!("server {}", sanitize(version)),
        Err(error) => format!(
            "could not read the server version ({})",
            because(k8s::fault(error), "`get /version`", renewal)
        ),
    }];
    match &session.served {
        Ok(served) => {
            said.push(format!("{} kinds", served.kinds.len()));
            said.push(match &served.capabilities {
                // The `Debug` of an enum that carries nothing at all — no address, no name, no
                // string the cluster wrote ([`k8s::Capability`]). Saying each one in plain
                // language is `views.rs`'s job and belongs beside the feature it turns on;
                // spelling them here would be the second copy that goes stale.
                Some(present) => format!("{present:?}"),
                // `None` is *the discovery answer named nothing*, which is not *this cluster has
                // none of them* — the distinction `capabilities` exists to keep.
                None => "discovery named nothing at all".to_string(),
            });
        }
        Err(error) => said.push(format!(
            "could not list what this cluster serves, so k8rs cannot show you what is in it or \
             tell which add-ons it has ({})",
            because(k8s::fault(error), "`get /apis`", renewal)
        )),
    }
    said
}

/// **Watch, and print the report every time it changes** — until the process is killed.
///
/// **It takes what connecting produced rather than doing it**, so a test can hand it a session
/// over a cluster that is not there: `k8s::connect` needs a kubeconfig and there is none in a
/// test. `main` is left holding one call and no decision.
///
/// **It never returns happily and its return type says so**: the only two ways out are a
/// kubeconfig that will not connect and every watch having stopped, and the second one is
/// unreachable by construction — kube's `watcher()` cannot end (`k8s.rs` § THE DRIVER) and the
/// backoff under it never gives up. A `main` that treated a return as an ordinary exit would be
/// the failure `PRIOR-ART § B3` is about, so the sentence it comes back with is an error and the
/// exit code is 2.
///
/// **Every typed error this driver holds is turned into a sentence by [`because`], and there is
/// no other source of one** (todo.md § Phase 5, `PRIOR-ART § C1`). Four sites hold one — the
/// connection itself, the version, the discovery answer, and each watch that is in trouble — and
/// all four name *what* failed and *why*. The error's own text never reaches a screen: `Display`
/// on a `kube` error interpolates its source down to an `exec` plugin's stdout
/// (`docs/security.md` § Token hygiene), so what travels is [`k8s::Fault`], which carries no
/// string at all.
///
/// **A refusal on discovery is two features off and not a session that failed** (§ CONNECTING,
/// NOTES § D160): the sentence says so, and the watches below it start anyway.
async fn live(connected: Result<k8s::Session, k8s::NotConnected>, analysis: bool) -> String {
    use std::io::Write;
    let session = match connected {
        Ok(session) => session,
        // **The renewal comes off the failure**, and getting that wrong is what shipped the
        // first draft (`tester`, 2026-08-27): the commonest failure here is an `exec` block whose
        // program is missing or broken, and it is the one fault in the taxonomy whose fix is on
        // the reader's own machine. A sentence about it that cannot name the program has thrown
        // away the only actionable thing it had. [`k8s::NotConnected::renewal`] answers `None`
        // only for the arm where the file itself would not load.
        //
        // **Nothing has been sent to a cluster at this point**, so of the six only `Kubeconfig`,
        // `NoCredential` and `Unanswered` (a proxy protocol kube will not speak, a TLS stack that
        // would not build) are reachable. A `403` or a `404` here would read oddly against *reach
        // this cluster*; neither can arrive, and a guard for a sentence nobody can produce is a
        // second copy of the reasoning above.
        Err(problem) => {
            return format!(
                "k8rs: no cluster to watch — {}",
                because(problem.fault(), "reach this cluster", problem.renewal())
            );
        }
    };
    // Read out once, because `session.watches` is moved below and the borrow would not survive
    // it.
    let renewal = session.renewal.clone();
    let renewal = renewal.as_deref();
    // Read out here for the reason `renewal` is: `session.watches` is moved below and the borrow
    // would not survive it. It is a `Copy` number, so this is a read and not a clone.
    let skew = session.skew;
    let mut err = std::io::stderr();
    let _ = writeln!(err, "k8rs: watching — {}", greeting(&session).join(" · "));
    // The one line N4 has never had a server to say it about: a cluster outside the window this
    // build was checked against gets told, and still runs (NOTES § D149).
    if let Ok(version) = &session.version
        && let Some(note) = k8s::version_note(version)
    {
        let _ = writeln!(err, "k8rs: {note}");
    }
    let mut store = k8s::Store::default();
    // **The three facts no watch delivers, handed over once** (`k8s::Identity`, NOTES § D169):
    // the control plane's version, the context this run is on and the certificate it logs in
    // with. Read at connect and before the watches move, because `session.watches` is taken
    // below and the borrow would not survive it — the same reason `renewal` is read out above.
    store.identify(k8s::Identity::of(&session));
    let mut last = String::new();
    k8s::drive_watching(session.watches, &mut store, |store| {
        // A clock this driver cannot read is not a reason to stop watching; the next event asks
        // again. `wall_clock`'s own `Err` is a machine set before 1970.
        let Ok(now) = wall_clock() else { return };
        if let Some(report) = live_report(store, now, &mut last, renewal, analysis, skew) {
            // The write is dropped if it fails, for the reason `main` drops a failed stderr
            // write: there is nowhere left to report it, and this loop has no exit to take.
            let _ = writeln!(std::io::stdout(), "{report}\n");
        }
    })
    .await;
    "k8rs: every watch has stopped, so nothing is being read any more".to_string()
}

// --- WATCHING A CLUSTER END ---
