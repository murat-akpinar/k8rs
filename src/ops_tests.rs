use super::*;

use std::cell::RefCell;
use std::rc::Rc;

// --- WHAT THE CONTRACT DID, IN ORDER ---
//
// **The whole box is an ordering claim**, so the double is a single transcript that the audit
// sink, the dialog and both closures push into: *the attempt line is on disk before anything is
// sent, the dialog is open before the check goes out, the check is answered before anybody can
// confirm, nobody is asked after a refused check, and the real call is last.* Separate spies
// could each be green while the order between them was wrong, which is the only defect this box
// exists to prevent.
//
// **The sink counts records and not syscalls.** It short-writes on purpose — eight bytes at a
// time, which a real `File` is allowed to do — so `breaks_at = 2` means *the second record*
// rather than *the second call into `write`*, and nothing here depends on `write_all` making one
// syscall per line (`tester`, 2026-09-04).

/// The most one `write` accepts. A destination is allowed to take less than it was offered, and
/// `File` does; a test whose record boundaries are syscall boundaries is testing the double.
const SHORT_WRITE: usize = 8;

/// What the dialog was handed when it opened.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Dialog {
    object: String,
    namespace: Option<String>,
    consequence: String,
    kubectl: String,
}

/// Everything that happened, in the order it happened.
#[derive(Default)]
struct Trace {
    steps: Vec<String>,
    /// Bytes accepted that have not yet reached the end of a record.
    partial: Vec<u8>,
    /// Complete records the sink has taken.
    records: usize,
    /// The 1-based *record* whose write fails; `0` for a sink that always works.
    breaks_at: usize,
    /// Whether the *flush* fails, which is the other half of "written and flushed".
    breaks_flush: bool,
    /// What [`Shown`] carried, the last time a dialog was opened.
    dialog: Option<Dialog>,
    /// What [`Checked::verdict`] carried, the last time a dialog's button went live.
    verdict: Option<String>,
}

type Shared = Rc<RefCell<Trace>>;

/// An audit destination that records what it was handed and can be made to fail.
struct Sink(Shared);

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut trace = self.0.borrow_mut();
        let taken = buf.len().min(SHORT_WRITE);
        let chunk = &buf[..taken];
        if let Some(end) = chunk.iter().position(|byte| *byte == b'\n') {
            if trace.records + 1 == trace.breaks_at {
                return Err(std::io::Error::other("no space left on device"));
            }
            trace.records += 1;
            trace.partial.extend_from_slice(&chunk[..=end]);
            let whole = String::from_utf8(std::mem::take(&mut trace.partial))
                .expect("a record is built from strings and cannot be invalid UTF-8");
            trace.steps.push(format!("audit: {whole}"));
            trace.partial.extend_from_slice(&chunk[end + 1..]);
        } else {
            trace.partial.extend_from_slice(chunk);
        }
        Ok(taken)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if self.0.borrow().breaks_flush {
            return Err(std::io::Error::other("no space left on device"));
        }
        Ok(())
    }
}

/// A sink with nothing wrong with it.
fn trace() -> Shared {
    Shared::default()
}

/// `2026-09-03T12:34:56Z`, so the attempt line is a fixed string a test can read.
fn stamp() -> Timestamp {
    Timestamp::from_second(1_788_438_896).expect("a timestamp inside jiff's range")
}

/// The scale this phase's first operation performs, described the way `scale` will describe it.
fn scaling() -> Mutation<'static> {
    Mutation {
        context: "kind-k8rs",
        namespace: Some("payments"),
        object: "deployment/web",
        consequence: "This changes how many copies of deployment/web are running, from 2 to 3.",
        kubectl: "kubectl scale deployment/web --replicas=3 -n payments",
        verb: "PATCH",
        path: "/apis/apps/v1/namespaces/payments/deployments/web/scale",
        // **None, because nothing [`Pass::patch`] returns carries one** — `PatchParams` is
        // `dry_run`, `force`, `field_manager`, `field_validation` and nothing else. Sending a
        // `resourceVersion` is todo.md 3693's work; a fixture that claims one today has the
        // audit line stating a field the request beside it does not send.
        version: None,
        checkable: true,
    }
}

/// The attempt line [`scaling`] produces, written out once so every test that expects it reads
/// the same string.
const ATTEMPT: &str = "audit: 2026-09-03T12:34:56Z attempt · deployment/web · context kind-k8rs · \
                       namespace payments · kubectl: kubectl scale deployment/web --replicas=3 \
                       -n payments · call: PATCH \
                       /apis/apps/v1/namespaces/payments/deployments/web/scale · \
                       resourceVersion not sent\n";

/// The `dry-run:` field every result line for a [`scaling`] that was checked and accepted carries
/// — written out from `screens/dialogs.md`'s own sentence, not from what the code returned.
const CHECKED_FIRST: &str = "dry-run: the cluster checked it first and accepted it";

/// A server refusal, built the way `k8s_tests.rs` builds one.
fn refusal(message: &str, reason: &str, code: u16) -> kube::Error {
    kube::Error::Api(
        kube::core::Status::failure(message, reason)
            .with_code(code)
            .boxed(),
    )
}

/// A failure with no `Status` behind it at all — a socket that died, which is the shape three of
/// this file's sentences used to be wrong about.
fn dead_socket(what: &'static str) -> kube::Error {
    kube::Error::ReadEvents(std::io::Error::other(what))
}

