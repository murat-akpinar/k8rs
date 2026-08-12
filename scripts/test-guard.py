#!/usr/bin/env python3
"""Fail the build when the test suite reports success without having run.

Two ways that happens, both cheap to check and both invisible in a green log:

1. A test is *declared* in the source but never *listed* by the runner — a
   `#[cfg(...)]` gate, a module that stopped being `mod`-declared, a rename
   that left the file orphaned. The suite passes because it ran nothing.
2. `#[ignore]` disables a test while leaving its name in the file, so the
   count still looks right to a human reading the diff.
3. There is no test at all. Deleting the last one leaves nothing to compare,
   so every count above agrees at zero and the guard reports OK — which is
   what it did until the floor in `check()` existed.

None of the three is caught by `cargo test`, which exits 0 over zero tests.

Usage:
    test-guard.py            # check this repository
    test-guard.py --self-test  # prove the guard fails when it should
"""

import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# `#[test]` / `#[tokio::test]` / `#[test_case(...)]`, not `#[test_only_helper]`
DECLARED = re.compile(r"#\[\s*(?:[\w:]+::)?test\b")
# An #[ignore] is allowed only when the same line says why: #[ignore = "..."]
IGNORED = re.compile(r"#\[\s*ignore\s*(?:\]|=\s*\"(?P<reason>[^\"]*)\")")


def sources(root: Path) -> list[Path]:
    """Every .rs file cargo actually compiles — an allowlist of roots.

    This used to walk the whole tree minus `target/` and `tmp/`, which counts
    every copy of the source anyone leaves lying around. `just mutants` leaves
    exactly that: `cargo mutants` writes a full working copy under
    `mutants.out/`, so a run of it doubled the declared count and `just check`
    reported "45 tests never run" — a red build with nothing wrong, and the kind
    that gets repaired by weakening the guard. The blocklist would need a new
    entry per tool; these four directories plus build.rs are the whole of what
    the compiler reads, and they are the same roots write-guard.py scans.
    """
    return sorted(
        p
        for r in ("src", "tests", "examples", "benches")
        for p in (root / r).rglob("*.rs")
    ) + [p for p in [root / "build.rs"] if p.is_file()]


def declared_and_ignored(root: Path) -> tuple[int, list[str]]:
    """Count declared tests; collect `#[ignore]`s that carry no reason."""
    declared, unexplained = 0, []
    for path in sources(root):
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            declared += len(DECLARED.findall(line))
            for m in IGNORED.finditer(line):
                if not (m.group("reason") or "").strip():
                    rel = path.relative_to(root)
                    unexplained.append(f"{rel}:{n}")
    return declared, unexplained


def listed(root: Path, only_ignored: bool = False) -> int:
    """How many tests the runner actually sees."""
    args = ["cargo", "test", "--all-targets", "--", "--list"]
    if only_ignored:
        args.append("--ignored")
    out = subprocess.run(args, cwd=root, capture_output=True, text=True)
    if out.returncode != 0:
        print(out.stderr.strip(), file=sys.stderr)
        sys.exit("test-guard: `cargo test -- --list` failed; fix the build first")
    return len(re.findall(r"^\S.*: test$", out.stdout, re.MULTILINE))


def check(root: Path) -> list[str]:
    declared, unexplained = declared_and_ignored(root)
    errors = [f"{loc}  #[ignore] with no reason — say why, or delete the test"
              for loc in unexplained]
    # The floor D26 asks for, and the one check here that is not a comparison.
    # Every other rule below relates two counts, and every one of them holds at
    # zero: delete the last test and `0 declared, 0 listed, 0 ignored` reads OK
    # — measured, that is exactly what this guard printed before this line.
    # It returns rather than appends because with no test declared the counts
    # below describe nothing, and a second derived complaint would only bury
    # the one fact that matters.
    if declared == 0:
        return errors + [
            "no test is declared anywhere in the source — `cargo test` reports "
            "`0 passed` and exits 0 (NOTES § D26)"
        ]
    seen = listed(root)
    if seen < declared:
        errors.append(
            f"{declared} tests declared in the source, {seen} listed by the "
            f"runner — {declared - seen} never run"
        )
    # A reason string is written by the same person doing the ignoring, so it
    # documents an intent rather than proving one. What it must never buy is a
    # whole suite: parked *every* test and `cargo test` reports `0 passed`,
    # which is the "passes because it ran nothing" case in the docstring above,
    # reached through the door the reason check leaves open.
    parked = listed(root, only_ignored=True) if seen else 0
    if seen and parked == seen:
        errors.append(
            f"all {seen} tests are #[ignore]d — `cargo test` reports 0 passed "
            f"and exits 0. A reason explains one parked test; it cannot explain "
            f"a parked suite"
        )
    if not errors:
        print(f"test-guard: {declared} declared, {seen} listed, {parked} ignored — OK")
    return errors


