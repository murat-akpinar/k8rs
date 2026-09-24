#!/usr/bin/env python3
"""Drive the real binary on a real pty, stop it three ways, and read the terminal back.

**The part of the handover no suite can hold** (NOTES § D24, § D277 ruling 7). `stopped()` ends in
`raise(SIGSTOP)`, which would stop the test binary and hang the run until somebody went looking;
raw mode is a `tcsetattr` on a real fd; and the two `select!` arms `SIGTSTP` and `SIGCONT` reach
need a process with a controlling terminal and somebody else to signal it. Here the terminal is a
pty this script owns, so all three are observable — the slave's `termios`, `waitpid(WUNTRACED)`,
and the bytes the app wrote.

**Three stops, because the code has three doors and they are not the same door:**

  · `ctrl-z`   — a key: raw mode cleared `ISIG`, so it arrives as a keystroke and `pressed`
                 answers it.
  · `kill -TSTP` — a signal we hold a handler for, forwarded in as `Woke::Stopping`. **It must
                 come back stopped on `SIGSTOP`, not on `SIGTSTP`** — that one number is the
                 difference between our handler running (terminal handed back first) and the
                 kernel's default action (terminal left raw).
  · `kill -STOP` — the stop nothing can catch. Nothing hands the terminal back, so the resume is
                 the only place it can be repaired, and the shell has meanwhile put the tty back
                 in cooked mode. This script does what `bash` does — `tcsetattr` to cooked while
                 the process is stopped — because that is what makes the resume's
                 `handed_back`-before-`taken_back` order load bearing: crossterm's
                 `enable_raw_mode` returns `Ok(())` and touches no tty while it believes raw mode
                 is already on (`crossterm-0.29.0 src/terminal/sys/unix.rs:108`).

**Why this is not in `just check`.** It needs a built binary and a pty, and CI has neither; what
runs in the gate is `--self-test`, which feeds the checks below a healthy transcript and then one
broken variant per check, and needs no terminal at all.

Usage:
    suspend-test.py                 # target/debug/k8rs
    suspend-test.py <binary>        # some other build
    suspend-test.py --self-test     # prove every check fails when it should
"""
import contextlib
import fcntl
import os
import pty
import re
import select
import signal
import struct
import sys
import termios
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

ALT_ON, ALT_OFF = "\x1b[?1049h", "\x1b[?1049l"
HIDE, SHOW = "\x1b[?25l", "\x1b[?25h"
# The pane label every console frame draws, still-loading included (`screens/states.md`), so the
# check does not need a cluster to mean something.
FRAME = "ALERTS"
ROWS, COLS = 24, 100
# `waitpid` reports the signal the process actually stopped on. 19 on Linux; read off the module
# rather than written down, because this script also has to be true on a host where it is not
# (NOTES § D276 refused hand-written signal numbers for the same reason).
STOPPED_BY = int(signal.SIGSTOP)