/// A closure that records the step and answers `Ok(())` for both passes.
fn works(trace: &Shared) -> impl Fn(Pass) -> std::future::Ready<Result<(), kube::Error>> + '_ {
    move |pass| {
        trace.borrow_mut().steps.push(step(pass));
        std::future::ready(Ok(()))
    }
}

/// **Whether this is the check, read the way an operation has to read it** — off the params it
/// would actually send, never off a `bool` beside them. A double that kept its own copy of the
/// flag would go on passing after [`Pass::patch`] stopped carrying it.
fn is_check(pass: Pass) -> bool {
    pass.patch().dry_run
}

/// What one pass of the operation is called in the transcript.
fn step(pass: Pass) -> String {
    if is_check(pass) { "dry-run" } else { "call" }.to_string()
}

/// A dialog that records what it was told to draw.
fn shows(trace: &Shared) -> impl FnOnce(&Shown<'_>) + '_ {
    move |shown| {
        let mut trace = trace.borrow_mut();
        trace.steps.push("shown".to_string());
        trace.dialog = Some(Dialog {
            object: shown.object.to_string(),
            namespace: shown.namespace.map(str::to_string),
            consequence: shown.consequence.to_string(),
            kubectl: shown.kubectl.to_string(),
        });
    }
}

/// A confirmation that records the verdict it was shown and ends the dialog with `answer`.
fn asked<R>(
    trace: &Shared,
    answer: Answer,
) -> impl FnOnce(Checked<R>) -> std::future::Ready<Answer> + '_ {
    move |checked| {
        let mut trace = trace.borrow_mut();
        trace.steps.push("asked".to_string());
        trace.verdict = Some(checked.verdict().to_string());
        std::future::ready(answer)
    }
}

/// Everything the transcript recorded, printed and returned.
fn transcript(trace: &Shared) -> Vec<String> {
    let steps = trace.borrow().steps.clone();
    // Printed so `cargo test -- --nocapture` shows the records and the consequence text a human
    // actually reads, rather than only that an assertion held (CLAUDE.md § Running it).
    println!("{}", steps.join("\n"));
    steps
}

/// **The whole contract, in one transcript** — the attempt on disk, then the dialog on screen,
/// then the check, then the confirmation, then the real call, then the result
/// (todo.md § Phase 7, `screens/dialogs.md` rule 3, NOTES § D21).
#[tokio::test]
async fn the_dialog_is_open_before_the_check_goes_out_and_the_attempt_is_recorded_before_both() {
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    assert_eq!(
        done,
        Performed {
            outcome: Some(Outcome::Done),
            recorded: true
        }
    );
    assert_eq!(
        transcript(&trace),
        vec![
            ATTEMPT.to_string(),
            "shown".to_string(),
            "dry-run".to_string(),
            "asked".to_string(),
            "call".to_string(),
            format!(
                "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · \
                 {CHECKED_FIRST} · the change was made\n"
            ),
        ],
        "the steps are no longer the order this box exists to fix — a dialog that opens only \
         after the check has come back is a keypress that appears to do nothing (NOTES § D20)"
    );
}

/// **The dialog is handed the object, the consequence and the kubectl line, all stripped**
/// (`screens/dialogs.md` rules 1 and 2, invariant 9).
#[tokio::test]
async fn the_dialog_gets_its_title_its_consequence_and_its_command_line_and_nothing_else() {
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let _ = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    assert_eq!(
        trace.borrow().dialog,
        Some(Dialog {
            object: "deployment/web".to_string(),
            namespace: Some("payments".to_string()),
            consequence: "This changes how many copies of deployment/web are running, from 2 to 3."
                .to_string(),
            kubectl: "kubectl scale deployment/web --replicas=3 -n payments".to_string(),
        }),
        "the dialog cannot draw its own title or its own $ line from what it was given"
    );
    assert_eq!(
        trace.borrow().verdict.as_deref(),
        Some("the cluster checked it first and accepted it"),
        "the verdict that turns the confirm button live is not the one the mockup draws"
    );
}

/// **The dry-run's own object reaches the dialog**, which is the whole of `edit`'s confirmation
/// in v0.4 and the reason the response is generic rather than `()`.
#[tokio::test]
async fn the_object_the_check_returned_is_what_the_dialog_is_given() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let seen = Rc::new(RefCell::new(None::<String>));
    let heard = seen.clone();

    let done = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        move |checked: Checked<String>| {
            *heard.borrow_mut() = checked.returned().cloned();
            std::future::ready(Answer::Confirmed)
        },
        |pass| {
            std::future::ready(Ok(if is_check(pass) {
                "replicas: 3 (not really)".to_string()
            } else {
                "replicas: 3".to_string()
            }))
        },
    )
    .await;

    assert_eq!(done.outcome, Some(Outcome::Done));
    assert_eq!(
        seen.borrow().as_deref(),
        Some("replicas: 3 (not really)"),
        "the object the cluster returned from the check never reached the dialog, so a diff \
         cannot be drawn from it"
    );
}

