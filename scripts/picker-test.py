#!/usr/bin/env python3
"""Drive the real binary's startup picker on a real pty and read the terminal back.

**The part of *which cluster* no suite can hold** (todo.md § Phase 12, NOTES § D279). `console()`
is only entered by a run that is at a keyboard, and `cargo test`'s own ends are both pipes, so
`main` answers `USAGE` and never builds a runtime — the whole startup-picker journey is
unreachable from `cargo test`, exactly as the three stop doors are (`suspend-test.py`, whose pty
machinery and check machinery this reuses rather than copying).

**The doors, and they are not the same door:**

  · the startup picker — two or more contexts, no `--context`, a terminal: it is the first thing
    drawn, over genuinely nothing (`screens/context.md` § Opening at startup).
  · `⏎` on a row that cannot be connected with — the failure box, never a stderr line, because raw
    mode is already on (NOTES § D279 ruling 4).
  · `esc` on that box — the **same** startup picker back, not a running app
    (`screens/context.md` § The same failure, from the startup picker).
  · `esc` on the picker — quits the way `q` does: exit **0**, nothing after the alternate screen is
    handed back (NOTES § D279 ruling 5).
  · **one context, none at all, a shadowed pair, a row whose cluster the file does not define, and
    a name that strips to nothing** — the kubeconfig shapes the rule is only proven for if it was
    fed each one (CLAUDE.md § A check is proven only for the shapes it was fed).
  · **`--context`**, naming a row that is not in the file and naming a shadowed one.
  · **no terminal at all** — `--once` with two contexts must not ask, and a console with pipes for
    ends answers `USAGE` rather than hanging. Those two need no pty and are run without one.

**A crafted context name is fed through the real path and not a test's constructor** (invariant 9):
an ANSI escape and a right-to-left override written into the kubeconfig as YAML's own `\\u` escapes
— repr-quoting them would put the *letters* `\\`,`u` in the file and prove nothing — read out by
`k8s::contexts`, drawn by the picker, and carried into the failure box's title.

**What a byte stream off a pty can and cannot say.** It cannot say how wide a frame is: a pty is a
fixed grid the program cannot draw outside of, and ratatui writes cells with cursor-positioning
escapes between them, so the stripped transcript is one long line whatever was drawn. The 80×24
claim belongs to `TestBackend` in the suite. What this proves instead is that the bound **ran** —
a 10k name carries `k8s::text`'s own shortening mark — and that the frame around it is still whole.

**Whitespace is not a fact about the screen here, for that same reason**: ratatui repaints only the
cells that changed, so `Choose a cluster` reaches the pty as `Choose`, a cursor jump, `a`, a jump,
`cluster`. Every content check therefore reads the transcript with its whitespace squeezed out, and
its needle squeezed the same way. Only the control bytes — the alternate screen — are read raw.

**No run here can reach a cluster.** Every kubeconfig this script writes names
`https://127.0.0.1:<port>` on a port that was bound and released, or an address that is not one,
and `KUBECONFIG` is set on the child, so the developer's own cluster is not inherited — the same
property `tests/binary.rs` § `no_test_here_can_reach_a_cluster` keeps for the suite.

**Why this is not in `just check`.** It needs a built binary and a pty, and CI has neither; what
runs in the gate is `--self-test`, which feeds the checks a healthy transcript and then one broken
variant per check, and needs no terminal at all.

Usage:
    picker-test.py                 # target/debug/k8rs
    picker-test.py <binary>        # some other build
    picker-test.py --self-test     # prove every check fails when it should
"""
import contextlib
import fcntl
import importlib.util
import os
import pty
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import termios
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Loaded by path because the filename has a hyphen, and safe to import — everything that runs there
# sits under its `if __name__ == "__main__"`. The pty plumbing and the check machinery are that
# script's, not a second copy: one harness of this shape is enough, and a second `drain` would be
# the copy that goes stale (CLAUDE.md § Write function-based).
_spec = importlib.util.spec_from_file_location("suspend_test", ROOT / "scripts/suspend-test.py")
_suspend = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_suspend)
holds, broken, drain, visible = _suspend.holds, _suspend.broken, _suspend.drain, _suspend.visible
stopped_within = _suspend.stopped_within

ALT_ON, ALT_OFF = _suspend.ALT_ON, _suspend.ALT_OFF
# The pane label every *console* frame draws. Behind the startup picker there is no console frame at
# all, so its absence is a claim and not a coincidence (`screens/context.md` § Opening at startup:
# *genuinely nothing*).
FRAME = _suspend.FRAME
ROWS, COLS = 24, 80


# The frame's own glyphs — every box-drawing character, and the picker's cursor mark. They are
# furniture, not content, and they sit *between* the halves of every wrapped sentence.
FURNITURE = {chr(c) for c in range(0x2500, 0x2580)} | {"\u25b8"}


def squeezed(text: str) -> str:
    """The characters the terminal was asked to draw, in order, with the furniture taken out.

    **Spacing is the renderer's diff and not the screen** (see the module docstring), so it goes.

    **So do the borders, and that is not cosmetic**: a sentence wider than the box is wrapped, and
    what lands between its halves in the byte stream is `│  │` — so a needle longer than one drawn
    row could never match however right the screen was. Measured 2026-09-26: *gave k8rs nothing to
    sign in with* wraps after `sign in` and read as absent, while the short needles either side of
    it read as present."""
    return "".join(c for c in visible(text) if not c.isspace() and c not in FURNITURE)


def S(needle: str) -> str:
    """A needle squeezed the way the transcript is, so the rows stay readable with their spaces."""
    return "".join(c for c in needle if not c.isspace() and c not in FURNITURE)


