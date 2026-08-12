#!/usr/bin/env python3
"""Fail the build when a screen mockup does not fit the terminal it claims.

Every fenced block in `screens/` is a drawing of an 80x24 terminal — the
minimum k8rs supports ([screens/README.md], [screens/widgets.md] §8). A mockup
that overflows it is a layout the code cannot draw, and nothing notices until
Phase 11 transcribes it.

Bytes are not columns, which is what makes this worth a script: `─` is three
bytes and one column, a wide CJK character is one code point and two columns,
and `wc -c` gets both wrong. Width here is display columns.

The frames are also rectangles. Inside a block that draws borders every line is
the same width, so a line one column short is a broken border — and it is well
under 80, which is exactly why the size check alone cannot see it.

Usage:
    screens-check.py             # check this repository
    screens-check.py --self-test # prove the guard fails when it should
"""

import re
import sys
import unicodedata
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

MAX_COLS = 80
MAX_ROWS = 24
FENCE = re.compile(r"^\s*```")
BORDER = "│┌┐└┘├┤┬┴"


def width(text: str) -> int:
    """Display columns. Combining marks are free, East Asian W/F cost two."""
    return sum(
        0
        if unicodedata.combining(ch)
        else 2
        if unicodedata.east_asian_width(ch) in ("W", "F")
        else 1
        for ch in text
    )


def blocks(path: Path) -> list[tuple[int, list[str]]]:
    """Every fenced block as (line number of its first content line, lines)."""
    found: list[tuple[int, list[str]]] = []
    inside, start, buf = False, 0, []
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if FENCE.match(line):
            if inside:
                found.append((start, buf))
            inside, start, buf = not inside, n + 1, []
        elif inside:
            buf.append(line)
    return found


def check(root: Path) -> tuple[int, list[str]]:
    errors, counted = [], 0
    for path in sorted((root / "screens").glob("*.md")):
        rel = path.relative_to(root)
        for start, lines in blocks(path):
            counted += 1
            for i, line in enumerate(lines):
                cols = width(line)
                if cols > MAX_COLS:
                    errors.append(f"{rel}:{start + i} is {cols} columns wide (max {MAX_COLS})")
            if len(lines) > MAX_ROWS:
                errors.append(
                    f"{rel}:{start} is {len(lines)} rows tall (max {MAX_ROWS})"
                )
            # A drawn frame is a rectangle: every bordered line is as wide as
            # the frame. The unbordered header row above it is not, so only
            # lines that carry a border character are compared.
            framed = [
                (start + i, width(line))
                for i, line in enumerate(lines)
                if any(ch in BORDER for ch in line)
            ]
            if framed:
                frame = Counter(cols for _, cols in framed).most_common(1)[0][0]
                errors += [
                    f"{rel}:{n} is {cols} columns wide; the frame around it is {frame}"
                    for n, cols in framed
                    if cols != frame
                ]
    return counted, errors


def self_test() -> None:
    """A guard nobody has seen fail is not a guard (CLAUDE.md, code phase rules)."""
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "screens").mkdir()
        # One line past 80 columns, one block past 24 rows, one frame with a
        # line a column short. The box-drawing characters make every line
        # longer in bytes than it is in columns — a byte-counting check would
        # fail the innocent ones and still miss the ragged frame.
        wide = "│" + "─" * 79 + "│"  # 81 columns, 241 bytes
        tall = ["│" + " " * 8 + "│"] * 25
        ragged = ["┌" + "─" * 8 + "┐", "│" + " " * 9 + "│", "└" + "─" * 8 + "┘"]
        (root / "screens" / "bad.md").write_text(
            "```\n" + wide + "\n```\n"
            "```\n" + "\n".join(tall) + "\n```\n"
            "```\n" + "\n".join(ragged) + "\n```\n"
        )
        _, errors = check(root)
        assert len(errors) == 3, f"expected 3 failures, got {len(errors)}: {errors}"
        assert "bad.md:2 is 81 columns wide" in errors[0], errors[0]
        assert "is 25 rows tall" in errors[1], errors[1]
        assert "the frame around it is 10" in errors[2], errors[2]

        # And the healthy shape must stay quiet — including the unbordered
        # header row, which is narrower than the frame on every real screen.
        (root / "screens" / "bad.md").write_text(
            "```\n"
            " nodes 3/3\n"
            "┌" + "─" * 8 + "┐\n"
            "│" + " " * 8 + "│\n"
            "└" + "─" * 8 + "┘\n"
            "```\n"
        )
        _, errors = check(root)
        assert not errors, f"false positive on a well-formed mockup: {errors}"
    print("screens-check: self-test passed — it fails on wide, tall and ragged mockups")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
        sys.exit(0)
    counted, problems = check(ROOT)
    for line in problems:
        print(f"FAIL {line}", file=sys.stderr)
    if not problems:
        print(f"screens-check: {counted} mockups fit {MAX_COLS}x{MAX_ROWS} — OK")
    sys.exit(1 if problems else 0)