/// **Three endings that are not "no", and none of them may be logged as one** (NOTES § D22,
/// `screens/dialogs.md` § The object went away while the dialog was open, invariant 4).
///
/// The scenario is the ReplicaSet replacing the pod while its name is being typed: k8rs correctly
/// refuses, and the record has to say *k8rs stopped a wrong delete* rather than *the operator
/// backed out*.
#[tokio::test]
async fn cancelled_gone_and_changed_are_three_different_records_and_not_one() {
    let mut seen = Vec::new();
    for (answer, outcome) in [
        (Answer::Cancelled, Outcome::Cancelled),
        (Answer::Gone, Outcome::Gone),
        (Answer::Changed, Outcome::Changed),
    ] {
        let trace = trace();
        let mut sink = Sink(trace.clone());

        let ended = perform(
            &scaling(),
            stamp(),
            &mut sink,
            shows(&trace),
            asked(&trace, answer),
            works(&trace),
        )
        .await;

        assert_eq!(ended.outcome, Some(outcome));
        let steps = transcript(&trace);
        assert!(
            !steps.contains(&"call".to_string()),
            "a mutation nobody confirmed reached the API server: {answer:?}"
        );
        let line = steps.last().cloned().expect("a result line");
        assert!(
            line.contains(CHECKED_FIRST),
            "a record that does not say whether the write was checked first: {line}"
        );
        seen.push(line);
    }

    assert_eq!(
        seen,
        vec![
            format!(
                "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · {CHECKED_FIRST} \
                 · nobody confirmed it, so nothing was changed\n"
            ),
            format!(
                "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · {CHECKED_FIRST} \
                 · the object was already gone, so nothing was changed\n"
            ),
            format!(
                "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · {CHECKED_FIRST} \
                 · the object changed while this was open, so nothing was changed\n"
            ),
        ],
        "two of the three ways a dialog ends without a call print the same sentence, so the log \
         says the operator declined when what happened is that k8rs refused"
    );
}

/// **A refused check aborts before anyone is asked, and carries the server's own sentence**
/// (todo.md § Phase 7, `k8s.rs` § WHAT WENT WRONG).
#[tokio::test]
async fn a_refused_check_stops_before_the_confirmation_and_keeps_what_the_server_said() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let denial = "deployments.apps \"web\" is forbidden: User \"dev\" cannot patch resource \
                  \"deployments/scale\" in API group \"apps\" in the namespace \"payments\"";

    let stopped = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        |pass| {
            trace.borrow_mut().steps.push(step(pass));
            std::future::ready(if is_check(pass) {
                Err(refusal(denial, "Forbidden", 403))
            } else {
                Ok(())
            })
        },
    )
    .await;

    assert_eq!(
        stopped.outcome,
        Some(Outcome::NotSent {
            fault: Fault::Refused,
            said: Some(denial.to_string()),
        }),
        "the one sentence that says why the write is not allowed was thrown away"
    );
    let steps = transcript(&trace);
    assert_eq!(
        steps,
        vec![
            ATTEMPT.to_string(),
            "shown".to_string(),
            "dry-run".to_string(),
            format!(
                "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · dry-run: not \
                 checked, the cluster would not allow it · the change was never sent: {denial}\n"
            ),
        ],
        "a refused check either asked for a confirmation it could not honour, or went on to the \
         real call"
    );
    // **A record may not deny a request the transcript above proves went out** (invariant 4).
    // The equality is satisfied by updating its literal; this pair states the rule the literal
    // has to keep, and the premise first so it can never hold vacuously.
    assert!(
        steps.contains(&"dry-run".to_string()),
        "no check went out, so nothing here says what the record is allowed to claim: {steps:?}"
    );
    let recorded = steps.last().expect("a result line");
    assert!(
        !recorded.contains("nothing was sent"),
        "the `dryRun=All` went out and the record denies it — an operator reading this beside \
         the apiserver's own audit log has two records and one of them is false: {recorded}"
    );
}

/// **A `422` is a rejected request and not a network that answered nothing** (NOTES § D213) —
/// the shape a dry-run rejection actually arrives in, where the server's sentence *is* the
/// diagnosis.
#[tokio::test]
async fn an_invalid_object_is_a_rejected_request_and_keeps_the_servers_explanation() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let invalid = "Deployment.apps \"web\" is invalid: spec.replicas: Invalid value: -1: must be \
                   greater than or equal to 0";

    let stopped = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        |_| std::future::ready(Err::<(), _>(refusal(invalid, "Invalid", 422))),
    )
    .await;

    assert_eq!(
        stopped.outcome,
        Some(Outcome::NotSent {
            fault: Fault::Rejected,
            said: Some(invalid.to_string()),
        }),
        "a rejected dry-run — the commonest answer the write path gets — is classified as \
         something the network did"
    );
    assert_eq!(
        transcript(&trace).last().cloned(),
        Some(format!(
            "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · dry-run: not \
             checked, the cluster would not accept the request k8rs made · the change was \
             never sent: {invalid}\n"
        )),
        "the audit log does not record which field the server rejected"
    );
}

/// **A failure on the operator's own machine is never recorded as the cluster saying no**
/// (`PRIOR-ART § C1`, `tester` 2026-09-04).
///
/// A dead socket, a login that ran out and a login program that produced nothing all wrote *"the
/// server refused the dry-run, so the change was never sent"* until 2026-09-04 — the server
/// refused nothing, and for two of the three it was never asked. The sentence is selected off the
/// [`Fault`], never off which branch fired.
#[tokio::test]
async fn a_failure_this_side_of_the_wire_is_not_recorded_as_the_server_refusing() {
    for (error, fault, words) in [
        (
            dead_socket("connection reset by peer"),
            Fault::Unanswered,
            "k8rs could not reach the cluster",
        ),
        (
            refusal("Unauthorized", "Unauthorized", 401),
            Fault::Expired,
            "the login k8rs was using had run out",
        ),
        (
            refusal("", "Conflict", 409),
            Fault::Conflict,
            "the object had already been changed by something else",
        ),
    ] {
        let trace = trace();
        let mut sink = Sink(trace.clone());

        let stopped = perform(
            &scaling(),
            stamp(),
            &mut sink,
            shows(&trace),
            asked(&trace, Answer::Confirmed),
            |_| std::future::ready(Err::<(), _>(clone_of(&error))),
        )
        .await;

        let Some(Outcome::NotSent { fault: got, .. }) = stopped.outcome else {
            panic!("a check that did not pass was not reported as one");
        };
        assert_eq!(got, fault);
        let line = transcript(&trace).last().cloned().expect("a result line");
        assert!(
            line.contains(&format!("dry-run: not checked, {words}")),
            "the record keys its sentence on which branch fired rather than on what failed: \
             {line}"
        );
        assert!(
            !line.contains("would not allow it"),
            "a failure the cluster had nothing to do with is recorded as the cluster refusing: \
             {line}"
        );
    }
}

