#!/usr/bin/env python3
"""Fail unless each constant that exists twice is the same duration in both files.

`rules.rs` is frozen and its constants are private, so `k8s.rs` carries a second
copy rather than reaching back to make the first `pub`. That is a deliberate
choice each time, and this guard is what it costs. Two pairs today:

* **`SKEW_ALLOWANCE`** — `rules::age` blanks a card's time past five minutes of
  skew (NOTES § D69) and `k8s::measure` says *why* past the same five
  (NOTES § D176, `screens/states.md` § The threshold).
* **`CERT_EXPIRY_WARN`** — C1 warns about the reader's own kubeconfig
  certificate at thirty days (`rules.rs` § the certificate rules) and C2's
  sentence about the API server's own certificate is drawn at the same thirty
  (NOTES § D178, `screens/once.md` § *One threshold, thirty days, shared with
  the kubeconfig certificate*).

**Nothing else ties either pair.** Measured, 2026-08-28: setting `k8s.rs`'s
`CERT_EXPIRY_WARN` to sixty days leaves `cargo test --all-targets` at 639 passed
/ 0 failed and every guard green, because `rules_tests` pins only `rules.rs`'s
copy and `main_tests` asks its boundary question *in terms of* `k8s.rs`'s — so
neither file's tests can see the other move. `SKEW_ALLOWANCE` is the same shape
one rule down (`tester`, the same day). A deliberate re-tune that updates one
file *and its tests* is green with the two out of step, and that ships the
defect the boxes closed: a screen that blanks with no sentence to explain it, or
one report warning about two certificates at two distances with nothing to say
why one gets more runway.

**Values, not strings.** `from_secs(300)` and `from_mins(5)` are the same five
minutes, and `from_hours(30 * 24)` is thirty days however it is written; a guard
that reddened on the spelling would be red for nothing, which is the gate people
learn to wave through. A spelling this cannot read is a constant it did not
find, and that fails loudly rather than passing quietly.

Usage:
    twin-guard.py                       # every pair below
    twin-guard.py NAME <f> <f> ...      # one constant over some other files
    twin-guard.py --self-test           # prove the guard fails when it should
"""
import contextlib, io, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# The pairs. Adding a third is one entry; this is not a general "two constants
# somewhere must agree" facility, and a name with only one home does not belong.
TWINS = {
    "SKEW_ALLOWANCE": (("src/rules.rs", "src/k8s.rs"), "NOTES § D69, § D176"),
    "CERT_EXPIRY_WARN": (("src/rules.rs", "src/k8s.rs"), "NOTES § D178"),
}

SECONDS = {"secs": 1, "mins": 60, "hours": 3600}
# `30 * 24` is how a day count is spelled in hours here. Anything richer than
# `N` or `N * M` is a spelling this guard cannot read, and that is a failure —
# never a silent skip.
AMOUNT = r"\d+(?:\s*\*\s*\d+)?"


def decl(name):
    """Tied to the name on purpose: a rename makes the declaration unfindable in
    that file, which is a zero count, which is a failure."""
    return re.compile(
        rf"^(?:pub(?:\([a-z]+\))?\s+)?const\s+{re.escape(name)}\s*:\s*SignedDuration\s*=\s*"
        rf"SignedDuration::from_(secs|mins|hours)\(({AMOUNT})\)\s*;",
        re.M,
    )


def amount(text):
    """`30 * 24` -> 720. The regex has already refused everything else."""
    n = 1
    for part in text.split("*"):
        n *= int(part.strip())
    return n


def declared(path: Path, name):
    """Every declaration of one constant in one file, as (seconds, source text)."""
    text = path.read_text(encoding="utf-8")
    return [(amount(m.group(2)) * SECONDS[m.group(1)], m.group(0).strip())
            for m in decl(name).finditer(text)]


