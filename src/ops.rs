//! The write path — the only file in the crate permitted to mutate cluster state.
//!
//! `clippy.toml` bans every `&self` method of `Api` and `Request` outside invariant 1's
//! allowlist, crate-wide, and the attribute below is the single visible exception to it: one
//! file to audit, one line that announces it (NOTES § Operations, "Structural consequence —
//! writes live in exactly one file"; CLAUDE.md invariant 1). The split is mechanical and not a
//! judgement about what mutates — `namespace` mutates nothing and is banned, `may_i` mutates
//! nothing and belongs here, because it is performed with `create` (NOTES § D23).
//!
//! **The ban is not the whole containment.** `Client::request` and `Client::send` are
//! verb-agnostic — the verb is data in the request object — and are off the list on purpose,
//! since Phase 5 needs both outside this file for reads. A write built as a hand-verbed request
//! is stopped by CLAUDE.md invariant 2 and by review, not by the lint (NOTES § D142).
#![allow(clippy::disallowed_methods)]
// `expect` rather than `allow` because it expires by itself, and `not(test)` because this file's
// own tests construct and read every item — under `cargo test` the expectation would be
// fulfilled by nothing and `-D warnings` rejects an unfulfilled expectation. The precedent, and
// the accepted module-wide blind spot, is `analysis.rs`'s and `k8s.rs`'s (NOTES § D38).
#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the operations that call this contract, and the driver that runs them, are \
                  later boxes of Phase 7"
    )
)]

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;

use std::future::Future;
use std::io::Write;

use k8s_openapi::jiff::Timestamp;

use crate::k8s::{FREE_TEXT, Fault, IDENTIFIER, fault, said, text};

// --- THE MUTATION CONTRACT START ---
//
// **The five steps are one function's body, not a checklist an operation is trusted to follow**
// (todo.md § Phase 7): *consequence text → dry-run → confirm → call → audit*. An operation
// hands [`perform`] a closure that would call the API and never awaits it itself, so the order
// is not something each operation restates and can get wrong — there is one copy of it.
//
// **What that is not is a privacy guarantee, and saying so is the point.** Rust's privacy is
// per module and this file is one module, so nothing *stops* an operation awaiting its own
// closure. What holds is narrower and still enough: outside `ops.rs` `clippy.toml` refuses the
// call at all (invariant 1), and inside it a second `await` on that closure is a visible line in
// the one file the whole design exists to keep small enough to read.
//
// **The dialog is on screen before the check goes out, and the button is dead until the answer
// comes back** (`screens/dialogs.md` rule 3, and the scale mockup's *"The cluster checked it
// first and accepted it."*). [`perform`] therefore has two callbacks and not one: [`show`] puts
// the dialog into the app's state and **returns** — it is `FnOnce(&Shown)` and not a future, so
// no implementation can park the dialog behind an `await` — and [`Ask`] is handed a [`Checked`]
// only once the cluster has answered. A keypress that awaits a round trip before anything is
// drawn is the frozen screen NOTES § D20 refuses, and the first draft of this file did exactly
// that.
//
// **[`Checked`] is what makes rule 3 structural rather than remembered.** It is constructible
// only in this file, and only after the check has been answered, so an operation cannot get its
// confirm button enabled without one. A refused check never calls [`Ask`] at all: the caller
// swaps in `screens/dialogs.md`'s refusal screen, which has only `esc dismiss`.
//
// **`esc` is inert until the verdict arrives, deliberately** (PM ruling, 2026-09-04). There is no
// cancellation path and no `Drop` guard: nothing has been sent that could be un-sent, and a
// *"k8rs stopped before the call returned"* line would be a record of something that did not
// happen. The pending dialog's own drawing is `screens/dialogs.md`'s to add and is boxed for
// Phase 11.
//
// **Both records are written from one [`Mutation`], stripped once into one [`Record`]** — which
// is what stops them disagreeing about a *name* (NOTES § D8). It is not a cross-check: `kubectl`
// and `verb` + `path` are independent free text and nothing here derives one from the other, so
// an operation whose kubectl line says `--replicas=3` while its path targets `/status` compiles
// and logs happily. What the struct buys is **one place to look** when they do disagree, and the
// guard on the pair is review (CLAUDE.md § step 6).
//
// **Two audit writes and not one** (NOTES § D21). The attempt is written and flushed before
// anything reaches the cluster, so a crash mid-call leaves an attempt with no result — the
// honest record. If that first write fails, nothing is sent at all: a mutation that cannot be
// recorded does not happen. **That rule governs the attempt and not the result**: a result line
// that cannot be written does not un-make a change that has already been made, so [`Performed`]
// carries the outcome *and* whether it was recorded, rather than replacing one with the other.
//
// **An operation can say it has no dry-run, because three of the six do not.**
// `Api::restart(&self, name)` takes no params argument at all
// (`kube-client-4.2.0/src/api/util/mod.rs:19`), and the same holds for `cordon`/`uncordon`; NOTES
// § Operations gives `restart`, `cordon`, `delete` and `drain` the guard *confirm* where only
// `scale`, `undo` and `edit` are *confirm + dry-run*; invariant 2 says `dryRun=All` **where the
// API supports it**. So [`Mutation::checkable`] is `false` there and the audit line **says so** —
// D8's verdict field recorded honestly rather than omitted.
//
// **A cluster can also refuse every dry-run there is**: a `ValidatingWebhookConfiguration` with
// `sideEffects: Some | Unknown` fails `dryRun=All` for a fully authorised user. That is accepted
// rather than designed around — there is no flag for it — and what makes it diagnosable is that
// the verdict is keyed on the [`Fault`] and the server's own *"admission webhook … does not
// support dry run"* travels beside it.

