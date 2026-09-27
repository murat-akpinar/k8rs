#!/usr/bin/env python3
"""Fail if the read deadline is missing at the client, or outside kube's window.

Two halves, and the second is why this file reads kube's own sources.

A watch whose socket stays open and delivers nothing produces no error at all:
kube's own bound above the socket returns `None` rather than `Err`, so
`Store::troubles` is empty and the header says `live` over a cluster that stopped
arriving (NOTES § D297). The whole fix is one field — `Config::read_timeout`, set
by `k8s::bounded_reads` and applied at `connect_with`'s single `Client::try_from`.

**Why a guard and not a test.** `Client` never hands its `Config` back, so no test
can read the field off a built client: `k8s_tests.rs` asserts what `bounded_reads`
*returns*, which stays green if the call site drops the wrapper. `cargo mutants`
cannot see it either — it replaces function bodies and return values, not call
arguments, so `Client::try_from(config)` is a one-word regression with no red
anywhere in `just check`. That is the class CLAUDE.md § Tests must not lie names:
a test that cannot fail, where a text guard is the only mechanism left.

Product files only. Every `Client::try_from` under `src/*_tests.rs` is a sandbox
client over a loopback port the kernel assigned, and is deliberately unbounded —
one of them, `never_answers_within(None)`, exists precisely to prove that an
unbounded read never comes back.

**Two: the deadline is still inside kube's window.** `READ_TIMEOUT` only does
anything in the five seconds between the server's own close and kube's silent
reconnect: at or below the close a healthy quiet watch draws a fault that did not
happen, at or above the reconnect kube's idle arm wins every race and the field is
inert — which reads as a fix (NOTES § D297).

Both edges of that window are kube's, and **`k8s_tests.rs` can pin neither of
them**. `kubes_watch_window_is_still_the_one_read_timeout_was_chosen_inside` pins
what the public API exposes — the `timeoutSeconds` kube puts on the wire, and
`WatchParams::validate`'s refusal at 295 — but the ceiling that actually races
this field is computed in `kube-runtime`'s `next_with_idle_timeout` out of a
private `const WATCH_IDLE_TIMEOUT_MARGIN` and a second `unwrap_or(290)` of its
own, in a different crate from the one the test reads. A release that moved either
leaves every assertion in the suite green over an inert field.

So the window is **derived here instead**, out of the kube `cargo metadata`
resolved — the technique `write-guard.py` already uses to derive its ban list from
the kube actually in `Cargo.lock`, so a kube bump is red in the commit that bumps
it rather than silent until somebody wedges a watch.

Usage:
    read-deadline-guard.py             # src/*.rs, product files only
    read-deadline-guard.py <dir>       # some other tree, for a red/green demonstration
    read-deadline-guard.py --self-test # prove the guard fails when it should
"""
import contextlib
import importlib.util
import io
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Loaded by path because the filename has a hyphen, the way `handover-guard.py`
# reaches the same helper. It blanks comments to spaces rather than deleting
# them, so an offset in the stripped text is still a line number in the file —
# which matters here: `k8s.rs` names `Client::try_from` in its prose repeatedly
# and exactly one of those is code.
_spec = importlib.util.spec_from_file_location(
    "security_guard", ROOT / "scripts/security-guard.py")
_security_guard = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_security_guard)
strip_comments = _security_guard.strip_comments

# `ClientBuilder::try_from` is the second door into the same connector stack —
# `Config` in, `build()` out — so it is matched on the same terms.
BUILDS = re.compile(r"\b(?:kube::)?Client(?:Builder)?::try_from\s*\(\s*")
# The wrapper, immediately inside the call. Named rather than inferred: a rename
# is a red guard, which is the loud failure, not a hole.
WRAPPER = re.compile(r"bounded_reads\s*\(")


def sources(root: Path) -> list[Path]:
    """The product files, which is every `src/*.rs` that is not a test module."""
    return sorted(p for p in (root / "src").glob("*.rs")
                  if not p.name.endswith("_tests.rs"))


def at(text: str, offset: int) -> int:
    """The 1-based line number of an offset, so a failure names a line."""
    return text.count("\n", 0, offset) + 1


