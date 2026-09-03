//! k8rs — the temporary driver, and the first code that shows a `Finding`.
//!
//! **Three ways in, one report out.** `k8rs <file.json>…` reads Kubernetes objects out of files
//! named on the command line; `k8rs --once` reads them off a cluster, prints one report and exits
//! — the shape v0.0.1 ships (`screens/once.md`, NOTES § D10, § D17); `k8rs --live` keeps watching
//! and reprints whenever the answer changes, which is the only way a watch that reconnects on its
//! own can be proven (NOTES § D161). All three end at [`render`], and none of them may grow a
//! second renderer.
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
//! about the reader's kubeconfig, and Capacity's `using …` paragraphs need a metrics API. All
//! three arrive with this flag: the third since the metrics poll landed beside it below
//! (`k8s.rs` § WHAT A NODE IS USING), which is gated on `--analysis` for the reason the six lists
//! are — there is no pane to open yet, and a poll for a report nobody asked for is a request on a
//! path that does not need one. Both modes call [`reports`], so there is one arrangement of the
//! seven and not two.

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
use rules::{
    ClusterSnapshot, ContainerSnapshot, ContainerState, Finding, ObjectId, ObjectKind, PodSnapshot,
    Severity, age, analyze,
};
use std::collections::BTreeMap;

/// **stdout is the findings, stderr is everything else** (`screens/once.md` § stdout and
/// stderr are split on purpose), and the two exit codes are `0` and `2` — never `1`, which is
/// reserved so a future `--exit-code` has somewhere to go (NOTES § D17).
///
/// Every decision is in a function over values that is tested — [`run`] for what to report,
/// [`live_context`] for which of the two this run is, [`cluster_run`] for how long a cluster run
/// may take and [`live`] for what it prints,
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
            // **`--live` has no happy ending to return** — it prints as it goes and comes back
            // only with the sentence that says why it stopped — but **`--once` does**, and it is
            // the only path in this driver that reaches exit `0` off a cluster
            // (§ WATCHING A CLUSTER, NOTES § D17).
            Some(context) => match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => match runtime.block_on(on_cluster(&args, context)) {
                    Some(sentence) => sentence,
                    None => return,
                },
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
/// the file-driven form, the cluster one, and what the first of them still cannot do — a usage
/// that named only half the binary would be the driver lying about itself.
///
/// **`--once|--live` and not a second synopsis line**, because the two differ in one word: both
/// read a cluster, and everything after the mode word is the same. The line count is asserted
/// (`tests/binary.rs`), and the flags a reader can only learn about here are asserted with it.
///
/// **The synopsis is one printed line written across two source lines**, and the `\` that joins
/// them keeps the three spaces before it: `scripts/width-guard.py` refuses a source line past 100
/// columns and `cargo fmt` will not wrap a string literal — it pulls the whole `const` back onto
/// one line however this is indented, so the break has to be inside the literal.
const USAGE: &str = "usage: k8rs [--analysis] <file.json>...   |   \
     k8rs --once|--live [--analysis] [--context <name>] [--namespace <name>]   |   \
     k8rs --logs --object <[namespace/]pod> [--container <name>] [--previous] [--follow] \
     [--context <name>] [--namespace <name>]   |   \
     k8rs --describe|--yaml --object <[namespace/]name> [--kind <kind>] [--context <name>] \
     [--namespace <name>]\n\
     Each file holds Kubernetes objects as JSON: one object, or a list of them.\n\
     Without --once, --live, --logs, --describe or --yaml this build reads files only — it \
     cannot reach a cluster.";

/// **Part of the released surface and not scaffolding** (NOTES § D188): `analysis.rs`'s seven
/// reports are whole-cluster answers rather than per-object cards, so they are a second report
/// under the first rather than more findings in it — and they are **the only reader three shipped
/// rules have**. N4, N5 and C1's expiring band return `Severity::Info` and nothing else, and the
/// card block above never draws that band. This doc called the flag *scaffolding like the driver
/// itself … Phase 9 draws them in panes and this goes away with the rest of the temporary main*
/// until 2026-08-30; what goes away at Phase 12 is this file, not the flag.
///
/// **A flag and not the default**, because the default output is what `tests/binary.rs` pins as
/// the report on stdout — and because a driver that printed seven panes for every `k8rs pod.json`
/// would bury the cards it exists to show.
///
/// **One meaning in every mode** (NOTES § D169): it was accepted and ignored beside `--live`
/// until the reports that need a cluster had nowhere else to be drawn — Versions has a
/// control-plane version only when there is a control plane, and C1's row and badge are about a
/// kubeconfig no file path has. A flag that is honoured in one mode and silently dropped in the
/// other is the second rule this driver would then have. [`ONCE`] costs no third answer: it is a
/// stopping point on the same cluster path, so it reads this flag through the same [`live`].
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
        .and_then(|now| load(&paths, now, wanted))
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
///
/// **It is a no-op on anything `k8s::text` produced, and it is the only strip several live inputs
/// ever meet.** Two claims, both measured, and neither of them is *the live path does not need
/// this* — a first draft of this paragraph said that and it was false
/// (`k8s-admin`, 2026-08-31).
///
/// **No-op on ingested text**: [`k8s::text`] removes or substitutes for every character
/// [`k8s::unprintable`] answers for, so a value that came off the API holds nothing left for a
/// second pass to find — 18 717 strings of every committed capture through both, 0 changed
/// (`k8s_tests.rs`'s `sanitize_cannot_act_on_anything_the_ingest_strip_left`). That is the box's
/// question answered: one string, one transformation.
///
/// **And the only strip for two live sources, which is why it stays.** A `.json` on disk builds
/// its snapshot straight off `rules.rs`'s `From` impls and never meets `k8s.rs`. **And argv never
/// meets it either, on any path** — a flag, a path or a namespace the reader typed is not an API
/// object, so a `--namespace` carrying an `ESC` reaches [`shown`] and a mistyped flag reaches
/// [`mistyped`] with this as the one thing between them and the terminal
/// (`a_crafted_path_comes_back_out_of_the_error_with_nothing_unprintable_left`,
/// `a_word_that_starts_like_a_flag_and_is_not_one_is_a_usage_error`,
/// `a_namespace_flag_with_nothing_usable_after_it_is_refused`). Deleting this prints a bidi
/// override to a terminal on a live cluster run, not only on the fixture path.
///
/// **Which is also why it may not be applied to a document.** `k8s::clean` deliberately keeps
/// `\n`, `\t` and `\r` (NOTES § D198) and this removes all three, so it is the one place
/// `sanitize` *would* be a second transformation — the reason `--yaml` writes what
/// `k8s::Document::yaml` returned straight to stdout with nothing in between.
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
    /// **When the API server's own certificate stops being accepted, when that was readable at
    /// all** — [`k8s::Session::serving_expiry`], and [`serving_certificate`] is what spells it.
    ///
    /// **`None` on the file-driven path, for [`Input::skew`]'s reason and not a weaker one.** A
    /// `.json` on disk presented no certificate to anybody, so there is nothing to have read —
    /// the same silence a live cluster whose handshake failed produces (`screens/once.md` § No
    /// reading at all is one silence, not several).
    ///
    /// **Unthresholded, on purpose**: `Some` here is *this is when it expires*, not *this is worth
    /// saying*. [`k8s::CERT_EXPIRY_WARN`] is applied where the sentence is drawn, because the days
    /// left move against `now` and a session outlives the instant it connected at.
    serving_expiry: Option<Timestamp>,
    /// **Whether `--analysis`'s panes are printing under this report** — the one thing the
    /// trailer has to know ([`ANALYSIS`], `screens/once.md` § When your own login is running out).
    ///
    /// **It suppresses exactly one line and nothing else** ([`login_certificate`]): [`reports`]
    /// already draws C1's expiring band as a Certificates row, so the trailer beside it would be
    /// the same fact twice in two shapes on one page. Every other trailer line is unaffected,
    /// because none of them has a pane that already says it.
    ///
    /// **A fact about the run and not about what was read**, which is why it is here and not in
    /// [`ClusterSnapshot`]: no rule may see a flag (invariant 5), and both drivers already hold
    /// it — [`run`] from argv, [`live_report`] from its caller.
    analysis: bool,
    /// **The kinds k8rs never got a list of at all** — the header's *blank, never guessed* rule
    /// ([`header`], `screens/widgets.md` § 1a).
    ///
    /// **Never read and stale are two different things and the header must not conflate them**
    /// ([`k8s::Trouble::listed`], which is where the two are told apart). `nodes` is cluster-scoped
    /// and cannot be granted by a namespaced `Role`, so *every* successful scoped run printed a
    /// measured-looking `0 nodes` over a list nobody was allowed to read
    /// (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R2, § R3, § R5). A watch that
    /// listed and then went down is the other case, and `widgets.md` is explicit one line further
    /// down: *stale vitals stay visible*.
    ///
    /// **It is a subset of [`Input::watch_trouble`] and never bigger**: [`k8s::Store::snapshot`]
    /// publishes nothing until every watch has listed *or settled*, and settling is a standing
    /// failure — so an unlisted watch inside a rendered report always has a line above the cards.
    ///
    /// **Always empty on the file-driven path**, for [`Input::skew`]'s reason: a `.json` on disk
    /// has no watch to have failed. A file that holds no Nodes is *a file with no Nodes*, and the
    /// header says `0 nodes` for it, correctly.
    unreadable: Vec<ObjectKind>,
    /// **Whether any watch feeding this report is in trouble right now** — the guard on the health
    /// claim ([`health`]).
    ///
    /// **Derived from the lines the reader is looking at, not from a second classification.**
    /// [`live_report`] sets it from the very [`k8s::Trouble`]s it has just turned into the
    /// watch-trouble lines above the cards, which makes *a trouble line and a health claim never
    /// appear in the same report* true by construction rather than by two rules agreeing.
    ///
    /// **Wider than [`Input::unreadable`] on purpose.** A watch that listed and *then* was refused
    /// — RBAC narrowed mid-run — still holds its last complete answer, so the counts stay; but a
    /// pod that broke after the refusal was never seen, and *nothing is broken* over that list is
    /// the reassuring wrong answer this whole field exists to stop.
    ///
    /// **`false` on the file-driven path**, which is why a fixture run keeps its claim.
    watch_trouble: bool,
}

/// Read every path into one snapshot, or say which file stopped it.
///
/// A document carrying an `items` array is a `kind: List` — `kubectl get -A`'s answer — and
/// each item is dispatched on its own `kind`; anything else is dispatched whole. `Err` is the
/// exit-2 path (NOTES § D17): a file that will not read, will not parse, or holds an object of
/// a kind we claim to understand and does not decode.
fn load(paths: &[String], now: Time, analysis: bool) -> Result<Input, String> {
    let mut input = Input {
        // No cluster answered, so nothing measured this machine's clock ([`Input::skew`]) and no
        // server presented a certificate to read ([`Input::serving_expiry`]).
        skew: None,
        serving_expiry: None,
        analysis,
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
            // rule in `rules.rs` reads any of these, and on a real cluster `k8s::report_lists`
            // and `k8s::certificate_requests` are what fill them.
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
        // No watch ran, so none of them failed ([`Input::unreadable`]).
        unreadable: Vec::new(),
        watch_trouble: false,
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
/// answers them from whatever files were named, so a field stays `None` until one object of that
/// kind is read and becomes `Some` on the first. That is what lets Waste say *nothing is going to
/// waste* over the lists it read and *not checked* over the ones it did not.
///
/// **What this path cannot say is *this cluster has none*, and the fetch can** — the one place
/// the two differ, stated because a comment claiming they match was wrong until 2026-08-28
/// (`tester`). [`load`] iterates a `kind: List`'s `.items[]`, so an *empty* envelope makes zero
/// calls to this function, `get_or_insert_with` never runs, and the field is left at *nobody
/// looked* over a capture that says in as many words that there is nothing to find.
/// `k8s::certificate_requests` answers that same cluster `Some(vec![])`.
///
/// **Closed, and the answer is that the two paths differ because this one was handed less**
/// (todo.md § Phase 5, *The typed lists `analysis.rs` needs* — the box that fetched all five and
/// owned the ruling). An empty envelope cannot be filed under a kind, because it does not name
/// one: `just fixtures` captures with `kubectl get "$kind" -A -o json`, and what lands on disk
/// carries `"kind": "List"` — not `ServiceList`, not `PodDisruptionBudgetList`. Measured on the
/// committed corpus, `services.json`'s top-level keys are
/// `["apiVersion", "items", "kind", "metadata"]` with `kind` reading `List`, and
/// `scripts/sanitize.jq` records the same fact independently. So
/// `{"apiVersion":"v1","items":[],"kind":"List","metadata":{…}}` is byte-identical whether it was
/// a capture of Services or of PDBs, and `Some(vec![])` filed from it would be a guess at which of
/// the six fields it belonged to.
///
/// **And the filename is not the missing kind.** `k8rs empty.json` is argv; keying a snapshot
/// field off what a file is called would make *this cluster has none* a property of a name a user
/// typed, which is the same guess wearing a path. A live fetch knows the kind because it asked
/// for it by name, and that is the whole of the difference: `k8s::services` answers the empty
/// cluster `Some(vec![])`, this path answers `None`, and both are true of what they were given.
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
    // **The blank line belongs to whatever follows it and not to the header**, because on the one
    // report that has nothing to follow — a scope that read nothing ([`health`]) — there is
    // nothing for it to separate, and a trailing blank before the trailer lines prints two.
    //
    // **An empty header is left out rather than drawn as an empty line.** [`header`] has nothing
    // to say when both vitals were refused and no namespace narrowed the run — reachable when the
    // scope probe timed out and both watches were then refused — and a blank first line is a
    // report that looks truncated rather than one that says less.
    let mut lines: Vec<String> = Vec::new();
    let head = header(input);
    if !head.is_empty() {
        lines.push(head);
    }
    // **The `Info` band is not drawn here, because this block is Alerts** (NOTES § D87, § D2):
    // `Severity::Info` on a rule already *means* this finding lives in a report rather than in
    // Alerts, which is how N4 and N5 use it and how C1's expiring band reaches the Certificates
    // pane at all — and, since the trailer box, the one line under the tally that restates it on
    // a run with no pane ([`login_certificate`]). Filtering once, above the `is_empty` check and
    // above [`tally`], is what stops the count and the cards disagreeing — the tally counted the
    // band only because the cards were drawn, and both follow this one line (NOTES § D121's third
    // divergence, now closed).
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
        // **The claim can be absent, which is why this is an `Option` and not a `String`**
        // ([`health`]): over a watch that read nothing there is no true sentence to put here, and
        // the trouble lines above have already said so.
        if let Some(claim) = health(input) {
            spaced(&mut lines, claim);
        }
    } else {
        order.sort_by_key(|f| f.severity);
        for finding in &order {
            spaced(&mut lines, card(finding, &input.snapshot.now));
        }
        spaced(&mut lines, tally(&order));
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
        spaced(&mut lines, clock);
    }
    // **Under the clock line, which is the trailer order `screens/once.md` § Stacked with the
    // other trailer lines fixes**: clock first because it qualifies every line above it, cards
    // included; this next, because it was the newest fact when it landed and took the one open
    // slot; C1's own line then took the next one under it; and the check-that-could-not-run line
    // is absolutely last, which [`check_switched_off`] draws.
    // **It never rode on [`Input::skipped`]** — that field is the header's, about kinds a file
    // held — and this comment said it did until the namespace-scoping box.
    //
    // **Last *of this block*, so `--analysis`'s panes still print under it** — the same placement
    // the clock line above takes and for the same reason (2026-08-28, a choice `screens/once.md`
    // does not make because it draws no panes): both qualify the cards, and both belong against
    // them rather than at the bottom of seven whole-cluster reports the reader may never reach.
    //
    // **Grouped by source and not by severity, deliberately.** A cluster days from refusing every
    // connection is closer to catastrophic than most things that earn a card — but everything read
    // off a [`k8s::Session`] rather than off a cluster object prints below every card that has one,
    // and reordering for this one would be the second rule that file does not need.
    if let Some(expiry) = serving_certificate(input.serving_expiry, &input.snapshot.now) {
        spaced(&mut lines, expiry);
    }
    // **Under the certificate the *cluster* presented, which is the trailer order
    // `screens/once.md` § Stacked with the other trailer lines fixes**: it is the newest fact, so
    // it takes the next open slot rather than displacing a line that already prints correctly
    // (NOTES § D176's *append, do not reorder*). Ordering the two by urgency instead of arrival
    // would mean weighing a certificate the cluster answers against one the reader's own laptop
    // holds, which no rule on this page has ever had to do for two cards, let alone two trailer
    // lines.
    //
    // **The one trailer line whose fact is a `Finding`, restated** (NOTES § D87, § D188). The
    // clock and the certificate above it are session facts and the line below is about the scope;
    // this one is C1's `Info` band, which the card block above filters out — so until this box a
    // bare run told the reader the *cluster's* credential was running out and never that their
    // own was, which is the one on the page they can renew without asking anybody.
    //
    // **Muted only when the Certificates pane is really going to draw this fact as a row**, which
    // needs the flag *and* the finding — [`drawn_as_a_row`]. Muting on the flag alone was a defect
    // caught in review (`k8s-admin`, 2026-09-03): the pane's row needs C1's `Finding`, and
    // `rules::kubeconfig_certificate_expiring` returns `None` with no context while this sentence
    // needs none, so a context name that strips to nothing (NOTES § D202) gave a bare run the
    // trailer and `--analysis` neither — the run with *more* reporting saying *less*, in front of
    // a credential that locks the reader out.
    if !drawn_as_a_row(input, findings)
        && let Some(login) = login_certificate(
            input
                .snapshot
                .client_certificate
                .as_deref()
                .and_then(rules::expires_at),
            &input.snapshot.now,
        )
    {
        spaced(&mut lines, login);
    }
    // **Absolutely last, under everything including both certificate lines**
    // (`screens/once.md` § Stacked with a check that could not run, and its § Stacked with the
    // other trailer lines, which fixes the whole order: clock, the cluster's certificate, this
    // login's, then this). The comment above the clock line has claimed this slot since before it
    // could be drawn; the certificate box filled it (NOTES § D176's *append, do not reorder*).
    if let Some(off) = check_switched_off(input.snapshot.namespace_scope.as_deref()) {
        spaced(&mut lines, off);
    }
    lines.join("\n")
}

/// **Whether the Certificates pane is going to draw C1's expiring band as a row** — the one
/// question the trailer has to ask before printing the same fact ([`login_certificate`]).
///
/// **Both halves, and the second is the one a flag alone gets wrong.** `--analysis` decides
/// whether [`reports`] runs at all; C1's `Finding` decides whether the pane has a row to draw when
/// it does. `analysis.rs`'s own `c1` picks the finding out of the slice by this same object kind,
/// and `rules.rs` is what writes it — three readers, one spelling, and the test
/// `the_trailer_is_muted_only_where_the_pane_really_draws_the_row` is what fails if any of them
/// moves. Asking the pane itself would mean building all seven reports to render one line.
///
/// **It deliberately does not ask which band.** Past the deadline C1 is `Severity::Critical` and
/// the card block draws it, but [`login_certificate`] has already answered `None` there, so the
/// two arms cannot both fire and this needs no third condition to say so.
fn drawn_as_a_row(input: &Input, findings: &[Finding]) -> bool {
    input.analysis
        && findings.iter().any(|finding| {
            matches!(&finding.object.kind, ObjectKind::Other(kind) if kind == "kubeconfig")
        })
}

/// **One block, one blank line above it — unless there is nothing above it to separate from.**
///
/// **Every block in a report is optional now, which is what this exists for.** The header can be
/// empty ([`header`]), the health claim can be absent ([`health`]), and the four trailer lines
/// each come and go — so a `push(String::new())` written beside any one of them prints a leading
/// blank line on the run where everything before it was left out. A copy of *two lines that have
/// to agree about emptiness* beside each block is one more place that gets missed once, and the
/// number of blocks has only ever gone up.
fn spaced(lines: &mut Vec<String>, line: String) {
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.push(line);
}

/// **The claim that nothing is wrong, scoped to what was actually read** — or `None` when no true
/// version of it exists (`screens/states.md` § Nothing broken, and something not checked).
///
/// # Why it can be `None`, which is the defect this function exists for
///
/// **`nothing is broken` is the strongest claim k8rs makes, and it was being made over a scope
/// that had returned nothing** (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R1,
/// § R4, § R10). Five measured shapes reached it: a namespaced `Role` whose context names no
/// namespace, `--namespace` on a namespace the role is refused, `--namespace` on one that does
/// not exist, a role with `get` and no `list`, and a cluster-wide reader that cannot list nodes.
/// In four of them the report had already printed `k8rs is not getting pods from this cluster` a
/// few lines above and then claimed the cluster was healthy — **before this box the tool hung on
/// *loading*, which was useless; it now says the cluster is fine, which is worse**.
///
/// **The rule is: no health claim while any watch feeding it could not be read**
/// ([`Input::unreadable`], the PM's ruling of 2026-08-29). It is at the root and not per caller,
/// so all five shapes are covered by one branch — and it is derived from the same
/// [`k8s::Trouble`]s the lines above the cards are drawn from, which makes *a trouble line and a
/// health claim never appear together* structural rather than a rule two places have to keep.
///
/// **It is deliberately wider than *standing failure*, and that is a choice, not a reading**
/// ([`Input::watch_trouble`]). A watch that is merely retrying after a blip prints the same line —
/// *until that works nothing here about them can be trusted* — so a health claim beside it
/// contradicts the sentence directly above it. Suppressing on the wider set costs a report that
/// goes quiet for a few seconds; the narrower one buys a report that contradicts itself, and a
/// watch refused *after* a good LIST keeps a list that can no longer see a pod that broke.
///
/// **What replaces the claim is nothing at all, and that is not an omission.** The trouble lines
/// say what happened, in the vocabulary this report already has; a *sorry, cannot say* line would
/// be a second sentence about the same fact, and `screens/` specifies no such string
/// (`screens/once.md` draws only the two states this report had before today).
///
/// # The scoped claim
///
/// **A genuinely empty read under a scope may still say something, but not about *the cluster***
/// (`screens/states.md` § Nothing broken, and something not checked: *"the sentence counts what
/// k8rs looked at, never what the cluster has"*). `--namespace payments` on a login that may read
/// it, with nothing wrong in it, is a real answer — so it gets `nothing is broken in payments`,
/// which is that rule at the length `--once`'s one-line claim has. The unscoped run keeps the
/// sentence it always had.
///
/// **The namespace is sanitised here as well as in [`header`]**, because it comes from argv or
/// from the reader's kubeconfig and this is a second place it reaches a terminal (invariant 9).
fn health(input: &Input) -> Option<String> {
    if input.watch_trouble {
        return None;
    }
    Some(match &input.snapshot.namespace_scope {
        Some(namespace) => format!("○ nothing is broken in {}", sanitize(namespace)),
        None => "○ nothing is broken".to_string(),
    })
}