/// **One mutation, described once**, so the command log line and the audit line are written from
/// the same value (NOTES § D8).
///
/// **Raw as an operation writes it.** Nothing here has been through the ingest guard yet;
/// [`Record::of`] is the one place that strips, and every consumer downstream of it —
/// [`Shown`], both audit lines — reads the stripped copy. Invariant 9's design is that the
/// strip is paid once on the way *in* so no consumer has to remember it.
pub struct Mutation<'a> {
    /// The kubeconfig context this is performed against.
    pub context: &'a str,
    /// The namespace, or `None` for a cluster-scoped object.
    pub namespace: Option<&'a str>,
    /// The object as the reader knows it — `deployment/web`.
    pub object: &'a str,
    /// **What is about to happen, in words someone in their first month reads without a
    /// glossary** (invariant 14). It is what [`Shown`] puts on screen.
    pub consequence: &'a str,
    /// **The equivalent kubectl command** — the teaching device, and never what ran
    /// (NOTES § D8). k8rs does not execute it and nothing is fed back from it into a process.
    pub kubectl: &'a str,
    /// The verb of the real call: `PATCH`, `DELETE`, `POST`.
    pub verb: &'a str,
    /// The path of the real call.
    pub path: &'a str,
    /// The `resourceVersion` sent with the call, where one is sent — the value a `409` is
    /// argued from.
    pub version: Option<&'a str>,
    /// **Whether this operation's API call can carry `dryRun=All`** — `false` for `restart`,
    /// `cordon` and `uncordon`, whose kube entry points take no params argument at all
    /// (invariant 2's *where the API supports it*). When it is `false` nothing is sent before
    /// the confirmation and the audit line records that no check was run.
    pub checkable: bool,
}

/// **One mutation, stripped** — every string on it has been through [`crate::k8s::text`] and is
/// safe to put on a screen or in a record (invariant 9, NOTES § D154, § D213).
struct Record {
    context: String,
    namespace: Option<String>,
    object: String,
    consequence: String,
    kubectl: String,
    verb: String,
    path: String,
    version: Option<String>,
    checkable: bool,
}

/// **What the dialog is given when it opens, before anything has been sent.**
///
/// Everything on it is stripped, and it is only what a dialog draws: the title
/// (`screens/dialogs.md` rule 1), the consequence and the `$ …` line. The verb, the path and the
/// `resourceVersion` are the audit log's and are deliberately not here.
pub struct Shown<'a> {
    /// The object as the reader knows it — the dialog's title.
    pub object: &'a str,
    /// The namespace, or `None` for a cluster-scoped object.
    pub namespace: Option<&'a str>,
    /// What is about to happen, in plain language (invariant 14).
    pub consequence: &'a str,
    /// The equivalent kubectl command, for the `$ …` line. **Display text**: k8rs never executes
    /// it and nothing is fed back from it into a process.
    pub kubectl: &'a str,
}

