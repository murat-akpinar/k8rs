#!/usr/bin/env python3
"""Fail if the terminal handover in `src/main.rs` asks the terminal a question.

`ctrl-z` hands the terminal back, stops the process and takes it back on `fg`
(NOTES § D24). The repaint that follows must not *read* from the terminal:
stdin belongs to the key thread, which swallows any reply, and the call then
fails after crossterm's timeout instead of returning. That is not a theory —
`Terminal::clear`, the obvious call, opens with `get_cursor_position`
(ratatui-core 0.1.2, `src/terminal/buffers.rs:148`), which writes `CSI 6n` and
waits for the answer on stdin. It ended the run on the first `fg` with *"the
screen could not be drawn — The cursor position could not be read within a
normal duration"* (test host, 2026-09-24, a real `ctrl-z` in `bash -i` on a
pty). `Terminal::resize` reaches the same `clear_viewport` through an `ioctl`
and touches stdin nowhere.

**Why this is a guard and not a test.** `TestBackend` answers that read out of
a field, so the suite as written — the repaint's own test included — passes with
`clear` in place, and the run that does catch it needs a pty and a shell with
job control.

**The ceiling, and it is a spelling.** This reads source text: a stdin read
reached through a name not listed below walks past it. The behavioural version
belongs beside [`repainted`] — a `Backend` whose `get_cursor_position` panics —
and is `src/main_tests.rs`'s to write, not this file's (CLAUDE.md § Ownership).

Comments are blanked first, by `security-guard.py`'s own stripper rather than a
second copy of one (CLAUDE.md § Write function-based): the doc comment on
[`repainted`] *names* `get_cursor_position` to say why it is not called, and a
scanner that read prose would refuse the explanation along with the call.

Usage:
    handover-guard.py              # src/main.rs
    handover-guard.py <main.rs>    # some other file, for a red/green demonstration
    handover-guard.py --self-test  # prove the guard fails when it should
"""
import contextlib
import importlib.util
import io
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Loaded by path because the filename has a hyphen, and safe to import —
# everything that runs there sits under its `if __name__ == "__main__"`. It
# blanks comments to spaces rather than deleting them, so an offset in the
# stripped text is still a line number in the file.
_spec = importlib.util.spec_from_file_location(
    "security_guard", ROOT / "scripts/security-guard.py")
_security_guard = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_security_guard)
strip_comments = _security_guard.strip_comments

START = "// --- THE TERMINAL HANDOVER START ---"
END = "// --- THE TERMINAL HANDOVER END ---"

# Every call in the region's reach that writes a query and reads the reply off
# stdin. `clear` is the one that shipped; the rest are the same mistake wearing
# another name, and `event::read`/`poll` would take the key thread's own input.
# `.clear()` is matched on any receiver rather than on a `Terminal`-shaped one: in
# a region this size any clear is worth a look, and the message names the route
# that does not read.
READS_STDIN = [
    (re.compile(r"\.clear\(\)"),
     "`Terminal::clear` opens with `get_cursor_position` — a `CSI 6n` whose reply "
     "the key thread swallows. `Terminal::resize` clears through the same "
     "`clear_viewport` and reads the size with an `ioctl`"),
    (re.compile(r"\bget_cursor_position\b|\bget_cursor\b"),
     "reads the cursor position off stdin, which the key thread owns"),
    (re.compile(r"\bcursor::position\b"),
     "crossterm's cursor query — a `CSI 6n` read straight off stdin"),
    (re.compile(r"\bevent::(?:read|poll)\b"),
     "takes the key thread's input; the handover has no reader of its own"),
    (re.compile(r"\binsert_before\b"),
     "reads the cursor position off stdin (ratatui-core `terminal/inline.rs`)"),
]

# The canary: "found no banned call" and "read no region at all" print the same
# line (CLAUDE.md § A derived list asserts it found something). A region that
# lost the repaint passes every absence above, so what the repaint is made of is
# asserted to still be there.
REPAINT = [
    (re.compile(r"\bfn repainted\b"), "`fn repainted`"),
    (re.compile(r"\.resize\("), "the `Terminal::resize` that does the repaint"),
]