def check(root: Path) -> tuple[list[str], int]:
    """What is wrong, and how many client builds were vetted."""
    problems, found = [], 0
    for path in sources(root):
        text = path.read_text()
        for m in BUILDS.finditer(strip_comments(text)):
            found += 1
            if not WRAPPER.match(text, m.end()):
                problems.append(
                    f"{path.name}:{at(text, m.start())}  {m.group(0).strip()}…) is "
                    f"handed a Config that did not go through bounded_reads, so this "
                    f"client has kube's read_timeout of None and a watch that stays "
                    f"open delivering nothing is silent again (NOTES § D297)")
    if not found:
        problems.append(
            "no Client build was found in any product file — either the one at "
            "connect_with moved out of src/*.rs, or this guard was about to pass "
            "by vetting nothing (CLAUDE.md § A derived list asserts it found "
            "something)")
    return problems, found


# The window's three numbers, each where kube actually computes it. One match
# each, or this half vetted nothing and says so.
WINDOW = {
    "close": ("kube-core", "src/params.rs",
              re.compile(r'append_pair\(\s*"timeoutSeconds",\s*'
                         r'&self\.timeout\.unwrap_or\((\d+)\)'),
              "the timeoutSeconds kube asks the server to close the watch on"),
    "idle": ("kube-runtime", "src/watcher.rs",
             re.compile(r"Duration::from_secs\(u64::from\("
                        r"timeout\.unwrap_or\((\d+)\)\)\)"),
             "next_with_idle_timeout's own default, the base of kube's reconnect"),
    "margin": ("kube-runtime", "src/watcher.rs",
               re.compile(r"WATCH_IDLE_TIMEOUT_MARGIN:\s*Duration\s*=\s*"
                          r"Duration::from_secs\((\d+)\)"),
               "the private margin kube adds on top of it"),
}
# Either spelling of the path, so importing `Duration` is not a red build for
# nothing — the number is the subject, not how the type is reached.
DEADLINE = re.compile(r"READ_TIMEOUT:\s*(?:std::time::)?Duration\s*=\s*"
                      r"(?:std::time::)?Duration::from_secs\((\d+)\)")


def crates() -> dict[str, Path]:
    """Where cargo resolved kube's crates, so the numbers come from the real build."""
    try:
        out = subprocess.run(["cargo", "metadata", "--format-version", "1"],
                             cwd=ROOT, capture_output=True, text=True)
    except OSError as why:
        # A missing binary is a loud error and not a skipped step
        # (CLAUDE.md § Running it — and `just check`).
        sys.exit(f"read-deadline-guard: cargo did not run, and kube's own window "
                 f"cannot be read without it: {why}")
    if out.returncode != 0:
        sys.exit(f"read-deadline-guard: cargo metadata failed\n{out.stderr.strip()}")
    return {p["name"]: Path(p["manifest_path"]).parent
            for p in json.loads(out.stdout)["packages"]
            if p["name"] in ("kube-core", "kube-runtime")}


def window(root: Path) -> tuple[list[str], dict[str, int]]:
    """kube's three numbers and this repo's one, or what could not be read."""
    problems, read = [], {}
    resolved = crates()
    for name, (crate, where, pattern, why) in WINDOW.items():
        source = resolved.get(crate)
        at_path = source / where if source else None
        found = (pattern.findall(at_path.read_text())
                 if at_path and at_path.is_file() else [])
        if len(found) != 1:
            problems.append(
                f"{crate}/{where}: {len(found)} match(es) for {why} — kube moved it, "
                f"so READ_TIMEOUT's window cannot be derived and this guard was "
                f"about to pass without checking it (NOTES § D297)")
        else:
            read[name] = int(found[0])
    holds = root / "src/k8s.rs"
    ours = DEADLINE.findall(holds.read_text()) if holds.is_file() else []
    if len(ours) != 1:
        problems.append(
            f"src/k8s.rs: {len(ours)} match(es) for READ_TIMEOUT's definition — "
            f"either it is gone or it was respelled, and either way this guard "
            f"stopped reading the number it is about")
    else:
        read["ours"] = int(ours[0])
    if len(read) < 4:
        return problems, read

    ceiling = read["idle"] + read["margin"]
    if read["ours"] <= read["close"]:
        problems.append(
            f"READ_TIMEOUT is {read['ours']}s and the server closes a healthy watch "
            f"at {read['close']}s — an ordinary quiet watch would report a failure "
            f"that did not happen (NOTES § D297)")
    if read["ours"] >= ceiling:
        problems.append(
            f"READ_TIMEOUT is {read['ours']}s and kube reconnects a silent watch at "
            f"{read['idle']}+{read['margin']}={ceiling}s — the idle arm wins every "
            f"race and the field is inert, which reads as a fix (NOTES § D297)")
    return problems, read