/// **The one sentence that says this report is less complete than it looks**, or `None` when
/// every check ran (`screens/once.md` § When a check could not run, `screens/states.md`
/// § You can only see some namespaces).
///
/// **The check is N2** — a node someone started emptying and did not finish — and it is off
/// because it adds up *every* pod on a node and this run can see one namespace's
/// (`rules.rs`, which returns `None` the moment `namespace_scope` is `Some`). Singular *one node
/// check*, because this is the Alerts stream: the second check a namespace scope switches off is
/// Capacity's overcommit row, and `analysis.rs` says so on that pane rather than here. Each
/// screen names the check it would have run, so a third disabled check grows one screen by a
/// sentence instead of growing this line into a list.
///
/// **It prints whether or not there are findings**, and that is the whole reason it exists: a
/// report with cards is no more complete than an empty one when the same check was switched off,
/// and *nothing is broken* is the strongest claim k8rs makes.
///
/// **On stdout, with the findings, unlike every other line this driver writes about itself.**
/// `k8rs --live > findings.txt` that drops it produces a file claiming a clean cluster with no
/// note that a check was off, which is the failure the line exists to prevent.
///
/// # Three lines are easy to confuse here and this is none of the other two
///
/// * The **watch-trouble** line, `▲ k8rs is not getting …` — [`unreadable`]'s, per watch, in this
///   report's own `● ▲` vocabulary, above the cards, and drawn in no `screens/` file.
/// * The **header fragment** `N objects no rule reads (…)` — [`Input::skipped`]'s, about kinds a
///   *file* held that no rule reads, and empty on every live path. A comment in this file used to
///   say this sentence rode on that field; it never did, and the field is not reachable from a
///   cluster at all.
/// * **This one**, which rides on the namespace scope and nothing else.
///
/// **It takes the namespace rather than the `Input`**, so both arms are one call away and the
/// name in the scope cannot leak into a sentence that never names it — both causes print
/// identically, and the header is where the namespace is said.
fn check_switched_off(namespace_scope: Option<&str>) -> Option<String> {
    namespace_scope.map(|_| {
        "One node check is off: spotting a node someone started emptying and did not finish \
         needs every pod in the cluster."
            .to_string()
    })
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

/// **The one sentence that says this cluster is running out of the certificate it answers every
/// connection with** — C2, or `None` when there is nothing to say (`screens/once.md` § When the
/// API server's own certificate is running out).
///
/// **Two drawings, one string each, and there is exactly one draw site** — this function, called
/// once by [`render`], which is what both the file path and the live path go through. NOTES § D177
/// is the class: a sentence spelled in two places is two sentences the day one of them is edited.
///
/// **It is a session fact and not a `Finding`** (NOTES § D178): it names no cluster object, so it
/// carries no severity band and appears in no tally, the same way the clock line never has.
///
/// **Below thirty days nothing prints at all.** [`k8s::CERT_EXPIRY_WARN`] is the same distance C1
/// warns the reader's *own* certificate at, reused rather than a second unbacked number invented
/// beside it — two certificates on one report warning at two distances, with nothing to justify why
/// one gets more runway, is what invariant 14 calls noise before information. A healthy control
/// plane renews its serving certificate on its own schedule, and a `210 days` line on every run
/// would be telling the reader to check something that needs no checking.
///
/// **The `— not your kubeconfig's —` clause is in every drawing and not only the shared one.** C1
/// and C2 can print on the same report — one team's renewal habit often misses both — and a reader
/// who has just read a card about *their own* certificate needs one clause saying this is a second,
/// different one rather than the same fact twice.
///
/// **The expired sentence is timeless and says *a* cluster, not *this* one**, which is the article
/// doing work the tense cannot. This report exists, so this cluster was plainly reachable a moment
/// ago; naming it beside a claim that it cannot be reached would contradict the page it is printed
/// on. What really happened is narrower — the connection got through while that certificate is
/// expired, which is ordinary behind a load-balanced control plane where one replica has fallen
/// behind on renewal. The expiring sentence makes no such claim and names the cluster directly.
///
/// **It does not touch the exit code.** `0` still means *k8rs ran and reported*: a certificate
/// running out is a fact about the cluster, not a failure of this run to read it
/// (`screens/once.md` § Exit codes).
///
/// **No `⚠`.** `● ▲ ○` is this report's whole vocabulary, and the console's pointer family has
/// never been drawn on this stream.
fn serving_certificate(expiry: Option<Timestamp>, now: &Time) -> Option<String> {
    let expiry = expiry?;
    let left = expiry.duration_since(now.0);
    if left > k8s::CERT_EXPIRY_WARN {
        return None;
    }
    // RFC 5280 §4.1.2.5: the certificate is valid *through* `notAfter`, so only what is past the
    // deadline has run out — C1's own boundary, one file over.
    if left < SignedDuration::ZERO {
        return Some(format!(
            "A certificate the API server presented — not your kubeconfig's — expired {} ago \
             (was valid until {expiry}). When that happens, kubectl and everything else stop \
             being able to reach a cluster until someone on the control plane renews its \
             certificate — not something k8rs can do.",
            in_days(left)
        ));
    }
    Some(format!(
        "A certificate the API server presented — not your kubeconfig's — expires in {} (valid \
         until {expiry}). Once it runs out, kubectl and everything else stop being able to reach \
         this cluster until someone on the control plane renews it — not something k8rs can do.",
        in_days(left)
    ))
}

/// **The one sentence that says the reader's own login is running out** — C1's expiring band, or
/// `None` when there is nothing to say (`screens/once.md` § When your own login is running out).
///
/// **A trailer line and not a fourth card, which is what [`Severity::Info`] already means**
/// (NOTES § D87): the band lives in a report rather than in Alerts, and until this box the only
/// report that drew it was the Certificates pane behind [`ANALYSIS`] — so a bare run showed the
/// reader the *cluster's* expiring certificate and never their own, the one credential on the page
/// they can renew without asking anybody (NOTES § D188).
///
/// **The expired band is deliberately not here.** Past the deadline C1 is [`Severity::Critical`],
/// which is a card the block above already draws, and a second path to the same fact is the
/// duplicate this line exists to avoid rather than create.
///
/// **The same threshold, because it is this rule's own number** — [`k8s::CERT_EXPIRY_WARN`] is
/// C1's before it is anything [`serving_certificate`] borrows, and a 210-day reading on every run
/// is noise before it is information (invariant 14).
///
/// **No mirror of C2's `— not your kubeconfig's —` clause, and that is a ruling.** C2 needs it
/// because *a certificate is expiring* is read as *my own credential* by default and it has to
/// deny that; this sentence opens with `Your kubeconfig certificate`, which already commits to the
/// one referent a reader could have.
///
/// **It carries no severity, no place in the tally, no `⚠` and no change to the exit code** —
/// [`serving_certificate`]'s reasons, unchanged: a session fact printed in the same slot is not a
/// fifth vocabulary, it is the same one used again.
///
/// **The cluster refuses, and the sentence says so.** *"kubectl and k8rs both stop letting you
/// log in"* put the refusal in the tools, and the reader invariant 14 is written for reads that as
/// *kubectl is broken* and goes to reinstall it (`k8s-admin`, 2026-09-03). What actually happens is
/// that the cluster stops accepting the certificate; the tools are the messengers.
///
/// **And it ends on *k8rs cannot renew it*, which its three siblings all carry.** C1's own card
/// says it, [`serving_certificate`] says *not something k8rs can do* twice — this line said
/// neither, and a reader who has just watched this tool print the commands it ran will otherwise
/// go hunting for a key it does not have.
///
/// **The facts are restated, not stitched from C1's three fields.** The pane draws `title`,
/// `evidence` and `action` as three lines a row can hold; a trailer has no lines to keep apart, so
/// it joins the same facts the way C2's own trailer joins its own (`screens/once.md`).
///
/// **It reads the certificate rather than the `Finding`, and there is one shape where that says
/// more than C1 does.** `rules::kubeconfig_certificate_expiring` also needs
/// `ClusterSnapshot::context`, because its card is *about* a named context (NOTES § D51); this
/// sentence names no context, so it does not. A kubeconfig with a certificate and no current
/// context would print this line and no pane row — a state `k8s::connect` cannot produce, since a
/// file with no current context is one k8rs cannot connect with at all, and the more useful
/// answer of the two if it ever became reachable. The threshold is the shared
/// [`k8s::CERT_EXPIRY_WARN`], which `scripts/twin-guard.py` keeps equal to the rule's own, so the
/// two cannot disagree about *when* (dev-core's choice, 2026-09-02 — the brief left it open).
fn login_certificate(expiry: Option<Timestamp>, now: &Time) -> Option<String> {
    let expiry = expiry?;
    let left = expiry.duration_since(now.0);
    // RFC 5280 §4.1.2.5 at the top and C1's `Critical` card at the bottom: the deadline itself is
    // still inside the window, and everything past it is already drawn above the tally.
    if left > k8s::CERT_EXPIRY_WARN || left < SignedDuration::ZERO {
        return None;
    }
    Some(format!(
        "Your kubeconfig certificate — the file on your own machine that proves who you are, not \
         anything in the cluster — expires in {} (valid until {expiry}). Once it runs out the \
         cluster stops accepting it, so kubectl stops working for you too — ask whoever gave you \
         access for a new kubeconfig before that date, because k8rs cannot renew it.",
        in_days(left)
    ))
}

/// **Whole days, in the words the sentence prints** — `12 days`, `1 day`, and **`less than a day`
/// where a truncated `0 days` would be both wrong and the most urgent thing this line ever says**.
///
/// **A second spelling of `rules.rs`'s own `in_days`, and only because that one is private to a
/// frozen file** — the position [`k8s::CERT_EXPIRY_WARN`] is in, one layer down. C1 and C2 print
/// day counts on the same report and a reader may not be shown two ways of counting a day; the two
/// merge when C1's own renderer arrives.
///
/// The sign is dropped: the caller's sentence carries the direction — *expires in* one way,
/// *expired … ago* the other — and the same length has to read correctly in both.
///
/// **The cast is exact on every target this repo builds for** — CI's four and the gnu host beside
/// them are all 64-bit, the note [`clock`] already carries for its own. On a 32-bit one it could
/// truncate a day count no certificate a server presents can carry.
fn in_days(span: SignedDuration) -> String {
    let days = span.as_hours().abs() / 24;
    if days == 0 {
        "less than a day".to_string()
    } else {
        plural(days as usize, "day")
    }
}

/// What the report covered — the first line, so an empty report cannot be mistaken for a
/// clean cluster (`screens/once.md` § When nothing is broken).
///
/// **`ns: payments` when this run covers one namespace** and nothing at all when it covers the
/// cluster, which is the shape `screens/once.md` draws. A report that does not say what it
/// covered is the one that gets pasted into a ticket as *nothing is broken*.
///
/// **A vital nobody was allowed to read is left out, not printed as `0`**
/// ([`Input::unreadable`], `screens/widgets.md` § 1a). So this line can be `ns: payments` alone,
/// and on a run where nothing narrowed it and neither watch ever listed it can be **empty** —
/// which [`render`] leaves out rather than drawing as a blank first line.
///
/// **The context is not in it yet.** `screens/once.md`'s header samples all begin `prod-eu · `,
/// and [`crate::rules::ClusterSnapshot::context`] is filled on every live run — but naming the
/// cluster is a different fact from naming the scope, this box owns the scope, and a box does not
/// grow a second header field on its way past (`backlog.md`).
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
    let mut parts = Vec::new();
    // **A vital k8rs was refused is left out, never printed as a measured zero**
    // (`screens/widgets.md` § 1a: *a vital that cannot be read is blank, never guessed*; the TUI
    // leaves the left zone empty and this line leaves the fragment out). `nodes` is cluster-scoped
    // and cannot be granted by a namespaced `Role`, so **every** successful scoped run printed
    // `0 nodes` until this landed — a number the reader has no way to tell from an empty cluster
    // (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R2, § R3, § R5).
    let read = |kind: &ObjectKind| !input.unreadable.contains(kind);
    // **First, and before the counts, because it says what they are counts *of***
    // (`screens/once.md` § When a check could not run — the header line gains `ns: payments` "for
    // the same reason the TUI header does: a report that does not say what it covered cannot be
    // trusted after it is pasted into a ticket").
    //
    // **One line for both causes.** `--namespace` and the 403 fallback produce the same scope and
    // the same header (`k8s::Coverage`, NOTES § D46); which of the two it was is said once, on
    // stderr, by the driver that decided it — this is stdout, and stdout is the answer.
    if let Some(namespace) = &snapshot.namespace_scope {
        parts.push(format!("ns: {}", sanitize(namespace)));
    }
    if read(&ObjectKind::Pod) {
        parts.push(plural(snapshot.pods.len(), "pod"));
    }
    if read(&ObjectKind::Node) {
        parts.push(plural(snapshot.nodes.len(), "node"));
    }
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

/// **When the on-demand lists were read, said out loud, because three panes below draw a live
/// number beside a frozen one** (`k8s.rs` § WHAT A REPORT ASKS FOR, NOTES § D46).
///
/// **The panes update and half of what they update from does not.** `analysis::drain_safety`
/// recomputes its per-node rows on every watch event, off pods and nodes that are streaming; the
/// PodDisruptionBudget behind the same row — `disruptionsAllowed`, `currentHealthy`,
/// `desiredHealthy`, `observedGeneration` — was read once, before the first watch, and is never
/// read again. So *"node-3 is ready to drain"* can be printed against a budget that is minutes
/// old, and a budget applied after connect is not in the answer at all. That is D46's own
/// failure — **a false green light in front of a destructive operation** — and it is reachable
/// for the first time in this box, because `disruption_budgets` was `None` before it and a
/// `None` draws *not checked* rather than a verdict. Waste has the milder version of the same
/// thing: a `Critical` *matches no pod* row outlives the pod that fixed it.
///
/// **The fix is a re-read when a reader opens a pane, and it cannot be written here.** A timer is
/// what invariant 6 forbids by name — six cluster-wide LISTs a pass is the poll-list this project
/// refuses — and *on demand* needs a pane to be opened from, which does not exist until Phase 10.
/// So what this box owes the reader is not freshness, it is **knowing which half is old**
/// (`backlog.md` holds the re-read).
///
/// **The precedent is eight lines up**: [`Input::skew`] and [`Input::serving_expiry`] are read
/// once at connect and each says so in its own doc. The difference that earns this a line *on
/// screen* rather than in a comment is direction — those two are stable or under-report, and
/// these four over-report, on a pane that looks live.
///
/// **`None` when nothing was read**, which is not the same sentence and must not borrow this
/// one: a run whose six lists were all refused has read nothing, the panes already say *not
/// checked*, and naming a moment would be inventing a read that never happened.
///
/// **And it names only the panes whose lists actually came back, because the commonest
/// partial-permission case makes the other names false.** The built-in `view` ClusterRole grants
/// all five namespaced kinds and grants `certificatesigningrequests` in neither `view` nor `edit`
/// (`k8s.rs` § WHAT A REPORT ASKS FOR), so the ordinary cluster-wide read-only login gets **five
/// `Some` and a CSR `None`** — and an unconditional list of three printed *machines waiting to
/// join read their lists 4 min ago* directly above a certificates pane saying **not checked**.
/// Two sentences on one screen that cannot both be true, which is the class this line exists to
/// remove rather than to join (`k8s-admin`, 2026-08-29).
///
/// **The age is [`age`]'s ladder and not a wall-clock time** (NOTES § D68, `screens/widgets.md`
/// § 1b) — the one ladder every age in this tool is drawn on, so this cannot drift from the
/// card timestamps beside it. It is also the more useful half at 3am: `47 min ago` needs no
/// subtraction, and it *grows* while the reader watches, which a frozen `02:40:15` does not.
///
/// **A clock this driver could not read still gets the warning**, without the number. The two
/// facts the reader needs are *how old* and *it does not refresh*; only the first can go missing,
/// and dropping the whole line to lose it would trade the load-bearing half for the decoration.
///
/// **The mapping is written once, in the array below, and nothing checks it — that is the
/// ceiling.** A seventh on-demand list added to [`ClusterSnapshot`] and forgotten there would be
/// covered by no warning and no test, which is this function's own failure one field along. What
/// the array fixed is the *second* copy: whether to speak and which panes to name were two lists
/// for one turn, and they disagreed. Making the remaining one mechanical needs the source-text
/// walk `k8s_tests.rs` runs over `rules.rs`, which `main_tests.rs` cannot reach — the two test
/// files are `#[path]` children of different product files with no module between them — so it is
/// a box and not a line. **`metrics` was the next `Option` due on that type and it has landed**,
/// and it did not join this sentence: it is **polled** rather than read once (`k8s.rs` § WHAT A
/// NODE IS USING), so there is no *how old is this* to warn about — the field is refilled while
/// the reader watches, which is the opposite of what this line exists for.
///
/// **The list join is spelled here and not shared with `analysis.rs`'s `and_list`.** That one is
/// private to a frozen file and carries an `over` tail for a *"and 2 more"* budget this sentence
/// has no use for; reaching it would mean unfreezing `analysis.rs` to export six lines.
///
/// **Only [`live_report`] prints it, and the file path deliberately does not.** `k8rs *.json`
/// prints once and exits — a photograph that looks like a photograph, with nothing redrawing
/// beside a frozen number. The defect this line exists for is a *live* pane recomputing half its
/// row, so on the path where nothing recomputes there is nothing to warn about.
fn lists_were_read(
    snapshot: &ClusterSnapshot,
    now: &Time,
    read_at: Option<&Time>,
) -> Option<String> {
    // **The one place the six lists are mapped to the panes they feed.** Every field appears
    // exactly once, beside the pane it is drawn in, so naming a pane and deciding whether to
    // speak at all cannot disagree — they were two lists for one turn and the six-way `||`
    // spoke for panes that had been refused.
    let named: Vec<&str> = [
        (
            "waste",
            snapshot.replica_sets.is_some()
                || snapshot.services.is_some()
                || snapshot.endpoint_slices.is_some()
                || snapshot.claims.is_some(),
        ),
        ("drain safety", snapshot.disruption_budgets.is_some()),
        (
            "machines waiting to join",
            snapshot.certificate_requests.is_some(),
        ),
    ]
    .into_iter()
    .filter_map(|(pane, read)| read.then_some(pane))
    .collect();

    // Nothing was read, so there is no reading to date — the panes already say *not checked*.
    let (last, head) = named.split_last()?;
    let panes = match head {
        [] => (*last).to_string(),
        head => format!("{} and {last}", head.join(", ")),
    };
    let when = read_at
        .and_then(|at| age(now, at))
        .unwrap_or_else(|| "earlier in this run".to_string());
    Some(format!(
        "k8rs read the lists behind {panes} {when} and does not read them again while it is \
         running — anything added to the cluster since then is missing from them"
    ))
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
// **Both cluster modes live here, and the difference between them is one parameter.**
// [`ONCE`] stops at the first complete report and hands `main` an exit code (`screens/once.md`,
// NOTES § D17); [`LIVE`] prints the same card `render` already draws, again, whenever the answer
// changes, and never ends on its own. Nothing here draws and nothing here reads a key; `--live`
// goes away with the rest of the temporary `main` at Phase 12 and `--once` does not.
//
// **stdout is the findings and stderr is everything else**, which is the split the file-driven
// path already keeps (`screens/once.md`). So `k8rs --live > findings.txt` collects reports and
// the connection's own story stays on the terminal.

/// What every flag on this line starts with, and the whole test for *is this a path?*
const FLAG: &str = "--";

/// The `--live` flag.
const LIVE: &str = "--live";

/// **What a run whose watches all ended says.** Written once because two arms of [`live`] print
/// it — the mode that has no other ending, and the mode that had one and did not reach it — and a
/// sentence a reader sees is not a thing this file keeps two copies of.
///
/// **Unreachable against a real cluster**: kube's `watcher()` cannot finish and
/// `k8s::StandingBackoff` never gives up (`k8s.rs` § THE DRIVER), so what produces it is a test's
/// `stream::iter` running out. It is still an error and still exit `2`, because a driver that
/// returned quietly here would look exactly like a clean shutdown of a tool that is supposed to
/// keep reading (`PRIOR-ART § B3`).
const ALL_STOPPED: &str = "k8rs: every watch has stopped, so nothing is being read any more";

/// **The flag v0.0.1 ships** (NOTES § D10, § D17, `screens/once.md`): connect, print one report,
/// exit — `0` if it ran and reported, `2` if it could not run, and never `1`.
///
/// **The same cluster path as [`LIVE`] and deliberately not a second one.** Both call [`live`],
/// which calls [`live_report`], which calls [`render`]; what `--once` adds is a stopping point and
/// an exit code, and it may not add a renderer — two spellings of one report is the failure
/// `screens/once.md` opens with (*if `--once` and the Alerts screen could ever disagree, one of
/// them is lying*).
const ONCE: &str = "--once";

/// **How long `--once` gives the whole run**, after which it says so on stderr and exits `2`
/// ([`cluster_run`], [`too_slow`]).
///
/// **A number exists here and nowhere else in the design, and the difference is the mode and not
/// a change of mind.** `k8s.rs` refuses one outright — *nothing here cancels anything; the tool
/// does not quit because a cluster is slow* (§ THE DRIVER, NOTES § D150) — and that stays true of
/// every long-running path: `--live` passes `None` and waits forever, because a screen can show
/// a wait and a person can look at it. `--once` has no screen and no person watching it; it is a
/// command in a pipeline, and a command that never returns is worse than one that says it gave up.
///
/// **Thirty seconds, and it is derived rather than picked.** `k8s.rs`'s 500-object pages make a
/// 10 000-pod cluster twenty sequential round trips, so this allows 1.5 s per round trip at that
/// size — which no working cluster spends.
///
/// **10 000 pods is *above* the size the paint budget is stated at, and the difference is the
/// whole of why this number has headroom** (NOTES § D115, `k8s-admin` 2026-08-30).
/// `REQUIREMENTS.md § Non-functional targets` budgets first paint under a second at **~1000
/// pods**; the 10 000 figure is `PRIOR-ART § A2`'s, which is why D115 exists to say the two are
/// not the same claim. This doc called 10 000 *the largest size the paint budget is stated
/// against* — the arithmetic was right and the attribution was the thing D115 was written about.
///
/// **It is a budget for the run, and that is true since [`cluster_run`] rather than as an
/// aspiration.** It was a budget for one *segment* — the watch loop — while the connection ahead
/// of it was unbounded, which measured 140 s on an unroutable endpoint; `cluster_run` turns it
/// into a moment and both segments end at it. The one thing outside it is named there.
///
/// **It is not a diagnosis of the cluster**: [`too_slow`] hands back the two numbers D150 says
/// separate *slow* from *hung* and lets the reader decide — and [`pods_unread`] answers ahead of
/// it whenever the store holds a typed fault, because *not reachable* is neither of those two.
const ONCE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(30);

/// **Whether this run stops after one report** ([`ONCE`]), read the way [`analysis_wanted`] reads
/// its flag because it is the same question about the same line.
fn once_wanted(args: &[String]) -> bool {
    args.iter().any(|arg| arg == ONCE)
}

/// **Accepted, and it does nothing in v0.0.1** (`screens/once.md` § What `--once` does not do,
/// which says so in those words and is what this build ships).
///
/// **Honest rather than a stub.** [Invariant 2](CLAUDE.md) says `--read-only` makes the write
/// path *unreachable* rather than merely unbound, and it is unreachable: `ops.rs` does not exist
/// yet. Refusing it — which is what this build did until 2026-08-30 — teaches the reader that
/// k8rs has no such flag, and they learn otherwise a release later.
///
/// # Phase 7 has to make it load-bearing, and this is the line that says so
///
/// **A flag that silently means nothing after there is something to guard is the failure this is
/// one phase away from.** The moment `ops.rs` lands, this word must reach it: keys unbound *and*
/// the path structurally unreachable, which is the invariant and not a preference. Nothing else
/// in this file will notice if it does not, because nothing else in this file writes.
///
/// **It is not in [`USAGE`], and that is the same decision rather than an oversight.** The
/// synopsis is where a reader learns which flags exist (`tests/binary.rs` asserts the four that
/// are there for exactly that reason), and offering one that changes nothing about this build
/// would be teaching a reader to type a word for its effect. The change that makes it
/// load-bearing is the change that puts it in the line.
const READ_ONLY: &str = "--read-only";

/// The context `--live` connects to, when the run names one. **The real `--context` flag is
/// Phase 12's** — this is the same spelling so the muscle memory transfers, and it is here at
/// all because the machine that runs the reconnect proof does not have to be the machine whose
/// current context is the test cluster.
const CONTEXT: &str = "--context";

/// **Which namespace `--live` scopes the watches to**, when the run names one (NOTES § D5).
///
/// **Unlike [`CONTEXT`] this is the real flag and not scaffolding.** The scope it sets is read by
/// rules and reports that were written for it — N2 and N5 switch themselves off under one, and
/// three of the seven panes draw a different title — so what it does outlives this driver even
/// though the parsing here does not.
const NAMESPACE: &str = "--namespace";

/// **`kubectl`'s own short spelling of [`NAMESPACE`]**, because the muscle memory is the point:
/// somebody who types `kubectl get pods -n payments` all day types `-n` here too.
///
/// **`-npayments` is neither accepted nor ignored — it is refused** ([`mistyped`]). `kubectl`
/// takes it, because Go's `pflag` splits a shorthand cluster, and taking it here would make
/// `-nginx` mean *the namespace `ginx`*, which is a silent wrong scope for a word somebody
/// plausibly types. The two spellings that *are* accepted are the two [`NAMESPACE`] itself
/// accepts, so there is one rule and not two.
///
/// **Refusing to read it and refusing to accept it are two different things, and only the second
/// closes the hole** (`k8s-admin` and `tester`, independently, 2026-08-29). This doc argued the
/// first and stopped there: the word is not a `--` word, so [`mistyped`]'s *is this a flag k8rs
/// has* check never saw it, and it fell through as a stray positional. Measured, the run went
/// **cluster-wide** with nothing on screen — the silent wider scope this spelling was rejected to
/// avoid, arrived at by refusing to read it.
///
/// **The `--` check is still the reason `k8rs -x file.json` is a path** and not a usage error,
/// exactly as it was before this flag existed; what is refused is this one prefix and nothing
/// else. What *reads* `-n` is [`namespace_arg`], and its value is checked in [`mistyped`] beside
/// the refusal above.
///
/// **What that costs is a file literally named `-notes.json`**, which is now a usage error rather
/// than a path — the price of the prefix being refused at all. It is worth naming and it is not
/// worth an escape hatch: a leading `-` already makes a filename unusable with most tools, `./`
/// in front of it works here as it works everywhere, and Phase 12's real flag parsing is where a
/// `--` separator belongs.
const NAMESPACE_SHORT: &str = "-n";

/// **Which cluster this run reads, or `None` when it reads files.**
///
/// Three answers in one: `None` is the file-driven path this driver had before, `Some(None)` is a
/// cluster run on the kubeconfig's own current context, and `Some(Some(name))` is
/// `--context name`. The nesting is the same shape [`k8s::connect`] takes, so nothing translates
/// between them.
///
/// **[`ONCE`] and [`LIVE`] answer this question identically, because it is not the question they
/// differ on.** Which cluster is one decision and how long to stay is another; the second is
/// [`live`]'s parameter, so a reader looking for the difference finds it in one place rather than
/// two. `--once --live` together is a cluster run with a stopping point — the narrower of the two
/// wins, the same way `--live` already wins over a path.
///
/// **Both spellings, because the wrong one silently watched the wrong cluster.** `--context=NAME`
/// is what GNU getopt and `kubectl` accept, and matching only `--context NAME` let the other form
/// fall through to the kubeconfig's *current* context with no message at all — which, for a flag
/// whose whole job is to point the reconnect proof at a cluster that is not the current one, is
/// the worst available failure (`tester`, 2026-08-27).
///
/// **`--context` with nothing after it at all was the silent-wrong-cluster failure until
/// 2026-08-30, and it is refused now** (`k8s-admin`, twice). `k8rs --live --context` with nothing
/// following — what `--context $CTX` unquoted becomes when `CTX` is unset — fell through to
/// `Some(None)` and watched the current cluster in silence, and `k8rs --once --context && kubectl
/// apply -f prod/` made that a green light about the wrong cluster. [`mistyped`] refuses all
/// three spellings of nothing before this function is reached, so the `Some(None)` this can still
/// answer is *no `--context` on the line at all*, which is the kubeconfig's current context on
/// purpose. **What is left to Phase 12's real flag parsing is the general shape** — an option
/// that declares it requires a value — and not this flag's own hole.
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
/// **A cluster flag wins over anything else on the line, and this function is the second line of
/// that rather than the first.** The two inputs are a cluster and a file, and a run that silently
/// merged them would print a report about neither — so a path beside `--once` or `--live` is now
/// **refused**, by [`mistyped`], which runs first and has somewhere to print. This function still
/// ignores it, for the reason it still ignores `--context --live`: it alone must not be able to
/// answer *the file, plus a cluster*.
fn live_context(args: &[String]) -> Option<Option<&str>> {
    if args.iter().all(|arg| arg != LIVE) && !once_wanted(args) && verbs(args).is_empty() {
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
        // `--context NAME`. **Nothing usable after it never reaches here** — [`mistyped`] runs
        // first and refuses all three spellings of it, which is where the sentence about it can
        // be printed. The `filter` is this function's second line and not its first: it alone
        // must not be able to answer *the context named `--live`*.
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

/// **What follows `--namespace` or `-n` on this line** — the one parser for both flags and both
/// of the two spellings each takes, or `None` when neither is on the line at all.
///
/// **`Some(None)` is the flag with nothing after it**, which is a real state and not an
/// impossible one: `k8rs --live -n "$NS"` with `NS` unset is exactly that word at the end of the
/// line, and it is the commonest way to get here.
///
/// **One function, because [`mistyped`] and [`live_namespace`] must not disagree about which word
/// is the value.** Two parsers over one flag is how a run gets refused for a namespace it is not
/// about to use, or accepts a word this one would have refused — and the shape is already in this
/// file once, at [`live_context`], where the *value* checks and the *reading* are split across
/// two functions and each doc has to explain what the other does not catch.
///
/// **Both spellings, for [`live_context`]'s reason**: matching only `--namespace NAME` lets
/// `--namespace=NAME` fall through to *every namespace*, which is silently the widest possible
/// scope for a flag whose whole purpose is to narrow one.
///
/// **First wins on repeats, which is [`live_context`]'s rule and not `kubectl`'s.** `kubectl` is
/// last-wins. It is written down rather than argued because an unwritten tie-break is the one
/// that changes by accident, and Phase 12's real flag parsing is where the two should be made to
/// agree — with each other and with `kubectl`.
///
/// **Nothing here judges the value**; [`mistyped`] does, once, so there is one sentence and one
/// place it comes from. So this will hand back `Some(Some("--live"))` for
/// `--namespace --live` — which is refused a moment later, before anything is connected with it.
///
/// **One shape it reads oddly on, and it is an edge of an edge**: a *context* literally named
/// `-n` — `k8rs --live --context -n` — is found here as the short flag with nothing after it, and
/// the run is refused with a namespace sentence about a namespace nobody typed. Naming a context
/// `-n` is the sort of thing Phase 12's real parsing settles with a `--`; a check for it here
/// would be longer than the failure is likely.
fn namespace_arg(args: &[String]) -> Option<Option<&str>> {
    value_of(args, &[NAMESPACE, NAMESPACE_SHORT])
}

/// **Which namespace this run watches, or `None` for every namespace it is allowed to see**
/// (NOTES § D5).
///
/// **`None` is not *the whole cluster*, it is *do not narrow here***. What happens next is
/// `k8s.rs`'s: a cluster-wide pod LIST decides, and a `403` on it falls back to the context's
/// namespace rather than leaving the reader an empty tool (`k8s::Coverage`).
///
/// **It is read for a live run only**, like [`live_context`]: a `.json` on disk covers whatever
/// it covers, and narrowing a file after it has been read would be a filter this driver does not
/// have.
///
/// **Reached only after [`mistyped`] has passed**, which is what makes the value safe to hand on
/// without a second check here — and why this returns the value rather than a `Result`.
///
/// **In file mode the flag and its value are read as paths**, so `k8rs -n payments pod.json`
/// comes back *`-n`: No such file or directory*. That is exactly what `--context` beside it does
/// today and it is [`mistyped`]'s own documented limit — a flag that is real but useless in this
/// mode is accepted rather than refused, and Phase 12's real flag parsing is where an option that
/// requires a value can say which modes it belongs to. It is written here so the next reader of
/// this flag does not discover it from the error.
fn live_namespace(args: &[String]) -> Option<&str> {
    namespace_arg(args).flatten()
}

/// **What a refused value is called in the sentence that refuses it** — stripped, bounded, and
/// **never silently a different word from the one that was judged**.
///
/// **The check runs on what the reader typed and the echo cannot**, which is invariant 9: a value
/// with a bidi override in it is refused *as typed* and printed with the override gone. Until
/// 2026-08-30 that left `k8rs --logs --object $'default/we\u{202e}b'` answering *"and web is not
/// one"* — and `web` is a perfectly good name, so the reader was sent to fix something that looks
/// correct (`tester`, 2026-08-30). The two records may not lie about which string they mean
/// (invariant 4), so where the echo is not the value, the echo says so.
///
/// **Two facts and two clauses, because a value can be both**: characters removed for having no
/// printed form, and a cut for length. The cut is not new — a value refused *for* being eight
/// kilobytes long may not be printed at eight kilobytes to say so (the security gate's *sizes are
/// bounded* row) — but it was as silent as the strip was, and `--namespace` with 64 `a`s echoed
/// 63 of them, which is a namespace name.
///
/// **Nothing printable left is a clause and not an empty gap.** `--object web/` printed *"and  is
/// not one"* — a doubled space naming nothing (`k8s-admin`, 2026-08-30). That shape is refused
/// one layer up now ([`mistyped`] names the empty half), and this arm is what catches every other
/// door to it: a value that is *entirely* characters with no printed form.
fn shown(value: &str, most: usize) -> String {
    let clean = sanitize(value);
    let cut: String = clean.chars().take(most).collect();
    if cut.is_empty() {
        return "a value with nothing printable in it".to_string();
    }
    let mut said = cut.clone();
    if clean.chars().count() != value.chars().count() {
        said.push_str(" (with what cannot print removed)");
    }
    if cut.chars().count() != clean.chars().count() {
        said.push_str(" (shortened by k8rs)");
    }
    said
}

/// **The sentence a word that was supposed to be a namespace and is not gets**, wherever on the
/// line it was typed — [`NAMESPACE`]'s value, or the left half of [`OBJECT`]'s.
///
/// **One sentence and not two**, because there is one rule (`k8s::namespace_name`) and a second
/// wording of it is a second thing that can drift from the check. `subject` is what the reader
/// typed it under, so the sentence still names the flag they have to fix.
fn not_a_namespace(subject: &str, value: &str) -> String {
    format!(
        "k8rs: {subject} needs the name of a namespace, and {} is not one — a namespace is \
         lowercase letters, digits and dashes, up to {} characters\n{USAGE}",
        shown(value, k8s::NAMESPACE_MAX),
        k8s::NAMESPACE_MAX
    )
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
/// **`--namespace` is checked harder than `--context`, and the paragraph in the body says why**:
/// its missing value widens the run instead of narrowing it, and its value is the one word on this
/// line that ends up inside a URL path. All three bad shapes — absent, empty, and something that
/// is not a namespace name — come out of [`namespace_arg`] and get one sentence here.
///
/// **A path where a cluster was asked for is refused too**, and last, because a mistyped flag is
/// the more specific complaint about the same line: `k8rs --once --anaylsis pod.json` should be
/// told about the typo and not about the file. Before this the file was read by nothing and
/// mentioned by nobody ([`live_context`]).
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
    // **`--context` with *nothing* after it is refused too, and the two modes are one rule**
    // (`k8s-admin`, 2026-08-30). `k8rs --once --context` exited **0** on the current cluster,
    // ten lines from where `k8rs --once --namespace` exits 2 for the identical shape — so
    // `k8rs --once --context "$CTX" && kubectl apply -f prod/` with `CTX` unset was a green light
    // about the wrong cluster, in silence. That is the silent-wrong-cluster class this file
    // already refuses `--context --live` for, arriving through the last word on the line.
    //
    // **The comment that used to sit below argued for the fallback** — *very often what the
    // reader wanted anyway* — and it was written when `--live` was a harness with a person
    // watching it. `--once` is a command in a pipeline; there is nobody to notice. One rule for
    // both modes, like the path refusal [`ONCE`] already made (NOTES § D189).
    //
    // **Three spellings of nothing**: the flag as the last word, an empty value after it, and
    // `--context=` with nothing on the right. `--context=--live` is *not* one of them and stays
    // accepted, for [`live_context`]'s stated reason: an `=` says the value was meant.
    if args.last().is_some_and(|last| last == CONTEXT)
        || args
            .windows(2)
            .any(|pair| pair[0] == CONTEXT && pair[1].is_empty())
        || args
            .iter()
            .any(|arg| arg.strip_prefix(CONTEXT).is_some_and(|rest| rest == "="))
    {
        return Some(format!(
            "k8rs: {CONTEXT} needs the name of a context\n{USAGE}"
        ));
    }
    // **One check for all three ways `--namespace` can be given nothing usable.** It reads
    // like [`CONTEXT`]'s above because the two are now one rule — a flag that takes a value and
    // is given none is refused, in both modes — and it is the check that is *more* than that:
    // this flag's value is the one word on this line that ends up inside a URL path, so the arms
    // below judge the value as well as its absence. The absence alone would already be enough,
    // because a missing namespace falls back to **every** namespace, which is the opposite of
    // what a flag whose whole job is to narrow the run was asked for — a silently *wider* scope,
    // and the reader has no line on screen to notice it by. *What a namespace may look like* is
    // answered once, by `k8s::namespace_name`, rather than by a second spelling of the rule here
    // (the security gate's *names build paths* row).
    //
    // **`k8s::path_safe` used to stand here and it answers a different question** — *can this go
    // in a path* — so `PAYMENTS`, `foo.bar` and 8 KiB of `a` all got through, and the API server
    // answered every one of them `200` with an empty list. A namespace that does not exist and a
    // namespace with nothing wrong in it then printed the same report
    // (`reports/2026-08-29-namespace-scope-under-a-real-role.md` § R10).
    match namespace_arg(args) {
        Some(None) | Some(Some("")) => {
            return Some(format!(
                "k8rs: {NAMESPACE} needs the name of a namespace\n{USAGE}"
            ));
        }
        // **The echo is cut to what a namespace name could have been** ([`shown`],
        // [`k8s::NAMESPACE_MAX`]). A value is refused here *for* being 8 KiB long, among other
        // things, and printing 8 KiB back to say so is the same unbounded thing one line later
        // (the security gate's *sizes are bounded* row). 63 characters is enough to recognise
        // what was typed.
        Some(Some(value)) if !k8s::namespace_name(value) => {
            return Some(not_a_namespace(NAMESPACE, value));
        }
        Some(Some(_)) | None => {}
    }
    // **[`OBJECT`] is checked the way [`NAMESPACE`] is, and for a stronger version of the same
    // reason.** Its value is *two* words that end up inside a URL path — a namespace and a name —
    // and it is the only place in this build where a name comes from argv rather than from an API
    // server that already bounded it (`k8s::object_name`, `k8s::namespace_name`). A `/` inside
    // the name half is a request path this driver would be writing for somebody else.
    //
    // **The three shapes of nothing are [`CONTEXT`]'s three**: the flag as the last word, an
    // empty value, and `--object=` with nothing on the right. All three come out of
    // [`object_arg`] as `Some(None)` or `Some(Some(""))`, so one arm answers for them.
    match object_arg(args) {
        Some(None) | Some(Some("")) => {
            return Some(format!(
                "k8rs: {OBJECT} needs the name of an object\n{USAGE}"
            ));
        }
        // **Two halves, two rules, two sentences.** A namespace is a DNS-1123 *label* and a pod
        // name is a *subdomain*, so `PAYMENTS/web` is wrong in its left half and `default/a b` in
        // its right — and one sentence covering both told a reader that `PAYMENTS` breaks the
        // rule about dots, which is true of nothing (`dev-core`'s own run, 2026-08-30).
        Some(Some(value)) => {
            let (namespace, name) = split_object(value);
            // **An empty half costs the clause rather than printing an empty one** (invariant 14,
            // and the `server ` box's shape one file over). `--object web/` — a trailing slash off
            // tab completion — came back *"and  is not one"*: nothing named, and a doubled space
            // where the value would have been (`k8s-admin`, 2026-08-30). `--object /web` is the
            // same defect on the other side. **Nothing is echoed**, because there is nothing to
            // echo: what is wrong is the shape and the position says it.
            if namespace.is_some_and(str::is_empty) {
                return Some(format!(
                    "k8rs: {OBJECT} has nothing before the `/`, so it names no namespace — write \
                     it as `<namespace>/<name>`, or leave the `/` off to use the current \
                     namespace\n{USAGE}"
                ));
            }
            if name.is_empty() {
                return Some(format!(
                    "k8rs: {OBJECT} has nothing after the `/`, so it names no pod — write it as \
                     `<namespace>/<name>`\n{USAGE}"
                ));
            }
            if let Some(namespace) = namespace
                && !k8s::namespace_name(namespace)
            {
                return Some(not_a_namespace(
                    &format!("the namespace in {OBJECT}"),
                    namespace,
                ));
            }
            if !k8s::object_name(name) {
                return Some(format!(
                    "k8rs: {OBJECT} names one pod, written as `<namespace>/<name>` or just \
                     `<name>`, and {} is not one — a name is letters, digits, dashes and dots, \
                     up to {} characters\n{USAGE}",
                    shown(name, k8s::NAME_MAX),
                    k8s::NAME_MAX
                ));
            }
        }
        None => {}
    }
    // **[`CONTAINER`] is checked for the same reason and against the same predicate**: it becomes
    // a query parameter on the same request, and a value with `&` or `#` in it puts parameters on
    // a call the `kubectl` line prints without them, which is invariant 4's record lying.
    match container_arg(args) {
        Some(None) | Some(Some("")) => {
            return Some(format!(
                "k8rs: {CONTAINER} needs the name of a container\n{USAGE}"
            ));
        }
        Some(Some(value)) if !k8s::object_name(value) => {
            return Some(format!(
                "k8rs: {CONTAINER} needs the name of a container, and {} is not one\n{USAGE}",
                shown(value, k8s::NAME_MAX)
            ));
        }
        Some(Some(_)) | None => {}
    }
    // **[`KIND`] gets [`CONTEXT`]'s three shapes of nothing and no value check beyond that.**
    // The word never becomes a path segment: what [`k8s::kind_named`] hands back is a
    // [`k8s::Browsable`] the *cluster* named, and `k8s::Fetch::table` runs `path_safe` over
    // **its** group, version and plural (§ THE BROWSER'S ROWS). So the only judgement this word
    // can be given offline is *is it a kind this cluster serves*, which needs the cluster — and
    // it is asked there, with the two sentences `screens/detail.md` writes for it.
    if matches!(kind_arg(args), Some(None) | Some(Some(""))) {
        return Some(format!("k8rs: {KIND} needs the name of a kind\n{USAGE}"));
    }
    // **`-npayments` is refused rather than ignored** (`k8s-admin` and `tester`, both
    // independently, 2026-08-29). [`NAMESPACE_SHORT`]'s doc argues correctly against *accepting*
    // the attached short form — `-nginx` would mean the namespace `ginx` — and then the word fell
    // through as a stray positional, because it is not a `--` word and the check below never sees
    // it. Measured, the run went **cluster-wide** with no line on screen: the silent wider scope
    // the flag shape was rejected to avoid, arrived at by refusing to read it.
    //
    // **Nothing of the value is echoed**, because there is nothing to echo it for: what is wrong
    // is the spelling and not the name, and the value is the one word on this line with no bound
    // on it (the arm above).
    if args.iter().any(|arg| {
        arg.strip_prefix(NAMESPACE_SHORT)
            .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('='))
    }) {
        return Some(format!(
            "k8rs: the namespace has to be separate from {NAMESPACE_SHORT} — write it as \
             `{NAMESPACE_SHORT} <name>` or `{NAMESPACE_SHORT}=<name>`\n{USAGE}"
        ));
    }
    let known = |arg: &String| {
        arg == ANALYSIS
            || arg == LIVE
            || arg == ONCE
            || arg == READ_ONLY
            || arg == CONTEXT
            || arg == NAMESPACE
            || arg == LOGS
            || arg == DESCRIBE
            || arg == YAML
            || arg == OBJECT
            || arg == CONTAINER
            || arg == KIND
            || arg == PREVIOUS
            || arg == FOLLOW
            || [CONTEXT, NAMESPACE, OBJECT, CONTAINER, KIND]
                .iter()
                .any(|flag| arg.strip_prefix(flag).is_some_and(|r| r.starts_with('=')))
    };
    if let Some(unknown) = args.iter().find(|arg| arg.starts_with(FLAG) && !known(arg)) {
        return Some(format!(
            "k8rs: {} is not a flag k8rs has\n{USAGE}",
            sanitize(unknown)
        ));
    }
    // **After the unknown-flag check and not before it**, for the reason the path check below is
    // last: a mistyped flag is the more specific complaint about the same line, and
    // `k8rs --lgos --object default/web` should be told about the typo rather than about a
    // `--logs` the reader did type. All three checks below share that position.
    //
    // **Two verbs over one object are refused rather than ranked** (`screens/detail.md` leaves
    // the tie-break here; NOTES § D194 left the spelling here for the same reason). `--once
    // --live` *is* ranked, and the difference is what is being chosen between: those two are two
    // **breadths** of one read and the narrower is obviously meant. `--logs`, `--describe` and
    // `--yaml` are equally narrow — one object each — so picking one prints a payload the reader
    // did not ask for and gives no sign of it, which is the silent-wrong-output class this
    // function already refuses four other ways round.
    let verbs = verbs(args);
    if verbs.len() > 1 {
        return Some(format!(
            "k8rs: {} each print a different thing about the same object, so k8rs will not do \
             more than one of them in a run — pick one\n{USAGE}",
            joined(&verbs, " and ")
        ));
    }
    // **[`DESCRIBE`] reads a pod and says so before anything connects** (`screens/detail.md`
    // § Printed instead of drawn — describe). It is a string check and not a resolved kind,
    // because there is no cluster yet — but it has to *agree* with the resolution it would have
    // met. `k8s::kind_named` lowercases and matches the plural as well as the kind, so a raw
    // `!= "pod"` refused `--describe --kind pods` while `--yaml --kind pods` worked: the spelling
    // `kubectl get pods` teaches, turned down with a sentence about Secrets
    // (`k8s-admin`, 2026-08-31). Matching the same normalisation is what keeps one flag from
    // having two readers.
    if verbs.first() == Some(&DESCRIBE)
        && kind_arg(args)
            .flatten()
            .is_some_and(|kind| !matches!(kind.to_lowercase().as_str(), "pod" | "pods"))
    {
        return Some(format!(
            "k8rs: {DESCRIBE} only knows how to read a pod right now — containers and events \
             don't mean the same thing on a Secret. {KIND} {POD} is the only value it \
             accepts\n{USAGE}"
        ));
    }
    // **The two halves of one instruction, and neither is useful alone** (NOTES § D194). A verb
    // says what to do and [`OBJECT`] says what to do it to; the second is deliberately not a
    // value on the first, so that all three verbs share it without inventing three more
    // spellings of *which object*. A run that gives one and not the other has named half an
    // instruction, and guessing the other half is how a tool reads the wrong object in silence.
    //
    // **The sentence names the verb that is on the line**, and falls back to [`LOGS`] for the
    // half where there is none — `--object` alone, where naming any one of the three would be a
    // guess and naming the first is at least the one the usage lists first.
    if verbs.is_empty() != object_arg(args).is_none() {
        let verb = verbs.first().copied().unwrap_or(LOGS);
        return Some(format!(
            "k8rs: {verb} and {OBJECT} go together — {verb} says what to print and {OBJECT} says \
             which object to print it for\n{USAGE}"
        ));
    }
    // **A path beside a cluster flag is refused rather than silently ignored.** The two inputs
    // are a cluster and a file; [`live_context`] answers *the cluster* and drops the path, so
    // before this `k8rs --live pod.json` read the cluster and said nothing about the file the
    // reader had named — the silent-wrong-input shape this function already refuses three other
    // ways round. It applies to `--live` as well as [`ONCE`] because it is one rule about one
    // ambiguity, and a rule that held for one of two modes is the second rule this driver would
    // then have (the `--analysis` paragraph above).
    //
    // **What is a path here is *not a flag and not a flag's value***. The three flags that take
    // the next word own it whatever it looks like — `--namespace payments` must not read
    // `payments` as a file — so their value is skipped with them.
    //
    // **A one-dash word this build does not have is a usage error and not a silently dropped
    // one** (`k8s-admin`, 2026-08-30). `k8rs --once -o json` came back *"--once and --live read a
    // cluster, so k8rs cannot also read json"*, and **neither half of that is true**: `-o` was
    // skipped without a word because the `known` check above only tests `--` words, and `json`
    // then fell through as a stray positional. `screens/once.md` § What `--once` does not do lists
    // `-o json` by name as a shape readers will try, so it gets the same sentence every other
    // flag k8rs does not have gets. `-n=payments` is the one one-dash word that is real, and
    // `-nginx` was refused further up.
    //
    // **Only on a cluster run.** With no cluster flag there is no ambiguity and `k8rs -x
    // file.json` stays a path, which is what [`NAMESPACE_SHORT`]'s doc promises and what the
    // `--` test in `known` above is for.
    if live_context(args).is_some() {
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            if arg == CONTEXT
                || arg == NAMESPACE
                || arg == NAMESPACE_SHORT
                || arg == OBJECT
                || arg == CONTAINER
                || arg == KIND
            {
                rest.next();
                continue;
            }
            if arg.starts_with('-') {
                // A `--` word was already vetted by `known` above, and `-n=payments` is the one
                // one-dash word this build has. Everything else with a dash on the front is a
                // flag k8rs does not have.
                if arg.starts_with(FLAG)
                    || arg
                        .strip_prefix(NAMESPACE_SHORT)
                        .is_some_and(|rest| rest.starts_with('='))
                {
                    continue;
                }
                return Some(format!(
                    "k8rs: {} is not a flag k8rs has\n{USAGE}",
                    sanitize(arg)
                ));
            }
            // **The sentence names the mode that is on the line and not the two it used to
            // name always.** `k8rs --logs --object default/web pod.json` came back *"--once and
            // --live read a cluster"* about a run with neither flag on it — a message that is
            // not true of the run it is about, which is the class NOTES § D190 is named for
            // (`dev-core`'s own run, 2026-08-30).
            // **The verb that is on the line wins over the two breadth flags**, which is the
            // same rule the sentence already had for [`LOGS`] — now read off [`verbs`] so a
            // fourth verb cannot be added without joining it.
            let mode = match (verbs.first(), once_wanted(args)) {
                (Some(verb), _) => verb,
                (None, true) => ONCE,
                (None, false) => LIVE,
            };
            return Some(format!(
                "k8rs: {mode} reads a cluster, so k8rs cannot also read {} — run it with the \
                 flag, or with the file, not both\n{USAGE}",
                sanitize(arg)
            ));
        }
    }
    None
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
/// **The facts read once, at connect, that a report needs and [`live_report`] cannot reach a
/// session from** — one value because they are one category, and because four bare `Option`s in a
/// row at a call site is a swap nobody sees. It is also what kept [`live_report`] under
/// `clippy::too_many_arguments` when the fourth arrived.
///
/// **Every one of them is a *photograph* the panes are drawn over.** The watches keep running and
/// these values do not change again — what *is* redrawn from them can still move, which each
/// field's own doc says where it is true, and which [`lists_were_read`] says out loud for the one
/// that makes a pane lie.
#[derive(Default)]
struct AtConnect<'a> {
    /// A fact about the reader's kubeconfig ([`k8s::Session::renewal`]) — what a `401` on a watch
    /// is named beside, the ordinary EKS/GKE/AKS mid-session failure NOTES § D19 is about.
    renewal: Option<&'a str>,
    /// **The same sentence for every report this session prints** ([`k8s::Session::skew`]): a
    /// session that has something to say says it on the first report and keeps saying it, and one
    /// that has nothing never starts.
    skew: Option<SignedDuration>,
    /// **Read once, and *not* the same sentence every pass** ([`k8s::Session::serving_expiry`]) —
    /// the days left are measured against the snapshot's own `now`, so a session left open across
    /// a threshold starts saying so without reconnecting.
    serving_expiry: Option<Timestamp>,
    /// **When the six on-demand lists were read** ([`lists_were_read`], which turns it into the
    /// line above the panes). `None` is a run that fetched nothing, or a clock this machine could
    /// not read.
    lists_read_at: Option<Time>,
}

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
    analysis: bool,
    at: &AtConnect,
) -> Option<String> {
    let troubles = store.troubles();
    // **Read off the same list the lines below are drawn from** ([`Input::unreadable`]): a report
    // cannot carry a trouble line and a health claim at once, and this is where that is made true
    // rather than remembered.
    let never_listed: Vec<ObjectKind> = troubles
        .iter()
        .filter(|trouble| !trouble.listed)
        .map(|trouble| trouble.kind.clone())
        .collect();
    let watch_trouble = !troubles.is_empty();
    let mut report = unreadable(&troubles, at.renewal);
    match store.snapshot(now) {
        Some(snapshot) => {
            let input = Input {
                snapshot,
                unreadable: never_listed,
                watch_trouble,
                // Nothing was read that no rule reads: a watch carries the five kinds `k8s.rs`
                // watches and nothing else, so the header's second half has nothing to say.
                skipped: BTreeMap::new(),
                // **Measured once, at connect, and the same for every report this session
                // prints** ([`k8s::Session::skew`]) — so a session that says it lands on the
                // first report and stays, and one that has nothing to say never starts.
                skew: at.skew,
                // **Read once, at connect, for the same reason** ([`k8s::Session::serving_expiry`])
                // — but *unlike* the skew this one is not the same sentence every pass: the days
                // left are measured against the snapshot's own `now`, so a session left open over
                // a threshold starts saying so without reconnecting.
                serving_expiry: at.serving_expiry,
                // **The flag this function was already handed**, so the one trailer line a pane
                // would repeat is silent on the same runs there ([`Input::analysis`]).
                analysis,
            };
            let findings = analyze(&input.snapshot);
            let mut block = render(&findings, &input);
            if analysis {
                block.push('\n');
                // **Above the panes and not inside one**, because it is true of three of them
                // ([`lists_were_read`]) — and a caveat repeated per pane is a caveat the reader
                // stops seeing.
                if let Some(said) = lists_were_read(
                    &input.snapshot,
                    &input.snapshot.now,
                    at.lists_read_at.as_ref(),
                ) {
                    block.push_str(&said);
                    block.push_str("\n\n");
                }
                block.push_str(&reports(&input.snapshot, &findings));
            }
            // **An empty block is not pushed, and neither is the blank line that would have
            // separated it** — the rule [`render`] states about its own trailer, one layer up and
            // reintroduced here until 2026-08-30. A kubeconfig granting none of the five kinds
            // refuses every watch before it lists: the header has no vital it is allowed to
            // print, there are no cards, no health claim may be made, and `render` correctly
            // answers `""`. Pushing that ended the report on a blank line and the caller's own
            // `\n` ([`run`]) made it two.
            if !block.is_empty() {
                if !report.is_empty() {
                    report.push(String::new());
                }
                report.push(block);
            }
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
/// that has a fault in hand — the connection, the version, the discovery answer, each watch —
/// routes through here.
///
/// **That sentence used to end *so there is nowhere on it for a fallback to grow*, and one had
/// grown** (`k8s-admin`, `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 5). [`ONCE`]'s
/// deadline is a site on the cluster path, and it reported an endpoint with nothing listening as
/// a **slow** cluster from `k8s::Store::still_listing` alone, while `k8s::Store::troubles` held
/// `k8s::Fault::Unanswered` on all five watches. The claim is repaired by the code and not by the
/// wording: that arm asks [`pods_unread`] first, which routes through here. What is left outside
/// is [`too_slow`], and it is outside by construction rather than by omission — it reports two
/// measurements about a LIST that has not failed, so there is no fault in scope for it to
/// interpolate.
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
        // **The only arm whose fix is in k8rs and not on the reader's side**, so it says so
        // instead of handing out an errand. A `400` was [`k8s::Fault::Unanswered`] until
        // 2026-08-30 and printed *nothing usable came back* — a network sentence for a request
        // this side got wrong, which is `PRIOR-ART § C1` in the box written to close it
        // (`k8s-admin`; `k8s::Fault::Rejected` carries the measured shape).
        k8s::Fault::Rejected => {
            format!(
                "this cluster would not accept the request k8rs made to {asked} — that is a \
                     fault in k8rs, and nothing is wrong with the cluster or with this login"
            )
        }
        k8s::Fault::Unanswered => format!("nothing usable came back when k8rs tried to {asked}"),
    }
}

/// **The word a reader scans for a watched kind, and the plural a `Role` spells** — they differ
/// for three of the five, which is why one match hands back both rather than two matches
/// drifting.
///
/// **One function because two callers say the same word.** [`unreadable`] names the kind a watch
/// is in trouble on and [`too_slow`] names the kind a LIST has not finished, and a driver that
/// spelled `DaemonSets` in one line and `daemonsets` in the next would be inventing a second
/// vocabulary for one set of objects (CLAUDE.md § Single point of change). Second copies of a
/// shared string are what this repo has paid most for.
///
/// **The catch-all is unreachable and is a word rather than a panic**: `k8s::Store::troubles` and
/// `k8s::Store::still_listing` both answer for the five watched kinds and no others, and a driver
/// is not the place to discover otherwise.
fn plain_kind(kind: &ObjectKind) -> (&'static str, &'static str) {
    match kind {
        ObjectKind::Pod => ("pods", "pods"),
        ObjectKind::Node => ("nodes", "nodes"),
        ObjectKind::Deployment => ("Deployments", "deployments"),
        ObjectKind::StatefulSet => ("StatefulSets", "statefulsets"),
        ObjectKind::DaemonSet => ("DaemonSets", "daemonsets"),
        _ => ("some objects", "them"),
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
            let (kind, resource) = plain_kind(&trouble.kind);
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
    let mut said = Vec::new();
    match &session.version {
        // **A version with nothing readable in it costs the clause rather than printing
        // `server ` with nothing after it.** Four shapes arrive here blank and only the first is
        // obvious: no `gitVersion` in the answer at all, one the server wrote as `""`, one that is
        // only spaces, and one made entirely of characters invariant 9 strips — which is why the
        // test is on the *stripped* value and why it is `trim` and not `is_empty`. A real
        // kube-apiserver always sets the field; a proxy or gateway in front of one is where all
        // four come from.
        //
        // **Silence and not a failure sentence**: the call was answered, so *could not read the
        // server version* would be false, and the clause has nothing true left to say.
        //
        // **The trimmed value is what prints, because it is what was decided on.** The first draft
        // kept the untrimmed string on the grounds that trimming invents text the cluster did not
        // send; that defence does not hold (`k8s-admin`, 2026-09-03) — `k8s::session` has already
        // run `text(&mut version, IDENTIFIER)` over it, so this is not the server's untouched
        // bytes either, and `server  v1.36.1  ·` was the only thing the split bought.
        Ok(version) => {
            let version = sanitize(version);
            let version = version.trim();
            if !version.is_empty() {
                said.push(format!("server {version}"));
            }
        }
        Err(error) => said.push(format!(
            "could not read the server version ({})",
            because(k8s::fault(error), "`get /version`", renewal)
        )),
    }
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

/// **Every read this run performs, spelled as the `kubectl` a reader could paste** — the command
/// log outside the TUI (invariant 4, `screens/once.md` § stdout and stderr are split on purpose).
///
/// **It joins the convention [`kubectl_get`] and [`describe_run`] already ship rather than
/// starting a second one**: one `$ kubectl …` line per read, on stderr, sanitized like every other
/// free-text field. **It is display text — nothing here is executed and nothing in it is fed back
/// into a process** (the security gate).
///
/// **The four groups are in the order they happen; inside the last two there is no order to be
/// in, and this log does not pretend otherwise.** The scope probe is first and alone, then the
/// version and discovery, one after the other (`k8s.rs` § CONNECTING); then, under [`ANALYSIS`],
/// the seven a report fetches
/// **on one deadline and at the same time** (`k8s::report_lists`' `tokio::join!` — concurrency is
/// the whole reason it is a `join!`); then the five permanent watches, whose LISTs are also in
/// flight together the moment the loop starts polling them (invariant 6). So this list is spelled
/// in the order the code *starts* them — the `join!`'s own arms, then `k8s::watches`' own vec —
/// which is the only stable order a concurrent group has. **Measured rather than assumed**: a
/// logging stub in front of `--once --analysis` saw the seven arrive services · endpointslices ·
/// pvcs · pdbs · replicasets · metrics · csrs, and the five arrive deployments · pods · daemonsets
/// · nodes · statefulsets — neither is a declaration order, and neither is stable (dev-core,
/// 2026-09-02). Nothing above rests on the wire order; what the reader is owed is *which reads
/// this run makes*, and a line they can paste.
///
/// **`/version` prints once and is read twice.** The second round trip is for the `Date` header
/// the clock line is built from and it is the identical request; a reader who pastes the line once
/// has reproduced both, and a second identical line would read as a stutter rather than a fact.
///
/// **Discovery prints as `kubectl api-resources` and not as the two raw paths it sends.**
/// `get --raw /apis` alone leaves out what `/api`'s own versions serve, so it would not answer the
/// question the reader has — the same one-command-stands-for-several-calls shape
/// `kubectl describe pod` already has in this file.
///
/// **`--verbs=list`, because that is the filter [`greeting`] counts through.** `k8s::browsable`
/// keeps only kinds whose discovery entry `supports_operation(verbs::LIST)`, which is exactly what
/// the flag selects; the flag changes nothing on the wire (the same `/api` and `/apis`, measured).
/// Without it the greeting says `62 kinds` two lines above a command that prints 69, and a reader
/// reconciling the two concludes the tool is off by seven (`k8s-admin`, 2026-09-03).
///
/// **The watches print as `--watch`, which is the closest one command gets.** k8rs re-lists and
/// keeps going when a watch breaks (`k8s::StandingBackoff`) and `kubectl get --watch` stops; that
/// gap is about a dropped connection, not about what comes back while it holds.
///
/// **The first line is a probe, and it earns a line under the same bar the two below it do**
/// (`screens/once.md`, `tui-designer`'s ruling of 2026-09-03). `k8s::coverage` asks *may this
/// login list pods at all* before anything else on this path — one `LIST` capped at a single
/// object — and what its answer decides is *scope*, not a fact any card is built from. That is a
/// real reason to leave a line off and it is exactly the reason the handshake below has none; but
/// this is an ordinary request with an ordinary `kubectl` spelling, and the bar is *every read
/// k8rs performs*, not *every read the report is built from*. `/version` and discovery are already
/// printed under the wider bar while deciding a greeting and a kind list, so excluding this one
/// would be a rule this function does not hold its own next two lines to.
///
/// **It prints as a raw path, and `--chunk-size=1` is the spelling that was refused.**
/// `k8s::lists_pods` sends **one** `GET /api/v1/pods?limit=1`. `kubectl get pods -A
/// --chunk-size=1` sets the *page size* and then pages to completion: measured against the
/// fixture cluster, 41 pods cost **41 sequential requests and 6.3 s**, where the raw path costs
/// one (the PM, 2026-09-03). It is exact for the first request and wrong for every one after it,
/// and it would teach the reader that k8rs listed every pod one at a time to answer *may I look
/// at pods at all* — the opposite of the truth, on line 1 of every unscoped run, from the tool
/// whose invariant 6 is *watch, never poll-list*. On a five-thousand-pod cluster it is
/// `PRIOR-ART § A2`'s pathological case handed out as a teaching command.
///
/// **`get --raw` is not a new shape here**, it is the one `/version` two lines below already
/// uses, and it is exact rather than approximate. The path is single-quoted so a `?` reaches
/// `kubectl` instead of the shell's globbing.
///
/// **Nothing else on this list carries a page-size flag, and that is the consistent answer rather
/// than an oversight.** `k8s::whole_list` sends no `limit` at all while `kubectl get` defaults to
/// `--chunk-size=500`, so exactness of that kind would owe every report line a `--chunk-size=0`.
/// It does not: those lines are bare, and the reader who pastes one gets the same objects back
/// (`screens/once.md`). Consistency here is the absence of the flag.
///
/// **Which probe lines print is read back off [`k8s::Coverage`] and the context's namespace,
/// because that pair is what `k8s::coverage` branched on.** `--namespace` answers the question
/// before it is asked, so nothing is sent and nothing prints; a refusal sends a second probe at
/// the fallback namespace **only** when the kubeconfig's context names none to use instead — and
/// the filter here is `k8s::namespace_name`, the same one `context_scope` applies, or the two
/// disagree about a context namespace that is not a namespace name. **The distinction is not
/// cosmetic**: a context that itself names `default` produces `Refused("default")` with *one*
/// probe sent, and a line under it would claim a request k8rs never made (invariant 4).
///
/// **`nodes` never carries a scope flag, on any run.** It is cluster-scoped, so there is no
/// namespaced node list to ask for — `k8s::scoped`'s bound is what makes that structural — and
/// `certificatesigningrequests` is bare for the same reason.
///
/// **One line per kind and not one merged `deployments,statefulsets,daemonsets --watch`.**
/// `kubectl` would accept the list and print one plausible line, but k8rs opens three separate
/// watches and a reader troubleshooting which kind misbehaves needs a line they can run alone.
///
/// **Two reads on this path deliberately get no line, and both exemptions are structural.** The
/// TLS handshake C2 reads its certificate off sends no request at all (`k8s.rs` § THE SERVER'S OWN
/// CERTIFICATE) — there is no `kubectl` spelling of *look at a certificate and hang up* — and the
/// second `/version` is the one above, printed once. **The scope probe was a third until
/// 2026-09-03 and is not one any more**: it was measured as request 1 of a bare run and reported
/// as a gap in `screens/once.md`, which then ruled it a line rather than an exemption.
///
/// **Two run shapes print no log at all, and both are runs with no report.** A connection that
/// never happened returns above this ([`live`]'s `Err` arm), and so does the one session-level
/// reading that ends the run instead of joining it ([`certificate_is_why`]) — a wall is not a
/// place to teach commands from. **The three object verbs are outside it too**: `--logs`,
/// `--describe` and `--yaml` connect through the same [`k8s::connect`] and so perform the first
/// two reads here, but they draw `screens/detail.md` rather than `screens/once.md` and already
/// carry their own one line each ([`kubectl_get`], [`describe_run`]). Widening this to them is a
/// box that page has not been written for, and all three flags die at Phase 12 (NOTES § D194).
///
/// **A function so both shapes can be asserted.** `live` writes this to stderr and a test cannot
/// read the process's own stream back — the reason [`greeting`] is one too.
/// **Everything read before the first watch, as the `kubectl` a reader could paste** — the scope
/// probe, the version, and discovery ([`command_log`], which has the whole of why each line is
/// spelled the way it is).
///
/// **It is a function because two runs print it and only one of them goes on** ([`live`]). An
/// ordinary run prints this and then the rest; the run [`certificate_is_why`] ends on a wall
/// prints exactly this and nothing more, because on that path the report lists and the watches
/// never happen. A wall is where *here are the requests I did make* is the most useful thing on
/// the screen, and printing the whole log there would name reads that never ran.
fn connect_log(coverage: &k8s::Coverage, context_namespace: Option<&str>) -> Vec<String> {
    let mut log = Vec::new();
    // **`k8s::coverage`'s own branches, read back off what it answered** — never a second guess at
    // which requests it sent. The cluster-wide probe is always cluster-wide, whatever scope the
    // run ended up with; only the fallback probe names a namespace.
    let cluster_wide = "$ kubectl get --raw '/api/v1/pods?limit=1'";
    match coverage {
        // Typing `--namespace` answers the question the probe exists to ask, so nothing is sent.
        k8s::Coverage::Asked(_) => {}
        // Answered cluster-wide: one request, no fallback needed.
        k8s::Coverage::Cluster => log.push(cluster_wide.to_string()),
        // Refused cluster-wide. The second probe went out only when the file held no namespace to
        // fall back to instead — the doc above has why the filter has to be the same one.
        k8s::Coverage::Refused(_) | k8s::Coverage::Blind(_) => {
            log.push(cluster_wide.to_string());
            if context_namespace
                .filter(|named| k8s::namespace_name(named))
                .is_none()
            {
                // **The namespace the probe was really sent to**, off the value `k8s::coverage`
                // answered with rather than a second copy of `FALLBACK_NAMESPACE` — this arm is
                // only reachable when the context named nothing, so it is that constant, and
                // taking it from here means the line cannot drift from where the request went.
                log.push(format!(
                    "$ kubectl get --raw '/api/v1/namespaces/{}/pods?limit=1'",
                    sanitize(coverage.namespace().unwrap_or(""))
                ));
            }
        }
    }
    log.push("$ kubectl get --raw /version".to_string());
    log.push("$ kubectl api-resources --verbs=list".to_string());
    log
}

/// The command log on its way to stderr, from the two places that write one — the ordinary run
/// and the wall [`certificate_is_why`] ends it on.
fn log_to(err: &mut impl std::io::Write, lines: Vec<String>) {
    for line in lines {
        let _ = writeln!(err, "{line}");
    }
}

fn command_log(
    analysis: bool,
    coverage: &k8s::Coverage,
    context_namespace: Option<&str>,
) -> Vec<String> {
    // `-A` or `-n payments`, written once: five of these lines follow the scope under
    // [`ANALYSIS`] and four of the five watches do, and a second spelling of *which namespace* is
    // another place it can be forgotten in one.
    let scope = match coverage.namespace() {
        Some(namespace) => format!(" -n {}", sanitize(namespace)),
        None => " -A".to_string(),
    };
    let mut log = connect_log(coverage, context_namespace);
    if analysis {
        log.push("$ kubectl get certificatesigningrequests".to_string());
        for kind in [
            "replicasets",
            "services",
            "endpointslices",
            "persistentvolumeclaims",
            "poddisruptionbudgets",
        ] {
            log.push(format!("$ kubectl get {kind}{scope}"));
        }
        // **`kubectl top nodes` and not a raw path into `metrics.k8s.io`** — the command a reader
        // already knows for this question, and the one line here that is not a `kubectl get`. It
        // prints once whether the reading is [`ONCE`]'s single fetch or `--live`'s thirty-second
        // poll (`k8s::node_usage_poll`): a line means *this read began*, not *this stream is still
        // open*.
        log.push("$ kubectl top nodes".to_string());
    }
    log.push(format!("$ kubectl get pods{scope} --watch"));
    log.push("$ kubectl get nodes --watch".to_string());
    for kind in ["deployments", "statefulsets", "daemonsets"] {
        log.push(format!("$ kubectl get {kind}{scope} --watch"));
    }
    log
}

/// **Whether an expired serving certificate is why this session read nothing** — `Some` is the
/// message that replaces the whole report, and `None` is every other run.
///
/// **The sentence is `screens/states.md` § Before the TUI ever starts, byte for byte**, including
/// its wrapping and its indent. It is a *more specific* cannot-reach-the-cluster and not a fourth
/// kind of failure, which is that section's own ruling: the wall it replaces is
/// `k8s::Fault::Unanswered` said once per call, and the only thing added is the reason the three
/// have in common. Its last paragraph is what makes it honest — the condition below cannot reach
/// zero false positives behind a load balancer, and that is where the reader is told so.
///
/// **The age and the stamp are two spellings of one `notAfter` and are derived from one value.**
/// [`k8s::Serving::Expired`] carries the date rustls refused the handshake over, so *expired 3
/// days ago* and *ran out on …* cannot disagree the way two independently formatted readings of
/// one instant can. [`in_days`] is the same day count the report trailer prints, and drops the
/// sign because the sentence carries the direction — and it cannot be *counting* backwards here,
/// because rustls refused this handshake against this same machine's clock, so a `now` this
/// message could compare against is already past that `notAfter`.
///
/// **The invariant this function exists to keep is that k8rs never refuses to start on a cluster
/// it could otherwise read** (`k8s-admin`, 2026-08-28). A control plane can run several API
/// servers behind one address, so k8rs's probe can meet an expired replica while the client is
/// being served by a healthy one; a typed expiry that ended the session *by itself* would turn a
/// diagnostic into an outage on a cluster that works. So the reading may only replace the
/// **generic** wording of a session that has comprehensively failed anyway — it never causes a
/// failure, it renames one.
///
/// **The condition, and why each half is in it.**
///
/// * **[`k8s::Serving::Expired`], which is already *no sample completed a handshake*.** A single
///   completed one outranks a typed expiry inside `k8s::Serving::soonest`, so a `Serving::Until`
///   from any of the five samples takes this branch off the table — and a completed handshake is
///   proof that something behind this address serves a certificate a verifying client accepts.
/// * **`get /version` and `get /apis` both came back [`k8s::Fault::Unanswered`].** Not
///   `Refused`: a kubeconfig whose role lacks the `nonResourceURLs` grant gets `403` on both of
///   them and lists pods perfectly well (NOTES § D160), and that run must start. Not a success
///   either, obviously. `Unanswered` is *nothing usable came back*, which is what a refused
///   handshake looks like from above.
///
/// **Two calls and not the watches, because the watches cannot be asked.** kube's `watcher()`
/// never ends and the backoff under it never gives up (`k8s.rs` § THE DRIVER), so there is no
/// moment before the first screen at which their verdict exists. What is available is the two
/// round trips the session already made, and they are the same TLS to the same address.
///
/// **What is left is a residue rather than a hole, and it is priced.** It fires wrongly only when
/// every one of the five probe samples *and* both session calls land on the expired replica.
/// Round-robin over one expired replica of two, that is one run in 128; of three, one in 2187 —
/// and the message's own last paragraph tells that reader to try again. **The probe alone would be
/// one in 32 and one in 243**, which is the whole reason the session's own answers are in the
/// condition — a one-in-32 refusal to start is a tool an operator stops trusting, and the cluster
/// it refuses is working.
fn certificate_is_why(session: &k8s::Session, now: &Time) -> Option<String> {
    let k8s::Serving::Expired(at) = session.serving_expiry else {
        return None;
    };
    let unanswered = |fault: Option<k8s::Fault>| fault == Some(k8s::Fault::Unanswered);
    (unanswered(session.version.as_ref().err().map(k8s::fault))
        && unanswered(session.served.as_ref().err().map(k8s::fault)))
    .then(|| {
        format!(
            "k8rs: the certificate the API server presented expired {} ago

  Not your kubeconfig's — the API server's own, and it ran out on
  {at}. That is why nothing about this cluster
  could be read this run: kubectl and anything else that connects
  to it the normal way is refused too, until someone on the
  control plane renews it — not something k8rs can do.

  If this cluster runs more than one API server behind a load
  balancer, trying again may reach one that still works.",
            in_days(at.duration_since(now.0))
        )
    })
}

/// **Why this run is watching one namespace instead of the cluster**, or `None` when nothing
/// narrowed it — the sentence the security gate's *a 403 degrades that one feature and names the
/// missing verb and resource* row is earned with (NOTES § D5, `PRIOR-ART § B4`).
///
/// **Only the refused arm has anything to say.** `--namespace payments` is the reader's own
/// choice, made a second ago, and the header already prints `ns: payments`; a sentence explaining
/// it back to them is noise. The fallback is the opposite — the reader did not ask for it, may
/// not know their role is namespaced, and the thing they need is the string to hand to whoever
/// owns the cluster.
///
/// **The frame is [`because`]'s**, so the refusal wears the same words here as it does on a
/// watch: one place decides how a `k8s::Fault` reads. The fault is
/// [`k8s::Fault::Refused`] by construction — `k8s::Coverage::Refused` is reachable from nothing
/// else — rather than by a second classification of an error this function does not hold.
///
/// **It does not promise the namespace came from the kubeconfig.** It may be
/// `k8s::FALLBACK_NAMESPACE` where the context named none, so the sentence names the namespace it
/// is watching and does not tell the reader where it was read from — which would be wrong half
/// the time and is not a thing they can act on either way.
///
/// **`--namespace` in it is not a shell command and is not run.** It is the flag they would
/// type, printed the way the usage line prints it.
///
/// # `stopping` is [`ONCE`], and it silences exactly one arm
///
/// **`k8s::Coverage::Blind` says this twice under a mode that ends** (`k8s-admin`, 2026-08-30).
/// That coverage means the cluster-wide LIST *and* the guessed namespace were both refused, so
/// the pod watch is refused too — and [`pods_unread`] then prints the same refusal with the scope
/// and the action in it. Measured, the reader got one fact in two sentences with two different
/// verb sets: `` `list` pods across the whole cluster `` here and `` `list` and `watch` pods ``
/// there, which is the wall of symptoms in miniature.
///
/// **Only that arm, and only that mode.** `k8s::Coverage::Refused` is the run that *works* — pods
/// read, cards drawn — so this is the only line telling the reader why the header says one
/// namespace, and dropping it would lose the sentence they need. `--live` keeps both, because it
/// never reaches an ending that could carry the second.
///
/// **The `bool` is here rather than at the call site because that is where nothing could test
/// it**: `live` writes this to stderr and a test cannot read the process's own stream back, which
/// is the same reason [`greeting`] is a function (2026-08-27) — and the mutation gate said so,
/// with two mutants that survived on the day the `if` was spelled inline.
fn scoped_because(session: &k8s::Session, stopping: bool) -> Option<String> {
    let refused = |asked: &str| because(k8s::Fault::Refused, asked, session.renewal.as_deref());
    match &session.coverage {
        k8s::Coverage::Cluster | k8s::Coverage::Asked(_) => None,
        k8s::Coverage::Blind(_) if stopping => None,
        k8s::Coverage::Refused(namespace) => Some(format!(
            "{} — so k8rs is watching one namespace instead: {}. Pass --namespace <name> for a \
             different one, or ask for cluster-wide read access",
            refused("`list` pods across the whole cluster"),
            sanitize(namespace)
        )),
        k8s::Coverage::Blind(namespace) => Some(format!(
            "{} — and this kubeconfig names no namespace, so k8rs tried {} and was refused there \
             too. Pass --namespace <name> to say which namespace you work in",
            refused("`list` pods across the whole cluster"),
            sanitize(namespace)
        )),
    }
}

/// **The whole of a cluster run, the connection included** — and the one place [`ONCE_DEADLINE`]
/// is turned into a moment the run has to be over by.
///
/// **It exists because the deadline bounded one segment and its doc claimed it bounded the run**
/// (`k8s-admin`, `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 5). Measured: an
/// unroutable endpoint took **140 seconds** and one that accepted TCP and then said nothing was
/// still going at 75. `k8s::connect` and the session reads under it carry kube's 30 s *connect*
/// timeout, but its `read_timeout` default is `None`, so a server that completes the handshake
/// and never speaks hangs before [`live`] arms anything. The old doc excused that as *a change to
/// `k8s.rs`, which is frozen* — true of the timeout `k8s.rs` would have to grow, and not true of
/// the **placement**: both awaits sit in this file, and one `timeout_at` around the first bounds
/// what the second cannot see.
///
/// **An absolute instant and not a second duration**, which is what makes it one budget rather
/// than two. A `timeout` around each segment would give a 30 s connect and *then* a 30 s read;
/// the moment is computed once here and handed to [`live`], so whatever the connection spends is
/// spent out of the same thirty seconds and the sentence at the end is still the per-kind one.
///
/// **What it still does not cover, named rather than left to be discovered:** the six lists an
/// `--analysis` run fetches carry their own [`k8s::REPORT_FETCH`] and are not cut short by this,
/// so that mode can overrun the budget by up to ten seconds. Every other path is inside it.
///
/// **It takes the connect as a future rather than making one**, so a test can hand it something
/// that never finishes without a kubeconfig on the machine — the same reason [`live`] takes what
/// connecting produced instead of doing it. **And it takes the budget rather than reading
/// [`ONCE_DEADLINE`]**, so a test can prove the bound in a fraction of a second instead of
/// spending thirty real ones on a constant that is not what is under test.
async fn cluster_run(
    connecting: impl std::future::Future<Output = Result<k8s::Session, k8s::NotConnected>>,
    analysis: bool,
    once: Option<std::time::Duration>,
) -> Option<String> {
    let Some(whole) = once else {
        return live(connecting.await, analysis, None).await;
    };
    let budget = Budget {
        whole,
        ends_at: tokio::time::Instant::now() + whole,
    };
    match tokio::time::timeout_at(budget.ends_at, connecting).await {
        Ok(connected) => live(connected, analysis, Some(budget)).await,
        // **No store, so no kind to name and no count to report** — [`too_slow`]'s empty shape,
        // which this call is what makes reachable.
        Err(_) => Some(too_slow(&[], wall_clock().ok(), whole)),
    }
}

/// **Whether what each node is using is asked for on a timer** — `k8s::node_usage_poll` merged
/// into the watch loop as a sixth stream (invariant 6).
///
/// **Four rows and each is a different reason, which is why this is a function and not an `if`**
/// (the mutation gate, 2026-08-30: with the condition spelled inline, deleting the `!` changed
/// nothing any test could see). `live` cannot answer it — the poll stream never ends, so a test
/// that drove `--live --analysis` to a conclusion would be waiting for one that cannot come.
///
/// | `--analysis` | [`ONCE`] | polls | why |
/// |---|---|---|---|
/// | no | no | no | `--live` with no Capacity pane on screen would ask every thirty seconds for a paragraph nothing draws |
/// | no | yes | no | the same, and the run is over before a second answer could arrive |
/// | yes | no | **yes** | `--live` redraws, so a metrics-server that starts answering starts showing (NOTES § D181) |
/// | yes | yes | no | one *fetch* at connect instead, because a run that stops has no later pass to reprint with the numbers — the `join!` in [`live`] has the measurement |
fn polls_node_usage(analysis: bool, stopping: bool) -> bool {
    analysis && !stopping
}

/// **The one budget a [`ONCE`] run has, and the moment it is over by.**
///
/// **A pair because the two places that give up inside it need different halves of it**
/// ([`cluster_run`]'s connect and [`live`]'s watch loop): what bounds the wait is the *moment*,
/// so whatever an earlier segment spent is spent out of the same thirty seconds, and what
/// [`too_slow`] prints is the *budget*, because *has not finished answering after 30 seconds* is
/// a fact about what the reader was promised and not about which segment noticed.
///
/// **Carrying both is what stops the sentence drifting from the deadline that produced it.** A
/// single `Instant` would leave the sentence reading a constant that a test can never vary, so a
/// test with a short budget would print `30 seconds` and assert it.
struct Budget {
    whole: std::time::Duration,
    ends_at: tokio::time::Instant,
}

/// **Read the cluster and print the report** — once and exit, or every time it changes until the
/// process is killed.
///
/// **It takes what connecting produced rather than doing it**, so a test can hand it a session
/// over a cluster that is not there: `k8s::connect` needs a kubeconfig and there is none in a
/// test. `main` is left holding one call and no decision.
///
/// # `once` is the whole of the difference between the two modes
///
/// **`None` is [`LIVE`] and `Some(deadline)` is [`ONCE`]**, and one parameter rather than a `bool`
/// beside a constant because the two facts are one fact: a run that stops after the first report
/// is the only run that can be too slow to produce one. `--live` has no deadline for the reason
/// `k8s.rs` has none (NOTES § D150) — it is a screen somebody is looking at — and `--once` has one
/// for the reason a command in a pipeline may not hang ([`ONCE_DEADLINE`]).
///
/// **The stopping point is the bootstrap gate and nothing softer** (NOTES § D28). `k8s::Store`
/// publishes no snapshot until every watch has listed *or settled*, so *the first report drawn
/// over a snapshot* is the first complete answer there is — and `k8s::Store::still_listing` being
/// empty is that same gate, read through the one call `k8s::Store::snapshot` derives it from, not
/// a second copy of it. **A trouble-only pass is not a report here**: `--live` prints those as
/// they happen and `--once` must not, or the one report it exists to print would arrive with the
/// same trouble lines above it twice.
///
/// **A refused watch is a report and exit `0` — unless it is the pod watch.** A watch the cluster
/// refuses *settles* (`k8s::Fault::standing`), the gate opens without it, and the reader gets the
/// cards for the kinds that answered with a line above them naming the verb and resource the
/// missing one needs ([`unreadable`]). That is the ordinary namespaced-`Role` run and exit `2`
/// for it would be k8rs calling its own successful report a failure. **Pods are where every
/// finding starts**, so a run that was never shown one has no report at all and is the `2` row's
/// own *not allowed to list pods* ([`pods_unread`], `screens/once.md` § Exit codes).
///
/// # What the deadline does, which this paragraph used to be silent about
///
/// **The same question is asked again when the budget runs out, and it gets the same answer.**
/// [`pods_unread`] reads *pods produced nothing and here is the typed reason*, which is as true
/// at the deadline as at the gate, so an unreachable cluster ends on the reason rather than on
/// *this cluster has not finished answering* ([`too_slow`], and the arm below has the
/// measurement). Only when pods carry no standing fault is the answer *slow or hung*, which is
/// the one NOTES § D150 refuses to split.
///
/// **What that does not make symmetric is a kind that is wedged rather than refused, and it
/// cannot be made symmetric from this file** (`k8s-admin`,
/// `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 3 vs § 4c). Measured on one
/// cluster: `nodes` refused gives a full report, 41 pods and exit `0`; a `nodes` endpoint that
/// accepts and never answers gives 0 bytes and exit `2`, with the same 41 pods sitting in the
/// store. `k8s::Fault::Refused` settles and opens the gate; a hang settles nothing, so
/// `k8s::Store::snapshot` answers `None` and **there is no report to print** — [`live_report`]
/// over that store has no cards, and a wedged kind with no failure recorded is not even a
/// [`k8s::Trouble`], so it has no line either. *Print the report you have and exit `0`* would
/// print nothing and exit `0`, which turns `k8rs --once && deploy` green on a run that read
/// nothing. Publishing a partial snapshot is `k8s::Store`'s decision and NOTES § D28's, so it is
/// a box against that file and not a fix here.
///
/// # What it returns
///
/// **`None` is the only happy ending in this driver and only [`ONCE`] can reach it** — it ran and
/// reported, so `main` exits `0` whether or not anything was broken (NOTES § D17). `Some` is a
/// sentence for stderr and exit `2`.
///
/// **`--live` still never returns happily**: its two ways out are a kubeconfig that will not
/// connect and every watch having stopped, and the second is unreachable by construction — kube's
/// `watcher()` cannot end (`k8s.rs` § THE DRIVER) and the backoff under it never gives up. A
/// `main` that treated *that* return as an ordinary exit would be the failure `PRIOR-ART § B3` is
/// about, which is why the sentence it comes back with is an error.
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
async fn live(
    connected: Result<k8s::Session, k8s::NotConnected>,
    analysis: bool,
    once: Option<Budget>,
) -> Option<String> {
    use std::io::Write;
    // **Read first, because five decisions below turn on it** — which sentence a `Blind` coverage
    // gets, whether the metrics read is a fetch or a poll, whether the observer stops, whether a
    // failed write has an exit to take, and whether the pump is bounded at all.
    let stopping = once.is_some();
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
            return Some(format!(
                "k8rs: no cluster to watch — {}",
                because(problem.fault(), "reach this cluster", problem.renewal())
            ));
        }
    };
    // **The one session-level reading that ends the run instead of joining the report**
    // (`screens/states.md` § Before the TUI ever starts). It is placed above the greeting because
    // the greeting is the wall it replaces: without it the operator gets *nothing usable came
    // back* once per call, while k8rs is holding the typed error that says why.
    // A clock this machine cannot read costs the sentence and not the run: with no `now` there is
    // no *how long ago*, and what is left is the generic wall. That machine has no report either —
    // [`live_report`]'s loop drops every pass for the same `Err`.
    if let Some(why) = wall_clock()
        .ok()
        .and_then(|n| certificate_is_why(&session, &n))
    {
        // **The wall gets a command log too, and it is the reads that really happened**
        // ([`connect_log`], `k8s-admin`, 2026-09-03). This run ends here, so the report lists and
        // the five watches below never start and may not be named; what did run is the probe, the
        // version and discovery, and this is the screen where a reader most needs to reproduce
        // them by hand. The pods-unread wall further down prints the whole log for the same
        // reason — by then the whole log is true.
        log_to(
            &mut std::io::stderr(),
            connect_log(&session.coverage, session.namespace.as_deref()),
        );
        return Some(why);
    }

    // Read out once, because `session.watches` is moved below and the borrow would not survive
    // it.
    let renewal = session.renewal.clone();
    let renewal = renewal.as_deref();
    // Read out here for the reason `renewal` is: `session.watches` is moved below and the borrow
    // would not survive it. Both are `Copy`, so these are reads and not clones.
    let skew = session.skew;
    // **The date and not the whole reading** ([`k8s::Serving::until`]) — including one the probe
    // met while this session read the cluster fine, which is a replica that has already run out
    // (`screens/once.md` § *A clean tally does not mean every replica is current*). The
    // run-ending case returned above.
    let serving_expiry = session.serving_expiry.until();
    let mut err = std::io::stderr();
    let _ = writeln!(err, "k8rs: watching — {}", greeting(&session).join(" · "));
    // **The one line that says *why* this run is scoped**, and it is on stderr because the cause
    // is the connection's story and the header on stdout already says *which*
    // (`screens/once.md`: both causes print the report identically). A reader who typed
    // `--namespace` needs no sentence; a reader who did not gets the only one there is.
    //
    // **Which arms have anything to say is [`scoped_because`]'s and not this line's**, including
    // the one [`ONCE`] silences because [`pods_unread`] is about to say it better.
    if let Some(narrowed) = scoped_because(&session, stopping) {
        let _ = writeln!(err, "k8rs: {narrowed}");
    }
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
    // **The six lists a report asks for, fetched once and only on a run that draws reports**
    // (`k8s.rs` § WHAT A REPORT ASKS FOR, NOTES § D178). None of them is watched (invariant 6) and
    // there is no pane to open yet, so [`ANALYSIS`] is the closest honest analogue this driver has
    // of a report being opened: a `k8rs --live` without it prints no Certificates, Waste or Drain
    // safety pane, and a request sent for a pane nobody asked for is a request on a path that does
    // not need one.
    //
    // **Once, and never again.** A refusal is a standing fact about this kubeconfig's role —
    // `list certificatesigningrequests` is cluster-scoped, and the five namespaced kinds are read
    // cluster-wide, which a namespaced Role does not grant either — so re-asking per pass is the
    // retry loop the security gate forbids by name (`k8s::Store::unresolved_owners`'s rule,
    // NOTES § D151). What that costs is the ceiling [`k8s::Identity`] already states for the three
    // facts beside it: a kubelet that starts waiting to join after this line has run is not seen
    // until the next connect. The refresh belongs to the phase that has a pane to open one from.
    //
    // **Bounded, and the bound is the reason this line is not where the run stops.** Nothing
    // under it has a read deadline of its own ([`k8s::REPORT_FETCH`]), and this `await` sits
    // *after* the greeting above and *before* the first watch — so an unbounded one prints
    // `k8rs: watching — …` and then nothing, which is a tool that looks connected while it is
    // hung (`tester`, 2026-08-28).
    //
    // **One `join!` and not six `await`s, so the bound stays ten seconds and does not become
    // sixty.** [`k8s::REPORT_FETCH`] bounds one fetch; awaited in a row against a cluster that
    // accepts connections and answers nothing, six of them hold this exact line — greeting
    // printed, no watch started — for a minute, which is the failure the deadline was added to
    // prevent, six times over. `tokio::join!` needs no task and no thread (`k8s::report_lists`).
    //
    // **And the moment is stamped here, because nothing downstream can recover it.** The panes
    // redraw on every watch event off lists that stopped changing on this line, so
    // [`lists_were_read`] needs to say how old they are — and a `None` from a clock this machine
    // could not read is handled there rather than dropped.
    let mut lists_read_at = None;
    if analysis {
        // **Stamped before the `await` and not after it, so the age is never flattering.** The
        // fetch may take the whole of [`k8s::REPORT_FETCH`], and a stamp taken on the way out
        // would call ten-second-old data fresh — under-reporting staleness in the one direction
        // [`lists_were_read`] exists to prevent. Read *at least this old* is the honest claim.
        lists_read_at = wall_clock().ok();
        let (certificates, reports, metrics) = tokio::join!(
            k8s::certificate_requests(&session.client, k8s::REPORT_FETCH),
            k8s::report_lists(&session.client, &session.coverage, k8s::REPORT_FETCH),
            // **What each node is using is a *fetch* under [`ONCE`] and a poll under [`LIVE`],
            // and the difference is that only one of the two has a later pass**
            // (`k8s-admin`, `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 4d). The
            // poll below is a sixth stream merged into the watch loop, and the loop's stopping
            // point is the *five watches'* bootstrap gate — which does not cover it. Measured
            // with metrics-server three seconds slower than the pod LIST, Capacity printed
            // *"That number comes from metrics-server, and k8rs does not read it. Nothing to ask
            // for"* in the same run whose greeting said `{Metrics, …}`: k8rs's own discovery had
            // found the API it was telling the reader it does not read. `--live` reprints with
            // the numbers a moment later and `--once` has no moment later, so it waits here,
            // inside the bound the six lists beside it already carry.
            async {
                match stopping {
                    true => Some(k8s::node_usage(&session.client, k8s::REPORT_FETCH).await),
                    false => None,
                }
            },
        );
        store.certificates_fetched(certificates);
        store.reports_fetched(reports);
        if let Some(metrics) = metrics {
            store.metrics_polled(metrics);
        }
    }
    // **One value, assembled once** ([`AtConnect`]) — every field above is already read out of
    // `session`, which is moved into the watch loop below.
    let at = AtConnect {
        renewal,
        skew,
        serving_expiry,
        lists_read_at,
    };
    // **The one thing on this path that runs on a timer, and it is a stream like the five
    // watches** (`k8s.rs` § WHAT A NODE IS USING, invariant 6). It is merged in rather than
    // spawned so the store needs no lock: every update — watch event and poll alike — lands on
    // this one loop.
    //
    // **Behind [`ANALYSIS`] for the reason the six lists above are**, and behind it for longer:
    // those are read once at connect, this one keeps asking for as long as the run lasts, so a
    // `k8rs --live` with no report on screen would be sending a request every thirty seconds for
    // a paragraph nothing draws.
    //
    // **It is deliberately *not* part of [`lists_were_read`]'s sentence.** That line exists
    // because the panes redraw off lists that stopped changing at connect; this field does not
    // stop changing, so it needs no *how old is this* caveat and would make the one above less
    // true by joining it.
    //
    // **And it is not pushed under [`ONCE`], because a poll is a stream and this run has a
    // stopping point that does not watch it.** That mode read the same number once, above,
    // before the gate could open — the paragraph in the `join!` has the measurement.
    //
    // **`coverage` is read out here for the reason `renewal` and `skew` are**: `session.watches`
    // moves on the next line and the borrow would not survive it. [`pods_unread`] needs it to say
    // where k8rs looked.
    let coverage = session.coverage.clone();
    let mut watches = session.watches;
    if polls_node_usage(analysis, stopping) {
        watches.push(k8s::node_usage_poll(session.client.clone()));
    }
    // **Printed here because this is the first instant every line of it is true**
    // ([`command_log`]): the probe, the version and discovery came back at connect, the seven
    // above have just answered inside their one deadline, and every stream below — the five
    // watches and, under `--analysis`, the metrics poll — is in the vec and about to be polled for
    // the first time. **Below the `push` and not above it**, because on a `--live --analysis` run
    // that poll is merged here rather than fetched in the `join!`, and `$ kubectl top nodes`
    // printed a line higher up would be a promise rather than a read starting
    // (`k8s-admin`, 2026-09-03).
    log_to(
        &mut err,
        command_log(
            analysis,
            &coverage,
            // **The context's own namespace and not [`k8s::Session::namespace_scope`]** — this is
            // the field `k8s::coverage` branched on, and the two differ on every scoped run.
            session.namespace.as_deref(),
        ),
    );
    let mut last = String::new();
    // **What [`ONCE`] came back with, filled in by the closure below**: `None` is *it reported*,
    // `Some` is the one failure that ends the run instead of joining it ([`pods_unread`]).
    let mut ending = None;
    // **`abort()` is not a stop, and this latch is** (`tester` and `k8s-admin`, independently,
    // 2026-08-30). `futures_util::future::Abortable` checks the flag between polls —
    // *"`abort` was called while the task was being polled: the task may still be running and
    // will not be stopped until `poll` returns"* (`abortable.rs:64-68`) — and
    // `k8s::drive_watching`'s `while let Some(update) = merged.next().await` has no yield point
    // between iterations. So every update `select_all` already had in hand lands on this closure
    // in the same poll, passes the now-empty gate, and prints another report whenever the text
    // differs. Measured over a stub answering five empty LISTs: `[1, 2, 6, 6, 2]` reports in five
    // runs, where the doc two paragraphs down said *`--once` prints exactly one thing*. Against a
    // real cluster it printed one 40 times out of 40, which is the timing and not the program.
    let mut done = false;
    // **[`ONCE`] stops the pump from outside it, because `k8s::drive_watching`'s observer
    // deliberately cannot.** That closure returns `()` — "no `Result` for a `?` to sit on and no
    // `bool` for it to stop on", which is what stops the reconnector k9s lost
    // (`PRIOR-ART § B3`) from being killed by an edit — so the stop is an
    // `AbortHandle` around the whole future rather than a value it hands back. `futures-util` is
    // already the crate that supplies `Stream` to this build (NOTES § D143); nothing new is
    // linked and `--live` never touches it.
    let (stop, waiting) = futures_util::future::AbortHandle::new_pair();
    let driving = futures_util::future::Abortable::new(
        k8s::drive_watching(watches, &mut store, |store| {
            // **The latch, and it is the first line because everything under it prints.** See
            // `done`'s own comment: the abort above is a request the pump honours on its next
            // poll, and every update already queued in this one arrives here first.
            if done {
                return;
            }
            // A clock this driver cannot read is not a reason to stop watching; the next event
            // asks again. `wall_clock`'s own `Err` is a machine set before 1970.
            let Ok(now) = wall_clock() else { return };
            // **The bootstrap gate, read through the one call `k8s::Store::snapshot` derives it
            // from** (NOTES § D28). Under [`ONCE`] a pass before the gate opens has nothing
            // complete to print — [`live_report`] would print the trouble lines alone and then
            // print them again inside the report a moment later — so it is skipped whole, and
            // `--once` prints exactly one thing.
            if stopping {
                // Asked only on the mode that stops: `--live` never reads this and should not
                // pay five `progress()` calls per watch event for an answer it drops.
                if !store.still_listing().is_empty() {
                    return;
                }
                // **The one refusal that is not a report** (`screens/once.md` § Exit codes,
                // § When the certificate is why nothing came back): *no permission to list pods*
                // is answered with one sentence and a non-zero exit, never with a list of every
                // symptom on stdout. `--live` prints those symptoms and keeps asking, which is
                // right for a screen somebody is watching and wrong for `k8rs --once && …`.
                if let Some(why) = pods_unread(&store.troubles(), &coverage, at.renewal) {
                    ending = Some(why);
                    done = true;
                    stop.abort();
                    return;
                }
            }
            if let Some(report) = live_report(store, now, &mut last, analysis, &at) {
                // **`--live` drops a failed write and `--once` does not, and the difference is
                // that one of them has an exit to take.** Under `--live` there is nowhere left to
                // report it and no ending to give it, the same reason `main` drops a failed
                // stderr write. Under `--once` the report *is* the run: `k8rs --once >
                // findings.txt` onto a full disk would otherwise leave half a report behind and
                // exit `0` — the truncated-report-claiming-success failure [`stdout_failure`]
                // exists for, which the file-driven path already refuses. `head` closing the
                // pipe stays exit `0`, because that is the pipeline working (NOTES § D17).
                //
                // **The blank line after the report is `--live`'s and not this mode's**: it
                // separates one report from the next, and `--once` has no next
                // (`screens/once.md` § What it prints ends at the tally). It was inherited
                // wholesale when this path grew a stopping point, so a redirected `--once` report
                // ended on two newlines where the file-driven one ends on one (`tester`,
                // `tests/binary.rs`).
                let separator = if stopping { "" } else { "\n" };
                if let Err(failed) = writeln!(std::io::stdout(), "{report}{separator}")
                    && stopping
                {
                    ending = stdout_failure(&failed);
                }
            }
            // **Aborted whether or not [`live_report`] had anything to say**, because *the gate
            // opened* is the condition and *the text changed* is not: a second reason to stop is
            // a second way to not stop.
            if stopping {
                done = true;
                stop.abort();
            }
        }),
        waiting,
    );
    match once {
        None => {
            let _ = driving.await;
            Some(ALL_STOPPED.to_string())
        }
        // **The moment the whole run must be over by, not a fresh thirty seconds**
        // ([`cluster_run`]): the connection and the six lists above have already spent out of it,
        // so this is `timeout_at` and not `timeout`.
        Some(budget) => {
            // **Bound to a name before the `match`, so the borrow of `store` ends with this
            // statement.** A future held as the scrutinee lives for the whole `match`, and the
            // last arm reads the store the closure inside it was writing.
            let outcome = tokio::time::timeout_at(budget.ends_at, driving).await;
            match outcome {
                // Aborted: either the report went to stdout one line up — the only `None` in this
                // driver — or the pod watch produced nothing and [`pods_unread`] wrote the block.
                Ok(Err(futures_util::future::Aborted)) => ending,
                // Every stream ended before the gate opened, so there was nothing to print.
                // Kube's `watcher()` cannot end (`k8s.rs` § THE DRIVER), so this is a test's
                // `stream::iter` running out rather than anything a cluster does — and it is
                // still an exit `2`, because no report was written.
                Ok(Ok(())) => Some(ALL_STOPPED.to_string()),
                // **The typed fault first, and *slow* only when there is not one**
                // (`k8s-admin`, `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 5,
                // `PRIOR-ART § C1`). This arm read [`k8s::Store::still_listing`] and never
                // [`k8s::Store::troubles`], so an endpoint with nothing listening spent thirty
                // seconds and then said *this cluster has not finished answering … run it again*
                // — while the store held `k8s::Fault::Unanswered` on all five watches and
                // `--live` over the identical endpoint had said so in the first second. VPN down,
                // wrong port, API server restarting: the reader is told the cluster is busy and
                // burns another thirty seconds proving it is not. `k8s::Fault::Refused` settles
                // and opens the gate, so it never reaches here; `Unanswered` does not settle
                // (NOTES § D28 — do not blank on a blip) and reaches here every time.
                //
                // **It is not D150's threshold question.** That decision refuses to separate
                // *slow* from *hung* and this does not try to: *not reachable* is a third state
                // and it is a typed fact, so it is reported rather than diagnosed. When pods have
                // no standing fault the sentence below is unchanged, which is D150 intact.
                Err(_) => Some(
                    pods_unread(&store.troubles(), &coverage, at.renewal).unwrap_or_else(|| {
                        too_slow(&store.still_listing(), wall_clock().ok(), budget.whole)
                    }),
                ),
            }
        }
    }
}

/// **The one watch whose failure ends a `--once` run instead of joining its report**, and the
/// block it ends with — or `None` when pods were read.
///
/// **Pods, and only pods, because that is where every finding starts.** Refused `nodes` is the
/// ordinary namespaced-`Role` run and costs two node checks
/// (`reports/2026-08-29-namespace-scope-under-a-real-role.md`); refused Deployments costs an owner
/// name. Unread **pods** leaves [`render`] with no card to draw and no vital it is allowed to
/// print, and a run that exited `0` on that would make `k8rs --once && echo all good` print *all
/// good* about a cluster it was never shown. `screens/once.md` § Exit codes puts *not allowed to
/// list pods* in the `2` row for that reason.
///
/// **It was `pods_refused` until 2026-08-30 and a refusal is only one of the faults it now
/// answers for** (`k8s-admin`, `reports/2026-08-30-once-flag-against-a-live-cluster.md` § 5).
/// [`live`] asks it at the deadline as well as at the gate, so an unreachable cluster — five
/// watches holding `k8s::Fault::Unanswered`, none of them settling, the gate never opening —
/// reaches this instead of being reported as a *slow* one. The name said *refused* while the
/// commonest thing it now catches is a cluster nobody can reach.
///
/// **One block and not a list of symptoms** (`screens/once.md`: `--once` *already answers every
/// other startup failure it can name … with one specific sentence and a non-zero exit, never a
/// list of every symptom*). `--live` prints [`unreadable`]'s line per kind and keeps asking, which
/// is right for a screen somebody is watching; a command in a pipeline gets the one fact, the one
/// action, and the exit code.
///
/// **The shape is `screens/states.md`'s — what is missing, the context, what to do next** —
/// rather than its bytes, and three things in that block are corrected here rather than copied
/// (`k8s-admin`, 2026-08-30). It cites *one of the two roles in the README* and there is no
/// README until Phase 13, so what is named is `docs/security.md`'s `k8rs-readonly`. Its *or run
/// k8rs against a single namespace* is a spent door for a reader who already typed `--namespace`,
/// so the next step is chosen per scope. And **the scope is in the sentence**: without it the
/// reader cannot tell whether to ask for a `Role` or a `ClusterRole`, which is the whole of what
/// they go and request.
///
/// **The reason is [`because`]'s and names the verb and the resource** — the security gate's
/// *a 403 names the missing verb + resource* row — so `list` and `watch` `pods` reach the reader
/// as something to put in a `Role`, and [`plain_kind`] supplies **both** words: the one a reader
/// scans in the display line and the API's own plural in the RBAC clause. Filling both slots from
/// the RBAC half was invisible while this was pods-only and would have printed *did not show k8rs
/// its daemonsets* beside [`unreadable`]'s *DaemonSets* the moment it generalised.
///
/// **Only two faults have a next step, and inventing one for the rest is the fallback this
/// driver refuses** ([`because`]). A refusal is answered with the role to ask for, nothing
/// answering is answered with the address to check; an expired login already carries its own
/// action inside [`because`], and a stream that ended without saying why has no honest one.
///
/// **`listed` is what makes this a failure rather than a blip** (`k8s::Trouble::listed`): a watch
/// that listed once and then broke has stale pods, which is a report with a line above it. At the
/// gate, unlisted means the cluster refuses — `k8s::Store` does not publish until every watch has
/// listed *or settled*. At the deadline it means *and thirty seconds were not enough either*,
/// which is the same thing to tell the reader and the same exit code.
fn pods_unread(
    troubles: &[k8s::Trouble<'_>],
    coverage: &k8s::Coverage,
    renewal: Option<&str>,
) -> Option<String> {
    let unread = troubles
        .iter()
        .find(|trouble| trouble.kind == ObjectKind::Pod && !trouble.listed)?;
    let (kind, resource) = plain_kind(&unread.kind);
    // Read once: the reason and the next step are two readings of one fact, and two calls is
    // where they would come to disagree about which fault this is.
    let fault = unread.fault();
    let why = match fault {
        Some(fault) => because(fault, &format!("`list` and `watch` {resource}"), renewal),
        // `ended` with no failure: the stream finished and never said why. [`unreadable`]'s
        // clause, because it is the same fact and there is only one honest way to say it.
        None => "nothing was ever said about why".to_string(),
    };
    // **Where k8rs looked, in the reader's words** (`k8s::Coverage::namespace`) — the fact that
    // decides whether a `Role` or a `ClusterRole` is what they go and ask for.
    let scope = match coverage.namespace() {
        None => "across the whole cluster".to_string(),
        Some(namespace) => format!("in the namespace {}", sanitize(namespace)),
    };
    let next = match fault {
        Some(k8s::Fault::Refused) => Some(match coverage {
            k8s::Coverage::Cluster => format!(
                "Ask whoever runs this cluster for a role that may read {resource} in every \
                 namespace — `k8rs-readonly` in the k8rs docs is that role — or run k8rs in one \
                 namespace you can read: {NAMESPACE} <name>"
            ),
            // **The one arm where the namespace was not the reader's choice**, so the door the
            // arm below has already spent is the door this one has to open
            // (`k8s::Coverage::Blind`).
            k8s::Coverage::Blind(namespace) => format!(
                "This kubeconfig names no namespace, so k8rs had to guess {} and was refused \
                 there too. Say which namespace you work in: {NAMESPACE} <name>",
                sanitize(namespace)
            ),
            k8s::Coverage::Asked(namespace) | k8s::Coverage::Refused(namespace) => format!(
                "Ask whoever runs this cluster for a role that may read {resource} in {} — the \
                 same rules as `k8rs-readonly` in the k8rs docs, granted in one namespace \
                 instead of all of them",
                sanitize(namespace)
            ),
        }),
        Some(k8s::Fault::Unanswered) => Some(
            "Check the server address this kubeconfig names, and that this machine can reach it"
                .to_string(),
        ),
        _ => None,
    };
    Some(format!(
        "k8rs: this cluster did not show k8rs its {kind}, and every finding starts there, so \
         there is nothing to report\n\n  \
         What k8rs asked for: {kind} {scope}\n  \
         What happened: {why}{}",
        next.map_or(String::new(), |action| format!("\n\n  {action}"))
    ))
}

/// **What `--once` says when the run's budget ran out with no complete answer in it**
/// ([`ONCE_DEADLINE`], [`cluster_run`]) — one sentence on stderr, exit `2`, and nothing at all on
/// stdout.
///
/// **It reports the two facts and diagnoses neither** (NOTES § D150): how many objects each
/// unfinished LIST has decoded, and when the last one arrived. *Slow* and *hung* overlap by
/// construction — `k8s.rs` refuses to pick a threshold between them for exactly this reason — so
/// what a reader gets is the numbers and the one action that separates them, which is to run it
/// again and see whether they moved. A sentence that called this cluster broken would be the
/// threshold that file declined to invent, printed as a verdict.
///
/// **An empty list is [`cluster_run`]'s shape and no longer an unreachable one.** The deadline
/// inside [`live`] fires only while something is listing, but the one around the connection fires
/// before any watch exists — there is no kind to name and no count to report, and *run it again
/// and see whether the counts moved* is advice about numbers that were never printed. That arm
/// names the one thing that is still true and still actionable.
///
/// **`now` is the caller's and may be absent** (invariant 5, NOTES § D18). A machine whose clock
/// will not read loses *when the last one arrived* and keeps the counts, which is
/// [`Input::skew`]'s rule about evidence: no reading is printed as no reading.
///
/// **`0 read so far` never carries an age, and that is a correction rather than a tidy-up**
/// (`k8s-admin` and `tester`, independently, 2026-08-30). `k8s::Listing::since` is stamped by the
/// `Init` that *opens* the watch, so `0` with a stamp is the ordinary reading for a whole first
/// round trip — and the sentence read *0 read so far, the last one 30s ago* when there was no
/// last one for *one* to bind to (invariant 14). Worse, on an unreachable cluster the five ages
/// read `12s, 12s, 10s, 12s, 10s`: numbers that move while nothing whatever arrives, which points
/// D150's *counts that have moved mean it is slow* separator at the wrong answer. `k8s::Watch`
/// carries `Watch::settled` because that same restamping was *"a screen actively lying
/// about progress"* for the refused case.
///
/// **The number of seconds is the caller's** ([`Budget::whole`]), so the sentence cannot drift
/// from the deadline that produced it — and both places that give up inside a run hand over the
/// same one, which is the point of there being a budget type at all.
///
/// **Nothing here is outside text.** The kinds are [`plain_kind`]'s words, the counts are `usize`
/// and the ages are [`age`]'s ladder — a cluster wrote none of it, so nothing needs
/// [`sanitize`].
fn too_slow(listing: &[k8s::Listing], now: Option<Time>, deadline: std::time::Duration) -> String {
    let waited: Vec<String> = listing
        .iter()
        .map(|one| {
            let arrived = now
                .as_ref()
                .filter(|_| one.so_far > 0)
                .and_then(|now| one.since.as_ref().and_then(|since| age(now, since)))
                .map_or(String::new(), |ago| format!(", the last one {ago}"));
            format!(
                "{} ({} read so far{arrived})",
                plain_kind(&one.kind).0,
                one.so_far
            )
        })
        .collect();
    let (still, next) = if waited.is_empty() {
        (
            String::new(),
            "Nothing came back from it at all: check the server address this kubeconfig names, \
             and that this machine can reach it",
        )
    } else {
        (
            format!(" — still reading {}", waited.join(", ")),
            "Run it again: counts that have moved mean it is slow, counts that have not mean \
             nothing is coming",
        )
    };
    format!(
        "k8rs: this cluster has not finished answering after {} seconds, so there is nothing to \
         report{still}. {next}",
        deadline.as_secs()
    )
}

// --- WATCHING A CLUSTER END ---

// --- ONE OBJECT'S LOG START ---
//
// **The headless half of `screens/detail.md`'s logs tab**: Phase 6 has no TUI, so the temporary
// driver prints the tab's payload to stdout the way `--once` prints findings to it, with the same
// split (`screens/once.md` § stdout and stderr are split on purpose) — the lines are the payload,
// the `kubectl` line and everything about *which* container is being read are the teaching device
// on stderr.
//
// **One selector, and it names an object rather than a log** (NOTES § D194). Logs, the per-object
// events fetch, `describe` and YAML are four consumers of one answer — *which object* — and four
// spellings of that question is how they come to disagree, so [`OBJECT`] is designed here for all
// four even though only [`LOGS`] reads it today. It is scaffolding and dies with this file at
// Phase 12, which is where `namespace_arg`'s own doc already sends the real parsing.
//
// **Two shapes and not one, because only one of them can lose a line**
// (`screens/detail.md` § Printed instead of drawn). A fetch fills [`k8s::LogLines`], which is
// bounded and says out loud how many lines it dropped; a follow prints each line as it arrives
// and forgets it, holds no log bytes between two lines, and so has nothing to report. That is the
// screen's own rule: only a driver with no lossy buffer gets to print nothing there.

/// The flag that says *print this object's log*.
///
/// **A verb beside [`OBJECT`] rather than a flag that carries the object itself.** `--logs <pod>`
/// would read better today and would make `--describe <pod>` and `--yaml <pod>` the next two
/// spellings of *which object* — which is the thing NOTES § D194 names as how four consumers come
/// to disagree. `--describe`, `--yaml` and the events fetch are the next three verbs, and each is
/// a valueless flag like this one beside the same [`OBJECT`].
const LOGS: &str = "--logs";

/// **Which object this run is about** — `<namespace>/<name>`, or `<name>` with the namespace
/// coming from `--namespace` or the context (NOTES § D194).
///
/// **`namespace/name` is the product's own spelling of an object**, printed that way on every
/// screen and in every card, so a reader types back what k8rs showed them.
///
/// **Both spellings, for [`CONTEXT`]'s reason**: matching only `--object NAME` lets
/// `--object=NAME` fall through, and a selector that silently selects nothing is worse than one
/// that refuses.
const OBJECT: &str = "--object";

/// **Which container of the pod to read** — `kubectl`'s own long spelling, so somebody who
/// types `kubectl logs -c app` all day types the same word here.
///
/// **The short `-c` is deliberately absent.** `-npayments` is refused one flag over because a
/// one-dash cluster silently means the wrong thing, and adding a second one-dash flag to this
/// scaffolding buys nothing the long spelling does not.
const CONTAINER: &str = "--container";

/// `kubectl`'s spelling of *the log from before the last crash*, which is the log a crash loop
/// needs (`screens/detail.md`).
const PREVIOUS: &str = "--previous";

/// **Follow the stream instead of printing what is there and stopping** — `kubectl`'s `-f`, spelled
/// long for [`CONTAINER`]'s reason.
const FOLLOW: &str = "--follow";

/// **How long any one read about the object a run named may take** — every request on all three
/// verbs: the pod before a log stream, the re-read after a followed stream ends, `--describe`'s
/// pod and its events, and `--yaml`'s document.
///
/// **One number for all of them, because they are one kind of wait**: a reader who typed
/// `--object` is waiting for one object, and a per-verb budget would be three numbers nothing
/// tells apart. It was `OBJECT_READ` while `--logs` was the only verb.
///
/// **A stream is deliberately not bounded by it**: a follow that ended after ten seconds would be
/// a `--follow` that does not follow. What is bounded is every request that has an answer to wait
/// for, which is the shape `k8s::REPORT_FETCH` already names on the startup path — an unbounded
/// one against a cluster that accepts connections and answers nothing prints the `kubectl` line
/// and then hangs with no output at all.
const OBJECT_READ: std::time::Duration = k8s::REPORT_FETCH;

/// **What follows a flag that takes a value**, in either spelling — the one loop
/// [`namespace_arg`], [`object_arg`] and [`container_arg`] all are.
///
/// **`Some(None)` is the flag with nothing usable after it**, which is a real state and the
/// commonest way to reach it is `--object "$POD"` with `POD` unset. **First wins on a repeat**,
/// which is [`live_context`]'s rule and not `kubectl`'s (`kubectl` is last-wins); it is written
/// down because an unwritten tie-break is the one that changes by accident, and Phase 12's real
/// parsing is where the two should be made to agree.
///
/// **Nothing here judges the value.** [`mistyped`] does, once, so there is one sentence per flag
/// and one place it comes from — which is the whole reason this is one function: two parsers over
/// one flag is how a run gets refused for a value it was not about to use.
///
/// `flags` is a slice because `--namespace` has two spellings and the others have one.
fn value_of<'a>(args: &'a [String], flags: &[&str]) -> Option<Option<&'a str>> {
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        // `--flag=VALUE`. Written as a strip per flag rather than a second literal per flag, so
        // each flag is spelled once in this file.
        for flag in flags {
            if let Some(attached) = arg
                .strip_prefix(flag)
                .and_then(|rest| rest.strip_prefix('='))
            {
                return Some(Some(attached));
            }
        }
        if flags.contains(&arg.as_str()) {
            return Some(rest.next().map(String::as_str));
        }
    }
    None
}