/// `kube::Error` is not `Clone`, and three shapes are needed twice each.
fn clone_of(error: &kube::Error) -> kube::Error {
    match error {
        kube::Error::Api(status) => kube::Error::Api(status.clone()),
        _ => dead_socket("connection reset by peer"),
    }
}

/// **A call that fails after a good check is a different fact from a refused check**: something
/// was sent, and the two lines a screen prints are not the same.
///
/// **A credential that expired between the two calls is the shape this is written for** and not
/// an invented one — `k8s.rs` § WHAT WENT WRONG measured a login program that answers once and
/// then stops, and a `dryRun=All` pass is exactly the round trip that can sit either side of
/// that boundary.
#[tokio::test]
async fn a_call_that_fails_after_a_good_check_is_told_apart_from_a_refused_check() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let expired = "Unauthorized";

    let failed = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        |pass| {
            trace.borrow_mut().steps.push(step(pass));
            std::future::ready(if is_check(pass) {
                Ok(())
            } else {
                Err(refusal(expired, "Unauthorized", 401))
            })
        },
    )
    .await;

    assert_eq!(
        failed.outcome,
        Some(Outcome::Failed {
            fault: Fault::Expired,
            said: Some(expired.to_string()),
        }),
        "a call that failed after the check passed was reported as something else, or lost which \
         failure it was"
    );
    assert_eq!(
        transcript(&trace).last().cloned(),
        Some(format!(
            "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · {CHECKED_FIRST} · \
             nothing was changed: the login k8rs was using had run out: {expired}\n"
        )),
        "a failed mutation is not in the audit log as one"
    );
}

/// **A `409` on the real call is a conflict and not a network failure** (NOTES § D213), which is
/// the input the re-read offer is built out of (todo.md § Phase 7, the `resourceVersion` box).
#[tokio::test]
async fn a_conflict_on_the_real_call_is_its_own_fault_and_keeps_the_servers_own_words() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let conflict = "Operation cannot be fulfilled on deployments.apps \"web\": the object has \
                    been modified; please apply your changes to the latest version and try again";

    let failed = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        |pass| {
            std::future::ready(if is_check(pass) {
                Ok(())
            } else {
                Err(refusal(conflict, "Conflict", 409))
            })
        },
    )
    .await;

    assert_eq!(
        failed.outcome,
        Some(Outcome::Failed {
            fault: Fault::Conflict,
            said: Some(conflict.to_string()),
        }),
        "a lost race cannot be told from a dead socket, so the re-read offer has nothing to \
         branch on"
    );
    assert!(
        transcript(&trace)
            .last()
            .is_some_and(|line| line.contains(conflict)),
        "the audit log does not say the object had moved under the write"
    );
}

/// **k8rs may not assert a failure it cannot see** (invariant 4, `PRIOR-ART § C1`).
///
/// A broken pipe *after* the request went out leaves the mutation's fate unknown. The line said
/// *"the dry-run passed and the call itself failed"* until 2026-09-04, which claims to know it
/// did not land.
#[tokio::test]
async fn a_broken_pipe_after_the_request_went_out_says_k8rs_does_not_know() {
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let failed = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        |pass| {
            std::future::ready(if is_check(pass) {
                Ok(())
            } else {
                Err(dead_socket("broken pipe"))
            })
        },
    )
    .await;

    assert_eq!(
        failed.outcome,
        Some(Outcome::Failed {
            fault: Fault::Unanswered,
            said: None,
        })
    );
    assert_eq!(
        transcript(&trace).last().cloned(),
        Some(format!(
            "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · {CHECKED_FIRST} · \
             k8rs does not know whether the change was made — k8rs could not reach the cluster\n"
        )),
        "the record asserts the change did not happen, which is the one thing k8rs cannot see \
         from here"
    );
}

/// **Every record says whether the write was checked first** (NOTES § D8: verb, path,
/// resourceVersion sent, **dry-run verdict**, result — unconditionally).
///
/// *Did this write get checked first?* is the one question the log exists to answer about the
/// contract itself, and on the two outcomes an operator reads most it could not be answered at
/// all: the word "dry-run" appeared nowhere on `Done` or on a declined mutation (`tester`,
/// 2026-09-04).
#[tokio::test]
async fn the_dry_run_verdict_is_on_every_result_line_and_not_only_where_it_failed() {
    for answer in [
        Answer::Confirmed,
        Answer::Cancelled,
        Answer::Gone,
        Answer::Changed,
    ] {
        for checkable in [true, false] {
            let trace = trace();
            let mut sink = Sink(trace.clone());
            let record = Mutation {
                checkable,
                ..scaling()
            };

            let _ = perform(
                &record,
                stamp(),
                &mut sink,
                shows(&trace),
                asked(&trace, answer),
                works(&trace),
            )
            .await;

            let line = transcript(&trace).last().cloned().expect("a result line");
            assert!(
                line.contains("dry-run: "),
                "a record that cannot answer whether the write was checked first: {line}"
            );
        }
    }
}

