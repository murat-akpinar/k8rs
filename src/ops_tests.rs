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
    /// **What [`Checked::asks`] carried** — the name invariant 2 wants typed back, or `None` for
    /// an operation a press confirms. Recorded because it is the fact each operation *chose*
    /// (NOTES § D225 ruling 2), and nothing else in a transcript can see it.
    asks: Option<Option<String>>,
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
        // **A press, because a scale is not a delete** — invariant 2 raises the bar to the typed
        // name for `delete` and `drain` and nothing else (NOTES § D225 ruling 2).
        confirm: Confirm::Press,
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

/// **One dialog, recording what it was shown and ending the way `answer` says** — the shape all
/// three helpers below are built from.
///
/// **The answer is built *from the `Checked` this dialog was handed*, and it has to be**
/// (NOTES § D225 ruling 2): [`Agreed`] carries [`perform`]'s ticket, so a confirmation forged
/// beside the call — which is what these tests did until 2026-09-04 — is a confirmation for no
/// mutation at all and would be refused.
fn answering<'a, R>(
    trace: &'a Shared,
    answer: impl FnOnce(&Checked<R>) -> Answer + 'a,
) -> impl FnOnce(Checked<R>) -> std::future::Ready<Answer> + 'a {
    move |checked| {
        {
            let mut trace = trace.borrow_mut();
            trace.steps.push("asked".to_string());
            trace.verdict = Some(checked.verdict().to_string());
            trace.asks = Some(checked.asks().map(str::to_string));
        }
        std::future::ready(answer(&checked))
    }
}

/// A dialog that records the verdict it was shown and ends with `answer` — the three endings that
/// are not a confirmation, which stay freely constructible ([`Answer`]).
fn asked<R>(
    trace: &Shared,
    answer: Answer,
) -> impl FnOnce(Checked<R>) -> std::future::Ready<Answer> + '_ {
    answering(trace, move |_| answer)
}

/// **A dialog that pressed** — the confirmation for a [`Confirm::Press`] mutation, built the only
/// way one can be built.
fn confirms<'a, R: 'a>(
    trace: &'a Shared,
) -> impl FnOnce(Checked<R>) -> std::future::Ready<Answer> + 'a {
    answering(trace, Checked::pressed)
}

/// **One of the four endings, named** — the table shape the two tests over *every* ending need,
/// now that a confirmation is built from the [`Checked`] and so cannot be a value sitting in a
/// table beside the case it belongs to ([`Agreed`]).
///
/// **One closure and not a `match` over four**, because four arms would be four `impl` types.
fn ends<'a, R>(
    trace: &'a Shared,
    which: &'static str,
) -> impl FnOnce(Checked<R>) -> std::future::Ready<Answer> + 'a {
    answering(trace, move |checked| match which {
        "confirmed" => checked.pressed(),
        "cancelled" => Answer::Cancelled,
        "gone" => Answer::Gone,
        "changed" => Answer::Changed,
        other => panic!("{other} is not one of the four endings a dialog has"),
    })
}

/// **A dialog that typed `name`** — the confirmation for a [`Confirm::Type`] mutation.
fn types<'a, R>(
    trace: &'a Shared,
    name: &'a str,
) -> impl FnOnce(Checked<R>) -> std::future::Ready<Answer> + 'a {
    answering(trace, move |checked| checked.typed(name))
}

