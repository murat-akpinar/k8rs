#!/usr/bin/env python3
"""Drive the confirmation arm on a real pty: press `r`, read the box back, and answer it both ways.

**The arm no gate reached** (todo.md § Phase 13, NOTES § D290, § D291). `main::pressed` and
`main::over_modal` are entered only by a run that is at a keyboard, and `cargo test`'s own ends are
both pipes, so `console()` is never entered from the suite (NOTES § D279 ruling 2). `scripts/e2e.sh`
drives the headless `ops` driver, which has no modal at all — it reads a confirmation off stdin. So
the whole of `just check` said nothing about the arm that decides whether a write goes out, and the
only end-to-end evidence for D22's guard and the refusal key was two hand journeys.

**Why this is a second script and not more of `scripts/picker-test.py`.** That file's own doc states
*no run here can reach a cluster* as a property it keeps, and every journey below needs a real
apiserver — an `Offer::Act` needs a selected Alerts card, and a card needs a snapshot with an object
in it. Its pty machinery is imported rather than copied, exactly as it imports `suspend-test.py`'s.

**What the default run sends, and it is not a mutation.** `cancel` and `typed` both send the
`dryRun=All` invariant 2 requires and then answer *no*; `readonly` sends nothing at all. So the
default writes nothing into the cluster and can be re-run, the same property `scripts/e2e.sh`'s two
legs keep. The one journey that writes is `--confirm`, opt in, and the object it restarts is named
below. `--gone` needs a delete somebody else runs (D92) and refuses to press `⏎` until it has
checked that the object really went away.

**Which object, and why this one.** `deployment/broken-quota` in `k8rs-quota` — W2's own fixture
(`scripts/broken.yaml`), whose finding fires on the Deployment itself. Every *other* broken
Deployment in that file is reached through a pod, whose card owner is the ReplicaSet until the
on-demand fetch resolves it (`k8s.rs` § RESOLVING AN OWNER) — a card whose kind changes under the
cursor is not what a gate should be timing against. A restart of it changes `spec.template`'s
annotation and nothing else: its pods cannot be created at all, so the W2 state it is the fixture
for is the state it returns to. Overridable with `K8RS_CONFIRM_OBJECT` / `K8RS_CONFIRM_NAMESPACE`.

**The cursor is placed by `--namespace` and not by the filter.** Scoping the run to the object's own
namespace leaves Alerts holding exactly one card, so row 0 is the card every journey is about — no
`/` to type, no ordering to assume, and a namespace that has picked up a second finding fails the
*this box is about our object* row by name rather than acting on somebody else's object.

**What a byte stream off a pty can and cannot say** — `picker-test.py`'s lesson, paid for there and
not re-argued: ratatui repaints only the cells that changed, so whitespace is not a fact about the
screen and no column count may be read off a transcript. Every content check reads the transcript
with its whitespace and the box-drawing glyphs squeezed out, and its needle squeezed the same way.

**And one layer deeper than that file ever had to go: a squeezed transcript is not a row-ordered
reading of a sentence.** A dialog here is drawn *over* a console that has a sidebar and a card
beneath it, and the squeeze concatenates each terminal row left to right — so the frame underneath
bleeds into the byte stream **between the halves of every wrapped dialog line**. Measured on a real
cluster, 2026-09-28: the consequence came back as
`…everycopyofyourappwith` **`outconfi`** `anewone.Howmanystop…`, with `config`, `cluster`,
`network`, `storage` and eight more Analysis names in fragments through it. `picker-test.py` never
met this, because its picker is drawn over genuinely nothing (its own § *Opening at startup*).
**So every needle here has to fit inside one drawn row**, and the rows are what the box wraps to —
never a sentence as `ops.rs` spells it. Three of this file's checks were written spanning a wrap and
could not match however right the screen was; `healthy()` now models the bleed, so the self-test
refuses that shape rather than waiting for a cluster to find it.

**The taught `$` line is cut to the box**, and `screens/dialogs.md` § When the object's own name does
not fit *requires* that: once `-n` and `--context` are down to bare flags the **name** front-cuts
too, and the line draws `rollout restart deployment/…-quota -n…`. What that section guarantees is
the `kind/` in front of the name, so that is what the needle over it reads — the object's full
identity is pinned by the **title**, which is rule 1's job and never gives way here.

**Why this is not in `just check`.** It needs a built binary, a pty and a kind cluster, and CI has
none of the three. What runs in the gate is `--self-test`, which feeds every check a healthy
transcript and then one broken variant of itself, and drives every preflight refusal — the same
split `scripts/e2e.sh` keeps, and for the same reason: a run that exits 0 because there was no
cluster, no object or no card is the invisible gap, and it is invisible on exactly the machines CI
runs on.

Usage:
    confirm-test.py                 # the three journeys that write nothing
    confirm-test.py --confirm       # and the one that really restarts the object
    confirm-test.py --gone          # only the gone journey; needs a delete run beside it
    confirm-test.py --self-test     # prove every check fails when it should
"""
import importlib.util
import json
import os
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Loaded by path because the filename has a hyphen, and safe to import — everything that runs in
# that file sits under its `if __name__ == "__main__"`. The pty plumbing, the transcript squeeze and
# the check machinery are its (and `suspend-test.py`'s beneath it), not a third copy: a second
# `drain` or a second `squeezed` is the copy that goes stale (CLAUDE.md § Write function-based).
_spec = importlib.util.spec_from_file_location("picker_test", ROOT / "scripts/picker-test.py")
_picker = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_picker)
squeezed, S, run_binary, FRAME = _picker.squeezed, _picker.S, _picker.run_binary, _picker.FRAME
holds, broken = _picker.holds, _picker.broken

# The object every journey is about, and the namespace the run is scoped to.
NAME = os.environ.get("K8RS_CONFIRM_OBJECT", "broken-quota")
NS = os.environ.get("K8RS_CONFIRM_NAMESPACE", "k8rs-quota")
OBJECT = f"deployment/{NAME}"
# What the title bar draws — `namespace/name`, never `kind/name` (`screens/dialogs.md` rule 1).
TITLED = f"Restart {NS}/{NAME}"

