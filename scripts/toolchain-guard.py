#!/usr/bin/env python3
"""Fail unless the toolchain CI installs is the toolchain this machine runs.

`just check` is the whole of CI, or it is a lie (CLAUDE.md). It was a lie for
seven days and roughly thirty pushes. CI asked for `toolchain: stable`, `stable`
moved under it twice inside one phase — rustc 1.97.x, then 1.98.0 on 2026-08-27,
then 1.98.1 on 2026-09-01 — and clippy 1.98 began emitting `result_large_err` on
two functions nobody had touched. **A lint the local toolchain cannot emit is a
lint the local gate cannot fail on**, so `just check` stayed green on the dev
machine while every single push was red, and the phase-close PR was the first
place anybody looked.

So the version is written in exactly one place — `RUST_TOOLCHAIN` in CI's own
`env:` block — and this guard holds three things together:

* The pin is a **concrete version, never a channel.** `stable` is precisely what
  made this a moving target, so a guard that accepted it would guard nothing.
* **Every** `dtolnay/rust-toolchain` step reads that one value. One job left
  behind on `stable` reopens the drift for whichever job matters most, silently.
* The `rustc` on this PATH **is** that version. This is the half that runs
  locally, and it catches the next bump at the desk of whoever ran the upgrade
  rather than on a phase-close PR a week later. In CI it is not redundant: there
  it asserts the action actually honoured the pin.

**It is a hard failure and not a warning.** A warning about a toolchain mismatch
is a permanent one on every machine that has not bumped yet, and a baseline of
permanent warnings is how a real one goes unread — the same reasoning
`clippy.toml` gives for pinning its own `allow-invalid` set.

Bumping is deliberate, which is the whole point: change `RUST_TOOLCHAIN`, run
`just check`, read what the newer clippy found, fix it, commit.

Usage:
    toolchain-guard.py              # the real run
    toolchain-guard.py --self-test  # prove the guard fails when it should
"""
import contextlib, io, re, subprocess, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"

# The action whose `toolchain:` input decides what CI compiles with. Pinned by
# SHA in the workflow; matched by name here, because the SHA is security-guard's
# business and a rotation of it must not red-light this guard as well.
ACTION = re.compile(r"^\s*-?\s*uses:\s*dtolnay/rust-toolchain@\S+", re.M)
PIN = re.compile(r"^\s*RUST_TOOLCHAIN:\s*\"?([^\"\s#]+)\"?\s*(?:#.*)?$", re.M)
INPUT = re.compile(r"^\s*toolchain:\s*(.+?)\s*$", re.M)
# `1.97.1`, and `1.97` for a release that never got a patch. A channel name has
# no digits and lands in the refusal below.
VERSION = re.compile(r"^\d+\.\d+(?:\.\d+)?$")


def normalise(value: str) -> str:
    """`${{  env.RUST_TOOLCHAIN  }}` and `${{ env.RUST_TOOLCHAIN }}` are one
    expression; YAML lets the braces breathe and a string compare must not care.

    A trailing `# …` is a YAML comment and not part of the value. Dropping it
    here rather than in the regex keeps the *count* of `toolchain:` inputs
    honest — a commented step is still a step, and a guard that silently stopped
    counting one would be back to vetting less than it claims.
    """
    return re.sub(r"\s+", " ", re.sub(r"\s+#.*$", "", value.strip())).strip()


EXPECTED = "${{ env.RUST_TOOLCHAIN }}"
# `clippy 0.1.97` and `clippy 0.1.98 (48a229ceae 2026-09-01)` — the distro build
# carries no hash, so the minor is the only field both spellings share.
CLIPPY = re.compile(r"^clippy 0\.1\.(\d+)")


def ask(argv: list[str]) -> tuple[str | None, str]:
    """Run a version command and hand back its first line, or `None` and why not.

    An argument vector, never a command string (CLAUDE.md § security gate).
    A missing tool is a loud error and not a skip: the whole subject of this
    guard is a toolchain that is not what it should be, and *absent* is a case
    of that, not an exemption from it.
    """
    try:
        out = subprocess.run(argv, capture_output=True, text=True, timeout=120)
    except (OSError, subprocess.SubprocessError) as failure:
        return None, f"could not run `{' '.join(argv)}` ({failure})"
    if out.returncode != 0:
        return None, f"`{' '.join(argv)}` exited {out.returncode}"
    return out.stdout.strip(), ""