/// **Which object this line names**, or `None` when [`OBJECT`] is not on it ([`value_of`]).
fn object_arg(args: &[String]) -> Option<Option<&str>> {
    value_of(args, &[OBJECT])
}

/// **Which container this line names**, or `None` when [`CONTAINER`] is not on it ([`value_of`]).
fn container_arg(args: &[String]) -> Option<Option<&str>> {
    value_of(args, &[CONTAINER])
}

/// **An object's namespace and its name**, split on the one `/` the spelling has.
///
/// **The first `/` and not the last**, so `payments/web/oops` splits as
/// `("payments", "web/oops")` and the name half is then refused by `k8s::object_name` — a name
/// with a slash in it is a request path this driver would otherwise write for somebody else (the
/// security gate's *names build paths* row). Splitting on the last `/` would have handed
/// `k8s::object_name` a clean `oops` and quietly read the wrong pod.
fn split_object(value: &str) -> (Option<&str>, &str) {
    match value.split_once('/') {
        Some((namespace, name)) => (Some(namespace), name),
        None => (None, value),
    }
}

/// **Everything a log run needs off the command line, as one value** — which object, which
/// container, and the two switches.
///
/// **The namespace is already resolved as far as the command line can resolve it**: the
/// `namespace/name` half of [`OBJECT`] wins over `--namespace`, because it is the more specific
/// of the two things the reader typed. What is left — a run that named neither — is the session's
/// to answer, and [`logs_run`] does it there because only it has a session.
struct Asked<'a> {
    /// **Which of the three this run is** — one field, so [`on_cluster`] holds a `match` with no
    /// arm that can be reached by two flags at once ([`mistyped`] has already refused that).
    verb: Verb,
    namespace: Option<&'a str>,
    /// The object's own name, the right half of [`OBJECT`]. **`name` and not `pod`**: two of the
    /// three verbs are pod-only and [`YAML`] is not, and a field called `pod` holding a Secret's
    /// name is the sort of thing a later reader trusts.
    name: &'a str,
    /// **Which kind [`YAML`] reads**, `None` for a line that named none — [`POD`] is the default
    /// and it is applied where the kind is resolved, not here, so this field says what was
    /// *typed*. [`LOGS`] and [`DESCRIBE`] do not read it ([`mistyped`] refuses a [`DESCRIBE`]
    /// that names anything else).
    kind: Option<&'a str>,
    container: Option<&'a str>,
    previous: bool,
    follow: bool,
}

