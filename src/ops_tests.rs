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
        // The `server:` of the kubeconfig entry — the fact that says *which* `kind-k8rs`, since
        // a second `kind create cluster` writes the same context name and a different port. A
        // reserved host rather than the `127.0.0.1` a real kind writes, because
        // `scripts/security-guard.py` reads a loopback URL in this tree as a second outbound path
        // and is right to; the port is what carries the point.
        server: "https://k8rs-tests.invalid:41751",
        namespace: Some("payments"),
        object: "deployment/web",
        // **A deployment's own `uid`, because `scale` has read the deployment by the time it
        // builds the consequence** — *from 2 to 3* is off that object. It is not a field the
        // request carries, which is why it is `Some` here where `version` is `None`: it is what
        // k8rs saw, not what k8rs sent.
        uid: Some("18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11"),
        // **What [`consequence`] really produces for 2 → 3** — `screens/dialogs.md` § Scale's own
        // *up, by one* relation. It was a hand-written sentence while no operation existed to
        // produce one; a fixture whose doc says *the way `scale` will describe it* and does not
        // is the second copy that goes stale (CLAUDE.md § D103).
        consequence: "This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies.",
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
                       server https://k8rs-tests.invalid:41751 · namespace payments · \
                       uid 18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11 · \
                       kubectl: kubectl scale deployment/web --replicas=3 \
                       -n payments · call: PATCH \
                       /apis/apps/v1/namespaces/payments/deployments/web/scale · \
                       resourceVersion not sent\n";

/// **The head of every result line a [`scaling`] on [`stamp`] produces** — both stamps and the
/// object, written out once so a test that cares about the sentence after them says only that.
///
/// **The two stamps read the same here because [`stamp`] is a fixed clock**, which is exactly why
/// it cannot show that the second one is a second reading;
/// `a_result_says_when_it_was_recorded_and_not_only_when_it_was_attempted` uses a clock that
/// moves, and is the only test that can tell the two fields apart.
const RESULT: &str = "audit: result · attempt 2026-09-03T12:34:56Z · recorded \
                      2026-09-03T12:34:56Z · deployment/web";

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
        stamp,
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
            format!("{RESULT} · {CHECKED_FIRST} · the change was made\n"),
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
        stamp,
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
            consequence:
                "This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies."
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
        stamp,
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
            stamp,
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
            format!("{RESULT} · {CHECKED_FIRST} · nobody confirmed it, so nothing was changed\n"),
            format!(
                "{RESULT} · {CHECKED_FIRST} · the object was already gone, so nothing was \
                 changed\n"
            ),
            format!(
                "{RESULT} · {CHECKED_FIRST} · the object changed while this was open, so \
                 nothing was changed\n"
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
        stamp,
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
                "{RESULT} · dry-run: not checked · the change was never sent — the cluster would \
                 not allow it: {denial}\n"
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
        stamp,
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
            "{RESULT} · dry-run: not checked · the change was never sent — the cluster would not \
             accept the request k8rs made: {invalid}\n"
        )),
        "the audit log does not record which field the server rejected"
    );
}

/// **The surface the operator is looking at names the fault *and* the cluster's own words** —
/// `403` and `422` on the same call printed one identical line until 2026-09-04 (`k8s-admin`).
///
/// The audit log had both all along — [`Record::check`] read the fault and [`Record::result_line`]
/// appended the message — while the person who ran the operation got `k8rs: the change was never
/// sent`, byte for byte, whichever it was. That breaks the security gate's *a 403 … names the
/// missing verb + resource*, `PRIOR-ART § C1`'s *a fallback message may never replace a typed
/// error*, and the operation's own consistency: a `403` on the `GET` forty milliseconds earlier
/// names both.
///
/// **Both halves are load-bearing on every row.** Dropping the fault makes rows 1 and 2 the same
/// sentence, which is the defect itself; dropping the message makes every row short of what the
/// operator needs. Neither can be lost and leave this test green — which is the property, not that
/// the rows have distinct prefixes, since rows 1 and 2 deliberately share one.
///
/// **The last row is the unhappy path composed with the unhappy path**: a failure whose words
/// arrived, on a run whose log could not be written. The two clauses have to stack in that order
/// or the *k8rs could not write that down* clause sits between the fault and the server's answer.
#[test]
fn the_sentence_the_operator_reads_names_the_fault_and_what_the_cluster_said() {
    let denial = "deployments.apps \"web\" is forbidden: User \"dev\" cannot patch resource \
                  \"deployments/scale\" in API group \"apps\" in the namespace \"payments\"";
    let invalid = "Deployment.apps \"web\" is invalid: spec.replicas: Invalid value: -1: must be \
                   greater than or equal to 0";
    for (outcome, recorded, expected) in [
        (
            Outcome::NotSent {
                fault: Fault::Refused,
                said: Some(denial.to_string()),
            },
            true,
            format!("the change was never sent — the cluster would not allow it: {denial}"),
        ),
        (
            Outcome::NotSent {
                fault: Fault::Rejected,
                said: Some(invalid.to_string()),
            },
            true,
            format!(
                "the change was never sent — the cluster would not accept the request k8rs made: \
                 {invalid}"
            ),
        ),
        (
            Outcome::Failed {
                fault: Fault::Rejected,
                said: Some(invalid.to_string()),
            },
            true,
            format!(
                "nothing was changed — the cluster would not accept the request k8rs made: \
                 {invalid}"
            ),
        ),
        (
            Outcome::Failed {
                fault: Fault::Unanswered,
                said: None,
            },
            true,
            "k8rs does not know whether the change was made — k8rs could not reach the cluster"
                .to_string(),
        ),
        (
            Outcome::Failed {
                fault: Fault::Refused,
                said: Some(denial.to_string()),
            },
            false,
            format!(
                "nothing was changed — the cluster would not allow it: {denial} — but k8rs could \
                 not write that to the audit log, so the trail of it is short a line"
            ),
        ),
    ] {
        let performed = Performed {
            outcome: Some(outcome),
            recorded,
        };
        let sentence = performed.plainly();
        println!("{sentence}\n");
        assert_eq!(sentence, expected);
    }
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
            stamp,
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
            line.contains(&format!("the change was never sent — {words}")),
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
        stamp,
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
            "{RESULT} · {CHECKED_FIRST} · nothing was changed — the login k8rs was using had \
             run out: {expired}\n"
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
        stamp,
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
        stamp,
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
            "{RESULT} · {CHECKED_FIRST} · k8rs does not know whether the change was made — \
             k8rs could not reach the cluster\n"
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
                stamp,
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
        stamp,
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
        Some(format!(
            "{RESULT} · dry-run: k8rs did not check this one with the cluster first · the \
                 change was made\n"
        )),
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
        stamp,
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
        stamp,
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
        stamp,
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
            stamp,
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
        stamp,
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
                move || inner_stamp,
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
        records[2].contains(
            "result · attempt 2026-09-03T12:36:40Z · recorded 2026-09-03T12:36:40Z · \
             node/k8rs-worker2"
        ),
        "a result line that sits under the wrong attempt cannot say which one it is: {records:#?}"
    );
    assert!(
        records[3].contains(
            "result · attempt 2026-09-03T12:34:56Z · recorded 2026-09-03T12:34:56Z · \
             deployment/web"
        ),
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
        // **The two fields added on 2026-09-04 wear the same crafted shapes as their neighbours**
        // — a `server:` is a kubeconfig string and a `uid` is an API string, and neither being
        // fed is how a new field arrives outside the strip (NOTES § D29).
        server: "https://k8rs-tests.invalid:41751\u{1b}[2J/evil",
        namespace: Some("pay\u{200b}ments"),
        object: "deployment/web\u{202e}gnp",
        uid: Some("18f0b6ee\u{7}2b0e"),
        consequence: "This deletes deployment/web\u{202e}gnp.",
        kubectl: "kubectl delete deployment/web\nresult · the change was made",
        verb: "DELETE",
        path: "/apis/apps/v1/namespaces/payments/deployments/web\u{7}",
        version: Some("81\u{feff}23"),
        checkable: true,
    };

    let done = perform(
        &crafted,
        stamp,
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
             server https://k8rs-tests.invalid:41751[2J/evil · namespace payments · \
             uid 18f0b6ee2b0e · \
             kubectl: kubectl delete deployment/web result · the change was \
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
        stamp,
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
        stamp,
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
        stamp,
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

/// **Every gap the attempt line can have, named rather than left dangling** (NOTES § D8, and the
/// PM's ruling of 2026-09-04 that an absent field and an empty one record the same way).
///
/// **Four of them on one line**: a cluster-scoped object, a caller that read no `uid`, a call
/// that sent no `resourceVersion`, and a **`server` that is the empty string**. The last is the
/// one whose type says it is always there — so [`Record::attempt_line`] putting it through
/// [`gap`] anyway is only a fact if something feeds it the empty string, and nothing did until
/// this row (my own second pass, 2026-09-04).
#[tokio::test]
async fn a_cluster_scoped_call_with_nothing_to_put_in_three_fields_names_every_gap() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let cordon = Mutation {
        context: "kind-k8rs",
        server: "",
        namespace: None,
        object: "node/k8rs-worker2",
        uid: None,
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
        stamp,
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
             server not known · cluster-wide · uid not read · kubectl: kubectl cordon \
             k8rs-worker2 · call: PATCH /api/v1/nodes/k8rs-worker2 · resourceVersion not sent\n"
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
            stamp,
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
        stamp,
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
// **Each of these names the one parameter it is about, and asserts nothing else about the
// request** (NOTES § D215). An exact-equality assertion on a delete's body reads every future
// field as this box's defect: add todo.md 3692's `propagationPolicy: Background` and the failure
// printed *"the body does not carry it"* beside a body that plainly did, whose obvious repair is
// to paste the new output into the literal — the assertion drift CLAUDE.md § Code phase rules
// forbids (`tester`, 2026-09-04). `dryRun` was the only such parameter when the region was
// written; `fieldValidation` is the second, and it is a separate test rather than a clause bolted
// onto the first for the same reason.

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

/// **Both passes ask the server to reject a field the cluster does not have** — the box's
/// `fieldValidation=Strict`, asserted in the query string the API server reads and not on a
/// struct field.
///
/// **On the change and not only on the check, which is why one test asserts both.** Where an
/// operation sets [`Mutation::checkable`] `false` there is no check pass to have carried it, so a
/// `Strict` that only rode on [`DRY_RUN`] would leave that write validated by nothing: the patch
/// would answer `200 OK`, alter nothing, and be recorded as a successful mutation — invariant 4's
/// *neither record may lie*, broken by the server rather than by us.
///
/// **The delete carries none, and that absence is asserted rather than assumed.** `DeleteParams`
/// has no `field_validation` field at all (`kube-core-4.2.0/src/params.rs:763-791`), which is
/// right rather than missing: a delete sends `DeleteOptions`, not an object, so there is no
/// schema to validate a body against. A reader who greps a delete for `fieldValidation` finds
/// nothing, and this says the absence was decided.
#[test]
fn both_passes_ask_the_server_to_reject_an_unknown_field() {
    assert!(
        patch_uri(DRY_RUN).contains("fieldValidation=Strict"),
        "the check would accept a field the cluster does not have, so it is not the guard the \
         confirmation dialog claims it is"
    );
    assert!(
        patch_uri(FOR_REAL).contains("fieldValidation=Strict"),
        "the real patch would answer 200 OK to a field the cluster does not have, change \
         nothing, and be recorded as a successful mutation"
    );
    let (uri, body) = delete_wire(DRY_RUN);
    assert!(
        !uri.contains("fieldValidation") && !body.contains("fieldValidation"),
        "a delete now carries a fieldValidation this file's doc comment says it cannot — \
         {uri} · {body}"
    );
}

// --- SCALE ---
//
// **The wire is what these tests read, because the box is a claim about it**: the scale
// subresource and not the object, `dryRun=All` on the first pass and not on the second, and a
// body that carries `spec.replicas` and nothing else. A double over `Api` could satisfy every one
// of those while sending something different, so the assertions are made against a socket.
//
// **The stub is this module's own and is not `k8s_tests.rs`'s.** That one is private to `k8s`,
// this file is a child module of `ops`, and D50 refuses the `lib.rs` that would let them share —
// so the twenty lines below are the accepted price of the no-lib-target rule rather than a
// helper somebody forgot about.

/// **What a real `autoscaling/v1 Scale` looks like coming back** — the fields the apiserver puts
/// on a Deployment's scale subresource, and no more
/// (`pkg/registry/apps/deployment/storage/storage.go:370-393`).
fn scale_body(replicas: i32) -> String {
    format!(
        r#"{{"kind":"Scale","apiVersion":"autoscaling/v1","metadata":{{"name":"web",
           "namespace":"payments","uid":"18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11",
           "resourceVersion":"41751","creationTimestamp":"2026-09-01T00:00:00Z"}},
           "spec":{{"replicas":{replicas}}},"status":{{"replicas":{replicas}}}}}"#
    )
}

