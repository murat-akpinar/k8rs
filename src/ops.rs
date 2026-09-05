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

use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::future::Future;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, ResourceRule, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
    SelfSubjectRulesReview, SelfSubjectRulesReviewSpec,
};
use k8s_openapi::api::autoscaling::v1::Scale;
use k8s_openapi::api::core::v1::{Node, Pod};
use k8s_openapi::jiff::Timestamp;
use k8s_openapi::serde_json::{Value, json};
use kube::api::{
    Api, ApiResource, DeleteParams, DynamicObject, Patch, PatchParams, PostParams,
    PropagationPolicy, ValidationDirective,
};
use kube::{Client, Resource};

use crate::k8s::{FREE_TEXT, Fault, IDENTIFIER, fault, namespace_name, object_name, said, text};

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
// anything reaches the cluster, so *this process* crashing mid-call leaves an attempt with no
// result — the honest record. **Flushed and not synced**, so a machine losing power is not the
// crash this covers; the line inside [`perform`] says why that is the right stopping point. If
// that first write fails, nothing is sent at all: a mutation that cannot be
// recorded does not happen. **That rule governs the attempt and not the result**: a result line
// that cannot be written does not un-make a change that has already been made, so [`Performed`]
// carries the outcome *and* whether it was recorded, rather than replacing one with the other.
//
// **An operation can say it sends no check, and the audit line then says so** — D8's verdict
// field recorded honestly rather than omitted. **What [`Mutation::checkable`] is not is a fact
// about the API.** `restart`, `cordon` and `uncordon` are ordinary PATCHes on ordinary paths and
// a real cluster dry-runs all three; what cannot carry the parameter is
// `kube_core::util::Request::restart`/`cordon`/`uncordon`, whose own `PatchParams::default()` is
// built internally (`kube-core-4.2.0/src/util.rs:21-60`) — a client helper's signature, which
// this region read as the API's capability for a round (NOTES § D215). So `false` means k8rs
// *declined* a preflight it could have had, and each operation's own box says why — `scale`
// (todo.md 3687), `restart` (3689), `delete` (3692). This region carries the seam and records
// what happened, and asserts nothing about which way any operation sets it.
//
// **In-flight is this region's already, and it needs nothing added** (NOTES § D232, § D20). Two
// facts, both easy to look for and not find:
//
// **Started is [`Ask`]'s return value, and a `started` callback would be a second copy of it.**
// [`Answer::Confirmed`] carrying this call's ticket is the *one* arm that reaches
// `call(FOR_REAL)` — `Cancelled`, `Gone` and `Changed` each end the mutation with nothing sent —
// so the caller's own `ask` returning it **is** the signal, it arrives exactly when D20 wants the
// modal to close, and it cannot drift from what [`perform`] does because it is what [`perform`]
// branches on (D232 ruling 1).
//
// **Exactly one mutation can be outstanding, and the borrow checker is what says so.** `audit` is
// `&mut impl Write` and k8rs opens one log ([`audit_log`]), so two concurrent [`perform`] calls
// need two `&mut` borrows of one sink and do not compile. That is stronger than a runtime flag
// and it costs nothing — and it is worth writing down for [`Checked`]'s reason: a reader looking
// for the guard D20 asks for finds no guard, and is right to worry until told why there is none
// (D232 ruling 2).
//
// **And the signature can be driven beside an event loop, which was the freeze risk** — proven
// rather than reasoned, in `ops_tests.rs` § THE CALL THAT IS STILL OUT, both pinned to a stack
// and parked in a struct. What a caller may *not* do is own the audit log inside the same struct
// that holds the future: that is a self-reference and `rustc` refuses it (D232 ruling 3).
//
// **A cluster can also refuse every dry-run there is, and it is rarer than this used to read**:
// a `ValidatingWebhookConfiguration` with `sideEffects: Some | Unknown` fails `dryRun=All` for a
// fully authorised user. **On v1.36.1 the API refuses to register one** — *"Unsupported value:
// `Unknown`: supported values: `None`, `NoneOnDryRun`"* — because those two values were settable
// only through `v1beta1`, gone since 1.22, and validation runs on write (`k8s-admin`,
// 2026-09-05). So the shape survives only on a configuration written before 1.22 and never
// rewritten since; it is not something k8rs can meet on a cluster built today. That is accepted
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
    /// **Which cluster that context reached** — `kube::Config::cluster_url`, the `server:` the
    /// call actually went to.
    ///
    /// **A context name does not identify a cluster and the record has to** (`k8s-admin`,
    /// 2026-09-04). `kubeadm` writes `kubernetes-admin@kubernetes` for every cluster it builds,
    /// `kind` names one per cluster but an operator merging two kubeconfigs has two entries with
    /// one string, and a context is renamed freely while the record outlives the kubeconfig it
    /// was written from. At 3am *a deployment was scaled in `payments` on
    /// `kubernetes-admin@kubernetes`* answers nothing; the server URL is the part that is free
    /// today and it is on [`Record::attempt_line`] beside the context.
    ///
    /// **Not the identity that performed it.** k8rs cannot know the effective subject without a
    /// `SelfSubjectReview`, which is `may_i`'s box and not this one — so the record says which
    /// cluster and not who, rather than guessing at the second.
    pub server: &'a str,
    /// The namespace, or `None` for a cluster-scoped object.
    pub namespace: Option<&'a str>,
    /// The object as the reader knows it — `deployment/web`.
    pub object: &'a str,
    /// **The object's `uid`, where the caller read one** — the cluster's own name for *this*
    /// instance, as opposed to whatever holds that name now.
    ///
    /// **It is what makes [`Answer::Gone`] and [`Answer::Changed`] checkable after the fact**
    /// (`k8s-admin`, 2026-09-04): the ReplicaSet replaces the pod while the name is being typed,
    /// k8rs refuses, and *which* pod that was is a question only the `uid` answers — the name is
    /// now somebody else's. `None` where the caller had none to give.
    ///
    /// **On its own it says what k8rs *read* and never what k8rs *changed*, and the attempt line
    /// now says so in those words** (NOTES § D235). Measured on a real cluster: a Deployment
    /// deleted and recreated between the dry-run and the yes left the record naming
    /// `uid 8656c3ec…`, which nothing changed, beside a `PATCH` that landed on a different
    /// instance. The value is honest; the label was not.
    pub uid: Option<&'a str>,
    /// **Whether that `uid` went out as a `preconditions.uid`** — the difference between a value
    /// k8rs read and a value the cluster agreed to (NOTES § D235, invariant 4's *resourceVersion
    /// sent*, which is this class).
    ///
    /// **Only [`delete`] can set it**, because `DeleteParams` is the only params type in this
    /// file with a `preconditions` field — `PatchParams` has none, which is why [`scale`]'s own
    /// case is answered by the label and not by a guard.
    ///
    /// **It is a field and not derived from `uid.is_some()`**, which is true of a `scale` too, nor
    /// from [`Self::verb`], which is free text an operation writes. Both would be the second copy
    /// NOTES § D103 is named for, on the one line invariant 4 says may not lie.
    pub uid_sent: bool,
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
    ///
    /// **No operation sets it today** (NOTES § D228 took [`scale`]'s back out), and v0.4's `edit`
    /// is the first that will — it is a genuine read-modify-write, which is where
    /// [REQUIREMENTS.md](../REQUIREMENTS.md) always put conflict handling.
    ///
    /// **Whatever an operation puts here must survive [`Record::of`]'s strip unchanged, and the
    /// operation owes that guard at the point it reads the value.** This is the one field that
    /// travels both ways — raw in the request body, stripped into the record — so a value the
    /// strip alters makes the two disagree about the field a `409` is argued from. `edit` owes it
    /// in its own box.
    ///
    /// **It is prose here and not a check in [`Record::of`], and that was measured** (`tester`,
    /// 2026-09-05). An assertion there refuses every input where `cleaned(v) != v`, so every value
    /// a test may then supply satisfies `cleaned(v) == v` by construction and nothing can move the
    /// strip: planting its removal went from **three failing tests to none**. It also could not
    /// close the hole, since a `debug_assert` is compiled out of the build the divergence would
    /// ship in. The enforcement belongs where both halves of the invariant are visible — the
    /// operation, which holds the value it is about to send *and* the record it is about to
    /// write — and not here.
    pub version: Option<&'a str>,
    /// **Whether this operation sends a `dryRun=All` check before the change.** When it is
    /// `false` nothing is sent before the confirmation and the audit line records that no check
    /// was run.
    ///
    /// **Not a fact about the API** — a real cluster dry-runs both verbs this file sends,
    /// `PATCH` and `DELETE` (NOTES § D215). `false` means k8rs declined the preflight, and the
    /// reason belongs to the operation's own box.
    pub checkable: bool,
    /// **How invariant 2's confirmation is given for this mutation** — a deliberate yes, or the
    /// object's own name typed back.
    ///
    /// **It is the requirement, and it is *half* of what makes invariant 2's *deletes
    /// additionally require typing the object name* structural** (NOTES § D225 ruling 2).
    /// [`perform`] hands it to [`Checked`], and [`Answer::Confirmed`] can be built nowhere but
    /// [`Checked::pressed`] and [`Checked::typed`], each of which refuses the requirement that is
    /// not its own. So a dialog that never asked for a name cannot *construct* the answer that
    /// proceeds; it gets [`Answer::Cancelled`].
    ///
    /// **The other half is that the answer cannot be moved between mutations**, and this field
    /// alone did not buy it: the first draft's token was `Copy` and carried nothing, so one yes to
    /// a `Press` scale could be kept and returned at a `Type` delete — proven on the wire, not
    /// argued. [`Agreed`] carries [`perform`]'s ticket now and that is what closes it.
    ///
    /// **A `bool` an operation asserts about itself would have bought neither**, which is the
    /// distinction the box proposing this field did not draw: [`Self::checkable`] is exactly that
    /// shape, and what holds it is review.
    pub confirm: Confirm<'a>,
}

/// **What invariant 2 requires of the person at the keyboard before a mutation may proceed** —
/// one of two things, and the operation says which.
///
/// **`Type` is the ctrl-key-slip guard** `screens/dialogs.md` § Delete gives `delete` and `drain`
/// and nothing else: the object's own name, typed back, so a key pressed by accident cannot
/// destroy anything. The value is the name that has to be matched — [`Record::of`] strips it
/// like every other field, and [`Checked::typed`] compares against the stripped copy, which is
/// the one the dialog is looking at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm<'a> {
    /// A deliberate yes and nothing more — [`scale`] and [`restart`].
    Press,
    /// The object's own name, typed back — [`delete`].
    Type(&'a str),
}

/// **One mutation, stripped** — every string on it has been through [`crate::k8s::text`] and is
/// safe to put on a screen or in a record (invariant 9, NOTES § D154, § D213).
struct Record {
    context: String,
    server: String,
    namespace: Option<String>,
    object: String,
    uid: Option<String>,
    consequence: String,
    kubectl: String,
    verb: String,
    path: String,
    version: Option<String>,
    uid_sent: bool,
    checkable: bool,
    /// The name that has to be typed back, or `None` for [`Confirm::Press`] — the enum flattened
    /// to the one thing that is left to compare once it has been stripped.
    confirm: Option<String>,
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
/// **`Response` is whatever the operation made of the call's answer, and it is generic on
/// purpose.** `edit` in v0.4 shows a diff of what the dry-run gave back, and that *is* its
/// confirmation; `ops.rs` freezes at the end of this phase, so the alternative is a second entry
/// point into a frozen file. The channel exists for the verdict either way, and carrying something
/// in it is free.
///
/// **Generic over what the operation *mapped to*, not over the object** (NOTES § D224). The type
/// was written as *the object the call returned* and `restart` is the operation that showed why it
/// should not be: it maps the response down to one `bool` — [`paused`] — inside its closure, so a
/// `Checked<bool>` crosses into the dialog and a Deployment carrying
/// `spec.template.spec.containers[].env[].value` never does. What an operation puts here is a
/// decision it makes per call and not a shape this type imposes.
pub struct Checked<Response> {
    verdict: &'static str,
    returned: Option<Response>,
    asks: Option<String>,
    /// **Which mutation this dialog is about** — [`perform`]'s own [`ticket`], copied into every
    /// [`Agreed`] built here and compared back before the real call.
    ticket: u64,
}

impl<Response> Checked<Response> {
    /// **What the cluster's check said, in the words the dialog prints** — the mockup's *"The
    /// cluster checked it first and accepted it."*, or the sentence for an operation that has no
    /// check to run.
    pub fn verdict(&self) -> &'static str {
        self.verdict
    }

    /// **What the operation made of the dry-run's answer**, or `None` where there was no dry-run
    /// to answer. `scale` puts the `Scale` here; `restart` puts one `bool` (NOTES § D224);
    /// `delete` sends no check at all and puts nothing.
    pub fn returned(&self) -> Option<&Response> {
        self.returned.as_ref()
    }

    /// **The name that has to be typed back before this may proceed**, or `None` where a
    /// deliberate yes is the whole of it — [`Mutation::confirm`], stripped by [`Record::of`].
    ///
    /// A dialog reads this to know which question to ask. It decides nothing: the answer is still
    /// [`Self::pressed`]'s or [`Self::typed`]'s to build, and each of those refuses the
    /// requirement that is not its own.
    pub fn asks(&self) -> Option<&str> {
        self.asks.as_deref()
    }

    /// **The answer a dialog that asked only for a press may give** — and [`Answer::Cancelled`]
    /// where the mutation requires a name, because a press is not one (NOTES § D225 ruling 2).
    ///
    /// This is the half that makes invariant 2 a type's problem rather than a reviewer's: a
    /// press-only delete dialog compiles, calls this, and confirms nothing — loudly in a debug
    /// build, where the assertion below fires, and silently-but-safely in release.
    pub fn pressed(&self) -> Answer {
        // **An author error and not the cluster's, so it is an assertion and not an outcome** —
        // the shape [`Record::of`]'s own `debug_assert!` two screens up already settled for this
        // region (`k8s-admin`, 2026-09-04). The [`Answer::Cancelled`] below is the release
        // behaviour and is the safe direction; what it is not is *visible*, and a `ctrl-d` wired
        // to a press-only dialog would ship, never work, and leave an audit trail saying *nobody
        // confirmed it* about an operator who pressed the button.
        debug_assert!(
            self.asks.is_none(),
            "a press-only dialog was opened on a mutation that requires the object's name \
             (invariant 2)"
        );
        match self.asks {
            Some(_) => Answer::Cancelled,
            None => Answer::Confirmed(Agreed(self.ticket)),
        }
    }

    /// **The answer a dialog that asked for the object's own name may give** — the only route to
    /// [`Answer::Confirmed`] for a mutation that requires one (invariant 2), and
    /// [`Answer::Cancelled`] for one that does not, so the two dialogs cannot be swapped.
    ///
    /// **An empty requirement confirms nothing.** `typed == name` holds for `("", "")`, which is
    /// *typing the object name* satisfied by typing nothing. No argv reaches it —
    /// [`crate::k8s::object_name`] refuses an empty name — but [`Record::of`]'s strip can empty a
    /// name that argv could not, and the guard belongs in the one function every dialog routes
    /// through rather than in each of them (`k8s-admin`, 2026-09-04, on `src/main.rs`'s `ask`,
    /// where it lived until this box moved it one layer down).
    pub fn typed(&self, typed: &str) -> Answer {
        // **[`Self::pressed`]'s assertion, facing the other way** — and only over the half that is
        // the author's. A name that does not match is the *operator* typing something else and is
        // an ordinary [`Answer::Cancelled`]; a dialog asking for a name at all where the mutation
        // wants a press is the author error.
        debug_assert!(
            self.asks.is_some(),
            "a typed-name dialog was opened on a mutation a press confirms (invariant 2)"
        );
        match self.asks.as_deref() {
            Some(name) if !name.is_empty() && typed == name => {
                Answer::Confirmed(Agreed(self.ticket))
            }
            _ => Answer::Cancelled,
        }
    }
}

/// **Proof that the confirmation [`Mutation::confirm`] asked for was actually given, *for one
/// mutation*** — and the second half of that is the whole of it (NOTES § D225 ruling 2).
///
/// **The first draft was forgeable and the doc here said it was not, which is why this paragraph
/// leads.** It was a `Copy` token with no contents. **An enum variant's fields are as public as
/// the enum**, however private the struct's own field is, so any code holding one
/// `Answer::Confirmed` could destructure the token and re-wrap it: press-confirm one scale, keep
/// what falls out, and every later delete proceeds with no name typed. `tester` drove it from
/// `src/main_tests.rs` — where every dialog lives — through the real [`perform`], and got a
/// `DELETE` on the wire. Not reachable through today's driver, which performs one operation per
/// process; reachable at Phase 12, where the console is one process with many dialogs, and this
/// file freezes at the end of this phase.
///
/// **So the token carries [`perform`]'s own ticket**, taken from [`ticket`] when the call begins,
/// stamped in by [`Checked::pressed`] and [`Checked::typed`] from the [`Checked`] they belong to,
/// and compared back before the real call goes out. A token from any *other* mutation — including
/// an earlier one with the same [`Confirm`] — carries a different number and confirms nothing.
///
/// **A number and not a branded lifetime**, which was the other shape offered. A generative
/// lifetime is the stronger tool and it needs the callback to be higher-ranked over a
/// future-returning closure, which is a signature nobody at 3am can read, on the file whose whole
/// design goal is to stay small enough to audit. The ticket is six lines and its failure mode is a
/// comparison this file makes in one place.
///
/// **`Copy` and `Clone` are gone too, and they are not what makes it hold** — a token can be moved
/// out and stored either way, which is exactly what the probe does. What they were was a free
/// second copy for an attacker to keep; the ticket is the mechanism and this is the door shut
/// behind it.
///
/// **None of this is a privacy guarantee *inside* this file, for [`Checked`]'s own reason** — this
/// file is one module. What holds is that every dialog k8rs has is outside it.
#[derive(Debug, PartialEq, Eq)]
pub struct Agreed(u64);