# `ops::OWNER_ONLY`, the mode `ops::open_log` opens the trail at. Written here because it is the
# security gate's own row (*the audit log is mode 0600 and append-only*) and the number is what the
# row claims; `ops.rs` is where it is spelled for the product.
OWNER_ONLY = 0o600
# `ratatui::crossterm`'s reading of these bytes: `esc`, `ctrl-d`, `⏎`.
ESC, CTRL_D, ENTER = b"\x1b", b"\x04", b"\r"

# --- the checks -----------------------------------------------------------
#
# One row is one claim, and the row is both the assertion and its own plant, exactly as
# `suspend-test.py` § the checks and `picker-test.py` lay out: `--self-test` breaks the fact the row
# reads and asserts that *this* row goes red.
#
#   (leg, "what is claimed", key, kind, argument)
#
# `leg` is which journey collected the fact, and it is what lets one table serve three modes: a run
# reads only the rows whose leg it drove, so a mode-specific row can never go red for not having
# been run (and `--self-test` reads every row, in every leg).
CHECKS = [
    # --- the console the journey starts from, so every "not in" below has a canary ------------
    ("cancel", "the console drew a frame before anything was pressed", "open", "in", S(FRAME)),
    ("cancel", "the console took the alternate screen", "open.raw", "in", _picker.ALT_ON),
    ("cancel", "the card the journey is about is the one on screen", "open", "in", S(NAME)),
    ("cancel", "and the footer offers the key this whole box is about", "open", "in",
     S("r restart")),
    ("cancel", "nothing was confirmed before `r`, so the strip teaches no restart yet",
     "open", "not in", S("rollout restart")),

    # --- `r`: what the confirmation actually says (`screens/dialogs.md` § Restart) -------------
    ("cancel", "`r` opened a box titled for the operation and the object", "press", "in",
     S(TITLED)),
    # **Three needles over one sentence, each inside one drawn row** (see the header's own
    # interleaving note): the box wraps the consequence and the frame underneath bleeds between the
    # rows, so the sentence as `ops::rollout` spells it can never match. The second row is what
    # makes this the *deployment* consequence and not the statefulset's, which shares the first
    # clause word for word and diverges at `a new one, working down from…`.
    ("cancel", "the consequence is the restart sentence, in plain language", "press", "in",
     S("This asks Kubernetes to replace every copy")),
    ("cancel", "it says the pacing is the object's own setting rather than claiming a number",
     "press", "in", S("How many stop at the same time is a setting on")),
    ("cancel", "and it is the deployment's own consequence, not a statefulset's or a daemonset's",
     "press", "in", S("it can be a few, or all of them at once.")),
    ("cancel", "and it warns what a paused deployment does, which only this kind carries",
     "press", "in", S("A paused deployment will not start until you resume it.")),
    ("cancel", "the dry-run verdict is in the box before the button is live", "press", "in",
     S("The cluster checked it first and accepted it.")),
    ("cancel", "the box never claims a check it did not make", "press", "not in",
     S("k8rs did not check this one")),
    # **The two needles that page promises are never cut** (§ The command log's own line): the
    # command's own head, and its `kind/name` word. Everything between them is a flag and gives way.
    ("cancel", "the box teaches the command it will run", "press", "in", S("$ kubectl")),
    ("cancel", "the taught command is a rollout restart", "press", "in", S("rollout restart")),
    # **The `kind/` and never the name** (§ When the object's own name does not fit): once `-n` and
    # `--context` are bare the name front-cuts, so `deployment/broken-quota` is a string that line
    # does not draw at this width. `deployment/` is what the page promises is never given up, and it
    # appears nowhere else on the frame — the strip's own two lines are `get statefulsets` and
    # `get daemonsets`. The object in full is the **title**'s row, above.
    ("cancel", "the taught line keeps the kind in front of the name, which never gives way",
     "press", "in", S("rollout restart deployment/")),
    ("cancel", "the taught line carries no --dry-run, which kubectl rollout restart has not",
     "press", "not in", S("--dry-run")),
    ("cancel", "both buttons are drawn, and the confirm one is a press", "press", "in",
     S("[ ⏎ do it ]")),
    ("cancel", "the way out is drawn beside it", "press", "in", S("[ esc cancel ]")),
    ("cancel", "the footer names the two keys that are live and no others", "press", "in",
     S("⏎ do it  esc cancel")),
    ("cancel", "a restart never asks for a name to be typed", "press", "not in",
     S("type the name to enable")),

    # **The strip under an open box is unchanged from before `r` was pressed** (rule 7 and
    # § Restart's own mockup). It is read here rather than as *no restart line yet*: the box's own
    # `$` row teaches that same command, and a squeezed transcript cannot tell the two apart — the
    # row that can is `escape`'s, below, with the box gone.
    #
    # **What is on the strip is the last *two* manifest lines and not the read the mockup draws**
    # (NOTES § D301): `ui::LOG_LINES` is 2 and `main::command_log` appends the five watches
    # pods-first, so the window holds `statefulsets` and `daemonsets` and a `get pods` line is never
    # on screen at connect. Measured against a real cluster, 2026-09-28, which is also that box's
    # independent confirmation from the other side.
    ("cancel", "the strip under the open box holds the two reads its window can show",
     "press", "in", S(f"get statefulsets -n {NS} --watch")),
    ("cancel", "and the second of them, so the window is two rows and not one", "press", "in",
     S(f"get daemonsets -n {NS} --watch")),
    ("cancel", "oldest first, which is the order the window slides in", "press", "before",
     (S(f"get statefulsets -n {NS} --watch"), S(f"get daemonsets -n {NS} --watch"))),
    ("cancel", "and the read pushed first is already off the window (NOTES § D301)",
     "press", "not in", S(f"get pods -n {NS} --watch")),

    # --- a key the box does not offer: D22's guard is about what goes out, and nothing does ---
    ("cancel", "a key the box does not offer leaves the box open", "stray", "in", S(TITLED)),
    ("cancel", "and the buttons still waiting", "stray", "in", S("[ ⏎ do it ]")),
    # **The one observable difference between *sent* and *not sent* while a box is open**: a call in
    # flight replaces Alerts' whole footer with `· changing <name> first`
    # (`screens/dialogs.md` § While the call is running), and nothing else on screen moves.
    ("cancel", "a key the box does not offer started no call, so nothing is changing",
     "stray", "not in", S("changing")),

    # --- `esc`: the refusal (`screens/dialogs.md` rule 7, NOTES § D233 ruling 1) ---------------
    ("cancel", "esc closed the box and gave the console back", "escape", "in", S(FRAME)),
    ("cancel", "the box is gone rather than redrawn", "escape", "not in", S(TITLED)),
    # **The one that catches an append on dialog-open.** With the box closed, the only place a
    # taught restart could be drawn is the command-log strip, and a refusal appends nothing at all.
    ("cancel", "a refused restart left no line on the command log", "escape", "not in",
     S("rollout restart")),
    ("cancel", "and no outcome word about a call that never went out", "escape", "not in",
     S("→ done")),
    ("cancel", "nothing is changing once the box has been refused", "escape", "not in",
     S("changing")),
    ("cancel", "the run ended when the reader quit", "cancel.exited", "on", None),

    # --- the audit log, as a file (the security gate's own rows, invariant 2, NOTES § D21) -----
    ("cancel", "a refusal reached the audit log", "cancel.lines", "is", 2),
    ("cancel", "the trail opens with the attempt and not with the result (NOTES § D21)",
     "cancel.attempt", "in", "attempt"),
    ("cancel", "the attempt names the object it is about", "cancel.attempt", "in", OBJECT),
    ("cancel", "the attempt names the namespace it was in", "cancel.attempt", "in",
     f"namespace {NS}"),
    ("cancel", "the attempt records the equivalent kubectl line — invariant 4's first record",
     "cancel.attempt", "in", "kubectl: kubectl"),
    ("cancel", "and the real call beside it, which is a PATCH and not that command",
     "cancel.attempt", "in", f"call: PATCH /apis/apps/v1/namespaces/{NS}/deployments/{NAME}"),
    ("cancel", "a restart sends no resourceVersion precondition (NOTES § D228)",
     "cancel.attempt", "in", "resourceVersion not sent"),
    # **One half of `ops::which_uid`'s split, and the half that makes the other one mean
    # something** (NOTES § D223 ruling 3): `ops::restart` reads nothing before it patches, so there
    # is no `uid` to give and `PatchParams` has nowhere to send one. Without this row the delete's
    # own clause below would pass just as well on a day every attempt grew a uid.
    ("cancel", "a restart read no uid, so its record claims no condition on the change",
     "cancel.attempt", "in", "no uid was read"),
    ("cancel", "the result line says nobody confirmed it", "cancel.result", "in",
     "nobody confirmed it, so nothing was changed"),
    ("cancel", "the result records the dry-run verdict that really went out", "cancel.result",
     "in", "dry-run: the cluster checked it first and accepted it"),
    ("cancel", "and it never claims the change was made", "cancel.result", "not in",
     "the change was made"),
    ("cancel", "the audit log is readable and writable by its owner and nobody else",
     "audit.mode", "is", OWNER_ONLY),

    # --- `ctrl-d`: the typed-name half of the same arm, answered with a near miss --------------
    ("typed", "ctrl-d opened a box that asks for the name to be typed back", "typed.press", "in",
     S("type the name to enable")),
    ("typed", "its confirm button prints the operation's own word instead of a key",
     "typed.press", "in", S("[ delete ]")),
    ("typed", "a delete says it checked nothing with the cluster first", "typed.press", "in",
     S("k8rs did not check this one with the cluster first.")),
    ("typed", "and never borrows the sentence an operation that did check prints",
     "typed.press", "not in", S("checked it first and accepted")),
    ("typed", "a name one character short leaves the button dead", "typed.near", "in",
     S("type the name to enable")),
    ("typed", "so ⏎ on a near miss sends nothing and the box is still open", "typed.enter", "in",
     S("type the name to enable")),
    ("typed", "and started no call, so nothing is changing", "typed.enter", "not in",
     S("changing")),
    ("typed", "esc on it gave the console back", "typed.escape", "in", S(FRAME)),
    # **The needle is the command's own `kind/name` word and not `kubectl delete`** — every taught
    # line carries `--context` between the two, so `kubectl delete` is a string the product never
    # writes and a row reading for it could not have failed however wrong the screen was. `S()`
    # squeezes the needle, not the line's flags out of it (my own second pass; the self-test proves
    # a row goes red on a *planted* fact and can say nothing about a needle nothing ever draws).
    ("typed", "a refused delete left no line on the command log either", "typed.escape",
     "not in", S(f"delete {OBJECT}")),
    ("typed", "the refused delete reached the audit log as its own attempt and result",
     "typed.lines", "is", 4),
    ("typed", "its result says nobody confirmed it", "typed.result", "in",
     "nobody confirmed it, so nothing was changed"),
    ("typed", "a delete's attempt records the DELETE it would have sent", "typed.attempt", "in",
     f"call: DELETE /apis/apps/v1/namespaces/{NS}/deployments/{NAME}"),
    # **The other half of the split, and the needle is the *sent* arm's own clause.**
    # `ops::which_uid` has three answers — a uid that goes out as a precondition, a uid that was
    # only read, and none — so this parenthesis says both *there is a uid* and *it is a condition
    # the cluster will enforce*, which a bare `uid ` could not. Read beside the restart's
    # `no uid was read`, above: one operation conditions the change on identity and the other has
    # nothing to condition it with, and the audit log is where that is visible.
    ("typed", "a delete conditions the change on the object's own identity, and says so",
     "typed.attempt", "in",
     "(a condition on the change — the cluster does not make it unless the object is this one)"),
    # **D225 ruling 1, in the record rather than in a unit test**: `delete` is the one operation
    # that sends no `dryRun=All` — a dry-run delete and a real one build the identical request
    # line — and it declines it *where a reader can see it*. The accepted half is pinned beside it
    # on `cancel.result`, above; *declined* says nothing on its own.
    ("typed", "a delete's record says k8rs checked nothing with the cluster first",
     "typed.result", "in", "dry-run: k8rs did not check this one with the cluster first"),
    ("typed", "and never borrows the verdict of an operation that really did check",
     "typed.result", "not in", "the cluster checked it first"),
    # **Append-only, read as a fact about the bytes** — the second run's log still opens with every
    # line the first one wrote, rather than a file truncated and started again.
    ("typed", "the second run appended to the trail rather than replacing it", "audit.appended",
     "on", None),

    # --- `--read-only`: unreachable, not merely unbound (invariant 2, dialogs.md rule 6) -------
    ("readonly", "--read-only drew a console to press the key in", "readonly.open", "in",
     S(FRAME)),
    ("readonly", "and said so, so the run under test is the one the flag was given to",
     "readonly.open", "in", S("read-only")),
    ("readonly", "its footer offers no restart key to press", "readonly.open", "not in",
     S("r restart")),
    ("readonly", "`r` under --read-only opens no confirmation at all", "readonly.press",
     "not in", S(TITLED)),
    ("readonly", "and teaches no command", "readonly.press", "not in", S("rollout restart")),
    ("readonly", "--read-only opened no audit log, because there is nothing to record",
     "readonly.audit", "off", None),

    # --- the object itself, read off the cluster: nothing was changed --------------------------
    ("object", "the deployment is byte for byte what it was before the journeys ran",
     "object.same", "on", None),

    # --- `--confirm`: the one journey that really sends it (the PM's) --------------------------
    ("confirm", "a confirmed restart teaches the command it really sent", "confirm.answer", "in",
     S("rollout restart")),
    ("confirm", "and the command log's line gained its outcome", "confirm.answer", "in",
     S("→ done")),
    # **The in-flight footer is not this harness's to read, and the arithmetic is why.** `perform`
    # writes the attempt line *before* the dry-run and before the box waits for a human, so the
    # 14.257 s between attempt and result on the real run (2026-09-28) is almost all of it this
    # script's own 12 s `press` drain plus `repainted`'s 2.1 s of resizes: `⏎` landed at +14.1 s and
    # the result was stamped at +14.257 s, so **the real PATCH was out for ~150 ms**. `repainted`
    # cannot force a frame inside that, and the only other fact a step leaves is the diff stream —
    # which is the one thing this file's header says a content needle may not be read off.
    #
    # **It is also already pinned, by the one thing that can pin it**: `App::footer` is a pure
    # function over `App`, and `views_tests.rs` asserts the whole literal
    # (`↑↓ move  ⏎ open  ? keys  ·  changing payments/web first`), that `q quit` is gone with it,
    # the empty-name case, and that the arm is `changing`'s and never the argument's. That is
    # `picker-test.py`'s own ruling about the 80×24 claim, one screen along: a state that lives for
    # 150 ms belongs to `TestBackend` in the suite.
    #
    # **What is left for a pty, and it is a property no unit test drives**: the marker is *cleared*
    # when the call returns. `main::settled` takes `App::changing` on every terminal path of
    # `ops::perform`, and a footer still claiming a call is out is one lying about the cluster.
    # **Not vacuous, and the row above it is the canary**: `views::Log::outcome` writes `→ done`
    # only where something is `waiting`, and `views::Log::sent` is the only thing that sets that —
    # in the same three lines of `over_modal`'s confirm arm that set `changing`. So `→ done` on the
    # frame *is* the proof the marker was set, and this row is the proof it did not survive.
    ("confirm", "and the in-flight marker did not outlive the call that set it",
     "confirm.answer", "not in", S("changing")),
    ("confirm", "the box closed on confirmation rather than on completion", "confirm.answer",
     "not in", S("[ ⏎ do it ]")),
    ("confirm", "the real call was recorded as well as the command", "confirm.attempt", "in",
     f"call: PATCH /apis/apps/v1/namespaces/{NS}/deployments/{NAME}"),
    ("confirm", "the result line says the change was made", "confirm.result", "in",
     "the change was made"),
    ("confirm", "and never that nobody confirmed it", "confirm.result", "not in",
     "nobody confirmed it"),
    ("confirm-object", "the deployment really moved, so the write was not a no-op",
     "confirm.moved", "on", None),

    # --- `--gone`: D22's guard, with the object deleted while the box was open -----------------
    ("gone", "the object was really gone before ⏎ was pressed", "gone.vanished", "on", None),
    ("gone", "the box that replaced it names the outcome rather than the operation",
     "gone.answer", "in", S("Already gone")),
    # **Short, and now measured rather than guessed.** The four rows around these read things the
    # renderer never wraps — the title bar, the identity line, `Nothing was changed.` and the
    # button — but this box's body is a sentence `ui::margined` breaks at a word boundary, and the
    # frame underneath bleeds into the break (the header's own interleaving note). Measured
    # 2026-09-28: the real box wraps after `something else` and after `Nothing will take`, with the
    # sidebar's `cluster` row landing in the second of those — so `take its place on its own`
    # could not match and `its place on its own` is the same claim inside one drawn row. The
    # sample below is that frame, fragments included.
    ("gone", "it says what happened, in the words a deployment gets", "gone.answer", "in",
     S("This deployment is already gone")),
    ("gone", "and that nothing puts a deployment back on its own", "gone.answer", "in",
     S("its place on its own")),
    ("gone", "it names what disappeared", "gone.answer", "in", S(f"{NS}/{NAME}")),
    ("gone", "it states the outcome the audit log also records", "gone.answer", "in",
     S("Nothing was changed.")),
    ("gone", "its one way out is a dismiss, because there is nothing left to confirm",
     "gone.answer", "in", S("[ esc dismiss ]")),
    ("gone", "and no confirm button survived beside it", "gone.answer", "not in",
     S("[ ⏎ do it ]")),
    # **The guard decided before the strip gained anything** (`screens/dialogs.md` § The object went
    # away: *"the strip appends nothing here at all"*, NOTES § D289 ruling 1) — a line here would be
    # invariant 4 with a record about a call that never left.
    ("gone", "nothing was taught, because nothing was sent", "gone.answer", "not in",
     S("rollout restart")),
    ("gone", "the result line says the object was already gone", "gone.result", "in",
     "the object was already gone, so nothing was changed"),
    ("gone", "and never that the change was made", "gone.result", "not in",
     "the change was made"),
]

