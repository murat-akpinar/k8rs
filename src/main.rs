//! k8rs — the temporary driver, and the first code that shows a `Finding`.
//!
//! It reads Kubernetes objects out of JSON files named on the command line, runs the rule
//! engine over them and prints what is broken. **It cannot reach a cluster**: `k8s.rs` is
//! Phase 5, which is where `--once` and the v0.0.1 release therefore sit. Until then this is
//! how the rules are exercised for real (CLAUDE.md § Running it).
//!
//! The output is `screens/once.md`'s card, minus the two things that need a later phase —
//! owner grouping and its `3 of 5 pods` count, and recency as a second sort key (Phase 10) —
//! and plus one that is not a phase: it draws the `Info` band `--once` will not, because a
//! driver whose whole job is to show what `analyze` returned may not drop one of them, and
//! Phase 5's `--once` box takes the band off (NOTES § D121).
//! It may not invent a third format: one `rules.rs`, one set of strings.
//!
//! **`--analysis` prints `analysis.rs`'s seven reports under the cards**, which is what makes them
//! runnable at all before `views.rs` exists: until this flag nothing outside `#[cfg(test)]` had
//! ever rendered a `Report`, so invariant 9's strip was unexercised for every string in all of
//! them ([`pane`]). The five lists those reports join — Services, EndpointSlices, PVCs,
//! PodDisruptionBudgets and CertificateSigningRequests — are read here from whatever files are
//! named, so a pane's *not checked* state is still reachable by simply not naming one ([`take`]).

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
use k8s_openapi::jiff::Timestamp;
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
/// [`stdout_failure`] for what a failed write costs — and what is left here is argv, the choice
/// of stream, the runtime a cluster needs, and calling `exit`.
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
                Ok(runtime) => runtime.block_on(async { live(k8s::connect(context).await).await }),
                Err(_) => {
                    "k8rs: this machine would not start the runtime a cluster needs".to_string()
                }
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

/// The three lines a run with no arguments gets. **Three, and `tests/binary.rs` counts them**:
/// the file-driven form, the live one, and what the first of them still cannot do — a usage that
/// named only half the binary would be the driver lying about itself.
const USAGE: &str = "usage: k8rs [--analysis] <file.json>...   |   k8rs --live [--context <name>]\n\
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
const ANALYSIS: &str = "--analysis";

/// The report, or the sentence that goes to stderr instead. `Err` is the whole of exit 2.
fn run(args: &[String]) -> Result<String, String> {
    let wanted = args.iter().any(|arg| arg == ANALYSIS);
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

/// **The one clock call in the program**, read once and handed on as a value
/// (invariant 5, NOTES § D18).
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
}