/// **The next mutation's ticket** — a number that has not been handed out before, so that a
/// confirmation belongs to exactly one call of [`perform`].
///
/// **A process-wide counter is the one piece of ambient state in this file, and it is ambient on
/// purpose**: what it has to be is unique across every [`perform`] in the process, which is the
/// thing no argument threaded through one call can promise. It is not a fact about the world, so
/// NOTES § D18's *the clock is an input* does not reach it — nothing observable is read here and
/// nothing is reported from it.
///
/// **`Relaxed` is enough**: the only requirement is that no two `fetch_add`s return the same
/// value, which is the operation's own guarantee and needs no ordering with anything else.
fn ticket() -> u64 {
    static CONFIRMATIONS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    CONFIRMATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// **How the dialog ended** — four endings, because three of them are not "no".
///
/// `bool` collapsed them into one and the audit log said *nobody confirmed it* for all three
/// (`k8s-admin`, 2026-09-04). The ReplicaSet replaces the pod while its name is being typed,
/// k8rs correctly refuses to delete whatever now holds that name, and the record read as *someone
/// opened a delete dialog on prod and backed out* — invariant 4's *neither record may lie*
/// (NOTES § D22, `screens/dialogs.md` § The object went away while the dialog was open).
// **No `Copy` and no `Clone`** ([`Agreed`]): a confirmation is a thing that happened once, and
// the two derives were a free second copy of it for anything that wanted to keep one.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    /// The person at the keyboard agreed — and, where invariant 2 requires it, typed the name.
    ///
    /// **Constructible only through [`Checked::pressed`] or [`Checked::typed`], and only usable
    /// for the mutation whose [`Checked`] built it** ([`Agreed`], which carries [`perform`]'s
    /// ticket). Neither half is enough alone: without the first anyone can say yes, and without
    /// the second one yes can be kept and replayed at a delete. Every other variant stays freely
    /// constructible — they are refusals, and a refusal built too easily changes nothing.
    Confirmed(Agreed),
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
    /// The check passed or was never sent, the confirmation was given, the call succeeded, and
    /// the cluster says it is **finished**.
    Done,
    /// **The cluster accepted the change and has not finished it** — measured on a real
    /// apiserver, on a Node carrying a finalizer and on a pod inside its grace period
    /// (`k8s-admin`, 2026-09-04). Both are `200 OK`, and [`Self::Done`]'s *the change was made*
    /// over one of them is invariant 4's *neither record may lie*: the object is still listed
    /// seconds later.
    ///
    /// **It is still a change**, so [`Performed::changed`] is true of it and the exit code does
    /// not move (NOTES § D220 ruling 1): `deletionTimestamp` is set and the removal is under way.
    /// What was wrong was the sentence.
    ///
    /// **Only an operation whose cluster answer can say so ever produces it**, which today is
    /// `delete` alone — a `PATCH`'s `200` means the patch is applied, so [`scale`] and [`restart`]
    /// have no such case and grow none ([`Landing`]).
    Started,
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

impl Outcome {
    /// **What the server said about this outcome, where it said anything** — one match, read by
    /// the audit line through [`Record::result_line`] and by the operator's own closing sentence
    /// through [`Performed::plainly`], so the record and the screen cannot end up holding
    /// different halves of one failure.
    ///
    /// **Annotated and exhaustive**, so that dropping an arm is a *testable* change rather than
    /// one the compiler refuses for want of a type (NOTES § D133), and so that a seventh outcome —
    /// there are six — has to decide whether it carries words rather than defaulting to silence.
    ///
    /// The string is [`crate::k8s::said`]'s and is not cleaned again here — [`Record::of`]'s own
    /// doc says why.
    fn said(&self) -> Option<&str> {
        match self {
            Self::NotSent { said, .. } | Self::Failed { said, .. } => said.as_deref(),
            Self::Done | Self::Started | Self::Cancelled | Self::Gone | Self::Changed => None,
        }
    }
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

impl Performed {
    /// **Whether the cluster changed** — the one question an exit code may answer
    /// (NOTES § D220 ruling 1).
    ///
    /// **`recorded` is deliberately not part of it.** A `Done` k8rs could not write down still
    /// happened, and a script told otherwise re-runs a mutation that already landed — which
    /// `restart` and `delete` are not idempotent under the way a scale is. The incomplete trail
    /// travels in [`Self::plainly`] instead, which is a sentence and not a code.
    pub fn changed(&self) -> bool {
        // **[`Outcome::Started`] is a change** (`k8s-admin`, 2026-09-04, NOTES § D220 ruling 1's
        // own split): the cluster set `deletionTimestamp` and the removal is under way, so a `2`
        // here would send a script back to re-run a delete that already landed. What that case
        // costs is the *sentence*, not the code.
        matches!(self.outcome, Some(Outcome::Done | Outcome::Started))
    }

    /// **The whole of what happened, in one sentence somebody reads at 3am** (invariant 14) —
    /// the outcome's own words, and the clause that says the trail is short of them.
    ///
    /// **Something reads [`Self::recorded`] now, which is the point** (todo.md 3749): `#[must_use]`
    /// is on the struct and not on the field, so *the change was made and k8rs could not write it
    /// down* — NOTES § D214's fourth lie — reached nobody until a caller said it out loud.
    ///
    /// **The cluster's own words are part of the sentence** (`k8s-admin`, 2026-09-04). The audit
    /// line carried the fault *and* the server's explanation while this surface carried neither,
    /// so a `403` on the `dryRun=All` and a `422` strict rejection printed one identical line —
    /// *the change was never sent* — and the person who cannot go and open the log is exactly the
    /// person reading this. The fault is [`verdict`]'s now and the join is [`and_said`]'s, once, so
    /// this cannot come to spell either of them differently from the record.
    pub fn plainly(&self) -> String {
        // NOTES § D21: the attempt line could not be written, so nothing was sent, nobody was
        // asked, and there is no outcome to report. The words are [`without`]'s, which is what
        // the same machine is told when the log cannot even be opened.
        let Some(outcome) = self.outcome.as_ref() else {
            return "nothing was changed — k8rs could not write this to its audit log first, and \
                    every change k8rs makes is written to that log before it is sent"
                .to_string();
        };
        // The whole sentence first, so the clause below lands after the cluster's own words rather
        // than between them and the fault they belong to.
        let sentence = and_said(verdict(outcome), outcome.said());
        if self.recorded {
            return sentence;
        }
        format!(
            "{sentence} — but k8rs could not write that to the audit log, so the trail of it is \
             short a line"
        )
    }
}

/// **Which of the two passes an operation is being asked to make — and the only thing it is
/// told about it** (invariant 2).
///
/// [`perform`] used to hand the closure a `bool` that an operation was then trusted to turn into
/// `?dryRun=All`: the *"an operation follows a checklist"* shape this whole region exists to
/// remove, and the hole `tester` named — an operation that ignored the flag and sent the real
/// call twice satisfied every test in the file. The `bool` is now behind a private field, so the
/// only thing an operation can do with a `Pass` is ask it for the params of the call it is about
/// to make, and there is **one** conversion from *which pass* to *what goes on the wire* rather
/// than one per operation.
///
/// **What that is not is a privacy guarantee, for the same reason [`Checked`] is not.** This file
/// is one module, so nothing *stops* an operation building its own `PatchParams::default()` on
/// the line below. What holds is narrower and still enough: outside `ops.rs` `clippy.toml`
/// refuses the call at all (invariant 1), and inside it that line is visible in the one file the
/// design exists to keep small enough to read.
///
/// **Two shapes, because a mutation in this phase is a `PATCH` or a `DELETE`.** A third for the
/// eviction v0.2's drain sends was written here and removed: kube's eviction body spells a field
/// the API does not, `PostParams` has no `field_validation` for the next box in this phase to
/// reach for, and [`perform`] does not fit a drain anyway — so `ops.rs` reopens for it whatever
/// is left standing here, and a freeze argument only rescues code the thing after the freeze
/// could use (NOTES § D215).
#[derive(Clone, Copy)]
pub struct Pass(bool);

impl Pass {
    /// Params for a `PATCH`. `scale`'s scale subresource is the only caller written so far;
    /// `restart` (todo.md 3689) reaches here too, because it builds its patch by hand rather
    /// than calling `Api::restart` — that helper hard-codes `PatchParams::default()`, so it can
    /// carry no `dryRun`, and writes `kube.kubernetes.io/restartedAt` where `kubectl` writes
    /// `kubectl.kubernetes.io/restartedAt` (NOTES § D215).
    ///
    /// **`dryRun` rides in the query string here**, appended by `PatchParams::populate_qp` —
    /// which both `Request::patch` and the `Request::patch_subresource` behind `Api::patch_scale`
    /// call (`kube-core-4.2.0/src/request.rs:148` and `:221`). Measured off a built request rather
    /// than read off the field name: the URI ends `…/deployments/web/scale?&dryRun=All`.
    ///
    /// **`fieldValidation=Strict` on both passes, and it is the dry-run's missing half.** A merge
    /// patch carrying a field the cluster does not have answers `200 OK` under `dryRun=All` and
    /// again on the real call: the objection travels only in a `Warning: 299` header that kube's
    /// `Api` methods do not surface, so the check passes, the change passes, and the audit log
    /// records a successful mutation that altered nothing — invariant 4's *neither record may
    /// lie*, broken by the server rather than by us. `Strict` turns both into a `422`, which
    /// NOTES § D213 already classifies as [`Fault::Rejected`]. `populate_qp` appends it for any
    /// patch and not only an apply, whatever kube's own field doc says (`params.rs:705-707`, its
    /// own test at `:908-927`). `DeleteParams` has no counterpart and needs none: a delete sends
    /// `DeleteOptions`, not an object.
    ///
    /// **Both passes and not only the check, because [`Mutation::checkable`] exists.** Where an
    /// operation declines the preflight there *is* no check pass, and a `Strict` that rode only
    /// on [`DRY_RUN`] would leave those writes validated by nothing at all. Each request is
    /// judged on its own by the server, and a verdict on the check is not one k8rs may carry
    /// onto a different request.
    ///
    /// **What a `422` says back is the object, so this widens what one failure can print.** The
    /// apiserver builds it as `field.Invalid(field.NewPath("patch"), string(patchedObjJS), …)` —
    /// the *patched object*, whole and untruncated — and `NewInvalid` folds that bad value into
    /// `Status.message` and not only into `details.causes`
    /// (`apiserver/pkg/endpoints/handlers/patch.go:351-354`,
    /// `apimachinery/pkg/api/errors/errors.go:305-310`,
    /// `apimachinery/pkg/util/validation/field/errors.go:109-154`, release-1.36). For `scale` that
    /// object is an `autoscaling/v1 Scale`, and **what it carries was measured against a real
    /// apiserver rather than read off `storage.go`**
    /// (`reports/2026-09-04-restart-round-two-paused-and-the-scale-422.md` § 4): under
    /// `application/merge-patch+json` — what [`scale`] sends — the `Status.message` is **646
    /// bytes** around a **485-byte** object carrying `name`, `namespace`, `uid`,
    /// `resourceVersion`, `creationTimestamp`, **`managedFields`** (one entry, manager
    /// `kubectl-client-side-apply`), two replica counts and `status.selector`. Under
    /// `application/strategic-merge-patch+json` it is **120** and quotes only the patch. This doc
    /// asserted `managedFields` *absent* off `pkg/registry/apps/deployment/storage/storage.go`
    /// and it is **present**, and `status.selector` went unnamed; no labels, annotations or pod
    /// template, which that source had right.
    ///
    /// **The safety conclusion survives the correction, which is the reason to state it here at
    /// all**: a literal planted in both a container environment value *and* the apply annotation
    /// appeared in **neither** message — so [`crate::k8s::said`]'s existing strip and `FREE_TEXT`
    /// cut are the whole of what is owed here.
    ///
    /// **A patch on the object itself is not that shape, and `restart` has since paid the check
    /// this one does not** (NOTES § D224) — measured against a real apiserver rather than read off
    /// Kubernetes source. On one Deployment carrying a planted container environment value, a
    /// strict rejection answers with **109 bytes** under
    /// `application/strategic-merge-patch+json` — k8rs's own patch, and nothing of the server's
    /// copy — where `application/merge-patch+json` and `application/json-patch+json` on the same
    /// object answer with **4895**, carrying `managedFields`, `containers` and that literal. So
    /// what bounds a workload patch's `422` is its media type, and [`restart`] picks one
    /// (NOTES § D223 ruling 4).
    ///
    /// **What v0.4's `edit` inherits from that is narrower than *the 422 is small***. The
    /// strategic message echoes *the patch that was sent*: a deeper unknown field came back
    /// quoting k8rs's own body, environment value included. Strategic merge protects the operator
    /// from the server's copy of the object and does nothing about the operator's own YAML, which
    /// is exactly where `edit`'s unknown fields come from.
    pub fn patch(self) -> PatchParams {
        PatchParams {
            dry_run: self.0,
            field_validation: Some(ValidationDirective::Strict),
            ..PatchParams::default()
        }
    }

    /// Params for a `DELETE`.
    ///
    /// **`propagationPolicy: Background`, set explicitly, so invariant 4's *equivalent* command
    /// needs no flag** (NOTES § D225 ruling 5). `DeleteParams::default()` leaves
    /// `propagation_policy` `None` (`params.rs:784`) and every other field is
    /// `skip_serializing_if`, so a default pass sends `{}` and lets the server fall back to the
    /// object's own default. Spelled out beside the `dry_run` rather than taken from kube's
    /// `DeleteParams::background()`, for [`Self::patch`]'s reason: what goes on the wire is what
    /// this function's body says, in both halves of the conversion.
    ///
    /// **Measured against a real apiserver rather than read off kubectl's documentation**
    /// (`reports/2026-09-04-delete-on-the-wire.md` § 1-2, kind/kubelet v1.36.1). `kubectl delete`
    /// sends exactly `{"propagationPolicy":"Background"}` — 34 bytes, **no query string at all**,
    /// no `kind` and no `apiVersion` — and the body is **byte-identical across all six kinds**
    /// [`removal`] serves, only the path differing for the cluster-scoped one.
    /// `--cascade=background` produces the same bytes, so a taught line with no flag on it is
    /// exact and not merely defensible.
    ///
    /// **What the constant buys is exactness and not a behaviour change, and the doc says so
    /// rather than claiming a fix.** On server v1.36.1 an empty body and `Background` were
    /// indistinguishable by every observation the report took, while `Foreground` was plainly
    /// different: what this removes is a dependence on a per-resource server default.
    ///
    /// **The `uid` is a precondition and not a read** (NOTES § D235). Measured on a real cluster:
    /// a StatefulSet pod whose controller reuses its name had **three** different `uid`s across
    /// one k8rs run, and k8rs deleted one the operator never saw — NOTES § D22's scenario, live,
    /// with the guard the design leans on ([`Answer::Gone`]) switched off in the only shape that
    /// exists today, because a headless script has no watch to notice with. A `uid` that does not
    /// match is a `409` naming both and *"The object might have been deleted and then
    /// recreated"*, measured, with the object surviving.
    ///
    /// **It costs no `GET`, which is why NOTES § D225 ruling 4 is untouched**: `delete` still
    /// reads nothing at all. The value comes from a caller that already has it — Phase 11's dialog
    /// off the watch running behind the modal, which is where NOTES § D22 put it — and the
    /// headless driver passes `None` because it has no watch and may not buy one.
    ///
    /// **This is not NOTES § D228's reversal.** That removed a `resourceVersion` precondition
    /// because the field moves when nothing about the object changed; `metadata.uid` is immutable
    /// and differs only when the object genuinely is a different one, which is the question.
    ///
    /// **These are also what `Api::delete_collection` accepts, and bulk mutation does not
    /// exist** (invariant 2). No caller exists, and outside this file `clippy.toml` refuses the
    /// call — a note to the next reader, not a mechanism.
    ///
    /// **`dryRun` rides in the request *body* here, not the query string.** `DeleteParams` has no
    /// `populate_qp` at all; `Request::delete` builds an empty query and serialises the whole
    /// struct into the body, where a `serialize_with` turns the `bool` into the array the API
    /// wants (`kube-core-4.2.0/src/request.rs:109`, `params.rs:861`). Measured off a built
    /// request: the URI ends `…/deployments/web?` with nothing after it, and the body carries
    /// `"dryRun":["All"]`. Nothing here has to do anything about it; it is written down because
    /// a reviewer grepping a delete's URL for `dryRun` finds nothing and is right to worry.
    pub fn delete(self, uid: Option<&str>) -> DeleteParams {
        DeleteParams {
            dry_run: self.0,
            propagation_policy: Some(PropagationPolicy::Background),
            // **The one precondition k8rs sends, and it is `None` unless a caller had a `uid`
            // to give** (NOTES § D235). `Preconditions::resource_version` stays empty for
            // NOTES § D228's reason and is a different field with a different failure mode.
            preconditions: uid.map(|uid| kube::api::Preconditions {
                uid: Some(uid.to_string()),
                resource_version: None,
            }),
            ..DeleteParams::default()
        }
    }
}

/// **What the cluster's answer to the *real* call says about whether the change has finished** —
/// [`perform`]'s last question, and the one only the operation can answer.
///
/// **It exists because a `200` is not one fact.** `Api::delete` answers a completed removal with a
/// `Status` and an accepted-but-unfinished one with the object itself; a `PATCH` has no such
/// split, so [`scale`] and [`restart`] answer [`Self::Finished`] through [`finished`] and grow no
/// case (`k8s-admin`, 2026-09-04).
///
/// **The operation reads it inside its own closure and hands over a fact, not an object** —
/// NOTES § D223 ruling 3 and § D224's shape, so nothing here holds a workload for a dialog that
/// shows none of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Landing {
    /// The cluster says the change is done — [`Outcome::Done`].
    Finished,
    /// The cluster accepted it and something is still finishing — [`Outcome::Started`].
    Started,
}

/// **What an operation whose answer cannot say otherwise passes** — a `PATCH`'s `200` means the
/// patch is applied, so [`scale`] and [`restart`] read this and never a response.
///
/// One function rather than a closure at each call site: two spellings of *this verb has no
/// pending case* is the second copy NOTES § D103 is named for.
fn finished<Response>(_: &Response) -> Landing {
    Landing::Finished
}

/// Handed to the operation's closure for the server-side `dryRun=All` pass (invariant 2).
const DRY_RUN: Pass = Pass(true);

/// Handed to the same closure for the call that actually changes something.
const FOR_REAL: Pass = Pass(false);

/// The verdict when the cluster ran the check and accepted it — `screens/dialogs.md`'s own line.
const ACCEPTED: &str = "the cluster checked it first and accepted it";

/// **The verdict when k8rs sent no check before the change** — [`Mutation::checkable`] `false`.
///
/// It says what happened and not why. The cluster would have run one (NOTES § D215); whether a
/// given operation asks for it is that operation's own box, and the reasons differ.
const UNCHECKABLE: &str = "k8rs did not check this one with the cluster first";

/// **The mutation contract — every write in k8rs goes through here** (todo.md § Phase 7).
///
/// The order is *dialog opens pending → check → verdict into the open dialog → button lives →
/// answer → real call → audit*, which is the one sequence that satisfies invariant 2 (the dialog
/// is shown before the check), `screens/dialogs.md` rule 3 (the verdict lands *in* it) and this
/// box (the answer comes back after it).
///
/// `call` is the operation itself, and where the operation asks for a check it is called twice
/// with the same body: once with [`DRY_RUN`], once with [`FOR_REAL`]. One closure rather than
/// two is what stops the dry-run validating something other than what is sent; the [`Pass`] it
/// is handed — rather than a `bool` — is what stops the first call being sent for real.
///
/// `show` is synchronous by design — see the region's doc.
///
/// `ask` is handed a [`Checked`] and answers how the dialog ended. Typing the object's name where
/// invariant 2 requires it is part of *asking*, and belongs to the dialog that implements it.
///
/// `clock` is a parameter because the clock is an input rather than an ambient fact
/// (NOTES § D18), and it is read **twice**: once for the attempt line and once when the result
/// line is built. The second reading is what gives the record a landing time at all — see
/// [`Record::result_line`], which is where the box that owns the log took it (todo.md 3696,
/// NOTES § D214's closing paragraph).
///
/// `audit` is any destination that can be written and flushed. [`audit_log`] is the one k8rs
/// opens; a caller with an unwritable state directory has already been refused there and never
/// reaches this function (NOTES § D21).
///
/// `landed` reads the *real* call's answer for the one thing this function cannot know: whether
/// the change has finished. A `PATCH`'s `200` means applied and passes [`finished`]; a delete's
/// does not, and [`Landing`] carries why. It is handed a reference and the operation has already
/// mapped the response down to a fact, so nothing here holds an object either.
///
/// **The confirmation is checked against this call's own [`ticket`] before the real call goes
/// out** — an [`Agreed`] built by some *other* mutation's [`Checked`] confirms nothing, which is
/// what stops a yes being kept and replayed (NOTES § D225 ruling 2).
pub async fn perform<Show, Ask, Asked, Call, Called, Response>(
    record: &Mutation<'_>,
    clock: impl Fn() -> Timestamp,
    audit: &mut impl Write,
    show: Show,
    ask: Ask,
    call: Call,
    landed: impl FnOnce(&Response) -> Landing,
) -> Performed
where
    Show: FnOnce(&Shown<'_>),
    Ask: FnOnce(Checked<Response>) -> Asked,
    Asked: Future<Output = Answer>,
    Call: Fn(Pass) -> Called,
    Called: Future<Output = Result<Response, kube::Error>>,
{
    let record = Record::of(record);
    // **Taken before anything is shown, so the dialog that opens is the one this ticket names.**
    // Every [`Agreed`] the [`Checked`] below can build carries it, and the confirmation is checked
    // against it before the real call — which is what stops a yes from an earlier mutation being
    // kept and returned here (NOTES § D225 ruling 2, [`Agreed`]).
    let ticket = ticket();
    let attempt = clock();

    // NOTES § D21 — **written out and flushed** before anything reaches the cluster, dry-run
    // included. *On disk* is what this said until 2026-09-04 and it is not what happens:
    // `impl Write for File`'s `flush` is a no-op, so the bytes are in the page cache and a
    // machine that loses power here loses the attempt line (`k8s-admin`; measured beside it,
    // `sync_data` costs 1754 µs against `flush`'s 1.3 µs). What D21 buys is that *this process*
    // crashing leaves an attempt with no result; a machine crash is outside invariant 3's trust
    // model, and [`write_line`] takes an `impl Write`, which has no `sync` to call.
    if write_line(audit, &record.attempt_line(attempt)).is_err() {
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
                asks: record.confirm.clone(),
                ticket,
            })
            .await
            {
                Answer::Cancelled => Outcome::Cancelled,
                Answer::Gone => Outcome::Gone,
                Answer::Changed => Outcome::Changed,
                // **A yes, and this mutation's own** — the guard is the whole of [`Agreed`]'s
                // mechanism, and the arm under it is what a replay falls into.
                Answer::Confirmed(agreed) if agreed.0 == ticket => {
                    match call(FOR_REAL).await {
                        // **The cluster's own answer decides which of the two this is**, and the
                        // operation is what reads it: [`Landing`].
                        Ok(returned) => match landed(&returned) {
                            Landing::Finished => Outcome::Done,
                            Landing::Started => Outcome::Started,
                        },
                        Err(error) => Outcome::Failed {
                            fault: fault(&error),
                            said: said(&error),
                        },
                    }
                }
                // **A yes built for some other mutation** ([`Agreed`]) — an author error like
                // [`Checked::pressed`]'s, reported the same way: loud where it can be, and the
                // safe direction where it cannot.
                //
                // **The guard above is what routes and this assertion only decorates**, which is
                // not a style choice: with the comparison inside this arm instead, a widened
                // guard would still panic in a debug build and the test watching for the panic
                // would go on passing over a hole (my own second pass, 2026-09-04).
                Answer::Confirmed(agreed) => {
                    debug_assert_eq!(
                        agreed.0, ticket,
                        "a confirmation built for another mutation was replayed here \
                         (invariant 2)"
                    );
                    Outcome::Cancelled
                }
            }
        }
    };

    // **The second reading, and it is taken here rather than beside the call** — the line it
    // stamps is the line being written, and a stamp taken anywhere else would name a moment the
    // record does not describe.
    let recorded = write_line(audit, &record.result_line(attempt, clock(), &outcome)).is_ok();
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
        let consequence = cleaned(record.consequence, FREE_TEXT);
        // **Invariant 2 requires the dialog to *state* the consequence**, and an empty string
        // states nothing. No operation can reach this today — every consequence is a k8rs
        // sentence with a name interpolated into it — so this is the author's error and not the
        // cluster's, which is what makes an assertion the right shape rather than an outcome.
        debug_assert!(
            !consequence.is_empty(),
            "a mutation reached the contract with nothing to state on screen (invariant 2)"
        );
        Record {
            context: cleaned(record.context, IDENTIFIER),
            // **A sentence by D146's rule, not a name**: a `server:` can carry a path and a port
            // and nothing scans it as a word, which is the same reading that puts `path` here.
            server: cleaned(record.server, FREE_TEXT),
            namespace: record.namespace.map(|value| cleaned(value, IDENTIFIER)),
            object: cleaned(record.object, IDENTIFIER),
            uid: record.uid.map(|value| cleaned(value, IDENTIFIER)),
            consequence,
            kubectl: cleaned(record.kubectl, FREE_TEXT),
            verb: cleaned(record.verb, IDENTIFIER),
            path: cleaned(record.path, FREE_TEXT),
            version: record.version.map(|value| cleaned(value, IDENTIFIER)),
            uid_sent: record.uid_sent,
            checkable: record.checkable,
            // **A name by D146's rule, so [`IDENTIFIER`]** — and it is stripped for a reason the
            // other fields do not have: this one is *compared* against what a person typed, and
            // what they typed is what the dialog showed them, which came through here.
            confirm: match record.confirm {
                Confirm::Press => None,
                Confirm::Type(name) => Some(cleaned(name, IDENTIFIER)),
            },
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
    /// record that was cut off (PM ruling, 2026-09-04). A gap word says which gap it is — and
    /// [`gap`] is what [`Mutation::server`] is put through too, though its type says it is always
    /// there: an operation that hands over an empty string writes the same dangling label.
    ///
    /// **[`Mutation::context`] goes through it for that same reason, and the first operation to
    /// use this file is what found it** (todo.md 3749). `k8s::Session::context` is an `Option`
    /// with three ways to be `None` (NOTES § D202), and the third of them — a context whose name
    /// is nothing but characters invariant 9 strips — arrives on a connection that *worked*. The
    /// driver has no name to hand over there, and `context ·` with nothing after it is exactly
    /// the dangling label the paragraph above refuses for its neighbour.
    ///
    /// **Which cluster, then which object, then what was sent.** `context` and `server` are one
    /// pair because neither answers *which cluster* alone ([`Mutation::server`]), and `namespace`
    /// and `uid` sit with the object for the same reason — a name plus a namespace names whatever
    /// holds it now, and the `uid` names the thing that was there (`k8s-admin`, 2026-09-04).
    ///
    /// **The `uid` says which of two things it is, because it was one word for both and one of
    /// them was a lie** (NOTES § D235, [`which_uid`]). Written `uid <x>`, the field read as *the
    /// object k8rs changed* — measured false on a real cluster.
    fn attempt_line(&self, now: Timestamp) -> String {
        format!(
            "{now} attempt · {} · context {} · server {} · {} · {} · kubectl: {} · \
             call: {} {} · resourceVersion {}\n",
            self.object,
            gap(Some(&self.context), "not named", ""),
            gap(Some(&self.server), "not known", ""),
            gap(self.namespace.as_deref(), "cluster-wide", "namespace "),
            which_uid(self.uid.as_deref(), self.uid_sent),
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
    /// **Two stamps, because the second one is a second reading of the clock** — the landing time
    /// NOTES § D214's closing paragraph left to the box that owns the log (todo.md 3696). Until
    /// then the line named its attempt and nothing else, so *how long did it take* had no answer
    /// at all, and a drain takes minutes (NOTES § D20). An attempt with no result under it is
    /// still D21's crash record.
    ///
    /// **`recorded` and not `took`, because the gap is not the call's.** Between the two readings
    /// sit the dry-run, the dialog, and however long the person at the keyboard spent reading it;
    /// a duration printed as *took 4 min* would claim to be the cluster's when most of it is the
    /// operator's. What this states is the one thing k8rs can see — when the result was written
    /// down — and two absolute stamps are also what cross-references against the apiserver's own
    /// audit log. Subtracting them is the reader's.
    ///
    /// **A second reason not to print a duration: a wall clock steps.** NTP moving it back
    /// mid-mutation makes a computed gap negative, which is NOTES § D55's class exactly; a stamp
    /// that says what the clock said cannot be negative, whatever the clock did.
    ///
    /// **The dry-run verdict is on every result line and not only the two where the sentence
    /// happens to mention it** (NOTES § D8). *Did this write get checked first?* is the one
    /// question the log exists to answer about the contract itself, and on `Done` and
    /// `Cancelled` the word did not appear at all until 2026-09-04 (`tester`).
    fn result_line(&self, attempt: Timestamp, recorded: Timestamp, outcome: &Outcome) -> String {
        let line = format!(
            "result · attempt {attempt} · recorded {recorded} · {} · dry-run: {} · {}",
            self.object,
            self.check(outcome),
            verdict(outcome),
        );
        format!("{}\n", and_said(line, outcome.said()))
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
    ///
    /// **It says what happened to the check and no longer why, because [`verdict`] now does**
    /// (`k8s-admin`, 2026-09-04). This field answers *was this write checked first?* — the one
    /// question the log exists to answer about the contract itself — and the fault that stopped it
    /// belongs beside the outcome, where the operator's own sentence can read it too. Named in
    /// both places it was one clause printed twice on one line — and with the fault gone it is a
    /// `&'static str` again, which is what it was before a `format!` was needed here.
    ///
    /// **A refused arm said *not checked*, which is [`UNCHECKABLE`]'s meaning and not this one**
    /// (NOTES § D224). Measured on a real `404`, an operator holding the apiserver's own audit log
    /// beside this line found the request and a k8rs line denying it — invariant 4's *neither
    /// record may lie*.
    ///
    /// **The fix for that overshot, and the fault decides rather than the arm** (`k8s-admin`,
    /// 2026-09-04, measured against a port nothing was listening on). [`Outcome::NotSent`] is
    /// reachable only from the `Err` of `call(DRY_RUN)` — true — but *that `Err`* is not *a check
    /// that was sent*: a refused connect, a name that will not resolve, a request that could not
    /// be built and an `exec` login that exits non-zero all reach that `Err` with nothing on the
    /// wire, and one flat *the check was sent* is then the same lie one column left that
    /// [`verdict`] was corrected for. Three answers, because there are three things to be told:
    /// the cluster answered, or it never left this machine, or **k8rs does not know** — the
    /// honest third that a [`Fault`] alone cannot resolve, since a connection dying after the
    /// request went out and one that never opened arrive as the same [`Fault::Unanswered`].
    ///
    /// **Exhaustive and no `_` arm**, for [`Outcome::said`]'s reason: a twelfth [`Fault`] has to
    /// choose which of the three it is, rather than inherit the loudest of them.
    fn check(&self, outcome: &Outcome) -> &'static str {
        let Outcome::NotSent { fault, .. } = outcome else {
            return self.accepted();
        };
        match fault {
            Fault::Refused | Fault::Rejected | Fault::Conflict | Fault::Expired | Fault::Gone => {
                "the check was sent and did not pass"
            }
            // The four that never left this machine — [`answered`], a few functions below,
            // groups them with the *answered* faults instead, and is right to: it asks whether
            // anything was changed, which these did not, and this asks whether the request went
            // out, which these did not either. One helper cannot answer both.
            Fault::Kubeconfig | Fault::NoContext | Fault::BadEntry | Fault::NoCredential => {
                "the check never left this machine"
            }
            Fault::Unanswered | Fault::Unfinished => {
                "k8rs does not know whether the check reached the cluster"
            }
        }
    }
}

/// **A k8rs sentence, then the cluster's own words where there are any** — the one place a server
/// message is joined onto one, read by the audit line ([`Record::result_line`]), by the sentence
/// the operator is left looking at ([`Performed::plainly`]) and by [`unread`].
///
/// **It exists because the third caller was missing rather than different** (`k8s-admin`,
/// 2026-09-04): two of these spelled the join and the operator's own surface threw the message
/// away, so a `403` and a `422` on the same call printed one identical line. A colon, because
/// that is the shape the other two already had and the shape a server message already arrives in.
fn and_said(line: String, said: Option<&str>) -> String {
    match said {
        Some(said) => format!("{line}: {said}"),
        None => line,
    }
}

/// **The `uid` field of an attempt line: the value first, then which of two things it is**
/// (NOTES § D235).
///
/// **`uid <x>` alone read as *the object k8rs changed*, and that was measured false.** A `scale`
/// whose Deployment was deleted and recreated between the dry-run and the yes recorded the
/// instance it had *read* over a `PATCH` that landed on the one that replaced it. Only a `delete`
/// carrying a `preconditions.uid` has the cluster's word for it ([`Mutation::uid_sent`]).
///
/// **The qualifier goes after the value and not between the label and it**, which is not a style
/// choice: an operator greps an audit log for `uid <value>`, and a phrase in the middle of that
/// breaks the log's own primary access pattern to say something a trailing clause says as well.
///
/// **It is [`gap`]'s job with one more fact, and not [`gap`] with a suffix parameter** — a fourth
/// argument used by one of its five callers is a helper shaped by its exception.
fn which_uid(uid: Option<&str>, sent: bool) -> String {
    match uid {
        Some(uid) if !uid.is_empty() && sent => {
            format!("uid {uid} (the cluster checked this was the object)")
        }
        Some(uid) if !uid.is_empty() => format!("uid {uid} (what k8rs read, not what it changed)"),
        _ => "no uid was read".to_string(),
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
///
/// **The cluster's own words are not in here**, because both readers append them through
/// [`and_said`] and a sentence that carried them would put the message on the audit line twice.
/// What this owes each of them is the *fault*, which is why the two arms that have one name it.
fn verdict(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Done => "the change was made".to_string(),
        // **The other half of a sentence `screens/dialogs.md` § Delete already started.** Four of
        // its six consequences say *something there may delay this or act first*; this is what
        // says whether it did, and it reuses that clause's own word — *delay* — so the dialog and
        // the closing line are one story rather than two people's wording.
        //
        // **What it does not claim is *what* is delaying it.** A finalizer and a pod inside its
        // grace period are the same fact here, and k8rs has read neither
        // (NOTES § D225 ruling 4).
        //
        // **And it is the one place the taught line and k8rs differ**
        // (`k8s-admin`, 2026-09-04). `kubectl delete` waits by default (`--wait=true`, measured:
        // `timeout 5 kubectl delete node/…` exited 124 still waiting) and k8rs returns as soon as
        // the cluster has accepted it — so the command above is not a lie, it is slower, and the
        // reader is told which. Nothing here reads a `deletionTimestamp`: the *shape* of the
        // answer is what says this ([`Landing`]), and k8rs holds no object to read a field off.
        Outcome::Started => "the cluster accepted this and the object is still there — something \
                             is delaying the removal, and the command above waits for that where \
                             k8rs does not"
            .to_string(),
        Outcome::Cancelled => "nobody confirmed it, so nothing was changed".to_string(),
        Outcome::Gone => "the object was already gone, so nothing was changed".to_string(),
        Outcome::Changed => {
            "the object changed while this was open, so nothing was changed".to_string()
        }
        // **"Sent" and "changed" are not the same word.** *"Nothing was sent"* — what this said
        // until 2026-09-04 — is false wherever the check did go out: an operator holding this
        // beside the apiserver's own audit log finds the `?dryRun=All` at that timestamp and a
        // k8rs line denying it, and invariant 4 is that neither record may lie. What was never
        // sent is *the change*, and that is true of every [`Fault`] this arm can carry.
        //
        // **The premise the first draft of this reasoned from was that the arm is reachable
        // only from the `Err` of a `dryRun=All` that went out.** It is not — a refused connect
        // reaches it with nothing on the wire — and one column left, in [`Record::check`], that
        // step became a shipped lie. This sentence survived it because it says nothing about
        // the check; the distinction lives there, where it has to.
        //
        // **And it names the fault, because [`Record::check`] is not the only reader**
        // (`k8s-admin`, 2026-09-04). Whoever ran the operation sees [`Performed::plainly`] and
        // nothing else, and this sentence alone made a `403` and a `422` on the same call print
        // one identical line — the security gate's *a 403 … names the missing verb + resource*
        // and `PRIOR-ART § C1`'s *a fallback message may never replace a typed error*, both, in
        // the arm written to close the second one.
        Outcome::NotSent { fault, .. } => {
            format!("the change was never sent — {}", in_words(*fault))
        }
        // **k8rs may not assert a failure it cannot see.** A broken pipe *after* the request went
        // out leaves the mutation's fate unknown, and *"the call itself failed"* — what this said
        // until 2026-09-04 — claims to know it did not land. Where the cluster answered, it did
        // not land and the sentence says so; where nothing usable came back, the honest line is
        // that k8rs does not know. Keyed on the fault, never on which branch fired
        // (`PRIOR-ART § C1`).
        //
        // **An em dash and not a colon, because a colon is what [`and_said`] puts after this** —
        // `nothing was changed: <fault>: <what the server said>` reads as one clause nested in
        // another. Every sentence a server message can land after now separates its own clause
        // the same way, which is [`unread`]'s shape one region down.
        Outcome::Failed { fault, .. } if answered(*fault) => {
            format!("nothing was changed — {}", in_words(*fault))
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
        // **The one fault whose sentence names a next step, because it is the one whose fix the
        // reader can act on** — todo.md § Phase 7's `resourceVersion` box, whose own title is *a
        // `409` offers a re-read, never a blind overwrite*. Headlessly the offer *is* the
        // sentence; the dialog that re-reads for real is Phase 11's, drawn in
        // `screens/dialogs.md` § The object went away while the dialog was open. It is written
        // here rather than in [`verdict`]'s two arms so that *the change was never sent* and
        // *nothing was changed* say it once between them (NOTES § D103).
        //
        // **The sentence outlived the precondition it was written beside** (NOTES § D228). No
        // operation in this file sends one now, so a `409` here comes from somewhere else — and
        // there is no divergence from the taught line to note, because `kubectl scale` carries no
        // precondition either and the two now behave the same on a conflict.
        Fault::Conflict => {
            "the object had already been changed by something else, so look at it again before \
             deciding whether you still want this change"
        }
        Fault::Expired => "the login k8rs was using had run out",
        // **Not *any more*, which asserts it used to be there** (`k8s-admin`, 2026-09-04). A `404`
        // is reached by a mistyped name far more often than by a deletion — `scale
        // deployment/wbe` gets here — and a sentence claiming the object *was* there sends that
        // reader hunting for whoever removed their deployment. Naming the name instead is true of
        // both, and points at the half that is usually wrong.
        Fault::Gone => "the cluster has no object with that name",
        Fault::Kubeconfig | Fault::NoContext | Fault::BadEntry | Fault::NoCredential => {
            "k8rs could not build a connection from this kubeconfig"
        }
        Fault::Unanswered | Fault::Unfinished => "k8rs could not reach the cluster",
    }
}

/// **One line, one `write_all`, then a flush** — so a destination that stamps or locks per line
/// can, and so two k8rs appending to one log do not interleave halves of a line.
///
/// **A *record* is [`perform`]'s pair of lines and this function writes one of them**, which is
/// the word this doc used both ways until 2026-09-04 (`tester`). Everything below is about a
/// line; nothing here is atomic across the two.
///
/// **That second half is a property of the destination and was stated here as if it were this
/// function's** (todo.md 3696, NOTES § D214's closing paragraph). `write_all` loops on a short
/// write, and a line is not small: a server message alone is bounded at [`FREE_TEXT`], and
/// NOTES § D217 measured a real `422` at 4859 bytes before the cut. What makes the claim true is
/// [`audit_log`]'s file and the bound on a line, both measured on this machine rather than
/// reasoned about (Linux 7.1, btrfs and tmpfs, 2026-09-04):
///
/// - **The file is opened `O_APPEND`**, so every `write(2)` seeks to the end as part of the same
///   call. Four handles on one file, 2000 lines each, at 5000 and at 65536 bytes: 8000 whole
///   lines both times, none torn.
/// - **`write_all` makes one call for a line this size**, so the loop it could break in never
///   runs twice. `Write::write` on a regular file took the whole buffer at 4096, 5000, 16384,
///   1 MiB and 16 MiB.
///
/// **The size is measured and not summed, and *line* is not *record*** (`tester`, 2026-09-04).
/// This doc said *a record is at most ~11 KB*, using "record" for one line where
/// [`perform`] uses it for the pair — and the number was three people's arithmetic over the
/// caps, which went stale the moment [`Mutation::server`] and [`Mutation::uid`] were added.
/// `a_line_and_a_record_have_a_measured_ceiling_and_it_is_far_under_one_write` builds a mutation
/// with every field far past its cap, **prints** the longest attempt line, the longest result line
/// and the longest record, and asserts a **32 KiB** ceiling on the line — which is what the claim
/// above needs, since a line is what one `write_all` is handed, and the measured figure is three
/// orders below where the kernel starts short-writing a regular file.
///
/// **The figures live in that test's output and are deliberately not repeated here**
/// (2026-09-05). They were, and two of the three were stale at `HEAD` with nothing red: the
/// ceiling assertion is on the order of magnitude and correctly does not care about a byte. A
/// number quoted in prose beside a test that prints it is a second copy, and this file has paid
/// for those (NOTES § D103). Run the test to see them. The ceiling is the claim; a cap that moves
/// by a byte is not a red build and a cap that moves by an order of magnitude is.
///
/// **The real ceiling on a record is not its size, it is a full disk** (`tester`, 2026-09-04).
/// Filling a tmpfs mid-record, `write_all` came back `StorageFull` part-way and the file ended
/// with half a line and no newline, so the next record appends onto the same physical line.
/// [`perform`] handles that correctly — the attempt line failing means `recorded: false` and
/// nothing sent — and what is owed here is the note to whoever reads the file: **a reader resyncs
/// on `<timestamp> attempt · ` and on `result · `**, not on newlines alone.
///
/// **For any other destination the claim belongs to that destination.** The tests' sink
/// short-writes eight bytes at a time on purpose, and nothing here depends on it not doing so.
fn write_line(audit: &mut impl Write, line: &str) -> std::io::Result<()> {
    audit.write_all(line.as_bytes())?;
    audit.flush()
}

/// **One untrusted string, cleaned and bounded** (invariant 9, NOTES § D154, § D213).
///
/// [`crate::k8s::text`] works in place, which a `format!` argument cannot be; this is that call
/// and the copy it needs, in **one** place. It was a closure inside [`Record::of`] until the
/// audit log's own refusals needed the same thing for a path out of the environment — and a
/// second spelling of a strip is what NOTES § D213 already caught this file writing once.
fn cleaned(value: &str, cap: usize) -> String {
    let mut value = value.to_string();
    text(&mut value, cap);
    value
}

/// **The subject of a refusal that names a kind: the word itself, or the clause where quoting it
/// back would not be quoting what was typed** (invariant 9, invariant 14, NOTES § D224).
///
/// A kind word out of argv is free text, and [`cleaned`] can do two things to it that a sentence
/// then lies about (`tester`, 2026-09-04). It can **empty** it — `""` and a lone `U+202E` both
/// produced *k8rs cannot restart a  — restarting replaces…*, a gap where the kind should be, the
/// class `src/main.rs`'s `ops_object` settled once for the empty half of `--object web/`. And it
/// can **change** it into a kind the operation does serve: `deployment\n`, `deploy\0ment` and
/// `dep\u{200b}loyment` all produced *k8rs cannot restart a deployment — … k8rs does that for a
/// deployment, a statefulset and a daemonset*, one sentence contradicting its own second clause.
///
/// One rule covers both, and it is the honest one: **the word is quoted only where the strip left
/// it alone**. Anything else is a word k8rs was not given, so the clause costs itself and the
/// sentence that follows still names every kind the operation does serve.
///
/// **Both operations read this.** Neither is reachable from argv today — `known_kind` hands these
/// functions one of six canonical singulars — but both are `pub` in a file that freezes at the end
/// of this phase, and [`scalable`] has the identical shape and the identical exposure. A fix that
/// left the two disagreeing is the defect the family review exists to catch.
fn a_kind(kind: &str) -> String {
    if kind.is_empty() || cleaned(kind, IDENTIFIER) != kind {
        "that kind".to_string()
    } else {
        format!("a {kind}")
    }
}

// --- THE MUTATION CONTRACT END ---

// --- SCALE START ---
//
// **The first operation, and every write after it is this shape** (todo.md 3749). It builds one
// [`Mutation`], hands [`perform`] a closure it never awaits itself, and lets that function's body
// be the five steps — so *what a scale is* lives here and *what every mutation must do* stays one
// copy over there.
//
// **The scale subresource and not a patch of the object.** `Api::get_scale` reads the count the
// consequence sentence is built from and `Api::patch_scale` changes it, which is `kubectl scale`'s
// own route: the request never carries a pod template, so nothing here can drift a workload's spec
// while claiming to be counting copies, and NOTES § D217's `422`-hands-back-the-object hazard is
// bounded to an `autoscaling/v1 Scale` — the shape [`Pass::patch`] already priced.
//
// **Which kinds is this file's fact and the driver holds no copy of it** (NOTES § D220 ruling 4).
// `src/main.rs`'s driver accepts all six kinds for all three verbs on purpose, so
// `k8rs ops scale pod/web 3` reaches [`scalable`] and is refused with a sentence that names what
// scaling *is*. Scope — namespaced or not — is the driver's answer and is taken as a parameter;
// re-deriving it here would be the third copy that ruling refuses.
//
// **No `resourceVersion` precondition, and this is a reversal of a version that shipped**
// (NOTES § D228). `metadata.resourceVersion` bumps on **every** write to the object, a `status`
// write by the deployment controller included — measured: a `status`-only patch moved the scale
// subresource's version `954 → 1102` with `generation` and `spec.replicas` unchanged, and the
// scale after it was a `409`. So the precondition is strictly broader than the thing it defends,
// and a rolling Deployment is a healthy one: 15 writes in 3.46 s under a `kubectl set image`, 20
// in 99.4 s on a `CrashLoopBackOff`, against 0 in 180 s when settled. Through this binary with a
// 10 s confirmation, 5 of 9 runs against churning objects failed *after* the operator typed yes.
//
// **`--replicas=N` is absolute intent and not a read-modify-write**, which is why nothing is lost:
// if something else scaled to 4 while the dialog was open, sending 5 still delivers the 5 that was
// asked for. [REQUIREMENTS.md](../REQUIREMENTS.md) puts conflict handling under **Edit flow**, and
// v0.4's `edit` is the operation that reads, modifies and writes back — [`Mutation::version`] and
// the audit line's column are there for it and are set by nothing today.

/// **What `scale` can be pointed at, in the words the refusal uses** — NOTES § Operations' `s`
/// row.
///
/// **It is a sentence and [`scalable`] is a `match`, and nothing in the language keeps the two in
/// step** — a fourth arm added twelve lines below would not appear here. What keeps them honest is
/// a test rather than a type: `scale_takes_the_three_kinds_it_works_on_and_names_them_when_it
/// _refuses_the_rest` feeds every kind `src/main.rs`'s `KINDS` holds and asserts the three that
/// work and the three that are refused, so a widened arm is a red build for the kind it widened
/// to. Saying so is the point; a claim that they *cannot* diverge would be the second copy this
/// comment exists to warn about.
const SCALABLE: &str = "a deployment, a statefulset and a replicaset";

/// **Whether `scale` works on a kind, and the resource it is when it does** — or the sentence
/// that says what it works on instead (invariant 14).
///
/// **It is `pub` because the driver reads it before the audit log is opened**
/// (NOTES § D220 ruling 7). A line k8rs is going to refuse anyway leaves no state directory
/// behind — the rule `k8rs ops bogus` already had — and a kind refusal is that same line.
///
/// **The group, the version and the plural are `k8s-openapi`'s and are not spelled here.**
/// `ApiResource::erase` reads all three off the same types `k8s.rs`'s permanent watches are built
/// over, so `apps/v1` and `deployments` are the crate's declaration and not a literal this file
/// could get wrong. That is as close to invariant 12 as a function with no cluster to ask can
/// get: what is written down is three kind *words*, which is NOTES § Operations' matrix and not a
/// column list.
pub fn scalable(kind: &str) -> Result<ApiResource, String> {
    match kind {
        "deployment" => Ok(ApiResource::erase::<Deployment>(&())),
        "statefulset" => Ok(ApiResource::erase::<StatefulSet>(&())),
        "replicaset" => Ok(ApiResource::erase::<ReplicaSet>(&())),
        // **The kind word is quoted back only where it survives the strip unchanged**
        // ([`a_kind`], NOTES § D224) — `scalable` is public and the word reaching it came off a
        // command line.
        other => Err(format!(
            "k8rs cannot scale {} — scaling changes how many copies are running, and k8rs does \
             that for {SCALABLE}",
            a_kind(other)
        )),
    }
}

/// **One scale as the line asked for it** — everything [`scale`] needs that is not the connection.
///
/// **`namespace` is an `Option` and is not re-derived here** (NOTES § D220 ruling 4). The driver
/// already refuses a namespaced object with no namespace on the line; what this type does with
/// `None` is refuse, in one place, rather than hold a second copy of which kinds live in one.
pub struct Scaling<'a> {
    /// The kubeconfig context this is performed against — [`Mutation::context`].
    pub context: &'a str,
    /// The `server:` that context reached — [`Mutation::server`].
    pub server: &'a str,
    /// The kind, spelled the way a manifest spells it: `deployment`, never `deploy`
    /// (`screens/dialogs.md` § Scale).
    pub kind: &'a str,
    /// The object's own name.
    pub name: &'a str,
    /// The namespace it is in.
    pub namespace: Option<&'a str>,
    /// **How many copies were asked for.** The type holds the upper bound — `replicas` is an
    /// `i32` on the scale subresource as it is on every workload — and [`scale`] holds the lower
    /// one, because a type cannot: `i32` admits negatives and a caller that does not bound them
    /// gets two records that lie before the cluster answers (`k8s-admin`, 2026-09-04).
    pub count: i32,
}

/// **A scale, performed** — the whole of NOTES § Operations' `s` row.
///
/// `Err` is a refusal of the *request*, before anything has been recorded and before anything has
/// been changed: a kind [`scalable`] does not serve, a namespace nobody named, a name or a
/// namespace that is not an address, a count below zero, or a cluster that would not say how many
/// copies are running now. **The last of those has sent something** — the `GET` of the scale
/// subresource, which is what the refusal is about — and none of them is a *mutation* that was
/// attempted, so none of them writes an audit line (NOTES § D221). The log records what k8rs tried
/// to change, and here k8rs never got as far as describing a change.
///
/// **The count that is *right now* is `spec.replicas` and not `status.replicas`.** The dialog
/// describes the change this call makes, and the change is to the desired count: a Deployment
/// asking for 3 with 1 running, scaled to 3, would otherwise be announced as *starts 2 more
/// copies* over a request that alters nothing — invariant 4's *neither record may lie*, about the
/// one number the reader is agreeing to.
///
/// **A `Scale` with no `spec.replicas` is refused rather than read as zero.** Every cluster fills
/// it, and *Right now: 0 copies* over an object k8rs could not read is a sentence that invents
/// the number the whole consequence turns on.
pub async fn scale<Show, Ask, Asked>(
    client: &Client,
    scaling: &Scaling<'_>,
    clock: impl Fn() -> Timestamp,
    audit: &mut impl Write,
    show: Show,
    ask: Ask,
) -> Result<Performed, String>
where
    Show: FnOnce(&Shown<'_>),
    Ask: FnOnce(Checked<Scale>) -> Asked,
    Asked: Future<Output = Answer>,
{
    let resource = scalable(scaling.kind)?;
    let object = format!("{}/{}", scaling.kind, scaling.name);
    let Some(namespace) = scaling.namespace else {
        return Err(format!(
            "k8rs will not scale {} without being told which namespace it is in",
            cleaned(&object, FREE_TEXT)
        ));
    };
    // **The name and the namespace become segments of the request path, so they are checked
    // where the path is built and not only where the line was parsed.** That is `k8s::owner`'s
    // own shape one file over — it runs [`crate::k8s::path_safe`] over a name it is about to
    // interpolate, rather than trusting whoever handed it one. `src/main.rs`'s driver already
    // refuses both, and it is the only caller today; this function is `pub` in a file that
    // freezes at the end of this phase, and a guard at the point of use is the one that still
    // holds for the console that calls it at Phase 12.
    if !object_name(scaling.name) {
        return Err(unaddressable(&object, "an object's own name"));
    }
    if !namespace_name(namespace) {
        return Err(unaddressable(&object, "the name of a namespace"));
    }
    // **The same argument as the two guards above, applied to the one field the type does not
    // constrain** (`k8s-admin`, 2026-09-04). `count: i32` admits negatives, `src/main.rs`'s
    // `refuse_count` bounds it for a command line, and Phase 12's console is a caller that has not
    // been written. Unguarded, `-5` produced a consequence sentence reading *This stops 8 copies …
    // After: -5 copies*, a command log reading `--replicas=-5` and an audit line recording both —
    // two records lying before the cluster got its say at the `422` (invariant 4). No upper bound
    // is needed or wanted: the field is an `i32` on the scale subresource and so is this.
    if scaling.count < 0 {
        return Err(format!(
            "k8rs will not scale {} to {}: the fewest Kubernetes takes is 0",
            cleaned(&object, FREE_TEXT),
            copies(i64::from(scaling.count))
        ));
    }
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &resource);
    let read = api.get_scale(scaling.name).await.map_err(|failed| {
        unread(
            &object,
            namespace,
            in_words(fault(&failed)),
            said(&failed).as_deref(),
        )
    })?;
    let Some(running) = read.spec.and_then(|spec| spec.replicas) else {
        return Err(unread(
            &object,
            namespace,
            "the cluster's answer did not say how many it is asking for",
            None,
        ));
    };
    let consequence = consequence(scaling.kind, scaling.name, running, scaling.count);
    // **The kind is spelled out — `deployment/web`, never `deploy/web`** (`screens/dialogs.md`
    // § Scale). `deploy` is real kubectl shorthand and buys nothing invariant 4 needs; this line's
    // whole job is teaching a newcomer a command they can read (invariant 14).
    let kubectl = format!(
        "kubectl scale {object} --replicas={} -n {namespace}",
        scaling.count
    );
    // **Derived from the same `ApiResource` the call is built with**, so the audit line cannot
    // name a path the request did not take: `Api::patch_scale` is `Request::patch_subresource`,
    // which is this base, the name and the subresource in that order
    // (`kube-core-4.2.0/src/request.rs:221`).
    let path = format!(
        "{}/{}/scale",
        DynamicObject::url_path(&resource, Some(namespace)),
        scaling.name
    );
    let patch = Patch::Merge(json!({ "spec": { "replicas": scaling.count } }));
    let mutation = Mutation {
        context: scaling.context,
        server: scaling.server,
        namespace: Some(namespace),
        object: &object,
        // **What k8rs saw, not what k8rs sent** — the `uid` is off the `Scale` that was just
        // read, and it is what makes a record checkable after the object's name has moved on.
        uid: read.metadata.uid.as_deref(),
        // **And `PatchParams` has no `preconditions`, so nothing verifies it** (NOTES § D235).
        // Measured: an object deleted and recreated between the check and the yes leaves this
        // naming the instance that was read over a `PATCH` that landed on its replacement. The
        // record says *uid k8rs read* for exactly that reason, and the fix is a label rather than
        // a guard because there is no guard to be had on this verb.
        uid_sent: false,
        consequence: &consequence,
        kubectl: &kubectl,
        verb: "PATCH",
        path: &path,
        // **No precondition, and the region comment above is where the measurement is**
        // (NOTES § D228).
        version: None,
        // **A scale is checkable and this one asks.** `dryRun=All` on the scale subresource is a
        // request every cluster answers, and `screens/dialogs.md` rule 3 is what makes the button
        // wait for it.
        checkable: true,
        // **A press and not a typed name** — invariant 2 raises the bar to typing the object's
        // own name for `delete` and `drain` and for nothing else, and a scale down to zero is
        // warned about in words rather than by a second confirmation kind ([`consequence`]).
        confirm: Confirm::Press,
    };
    // **Both passes are one closure and one body** — [`perform`]'s whole reason — and the
    // borrows are named so calling it twice moves nothing: `api` and `patch` travel as shared
    // references, and the [`PatchParams`] are built inside, once per pass, from the [`Pass`] the
    // contract handed over. The `async move` is what keeps them alive across the `await`;
    // returning `patch_scale`'s future directly would return one borrowing a temporary.
    let (api, patch) = (&api, &patch);
    Ok(perform(
        &mutation,
        clock,
        audit,
        show,
        ask,
        move |pass| {
            let params = pass.patch();
            async move { api.patch_scale(scaling.name, &params, patch).await }
        },
        // A `PATCH`'s `200` means the patch is applied — [`finished`], and no pending case.
        finished,
    )
    .await)
}

/// **Why a name k8rs will not put in a request path is refused** — said apart for the object and
/// for the namespace, because they are two different things to go and fix (invariant 14).
///
/// **It names the address and not the character class**, which is what a reader can act on: the
/// two rules are `k8s::object_name`'s and `k8s::namespace_name`'s, the driver's own refusals
/// already spell them for a line somebody typed, and nothing can reach this one from argv.
fn unaddressable(object: &str, which: &str) -> String {
    format!(
        "k8rs will not send a change to {}: {which} becomes part of the address the request goes \
         to, and this is not one Kubernetes would accept",
        cleaned(object, FREE_TEXT)
    )
}

/// **Why k8rs cannot say what a scale would do** — one sentence, with the cluster's own words
/// after it where there are any.
///
/// The reason is [`in_words`]'s, keyed on the [`Fault`] and never on which call raised it, for
/// the reason [`verdict`] is (`PRIOR-ART § C1`).
///
/// **It names the namespace, and it is the only refusal in this file that had to be told to**
/// (NOTES § D235, invariant 14). Measured on a real cluster:
/// `ops scale deploy/web 3 -n no-such-ns` answered *"the cluster has no object with that name"*
/// and **nothing in the whole run named `no-such-ns`** — so the reader is sent to look at
/// `deployment/web`, which is fine, instead of at the namespace, which is the likeliest mistake
/// a line like that carries. [`restart`] and [`delete`] escape it only by accident: they fail
/// *inside* [`perform`], where [`show`](Shown) has already printed the namespace. [`scale`] is
/// the one operation that can fail **above** the contract, so this sentence is the whole screen.
fn unread(object: &str, namespace: &str, why: &str, message: Option<&str>) -> String {
    and_said(
        format!(
            "k8rs could not read how many copies of {} in {} are running right now — {why}",
            cleaned(object, FREE_TEXT),
            cleaned(namespace, IDENTIFIER)
        ),
        message,
    )
}

/// **What a scale is about to do, in `screens/dialogs.md` § Scale's own words.**
///
/// **Five relations between what is running and what was asked for**, and the count on both
/// sides of every one of them, so nothing depends on the reader remembering the old number.
/// "Copies", never "replicas" (that file's rule 2, invariant 14).
///
/// **Down to zero gets its own sentence and not a stricter guard.** Invariant 2 raises the bar to
/// typing the name for `delete` and `drain` and for nothing else, so the warning is carried by the
/// words rather than by a second confirmation kind.
///
/// **The arithmetic is `i64` over two `i32`s**, which costs nothing and removes the one overflow a
/// hostile or broken `spec.replicas` could reach.
fn consequence(kind: &str, name: &str, running: i32, asked: i32) -> String {
    let (running, asked) = (i64::from(running), i64::from(asked));
    // **One `cmp` and not a chain of `<`/`>`/`==`**, which is a readability preference *and* the
    // thing that makes this function's tests able to fail: `asked > running` written under an
    // `asked == running` that already returned is equal to `asked >= running`, so the mutation
    // gate reported an operator nothing could distinguish and no test could ever kill
    // (`just mutants-diff`, 2026-09-04, NOTES § D26's shape). An `Ordering` has three arms and
    // deleting any of them is a sentence that stops being printed.
    let change = match (asked.cmp(&running), asked, running) {
        // **It describes the request and no longer asserts *no change*** (`k8s-admin` and a PM
        // ruling, 2026-09-04). The `PATCH` is sent, the cluster accepts it, and [`verdict`] then
        // says *the change was made*: *This makes no change* four lines above that was two
        // sentences on one screen that cannot both be read plainly (invariant 14), over a record
        // claiming a change that was not one (invariant 4). What the reader is agreeing to is the
        // request, and the two counts either side of it already say the cluster ends where it
        // started — so nothing is lost by saying the true half.
        //
        // **`screens/dialogs.md` § Scale's *Unchanged* bullet still carries the old sentence and
        // is not this file's author's to edit** — raised to the PM in the same turn.
        (Ordering::Equal, _, _) => format!("This asks for the count {name} is already running."),
        // **`all 1 copy` is not a sentence anybody says**, and the relation list's *all N copies*
        // reads as one for every count but this. The fact is the same: the app stops.
        (Ordering::Less, 0, 1) => {
            "This stops the only copy of your app — nothing will be left running.".to_string()
        }
        (Ordering::Less, 0, _) => format!(
            "This stops all {} of your app — nothing will be left running.",
            copies(running)
        ),
        (Ordering::Less, _, _) => {
            format!("This stops {} of your app.", copies(running - asked))
        }
        (Ordering::Greater, _, _) => format!(
            "This starts {} more {} of your app.",
            asked - running,
            noun(asked - running)
        ),
    };
    format!(
        "{change} Right now: {}. After: {}.{}",
        copies(running),
        copies(asked),
        reverted(kind)
    )
}

/// **The clause a `replicaset` gets and no other kind does** (NOTES § D235).
///
/// **[`scale`] and [`restart`] disagreed about this kind and the disagreement was silent.**
/// [`rollout`] refuses a ReplicaSet in words; [`scalable`] admits it — and measured on a real
/// cluster, scaling a Deployment-owned ReplicaSet had both records saying *the change was made*
/// while the controller put the count back in under three seconds. NOTES § D224's class, with no
/// warning at all.
///
/// **A sentence and not a refusal, because a standalone ReplicaSet is legal and scaling it
/// works.** Refusing every one would break a real operation to fix a common one.
///
/// **And k8rs cannot tell the two apart for free** — measured, the `Scale` subresource
/// [`scale`] already reads **strips `ownerReferences`**, so knowing which it is costs a second
/// `GET` that NOTES § D223 ruling 3 discourages. So the clause is conditional in its own words
/// rather than in code: *if* a deployment manages it. That is true of both, and true for free.
fn reverted(kind: &str) -> &'static str {
    if kind == "replicaset" {
        " If a deployment manages this replicaset, its controller will put the count back."
    } else {
        ""
    }
}

/// **`1 copy`, `3 copies`** — the counted noun, in one place, so the five relations above and the
/// two `Right now:`/`After:` halves cannot disagree about the plural.
fn copies(count: i64) -> String {
    format!("{count} {}", noun(count))
}

/// The noun on its own, for the one sentence that puts a word between the number and it.
fn noun(count: i64) -> &'static str {
    if count == 1 { "copy" } else { "copies" }
}

// --- SCALE END ---

// --- RESTART START ---
//
// **A sibling of SCALE and not a variation on it** (todo.md 3777): the same guards, the same
// [`unaddressable`], the same one-closure-two-passes call into [`perform`]. Two things differ, and
// both are rulings rather than taste — nothing is read before the patch, and the patch is of the
// workload object itself rather than of a subresource.
//
// **k8rs builds the patch and does not call `Api::restart`** (NOTES § D215). That helper writes
// `kube.kubernetes.io/restartedAt` where `kubectl rollout restart` writes [`RESTARTED_AT`], so the
// command log would teach a line that produces a *second* rollout when the operator runs it; and
// it builds its own `PatchParams::default()` internally, so neither `dryRun=All` nor
// `fieldValidation=Strict` could ride with it. Six lines here buy the right key and both
// parameters.
//
// **`Patch::Strategic`, and it is a safety property before it is a fidelity one**
// (NOTES § D223 ruling 4). NOTES § D217's `422` hands back the object the server patched, and for
// a workload that object carries annotations, `managedFields` and
// `spec.template.spec.containers[].env[].value` — measured at 4859 bytes on a trivial Deployment,
// past [`FREE_TEXT`], with the annotations inside the first few hundred bytes, so truncation does
// not redact anything. A strategic merge patch is answered with k8rs's own six lines instead
// (`patch.go:770-786` rather than `:353`), and it is what `kubectl rollout restart` sends anyway —
// so invariant 4's *equivalent command* and the exposure fix are one choice.
//
// **Nothing is read before the patch** (NOTES § D223 ruling 3), which is the other half of the
// same exposure: a `GET` of a Deployment would pull those container environments into k8rs for a
// dialog that shows none of them. So `uid` and `version` are both `None`, and a missing object is
// refused by the dry-run in the apiserver's own words.
//
// **The box that would have sent a `resourceVersion` landed and was then reversed, and neither
// state ever reached this operation** (NOTES § D227 ruling 1, § D228). Nothing is read here, so
// there was never a version to send. The one mechanism that looked like it bought a version
// without the exposure was measured and does not: `Api::get_metadata`'s
// `PartialObjectMetadata` returns no `spec`, but it returns `metadata.annotations`, and on any
// object created with `kubectl apply` that carries
// `kubectl.kubernetes.io/last-applied-configuration` — the whole applied pod spec, planted canary
// included (D227 ruling 2). It is also not cheap: 5247 bytes against the full `GET`'s 7243, of
// which 2148 are `managedFields`.
//
// **What the check's *answer* says is read, and that is not the same thing** (NOTES § D224). The
// apiserver accepts this patch on a paused Deployment and changes nothing an operator can see, so
// the consequence, the result sentence and the taught command all lied at once and no preflight
// could catch it. [`paused`] takes one `bool` off the response `perform` is already handed, inside
// the closure, and the workload is dropped there — a fact k8rs was sent, never a read k8rs went
// looking for.
//
// **A pod is refused in words and never deleted** (NOTES § D223 ruling 1). Restarting a pod is a
// `DELETE`, and both things a `DELETE` needs — invariant 2's typed name made structural, and
// whether a delete is checkable at all — belong to todo.md 3811. `screens/dialogs.md` rule 4's
// dialog is Phase 11's, over the path that box will have proven.
//
// **`checkable` is `true` and the taught line carries no dry-run flag**, which is not a
// contradiction: the API dry-runs an ordinary `PATCH` on an ordinary path (NOTES § D215), and
// `kubectl rollout restart` has no `--dry-run` flag at all. So k8rs says it checked this one and
// no sentence here claims the taught command could.

/// **The annotation `kubectl rollout restart` writes, and the whole reason this operation does not
/// call `Api::restart`** (NOTES § D215, measured against kubectl v1.36.3).
///
/// Any change to `spec.template` starts a rollout; what this key buys is that the operator's next
/// `kubectl rollout restart` overwrites *this* annotation instead of adding a second one beside
/// it — which would be a second rollout, from the command log's own line (invariant 4).
const RESTARTED_AT: &str = "kubectl.kubernetes.io/restartedAt";

/// **What `restart` can be pointed at, in the words the refusal uses** — NOTES § Operations' `r`
/// row.
const RESTARTABLE: &str = "a deployment, a statefulset and a daemonset";

/// **Why a pod has no restart** — NOTES § D223 ruling 1, and `screens/dialogs.md` rule 4's own
/// *nobody learns "restart" as a synonym for "delete" by accident*.
///
/// **It names what k8rs does restart**, because a reader told only *no* has to go and find the
/// table this sentence is (invariant 14) — and it reads [`RESTARTABLE`] rather than spelling the
/// three kinds a second time. It is a function and not a `const` for exactly that: a `const` cannot
/// interpolate one, and a hand-written twin of a kind list is the copy NOTES § D103 is named for.
/// **It was written both ways, one line apart, before it was caught** (`dev-core`, 2026-09-04).
///
/// **Three clauses were rewritten because they taught nothing** (`k8s-admin`, NOTES § D224).
/// *letting whatever made it start another one* garden-paths on *made it start*; *which is not
/// what the word restart does here* leaves *here* undefined and says nothing a reader can act on;
/// and the sentence ended naming three kinds the reader had not asked about without ever saying
/// what to do. It now ends with the instruction: find the object this pod belongs to and restart
/// that.
fn pod_is_a_delete() -> String {
    format!(
        "k8rs will not restart a pod: restarting a pod means deleting it and letting the thing \
         that created it start a replacement. k8rs restarts {RESTARTABLE} — if this pod belongs \
         to one, restart that instead"
    )
}

/// **What a restart of one kind is: the resource to send it to, and what it does in plain words**
/// — or the sentence that says what k8rs restarts instead (invariant 14).
///
/// **One `match` decides the type and the consequence together**, so a fourth arm cannot be added
/// with the wrong sentence attached: there is no second place for a *consequence* to live.
/// [`RESTARTABLE`] is still a separate sentence naming the same three kinds, exactly as
/// [`SCALABLE`] is beside [`scalable`], and nothing in the language keeps that one in step — what
/// does is `restart_takes_the_three_kinds_it_works_on_and_names_them_when_it_refuses_the_rest`,
/// which feeds every kind `src/main.rs`'s `KINDS` holds.
///
/// **The group, the version and the plural are `k8s-openapi`'s and are not spelled here** —
/// [`scalable`]'s own reason, off the same types `k8s.rs`'s permanent watches are built over.
///
/// **The consequences differ by kind because what the object is differs** (invariant 14): a
/// Deployment and a StatefulSet have copies, a DaemonSet has one copy per node it runs on, and a
/// StatefulSet is worked through in a direction. Saying one sentence for all three would be a
/// sentence that is true of one of them.
///
/// **The three are `screens/dialogs.md` § Restart's own, and every one of them was wrong here
/// first** (NOTES § D224). They promised *a few at a time*, *one at a time and in order* and *a
/// node at a time* — a pacing the cluster owns and k8rs deliberately reads none of
/// (NOTES § D223 ruling 3) — and four configurations on a real cluster falsified them, three of
/// which need no feature gate: a DaemonSet with `maxUnavailable: 3` took every node down at once,
/// a `partition`ed StatefulSet left two of three copies on the old template indefinitely, a
/// `nodeSelector`'d DaemonSet ran on one node of three, and `OnDelete` moved nothing on either
/// kind.
///
/// ***Asks* is the load-bearing word.** The patch is a request and the controller decides, which
/// stays true under `paused`, `OnDelete`, `partition` and every pacing knob at once. Each sentence
/// then names *that* the object has such settings without naming one, so nothing here reads a
/// field ruling 3 keeps k8rs out of.
///
/// **The rule this doc used to state is not a rule, and it is written down so it is not read back
/// out of the code.** *Hedge where the truth is worse than the words and not where it is milder*
/// put the one hedge on the Deployment and named `Recreate`; a `RollingUpdate` Deployment with
/// `maxSurge: 0` stops every copy first as well, and a `partition`ed StatefulSet stops short of
/// *every copy* — milder, and just as false. A setting decides the pace on all three kinds, so
/// all three say so.
fn rollout(kind: &str) -> Result<(ApiResource, &'static str), String> {
    match kind {
        "deployment" => Ok((
            ApiResource::erase::<Deployment>(&()),
            "This asks Kubernetes to replace every copy of your app with a new one. How many stop \
             at the same time is a setting on this deployment — it can be a few, or all of them \
             at once. A paused deployment will not start until you resume it.",
        )),
        "statefulset" => Ok((
            ApiResource::erase::<StatefulSet>(&()),
            "This asks Kubernetes to replace every copy of your app with a new one, working down \
             from the highest-numbered copy. How many stop at the same time, how far down it \
             goes, and whether it waits for you to delete a copy yourself are all settings on \
             this statefulset.",
        )),
        "daemonset" => Ok((
            ApiResource::erase::<DaemonSet>(&()),
            "This asks Kubernetes to replace the copy of your app on each node it runs on. How \
             many nodes it takes at a time, and whether it waits for you to delete a copy \
             yourself, are settings on this daemonset.",
        )),
        // NOTES § D223 ruling 1 — its own sentence, because *k8rs cannot restart a pod* would be
        // true of a word nobody uses that way and would teach nothing.
        "pod" => Err(pod_is_a_delete()),
        // **The one refused kind whose copies an operator would actually want replaced**
        // (`k8s-admin`, NOTES § D224), so it gets the answer rather than the general sentence,
        // which named what restart works on and never what to do about *this*.
        //
        // **`normally` and not `the deployment that owns it`**: a ReplicaSet can be created on its
        // own, and k8rs has read no `ownerReferences` here to know which this is
        // (NOTES § D223 ruling 3).
        "replicaset" => Err(format!(
            "k8rs cannot restart a replicaset: a replicaset is normally made by a deployment, and \
             restarting that deployment is what replaces its copies. k8rs restarts {RESTARTABLE}"
        )),
        // **The kind word is quoted back only where it survives the strip unchanged**
        // ([`a_kind`], NOTES § D224) — [`restartable`] is public and the word reaching it came off
        // a command line.
        other => Err(format!(
            "k8rs cannot restart {} — restarting replaces the copies an object is running, and \
             k8rs does that for {RESTARTABLE}",
            a_kind(other)
        )),
    }
}

/// **The six lines `kubectl rollout restart` sends** — and the media type that keeps a rejection
/// from quoting the whole object back (NOTES § D223 ruling 4, § D217).
///
/// **Built once, outside the closure [`perform`] calls twice**, so both passes carry the same
/// stamp. A `clock()` inside that closure would dry-run one annotation value and send another,
/// which is the one thing the one-closure shape exists to prevent.
///
/// **The `Patch` variant is asserted here and its media type one link along.** kube keeps
/// `Patch::content_type` `pub(crate)` (`kube-core-4.2.0/src/params.rs:637`), so nothing in this
/// crate can read it off a patch; what `ops_tests.rs` does instead is assert this function's
/// variant, and assert separately — off a request `Request::patch` built — that
/// `Patch::Strategic` is `application/strategic-merge-patch+json` on the wire.
fn restart_patch(stamp: &Timestamp) -> Patch<Value> {
    Patch::Strategic(json!({
        "spec": { "template": { "metadata": { "annotations": {
            RESTARTED_AT: stamp.to_string()
        } } } }
    }))
}

/// **Whether the workload the cluster answered with is paused** — the one fact [`restart`] keeps
/// off a response it otherwise drops whole (NOTES § D224).
///
/// **This is not a pre-read, and NOTES § D223 ruling 3 is not bent by it.** Ruling 3 forbids a
/// `GET` *before* the patch; this reads the answer to a `dryRun=All` k8rs has already sent and is
/// already handed. What it does not do is *hold* the workload: the `bool` is taken inside the
/// closure and the [`DynamicObject`] is dropped there, so [`Checked`] carries a flag rather than
/// a Deployment with `spec.template.spec.containers[].env[].value` in it, for as long as a dialog
/// is open and somebody is reading it.
///
/// **Why the flag is worth a call at all** (NOTES § D224, measured on a real cluster): on a paused
/// Deployment the apiserver accepts this patch, k8rs said *the change was made* and exited `0`,
/// and twelve seconds later the three pods had the same names — while the line k8rs printed,
/// `kubectl rollout restart`, exits `1` with *can't restart paused deployment*. Three records
/// lying at once, and the preflight that exists to catch it cannot, because the request is valid.
///
/// **`spec.paused` is a Deployment field and nothing invents one for the other two.** A
/// StatefulSet and a DaemonSet have no pause, so their answers simply do not carry it and this is
/// `false` — no kind is matched on here, because the object's own shape already decides it.
fn paused(workload: &DynamicObject) -> bool {
    workload.data.pointer("/spec/paused") == Some(&Value::Bool(true))
}

/// **Whether `restart` works on a kind, and the resource it is when it does** — or the sentence
/// that says what it works on instead (invariant 14).
///
/// **It is `pub` because the driver reads it before the audit log is opened**
/// (NOTES § D220 ruling 7), and it is [`rollout`] with the consequence dropped: the driver has no
/// use for a sentence it is not going to show.
pub fn restartable(kind: &str) -> Result<ApiResource, String> {
    rollout(kind).map(|(resource, _)| resource)
}

/// **One restart as the line asked for it** — everything [`restart`] needs that is not the
/// connection. [`Scaling`]'s shape without the count: a restart is described by its kind alone.
///
/// **`namespace` is an `Option` and is not re-derived here** (NOTES § D220 ruling 4).
pub struct Restarting<'a> {
    /// The kubeconfig context this is performed against — [`Mutation::context`].
    pub context: &'a str,
    /// The `server:` that context reached — [`Mutation::server`].
    pub server: &'a str,
    /// The kind, spelled the way a manifest spells it: `deployment`, never `deploy`
    /// (`screens/dialogs.md` § Scale).
    pub kind: &'a str,
    /// The object's own name.
    pub name: &'a str,
    /// The namespace it is in.
    pub namespace: Option<&'a str>,
}