def installed() -> tuple[str | None, str]:
    """This machine's rustc version, having first checked clippy agrees with it.

    **clippy and not just rustc, because clippy is what broke.** `rustc` and
    `cargo clippy` normally ship as one toolchain, so this is usually a
    tautology — but a guard whose entire subject is *which clippy runs* and
    which never asks clippy anything is proven for a shape it was never fed
    (CLAUDE.md § A check is proven only for the input shapes it was fed).
    """
    line, why = ask(["rustc", "--version"])
    if line is None:
        return None, why
    # `rustc 1.97.1 (8bab26f4f 2026-07-14)` -> `1.97.1`
    fields = line.split()
    if len(fields) < 2 or not VERSION.match(fields[1]):
        return None, f"could not read a version out of `rustc --version`: {line!r}"
    version = fields[1]

    clippy, why = ask(["cargo", "clippy", "--version"])
    if clippy is None:
        return None, why
    found = CLIPPY.match(clippy)
    if not found:
        return None, f"could not read a version out of `cargo clippy --version`: {clippy!r}"
    # clippy 0.1.97 belongs to rustc 1.97.x. A mixed install — a distro rustc
    # with a rustup clippy, say — is exactly the drift this guard is about, one
    # level further in.
    if found.group(1) != version.split(".")[1]:
        return None, (f"rustc is {version} but `cargo clippy` is {clippy.split()[1]} — "
                      f"they are not from one toolchain, and clippy is the half that "
                      f"decides whether this gate can fail")
    return version, ""