/// **The log run this line asked for, or `None` when it asked for something else.**
///
/// **Reached only after [`mistyped`] has passed**, which is what makes the values safe to hand on
/// without a second check here — the same contract [`live_namespace`] already has.
fn asked(args: &[String]) -> Option<Asked<'_>> {
    let verb = Verb::of(verbs(args).first()?)?;
    let (namespace, name) = split_object(object_arg(args).flatten()?);
    Some(Asked {
        verb,
        namespace: namespace.or_else(|| live_namespace(args)),
        name,
        kind: kind_arg(args).flatten(),
        container: container_arg(args).flatten(),
        previous: args.iter().any(|arg| arg == PREVIOUS),
        follow: args.iter().any(|arg| arg == FOLLOW),
    })
}

/// **Which of the two cluster runs this line is**, so `main` holds one call and no decision.
///
/// **One `connect` and not two**: both runs read the same kubeconfig, the same `--context` and the
/// same `--namespace`, and a second call site is a second place one of the three can be forgotten.
///
/// **[`LOGS`] wins over [`ONCE`] and [`LIVE`], which is the same tie-break `--once --live`
/// already has**: the narrower of the two runs. `--once` reads the whole cluster and `--logs`
/// reads one object somebody named, so a line carrying both asked for the object.
async fn on_cluster(args: &[String], context: Option<&str>) -> Option<String> {
    let connecting = k8s::connect(context, live_namespace(args));
    match (verbs(args).is_empty(), asked(args)) {
        (false, Some(asked)) => match asked.verb {
            Verb::Logs => logs_run(connecting, &asked).await,
            // **The clock is read here and handed down as a value** (invariant 5, NOTES § D18) —
            // and before anything is connected, so a machine whose clock will not read says so
            // instead of dialling a cluster it cannot date anything against.
            Verb::Describe => match wall_clock() {
                Ok(now) => describe_run(connecting, &asked, &now, OBJECT_READ).await,
                Err(problem) => Some(format!("k8rs: {problem}")),
            },
            Verb::Yaml => yaml_run(connecting, &asked).await,
        },
        // **A verb with no object never reaches here** — [`mistyped`] refuses the pair before
        // the mode is chosen — and the arm exists so that an edit which lets one through is a
        // usage error rather than a `--live` that watches forever with nothing on screen. It is
        // dispatched on the *flag* and not on [`asked`]'s answer for exactly that reason.
        (false, None) => Some(USAGE.to_string()),
        (true, _) => {
            cluster_run(
                connecting,
                analysis_wanted(args),
                once_wanted(args).then_some(ONCE_DEADLINE),
            )
            .await
        }
    }
}