/// Where a header block ends, or `None` while the request is still arriving.
fn at(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// **How long the body is**, off the one header that says so. Absent means there is none.
fn body_bytes(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .and_then(|(_, value)| value.trim().parse().ok())
        })
        .unwrap_or(0)
}

/// **A stub API server that logs `METHOD target body` and answers what it is told to.**
///
/// **The address is built and not written**, which is not a trick played on
/// `scripts/security-guard.py`: the port is whatever `:0` gave us and the string does not exist
/// until the test runs, so there is no hardcoded loopback URL for the guard to be right about.
async fn stub(
    answer: impl Fn(&str) -> (String, String) + Send + Sync + 'static,
) -> (Client, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port");
    let address = listener.local_addr().expect("the port it picked");
    let asked = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = std::sync::Arc::clone(&asked);
    let answer = std::sync::Arc::new(answer);
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let log = std::sync::Arc::clone(&log);
            let answer = std::sync::Arc::clone(&answer);
            tokio::spawn(async move {
                // One connection carries three requests: hyper keeps it alive, so this reads
                // until the socket closes rather than answering once and giving up.
                let mut pending: Vec<u8> = Vec::new();
                loop {
                    let mut chunk = [0_u8; 4096];
                    match socket.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(read) => pending.extend_from_slice(&chunk[..read]),
                    }
                    // **A PATCH has a body, so a request does not end at the blank line** — the
                    // header says how much more there is, and reading one byte short leaves the
                    // next request's first line glued to this one's.
                    while let Some(end) = at(&pending, b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&pending[..end]).to_string();
                        let length = body_bytes(&head);
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
                        );
                        let asked = asked.trim_end().to_string();
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
    let client = Client::try_from(kube::Config::new(
        format!("http://{address}")
            .parse()
            .expect("an address the kernel just gave us"),
    ))
    .expect("a client over plain http asks the machine for nothing");
    (client, asked)
}

/// The scale `k8rs ops scale deploy/web 3 -n payments` describes.
fn asking(count: i32) -> Scaling<'static> {
    Scaling {
        context: "kind-k8rs",
        server: "https://k8rs-tests.invalid:41751",
        kind: "deployment",
        name: "web",
        namespace: Some("payments"),
        count,
    }
}

/// **What `scale` can be pointed at, and what it says about everything else** —
/// NOTES § Operations' `s` row, over every kind the driver lets through (NOTES § D220 ruling 7).
///
/// **`main.rs`'s `KINDS` is read, not copied**, because the driver accepts all of them for all
/// three verbs on purpose and the refusal is what stops the ones scale does not serve.
///
/// **It used to say that and write six string literals** (`k8s-admin`, 2026-09-04). The two agreed
/// on the day it was written, so a seventh kind would have gone unfed while the sentence above
/// still claimed otherwise — CLAUDE.md's *a derived list asserts it found something* row, which is
/// what the count at the bottom is for.
#[test]
fn scale_takes_the_three_kinds_it_works_on_and_names_them_when_it_refuses_the_rest() {
    let works = [
        ("deployment", "deployments"),
        ("statefulset", "statefulsets"),
        ("replicaset", "replicasets"),
    ];
    let (mut served, mut refused) = (0, 0);
    for kind in &crate::KINDS {
        let kind = kind.singular;
        if let Some((_, plural)) = works.iter().find(|(named, _)| *named == kind) {
            served += 1;
            let resource = scalable(kind).unwrap_or_else(|refusal| panic!("{kind}: {refusal}"));
            println!(
                "{kind} → {}/{} {}",
                resource.group, resource.version, resource.plural
            );
            assert_eq!(
                (
                    resource.group.as_str(),
                    resource.version.as_str(),
                    resource.plural.as_str()
                ),
                ("apps", "v1", *plural),
                "{kind} did not resolve to the apps/v1 resource its own type declares"
            );
        } else {
            refused += 1;
            let refusal = scalable(kind).expect_err("a kind scale does not work on is refused");
            println!("{kind}\n{refusal}");
            assert!(
                refusal.contains(&format!("cannot scale a {kind}")),
                "{kind}: the refusal does not name the kind that was asked for: {refusal:?}"
            );
            // **It names what it *can* do, in plain words** (invariant 14): a reader told only
            // *no* has to go and find the table this sentence is.
            assert!(
                refusal.contains("a deployment, a statefulset and a replicaset"),
                "{kind}: the refusal does not say what scale works on: {refusal:?}"
            );
        }
    }
    // **The derived list says what it found.** An empty `KINDS`, or a renamed entry that no longer
    // matches `works`, passes every assertion above by running none of them — *extracted nothing*
    // and *nothing to extract* print the same line.
    assert_eq!(
        (served, refused),
        (works.len(), crate::KINDS.len() - works.len()),
        "the driver's kind table no longer splits into the three scale serves and the rest"
    );
}