# --- the checks -----------------------------------------------------------
#
# One row is one claim, and the row is both the assertion and its own plant: `--self-test` breaks
# exactly the fact the row reads and asserts that *this* row is what goes red. A row carries the
# sentence it fails with, so a red run names which of the checks failed and not merely that one
# did (CLAUDE.md § A derived list asserts it found something).
#
#   ("what is claimed", key, kind, argument)
#     in      — the fact contains this text        broken by deleting it
#     not in  — it does not                        broken by inserting it
#     on      — it is true                         broken by flipping it
#     off     — it is false                        broken by flipping it
#     is      — it equals this number              broken by another signal's number
#     before  — a comes before b in the text       broken by swapping them
CHECKS = [
    ("the console took the alternate screen", "start.text", "in", ALT_ON),
    ("the console drew a frame", "start.text", "in", FRAME),
    ("the terminal is in raw mode while the console runs", "start.raw", "on", None),

    ("ctrl-z stopped the process", "key.stopsig", "is", STOPPED_BY),
    ("ctrl-z left the alternate screen", "key.text", "in", ALT_OFF),
    ("ctrl-z showed the cursor", "key.text", "in", SHOW),
    ("ctrl-z did not also take the screen back", "key.text", "not in", ALT_ON),
    ("raw mode is off while ctrl-z holds the process", "key.raw", "off", None),

    ("the resume took the alternate screen back", "key.back", "in", ALT_ON),
    ("the resume hid the cursor again", "key.back", "in", HIDE),
    ("the resume gave the terminal back before it took it", "key.back", "before", (ALT_OFF, ALT_ON)),
    ("nothing was drawn before the alternate screen came back", "key.back", "before", (ALT_ON, FRAME)),
    ("the resume redrew a whole frame", "key.back", "in", FRAME),
    ("raw mode is back on after the resume", "key.back.raw", "on", None),

    # `is SIGSTOP` is also `is not SIGTSTP`, which is the whole point of the row: the kernel's
    # default action would have stopped it on SIGTSTP with the tty still raw.
    ("kill -TSTP stopped it through our handler (SIGSTOP), not the default action (SIGTSTP)",
     "tstp.stopsig", "is", STOPPED_BY),
    ("an external kill -TSTP left the alternate screen", "tstp.text", "in", ALT_OFF),
    ("an external kill -TSTP showed the cursor", "tstp.text", "in", SHOW),
    ("raw mode is off while an external stop holds the process", "tstp.raw", "off", None),
    ("the resume from an external stop took the screen back", "tstp.back", "in", ALT_ON),
    ("the resume from an external stop redrew a whole frame", "tstp.back", "in", FRAME),
    ("raw mode is back on after an external stop", "tstp.back.raw", "on", None),

    ("an uncatchable kill -STOP stopped the process", "stop.stopsig", "is", STOPPED_BY),
    ("the shell's cooked tty was in place before the resume", "stop.cooked", "on", None),
    ("the resume from an uncatchable stop took the screen back", "stop.back", "in", ALT_ON),
    ("the resume from an uncatchable stop redrew a whole frame", "stop.back", "in", FRAME),
    ("raw mode is really back on after an uncatchable stop", "stop.back.raw", "on", None),

    # `Restoring`'s `Drop` is the only thing that hands the terminal back when the run ends, and
    # nothing constructs it in a test: `q` here is what reaches it.
    ("`q` ended the run", "quit.exited", "on", None),
    ("the run ending gave the alternate screen back", "quit.text", "in", ALT_OFF),
    ("the run ending showed the cursor", "quit.text", "in", SHOW),
    ("raw mode is off once the run has ended", "quit.raw", "off", None),
]


def holds(kind: str, fact, arg) -> bool:
    """Whether one row's claim holds of the fact it reads."""
    if kind == "in":
        return arg in fact
    if kind == "not in":
        return arg not in fact
    if kind == "on":
        return fact is True
    if kind == "off":
        return fact is False
    if kind == "is":
        return fact == arg
    if kind == "before":
        first, second = arg
        return first in fact and second in fact and fact.find(first) < fact.find(second)
    raise AssertionError(f"no such check kind: {kind}")


def broken(kind: str, fact, arg):
    """The same fact with exactly this row's claim made false, and nothing else touched."""
    if kind == "in":
        return fact.replace(arg, "")
    if kind == "not in":
        return fact + arg
    if kind in ("on", "off"):
        return not fact
    if kind == "is":
        return arg + 1
    if kind == "before":
        first, second = arg
        swapped = fact.replace(first, "\0").replace(second, first)
        return swapped.replace("\0", second)
    raise AssertionError(f"no such check kind: {kind}")


def verdicts(observed: dict) -> list[tuple[str, bool]]:
    """Every row read against what was observed.

    A fact the run never got to collect — the child died halfway — reads as the empty string and
    fails its own row by name, rather than raising where the verdicts should have been. A row
    whose key is a typo would do the same silently, so `--self-test` asserts every key is one the
    run writes."""
    return [(what, holds(kind, observed.get(key, ""), arg)) for what, key, kind, arg in CHECKS]


