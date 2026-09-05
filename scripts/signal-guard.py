#!/usr/bin/env python3
"""Fail unless every mark `theme.rs` ships is drawn, whole, by the screen file that promises it.

`theme.rs` holds eight `Signal` constants. Seven are `Signal::Mark(..)` — a glyph or a word the
screen keeps when colour is gone — and each one is a transcription of a drawing in `screens/`:

    ● ▲ ○      screens/once.md      the severity band
    ▸          screens/context.md   the row the keys act on
    changing…  screens/dialogs.md   a write in flight
    ⚠          screens/states.md    the alarm glyph, carrier for a family of banners
    read-only  screens/states.md    the header's mode field

`theme_tests.rs` pins each constant against a hand-copy in its own `SIGNALS` table, so the code
and that copy cannot drift. **Nothing there reads `screens/`, and nothing there may.**
`Cargo.toml`'s `exclude` keeps `screens/` out of the published package (NOTES § D193), so an
`include_str!("../screens/…")` in the test file compiles here and fails for anyone who runs the
suite on the crate they downloaded — and `cargo publish` verifies with a *build*, which never
compiles a `#[cfg(test)]` module, so it would ship green and break in their hands. That is why
the screen-file half of the link is this script and not a test: on a `just check` run the file is
always on disk, and the guard never enters the package.

**Whole, not contained.** `screen.contains(mark)` is what the test did until 2026-09-05, and a
substring search passes anything a mark is truncated to — `read-only` cut to `read-onl`, or
`changing…` with its ellipsis dropped; six such plantings survived the suite (`tester`). So the
mark is compared against the *cells* a mockup line offers: split the line on the separators the
drawings use (`│ ┃ | ·` and the backtick, which four mockup lines draw literally because the app
prints one), trim each field, and take the field and its whitespace-separated words. `read-only`
is one of those cells; `read-onl` is not, and neither is a mark that arrives with a stray space
or a zero-width joiner.

**A drawing, not an occurrence** — only fenced blocks are read. `screens/` is prose as well as
mockups, and a mark the prose happens to spell is not a mark the screen draws. Measured
2026-09-05: `read-only` sits in `states.md` seven times, and four of them — :171, :399, :711
("was outside a read-only pass over a shared") and :747 — are prose, so every mockup in that
file could be deleted and a whole-file search would still pass; `disconnected` had the same hole
through a "While disconnected…" heading, which is the ceiling NOTES § D242 recorded, and
restricting to fences closes both. Measured with the mockups actually deleted: `states.md`,
`context.md` and `once.md` each stayed *green* under the whole-file read, with `read-only`, `⚠`,
`▸`, `●`, `▲` and `○` all still spelled in the prose that was left.
Of the seven marks, five lose occurrences to this and none loses its last one — `read-only` in
`states.md` goes 7 → 3, `⚠` 18 → 12, and `changing…` was never anything but the one drawing at
`dialogs.md`:816.

The fences come from `screens-check.py`'s own `blocks()` rather than a second parser here: it
already handles ``` and ~~~, remembers the opener so a ``` inside a ~~~ block does not end it,
and flags a block nobody closed. Unclosed blocks are dropped, because an unclosed fence would
otherwise turn the rest of the file into one runaway drawing — the loose direction. That is not
this script's to report: `screens-check.py` fails on it, and it runs immediately before this one
in `scripts/guards.sh`.

**`FOCUS` is not checked, and this says so on every run.** It is `Signal::Reverse` — a fill, not
a mark — and no drawing in `screens/` can fix it: a monochrome ASCII mockup has no way to show a
reversed row, and which pane the keys are aimed at is never drawn at all (`focus` appears zero
times in all eleven files, measured 2026-09-05). `▸` is a different thing — it marks the row
inside a pane, not the pane. A guard that skipped `FOCUS` silently would be indistinguishable
from one that lost it. If it ever becomes a `Mark`, this fails: a mark with no screen file behind
it is the thing the script exists to refuse.

Usage:
    signal-guard.py                       # src/theme.rs against screens/
    signal-guard.py <theme.rs> <screens/> # some other pair, for a red/green demonstration
    signal-guard.py --self-test           # prove the guard fails when it should
"""
import contextlib
import importlib.util
import io
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# One fence parser in `scripts/`, not two. `screens-check.py` already walks these files and its
# `blocks()` is the tested one; a copy here is the second copy that goes stale (CLAUDE.md § Write
# function-based). Loaded by path because the filename has a hyphen, and it is safe to import —
# everything that runs sits under its `if __name__ == "__main__"`.
_spec = importlib.util.spec_from_file_location("screens_check", ROOT / "scripts/screens-check.py")
_screens_check = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_screens_check)
blocks = _screens_check.blocks