# Which rows a mode reads. The default is every leg that writes nothing into the cluster.
LEGS = {
    "": ("cancel", "typed", "readonly", "object"),
    "--confirm": ("cancel", "typed", "readonly", "confirm", "confirm-object"),
    "--gone": ("gone",),
}


def verdicts(observed: dict, legs) -> list[tuple[str, bool]]:
    """Every row of these legs read against what was observed.

    A fact the run never collected — the child died halfway — reads as the empty string and fails
    its own row by name, rather than raising where the verdicts should have been."""
    return [(what, holds(kind, observed.get(key, ""), arg))
            for leg, what, key, kind, arg in CHECKS if leg in legs]


# --- the preflight --------------------------------------------------------
#
# A pure function over facts somebody else gathered, so `--self-test` drives every refusal with no
# fake `kubectl` and no re-exec. It is the loudest half of this script: a `just confirm` that exits
# 0 because there was no cluster, no object or no card proves nothing, and `--confirm` typed in the
# wrong terminal would restart a Deployment somebody depends on.


def refusal(context: str, answered: bool, exists: bool, writing: bool,
            acknowledged: bool) -> str | None:
    """Why this run may not go ahead, or `None`.

    **The context is the binary's, not this script's** — a console takes `--context`, but the one
    the journeys use is whatever is current, and following it is the only honest thing to do.
    Refusing anything that is not kind is what stops `--confirm`, typed in the wrong terminal,
    pointing a real restart at somebody's production cluster (`scripts/e2e.sh`'s own reason)."""
    if not context:
        return ("confirm: no kubeconfig context is current, so there is no apiserver to prove "
                "anything against — 'just cluster-up' brings the test cluster up")
    if not context.startswith("kind-"):
        return (f"confirm: the current context is '{context}' and this only runs against kind — "
                f"the journeys press a key that mutates, so a wrong context is a wrong cluster. "
                f"Switch with 'kubectl config use-context kind-k8rs'.")
    if not answered:
        return (f"confirm: '{context}' is the current context but its apiserver did not answer — "
                f"'just cluster-up' brings it back")
    if not exists:
        return (f"confirm: there is no {OBJECT} in {NS} on '{context}' — this needs the workload "
                f"scripts/broken.yaml creates, and without it no Alerts card is selectable, so "
                f"every key below would reach nothing and say nothing")
    if writing and not acknowledged:
        return ("confirm: --confirm really restarts " + OBJECT + " in " + NS + ", which is a "
                "write into the cluster — set K8RS_CONFIRM_WRITE=yes to say that is what you "
                "meant, and run 'scripts/cluster.sh reset' afterwards")
    return None