/// **The plain-language phrase for a state word, or `None` for one no table names.**
///
/// **A short list per surface and a fall-through, never a guess.** A reason with no phrase prints
/// as its own raw word beside the controller's message ([`raw_and_message`]) — which is strictly
/// more informative than an invented sentence and cannot be false, the discipline NOTES § D198
/// generalised from `BackOff` to everything.
fn phrase(table: &'static [(&'static str, &'static str)], reason: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(word, _)| *word == reason)
        .map(|(_, said)| *said)
}

/// **`(Evicted) The node was low on resource: ephemeral-storage.`** — the raw API word and the
/// controller's verbatim message, which is the second line of every *word that explains a state*
/// block on this surface: a pod's own reason, a container's, and an event's
/// (`screens/detail.md` — *three separate inventions here would be three things to keep agreeing*).
///
/// **The message is never replaced and never summarised** (NOTES § D37, § D198). A missing one
/// costs the space and nothing else: `(Evicted)` alone is what a pod prints today, because
/// `status.message` is not a field `rules.rs` carries and that file is frozen.
fn raw_and_message(reason: &str, message: Option<&str>) -> String {
    let said = message.map_or(String::new(), sanitize);
    // **An empty reason draws no empty brackets.** The API allows an Event with no `reason`, and
    // `()` in front of a message is a word this file invented out of a field that was not there.
    match sanitize(reason).as_str() {
        "" => said,
        word => format!("({word}) {said}").trim_end().to_string(),
    }
}