/// **A rolling restart, performed** — the whole of NOTES § Operations' `r` row.
///
/// `Err` is a refusal of the *request*, before anything has been sent and before anything has been
/// recorded: a kind [`restartable`] does not serve, a namespace nobody named, or a name or a
/// namespace that is not an address. **None of them has sent anything at all** — which is where
/// this differs from [`scale`], whose last refusal comes back from a `GET` — and none of them is a
/// mutation that was attempted, so none writes an audit line (NOTES § D221).
///
/// **The stamp is read once, before the closure, and both passes carry the same one.** A `clock()`
/// inside the closure would dry-run one annotation value and send another, which is the whole
/// thing [`perform`]'s one-closure shape exists to prevent.
///
/// **UTC `Z` and not kubectl's local offset** (NOTES § D223 ruling 2): the value's only contract is
/// an RFC3339 instant that differs from the last one, and the taught line carries no timestamp for
/// this to diverge from.
///
/// **The check's answer decides one thing before the confirmation: whether this Deployment is
/// paused** (NOTES § D224). [`Checked`] carries the `bool` [`paused`] took off the response, and
/// what the caller does with it is the caller's — `src/main.rs`'s `restarted` puts a line above
/// the prompt. **The operator still decides, and the exit code does not move**: writing the
/// annotation on a paused Deployment is not destructive and it takes effect on resume. What was
/// wrong was being told the copies had been replaced.
///
/// **It also carries whatever precision the clock has, where kubectl truncates to the second** —
/// measured on the real binary, `2026-09-04T13:57:59.520444947Z`. Both are RFC3339 and the
/// annotation is validated by nothing, and the finer one is the one that *keeps* the contract: two
/// `kubectl rollout restart` runs inside one second write the identical value and start no second
/// rollout, and k8rs's cannot. Nothing is rounded to buy a resemblance the taught line never
/// claims.
pub async fn restart<Show, Ask, Asked>(
    client: &Client,
    restarting: &Restarting<'_>,
    clock: impl Fn() -> Timestamp,
    audit: &mut impl Write,
    show: Show,
    ask: Ask,
) -> Result<Performed, String>
where
    Show: FnOnce(&Shown<'_>),
    Ask: FnOnce(Checked<bool>) -> Asked,
    Asked: Future<Output = Answer>,
{
    let (resource, consequence) = rollout(restarting.kind)?;
    let object = format!("{}/{}", restarting.kind, restarting.name);
    let Some(namespace) = restarting.namespace else {
        return Err(format!(
            "k8rs will not restart {} without being told which namespace it is in",
            cleaned(&object, FREE_TEXT)
        ));
    };
    // The two guards are [`scale`]'s, for [`scale`]'s reason: the name and the namespace become
    // segments of the request path, so they are checked where the path is built and not only
    // where the line was parsed.
    if !object_name(restarting.name) {
        return Err(unaddressable(&object, "an object's own name"));
    }
    if !namespace_name(namespace) {
        return Err(unaddressable(&object, "the name of a namespace"));
    }
    let api: Api<DynamicObject> = Api::namespaced_with(client.clone(), namespace, &resource);
    // **The kind is spelled out — `deployment/web`, never `deploy/web`** (`screens/dialogs.md`
    // § Scale), and there is no dry-run flag on it because `kubectl rollout restart` has none
    // (NOTES § D223 ruling 4).
    let kubectl = format!("kubectl rollout restart {object} -n {namespace}");
    // **Derived from the same `ApiResource` the call is built with**, so the audit line cannot
    // name a path the request did not take: `Api::patch` is `Request::patch`, which is this base
    // and the name — no subresource, which is what makes this the first operation to reach it
    // (`kube-core-4.2.0/src/request.rs:148`).
    let path = format!(
        "{}/{}",
        DynamicObject::url_path(&resource, Some(namespace)),
        restarting.name
    );
    let patch = restart_patch(&clock());
    let mutation = Mutation {
        context: restarting.context,
        server: restarting.server,
        namespace: Some(namespace),
        object: &object,
        // **Nothing was read, so there is no `uid` to give** (NOTES § D223 ruling 3) — the
        // field's own documented case for a caller with none, and nothing to send one on either
        // (`PatchParams` has no `preconditions`).
        uid: None,
        uid_sent: false,
        consequence,
        kubectl: &kubectl,
        verb: "PATCH",
        path: &path,
        version: None,
        // **A restart is checkable and this one asks** (NOTES § D215): a `PATCH` on an ordinary
        // path is dry-run by every cluster, and only kube's own helper could not carry the
        // parameter.
        checkable: true,
        // **A press** — invariant 2's typed name is `delete`'s and `drain`'s, and a restart
        // replaces copies rather than removing anything.
        confirm: Confirm::Press,
    };
    // Both passes are one closure and one body — [`scale`]'s own borrows, for [`perform`]'s own
    // reason.
    //
    // **The object the calls return is mapped down to one `bool` here rather than carried into
    // [`Checked`]**, which is NOTES § D223 ruling 3 pointed at the direction that box did not name
    // (`dev-core`, 2026-09-04). A `PATCH` is answered with the patched workload whatever
    // k8rs does, so kube deserialises one either way; what the `map` decides is whether k8rs then
    // *holds* it — with container environments in it — for as long as the dialog is open and the
    // person at the keyboard is reading. So [`Checked`] is a `Checked<bool>` and never a
    // `Checked<DynamicObject>`, and the one fact taken off the response is [`paused`], which is
    // the check's own answer to a question invariant 4 turns on (NOTES § D224).
    // [`Checked::returned`] still carries whatever an operation maps to, which is v0.4's `edit`
    // ([`Checked`]'s own doc).
    let (api, patch) = (&api, &patch);
    Ok(perform(
        &mutation,
        clock,
        audit,
        show,
        ask,
        move |pass| {
            let params = pass.patch();
            async move {
                api.patch(restarting.name, &params, patch)
                    .await
                    .map(|workload| paused(&workload))
            }
        },
        // [`scale`]'s reason: a `PATCH`'s `200` means the patch is applied.
        finished,
    )
    .await)
}

// --- RESTART END ---

// --- DELETE START ---
//
// **The third operation, the first destructive one, and the first that is not namespaced**
// (todo.md § Phase 7's `delete` box, NOTES § D225). Everything above the call is [`scale`]'s and
// [`restart`]'s shape — the same guards, the same [`unaddressable`], the same one closure handed
// to [`perform`] and never awaited here. Four things differ by ruling, and a fifth by measurement.
//
// **Every kind is served and none is refused** (D225 ruling 3). There is no `deletable()` beside
// [`scalable`] and [`restartable`]: those exist because a restart of a ReplicaSet is a word with
// no meaning, and a delete of one is not — so the second matrix NOTES § D103 is named for is not
// worth writing to refuse nothing. [`removal`] still refuses a *word that names no kind*, which is
// what makes it total and is not the same thing.
//
// **A node is cluster-scoped, and the namespace check inverts** (D225 ruling 3). `Api::all_with`
// where the other two operations have only ever built `Api::namespaced_with`, and a namespace
// named for a node is the error rather than a namespace missing.
//
// **Nothing is read before the delete** (D225 ruling 4), which is [`restart`]'s ruling 3 for one
// more reason: a `GET` to fetch a `uid` pulls the object — container environments included — into
// k8rs for a dialog that shows none of it. So `uid` and `version` are both `None`, there are no
// `Preconditions`, and a missing object is refused by the apiserver's own `404` on the real call.
//
// **That box left this operation alone twice over** — *every call sends the resourceVersion that
// was read* was a conditional and nothing is read here (NOTES § D227 ruling 1), and the one
// operation it did reach has since had it taken back out (NOTES § D228).
// `DeleteParams::preconditions` is
// measured to work — `resourceVersion` and `uid`, namespaced and cluster-scoped, each with its own
// `409` sentence — and `uid` is the guard for NOTES § D22's *wrong pod deleted*, the worst case in
// the write path. It is Phase 11's, because that is where the dialog can supply a `uid` off the
// watch rather than a `GET` buying one at the exposure this ruling refuses. **And an empty
// precondition is a trap on this verb specifically**: `{"preconditions":{"resourceVersion":""}}`
// is a `409` that can never clear, where the same value in a patch is silently *no precondition*
// (D227 ruling 6).
//
// **And this is the operation that sends no check at all: `checkable` is `false`**
// (D225 ruling 1). A `dryRun=All` delete is a real `DELETE` on the wire, sent before anybody has
// typed anything, and the 17 bytes that distinguish it — `,"dryRun":["All"]` — ride in the
// *body*, with nothing in the URI or the headers
// (`reports/2026-09-04-delete-on-the-wire.md`). So in the cluster's own audit record at the
// `Metadata` level most clusters run, opening a delete dialog on `prod` and cancelling it is
// indistinguishable from having deleted the object — a lie in a record k8rs does not own, cannot
// annotate and cannot correct. A preflight also *adds* a failure mode: a
// `ValidatingWebhookConfiguration` with `sideEffects: Some | Unknown` fails `dryRun=All` for a
// fully authorised user, so k8rs would refuse a delete that would have worked, on the one
// operation where being refused wrongly is least acceptable. What is given up is small — a `403`
// and a `404` come back from the real call regardless, and *the object went away* is
// NOTES § D22's watch and not a dry-run's.
//
// **And a fifth thing, found by a review rather than by a ruling: a `200` from a delete is not
// one fact** (`k8s-admin`, 2026-09-04). A Node held by a finalizer and a pod inside its grace
// period are both accepted and both still there seconds later; `Api::delete` says which by
// answering with a `Status` when the object is gone and with the object itself when it is going.
// So this is the one operation that can produce [`Outcome::Started`] — see [`Landing`], and note
// that the exit code does not move, because `deletionTimestamp` being set *is* a change.
//
// **So the verdict is [`UNCHECKABLE`]'s existing sentence, and the only call this operation ever
// makes is the real one.** Everything a dialog is shown is therefore true the moment it opens:
// nothing is being waited on, and `screens/dialogs.md` § Delete says so rather than dressing the
// absence up as a wait.

/// **What `delete` can be pointed at, in the words the refusal uses** — every kind
/// `src/main.rs`'s `KINDS` holds, which is exactly why there is no refusal table beside it
/// (NOTES § D225 ruling 3).
///
/// **It is read by one sentence and not by six**, unlike [`SCALABLE`] and [`RESTARTABLE`], whose
/// refusals are the point. The only refusal here is for a word that names no kind at all, and
/// `delete_takes_every_kind_the_driver_can_name_and_refuses_only_a_word_that_names_none` feeds it
/// every kind `KINDS` holds.
const DELETABLE: &str = "a deployment, a statefulset, a daemonset, a replicaset, a pod and a node";

/// **What a delete of one kind is: the resource to send it to, what it does in plain words, and
/// whether its objects live in a namespace** — or the sentence for a word that names no kind.
///
/// **One `match` decides all three for [`rollout`]'s reason**: there is no second place for a
/// consequence — or for a scope — to live, so a seventh kind cannot be added with the wrong one
/// of either attached. **The scope is here and not re-derived from the driver's own `KINDS`
/// table**, which NOTES § D220 ruling 4 would refuse: what that ruling keeps out of this file is
/// a *copy of the driver's matrix*, and what this is is the fact `Api::all_with` versus
/// `Api::namespaced_with` turns on, at the point the request path is built. It is the same
/// argument as the [`object_name`] and [`namespace_name`] guards two functions down, which the
/// driver also makes and this file makes again: a guard at the point of use is the one that still
/// holds for the console at Phase 12.
///
/// **The five namespaced sentences and the node's are `screens/dialogs.md` § Delete's own,
/// verbatim.** The pod's hedge is the replicaset's word for word, because k8rs has read no
/// `ownerReferences` and cannot say what will replace either (NOTES § D225 ruling 4); a
/// deployment, a statefulset and a daemonset get no hedge, because nothing inside a running
/// cluster recreates one of those by itself.
///
/// **Four of the six were rewritten by `screens/dialogs.md`'s finalizer round** (2026-09-04): a
/// deployment, a statefulset, a daemonset and a node now *ask* the cluster to remove rather than
/// removing, and each closes on the same hedge — k8rs has read no `finalizers` any more than it
/// has read `ownerReferences`, and something attached may delay the removal or act first. The pod
/// and the replicaset kept theirs, because their hedge was already about the same unread thing.
/// [`verdict`]'s [`Outcome::Started`] sentence is the other half of that clause and shares its
/// word.
///
/// **The node's is the one consequence a beginner reads exactly backwards, so it draws the more
/// destructive reading** — measured before any of this was written
/// (`reports/2026-09-04-delete-on-the-wire.md` § 7, kubelet v1.36.1). A running kubelet does not
/// re-register: `registerWithAPIServer` runs once per process, the node stayed absent for the full
/// 2 min 45 s watched, and it came back 2 s after the kubelet *process* was restarted. And the
/// pods went either way — 55 s after the delete the pod on that node was gone and its ReplicaSet
/// had made two replacements, both `Pending`. So it is the one arm that interpolates the name,
/// because the sentence is about *this* machine's record, and the one arm that is cluster-scoped.
///
/// **The group, the version and the plural are `k8s-openapi`'s and are not spelled here** —
/// [`scalable`]'s own reason, off the same types `k8s.rs`'s permanent watches are built over.
fn removal(kind: &str, name: &str) -> Result<(ApiResource, String, bool), String> {
    let (resource, consequence, namespaced) = match kind {
        "deployment" => (
            ApiResource::erase::<Deployment>(&()),
            "This asks the cluster to remove the deployment and every copy of the app it runs. \
             k8rs has not read what may be attached to it, and something there may delay this or \
             act first — left alone, nothing is left running."
                .to_string(),
            true,
        ),
        "statefulset" => (
            ApiResource::erase::<StatefulSet>(&()),
            "This asks the cluster to remove the statefulset and every copy of the app it runs. \
             k8rs has not read what may be attached to it, and something there may delay this or \
             act first — left alone, nothing is left running."
                .to_string(),
            true,
        ),
        "daemonset" => (
            ApiResource::erase::<DaemonSet>(&()),
            "This asks the cluster to remove the daemonset and the copy of the app it runs on \
             every node. k8rs has not read what may be attached to it, and something there may \
             delay this or act first — left alone, nothing is left running."
                .to_string(),
            true,
        ),
        "replicaset" => (
            ApiResource::erase::<ReplicaSet>(&()),
            "This removes the replicaset and every pod it manages. Whatever created it will \
             normally replace it — k8rs has not checked whether anything did."
                .to_string(),
            true,
        ),
        "pod" => (
            ApiResource::erase::<Pod>(&()),
            "This removes the pod. Whatever created it will normally replace it — k8rs has not \
             checked whether anything did."
                .to_string(),
            true,
        ),
        "node" => (
            ApiResource::erase::<Node>(&()),
            format!(
                "This asks the cluster to remove its record of {name}, not the machine. \
                 Something attached to it, unread by k8rs, may delay this or act first. Left \
                 alone, its pods are deleted and the machine keeps running until its kubelet \
                 restarts."
            ),
            false,
        ),
        // **The kind word is quoted back only where it survives the strip unchanged**
        // ([`a_kind`], NOTES § D224). Nothing reaches this arm from argv — the driver's
        // `known_kind` hands over one of six canonical singulars — and [`delete`] is `pub` in a
        // file that freezes at the end of this phase, exactly as [`scalable`] and [`restartable`]
        // are.
        other => {
            return Err(format!(
                "k8rs cannot delete {} — k8rs deletes {DELETABLE}",
                a_kind(other)
            ));
        }
    };
    Ok((resource, consequence, namespaced))
}

/// **One delete as the line asked for it** — everything [`delete`] needs that is not the
/// connection. [`Restarting`]'s shape exactly; what differs is what `namespace` is allowed to be.
///
/// **Both states of `namespace` are legitimate here, which is what makes this operation unlike
/// the other two** (NOTES § D225 ruling 3): `None` is a node. [`delete`] refuses the *pairing*
/// that is wrong — a namespace named for a node, or a namespaced object with none — and the kind
/// is what decides which of the two that is.
pub struct Deleting<'a> {
    /// The kubeconfig context this is performed against — [`Mutation::context`].
    pub context: &'a str,
    /// The `server:` that context reached — [`Mutation::server`].
    pub server: &'a str,
    /// The kind, spelled the way a manifest spells it: `deployment`, never `deploy`
    /// (`screens/dialogs.md` § Scale).
    pub kind: &'a str,
    /// The object's own name — and, for this operation, the name that has to be typed back
    /// ([`Confirm::Type`], invariant 2).
    pub name: &'a str,
    /// The namespace it is in, or `None` for a node.
    pub namespace: Option<&'a str>,
    /// **The `uid` of the object the caller is looking at, where it has one** — sent as a
    /// `preconditions.uid`, so the cluster refuses the delete if the name has moved on to a
    /// different object (NOTES § D235, [`Pass::delete`]).
    ///
    /// **`None` is the headless driver and is not a default worth having.** A script has no watch
    /// and `delete` reads nothing (NOTES § D225 ruling 4), so there is no `uid` to give — and a
    /// delete by name alone is still the hazard it was, with a window of milliseconds instead of
    /// the seconds a human takes to type. That residue is real and is not closed by this field.
    ///
    /// **The field exists in Phase 7 because `ops.rs` freezes at the end of it.** A `Deleting`
    /// with no `uid` is a `delete` Phase 11 could not hand one to without reopening a frozen
    /// file — NOTES § D232 ruling 3's shape, found by a cluster instead of by a question.
    pub uid: Option<&'a str>,
}