# --- the cluster, read only -----------------------------------------------


def kubectl(*words: str) -> tuple[int, str]:
    """`kubectl` with an argument vector and never a command string (the security gate's own row)."""
    try:
        done = subprocess.run(["kubectl", *words], capture_output=True, text=True, timeout=60,
                              check=False)
    except (OSError, subprocess.TimeoutExpired):
        # A `kubectl` that is not installed, or one that hung, reads as a refusal the preflight
        # names — never a traceback where the verdicts should have been.
        return 127, ""
    return done.returncode, done.stdout


def witness(context: str) -> str | None:
    """What *the object did not change* is read off, and what is deliberately not in it.

    `status` and `metadata.resourceVersion` both move when the Deployment's own controller writes,
    which falsifies nothing anybody agreed to (NOTES § D228) — a witness carrying either would go
    red on a healthy cluster. `generation` moves on a spec write, which is what a restart is."""
    code, out = kubectl("--context", context, "get", "deployment", NAME, "-n", NS, "-o", "json")
    if code != 0:
        return None
    held = json.loads(out)
    return json.dumps({"uid": held["metadata"].get("uid"),
                       "generation": held["metadata"].get("generation"),
                       "deleting": held["metadata"].get("deletionTimestamp"),
                       "spec": held.get("spec")}, sort_keys=True)