def run(name, paths, why="NOTES § D69, § D176, § D178") -> int:
    # One path compares a file with itself and is green whatever it says. Found by
    # this guard's own second pass, before it was ever wired in.
    if len(paths) < 2:
        print(f"FAIL {name}: {len(paths)} path(s) given, need at least 2 — one file agrees "
              f"with itself, so this guard was about to vet nothing")
        return 1
    found = {}
    for entry in paths:
        path = Path(entry) if Path(entry).is_absolute() else ROOT / entry
        if not path.exists():
            print(f"FAIL {name}: {entry} does not exist — this guard was about to vet nothing")
            return 1
        hits = declared(path, name)
        # The canary. "The two agree" and "I read neither" print the same line
        # otherwise (CLAUDE.md § A derived list asserts it found something) — and
        # a renamed, deleted or re-spelled constant lands here, not in the
        # comparison below.
        if len(hits) != 1:
            print(f"FAIL {name}: {entry} declares it {len(hits)} time(s), expected 1 — "
                  f"renamed, deleted, duplicated, or written in a unit this guard cannot "
                  f"read ({'/'.join(SECONDS)}). It was about to vet nothing")
            return 1
        found[entry] = hits[0]

    print(f"twin-guard: {name} — " + " · ".join(f"{n} = {s}s" for n, (s, _) in found.items()))
    values = {s for s, _ in found.values()}
    if len(values) != 1:
        for n, (s, src) in found.items():
            print(f"FAIL {n}:  {src}   ({s}s)")
        print(f"the two {name} constants disagree, so one of them is now lying to the reader "
              f"({why})")
        return 1
    print(f"OK — all {len(found)} agree at {values.pop()}s")
    return 0


def every() -> int:
    # An empty table would print nothing at all and exit 0, which is this guard
    # reporting success over a list it never built.
    if not TWINS:
        print("FAIL TWINS is empty — this guard was about to vet nothing")
        return 1
    return max(run(name, paths, why) for name, (paths, why) in TWINS.items())


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    import tempfile

    def plant(d, name, body):
        (d / name).write_text(body)
        return str(d / name)

    same = "const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(5);\n"
    # The spellings that must NOT be a failure: the same five minutes written the
    # other way, and the same behind a `pub(crate)`, which is the shape `k8s.rs`
    # keeps because the renderer applies the threshold (NOTES § D178).
    spelled = "const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_secs(300);\n"
    exported = "pub(crate) const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(5);\n"
    drifted = "const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_mins(10);\n"
    renamed = "const SKEW_TOLERANCE: SignedDuration = SignedDuration::from_mins(5);\n"
    unreadable = "const SKEW_ALLOWANCE: SignedDuration = SignedDuration::from_days(1);\n"
    # The product spelling of a day count, and the same thirty days in seconds.
    hours = "const CERT_EXPIRY_WARN: SignedDuration = SignedDuration::from_hours(30 * 24);\n"
    secs = "const CERT_EXPIRY_WARN: SignedDuration = SignedDuration::from_secs(2592000);\n"
    short = "const CERT_EXPIRY_WARN: SignedDuration = SignedDuration::from_hours(20 * 24);\n"

    def check(a, b, want, why, name="SKEW_ALLOWANCE"):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            paths = [plant(d, "a.rs", a), plant(d, "b.rs", b)]
            buf = io.StringIO()
            with contextlib.redirect_stdout(buf):
                rc = run(name, paths)
            assert rc == want, f"{why}: expected {want}, got {rc}\n{buf.getvalue()}"
            return buf.getvalue()

    check(same, same, 0, "two identical constants")
    check(same, spelled, 0, "the same five minutes spelled in seconds")
    check(same, exported, 0, "the same five minutes behind a pub(crate)")
    out = check(same, drifted, 1, "a drifted constant")
    assert "disagree" in out, out
    # The arithmetic spelling, which is what a day count is written as here.
    check(hours, hours, 0, "two day counts written in hours", "CERT_EXPIRY_WARN")
    check(hours, secs, 0, "thirty days in hours and in seconds", "CERT_EXPIRY_WARN")
    out = check(hours, short, 1, "a shortened window", "CERT_EXPIRY_WARN")
    assert "disagree" in out, out
    # A constant that exists under one name and not the other is not agreement.
    out = check(hours, same, 1, "the other pair's constant", "CERT_EXPIRY_WARN")
    assert "vet nothing" in out, out
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
                rc = run("SKEW_ALLOWANCE", paths)
            assert rc == 1 and want in buf.getvalue(), (paths, buf.getvalue())
    # An empty table is this guard passing over a list it never built.
    global TWINS
    kept, TWINS = TWINS, {}
    buf = io.StringIO()
    with contextlib.redirect_stdout(buf):
        rc = every()
    TWINS = kept
    assert rc == 1 and "vet nothing" in buf.getvalue(), buf.getvalue()

    print("twin-guard: self-test passed — a drifted constant is refused; the same duration "
          "spelled `from_secs(300)`, `from_hours(30 * 24)` or behind a `pub(crate)` is not; "
          "and a renamed, deleted, duplicated or unreadably-spelled constant, the wrong pair's "
          "name, an empty file, a missing one, fewer than two files and an empty table each "
          "fail as *vetted nothing* rather than passing as agreement")


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

if len(sys.argv) > 1:
    sys.exit(run(sys.argv[1], sys.argv[2:]))
sys.exit(every())