/// **Proof that the check is over, and the only key that unlocks the confirm button**
/// (`screens/dialogs.md` rule 3).
///
/// Its fields are private and nothing outside this file can build one, so an operation cannot
/// reach a confirmation without a check having been answered first — rule 3 made structural
/// rather than remembered.
///
/// **`Response` is the object the call returned, and it is generic on purpose.** `edit` in v0.4
/// shows a diff of what the dry-run gave back, and that *is* its confirmation; `ops.rs` freezes
/// at the end of this phase, so the alternative is a second entry point into a frozen file. The
/// channel exists for the verdict either way, and carrying the object in it is free.
pub struct Checked<Response> {
    verdict: &'static str,
    returned: Option<Response>,
}

impl<Response> Checked<Response> {
    /// **What the cluster's check said, in the words the dialog prints** — the mockup's *"The
    /// cluster checked it first and accepted it."*, or the sentence for an operation that has no
    /// check to run.
    pub fn verdict(&self) -> &'static str {
        self.verdict
    }

    /// **The object the dry-run returned**, or `None` where there was no dry-run to return one.
    pub fn returned(&self) -> Option<&Response> {
        self.returned.as_ref()
    }
}

/// **How the dialog ended** — four endings, because three of them are not "no".
///
/// `bool` collapsed them into one and the audit log said *nobody confirmed it* for all three
/// (`k8s-admin`, 2026-09-04). The ReplicaSet replaces the pod while its name is being typed,
/// k8rs correctly refuses to delete whatever now holds that name, and the record read as *someone
/// opened a delete dialog on prod and backed out* — invariant 4's *neither record may lie*
/// (NOTES § D22, `screens/dialogs.md` § The object went away while the dialog was open).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The person at the keyboard agreed — and, where invariant 2 requires it, typed the name.
    Confirmed,
    /// They said no.
    Cancelled,
    /// **The object stopped existing while the dialog was open** — the `uid` the dialog holds is
    /// no longer the cluster's. Sending a delete by name here is how the wrong pod gets deleted.
    Gone,
    /// **The object changed underneath** — the dialog offers a re-read rather than a blind
    /// overwrite. The `409` mechanic, moved to where it costs nothing.
    Changed,
}

/// **What happened to one mutation** — one variant per sentence a screen has to be able to say.
///
/// **The classification is [`Fault`]'s and this grows no second vocabulary for the same errors**
/// (`k8s.rs` § WHAT WENT WRONG). A `Fault` is a fact and carries no string; the server's own
/// sentence travels beside it in `said`, already stripped and bounded on its way out of
/// [`crate::k8s::said`].
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The check passed or was not available, the confirmation was given, and the call succeeded.
    Done,
    /// Nobody confirmed it. Nothing was changed.
    Cancelled,
    /// The object was already gone when the confirmation came back. Nothing was changed —
    /// where the operation is checkable the `dryRun=All` had already gone out, and only the
    /// change itself never did.
    Gone,
    /// The object had changed underneath. Nothing was changed — the same ordering as
    /// [`Self::Gone`], any check having already gone out — and a re-read is what comes next.
    Changed,
    /// **The check did not pass, so the real call was never made.** Named for what is true of
    /// the mutation rather than for the fault: [`Fault::Refused`] is a `403` specifically, and
    /// this arm is reached by a dead socket and an expired login too.
    NotSent {
        /// What the failure was.
        fault: Fault,
        /// What the server said about it, where it said anything.
        said: Option<String>,
    },
    /// **The real call failed.** Whether the change happened is [`Fault`]'s to say: for a fault
    /// the server answered it did not, and for one where nothing came back k8rs does not know.
    Failed {
        /// What the failure was.
        fault: Fault,
        /// What the server said about it, where it said anything.
        said: Option<String>,
    },
}

/// **What [`perform`] hands back: what happened, and whether the log knows it.**
///
/// **`outcome: None` is NOTES § D21's refusal** — the attempt line could not be written, so
/// nothing was sent, nobody was asked and there is no outcome to have.
///
/// **`recorded: false` beside a `Some` is a different fact and it may not swallow the first
/// one.** D21 governs the *attempt* line, which is the one that can still prevent a mutation; it
/// says nothing about a result that already exists. *"The change was made — but k8rs could not
/// write it to the audit log"* is more honest than *"go and look"*, because k8rs holds that fact
/// at the moment the operator is least able to go and get it (`k8s-admin`, 2026-09-04).
#[derive(Debug, PartialEq, Eq)]
#[must_use]
pub struct Performed {
    /// What happened, or `None` when nothing was attempted because the attempt could not be
    /// recorded.
    pub outcome: Option<Outcome>,
    /// Whether the audit log holds the whole record — attempt line *and* result line.
    pub recorded: bool,
}

