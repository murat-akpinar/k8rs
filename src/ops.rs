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

use k8s_openapi::api::apps::v1::{Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::autoscaling::v1::Scale;
use k8s_openapi::jiff::Timestamp;
use k8s_openapi::serde_json::json;
use kube::api::{
    Api, ApiResource, DeleteParams, DynamicObject, Patch, PatchParams, ValidationDirective,
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
    pub uid: Option<&'a str>,
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
    /// **Whether this operation sends a `dryRun=All` check before the change.** When it is
    /// `false` nothing is sent before the confirmation and the audit line records that no check
    /// was run.
    ///
    /// **Not a fact about the API** — a real cluster dry-runs both verbs this file sends,
    /// `PATCH` and `DELETE` (NOTES § D215). `false` means k8rs declined the preflight, and the
    /// reason belongs to the operation's own box.
    pub checkable: bool,
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
    /// The check passed or was never sent, the confirmation was given, and the call succeeded.
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
            Self::Done | Self::Cancelled | Self::Gone | Self::Changed => None,
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
        matches!(self.outcome, Some(Outcome::Done))
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
    /// object is an `autoscaling/v1 Scale`: name, namespace, uid, resourceVersion,
    /// creationTimestamp and two replica counts, and no labels, annotations, `managedFields` or
    /// pod template (`pkg/registry/apps/deployment/storage/storage.go:370-393`) — so
    /// [`crate::k8s::said`]'s existing strip and `FREE_TEXT` cut are the whole of what is owed
    /// here. **A patch on the object itself is not that shape**: `restart` (todo.md 3689) and
    /// v0.4's `edit` both patch the workload, whose dump carries annotations and
    /// `spec.template.spec.containers[].env[].value` — and `edit`'s unknown fields come from YAML
    /// the operator typed. Those boxes owe the check this one does not.
    pub fn patch(self) -> PatchParams {
        PatchParams {
            dry_run: self.0,
            field_validation: Some(ValidationDirective::Strict),
            ..PatchParams::default()
        }
    }

    /// Params for a `DELETE` — **the dry-run half of the conversion, and only that.**
    /// `DeleteParams::default()` leaves `propagation_policy` `None` (`params.rs:784`), and every
    /// other field of it is `skip_serializing_if`, so a real pass sends `{}` and the server falls
    /// back to the object's own default. `kubectl delete` sends `propagationPolicy: Background`,
    /// so invariant 4's *equivalent* command needs that overridden — the delete box's work
    /// (todo.md 3692), written here because the method name reads like the whole conversion.
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
    pub fn delete(self) -> DeleteParams {
        DeleteParams {
            dry_run: self.0,
            ..DeleteParams::default()
        }
    }
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
pub async fn perform<Show, Ask, Asked, Call, Called, Response>(
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
    Asked: Future<Output = Answer>,
    Call: Fn(Pass) -> Called,
    Called: Future<Output = Result<Response, kube::Error>>,
{
    let record = Record::of(record);
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
    fn attempt_line(&self, now: Timestamp) -> String {
        format!(
            "{now} attempt · {} · context {} · server {} · {} · uid {} · kubectl: {} · \
             call: {} {} · resourceVersion {}\n",
            self.object,
            gap(Some(&self.context), "not named", ""),
            gap(Some(&self.server), "not known", ""),
            gap(self.namespace.as_deref(), "cluster-wide", "namespace "),
            gap(self.uid.as_deref(), "not read", ""),
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
    fn check(&self, outcome: &Outcome) -> &'static str {
        match outcome {
            Outcome::NotSent { .. } => "not checked",
            _ => self.accepted(),
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
        // sent is the change.
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
        Fault::Conflict => "the object had already been changed by something else",
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
/// with every field past its cap and prints what comes out: **the longest attempt line is 15 689
/// bytes, the longest result line 4 864, and the longest record — both lines — 20 553**. What the
/// claim above needs is the *line*, since that is what one `write_all` is handed, and 15.7 KB is
/// still three orders below where the kernel starts short-writing a regular file. The test asserts
/// a 32 KiB ceiling rather than the figure, so a cap that moves by a byte is not a red build and
/// a cap that moves by an order of magnitude is.
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
// **No `resourceVersion` and no `409` re-read** (NOTES § D220 ruling 6). todo.md 3824 wires that
// for every call at once, because the precondition and the re-read are one mechanism; half of one
// built inside this operation would be a box added to a running phase.

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
        // Cleaned and bounded like every other outside string this file prints (invariant 9,
        // NOTES § D213) — `scalable` is public and the word reaching it came off a command line.
        other => Err(format!(
            "k8rs cannot scale a {} — scaling changes how many copies are running, and k8rs does \
             that for {SCALABLE}",
            cleaned(other, IDENTIFIER)
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
    let read = api
        .get_scale(scaling.name)
        .await
        .map_err(|failed| unread(&object, in_words(fault(&failed)), said(&failed).as_deref()))?;
    let Some(running) = read.spec.and_then(|spec| spec.replicas) else {
        return Err(unread(
            &object,
            "the cluster's answer did not say how many it is asking for",
            None,
        ));
    };
    let consequence = consequence(scaling.name, running, scaling.count);
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
        consequence: &consequence,
        kubectl: &kubectl,
        verb: "PATCH",
        path: &path,
        version: None,
        // **A scale is checkable and this one asks.** `dryRun=All` on the scale subresource is a
        // request every cluster answers, and `screens/dialogs.md` rule 3 is what makes the button
        // wait for it.
        checkable: true,
    };
    // **Both passes are one closure and one body** — [`perform`]'s whole reason — and the
    // borrows are named so calling it twice moves nothing: `api` and `patch` travel as shared
    // references, and the [`PatchParams`] are built inside, once per pass, from the [`Pass`] the
    // contract handed over. The `async move` is what keeps them alive across the `await`;
    // returning `patch_scale`'s future directly would return one borrowing a temporary.
    let (api, patch) = (&api, &patch);
    Ok(perform(&mutation, clock, audit, show, ask, move |pass| {
        let params = pass.patch();
        async move { api.patch_scale(scaling.name, &params, patch).await }
    })
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
fn unread(object: &str, why: &str, message: Option<&str>) -> String {
    and_said(
        format!(
            "k8rs could not read how many copies of {} are running right now — {why}",
            cleaned(object, FREE_TEXT)
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
fn consequence(name: &str, running: i32, asked: i32) -> String {
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
        "{change} Right now: {}. After: {}.",
        copies(running),
        copies(asked)
    )
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