# --- the pty run ----------------------------------------------------------


def drain(fd: int, seconds: float) -> str:
    out = b""
    end = time.time() + seconds
    while time.time() < end:
        if not select.select([fd], [], [], 0.2)[0]:
            continue
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            break
        if not chunk:
            break
        out += chunk
    return out.decode("utf-8", "replace")


def raw_mode(fd: int) -> bool | None:
    """Whether the tty is raw — canonical input and echo both off.

    `None` when the terminal cannot be read at all, so a pty that went away is a named red check
    rather than a traceback where the verdicts should have been."""
    try:
        lflag = termios.tcgetattr(fd)[3]
    except OSError:
        return None
    return not (lflag & termios.ICANON) and not (lflag & termios.ECHO)


def cooked(fd: int) -> None:
    """Put the tty back the way a shell does while it owns the terminal again."""
    mode = termios.tcgetattr(fd)
    mode[3] |= termios.ICANON | termios.ECHO | termios.ISIG
    termios.tcsetattr(fd, termios.TCSANOW, mode)


def stopped_within(pid: int, seconds: float) -> int | None:
    """The status of a process that stopped inside the deadline, or `None`.

    **Bounded, and that is the whole reason it is a function**: a blocking `waitpid` against a
    program that never stops hangs with no output at all, and a harness that hangs names nothing.
    A door that does not stop the process is a red check with a sentence on it."""
    end = time.time() + seconds
    while time.time() < end:
        gone, status = os.waitpid(pid, os.WNOHANG | os.WUNTRACED)
        if gone == pid:
            return status
        time.sleep(0.1)
    return None


def ended(pid: int, seconds: float) -> bool:
    """Whether the process is gone within the deadline — a bounded wait, because a `q` that does
    not quit must be a red check and not a harness that hangs."""
    end = time.time() + seconds
    while time.time() < end:
        try:
            gone, status = os.waitpid(pid, os.WNOHANG | os.WUNTRACED)
        except ChildProcessError:
            return True
        if gone == pid and os.WIFEXITED(status):
            return True
        time.sleep(0.1)
    return False


def stop_signal(status: int) -> int | None:
    """The signal a stop came back on, or `None` when the process did not stop at all."""
    return os.WSTOPSIG(status) if os.WIFSTOPPED(status) else None


def visible(text: str) -> str:
    return re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]", "", text)


def observe(binary: Path) -> dict:
    """Run the binary under a pty, stop it three ways, and write down what the terminal did."""
    pid, fd = pty.fork()
    if pid == 0:
        # The child is the session leader with the pty as its controlling terminal. A pty is born
        # 0x0, and a console with no room is not what is under test.
        fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        os.environ["TERM"] = "xterm-256color"
        os.execv(str(binary), [str(binary)])
        os._exit(127)

    seen = {}
    try:
        seen["start.text"] = drain(fd, 4.0)
        gone, status = os.waitpid(pid, os.WNOHANG | os.WUNTRACED)
        if gone == pid and not os.WIFSTOPPED(status):
            raise SystemExit(
                f"suspend-test: the console exited before anything was pressed (status {status}) — "
                f"what it drew:\n{visible(seen['start.text'])[:600]}"
            )
        seen["start.raw"] = raw_mode(fd)

        for name, stop in [
            ("key", lambda: os.write(fd, b"\x1a")),
            ("tstp", lambda: os.kill(pid, signal.SIGTSTP)),
            ("stop", lambda: os.kill(pid, signal.SIGSTOP)),
        ]:
            with contextlib.suppress(ProcessLookupError, OSError):
                stop()
            status = stopped_within(pid, 5.0)
            seen[f"{name}.stopsig"] = stop_signal(status) if status is not None else None
            seen[f"{name}.text"] = drain(fd, 1.0)
            seen[f"{name}.raw"] = raw_mode(fd)
            if name == "stop":
                # What `bash` does the moment it has the terminal back. Without it the resume's
                # `enable_raw_mode` would be repairing something that was never broken.
                cooked(fd)
                seen["stop.cooked"] = not raw_mode(fd)
            # A child that died instead of stopping has already failed its row above; the run
            # goes on collecting so that every row gets a verdict.
            with contextlib.suppress(ProcessLookupError):
                os.kill(pid, signal.SIGCONT)
            seen[f"{name}.back"] = drain(fd, 3.0)
            seen[f"{name}.back.raw"] = raw_mode(fd)

        with contextlib.suppress(OSError):
            os.write(fd, b"q")
        seen["quit.text"] = drain(fd, 2.0)
        seen["quit.raw"] = raw_mode(fd)
        seen["quit.exited"] = ended(pid, 5.0)
    finally:
        # Cleanup is a trap, and "I killed it" is a claim until you look (NOTES § D185).
        for sig in (signal.SIGCONT, signal.SIGKILL):
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass
        for _ in range(50):
            try:
                if os.waitpid(pid, os.WNOHANG)[0] == pid:
                    break
            except ChildProcessError:
                break
            time.sleep(0.1)
        if os.path.exists(f"/proc/{pid}"):
            print(f"suspend-test: pid {pid} is still alive after SIGKILL", file=sys.stderr)
        os.close(fd)
    return seen


