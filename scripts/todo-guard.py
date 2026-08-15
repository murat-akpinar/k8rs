#!/usr/bin/env python3
"""Assert `todo.md` still carries every phase heading and its done-when blocks.

A context compaction deleted the `## Phase 3` heading together with Phase 2's
`**Done when:**` / `**Frozen after:**` block — the block that carries the
justfile exception Phase 7 depends on (restored in a9e9397). Everything *under*
them survived, so the loss was invisible box by box: the file simply read as
though Phase 2 ran on for another 130 lines, and "the first unchecked box in the
lowest open phase" — the rule CLAUDE.md and `.claude/commands/basla.md` use to
pick the next box — pointed into a closed phase.

`check-docs.py` cannot see this and is not the place to fix it: nothing linked
to the deleted heading, so no anchor broke. A heading with no inbound link is
precisely the shape a link checker is blind to.

Structure only. What it asserts:

  * `## Phase N —` for N = 0…13, each exactly once, in ascending order
  * `**Done when:**` under every phase the file's own convention gives one, and
    `**Frozen after:**` likewise — read off the file, see the sets below
  * the headings still carry *this* file's titles (CANARY), so a parser that
    counted fourteen of something else goes red rather than green

Ceiling: it checks that the marker *line* is present, not that the paragraph
under it still says anything. A block gutted below its `**Done when:**` reads
green here.

Usage:
    todo-guard.py             # check todo.md
    todo-guard.py --self-test # prove the guard fails when it should
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TODO = ROOT / "todo.md"

PHASES = tuple(range(14))  # 0…13. A genuinely new phase updates this line.

# Both sets are read off todo.md, not assumed, and both are *exemptions from a
# requirement* rather than bans: a phase that gains a marker later is not the
# damage this guard exists for, so it is not reported.
#
# Phase 0 is the design phase — it ends on a rule about when Phase 1 may start,
# not on a done-when, and it freezes no code.
NO_DONE_WHEN = {0}
# Phase 8's code is a throwaway spike that never enters `src/`; Phase 12 wires
# the top layer, with nothing left underneath it to freeze; Phase 13 ships.
NO_FROZEN_AFTER = {0, 8, 12, 13}

# First, middle and last phase titles. The order check is already self-canarying
# for an empty parse — no headings found is `[] != PHASES`, which is red — but
# it cannot tell todo.md from any other file with fourteen phase headings in it.
CANARY = {0: "Design", 3: "The product: rules", 13: "Ship v0.1"}

HEADING = re.compile(r"^##\s+(.*?)\s*$")
# The em dash is the file's convention across all fourteen headings. Requiring
# it is deliberate: `## Phase 3 - …` is a heading this repo did not write.
PHASE = re.compile(r"^Phase\s+(\d+)\s+—\s*(.*)$")
# Anchored at column 0 and with the colon, because the file also writes
# `**Done when**` *inside* a box, indented, to state one box's own done-when
# (todo.md:1390, 1811, 2023). Those are not a phase's block and must not
# satisfy it.
MARKER = re.compile(r"^\*\*(Done when|Frozen after):\*\*")
FENCE = re.compile(r"^\s*(`{3,}|~{3,})")


def outside_fences(text):
    """(line number, line) for every line that is not inside a code fence.

    The opening marker is remembered rather than toggled, so a ``` written
    inside a ~~~ block does not close it. todo.md carries no fence today; a
    fenced `**Done when:**` added later would otherwise satisfy a phase that
    has none, which is the silent direction.
    """
    opener = None
    for n, line in enumerate(text.splitlines(), 1):
        m = FENCE.match(line)
        if m:
            tok = m.group(1)[0]
            if opener is None:
                opener = tok
            elif opener == tok:
                opener = None
            continue
        if opener is None:
            yield n, line


def parse(text):
    """([(line, number, title)], {number: {marker: [lines]}}) for one todo.md.

    A marker belongs to the nearest phase heading above it. Any other `##`
    heading ends the phase — `## Later (recorded, not planned)` closes the file,
    and a marker under it belongs to no phase.
    """
    phases, markers, current = [], {}, None
    for n, line in outside_fences(text):
        h = HEADING.match(line)
        if h:
            p = PHASE.match(h.group(1))
            current = int(p.group(1)) if p else None
            if p:
                phases.append((n, current, p.group(2).strip()))
                markers.setdefault(current, {})
            continue
        m = MARKER.match(line)
        if m and current is not None:
            markers[current].setdefault(m.group(1), []).append(n)
    return phases, markers


def problems(phases, markers):
    """Every structural failure, as printable lines."""
    out = []
    nums = [num for _, num, _ in phases]

    if nums != list(PHASES):
        missing = [p for p in PHASES if p not in nums]
        duplicated = sorted({p for p in nums if nums.count(p) > 1})
        unknown = sorted({p for p in nums if p not in PHASES})
        if missing:
            out.append(
                f"missing phase heading(s) {missing} — a deleted `## Phase N —` "
                f"line silently merges its boxes into the phase above (a9e9397)"
            )
        if duplicated:
            out.append(f"phase heading(s) {duplicated} appear more than once")
        if unknown:
            out.append(
                f"phase heading(s) {unknown} are not in PHASES — if a phase was "
                f"genuinely added, update PHASES in this file"
            )
        if not (missing or duplicated or unknown):
            out.append(f"phase headings out of order: {nums}")

    titles = {num: title for _, num, title in phases}
    for num, fragment in CANARY.items():
        if fragment not in titles.get(num, ""):
            out.append(
                f"canary: Phase {num}'s title does not contain {fragment!r} — "
                f"the guard is not reading the todo.md it was written for"
            )

    for num in PHASES:
        if num not in markers:
            continue  # already reported as a missing heading
        for marker, exempt in (("Done when", NO_DONE_WHEN), ("Frozen after", NO_FROZEN_AFTER)):
            if num not in exempt and marker not in markers[num]:
                out.append(f"Phase {num} has no `**{marker}:**` block")
    return out


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    real = TODO.read_text(encoding="utf-8")
    base = problems(*parse(real))
    assert not base, f"todo.md itself is red — fix that before trusting this: {base}"

    lines = real.splitlines()
    three = next(l for l in lines if l.startswith("## Phase 3 —"))
    four = next(l for l in lines if l.startswith("## Phase 4 —"))
    missing_three = [
        "missing phase heading(s) [3] — a deleted `## Phase N —` line silently "
        "merges its boxes into the phase above (a9e9397)",
        "canary: Phase 3's title does not contain 'The product: rules' — the "
        "guard is not reading the todo.md it was written for",
    ]

    def drop(match, after=0):
        """`real` with the first line at or after `after` that matches removed."""
        i = next(i for i, line in enumerate(lines) if i >= after and match(line))
        return "\n".join(lines[:i] + lines[i + 1 :]) + "\n"

    # --- the damage a9e9397 restored START ---
    # 1. the `## Phase 3` heading, gone. Phase 3's 1000-odd boxes then read as
    #    Phase 2's, and the next box is picked out of a closed phase. Asserted
    #    whole rather than by substring: nothing *else* may fire, or a later
    #    edit that breaks Phase 2 could keep this plant green on its own noise.
    hit = problems(*parse(drop(lambda l: l == three)))
    assert hit == missing_three, hit

    # 2. a `**Done when:**` block, gone. Phase 2's is the one that went, and it
    #    is the one that carries the justfile exception Phase 7 depends on. The
    #    message naming Phase 2 is what proves `after` skipped Phase 1's block.
    phase2_at = next(i for i, l in enumerate(lines) if l.startswith("## Phase 2 —"))
    hit = problems(*parse(drop(lambda l: l.startswith("**Done when:**"), after=phase2_at)))
    assert hit == ["Phase 2 has no `**Done when:**` block"], hit

    # 3. and `**Frozen after:**`, the other half of the same block.
    hit = problems(*parse(drop(lambda l: l.startswith("**Frozen after:**"), after=phase2_at)))
    assert hit == ["Phase 2 has no `**Frozen after:**` block"], hit
    # --- the damage a9e9397 restored END ---

    # Demoting a heading is a deletion as far as the phase index is concerned,
    # and it is the shape an over-eager reformat produces rather than a drop.
    hit = problems(*parse(real.replace(three, "#" + three, 1)))
    assert hit == missing_three, hit

    # Duplicated, and out of order: both leave the count at fourteen, which is
    # what a check that only counted headings would call green.
    hit = problems(*parse(real.replace(four, three, 1)))
    assert hit == [
        "missing phase heading(s) [4] — a deleted `## Phase N —` line silently "
        "merges its boxes into the phase above (a9e9397)",
        "phase heading(s) [3] appear more than once",
    ], hit

    # Out of order with all fourteen still present exactly once: the count, the
    # canary and every marker are untouched, so ordering is the only thing left
    # that can catch a phase moved above the one it depends on.
    swapped = real.replace(three, "\0", 1).replace(four, three, 1).replace("\0", four, 1)
    hit = problems(*parse(swapped))
    assert hit == [
        "phase headings out of order: [0, 1, 2, 4, 3, 5, 6, 7, 8, 9, 10, 11, 12, 13]"
    ], hit

    # --- shapes this file actually produces (NOTES § D29) START ---
    synthetic = (
        "## Phase 0 — Design\n"
        "## Phase 1 — x\n"
        "      inherit it too. **Done when** a fixture carries a status with no\n"
        "      declaration, the rule says so.\n"
        "**Frozen after:** clippy.toml\n"
    )
    _, markers = parse(synthetic)
    # The in-box `**Done when**` — indented, no colon — is not the phase's
    # block, and a guard that accepted it would have called the real damage
    # green from anywhere in the 130 lines below it.
    assert markers[1] == {"Frozen after": [5]}, markers

    # A marker inside either fence is sample text, not a phase's block.
    fenced = (
        "## Phase 0 — Design\n"
        "## Phase 1 — x\n"
        "```\n**Done when:** not really\n```\n"
        "~~~\n```\n**Frozen after:** still not\n~~~\n"
    )
    _, markers = parse(fenced)
    assert markers[1] == {}, markers
    # …and a heading inside a fence is not a heading, or a fenced example of
    # this file's own layout becomes a duplicate phase.
    phases, _ = parse("## Phase 0 — Design\n```\n## Phase 0 — Design\n```\n")
    assert [num for _, num, _ in phases] == [0], phases
    # --- shapes this file actually produces (NOTES § D29) END ---

    # The canary is not decorative: fourteen headings in order, none of them
    # this file's, must not read as a healthy todo.md.
    other = "\n".join("## Phase %d — Something Else" % n for n in PHASES)
    hit = problems(*parse(other))
    assert any(p.startswith("canary:") for p in hit), hit

    # The negative half of every plant above: nothing was left mutated, and the
    # unmodified file is still green.
    assert not problems(*parse(real)), "the self-test mutated the real file"

    print(
        "todo-guard: self-test passed — a deleted phase heading, a deleted "
        "done-when and a deleted frozen-after are each caught, a duplicated or "
        "reordered heading is caught, and an in-box `**Done when**` or a fenced "
        "one does not satisfy a phase"
    )


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
        sys.exit(0)

    phases, markers = parse(TODO.read_text(encoding="utf-8"))
    found = problems(phases, markers)
    for line in found:
        print(f"FAIL todo.md  {line}", file=sys.stderr)
    if found:
        sys.exit(1)
    counts = {
        m: sum(len(v.get(m, [])) for v in markers.values())
        for m in ("Done when", "Frozen after")
    }
    print(
        f"todo-guard: {len(phases)} phase headings ({PHASES[0]}–{PHASES[-1]}) in "
        f"order, {counts['Done when']} done-when and {counts['Frozen after']} "
        f"frozen-after blocks — OK"
    )