def self_test() -> None:
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "src").mkdir()
        # One test hidden behind a feature nobody enables, one #[ignore]
        # without a reason: declared 2, listed 0.
        (root / "src" / "lib.rs").write_text(
            '#[cfg(feature = "never-enabled")]\n'
            "mod hidden {\n"
            "    #[test]\n"
            "    fn hidden_test() {}\n"
            "}\n"
            "\n"
            "#[test]\n"
            "#[ignore]\n"
            "fn parked() {}\n"
        )
        declared, unexplained = declared_and_ignored(root)
        assert declared == 2, f"expected 2 declared tests, counted {declared}"
        assert unexplained, "an #[ignore] with no reason went unreported"

        # And the healthy shape must stay quiet.
        (root / "src" / "lib.rs").write_text(
            "#[test]\n"
            "fn real() {}\n"
            "\n"
            "#[test]\n"
            '#[ignore = "needs a cluster"]\n'
            "fn e2e() {}\n"
        )
        declared, unexplained = declared_and_ignored(root)
        assert declared == 2, f"expected 2 declared tests, counted {declared}"
        assert not unexplained, f"false positive on an explained ignore: {unexplained}"

        # A second copy of the source tree, which is what `just mutants` leaves
        # in the repo root on every run. Counting it doubles `declared`, and the
        # guard then reports every real test as "never run".
        copy = root / "mutants.out" / "scratch" / "src"
        copy.mkdir(parents=True)
        (copy / "lib.rs").write_text((root / "src" / "lib.rs").read_text())
        declared, _ = declared_and_ignored(root)
        assert declared == 2, (
            f"a copy of the source under mutants.out/ was counted: {declared} declared, "
            f"expected 2"
        )
        shutil.rmtree(root / "mutants.out")

        # The empty suite. Source exists, no test in it — the shape left behind
        # by deleting the last test, which every count-comparison in `check()`
        # reports as OK because they all agree at zero.
        #
        # This case makes the temp directory a real crate, and that is not
        # decoration. Without a Cargo.toml, deleting the floor makes `check()`
        # fall through to `listed()`, which dies on "could not find Cargo.toml"
        # — the run goes red, but for a reason that has nothing to do with the
        # floor, so it proves nothing about the assertion below. With a crate
        # here, removing the floor makes `check()` return no errors at all and
        # the assertion is what fails. Red for the right reason, or it is not a
        # witness (NOTES § D26).
        (root / "Cargo.toml").write_text(
            '[package]\nname = "guard-self-test"\nversion = "0.0.0"\n'
            'edition = "2021"\n\n[lib]\npath = "src/lib.rs"\n'
        )
        (root / "src" / "lib.rs").write_text("pub fn nothing() {}\n")
        errors = check(root)
        assert any("no test is declared" in e for e in errors), (
            f"an empty test suite reported OK: {errors}"
        )

        # …and the negative, in the same crate: one real test must NOT trip the
        # floor. Without it the floor could return unconditionally and the
        # positive above would still pass.
        (root / "src" / "lib.rs").write_text("#[test]\nfn real() {}\n")
        errors = check(root)
        assert not errors, f"false positive — one real test is not an empty suite: {errors}"
    print("test-guard: self-test passed — it fails on hidden, parked and absent "
          "tests, and does not count a copy of the tree under mutants.out/")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
        sys.exit(0)
    problems = check(ROOT)
    for line in problems:
        print(f"FAIL {line}", file=sys.stderr)
    sys.exit(1 if problems else 0)