/// **A delete, performed** — the whole of NOTES § Operations' `d` row.
///
/// `Err` is a refusal of the *request*, before anything has been sent and before anything has been
/// recorded: a word [`removal`] does not know, a namespaced object with no namespace, a node with
/// one, or a name or a namespace that is not an address. **None of them has sent anything at
/// all** — [`restart`]'s position and not [`scale`]'s, since nothing is read first — and none is a
/// mutation that was attempted, so none writes an audit line (NOTES § D221).
///
/// **The name that has to be typed is the object's own and not `kind/name`**
/// (`screens/dialogs.md` § Delete: *"Type the pod's name to confirm"*, over a field holding
/// `web-7d9f4`). What the reader is asked for is what the dialog's title bar shows them.
///
/// **No check goes out, so the dialog is complete the moment it opens** (NOTES § D225 ruling 1).
/// [`perform`] skips the `DRY_RUN` pass entirely for a `checkable: false` mutation, hands
/// [`Checked`] the [`UNCHECKABLE`] verdict, and the only request this function ever makes is the
/// real `DELETE`.
///
/// **The response is mapped to one `bool` inside the closure and the object dropped there**,
/// [`restart`]'s `paused` move exactly (NOTES § D224, § D223 ruling 3): `Api::delete` answers with
/// either the object or a `Status`, kube deserialises one either way, and what the `map` decides is
/// whether k8rs then *holds* a workload — with `spec.template.spec.containers[].env[].value` in
/// it — for as long as the dialog is open.
///
/// **That `bool` is the difference between *gone* and *going*, and it is the whole of
/// [`Outcome::Started`]** (`k8s-admin`, 2026-09-04). A Node carrying a finalizer and a pod inside
/// its grace period both answer `200 OK` with the object; only a completed removal answers with a
/// `Status`. So [`Checked`] here is a `Checked<bool>` — a caller may read it, and today's driver
/// does not, because there is nothing for a *dialog* to say about it: the fact lands after the
/// confirmation, in the sentence and the record.
pub async fn delete<Show, Ask, Asked>(
    client: &Client,
    deleting: &Deleting<'_>,
    clock: impl Fn() -> Timestamp,
    audit: &mut impl Write,
    show: Show,
    ask: Ask,
) -> Result<Performed, String>
where
    Show: FnOnce(&Shown<'_>),
    Ask: FnOnce(Checked<bool>) -> Asked,
    Asked: Future<Output = Answer>,
{
    let (resource, consequence, namespaced) = removal(deleting.kind, deleting.name)?;
    let object = format!("{}/{}", deleting.kind, deleting.name);
    // **The two halves of one refusal, and which one it is depends on the kind**
    // (NOTES § D225 ruling 3). `scale` and `restart` have only the first; a node has only the
    // second, and a namespace named for one is a namespace nobody's object is in.
    match (namespaced, deleting.namespace) {
        (true, None) => {
            return Err(format!(
                "k8rs will not delete {} without being told which namespace it is in",
                cleaned(&object, FREE_TEXT)
            ));
        }
        (false, Some(_)) => {
            return Err(format!(
                "k8rs will not delete {}: {} belongs to the whole cluster and is in no namespace",
                cleaned(&object, FREE_TEXT),
                a_kind(deleting.kind)
            ));
        }
        _ => {}
    }
    // The two guards are [`scale`]'s, for [`scale`]'s reason: the name and the namespace become
    // segments of the request path, so they are checked where the path is built and not only
    // where the line was parsed.
    if !object_name(deleting.name) {
        return Err(unaddressable(&object, "an object's own name"));
    }
    if deleting
        .namespace
        .is_some_and(|namespace| !namespace_name(namespace))
    {
        return Err(unaddressable(&object, "the name of a namespace"));
    }
    // **The first cluster-scoped call k8rs makes** (NOTES § D225 ruling 3). `Api::all_with` for a
    // node, `Api::namespaced_with` for the other five — and the `None` that picks it is the same
    // `None` the path below is built from, so the request and the record cannot disagree about
    // which of the two this was.
    let api: Api<DynamicObject> = match deleting.namespace {
        Some(namespace) => Api::namespaced_with(client.clone(), namespace, &resource),
        None => Api::all_with(client.clone(), &resource),
    };
    // **The kind is spelled out — `deployment/web`, never `deploy/web`** (`screens/dialogs.md`
    // § Scale) — and the line carries no flag at all: `propagationPolicy: Background` is what
    // `kubectl delete` sends when none is given, so what k8rs sends is what this line does
    // ([`Pass::delete`], NOTES § D225 ruling 5). No `--dry-run` either, because none was run.
    let kubectl = match deleting.namespace {
        Some(namespace) => format!("kubectl delete {object} -n {namespace}"),
        None => format!("kubectl delete {object}"),
    };
    // **Derived from the same `ApiResource` the call is built with**, so the audit line cannot
    // name a path the request did not take: `Api::delete` is `Request::delete`, which is this base
    // and the name (`kube-core-4.2.0/src/request.rs:109`). For a node the base carries no
    // namespace segment, which is the whole visible difference on the wire
    // (`reports/2026-09-04-delete-on-the-wire.md` § 2).
    let path = format!(
        "{}/{}",
        DynamicObject::url_path(&resource, deleting.namespace),
        deleting.name
    );
    let mutation = Mutation {
        context: deleting.context,
        server: deleting.server,
        namespace: deleting.namespace,
        object: &object,
        // **Still nothing read — this `uid` is the caller's** (NOTES § D225 ruling 4 stands,
        // NOTES § D235 adds the field). `None` from the headless driver, `Some` from a dialog
        // holding a watch, and either way `delete` sends no `GET` of its own.
        uid: deleting.uid,
        // **And it goes out as a `preconditions.uid`**, so where there is one the record names
        // the instance the *cluster* agreed to rather than the one k8rs happened to read.
        uid_sent: deleting.uid.is_some(),
        consequence: &consequence,
        kubectl: &kubectl,
        verb: "DELETE",
        path: &path,
        version: None,
        // **The one operation that declines the preflight** (NOTES § D225 ruling 1) — the region's
        // own doc says why, and [`UNCHECKABLE`] is what the dialog and the audit line both print.
        checkable: false,
        // **Invariant 2's *deletes additionally require typing the object name*, as a type**
        // (NOTES § D225 ruling 2).
        confirm: Confirm::Type(deleting.name),
    };
    // One closure and one body, [`scale`]'s own borrows — though [`perform`] calls it once here
    // and not twice, since there is no check to send.
    let api = &api;
    Ok(perform(
        &mutation,
        clock,
        audit,
        show,
        ask,
        move |pass| {
            let params = pass.delete(deleting.uid);
            async move {
                api.delete(deleting.name, &params)
                    .await
                    // **One `bool` off the answer, and the object dropped here** — [`restart`]'s
                    // `paused` move (NOTES § D224), for NOTES § D223 ruling 3's reason. `.map(|_|
                    // ())` threw this away until 2026-09-04 and both records then said *the change
                    // was made* over a Node a finalizer was still holding (`k8s-admin`).
                    //
                    // **The shape of the answer is the fact, and no field is read.** `Api::delete`
                    // answers `Either<K, Status>`: a `Status` is the cluster confirming the object
                    // is gone, and the object itself — carrying `deletionTimestamp` — is the
                    // cluster saying it has started. `is_right` is inherent on that type, so
                    // nothing here names a crate invariant 10 has not approved.
                    .map(|answer| answer.is_right())
            }
        },
        // **The one operation with a pending case** ([`Landing`], `k8s-admin`, 2026-09-04).
        |gone| {
            if *gone {
                Landing::Finished
            } else {
                Landing::Started
            }
        },
    )
    .await)
}

// --- DELETE END ---

// --- MAY I START ---
//
// **The one thing in this file that changes nothing, and the reason it is in this file anyway**
// (NOTES § D23). Both reviews are performed with `create`, which `clippy.toml` bans crate-wide, so
// the choice was to widen invariant 1's allowlist with a *but this create is harmless* clause or
// to put a read-only function in the file called "every write". The allowlist stays mechanical
// and this is what that costs: two calls that send a question and receive an opinion.
//
// **What it is for is D23's own case, and D229 ruling 2 narrowed it to one operation.** `scale`
// and `restart` are `checkable: true`, so their `dryRun=All` goes out before [`perform`] calls
// `ask` and a `403` lands with nobody having typed anything. `delete` is the only
// `checkable: false` mutation (NOTES § D225 ruling 1), so it asks for the object's name typed in
// full before it sends anything at all — and *the user types a pod name and only then learns they
// were never allowed* is now `delete`'s alone. **Nothing here is wired into that path**
// (D229 ruling 3): a permission check inside [`perform`] would change three landed operations,
// and the dimming this answer is for is Phase 11's. `ops.rs` freezes at the end of Phase 7 and
// these calls must live in it, so the function lands now and the key map reads it later.
//
// **It fails open, and that is the whole ruling rather than a fallback** (D229 ruling 4,
// `PRIOR-ART § B4`). A cluster can refuse the review itself, answer half of it, or not answer at
// all; every one of those is [`Verdict::CouldNotTell`], and **nothing about k8rs's behaviour may
// turn on it**. A probe that became the reason a permitted action was hidden is k9s 0.50.12
// gating a node shell on a read the user could not do — a diagnostic turned into an outage. So
// this file gives a caller three values and never two, and the third is not an error variant to
// be unwrapped into one of the others.
//
// **Two shapes, because they answer two differently sized questions.** [`may_i_in`] sends one
// `SelfSubjectRulesReview` and gets back everything the subject may do in one namespace, which is
// what makes a screenful of keys one round trip rather than one each; [`may_i`] sends a
// `SelfSubjectAccessReview` and gets one answer, which is what a cluster-scoped question has —
// `delete node/<name>` being the only cluster-scoped mutation k8rs performs (D229 ruling 1). The
// browser's own note points here (`k8s.rs` § THE BROWSER'S ROWS: *the only call that answers that
// is a `SelfSubjectAccessReview`, which is performed with `create` and therefore lives in
// `ops.rs`*), and [`may_i`] is that call.
//
// **Nothing here writes an audit line and nothing here calls [`perform`].** The audit log records
// mutations (NOTES § D221), a probe is not one, and a log with a line per dimmed key in it is a
// log nobody reads. What a probe leaves behind is its answer.
//
// **The rules are read and never shown.** A `ResourceRule` is compared against words k8rs wrote
// itself, so nothing in [`Permits`] reaches a screen — the two strings that *do*, an
// `evaluationError` and whatever the server said about a refusal, are stripped where they enter
// (invariant 9, [`cleaned`], [`crate::k8s::said`]).

/// **What a rule means by "all of them"** — RBAC's wildcard, in `verbs`, `apiGroups`, `resources`
/// and `resourceNames` alike.
const ALL: &str = "*";

/// **The answer to one permission question — and the third value is the point** (NOTES § D229
/// ruling 4).
///
/// [`Self::CouldNotTell`] is not a failure to be collapsed into [`Self::No`]. It is what k8rs
/// knows when the review was refused, half answered, or never came back, and the sentence it
/// carries is why. **A caller may not dim a key on it, refuse an operation on it, or report it as
/// a refusal**: the operation stays available and the real call decides, which is exactly what
/// happens today with no probe at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// **This login is allowed to do it**, and the answer is not always the cluster's own
    /// (NOTES § D230 ruling 6). [`may_i`] gets it from the server; [`Permits::may`] reads it off
    /// one namespace's listed rules and deliberately over-reports on `resourceNames` — measured
    /// against a `kubectl auth can-i` answering **no** to the same question. The over-report is
    /// D229 ruling 4's direction and stays; what may not is a record claiming the cluster said so.
    ///
    /// **This paragraph is the second copy the ruling had already fixed once.** [`Self::plainly`]
    /// lost *"the cluster says"* on 2026-09-05 and this doc kept it, which is CLAUDE.md's own
    /// *the second copy is the one that goes stale, and it is never the one that gets fixed*.
    Yes,
    /// **This login is not allowed to do it** — the server's answer through [`may_i`], or no
    /// listed rule granting it through [`Permits::may`], which only reports one where the answer
    /// it read was whole. [`Self::Yes`]'s note about whose answer this is applies here too
    /// (NOTES § D230 ruling 6).
    No,
    /// **k8rs could not find out**, and the sentence saying why — already stripped and bounded.
    CouldNotTell(String),
}