# What the contexts are called. `ALPHA` is `current-context` everywhere and is the row a picker
# opens on; its cluster's `server:` is not an address, which is what makes `⏎` on it the failure
# box rather than a console (measured 2026-09-25: a bad `certificate-authority-data` does *not*
# refuse — the client builds and the watches fail, which is a header word and not a modal).
ALPHA, BETA, SOLO, TWIN, GHOST = "alpha-cluster", "beta-cluster", "solo-cluster", "twin", "ghost"
# The context whose user logs in with a program.
LOGIN = "needs-login"
# A name carrying two framings invariant 9 is about — a C0 escape and a right-to-left override —
# written as YAML's own escapes so the file holds the bytes and not the letters that spell them.
CRAFTED_YAML = '"wr\\u001B[31mecked\\u202Eing"'
# What is left once `k8s::text` has run: the escape byte and the override gone, the printable
# remains kept.
CRAFTED = "wr[31meckeding"
# A name far longer than any row, to prove the ingest bound ran rather than the renderer's clip.
HUGE = "z" * 10000
# `k8s::SHORTENED`, the mark that bound leaves. Named here because it is what the row must carry;
# the number it cuts at is `k8s.rs`'s and this script keeps no copy of it.
SHORTENED = "… (shortened by k8rs)"
# A context name made only of characters invariant 9 removes, which `screens/context.md` draws as
# `views::UNNAMED`.
UNNAMED_YAML = '"\\u200B"'
UNNAMED = "(unnamed)"