def kubeconfig() -> Path:
    """The reader's own kubeconfig, which is the only one that can reach the cluster.

    `KUBECONFIG` may name several files; `Path` keeps the string whole and `run_binary` hands it to
    the child as it is."""
    return Path(os.environ.get("KUBECONFIG") or (Path.home() / ".kube/config"))


# --- the audit log, as a file ---------------------------------------------


def trail(state: Path) -> Path:
    return state / "k8rs/audit.log"


def recorded(state: Path) -> list[str]:
    """The lines in the trail, or none at all where there is no trail."""
    path = trail(state)
    if not path.exists():
        return []
    return [line for line in path.read_text(encoding="utf-8", errors="replace").splitlines()
            if line.strip()]


def lined(state: Path, at: int) -> str:
    """One line of the trail, or the empty string — which fails its own row rather than raising."""
    lines = recorded(state)
    return lines[at] if -len(lines) <= at < len(lines) else ""


def whole(state: Path) -> str:
    """The trail's bytes, or nothing at all — a run whose binary never started is a red row."""
    path = trail(state)
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ""


def mode_of(state: Path) -> int:
    """The trail's permission bits, or `-1` — a number no row can pass, rather than zero."""
    path = trail(state)
    try:
        return stat.S_IMODE(path.stat().st_mode)
    except OSError:
        return -1


# --- the journeys ---------------------------------------------------------


def opened(binary: Path, config: Path, state: Path, args: list[str], keys) -> dict:
    """One run of the binary, with its own state directory for the trail it writes.

    `run_binary` is `picker-test.py`'s and sets `KUBECONFIG` on the child; `$XDG_STATE_HOME` is set
    here, in the parent, so the child inherits it — and no journey can ever append to the reader's
    own audit log, which is what makes every count above a fact about this run."""
    os.environ["XDG_STATE_HOME"] = str(state)
    return run_binary(binary, args, config, keys)


