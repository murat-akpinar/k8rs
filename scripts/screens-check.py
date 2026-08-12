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
# Both markers, like check-docs.py. Matching only ``` meant a mockup written
# inside a ~~~ block was not a mockup here — and ~~~ is exactly what someone
# reaches for when the drawing itself contains a ``` line. Measured: a
# 201-column frame inside a ~~~ fence produced "0 mockups ... OK".
FENCE = re.compile(r"^\s*(`{3,}|~{3,})")
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


def blocks(path: Path) -> list[tuple[int, list[str], bool]]:
    """Every fenced block as (first content line, lines, was it ever closed).

    The opening marker is remembered rather than toggled, so a ``` inside a ~~~
    block does not end it. A block nobody closed is still returned, and flagged:
    dropping it is how a mockup stops being measured without anyone noticing.
    """
    found: list[tuple[int, list[str], bool]] = []
    opener, start, buf = None, 0, []
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        m = FENCE.match(line)
        if m:
            tok = m.group(1)[0]
            if opener is None:
                opener, start, buf = tok, n + 1, []
            elif opener == tok:
                found.append((start, buf, True))
                opener = None
            continue
        if opener is not None:
            buf.append(line)
    if opener is not None:
        found.append((start, buf, False))
    return found


def check(root: Path) -> tuple[int, list[str]]:
    errors, counted = [], 0
    # rglob: a mockup one directory down is still a mockup, and `glob` walked
    # straight past it.
    for path in sorted((root / "screens").rglob("*.md")):
        rel = path.relative_to(root)
        for start, lines, closed in blocks(path):
            counted += 1
            if not closed:
                errors.append(
                    f"{rel}:{start - 1} opens a fence that is never closed — "
                    f"the rest of the file is one runaway mockup"
                )
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
    # The floor. Every check above relates a mockup to a limit, and every one of
    # them holds over an empty list: rename the directory, rename the files off
    # `.md`, or leave every fence unmatched, and this printed
    # "0 mockups fit 80x24 — OK". Measuring nothing is not measuring zero
    # defects (CLAUDE.md § a derived list asserts it found something).
    if not counted:
        errors.append(
            "no mockup found under screens/ — this guard measured nothing and "
            "reported OK, which is what it does when the directory has moved"
        )
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

        # --- the shapes that used to print "0 mockups ... OK" START ---
        # Each one was measured green over a 201-column frame — a width no
        # screen could ever be — because nothing here counted as a mockup.
        overflowing = "│" + "─" * 199 + "│"

        # A ~~~ fence. Nothing in screens/ uses one today, which is exactly when
        # to nail it down: the reason to reach for ~~~ is a drawing containing
        # a ``` line, and that is a mockup like any other.
        (root / "screens" / "bad.md").write_text("~~~\n" + overflowing + "\n~~~\n")
        _, errors = check(root)
        assert any("201 columns" in e for e in errors), f"a ~~~ mockup was not measured: {errors}"

        # …and a ``` inside a ~~~ block must not close it, or the second half of
        # the drawing stops being measured.
        (root / "screens" / "bad.md").write_text(
            "~~~\n```\n" + overflowing + "\n~~~\n"
        )
        _, errors = check(root)
        assert any("201 columns" in e for e in errors), f"the inner ``` closed the ~~~ block: {errors}"

        # A fence nobody closed. The block was dropped on the floor, so the
        # drawing inside it was never measured.
        (root / "screens" / "bad.md").write_text("```\n" + overflowing + "\n")
        _, errors = check(root)
        assert any("never closed" in e for e in errors), f"an unterminated fence went unreported: {errors}"

        # A mockup one directory down.
        (root / "screens" / "bad.md").unlink()
        (root / "screens" / "sub").mkdir()
        (root / "screens" / "sub" / "deep.md").write_text("```\n" + overflowing + "\n```\n")
        _, errors = check(root)
        assert any("201 columns" in e for e in errors), f"a nested mockup was skipped: {errors}"

        # And the floor: nothing to measure is not a pass.
        (root / "screens" / "sub" / "deep.md").unlink()
        counted, errors = check(root)
        assert counted == 0 and any("measured nothing" in e for e in errors), (
            f"an empty screens/ reported OK: {counted}, {errors}"
        )
        # --- the shapes that used to print "0 mockups ... OK" END ---
    print("screens-check: self-test passed — it fails on wide, tall and ragged "
          "mockups, on either fence marker, on a fence left open, on a nested "
          "file, and on finding nothing to measure at all")


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