# --- the checks -----------------------------------------------------------
#
# One row is one claim, and the row is both the assertion and its own plant, exactly as
# `suspend-test.py` § the checks lays out: `--self-test` breaks the fact the row reads and asserts
# that *this* row goes red.
#
#   ("what is claimed", key, kind, argument)
CHECKS = [
    # --- the startup picker, on a file with two contexts and no `--context` ---
    ("the console took the alternate screen before the picker drew", "open.raw", "in", ALT_ON),
    ("the startup picker is the first thing drawn", "open", "in", S("Choose a cluster")),
    ("the header says a cluster is being chosen", "open", "in", S("choose a cluster · admin")),
    ("no context is named before one has been picked", "open", "not in", S("ctx:")),
    ("nothing claims to be connecting before a row is chosen", "open", "not in", S("connecting")),
    ("no app frame is drawn behind the startup picker", "open", "not in", S(FRAME)),
    ("the startup picker offers connect and not switch", "open", "in", S("⏎ connect")),
    ("the startup picker's esc quits rather than cancels", "open", "in", S("esc quit")),
    ("the startup picker never offers cancel, which would mean a cluster to go back to",
     "open", "not in", S("esc cancel")),
    ("both contexts in the file are listed", "open", "in", S(BETA)),
    ("the one read behind the picker is in the command log",
     "open", "in", S("$ kubectl config get-contexts")),
    ("the picker never teaches use-context, which would edit the kubeconfig",
     "open", "not in", S("use-context")),

    # --- `⏎` on a row whose client cannot be built ---
    ("a switch that cannot connect draws the failure box", "enter", "in", S("could not be opened")),
    ("the failure box names the context that was tried", "enter", "in", S(ALPHA)),
    ("the failure box's way out is back to the list, not a cluster nobody is on",
     "enter", "in", S("esc back to the list")),
    ("the failure box never offers dismiss, which would name a cluster nobody is on",
     "enter", "not in", S("esc dismiss")),
    ("a connection that never opened is not called live", "enter", "not in", S("· live")),
    # **Step 3's frame is drawn and then replaced, and the order is the whole claim**
    # (`screens/context.md` § What happens on `⏎`, whose first call is the startup `⏎`): the
    # reader sees `connecting…` while the connect is out, and the box once it answers — never a
    # box that then says it is connecting.
    ("the connecting frame comes first and the box replaces it",
     "enter.stream", "before", (S("connecting…"), S("could not be opened"))),
    ("nothing was written to stderr behind the alternate screen",
     "enter", "not in", S("k8rs: no cluster to watch")),

    # --- `esc` on the failure box ---
    ("esc on the failure box reopens the picker", "back", "in", S("Choose a cluster")),
    ("the picker esc reopens is the startup one", "back", "in", S("esc quit")),
    ("the reopened picker still offers connect", "back", "in", S("⏎ connect")),

    # --- `esc` on the startup picker ---
    ("esc on the startup picker ended the run", "quit.exited", "on", None),
    ("the run ending gave the alternate screen back", "quit.raw", "in", ALT_OFF),
    ("esc on the startup picker exits 0, the way q does", "quit.code", "is", 0),
    ("a reader leaving wrote no sentence to stderr", "quit.said", "off", None),

    ("a console with nothing connected blocks rather than spinning",
     "idle.ticks", "under", 20),

    # --- the crafted context name, through `k8s::contexts` and into every surface ---
    ("the crafted row is drawn with its escape stripped", "crafted.open", "in", S(CRAFTED)),
    ("no escape byte from the kubeconfig reached the screen", "crafted.escaped", "off", None),
    ("no right-to-left override reached the screen", "crafted.open", "not in", "‮"),
    ("the crafted name is still drawn in the failure box's title",
     "crafted.enter", "in", S(CRAFTED)),
    ("no escape byte reached the failure box either", "crafted.enter.escaped", "off", None),
    ("the 10k name was bounded where it was read, not where it was drawn",
     "crafted.open", "in", S(SHORTENED)),
    ("the frame is still whole under a 10k name", "crafted.open", "in", S("esc quit")),
    ("a 10k name did not push the picker's own list off the box",
     "crafted.open", "in", S(ALPHA)),

    # --- one context, and none at all ---
    ("one context in the file opens no picker", "solo", "not in", S("Choose a cluster")),
    ("one context connects straight through to a console", "solo", "in", S(FRAME)),
    ("the one context is the one named in the header", "solo", "in", S(f"ctx: {SOLO}")),
    ("a kubeconfig with no contexts is the stderr wall", "empty.said", "in", S("k8rs: no cluster")),
    ("a kubeconfig with no contexts never took the alternate screen",
     "empty.raw", "not in", ALT_ON),
    ("a kubeconfig with no contexts exits 2", "empty.code", "is", 2),

    # --- a shadowed pair, and a row whose cluster the file does not define ---
    ("a shadowed pair is still two rows, so the picker opens", "twins", "in",
     S("Choose a cluster")),
    ("the shadowed row says why it can never be opened", "twins.down", "in",
     S("another context earlier in this file is also named")),
    # **`⏎` on a shadowed row is `Did::Nothing`, so nothing is repainted** — which makes that
    # step's own transcript empty and every "not in" over it vacuously true. The claim is read
    # over the whole run, with the picker as the canary that anything was drawn at all.
    ("⏎ on a shadowed row never drew a failure box", "twins.all", "not in",
     S("could not be opened")),
    ("⏎ on a shadowed row never opened a console either", "twins.all", "not in", S(FRAME)),
    # **The canary for the two rows above, on the fact they read** (CLAUDE.md § A derived list
    # asserts it found something): a run that collected nothing would pass both of them.
    ("and that run drew a picker at all", "twins.all", "in", S("Choose a cluster")),
    ("a row whose cluster the file does not define is still listed", "ghost.open", "in", S(GHOST)),
    ("the cursor cannot land on it, so ⏎ after ↓ still names the row it was on",
     "ghost.enter", "in", S(f"{ALPHA} could not be opened")),
    ("a context whose name strips to nothing is drawn as such", "unnamed.open", "in", S(UNNAMED)),
    ("and the failure box titles it the same way, so the two cannot disagree",
     "unnamed.enter", "in", S(f"{UNNAMED} could not be opened")),

    # --- `--context`: it beats the picker, and a name the file does not hold is the wall ---
    ("a --context that names nothing is the stderr wall", "named.said", "in", S("k8rs: no cluster")),
    ("the wall never opened a picker", "named.said", "not in", S("Choose a cluster")),
    ("the wall never took the alternate screen", "named.raw", "not in", ALT_ON),
    ("a --context that names nothing exits 2", "named.code", "is", 2),
    ("--context beats the picker even where two contexts would have opened one",
     "shadowed_flag", "not in", S("Choose a cluster")),
    ("--context naming a shadowed name connects to the entry that shadows it",
     "shadowed_flag", "in", S(f"ctx: {TWIN}")),

    # --- the whole switch, on a file whose rows connect: startup ⏎, then `X`, then ⏎ again ---
    #
    # **The only journey here where a connection succeeds**, which is what makes it the one that
    # can read the mid-session picker, the second connect, and `--read-only` outliving both
    # (todo.md § Phase 12: *`Screen::writes` survives `App::switched`*).
    ("--read-only is in the header before any cluster is chosen",
     "switch.open", "in", S("choose a cluster · read-only")),
    ("the startup ⏎ connects rather than only ever failing", "switch.first", "in", S(FRAME)),
    ("the header names the context the startup picker chose",
     "switch.first", "in", S(f"ctx: {ALPHA}")),
    ("a context that does verify TLS is not marked as one that does not",
     "switch.first", "not in", S("TLS not verified")),
    ("X after a connection opens the mid-session picker and not the startup one",
     "switch.x", "in", S("⏎ switch")),
    ("and its esc cancels back to the cluster that is live, rather than quitting",
     "switch.x", "in", S("esc cancel")),
    ("X says what it read, the same line the startup picker appended",
     "switch.x", "in", S("$ kubectl config get-contexts")),
    ("⏎ on another row connects to it", "switch.second", "in", S(f"ctx: {BETA}")),
    ("every line the new connection teaches carries its own --context",
     "switch.second", "in", S(f"--context {BETA}")),
    ("--read-only outlived the switch", "switch.second", "in", S("read-only")),
    ("the header of the new cluster does not still name the old one",
     "switch.second", "not in", S(f"ctx: {ALPHA}")),

    # --- a context whose kubeconfig turns TLS verification off ---
    ("a context that turns TLS verification off is marked in the picker",
     "insecure.open", "in", S("⚠ TLS not verified")),
    ("and the header says so once that is the context connected to",
     "insecure.enter", "in", S("TLS not verified")),

    # --- the other `views::Before`, with a crafted name inside it ---
    #
    # **The one framing the suite proves only by where the value came from**: `Before::Connected`
    # carries the context that was live, and the sentence it builds is the box's way out. Fed here
    # by connecting to a crafted-named context first and then failing off it.
    ("the way out names the cluster that is still fine, with its escape stripped",
     "carried.fail", "in", S(f"Nothing is wrong with {CRAFTED}")),
    ("no escape byte reached that sentence", "carried.escaped", "off", None),
    ("a box with a cluster behind it says dismiss, not back to the list",
     "carried.fail", "in", S("esc dismiss")),

    # --- what a failed connect leaves on the screen (NOTES § D280 item 1) ---
    #
    # **One word for every fault that can reach a failed connect**, joined onto the zone by
    # `main::not_connected` rather than answered by `ui::Link`, whose four words all claim
    # something a connection that never opened has not done.
    ("a failed startup connect says so in the header", "enter", "in", S("⚠ not connected")),
    ("and never the word that belonged to a fault that cannot get here",
     "enter", "not in", S("⚠ not allowed")),
    ("the strip a failed startup connect leaves is the read that really happened",
     "enter", "in", S("$ kubectl config get-contexts")),
    ("a failed mid-session switch says the same one word", "carried.fail", "in",
     S("⚠ not connected")),
    # **The property that keeps the old cluster's line from being read as the new one's**: it
    # names its own context. `main::kubectl` puts `--context <name>` on every taught cluster line
    # (NOTES § D278 ruling 5), so the strip under `ctx: <failed> · ⚠ not connected` can never be
    # silently about a cluster it does not name.
    # **Shell-quoted by `ops::pasteable` and not by a second rule** (NOTES § D278 ruling 5): this
    # name carries `[` and `]`, so the one quoting rule wraps it — and the needle says so rather
    # than matching a bare name the product never writes.
    ("the line a failed switch leaves on the strip names its own cluster",
     "carried.fail", "in", S(f"--context '{CRAFTED}'")),
    ("a connect that succeeded emptied the strip of the cluster it left",
     "switch.second", "not in", S(f"--context {ALPHA} get")),

    # --- the login program: the one reachable fault that earns a next step ---
    #
    # **`screens/context.md` § Which faults can actually reach this box** rules three of eleven
    # reachable through `switched`, and the suite's only failed-switch test drives `NoContext`,
    # which that same table rules **out**. These journeys feed two of the real three: `BadEntry`
    # (a `server:` line that is not an address, above) and `NoCredential` (here).
    #
    # **Two runs, and each answers the one question it can answer deterministically.** A program
    # that does not exist fails the exec at once and is always `NoCredential`, so it is what every
    # sentence below is read off. `/bin/cat` exists and blocks, so which fault kube ends on is a
    # race — measured 2026-09-26, the next step was there on one run and gone on the next — and
    # the only thing read off it is whether the run ended at all.
    ("a login program that gives nothing back is the failure box",
     "crafted_login.fail", "in", S("could not be opened")),
    ("and the box says the login program is why", "crafted_login.fail", "in",
     S("gave k8rs nothing to sign in with")),
    ("a login program earns the one next step a connection that sent nothing gets",
     "crafted_login.fail", "in", S("Run it yourself first:")),
    ("that next step names the context it is about", "crafted_login.fail", "in",
     S(f"--context {LOGIN} version")),
    ("a failed login-program connect says so in the header too",
     "crafted_login.fail", "in", S("⚠ not connected")),
    # **The login program's own path is free text out of the kubeconfig reaching a dialog**
    # (invariant 9): `k8s::renewal` strips it where it reads it, and this is the framing that
    # says so through the real path rather than through a constructor.
    ("a crafted login-program path is drawn stripped", "crafted_login.fail", "in",
     S("/bin/nosuchthing")),
    ("no escape byte from a login program's path reached the screen",
     "crafted_login.escaped", "off", None),
    # **The direct reading of *it did not hang***: `connect_with` is awaited inside `switched`, so
    # a plugin holding the terminal blocks the whole loop and the `esc` that follows is never
    # read. `/bin/cat` reads stdin and would never return if it had the terminal; with
    # `interactive_mode: Never` kube pipes it instead, and a run that ends is a run whose plugin
    # gave the terminal back (NOTES § D279 ruling 6).
    ("the run ended rather than blocking in the login program", "login.exited", "on", None),
    # **The one quoting rule, on the taught command a reader is told to run** (NOTES § D278
    # ruling 5, § D281 ruling 2): `views::run_the_login` goes through `sanitize` then
    # `ops::pasteable`, the same order `main::kubectl` uses — so a name carrying a shell
    # metacharacter is spelled once, and the same way, wherever it is taught. Written and held
    # back last round because the fix was one line in a file this tree does not own.
    ("the taught login command quotes its context the way every other taught line does",
     "spaced_login.fail", "in", S("--context 'needs login' version")),

    # --- a login program on a context whose name strips to nothing: the decline ---
    #
    # **`run_the_login` answers `None` and `ui::failed` joins nothing** (NOTES § D281 ruling 2):
    # the alternative is a command naming `(unnamed)`, which names no context, or a bare
    # `kubectl version` that runs against whatever `current-context` happens to be — a third
    # cluster, neither the one on the header nor the one the reader chose.
    ("a context with no runnable name still draws the box", "declined.fail", "in",
     S("could not be opened")),
    ("its reason ends cleanly rather than running into a joined empty step",
     "declined.fail", "in", S("gave k8rs nothing to sign in with.")),
    ("no next step is offered at all", "declined.fail", "not in", S("Run it yourself first")),
    ("and no half of one is left dangling", "declined.fail", "not in", S("Then try again")),
    ("the placeholder is never taught as a context", "declined.fail", "not in",
     S(f"--context {UNNAMED}")),
    ("and no context-less kubectl is taught, which would run against a third cluster",
     "declined.fail", "not in", S("`kubectl version`")),

    # --- no terminal at all: the half of the box that is about a pipeline ---
    ("--once with two contexts never opens a picker", "once.out", "not in", S("Choose a cluster")),
    ("--once with two contexts never took the alternate screen", "once.raw", "not in", ALT_ON),
    ("--once resolved the ambiguity silently, on current-context", "once.out", "in", S(ALPHA)),
    ("--once with no terminal ended rather than waiting for a keypress", "once.ended", "on", None),
    ("a console with pipes for ends answers the usage line rather than hanging",
     "piped.out", "in", S("usage: k8rs")),
    ("a console with pipes for ends never opened a picker",
     "piped.out", "not in", S("Choose a cluster")),
    ("a console with pipes for ends never took the alternate screen either",
     "piped.raw", "not in", ALT_ON),
    ("a console with pipes for ends ended", "piped.ended", "on", None),
]


