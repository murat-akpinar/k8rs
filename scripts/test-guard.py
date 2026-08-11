#!/usr/bin/env python3
"""Fail the build when the test suite reports success without having run.

Two ways that happens, both cheap to check and both invisible in a green log:

1. A test is *declared* in the source but never *listed* by the runner — a
   `#[cfg(...)]` gate, a module that stopped being `mod`-declared, a rename
   that left the file orphaned. The suite passes because it ran nothing.
2. `#[ignore]` disables a test while leaving its name in the file, so the
   count still looks right to a human reading the diff.

Neither is caught by `cargo test`, which exits 0 over zero tests.

Usage:
    test-guard.py            # check this repository
    test-guard.py --self-test  # prove the guard fails when it should
"""

import re
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
    # Skipped paths are matched *relative to the root* — an absolute path under
    # /tmp is not a `tmp/` directory of ours, and treating it as one made this
    # function silently return nothing.
    skip = {"target", "tmp"}
    return sorted(
        p
        for p in root.rglob("*.rs")
        if not skip & set(p.relative_to(root).parts)
    )


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


def listed(root: Path) -> int:
    """How many tests the runner actually sees."""
    out = subprocess.run(
        ["cargo", "test", "--all-targets", "--", "--list"],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if out.returncode != 0:
        print(out.stderr.strip(), file=sys.stderr)
        sys.exit("test-guard: `cargo test -- --list` failed; fix the build first")
    return len(re.findall(r"^\S.*: test$", out.stdout, re.MULTILINE))


def check(root: Path) -> list[str]:
    declared, unexplained = declared_and_ignored(root)
    errors = [f"{loc}  #[ignore] with no reason — say why, or delete the test"
              for loc in unexplained]
    seen = listed(root)
    if seen < declared:
        errors.append(
            f"{declared} tests declared in the source, {seen} listed by the "
            f"runner — {declared - seen} never run"
        )
    if not errors:
        print(f"test-guard: {declared} declared, {seen} listed — OK")
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
    print("test-guard: self-test passed — it fails on hidden and parked tests")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
        sys.exit(0)
    problems = check(ROOT)
    for line in problems:
        print(f"FAIL {line}", file=sys.stderr)
    sys.exit(1 if problems else 0)