/// **The only `status.reason` this build translates** (`screens/detail.md` § The pod's own reason).
///
/// **One entry, because one is what has been measured.** Anything else the field can hold falls
/// through to its raw word beside the message, which is the safe fallback every table on this
/// surface uses.
const POD_REASONS: &[(&str, &str)] = &[("Evicted", "removed by the node to take back room")];

/// **The only terminated reason this build translates** — invariant 14's own worked example
/// (`CLAUDE.md`: `OOMKilled` reads *container exceeded its memory limit*, not the raw word).
///
/// **Everything else falls through to the exit code alone, never a guessed word**
/// (`screens/detail.md`). `Error`, `ContainerCannotRun` and the empty string a real container can
/// carry — `k8s-admin` measured `reason=Error, exit=1` and a bare `exit=255` with nothing in
/// `reason` on one pod — say no more than the number already does.
const STOPPED_REASONS: &[(&str, &str)] = &[("OOMKilled", "container exceeded its memory limit")];

/// **The waiting reasons this build translates**, each phrase derived from the card `rules.rs`
/// already draws for the same state rather than invented beside it — rule 1's *keeps crashing*,
/// rule 3's *image is not usable, so the container never started*, rule 4's *needs a ConfigMap or
/// Secret that does not exist*.
///
/// **The other five of rule 3's seven image reasons are not here**, and that is a limit rather
/// than a decision: `UNUSABLE_IMAGE` is private to the frozen `rules.rs`, so the two
/// `screens/detail.md` names by name are the two spelled here and `InvalidImageName` and its
/// siblings fall through to their own raw word — which is honest and is what the fall-through is
/// for.
const WAITING_REASONS: &[(&str, &str)] = &[
    ("CrashLoopBackOff", "keeps crashing and restarting"),
    ("ImagePullBackOff", "cannot get its image"),
    ("ErrImagePull", "cannot get its image"),
    (
        "CreateContainerConfigError",
        "needs a ConfigMap or Secret that does not exist",
    ),
];