def region(text: str, stripped: str) -> tuple[str, list[str]]:
    """The handover region's *code*, and what is wrong if it could not be read.

    The markers are comments, so they are found in `text` and the body is taken
    from `stripped` — the same length, blanked in place."""
    start, end = text.find(START), text.find(END)
    if start < 0 or end < 0:
        missing = START if start < 0 else END
        return "", [f"the handover region has no {missing!r} marker — either the "
                    f"region was renamed and this guard was about to check "
                    f"nothing, or it is gone and the check has to move with it"]
    if end < start:
        return "", ["the handover region's END marker comes before its START"]
    body = stripped[start + len(START):end]
    if not body.strip():
        return "", ["the handover region has no code in it — this guard was about "
                    "to pass by reading nothing"]
    return body, []


def at(text: str, offset: int) -> int:
    """The 1-based line number of an offset, so a failure names a line."""
    return text.count("\n", 0, offset) + 1


def check(path: Path) -> list[str]:
    text = path.read_text()
    body, problems = region(text, strip_comments(text))
    if problems:
        return [f"{path.name}: {p}" for p in problems]
    offset = text.find(START) + len(START)  # `body` starts here, so a match's line does too
    for pattern, why in REPAINT:
        if not pattern.search(body):
            problems.append(
                f"{path.name}: the handover region no longer contains {why}, so "
                f"every absence this guard checks would pass on a region that "
                f"does not repaint at all")
    for pattern, why in READS_STDIN:
        for m in pattern.finditer(body):
            problems.append(
                f"{path.name}:{at(text, offset + m.start())}  {m.group(0)} — "
                f"{why} (NOTES § D24)")
    return problems


def run(path: Path) -> list[str]:
    problems = check(path)
    for p in problems:
        print(f"FAIL {p}")
    print(f"handover-guard: {path.name} — "
          f"{len(READS_STDIN)} banned call(s) looked for, "
          f"{len(REPAINT)} canar(ies) asserted — "
          f"{'OK' if not problems else f'{len(problems)} problem(s)'}")
    return problems


def self_test() -> None:
    """Every check red on a planted copy of the real file, and the real file green."""
    real = (ROOT / "src/main.rs").read_text()
    _, why = region(real, strip_comments(real))
    assert not why, f"the self-test's own baseline is unreadable: {why}"

    def verdict(text: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            planted = Path(tmp) / "main.rs"
            planted.write_text(text)
            with contextlib.redirect_stdout(io.StringIO()):
                return check(planted)

    assert not verdict(real), f"the real file is not green: {verdict(real)}"

    planted = {}
    # One plant per banned call, each a line the region could plausibly grow.
    for line, name in [
        ("    let _ = terminal.clear();\n", "Terminal::clear"),
        ("    let _ = terminal.get_cursor_position();\n", "get_cursor_position"),
        ("    let _ = ratatui::crossterm::cursor::position();\n", "cursor::position"),
        ("    let _ = ratatui::crossterm::event::read();\n", "event::read"),
        ("    let _ = terminal.insert_before(1, |_| {});\n", "insert_before"),
    ]:
        planted[name] = verdict(real.replace(START, START + "\n" + line, 1))
        assert planted[name], f"a planted {name} was not refused"
        assert name.split("::")[-1].rstrip("()") in planted[name][0], planted[name]

    # The three ways the region stops being readable, each of which would
    # otherwise turn every absence above into a pass.
    planted["no END marker"] = verdict(real.replace(END, "// --- MOVED ---", 1))
    assert any("marker" in p for p in planted["no END marker"]), planted["no END marker"]

    emptied = real[:real.find(START) + len(START)] + "\n" + real[real.find(END):]
    planted["an empty region"] = verdict(emptied)
    assert any("no code in it" in p for p in planted["an empty region"]), \
        planted["an empty region"]

    without = real.replace("fn repainted", "fn repainted_elsewhere", 1)
    without = without.replace("terminal.resize(", "terminal.reflow(")
    planted["a region that does not repaint"] = verdict(without)
    assert len(planted["a region that does not repaint"]) == 2, \
        planted["a region that does not repaint"]

    for name, problems in planted.items():
        print(f"handover-guard --self-test: {name} — refused: {problems[0]}")
    print(f"handover-guard --self-test: {len(planted)} plant(s) refused, "
          f"src/main.rs green — OK")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    else:
        where = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "src/main.rs"
        sys.exit(1 if run(where) else 0)