/// **A kind word out of argv is free text and is stripped before it is quoted back**
/// (invariant 9). `scalable` is `pub` and the word reaching it came off a command line.
#[test]
fn a_crafted_kind_cannot_rewrite_the_terminal_on_its_way_into_scales_refusal() {
    let refusal = scalable("pod\u{1b}[2J\u{202e}").expect_err("that is not a kind scale works on");
    println!("{refusal}");
    assert!(
        !refusal.chars().any(crate::k8s::unprintable),
        "a kind word carried an escape into the refusal: {refusal:?}"
    );
    assert!(
        refusal.contains("cannot scale a pod"),
        "the strip ate the readable part of the word too: {refusal:?}"
    );
}

/// **The five relations, in `screens/dialogs.md` § Scale's own sentences** — asserted against that
/// file's words and not against what the function happens to return.
///
/// **The count is on both sides of every one of them**, which is the rule that stops anything
/// depending on the reader remembering the old number.
#[test]
fn the_five_relations_between_what_is_running_and_what_was_asked_for_read_as_the_screen_says() {
    for (running, asked, expected) in [
        (
            2,
            3,
            "This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies.",
        ),
        (
            2,
            5,
            "This starts 3 more copies of your app. Right now: 2 copies. After: 5 copies.",
        ),
        (
            3,
            2,
            "This stops 1 copy of your app. Right now: 3 copies. After: 2 copies.",
        ),
        (
            3,
            0,
            "This stops all 3 copies of your app — nothing will be left running. Right now: 3 \
             copies. After: 0 copies.",
        ),
        // **The unchanged relation describes the request, because the request is made**
        // (`k8s-admin` and a PM ruling, 2026-09-04). Both `PATCH`es go out, the cluster accepts
        // them and the run ends *the change was made* — so a consequence asserting *This makes no
        // change* was the same screen saying both (invariant 14, invariant 4).
        (
            3,
            3,
            "This asks for the count web is already running. Right now: 3 copies. After: 3 \
             copies.",
        ),
    ] {
        let said = consequence("web", running, asked);
        println!("{running} → {asked}\n{said}\n");
        assert_eq!(said, expected, "{running} → {asked}");
    }
}

/// **A count of one is a copy and everything else is copies**, in every place a count is printed —
/// the rule `screens/dialogs.md` writes as *1 more copy* and *3 more copies* and never spells for
/// the two halves of the `Right now:` line.
///
/// **`all 1 copy` is not a sentence anybody says**, so the one relation that would have produced
/// it has its own words. The fact is the same and the grammar is not.
#[test]
fn one_is_a_copy_and_the_sentence_that_would_have_read_all_one_copy_does_not_exist() {
    assert_eq!(copies(1), "1 copy");
    for count in [0, 2, 3, i64::from(i32::MAX)] {
        assert_eq!(copies(count), format!("{count} copies"), "{count}");
    }
    assert_eq!(
        consequence("web", 1, 0),
        "This stops the only copy of your app — nothing will be left running. Right now: 1 copy. \
         After: 0 copies."
    );
    assert_eq!(
        consequence("web", 0, 1),
        "This starts 1 more copy of your app. Right now: 0 copies. After: 1 copy."
    );
    // **Nothing running and nothing asked for is *no change*, not *stops all 0 copies*** — the
    // unchanged relation is decided first, and it has to be.
    assert_eq!(
        consequence("web", 0, 0),
        "This asks for the count web is already running. Right now: 0 copies. After: 0 copies."
    );
}

/// **The widest counts a `replicas` field can hold do not overflow the arithmetic that describes
/// them.** Both ends are `i32`, so the difference is not one — which is why it is taken in `i64`.
#[test]
fn the_two_ends_of_the_replicas_field_are_described_without_overflowing() {
    let said = consequence("web", i32::MIN, i32::MAX);
    println!("{said}");
    assert!(
        said.starts_with("This starts 4294967295 more copies of your app."),
        "{said}"
    );
}

/// **A scale reads the count off the scale subresource and changes only that** — the whole box, on
/// a socket, so what is asserted is what went on the wire (todo.md 3749).
///
/// **Three requests and their order is the claim**: the `GET` that the consequence sentence is
/// built from, the `PATCH` carrying `dryRun=All`, then the `PATCH` that is not a dry run. Every
/// one of them is on `…/deployments/web/scale`; none of them touches the deployment itself, which
/// is what keeps NOTES § D217's *a `422` hands back the object you sent* bounded to a `Scale`.
///
/// **The body is `spec.replicas` and nothing else.** A full-object patch would be a scale that can
/// drift a pod template while claiming to be counting copies.
#[tokio::test]
async fn a_scale_reads_the_count_off_the_subresource_and_patches_only_that() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), scale_body(2))).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = scale(
        &client,
        &asking(3),
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
    )
    .await
    .expect("a deployment in a namespace, with a cluster that answers");

    let requests = sent.lock().expect("the log is never poisoned").clone();
    println!("{}", requests.join("\n"));
    assert_eq!(
        requests,
        vec![
            "GET /apis/apps/v1/namespaces/payments/deployments/web/scale".to_string(),
            "PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale\
             ?&dryRun=All&fieldValidation=Strict {\"spec\":{\"replicas\":3}}"
                .to_string(),
            "PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale\
             ?&fieldValidation=Strict {\"spec\":{\"replicas\":3}}"
                .to_string(),
        ],
        "the scale did not read the subresource, check it, and then change it"
    );
    assert_eq!(
        done,
        Performed {
            outcome: Some(Outcome::Done),
            recorded: true
        }
    );
    assert_eq!(
        done.plainly(),
        "the change was made",
        "a scale that landed does not say so"
    );
    assert!(done.changed(), "a scale that landed is not an exit 0");
}

/// **What the dialog and the audit log were given, off a real answer from a real socket** —
/// the consequence built from the count that came back, the object as the reader knows it, the
/// `uid` the `Scale` carried, and the path the request really took.
#[tokio::test]
async fn what_a_scale_records_is_the_object_it_read_and_the_call_it_made() {
    let (client, _) = stub(|_| ("200 OK".to_string(), scale_body(2))).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = scale(
        &client,
        &asking(3),
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
    )
    .await
    .expect("a deployment in a namespace, with a cluster that answers");
    assert_eq!(done.outcome, Some(Outcome::Done));

    assert_eq!(
        trace.borrow().dialog,
        Some(Dialog {
            object: "deployment/web".to_string(),
            namespace: Some("payments".to_string()),
            consequence: "This starts 1 more copy of your app. Right now: 2 copies. After: 3 \
                          copies."
                .to_string(),
            // **`deployment/web`, never `deploy/web`** (`screens/dialogs.md` § Scale): this line's
            // whole job is teaching a newcomer a command they can read.
            kubectl: "kubectl scale deployment/web --replicas=3 -n payments".to_string(),
        }),
        "the dialog was not given the object, the count it read, or a runnable kubectl line"
    );
    let lines = transcript(&trace);
    let attempt = lines
        .iter()
        .find(|line| line.contains("attempt ·"))
        .expect("the attempt line is written before anything is sent");
    assert_eq!(
        attempt,
        "audit: 2026-09-03T12:34:56Z attempt · deployment/web · context kind-k8rs · server \
         https://k8rs-tests.invalid:41751 · namespace payments · \
         uid 18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11 · kubectl: kubectl scale deployment/web \
         --replicas=3 -n payments · call: PATCH \
         /apis/apps/v1/namespaces/payments/deployments/web/scale · resourceVersion not sent\n",
        "the attempt line does not name the call that was actually made"
    );
}

/// **Cancelling sends the check and nothing after it** — invariant 2 through the real operation
/// and not only through the contract's own double.
#[tokio::test]
async fn a_scale_nobody_confirmed_sends_the_check_and_never_the_change() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), scale_body(2))).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = scale(
        &client,
        &asking(0),
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Cancelled),
    )
    .await
    .expect("a deployment in a namespace, with a cluster that answers");

    let requests = sent.lock().expect("the log is never poisoned").clone();
    println!("{}", requests.join("\n"));
    assert_eq!(
        requests.len(),
        2,
        "a cancelled scale sent the change anyway"
    );
    assert!(
        requests[1].contains("dryRun=All"),
        "the one call after the read was not the check: {:?}",
        requests[1]
    );
    assert_eq!(done.outcome, Some(Outcome::Cancelled));
    assert!(!done.changed(), "a cancelled scale is not an exit 0");
    // **The scale-to-zero wording is what the reader was shown before saying no**, which is the
    // one relation `screens/dialogs.md` prints in full for the headless surface.
    assert_eq!(
        trace
            .borrow()
            .dialog
            .as_ref()
            .map(|dialog| dialog.consequence.clone()),
        Some(
            "This stops all 2 copies of your app — nothing will be left running. Right now: 2 \
             copies. After: 0 copies."
                .to_string()
        )
    );
}

