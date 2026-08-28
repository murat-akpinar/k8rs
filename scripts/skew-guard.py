#!/usr/bin/env python3
"""Fail unless the two `SKEW_ALLOWANCE` constants are the same duration.

`rules::age` blanks a card's time past five minutes of skew
(NOTES § D69), and `k8s::measure` says *why* past the same five
(NOTES § D176, `screens/states.md` § The threshold). `rules.rs` is frozen and its
constant is private, so `k8s.rs` carries a second copy rather than reaching back
to make the first `pub` — a deliberate choice, and the one this guard pays for.

**Nothing else ties them.** Each file's tests pin only its own copy: drifting
`k8s.rs`'s alone fails `k8s::tests`, drifting `rules.rs`'s alone fails
`rules::tests::snapshot`, and neither notices the other (`tester`, 2026-08-28).
So a deliberate re-tune that updates one file *and its tests* is green with the
two out of step — and that ships exactly the defect the box closed: a screen that
blanks with no sentence to explain it, or a sentence promising a blank while
`age` is still drawing the number.

**Values, not strings.** `from_secs(300)` and `from_mins(5)` are the same five
minutes, and a guard that reddened on the spelling would be red for nothing,
which is the gate people learn to wave through. A spelling this cannot read is a
constant it did not find, and that fails loudly rather than passing quietly.

Usage:
    skew-guard.py                  # check src/rules.rs and src/k8s.rs
    skew-guard.py <f> <f> ...      # check some other files (the self-test uses this)
    skew-guard.py --self-test      # prove the guard fails when it should
"""
import contextlib, io, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# The pair. Adding a third file is one entry; there is no third today and this is
# not a general "two constants somewhere must agree" facility.
FILES = ("src/rules.rs", "src/k8s.rs")

# Tied to the name on purpose: a rename in one file makes the declaration
# unfindable there, which is a zero count, which is a failure.
SECONDS = {"secs": 1, "mins": 60, "hours": 3600}
DECL = re.compile(
    r"^const\s+SKEW_ALLOWANCE\s*:\s*SignedDuration\s*=\s*"
    r"SignedDuration::from_(secs|mins|hours)\((\d+)\)\s*;",
    re.M,
)


def declared(path: Path):
    """Every `SKEW_ALLOWANCE` declaration in one file, as (seconds, source text)."""
    text = path.read_text(encoding="utf-8")
    return [(int(n) * SECONDS[unit], m.group(0).strip())
            for m in DECL.finditer(text)
            for unit, n in [m.groups()]]


def run(paths) -> int:
    # One path compares a file with itself and is green whatever it says. Found by
    # this guard's own second pass, before it was ever wired in.
    if len(paths) < 2:
        print(f"FAIL {len(paths)} path(s) given, need at least 2 — one file agrees with "
              f"itself, so this guard was about to vet nothing")
        return 1
    found = {}
    for name in paths:
        path = Path(name) if Path(name).is_absolute() else ROOT / name
        if not path.exists():
            print(f"FAIL {name} does not exist — this guard was about to vet nothing")
            return 1
        hits = declared(path)
        # The canary. "The two agree" and "I read neither" print the same line
        # otherwise (CLAUDE.md § A derived list asserts it found something) — and
        # a renamed, deleted or re-spelled constant lands here, not in the
        # comparison below.
        if len(hits) != 1:
            print(f"FAIL {name} declares SKEW_ALLOWANCE {len(hits)} time(s), expected 1 — "
                  f"renamed, deleted, duplicated, or written in a unit this guard cannot "
                  f"read ({'/'.join(SECONDS)}). It was about to vet nothing")
            return 1
        found[name] = hits[0]

    print("skew-guard: " + " · ".join(f"{n} = {s}s" for n, (s, _) in found.items()))
    values = {s for s, _ in found.values()}
    if len(values) != 1:
        for n, (s, src) in found.items():
            print(f"FAIL {n}:  {src}   ({s}s)")
        print("the two SKEW_ALLOWANCE constants disagree. `rules::age` blanks a time at one "
              "of these and `k8s::measure` explains it at the other, so one of them is now "
              "lying to the reader (NOTES § D69, § D176)")
        return 1
    print(f"OK — all {len(found)} agree at {values.pop()}s")
    return 0


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    import tempfile

    def plant(d, name, body):
        (d / name).write_text(body)
        return str(d / name)

    same = "const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(5);\n"
    # The spelling that must NOT be a failure: the same five minutes, written the
    # other way. A guard red here is red for nothing.
    spelled = "const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_secs(300);\n"
    drifted = "const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(10);\n"
    renamed = "const SKEW_TOLERANCE: SignedDuration = SignedDuration::from_mins(5);\n"
    unreadable = "const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_days(1);\n"

    def check(a, b, want, why):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            paths = [plant(d, "a.rs", a), plant(d, "b.rs", b)]
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                rc = run(paths)
            assert rc == want, f"{why}: expected {want}, got {rc}\n{buf.getvalue()}"
            return buf.getvalue()

    check(same, same, 0, "two identical constants")
    check(same, spelled, 0, "the same five minutes spelled in seconds")
    out = check(same, drifted, 1, "a drifted constant")
    assert "disagree" in out, out
    # The three shapes that break a literal-comparing guard by making it match
    # nothing — each has to fail, and each has to say it vetted nothing rather
    # than that the two agree.
    for other, why in ((renamed, "a renamed constant"),
                       ("fn a() {}\n", "a deleted constant"),
                       (unreadable, "a unit this guard cannot read"),
                       (same + same, "a duplicated constant")):
        out = check(same, other, 1, why)
        assert "vet nothing" in out, (why, out)
    # …and the canary in its purest form: two files with nothing in them at all
    # must fail, not report agreement.
    out = check("", "", 1, "two empty files")
    assert "vet nothing" in out, out
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        for paths, want in (([str(d / "gone.rs"), plant(d, "b.rs", same)], "does not exist"),
                            # Fewer than two files is a comparison with nothing on the
                            # other side of it.
                            ([plant(d, "a.rs", same)], "vet nothing"),
                            ([], "vet nothing")):
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                rc = run(paths)
            assert rc == 1 and want in buf.getvalue(), (paths, buf.getvalue())

    print("skew-guard: self-test passed — a drifted constant is refused; the same five "
          "minutes spelled `from_secs(300)` is not; and a renamed, deleted, duplicated or "
          "unreadably-spelled constant, an empty file, a missing one and fewer than two "
          "files each fail as *vetted nothing* rather than passing as agreement")


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

sys.exit(run(sys.argv[1:] or FILES))