/// **What one container's row says on `--describe`** — the word after its name, and the indented
/// line under it where there is one (`screens/detail.md` § The describe tab).
///
/// **This is not [`doing`] and deliberately does not replace it.** That function's doc argues
/// correctly that the jargon card is the Alerts view's — and it was written for
/// [`container_choice`], the **log picker**, where that card is one keypress away. Describe is the
/// headless surface: there is no card in the same output, so `waiting` printed alike for
/// `ImagePullBackOff`, `CrashLoopBackOff` and `CreateContainerConfigError` is the whole of what a
/// reader gets (`k8s-admin`, 2026-08-31). **The picker's wording is unchanged**; this is a second
/// reader of one state, not a second spelling of one sentence.
///
/// **`done` is not renamed to `failed` before it earns the word.** A clean `exit 0` is the healthy
/// case and stays `done`; measured, three containers that exited 1, 0 and 255 all printed `done`,
/// and `done` is a false statement about the third.
///
/// **A momentary `ContainerCreating` stays the calm `not started`** rather than being dressed up
/// as a problem — it is the ordinary first second of every pod.
fn container_state(state: Option<&ContainerState>) -> (String, Option<String>) {
    match state {
        Some(ContainerState::Running { .. }) => ("running".to_string(), None),
        Some(ContainerState::Terminated(stopped)) if stopped.exit_code == 0 => {
            ("done".to_string(), None)
        }
        Some(ContainerState::Terminated(stopped)) => {
            let said = stopped
                .reason
                .as_deref()
                .and_then(|reason| phrase(STOPPED_REASONS, reason))
                .map_or(String::new(), |phrase| format!("{phrase} — "));
            (
                "failed".to_string(),
                Some(format!("{said}exit {}", stopped.exit_code)),
            )
        }
        Some(ContainerState::Waiting { reason, .. }) => {
            let word = match reason.as_deref() {
                // The kubelet has taken the pod and is making the sandbox: nothing is wrong yet.
                Some("ContainerCreating" | "PodInitializing") => "not started".to_string(),
                Some(reason) => {
                    phrase(WAITING_REASONS, reason).map_or_else(|| sanitize(reason), str::to_string)
                }
                None => "waiting".to_string(),
            };
            (word, None)
        }
        // **A container the pod declares and the kubelet has not reported on** — a `Pending` pod.
        None => ("not started".to_string(), None),
    }
}

/// **`, 3 restarts`, or nothing at all** — the one spelling of a fact two screens draw
/// (`screens/detail.md` says outright they are one rule).
///
/// **It was written twice, byte for byte, in [`container_choice`] and [`described`]**, and a third
/// reader already disagreed with both: [`no_previous_run`] compares `restarts != 0` on the raw
/// `i32` where the two display sites floor a negative one to zero, so a `restartCount` below zero
/// would have had two screens say *no restarts* while `--previous` said it had restarted
/// (`k8s-admin`, 2026-08-31). No API server produces one — which is why this is an extraction and
/// not a fix — but one field with three readers and one already out of step is the family shape
/// this repo pays most for.
///
/// **`restartCount` is an `i32` the API server never sets below zero**; a negative one is not a
/// count and is drawn as none rather than as its absolute value. **A container the kubelet has not
/// reported on has no count at all**, which is not a zero it chose.
fn restarts(status: Option<&ContainerSnapshot>) -> String {
    match status
        .map(|container| usize::try_from(container.restarts).unwrap_or(0))
        .unwrap_or(0)
    {
        0 => String::new(),
        counted => format!(", {}", plural(counted, "restart")),
    }
}

/// **What a container is doing, in one word a beginner reads** (invariant 14).
///
/// **A waiting container's `reason` is not printed here.** It is the jargon this product exists to
/// translate — `CrashLoopBackOff`, `ImagePullBackOff` — and the card that explains it is the
/// Alerts view's, not a line above a log.
fn doing(state: Option<&ContainerState>) -> &'static str {
    match state {
        Some(ContainerState::Running { .. }) => "running",
        Some(ContainerState::Terminated(_)) => "done",
        Some(ContainerState::Waiting { .. }) => "waiting",
        // **A container the pod declares and the kubelet has not reported on** — a `Pending`
        // pod, which the picker can now list because the list comes from `spec` and not from
        // `status` (`k8s::PodRead`). *waiting* would be a claim the kubelet has not made.
        None => "not started",
    }
}

/// **The container the log is read from**, or the sentence saying why there is none
/// (`screens/detail.md` § Choosing a container).
///
/// **The pod's *declared* containers and never its reported ones** ([`k8s::PodRead`]). The
/// kubelet sorts `status.containerStatuses` by name and `spec.containers` keeps the author's
/// order, so choosing off the snapshot opened `alpha` where `kubectl logs` opens `zeta`, and
/// `[web, envoy]` opened the proxy (`k8s-admin`, 2026-08-30).
///
/// **`Ok(None)` is a pod that declares no container at all**, which the API server refuses
/// (NOTES § D156, ruling 1), so it is a `Pod` whose `spec` did not decode rather than one a
/// cluster serves: the request then names none and the server picks. A `Pending` pod is **not**
/// this case any more — it has a `spec` like every other pod.
///
/// **A name that is not one of the pod's is refused with the list**, because the reader's next
/// action is to retype it and the only thing they need is the spelling.
fn which_container<'a>(
    read: &'a k8s::PodRead,
    asked: Option<&str>,
) -> Result<Option<&'a str>, String> {
    let Some(name) = asked else {
        return Ok(read.default_container());
    };
    match read.declared().find(|held| *held == name) {
        Some(chosen) => Ok(Some(chosen)),
        None if read.declared().len() == 0 => Err(format!(
            "k8rs: this pod declares no container at all, so there is no {} to read",
            sanitize(name)
        )),
        None => Err(format!(
            "k8rs: this pod has no container named {} — it has {}",
            sanitize(name),
            container_names(read)
        )),
    }
}

/// Every container the pod declares, by name, in `spec` order — the regular ones and then the
/// init containers, which is the order `kubectl` lists them in after *Defaulted container … out
/// of:* and the order `screens/detail.md`'s picker draws.
fn container_names(read: &k8s::PodRead) -> String {
    read.declared().map(sanitize).collect::<Vec<_>>().join(", ")
}

/// **The headless form of the container picker** — what there was to choose from and what was
/// chosen, or `None` when there was nothing to choose (`screens/detail.md` § Choosing a container).
///
/// **Silent on a single-container pod**, which is the screen's own invariant one layer up: it does
/// not offer the picker at all, because a key that does nothing is a bug already shipped once here.
/// Silent too when the reader named the container, because they know.
///
/// **The restart count is beside the containers that have one**, for the screen's reason: that is
/// exactly the signal that makes [`PREVIOUS`] worth typing.
///
/// **The list is the pod's declared one and the state beside each name is looked up by name**
/// ([`k8s::PodRead::status`]) — one place decides what the pod's containers are, so the picker,
/// the refusal above it and the request all name the same set in the same order.
fn container_choice(
    read: &k8s::PodRead,
    asked: Option<&str>,
    chosen: Option<&str>,
) -> Option<String> {
    let chosen = chosen?;
    if asked.is_some() || read.declared().len() < 2 {
        return None;
    }
    let listed: Vec<String> = read
        .declared()
        .map(|name| {
            let status = read.status(name);
            format!(
                "{} ({}{})",
                sanitize(name),
                doing(status.map(|container| &container.state)),
                restarts(status)
            )
        })
        .collect();
    Some(format!(
        "k8rs: this pod has {} — {}\nk8rs: reading {}. Name another with `{CONTAINER} <name>`.",
        plural(read.declared().len(), "container"),
        listed.join(", "),
        sanitize(chosen)
    ))
}

/// **What [`PREVIOUS`] says when there is no previous run to show** — `screens/detail.md`'s own
/// words, and `None` when there is one or when it was not asked for.
///
/// **k8rs does not print the API's refusal and does not leave the flag pointed at nothing.** The
/// container has never restarted, so there is no earlier run to serve and the API server refuses
/// the request — in its own words, about a request the reader did not knowingly make. What that
/// refusal says exactly is not quoted here, because nothing in this repo has measured it; what is
/// measured is that k8rs stops sending it
/// (`previous_on_a_container_that_never_restarted_asks_for_the_run_that_exists`). It says so in
/// one line and falls back to the run that does exist.
///
/// **A container the kubelet has not reported on has not restarted either**, so it takes the same
/// line — which is right and is also what stops `--previous` reaching a `Pending` pod, where the
/// API server has nothing to serve it from.
fn no_previous_run(read: &k8s::PodRead, chosen: Option<&str>, previous: bool) -> Option<String> {
    let chosen = chosen?;
    let restarts = read
        .status(chosen)
        .map_or(0, |container| container.restarts);
    if !previous || restarts != 0 {
        return None;
    }
    Some(format!(
        "k8rs: {} hasn't restarted, so there's no previous run to show. Showing the current run \
         instead.",
        sanitize(chosen)
    ))
}

/// **The marker a followed stream ends with**, or `None` when there is nothing honest to say.
///
/// **`reread` is what the cluster answered when asked for the pod again** — the outer `None` is a
/// re-read that did not answer inside [`OBJECT_READ`], `Err` a failure, `Ok` the pod itself. Two
/// shapes are the same answer: a `404`, and a pod that is still there and **carrying a
/// `deletionTimestamp`**.
///
/// **The second is the one that actually happens, and without it this marker never fired at
/// all.** The stream ends when the container dies; the object outlives it by its grace period, so
/// the re-read *succeeds* — measured twice, deleting four seconds into a follow: at `grace 1s`
/// the pod still had a `deletionTimestamp` at t+1 and t+2, at `kubectl`'s default `grace 30s` at
/// t+1 through t+6, and both runs ended with the last log line, nothing, and exit `0`
/// (`k8s-admin`, 2026-08-30). So `screens/detail.md`'s deleted-pod mockup and the one case
/// `PRIOR-ART § E1` names by name were unreachable in the ordinary case, and the test that should
/// have said so hand-built a `Fault::Gone` the pipeline does not produce (NOTES § D29).
///
/// **A pod still terminating is *deleted* and not *being deleted*, on purpose.** The container
/// whose log this was is already gone — that is why the stream ended — and the difference between
/// *deleted* and *deleted, object not yet collected* is not one a reader at 3am has an action
/// for. `screens/detail.md`'s wording is the one printed.
///
/// **Every other ending gets no marker at all**, and that is deliberate rather than unfinished: a
/// stream that ended while the pod is still there ended because the container stopped writing, a
/// middlebox timed out, or the connection broke — three different facts this driver cannot tell
/// apart, and inventing one sentence for all three is the *viewer says one thing* failure E1 is
/// about, wearing the other coat.
fn stream_ended(reread: Option<Result<&PodSnapshot, k8s::Fault>>) -> Option<&'static str> {
    let gone = match reread? {
        Err(fault) => fault == k8s::Fault::Gone,
        Ok(pod) => pod.deletion_timestamp.is_some(),
    };
    gone.then_some("--- stream ended: pod deleted ---")
}

/// **The line a log with nothing in it gets, and `None` when something arrived**
/// (`screens/detail.md` § No logs yet, `PRIOR-ART § E1`).
///
/// **A state, not a hang and not an error**: a `Pending` pod, or a container that just started.
/// It is on stderr because stdout is the payload and the payload really is empty, so
/// `k8rs --logs … | wc -l` still answers `0` and the reader still learns why.
///
/// **A function over a `bool` rather than a count compared to zero.** Counting is what the two
/// arms of [`logs_run`] used to do, and every arithmetic mutant of that count survived the gate
/// because nothing a test can read depends on the number — only on whether it is zero
/// (`dev-core`'s run, 2026-08-30). The number was never the question.
fn nothing_written(arrived: bool) -> Option<&'static str> {
    match arrived {
        true => None,
        false => Some("k8rs: nothing has been written to this container's log yet"),
    }
}

/// **The whole of a fetched log on stdout**: how many lines were lost first, then what is left.
///
/// **The dropped-lines sentence is above the content and not below it**, because that is literally
/// where the gap is — the lines missing are the oldest, which would have been above what is now
/// the first line (`screens/detail.md` § When the buffer fills).
///
/// **It is payload and goes to stdout with the lines**, not to stderr with the teaching device:
/// it is a fact about the log itself, and a reader piping this somewhere needs to know it arrived
/// short (`screens/once.md` § stdout and stderr are split on purpose).
fn dump(held: &k8s::LogLines, out: &mut impl std::io::Write) -> std::io::Result<()> {
    if let Some(dropped) = held.dropped_line() {
        writeln!(out, "{dropped}")?;
    }
    held.lines().try_for_each(|line| writeln!(out, "{line}"))
}

/// **Print one container's log and stop, or follow it until it ends** — the headless logs tab.
///
/// **It takes the connect future rather than a session**, like [`cluster_run`], so a test can hand
/// it a session over a cluster that is not there: `k8s::connect` needs a kubeconfig and there is
/// none in a test.
///
/// **`None` is the happy ending and `main` exits `0`** — including a log with nothing in it, which
/// is a state and not a failure. `Some` is a sentence for stderr and exit `2`.
///
/// **Nothing here is bulk and nothing here writes.** Every request is one object named on the
/// command line, and the two verbs are `get` and `get pods/log`.
async fn logs_run(
    connecting: impl std::future::Future<Output = Result<k8s::Session, k8s::NotConnected>>,
    asked: &Asked<'_>,
) -> Option<String> {
    use std::io::Write;
    let session = match opened(connecting).await {
        Err(sentence) => return Some(sentence),
        Ok(session) => session,
    };
    let renewal = session.renewal.clone();
    let renewal = renewal.as_deref();
    let namespace = match in_namespace(asked.namespace, &session) {
        Err(sentence) => return Some(sentence),
        Ok(namespace) => namespace,
    };
    let mut err = std::io::stderr();

    // **The pod before the log, because the container picker and [`PREVIOUS`] are both questions
    // about it** — which containers there are, and which of them has a previous run at all.
    let pod = match tokio::time::timeout(
        OBJECT_READ,
        k8s::pod(&session.client, namespace, asked.name),
    )
    .await
    {
        Ok(Ok(pod)) => pod,
        Ok(Err(failure)) => {
            return Some(read_failed(
                &failure,
                POD,
                asked.name,
                Some(namespace),
                renewal,
            ));
        }
        Err(_) => {
            return Some(no_answer(
                POD,
                asked.name,
                Some(namespace),
                OBJECT_READ.as_secs(),
            ));
        }
    };

    let chosen = match which_container(&pod, asked.container) {
        Ok(chosen) => chosen,
        Err(sentence) => return Some(sentence),
    };
    if let Some(block) = container_choice(&pod, asked.container, chosen) {
        let _ = writeln!(err, "{block}");
    }
    let mut previous = asked.previous;
    if let Some(sentence) = no_previous_run(&pod, chosen, previous) {
        let _ = writeln!(err, "{sentence}");
        previous = false;
    }

    // **One value, and both records are built off it** (invariant 4): what goes on the wire is
    // `k8s::LogRequest::params` and what the reader is taught is `k8s::LogRequest::kubectl`, so
    // the line printed here cannot describe a request that was not sent.
    let request = k8s::LogRequest::new(namespace, asked.name, chosen, previous, asked.follow);
    let _ = writeln!(err, "{}", request.kubectl());

    let reader = match k8s::log_stream(&session.client, &request).await {
        Ok(reader) => reader,
        Err(failure) => {
            return Some(format!(
                "k8rs: {}",
                because(
                    k8s::fault(&failure),
                    &format!("get pods/log in {}", request.namespace),
                    renewal
                )
            ));
        }
    };

    let mut ending = None;
    // **Whether anything at all came out of the stream**, which is the only thing either arm has
    // to remember about how much did ([`nothing_written`]).
    let mut arrived = false;
    let mut out = std::io::stdout();
    let read = if request.follow {
        // **Printed and forgotten, so nothing is retained and nothing can be dropped**
        // (`screens/detail.md` § Printed instead of drawn) — which is why this arm has no
        // dropped-lines sentence to print and is not silently missing one.
        //
        // **A line reaches the reader when it is written and not when the run ends**, which is
        // what makes `--follow` follow rather than fetch. `std::io::Stdout` is a `LineWriter`, so
        // the `\n` is the flush and nothing here has to ask for one — **measured through a pipe
        // rather than read off the type**, because that is where a block-buffered stdout would
        // hide: against a server sending one line every 300ms the four lines were stamped at
        // 0.59s, 0.89s, 1.19s and 1.49s (`dev-core`, 2026-08-30).
        k8s::read_lines(reader, |line| {
            arrived = true;
            match writeln!(out, "{line}") {
                Ok(()) => true,
                // **`false` stops the read**, and that is what makes `k8rs --logs --follow | head`
                // end rather than drain a socket nobody is listening to for as long as the
                // container keeps writing. `BrokenPipe` costs nothing and exits `0`
                // ([`stdout_failure`]).
                Err(failed) => {
                    ending = stdout_failure(&failed);
                    false
                }
            }
        })
        .await
    } else {
        let mut held = k8s::LogLines::default();
        let read = k8s::read_lines(reader, |line| {
            held.push(line);
            true
        })
        .await;
        arrived = held.arrived();
        if let Err(failed) = dump(&held, &mut out) {
            ending = stdout_failure(&failed);
        }
        read
    };
    // **A log that stopped arriving is not a log that ended** (`PRIOR-ART § E1`). Swallowed, a
    // connection reset half way through prints what arrived and exits `0`, which is
    // [`stdout_failure`]'s *truncated report claiming success* on the other stream. What is above
    // stays on stdout — those bytes are real — and the reader is told it is not all of it.
    if let (Err(failed), None) = (read, &ending) {
        return Some(format!(
            "k8rs: the log stopped arriving before it ended, so what is above is not all of it — \
             {}",
            sanitize(&failed.to_string())
        ));
    }
    if let Some(sentence) = nothing_written(arrived) {
        let _ = writeln!(err, "{sentence}");
    }
    // **Why a followed stream ended is asked of the cluster and not of the bytes**, which is the
    // one thing `PRIOR-ART § E1` asks for by name ([`stream_ended`]). A fetch does not ask: it
    // ended because the log ended, which is what a fetch is.
    if request.follow && ending.is_none() {
        // **The pod itself and not only its failure** ([`stream_ended`]): the object outlives the
        // container by its grace period, so the ordinary delete answers `200` with a
        // `deletionTimestamp` and never the `404` this used to be the whole of.
        let answer = tokio::time::timeout(
            OBJECT_READ,
            k8s::pod(&session.client, &request.namespace, &request.pod),
        )
        .await;
        let reread = answer.as_ref().ok().map(|answer| {
            answer
                .as_ref()
                .map(|read| &read.snapshot)
                .map_err(k8s::fault)
        });
        if let Some(marker) = stream_ended(reread) {
            let _ = writeln!(out, "{marker}");
        }
    }
    ending
}

// --- ONE OBJECT'S LOG END ---

// --- ONE OBJECT'S OWN STORY START ---
//
// **The headless half of `screens/detail.md`'s describe and yaml tabs**, beside the logs tab
// above and split the same way: the payload on stdout, the `kubectl` line and everything about
// *how* it was read on stderr (`screens/once.md` § stdout and stderr are split on purpose).
//
// **Three verbs over one [`OBJECT`] and one refusal between them** (NOTES § D194). `--logs`,
// `--describe` and `--yaml` all narrow to one object the reader named, so a line carrying two of
// them is refused by [`mistyped`] rather than ranked — the reasoning is there, at the check.
//
// **`--describe` reads a typed pod and `--yaml` reads an untyped tree, and that is required
// rather than accidental.** Serialising a `k8s_openapi` struct back out silently drops every
// field this build's `k8s-openapi` does not know, so a yaml pane fed through one would quietly
// delete a newer server's fields and a webhook's additions — a record that lies (invariant 4's
// spirit). `k8s.rs` § ONE OBJECT'S OWN STORY is where both reads live and where that is argued
// in full; the two paths below are two paths on purpose and are not to be merged into one.
//
// **The three verbs share their first four steps and share them as functions** ([`opened`],
// [`in_namespace`], [`read_failed`], [`no_answer`]): connect, settle the namespace, check it, and
// say why a named-object `get` did not answer. Three copies of those sentences is three things
// that can drift, and the one this file already paid for was a namespace check written once and
// needed three times.

/// The flag that says *print what is going on with this object*.
///
/// **A verb beside [`OBJECT`], like [`LOGS`]** — the shape NOTES § D194 settled, so *which
/// object* is asked once and answered once for all three.
const DESCRIBE: &str = "--describe";

/// The flag that says *print this object as YAML*.
const YAML: &str = "--yaml";

/// **Which kind [`YAML`] reads**, defaulting to [`POD`].
///
/// **It exists so the Secret masking has a caller** (`screens/detail.md` § Printed instead of
/// drawn — yaml). `k8s.rs` freezes at the end of this phase, and code that ships with no reachable
/// caller can only ever be unit-tested, never run for real — which is the one thing this repo
/// asks of every box.
///
/// **A flag and not a second parse of [`OBJECT`]**, so the kind travels in its own word instead of
/// a second reader of `[namespace/]name` learning to disagree with [`split_object`].
const KIND: &str = "--kind";

/// **What [`KIND`] means when a run does not give it** — and the only value [`DESCRIBE`] accepts.
///
/// **The singular and not the plural**, because it is the word a reader types: `k8s::kind_named`
/// matches a resource's plural *and* its kind lowercased, so `pod` and `pods` both resolve, and
/// this is the half of that pair the sentences quote.
const POD: &str = "pod";

/// **The three verbs that read one object the reader named**, in the order [`USAGE`] lists them.
///
/// **One array so that a fourth cannot be added without joining every check that reads it** —
/// [`mistyped`]'s collision refusal, its verb-and-object pairing, its path refusal's *which mode
/// is on this line*, [`live_context`]'s *is this a cluster run at all*, and [`on_cluster`]'s
/// dispatch. Five sites, one list.
const VERBS: [&str; 3] = [LOGS, DESCRIBE, YAML];

/// Which of [`VERBS`] this line carries, in [`VERBS`]' own order — empty for a run that carries
/// none.
fn verbs(args: &[String]) -> Vec<&'static str> {
    VERBS
        .into_iter()
        .filter(|verb| args.iter().any(|arg| arg == verb))
        .collect()
}

/// **Which of the three a run is**, once [`mistyped`] has established there is exactly one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Logs,
    Describe,
    Yaml,
}

impl Verb {
    /// The verb one of [`VERBS`] names. `None` is a word that is not one of them, which
    /// [`verbs`] cannot produce — the arm is there so that an edit which adds a fourth flag to
    /// [`VERBS`] and forgets this `match` is a run that says [`USAGE`] rather than one that
    /// silently reads a log.
    fn of(flag: &str) -> Option<Self> {
        match flag {
            LOGS => Some(Self::Logs),
            DESCRIBE => Some(Self::Describe),
            YAML => Some(Self::Yaml),
            _ => None,
        }
    }
}

/// **Which kind this line names**, or `None` when [`KIND`] is not on it ([`value_of`]).
fn kind_arg(args: &[String]) -> Option<Option<&str>> {
    value_of(args, &[KIND])
}

/// **`a, b and c`** — one joiner for the two sentences in this file that list things the reader
/// typed or has to choose between.
///
/// **`last` is the whole separator and not a conjunction**, because the two callers do not agree
/// on the comma: `screens/detail.md` spells its two-item list *"the original one, and the one
/// events.k8s.io adds"*, and *"--logs, and --yaml"* is not English. A parameter that carried only
/// the word would have to pick one of them for both.
fn joined(items: &[impl std::fmt::Display], last: &str) -> String {
    let mut said: Vec<String> = items.iter().map(ToString::to_string).collect();
    match said.pop() {
        None => String::new(),
        Some(end) if said.is_empty() => end,
        Some(end) => format!("{}{last}{end}", said.join(", ")),
    }
}

/// **The session all three verbs open, or the sentence a run that could not ends with.**
///
/// **The same sentence [`live`] gives**, because it is the same failure: nothing has been sent to
/// a cluster at this point, so what failed is the kubeconfig or the client built from it.
async fn opened(
    connecting: impl std::future::Future<Output = Result<k8s::Session, k8s::NotConnected>>,
) -> Result<k8s::Session, String> {
    connecting.await.map_err(|problem| {
        format!(
            "k8rs: no cluster to watch — {}",
            because(problem.fault(), "reach this cluster", problem.renewal())
        )
    })
}

/// **Which namespace a run about one object lands in, checked once, whichever of the three
/// sources it came from** (the security gate's *names build paths* row).
///
/// **The context's own namespace, and `default` under it** — `kubectl`'s rule and the one
/// `k8s::coverage` already falls back to, so a run that named neither looks where `kubectl` would
/// have looked.
///
/// **The check is on the word that is actually used and not on where it came from.**
/// [`mistyped`] already refuses the one that came off the command line; the *kubeconfig's* is not
/// checked anywhere — `k8s::kubeconfig_namespace` strips and bounds it and never asks whether it
/// is a namespace — so a context whose `namespace:` reads `../secrets` would otherwise reach a
/// request this driver wrote. A predicate applied to the word that is actually used cannot be the
/// one that was forgotten when the source changed.
fn in_namespace<'a>(asked: Option<&'a str>, session: &'a k8s::Session) -> Result<&'a str, String> {
    let namespace = asked
        .or(session.namespace.as_deref())
        .unwrap_or(k8s::FALLBACK_NAMESPACE);
    match k8s::namespace_name(namespace) {
        true => Ok(namespace),
        false => Err(format!(
            "k8rs: {} is not a namespace, so k8rs will not ask this cluster about it — a \
             namespace is lowercase letters, digits and dashes, up to {} characters",
            shown(namespace, k8s::NAMESPACE_MAX),
            k8s::NAMESPACE_MAX
        )),
    }
}