/// Passed to the operation's closure for the server-side `dryRun=All` pass (invariant 2).
const DRY_RUN: bool = true;

/// Passed to the same closure for the call that actually changes something.
const FOR_REAL: bool = false;

/// The verdict when the cluster ran the check and accepted it — `screens/dialogs.md`'s own line.
const ACCEPTED: &str = "the cluster checked it first and accepted it";

/// The verdict when the API has no `dryRun=All` for this operation, so none was run
/// (invariant 2's *where the API supports it*).
const UNCHECKABLE: &str = "the cluster has no way to check this one first, so nothing was tried";

/// **The mutation contract — every write in k8rs goes through here** (todo.md § Phase 7).
///
/// The order is *dialog opens pending → check → verdict into the open dialog → button lives →
/// answer → real call → audit*, which is the one sequence that satisfies invariant 2 (the dialog
/// is shown before the check), `screens/dialogs.md` rule 3 (the verdict lands *in* it) and this
/// box (the answer comes back after it).
///
/// `call` is the operation itself, and where a dry-run exists it is called twice with the same
/// body: once with `dry_run` true and once false. One closure rather than two is what stops the
/// dry-run validating something other than what is sent.
///
/// `show` is synchronous by design — see the region's doc.
///
/// `ask` is handed a [`Checked`] and answers how the dialog ended. Typing the object's name where
/// invariant 2 requires it is part of *asking*, and belongs to the dialog that implements it.
///
/// `now` is a parameter because the clock is an input rather than an ambient fact
/// (NOTES § D18); `audit` is any destination that can be written and flushed, which is what
/// keeps opening, locating and permissioning `~/.local/state/k8rs/audit.log` in its own box.
pub async fn perform<Show, Ask, Asked, Call, Called, Response>(
    record: &Mutation<'_>,
    now: Timestamp,
    audit: &mut impl Write,
    show: Show,
    ask: Ask,
    call: Call,
) -> Performed
where
    Show: FnOnce(&Shown<'_>),
    Ask: FnOnce(Checked<Response>) -> Asked,
    Asked: Future<Output = Answer>,
    Call: Fn(bool) -> Called,
    Called: Future<Output = Result<Response, kube::Error>>,
{
    let record = Record::of(record);

    // NOTES § D21 — on disk and flushed before anything reaches the cluster, dry-run included.
    if write_line(audit, &record.attempt_line(now)).is_err() {
        return Performed {
            outcome: None,
            recorded: false,
        };
    }

    // `screens/dialogs.md` rule 3: the dialog is on screen *before* the check goes out, with a
    // dead button, so a busy API server is a wait the reader can see rather than a keypress that
    // does nothing.
    show(&record.shown());

    let checked = if record.checkable {
        call(DRY_RUN).await.map(Some)
    } else {
        Ok(None)
    };

    let outcome = match checked {
        Err(error) => Outcome::NotSent {
            fault: fault(&error),
            said: said(&error),
        },
        Ok(returned) => {
            match ask(Checked {
                verdict: record.accepted(),
                returned,
            })
            .await
            {
                Answer::Cancelled => Outcome::Cancelled,
                Answer::Gone => Outcome::Gone,
                Answer::Changed => Outcome::Changed,
                Answer::Confirmed => match call(FOR_REAL).await {
                    Ok(_) => Outcome::Done,
                    Err(error) => Outcome::Failed {
                        fault: fault(&error),
                        said: said(&error),
                    },
                },
            }
        }
    };

    let recorded = write_line(audit, &record.result_line(now, &outcome)).is_ok();
    Performed {
        outcome: Some(outcome),
        recorded,
    }
}

impl Record {
    /// **The one place a [`Mutation`] is stripped** (invariant 9, NOTES § D213).
    ///
    /// The predicate and the disposal are both [`crate::k8s::text`]'s and never a second
    /// spelling of either (NOTES § D154, `k8s.rs` § THE INGEST GUARD). This file spelled its own
    /// `clean` for one box and it diverged on disposal immediately: it *removed* where `text`
    /// substitutes one space, so `screens/dialogs.md:39`'s own consequence came out *"will start
    /// areplacement"* and two object names fused into `deployment/webdeployment/db`. It also had
    /// no bound, so a 500 000-byte object name produced a 500 237-byte audit line.
    ///
    /// **A sentence is [`FREE_TEXT`] and a name is an [`IDENTIFIER`]** — D146's split, read off
    /// what the field is rather than off where it came from. The kubectl line and the path are
    /// sentences by that rule: both can run long and neither is scanned as a word.
    ///
    /// **What [`crate::k8s::said`] returns is not passed through here.** It has already been
    /// through the ingest guard, stripped and bounded, and cleaning it twice would say this file
    /// distrusts that guard.
    fn of(record: &Mutation<'_>) -> Self {
        let clean = |value: &str, cap: usize| {
            let mut value = value.to_string();
            text(&mut value, cap);
            value
        };
        let consequence = clean(record.consequence, FREE_TEXT);
        // **Invariant 2 requires the dialog to *state* the consequence**, and an empty string
        // states nothing. No operation can reach this today — every consequence is a k8rs
        // sentence with a name interpolated into it — so this is the author's error and not the
        // cluster's, which is what makes an assertion the right shape rather than an outcome.
        debug_assert!(
            !consequence.is_empty(),
            "a mutation reached the contract with nothing to state on screen (invariant 2)"
        );
        Record {
            context: clean(record.context, IDENTIFIER),
            namespace: record.namespace.map(|value| clean(value, IDENTIFIER)),
            object: clean(record.object, IDENTIFIER),
            consequence,
            kubectl: clean(record.kubectl, FREE_TEXT),
            verb: clean(record.verb, IDENTIFIER),
            path: clean(record.path, FREE_TEXT),
            version: record.version.map(|value| clean(value, IDENTIFIER)),
            checkable: record.checkable,
        }
    }

    /// What the dialog draws, borrowed from the stripped copy.
    fn shown(&self) -> Shown<'_> {
        Shown {
            object: &self.object,
            namespace: self.namespace.as_deref(),
            consequence: &self.consequence,
            kubectl: &self.kubectl,
        }
    }

    /// The line written before anything is sent — the equivalent kubectl command beside the call
    /// that actually goes out (NOTES § D8, § D21).
    ///
    /// **An absent field and an empty one record the same way**, because to a reader they are:
    /// `resourceVersion ` with nothing after it is a dangling label, indistinguishable from a
    /// record that was cut off (PM ruling, 2026-09-04). A gap word says which gap it is.
    fn attempt_line(&self, now: Timestamp) -> String {
        format!(
            "{now} attempt · {} · context {} · {} · kubectl: {} · call: {} {} · \
             resourceVersion {}\n",
            self.object,
            self.context,
            gap(self.namespace.as_deref(), "cluster-wide", "namespace "),
            self.kubectl,
            self.verb,
            self.path,
            gap(self.version.as_deref(), "not sent", ""),
        )
    }

    /// The line appended when the call returns.
    ///
    /// **It names the attempt it belongs to.** Two k8rs against two clusters share one
    /// `~/.local/state/k8rs/audit.log`, and `attempt(A) attempt(B) result(A) result(B)` makes
    /// B's result read as A's if adjacency is the only pairing there is — which it was until
    /// 2026-09-04. A drain takes minutes (NOTES § D20), so the window is not theoretical.
    ///
    /// **The stamp is the attempt's and says so.** It is not a landing time and may not be read
    /// as one: [`perform`] reads the clock once (NOTES § D18) and therefore cannot say when the
    /// call came back or how long it took. An attempt with no result under it is still D21's
    /// crash record.
    ///
    /// **The dry-run verdict is on every result line and not only the two where the sentence
    /// happens to mention it** (NOTES § D8). *Did this write get checked first?* is the one
    /// question the log exists to answer about the contract itself, and on `Done` and
    /// `Cancelled` the word did not appear at all until 2026-09-04 (`tester`).
    fn result_line(&self, now: Timestamp, outcome: &Outcome) -> String {
        // Annotated, so that dropping the arm below is a *testable* change rather than one the
        // compiler refuses for want of a type — an `unviable` mutant is a line no test was asked
        // about (NOTES § D133).
        let message: Option<&str> = match outcome {
            Outcome::NotSent { said, .. } | Outcome::Failed { said, .. } => said.as_deref(),
            _ => None,
        };
        let line = format!(
            "result · attempt {now} · {} · dry-run: {} · {}",
            self.object,
            self.check(outcome),
            verdict(outcome),
        );
        message.map_or_else(
            || format!("{line}\n"),
            |message| format!("{line}: {message}\n"),
        )
    }

    /// **What a check that did not fail says** — one rule, read by the dialog through
    /// [`Checked::verdict`] and by the audit log through [`Record::check`], so the screen and the
    /// record cannot come to disagree about whether a check was run.
    fn accepted(&self) -> &'static str {
        if self.checkable {
            ACCEPTED
        } else {
            UNCHECKABLE
        }
    }

    /// **What the check did**, as the audit log records it.
    fn check(&self, outcome: &Outcome) -> String {
        match outcome {
            Outcome::NotSent { fault, .. } => format!("not checked, {}", in_words(*fault)),
            _ => self.accepted().to_string(),
        }
    }
}