/// **A cluster that will not say how many copies are running is a refusal, and nothing is
/// recorded** — there is no mutation to describe, so there is no attempt line to write.
///
/// **The reason is keyed on the `Fault` and the server's own words travel beside it**
/// (`PRIOR-ART § C1`): a `403` here is the cluster saying no, and it is the one sentence that
/// tells an operator whether to fix their RBAC or their network.
#[tokio::test]
async fn a_scale_that_cannot_read_the_current_count_refuses_and_records_nothing() {
    let (client, sent) = stub(|_| {
        (
            "403 Forbidden".to_string(),
            r#"{"kind":"Status","apiVersion":"v1","status":"Failure","code":403,
               "reason":"Forbidden","message":"deployments.apps \"web\" is forbidden"}"#
                .to_string(),
        )
    })
    .await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let refusal = scale(
        &client,
        &asking(3),
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
    )
    .await
    .expect_err("a cluster that will not answer the read cannot be scaled");

    println!("{refusal}");
    assert!(
        refusal.starts_with(
            "k8rs could not read how many copies of deployment/web are running \
                             right now — the cluster would not allow it"
        ),
        "{refusal:?}"
    );
    assert!(
        refusal.contains("is forbidden"),
        "the server's own explanation was dropped: {refusal:?}"
    );
    assert_eq!(
        sent.lock().expect("the log is never poisoned").len(),
        1,
        "a read that failed was followed by something else"
    );
    assert!(
        transcript(&trace).is_empty(),
        "a mutation that was never described was written into the audit log"
    );
}

/// **A `Scale` that does not say what it is asking for is refused rather than read as zero.**
///
/// *Right now: 0 copies* over an object k8rs could not read invents the one number the whole
/// consequence turns on — and the sentence the reader then agrees to is about a scale that is not
/// the one they typed.
#[tokio::test]
async fn a_scale_the_cluster_gave_no_count_for_is_refused_rather_than_read_as_none() {
    let (client, _) = stub(|_| {
        (
            "200 OK".to_string(),
            r#"{"kind":"Scale","apiVersion":"autoscaling/v1","metadata":{"name":"web"}}"#
                .to_string(),
        )
    })
    .await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let refusal = scale(
        &client,
        &asking(3),
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
    )
    .await
    .expect_err("a Scale with no spec.replicas says nothing about what is running");

    println!("{refusal}");
    assert!(
        refusal.contains("did not say how many it is asking for"),
        "{refusal:?}"
    );
    assert!(
        transcript(&trace).is_empty(),
        "a mutation nobody could describe was written into the audit log"
    );
}

/// **A namespace nobody named is refused inside the operation, before anything is read** —
/// one place, rather than a second copy of which kinds live in a namespace
/// (NOTES § D220 ruling 4).
#[tokio::test]
async fn a_scale_with_no_namespace_is_refused_before_a_single_call_goes_out() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), scale_body(2))).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let nowhere = Scaling {
        namespace: None,
        ..asking(3)
    };

    let refusal = scale(
        &client,
        &nowhere,
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
    )
    .await
    .expect_err("a namespaced object with no namespace is not something to scale");

    println!("{refusal}");
    assert_eq!(
        refusal,
        "k8rs will not scale deployment/web without being told which namespace it is in"
    );
    assert!(
        sent.lock().expect("the log is never poisoned").is_empty(),
        "a scale with nowhere to send anything sent something"
    );
}

/// **A name that would change the address the request goes to is refused where the path is
/// built**, not only where the command line was parsed.
///
/// **Nothing on a command line can reach this** — `k8s::object_name` and `k8s::namespace_name`
/// refuse both in the driver — which is exactly why the guard is here: `scale` is `pub` in a file
/// that freezes at the end of this phase, and Phase 12's console is a caller nobody has written
/// yet. `k8s::owner` keeps the same guard for the same reason one file over.
#[tokio::test]
async fn a_name_that_would_rewrite_the_request_path_is_refused_where_the_path_is_built() {
    for (name, namespace, which) in [
        ("web/../../secrets", "payments", "an object's own name"),
        ("web", "payments/../kube-system", "the name of a namespace"),
        ("", "payments", "an object's own name"),
        ("web", "", "the name of a namespace"),
    ] {
        let (client, sent) = stub(|_| ("200 OK".to_string(), scale_body(2))).await;
        let trace = trace();
        let mut sink = Sink(trace.clone());
        let crafted = Scaling {
            name,
            namespace: Some(namespace),
            ..asking(3)
        };

        let refusal = scale(
            &client,
            &crafted,
            stamp,
            &mut sink,
            shows(&trace),
            asked(&trace, Answer::Confirmed),
        )
        .await
        .expect_err("a name that is not addressable is not something to scale");

        println!("{name:?} in {namespace:?}\n{refusal}");
        assert!(
            refusal.contains(which) && refusal.contains("part of the address"),
            "{name:?} in {namespace:?} was refused for something else: {refusal:?}"
        );
        assert!(
            sent.lock().expect("the log is never poisoned").is_empty(),
            "{name:?} in {namespace:?} reached the cluster anyway"
        );
        assert!(
            transcript(&trace).is_empty(),
            "{name:?} in {namespace:?} was written into the audit log"
        );
    }
}

/// **A count below zero is refused where the request is built, for the same reason the two names
/// above are** (`k8s-admin`, 2026-09-04). `count: i32` is the one field on [`Scaling`] the type
/// does not constrain; `src/main.rs`'s `refuse_count` bounds a command line and Phase 12's console
/// is a caller nobody has written, and `scale` is `pub` in a file that freezes.
///
/// **Unguarded it produced two records that lie before the cluster gets a say** (invariant 4): a
/// consequence reading *This stops 8 copies of your app … After: -5 copies*, a command log reading
/// `--replicas=-5`, an audit line holding both — and only then a `422`.
///
/// **No upper bound is fed, because there is none to feed**: `replicas` is an `i32` and so is this,
/// so `i32::MAX` is a legal ask and is asserted to go through.
#[tokio::test]
async fn a_count_below_zero_is_refused_before_anything_describes_it() {
    for count in [-1, -5, i32::MIN] {
        let (client, sent) = stub(|_| ("200 OK".to_string(), scale_body(3))).await;
        let trace = trace();
        let mut sink = Sink(trace.clone());

        let refusal = scale(
            &client,
            &asking(count),
            stamp,
            &mut sink,
            shows(&trace),
            asked(&trace, Answer::Confirmed),
        )
        .await
        .expect_err("a count Kubernetes cannot hold is not something to scale to");

        println!("{count}\n{refusal}");
        assert_eq!(
            refusal,
            format!(
                "k8rs will not scale deployment/web to {count} copies: the fewest Kubernetes \
                 takes is 0"
            )
        );
        assert!(
            sent.lock().expect("the log is never poisoned").is_empty(),
            "{count} reached the cluster anyway"
        );
        assert!(
            transcript(&trace).is_empty(),
            "{count} was described to somebody and written into the audit log"
        );
    }
    // **The other end is not a refusal**, which is what keeps the guard from being a bound the
    // field does not have.
    let (client, sent) = stub(|_| ("200 OK".to_string(), scale_body(3))).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let largest = scale(
        &client,
        &asking(i32::MAX),
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Cancelled),
    )
    .await
    .expect("the largest count the replicas field holds is a scale k8rs describes");
    println!("{}", largest.plainly());
    assert_eq!(
        largest.outcome,
        Some(Outcome::Cancelled),
        "the largest legal count was refused rather than described and declined"
    );
    assert!(
        !sent.lock().expect("the log is never poisoned").is_empty(),
        "the largest legal count never reached the cluster"
    );
}

/// **A kind `scale` does not work on never reaches a cluster**, whatever the caller does — the
/// driver asks first (NOTES § D220 ruling 7), and the operation refuses again if it is asked
/// anyway.
#[tokio::test]
async fn a_scale_pointed_at_a_kind_it_does_not_work_on_sends_nothing() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), scale_body(2))).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let pod = Scaling {
        kind: "pod",
        ..asking(3)
    };

    let refusal = scale(
        &client,
        &pod,
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Confirmed),
    )
    .await
    .expect_err("a pod is not something k8rs scales");

    println!("{refusal}");
    assert!(refusal.contains("cannot scale a pod"), "{refusal:?}");
    assert!(
        sent.lock().expect("the log is never poisoned").is_empty(),
        "a kind k8rs will not scale was looked up on the cluster anyway"
    );
}

/// **`recorded: false` beside a `Done` is a sentence and never a different exit code**
/// (NOTES § D220 ruling 1, § D214's fourth lie).
///
/// **The change happened.** A `2` here sends a script back to re-run a mutation that already
/// landed, and `restart` and `delete` are not idempotent under that re-run the way a scale is.
#[test]
fn a_change_that_could_not_be_written_down_still_says_it_happened_and_still_exits_zero() {
    let unrecorded = Performed {
        outcome: Some(Outcome::Done),
        recorded: false,
    };
    println!("{}", unrecorded.plainly());
    assert!(
        unrecorded.changed(),
        "a change that happened was reported as one that did not"
    );
    assert_eq!(
        unrecorded.plainly(),
        "the change was made — but k8rs could not write that to the audit log, so the trail of it \
         is short a line"
    );
}