impl Verdict {
    /// **One answer in words somebody in their first month reads without a glossary**
    /// (invariant 14) — the clause a caller prints after the question it asked.
    ///
    /// **[`Self::CouldNotTell`]'s sentence says out loud that it is not a no**, because that is
    /// the confusion NOTES § D229 ruling 4 exists to prevent and the reader is the person most
    /// likely to make it.
    ///
    /// **The reason is a whole clause and this adds no lead-in to it.** A `k8rs could not find
    /// out — ` in front of [`unasked`]'s own *k8rs tried to ask* said the same thing twice, which
    /// is what the first draft printed.
    ///
    /// **Neither answer says *the cluster says*, and that is a correction rather than a trim**
    /// (NOTES § D230 ruling 6). [`Permits::may`] over-reports on `resourceNames` on purpose —
    /// D229 ruling 4's direction, and right — so *"yes — the cluster says this login is allowed"*
    /// was measured against a `kubectl auth can-i` answering **no** to the same question. The
    /// over-report stays; a record claiming the cluster's authority for it is invariant 4's
    /// *neither record may lie*. `No` drops the clause with it: one of the two keeping an
    /// attribution the other cannot have is a distinction no reader would read as one.
    pub fn plainly(&self) -> String {
        match self {
            Self::Yes => "yes — this login is allowed to do that".to_string(),
            Self::No => "no — this login is not allowed to do that".to_string(),
            Self::CouldNotTell(why) => format!(
                "{why}. That is not a no — k8rs hides nothing and refuses nothing because of it, \
                 and the operation is still there to run"
            ),
        }
    }
}