# Which drawing promises which constant. Measured, not assumed — occurrences *inside fenced
# blocks*, 2026-09-05: once.md ●=7 ▲=8 ○=6, context.md ▸=2, dialogs.md `changing…`=1, states.md
# ⚠=12 and `read-only`=3.
#
# Two of these rows are the witness and not merely a file that contains the mark:
#
# `SELECTION` was `alerts.md` until 2026-09-05, and alerts.md's only two `▸` (lines 8 and 376)
# are the *sidebar* nav row — a different thing from what the constant's own doc describes, "the
# row the keys would act on, in any list, table or picker" (`tui-designer`, Phase 9 close review,
# finding 5). Every `▸` in context.md (:29, :216) is that row, which is why it is here and not
# resources.md: resources.md:13 and :172 are content rows too, but resources.md:12 is a sidebar
# row that would hold this green on its own.
#
# `ALARM` replaced `DISCONNECTED` when the sentence moved out of theme.rs and the glyph stayed.
# The glyph carries a family wider than any one file — measured 2026-09-05, `⚠ disconnected,
# retrying`, `⚠ login expired` and `⚠ your clock is behind`/`ahead` here, and `⚠ TLS not
# verified`, `⚠ not allowed`, `⚠ cluster undefined`, `⚠ duplicate name` in context.md. states.md
# keeps the row because that file's subject *is* the alarm family (12 fenced occurrences to
# context.md's 7) and because it was DISCONNECTED's file, so nothing moved that did not have to.
# One file is what this table can name; if the glyph ever stops being drawn everywhere *except*
# states.md, this stays green — the same shape as every other row, and not a hole this one opens.
SCREENS = {
    "CRITICAL_SIGNAL": "once.md",
    "WARN_SIGNAL": "once.md",
    "INFO_SIGNAL": "once.md",
    "SELECTION": "context.md",
    "CHANGING": "dialogs.md",
    "ALARM": "states.md",
    "READ_ONLY": "states.md",
}

# The constants no screen file can pin, with the reason printed on every run.
UNPINNED = {
    "FOCUS": "Signal::Reverse — a fill and not a mark, and no mockup can draw a reversed row, "
    "so nothing in screens/ fixes it; which pane the keys are aimed at is never drawn at all "
    "(`focus` appears zero times in all eleven files)",
}

# `pub const NAME: Signal = Signal::Mark("…");` / `= Signal::Reverse;`. Tied to the shape on
# purpose: a constant written any other way is one this guard did not find, which is a count
# mismatch below, which is loud.
DECL = re.compile(
    r"^pub const ([A-Z][A-Z0-9_]*)\s*:\s*Signal\s*=\s*"
    r"Signal::(?:Mark\(\"([^\"\n]*)\"\)|(Reverse))\s*;",
    re.M,
)

# What a mockup line is divided by: the box borders, the header's middle dot, and the backtick —
# which is here because four mockup lines draw one, not because prose quotes marks in them; prose
# is no longer read. Not the comma — `⚠ disconnected, retrying` contains one.
SEPARATORS = "│┃|·`"