/// **Every other ending says what happened and none of them is a `0`** — one sentence per
/// `Outcome`, and NOTES § D21's *nothing was sent at all* beside them.
#[test]
fn everything_that_is_not_a_change_says_so_and_none_of_it_exits_zero() {
    let refused = Performed {
        outcome: None,
        recorded: false,
    };
    println!("{}", refused.plainly());
    assert!(!refused.changed());
    assert_eq!(
        refused.plainly(),
        "nothing was changed — k8rs could not write this to its audit log first, and every \
         change k8rs makes is written to that log before it is sent"
    );
    for (outcome, expected) in [
        (
            Outcome::Cancelled,
            "nobody confirmed it, so nothing was changed",
        ),
        (
            Outcome::Gone,
            "the object was already gone, so nothing was changed",
        ),
        (
            Outcome::Changed,
            "the object changed while this was open, so nothing was changed",
        ),
        // **The fault is on the sentence even where the server said nothing** (`k8s-admin`,
        // 2026-09-04) — that is the half [`and_said`] cannot supply, and the half a `403` and a
        // `422` were indistinguishable without.
        (
            Outcome::NotSent {
                fault: Fault::Refused,
                said: None,
            },
            "the change was never sent — the cluster would not allow it",
        ),
        (
            Outcome::Failed {
                fault: Fault::Conflict,
                said: None,
            },
            "nothing was changed — the object had already been changed by something else",
        ),
        // **A `404` does not claim the object used to be there.** *no such object any more* over a
        // mistyped name — the commonest way to reach this — sends the reader looking for whoever
        // deleted their deployment (`k8s-admin`, 2026-09-04).
        (
            Outcome::Failed {
                fault: Fault::Gone,
                said: None,
            },
            "nothing was changed — the cluster has no object with that name",
        ),
    ] {
        let performed = Performed {
            outcome: Some(outcome),
            recorded: true,
        };
        println!("{}", performed.plainly());
        assert_eq!(performed.plainly(), expected);
        assert!(
            !performed.changed(),
            "{expected:?} was reported as a cluster that changed"
        );
    }
}

// --- THE AUDIT LOG'S OWN FILE ---
//
// **These tests touch a real disk, and everything above this line does not.** The mode, the
// append and the two refusals are facts about `open(2)` and no double can stand in for them —
// which is also why each one names its own directory and takes it away again in `Drop` rather
// than on its last line (NOTES § D185).
//
// **Nothing here sets an environment variable.** Edition 2024 makes `set_var` `unsafe` and
// `cargo test` runs these in threads of one process, so a test that set `$XDG_STATE_HOME` would
// be racing every other test in the file. [`audit_path`] takes both variables as values for
// exactly this reason, and [`open_log`] takes the path it produced.