def journeys(binary: Path, config: Path, work: Path, context: str, modes: set[str]) -> dict:
    """Every journey the modes asked for, in the order the checks read them."""
    seen: dict = {}
    shared = work / "shared"

    # **Every content fact is read off the repaint, not the stream**: a diff-based renderer writes
    # only the cells that changed, so a needle spanning a cell that did not change can never appear
    # in the bytes however right the screen is (`picker-test.py` § `repainted`).
    def take(run: dict, step: str, key: str) -> None:
        # `get` and not an index: a journey whose generator ended early left no fact for this step,
        # and the empty string fails that step's own row rather than raising here.
        seen[key] = squeezed(run.get(step, ""))

    if "--gone" not in modes:
        # --- `r`, a key the box does not offer, then `esc` --------------------------------------
        #
        # **`press` waits past `ops::CHECK_DEADLINE`'s own window.** The box is drawn before the
        # check goes out (`screens/dialogs.md` rule 3), and the verdict row this reads only exists
        # once the cluster has answered — milliseconds on kind, 35 seconds at the bound.
        walked = opened(binary, config, shared, ["--namespace", NS], [
            ("open", b"", 10.0),
            ("press", b"r", 12.0),
            # A printable the box has no arm for: not `⏎`, not `esc`, and a restart asks for no
            # text, so this falls through to `Did::Nothing` and must leave the wire alone.
            ("stray", b"y", 2.0),
            ("escape", ESC, 3.0),
            ("quit", b"q", 2.0),
        ])
        seen["open.raw"] = walked["open.stream"]
        for step in ("open", "press", "stray", "escape"):
            take(walked, step, step)
        seen["cancel.exited"] = walked["exited"]
        seen["cancel.lines"] = len(recorded(shared))
        seen["cancel.attempt"], seen["cancel.result"] = lined(shared, 0), lined(shared, 1)
        seen["audit.mode"] = mode_of(shared)
        first = whole(shared)

        # --- `ctrl-d`, a name one character short, `⏎`, then `esc` -----------------------------
        #
        # **The same state directory**, which is what makes the append-only row a fact about bytes
        # rather than about a mode. A delete sends no check at all (NOTES § D225 ruling 1), so its
        # box is complete from the moment it opens and needs no window.
        near = NAME[:-1] or NAME
        typed = opened(binary, config, shared, ["--namespace", NS], [
            ("open", b"", 10.0),
            ("press", CTRL_D, 4.0),
            ("near", near.encode(), 2.0),
            ("enter", ENTER, 3.0),
            ("escape", ESC, 3.0),
            ("quit", b"q", 2.0),
        ])
        for step in ("press", "near", "enter", "escape"):
            take(typed, step, f"typed.{step}")
        seen["typed.lines"] = len(recorded(shared))
        seen["typed.attempt"], seen["typed.result"] = lined(shared, 2), lined(shared, 3)
        # **`first` has to be something, or this row is vacuous**: every string starts with the
        # empty one, so a run that wrote nothing at all would pass *appended rather than replaced*
        # (CLAUDE.md § A derived list asserts it found something).
        seen["audit.appended"] = bool(first) and whole(shared).startswith(first)

        # --- the same key under `--read-only` ---------------------------------------------------
        #
        # Its own state directory, because *no audit log was opened at all* is the assertion and a
        # shared one would already hold a file.
        locked = work / "locked"
        held = opened(binary, config, locked, ["--read-only", "--namespace", NS], [
            ("open", b"", 10.0),
            ("press", b"r", 4.0),
            ("quit", b"q", 2.0),
        ])
        take(held, "open", "readonly.open")
        take(held, "press", "readonly.press")
        seen["readonly.audit"] = (locked / "k8rs").exists()

    if "--confirm" in modes:
        before = witness(context)
        confirmed = opened(binary, config, work / "sent", ["--namespace", NS], [
            ("open", b"", 10.0),
            ("press", b"r", 12.0),
            ("answer", ENTER, 12.0),
            ("quit", b"q", 2.0),
        ])
        take(confirmed, "answer", "confirm.answer")
        seen["confirm.attempt"] = lined(work / "sent", 0)
        seen["confirm.result"] = lined(work / "sent", 1)
        seen["confirm.moved"] = before is not None and witness(context) != before

    if "--gone" in modes:
        gone = work / "gone"
        state: dict = {}

        # **The delete is somebody else's to run** (D92), so this waits for it rather than sending
        # it — and it decides *between two keystrokes* whether the key it presses next is `⏎` at
        # all. **A generator is what expresses that**, because `run_binary` walks whatever it is
        # handed and yields nothing back: a callable in the key list it would try to write, and a
        # hook in `picker-test.py` is another box's file.
        #
        # **It will not press `⏎` on a hunch.** The cluster is read while the box is still open, and
        # a run whose delete never happened presses `esc` instead — otherwise the one key this
        # journey exists to test would send a real restart to a live object.
        def walking():
            yield ("open", b"", 10.0)
            yield ("press", b"r", 12.0)
            # Reached after that step's repaint, so the box is drawn and its verdict has landed.
            print(f"\nconfirm --gone: the box is open on {OBJECT} in {NS}. Now, in another "
                  f"terminal:\n\n    kubectl --context {context} delete deployment/{NAME} "
                  f"-n {NS}\n")
            try:
                input("confirm --gone: press Enter *here* once that command has returned. ")
            except EOFError:
                # Nobody is at this end, so nobody ran the delete — which is exactly the case the
                # check below refuses to press `⏎` in.
                print("confirm --gone: stdin is closed, so nobody could have run the delete.",
                      file=sys.stderr)
            state["vanished"] = kubectl(
                "--context", context, "get", "deployment", NAME, "-n", NS)[0] != 0
            if not state["vanished"]:
                print("confirm --gone: the deployment is still there, so ⏎ would send a real "
                      "restart. Pressing esc instead.", file=sys.stderr)
                yield ("answer", ESC, 3.0)
            else:
                # The watch has to carry the delete into the store before `main::vanished` can read
                # it, and the box stays open for as long as nobody presses anything.
                yield ("settle", b"", 3.0)
                yield ("answer", ENTER, 12.0)
            # Two steps and not one write: sent together, crossterm reads `\x1bq` as one sequence
            # and the run does not end (`picker-test.py`'s own measurement).
            yield ("dismiss", ESC, 1.5)
            yield ("quit", b"q", 2.0)

        walked = opened(binary, config, gone, ["--namespace", NS], walking())
        take(walked, "answer", "gone.answer")
        seen["gone.vanished"] = bool(state.get("vanished"))
        seen["gone.result"] = lined(gone, 1)
    return seen


# --- the run --------------------------------------------------------------