def cells(path: Path) -> set[str]:
    """Every whole thing this file's *drawings* offer: each separated field, and each of its words.

    Fenced blocks only — the prose around them is not a drawing, see the module docstring. A
    block left open is dropped rather than read to the end of the file.

    `│▸ ALERTS     3 ● 7 ▲│` offers `▸`, `ALERTS`, `3`, `●`, `7`, `▲` and the whole field;
    `ctx: prod-eu · ⚠ disconnected, retrying` offers the banner entire. Nothing here offers a
    truncation of any of them, which is the whole point.
    """
    found: set[str] = set()
    for _, lines, closed in blocks(path):
        if not closed:
            continue
        for line in lines:
            for field in re.split(f"[{re.escape(SEPARATORS)}]", line):
                field = field.strip()
                if field:
                    found.add(field)
                    found.update(field.split())
    return found


def run(theme: Path, screens: Path) -> int:
    if not theme.exists():
        print(f"FAIL {theme} does not exist — this guard was about to vet nothing")
        return 1
    if not screens.is_dir():
        print(f"FAIL {screens} is not a directory — this guard was about to vet nothing")
        return 1

    matches = DECL.findall(theme.read_text(encoding="utf-8"))
    signals: dict[str, str | None] = {n: None if rev else m for n, m, rev in matches}
    expected = set(SCREENS) | set(UNPINNED)

    # The canary. "Every mark is drawn" and "I read no constants at all" print the same line
    # otherwise (CLAUDE.md § A derived list asserts it found something) — and a renamed, deleted,
    # duplicated or unreadably-spelled constant lands here rather than in the comparison below.
    if len(signals) != len(matches):
        print(f"FAIL {theme.name} declares {len(matches)} Signal constants under "
              f"{len(signals)} names — one is declared twice, so which one ships is a coin toss")
        return 1
    if set(signals) != expected:
        missing = ", ".join(sorted(expected - set(signals))) or "none"
        extra = ", ".join(sorted(set(signals) - expected)) or "none"
        print(f"FAIL {theme.name}: declares {len(signals)} Signal constants, of which "
              f"{len(set(signals) & expected)} of the {len(expected)} this guard knows about. "
              f"Not found: {missing}. Not in the table: {extra}. "
              f"Renamed, deleted, added, or written in a shape this guard cannot read "
              f"(`pub const NAME: Signal = Signal::Mark(\"…\");`) — either way it was about to "
              f"vet nothing for those names")
        return 1

    # A mark spelled `"\u{25b8}"` is the same glyph to rustc and a different string to this
    # script, which compares source text. Refusing it names the spelling; comparing it anyway
    # would report a correct constant as one the screen does not draw (found by this guard's own
    # self-test, before it was wired in).
    escaped = sorted(n for n, m in signals.items() if m and "\\" in m)
    if escaped:
        print(f"FAIL {', '.join(escaped)} carry a Rust escape in the literal. This guard "
              f"compares the source text against the drawing, so it cannot read that spelling — "
              f"write the character itself, as the file does today")
        return 1

    rc = 0
    for name, reason in sorted(UNPINNED.items()):
        if signals[name] is not None:
            print(f"FAIL {name} is now Signal::Mark({signals[name]!r}) and no screen file "
                  f"promises it. Draw it in screens/ and give it a row in SCREENS here, or it "
                  f"ships pinned by nothing")
            rc = 1
            continue
        print(f"signal-guard: {name} is NOT checked against any screen file — {reason}")

    for name, filename in sorted(SCREENS.items()):
        path = screens / filename
        mark = signals[name]
        if mark is None:
            print(f"FAIL {name} is Signal::Reverse, but {filename} promises a mark for it — a "
                  f"meaning that was drawn is now carried by the fill alone")
            rc = 1
            continue
        if not path.exists():
            print(f"FAIL {name}: {path} does not exist — this guard was about to vet nothing "
                  f"for {mark!r}")
            rc = 1
            continue
        drawn = cells(path)
        if not drawn:
            print(f"FAIL {name}: {filename} offered no drawing to compare {mark!r} against — it "
                  f"has no closed fenced block in it, so this guard was about to vet nothing")
            rc = 1
            continue
        if mark in drawn:
            print(f"signal-guard: {name} = {mark!r} — drawn by screens/{filename}")
            continue
        inside = sorted((c for c in drawn if mark and mark in c), key=len)[:3]
        hint = (f" It appears only inside {', '.join(repr(c) for c in inside)}, so what ships is "
                f"a truncation of what the screen draws." if inside else
                f" Nothing that file draws contains it at all.")
        print(f"FAIL {name} = {mark!r} is not drawn by screens/{filename}.{hint} The screen file "
              f"changes first, or the constant goes back")
        rc = 1

    if rc == 0:
        print(f"OK — {len(SCREENS)} marks drawn verbatim by their screen files, "
              f"{len(UNPINNED)} unchecked and named above")
    return rc