/// **An operation that declines the check says so, and sends nothing before the confirmation.**
///
/// **The verdict names k8rs and not the cluster** (NOTES § D215). A real cluster dry-runs both
/// verbs this file sends, so *"the cluster has no way to check this one first"* — what this
/// asserted until 2026-09-04 — was false every time it printed, in a dialog invariant 14 writes
/// for someone in their first month.
///
/// **The subject is the flag and not any operation.** This used to set `checkable: false` on a
/// fixture whose kubectl line read `kubectl rollout restart`, which is a claim about `restart`
/// that D215 measured to be wrong; which operations decline, and why, is each operation's own
/// box (todo.md 3687, 3689, 3692) and not this one's.
#[tokio::test]
async fn an_operation_that_declines_the_check_records_that_none_was_run_and_sends_nothing_early() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let unchecked = Mutation {
        checkable: false,
        ..scaling()
    };

    let done = perform(
        &unchecked,
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    assert_eq!(done.outcome, Some(Outcome::Done));
    let steps = transcript(&trace);
    assert_eq!(
        steps.iter().filter(|step| *step == "dry-run").count(),
        0,
        "an operation that declared it sends no check was sent one anyway"
    );
    assert_eq!(
        trace.borrow().verdict.as_deref(),
        Some("k8rs did not check this one with the cluster first"),
        "the dialog claims a check that never ran, or blames the cluster for k8rs not running one"
    );
    assert_eq!(
        steps.last().cloned(),
        Some(
            "audit: result · attempt 2026-09-03T12:34:56Z · deployment/web · dry-run: k8rs did \
             not check this one with the cluster first · the change was made\n"
                .to_string()
        ),
        "the audit log omits the verdict field rather than recording that there was no check"
    );
}

/// **A mutation that cannot be recorded does not happen** (NOTES § D21) — not the dialog, not the
/// check, not the confirmation, not the call.
///
/// **The canary is the first half of this test.** *Found nothing* and *nothing to find* print the
/// same pass, so the same trace machinery is made to record a full transcript first; only then is
/// its emptiness under a broken sink evidence of anything.
#[tokio::test]
async fn an_attempt_that_cannot_be_written_stops_before_anything_is_sent() {
    let canary = trace();
    let mut working = Sink(canary.clone());
    let _ = perform(
        &scaling(),
        stamp(),
        &mut working,
        shows(&canary),
        asked(&canary, Answer::Confirmed),
        works(&canary),
    )
    .await;
    assert_eq!(
        transcript(&canary).len(),
        6,
        "the transcript records nothing even when the sink works, so an empty one below would \
         prove nothing"
    );

    let trace = trace();
    trace.borrow_mut().breaks_at = 1;
    let mut sink = Sink(trace.clone());

    let refused = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    assert_eq!(
        refused,
        Performed {
            outcome: None,
            recorded: false
        }
    );
    assert!(
        transcript(&trace).is_empty(),
        "the audit log could not be written and k8rs went to the cluster anyway"
    );
}

/// **Written *and* flushed** — a line sitting in a buffer is not a record (NOTES § D21).
#[tokio::test]
async fn an_attempt_that_cannot_be_flushed_stops_too() {
    let trace = trace();
    trace.borrow_mut().breaks_flush = true;
    let mut sink = Sink(trace.clone());

    let refused = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    assert_eq!(
        refused,
        Performed {
            outcome: None,
            recorded: false
        }
    );
    assert_eq!(
        transcript(&trace),
        vec![ATTEMPT.to_string()],
        "the attempt was accepted by a sink that could not flush it, and the call went out"
    );
}

/// **A result line that cannot be written does not un-make the change** (NOTES § D21 governs the
/// attempt line; a result that already exists is a fact k8rs holds).
///
/// *"The change was made — but k8rs could not write it to the audit log"* is what an operator
/// needs at the moment they are least able to go and look; the outcome used to be thrown away and
/// replaced by *go and look* (`k8s-admin`, 2026-09-04).
#[tokio::test]
async fn a_result_that_cannot_be_recorded_keeps_the_outcome_k8rs_already_knows() {
    for (answer, outcome) in [
        (Answer::Confirmed, Outcome::Done),
        (Answer::Cancelled, Outcome::Cancelled),
    ] {
        let trace = trace();
        trace.borrow_mut().breaks_at = 2;
        let mut sink = Sink(trace.clone());

        let performed = perform(
            &scaling(),
            stamp(),
            &mut sink,
            shows(&trace),
            asked(&trace, answer),
            works(&trace),
        )
        .await;

        assert_eq!(
            performed,
            Performed {
                outcome: Some(outcome),
                recorded: false
            },
            "the trail broke after the call returned and k8rs discarded what it already knew"
        );
        assert_eq!(
            transcript(&trace),
            vec![
                ATTEMPT.to_string(),
                "shown".to_string(),
                "dry-run".to_string(),
                "asked".to_string(),
            ]
            .into_iter()
            .chain(match answer {
                Answer::Confirmed => vec!["call".to_string()],
                _ => vec![],
            })
            .collect::<Vec<_>>(),
            "the result line reached a log that was supposed to have refused it"
        );
    }
}

