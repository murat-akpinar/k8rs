#!/usr/bin/env python3
"""CI's cross matrix, parsed once, for the two readers that need it.

CI's `cross:` matrix is **the one list of release targets** (NOTES § D305
ruling 1): `just cross` reads it rather than keeping a copy, and
`.github/workflows/release.yml` builds exactly that list rather than declaring
one of its own. Two readers with two parsers is the same drift one level down —
a `sed` in the justfile and an `awk` in a workflow agree right up until the
matrix is reformatted — so both go through this file.

The release needs the **runner** as well as the triple, which is the other
reason the justfile's `sed -n 's/- target: //p'` could not simply be pasted into
YAML: `aarch64-unknown-linux-musl` is built on an ARM runner for a reason CI's
own comment gives, and a matrix that lost the `os:` column would build it on
amd64 and fail in `ring`'s build script.

Both canaries the justfile used to carry live here now, because "extracted
nothing" and "nothing to extract" print the same line (CLAUDE.md § A derived
list asserts it found something):

* the parse must yield at least one row, and
* it must yield `x86_64-unknown-linux-musl` — the target REQUIREMENTS § Build &
  release names as the primary artifact. If that one goes on purpose, this line
  moves to whatever replaced it; it does not get deleted.

Usage:
    ci-targets.py --targets     # one triple per line — `just cross`
    ci-targets.py --matrix      # {"include": [{"target": …, "os": …}]} — a GH matrix
    ci-targets.py --self-test
"""
import json, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"

# A matrix row is two lines: the `- target:` that opens it and the `os:` that
# belongs to it. Anchored, so `runs-on: ${{ matrix.os }}` is not an `os:` line
# and a `- target:` inside a comment is not a row.
TARGET = re.compile(r"^\s*-\s*target:\s*(\S+)\s*$")
RUNNER = re.compile(r"^\s*os:\s*(\S+)\s*$")
CANARY = "x86_64-unknown-linux-musl"


class Unreadable(Exception):
    """The matrix could not be read — never a silent empty list."""


def rows(text: str) -> list[dict[str, str]]:
    """The `{target, os}` pairs of a workflow's matrix, in file order.

    Pure, so the self-test can feed it every broken shape without a workflow on
    disk. A half-written row is an error and not a skip: a target with no runner
    would otherwise vanish from the release and from `just cross` at once.
    """
    found: list[dict[str, str]] = []
    pending: tuple[str, int] | None = None
    for n, raw in enumerate(text.splitlines(), 1):
        line = raw.split(" #", 1)[0].rstrip()
        if line.lstrip().startswith("#"):
            continue
        hit = TARGET.match(line)
        if hit:
            if pending:
                raise Unreadable(f"ci.yml:{n}  `- target: {hit.group(1)}` follows "
                                 f"`- target: {pending[0]}` (line {pending[1]}) with no "
                                 f"`os:` between them — one of the two has no runner")
            pending = (hit.group(1), n)
            continue
        hit = RUNNER.match(line)
        if hit:
            if not pending:
                raise Unreadable(f"ci.yml:{n}  `os: {hit.group(1)}` belongs to no "
                                 f"`- target:` — the matrix is not the shape this parses")
            found.append({"target": pending[0], "os": hit.group(1)})
            pending = None
    if pending:
        raise Unreadable(f"ci.yml:{pending[1]}  `- target: {pending[0]}` has no `os:` — "
                         f"the row names no runner to build it on")
    if not found:
        raise Unreadable("no `- target:` / `os:` rows in ci.yml's matrix — either the "
                         "matrix was renamed and this parser was about to hand back an "
                         "empty list, or the cross job is gone")
    if not any(r["target"] == CANARY for r in found):
        raise Unreadable(f"{CANARY} is not in ci.yml's matrix — it is the primary "
                         f"release artifact (REQUIREMENTS § Build & release), so either "
                         f"the parse is reading the wrong lines or the target was dropped "
                         f"on purpose and this canary moves to whatever replaced it")
    return found


def read() -> list[dict[str, str]]:
    if not WORKFLOW.exists():
        raise Unreadable(f"{WORKFLOW} does not exist — there is no matrix to read")
    return rows(WORKFLOW.read_text(encoding="utf-8"))


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    good = ("    strategy:\n      matrix:\n        include:\n"
            "          - target: x86_64-unknown-linux-musl\n"
            "            os: ubuntu-latest\n"
            "          # a comment, and `- target: not-a-row` inside it\n"
            "          - target: aarch64-apple-darwin\n"
            "            os: macos-latest\n"
            "    steps:\n      - run: echo ${{ matrix.os }}\n"
            "    runs-on: ${{ matrix.os }}\n")

    def refused(text, want):
        try:
            rows(text)
        except Unreadable as why:
            assert want in str(why), f"expected {want!r} in {why}"
            return
        raise AssertionError(f"expected a refusal mentioning {want!r}")

    got = rows(good)
    assert got == [{"target": CANARY, "os": "ubuntu-latest"},
                   {"target": "aarch64-apple-darwin", "os": "macos-latest"}], got
    # `runs-on: ${{ matrix.os }}` is not an `os:` row, and neither is a comment.
    assert len(got) == 2, got

    refused(good.replace("            os: ubuntu-latest\n", "", 1), "no `os:` between them")
    refused(good.replace("          - target: x86_64-unknown-linux-musl\n", "", 1),
            "belongs to no")
    refused(good + "          - target: x86_64-unknown-linux-freebsd\n", "names no runner")
    # The two canaries: each must fail as *read nothing*, never pass as a list.
    refused("jobs:\n  cross:\n    runs-on: ubuntu-latest\n", "about to hand back an")
    refused("", "about to hand back an")
    refused(good.replace(CANARY, "riscv64gc-unknown-linux-musl"), "primary")

    # The real ci.yml, because a self-test that only sees planted strings says
    # nothing about the file this actually parses.
    live = read()
    assert len(live) >= 2, live
    assert any(r["target"] == CANARY for r in live), live
    assert all(r["os"] for r in live), live
    print(f"ci-targets: self-test passed — {len(live)} row(s) read out of the real "
          f"ci.yml ({' · '.join(r['target'] + ' on ' + r['os'] for r in live)}); a row "
          f"with no runner, a runner with no row, a matrix that was renamed away and an "
          f"empty file each fail as *read nothing* rather than as an empty list, and so "
          f"does a matrix that no longer builds {CANARY}")


def main(argv: list[str]) -> int:
    if "--self-test" in argv:
        self_test()
        return 0
    try:
        found = read()
    except Unreadable as why:
        print(f"FAIL: {why}", file=sys.stderr)
        return 1
    if "--matrix" in argv:
        # A whole `strategy.matrix` object rather than a bare array: that is the
        # shape `matrix: ${{ fromJSON(…) }}` takes, and one line, because
        # release.yml captures it into a job output.
        print(json.dumps({"include": found}, separators=(",", ":")))
    elif "--targets" in argv:
        for row in found:
            print(row["target"])
    else:
        print("FAIL: pass --targets, --matrix or --self-test", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