/// **` in payments`, or nothing at all for a kind that lives in no namespace** — the clause every
/// sentence about one named object ends with, spelled once.
fn within(namespace: Option<&str>) -> String {
    namespace.map_or(String::new(), |namespace| {
        format!(" in {}", sanitize(namespace))
    })
}

/// **`the pod web-7d9f4 in payments`** — one object named the same way in the two sentences that
/// name one, and `the node k8rs-worker` for a kind that lives in no namespace.
fn about(singular: &str, name: &str, namespace: Option<&str>) -> String {
    format!(
        "the {} {}{}",
        sanitize(singular),
        sanitize(name),
        within(namespace)
    )
}

/// **Why a `get` of one named object did not answer** — the two sentences a failure gets, and the
/// one place either is spelled.
///
/// **A `404` on one named object is *that object is not there*, and it gets its own sentence.**
/// [`because`]'s `Gone` arm is written for a *kind* the server does not serve — "there is no such
/// thing when k8rs tries to …" — which is true and unhelpful about a name somebody just typed.
///
/// **`singular` is the kind's own word**, so `--yaml --kind secret` on a name no Secret has says
/// *there is no secret named …* rather than naming a pod that was never asked for
/// (`screens/detail.md`: *the kind's own singular in place of pod*).
///
/// **A cluster-scoped kind is not sent to check a namespace it does not have.** *check the name
/// and the namespace* is advice about a word that is not on the line for `--kind node`, and
/// advice a reader cannot act on is `screens/states.md`'s own failure.
fn read_failed(
    failure: &kube::Error,
    singular: &str,
    name: &str,
    namespace: Option<&str>,
    renewal: Option<&str>,
) -> String {
    if k8s::fault(failure) == k8s::Fault::Gone {
        let check = match namespace {
            Some(_) => "check the name and the namespace",
            None => "check the name",
        };
        return format!(
            "k8rs: there is no {} named {}{} — {check}",
            sanitize(singular),
            sanitize(name),
            within(namespace)
        );
    }
    format!(
        "k8rs: {}",
        because(
            k8s::fault(failure),
            &format!("get {}", about(singular, name, namespace)),
            renewal
        )
    )
}

/// **The sentence a named-object `get` that ran out of time gets** ([`OBJECT_READ`]).
fn no_answer(singular: &str, name: &str, namespace: Option<&str>, seconds: u64) -> String {
    format!(
        "k8rs: this cluster has not answered for {} after {seconds} seconds",
        about(singular, name, namespace)
    )
}

/// **One column of a printed block**, padded to `width` **characters** and never to `width`
/// bytes: a multi-byte name padded by `len()` is short by the difference, every time.
///
/// **A character is not a column either, and that ceiling is named rather than closed.** A CJK
/// character occupies two terminal columns and one `char`, so a container named in Japanese still
/// draws wide — `scripts/screens-check.py` measures display width because the drawn screens need
/// it, and the pane that draws this block is Phase 11's, where a width function belongs. What this
/// closes is the byte/character gap, which is the one a `format!("{:<width$}")` would have had.
fn column(text: &str, width: usize) -> String {
    let mut padded = text.to_string();
    for _ in text.chars().count()..width {
        padded.push(' ');
    }
    padded
}

/// **The widest of a set of already-printable strings, in characters**, plus the gap the block
/// puts after it — the two blocks [`described`] draws use two different gaps, which is what the
/// mockups draw (`screens/detail.md` § Printed instead of drawn — describe).
fn widest<'a>(items: impl Iterator<Item = &'a str>, gap: usize) -> usize {
    items.map(|item| item.chars().count()).max().unwrap_or(0) + gap
}

/// **The whole of what `--describe` puts on stdout** — the identity line, the containers block,
/// and the events under it where there are any (`screens/detail.md` § Printed instead of drawn).
///
/// **A function over values, so a test can read it**: stdout belongs to the process and a test
/// cannot read it back (§ WATCHING A CLUSTER), and every decision in describe's output is here.
///
/// **`happened` of `None` prints no events section at all, and so does an empty one** — the
/// screen's own rule, and the reason exit codes carry what stdout cannot: *"No events prints no
/// heading, on stdout or stderr … stdout is the payload, and when the payload really is empty it
/// stays empty rather than dressing itself up"*. A read that succeeded and found nothing and a
/// read that failed print the identical block; `0` and `2` are what tell them apart.
///
/// **The containers block is the picker's own list, unchanged** — same order (declared, then
/// init), the same word out of [`doing`], and the same rule about when a restart count is shown
/// (`screens/detail.md` § Choosing a container). A second wording for the same fact is the drift
/// this file keeps refusing.
///
/// **The age is [`age`]'s one ladder** — the same strings a card's right edge draws, because it is
/// one function reached from here too (`screens/widgets.md` § 1b, NOTES § D68).
fn described(read: &k8s::PodRead, happened: Option<&k8s::Happened>, now: &Time) -> String {
    // **Each part is dropped rather than guessed at when the field is absent** — a pod with no
    // `phase` prints `Pod`, never `Pod · unknown`, which would be a reading nothing took
    // (`screens/states.md` § When there is nothing to say).
    let mut identity = vec!["Pod".to_string()];
    if let Some(phase) = &read.snapshot.phase {
        identity.push(sanitize(phase).to_lowercase());
    }
    if let Some(created) = read
        .snapshot
        .creation_timestamp
        .as_ref()
        .and_then(|created| age(now, created))
    {
        identity.push(format!("created {created}"));
    }
    let mut out = identity.join(" · ");
    // **The pod's own `status.reason`, which was dropped entirely until this review** — a pod
    // carrying `reason: Evicted` printed `Pod · failed · created 8 days ago` and never said why,
    // which is a `Failed` that tells a reader nothing any other `Failed` would not
    // (`k8s-admin`, 2026-08-31, `screens/detail.md` § The pod's own reason).
    if let Some(reason) = &read.snapshot.reason {
        // **`status.message` is not on [`PodSnapshot`] and this build cannot print it.**
        // `rules.rs` is frozen, so the sentence the screen draws beside `(Evicted)` waits for the
        // snapshot field the PM has boxed; what is here is the half that is reachable.
        if let Some(phrase) = phrase(POD_REASONS, reason) {
            out.push_str(&format!("\n{phrase}"));
        }
        out.push_str(&format!("\n{}", raw_and_message(reason, None)));
    }

    let names: Vec<String> = read.declared().map(sanitize).collect();
    if !names.is_empty() {
        let width = widest(names.iter().map(String::as_str), 3);
        out.push_str("\n\ncontainers:");
        for name in &names {
            let status = read.status(name);
            let (word, detail) = container_state(status.map(|container| &container.state));
            // **The restart count goes on the last line of the row**, which is the detail line
            // where there is one and the state word where there is not — the mockup's
            // `container exceeded its memory limit — exit 137, 4 restarts` against its
            // `keeps crashing and restarting, 12 restarts` (`screens/detail.md`).
            let counted = restarts(status);
            match detail {
                None => out.push_str(&format!("\n  {}{word}{counted}", column(name, width))),
                Some(detail) => out.push_str(&format!(
                    "\n  {}{word}\n    {detail}{counted}",
                    column(name, width)
                )),
            }
        }
    }

    let Some(happened) = happened.filter(|happened| !happened.lines.is_empty()) else {
        return out;
    };
    let ages: Vec<String> = happened
        .lines
        .iter()
        .map(|line| {
            line.at
                .as_ref()
                .and_then(|at| age(now, at))
                .unwrap_or_default()
        })
        .collect();
    let width = widest(ages.iter().map(String::as_str), 2);
    // **The heading carries the cut, because the heading is where the claim is.** *newest first*
    // is not true of a list the server stopped at [`k8s::EVENTS_KEPT`] — a `limit` returns the
    // cluster's own order, not the newest — so the words that promise it are the words that have
    // to be withdrawn (`k8s::Happened::cut`).
    out.push_str(&match happened.cut {
        false => "\n\nevents (newest first):".to_string(),
        // **The bound is interpolated and not written out**, because a second copy of a number is
        // the copy that goes stale — the reason `scripts/twin-guard.py` exists one layer up.
        true => format!(
            "\n\nevents (the first {} k8rs was given — there are more, and these are not the \
             newest):",
            k8s::EVENTS_KEPT
        ),
    });
    for (line, at) in happened.lines.iter().zip(ages) {
        // **The phrase where there is one, then the raw word and the message under it, always**
        // (NOTES § D198). A phrase that stood *instead of* the message was measurably false for
        // `Pulled` and deleted the probe kind for `Unhealthy`; the message is what says which.
        // **The first line is dropped when there is nothing on it**, which is an event with no
        // phrase *and* no stamp: `column` would otherwise pad a blank to the age width and leave
        // a row of spaces above the message.
        let head = format!(
            "  {}{}",
            column(&at, width),
            line.plainly().unwrap_or_default()
        );
        let mut row: Vec<String> = Vec::new();
        if !head.trim_end().is_empty() {
            row.push(head.trim_end().to_string());
        }
        row.push(format!(
            "    {}",
            raw_and_message(&line.reason, Some(&line.message))
        ));
        if let Some(repeated) = repeated(line, now) {
            row.push(format!("    {repeated}"));
        }
        out.push('\n');
        out.push_str(&row.join("\n"));
    }
    out
}

/// **`happened 2,383 times since 4 days ago`, and `None` for something that happened once**
/// (`screens/detail.md` § A repeated event).
///
/// **Both numbers or neither is not the rule — both numbers where both are known.** The count
/// without the span is *a lot*, of unknown recency; the span without the count is *still going*,
/// of unknown severity. An event whose first stamp did not survive prints the count alone rather
/// than a span this file guessed.
///
/// **Exact, with a comma at the thousand, never rounded** — the discipline
/// [`k8s::LogLines::dropped_line`] already keeps for a number a reader is counting on, and
/// [`k8s::grouped`] is the one spelling of the separator.
///
/// **Silent at `1`**, because a thing that happened once needs no sentence saying so.
fn repeated(line: &k8s::Happening, now: &Time) -> Option<String> {
    let counted = usize::try_from(line.count?).unwrap_or(0);
    if counted < 2 {
        return None;
    }
    let since = line
        .first
        .as_ref()
        .and_then(|first| age(now, first))
        .map_or(String::new(), |span| format!(" since {span}"));
    Some(format!("happened {} times{since}", k8s::grouped(counted)))
}

/// **What a pod with no events gets, on stderr** (`screens/detail.md` § No events at all).
///
/// **Two facts wearing one empty list, and only one of them is *nothing happened*.** Kubernetes
/// keeps events for a while and then drops them, so a pod up for a week has almost certainly
/// outlived every event it ever had — and saying only *nothing happened* would be true the day it
/// started and false a week later, in the one case a reader has no other way to check.
const NO_EVENTS: &str = "k8rs: Kubernetes only keeps events for a while, and this pod has run \
                         long enough that none are left.";

/// **[`NO_EVENTS`] when the read found nothing, and `None` when it found something** — the same
/// shape [`nothing_written`] is, for the same measured reason.
///
/// **The emptiness is decided here and not at the call site.** Spelled as a `match` guard in
/// [`describe_run`], both `happened.lines.is_empty() -> true` and `-> false` survived the mutation
/// gate: the only thing that depends on the answer there is a line on stderr, and stderr belongs
/// to the process (`dev-core`'s run, 2026-08-31 — the second time this file has paid for it, and
/// [`nothing_written`]'s doc is where the first is written down).
fn no_events(happened: &k8s::Happened) -> Option<&'static str> {
    happened.lines.is_empty().then_some(NO_EVENTS)
}

/// **What the events selector names as the kind** — `describe` is pod-only, so the one caller in
/// this build sends one word (`k8s::events`, which takes it as an argument for the Phase 11 tab
/// that will send others).
const EVENTS_ABOUT: &str = "Pod";

/// **Print one pod and what happened to it, then stop** — the headless describe tab.
///
/// **Two reads and one `kubectl` line** (invariant 4). `kubectl describe pod` is one word the
/// reader would have typed for both, so the command log shows the *equivalent* command rather
/// than the two calls underneath it (`screens/detail.md`).
///
/// **The pod read is `logs_run`'s own**, down to the namespace check and the three failure
/// sentences, which is why they are functions above rather than lines in either.
///
/// **`None` is the happy ending and `main` exits `0`, including a pod with no events at all.**
/// `Some` is exit `2`, and the events fetch failing is the one failure that reaches it *after*
/// stdout already carries the payload: both endings print byte-identical stdout, so the exit code
/// is the only thing that can carry *calm* against *k8rs could not find out*.
///
/// **`deadline` is a parameter for [`cluster_run`]'s reason and not for flexibility**: it is
/// [`OBJECT_READ`] at the one call site, and a test can prove the bound in a fraction of a second
/// instead of ten. The arm it reaches is the one `screens/detail.md` gives exit `2` for a read
/// that did not finish, which is half of the pair this whole verb rests on — and an arm no test
/// can enter is an arm that rots (NOTES § D26). `logs_run` and [`yaml_run`] keep theirs inline:
/// their timeout arms are as unreached as they were, which is a gap this box did not widen and
/// did not close.
async fn describe_run(
    connecting: impl std::future::Future<Output = Result<k8s::Session, k8s::NotConnected>>,
    asked: &Asked<'_>,
    now: &Time,
    deadline: std::time::Duration,
) -> Option<String> {
    use std::io::Write;
    let session = match opened(connecting).await {
        Err(sentence) => return Some(sentence),
        Ok(session) => session,
    };
    let renewal = session.renewal.clone();
    let renewal = renewal.as_deref();
    let namespace = match in_namespace(asked.namespace, &session) {
        Err(sentence) => return Some(sentence),
        Ok(namespace) => namespace,
    };
    let mut err = std::io::stderr();

    let pod = match tokio::time::timeout(deadline, k8s::pod(&session.client, namespace, asked.name))
        .await
    {
        Ok(Ok(pod)) => pod,
        Ok(Err(failure)) => {
            return Some(read_failed(
                &failure,
                POD,
                asked.name,
                Some(namespace),
                renewal,
            ));
        }
        Err(_) => {
            return Some(no_answer(
                POD,
                asked.name,
                Some(namespace),
                deadline.as_secs(),
            ));
        }
    };
    let _ = writeln!(
        err,
        "$ kubectl describe pod {} -n {}",
        sanitize(asked.name),
        sanitize(namespace)
    );

    // **The uid, so a pod deleted and recreated under one name does not inherit the dead one's
    // events** — `kubectl describe`'s own selector term (`k8s::events`).
    let happened = tokio::time::timeout(
        deadline,
        k8s::events(
            &session.client,
            namespace,
            EVENTS_ABOUT,
            asked.name,
            pod.snapshot.id.uid.as_deref(),
        ),
    )
    .await;
    // **The block is built and written once, whatever the events came back as** — the screen's
    // *both print byte-identical stdout* is a property of this line rather than of two arms that
    // have to be kept in step.
    let read = happened.as_ref().ok().and_then(|read| read.as_ref().ok());
    if let Err(failed) = writeln!(std::io::stdout(), "{}", described(&pod, read, now)) {
        return stdout_failure(&failed);
    }
    match happened {
        Ok(Ok(happened)) => {
            if let Some(sentence) = no_events(&happened) {
                let _ = writeln!(err, "{sentence}");
            }
            None
        }
        Ok(Err(failure)) => Some(format!(
            "k8rs: {}",
            because(
                k8s::fault(&failure),
                &format!("list events in {}", sanitize(namespace)),
                renewal
            )
        )),
        Err(_) => Some(format!(
            "k8rs: this cluster has not answered for the events of {} after {} seconds",
            about(POD, asked.name, Some(namespace)),
            deadline.as_secs()
        )),
    }
}

/// **Which kind `--yaml` was asked for, resolved against what the cluster says it serves**, or
/// the sentence that refuses (`screens/detail.md` § Printed instead of drawn — yaml).
///
/// **The two refusals are the screen's own.** A word nothing serves is a spelling mistake; a word
/// two resources answer to is `events`, which `core/v1` and `events.k8s.io/v1` both serve as
/// different resources — so the sentence spells both qualified forms rather than picking one.
///
/// **The core group is *the original one* and it is spelled with a trailing dot**, `--kind
/// 'events.'`, which is `kubectl`'s own way of saying *the group with no name*.
fn which_kind<'a>(kinds: &'a [k8s::Browsable], word: &str) -> Result<&'a k8s::Browsable, String> {
    let matched = k8s::kind_named(kinds, word);
    let [only] = matched[..] else {
        if matched.is_empty() {
            return Err(format!(
                "k8rs: this cluster does not serve a kind named {} — check the spelling",
                shown(word, k8s::NAME_MAX)
            ));
        }
        let says = |kind: &k8s::Browsable| match kind.group.is_empty() {
            true => "the original one".to_string(),
            false => format!("the one {} adds", sanitize(&kind.group)),
        };
        let spelling = |kind: &k8s::Browsable| {
            format!(
                "{KIND} '{}.{}'",
                sanitize(&kind.plural),
                sanitize(&kind.group)
            )
        };
        // **The last choice reads *for the other* only when there are exactly two**, which is the
        // shape the screen draws and the only shape any cluster has produced; a third would be
        // *the other* naming two things.
        let choices: Vec<String> = matched
            .iter()
            .enumerate()
            .map(|(at, kind)| match at == 1 && matched.len() == 2 {
                true => format!("{} for the other", spelling(kind)),
                false => format!("{} for {}", spelling(kind), says(kind)),
            })
            .collect();
        return Err(format!(
            "k8rs: {KIND} {} matches {} things this cluster serves — {}. Say which: {}",
            shown(word, k8s::NAME_MAX),
            // **Spelled as a word as far as a cluster plausibly goes, and a digit past that.**
            // `screens/detail.md` writes *"matches two things"*, and `events` is that two; three
            // groups serving one plural is rare and still readable as a word. **Four is where the
            // digit starts**, because a number-speller for a shape no cluster has produced is code
            // nothing in this build would ever run.
            match matched.len() {
                2 => "two".to_string(),
                3 => "three".to_string(),
                more => more.to_string(),
            },
            joined(
                &matched.iter().map(|kind| says(kind)).collect::<Vec<_>>(),
                ", and "
            ),
            joined(&choices, ", or ")
        ));
    };
    Ok(only)
}

/// **The command a reader could have typed to ask the same server for the same object** — the
/// teaching device, and a function for [`k8s::LogRequest::kubectl`]'s reason: `live` writes it to
/// stderr and a test cannot read the process's own stream back, which is what left it unproven
/// while it was inline.
///
/// **The *request* and not *the same bytes back*, which is the narrowing
/// [`k8s::LogRequest::kubectl`] took on 2026-08-30 and this line needed too.** Measured on
/// `kube-system/coredns`: 43 lines each, every key and every value present both ways, and two
/// differences that are the printer's — `kubectl get -o yaml` alphabetises where this pane keeps
/// the API's own order, and it quotes a timestamp this pane leaves bare (`k8s-admin`,
/// 2026-08-31). Same object, not the same file.
///
/// **`--show-managed-fields`, because without it the printed line does not produce what was
/// printed** (invariant 4: neither record may lie). `kubectl` has hidden `managedFields` from
/// `get -o yaml` since v1.21 and this pane does not — measured, 95 of a pod's 246 lines, **39% of
/// the document** (`k8s-admin`, 2026-08-31). Dropping the field instead was refused: this pane's
/// only claim is that it is the object.
///
/// **It is display text**: k8rs does not execute it and nothing in it is fed back into a process
/// (the security gate).
fn kubectl_get(qualified: &str, name: &str, namespace: Option<&str>) -> String {
    format!(
        "$ kubectl get {} {}{} -o yaml --show-managed-fields",
        sanitize(qualified),
        sanitize(name),
        namespace.map_or(String::new(), |namespace| format!(
            " -n {}",
            sanitize(namespace)
        ))
    )
}

/// **Print one object as the API server returned it, and stop** — the headless yaml tab.
///
/// **One read, unpruned and untyped** (`k8s.rs` § ONE OBJECT'S OWN STORY, ruling 2 of this box's
/// brief): the store is pruned to the fields the rules name (invariant 6), so a document built
/// from it would show a partial object and call it the object.
///
/// **The Secret masking is `k8s::document`'s and not this function's**, so no caller ever holds an
/// unmasked value — the security gate's *the redaction happens before the value is anywhere a
/// formatter can find it*, made structural rather than remembered.
///
/// **There is no `--reveal` and there will not be one on this surface.** A reveal is a keypress on
/// a drawn pane and Phase 6 has no pane, so `--yaml` on a Secret redacts unconditionally.
async fn yaml_run(
    connecting: impl std::future::Future<Output = Result<k8s::Session, k8s::NotConnected>>,
    asked: &Asked<'_>,
) -> Option<String> {
    use std::io::Write;
    let session = match opened(connecting).await {
        Err(sentence) => return Some(sentence),
        Ok(session) => session,
    };
    let renewal = session.renewal.clone();
    let renewal = renewal.as_deref();
    let namespace = match in_namespace(asked.namespace, &session) {
        Err(sentence) => return Some(sentence),
        Ok(namespace) => namespace,
    };
    // **Discovery is the session's own answer and costs no round trip** (`k8s::Served`). Its
    // failure is the same clause [`greeting`] prints, because it is the same refusal on the same
    // path — a `nonResourceURL` on `/apis`, whose `Status` carries an empty `details` and so can
    // only be named by that path (NOTES § D160).
    let served = match &session.served {
        Ok(served) => served,
        Err(failure) => {
            return Some(format!(
                "k8rs: this cluster would not say what kinds it serves, so k8rs cannot tell \
                 which one {KIND} means — {}",
                because(k8s::fault(failure), "`get /apis`", renewal)
            ));
        }
    };
    let kind = match which_kind(&served.kinds, asked.kind.unwrap_or(POD)) {
        Ok(kind) => kind,
        Err(sentence) => return Some(sentence),
    };
    // **The singular is the kind's own word lowercased**, which is what `k8s::kind_named` already
    // accepts as a spelling — one notion of the singular, not two (invariant 12).
    let singular = kind.kind.to_lowercase();
    // **The namespace is dropped for a kind that lives in none**, by `k8s::Fetch::table`'s own
    // line, so every sentence below and the `kubectl` line are told the same thing.
    let scope = kind.namespaced.then_some(namespace);
    let Some(fetch) = k8s::Fetch::table(kind, scope).map(|fetch| fetch.plain()) else {
        // **A kind whose own words cannot build a URL is offered by `browsable` and refused
        // here** (`k8s.rs` § THE BROWSER'S ROWS, `path_safe`). The sentence names the word the
        // reader typed, because the group and version behind it are not words they chose.
        return Some(format!(
            "k8rs: this cluster describes a kind named {} with characters k8rs will not put in a \
             request, so it cannot be read",
            shown(asked.kind.unwrap_or(POD), k8s::NAME_MAX)
        ));
    };
    let mut err = std::io::stderr();
    // **The group is on the line only when there is one**, so the core kinds read the way the
    // screen draws them — `kubectl get secret …` — and a CRD reads unambiguously.
    let qualified = match kind.group.is_empty() {
        true => singular.clone(),
        false => format!("{singular}.{}", sanitize(&kind.group)),
    };
    let _ = writeln!(err, "{}", kubectl_get(&qualified, asked.name, scope));

    let document = match tokio::time::timeout(
        OBJECT_READ,
        k8s::document(&session.client, &fetch, asked.name, &kind.kind),
    )
    .await
    {
        Ok(Ok(document)) => document,
        Ok(Err(failure)) => {
            return Some(read_failed(&failure, &singular, asked.name, scope, renewal));
        }
        Err(_) => {
            return Some(no_answer(
                &singular,
                asked.name,
                scope,
                OBJECT_READ.as_secs(),
            ));
        }
    };
    let printed = match document.yaml() {
        Ok(printed) => printed,
        // The emitter's own reason, through [`sanitize`] like every other string this file did
        // not write. The tree is masked and stripped before [`k8s::Document`] exists, so there is
        // nothing secret in scope for it to have named.
        Err(failed) => {
            return Some(format!(
                "k8rs: this object could not be written out as YAML — {}",
                sanitize(&failed)
            ));
        }
    };
    // **`write!` and not `writeln!`**: `serde_yaml_ng` ends its document with a newline, and a
    // second one is a blank line at the end of a file a reader may be diffing.
    if let Err(failed) = write!(std::io::stdout(), "{printed}") {
        return stdout_failure(&failed);
    }
    None
}

// --- ONE OBJECT'S OWN STORY END ---