/// **A result can be paired with the attempt it belongs to, by content and not by adjacency.**
///
/// Two k8rs against two clusters share one `~/.local/state/k8rs/audit.log`, and a drain takes
/// minutes (NOTES § D20), so `attempt(A) attempt(B) result(B) result(A)` is ordinary. Here the
/// second mutation runs *inside* the first one's confirmation, so the interleave is real rather
/// than described.
#[tokio::test]
async fn a_result_names_the_attempt_it_belongs_to_rather_than_the_one_above_it() {
    let trace = trace();
    let mut outer = Sink(trace.clone());
    let inner_stamp = Timestamp::from_second(1_788_439_000).expect("inside jiff's range");
    let inner_trace = trace.clone();

    let _ = perform(
        &scaling(),
        stamp(),
        &mut outer,
        shows(&trace),
        move |_: Checked<()>| async move {
            let mut inner = Sink(inner_trace.clone());
            let cordon = Mutation {
                namespace: None,
                object: "node/k8rs-worker2",
                consequence: "This stops new pods being scheduled onto k8rs-worker2.",
                kubectl: "kubectl cordon k8rs-worker2",
                verb: "PATCH",
                path: "/api/v1/nodes/k8rs-worker2",
                version: None,
                checkable: false,
                ..scaling()
            };
            let _ = perform(
                &cordon,
                inner_stamp,
                &mut inner,
                |_: &Shown<'_>| {},
                |_: Checked<()>| std::future::ready(Answer::Confirmed),
                |_| std::future::ready(Ok::<(), kube::Error>(())),
            )
            .await;
            Answer::Confirmed
        },
        works(&trace),
    )
    .await;

    let records: Vec<String> = transcript(&trace)
        .into_iter()
        .filter(|step| step.starts_with("audit: "))
        .collect();
    assert_eq!(records.len(), 4, "two mutations write four records");
    assert!(
        records[1].contains("2026-09-03T12:36:40Z attempt · node/k8rs-worker2"),
        "the inner mutation did not interleave, so this test proves nothing: {records:#?}"
    );
    assert!(
        records[2].contains("result · attempt 2026-09-03T12:36:40Z · node/k8rs-worker2"),
        "a result line that sits under the wrong attempt cannot say which one it is: {records:#?}"
    );
    assert!(
        records[3].contains("result · attempt 2026-09-03T12:34:56Z · deployment/web"),
        "the outer mutation's result names neither its attempt nor its object: {records:#?}"
    );
}

/// **Nothing a person typed or a cluster sent can forge a log line or move a cursor**
/// (invariant 9, NOTES § D154). The object name here is what `--object` accepts from argv, which
/// never passed the ingest guard.
///
/// **The assertion is the literal record and not the predicate the strip filters on.** Asserting
/// `!line.chars().any(unprintable)` can only fail if the strip is not called at all, so it is
/// structurally incapable of catching `unprintable` being too narrow — which is the defect
/// NOTES § D154 exists about (`tester`, 2026-09-04).
#[tokio::test]
async fn nothing_written_into_a_record_can_forge_a_line_or_rewrite_the_terminal() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let crafted = Mutation {
        context: "kind\u{1b}[2Jk8rs",
        namespace: Some("pay\u{200b}ments"),
        object: "deployment/web\u{202e}gnp",
        consequence: "This deletes deployment/web\u{202e}gnp.",
        kubectl: "kubectl delete deployment/web\nresult · the change was made",
        verb: "DELETE",
        path: "/apis/apps/v1/namespaces/payments/deployments/web\u{7}",
        version: Some("81\u{feff}23"),
        checkable: true,
    };

    let done = perform(
        &crafted,
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    assert_eq!(done.outcome, Some(Outcome::Done));
    let steps = transcript(&trace);
    assert_eq!(
        steps.first().cloned(),
        Some(
            "audit: 2026-09-03T12:34:56Z attempt · deployment/webgnp · context kind[2Jk8rs · \
             namespace payments · kubectl: kubectl delete deployment/web result · the change was \
             made · call: DELETE /apis/apps/v1/namespaces/payments/deployments/web · \
             resourceVersion 8123\n"
                .to_string()
        ),
        "a control, bidi or zero-width character reached a record a human reads"
    );
    for line in &steps {
        assert_eq!(
            line.matches('\n').count(),
            usize::from(line.starts_with("audit: ")),
            "{line:?} carries a newline that is not the one ending the record — a second log \
             line can be forged from an object name"
        );
    }
    assert_eq!(
        trace
            .borrow()
            .dialog
            .as_ref()
            .map(|shown| shown.consequence.clone()),
        Some("This deletes deployment/webgnp.".to_string()),
        "the consequence text put on screen was not stripped"
    );
}

/// **What the server wrote cannot forge a record either** (invariant 9, `k8s.rs` § `message`).
///
/// When a response body is not JSON at all, kube puts **the whole body** in `Status::message`
/// (`kube-client-4.2.0/src/client/mod.rs:551-558`), so a proxy's HTML error page arrives as one
/// message — newlines, the audit format's own ` · ` separator and all. It reaches the record
/// through `k8s::said`, which is where the strip and the bound are; this is the assertion that
/// nothing here undoes them (`tester`, 2026-09-04).
#[tokio::test]
async fn a_server_message_cannot_forge_a_second_record_or_a_field_that_was_never_sent() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let body = "<html>\n<head><title>502 Bad Gateway</title></head>\n\
                result · attempt 2026-09-03T12:34:56Z · deployment/web · dry-run: the cluster \
                checked it first and accepted it · the change was made\n</html>";

    let _ = perform(
        &scaling(),
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        |_| {
            std::future::ready(Err::<(), _>(refusal(
                body,
                "Failed to parse error data",
                502,
            )))
        },
    )
    .await;

    let records: Vec<String> = transcript(&trace)
        .into_iter()
        .filter(|step| step.starts_with("audit: "))
        .collect();
    assert_eq!(
        records.len(),
        2,
        "a server body was written as more than the two records this mutation has: {records:#?}"
    );
    for record in &records {
        assert_eq!(
            record.matches('\n').count(),
            1,
            "a second audit record was forged out of what the server wrote: {record:?}"
        );
    }
    assert!(
        records[1].contains("502 Bad Gateway") && !records[1].contains("<head>\n"),
        "the server's own words were dropped, or its line breaks survived into the record: {}",
        records[1]
    );
}