def run(binary: Path) -> int:
    if not binary.exists():
        print(f"suspend-test: {binary} is not built — `cargo build` first, or `just suspend`, "
              f"which does it for you", file=sys.stderr)
        return 1
    observed = observe(binary)
    read = verdicts(observed)
    failed = [what for what, ok in read if not ok]
    for what, ok in read:
        print(("  ok   " if ok else "  FAIL ") + what)
    print("--- the frame the resume redrew (escapes stripped) ---")
    print(visible(observed["key.back"])[:400].rstrip())
    print(f"suspend-test: {len(CHECKS)} check(s), {len(failed)} failure(s) — "
          f"{'OK' if not failed else 'FAILED: ' + '; '.join(failed)}")
    return 1 if failed else 0


# --- the self-test --------------------------------------------------------


def healthy() -> dict:
    """A transcript of a run where every door worked, built out of the rows' own constants."""
    handed = f"{ALT_OFF}{SHOW}"
    back = f"{ALT_OFF}{SHOW}{ALT_ON}{HIDE}k8rs {FRAME} 2 findings"
    seen = {"start.text": f"{ALT_ON}{HIDE}k8rs {FRAME} 2 findings", "start.raw": True}
    for name in ("key", "tstp", "stop"):
        seen[f"{name}.stopsig"] = STOPPED_BY
        seen[f"{name}.text"] = handed
        seen[f"{name}.raw"] = False
        seen[f"{name}.back"] = back
        seen[f"{name}.back.raw"] = True
    # An uncatchable stop hands nothing back, so this one is still raw — no row reads it, and a
    # sample that said otherwise would be a lie about the object for the next reader.
    seen["stop.raw"] = True
    seen["stop.cooked"] = True
    seen["quit.text"] = handed
    seen["quit.raw"] = False
    seen["quit.exited"] = True
    return seen


def self_test() -> None:
    sample = healthy()
    missing = [key for _, key, _, _ in CHECKS if key not in sample]
    assert not missing, f"the healthy sample does not carry {missing}"
    green = verdicts(sample)
    assert all(ok for _, ok in green), \
        f"the healthy transcript is not green: {[w for w, ok in green if not ok]}"

    for what, key, kind, arg in CHECKS:
        planted = dict(sample)
        planted[key] = broken(kind, sample[key], arg)
        went = [name for name, ok in verdicts(planted) if not ok]
        assert what in went, f"breaking {key!r} did not fail {what!r} — it fails nothing"
        print(f"suspend-test --self-test: refused — {what}")
    print(f"suspend-test --self-test: {len(CHECKS)} check(s), each red on its own plant and green "
          f"on the healthy transcript — OK")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    else:
        where = sys.argv[1] if len(sys.argv) > 1 else ROOT / "target/debug/k8rs"
        sys.exit(run(Path(where)))