/// **An absent value and an empty one are one gap**, named rather than left as a dangling label.
fn gap(value: Option<&str>, absent: &str, prefix: &str) -> String {
    match value {
        Some(value) if !value.is_empty() => format!("{prefix}{value}"),
        _ => absent.to_string(),
    }
}

/// **What happened to the mutation, in one sentence per [`Outcome`]** (invariant 14).
fn verdict(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Done => "the change was made".to_string(),
        Outcome::Cancelled => "nobody confirmed it, so nothing was changed".to_string(),
        Outcome::Gone => "the object was already gone, so nothing was changed".to_string(),
        Outcome::Changed => {
            "the object changed while this was open, so nothing was changed".to_string()
        }
        // **"Sent" and "changed" are not the same word.** This arm is reached only from the
        // `Err` of a `dryRun=All` that went out, so *"nothing was sent"* — what this said until
        // 2026-09-04 — is false for every [`Fault`] it can carry: an operator holding this
        // beside the apiserver's own audit log finds the `?dryRun=All` at that timestamp and a
        // k8rs line denying it, and invariant 4 is that neither record may lie. What was never
        // sent is the change; the fault is already on the line, from [`Record::check`].
        Outcome::NotSent { .. } => "the change was never sent".to_string(),
        // **k8rs may not assert a failure it cannot see.** A broken pipe *after* the request went
        // out leaves the mutation's fate unknown, and *"the call itself failed"* — what this said
        // until 2026-09-04 — claims to know it did not land. Where the cluster answered, it did
        // not land and the sentence says so; where nothing usable came back, the honest line is
        // that k8rs does not know. Keyed on the fault, never on which branch fired
        // (`PRIOR-ART § C1`).
        Outcome::Failed { fault, .. } if answered(*fault) => {
            format!("nothing was changed: {}", in_words(*fault))
        }
        Outcome::Failed { fault, .. } => format!(
            "k8rs does not know whether the change was made — {}",
            in_words(*fault)
        ),
    }
}