/// **One permission question, in the words the API asks it in.**
///
/// **The verb and the resource are the *API's* and not k8rs's** — `patch` and `deployments`, not
/// *scale* and *deployment* — because that is what a `ResourceRule` and a `ResourceAttributes`
/// are written in, and a translation table between the two would be the second copy NOTES § D103
/// is named for. What owns the translation is the caller that knows which call it is about to
/// make.
pub struct Asking<'a> {
    /// The API verb: `get`, `list`, `watch`, `patch`, `create`, `delete`.
    pub verb: &'a str,
    /// The API group, `apps` — and `""` for the core group, which is what the API calls it too.
    pub group: &'a str,
    /// The resource in the plural the API spells: `deployments`, `pods`, `nodes`.
    pub resource: &'a str,
    /// The subresource where the question is about one — `scale`, which is the path `scale`
    /// patches and therefore the thing it needs permission on.
    ///
    /// **A rule granting the parent does not grant this, and that is measured off the API
    /// server's own `SubjectAccessReview` rather than off `kubectl`** (NOTES § D230,
    /// `reports/2026-09-05-may-i-against-a-real-cluster.md` § 3a): under one rule granting
    /// `patch deployments`, `subresource: ""` is allowed and `subresource: "scale"` is refused.
    /// It is the one reasoned claim this file shipped, and the operator review is what turned it
    /// into a measured one.
    ///
    /// **The driver spells it `--subresource=<name>` and not with a `/`** (D230 ruling 1) —
    /// `kubectl auth can-i`'s own spelling, where the `/` means something else entirely.
    pub subresource: Option<&'a str>,
    /// **The object's own name, where the question is about one object.** `None` asks about the
    /// resource in general, which is what a key map wants before a row is selected.
    ///
    /// **The driver's `/` fills it in** (NOTES § D230 ruling 1): `may-i delete pods/web` asks about
    /// `web`, which is what `kubectl auth can-i` reads the same string as — and the opposite of
    /// what this file first read it as. It is also what makes [`Permits::may`]'s `resourceNames`
    /// over-report answerable exactly, rather than only in the reader's favour.
    pub name: Option<&'a str>,
    /// The namespace, or `None` for a cluster-scoped question — [`may_i`]'s `delete nodes`.
    pub namespace: Option<&'a str>,
}