/// A scratch directory that takes itself away again — **in `Drop`, so a panicking assertion still
/// cleans up** (NOTES § D185; `k8s_tests.rs`'s `Scratch` is the same shape one file over).
struct Dir(std::path::PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

impl std::ops::Deref for Dir {
    type Target = std::path::Path;

    fn deref(&self) -> &std::path::Path {
        &self.0
    }
}

/// **One empty directory nobody else is in** — the process id and a counter, because two tests
/// sharing a name in `TMPDIR` is one of them deleting the other's file mid-assertion.
fn dir(name: &str) -> Dir {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "k8rs-ops-tests-{}-{}-{name}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path)
        .unwrap_or_else(|e| panic!("{} could not be made: {e}", path.display()));
    Dir(path)
}

/// **Where the log goes under a scratch `$HOME`, and which variable chose it** — the two values
/// every test below opens with, so the tuple is destructured in one place.
fn under(home: &Dir) -> (std::path::PathBuf, Source) {
    audit_path(None, Some(home.as_os_str())).expect("an absolute HOME names a path")
}

/// The mode bits of a path, printed as well as returned so a failure shows both numbers.
fn mode_of(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(path)
        .unwrap_or_else(|e| panic!("{} has no metadata: {e}", path.display()))
        .permissions()
        .mode()
        & 0o7777;
    println!("{} is {mode:04o}", path.display());
    mode
}

/// **A result line says when it was written down, and not only which attempt it belongs to**
/// (todo.md 3696, NOTES § D214's closing paragraph).
///
/// **The clock moves between the two readings, which is the only way this can be shown.** Every
/// other test in this file runs on [`stamp`], a fixed clock, where the two fields are the same
/// string and a record that read the clock once looks identical. Here `perform` is handed a clock
/// that steps a minute each time it is asked, so the attempt line carries the first reading and
/// the result line carries both — in that order, which is what says the two are not swapped.
///
/// **The gap this makes readable is the whole mutation's and not the call's**, which is why the
/// field is `recorded`: the confirmation happens between the two readings, and the test steps the
/// clock inside `ask` to keep that true rather than incidental.
#[tokio::test]
async fn a_result_says_when_it_was_recorded_and_not_only_when_it_was_attempted() {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let ticks = std::cell::Cell::new(0i64);
    let clock = || {
        let tick = ticks.get();
        ticks.set(tick + 1);
        Timestamp::from_second(1_788_438_896 + tick * 60).expect("inside jiff's range")
    };

    let done = perform(
        &scaling(),
        &clock,
        &mut sink,
        shows(&trace),
        |checked: Checked<()>| {
            // The reader spends a minute on the dialog. It is between the two readings on
            // purpose: a field called `took` would be charging the cluster for it.
            clock();
            trace.borrow_mut().steps.push("asked".to_string());
            let _ = checked.verdict();
            std::future::ready(Answer::Confirmed)
        },
        works(&trace),
    )
    .await;

    assert_eq!(done.outcome, Some(Outcome::Done));
    let records: Vec<String> = transcript(&trace)
        .into_iter()
        .filter(|step| step.starts_with("audit: "))
        .collect();
    assert!(
        records[0].starts_with("audit: 2026-09-03T12:34:56Z attempt · "),
        "the attempt line is not stamped with the clock's first reading: {records:#?}"
    );
    assert!(
        records[1].starts_with(
            "audit: result · attempt 2026-09-03T12:34:56Z · recorded 2026-09-03T12:36:56Z · "
        ),
        "the result line cannot say when it was written down, or it is stamped with the attempt's \
         reading twice — so how long the mutation took is unanswerable: {records:#?}"
    );
}

/// **The log is created readable and writable by its owner and by nobody else** (CLAUDE.md
/// § Security gate).
///
/// **The mode is asserted off the file and not off the call**, and it is asserted on a file this
/// run *created* — which is what makes a dropped `.mode()` a red test rather than something a
/// later `chmod` would have covered up. There is no later `chmod`: `open_log` sets the mode at
/// creation and nowhere else.
///
/// **And so is the state directory it sits in** (`k8s-admin`, 2026-09-04): `create_dir_all`
/// carries no mode and takes `0777 & ~umask`, so on a machine with `umask 0` — a CI runner, some
/// daemons — `~/.local/state/k8rs/` came out `drwxrwxrwx`, which is the precondition for planting
/// the FIFO or the symlink the test below refuses. `0700` is [`STATE_DIR_ONLY`] and the XDG base
/// directory specification's own recommendation.
///
/// **It asserts the mode of every directory the run made, not only the last one**, because
/// `DirBuilder`'s mode applies to each — a `.mode()` moved onto a non-recursive call would leave
/// `~/.local` and `~/.local/state` at the umask's mercy and this would still be green.
#[test]
fn the_audit_log_and_the_directory_it_sits_in_are_readable_by_nobody_but_their_owner() {
    let home = dir("owner-only");
    let (path, source) = under(&home);
    assert_eq!(
        path,
        home.join(".local/state/k8rs/audit.log"),
        "the log is not where NOTES § D21 says it is"
    );
    assert_eq!(source, Source::Home, "$HOME did not choose this path");
    assert!(
        !path.exists(),
        "the log was already there, so creating it below proves nothing"
    );

    let (log, notes) = open_log(&path, source).expect("a writable HOME holds an audit log");
    drop(log);

    assert!(path.is_file(), "the audit log was not created");
    assert_eq!(
        notes,
        Vec::<String>::new(),
        "a log k8rs just made at its own mode was complained about: {notes:#?}"
    );
    assert_eq!(
        mode_of(&path),
        0o600,
        "the audit log k8rs created can be read by somebody other than its owner"
    );
    for made in [".local", ".local/state", ".local/state/k8rs"] {
        assert_eq!(
            mode_of(&home.join(made)),
            0o700,
            "{made} can be read or written by somebody other than its owner — which is where a \
             pipe or a symlink gets planted where the audit log goes"
        );
    }
}

/// **Append-only: a second handle adds to the log and neither overwrites what the other wrote**
/// (CLAUDE.md § Security gate, `write_line`).
///
/// **Two handles rather than one, because one handle appends whatever flags it was opened with.**
/// A `write(true)` open starts at offset 0, so the second handle would sit on top of the first
/// one's record and the first record would be gone — which is what this asserts is not what
/// happens. It is also the flag `write_line`'s no-interleaving claim rests on.
#[test]
fn two_handles_on_one_audit_log_both_append_and_neither_overwrites_the_other() {
    let home = dir("append-only");
    let (path, source) = under(&home);

    let (mut first, _) = open_log(&path, source).expect("a writable HOME holds an audit log");
    write_line(
        &mut first,
        "first · a long line so a second one starting at nought would eat it\n",
    )
    .expect("the log takes a line");

    let (mut second, _) = open_log(&path, source).expect("the log opens a second time");
    write_line(&mut second, "second\n").expect("the log takes a second line");
    // Written through the *first* handle after the second one has already appended: with
    // `O_APPEND` this lands at the end, and without it, at wherever the first handle's own
    // offset happened to be.
    write_line(&mut first, "third\n").expect("the first handle still appends");

    // **Read lossily and not as UTF-8**, so a handle that started at offset nought and cut a
    // record in half fails the assertion below rather than the read — the point of this test is
    // *which lines are there*, and a decode error says nothing about that.
    let bytes = std::fs::read(&path).expect("the log reads back");
    let written = String::from_utf8_lossy(&bytes);
    println!("--- the log ---\n{written}");
    assert_eq!(
        written.lines().collect::<Vec<_>>(),
        vec![
            "first · a long line so a second one starting at nought would eat it",
            "second",
            "third"
        ],
        "a record was overwritten or landed somewhere other than the end of the log"
    );
}

/// **A state directory that cannot hold the log is a sentence, not a crash and not a panic**
/// (NOTES § D21 — k8rs says so and carries on read-only).
///
/// **Both refusals, because they are two different things to fix.** A path whose parent is a
/// regular file cannot be made a directory at all; a path whose directory made fine and whose
/// *name* the system will not take is an `open` that failed. Neither depends on being a
/// non-root user, which a `chmod 0500` test would — the second row is a name past `NAME_MAX`,
/// which the kernel refuses for root as readily as for anybody.
///
/// **The sentences are asserted whole**, since what this box owes the reader is D21's ruling in
/// words: what k8rs will not do now, and what still works.
///
/// **Both name which variable chose the path** (`k8s-admin`, 2026-09-04). A refusal that gives a
/// path and not its provenance sends a reader who set `$XDG_STATE_HOME` to look at their home
/// directory, and one who did not to look at a variable they never set.
#[test]
fn a_state_directory_that_cannot_hold_the_log_is_a_sentence_and_not_a_crash() {
    let home = dir("no-place");
    std::fs::write(home.join(".local"), b"not a directory\n").expect("a file where a dir goes");
    let (blocked, source) = under(&home);
    let refusal = open_log(&blocked, source).expect_err("a file cannot hold a directory");
    println!("--- no place ---\n{refusal}");
    assert!(
        refusal.starts_with(&format!(
            "k8rs could not make a place for its audit log at {} (under your home directory): ",
            blocked.display()
        )),
        "the refusal does not say what k8rs could not do, where, or which variable put it \
         there: {refusal}"
    );

    // A name longer than `NAME_MAX`: the directory above it is made, and the `open` inside it is
    // the failure. `$XDG_STATE_HOME` is the caller here, so the other provenance clause is the
    // one under test.
    let other = dir("no-such-name");
    let (taken, _) = audit_path(Some(other.as_os_str()), None).expect("an absolute path");
    let taken = taken.with_file_name("a".repeat(300));
    let refused =
        open_log(&taken, Source::StateHome).expect_err("no filesystem takes a 300-byte name");
    println!("--- not opened ---\n{refused}");
    assert!(
        refused.starts_with(&format!(
            "k8rs could not open its audit log at {} (from $XDG_STATE_HOME): ",
            taken.display()
        )),
        "the refusal does not say what k8rs could not do, where, or which variable put it \
         there: {refused}"
    );

    for refusal in [&refusal, &refused] {
        assert!(
            refusal.ends_with(
                " — every change k8rs makes is written to that log before it is sent, so k8rs \
                 will not change anything until that is fixed, and reading your cluster still \
                 works"
            ),
            "the refusal does not say what it costs or what still works (NOTES § D21): {refusal}"
        );
    }
}

/// **Anything at the log's path that is not an ordinary file is refused, and the FIFO is why**
/// (`tester` and `k8s-admin`, 2026-09-04; NOTES § D21).
///
/// **Measured on the built binary before this check existed**: `mkfifo` at the log path, then
/// `timeout 6 k8rs ops scale deploy/web 3 -n payments` — **exit 124, no output at all**.
/// `open(O_WRONLY|O_APPEND|O_CREAT)` on a FIFO with no reader blocks forever, so D21's three
/// endings — it opens; it does not and k8rs says so and reads on — gained a fourth nobody ruled
/// on: says nothing, does nothing, forever. *Pipe my audit trail into a collector* is a thing an
/// operator tries, and the hang arrives later, when the reader dies.
///
/// **Four shapes and not just the FIFO**, because the predicate is *not a regular file* and each
/// of them is a different reason a reader could be looking at that path:
///
/// - a **socket**, which stands in for the FIFO here — see below;
/// - a **directory**, which used to come back as the system's *Is a directory*;
/// - a **symlink to an ordinary file**, which `metadata` would have followed and appended
///   through — the door `O_NOFOLLOW` half-closes and `symlink_metadata` closes;
/// - a **dangling symlink**, which is the one a following `stat` calls `NotFound`: `open` would
///   create the file wherever it points, so what gets planted there between the two calls is what
///   k8rs appends to.
///
/// **The FIFO itself is deliberately not one of the rows, and the reason is what would happen if
/// this check were deleted.** A `open(O_WRONLY)` on one blocks, so the row would not fail — it
/// would **hang `cargo test` forever**, and a suite that hangs on a regression is worse than one
/// that has no row at all. The socket is the same branch, arrives with no `mkfifo` to spawn
/// (`std::os::unix::net`), and fails *loudly* with the check removed: `open` on a socket is
/// `ENXIO` and the sentence comes back as the system's rather than as this one. The FIFO is
/// measured where a hang is visible and survivable — on the built binary, under `timeout`.
///
/// **A character device is not a row and the reason is the machine, not the design**: `/dev/null`
/// is not somewhere a `$HOME` can be made to point without root. The predicate is
/// `file_type().is_file()`, so all of these and a device are one branch, not five.
#[test]
fn nothing_that_is_not_an_ordinary_file_is_written_to_as_an_audit_log() {
    for (what, plant) in [
        (
            "a socket",
            &(|path: &std::path::Path| {
                // Binding creates the socket file and dropping the listener leaves it there,
                // which is the whole of what this row needs. `sun_path` is 108 bytes, and the
                // scratch path under `std::env::temp_dir()` is well inside it — a `TMPDIR` long
                // enough to break that fails this `expect` loudly rather than skipping.
                let bound = std::os::unix::net::UnixListener::bind(path);
                bound.unwrap_or_else(|e| panic!("a socket at {}: {e}", path.display()));
            }) as &dyn Fn(&std::path::Path),
        ),
        ("a directory", &|path: &std::path::Path| {
            std::fs::create_dir(path).expect("a directory")
        }),
        ("a link to a file", &|path: &std::path::Path| {
            let target = path.with_file_name("somewhere-else.log");
            std::fs::write(&target, b"somebody else's file\n").expect("a file to point at");
            std::os::unix::fs::symlink(&target, path).expect("a symlink");
        }),
        ("a link to nothing", &|path: &std::path::Path| {
            std::os::unix::fs::symlink(path.with_file_name("not-there.log"), path)
                .expect("a dangling symlink");
        }),
    ] {
        let home = dir("not-a-file");
        let (path, source) = under(&home);
        std::fs::create_dir_all(path.parent().expect("the log has a parent"))
            .expect("the state directory");
        plant(&path);

        let Err(refusal) = open_log(&path, source) else {
            panic!("k8rs opened {what} at the audit log's path and would write a record into it");
        };
        println!("--- {what} ---\n{refusal}");
        assert_eq!(
            refusal,
            format!(
                "there is something at {} (under your home directory) that is not an ordinary \
                 file — a pipe, a device, a directory or a link — and k8rs will not write its \
                 audit log into it — every change k8rs makes is written to that log before it is \
                 sent, so k8rs will not change anything until that is fixed, and reading your \
                 cluster still works",
                path.display()
            ),
            "{what} at the log's path was not refused in D21's own words"
        );
    }
}

/// **A log other people can write to is said out loud, and still written to** (`k8s-admin`, and
/// the PM re-measured it, 2026-09-04).
///
/// **Measured on the built binary before this check existed**: `chmod 0666` on the log, then
/// `k8rs ops scale deploy/web 3 -n payments` — the run went through and said **nothing**, and
/// `ls -l` still read `-rw-rw-rw-`. k8rs was appending its audit trail to a file anybody on the
/// machine can forge or truncate, which is invariant 4's whole subject.
///
/// **A note and not a refusal, and readable is not writable.** Group-*readable* is the
/// log-collector case and stays quiet — narrowing it would be a `chmod` `open_log` does not
/// own. Group- or other-*writable* is the trail's integrity gone.
///
/// **The bits are fed one at a time**, because `0o022` written as `0o002` or `0o020` still
/// catches `0666` and would miss half of what it is for.
#[test]
fn a_log_other_people_can_write_to_is_complained_about_and_still_written_to() {
    use std::os::unix::fs::PermissionsExt;

    for (mode, complained) in [
        (0o600, false),
        (0o640, false),
        (0o644, false),
        (0o620, true),
        (0o602, true),
        (0o666, true),
    ] {
        let home = dir("widened");
        let (path, source) = under(&home);
        std::fs::create_dir_all(path.parent().expect("the log has a parent"))
            .expect("the state directory");
        std::fs::write(&path, b"somebody's earlier record\n").expect("a log already there");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            .expect("the mode is ours to set on our own scratch file");

        let (log, notes) = open_log(&path, source).expect("a widened log still opens");
        drop(log);
        println!("--- {mode:04o} ---\n{notes:#?}");

        if complained {
            assert_eq!(
                notes,
                vec![format!(
                    "the audit log at {} (under your home directory) can be written to by other \
                     people on this machine (it is {mode:04o}), so what is already in it may not \
                     be what k8rs wrote — k8rs is still recording to it, and `chmod 600 {}` \
                     makes it yours alone",
                    path.display(),
                    path.display()
                )],
                "a log at {mode:04o} was appended to without a word, or without saying what to do"
            );
        } else {
            assert_eq!(
                notes,
                Vec::<String>::new(),
                "a log at {mode:04o} was complained about — readable by a collector is not \
                 writable by a forger"
            );
        }
        assert_eq!(
            mode_of(&path),
            mode,
            "open_log narrowed a mode its owner chose — that is the chmod it does not own"
        );
    }
}

/// **Which user this process is, answered off the machine rather than assumed** ([`us`]).
///
/// **There is no `getuid` in `std`** and none of the twelve approved crates exposes one
/// (invariant 10), so [`us`] reads `/proc/self`. That is one `stat` and no dependency — and it is
/// also a claim about what procfs owns, which is exactly the kind that gets reasoned about
/// instead of measured (CLAUDE.md). So the expectation here is not a number: it is the owner of a
/// file this test has just created, which is by construction the user this process is.
///
/// **`/proc/self` is a symlink owned by root** — `stat` without `-L` says uid 0 — so a
/// `symlink_metadata` here would compare a file's owner against root's and be green only when run
/// as root. `std::fs::metadata` follows it, and this is the assertion that says so.
#[test]
fn the_user_this_process_is_comes_off_the_machine_and_not_off_an_assumption() {
    use std::os::unix::fs::MetadataExt;

    let home = dir("who-are-we");
    let mine = home.join("a-file-this-test-made");
    std::fs::write(&mine, b"ours\n").expect("a file in our own scratch directory");
    let owner = std::fs::metadata(&mine).expect("it has metadata").uid();

    // **`None` is the right answer where there is no procfs and the wrong one where there is**,
    // so the expectation is conditional rather than the test being skipped: a skipped row and a
    // passing row print the same thing (CLAUDE.md § *a derived list asserts it found something*).
    // The release targets include `*-apple-darwin`, which has no `/proc`.
    let procfs = std::path::Path::new("/proc/self").is_dir();
    let found = us();
    println!(
        "/proc/self is a directory: {procfs}; it says {found:?}; a file we just made is \
              owned by {owner}"
    );
    assert_eq!(
        found,
        procfs.then_some(owner),
        "the process cannot say which user it is, so the ownership half of `widened` is checking \
         nothing on a machine that does have procfs — or it claimed to know on one that has no \
         procfs at all"
    );
}

/// **Where the log goes, decided from the two variables and nothing else** (NOTES § D21, and the
/// XDG base directory specification for the fallback and the ignore).
///
/// **Every shape the environment actually hands it**, and the relative and empty rows are the two
/// the specification says to *ignore* rather than join (`k8rs/audit.log` under whatever directory
/// a shell was in is an audit trail with no one place to look). The last row is a machine with
/// neither variable, which is a sentence rather than a path.
/// **And it says which of the two answered**, since every sentence about the path carries that
/// clause ([`Source`]).
#[test]
fn the_log_follows_xdg_state_home_and_ignores_one_that_names_no_directory() {
    for (state_home, home, expected, source) in [
        (
            Some("/var/lib/k8rs-state"),
            Some("/home/ops"),
            Some("/var/lib/k8rs-state/k8rs/audit.log"),
            Some(Source::StateHome),
        ),
        (
            None,
            Some("/home/ops"),
            Some("/home/ops/.local/state/k8rs/audit.log"),
            Some(Source::Home),
        ),
        // Set but empty, and set but relative: ignored, and the fallback answers.
        (
            Some(""),
            Some("/home/ops"),
            Some("/home/ops/.local/state/k8rs/audit.log"),
            Some(Source::Home),
        ),
        (
            Some("state"),
            Some("/home/ops"),
            Some("/home/ops/.local/state/k8rs/audit.log"),
            Some(Source::Home),
        ),
        // A relative HOME is ignored for the same reason, and then there is nowhere left.
        (None, Some("ops"), None, None),
        (Some(""), Some(""), None, None),
        (None, None, None, None),
    ] {
        let found = audit_path(state_home.map(OsStr::new), home.map(OsStr::new));
        println!("{state_home:?} + {home:?} -> {found:?}");
        assert_eq!(
            found
                .as_ref()
                .map(|(path, _)| path.to_string_lossy())
                .as_deref(),
            expected,
            "XDG_STATE_HOME={state_home:?} HOME={home:?} put the audit log somewhere else"
        );
        assert_eq!(
            found.map(|(_, source)| source),
            source,
            "XDG_STATE_HOME={state_home:?} HOME={home:?} credits the wrong variable, so every \
             sentence about that path sends the reader to the wrong one"
        );
    }
}

/// **An `$XDG_STATE_HOME` that was set and not used is said out loud** (`k8s-admin`, and the PM
/// re-measured it, 2026-09-04).
///
/// **Measured on the built binary before this note existed**: `XDG_STATE_HOME=oops` with a good
/// `$HOME` put the trail in `$HOME/.local/state/k8rs/audit.log` and printed **nothing** about it.
/// An operator who set the variable to keep the trail on an encrypted volume never learned it had
/// been ignored. Ignoring a relative value is the base directory specification's own rule and
/// stays; not saying so was the defect.
///
/// **An empty value is not "ignored", it is "unset"** — the specification defines empty as *use
/// the default* — so it gets no note, and that is the row that separates the two.
#[test]
fn an_xdg_state_home_that_was_set_and_not_used_is_reported_and_an_empty_one_is_not() {
    let path = Path::new("/home/ops/.local/state/k8rs/audit.log");
    for (state_home, source, expected) in [
        (
            Some("oops"),
            Source::Home,
            Some(
                "k8rs is not keeping its audit log where $XDG_STATE_HOME points: oops is not a \
                 full path starting at /, so the log is at \
                 /home/ops/.local/state/k8rs/audit.log instead, under your home directory",
            ),
        ),
        // A control character in the value reaches a terminal here like any other free text
        // (invariant 9), and this is the only sentence that echoes the value back.
        (
            Some("oo\u{1b}[2Jps"),
            Source::Home,
            Some(
                "k8rs is not keeping its audit log where $XDG_STATE_HOME points: oo[2Jps is not \
                 a full path starting at /, so the log is at \
                 /home/ops/.local/state/k8rs/audit.log instead, under your home directory",
            ),
        ),
        (Some(""), Source::Home, None),
        (None, Source::Home, None),
        (Some("/var/lib/k8rs-state"), Source::StateHome, None),
    ] {
        let note = ignored(state_home.map(OsStr::new), source, path);
        println!("{state_home:?} + {source:?} -> {note:?}");
        assert_eq!(
            note.as_deref(),
            expected,
            "XDG_STATE_HOME={state_home:?} with {source:?} chosen was reported wrongly"
        );
    }
}

/// **A machine with neither variable is told so, and told what it costs** (NOTES § D21).
///
/// It is the one refusal that has no path to name, which is why it needs its own words rather
/// than an empty gap where a path would go.
///
/// **The sentence comes out of the product and the expectation is written from D21**, which is
/// the whole reason [`nowhere_to_keep`] is a function: this test first built the sentence itself
/// out of [`without`] and a hand-typed clause, so it compared a copy with a copy and would have
/// stayed green through any rewording of the arm it is about (my own second pass, 2026-09-04).
///
/// **It says what each variable actually was, because the one sentence it used to print was false
/// about one of them** (`k8s-admin`, and the PM re-measured it, 2026-09-04). Measured on the
/// built binary: `env -u HOME XDG_STATE_HOME=relative-dir k8rs ops scale deploy/web 3 -n
/// payments` answered *neither HOME nor XDG_STATE_HOME names a directory it can start from* —
/// and `$XDG_STATE_HOME` **is** set and **does** name a directory. A reader checks that in one
/// command, finds it set, and stops trusting the message; that is NOTES § D214's class in the box
/// built after it.
///
/// **Three states per variable and every combination that can reach here**, since *not set*,
/// *set to nothing* and *set to something relative* are three different things to go and fix.
#[test]
fn a_machine_with_nowhere_to_keep_the_log_is_told_what_that_costs_and_why() {
    let tail = " — every change k8rs makes is written to that log before it is sent, so k8rs \
                will not change anything until that is fixed, and reading your cluster still works";
    for (state_home, home, why) in [
        (
            None,
            None,
            "$XDG_STATE_HOME is not set, and $HOME is not set",
        ),
        (
            Some(""),
            Some(""),
            "$XDG_STATE_HOME is set to nothing, and $HOME is set to nothing",
        ),
        (
            Some("relative-dir"),
            None,
            "$XDG_STATE_HOME is relative-dir, which is not a full path starting at /, and \
             $HOME is not set",
        ),
        (
            None,
            Some("ops"),
            "$XDG_STATE_HOME is not set, and $HOME is ops, which is not a full path starting at /",
        ),
        // The value is free text out of the environment on its way to a terminal (invariant 9).
        (
            Some("rel\u{1b}[2Jative"),
            Some(""),
            "$XDG_STATE_HOME is rel[2Jative, which is not a full path starting at /, and \
             $HOME is set to nothing",
        ),
    ] {
        let sentence = nowhere_to_keep(state_home.map(OsStr::new), home.map(OsStr::new));
        println!("--- {state_home:?} + {home:?} ---\n{sentence}");
        assert_eq!(
            sentence,
            format!("k8rs has nowhere to keep its audit log: {why}{tail}"),
            "XDG_STATE_HOME={state_home:?} HOME={home:?} was described wrongly, or the sentence \
             does not say what it costs"
        );
    }
}

/// **A path out of the environment cannot rewrite the terminal on its way into a refusal**
/// (invariant 9, NOTES § D154).
///
/// **`$XDG_STATE_HOME` is free text like any other**, and it reaches a terminal the moment
/// something is wrong with it. An `ESC` in it is the same cursor-rewrite a crafted pod name is,
/// and it is the one string in this region that comes from outside the program.
///
/// **The assertion is the literal sentence and not a predicate over it** — asserting
/// `!refusal.chars().any(unprintable)` can only fail when the strip is not called at all, which
/// is the shape NOTES § D154 exists about.
#[test]
fn a_path_out_of_the_environment_cannot_rewrite_the_terminal_on_its_way_into_a_refusal() {
    let scratch = dir("crafted");
    // **A control character is a legal byte in a filename**, so a crafted `$XDG_STATE_HOME` that
    // merely *contains* one is made without complaint and never reaches a sentence. What makes
    // this reach one is the same file-where-a-directory-goes as above, wearing the crafted name.
    let crafted = scratch.join("blocked\u{1b}[2Jgone\u{202e}drow");
    std::fs::write(&crafted, b"not a directory\n").expect("a file where a dir goes");
    let (path, source) =
        audit_path(None, Some(crafted.as_os_str())).expect("an absolute HOME names a path");

    let refusal =
        open_log(&path, source).expect_err("a directory cannot be made under a regular file");
    println!("--- crafted ---\n{refusal}");
    assert!(
        refusal.starts_with(&format!(
            "k8rs could not make a place for its audit log at {}/blocked[2Jgonedrow/.local/state/\
             k8rs/audit.log (under your home directory): ",
            scratch.display()
        )),
        "the path was put on the terminal with what was in it: {refusal:?}"
    );
}

/// **How long a line and a record can actually get, measured rather than summed in a doc**
/// (CLAUDE.md § *a claim reasoned from a definition instead of measured*).
///
/// **[`write_line`]'s whole atomicity argument rests on this number** — one `write(2)` takes a
/// buffer this size on a regular file, so the loop `write_all` could break in never runs twice —
/// and the number was written into that doc three times by three people, each summing the caps by
/// hand. It moved again on 2026-09-04 when `server` and `uid` were added, which is exactly how a
/// summed number goes stale.
///
/// **Every field is fed something far past its cap**, so what comes out is
/// `k8s::text`'s bound plus its own marker and nothing shorter. The server's message goes through
/// the real `k8s::said`, which is where that one is bounded.
///
/// **The assertion is a ceiling and not the measured number**, because a cap moving by a byte is
/// not a defect and a cap moving by an order of magnitude is. 32 KiB is the ceiling the doc
/// quotes; the measured figures are printed, and the doc's job is to repeat what this prints.
#[test]
fn a_line_and_a_record_have_a_measured_ceiling_and_it_is_far_under_one_write() {
    let long = "x".repeat(100_000);
    let huge = Mutation {
        context: &long,
        server: &long,
        namespace: Some(&long),
        object: &long,
        uid: Some(&long),
        consequence: &long,
        kubectl: &long,
        verb: &long,
        path: &long,
        version: Some(&long),
        checkable: true,
    };
    let record = Record::of(&huge);
    let attempt = record.attempt_line(stamp());
    // The longest tail a result line can have: a failure the server answered, whose message is
    // bounded by `k8s::said` and nothing else.
    let failed = Outcome::Failed {
        fault: Fault::Rejected,
        said: said(&refusal(&long, &long, 422)),
    };
    let result = record.result_line(stamp(), stamp(), &failed);

    let (attempt_bytes, result_bytes) = (attempt.len(), result.len());
    println!(
        "longest attempt line {attempt_bytes} bytes · longest result line {result_bytes} bytes · \
         longest record {} bytes",
        attempt_bytes + result_bytes
    );
    assert!(
        attempt_bytes.max(result_bytes) < 32 * 1024,
        "a single audit line can reach {} bytes, which is no longer three orders below where the \
         kernel short-writes a regular file — write_line's one-write claim has to be re-measured",
        attempt_bytes.max(result_bytes)
    );
    // Every field is over its cap, so every one of them must show the marker — a field that
    // slipped past `k8s::text` would make the number above meaningless rather than merely wrong.
    assert_eq!(
        attempt.matches("(shortened by k8rs)").count(),
        9,
        "a field on the attempt line is not bounded: {attempt}"
    );
}

/// **One whole mutation, into the file this box opens** — the two records on a real disk, at the
/// real mode, in the real order (invariant 4, NOTES § D8, § D21).
///
/// **Every other test in this file writes into a double**, which is right for the ordering claims
/// and proves nothing about the destination. This is the only place [`perform`] meets
/// [`open_log`], and it is the shape `scale` (todo.md 3718) wires: a `File` handed straight in as
/// `audit`. What it catches that neither half catches alone is a record that cannot be written to
/// a real file at all.
///
/// **The expected text is [`ATTEMPT`] and [`RESULT`] without the double's prefix**, so this test
/// and the transcript tests cannot come to disagree about what a record says.
#[tokio::test]
async fn one_whole_mutation_lands_in_the_file_this_box_opens_and_nothing_else_does() {
    let home = dir("end-to-end");
    let (path, source) = under(&home);
    let (mut log, _) = open_log(&path, source).expect("a writable HOME holds an audit log");

    let done = perform(
        &scaling(),
        stamp,
        &mut log,
        |_: &Shown<'_>| {},
        |_: Checked<()>| std::future::ready(Answer::Confirmed),
        |_| std::future::ready(Ok::<(), kube::Error>(())),
    )
    .await;
    drop(log);

    assert_eq!(
        done,
        Performed {
            outcome: Some(Outcome::Done),
            recorded: true
        },
        "a real file would not take the record a double takes"
    );
    let written = std::fs::read_to_string(&path).expect("the log reads back");
    println!("--- {} ---\n{written}", path.display());
    let head = |line: &str| {
        line.strip_prefix("audit: ")
            .expect("the transcript's records are prefixed")
            .to_string()
    };
    assert_eq!(
        written,
        format!(
            "{}{} · {CHECKED_FIRST} · the change was made\n",
            head(ATTEMPT),
            head(RESULT)
        ),
        "what reached the disk is not the two records the contract's own tests read"
    );
    assert_eq!(
        mode_of(&path),
        0o600,
        "the log holding a record of what somebody changed can be read by somebody else"
    );
}