def run(root: Path) -> list[str]:
    problems, found = check(root)
    edges, read = window(root)
    problems += edges
    for p in problems:
        print(f"FAIL {p}")
    seen = " ".join(f"{k}={v}s" for k, v in read.items())
    print(f"read-deadline-guard: {found} client build(s) vetted in "
          f"{len(sources(root))} product file(s); window {seen} — "
          f"{'OK' if not problems else f'{len(problems)} problem(s)'}")
    return problems


def self_test() -> None:
    """Every failure red on a planted copy of the real file, and the real tree green."""
    real = (ROOT / "src/k8s.rs").read_text()

    def verdict(text: str | None) -> list[str]:
        """`None` plants no k8s.rs at all — the empty-read case."""
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "src").mkdir()
            if text is not None:
                (Path(tmp) / "src/k8s.rs").write_text(text)
            with contextlib.redirect_stdout(io.StringIO()):
                return check(Path(tmp))[0]

    assert not verdict(real), f"the real file is not green: {verdict(real)}"
    assert not run(ROOT), "the real tree is not green"

    planted = {}
    # The regression itself: the wrapper dropped at the call site.
    planted["the wrapper dropped"] = verdict(
        real.replace("Client::try_from(bounded_reads(config))",
                     "Client::try_from(config)", 1))
    assert any("bounded_reads" in p for p in planted["the wrapper dropped"]), \
        planted["the wrapper dropped"]

    # A second client added beside the first, unbounded — the shape a later box
    # grows and no test reaches.
    planted["a second unbounded client"] = verdict(
        real + "\nfn second() { let _ = kube::Client::try_from(Config::new(u)); }\n")
    assert len(planted["a second unbounded client"]) == 1, \
        planted["a second unbounded client"]

    # The other door into the same connector stack.
    planted["ClientBuilder"] = verdict(
        real + "\nfn third() { let _ = ClientBuilder::try_from(cfg).build(); }\n")
    assert planted["ClientBuilder"], planted["ClientBuilder"]

    # The canary: a tree with no client build at all must fail rather than
    # report OK over nothing.
    planted["nothing to vet"] = verdict(None)
    assert any("vetting nothing" in p for p in planted["nothing to vet"]), \
        planted["nothing to vet"]

    # The negative, and the whole reason `strip_comments` is here: `k8s.rs` names
    # this call in its prose repeatedly, none of them with an argument list.
    # One that grows one must not be read as a second unbounded client.
    prose = "/// A second client would be `Client::try_from(config)` and is not built.\n"
    assert not verdict(prose + real), (
        f"a doc comment naming Client::try_from(config) was refused as code — the "
        f"comment strip is not doing its job: {verdict(prose + real)}")
    assert len(BUILDS.findall(prose + real)) == 2, \
        "the planted doc mention did not match the pattern, so it proved nothing"
    assert len(BUILDS.findall(strip_comments(prose + real))) == 1, \
        "the comment strip left the planted doc mention in the code"

    # The window half. kube's three numbers come from the real resolved crates —
    # there is nothing to plant there that would still be kube — so what is
    # planted is this repo's own end of the comparison.
    def edges(text: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "src").mkdir()
            (Path(tmp) / "src/k8s.rs").write_text(text)
            with contextlib.redirect_stdout(io.StringIO()):
                return window(Path(tmp))[0]

    assert not edges(real), f"the real READ_TIMEOUT is refused: {edges(real)}"
    for seconds, expect in ((290, "did not happen"), (289, "did not happen"),
                            (295, "inert"), (600, "inert")):
        moved = real.replace("std::time::Duration::from_secs(292)",
                             f"std::time::Duration::from_secs({seconds})", 1)
        planted[f"READ_TIMEOUT at {seconds}s"] = edges(moved)
        assert any(expect in p for p in planted[f"READ_TIMEOUT at {seconds}s"]), \
            planted[f"READ_TIMEOUT at {seconds}s"]

    planted["READ_TIMEOUT respelled"] = edges(
        real.replace("pub(crate) const READ_TIMEOUT", "pub(crate) const READ_DEADLINE", 1))
    assert any("stopped reading the number" in p
               for p in planted["READ_TIMEOUT respelled"]), \
        planted["READ_TIMEOUT respelled"]

    for name, problems in planted.items():
        print(f"read-deadline-guard --self-test: {name} — refused: {problems[0]}")
    print(f"read-deadline-guard --self-test: {len(planted)} plant(s) refused, "
          f"a doc comment naming the call correctly ignored, tree green — OK")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    else:
        where = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT
        sys.exit(1 if run(where) else 0)