def held(kind: str, fact, arg) -> bool:
    """`suspend-test.py`'s check kinds, plus the one a tick count needs.

    **Wrapped and not added there**: that script is another box's and keeping one harness's kinds
    out of another's is cheaper than a shared edit neither owns."""
    if kind != "under":
        return holds(kind, fact, arg)
    # `-1` is [`burnt`]'s *the child was gone*, and it may not read as the smallest number
    # there is; a fact the run never collected is the empty string and is not a number at all.
    return isinstance(fact, int) and 0 <= fact < arg


def break_it(kind: str, fact, arg):
    """The same fact with this row's claim made false."""
    return arg * 10 if kind == "under" else broken(kind, fact, arg)


def verdicts(observed: dict) -> list[tuple[str, bool]]:
    """Every row read against what was observed.

    A fact the run never collected — the child died halfway — reads as the empty string and fails
    its own row by name, rather than raising where the verdicts should have been."""
    return [(what, held(kind, observed.get(key, ""), arg)) for what, key, kind, arg in CHECKS]


# --- the kubeconfigs ------------------------------------------------------


def dead_port() -> int:
    """A loopback port nothing is listening on — bound, read, released.

    A connection to it is refused at once, so no check here waits on a timeout, and no run can
    reach a cluster however the machine's own kubeconfig is set up."""
    with socket.socket() as held:
        held.bind(("127.0.0.1", 0))
        return held.getsockname()[1]


def written(directory: Path, name: str, current: str, clusters: str, contexts: str) -> Path:
    """One kubeconfig, under its own filename so no later one overwrites it."""
    path = directory / f"{name}.yaml"
    path.write_text(
        "apiVersion: v1\n"
        "kind: Config\n"
        f"current-context: {current}\n"
        f"clusters:\n{clusters}"
        f"contexts:\n{contexts}"
        "users:\n- name: nobody\n  user: {}\n",
        encoding="utf-8",
    )
    return path


# **`unreadable` is the row a reader can land on that cannot be connected with.** Its `server:` is
# not an address, so `Config::from_custom_kubeconfig` refuses before a packet is sent — and
# `views::landable` still accepts the row, because `k8s::Address::Unreadable` is not `Undefined`.
UNREADABLE = "- name: unreadable\n  cluster: {server: 'not a server address'}\n"