def run(text: str, local: str | None, why: str = "") -> int:
    """The whole guard over one workflow's text and one local version.

    Pure on purpose: the self-test below feeds it every shape, including the
    ones no file in this repo has, and it needs no rustc and no workflow to do it.
    """
    pins = PIN.findall(text)
    # The canary. "The pin agrees with rustc" and "there is no pin and I compared
    # nothing" print the same line otherwise (CLAUDE.md § A derived list asserts
    # it found something) — so a renamed or deleted key lands here rather than
    # passing quietly.
    if len(pins) != 1:
        print(f"FAIL: the workflow declares RUST_TOOLCHAIN {len(pins)} time(s), expected 1 — "
              f"renamed, deleted, or written twice. This guard was about to vet nothing")
        return 1
    pinned = pins[0]

    if not VERSION.match(pinned):
        print(f"FAIL: RUST_TOOLCHAIN is {pinned!r}, which is a channel and not a version. "
              f"A channel moves under the pin — that is the drift this guard exists for, "
              f"and `stable` moving twice in one phase is what put it here. Write the "
              f"concrete version CI should install, e.g. 1.97.1")
        return 1

    uses = ACTION.findall(text)
    # The second canary, and the one that matters most: a workflow that stopped
    # using the action at all would satisfy every `toolchain:` clause below by
    # having none to check.
    if not uses:
        print("FAIL: no `uses: dtolnay/rust-toolchain@…` in the workflow — either the "
              "action was replaced and this guard has to move with it, or the read above "
              "found nothing and was about to vet nothing")
        return 1

    inputs = [normalise(v) for v in INPUT.findall(text)]
    if len(inputs) != len(uses):
        print(f"FAIL: {len(uses)} `dtolnay/rust-toolchain` step(s) but {len(inputs)} "
              f"`toolchain:` input(s) — a step with no explicit toolchain takes whatever "
              f"the action defaults to, which is the moving target this pin replaced")
        return 1
    stray = sorted({v for v in inputs if v != EXPECTED})
    if stray:
        print(f"FAIL: {len(stray)} `toolchain:` input(s) do not read the pin: "
              + " · ".join(repr(v) for v in stray))
        print(f"       every one of them must be exactly `{EXPECTED}`. One job left on a "
              f"channel reopens the drift for that job alone, which is harder to see than "
              f"the whole workflow being wrong")
        return 1

    print(f"toolchain-guard: CI pins rustc {pinned} · {len(uses)} step(s) read it")

    if local is None:
        print(f"FAIL: {why}")
        print("       a hard failure and not a skip — this guard's whole subject is a "
              "toolchain that is not the one it should be, and *absent* and *mismatched* "
              "are both cases of that rather than exemptions from it")
        return 1

    if local != pinned:
        print(f"FAIL: this machine runs rustc {local}, CI installs {pinned}.")
        print("       Every clippy lint CI can emit and yours cannot is a lint `just check` "
              "cannot fail on — which is how a phase-close PR went red on a lint that had "
              "been firing on every push for a week.")
        print(f"       Either match CI:  rustup toolchain install {pinned} && "
              f"rustup override set {pinned}")
        print(f"       No rustup? One time, no root needed:  curl --proto '=https' "
              f"--tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain {pinned}")
        print(f"       Or bump the pin in .github/workflows/ci.yml to {local} and re-run — "
              f"deliberately, reading whatever the newer clippy finds.")
        return 1

    print(f"OK — rustc {local} here is rustc {pinned} on CI")
    return 0


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""

    def workflow(pin='  RUST_TOOLCHAIN: "1.97.1"', first=EXPECTED, second=EXPECTED,
                 actions=2):
        step = ("      - uses: dtolnay/rust-toolchain@e97e2d8c\n"
                "        with:\n"
                "          toolchain: {}\n")
        body = "env:\n" + (pin + "\n" if pin else "") + "jobs:\n"
        for value in ([first, second][:actions]):
            body += step.format(value)
        return body

    def check(text, local, want, why, expect=None):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = run(text, local, why="rustc is missing")
        out = buf.getvalue()
        assert rc == want, f"{why}: expected {want}, got {rc}\n{out}"
        if expect:
            assert expect in out, f"{why}: {expect!r} not in\n{out}"
        return out

    # The happy path, and it must really pass — a guard that is red for
    # everything is the gate people learn to wave through.
    check(workflow(), "1.97.1", 0, "a pin every step reads, matching rustc", "OK —")
    # A patchless version is a real release spelling.
    check(workflow(pin='  RUST_TOOLCHAIN: "1.97"'), "1.97", 0, "a two-part version")
    # Unquoted, and with a trailing comment: both are YAML the file may hold.
    check(workflow(pin="  RUST_TOOLCHAIN: 1.97.1  # bump deliberately"), "1.97.1", 0,
          "an unquoted pin with a trailing comment")

    # A YAML comment after the value is not part of the value. Found by this
    # guard's own second pass: `normalise` used to keep it and go red for nothing.
    check(workflow(first=EXPECTED + "  # pinned, see env above"), "1.97.1", 0,
          "a trailing comment on a toolchain input")

    # THE FAILURE THIS GUARD WAS WRITTEN FOR: local behind CI.
    check(workflow(), "1.98.1", 1, "a machine one version behind CI", "cannot fail on")
    # …and ahead of it, which is the same lie pointing the other way.
    check(workflow(pin='  RUST_TOOLCHAIN: "1.98.1"'), "1.97.1", 1, "a machine ahead of CI",
          "cannot fail on")

    # The channel that started all this.
    check(workflow(pin='  RUST_TOOLCHAIN: "stable"'), "1.97.1", 1, "a channel as the pin",
          "channel and not a version")
    # One job quietly left behind — the drift that is hardest to see.
    check(workflow(second="stable"), "1.97.1", 1, "one step still on a channel",
          "do not read the pin")
    # A step that names no toolchain at all takes the action's default.
    check(workflow(actions=2).replace("          toolchain: ${{ env.RUST_TOOLCHAIN }}\n", "", 1),
          "1.97.1", 1, "a step with no toolchain input", "no explicit toolchain")

    # The canaries: each must fail as *vetted nothing*, never pass as agreement.
    check(workflow(pin=""), "1.97.1", 1, "no pin at all", "vet nothing")
    check(workflow(pin='  RUST_TOOLCHAIN: "1.97.1"\n  RUST_TOOLCHAIN: "1.98.1"'),
          "1.97.1", 1, "the pin written twice", "vet nothing")
    check("env:\n  RUST_TOOLCHAIN: \"1.97.1\"\njobs:\n", "1.97.1", 1,
          "a workflow that no longer uses the action", "vet nothing")
    check("", "1.97.1", 1, "an empty workflow", "vet nothing")
    # An unreadable rustc is a failure, not a skip.
    check(workflow(), None, 1, "a missing rustc", "hard failure and not a skip")

    # `only_workflow`: today ci.yml is alone, and the ceiling has to be seen red.
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        (d / "ci.yml").write_text(workflow())
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = only_workflow(d, "ci.yml")
        assert rc == 0, buf.getvalue()
        # A release workflow that names its own toolchain — Phase 13's shape.
        (d / "release.yml").write_text(
            "jobs:\n  x:\n    steps:\n"
            "      - uses: dtolnay/rust-toolchain@e97e2d8c\n"
            "        with:\n          toolchain: stable\n")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = only_workflow(d, "ci.yml")
        assert rc == 1 and "release.yml" in buf.getvalue(), buf.getvalue()
        assert "decided once" in buf.getvalue(), buf.getvalue()
        # A workflow that names no toolchain at all is fine — cargo-deny's shape.
        (d / "release.yml").unlink()
        (d / "docs.yml").write_text("jobs:\n  x:\n    steps:\n      - run: echo hi\n")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = only_workflow(d, "ci.yml")
        assert rc == 0, buf.getvalue()

    # The real workflow in this repo must pass the text half — the half that
    # needs no rustc. A self-test that only ever sees planted strings says
    # nothing about the file the guard actually reads.
    assert WORKFLOW.exists(), f"{WORKFLOW} is missing"
    text = WORKFLOW.read_text(encoding="utf-8")
    pins = PIN.findall(text)
    assert len(pins) == 1, f"the real ci.yml declares RUST_TOOLCHAIN {len(pins)} time(s)"
    check(text, pins[0], 0, "the real ci.yml against its own pin", "OK —")

    print("toolchain-guard: self-test passed — a machine behind or ahead of CI is refused, "
          "so is `stable` as the pin, a single step left on a channel, a step with no "
          "toolchain input, and a rustc that is missing or paired with a clippy from "
          "another toolchain; a two-part version, an unquoted pin and a "
          "trailing comment are not failures; and no pin, a doubled pin, a workflow that "
          "dropped the action and an empty file each fail as *vetted nothing* rather than "
          "passing as agreement; and a second workflow naming a toolchain of its own is refused "
          "while one naming none is not")