def run(binary: Path, modes: set[str]) -> int:
    if not binary.exists():
        print(f"confirm: {binary} is not built — `cargo build` first, or `just confirm`, which "
              f"does it for you", file=sys.stderr)
        return 1
    context = kubectl("config", "current-context")[1].strip()
    answered = bool(context) and kubectl("--context", context, "get", "--raw", "/version")[0] == 0
    exists = answered and kubectl(
        "--context", context, "get", "deployment", NAME, "-n", NS)[0] == 0
    said = refusal(context, answered, exists, "--confirm" in modes,
                   os.environ.get("K8RS_CONFIRM_WRITE") == "yes")
    if said:
        print(said, file=sys.stderr)
        return 1

    legs = LEGS["--gone" if "--gone" in modes else
                "--confirm" if "--confirm" in modes else ""]
    with tempfile.TemporaryDirectory(prefix="k8rs-confirm-") as directory:
        work = Path(directory)
        before = witness(context)
        observed = journeys(binary, kubeconfig(), work, context, modes)
        if "object" in legs:
            observed["object.same"] = before is not None and witness(context) == before
        read = verdicts(observed, legs)
        failed = [what for what, ok in read if not ok]
        for what, ok in read:
            print(("  ok   " if ok else "  FAIL ") + what)
        for label, key in (("the confirmation `r` opened", "press"),
                           ("the console after esc", "escape"),
                           ("the typed-name box on a near miss", "typed.enter"),
                           ("the run under --read-only", "readonly.press"),
                           ("what the confirmed restart drew", "confirm.answer"),
                           ("the already-gone box", "gone.answer")):
            if key in observed:
                print(f"--- {label} (escapes and spacing stripped) ---")
                print(str(observed[key])[-900:])
        print("--- the audit trail ---")
        for state in ("shared", "sent", "gone"):
            for line in recorded(work / state):
                print(f"  {line}")
    print(f"confirm: {context} — {len(read)} check(s), {len(failed)} failure(s) — "
          f"{'OK' if not failed else 'FAILED: ' + '; '.join(failed)}")
    return 1 if failed else 0


# --- the self-test --------------------------------------------------------


def healthy() -> dict:
    """A transcript of a journey where every door worked, built out of the rows' own constants.

    **It models the frame underneath, and that is the whole point of the shape** (see the module
    header). A modal here is drawn over a console with a sidebar and a card behind it, so the
    squeeze puts foreign text *between* the halves of every wrapped line; a sample built out of
    clean rows is green for needles the real screen can never satisfy, which is exactly how three
    checks in this file reached a cluster before anything said no (measured 2026-09-28, 3 of 60).
    [`bleeding`] is what puts it back — the fragments are the ones that real run produced."""
    # The sidebar's own rows, cut where the dialog's box covers them, exactly as they arrived.
    LEAKS = ["netwo", "stora", "outconfi", "clust", "en", "ANALYS", "ut", "capac", "certi",
             "drain", "posture", "sta", "waste", "versions"]

    def bleeding(rows: list[str], leaks: list[str] | None = None) -> str:
        """Each drawn row followed by whatever of the frame underneath shared its terminal row.

        `leaks` is the fragments a *measured* frame really put there, in order, for a sample built
        off one; without it the module's own list cycles, which keeps the shape — a fragment between
        every pair of drawn rows — without claiming to be any particular frame."""
        beside = (lambda at: leaks[at] if at < len(leaks) else "") if leaks is not None \
            else (lambda at: LEAKS[at % len(LEAKS)])
        return squeezed("".join(f"{row}{beside(at)}\n" for at, row in enumerate(rows)))

    # **The two lines the strip's window actually holds** (NOTES § D301): `ui::LOG_LINES` is 2 and
    # `main::command_log` pushes the five watches pods-first, so these are the last two and a
    # `get pods` line is never on screen.
    STRIP = [f"$ kubectl --context kind-k8rs get statefulsets -n {NS} --watch",
             f"$ kubectl --context kind-k8rs get daemonsets -n {NS} --watch"]

    def console(footer: str = "↑↓ move  ⏎ open  r restart  ? all keys  q quit") -> str:
        return bleeding([f" nodes 4/4  ctx: kind-k8rs · ns: {NS} · live · admin",
                         f" ▸{FRAME} 1 ● ● {NS}/{NAME}  33s ago",
                         " This rollout gave up — Kubernetes has stopped work",
                         *STRIP, f" {footer}"])

    def box(verdict: str) -> str:
        return bleeding([
            f"  ┌ {TITLED} ─",
            "  │  This asks Kubernetes to replace every copy of your app with",
            "  │  a new one. How many stop at the same time is a setting on",
            "  │  this deployment — it can be a few, or all of them at once.",
            "  │  A paused deployment will not start until you resume it.",
            f"  │  {verdict}",
            f"  │  $ kubectl --context… rollout restart deployment/…-quota -n…",
            "  │            [ ⏎ do it ]    [ esc cancel ]",
            *STRIP, " ⏎ do it  esc cancel"])

    def deleting(field: str) -> str:
        return bleeding([
            f"  ┌ Delete {NS}/{NAME} ─",
            "  │  This asks Kubernetes to remove it. Something there may delay",
            "  │  this or act first.",
            "  │  k8rs did not check this one with the cluster first.",
            f"  │  $ kubectl --context… delete deployment/…-quota -n…",
            "  │            [ delete ]    [ esc cancel ]",
            *STRIP, f"  {field}"])

    checked = box("The cluster checked it first and accepted it.")
    asks = deleting("type the name to enable  esc cancel")
    attempt = (f"2026-09-28T09:00:00Z attempt · {OBJECT} · context kind-k8rs · server "
               f"https://127.0.0.1:6443 · namespace {NS} · no uid was read · kubectl: kubectl "
               f"--context kind-k8rs rollout restart {OBJECT} -n {NS} · call: PATCH "
               f"/apis/apps/v1/namespaces/{NS}/deployments/{NAME} · resourceVersion not sent")
    return {
        "open.raw": _picker.ALT_ON, "open": console(),
        "press": checked, "stray": checked,
        "escape": console(), "cancel.exited": True,
        "cancel.lines": 2, "cancel.attempt": attempt,
        "cancel.result": ("result · attempt 2026-09-28T09:00:00Z · recorded "
                          f"2026-09-28T09:00:04Z · {OBJECT} · dry-run: the cluster checked it "
                          "first and accepted it · nobody confirmed it, so nothing was changed"),
        "audit.mode": OWNER_ONLY,
        "typed.press": asks, "typed.near": asks, "typed.enter": asks,
        "typed.escape": console(), "typed.lines": 4,
        # The delete's own two differences from the restart's line, and they are the whole of what
        # the four rows above read: the verb it would have sent, and the `uid` it conditions on.
        "typed.attempt": attempt.replace(
            f"call: PATCH /apis/apps/v1/namespaces/{NS}/deployments/{NAME}",
            f"call: DELETE /apis/apps/v1/namespaces/{NS}/deployments/{NAME}").replace(
            "no uid was read",
            "uid 9feb5257-0000-4000-8000-000000000000 (a condition on the change — the cluster "
            "does not make it unless the object is this one)"),
        "typed.result": ("result · attempt 2026-09-28T09:01:00Z · recorded "
                         f"2026-09-28T09:01:00Z · {OBJECT} · dry-run: k8rs did not check this one "
                         "with the cluster first · nobody confirmed it, so nothing was changed"),
        "audit.appended": True,
        "readonly.open": bleeding([f" nodes 4/4  ctx: kind-k8rs · ns: {NS} · live · read-only",
                                   f" ▸{FRAME} 1 ● ● {NS}/{NAME}  33s ago",
                                   " ↑↓ move  ⏎ open  / filter  ? all keys  q quit"]),
        "readonly.press": bleeding([f" ▸{FRAME} 1 ● ● {NS}/{NAME}  33s ago",
                                    " ↑↓ move  ⏎ open  / filter  ? all keys  q quit"]),
        "readonly.audit": False,
        "object.same": True,
        # **The frame the harness really reads: after `settled()`, so no marker** — and on this
        # object no card either, because the restart cleared the very W2 finding the journey
        # selects (the `confirm-write` recipe's own warning). Modelled off the real run,
        # 2026-09-28. The strip's window has slid on by one, so the restart's own line is the
        # second of the two it holds.
        "confirm.answer": bleeding([
            f" nodes 4/4  ctx: kind-k8rs · ns: {NS} · live · admin",
            f" ▸{FRAME} ○ nothing is broken",
            " 0 pods and 4 nodes checked, none of them is in trouble right now.",
            f" $ kubectl --context kind-k8rs get daemonsets -n {NS} --watch",
            " $ kubectl --context kind-k8rs rollout restart deployment/…-quota -n…   → done",
            " ? all keys  q quit"]),
        "confirm.attempt": attempt,
        "confirm.result": ("result · attempt 2026-09-28T09:02:00Z · dry-run: the cluster checked "
                           "it first and accepted it · the change was made"),
        "confirm.moved": True,
        "gone.vanished": True,
        "gone.answer": bleeding([
            "  ┌ Already gone ─",
            "  │  This deployment is already gone — something else",
            "  │  removed it while this was open. Nothing will take",
            "  │  its place on its own.",
            f"  │    {NS}/{NAME}",
            "  │  Nothing was changed.",
            "  │            [ esc dismiss ]",
            *STRIP, " esc dismiss"],
            ["network storage", "config", "cluster", "ANALYSIS capacity",
             "certific drain sa", "posture restarts", "waste versions"]),
        "gone.result": ("result · attempt 2026-09-28T09:03:00Z · dry-run: the cluster checked it "
                        "first and accepted it · the object was already gone, so nothing was "
                        "changed"),
    }