/// **Everything one login may do in one namespace, from one call** — D23's own reason for the
/// rules review: a screenful of keys is one round trip and not one each.
///
/// **A `No` off this is only a `No` where the answer was whole.** `SubjectRulesReviewStatus`
/// carries `incomplete` and an `evaluationError` for an authorizer that cannot enumerate its own
/// rules, and a rule that is missing from the list is indistinguishable from a rule that does not
/// exist. So [`Self::may`] answers [`Verdict::Yes`] off a matching rule whatever the status said —
/// a grant that is *there* is a grant — and turns what would have been a `No` into
/// [`Verdict::CouldNotTell`] (NOTES § D229 ruling 4).
pub struct Permits {
    /// The namespace this was asked about — kept so a question about a different one cannot be
    /// answered from it by accident.
    namespace: String,
    /// What the cluster listed. Compared, never displayed.
    rules: Vec<ResourceRule>,
    /// **Why a `No` off [`Self::rules`] would not be one**, or `None` where the answer is whole.
    unsure: Option<String>,
}

impl Permits {
    /// **What this namespace's rules say about one question** — no call, because the call already
    /// happened.
    ///
    /// **A question about another namespace is answered by nobody.** Silently answering it from
    /// *this* namespace's rules is the wrong-object class the write path refuses five other ways
    /// round, and the answer here is [`Verdict::CouldNotTell`] because that is what it is: these
    /// rules genuinely say nothing about somewhere else.
    ///
    /// **And it is not a `debug_assert!`, unlike [`Checked::pressed`]'s neighbour — measured, not
    /// argued.** The first draft had one, on that function's *loud where it can be* reasoning, and
    /// `just mutants-diff` reported `replace != with ==` **surviving**: the assertion fires before
    /// the branch in every build a test runs in, so the guard underneath it could not be reached by
    /// any test and could not fail. The two cases are also not alike. [`Checked::pressed`]'s
    /// release behaviour is *safe but wrong* — an operator confirmed and is recorded as not having
    /// — and it needs the noise. This one's is simply **right**, so an assertion buys nothing and
    /// costs the only test that could hold the guard.
    pub fn may(&self, asking: &Asking<'_>) -> Verdict {
        // **`is_none_or` and not `is_some_and`, which is NOTES § D230 ruling 7's one character**
        // (`k8s-admin`, 2026-09-05). The first draft guarded a *mismatch* and let `None` fall
        // through, so the cluster-scoped question [`Asking`]'s own doc names — `delete nodes` —
        // would have been answered out of one namespace's rules. Unreachable from the driver,
        // because NOTES § D230 ruling 5 sends every single question to [`may_i`]; a trap for
        // Phase 11's key map, which is the caller this whole type exists for.
        if asking.namespace.is_none_or(|named| named != self.namespace) {
            return Verdict::CouldNotTell(format!(
                "this answer is only about the namespace {}, and this question is not",
                cleaned(&self.namespace, IDENTIFIER)
            ));
        }
        // **A grant that is listed is a grant, whatever else the status said** — the half of
        // NOTES § D229 ruling 4 that keeps an incomplete answer useful instead of useless.
        if self.rules.iter().any(|rule| grants(rule, asking)) {
            return Verdict::Yes;
        }
        match &self.unsure {
            Some(why) => Verdict::CouldNotTell(why.clone()),
            None => Verdict::No,
        }
    }
}

/// **One namespace's rules, in one `SelfSubjectRulesReview`** (NOTES § D23).
///
/// **Performed with `create` and therefore here** — invariant 1, mechanically. Nothing is created:
/// the API's review resources are questions posted to an endpoint that answers them, and no object
/// exists afterwards.
///
/// **`PostParams::default()`, with no `dryRun`.** A dry run of a question is a question the server
/// is asked not to answer, and there is nothing to rehearse: this sends nothing that could change
/// anything, which is the whole of why the function is documented rather than guarded.
///
/// **The namespace is not put through [`crate::k8s::namespace_name`], and that is a difference
/// from [`delete`] rather than an omission.** The reviews are cluster-scoped resources, so the
/// namespace rides in the request *body* and never becomes a path segment — the guards in the
/// operations are at the point a path is built, and there is no path here to build.
///
/// **Every failure is [`Verdict::CouldNotTell`] and none is an `Err`** (NOTES § D229 ruling 4).
/// A `Result` here would be a caller deciding what to do about a refused probe, and there is
/// exactly one thing to do about one: nothing.
pub async fn may_i_in(client: &Client, namespace: &str) -> Permits {
    let review = SelfSubjectRulesReview {
        spec: SelfSubjectRulesReviewSpec {
            namespace: Some(namespace.to_string()),
        },
        ..SelfSubjectRulesReview::default()
    };
    let api: Api<SelfSubjectRulesReview> = Api::all(client.clone());
    let (rules, unsure) = match api.create(&PostParams::default(), &review).await {
        Err(error) => (Vec::new(), Some(unasked(&error))),
        Ok(answered) => match answered.status {
            None => (Vec::new(), Some(half_answered(None))),
            Some(status) => (
                status.resource_rules,
                (status.incomplete || status.evaluation_error.is_some())
                    .then(|| half_answered(status.evaluation_error.as_deref())),
            ),
        },
    };
    Permits {
        namespace: namespace.to_string(),
        rules,
        unsure,
    }
}

/// **One question the cluster answers by itself, in a `SelfSubjectAccessReview`** — what a
/// cluster-scoped question has, since [`may_i_in`]'s review takes a namespace and a node is in
/// none (NOTES § D229 ruling 1).
///
/// **It answers a namespaced question too**, through [`Asking::namespace`], and is the wrong tool
/// for a screenful of them: one call per key is what the rules review exists not to do. It is the
/// right tool for one — and it is the call `k8s.rs` § THE BROWSER'S ROWS points at for *may this
/// kubeconfig list this kind*.
///
/// **`allowed` is the only yes**, and an `evaluationError` outranks every kind of no. A `false`
/// that carried *no opinion* is a `No` — the API server treats an unmatched request as refused, so
/// a refusal is what would happen — and so is an explicit `denied: true`. But a `false` of either
/// kind **beside an `evaluationError`** is [`Verdict::CouldNotTell`], for [`Permits`]'s reason: the
/// authorizer has said it could not work the whole thing out, and a `No` off a half-run check is
/// the one answer this file may not report. `denied` is therefore never read — it distinguishes
/// two things that both mean *no*, and the field that changes the answer is the error beside them.
pub async fn may_i(client: &Client, asking: &Asking<'_>) -> Verdict {
    let review = SelfSubjectAccessReview {
        spec: SelfSubjectAccessReviewSpec {
            resource_attributes: Some(ResourceAttributes {
                group: Some(asking.group.to_string()),
                resource: Some(asking.resource.to_string()),
                subresource: asking.subresource.map(str::to_string),
                name: asking.name.map(str::to_string),
                namespace: asking.namespace.map(str::to_string),
                verb: Some(asking.verb.to_string()),
                ..ResourceAttributes::default()
            }),
            non_resource_attributes: None,
        },
        ..SelfSubjectAccessReview::default()
    };
    let api: Api<SelfSubjectAccessReview> = Api::all(client.clone());
    match api.create(&PostParams::default(), &review).await {
        Err(error) => Verdict::CouldNotTell(unasked(&error)),
        Ok(answered) => match answered.status {
            None => Verdict::CouldNotTell(half_answered(None)),
            Some(status) if status.allowed => Verdict::Yes,
            Some(status) => match status.evaluation_error.as_deref() {
                Some(problem) => Verdict::CouldNotTell(half_answered(Some(problem))),
                None => Verdict::No,
            },
        },
    }
}

/// **The sentence for a review that never came back** — the fault named, and the server's own
/// words after it where there were any.
///
/// **Selected off the [`Fault`] and never off which call raised it**, which is `PRIOR-ART § C1`'s
/// rule and [`in_words`]'s existing vocabulary rather than a second one: a `403` on the review, a
/// dead socket and an expired login are three different things to go and fix, and one flat *the
/// probe failed* is the fallback message that may never replace a typed error.
///
/// **[`Fault::Refused`] is the one arm that needs words of its own** (NOTES § D230 ruling 7).
/// [`in_words`] spells it *"the cluster would not allow it"*, which is word for word what a refused
/// `delete` prints — so a reader whose *question* was refused met a denial, and the *"That is not a
/// no"* clause that corrects it 180 characters later. Selecting off the `Fault` is right; the
/// **subject** of that one sentence is the operation, and here it has to be the question. Every
/// other fault is about this machine or the connection and reads correctly either way.
fn unasked(error: &kube::Error) -> String {
    let said = said(error);
    let why = match fault(error) {
        Fault::Refused => "this login is not allowed to ask what it is allowed to do",
        other => in_words(other),
    };
    and_said(
        format!("k8rs could not put the question to this cluster — {why}"),
        said.as_deref(),
    )
}

/// **The sentence for a review that came back and says it is not the whole answer** — an
/// authorizer that cannot enumerate its own rules, or a status the server left off entirely.
///
/// The `evaluationError` is free text the API sent, so it goes through [`cleaned`] at
/// [`FREE_TEXT`] like every other one (invariant 9).
fn half_answered(problem: Option<&str>) -> String {
    let explained = problem.map(|value| cleaned(value, FREE_TEXT));
    and_said(
        "this cluster could not work the whole answer out".to_string(),
        explained.as_deref(),
    )
}

/// **Whether one listed rule answers this question yes** — RBAC's own four-part match, and no
/// more of it than a `ResourceRule` carries.
///
/// **An absent list grants nothing.** `apiGroups` and `resources` are `Option` on the wire and a
/// rule with neither is a rule about nothing, so `None` reads as the empty list, which matches
/// nothing at all.
///
/// **`nonResourceRules` is not read, and that is what [`Asking`] is.** A `nonResourceURL` — the
/// `/apis` grant NOTES § D160 found missing from the documented role — is a different question
/// with a different shape, and there is no caller for it; a resource matcher that quietly
/// answered one would be answering something it was not asked.
fn grants(rule: &ResourceRule, asking: &Asking<'_>) -> bool {
    covers(&rule.verbs, asking.verb)
        && covers(rule.api_groups.as_deref().unwrap_or_default(), asking.group)
        && addresses(rule.resources.as_deref().unwrap_or_default(), asking)
        && about(
            rule.resource_names.as_deref().unwrap_or_default(),
            asking.name,
        )
}

/// **Whether a list of rule words covers one word the question named** — the exact word, or
/// [`ALL`].
fn covers(listed: &[String], wanted: &str) -> bool {
    listed.iter().any(|entry| entry == ALL || entry == wanted)
}

/// **Whether a rule's `resources` covers the resource this question is about** — which is not
/// [`covers`], because a subresource is spelled into the same list.
///
/// **Three spellings answer a subresource question and a rule naming the parent is not one of
/// them**: `*`, the exact `deployments/scale`, and `*/scale` for the subresource across every
/// resource. `deployments` alone does **not** grant `deployments/scale`, which is RBAC's rule and
/// is the one a reader guesses wrong — a login that may patch a Deployment may still not patch its
/// scale.
fn addresses(listed: &[String], asking: &Asking<'_>) -> bool {
    let wanted = match asking.subresource {
        Some(subresource) => format!("{}/{subresource}", asking.resource),
        None => asking.resource.to_string(),
    };
    let across = asking
        .subresource
        .map(|subresource| format!("{ALL}/{subresource}"));
    listed
        .iter()
        .any(|entry| entry == ALL || *entry == wanted || Some(entry.as_str()) == across.as_deref())
}