/// **Whether the cluster itself answered**, which is what decides if a failed call is known to
/// have changed nothing (`k8s.rs` § WHAT WENT WRONG).
///
/// The four kubeconfig-and-login faults never left this machine, so nothing was changed by them
/// either — the two that leave it open are the two that mean *no usable answer came back*.
fn answered(fault: Fault) -> bool {
    !matches!(fault, Fault::Unanswered | Fault::Unfinished)
}

/// **One [`Fault`] in words, and the words are this file's** — a `Fault` is a fact and carries no
/// sentence (`k8s.rs` § WHAT WENT WRONG).
///
/// **Selected off the fault and never off which branch raised it**, which is the defect this
/// replaced: a dead socket, an expired credential, a moved `client-certificate` path and a login
/// program that exited `1` were all logged as *"the server refused the dry-run"* — three of them
/// failures on the operator's own machine, recorded as the cluster saying no (`PRIOR-ART § C1`,
/// `tester` and `k8s-admin`, 2026-09-04).
fn in_words(fault: Fault) -> &'static str {
    match fault {
        Fault::Refused => "the cluster would not allow it",
        Fault::Rejected => "the cluster would not accept the request k8rs made",
        Fault::Conflict => "the object had already been changed by something else",
        Fault::Expired => "the login k8rs was using had run out",
        Fault::Gone => "the cluster has no such object any more",
        Fault::Kubeconfig | Fault::NoContext | Fault::BadEntry | Fault::NoCredential => {
            "k8rs could not build a connection from this kubeconfig"
        }
        Fault::Unanswered | Fault::Unfinished => "k8rs could not reach the cluster",
    }
}

/// **One line, one `write_all`, then a flush** — so a destination that stamps or locks per line
/// can, and so two k8rs processes appending to one log cannot interleave halves of a record.
fn write_line(audit: &mut impl Write, line: &str) -> std::io::Result<()> {
    audit.write_all(line.as_bytes())?;
    audit.flush()
}

// --- THE MUTATION CONTRACT END ---