/// **[`perform`] with the landing a `PATCH` has** — [`finished`], which is what [`scale`] and
/// [`restart`] pass and what every test in this file wants but the two that are about the other
/// arm.
///
/// **A wrapper and not a seventh argument at twenty-eight call sites**, and it hides nothing that
/// is under test: `Landing::Started` is reached through [`delete`] against a stub that answers
/// with the object, and through the one direct [`perform`] test that asks for it by name.
async fn performed<Show, Ask, Asked, Call, Called, Response>(
    record: &Mutation<'_>,
    clock: impl Fn() -> Timestamp,
    audit: &mut impl Write,
    show: Show,
    ask: Ask,
    call: Call,
) -> Performed
where
    Show: FnOnce(&Shown<'_>),
    Ask: FnOnce(Checked<Response>) -> Asked,
    Asked: std::future::Future<Output = Answer>,
    Call: Fn(Pass) -> Called,
    Called: std::future::Future<Output = Result<Response, kube::Error>>,
{
    perform(record, clock, audit, show, ask, call, finished).await
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

    let done = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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

    let _ = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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

    let done = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        move |checked: Checked<String>| {
            *heard.borrow_mut() = checked.returned().cloned();
            std::future::ready(checked.pressed())
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
        // **The name is taken before the value is handed over**: an [`Answer`] is not `Copy` any
        // more, because a confirmation is a thing that happened once ([`Agreed`]).
        let named = format!("{answer:?}");

        let ended = performed(
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
            "a mutation nobody confirmed reached the API server: {named}"
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

    let stopped = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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
                "{RESULT} · dry-run: the check was sent and did not pass · the change was \
                 never sent — the cluster would not allow it: {denial}\n"
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

    let stopped = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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
            "{RESULT} · dry-run: the check was sent and did not pass · the change was never \
             sent — the cluster would not accept the request k8rs made: {invalid}\n"
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

        let stopped = performed(
            &scaling(),
            stamp,
            &mut sink,
            shows(&trace),
            confirms(&trace),
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

    let failed = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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

    let failed = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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

    let failed = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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
    // **A dialog and not an `Answer`**, since 2026-09-04: a confirmation is built from the
    // `Checked` the dialog was handed ([`Agreed`]'s ticket), so it cannot be a value in a table
    // beside the case it belongs to.
    for ending in ["confirmed", "cancelled", "gone", "changed"] {
        for checkable in [true, false] {
            let trace = trace();
            let mut sink = Sink(trace.clone());
            let record = Mutation {
                checkable,
                ..scaling()
            };

            let _ = performed(
                &record,
                stamp,
                &mut sink,
                shows(&trace),
                ends(&trace, ending),
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

    let done = performed(
        &unchecked,
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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
    let _ = performed(
        &scaling(),
        stamp,
        &mut working,
        shows(&canary),
        confirms(&canary),
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

    let refused = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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

    let refused = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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
    for (ending, outcome) in [
        ("confirmed", Outcome::Done),
        ("cancelled", Outcome::Cancelled),
    ] {
        let trace = trace();
        trace.borrow_mut().breaks_at = 2;
        let mut sink = Sink(trace.clone());

        let performed = performed(
            &scaling(),
            stamp,
            &mut sink,
            shows(&trace),
            ends(&trace, ending),
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
            .chain(if ending == "confirmed" {
                vec!["call".to_string()]
            } else {
                vec![]
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

    let _ = performed(
        &scaling(),
        stamp,
        &mut outer,
        shows(&trace),
        move |checked: Checked<()>| async move {
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
            let _ = performed(
                &cordon,
                move || inner_stamp,
                &mut inner,
                |_: &Shown<'_>| {},
                |checked: Checked<()>| std::future::ready(checked.pressed()),
                |_| std::future::ready(Ok::<(), kube::Error>(())),
            )
            .await;
            checked.pressed()
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
        // **The requirement wears a crafted shape too** (NOTES § D29, § D31): it is a name out of
        // the API like every neighbour here, and
        // `a_confirmation_compares_against_the_name_the_dialog_showed_and_not_the_one_the_api_sent`
        // is where what the strip does to it is asserted.
        confirm: Confirm::Type("web\u{202e}gnp"),
    };

    let done = performed(
        &crafted,
        stamp,
        &mut sink,
        shows(&trace),
        // **The stripped name and not the crafted one** — what the dialog was shown is what the
        // reader can type ([`Checked::typed`]).
        types(&trace, "webgnp"),
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

    let _ = performed(
        &scaling(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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

    let _ = performed(
        &wrapped,
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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

    let _ = performed(
        &record,
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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
        confirm: Confirm::Press,
        uid: None,
        consequence: "This stops new pods being scheduled onto k8rs-worker2. Pods already \
                      running there keep running.",
        kubectl: "kubectl cordon k8rs-worker2",
        verb: "PATCH",
        path: "/api/v1/nodes/k8rs-worker2",
        version: None,
        checkable: false,
    };

    let done = performed(
        &cordon,
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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

        let _ = performed(
            &record,
            stamp,
            &mut sink,
            shows(&trace),
            confirms(&trace),
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

    let _ = performed(
        &silent,
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
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
// onto the first for the same reason. `propagationPolicy` is the third, and it gets a third test
// (NOTES § D225 ruling 5) — the paragraph above predicted it by name, which is why it is not a
// clause bolted onto the delete's `dryRun` rows either.

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

/// **A plain `PATCH` of the object, and the media type it goes out as** — `Request::patch`, which
/// `restart` is the first operation to reach (todo.md 3777).
///
/// **A different entry point from the one above.** `Api::patch_scale` delegates to
/// `Request::patch_subresource`; `Api::patch` delegates to `Request::patch`
/// (`kube-core-4.2.0/src/request.rs:148`), which builds its own target and calls the same
/// `PatchParams::populate_qp`. Nothing in the suite had read that one until this box, and nothing
/// would have noticed it dropping either parameter.
///
/// **The patch is `restart`'s own** ([`restart_patch`]), so the media type this returns is the one
/// that operation really sends — the half of NOTES § D223 ruling 4 that no other test can see,
/// since kube keeps `Patch::content_type` `pub(crate)`.
fn plain_patch(pass: Pass) -> (String, String) {
    let request = deployments()
        .patch("web", &pass.patch(), &restart_patch(&stamp()))
        .expect("a patch request built from a valid name and this file's own params");
    let uri = request.uri().to_string();
    let media = request
        .headers()
        .get("content-type")
        .map(|value| String::from_utf8_lossy(value.as_bytes()).to_string())
        .unwrap_or_default();
    println!("PATCH {uri} · content-type {media}");
    (uri, media)
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

/// **A plain `PATCH` is a dry run on the check and is not one on the change** — the same claim as
/// the two tests above, over the entry point neither of them touches (`tester`, 2026-09-04).
///
/// **The negative is the point.** `Request::patch` reaching a different `populate_qp`, or
/// [`Pass::patch`] answering the same params either way, would be a restart that is confirmed and
/// never made — invariant 4's *neither record may lie*.
#[test]
fn a_plain_patch_of_the_object_is_a_dry_run_on_the_check_and_not_on_the_change() {
    assert!(
        plain_patch(DRY_RUN).0.contains("dryRun=All"),
        "a plain patch built from the check pass would change the cluster: nothing in the query \
         string tells the API server this is a dry run"
    );
    assert!(
        !plain_patch(FOR_REAL).0.contains("dryRun"),
        "the real patch of the object is still a dry run, so the change is confirmed and never \
         made"
    );
}

/// **Both passes of a plain `PATCH` ask the server to reject a field the cluster does not have** —
/// [`Pass::patch`]'s `fieldValidation=Strict`, over `Request::patch`.
///
/// **On the change and not only on the check**, for the reason the subresource's own test gives:
/// a `Strict` that rode only on [`DRY_RUN`] would let the real call answer `200 OK`, alter
/// nothing, and be recorded as a successful mutation.
#[test]
fn both_passes_of_a_plain_patch_ask_the_server_to_reject_an_unknown_field() {
    for (pass, which) in [(DRY_RUN, "check"), (FOR_REAL, "change")] {
        assert!(
            plain_patch(pass).0.contains("fieldValidation=Strict"),
            "the {which} would accept a field the cluster does not have"
        );
    }
}

/// **Both passes of a delete ask for background deletion** — NOTES § D225 ruling 5, in the body
/// the API server reads and not on a struct field.
///
/// **`kubectl delete` sends `{"propagationPolicy":"Background"}` and nothing else** — measured
/// against a real apiserver, byte-identical across all six kinds
/// (`reports/2026-09-04-delete-on-the-wire.md` § 1-2) — so invariant 4's *equivalent* command
/// needs this set explicitly: `DeleteParams::default()` leaves it `None` and lets the server pick
/// a per-resource default, which the taught line's *no flag* would then not be equivalent to.
///
/// **On both passes, for the reason `fieldValidation` is on both**: [`Pass`] converts *which pass*
/// into *what goes on the wire*, and a policy that rode only on [`FOR_REAL`] would make the check
/// a rehearsal of a different request. That `delete` sends no check today
/// (NOTES § D225 ruling 1) is the operation's decision and not this conversion's.
///
/// **A `contains` and not an equality**, which is this region's own rule: the next box in this
/// phase adds `preconditions` here, and an exact assertion would read that as this box's defect.
/// The end-to-end delete tests assert the whole body, where the subject is the request the
/// operation made.
#[test]
fn both_passes_of_a_delete_ask_the_server_to_delete_in_the_background() {
    for (pass, which) in [(DRY_RUN, "check"), (FOR_REAL, "change")] {
        let (_, body) = delete_wire(pass);
        assert!(
            body.contains(r#""propagationPolicy":"Background""#),
            "the {which} lets the server pick its own default, so `kubectl delete` with no flag \
             is not what k8rs sent: {body}"
        );
    }
}

/// **`restart` sends a strategic merge patch, and that is a safety property before it is a
/// fidelity one** (NOTES § D223 ruling 4, § D217).
///
/// **What the media type decides is what a `422` hands back.** A `application/merge-patch+json`
/// rejection on a workload is answered with the *patched object* — 4859 bytes on a trivial
/// Deployment, carrying `managedFields`, annotations and `spec.template.spec.containers[].env[]
/// .value` — and that message is what the audit line quotes. A strategic merge patch is answered
/// with k8rs's own six lines instead (`patch.go:770-786` rather than `:353`). It is also what
/// `kubectl rollout restart` sends, so the exposure fix and invariant 4's *equivalent command* are
/// one choice.
///
/// **Nothing else in the suite can see this**: kube keeps `Patch::content_type` `pub(crate)`, and
/// the stub API server the end-to-end tests drive logs the method, the target and the body — not
/// the headers.
#[test]
fn a_restarts_patch_goes_out_as_a_strategic_merge_and_not_as_a_json_merge() {
    for pass in [DRY_RUN, FOR_REAL] {
        assert_eq!(
            plain_patch(pass).1,
            "application/strategic-merge-patch+json",
            "a rejection of this patch would hand the whole workload back into the audit log"
        );
    }
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

/// **A client pointed at a loopback port nothing is listening on** — [`stub`]'s address without
/// [`stub`]'s server.
///
/// **The port is bound and released rather than picked**, for [`stub`]'s own reason: there is no
/// hardcoded loopback URL in this tree for `scripts/security-guard.py` to be right about, and a
/// number chosen by hand is a number some other process may hold.
async fn dead_port() -> Client {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port");
    let address = listener.local_addr().expect("the port it picked");
    drop(listener);
    Client::try_from(kube::Config::new(
        format!("http://{address}")
            .parse()
            .expect("an address the kernel just gave us"),
    ))
    .expect("a client over plain http asks the machine for nothing")
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

/// **A kind word out of argv is free text, and a refusal quotes it back only where the strip left
/// it alone** (invariant 9, invariant 14, NOTES § D224).
///
/// **Two shapes the old sentence got wrong, both measured** (`tester`, 2026-09-04). An **empty**
/// word — `""`, and a lone `U+202E` that cleans to one — printed *k8rs cannot scale a  — …*, a
/// gap where the kind should be. And a word the strip **changed into a served kind** —
/// `deployment\n`, `deploy\0ment`, `dep\u{200b}loyment` — printed *k8rs cannot scale a
/// deployment … k8rs does that for a deployment, …*, one sentence contradicting its own second
/// clause.
///
/// **Neither is reachable from a command line**, because `known_kind` hands these functions one of
/// six canonical singulars — and both functions are `pub` in a file that freezes at the end of
/// this phase.
///
/// **The word that survives the strip is still quoted**, which is what stops [`a_kind`] being
/// green by refusing to name anything.
#[test]
fn a_crafted_kind_cannot_rewrite_the_terminal_on_its_way_into_scales_refusal() {
    for crafted in [
        "pod\u{1b}[2J\u{202e}",
        "",
        "\u{202e}",
        "deployment\n",
        "deploy\0ment",
        "dep\u{200b}loyment",
    ] {
        let refusal = scalable(crafted).expect_err("that is not a kind scale works on");
        println!("{crafted:?}\n{refusal}");
        assert!(
            !refusal.chars().any(crate::k8s::unprintable),
            "{crafted:?} carried an escape into the refusal: {refusal:?}"
        );
        assert!(
            refusal.starts_with("k8rs cannot scale that kind — "),
            "{crafted:?} was quoted back as a word nobody typed: {refusal:?}"
        );
        assert!(
            refusal.contains(SCALABLE),
            "{crafted:?} was refused without being told what scale works on: {refusal:?}"
        );
    }
    // **A word the strip leaves alone is still named**, which is the half that would go missing
    // if `a_kind` simply stopped quoting anything.
    let refusal = scalable("widget").expect_err("that is not a kind scale works on");
    println!("{refusal}");
    assert!(
        refusal.starts_with("k8rs cannot scale a widget — "),
        "{refusal:?}"
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
        confirms(&trace),
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
        confirms(&trace),
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
    // **A press and not a typed name** — invariant 2 raises the bar to typing the object's own
    // name for `delete` and `drain` and for nothing else, so an operation that asked for one here
    // would be a second confirmation kind this repo deliberately does not have
    // (NOTES § D225 ruling 2).
    assert_eq!(
        trace.borrow().asks,
        Some(None),
        "this operation asked the reader to type the object's name"
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
        confirms(&trace),
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
        confirms(&trace),
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
        confirms(&trace),
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
            confirms(&trace),
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
            confirms(&trace),
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
        confirms(&trace),
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

// --- RESTART ---
//
// **The wire again, and for a second reason.** `restart` is the first operation to patch the
// object itself, so what these assert is that exactly two requests go out, both on
// `…/deployments/web` with no subresource on the end, carrying the annotation `kubectl` writes and
// not the one kube's helper writes (NOTES § D215) — and that the first one is a dry run and the
// second is not.
//
// **What no socket can see is the media type**, because the stub logs the method, the target and
// the body. NOTES § D223 ruling 4's `Patch::Strategic` is asserted twice instead, off values:
// [`restart_patch`]'s own variant here, and `Request::patch`'s header for that variant in
// § WHAT THE PASS PUTS ON THE WIRE.

/// The restart `k8rs ops restart deploy/web -n payments` describes.
fn restarting() -> Restarting<'static> {
    Restarting {
        context: "kind-k8rs",
        // A reserved host, for `scaling()`'s own reason: `scripts/security-guard.py` reads a
        // loopback URL in this tree as a second outbound path and is right to.
        server: "https://k8rs-tests.invalid:41751",
        kind: "deployment",
        name: "web",
        namespace: Some("payments"),
    }
}

/// **What a cluster hands back from a patched Deployment** — enough of one for `DynamicObject` to
/// deserialise, and nothing `restart` reads: it reads the object it got back for exactly nothing
/// (NOTES § D223 ruling 3).
fn patched() -> String {
    r#"{"apiVersion":"apps/v1","kind":"Deployment","metadata":{"name":"web",
       "namespace":"payments","uid":"18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11",
       "resourceVersion":"41752"},"spec":{},"status":{}}"#
        .to_string()
}

/// **The same Deployment after `kubectl rollout pause`** — [`patched`] with the one field the
/// check's answer is read for (NOTES § D224).
fn paused_deployment() -> String {
    r#"{"apiVersion":"apps/v1","kind":"Deployment","metadata":{"name":"web",
       "namespace":"payments","uid":"18f0b6ee-2b0e-4b53-9b3e-6f4d3a2c0f11",
       "resourceVersion":"41752"},"spec":{"paused":true},"status":{}}"#
        .to_string()
}

/// **A `404` from a cluster that has no object by that name** — the shape measured against a real
/// apiserver, `details` and all (NOTES § D224).
fn not_found(name: &str) -> (String, String) {
    (
        "404 Not Found".to_string(),
        format!(
            r#"{{"kind":"Status","apiVersion":"v1","status":"Failure","code":404,
               "reason":"NotFound","message":"deployments.apps \"{name}\" not found",
               "details":{{"name":"{name}","group":"apps","kind":"deployments"}}}}"#
        ),
    )
}

/// **A clock that reads one second later every time it is asked** — the only way to tell *the
/// stamp was read once* from *the stamp was read per pass*, which a fixed clock cannot.
///
/// [`perform`] reads the clock twice of its own accord (the attempt line and the landing time),
/// so the ticks a test sees are the operation's first and then those two.
fn ticking(second: &std::cell::Cell<i64>) -> impl Fn() -> Timestamp + '_ {
    move || {
        let now = second.get();
        second.set(now + 1);
        Timestamp::from_second(now).expect("a timestamp inside jiff's range")
    }
}

/// The `PATCH` a restart sends, as the stub logs it: the target, and the body under it.
fn restart_call(query: &str) -> String {
    format!(
        "PATCH /apis/apps/v1/namespaces/payments/deployments/web{query} \
         {{\"spec\":{{\"template\":{{\"metadata\":{{\"annotations\":\
         {{\"kubectl.kubernetes.io/restartedAt\":\"2026-09-03T12:34:56Z\"}}}}}}}}}}"
    )
}

/// **What `restart` can be pointed at, and what it says about everything else** —
/// NOTES § Operations' `r` row, over every kind the driver lets through (NOTES § D220 ruling 7).
///
/// **`main.rs`'s `KINDS` is read, not copied**, for [`scalable`]'s own reason: the driver accepts
/// all of them for all three verbs on purpose, and the refusal is what stops the ones restart does
/// not serve.
///
/// **The pod is neither served nor refused with the general sentence** — it has its own, which is
/// NOTES § D223 ruling 1 and is asserted below.
#[test]
fn restart_takes_the_three_kinds_it_works_on_and_names_them_when_it_refuses_the_rest() {
    let works = [
        ("deployment", "deployments"),
        ("statefulset", "statefulsets"),
        ("daemonset", "daemonsets"),
    ];
    let (mut served, mut refused) = (0, 0);
    for kind in &crate::KINDS {
        let kind = kind.singular;
        if let Some((_, plural)) = works.iter().find(|(named, _)| *named == kind) {
            served += 1;
            let resource = restartable(kind).unwrap_or_else(|refusal| panic!("{kind}: {refusal}"));
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
            let refusal =
                restartable(kind).expect_err("a kind restart does not work on is refused");
            println!("{kind}\n{refusal}");
            // The pod's sentence is its own (NOTES § D223 ruling 1); so is the replicaset's,
            // because a replicaset is the one refused kind whose copies an operator would
            // actually want replaced (NOTES § D224). Every other kind gets the general one.
            if kind == "pod" {
                assert_eq!(
                    refusal,
                    pod_is_a_delete(),
                    "the pod arm lost its own sentence"
                );
            } else if kind == "replicaset" {
                assert!(
                    refusal.contains("restarting that deployment is what replaces its copies"),
                    "the replicaset refusal does not say what to restart instead: {refusal:?}"
                );
            } else {
                assert!(
                    refusal.contains(&format!("cannot restart a {kind}")),
                    "{kind}: the refusal does not name the kind that was asked for: {refusal:?}"
                );
            }
            // **Every refusal names what restart *can* be pointed at, in plain words**
            // (invariant 14): a reader told only *no* has to go and find the table this is.
            assert!(
                refusal.contains("a deployment, a statefulset and a daemonset"),
                "{kind}: the refusal does not say what restart works on: {refusal:?}"
            );
        }
    }
    // **The derived list says what it found** — an empty `KINDS`, or a renamed entry that no
    // longer matches `works`, passes every assertion above by running none of them.
    assert_eq!(
        (served, refused),
        (works.len(), crate::KINDS.len() - works.len()),
        "the driver's kind table no longer splits into the three restart serves and the rest"
    );
}

/// **A kind word out of argv is free text, and a refusal quotes it back only where the strip left
/// it alone** (invariant 9, invariant 14, NOTES § D224).
///
/// **Two shapes the old sentence got wrong, both measured** (`tester`, 2026-09-04). An **empty**
/// word — `""`, and a lone `U+202E` that cleans to one — printed *k8rs cannot restart a  — …*, a
/// gap where the kind should be. And a word the strip **changed into a served kind** —
/// `deployment\n`, `deploy\0ment`, `dep\u{200b}loyment` — printed *k8rs cannot restart a
/// deployment … k8rs does that for a deployment, …*, one sentence contradicting its own second
/// clause.
///
/// **Neither is reachable from a command line**, because `known_kind` hands these functions one of
/// six canonical singulars — and both functions are `pub` in a file that freezes at the end of
/// this phase.
///
/// **The word that survives the strip is still quoted**, which is what stops [`a_kind`] being
/// green by refusing to name anything.
///
/// **The two siblings answer alike**, which is [`a_kind`]'s whole reason for being one function:
/// `scalable` has the identical shape and the identical exposure, and a fix that left them
/// disagreeing is what the family review exists to catch.
#[test]
fn a_crafted_kind_cannot_rewrite_the_terminal_on_its_way_into_restarts_refusal() {
    for crafted in [
        "job\u{1b}[2J\u{202e}",
        "",
        "\u{202e}",
        "deployment\n",
        "deploy\0ment",
        "dep\u{200b}loyment",
    ] {
        let refusal = restartable(crafted).expect_err("that is not a kind restart works on");
        println!("{crafted:?}\n{refusal}");
        assert!(
            !refusal.chars().any(crate::k8s::unprintable),
            "{crafted:?} carried an escape into the refusal: {refusal:?}"
        );
        assert!(
            refusal.starts_with("k8rs cannot restart that kind — "),
            "{crafted:?} was quoted back as a word nobody typed: {refusal:?}"
        );
        assert!(
            refusal.contains(RESTARTABLE),
            "{crafted:?} was refused without being told what restart works on: {refusal:?}"
        );
    }
    let refusal = restartable("widget").expect_err("that is not a kind restart works on");
    println!("{refusal}");
    assert!(
        refusal.starts_with("k8rs cannot restart a widget — "),
        "{refusal:?}"
    );
}

/// **Restarting a pod is refused in words and nothing is deleted** (NOTES § D223 ruling 1,
/// `screens/dialogs.md` rule 4).
///
/// **The sentence says what it would really be**, which is the whole of rule 4: nobody learns
/// "restart" as a synonym for "delete" by accident. The dialog that offers the delete instead is
/// Phase 11's, over the path todo.md 3811 will have proven.
///
/// **And no request goes out at all** — this box sends no `DELETE` anywhere, and the assertion is
/// on the socket rather than on the sentence.
#[tokio::test]
async fn restarting_a_pod_says_it_would_be_a_delete_and_deletes_nothing() {
    let refusal = restartable("pod").expect_err("a pod is not something k8rs restarts");
    println!("{refusal}");
    assert_eq!(
        refusal,
        pod_is_a_delete(),
        "the pod arm lost its own sentence"
    );
    // **The last row is the one the sentence never had** (`k8s-admin`, NOTES § D224): it ended
    // naming three kinds the reader had not asked about and never said what to do instead.
    for owed in [
        "will not restart a pod",
        "means deleting it",
        "letting the thing that created it start a replacement",
        "a deployment, a statefulset and a daemonset",
        "if this pod belongs to one, restart that instead",
    ] {
        assert!(refusal.contains(owed), "{owed:?} is not in {refusal:?}");
    }

    let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let pod = Restarting {
        kind: "pod",
        ..restarting()
    };
    let stopped = restart(
        &client,
        &pod,
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect_err("a pod is not something k8rs restarts");
    assert_eq!(stopped, refusal);
    assert!(
        sent.lock().expect("the log is never poisoned").is_empty(),
        "a pod restart sent something to the cluster"
    );
    assert!(
        transcript(&trace).is_empty(),
        "a pod restart was written into the audit log"
    );
}

/// **The three sentences, in `screens/dialogs.md` § Restart's own words** — asserted against that
/// file and not against what [`rollout`] happens to return, which is the whole of what was wrong
/// with them (NOTES § D224).
///
/// **They can now fail for being wrong and not only for changing.** The three this replaced were
/// exact-equality assertions pinned to copies of themselves: unlike `scale` there was no screen
/// for them to be derived from, and all three then stated a pacing the cluster owns — falsified on
/// a real cluster by a `maxUnavailable: 3` DaemonSet, a `partition`ed StatefulSet, a
/// `nodeSelector`'d DaemonSet and `OnDelete` on either kind (`tester` finding 2, `k8s-admin`).
///
/// **One sentence for all three would still be true of one of them**, which is why [`rollout`]
/// returns the consequence beside the resource rather than beside a count it never reads.
#[test]
fn each_kind_gets_the_sentence_that_is_true_of_how_its_copies_are_replaced() {
    let said = |kind: &str| {
        let (_, consequence) = rollout(kind).unwrap_or_else(|refusal| panic!("{kind}: {refusal}"));
        println!("{kind}\n{consequence}\n");
        consequence
    };
    assert_eq!(
        said("deployment"),
        "This asks Kubernetes to replace every copy of your app with a new one. How many stop at \
         the same time is a setting on this deployment — it can be a few, or all of them at once. \
         A paused deployment will not start until you resume it."
    );
    assert_eq!(
        said("statefulset"),
        "This asks Kubernetes to replace every copy of your app with a new one, working down from \
         the highest-numbered copy. How many stop at the same time, how far down it goes, and \
         whether it waits for you to delete a copy yourself are all settings on this statefulset."
    );
    assert_eq!(
        said("daemonset"),
        "This asks Kubernetes to replace the copy of your app on each node it runs on. How many \
         nodes it takes at a time, and whether it waits for you to delete a copy yourself, are \
         settings on this daemonset."
    );
    // **Every one of them *asks*** (NOTES § D224). The patch is a request and the controller
    // decides, which is the one framing that stays true under `paused`, `OnDelete`, `partition`
    // and every pacing knob at once — a sentence that says *replaces* is claiming a denominator
    // k8rs has not read (`PRIOR-ART § F2`).
    for kind in ["deployment", "statefulset", "daemonset"] {
        assert!(
            said(kind).starts_with("This asks Kubernetes to replace "),
            "{kind}'s consequence promises the rollout rather than asking for it: {:?}",
            said(kind)
        );
    }
    // **No API vocabulary reaches a dialog** (invariant 14) — the words a beginner has not met
    // are the ones the sentence exists to avoid.
    for kind in ["deployment", "statefulset", "daemonset"] {
        let consequence = said(kind).to_lowercase();
        for jargon in [
            "replica",
            "pod",
            "rollout",
            "template",
            "annotation",
            "patch",
        ] {
            assert!(
                !consequence.contains(jargon),
                "{kind}'s consequence uses {jargon:?}: {consequence:?}"
            );
        }
    }
}

/// **The patch is a strategic merge, and it is the six lines and nothing else**
/// (NOTES § D223 ruling 4, § D217).
///
/// **The variant is what decides what a `422` hands back.** Under `application/merge-patch+json`
/// the apiserver answers with the *patched object* — measured at 4859 bytes on a trivial
/// Deployment, carrying `managedFields`, annotations and container environments — and that message
/// is what the audit line quotes. Under a strategic merge it answers with this. Truncation is not
/// redaction, and `FREE_TEXT` cuts long after the annotations.
///
/// **kube keeps `Patch::content_type` `pub(crate)`**, so this asserts the variant and
/// `a_restarts_patch_goes_out_as_a_strategic_merge_and_not_as_a_json_merge` asserts what
/// `Request::patch` makes of it.
#[test]
fn the_patch_is_a_strategic_merge_of_six_lines_and_carries_kubectls_own_annotation() {
    let patch = restart_patch(&stamp());
    println!("{patch:?}");
    assert_eq!(
        patch,
        Patch::Strategic(serde_json::json!({
            "spec": { "template": { "metadata": { "annotations": {
                "kubectl.kubernetes.io/restartedAt": "2026-09-03T12:34:56Z"
            } } } }
        })),
        "the patch is not the six lines kubectl sends, under the media type that keeps a \
         rejection from quoting the whole workload back"
    );
    // **kube's helper writes the other key** (NOTES § D215), and an operator who then runs the
    // taught line gets a *second* rollout from an annotation kubectl has never seen.
    assert!(
        !format!("{patch:?}").contains("kube.kubernetes.io/restartedAt"),
        "the patch carries kube's own annotation key rather than kubectl's"
    );
}

/// **A restart patches the object, reads nothing first, and sends the check before the change** —
/// the whole box, on a socket, so what is asserted is what went on the wire (todo.md 3777).
///
/// **Two requests and no third.** `scale` opens with a `GET` because its sentence is built from a
/// number only the cluster has; a restart needs no fact about the object to describe itself
/// (NOTES § D223 ruling 3), so a `GET` here would be a read of container environments for a dialog
/// that shows none of them.
///
/// **Neither target ends in a subresource.** This is `Request::patch` and not
/// `Request::patch_subresource`, which is what makes it the first operation to reach the former.
#[tokio::test]
async fn a_restart_patches_the_object_itself_and_reads_nothing_before_it() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = restart(
        &client,
        &restarting(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect("a deployment in a namespace, with a cluster that answers");

    let requests = sent.lock().expect("the log is never poisoned").clone();
    println!("{}", requests.join("\n"));
    assert_eq!(
        requests,
        vec![
            restart_call("?&dryRun=All&fieldValidation=Strict"),
            restart_call("?&fieldValidation=Strict"),
        ],
        "the restart did not check the object and then patch it, with nothing read first"
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
        "a restart that landed does not say so"
    );
    assert!(done.changed(), "a restart that landed is not an exit 0");
}

/// **Both passes carry the same stamp**, because the annotation is read once and the closure
/// [`perform`] calls twice only reads it (`restart`'s own doc).
///
/// **A fixed clock cannot see this**, which is why this one ticks: a `clock()` moved inside the
/// closure would dry-run one annotation value and send another, and every assertion in the test
/// above would still hold.
#[tokio::test]
async fn the_check_and_the_change_carry_one_stamp_and_not_one_each() {
    let second = std::cell::Cell::new(1_788_438_896_i64);
    let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = restart(
        &client,
        &restarting(),
        ticking(&second),
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect("a deployment in a namespace, with a cluster that answers");
    assert_eq!(done.outcome, Some(Outcome::Done));

    let requests = sent.lock().expect("the log is never poisoned").clone();
    println!("{}", requests.join("\n"));
    let body = |request: &str| {
        request
            .split_once(" {")
            .map(|(_, body)| format!("{{{body}"))
            .expect("every patch this operation sends carries a body")
    };
    assert_eq!(
        body(&requests[0]),
        body(&requests[1]),
        "the cluster checked one restart and was sent another"
    );
    // **The stamp is the operation's own first reading of the clock** — the two after it are
    // [`perform`]'s attempt line and its landing time.
    assert!(
        body(&requests[0]).contains("\"2026-09-03T12:34:56Z\""),
        "the annotation was not stamped with the first reading: {}",
        requests[0]
    );
    assert_eq!(
        second.get(),
        1_788_438_899,
        "the clock was read some number of times other than three"
    );
}

/// **What the dialog and the audit log were given** — the consequence for the kind, the object as
/// the reader knows it, the kubectl line with no dry-run flag on it, and the path the request
/// really took.
///
/// **`uid not read` and `resourceVersion not sent` are the record being honest** about a mutation
/// that read nothing first (NOTES § D223 ruling 3): both gaps are named rather than left as
/// dangling labels.
#[tokio::test]
async fn what_a_restart_records_is_the_call_it_made_and_the_two_things_it_never_read() {
    let (client, _) = stub(|_| ("200 OK".to_string(), patched())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = restart(
        &client,
        &restarting(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect("a deployment in a namespace, with a cluster that answers");
    assert_eq!(done.outcome, Some(Outcome::Done));

    assert_eq!(
        trace.borrow().dialog,
        Some(Dialog {
            object: "deployment/web".to_string(),
            namespace: Some("payments".to_string()),
            consequence: "This asks Kubernetes to replace every copy of your app with a new \
                          one. How many stop at the same time is a setting on this deployment — \
                          it can be a few, or all of them at once. A paused deployment will not \
                          start until you resume it."
                .to_string(),
            // **`deployment/web`, never `deploy/web`** (`screens/dialogs.md` § Scale), and no
            // `--dry-run`: `kubectl rollout restart` has no such flag (NOTES § D223 ruling 4), so
            // the taught line cannot claim the preflight k8rs ran.
            kubectl: "kubectl rollout restart deployment/web -n payments".to_string(),
        }),
        "the dialog was not given the object, what a restart does to it, or a runnable kubectl line"
    );
    // **A press and not a typed name** — invariant 2 raises the bar to typing the object's own
    // name for `delete` and `drain` and for nothing else, so an operation that asked for one here
    // would be a second confirmation kind this repo deliberately does not have
    // (NOTES § D225 ruling 2).
    assert_eq!(
        trace.borrow().asks,
        Some(None),
        "this operation asked the reader to type the object's name"
    );
    assert!(
        !trace
            .borrow()
            .dialog
            .as_ref()
            .expect("the dialog was opened")
            .kubectl
            .contains("dry-run"),
        "the taught line offers a flag `kubectl rollout restart` does not have"
    );
    let lines = transcript(&trace);
    let attempt = lines
        .iter()
        .find(|line| line.contains("attempt ·"))
        .expect("the attempt line is written before anything is sent");
    assert_eq!(
        attempt,
        "audit: 2026-09-03T12:34:56Z attempt · deployment/web · context kind-k8rs · server \
         https://k8rs-tests.invalid:41751 · namespace payments · uid not read · kubectl: kubectl \
         rollout restart deployment/web -n payments · call: PATCH \
         /apis/apps/v1/namespaces/payments/deployments/web · resourceVersion not sent\n",
        "the attempt line does not name the call that was actually made"
    );
    let result = lines
        .iter()
        .find(|line| line.contains("result ·"))
        .expect("the result line is written when the call returns");
    assert!(
        result.contains("dry-run: the cluster checked it first and accepted it")
            && result.contains("· the change was made"),
        "the result line does not say the restart was checked and made: {result:?}"
    );
}

/// **Cancelling sends the check and nothing after it** — invariant 2 through the real operation.
///
/// **One request and not two**, because a restart reads nothing: the check *is* the first thing
/// this operation sends.
#[tokio::test]
async fn a_restart_nobody_confirmed_sends_the_check_and_never_the_change() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = restart(
        &client,
        &restarting(),
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
        requests,
        vec![restart_call("?&dryRun=All&fieldValidation=Strict")],
        "a cancelled restart sent the change anyway"
    );
    assert_eq!(done.outcome, Some(Outcome::Cancelled));
    assert!(!done.changed(), "a cancelled restart is not an exit 0");
    assert_eq!(
        done.plainly(),
        "nobody confirmed it, so nothing was changed"
    );
}

/// **A cluster that refuses the check is a mutation that was attempted and recorded** — unlike
/// [`scale`]'s read refusal, which happens before there is anything to describe.
///
/// **The reason is keyed on the `Fault` and the server's own words travel beside it**
/// (`PRIOR-ART § C1`), which is what tells an operator whether to fix their RBAC or their network.
#[tokio::test]
async fn a_restart_the_cluster_would_not_check_is_recorded_and_never_sent() {
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

    let done = restart(
        &client,
        &restarting(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect("a refused check is an outcome and not a refusal of the request");

    println!("{}", done.plainly());
    assert_eq!(
        done.outcome,
        Some(Outcome::NotSent {
            fault: Fault::Refused,
            said: Some("deployments.apps \"web\" is forbidden".to_string()),
        })
    );
    assert!(!done.changed(), "a refused restart is not an exit 0");
    assert_eq!(
        done.plainly(),
        "the change was never sent — the cluster would not allow it: deployments.apps \"web\" is \
         forbidden"
    );
    assert_eq!(
        sent.lock().expect("the log is never poisoned").len(),
        1,
        "a check the cluster refused was followed by the change anyway"
    );
    let lines = transcript(&trace);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("dry-run: the check was sent and did not pass")),
        "a refused check was recorded as one that never went out: {lines:?}"
    );
}

/// **A namespace nobody named is refused inside the operation, before anything is sent** — one
/// place, rather than a second copy of which kinds live in a namespace (NOTES § D220 ruling 4).
#[tokio::test]
async fn a_restart_with_no_namespace_is_refused_before_a_single_call_goes_out() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let nowhere = Restarting {
        namespace: None,
        ..restarting()
    };

    let refusal = restart(
        &client,
        &nowhere,
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect_err("a namespaced object with no namespace is not something to restart");

    println!("{refusal}");
    assert_eq!(
        refusal,
        "k8rs will not restart deployment/web without being told which namespace it is in"
    );
    assert!(
        sent.lock().expect("the log is never poisoned").is_empty(),
        "a restart with nowhere to send anything sent something"
    );
}

/// **A name that would change the address the request goes to is refused where the path is
/// built**, not only where the command line was parsed — [`scale`]'s own guard, for its own
/// reason: `restart` is `pub` in a file that freezes at the end of this phase, and Phase 12's
/// console is a caller nobody has written yet.
#[tokio::test]
async fn a_name_that_would_rewrite_a_restarts_request_path_is_refused_where_it_is_built() {
    for (name, namespace, which) in [
        ("web/../../secrets", "payments", "an object's own name"),
        ("web", "payments/../kube-system", "the name of a namespace"),
        ("", "payments", "an object's own name"),
        ("web", "", "the name of a namespace"),
    ] {
        let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
        let trace = trace();
        let mut sink = Sink(trace.clone());
        let crafted = Restarting {
            name,
            namespace: Some(namespace),
            ..restarting()
        };

        let refusal = restart(
            &client,
            &crafted,
            stamp,
            &mut sink,
            shows(&trace),
            confirms(&trace),
        )
        .await
        .expect_err("a name that is not addressable is not something to restart");

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

/// **A kind `restart` does not work on never reaches a cluster**, whatever the caller does — the
/// driver asks first (NOTES § D220 ruling 7), and the operation refuses again if it is asked
/// anyway.
///
/// **The replicaset is the one that matters here**, because it is a kind `scale` *does* serve: the
/// two matrices are genuinely different (NOTES § Operations), and a restart that inherited scale's
/// would patch a pod template a ReplicaSet's controller immediately reverts.
#[tokio::test]
async fn a_restart_pointed_at_a_kind_it_does_not_work_on_sends_nothing() {
    for kind in ["replicaset", "node"] {
        let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
        let trace = trace();
        let mut sink = Sink(trace.clone());
        let wrong = Restarting {
            kind,
            ..restarting()
        };

        let refusal = restart(
            &client,
            &wrong,
            stamp,
            &mut sink,
            shows(&trace),
            confirms(&trace),
        )
        .await
        .expect_err("a kind k8rs does not restart is not something to restart");

        println!("{refusal}");
        assert!(
            refusal.contains(&format!("cannot restart a {kind}")),
            "{refusal:?}"
        );
        assert!(
            sent.lock().expect("the log is never poisoned").is_empty(),
            "a kind k8rs will not restart was patched on the cluster anyway"
        );
    }
}

/// **The check's own answer says whether this Deployment is paused, and the dialog is told before
/// it asks** (NOTES § D224).
///
/// **This is the blocker no stand-in apiserver could produce**, because it is the *controller's*
/// answer to a `PATCH` the apiserver accepts. Measured on a real cluster: `kubectl rollout pause`,
/// then a restart whose consequence promised every copy replaced, whose dry-run passed, which said
/// *the change was made* and exited `0` — and whose three pods had the same names twelve seconds
/// later, while the `kubectl rollout restart` line it printed exits `1`.
///
/// **`Checked<bool>` and never `Checked<DynamicObject>`**, which the type annotation below asserts
/// at compile time: a `PATCH` is answered with the whole workload either way, and what the `map`
/// inside the closure decides is whether k8rs *holds* one — with
/// `spec.template.spec.containers[].env[].value` in it — for as long as the dialog is open
/// (NOTES § D223 ruling 3).
///
/// **It does not refuse, and the exit code does not move.** Writing the annotation on a paused
/// Deployment is not destructive and it takes effect on resume; what was wrong was the record.
#[tokio::test]
async fn a_paused_deployment_is_the_fact_the_check_hands_back_and_it_still_asks() {
    for (body, paused) in [(patched(), false), (paused_deployment(), true)] {
        let (client, sent) = stub(move |_| ("200 OK".to_string(), body.clone())).await;
        let trace = trace();
        let mut sink = Sink(trace.clone());
        let seen = std::cell::Cell::new(None);

        let done = restart(
            &client,
            &restarting(),
            stamp,
            &mut sink,
            shows(&trace),
            |checked| {
                let returned: Option<&bool> = checked.returned();
                seen.set(returned.copied());
                std::future::ready(checked.pressed())
            },
        )
        .await
        .expect("a deployment in a namespace, with a cluster that answers");

        println!("paused={paused} → returned={:?}", seen.get());
        assert_eq!(
            seen.get(),
            Some(paused),
            "the dialog was told the wrong thing about spec.paused"
        );
        // **The check answered it, so both passes still go out and the operator still decides.**
        assert_eq!(
            sent.lock().expect("the log is never poisoned").len(),
            2,
            "reading the check's answer changed how many requests a restart makes"
        );
        assert_eq!(done.outcome, Some(Outcome::Done));
        assert!(
            done.changed(),
            "a paused deployment turned a restart into an exit 2"
        );
    }
}

/// **`Gone` and `Changed` end a restart after the check and before the change** (NOTES § D22) —
/// asserted on the socket, because that is where *nothing was sent* is either true or not.
///
/// **A headless run cannot answer either of them** — nothing is watching the object behind a
/// prompt — so this is the console's path (Phase 12) proven over the operation it will call.
#[tokio::test]
async fn an_object_gone_or_changed_under_a_restart_leaves_the_check_as_the_only_request() {
    for (answer, outcome, said) in [
        (
            Answer::Gone,
            Outcome::Gone,
            "the object was already gone, so nothing was changed",
        ),
        (
            Answer::Changed,
            Outcome::Changed,
            "the object changed while this was open, so nothing was changed",
        ),
    ] {
        let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
        let trace = trace();
        let mut sink = Sink(trace.clone());
        // The name before the value — [`Answer`] is no longer `Copy` ([`Agreed`]).
        let named = format!("{answer:?}");

        let done = restart(
            &client,
            &restarting(),
            stamp,
            &mut sink,
            shows(&trace),
            asked(&trace, answer),
        )
        .await
        .expect("a deployment in a namespace, with a cluster that answers");

        let requests = sent.lock().expect("the log is never poisoned").clone();
        println!("{named} → {}\n{}", done.plainly(), requests.join("\n"));
        assert_eq!(
            requests,
            vec![restart_call("?&dryRun=All&fieldValidation=Strict")],
            "{named} sent the change anyway"
        );
        assert_eq!(done.outcome, Some(outcome), "{named}");
        assert_eq!(done.plainly(), said, "{named}");
        assert!(!done.changed(), "{named} is not an exit 0");
    }
}

/// **A restart with no audit line sends nothing, and a restart with no *result* line keeps what it
/// already knows** (NOTES § D21, NOTES § D220 ruling 1) — [`perform`]'s contract, over the
/// operation rather than over a fixture, so what is counted is requests on a socket.
#[tokio::test]
async fn a_restart_whose_audit_log_fails_sends_nothing_or_keeps_the_outcome_it_already_has() {
    for (breaks_at, requests, expected) in [
        (
            1,
            0,
            Performed {
                outcome: None,
                recorded: false,
            },
        ),
        (
            2,
            2,
            Performed {
                outcome: Some(Outcome::Done),
                recorded: false,
            },
        ),
    ] {
        let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
        let trace = trace();
        trace.borrow_mut().breaks_at = breaks_at;
        let mut sink = Sink(trace.clone());

        let done = restart(
            &client,
            &restarting(),
            stamp,
            &mut sink,
            shows(&trace),
            confirms(&trace),
        )
        .await
        .expect("a sink that cannot be written is an outcome and not a refusal of the request");

        println!("breaks_at={breaks_at} → {done:?}\n{}", done.plainly());
        assert_eq!(done, expected, "breaks_at={breaks_at}");
        assert_eq!(
            sent.lock().expect("the log is never poisoned").len(),
            requests,
            "breaks_at={breaks_at}: the cluster was sent the wrong number of requests"
        );
    }
    // **The derived assertion**: a sink that never fails records both lines and sends both
    // requests, so the rows above are not two spellings of a restart that does nothing.
    let (client, sent) = stub(|_| ("200 OK".to_string(), patched())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let done = restart(
        &client,
        &restarting(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect("a deployment in a namespace, with a cluster that answers");
    assert!(done.recorded, "a working sink recorded nothing");
    assert_eq!(sent.lock().expect("the log is never poisoned").len(), 2);
}

/// **A check that went out and failed is recorded as one that went out, on both operations**
/// (NOTES § D224, invariant 4).
///
/// **The field is [`Record::check`]'s and [`Record::check`] is shared**, so the defect was
/// `scale`'s as much as `restart`'s and the proof has to be too. Measured on a real `404`, the
/// line read `dry-run: not checked` over a `?dryRun=All` the apiserver's own audit log holds — and
/// *not checked* is already the sentence [`UNCHECKABLE`] means, which is a check that was never
/// sent.
#[tokio::test]
async fn a_check_that_was_sent_and_failed_is_never_recorded_as_one_that_was_not_sent() {
    const SENT: &str = "dry-run: the check was sent and did not pass";

    // `scale` reads the count first, so only its `PATCH` may 404 — a 404 on the `GET` is a
    // refusal of the request and writes no record at all.
    let (client, _) = stub(|asked| {
        if asked.starts_with("PATCH") {
            not_found("web")
        } else {
            ("200 OK".to_string(), scale_body(2))
        }
    })
    .await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let scaled = scale(
        &client,
        &asking(3),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect("a refused check is an outcome and not a refusal of the request");
    let scale_line = transcript(&trace)
        .last()
        .cloned()
        .expect("a result line for a mutation that was attempted");

    let (client, _) = stub(|_| not_found("wbe")).await;
    // A second sink, because the first is still holding the scale record above.
    let second = Shared::default();
    let mut sink = Sink(second.clone());
    let missing = Restarting {
        name: "wbe",
        ..restarting()
    };
    let restarted = restart(
        &client,
        &missing,
        stamp,
        &mut sink,
        shows(&second),
        confirms(&second),
    )
    .await
    .expect("a refused check is an outcome and not a refusal of the request");
    let restart_line = transcript(&second)
        .last()
        .cloned()
        .expect("a result line for a mutation that was attempted");

    for (verb, performed, line) in [
        ("scale", &scaled, &scale_line),
        ("restart", &restarted, &restart_line),
    ] {
        println!("{verb}\n{line}");
        assert!(
            matches!(
                performed.outcome,
                Some(Outcome::NotSent {
                    fault: Fault::Gone,
                    ..
                })
            ),
            "{verb}: a 404 on the check is not what reached the record: {performed:?}"
        );
        assert!(
            line.contains(SENT),
            "{verb}: a check that went out and failed is recorded as one that did not: {line:?}"
        );
        assert!(
            !line.contains("dry-run: not checked"),
            "{verb}: the line still denies the request the cluster answered: {line:?}"
        );
        // **And it does not collide with the value that means *no check was sent*.**
        assert!(
            !line.contains(UNCHECKABLE),
            "{verb}: a failed check is recorded as one k8rs declined to make: {line:?}"
        );
    }
}

/// **The `dry-run:` field is keyed on the [`Fault`] and never on the arm that raised it**
/// (NOTES § D224, invariant 4) — every one of the eleven, against the sentence the requirement
/// owes it rather than the one the code returns.
///
/// **The arm is not the fact.** [`Outcome::NotSent`] is reachable only from the `Err` of the
/// check, and round two read that as *so the check was sent* — which a refused connect, a name
/// that will not resolve and an `exec` login that exits non-zero all make false. Measured on the
/// built binary against a port nothing was listening on, the line said *the check was sent and
/// did not pass* beside *k8rs could not reach the cluster*, 305 µs after the attempt
/// (`k8s-admin`, 2026-09-04).
///
/// **Three classes, because there are three things to be told**, and the middle one is what
/// [`answered`] cannot give: it groups the four kubeconfig-and-login faults with the answered
/// ones, which is right for *was anything changed* and wrong for *did the request go out*.
///
/// **A twelfth [`Fault`] is caught by the compiler and not by this list** — [`Record::check`]
/// matches exhaustively with no `_` — so what this owes is that the eleven are eleven distinct
/// ones and not ten and a copy-paste.
#[test]
fn the_dry_run_field_is_keyed_on_the_fault_and_never_on_the_arm_that_raised_it() {
    const ANSWERED: &str = "the check was sent and did not pass";
    const NEVER_SENT: &str = "the check never left this machine";
    const UNKNOWN: &str = "k8rs does not know whether the check reached the cluster";

    let every = [
        // The cluster itself answered the check: a `403`, a `400`/`422`, a `409`, a `401`, a `404`.
        (Fault::Refused, ANSWERED),
        (Fault::Rejected, ANSWERED),
        (Fault::Conflict, ANSWERED),
        (Fault::Expired, ANSWERED),
        (Fault::Gone, ANSWERED),
        // Nothing left this machine — the kubeconfig would not build a connection, or the login
        // program produced no credential and kube's auth layer failed the request before it was
        // dispatched.
        (Fault::Kubeconfig, NEVER_SENT),
        (Fault::NoContext, NEVER_SENT),
        (Fault::BadEntry, NEVER_SENT),
        (Fault::NoCredential, NEVER_SENT),
        // Nothing usable came back, and a socket that died *after* the request went out is the
        // same `Fault` as one that never opened. k8rs cannot tell them apart and may not claim to.
        (Fault::Unanswered, UNKNOWN),
        (Fault::Unfinished, UNKNOWN),
    ];

    let record = Record::of(&scaling());
    for (fault, expected) in every {
        let said = record.check(&Outcome::NotSent {
            fault,
            said: Some("the cluster's own words".to_string()),
        });
        println!("{fault:?} → {said}");
        assert_eq!(
            said, expected,
            "{fault:?} is recorded as something else than what it is"
        );
        // **And no class of it collides with the value for a check k8rs declined to make**, which
        // is the collision NOTES § D224 was opened by.
        assert_ne!(
            said, UNCHECKABLE,
            "{fault:?} is recorded as a check k8rs chose not to send"
        );
    }

    // **The derived assertion**: every row distinct, so no variant went unfed behind a
    // copy-paste. `Fault` is `PartialEq` and nothing more, which is why this is a scan.
    //
    // **The count is not asserted, because a number here would be a gate that fails on the
    // correct edit** (my own second pass): eleven is what
    // `awk '/^pub enum Fault \{/,/^\}/' src/k8s.rs | grep -cE '^    [A-Z][A-Za-z]+,$'` says
    // today, and a twelfth variant is caught by the *compiler* — [`Record::check`] matches with
    // no `_`, so it cannot be added without an arm being chosen for it, and whoever chooses it
    // adds the row here. An `assert_eq!(every.len(), 11)` would instead go red for the person
    // who added the twelfth row, saying the table had *stopped* covering the enum.
    for (index, (fault, _)) in every.iter().enumerate() {
        assert!(
            !every[..index].iter().any(|(seen, _)| seen == fault),
            "{fault:?} is in the table twice, so some other fault is not in it at all"
        );
    }

    // **The healthy arm is not disturbed**, written out from `screens/dialogs.md`'s own sentence
    // rather than read back off [`ACCEPTED`] — the three above are new arms in a function that
    // already had one, and a `let … else` that fell through wrongly would be invisible from
    // inside the table.
    assert_eq!(
        record.check(&Outcome::Done),
        "the cluster checked it first and accepted it",
        "an outcome that is not a refused check stopped saying the check passed"
    );
}

/// **Neither operation claims a check was sent when nothing usable answered** (NOTES § D224,
/// invariant 4) — the shapes the real pipeline hands the arm, fed rather than reasoned about
/// (CLAUDE.md § D29).
///
/// **Both halves are [`Fault::Unanswered`] and only one of them left this machine, which is the
/// whole reason the sentence hedges** (my own second pass — an earlier name for this test said
/// *never left this machine* over a `500` that plainly had). A connect that was refused and a
/// server that answered unusably arrive as the same fault, so *k8rs does not know* is not a
/// softer way of saying *it was not sent*: it is the only claim true of both.
///
/// **The two operations reach the arm differently, which is why both are here.** `restart`
/// patches straight away, so a dead cluster fails on the check itself; `scale` reads the count
/// first and refuses before [`perform`] on a wholly dead one, so its only route to this line is a
/// connection that stops being usable *between* the `GET` and the dry-run.
#[tokio::test]
async fn neither_operation_claims_a_check_was_sent_when_nothing_usable_answered() {
    const ANSWERED: &str = "dry-run: the check was sent and did not pass";
    const UNKNOWN: &str = "dry-run: k8rs does not know whether the check reached the cluster";

    // **`restart` against a port nothing is listening on** — `k8s-admin`'s measured case, whose
    // audit line said the check had been sent 305 µs after the attempt, on the same line whose
    // next field said the cluster could not be reached.
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let restarted = restart(
        &dead_port().await,
        &restarting(),
        stamp,
        &mut sink,
        shows(&trace),
        confirms(&trace),
    )
    .await
    .expect("a cluster that cannot be reached is an outcome and not a refusal of the request");
    let restart_line = transcript(&trace)
        .last()
        .cloned()
        .expect("a result line for a mutation that was attempted");

    // **`scale` with the connection answering the read and then answering the check with
    // nothing usable** — a `500` whose reason nothing recognises, which is `k8s::answer`'s route
    // to the same [`Fault::Unanswered`] a dead socket takes.
    let (client, _) = stub(|asked| {
        if asked.starts_with("PATCH") {
            (
                "500 Internal Server Error".to_string(),
                r#"{"kind":"Status","apiVersion":"v1","status":"Failure","code":500,
                    "reason":"InternalError","message":"the server had an error"}"#
                    .to_string(),
            )
        } else {
            ("200 OK".to_string(), scale_body(2))
        }
    })
    .await;
    let second = Shared::default();
    let mut sink = Sink(second.clone());
    let scaled = scale(
        &client,
        &asking(3),
        stamp,
        &mut sink,
        shows(&second),
        confirms(&second),
    )
    .await
    .expect("a check that was not answered is an outcome and not a refusal of the request");
    let scale_line = transcript(&second)
        .last()
        .cloned()
        .expect("a result line for a mutation that was attempted");

    for (verb, performed, line) in [
        ("restart", &restarted, &restart_line),
        ("scale", &scaled, &scale_line),
    ] {
        println!("{verb}\n{line}");
        assert!(
            matches!(
                performed.outcome,
                Some(Outcome::NotSent {
                    fault: Fault::Unanswered,
                    ..
                })
            ),
            "{verb}: a cluster that did not answer is not what reached the record: {performed:?}"
        );
        assert!(
            !line.contains(ANSWERED),
            "{verb}: the line says a request went out that the cluster never answered: {line:?}"
        );
        assert!(
            line.contains(UNKNOWN),
            "{verb}: the line does not say k8rs cannot tell whether the check arrived: {line:?}"
        );
        // **The next field says the cluster could not be reached**, and the two were on one line
        // contradicting each other. Both are asserted so the pair cannot drift apart again.
        assert!(
            line.contains("the change was never sent — k8rs could not reach the cluster"),
            "{verb}: the outcome stopped naming the fault beside the check: {line:?}"
        );
    }
}

// --- DELETE ---
//
// **The wire again, and this time for what is *not* on it.** `delete` is the one operation that
// sends no `dryRun=All` (NOTES § D225 ruling 1), so what these assert is that exactly **one**
// request goes out where `scale` and `restart` send two, that its body is
// `{"propagationPolicy":"Background"}` and nothing else (ruling 5), and that a cancelled delete
// sends nothing at all — which no other operation in this file can show, because for the other
// two the check has already gone out by the time anybody is asked.
//
// **And it is the first cluster-scoped call**, so the node rows assert a path with no namespace
// segment, a taught line with no `-n`, and an attempt line that says `cluster-wide` rather than
// leaving a dangling label (ruling 3).

/// The delete `k8rs ops delete pod/web-7d9f4 -n payments` describes — `screens/dialogs.md`
/// § Delete's own object.
fn deleting() -> Deleting<'static> {
    Deleting {
        context: "kind-k8rs",
        // A reserved host, for [`scaling`]'s own reason.
        server: "https://k8rs-tests.invalid:41751",
        kind: "pod",
        name: "web-7d9f4",
        namespace: Some("payments"),
    }
}

/// **What a cluster hands back from a delete** — a `Status`, which is the half of
/// `Either<K, Status>` an accepted delete usually answers with. `delete` reads neither half
/// (NOTES § D225 ruling 4), which is what this being the *other* variant from [`patched`] proves
/// costs nothing.
fn gone() -> String {
    r#"{"kind":"Status","apiVersion":"v1","status":"Success","code":200}"#.to_string()
}

/// **What a cluster hands back when it has accepted the removal and not finished it** — the
/// object, carrying `deletionTimestamp`, which is the other half of `Api::delete`'s
/// `Either<K, Status>` (NOTES § D225, `k8s-admin`, 2026-09-04).
///
/// **Measured shape and not an invented one**: a Node held by a finalizer and a pod inside its
/// grace period both answer `200 OK` with the object. Nothing in `delete` reads a field off it —
/// the *shape* of the answer is the fact — so this carries the timestamp because a real one does,
/// not because anything looks for it.
fn terminating() -> String {
    r#"{"apiVersion":"v1","kind":"Pod","metadata":{"name":"web-7d9f4",
       "namespace":"payments","deletionTimestamp":"2026-09-04T20:15:49Z",
       "deletionGracePeriodSeconds":30,"finalizers":["example.com/termination"]},
       "spec":{},"status":{"phase":"Running"}}"#
        .to_string()
}

/// The `DELETE` a delete sends, as the stub logs it: the target, and the body under it.
///
/// **Exact and not a `contains`, unlike § WHAT THE PASS PUTS ON THE WIRE's own rule** — that
/// region asserts one parameter at a time because it is about [`Pass`], and this is about the
/// request the operation made. `restart_call` above has the same shape for the same reason, and a
/// later box that adds `preconditions` to the body *should* turn this red: it changes what
/// `kubectl delete` with no flags is equivalent to.
fn delete_call(path: &str) -> String {
    format!(r#"DELETE {path} {{"propagationPolicy":"Background"}}"#)
}

/// **What `delete` can be pointed at: everything, and there is no matrix beside it**
/// (NOTES § D225 ruling 3).
///
/// **`main.rs`'s `KINDS` is read, not copied**, for [`scalable`]'s reason — and here the assertion
/// is the opposite one: every kind the driver can name is *served*, so a `deletable()` that
/// refused any of them would be a red build.
///
/// **The consequence text is asserted per kind and comes from `screens/dialogs.md` § Delete**, not
/// from what [`removal`] returned. Six kinds, six sentences, and the pod's hedge is the
/// replicaset's word for word because k8rs has read no `ownerReferences`.
#[test]
fn delete_takes_every_kind_the_driver_can_name_and_refuses_only_a_word_that_names_none() {
    let expected = [
        (
            "deployment",
            "apps",
            "deployments",
            true,
            "This asks the cluster to remove the deployment and every copy of the app it \
             runs. k8rs has not read what may be attached to it, and something there may delay \
             this or act first — left alone, nothing is left running.",
        ),
        (
            "statefulset",
            "apps",
            "statefulsets",
            true,
            "This asks the cluster to remove the statefulset and every copy of the app it \
             runs. k8rs has not read what may be attached to it, and something there may delay \
             this or act first — left alone, nothing is left running.",
        ),
        (
            "daemonset",
            "apps",
            "daemonsets",
            true,
            "This asks the cluster to remove the daemonset and the copy of the app it runs on \
             every node. k8rs has not read what may be attached to it, and something there may \
             delay this or act first — left alone, nothing is left running.",
        ),
        (
            "replicaset",
            "apps",
            "replicasets",
            true,
            "This removes the replicaset and every pod it manages. Whatever created it will \
             normally replace it — k8rs has not checked whether anything did.",
        ),
        (
            "pod",
            "",
            "pods",
            true,
            "This removes the pod. Whatever created it will normally replace it — k8rs has not \
             checked whether anything did.",
        ),
        (
            "node",
            "",
            "nodes",
            false,
            "This asks the cluster to remove its record of node-3, not the machine. Something \
             attached to it, unread by k8rs, may delay this or act first. Left alone, its pods \
             are deleted and the machine keeps running until its kubelet restarts.",
        ),
    ];
    let mut served = 0;
    for kind in &crate::KINDS {
        let kind = kind.singular;
        let (_, group, plural, namespaced, consequence) = expected
            .iter()
            .find(|(named, ..)| *named == kind)
            .unwrap_or_else(|| {
                panic!("{kind} is a kind the driver can name and this table cannot")
            });
        served += 1;
        let (resource, said, scope) =
            removal(kind, "node-3").unwrap_or_else(|refusal| panic!("{kind}: {refusal}"));
        println!(
            "{kind} → {}/{} {} · namespaced {scope}\n{said}",
            resource.group, resource.version, resource.plural
        );
        assert_eq!(
            (
                resource.group.as_str(),
                resource.version.as_str(),
                resource.plural.as_str()
            ),
            (*group, "v1", *plural),
            "{kind} did not resolve to the resource its own type declares"
        );
        assert_eq!(
            scope, *namespaced,
            "{kind} is on the wrong side of the namespaced/cluster-wide split, so its request \
             path is built the wrong way"
        );
        assert_eq!(
            said, *consequence,
            "{kind}'s consequence is not the one `screens/dialogs.md` § Delete draws"
        );
    }
    // **The derived list says what it found** — an empty `KINDS`, or a renamed entry, would pass
    // every assertion above by running none of them.
    assert_eq!(
        (served, expected.len()),
        (crate::KINDS.len(), crate::KINDS.len()),
        "the driver's kind table and this one no longer name the same six kinds"
    );
    // **The only refusal `delete` has is for a word that names no kind at all** — and it names
    // every kind it does serve, in plain words (invariant 14).
    let refusal = removal("configmap", "web").expect_err("a word `delete` cannot address");
    println!("{refusal}");
    assert_eq!(
        refusal,
        "k8rs cannot delete a configmap — k8rs deletes a deployment, a statefulset, a daemonset, \
         a replicaset, a pod and a node"
    );
    // **A kind word out of argv is free text, and it is quoted back only where the strip left it
    // alone** ([`a_kind`], NOTES § D224) — the identical exposure [`scalable`] and [`restartable`]
    // have, which is why this function reads the same helper and not its own.
    for crafted in ["", "\u{202e}", "pod\n", "po\u{0}d", "p\u{200b}od"] {
        let refusal = removal(crafted, "web").expect_err("a kind word that is not one of the six");
        println!("{crafted:?}\n{refusal}");
        assert!(
            refusal.starts_with("k8rs cannot delete that kind —"),
            "{crafted:?} was quoted back into a sentence it did not survive: {refusal:?}"
        );
    }
}

/// **Every consequence this file produces is in `screens/dialogs.md` § Delete, word for word** —
/// the two files are one text and the screen is the one that owns it (invariant 14).
///
/// **It is mechanical because the copies drifted twice in one day.** The finalizer round rewrote
/// four of the six on 2026-09-04 and a hand-updated `ops.rs` is exactly the second copy
/// NOTES § D103 is named for; the test above asserts the *strings* and this asserts they are the
/// screen's. Read together they say: six sentences, in the file that draws them, reaching the
/// dialog unchanged.
///
/// **The screen is unwrapped before it is searched** — a mockup breaks a sentence across box rows
/// with `│` and padding around it, and the bullets wrap at the margin — so both sides are
/// collapsed to single-spaced words and the frame characters dropped. **Only the node's name is
/// substituted**, because that arm interpolates one and the mockup draws `node-3`.
///
/// **The derived list asserts it found something**: an unreadable or renamed screen file fails as
/// *vetted nothing* rather than passing by matching none of them.
#[test]
fn every_consequence_is_the_sentence_screens_dialogs_md_draws_for_that_kind() {
    let path = format!("{}/screens/dialogs.md", env!("CARGO_MANIFEST_DIR"));
    let drawn = std::fs::read_to_string(&path).unwrap_or_else(|failed| {
        panic!("{path} is the screen these sentences come from: {failed}")
    });
    let flattened = |text: &str| {
        text.chars()
            .map(|character| match character {
                '│' | '┌' | '┐' | '└' | '┘' | '─' | '├' | '┤' | '*' | '`' => ' ',
                other => other,
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let drawn = flattened(&drawn);
    let mut checked = 0;
    for kind in &crate::KINDS {
        let (_, said, _) = removal(kind.singular, "node-3").expect("every kind is served");
        let said = flattened(&said);
        checked += 1;
        assert!(
            drawn.contains(&said),
            "{}'s consequence is not the one screens/dialogs.md § Delete draws:\n{said}",
            kind.singular
        );
    }
    assert_eq!(
        checked,
        crate::KINDS.len(),
        "no kind was compared, so this test vetted nothing"
    );
    // **And the sentence that answers them.** Four of the six say *may delay this or act first*;
    // [`verdict`]'s [`Outcome::Started`] is what says whether it did, and the two share the word
    // rather than being two people's wording for one idea.
    assert!(
        verdict(&Outcome::Started).contains("delaying"),
        "the closing sentence stopped using the consequence's own word for the same idea: {}",
        verdict(&Outcome::Started)
    );
    assert!(
        drawn.contains("may delay this or act first"),
        "the screen stopped saying a removal may be delayed, so the closing sentence answers a \
         question the dialog no longer asks"
    );
}

/// **One request, and it is the real one** — the whole of NOTES § D225 ruling 1, seen from the
/// wire.
///
/// **The count is the assertion.** `scale` and `restart` each send two and this sends one, so a
/// `checkable` that flipped to `true` would put a live `DELETE` on the cluster before anybody had
/// typed a name — and the audit records of the two would be indistinguishable at the `Metadata`
/// level most clusters run.
///
/// **The body is exact** ([`delete_call`]): `propagationPolicy: Background` is what makes the
/// taught line's *no flag* equivalent to what k8rs sent (ruling 5), and a `dryRun` in there would
/// be a delete that is confirmed and never made.
#[tokio::test]
async fn a_delete_sends_exactly_one_request_and_it_is_the_change_itself() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), gone())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = delete(
        &client,
        &deleting(),
        stamp,
        &mut sink,
        shows(&trace),
        types(&trace, "web-7d9f4"),
    )
    .await
    .expect("a pod in a namespace, with a cluster that answers");

    let requests = sent.lock().expect("the log is never poisoned").clone();
    println!("{}", requests.join("\n"));
    assert_eq!(
        requests,
        vec![delete_call("/api/v1/namespaces/payments/pods/web-7d9f4?")],
        "a delete sent something other than exactly one real DELETE"
    );
    assert!(
        !requests[0].contains("dryRun"),
        "the one call a delete makes asked for a dry run, so nothing was deleted: {}",
        requests[0]
    );
    assert_eq!(
        done,
        Performed {
            outcome: Some(Outcome::Done),
            recorded: true
        }
    );
    assert_eq!(done.plainly(), "the change was made");
    assert!(done.changed(), "a delete that landed is not an exit 0");
}

/// **What the dialog and the audit log were given** — the consequence for the kind, the object as
/// the reader knows it, the kubectl line with no flag on it, the path the request really took, and
/// the verdict of a check that was never run.
///
/// **`uid not read` and `resourceVersion not sent` are the record being honest** about a mutation
/// that read nothing first (NOTES § D225 ruling 4).
#[tokio::test]
async fn what_a_delete_records_is_the_call_it_made_and_the_check_it_never_ran() {
    let (client, _) = stub(|_| ("200 OK".to_string(), gone())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = delete(
        &client,
        &deleting(),
        stamp,
        &mut sink,
        shows(&trace),
        types(&trace, "web-7d9f4"),
    )
    .await
    .expect("a pod in a namespace, with a cluster that answers");
    assert_eq!(done.outcome, Some(Outcome::Done));

    assert_eq!(
        trace.borrow().dialog,
        Some(Dialog {
            object: "pod/web-7d9f4".to_string(),
            namespace: Some("payments".to_string()),
            consequence: "This removes the pod. Whatever created it will normally replace it — \
                          k8rs has not checked whether anything did."
                .to_string(),
            // **`pod/web-7d9f4`, and no flag at all** — no `--dry-run`, because none was run, and
            // no `--cascade`, because `Background` is what `kubectl delete` sends when none is
            // given (NOTES § D225 ruling 5).
            kubectl: "kubectl delete pod/web-7d9f4 -n payments".to_string(),
        }),
        "the dialog was not given the object, what a delete does to it, or a runnable kubectl line"
    );
    assert_eq!(
        trace.borrow().verdict.as_deref(),
        Some("k8rs did not check this one with the cluster first"),
        "a delete told the reader something had been checked"
    );
    // **Invariant 2's typed name, chosen by the operation and not by the dialog**
    // (NOTES § D225 ruling 2). It is the object's *own* name and not `pod/web-7d9f4`: what the
    // reader is asked for is what `screens/dialogs.md` § Delete's field holds.
    assert_eq!(
        trace.borrow().asks,
        Some(Some("web-7d9f4".to_string())),
        "a delete let a press confirm it, which is the ctrl-key-slip guard gone"
    );
    let lines = transcript(&trace);
    let attempt = lines
        .iter()
        .find(|line| line.contains("attempt ·"))
        .expect("the attempt line is written before anything is sent");
    assert_eq!(
        attempt,
        "audit: 2026-09-03T12:34:56Z attempt · pod/web-7d9f4 · context kind-k8rs · server \
         https://k8rs-tests.invalid:41751 · namespace payments · uid not read · kubectl: kubectl \
         delete pod/web-7d9f4 -n payments · call: DELETE \
         /api/v1/namespaces/payments/pods/web-7d9f4 · resourceVersion not sent\n",
        "the attempt line does not name the call that was actually made"
    );
    let result = lines
        .iter()
        .find(|line| line.contains("result ·"))
        .expect("the result line is written when the call returns");
    assert!(
        result.contains("dry-run: k8rs did not check this one with the cluster first")
            && result.contains("· the change was made"),
        "the result line does not say the delete was unchecked and made: {result:?}"
    );
}

/// **A cluster that only *accepted* the removal is not told as one that finished it**
/// (`k8s-admin`, 2026-09-04, measured on a Node carrying a finalizer and on a pod inside its
/// grace period — both `200 OK`, both still listed seconds later).
///
/// **Three records said *the change was made* and the object was still there**: the sentence the
/// operator reads, the audit result line, and the exit code — invariant 4's *neither record may
/// lie*, over the one thing a delete is about.
///
/// **The exit code deliberately does not move.** `deletionTimestamp` is set, so the cluster *did*
/// change, and a `2` here would send a script back to re-run a delete that already landed
/// (NOTES § D220 ruling 1). What was wrong is the sentence.
///
/// **Nothing reads `deletionTimestamp`.** The shape of the answer decides it — a `Status` is
/// *gone*, the object is *going* — which is why `delete` maps to one `bool` inside its closure and
/// holds no object ([`Landing`], NOTES § D223 ruling 3).
#[tokio::test]
async fn a_delete_the_cluster_accepted_but_has_not_finished_says_so_and_still_exits_zero() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), terminating())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = delete(
        &client,
        &deleting(),
        stamp,
        &mut sink,
        shows(&trace),
        types(&trace, "web-7d9f4"),
    )
    .await
    .expect("a pod in a namespace, with a cluster that answers");

    println!("{}", done.plainly());
    assert_eq!(
        done.outcome,
        Some(Outcome::Started),
        "a removal the cluster had not finished was recorded as one it had"
    );
    assert!(
        done.changed(),
        "a delete the cluster accepted exited non-zero, so a script re-runs a delete that landed"
    );
    assert_eq!(
        done.plainly(),
        "the cluster accepted this and the object is still there — something is delaying the \
         removal, and the command above waits for that where k8rs does not",
        "the operator was not told the object is still there"
    );
    // **The taught line and k8rs differ on exactly this case, and the sentence says which way.**
    // `kubectl delete` waits by default (`--wait=true`); `timeout 5 kubectl delete node/…` exited
    // 124 still waiting where k8rs returned at once (`k8s-admin`, 2026-09-04).
    assert!(
        done.plainly().contains("the command above waits for that"),
        "the sentence does not say how the command it just taught behaves differently: {}",
        done.plainly()
    );
    let lines = transcript(&trace);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("· the cluster accepted this and the object is still there")),
        "the audit line claims a removal that has not happened: {lines:?}"
    );
    assert_eq!(
        sent.lock().expect("the log is never poisoned").len(),
        1,
        "an unfinished delete was followed by a second request"
    );
}

/// **And a cluster that says the object is gone is still `Done`** — the positive beside the row
/// above, over the same operation and the same stub with the other half of `Either<K, Status>`.
///
/// **It is the `Status` shape that decides it and not the status code**: both answers are
/// `200 OK`, which is why a test keyed on the code could not tell them apart and neither could
/// `.map(|_| ())`.
#[tokio::test]
async fn a_delete_the_cluster_finished_is_the_one_that_says_the_change_was_made() {
    let (client, _) = stub(|_| ("200 OK".to_string(), gone())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = delete(
        &client,
        &deleting(),
        stamp,
        &mut sink,
        shows(&trace),
        types(&trace, "web-7d9f4"),
    )
    .await
    .expect("a pod in a namespace, with a cluster that answers");

    println!("{}", done.plainly());
    assert_eq!(done.outcome, Some(Outcome::Done));
    assert_eq!(done.plainly(), "the change was made");
    assert!(done.changed());
}

/// **A node is the first cluster-scoped object k8rs mutates** (NOTES § D225 ruling 3) — a path
/// with no namespace segment, a taught line with no `-n`, and a consequence that names the machine
/// it is *not* removing.
///
/// **The attempt line says `cluster-wide`**, which is [`gap`]'s word: an object in no namespace
/// and a record that was cut off are not the same thing to a reader.
#[tokio::test]
async fn a_node_is_deleted_cluster_wide_and_every_record_of_it_names_no_namespace() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), gone())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let node = Deleting {
        kind: "node",
        name: "node-3",
        namespace: None,
        ..deleting()
    };

    let done = delete(
        &client,
        &node,
        stamp,
        &mut sink,
        shows(&trace),
        types(&trace, "node-3"),
    )
    .await
    .expect("a node belongs to the whole cluster and needs no namespace");
    assert_eq!(done.outcome, Some(Outcome::Done));

    let requests = sent.lock().expect("the log is never poisoned").clone();
    println!("{}", requests.join("\n"));
    assert_eq!(
        requests,
        vec![delete_call("/api/v1/nodes/node-3?")],
        "a node's delete did not go to the cluster-wide path"
    );
    assert_eq!(
        trace.borrow().dialog,
        Some(Dialog {
            object: "node/node-3".to_string(),
            namespace: None,
            consequence: "This asks the cluster to remove its record of node-3, not the \
                          machine. Something attached to it, unread by k8rs, may delay this or \
                          act first. Left alone, its pods are deleted and the machine keeps \
                          running until its kubelet restarts."
                .to_string(),
            kubectl: "kubectl delete node/node-3".to_string(),
        }),
        "a node's dialog carried a namespace, a `-n`, or somebody else's consequence"
    );
    let lines = transcript(&trace);
    assert!(
        lines.iter().any(|line| line.contains(
            "attempt · node/node-3 · context kind-k8rs · server \
             https://k8rs-tests.invalid:41751 · cluster-wide · uid not read"
        )),
        "a cluster-scoped delete left a dangling namespace label: {lines:?}"
    );
}

/// **A delete nobody confirmed sends nothing at all** — the one thing no other operation in this
/// file can demonstrate, because for `scale` and `restart` the check has already gone out by the
/// time anybody is asked (NOTES § D225 ruling 1).
///
/// **Which is the whole argument for declining the preflight**, stated as a test: a cancelled
/// delete sends no `DELETE`, so the cluster's own audit record cannot confuse it with one that
/// happened.
///
/// **What it does not say is *no trace*, which was false of the one kind `delete` alone reaches**
/// (`tester`, 2026-09-04). A node takes no `-n`, so `k8s::connect_with` has no namespace to scope
/// with and `k8s::coverage` sends a cluster-wide `GET /api/v1/pods?&limit=1` probe before the
/// operation runs — measured at three of them for three cancelled node deletes. That is a read and
/// not a mutation, it is `k8s.rs`'s and that file froze at Phase 6, and `delete` is merely the
/// first operation with a line that can reach it. Recorded in `backlog.md`; what this test claims
/// is what it can see, which is the socket under this operation.
#[tokio::test]
async fn a_delete_nobody_confirmed_puts_nothing_on_the_wire_at_all() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), gone())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = delete(
        &client,
        &deleting(),
        stamp,
        &mut sink,
        shows(&trace),
        asked(&trace, Answer::Cancelled),
    )
    .await
    .expect("a pod in a namespace, with a cluster that answers");

    assert_eq!(done.outcome, Some(Outcome::Cancelled));
    assert!(
        sent.lock().expect("the log is never poisoned").is_empty(),
        "a cancelled delete reached the cluster, so its record cannot tell the two apart"
    );
    assert_eq!(
        done.plainly(),
        "nobody confirmed it, so nothing was changed"
    );
}

/// **A delete the cluster refused is `Outcome::Failed` and never `NotSent`**, because there was no
/// check to stop it (`screens/dialogs.md` § Delete, *"Where delete's unhappy paths differ"*).
///
/// **The sentence is the fault's and the cluster's own words come after it.** A reader of
/// `scale`'s refusal is told the *check* stopped it; a reader of this one has to be told the real
/// delete was sent and refused, or the two records disagree with the apiserver's.
#[tokio::test]
async fn a_delete_an_rbac_role_refuses_says_the_real_call_was_the_one_that_failed() {
    let (client, sent) = stub(|_| {
        (
            "403 Forbidden".to_string(),
            // **One line, because a newline inside a JSON string value is not JSON** — the
            // message is the shape a real apiserver sends and the wrapping is not.
            concat!(
                r#"{"kind":"Status","apiVersion":"v1","status":"Failure","code":403,"#,
                r#""reason":"Forbidden","message":"pods \"web-7d9f4\" is forbidden: User "#,
                r#"\"jane\" cannot delete resource \"pods\" in API group \"\" in the "#,
                r#"namespace \"payments\""}"#
            )
            .to_string(),
        )
    })
    .await;
    let trace = trace();
    let mut sink = Sink(trace.clone());

    let done = delete(
        &client,
        &deleting(),
        stamp,
        &mut sink,
        shows(&trace),
        types(&trace, "web-7d9f4"),
    )
    .await
    .expect("a refusal from the cluster is an outcome and not a refused request");

    println!("{}", done.plainly());
    assert!(
        matches!(
            done.outcome,
            Some(Outcome::Failed {
                fault: Fault::Refused,
                ..
            })
        ),
        "a refused delete was not recorded as the real call failing: {:?}",
        done.outcome
    );
    assert!(
        done.plainly()
            .starts_with("nothing was changed — the cluster would not allow it: pods "),
        "the operator was not told the real call was refused, in the cluster's own words: {}",
        done.plainly()
    );
    assert!(!done.changed(), "a refused delete exited 0");
    assert_eq!(
        sent.lock().expect("the log is never poisoned").len(),
        1,
        "a delete the cluster refused sent something else as well"
    );
    let lines = transcript(&trace);
    assert!(
        lines.iter().any(
            |line| line.contains("dry-run: k8rs did not check this one with the cluster first")
        ),
        "a delete that never checked recorded a check: {lines:?}"
    );
}

/// **The namespace refusal inverts for a node, and both halves are refused before anything is
/// sent** (NOTES § D225 ruling 3).
///
/// **Two sentences and not one**, because they are two different things to go and fix: a
/// namespaced object with no namespace, and a namespace named for an object that is in none.
#[tokio::test]
async fn a_delete_refuses_a_namespace_that_is_missing_and_one_that_should_not_be_there() {
    for (deleting, expected) in [
        (
            Deleting {
                namespace: None,
                ..deleting()
            },
            "k8rs will not delete pod/web-7d9f4 without being told which namespace it is in",
        ),
        (
            Deleting {
                kind: "node",
                name: "node-3",
                namespace: Some("payments"),
                ..deleting()
            },
            "k8rs will not delete node/node-3: a node belongs to the whole cluster and is in no \
             namespace",
        ),
    ] {
        let (client, sent) = stub(|_| ("200 OK".to_string(), gone())).await;
        let trace = trace();
        let mut sink = Sink(trace.clone());

        let refusal = delete(
            &client,
            &deleting,
            stamp,
            &mut sink,
            shows(&trace),
            // Never reached: every row here is refused before a dialog opens.
            types(&trace, "web-7d9f4"),
        )
        .await
        .expect_err("a namespace that does not match the kind is not something to delete");

        println!("{refusal}");
        assert_eq!(refusal, expected);
        assert!(
            sent.lock().expect("the log is never poisoned").is_empty(),
            "a refused delete sent something"
        );
        assert!(
            transcript(&trace).is_empty(),
            "a request k8rs never described wrote an audit line (NOTES § D221)"
        );
    }
}

/// **A name that would change the address the request goes to is refused where the path is
/// built** — [`scale`]'s and [`restart`]'s guard, over the one operation whose namespace may
/// legitimately be `None`.
///
/// **The node rows are the ones neither sibling can cover**: a cluster-scoped path is built from
/// the name alone, so a name that escapes it has no namespace guard in front of it at all.
#[tokio::test]
async fn a_name_that_would_rewrite_a_deletes_request_path_is_refused_where_it_is_built() {
    for (kind, name, namespace, which) in [
        (
            "pod",
            "web/../../secrets",
            Some("payments"),
            "an object's own name",
        ),
        (
            "pod",
            "web",
            Some("payments/../kube-system"),
            "the name of a namespace",
        ),
        ("pod", "", Some("payments"), "an object's own name"),
        ("pod", "web", Some(""), "the name of a namespace"),
        ("node", "../../secrets", None, "an object's own name"),
        ("node", "", None, "an object's own name"),
    ] {
        let (client, sent) = stub(|_| ("200 OK".to_string(), gone())).await;
        let trace = trace();
        let mut sink = Sink(trace.clone());
        let crafted = Deleting {
            kind,
            name,
            namespace,
            ..deleting()
        };

        let refusal = delete(
            &client,
            &crafted,
            stamp,
            &mut sink,
            shows(&trace),
            types(&trace, "web-7d9f4"),
        )
        .await
        .expect_err("a name k8rs will not put in a request path is not something to delete");

        println!("{refusal}");
        assert!(
            refusal.contains(which) && refusal.starts_with("k8rs will not send a change to"),
            "{name:?} in {namespace:?} was not refused as {which}: {refusal:?}"
        );
        assert!(
            sent.lock().expect("the log is never poisoned").is_empty(),
            "a delete with an unusable address sent something"
        );
    }
}

/// **A word that names no kind is refused inside the operation, before anything is sent** —
/// [`scalable`]'s and [`restartable`]'s position, over the one operation that has no matrix.
#[tokio::test]
async fn a_delete_pointed_at_a_word_that_names_no_kind_sends_nothing() {
    let (client, sent) = stub(|_| ("200 OK".to_string(), gone())).await;
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let unknown = Deleting {
        kind: "configmap",
        ..deleting()
    };

    let refusal = delete(
        &client,
        &unknown,
        stamp,
        &mut sink,
        shows(&trace),
        types(&trace, "web-7d9f4"),
    )
    .await
    .expect_err("a word that names no kind is not something to delete");

    println!("{refusal}");
    assert!(
        refusal.starts_with("k8rs cannot delete a configmap"),
        "{refusal:?}"
    );
    assert!(
        sent.lock().expect("the log is never poisoned").is_empty(),
        "a delete of a kind k8rs cannot address sent something"
    );
}

// --- THE CONFIRMATION THAT CANNOT BE FAKED ---
//
// **Invariant 2's *deletes additionally require typing the object name*, as a type**
// (NOTES § D225 ruling 2). `Answer::Confirmed` carries an [`Agreed`] whose field is private, so
// the only two things that can build one are [`Checked::pressed`] and [`Checked::typed`] — and
// each refuses the requirement that is not its own.
//
// **The negatives are the box.** A press-only dialog over a `Confirm::Type` mutation is exactly
// the defect this replaced, and it is now a cancelled mutation with nothing on the wire rather
// than a review finding.

/// **What one answer did to the real contract** — [`perform`] driven with a chosen requirement and
/// a chosen dialog, answering whether the change went out.
///
/// **The steps are the assertion and not the [`Answer`]**: *the real call was made* is what
/// invariant 2 is about, and a test comparing two `Answer` values cannot see it.
async fn answered(
    confirm: Confirm<'_>,
    dialog: impl FnOnce(&Checked<()>) -> Answer,
) -> (Option<Outcome>, Vec<String>) {
    let trace = trace();
    let mut sink = Sink(trace.clone());
    let mutation = Mutation {
        confirm,
        ..scaling()
    };
    let done = performed(
        &mutation,
        stamp,
        &mut sink,
        |_: &Shown<'_>| {},
        |checked| std::future::ready(dialog(&checked)),
        works(&trace),
    )
    .await;
    let steps = trace.borrow().steps.clone();
    println!("{confirm:?} → {:?} · {steps:?}", done.outcome);
    (done.outcome, steps)
}

/// **The two dialogs that match their mutation confirm it, and nothing else does**
/// (NOTES § D225 ruling 2, invariant 2).
///
/// **The off-diagonal is two `#[should_panic]` tests below**, and that is [`Record::of`]'s own
/// shape one region up: a dialog asking the wrong question is the *author's* error, so it is an
/// assertion where one can be made and [`Answer::Cancelled`] where one cannot
/// (`k8s-admin`, 2026-09-04). Silent was wrong — a `ctrl-d` wired to a press-only dialog would
/// ship, never work, and leave an audit trail saying *nobody confirmed it* about an operator who
/// pressed the button.
#[tokio::test]
async fn a_dialog_that_asks_what_its_mutation_requires_is_the_one_that_confirms_it() {
    let (outcome, steps) = answered(Confirm::Press, Checked::pressed).await;
    assert_eq!(
        outcome,
        Some(Outcome::Done),
        "a press did not confirm a press"
    );
    assert!(steps.contains(&"call".to_string()));

    let (outcome, steps) = answered(Confirm::Type("web-7d9f4"), |checked| {
        checked.typed("web-7d9f4")
    })
    .await;
    assert_eq!(
        outcome,
        Some(Outcome::Done),
        "the object's own name did not confirm a delete"
    );
    assert!(steps.contains(&"call".to_string()));
}

/// **A press-only dialog over a mutation that wants a name is the author's error, and it is
/// loud** (`k8s-admin`, 2026-09-04) — [`Record::of`]'s `debug_assert!` shape, in the same region.
///
/// **The release behaviour is [`Answer::Cancelled`] and is the safe direction**; what this asserts
/// is that it does not pass in silence while somebody ships the dialog.
#[tokio::test]
#[should_panic(expected = "requires the object's name")]
async fn a_press_only_dialog_on_a_mutation_that_wants_a_name_is_an_author_error() {
    let _ = answered(Confirm::Type("web-7d9f4"), Checked::pressed).await;
}

/// **And the same facing the other way**: a dialog asking for a name where a press confirms.
///
/// **Only the *requirement* half is asserted, never the typing.** A name that does not match is
/// the operator typing something else and is an ordinary [`Answer::Cancelled`]
/// ([`Checked::typed`]).
#[tokio::test]
#[should_panic(expected = "a press confirms")]
async fn a_typed_name_dialog_on_a_mutation_a_press_confirms_is_an_author_error() {
    let _ = answered(Confirm::Press, |checked| checked.typed("web-7d9f4")).await;
}

/// **What has to be typed, and what does not do** (invariant 2, `screens/dialogs.md` § Delete's
/// ctrl-key-slip guard).
///
/// **`yes` is the row that matters**: a script that says yes to everything cannot delete anything.
/// The empty rows are the guard [`Checked::typed`] took over from `src/main.rs`'s `ask`, where it
/// was one caller's rather than every caller's.
#[tokio::test]
async fn a_typed_confirmation_takes_the_object_name_and_nothing_that_merely_looks_like_it() {
    for (wanted, typed, confirms) in [
        ("web-7d9f4", "web-7d9f4", true),
        ("web-7d9f4", "yes", false),
        ("web-7d9f4", "web", false),
        ("web-7d9f4", "web-7d9f5", false),
        ("web-7d9f4", "WEB-7D9F4", false),
        ("web-7d9f4", " web-7d9f4 ", false),
        ("web-7d9f4", "", false),
        ("", "", false),
        ("", "yes", false),
    ] {
        let (outcome, _) = answered(Confirm::Type(wanted), |checked| checked.typed(typed)).await;
        assert_eq!(
            outcome,
            Some(if confirms {
                Outcome::Done
            } else {
                Outcome::Cancelled
            }),
            "typing {typed:?} against {wanted:?} was read the wrong way"
        );
    }
}

/// **The name the reader has to type is the one the dialog showed them, and not the one the API
/// sent** (invariant 9, NOTES § D213).
///
/// **[`Record::of`] strips the requirement like every other field**, so a name carrying a
/// right-to-left override reaches [`Checked::asks`] cleaned. Typing the raw bytes therefore
/// confirms nothing — which is right: what the person can see and copy is the stripped title, and
/// a comparison against the raw name would be a field nobody can satisfy.
#[tokio::test]
async fn a_confirmation_compares_against_the_name_the_dialog_showed_and_not_the_one_the_api_sent() {
    let raw = "web\u{202e}gnp";
    let shown = "webgnp";
    let (outcome, _) = answered(Confirm::Type(raw), |checked| {
        assert_eq!(
            checked.asks(),
            Some(shown),
            "the dialog was asked for a name that had not been through the strip"
        );
        checked.typed(shown)
    })
    .await;
    assert_eq!(outcome, Some(Outcome::Done));

    let (outcome, _) = answered(Confirm::Type(raw), |checked| checked.typed(raw)).await;
    assert_eq!(
        outcome,
        Some(Outcome::Cancelled),
        "the raw API name confirmed a mutation whose dialog showed the stripped one"
    );
}

/// **A press-only mutation tells the dialog it needs no name** — [`Checked::asks`] is what a
/// dialog reads to know which question to ask, and getting it wrong is the row above.
#[tokio::test]
async fn what_the_dialog_is_told_to_ask_for_is_the_mutations_own_requirement() {
    let (outcome, _) = answered(Confirm::Press, |checked| {
        assert_eq!(
            checked.asks(),
            None,
            "a mutation that wants a press told the dialog to ask for a name"
        );
        checked.pressed()
    })
    .await;
    assert_eq!(outcome, Some(Outcome::Done));
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

    let done = performed(
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
            std::future::ready(checked.pressed())
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
        // **The requirement is a field like every other one and is fed the same shape**
        // (NOTES § D29): a name past its cap is what a caller with a hostile object name hands
        // over, and [`Record::of`] bounds it there rather than at the dialog.
        confirm: Confirm::Type(&long),
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

    let done = performed(
        &scaling(),
        stamp,
        &mut log,
        |_: &Shown<'_>| {},
        |checked: Checked<()>| std::future::ready(checked.pressed()),
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