def reachable() -> str:
    """A cluster whose address is fine and whose port nothing answers on."""
    return f"- name: refused\n  cluster: {{server: 'https://127.0.0.1:{dead_port()}'}}\n"


def entry(name: str, cluster: str = "unreadable") -> str:
    return f"- {{name: {name}, context: {{cluster: {cluster}, user: nobody}}}}\n"


# --- the pty run ----------------------------------------------------------


def repainted(fd: int) -> str:
    """The whole screen, redrawn, rather than the cells that happened to change.

    **A diff-based renderer writes only the cells that differ from the last frame**, so a needle
    that spans a cell which did not change can never appear in the byte stream however right the
    screen is: switching `alpha-cluster` to `beta-cluster` writes `bea` and leaves the `t` and the
    `-cluster` already on screen, and a run looking for `ctx: beta-cluster` reads a red check over
    a correct frame. Measured 2026-09-25 on the switch journey, where `read-only` — unchanged
    across the switch and therefore never rewritten — did the same.

    **A resize is the one thing that forces the whole frame out again**: `ratatui::Terminal`
    clears and repaints when the viewport changes, so one column out and back yields a full
    80-column paint of whatever is on screen at that moment. Errors are suppressed rather than
    raised — a child that has already exited is a fact for the rows to read, not a traceback where
    the verdicts should have been."""
    frame = ""
    for columns, seconds in ((COLS - 1, 0.6), (COLS, 1.5)):
        with contextlib.suppress(OSError):
            fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, columns, 0, 0))
        frame = drain(fd, seconds)
    return frame


def escaped(text: str) -> bool:
    """Whether an escape byte survived into the *text* the terminal drew.

    `visible()` removes the CSI sequences the renderer itself writes, so what is left is content.
    An `ESC` still in it came out of the kubeconfig, which is invariant 9 failing."""
    return "\x1b" in visible(text)


def burnt(pid: int, seconds: float) -> int:
    """Clock ticks of CPU the child spends over a quiet stretch — invariant 7, measured.

    **With nothing connected the merge is empty**, and a `SelectAll` holding no streams answers
    `None` at once and for ever: inside a `select!` that is a busy loop and not a blocked one.
    Nothing in the suite can tell the two apart — both draw the same screen — so the fact is read
    off the kernel instead. A blocked console spends ~0 ticks here; a spinning one spends one per
    tick of wall clock."""
    def ticks() -> int | None:
        try:
            fields = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
        except (OSError, IndexError):
            return None
        return int(fields[11]) + int(fields[12])
    before = ticks()
    time.sleep(seconds)
    after = ticks()
    # A child that has gone away reads as a number no row can pass, rather than as zero — which
    # would be the one value that makes this check green for the wrong reason.
    return -1 if before is None or after is None else after - before


def run_binary(binary: Path, args: list[str], config: Path, keys: list[tuple[str, bytes, float]],
               idle_after: str | None = None):
    """Start the binary on a pty with this kubeconfig, press these keys, and write down each frame.

    `keys` is `(name, bytes, seconds to read after)`. The transcript for a step is everything the
    terminal received after that step's keystroke, so a check reads the frame its own key produced
    and not the whole run. Each step leaves two facts: `<name>.stream`, the bytes that arrived
    while the key was being answered, and `<name>`, the screen afterwards ([`repainted`]).
    Both are raw; squeezing is the caller's.

    `idle_after` names the step after which the child's CPU is sampled ([`burnt`]) — a step
    name that is not in `keys` leaves the fact unset, which reads as a red row and never a pass."""
    pid, fd = pty.fork()
    if pid == 0:
        # A pty is born 0×0, and a console with no room is not what is under test.
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        os.environ["TERM"] = "xterm-256color"
        os.environ["KUBECONFIG"] = str(config)
        os.execv(str(binary), [str(binary), *args])
        os._exit(127)

    seen = {}
    try:
        for name, press, seconds in keys:
            if press:
                with contextlib.suppress(OSError):
                    os.write(fd, press)
            # **Two facts per step and they answer different questions**: what arrived while the
            # key was being answered — the order a frame gave way to another, and the control
            # bytes — and what is on the screen afterwards ([`repainted`]).
            seen[f"{name}.stream"] = drain(fd, seconds)
            seen[name] = repainted(fd)
            if name == idle_after:
                seen["idle.ticks"] = burnt(pid, 2.0)
        # **One reap, and it keeps the status** — a bounded wait that throws the status away and a
        # second `waitpid` for the code is a `ChildProcessError` and an exit code silently read as
        # `0`, which is the value half these rows are checking for.
        status = stopped_within(pid, 5.0)
        seen["exited"] = status is not None and os.WIFEXITED(status)
        seen["code"] = os.WEXITSTATUS(status) if seen["exited"] else None
    finally:
        # Cleanup is a trap, and "I killed it" is a claim until you look (NOTES § D185).
        for sig in (signal.SIGCONT, signal.SIGKILL):
            with contextlib.suppress(ProcessLookupError):
                os.kill(pid, sig)
        for _ in range(50):
            try:
                if os.waitpid(pid, os.WNOHANG)[0] == pid:
                    break
            except ChildProcessError:
                break
            time.sleep(0.1)
        if os.path.exists(f"/proc/{pid}"):
            print(f"picker-test: pid {pid} is still alive after SIGKILL", file=sys.stderr)
        os.close(fd)
    return seen


def piped(binary: Path, args: list[str], config: Path, seconds: float) -> tuple[str, bool]:
    """The same binary with pipes for ends, which is the half of *which cluster* about a pipeline.

    Bounded, because *a picker in a pipeline is a script that hangs for ever* is exactly the defect
    this proves the absence of — so a run that does hang has to be a red row and not a harness that
    hangs with it (NOTES § D116)."""
    try:
        done = subprocess.run(
            [str(binary), *args],
            env={**os.environ, "KUBECONFIG": str(config), "TERM": "xterm-256color"},
            stdin=subprocess.DEVNULL, capture_output=True, timeout=seconds,
        )
    except subprocess.TimeoutExpired as waited:
        return ((waited.stdout or b"").decode("utf-8", "replace")
                + (waited.stderr or b"").decode("utf-8", "replace"), False)
    return (done.stdout + done.stderr).decode("utf-8", "replace"), True