/// Read every path into one snapshot, or say which file stopped it.
///
/// A document carrying an `items` array is a `kind: List` — `kubectl get -A`'s answer — and
/// each item is dispatched on its own `kind`; anything else is dispatched whole. `Err` is the
/// exit-2 path (NOTES § D17): a file that will not read, will not parse, or holds an object of
/// a kind we claim to understand and does not decode.
fn load(paths: &[String], now: Time) -> Result<Input, String> {
    let mut input = Input {
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
    if findings.is_empty() {
        lines.push("○ nothing is broken".to_string());
        return lines.join("\n");
    }
    let mut order: Vec<&Finding> = findings.iter().collect();
    order.sort_by_key(|f| f.severity);
    for finding in &order {
        lines.push(card(finding, &input.snapshot.now));
        lines.push(String::new());
    }
    lines.push(tally(&order));
    lines.join("\n")
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
    // The band is counted here because the cards above are drawn — this file's third divergence
    // from `screens/once.md`, in the module doc (NOTES § D121; D87 is why `analyze` returns an
    // `Info` at all). A card above a summary that does not mention it is the half-way house.
    let notes = count(Severity::Info);
    if notes > 0 {
        parts.push(plural(notes, "note"));
    }
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
        // `--context NAME`, and `--context` with nothing usable after it is the current context.
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
/// **A flag that is real but useless in this mode is *not* refused** — `--analysis` beside
/// `--live`, `--context` without it. Neither can point the run at something the reader did not
/// name, which is the failure this guard is for, and Phase 12's real flag parsing is where a
/// tighter answer belongs.
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
fn live_report(store: &k8s::Store, now: Time, last: &mut String) -> Option<String> {
    let mut report = unreadable(&store.troubles());
    match store.snapshot(now) {
        Some(snapshot) => {
            let findings = analyze(&snapshot);
            if !report.is_empty() {
                report.push(String::new());
            }
            report.push(render(
                &findings,
                &Input {
                    snapshot,
                    // Nothing was read that no rule reads: a watch carries the five kinds
                    // `k8s.rs` watches and nothing else, so the header's second half has nothing
                    // to say.
                    skipped: BTreeMap::new(),
                },
            ));
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
/// hygiene), so what is read here is `is_some()` and a kind — the same *select, never format*
/// rule [`k8s::Trouble::failure`] states, kept by selecting nothing at all.
///
/// **No jargon** (invariant 14): the word *watch* does not appear, because a reader who has to
/// know what one is cannot use the sentence.
///
/// **And the sentence has to be true of a refusal as well as an outage, which the first one was
/// not** (`k8s-admin`, 2026-08-27). It read *"cannot read {kind} from this cluster right now —
/// what is shown about them may be out of date"*. Under a 403 neither half holds: it is not
/// *right now*, it is until somebody edits RBAC, and nothing **is** shown about that kind — the
/// list is empty, not stale. At 3am that reads *the cluster is flaky* to a reader whose actual
/// problem is *this kubeconfig is not allowed*. **Which of the two it is cannot be said here**:
/// [`k8s::Trouble`] carries `Option<&watcher::Error>` and this file may select on it, never format
/// it, so a sentence that is true of both is the honest one and the box that classifies a failure
/// is where a sharper one comes from. *Not getting* covers a refusal and an outage; *it keeps
/// asking* is true of both and is the retry said out loud; *nothing here about them can be
/// trusted* is true of an empty list and of a stale one, which *out of date* was not.
///
/// **`ended` gets `●` and not `▲`.** It is the most severe thing this tool can say about itself —
/// nothing about that kind will ever change again — and it was wearing the warning glyph while
/// the merely-degraded line wore the same one. The reuse of the severity glyphs for a second axis
/// is a real collision and is `tui-designer`'s to settle when `views.rs` lands (backlog); giving
/// the terminal state the heavier of the two is free and correct either way.
fn unreadable(troubles: &[k8s::Trouble<'_>]) -> Vec<String> {
    troubles
        .iter()
        .map(|trouble| {
            let kind = match trouble.kind {
                ObjectKind::Pod => "pods",
                ObjectKind::Node => "nodes",
                ObjectKind::Deployment => "Deployments",
                ObjectKind::StatefulSet => "StatefulSets",
                ObjectKind::DaemonSet => "DaemonSets",
                // Unreachable: `Store::troubles` answers for the five watched kinds and no
                // others. A word rather than a panic, because a driver is not the place to
                // discover that.
                _ => "some objects",
            };
            if trouble.ended {
                format!(
                    "● k8rs has stopped receiving {kind} from this cluster — what is shown about \
                     them will not change again"
                )
            } else {
                format!(
                    "▲ k8rs is not getting {kind} from this cluster — it keeps asking, and until \
                     that works nothing here about them can be trusted"
                )
            }
        })
        .collect()
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
/// **Nothing about the connection failure is printed**, and that is not an oversight: telling
/// `403` from `401` from *nothing answered* is the next box of Phase 5, and the one after it
/// forbids a generic sentence standing in for a typed error. Formatting the error we were handed
/// would also print a bearer token from an `exec` plugin's stdout (`docs/security.md` § Token
/// hygiene), so this driver names the step that failed and nothing else, and
/// [`k8s::NotConnected`] keeps the typed value for the box that will say it properly.
async fn live(connected: Result<k8s::Session, k8s::NotConnected>) -> String {
    use std::io::Write;
    let session = match connected {
        Ok(session) => session,
        Err(k8s::NotConnected::Kubeconfig(_)) => {
            return "k8rs: no cluster to watch — the kubeconfig could not be read, or names no \
                    such context"
                .to_string();
        }
        Err(k8s::NotConnected::Client(_)) => {
            return "k8rs: no cluster to watch — the kubeconfig was read and no client could be \
                    built from what is in it"
                .to_string();
        }
    };
    // Everything below is what one connection had to say for itself, on stderr, once.
    let mut said = vec![match &session.version {
        Ok(version) => format!("server {}", sanitize(version)),
        Err(_) => "the server would not say which version it is".to_string(),
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
        Err(_) => said.push("the cluster would not say which kinds it serves".to_string()),
    }
    let mut err = std::io::stderr();
    let _ = writeln!(err, "k8rs: watching — {}", said.join(" · "));
    // The one line N4 has never had a server to say it about: a cluster outside the window this
    // build was checked against gets told, and still runs (NOTES § D149).
    if let Ok(version) = &session.version
        && let Some(note) = k8s::version_note(version)
    {
        let _ = writeln!(err, "k8rs: {note}");
    }
    let mut store = k8s::Store::default();
    let mut last = String::new();
    k8s::drive_watching(session.watches, &mut store, |store| {
        // A clock this driver cannot read is not a reason to stop watching; the next event asks
        // again. `wall_clock`'s own `Err` is a machine set before 1970.
        let Ok(now) = wall_clock() else { return };
        if let Some(report) = live_report(store, now, &mut last) {
            // The write is dropped if it fails, for the reason `main` drops a failed stderr
            // write: there is nowhere left to report it, and this loop has no exit to take.
            let _ = writeln!(std::io::stdout(), "{report}\n");
        }
    })
    .await;
    "k8rs: every watch has stopped, so nothing is being read any more".to_string()
}

// --- WATCHING A CLUSTER END ---