def self_test() -> None:
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    import tempfile

    good = {
        "CRITICAL_SIGNAL": "●",
        "WARN_SIGNAL": "▲",
        "INFO_SIGNAL": "○",
        "SELECTION": "▸",
        "FOCUS": None,
        "CHANGING": "changing…",
        "ALARM": "⚠",
        "READ_ONLY": "read-only",
    }
    # Lines lifted from the shapes the real files draw — a bordered band, a picker row, a header
    # with middle-dot fields — inside fences, because a fence is now the whole of what is read.
    # Every one of them is wrapped in prose that spells its own marks, which is the real files'
    # shape too (the module docstring names the four prose `read-only` in states.md): that prose
    # is what a whole-file search mistook for a drawing, and none of it may pin anything.
    PROSE = ("While disconnected the header goes read-only and the picker keeps its ▸ on the\n"
             "row the keys act on; a write in flight says changing… and the band counts ● ▲ ○\n"
             "— every mark this guard reads, `⚠ login expired` included, spelled in prose that\n"
             "draws nothing. None of it may pin a constant.\n")
    drawings = {
        "once.md": PROSE + "```\n"
                   "│ ● 3 critical    ▲ 7 warnings    ○ nothing is broken │\n```\n",
        "context.md": PROSE + "```\n"
                      "│   │  ▸ prod-eu           aws · prod   (current)  │   │\n"
                      "│   │    ⚠ cluster undefined                       │   │\n```\n",
        "dialogs.md": PROSE + "```\nheader   ctx: prod-eu · live · admin · changing…\n```\n",
        "states.md": PROSE + "```\n"
                     " nodes 3/3 (40s ago)      ctx: prod-eu · ⚠ disconnected, retrying\n"
                     " nodes 3/3               ctx: prod-eu · ns: payments · read-only\n```\n"
                     "and `⚠ login expired` is the same banner with a different sentence.\n",
    }

    def source(marks: dict, extra: str = "") -> str:
        body = "".join(
            f"pub const {n}: Signal = Signal::Reverse;\n" if m is None
            else f'pub const {n}: Signal = Signal::Mark("{m}");\n'
            for n, m in marks.items()
        )
        return "/// doc\n" + body + extra

    def check(marks, want, why, screens=None, extra=""):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            (d / "theme.rs").write_text(source(marks, extra), encoding="utf-8")
            (d / "screens").mkdir()
            for name, text in (screens or drawings).items():
                (d / "screens" / name).write_text(text, encoding="utf-8")
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                rc = run(d / "theme.rs", d / "screens")
            assert rc == want, f"{why}: expected {want}, got {rc}\n{buf.getvalue()}"
            return buf.getvalue()

    out = check(good, 0, "the marks as they ship")
    # The unchecked one is named on a green run, not only on a red one.
    assert "FOCUS is NOT checked" in out, out
    assert "7 marks drawn verbatim" in out, out

    # 1. Truncated to a substring of what the screen draws — the defect `contains` let through,
    #    at both ends of the string and down to a single character. `ALARM` is one glyph, so its
    #    truncations are the banner it used to carry, cut short, and the empty string.
    for name, cut in (("READ_ONLY", "read-onl"), ("READ_ONLY", "read"), ("READ_ONLY", "r"),
                      ("ALARM", "⚠ disconnected"), ("ALARM", "⚠ disconnected,"),
                      ("CHANGING", "changing")):
        out = check({**good, name: cut}, 1, f"{name} truncated to {cut!r}")
        assert "is a truncation of what the screen draws" in out, (cut, out)
    out = check({**good, "ALARM": ""}, 1, "ALARM truncated to nothing at all")
    assert "is not drawn by screens/states.md" in out, out

    # 2. Changed to something no screen file draws — a look-alike glyph, and a word.
    for name, other in (("CRITICAL_SIGNAL", "◆"), ("READ_ONLY", "readonly"),
                        ("ALARM", "⛔"), ("CHANGING", "changing...")):
        out = check({**good, name: other}, 1, f"{name} changed to {other!r}")
        assert "Nothing that file draws contains it at all" in out, (other, out)
    # An ASCII stand-in for a glyph is red either way — it happens to sit inside `nothing`, so
    # it is reported as a truncation rather than as absent. Both messages are the same refusal.
    out = check({**good, "INFO_SIGNAL": "o"}, 1, "○ replaced by a latin o")
    assert "is not drawn by screens/once.md" in out, out

    # 3. A mark that draws nothing extra but is not the cell either: stray whitespace and an
    #    invisible character, both of which a `contains` search waves through.
    for name, padded in (("READ_ONLY", " read-only"), ("READ_ONLY", "read-only\u200b"),
                         ("CRITICAL_SIGNAL", "● ")):
        out = check({**good, name: padded}, 1, f"{name} as {padded!r}")
        assert "is not drawn by" in out, (padded, out)

    # 4. Renamed out of the pattern, deleted, spelled in a shape this cannot read, or duplicated.
    renamed = {("READ_ONLY_SIGNAL" if n == "READ_ONLY" else n): m for n, m in good.items()}
    out = check(renamed, 1, "a renamed constant")
    assert "Not found: READ_ONLY" in out and "Not in the table: READ_ONLY_SIGNAL" in out, out
    assert "7 of the 8 this guard knows about" in out, out
    out = check({n: m for n, m in good.items() if n != "SELECTION"}, 1, "a deleted constant")
    assert "vet nothing for those names" in out, out
    # A `const` that is not `pub`, and a mark carrying a Rust escape, are both spellings the
    # regex refuses — each has to land as *not found*, never as a silent skip.
    for extra_src, why in (
        ("const SELECTION: Signal = Signal::Mark(\"▸\");\n", "a private constant"),
    ):
        out = check({n: m for n, m in good.items() if n != "SELECTION"}, 1, why,
                    extra=extra_src)
        assert "Not found: SELECTION" in out, (why, out)
    # …and a spelling the regex *does* match but cannot compare fails as unreadable, not as a
    # mark the screen stopped drawing.
    out = check({**good, "SELECTION": "\\u{25b8}"}, 1, "an escaped mark")
    assert "cannot read that spelling" in out, out
    out = check(good, 1, "a duplicated constant",
                extra='pub const READ_ONLY: Signal = Signal::Mark("read-only");\n')
    assert "declared twice" in out, out
    # An empty product file is the canary in its purest form.
    out = check({}, 1, "a theme.rs with no constants in it")
    assert "declares 0 Signal constants, of which 0 of the 8" in out, out
    # A new constant nobody attributed to a screen fails too — silence would be a mark that
    # ships pinned by nothing.
    out = check({**good, "PENDING": "…"}, 1, "an unattributed new constant")
    assert "Not in the table: PENDING" in out, out
    # The Phase 9 close exactly as it arrived: `DISCONNECTED` became `ALARM` in theme.rs, and a
    # table still naming the old one vets nothing for either.
    stale = {("DISCONNECTED" if n == "ALARM" else n): m for n, m in good.items()}
    out = check(stale, 1, "ALARM still spelled DISCONNECTED")
    assert "Not found: ALARM" in out and "Not in the table: DISCONNECTED" in out, out

    # 5. The screen file stops drawing its mark — the drawing changed and the code did not.
    for name, text in (("states.md", "```\n nodes 3/3   ctx: prod-eu · ns: payments\n```\n"),
                       ("context.md", "```\n│   │  prod-eu   aws · prod  (current) │   │\n```\n"),
                       ("once.md", "")):
        out = check(good, 1, f"{name} without its mark",
                    screens={**drawings, name: text})
        assert "is not drawn by" in out or "offered no drawing" in out, (name, out)

    # 5b. The ceiling NOTES § D242 recorded, which is why only fenced blocks are read: the
    #     mockups deleted and the prose that spells the same marks left behind. First prove the
    #     planting plants — `PROSE` offers every mark as a whole cell, so each of these was green
    #     under the whole-file read this replaced, not merely under a `contains` search.
    with tempfile.TemporaryDirectory() as tmp:
        fenced_prose = Path(tmp) / "prose.md"
        fenced_prose.write_text(f"```\n{PROSE}```\n", encoding="utf-8")
        offered = cells(fenced_prose)
    assert all(m in offered for m in ("read-only", "⚠", "▸", "changing…", "●", "▲", "○")), offered
    for name in ("states.md", "context.md", "dialogs.md", "once.md"):
        out = check(good, 1, f"{name} reduced to prose that still spells its marks",
                    screens={**drawings, name: PROSE})
        assert "offered no drawing" in out, (name, out)

    # 5c. A fence nobody closed is not a drawing either: reading it to the end of the file is the
    #     loose direction, and reporting it is `screens-check.py`'s job, one step earlier.
    out = check(good, 1, "a mockup inside a fence left open",
                screens={**drawings, "dialogs.md": "```\nheader · live · changing…\n"})
    assert "offered no drawing" in out, out
    #     …and the skip reaches exactly that block: a closed drawing followed by a fence left
    #     open still pins its mark, or dropping the unclosed one would have dropped the file.
    check(good, 0, "a closed drawing with an unclosed fence after it",
          screens={**drawings,
                   "dialogs.md": "```\nheader · live · changing…\n```\n```\nrunaway\n"})

    # 6. FOCUS turned into a mark, and a mark turned into the fill. Both are a meaning changing
    #    carrier with no drawing behind it.
    out = check({**good, "FOCUS": "◆"}, 1, "FOCUS as a Mark")
    assert "no screen file promises it" in out, out
    out = check({**good, "SELECTION": None}, 1, "SELECTION as Reverse")
    assert "carried by the fill alone" in out, out

    # 7. Missing inputs, which is the guard vetting nothing while looking busy.
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        (d / "theme.rs").write_text(source(good), encoding="utf-8")
        (d / "screens").mkdir()
        for paths, want in (((d / "gone.rs", d / "screens"), "does not exist"),
                            ((d / "theme.rs", d / "nowhere"), "is not a directory"),
                            ((d / "theme.rs", d / "screens"), "does not exist")):
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                rc = run(*paths)
            assert rc == 1 and want in buf.getvalue(), (paths, buf.getvalue())

    print("signal-guard: self-test passed — a mark truncated to a substring of what the screen "
          "draws is refused (both ends, down to one character and to none), as is one changed to "
          "anything the file does not draw, one padded with a space or a zero-width joiner, a "
          "renamed, deleted, private, escape-spelled, duplicated or unattributed constant, "
          "ALARM still spelled DISCONNECTED, an empty product file, a screen file that stopped "
          "drawing its mark, one whose mockups are gone and whose prose still spells every mark, "
          "one whose fence was left open, FOCUS becoming a Mark, a mark becoming the fill, and a "
          "missing file or directory — and FOCUS is named as unchecked on every green run")


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

if len(sys.argv) == 3:
    sys.exit(run(Path(sys.argv[1]), Path(sys.argv[2])))
if len(sys.argv) != 1:
    print(__doc__.split("Usage:")[1], file=sys.stderr)
    sys.exit(2)
sys.exit(run(ROOT / "src/theme.rs", ROOT / "screens"))