def observe(binary: Path, directory: Path) -> dict:
    """Every journey, in the order the checks read them."""
    seen = {}
    ordinary = written(directory, "ordinary", ALPHA, UNREADABLE + reachable(),
                       entry(ALPHA) + entry(BETA, "refused"))

    # The journey: the picker draws, `⏎` fails, `esc` goes back to the list, `esc` quits.
    walked = run_binary(binary, [], ordinary, [
        ("open", b"", 3.0),
        ("enter", b"\r", 4.0),
        ("back", b"\x1b", 3.0),
        ("quit", b"\x1b", 3.0),
    ], idle_after="back")
    for step in ("open", "enter", "back", "quit"):
        # The control bytes arrive in the stream; the screen is what the repaint drew.
        seen[f"{step}.raw"] = walked[f"{step}.stream"]
        seen[step] = squeezed(walked[step])
    seen["enter.stream"] = squeezed(walked["enter.stream"])
    seen["idle.ticks"] = walked["idle.ticks"]
    seen["quit.exited"], seen["quit.code"] = walked["exited"], walked["code"]
    # What a reader leaving wrote: everything after the alternate screen was handed back. On a pty
    # stderr and stdout are the same fd, so a sentence printed on the way out lands here.
    handed = walked["quit.stream"]
    seen["quit.said"] = bool(visible(handed.split(ALT_OFF)[-1]).strip()) if ALT_OFF in handed \
        else bool(visible(handed).strip())

    # The crafted name and the 10k one, read out of the file by `k8s::contexts` and drawn.
    crafted_config = written(directory, "crafted", ALPHA, UNREADABLE,
                             entry(ALPHA) + entry(CRAFTED_YAML) + entry(f"'{HUGE}'"))
    crafted = run_binary(binary, [], crafted_config, [
        ("open", b"", 3.0),
        # `↓ ⏎` lands on the crafted row rather than on `current-context`.
        ("enter", b"\x1b[B\r", 4.0),
        ("quit", b"\x1b", 1.0),
        ("gone", b"\x1b", 2.0),
    ])
    seen["crafted.open"] = squeezed(crafted["open"])
    seen["crafted.escaped"] = escaped(crafted["open"])
    seen["crafted.enter"] = squeezed(crafted["enter"])
    seen["crafted.enter.escaped"] = escaped(crafted["enter"])

    # One context: no picker at all, a console straight away (`which_cluster`'s last row).
    solo = written(directory, "solo", SOLO, reachable(), entry(SOLO, "refused"))
    seen["solo"] = squeezed(run_binary(binary, [], solo, [("s", b"", 4.0), ("q", b"q", 1.0)])["s"])

    # No contexts at all: there is nothing to ask, so it is the wall and never a picker.
    empty = written(directory, "empty", "nothing-is-here", reachable(), "")
    nothing = run_binary(binary, [], empty, [("said", b"", 3.0)])
    seen["empty.raw"] = nothing["said.stream"]
    seen["empty.said"] = squeezed(nothing["said.stream"])
    seen["empty.code"] = nothing["code"]

    # A name used twice: two rows, the second unreachable however it is picked (NOTES § D174).
    twins = written(directory, "twins", TWIN, reachable(), entry(TWIN, "refused") * 2)
    paired = run_binary(binary, [], twins, [
        ("open", b"", 3.0),
        ("down", b"\x1b[B", 2.0),
        ("enter", b"\r", 3.0),
        ("quit", b"\x1b", 2.0),
    ])
    for step in ("open", "down", "enter"):
        seen[f"twins{'' if step == 'open' else '.' + step}"] = squeezed(paired[step])
    seen["twins.all"] = "".join(seen[key] for key in ("twins", "twins.down", "twins.enter"))

    # A row whose cluster the file does not define: listed, and the cursor may not land on it.
    ghost = written(directory, "ghost", ALPHA, UNREADABLE,
                    entry(ALPHA) + entry(GHOST, "nothing-defines-this"))
    haunted = run_binary(binary, [], ghost, [
        ("open", b"", 3.0),
        ("enter", b"\x1b[B\r", 4.0),
        ("quit", b"\x1b\x1b", 2.0),
    ])
    seen["ghost.open"], seen["ghost.enter"] = squeezed(haunted["open"]), squeezed(haunted["enter"])

    # A name made only of characters invariant 9 removes.
    blank = written(directory, "unnamed", ALPHA, UNREADABLE, entry(ALPHA) + entry(UNNAMED_YAML))
    nameless = run_binary(binary, [], blank, [
        ("open", b"", 3.0),
        ("enter", b"\x1b[B\r", 4.0),
        ("quit", b"\x1b\x1b", 2.0),
    ])
    seen["unnamed.open"] = squeezed(nameless["open"])
    seen["unnamed.enter"] = squeezed(nameless["enter"])

    # `--context` naming nothing in the file: the wall before any picker, exit 2.
    named = run_binary(binary, ["--context", "nothing-in-the-file-names-this"], ordinary, [
        ("said", b"", 3.0),
    ])
    seen["named.raw"] = named["said.stream"]
    seen["named.said"] = squeezed(named["said.stream"])
    seen["named.code"] = named["code"]

    # `--context` naming a shadowed name: it beats the picker, and finds the entry that shadows.
    flagged = run_binary(binary, ["--context", TWIN], twins, [("s", b"", 4.0), ("q", b"q", 1.0)])
    seen["shadowed_flag"] = squeezed(flagged["s"])

    # The whole switch, on a file whose rows connect: `--read-only`, startup `⏎`, `X`, `⏎` again.
    both = written(directory, "both", ALPHA, reachable(),
                   entry(ALPHA, "refused") + entry(BETA, "refused"))
    switching = run_binary(binary, ["--read-only"], both, [
        ("open", b"", 3.0),
        ("first", b"\r", 6.0),
        ("x", b"X", 3.0),
        ("second", b"\x1b[B\r", 6.0),
        ("q", b"q", 1.0),
    ])
    for step in ("open", "first", "x", "second"):
        seen[f"switch.{step}"] = squeezed(switching[step])

    # A context whose kubeconfig turns TLS verification off — marked on its row, and in the header
    # of the run that picks it (`screens/context.md` § The picker, the security gate's own row).
    unverified = written(
        directory, "insecure", ALPHA,
        reachable() + f"- name: unverified\n  cluster:\n    server: 'https://127.0.0.1:"
                      f"{dead_port()}'\n    insecure-skip-tls-verify: true\n",
        entry(ALPHA, "refused") + entry(BETA, "unverified"))
    lax = run_binary(binary, [], unverified, [
        ("open", b"", 3.0),
        ("enter", b"\x1b[B\r", 6.0),
        ("q", b"q", 1.0),
    ])
    seen["insecure.open"], seen["insecure.enter"] = squeezed(lax["open"]), squeezed(lax["enter"])

    # `views::Before::Connected`, carrying a crafted name: connect to it, then fail off it.
    carried = written(directory, "carried", CRAFTED_YAML, UNREADABLE + reachable(),
                      entry(CRAFTED_YAML, "refused") + entry(ALPHA))
    held = run_binary(binary, [], carried, [
        ("open", b"", 3.0),
        ("first", b"\r", 6.0),
        ("x", b"X", 3.0),
        ("fail", b"\x1b[B\r", 6.0),
        ("q", b"\x1bq", 1.0),
    ])
    seen["carried.fail"] = squeezed(held["fail"])
    seen["carried.escaped"] = escaped(held["fail"])

    # **`NoCredential`, through the real path** — the one reachable fault that earns a next step,
    # and the one `screens/context.md`'s own reachability table names that nothing else here feeds.
    #
    # **Two runs, one shape**: the program that returns nothing, and the same fault with the
    # program's own path crafted. The second says nothing new about the fault and everything about
    # what reaches the sentence (invariant 9).
    def logging_in(name: str, command: str, context: str = None):
        context = LOGIN if context is None else context
        path = written(directory, name, ALPHA, reachable(), entry(ALPHA, "refused")
                       + f"- {{name: {context}, context: {{cluster: refused, user: asks}}}}\n")
        path.write_text(path.read_text().replace(
            "users:\n- name: nobody\n  user: {}\n",
            "users:\n- name: nobody\n  user: {}\n"
            "- name: asks\n  user:\n    exec:\n"
            "      apiVersion: client.authentication.k8s.io/v1beta1\n"
            f"      command: {command}\n      interactiveMode: Always\n"), encoding="utf-8")
        # **The two `esc` presses are two steps and not one keystroke**: sent together, crossterm
        # reads `\x1b\x1b` as one sequence and the run does not end — which would fail the
        # *did it hang* row below for the opposite of its own reason.
        # **Twenty seconds and not the three the other journeys use**: a connect through an `exec`
        # plugin spawns the program, waits for it and only then gives up, and measured on the test
        # host that is past eight — at which the box has not drawn and all three rows below go red
        # for the harness's reason rather than the product's (seen exactly that, 2026-09-26).
        return run_binary(binary, [], path, [
            ("open", b"", 3.0),
            ("fail", b"\x1b[B\r", 20.0),
            ("back", b"\x1b", 1.5),
            ("q", b"\x1b", 2.0),
        ])

    # `/bin/cat` reads stdin and would never return if it had the terminal; with
    # `interactive_mode: Never` kube pipes it, so it reads EOF and the connect fails, bounded.
    logged = logging_in("login", "/bin/cat")
    seen["login.exited"] = logged["exited"]

    crafted_logged = logging_in("crafted-login", '"/bin/no\\u0007such\\u202Ething"')
    seen["crafted_login.fail"] = squeezed(crafted_logged["fail"])
    seen["crafted_login.escaped"] = escaped(crafted_logged["fail"])

    # A name carrying a shell metacharacter, taught by the login next step.
    spaced = logging_in("spaced-login", "/bin/nosuchthing", "'needs login'")
    seen["spaced_login.fail"] = squeezed(spaced["fail"])

    # A name made only of characters invariant 9 removes — nothing runnable to teach.
    declined = logging_in("declined", "/bin/nosuchthing", UNNAMED_YAML)
    seen["declined.fail"] = squeezed(declined["fail"])

    # No terminal: `--once` never asks, and a console with pipes answers the usage line.
    #
    # **Its own file, whose addresses are readable**: on `ordinary` the current context's
    # `server:` is not an address at all, so `--once` prints a sentence that names no context and
    # the row below would pass or fail for the wrong reason.
    reachable_pair = written(directory, "pipeline", ALPHA, reachable(),
                             entry(ALPHA, "refused") + entry(BETA, "refused"))
    seen["once.raw"], seen["once.ended"] = piped(binary, ["--once"], reachable_pair, 60.0)
    seen["piped.raw"], seen["piped.ended"] = piped(binary, [], reachable_pair, 20.0)
    seen["once.out"], seen["piped.out"] = squeezed(seen["once.raw"]), squeezed(seen["piped.raw"])
    return seen