/// **A break inside a field separates two words rather than gluing them** (NOTES § D154, D213).
///
/// This file spelled its own strip for one box and it diverged on *disposal* immediately: it
/// removed where `k8s::text` substitutes one space. Measured on the exact string
/// `screens/dialogs.md:39` draws, and on two object names a newline apart.
#[tokio::test]
async fn a_newline_inside_a_field_separates_two_words_instead_of_fusing_them() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let wrapped = Mutation {
        object: "deployment/web\ndeployment/db",
        consequence: "This removes the pod. Its Deployment will start a\nreplacement immediately \
                      — the app keeps running.",
        ..scaling()
    };

    let _ = perform(
        &wrapped,
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    let dialog = trace.borrow().dialog.clone().expect("a dialog was shown");
    assert_eq!(
        dialog.consequence,
        "This removes the pod. Its Deployment will start a replacement immediately — the app \
         keeps running.",
        "the consequence the screen draws has two words glued into one"
    );
    assert_eq!(
        dialog.object, "deployment/web deployment/db",
        "two object names were fused into one"
    );
}

/// **A field the ingest guard never saw is bounded here** — `Mutation`'s strings come from argv
/// and from k8rs's own formatting, not from a watch, so nothing upstream has bounded them
/// (NOTES § D146, § D213).
#[tokio::test]
async fn an_oversized_field_is_cut_and_says_it_was_cut() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let huge = "x".repeat(500_000);
    let record = Mutation {
        object: &huge,
        ..scaling()
    };

    let _ = perform(
        &record,
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    let attempt = trace.borrow().steps[0].clone();
    println!("attempt line is {} bytes", attempt.len());
    assert!(
        attempt.len() < 1_000,
        "a 500 000-byte object name reached the audit line whole: {} bytes",
        attempt.len()
    );
    assert!(
        attempt.contains("(shortened by k8rs)"),
        "a record was cut silently, which `screens/widgets.md` § 7 forbids: {attempt}"
    );
}

/// **A cluster-scoped object and a call with no `resourceVersion` say so**, rather than leaving
/// a reader to guess whether the field was empty or the record was short (NOTES § D8).
#[tokio::test]
async fn a_cluster_scoped_call_with_no_resource_version_records_both_absences() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let cordon = Mutation {
        context: "kind-k8rs",
        namespace: None,
        object: "node/k8rs-worker2",
        consequence: "This stops new pods being scheduled onto k8rs-worker2. Pods already \
                      running there keep running.",
        kubectl: "kubectl cordon k8rs-worker2",
        verb: "PATCH",
        path: "/api/v1/nodes/k8rs-worker2",
        version: None,
        checkable: false,
    };

    let done = perform(
        &cordon,
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;

    assert_eq!(done.outcome, Some(Outcome::Done));
    assert_eq!(
        transcript(&trace).first().cloned(),
        Some(
            "audit: 2026-09-03T12:34:56Z attempt · node/k8rs-worker2 · context kind-k8rs · \
             cluster-wide · kubectl: kubectl cordon k8rs-worker2 · call: PATCH \
             /api/v1/nodes/k8rs-worker2 · resourceVersion not sent\n"
                .to_string()
        ),
        "an absent namespace or resourceVersion is recorded as a gap rather than as a fact"
    );
}

/// **An empty value records the way an absent one does** (PM ruling, 2026-09-04).
///
/// `resourceVersion ` with nothing after it is a dangling label, indistinguishable from a record
/// that was cut off — and a value that is entirely unprintable strips to exactly that.
#[tokio::test]
async fn an_empty_or_wholly_stripped_field_records_as_the_gap_it_is() {
    for (name, namespace, version) in [
        ("empty strings", Some(""), Some("")),
        ("wholly unprintable", Some("\u{200b}"), Some("\u{7}")),
    ] {
        let trace = trace();
        let mut sink = Sink(trace.clone());
        let record = Mutation {
            namespace,
            version,
            ..scaling()
        };

        let _ = perform(
            &record,
            stamp(),
            &mut sink,
            shows(&trace),
            asked(&trace, Answer::Confirmed),
            works(&trace),
        )
        .await;

        let attempt = transcript(&trace)
            .first()
            .cloned()
            .expect("an attempt line");
        assert!(
            attempt.contains(" · cluster-wide · ")
                && attempt.ends_with("resourceVersion not sent\n"),
            "{name} left a dangling label a reader cannot tell from a truncated record: {attempt}"
        );
    }
}

/// **Invariant 2 requires the dialog to *state* the consequence, and an empty string states
/// nothing.**
///
/// No operation can reach this today — every consequence is a k8rs sentence with a name
/// interpolated into it — so it is the author's error rather than the cluster's, and an assertion
/// that costs nothing in release is the right shape.
#[tokio::test]
#[should_panic(expected = "nothing to state on screen")]
async fn a_consequence_that_states_nothing_is_stopped_before_anything_is_written() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let silent = Mutation {
        consequence: "\u{feff}",
        ..scaling()
    };

    let _ = perform(
        &silent,
        stamp(),
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
        works(&trace),
    )
    .await;
}