/// **Whether a rule limited to particular objects answers this question** — and the one place
/// this file knowingly over-reports (NOTES § D229 ruling 4).
///
/// An empty `resourceNames` is every object, which is RBAC's own reading. A **non-empty** one
/// beside a question that names no object is the interesting case: the login may perform the verb
/// on *something*, and answering `No` would dim a key the reader can use on the row in front of
/// them. Over-reporting leaves the key lit and lets the real call decide, which is the direction
/// ruling 4 requires; under-reporting is the one it forbids.
///
/// **`*` is read as *every object* here, and two sources disagree about that.** The API's own
/// description of this field on a `SelfSubjectRulesReview` says *"`*` means all"*; RBAC's
/// `ResourceNameMatches` compares names literally and has no wildcard, so a rule naming `*` grants
/// nothing that is not called `*`. [`covers`] follows the first, which is also the direction the
/// paragraph above requires: reading it as *all* over-reports, and reading it literally would
/// refuse a key over a rule the reader may well be able to use.
fn about(listed: &[String], name: Option<&str>) -> bool {
    listed.is_empty() || name.is_none_or(|name| covers(listed, name))
}

// --- MAY I END ---

// --- THE AUDIT LOG START ---
//
// **The file the second of invariant 4's two records lives in** — and, because NOTES § D21 makes
// a mutation that cannot be recorded a mutation that does not happen, the one thing between a
// well-formed operation and the cluster. This region opens it and says what to do when it cannot.
//
// **Opening only; what goes in it is [`Record`]'s, above.** The two are separate because the
// console (Phase 12) and the headless driver have to open the *same* file in the *same* mode and
// say the *same* thing when they cannot — and the way that stops being true is each of them
// spelling `~/.local/state/k8rs/audit.log` and a mode once more (NOTES § D103).
//
// **The environment is an input, and [`audit_path`] takes it as one** — the shape the clock has
// for the same reason (NOTES § D18). `$XDG_STATE_HOME` and `$HOME` are read in exactly one
// place, [`audit_log`], and every decision about them is a function over values a test can call.
// The alternative was a test that sets the variable, which edition 2024 makes `unsafe` and which
// `cargo test`'s threads make racy.
//
// **No rotation** (NOTES § D21), and this region adds no size cap of its own. What a *line*
// actually costs is measured in [`write_line`], which needed the number for a different reason —
// whether one of them fits a single `write(2)`.
//
// **What is at the path is checked before it is written to, and that is this region's and not
// [`write_line`]'s** ([`open_log`]). A FIFO there hangs the process forever; a log anybody can
// write to is invariant 4's whole subject gone quiet. Neither is visible from inside a function
// handed an `impl Write`.

/// **The audit log's mode: its owner, and nobody else** (CLAUDE.md § Security gate, § Secrets and
/// local files).
///
/// **It is handed to `open` rather than applied after it**, which is the whole of why it is here
/// and not a `chmod` two lines down: `OpenOptionsExt::mode` is the `mode` argument of `open(2)`,
/// so a log this run creates is *never* briefly wider. Create-then-narrow leaves a window in
/// which another process can open a handle that survives the narrowing, and a window is a window
/// however short it is.
///
/// **The kernel applies the process umask to it, and what that costs was measured rather than
/// reasoned about** (`k8s-admin` and my own run, 2026-09-04; CLAUDE.md § *a claim reasoned from a
/// definition instead of measured*). `0600 & ~umask` is `0600` for every umask that leaves the
/// owner's bits alone, which is every ordinary one. Two that do not:
///
/// - **`umask 0177`** does not narrow the log at all — `0600 & ~0177` is `0600` — and this doc
///   said it made the log *unwritable next run*, which is false twice over. What it narrows is
///   [`STATE_DIR_ONLY`]: `0700 & ~0177` is `0600`, a directory with no traverse bit, so the run
///   fails on the `open` inside it and the log is never created at all. Measured: `drw-------`
///   and *Permission denied (os error 13)*.
/// - **`umask 0400`** is the one that reaches the log — `0600 & ~0400` is `0200`, write-only —
///   and k8rs goes on appending to it happily, because appending needs the write bit and not the
///   read bit. Measured: two runs, both fine, `--w-------` on the file. What breaks is the
///   *operator* reading their own audit trail.
///
/// Both are left alone rather than worked around: the first is one `open` failure with D21's own
/// sentence on it, and the second is a umask that says *make my files unreadable to me* being
/// obeyed.
const OWNER_ONLY: u32 = 0o600;

/// **The state directory's mode: its owner, and nobody else** — the XDG base directory
/// specification's own `0700`, and the precondition [`open_log`]'s check below depends on.
///
/// **`create_dir_all` carries no mode and gets `0777 & ~umask`** (`k8s-admin` across thirteen
/// umasks, and re-run here before and after the fix rather than taken on trust — CLAUDE.md
/// § *somebody else's finding stays an estimate until you have run it*). `umask 0` gave
/// `drwxrwxrwx` on the built binary before, and `drwx------` after; `umask 0002` gives
/// `drwxrwxr-x` without this mode.
/// A world-writable `~/.local/state/k8rs/` is exactly what a FIFO or a symlink has to be planted
/// in for [`open_log`]'s refusal to have anything to refuse, and `umask 0` is a CI runner and
/// some daemons rather than a hypothetical.
///
/// **Every directory this run *creates* gets it, parents included**, because `DirBuilder`'s mode
/// applies to each one it makes. A `~/.local` that k8rs is the first thing on the machine to make
/// comes out `0700`, which is the same specification's recommendation one level up; a directory
/// that already exists is not touched, which is why this is not the `chmod` [`open_log`] refuses
/// to be.
const STATE_DIR_ONLY: u32 = 0o700;

/// Where the log sits under the state directory (NOTES § D21).
const UNDER_STATE: &str = "k8rs/audit.log";

/// The state directory relative to [`HOME`], when [`STATE_HOME`] does not name one — the XDG base
/// directory specification's own fallback, and D21's `~/.local/state`.
const DEFAULT_STATE: &str = ".local/state";

/// The variable that names the state directory outright.
const STATE_HOME: &str = "XDG_STATE_HOME";

/// The variable [`DEFAULT_STATE`] hangs off.
const HOME: &str = "HOME";

/// **What is still true when the log cannot be had** (NOTES § D21): k8rs continues, read-only.
///
/// D21 refuses to exit for this, and the reason is worth one clause on every refusal below — a
/// broken state directory must not stop somebody looking at a cluster that is on fire.
const STILL_READS: &str = "reading your cluster still works";

/// **The audit log, open and append-only, and created readable by its owner alone** — or the
/// sentence to print instead (NOTES § D21, invariant 4). *Created*, because [`open_log`] does not
/// **narrow** a file somebody has since widened, and saying *is* here would be a claim this
/// function cannot make. It does **look** at one, and says so — that is [`widened`], and it is a
/// note rather than a mode change.
///
/// **A refusal here is not an exit.** D21's ruling is that k8rs says so and continues read-only.
/// For the console that is the whole process, holding the file for its life with the write path
/// unreachable; for a one-shot `k8rs ops` line, which opens the log per run, the same ruling is
/// the run ending with the sentence and nothing sent.
///
/// **The two variables are read here and nowhere else**, so everything that decides anything
/// about them is [`audit_path`], which takes them as values.
///
/// **The notes are things the operator is told once and not refused for** — an ignored
/// `$XDG_STATE_HOME` ([`ignored`]) and a log somebody else can write to ([`widened`]). They are
/// returned rather than printed because this file draws nothing; the caller decides where a
/// sentence goes, the same way it does for the refusal.
pub fn audit_log() -> Result<(File, Vec<String>), String> {
    let state_home = std::env::var_os(STATE_HOME);
    let home = std::env::var_os(HOME);
    let Some((path, source)) = audit_path(state_home.as_deref(), home.as_deref()) else {
        return Err(nowhere_to_keep(state_home.as_deref(), home.as_deref()));
    };
    // **A refusal loses the [`ignored`] note, and that is the accepted trade.** `Err` has one
    // string in it and no room for a second; what the reader needs in that case is on the
    // refusal already, because every one of [`open_log`]'s carries [`Source::clause`] and so
    // names the variable the path came from.
    let (log, mut notes) = open_log(&path, source)?;
    // Last, so the note about what is *at* the path comes before the one about which variable
    // chose the path — the reader is looking at the file, not at their environment.
    notes.extend(ignored(state_home.as_deref(), source, &path));
    Ok((log, notes))
}

/// **The one refusal that has no path to name** — a machine where neither variable points
/// anywhere (NOTES § D21).
///
/// **It says what each variable actually was, because the sentence it replaced was false about
/// one the reader can check in a single command** (`k8s-admin`, and the PM re-measured it,
/// 2026-09-04). *Neither names a directory it can start from* was printed for
/// `XDG_STATE_HOME=relative-dir` with no `HOME` — where `$XDG_STATE_HOME` **is** set and **does**
/// name a directory, just not an absolute one. A reader who checks, finds it set, and stops
/// trusting the message is the whole cost, and it is NOTES § D214's class in the box built after
/// it.
///
/// **It is a function and not a `format!` inside [`audit_log`]** so that a test asserts these
/// words rather than a copy of them. `audit_log` reads the real environment and cannot be called
/// from a test without either writing into the developer's own state directory or setting a
/// variable — `unsafe` in edition 2024, and racy across `cargo test`'s threads — so a sentence
/// spelled inside it is a sentence only a hand-typed twin could check.
fn nowhere_to_keep(state_home: Option<&OsStr>, home: Option<&OsStr>) -> String {
    without(format!(
        "k8rs has nowhere to keep its audit log: {}, and {}",
        named(STATE_HOME, state_home),
        named(HOME, home)
    ))
}

/// **Why one variable did not name a place to start from** — the three things it can be, said
/// apart because they are three different things to go and fix.
///
/// **The value is echoed, cleaned and bounded** (invariant 9): it is free text out of the
/// environment on its way to a terminal, exactly like the path in [`open_log`]'s refusals.
fn named(variable: &str, value: Option<&OsStr>) -> String {
    match value {
        None => format!("${variable} is not set"),
        Some(value) if value.is_empty() => format!("${variable} is set to nothing"),
        Some(value) => format!(
            "${variable} is {}, which is not a full path starting at /",
            cleaned(&value.to_string_lossy(), FREE_TEXT)
        ),
    }
}

/// **Which variable put the log where it is** — the clause every sentence about the path carries.
///
/// **A refusal that names a path and not where the path came from sends the reader to the wrong
/// variable** (`k8s-admin`, 2026-09-04). `$XDG_STATE_HOME=oops` with a good `$HOME` puts the
/// trail under the home directory and said nothing at all about it, so an operator who set the
/// variable to keep the trail on an encrypted volume never learned it had been ignored. Ignoring
/// a relative value is the base directory specification's rule and stays; not saying so was the
/// defect.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    /// `$XDG_STATE_HOME` named an absolute directory and the log is under it.
    StateHome,
    /// `$HOME` did, and the log is under [`DEFAULT_STATE`] inside it.
    Home,
}

impl Source {
    /// The clause, written for somebody about to go and look at the path.
    fn clause(self) -> String {
        match self {
            Source::StateHome => format!("from ${STATE_HOME}"),
            Source::Home => "under your home directory".to_string(),
        }
    }
}

/// **Where the audit log goes, decided from two values and nothing else** (NOTES § D18's shape).
///
/// `$XDG_STATE_HOME` wins where it names an absolute directory, and `$HOME/.local/state` is the
/// fallback. **A relative or empty value is ignored rather than joined**, which is the base
/// directory specification's own rule and also the safe one here: `k8rs/audit.log` relative to
/// whatever directory a shell happened to be in is an audit trail scattered across the disk, and
/// invariant 4 needs one place to look. `Path::is_absolute` is false for the empty path, so one
/// filter settles both.
///
/// **Nothing from the cluster reaches this**, so the security gate's *object names are sanitised
/// before they build a filesystem path* row has no subject here: the two components are an
/// environment variable and two literals, and no object name is ever joined onto either.
///
/// **It says which variable answered as well as where**, because every sentence downstream of it
/// has to ([`Source`]).
fn audit_path(state_home: Option<&OsStr>, home: Option<&OsStr>) -> Option<(PathBuf, Source)> {
    let absolute = |value: Option<&OsStr>| {
        value
            .map(Path::new)
            .filter(|directory| directory.is_absolute())
            .map(Path::to_path_buf)
    };
    absolute(state_home)
        .map(|state| (state, Source::StateHome))
        .or_else(|| absolute(home).map(|home| (home.join(DEFAULT_STATE), Source::Home)))
        .map(|(state, source)| (state.join(UNDER_STATE), source))
}

/// **What to say when `$XDG_STATE_HOME` was set and k8rs did not use it** — the silent half of
/// [`Source`]'s reason.
///
/// **Empty is not ignored, it is unset.** The base directory specification defines an empty
/// value as *use the default*, so there is nothing to report; a relative one is a value the
/// operator meant and k8rs did not honour, and that is the note.
fn ignored(state_home: Option<&OsStr>, source: Source, path: &Path) -> Option<String> {
    let value = state_home.filter(|value| !value.is_empty())?;
    if source == Source::StateHome {
        return None;
    }
    Some(format!(
        "k8rs is not keeping its audit log where ${STATE_HOME} points: {} is not a full path \
         starting at /, so the log is at {} instead, {}",
        cleaned(&value.to_string_lossy(), FREE_TEXT),
        cleaned(&path.to_string_lossy(), FREE_TEXT),
        // The early return above leaves exactly one value here, so this is `Source::Home`'s
        // clause said once rather than twice.
        source.clause()
    ))
}

/// **The log at a path, created private** — or the sentence that says why it could not be.
///
/// **Two failures and two sentences, because they are two different things to go and fix**
/// (invariant 14): a state directory that cannot be made, and a file that cannot be opened in
/// it. One *"could not open the audit log"* for both sends a reader to check the wrong thing
/// half the time.
///
/// **k8rs sets the mode when it creates the log and does not police it afterwards, but it does
/// look before it writes** (`tester` and `k8s-admin`, 2026-09-04). A file somebody has since
/// widened stays as they left it — narrowing it would be a `chmod` this function does not own,
/// and an operator who deliberately made the log group-*readable* for a collector would find k8rs
/// silently undoing it on every run. **Readable and writable are two different facts and the
/// first draft of this doc collapsed them**: readable by others is the collector, and stays
/// quiet; writable by others is the audit trail's integrity gone, which is the whole reason
/// invariant 4 has this file. So [`widened`] says so once, and k8rs still writes.
///
/// **What is refused is anything at that path that exists and is not an ordinary file** — and
/// that one predicate closes four doors at once. The one that was measured is a FIFO: `open(2)`
/// with `O_WRONLY` on one with no reader **blocks forever**, so `k8rs ops scale …` sat there with
/// no output at all and `timeout 6` had to kill it. NOTES § D21 has three endings — the log
/// opens, or it does not and k8rs says so and reads on — and a silent hang is a fourth it never
/// ruled on. *Pipe my audit trail into a collector* is a plausible thing for an operator to try,
/// and the hang arrives later, when the reader dies. A directory, a device node and a symlink
/// come out of the same check.
///
/// **`symlink_metadata` and not `metadata`, which is what makes the symlink a door this closes**:
/// a *dangling* symlink is `NotFound` to a following `stat`, so the `open` below would follow it
/// and create the file wherever it points — and whatever is put there between the two calls is
/// what k8rs then appends to.
///
/// **`O_NOFOLLOW` is deliberately not added**, and the reason this doc gave for that was an
/// assumption rather than a fact: *`$XDG_STATE_HOME` is not a shared directory* is a claim about
/// somebody else's machine, and `XDG_STATE_HOME` under `/tmp` is a real CI configuration
/// (`k8s-admin`, 2026-09-04). The reason that holds is that `create_dir_all` follows a symlinked
/// `k8rs/` **directory** anyway, so the flag would close the final component and leave the
/// component above it open. The check above closes the file; the directory half is
/// [`STATE_DIR_ONLY`]'s, which keeps the place a symlink would be planted in unwritable by
/// anybody else.
///
/// **The check narrows the window, it does not close it**, and saying so is the honest version:
/// between the `stat` and the `open` two lines below, anything with write access to that
/// directory can swap what is there. What removes the write access is [`STATE_DIR_ONLY`] — which
/// is why the two are one fix and not two, and why neither alone would be worth much.
///
/// **The path is stripped on its way into the sentence** (invariant 9). It comes out of the
/// environment rather than out of the cluster, but it is still free text on its way to a
/// terminal, and an `ESC` in `$XDG_STATE_HOME` is the same cursor-rewrite a crafted pod name is.
fn open_log(path: &Path, source: Source) -> Result<(File, Vec<String>), String> {
    use std::os::unix::fs::DirBuilderExt;

    let shown = cleaned(&path.to_string_lossy(), FREE_TEXT);
    let from = source.clause();
    let blamed = |failed: &std::io::Error| cleaned(&failed.to_string(), FREE_TEXT);
    if let Some(directory) = path.parent() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(STATE_DIR_ONLY)
            .create(directory)
            .map_err(|failed| {
                without(format!(
                    "k8rs could not make a place for its audit log at {shown} ({from}): {}",
                    blamed(&failed)
                ))
            })?;
    }
    // A path with nothing at it is the ordinary first run, and so is one the `stat` itself could
    // not answer for — the `open` below says what is wrong with it, in the words of the system.
    let notes = match std::fs::symlink_metadata(path) {
        Ok(found) if !found.file_type().is_file() => {
            return Err(without(format!(
                "there is something at {shown} ({from}) that is not an ordinary file — a pipe, a \
                 device, a directory or a link — and k8rs will not write its audit log into it"
            )));
        }
        Ok(found) => Vec::from_iter(widened(&found, &shown, &from)),
        Err(_) => Vec::new(),
    };
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(OWNER_ONLY)
        .open(path)
        .map_err(|failed| {
            without(format!(
                "k8rs could not open its audit log at {shown} ({from}): {}",
                blamed(&failed)
            ))
        })?;
    Ok((log, notes))
}

/// **A log somebody other than its owner can write to, or one that is not ours** — said once,
/// and not refused for.
///
/// **Not a refusal, because the trail is worth more than the objection.** k8rs appends and says
/// so; what it may not do is what it did until 2026-09-04, which was append to a `0666` audit log
/// without a word (`k8s-admin`, and the PM re-measured it).
///
/// **Two facts and one sentence each**, because they are two things to go and fix — but at most
/// **one note comes back**, and the mode wins. It is the one with a fix on the end of it, and a
/// file that is both `0666` and somebody else's is already fully described by the first sentence.
/// The mode is checked everywhere; the owner only where the process can find out who it is —
/// see [`us`].
fn widened(found: &std::fs::Metadata, shown: &str, from: &str) -> Option<String> {
    use std::os::unix::fs::MetadataExt;

    let mode = found.mode() & 0o7777;
    if mode & 0o022 != 0 {
        return Some(format!(
            "the audit log at {shown} ({from}) can be written to by other people on this machine \
             (it is {mode:04o}), so what is already in it may not be what k8rs wrote — k8rs is \
             still recording to it, and `chmod 600 {shown}` makes it yours alone"
        ));
    }
    if us().is_some_and(|us| us != found.uid()) {
        return Some(format!(
            "the audit log at {shown} ({from}) belongs to another user on this machine — k8rs is \
             still recording to it, but the trail in it is not only k8rs's"
        ));
    }
    None
}

/// **Which user this process is, where the system will say** — and `None` where it will not.
///
/// **There is no `getuid` in `std`**, and the twelve approved crates do not include one that
/// exposes it (invariant 10): `libc` for a single number would be a thirteenth. What is free is
/// that on Linux `/proc/<pid>` is owned by the process's own user and `/proc/self` resolves to
/// it, so one `stat` answers the question with no dependency at all.
///
/// **Where there is no procfs this returns `None` and [`widened`]'s ownership half does not
/// run** — the release targets include `*-apple-darwin` (`docs/tech-stack.md`), and saying so is
/// better than a check that quietly means nothing. The mode half, which is the one an attacker
/// needs, runs everywhere.
fn us() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;

    std::fs::metadata("/proc/self")
        .ok()
        .map(|process| process.uid())
}

/// **What every refusal above ends with** — what it costs and what still works (NOTES § D21).
///
/// One tail rather than one per refusal, because the consequence is the same one every time and
/// D21 is what decides it: no change is made, and the run is not over.
fn without(trouble: String) -> String {
    format!(
        "{trouble} — every change k8rs makes is written to that log before it is sent, so k8rs \
         will not change anything until that is fixed, and {STILL_READS}"
    )
}

// --- THE AUDIT LOG END ---