def self_test() -> None:
    sample = healthy()
    keys = {key for _, _, key, _, _ in CHECKS}
    missing = sorted(key for key in keys if key not in sample)
    assert not missing, f"the healthy sample does not carry {missing}"
    unread = sorted(key for key in sample if key not in keys)
    assert not unread, f"the healthy sample carries {unread}, which no check reads"
    every = tuple(leg for legs in LEGS.values() for leg in legs)
    unrun = sorted({leg for leg, _, _, _, _ in CHECKS} - set(every))
    assert not unrun, f"no mode reads the {unrun} leg, so those rows never run"
    green = verdicts(sample, every)
    assert all(ok for _, ok in green), \
        f"the healthy transcript is not green: {[w for w, ok in green if not ok]}"

    for _, what, key, kind, arg in CHECKS:
        planted = dict(sample)
        planted[key] = broken(kind, sample[key], arg)
        went = [name for name, ok in verdicts(planted, every) if not ok]
        assert what in went, f"breaking {key!r} did not fail {what!r} — it fails nothing"
        print(f"confirm --self-test: refused — {what}")

    # **Every preflight refusal, driven directly** — the loud half. `refusal` is a function over
    # facts, so no fake `kubectl` and no re-exec is needed to reach any of them, and a run that
    # would have exited 0 over a missing cluster is what each of these names.
    walls = [
        ("no context", ("", False, False, False, False), "no kubeconfig context is current"),
        ("not kind", ("production", True, True, False, False), "only runs against kind"),
        ("apiserver down", ("kind-k8rs", False, False, False, False), "did not answer"),
        ("no object", ("kind-k8rs", True, False, False, False), f"there is no {OBJECT} in {NS}"),
        ("a write nobody asked for", ("kind-k8rs", True, True, True, False),
         "set K8RS_CONFIRM_WRITE=yes"),
    ]
    for name, facts, says in walls:
        said = refusal(*facts)
        assert said and says in said, f"the {name!r} wall does not say {says!r}: {said!r}"
        print(f"confirm --self-test: refused — the {name} wall says so")
    assert refusal("kind-k8rs", True, True, False, False) is None, \
        "a kind cluster with the object on it is refused, so no journey could ever run"
    assert refusal("kind-k8rs", True, True, True, True) is None, \
        "a write that was acknowledged is still refused, so --confirm could never run"
    print("confirm --self-test: refused — nothing at all when the cluster is ready, with and "
          "without the write acknowledged")

    print(f"confirm --self-test: {len(CHECKS)} check(s) and {len(walls)} wall(s), each red on its "
          f"own plant and green on the healthy transcript — OK")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    else:
        asked = {arg for arg in sys.argv[1:] if arg.startswith("--")}
        where = next((arg for arg in sys.argv[1:] if not arg.startswith("--")),
                     ROOT / "target/debug/k8rs")
        sys.exit(run(Path(where), asked))
