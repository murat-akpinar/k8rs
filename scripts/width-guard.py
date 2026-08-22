#!/usr/bin/env python3
"""Fail on any line in src/ past 100 columns.

`cargo fmt` reflows code and leaves comments alone, so the 100-column rule has
only ever been a convention: two over-long lines shipped into `rules.rs` on
2026-08-15 and were caught by a reviewer counting characters rather than by the
build (todo.md, Phase 4).

**Config is not the fix.** `rustfmt`'s `wrap_comments` and
`error_on_line_overflow` are both nightly-only, so a `rustfmt.toml` carrying
them is silently ignored on the pinned stable toolchain — worse than no gate,
because it looks like one. 100 is `rustfmt`'s own default `max_width`, which is
what formats the code half of every file today.

Usage:
    width-guard.py              # check src/
    width-guard.py <dir>        # check some other tree (the self-test uses this)
    width-guard.py --self-test  # prove the guard fails when it should
"""
import contextlib, io, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIMIT = 100

# The one exemption, and it is a line *shape*, not a file or a marker: a
# markdown table row inside a comment. A table row cannot be wrapped without
# ceasing to be a table row, which is what makes this a real exemption rather
# than an allowance — everything else over the limit gets rewrapped (PM ruling,
# todo.md Phase 4). Deliberately not "a line containing a pipe": a prose line
# that happens to mention `a | b` is prose and wraps like prose.
COMMENT = re.compile(r'^\s*(?://[/!]?)\s?(.*)$')


def table_row(line: str) -> bool:
    """A comment whose body starts and ends with `|` — a markdown table row."""
    m = COMMENT.match(line)
    return bool(m) and m.group(1).strip().startswith('|') and m.group(1).strip().endswith('|')


def over(path: Path):
    """(line number, width, line) for every line past the limit that is not exempt.

    Width is character count, which is what `rustfmt` produces for the ASCII this
    file is written in and what a reviewer counting characters did. A line built
    of double-width characters would be undercounted; none exists in `src/`, and
    the day one does the fix is a width table, not a different limit.
    """
    out = []
    for n, line in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
        if len(line) > LIMIT and not table_row(line):
            out.append((n, len(line), line))
    return out


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    import tempfile

    pad = 'x' * 120
    # `|` at both ends of the comment body, at each of the three comment
    # spellings the file uses. All three are exempt.
    for lead in ('/// ', '//! ', '// ', '    /// '):
        row = lead + '| ' + pad + ' | b |'
        assert table_row(row), row
    # …and the same row is genuinely over the limit, or the exemption is being
    # proven on a line the guard would have passed anyway.
    assert len('/// | ' + pad + ' | b |') > LIMIT

    # A long prose line is not exempt, at any of the spellings.
    for lead in ('/// ', '//! ', '// ', '    // ', ''):
        assert not table_row(lead + pad), lead

    # The shape the ruling names explicitly: a long line that merely *contains*
    # a pipe. Prose wraps like prose.
    assert not table_row('/// a value of `a | b` ' + pad)
    # A row that starts with a pipe and does not end with one is a wrapped table
    # row, which is already broken markdown — not exempt.
    assert not table_row('/// | ' + pad)
    # …and one that ends with a pipe without starting with one.
    assert not table_row('/// ' + pad + ' |')
    # Code is never exempt, however many pipes it holds: a bitmask or a match
    # arm is not a table (`|` at both ends is reachable in Rust).
    assert not table_row('    let m = a | b | c |')

    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        (d / 'clean.rs').write_text('fn a() {}\n' + '/// | ' + pad + ' | b |\n')
        assert over(d / 'clean.rs') == [], over(d / 'clean.rs')
        (d / 'dirty.rs').write_text('fn a() {}\n/// ' + pad + '\n')
        got = over(d / 'dirty.rs')
        assert [(n, w) for n, w, _ in got] == [(2, 124)], got

        # The canary: an empty tree must fail rather than report nothing wrong.
        # "No line is too long" and "I read no files" print the same line
        # otherwise (CLAUDE.md § A derived list asserts it found something).
        empty = d / 'empty'
        empty.mkdir()
        # `run` prints; swallow it so the canary's own FAIL line does not read
        # like a failure of the self-test in CI's trace.
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = run(empty)
        assert rc == 1, "an empty tree passed — this guard vets nothing"
        assert "vet nothing" in buf.getvalue(), buf.getvalue()

    print("width-guard: self-test passed — a comment holding a markdown table "
          "row is exempt at any width, a long prose line is not, and neither is "
          "a line that merely contains a `|`")


def run(tree: Path) -> int:
    files = sorted(tree.rglob('*.rs'))
    bad = [(f, n, w, line) for f in files for n, w, line in over(f)]
    if not files:
        print(f"FAIL {tree} holds no .rs files — this guard was about to vet nothing")
        return 1
    # The exemption is counted and printed, not merely named: a widening nobody
    # can see the size of is how one line-shape quietly becomes most of the file.
    exempt = sum(1 for f in files
                 for line in f.read_text(encoding='utf-8').splitlines()
                 if len(line) > LIMIT and table_row(line))
    print(f"checked {len(files)} Rust file(s) under {tree} at {LIMIT} columns; "
          f"{exempt} line(s) exempt as a markdown table row inside a comment — a "
          f"comment body starting and ending with `|`, which cannot be wrapped "
          f"and stay a table row (todo.md Phase 4, the PM's ruling)")
    for f, n, w, line in bad:
        try:
            name = f.relative_to(ROOT)
        except ValueError:
            name = f
        print(f"FAIL {name}:{n}  {w} columns  {line.strip()[:60]}…")
    print("OK — every line fits in 100 columns" if not bad
          else f"{len(bad)} line(s) past {LIMIT} columns — rewrap them; "
               f"`cargo fmt` will not")
    return 1 if bad else 0


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

sys.exit(run(Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "src"))