def run(binary: Path) -> int:
    if not binary.exists():
        print(f"picker-test: {binary} is not built — `cargo build` first, or `just picker`, "
              f"which does it for you", file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory(prefix="k8rs-picker-") as directory:
        observed = observe(binary, Path(directory))
    read = verdicts(observed)
    failed = [what for what, ok in read if not ok]
    for what, ok in read:
        print(("  ok   " if ok else "  FAIL ") + what)
    for label, key in [("the startup picker", "open"),
                       ("the box a failed ⏎ drew", "enter"),
                       ("the picker esc reopened", "back"),
                       ("the crafted and 10k rows", "crafted.open"),
                       ("the shadowed row selected", "twins.down"),
                       ("--once with no terminal", "once.out")]:
        print(f"--- {label} (escapes and spacing stripped) ---")
        print(str(observed.get(key, ""))[-900:])
    print(f"--- CPU with nothing connected: {observed.get('idle.ticks')} tick(s) over 2 s "
          f"(a spin is ~200) ---")
    print(f"picker-test: {len(CHECKS)} check(s), {len(failed)} failure(s) — "
          f"{'OK' if not failed else 'FAILED: ' + '; '.join(failed)}")
    return 1 if failed else 0


# --- the self-test --------------------------------------------------------


def healthy() -> dict:
    """A transcript of a journey where every door worked, built out of the rows' own constants."""
    def picker(second: str, tail: str = "") -> str:
        return squeezed(
            f"{ALT_ON} k8rs  choose a cluster · admin\n"
            f"  Choose a cluster \n  ▸ {ALPHA}\n    {second}\n{tail}"
            f"  $ kubectl config get-contexts\n"
            f"  ↑↓ move  type to filter  ⏎ connect  esc quit\n")

    def box(to: str) -> str:
        return squeezed(
            f" ctx: {to} · admin\n  {to} could not be opened \n"
            "  This kubeconfig loaded, and something it points at did not.\n"
            "  Nothing has connected yet — esc takes you back to the list.\n"
            "  [ esc back to the list ]\n")

    def console(context: str) -> str:
        return squeezed(f" nodes 1/1  ctx: {context} · live\n ▸{FRAME} │ RESOURCES \n"
                        " ? all keys  q quit\n")

    wall = "k8rs: no cluster to watch — this kubeconfig names no context called that\n"
    once = f"k8rs: watching — {ALPHA} · every namespace\nno findings\n"
    usage = "usage: k8rs [--read-only] [--context <name>] [--namespace <name>]\n"
    return {
        "open.raw": ALT_ON, "open": picker(BETA),
        "enter": box(ALPHA) + squeezed(
            f" ctx: {ALPHA} · ⚠ not connected · admin\n"
            "  $ kubectl config get-contexts\n"),
        "enter.stream": squeezed(f" ctx: {ALPHA} · connecting… · admin\n") + box(ALPHA),
        "back": picker(BETA),
        "quit.raw": ALT_OFF, "quit.exited": True, "quit.code": 0, "quit.said": False,
        "crafted.open": picker(CRAFTED, tail=f"    …zzz{SHORTENED}\n"),
        "crafted.escaped": False,
        "crafted.enter": box(CRAFTED), "crafted.enter.escaped": False,
        "solo": console(SOLO),
        "empty.raw": wall, "empty.said": squeezed(wall), "empty.code": 2,
        "twins": picker(TWIN),
        "twins.down": picker(TWIN, tail="    another context earlier in this file is also named "
                                        f"{TWIN}.\n"),
        "twins.all": picker(TWIN) + picker(TWIN),
        "ghost.open": picker(GHOST), "ghost.enter": box(ALPHA),
        "unnamed.open": picker(UNNAMED), "unnamed.enter": box(UNNAMED),
        "named.raw": wall, "named.said": squeezed(wall), "named.code": 2,
        "shadowed_flag": console(TWIN),
        "switch.open": squeezed(f" k8rs  choose a cluster · read-only\n  Choose a cluster \n"),
        "switch.first": console(ALPHA) + squeezed(" · read-only\n"),
        "switch.x": squeezed(" Switch cluster \n  $ kubectl config get-contexts\n"
                             "  ↑↓ move  ⏎ switch  esc cancel\n"),
        "switch.second": console(BETA) + squeezed(
            f" · read-only\n $ kubectl --context {BETA} get pods -A --watch\n"),
        "insecure.open": picker(BETA, tail="    ⚠ TLS not verified\n"),
        "insecure.enter": console(BETA) + squeezed(" · ⚠ TLS not verified\n"),
        "carried.fail": squeezed(
            f"  {CRAFTED} could not be opened \n"
            f"  Nothing is wrong with {CRAFTED} — X takes you back.\n"
            "  [ esc dismiss ]\n"
            f" ctx: {ALPHA} · ⚠ not connected · admin\n"
            f" $ kubectl --context '{CRAFTED}' get daemonsets -A --watch\n"),
        "carried.escaped": False,
        "spaced_login.fail": squeezed(
            "  needs login could not be opened \n"
            "  The program this kubeconfig logs in with (`/bin/nosuchthing`) gave\n"
            "  k8rs nothing to sign in with. Run it yourself first: `kubectl\n"
            "  --context 'needs login' version`. Then try again.\n"),
        "declined.fail": squeezed(
            f"  {UNNAMED} could not be opened \n"
            "  The program this kubeconfig logs in with (`/bin/nosuchthing`) gave\n"
            "  k8rs nothing to sign in with.\n"
            f" ctx: {UNNAMED} · ⚠ not connected · admin\n"),
        "login.exited": True,
        "crafted_login.fail": squeezed(
            f"  {LOGIN} could not be opened \n"
            "  The program this kubeconfig logs in with (`/bin/nosuchthing`) gave\n"
            "  k8rs nothing to sign in with. Run it yourself first:\n"
            f"  `kubectl --context {LOGIN} version`. Then try again.\n"
            f" ctx: {LOGIN} · ⚠ not connected · admin\n"),
        "crafted_login.escaped": False,
        "idle.ticks": 0,
        "once.raw": once, "once.out": squeezed(once), "once.ended": True,
        "piped.raw": usage, "piped.out": squeezed(usage), "piped.ended": True,
    }


def self_test() -> None:
    sample = healthy()
    missing = [key for _, key, _, _ in CHECKS if key not in sample]
    assert not missing, f"the healthy sample does not carry {missing}"
    unread = [key for key in sample if key not in {key for _, key, _, _ in CHECKS}]
    assert not unread, f"the healthy sample carries {unread}, which no check reads"
    green = verdicts(sample)
    assert all(ok for _, ok in green), \
        f"the healthy transcript is not green: {[w for w, ok in green if not ok]}"

    for what, key, kind, arg in CHECKS:
        planted = dict(sample)
        planted[key] = break_it(kind, sample[key], arg)
        went = [name for name, ok in verdicts(planted) if not ok]
        assert what in went, f"breaking {key!r} did not fail {what!r} — it fails nothing"
        print(f"picker-test --self-test: refused — {what}")
    print(f"picker-test --self-test: {len(CHECKS)} check(s), each red on its own plant and green "
          f"on the healthy transcript — OK")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    else:
        where = sys.argv[1] if len(sys.argv) > 1 else ROOT / "target/debug/k8rs"
        sys.exit(run(Path(where)))