def only_workflow(folder: Path = None, keep: str = None) -> int:
    """Fail if any workflow but `ci.yml` decides a toolchain.

    `run` above reads one file, so a *second* workflow could set
    `toolchain: stable` and drift with nothing watching it — and Phase 13 adds a
    release workflow. It cannot simply read this one's `env`, either: workflows
    do not share one, so a second pin would be a second copy of the one value
    this guard exists to keep single.

    So the ceiling is made loud instead of guessed at. When that workflow lands
    this goes red, and whoever lands it decides where the pin lives — rather
    than a release built by an unpinned compiler nobody chose.
    """
    folder = folder or WORKFLOW.parent
    keep = keep or WORKFLOW.name
    others = sorted(p for p in folder.glob("*.y*ml") if p.name != keep) \
        if folder.is_dir() else []
    problems = []
    for other in others:
        text = other.read_text(encoding="utf-8")
        if ACTION.search(text) or INPUT.search(text):
            problems.append(f"{other.name} names a toolchain of its own")
    if problems:
        print("FAIL: " + " · ".join(problems))
        print(f"       the toolchain is decided once, in {keep}'s RUST_TOOLCHAIN, and "
              f"this guard only reads that file. A second workflow cannot share its `env:`, so "
              f"decide where the pin lives and teach this guard about it — do not leave a job "
              f"compiling with whatever `stable` is that day")
        return 1
    print(f"toolchain-guard: {len(others)} other workflow(s), none naming a toolchain")
    return 0


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

if not WORKFLOW.exists():
    print(f"FAIL: {WORKFLOW} does not exist — this guard was about to vet nothing")
    sys.exit(1)
local, why = installed()
sys.exit(max(run(WORKFLOW.read_text(encoding="utf-8"), local, why), only_workflow()))