// --- WHAT THE PASS PUTS ON THE WIRE ---
//
// **A test that asserts `params.dry_run == true` asserts that the line above it wrote `true`.**
// What invariant 2 requires is `dryRun=All` reaching the API server, and kube does not put it in
// the same place for both shapes — a patch carries it in the query string, a delete carries it in
// the request body. `populate_qp` is `pub(crate)` and cannot be called from here, so these build
// the request kube itself builds — `kube::core::Request`, which `Api<K>`'s methods delegate
// to — and read the URI and the body off it.
//
// **Negatives are not decoration here.** `FOR_REAL` producing a request that still says
// `dryRun=All` would be a mutation that never lands, which no positive test can see.
//
// **What these assert is `dryRun`, and nothing else about the request** (NOTES § D215). An
// exact-equality assertion on a delete's body reads every future field as this box's defect: add
// todo.md 3692's `propagationPolicy: Background` and the failure printed *"the body does not
// carry it"* beside a body that plainly did, whose obvious repair is to paste the new output into
// the literal — the assertion drift CLAUDE.md § Code phase rules forbids (`tester`, 2026-09-04).

/// The collection path a `deployment/web` request is built against.
fn deployments() -> kube::core::Request {
    kube::core::Request::new("/apis/apps/v1/namespaces/payments/deployments")
}

/// A patch's URI, printed so `--nocapture` shows the query string a human would read off the
/// apiserver's own audit log.
///
/// **The scale subresource and not a plain `PATCH`**, because `scale` is this phase's only
/// written caller of [`Pass::patch`]. `Api::patch_scale` delegates to
/// `Request::patch_subresource` (`kube-client-4.2.0/src/api/subresource.rs:44`), which is a
/// different entry point from `Request::patch` and reaches the same `PatchParams::populate_qp`;
/// this asserts the one the operation will use rather than the one next to it.
fn patch_uri(pass: Pass) -> String {
    let request = deployments()
        .patch_subresource(
            "scale",
            "web",
            &pass.patch(),
            &kube::api::Patch::Merge(serde_json::json!({ "spec": { "replicas": 3 } })),
        )
        .expect("a patch request built from a valid name and this file's own params");
    let uri = request.uri().to_string();
    println!("PATCH {uri}");
    uri
}

/// A delete's URI and body, the second of which is where its `dryRun` lives.
fn delete_wire(pass: Pass) -> (String, String) {
    let request = deployments()
        .delete("web", &pass.delete())
        .expect("a delete request built from a valid name and this file's own params");
    let uri = request.uri().to_string();
    let body = String::from_utf8(request.body().clone())
        .expect("kube serialises DeleteParams with serde_json, which cannot emit invalid UTF-8");
    println!("DELETE {uri} · body {body}");
    (uri, body)
}

/// **The check pass puts `dryRun=All` on the wire for both shapes** — invariant 2's *server-side
/// `dryRun=All`*, asserted where the API server would read it and not on a struct field.
#[test]
fn the_check_pass_carries_dry_run_all_on_the_wire_for_a_patch_and_a_delete() {
    assert!(
        patch_uri(DRY_RUN).contains("dryRun=All"),
        "a patch built from the check pass would change the cluster: nothing in the query string \
         tells the API server this is a dry run"
    );
    let (uri, body) = delete_wire(DRY_RUN);
    assert!(
        body.contains(r#""dryRun":["All"]"#),
        "a delete built from the check pass would delete the object: its dryRun rides in the \
         body, and this body does not carry one — {body}"
    );
    assert!(
        !uri.contains("dryRun"),
        "the delete's dryRun was expected in the body and this URI carries one too — kube's \
         wire format has changed and this file's doc comment is now wrong"
    );
}

/// **The real pass says nothing about a dry run, for either shape.** A `dryRun=All` left on the
/// second call is a k8rs that confirms a change and never makes it — the audit log says *the
/// change was made* and the cluster disagrees, which is invariant 4's *neither record may lie*.
#[test]
fn the_real_pass_asks_for_no_dry_run_at_all_for_a_patch_and_a_delete() {
    assert!(
        !patch_uri(FOR_REAL).contains("dryRun"),
        "the real patch is still a dry run, so the change is confirmed and never made"
    );
    let (uri, body) = delete_wire(FOR_REAL);
    assert!(
        !body.contains("dryRun"),
        "the real delete is still a dry run, so the object is never deleted — {body}"
    );
    assert!(
        !uri.contains("dryRun"),
        "the real delete's URI carries a dryRun kube is not supposed to put there"
    );
}

/// **The two passes are not the same request**, which is the hole this box closes: a `Pass` that
/// answered the same params either way would satisfy every ordering test in this file, because
/// the contract can sequence two calls and cannot see what either one sent.
///
/// Two live requests compared against each other and never against a literal, so a field a later
/// box adds to both shapes leaves this saying what it means.
#[test]
fn the_check_and_the_real_call_are_not_the_same_request() {
    assert_ne!(
        patch_uri(DRY_RUN),
        patch_uri(FOR_REAL),
        "the check and the change are the same PATCH, so one of them is the wrong one"
    );
    assert_ne!(
        delete_wire(DRY_RUN),
        delete_wire(FOR_REAL),
        "the check and the change are the same DELETE, so one of them is the wrong one"
    );
}
