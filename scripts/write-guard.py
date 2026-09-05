#!/usr/bin/env python3
"""Enforce invariant 1: mutations live in `src/ops.rs` and nowhere else.

**The matching is clippy's.** `clippy.toml`'s `disallowed-methods` resolves
paths, so it tells `kube::Api::replace` from `str::replace`; with `-D warnings`
it is the containment. This script owns the two halves clippy cannot do.

**One: the ban list is still complete** against the kube actually in
`Cargo.lock` — the derivation below, and most of this file.

**Two: the exception to it is still singular.** An allowed lint never fires, so
an `#[allow(clippy::disallowed_methods)]` in some other file is a hole neither
clippy nor half one can report — there is nothing to report *about*. Invariant 1
puts exactly one such attribute in the tree, in `src/ops.rs`; § the single
exception proves no second one appeared, in any of the shapes clippy actually
accepts, including the three that never spell the lint out
(`clippy::style`, `clippy::all`, `warnings`) and the ones that are not in a
`.rs` file at all — `Cargo.toml`, `.cargo/config{,.toml}`, and a `-A` on a rustc
command line in the `justfile`, a workflow or a `scripts/` shell file
(todo.md, Phase 7).

**Ceilings of half two, each measured 2026-09-03 and not argued.** It does not
parse Rust, so an attribute quoted in a doc comment or a string literal is
counted — fail-closed on purpose: a naive `//.*$` strip turns
`let s = "http://x"; #[allow(…)]` into `let s = "http:` and loses a *real*
attribute, and a false positive is loud where a hole is not. Both failure
messages say so, because `src/ops.rs` is the one file whose prose is *about*
the attribute. What is left outside the repo after `flag_holes` is two things,
both watched silence a firing lint: `RUSTFLAGS` in the ambient environment, and
a `.cargo/config.toml` in a parent directory of the checkout (so also
`$CARGO_HOME`). No file this repo owns can rule those out.

Written as an **allowlist**, deliberately. Outside `ops.rs` only these kube
methods may appear, on any type:

    get*  ·  list*  ·  watch*  ·  logs  ·  log_stream  ·  apiserver_version

A denylist would have to know about `delete`, `patch`, `replace`, `create`,
`cordon`, `uncordon`, `restart`, `evict`, `exec`, `attach`, `portforward`,
`entry`, `patch_scale` — and about whatever kube-rs adds next release, which
is exactly the thing nobody will remember to update. So the ban list is
*derived*: every `&self` method of **`Api<K>` and `kube_core::Request`** in the
kube version actually in `Cargo.lock`, minus the allowlist above. `clippy.toml`
must name exactly that set, so a kube bump that adds a method is red in the
commit that bumps it.

The second type arrived on 2026-08-22: a `Request` built by hand and posted
through `Client::request` was a complete DELETE that this guard raised nothing
about, because the list came from `impl Api<K>` alone (NOTES § D142). The
allowlist above did not have to widen by one word to absorb it.

Until 2026-08-22 this script grepped every root cargo compiles — `src/`,
`tests/`, `examples/`, `benches/`, `build.rs` — for a bare `.name(` instead. The
receiver's type is not in the text it read, so `HashMap::entry` and
`str::replace` were indistinguishable from kube writes — five findings, all
false, zero true, on the first day kube was a dependency (NOTES § D141).

Usage:
    write-guard.py             # check clippy.toml against the kube in Cargo.lock
    write-guard.py --self-test # prove the guard fails when it should
"""

import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CLIPPY = "clippy.toml"

ALLOWED_PREFIXES = ("get", "list", "watch")
ALLOWED_EXACT = {"logs", "log_stream", "apiserver_version"}

# The types this guard derives from, with the path clippy resolves each by.
# Not every type that can reach the API server — see `Client` below.
#
# Each spelling was proven by making a call fire, never read off docs. The
# alternatives resolve to the same def-ids and were watched fire too
# (`kube::api::Api::replace`, `kube_core::request::Request::replace`), so one
# spelling per type is fixed only so the comparison can be a string set.
#
# `Request` is the second because the ban list was derived from `impl Api<K>`
# alone and a `Request` built by hand, posted through `Client::request`, was a
# complete DELETE that raised nothing (NOTES § D142). **The allowlist did not
# have to widen to absorb it**: `Request`'s readers are `list`, `watch`, `get`,
# `get_subresource`, `get_metadata`, `list_metadata`, `watch_metadata` and
# `logs`, and every one already matches `get*` / `list*` / `watch*` / `logs`.
#
# `Client` is deliberately absent. `Client::request` is verb-agnostic — the verb
# is *data* inside the request object — and Phase 5 needs it outside `ops.rs` for
# a read. No method-name ban can close that; invariant 2 is what stands there.
API = "kube::Api::"
REQ = "kube::core::Request::"
TYPES = {
    API: re.compile(r"^[ \t]*impl(?:<[^>]*>)?\s+Api<", re.M),
    # Anchored to the inherent block: `impl<S, B> Service<Request<B>> for X` in
    # kube-client is `http::Request` and must not be read as this one. The brace
    # is a lookahead, not a match — `impl_body` starts from the *next* `{` it
    # finds, so consuming this one would hand it a method body instead.
    REQ: re.compile(r"^[ \t]*impl\s+Request\s*(?=\{)", re.M),
}
# What makes those spellings real, checked rather than assumed. Measured
# 2026-08-22 (NOTES § D141): clippy warns about a `disallowed-methods` path it
# cannot resolve, but that warning is **not** promoted by `-D warnings`, and it
# is not emitted at all by a crate that does not link kube. So a `clippy.toml`
# whose every path resolved to nothing would leave CI green.
#
# Every hop of each chain, because a path dies at whichever hop is dropped:
# `kube::core::Request` is `kube` re-exporting `kube_core`, then `kube_core`
# re-exporting `request::Request`. A proxy for compiling a call — it catches a
# kube reorganisation, which is the way a spelling realistically dies.
RE_EXPORTS = {
    API: [("kube", re.compile(r"^\s*pub use api::Api\s*;", re.M))],
    REQ: [
        ("kube", re.compile(r"^\s*#\[doc\(inline\)\]\s*pub use kube_core as core\s*;", re.M)),
        ("kube-core", re.compile(r"^\s*pub use request::Request\s*;", re.M)),
    ],
}

# Methods behind a cargo feature we do not enable. This script reads source
# text, so it extracts them; the crate clippy compiles does not have them, and
# their entries resolve to nothing. They stay listed so that turning the feature
# on needs no one to remember, and they carry `allow-invalid` so the
# unresolved-path warning has a **silent baseline** — measured, and a real
# unresolved path reads exactly like them (NOTES § D141).
#
# Pinned rather than derived because clippy's own help text offers
# `allow-invalid = true` as *the* fix for that warning, so the plausible next
# edit is someone silencing a genuine hole with it. Every member below was
# confirmed unresolvable by clippy, not by reading a `#[cfg]`.
#
# Today that is exactly kube's `ws` feature, which D140 leaves off, on both
# types. `kube-core`'s `kubelet-debug` feature is *also* off and also parsed,
# but its four `kubelet_node_*` functions take no `&self` — they are associated
# functions, so `derive` never yields them and they need no entry here. That was
# checked against the source rather than assumed from the `#[cfg]`.
#
# `exec` is also a CANARY below, and the two do not conflict: a canary proves
# the *parser* still reads a signature shape out of kube's source, which is true
# whether or not the feature that compiles it is on.
FEATURE_GATED = {
    t + m for t in (API, REQ) for m in ("attach", "exec", "portforward")
}

# The ban list is *derived*, so the failure that matters is under-extraction:
# a signature the parser did not recognise — a `&self` wrapped onto its own
# line — drops a mutation off the list silently. (A cfg'd impl block is the
# *other* direction: this reads source text, so it extracts methods the compiled
# crate does not have. That is FEATURE_GATED above, and it is loud rather than
# silent, which is why it is the safe direction to be wrong in.) And now
# that `clippy.toml` is compared against that list rather than against the code,
# a short list is matched by a short `clippy.toml` and **both go green over the
# hole**. These have existed on `Api<K>` for the crate's whole life; if they are
# missing, the parser broke, not kube-rs.
#
# The first four are the obvious mutations. The last three are here because each
# is written in a *shape* the old line-anchored parser could not read, and all
# three were silently absent from the ban list while the four above were
# present — so the canary set passed while 17 of the 45 `Api` methods kube had
# at the time, including every `patch*`, went uncontained. One canary per signature form, not per method:
#   delete_collection — receiver wrapped onto its own line
#   exec              — generic list between the name and the paren
#   entry             — lifetime on the receiver (`&'a self`)
#
# Per type, because the under-extraction failure is now two parsers deep: a
# `Request` regex that matched nothing would leave that whole type unbanned
# while the `Api` half still looked healthy.
CANARIES = {
    API: {"delete", "patch", "replace", "create",
          "delete_collection", "exec", "entry"},
    # Request's four obvious mutations, and not merely "is the regex finding
    # the type": `delete_collection`, `patch` and `replace` wrap their receiver
    # onto its own line here exactly as `Api`'s do, so `patch` and `replace`
    # carry the same shape canary on this type. `Request` has no `entry`.
    REQ: {"delete", "patch", "replace", "create"},
}
# Matched across the joined impl body, never line by line. Real kube signatures
# wrap their receiver (`delete_collection(\n  &self,`), carry a generic list
# between the name and the paren (`exec<I, T>(&self`), and put a lifetime on the
# receiver itself (`entry<'a>(&'a self`). A line-anchored regex reads all three
# as ordinary text — which is how every `patch*` method, `delete_collection`,
# `exec` and `entry` stayed off the ban list.
METHOD = re.compile(
    r"\bpub\s+(?:async\s+)?fn\s+(?P<name>\w+)\s*(?:<[^()]*?>)?\s*"
    r"\(\s*&\s*(?:'\w+\s+)?(?:mut\s+)?self",
    re.S,
)


def allowed(name: str) -> bool:
    return name in ALLOWED_EXACT or name.startswith(ALLOWED_PREFIXES)


# The allowlist is three bare prefixes, so any future kube method whose name
# begins with one is exempted the day it appears, silently and forever. Nothing
# in kube today trips it — which is exactly when to write the assertion.
assert not [c for s in CANARIES.values() for c in s if allowed(c)], \
    "the allowlist exempts a known mutation"


def derive(sources: list[Path]) -> dict[str, set[str]]:
    """Every `&self` method of every type in TYPES, keyed by that type's prefix.

    `&self` is what excludes a constructor: `Api::namespaced` and `Request::new`
    take no receiver and never appear here. That matters for `Request::new`,
    which Phase 5's hand-built Table request may want on a *read* path — the
    parser leaves it out, so the allowlist never has to argue about it
    (NOTES § D142).
    """
    found: dict[str, set[str]] = {prefix: set() for prefix in TYPES}
    for path in sources:
        text = path.read_text(encoding="utf-8", errors="replace")
        for prefix, impl_re in TYPES.items():
            for m in impl_re.finditer(text):
                found[prefix].update(
                    mm.group("name") for mm in METHOD.finditer(impl_body(text, m.end()))
                )
    return found


def impl_body(text: str, start: int) -> str:
    """The text between the impl block's braces, matched rather than guessed.

    The old parser ended a block at the first line starting with `}`, which is
    also how a wrapped signature's own closing brace looks.
    """
    open_at = text.find("{", start)
    if open_at < 0:
        return ""
    depth = 0
    for i in range(open_at, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[open_at + 1 : i]
    return text[open_at + 1 :]


def kube_packages() -> dict[str, Path]:
    """The kube crates' source roots, as cargo resolved them.

    `kube-core` is here because `Request` lives in it and a write does not have
    to go through `Api<K>` (NOTES § D142).
    """
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if out.returncode != 0:
        sys.exit(f"write-guard: cargo metadata failed\n{out.stderr.strip()}")
    meta = json.loads(out.stdout)
    return {
        p["name"]: Path(p["manifest_path"]).parent
        for p in meta["packages"]
        if p["name"] in ("kube-client", "kube-core", "kube")
    }


def kube_sources(roots: dict[str, Path]) -> list[Path]:
    return [f for root in roots.values() for f in root.rglob("*.rs")]


def dead_prefixes(roots: dict[str, Path]) -> list[str]:
    """Prefixes whose re-export chain no longer holds — see RE_EXPORTS."""
    dead = []
    for prefix, hops in RE_EXPORTS.items():
        for package, pattern in hops:
            root = roots.get(package)
            lib = root / "src" / "lib.rs" if root else None
            if lib is None or not lib.is_file() or not pattern.search(
                lib.read_text(encoding="utf-8")
            ):
                dead.append(f"{prefix}… — {package} no longer carries the re-export it needs")
                break
    return dead


def wanted(found: dict[str, set[str]]) -> set[str]:
    """The exact `disallowed-methods` list `clippy.toml` has to carry.

    One allowlist over both types, unwidened: `Request`'s readers are `list`,
    `watch`, `get`, `get_subresource`, `get_metadata`, `list_metadata`,
    `watch_metadata` and `logs`, so `get*`/`list*`/`watch*`/`logs` already
    partitions the second type correctly (NOTES § D142).
    """
    return {
        prefix + m
        for prefix, methods in found.items()
        for m in methods
        if not allowed(m)
    }


def listed(path: Path) -> set[str]:
    """`clippy.toml`'s `disallowed-methods`, in either of the forms it allows.

    An entry may be a bare path or `{ path = "…", reason = "…" }`. Reading only
    the first form would turn a reformat into a false *pass* of the shorter
    list, which is the one direction this guard may never fail in.
    """
    cfg = tomllib.loads(path.read_text(encoding="utf-8"))
    out = set()
    for entry in cfg.get("disallowed-methods", []):
        out.add(entry if isinstance(entry, str) else entry.get("path", ""))
    return out


def silenced(path: Path) -> set[str]:
    """The entries carrying `allow-invalid` — see FEATURE_GATED for why it is pinned."""
    cfg = tomllib.loads(path.read_text(encoding="utf-8"))
    return {
        e["path"]
        for e in cfg.get("disallowed-methods", [])
        if not isinstance(e, str) and e.get("allow-invalid")
    }


def drift(want: set[str], got: set[str]) -> list[str]:
    """Both directions. A missing entry is a hole; an extra one is a lie."""
    return (
        [f"{p} — kube has this method and {CLIPPY} does not ban it. "
         f"Nothing stops a call to it outside src/ops.rs (invariant 1)."
         for p in sorted(want - got)]
        + [f"{p} — {CLIPPY} bans this and the kube in Cargo.lock has no such "
           f"method on that type. Either kube removed it, or the path is "
           f"misspelled — "
           f"and a path clippy cannot resolve bans nothing while looking like "
           f"it does (NOTES § D141)."
           for p in sorted(got - want)]
    )


def hatch_drift(got: set[str]) -> list[str]:
    """`allow-invalid` may cover feature-gated methods and nothing else.

    A function and not a block inside `main`, because `--self-test` cannot reach
    `main`. It was written there first and this file's own second pass moved it.
    """
    want = FEATURE_GATED
    return (
        [f"{p} carries `allow-invalid`, which silences the one warning clippy "
         f"gives when a path resolves to nothing. Only a method behind a cargo "
         f"feature we do not build may (see FEATURE_GATED) — if this was added "
         f"to quiet a warning, the "
         f"warning was the finding (NOTES § D141)."
         for p in sorted(got - want)]
        + [f"{p} is behind a cargo feature we do not build and needs "
           f"`allow-invalid`, or its unresolved-path warning becomes the "
           f"baseline noise a real one would hide in."
           for p in sorted(want - got)]
    )


# --- the single exception to the ban list START ---
# `clippy.toml` is the containment; an `#[allow]` is the hole in it, and neither
# clippy nor the drift check above can see one — an allowed lint never fires, so
# there is nothing for either to report. Invariant 1 puts exactly one such
# attribute in the tree, in `src/ops.rs`, and this half proves it is still the
# only one (todo.md, Phase 7: "CI's containment check now expects exactly this
# file"). Until it existed, the exception could have been added to any file in
# the crate and the build would have stayed green.
EXCEPTION = "src/ops.rs"
LINT = "clippy::disallowed-methods"

# `warnings` is not a member of anything — rustc's own group table describes it
# as "all lints that are set to issue warnings" and prints no sub-lint list — so
# the derivation below cannot reach it. `disallowed-methods` defaults to `warn`,
# and `#![allow(warnings)]` was watched silence it *under* `-D warnings` on
# 2026-09-03: a crate attribute beats the command-line flag.
ALWAYS = {"warnings"}

# What the derivation has to find, or it parsed nothing and is about to vet
# nothing (CLAUDE.md § A derived list asserts it found something). Measured the
# same day off `clippy-driver -W help`: `disallowed-methods` is a member of
# `clippy::style`, which is a member of `clippy::all`. Both were then confirmed
# against a compiling crate — `#![allow(clippy::style)]` and
# `#![allow(clippy::all)]` each turned a firing lint green.
GROUP_CANARIES = {"clippy::all", "clippy::style"}

# One group per line: the name, then whitespace, then its members. The second
# field is anchored to `clippy::` so the *lint* table printed above the group
# table — whose second field is a level word (`warn`, `allow`) — cannot be read
# as a group with one member called `warn`.
GROUP_LINE = re.compile(r"^\s*(clippy::[a-z-]+)\s\s+(clippy::[a-z-]+.*)$", re.M)

ATTR = re.compile(r"#!?\[")
# Run over text whose `::` has been un-spaced first, so this matches
# `clippy :: disallowed_methods` — valid Rust that a grep for the literal
# attribute string misses.
TOKEN = re.compile(r"[a-z_][a-z_0-9]*(?:::[a-z_][a-z_0-9]*)*")

# The roots cargo compiles under `--all-targets`, which is the list
# `security-guard.py` scans for the same reason. `src/*_tests.rs` and
# `src/*_tests/` need no entry of their own: invariant 11 makes them `#[path]`
# child modules of this crate, so `src/**` already holds them — and an inner
# `#![allow]` in one silences the lint for that module, measured 2026-09-03
# rather than reasoned from the module system.
ROOTS = ("src", "tests", "examples", "benches")


def rust_sources(root: Path) -> list[Path]:
    """Every .rs file cargo compiles, plus `build.rs` if there is one."""
    return sorted(
        p for r in ROOTS for p in (root / r).rglob("*.rs")
    ) + [p for p in [root / "build.rs"] if p.is_file()]


def silencing_lints(help_text: str) -> set[str]:
    """Every lint name whose `allow` turns `disallowed-methods` off.

    Derived from clippy rather than pinned, for the same reason the ban list is
    derived from kube: which group a lint belongs to is clippy's to change, and
    a pinned set goes stale in the one direction that opens a hole. Names are
    returned in the underscore spelling, because that is the only one an
    attribute can carry — `#![allow(clippy::disallowed-methods)]` does not parse
    at all (measured: "expected one of `(`, `,`, `::`, or `=`, found `-`").
    """
    found = {LINT} | ALWAYS
    for name, members in GROUP_LINE.findall(help_text):
        if LINT in [m.strip() for m in members.split(",")]:
            found.add(name)
    return {n.replace("-", "_") for n in found}


def clippy_help() -> str:
    """`clippy-driver -W help`, which prints the group table this derives from.

    `clippy-driver` and not `cargo clippy -- -W help`: the second compiles the
    crate first, and this needs a table, not a build. It ships with the same
    rustup component CI already installs, so a missing binary is a loud error
    rather than a missing step (CLAUDE.md § `just check` is the whole of CI).
    """
    out = subprocess.run(["clippy-driver", "-W", "help"], cwd=ROOT,
                         capture_output=True, text=True)
    if out.returncode != 0:
        sys.exit(f"write-guard: clippy-driver -W help failed, so the set of lints "
                 f"whose `allow` silences {LINT} cannot be derived\n{out.stderr.strip()}")
    return out.stdout + out.stderr


def attributes(text: str) -> list[str]:
    """Every `#[…]` / `#![…]` body, bracket-balanced.

    Not `#\\[[^]]*\\]`: an attribute carrying a `]` before its `allow` would be
    truncated into two harmless halves, and truncation is the direction this
    guard may not be wrong in. Deliberately not line-anchored either — a whole
    `fn f() { #[allow(…)] let x = …; }` on one line is a real silencer.
    """
    out = []
    for m in ATTR.finditer(text):
        depth = 0
        for i in range(m.end() - 1, len(text)):
            if text[i] == "[":
                depth += 1
            elif text[i] == "]":
                depth -= 1
                if depth == 0:
                    out.append(text[m.end():i])
                    break
    return out


def silences(attr: str, lints: set[str]) -> bool:
    """True if this attribute body turns `disallowed_methods` off.

    Both halves are required, which is what keeps `#[deny(clippy::all)]` and
    `#[allow(unused)]` out. `cfg_attr` needs no case of its own: its wrapped
    `allow(…)` is inside the same body.
    """
    names = set(TOKEN.findall(re.sub(r"\s*::\s*", "::", attr)))
    return bool(names & {"allow", "expect"}) and bool(names & lints)


def carriers(root: Path, lints: set[str]) -> dict[str, int]:
    """How many silencing attributes each file carries, files with none omitted."""
    out = {}
    for path in rust_sources(root):
        n = sum(1 for a in attributes(path.read_text(encoding="utf-8", errors="replace"))
                if silences(a, lints))
        if n:
            out[path.relative_to(root).as_posix()] = n
    return out


def exception_drift(got: dict[str, int]) -> list[str]:
    """Exactly one file, carrying exactly one attribute. Empty is a failure too."""
    bad = [
        f"{p} silences {LINT}. Invariant 1 allows that in {EXCEPTION} and nowhere "
        f"else — a call banned crate-wide is not banned in a file that turns the "
        f"lint off, and clippy reports nothing about a lint it was told to allow. "
        f"If the attribute was aimed at some other lint, it named a group that "
        f"contains this one (`clippy::all`, `clippy::style`, `warnings`): narrow "
        f"it to the lint you meant."
        for p in sorted(got) if p != EXCEPTION
    ]
    if EXCEPTION not in got:
        bad.append(
            f"no file silences {LINT}, so {EXCEPTION}'s single "
            f"`#![allow(clippy::disallowed_methods)]` is gone. Either the write "
            f"path lost the one line that announces the exception, or this check "
            f"stopped finding attributes and was about to vet nothing. This check "
            f"does not parse Rust: it would also have counted the attribute "
            f"quoted in a comment or a string literal, so neither is there "
            f"either."
        )
    elif got[EXCEPTION] > 1:
        bad.append(
            f"{EXCEPTION} carries {got[EXCEPTION]} silencing attributes. There is "
            f"one visible exception to audit, not a scattering of them "
            f"(NOTES § Operations, \"writes live in exactly one file\"). **Before "
            f"reading that as a second exception smuggled in:** this check does "
            f"not parse Rust, so an `#[allow(…)]` quoted inside a doc comment or "
            f"a string literal counts too — and this is the one file whose prose "
            f"is *about* the attribute. If that is what happened, reword the "
            f"prose; do not delete the attribute."
        )
    return bad


# Committed files that put flags on rustc's own command line. `-D warnings` is
# set job-wide in the first two — justfile:14 and ci.yml:17 — and justfile:10-13
# says why: setting it only on the clippy line left warnings invisible locally.
# That makes those exact lines where someone goes when a lint turns noisy, and a
# specific-lint `-A` beats a group-level `-D` later on the same rustc line
# (measured by k8s-admin, 2026-09-03: `-D warnings -A clippy::disallowed_methods`
# with `Api::delete` in `src/k8s.rs` leaves both this guard and `cargo clippy`
# at exit 0). `scripts/*.sh` is in the list because `guards.sh` is the file CI
# runs this guard *from*.
FLAG_FILES = ("justfile", ".github/workflows/*.y*ml", "scripts/*.sh")

# `-Aclippy::all`, `-A clippy::all`, `--allow clippy::all`, `--allow=clippy::all`.
# Gated on the lint name and not on the flag, which is what keeps `declare -A
# want` and `kubectl get pods -A -o json` out — both are all over `scripts/` and
# the justfile, and a guard red for those is one people learn to wave through.
# The hyphen spelling *is* live here, unlike in an attribute, so the captured
# name is normalised before it is looked up.
FLAG = re.compile(r"(?<![A-Za-z0-9])(?:-A|--allow)[=\s]*([A-Za-z][A-Za-z_:-]*)")


def flag_holes(root: Path, lints: set[str]) -> list[str]:
    """An `-A` on a committed rustc command line — the off-switch for all of this.

    Its own ceiling is the module docstring's: what is left after this is the
    ambient environment and a `.cargo/config.toml` above the checkout, neither
    of which is a file this repo owns.
    """
    bad = []
    for pattern in FLAG_FILES:
        for path in sorted(root.glob(pattern)):
            for name in FLAG.findall(path.read_text(encoding="utf-8", errors="replace")):
                if name.replace("-", "_") in lints:
                    bad.append(
                        f"{path.relative_to(root).as_posix()} names {name} in an "
                        f"`-A` / `--allow` flag to rustc. That silences {LINT} "
                        f"for every build the file "
                        f"drives — including the one CI runs this guard from — "
                        f"with nothing in src/, Cargo.toml or .cargo/ changed. A "
                        f"specific-lint `-A` beats the `-D warnings` beside it."
                    )
    return bad


def manifest_holes(root: Path) -> list[str]:
    """Lint levels set outside a `#[…]` attribute — the two places nobody greps.

    Measured 2026-09-03: a `[lints.clippy] disallowed_methods = "allow"` in
    `Cargo.toml` and a `-A` in `.cargo/config.toml`'s `[build] rustflags` each
    silence the containment with the word `allow` nowhere in any `.rs` file.

    The other committed off-switch — a `-A` on a rustc command line in the
    justfile or a workflow — is `flag_holes` above. This one is the manifest.
    """
    bad = []
    cfg = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    tables = [("lints", cfg.get("lints", {}))]
    tables += [("workspace.lints", cfg.get("workspace", {}).get("lints", {}))]
    for where, table in tables:
        for tool, lints in table.items():
            if not isinstance(lints, dict):
                continue
            for lint, level in lints.items():
                lvl = level if isinstance(level, str) else level.get("level", "")
                if lvl == "allow":
                    bad.append(
                        f"Cargo.toml [{where}.{tool}] sets {lint} = \"allow\". Lint "
                        f"levels are not configured here — clippy.toml plus the one "
                        f"attribute in {EXCEPTION} is the whole of invariant 1, and "
                        f"a manifest entry silences it with no `.rs` file changed."
                    )
    # Both names: cargo still reads the extensionless `.cargo/config`, and a
    # guard that only knew the modern spelling would be walked past by the
    # deprecated one. Matched as text rather than parsed, because `rustflags`
    # has several table paths (`build`, every `target.<cfg>`) and this file has
    # no business carrying any of them.
    for name in ("config.toml", "config"):
        conf = root / ".cargo" / name
        if conf.is_file() and "rustflags" in conf.read_text(encoding="utf-8"):
            bad.append(
                f".cargo/{name} sets rustflags. A `-A` there silences the "
                f"containment for every build in this checkout, including CI's, "
                f"with nothing in src/ to see. Lint levels belong in clippy.toml."
            )
    return bad
# --- the single exception to the ban list END ---


def render(want: set[str]) -> str:
    """The `disallowed-methods` body, in the exact form this guard accepts.

    The failure path printed bare strings for every entry until this existed,
    so pasting what the guard told you to paste produced a file it then rejected
    for the missing `allow-invalid` — a gate that cannot be passed by following
    its own instructions. The self-test round-trips this back through `listed`
    and `silenced` for that reason.
    """
    return "".join(
        f'    {{ path = "{p}", allow-invalid = true }},\n'
        if p in FEATURE_GATED else f'    "{p}",\n'
        for p in sorted(want)
    )


def self_test() -> None:
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        fake = Path(tmp)
        (fake / "api.rs").write_text(
            "impl<K> Api<K> {\n"
            "    pub fn namespaced(client: Client, ns: &str) -> Self { todo!() }\n"
            "    pub async fn get(&self, name: &str) -> Result<K> { todo!() }\n"
            "    pub async fn delete(&self, name: &str) -> Result<K> { todo!() }\n"
            "    pub async fn patch_scale(&mut self, name: &str) -> Result<K> { todo!() }\n"
            "}\n"
            "impl Client {\n"
            "    pub async fn request(&self, r: Request) -> Result<()> { todo!() }\n"
            "}\n"
            "impl Request {\n"
            "    pub fn new<S: Into<String>>(url_path: S) -> Self { todo!() }\n"
            "    pub fn get(&self, name: &str) -> Result<R> { todo!() }\n"
            "    pub fn delete(&self, name: &str) -> Result<R> { todo!() }\n"
            "}\n"
            # kube-client's own generics over `http::Request`. A regex that read
            # these as the inherent block would ban `Service::call` and friends.
            "impl<S, B> Service<Request<B>> for BaseUri<S> {\n"
            "    pub fn call(&self, req: Request<B>) -> S::Future { todo!() }\n"
            "}\n"
            "impl<B> AsyncPredicate<Request<B>> for RefreshableToken {\n"
            "    pub fn check(&self, r: Request<B>) -> Self::Future { todo!() }\n"
            "}\n"
        )
        found = derive([fake / "api.rs"])
        assert found[API] == {"get", "delete", "patch_scale"}, found[API]
        assert found[REQ] == {"get", "delete"}, found[REQ]
        # A constructor is not a method, and another type's methods are not ours.
        # `Request::new` takes no receiver, which is why the allowlist never has
        # to argue about it on Phase 5's read path (NOTES § D142).
        assert "namespaced" not in found[API] and "request" not in found[API], found[API]
        assert "new" not in found[REQ], found[REQ]
        # …and the trait impls over `http::Request` are not this type.
        assert not {"call", "check"} & found[REQ], found[REQ]

        # The three signature shapes real kube-rs uses that a line-anchored
        # parser cannot read. Each one hid live mutations: every `patch*` method
        # wraps its receiver, `exec` carries a generic list, `entry` puts a
        # lifetime on `&self`. Copied from kube-client 1.1.0, not invented.
        (fake / "shapes.rs").write_text(
            "impl<K> Api<K> {\n"
            "    pub async fn delete_collection(\n"
            "        &self,\n"
            "        dp: &DeleteParams,\n"
            "    ) -> Result<K> { todo!() }\n"
            "    pub async fn patch<P: Serialize + Debug>(\n"
            "        &self,\n"
            "        name: &str,\n"
            "    ) -> Result<K> { todo!() }\n"
            "    pub async fn exec<I, T>(&self, name: &str, cmd: I) -> Result<K> { todo!() }\n"
            "    pub async fn entry<'a>(&'a self, name: &'a str) -> Result<Entry<'a, K>> { todo!() }\n"
            "}\n"
        )
        shapes = derive([fake / "shapes.rs"])[API]
        assert shapes == {"delete_collection", "patch", "exec", "entry"}, shapes
        # Every one of those shapes is itself a canary, so a parser that stops
        # reading any of them turns the build red instead of quietly shrinking
        # the ban list — which is what happened for the whole of Phase 1, and
        # which `clippy.toml` can no longer contradict now that it is compared
        # against this list rather than against the code.
        assert CANARIES[API] & shapes == shapes, CANARIES[API] & shapes

        # --- the drift check, both directions START ---
        # `get` is allowlisted on *both* types — the same unwidened rule — so
        # the file must name exactly Api's delete/patch_scale and Request's
        # delete, and nothing else (NOTES § D142).
        want = wanted(found)
        assert want == {API + "delete", API + "patch_scale", REQ + "delete"}, want
        assert not drift(want, want), "an exact match is not drift"

        cfg = fake / CLIPPY
        cfg.write_text(
            'disallowed-methods = ["kube::Api::delete", "kube::Api::patch_scale", '
            '"kube::core::Request::delete"]\n'
        )
        assert listed(cfg) == want, listed(cfg)
        assert not drift(want, listed(cfg))

        # A method kube has that the file does not ban: the hole this guard
        # exists to catch, and the one clippy cannot report because there is
        # nothing in clippy.toml to report about.
        cfg.write_text(
            'disallowed-methods = ["kube::Api::delete", '
            '"kube::core::Request::delete"]\n'
        )
        missing = drift(want, listed(cfg))
        assert len(missing) == 1 and "patch_scale" in missing[0], missing

        # The same, for the second type — the whole of D142 is that a list which
        # looks complete on `Api` can be missing every `Request` mutation.
        cfg.write_text(
            'disallowed-methods = ["kube::Api::delete", "kube::Api::patch_scale"]\n'
        )
        gone = drift(want, listed(cfg))
        assert len(gone) == 1 and gone[0].startswith(REQ + "delete"), gone

        # A method kube does not have: a stale entry or a typo, which clippy
        # reports only as a warning `-D warnings` does not promote, and only in
        # a crate that links kube — so it is caught here or nowhere.
        cfg.write_text(
            'disallowed-methods = ["kube::Api::delete", "kube::Api::patch_scale", '
            '"kube::core::Request::delete", "kube::Api::deleet"]\n'
        )
        extra = drift(want, listed(cfg))
        assert len(extra) == 1 and "deleet" in extra[0], extra

        # A uniformly wrong prefix is caught too — the comparison is the whole
        # string, not the last segment, because the last segment of
        # `str::replace` is `replace`.
        cfg.write_text(
            'disallowed-methods = ["kube::api::Api::delete", '
            '"kube::api::Api::patch_scale", "kube_core::request::Request::delete"]\n'
        )
        assert len(drift(want, listed(cfg))) == 6, drift(want, listed(cfg))

        # The table form of an entry is read, so a reformat cannot shrink the
        # list the guard thinks it is looking at.
        cfg.write_text(
            "disallowed-methods = [\n"
            '  { path = "kube::Api::delete", reason = "invariant 1" },\n'
            '  "kube::Api::patch_scale",\n'
            '  "kube::core::Request::delete",\n'
            "]\n"
        )
        assert not drift(want, listed(cfg)), drift(want, listed(cfg))
        # --- the drift check, both directions END ---

        # --- the allow-invalid hatch is pinned START ---
        cfg.write_text(
            "disallowed-methods = [\n"
            '  { path = "kube::Api::delete", allow-invalid = true },\n'
            '  "kube::Api::patch_scale",\n'
            '  "kube::core::Request::delete",\n'
            "]\n"
        )
        # `delete` is not feature-gated, so silencing it is the edit clippy's own
        # help text invites and the one that would hide a real hole.
        assert silenced(cfg) == {API + "delete"}, silenced(cfg)
        assert not drift(want, listed(cfg)), "the hatch must not disturb the name check"
        # Silencing a method that is not feature-gated is caught…
        rogue = hatch_drift(silenced(cfg))
        assert len(rogue) == 1 + len(FEATURE_GATED), rogue
        assert "kube::Api::delete carries `allow-invalid`" in rogue[0], rogue[0]
        # …and a plain list silences nothing, so the check has a real negative —
        # it then reports every feature-gated method missing its flag.
        cfg.write_text('disallowed-methods = ["kube::Api::delete"]\n')
        assert silenced(cfg) == set(), silenced(cfg)
        assert len(hatch_drift(silenced(cfg))) == len(FEATURE_GATED)
        # The real file is the only input that satisfies it outright.
        assert not hatch_drift(FEATURE_GATED)
        # What the failure path prints has to be what this guard accepts, or
        # following its instructions is itself a red build.
        real = want | FEATURE_GATED
        cfg.write_text("disallowed-methods = [\n" + render(real) + "]\n")
        assert listed(cfg) == real, listed(cfg)
        assert not drift(real, listed(cfg)), drift(real, listed(cfg))
        assert not hatch_drift(silenced(cfg)), hatch_drift(silenced(cfg))
        # --- the allow-invalid hatch is pinned END ---

        # A prefix is only real while every hop of its re-export chain holds;
        # if one is dropped, every path under it resolves to nothing and clippy
        # says so in a warning `-D warnings` will not catch.
        kube, core = fake / "kube", fake / "kube-core"
        (kube / "src").mkdir(parents=True)
        (core / "src").mkdir(parents=True)
        live = {"kube": kube, "kube-core": core}
        (kube / "src" / "lib.rs").write_text(
            "cfg_client! {\n    pub use api::Api;\n}\n"
            "#[doc(inline)] pub use kube_core as core;\n"
        )
        (core / "src" / "lib.rs").write_text("pub use request::Request;\n")
        assert not dead_prefixes(live), dead_prefixes(live)

        # Each hop, dropped on its own — the second chain has two, and a check
        # that only read the first would call a dead path live.
        (kube / "src" / "lib.rs").write_text(
            "cfg_client! {\n    pub use api::Klient;\n}\n"
            "#[doc(inline)] pub use kube_core as core;\n"
        )
        assert len(dead_prefixes(live)) == 1, dead_prefixes(live)
        assert dead_prefixes(live)[0].startswith(API), dead_prefixes(live)

        (kube / "src" / "lib.rs").write_text("cfg_client! {\n    pub use api::Api;\n}\n")
        assert [d for d in dead_prefixes(live) if d.startswith(REQ)], dead_prefixes(live)

        (kube / "src" / "lib.rs").write_text(
            "cfg_client! {\n    pub use api::Api;\n}\n"
            "#[doc(inline)] pub use kube_core as core;\n"
        )
        (core / "src" / "lib.rs").write_text("pub use request::Requst;\n")
        assert [d for d in dead_prefixes(live) if d.startswith(REQ)], dead_prefixes(live)

        # …and a package that is not there at all is not a pass.
        assert len(dead_prefixes({})) == len(RE_EXPORTS), dead_prefixes({})

        # --- the single exception is singular START ---
        # The derived set, off the real clippy this checkout runs. Pinning it
        # would be the staleness this file refuses everywhere else.
        lints = silencing_lints(clippy_help())
        assert GROUP_CANARIES <= lints, lints
        assert {LINT.replace("-", "_"), "warnings"} <= lints, lints
        # Not every group, or the guard would go red on any `#[allow]` naming
        # any group at all — and a gate that is red for nothing is one people
        # learn to wave through. Stated as "fewer than clippy prints" rather
        # than by naming a group, so a lint clippy re-categorises tomorrow makes
        # this guard wider and not this assertion wrong.
        groups = {n for n, _ in GROUP_LINE.findall(clippy_help())}
        assert 1 < len(lints) < len(groups), (len(lints), len(groups))
        # The lint table above the group table has a level word in its second
        # column; reading one of those as a group is how the set silently widens.
        assert not any(n in lints for n in ("warn", "allow", "deny")), lints

        # Every shape measured against a compiling crate on 2026-09-03. Each one
        # turned a firing `disallowed_methods` green; the hyphen spelling is
        # absent because it does not parse, which was measured too.
        SILENCERS = [
            "#![allow(clippy::disallowed_methods)]",          # the sanctioned one
            "#[allow(clippy::disallowed_methods)]",           # outer, on an item
            "#[expect(clippy::disallowed_methods)]",          # the other keyword
            "#[allow(dead_code, clippy::disallowed_methods)]",  # buried in a list
            "#![allow(clippy::style)]",                       # the group…
            "#![allow(clippy::all)]",                         # …and the group of groups
            "#![allow(warnings)]",                            # rustc's own
            "#![allow(clippy :: disallowed_methods)]",        # whitespace in the path
            "#![cfg_attr(all(), allow(clippy::disallowed_methods))]",
            # Not at the start of a line — a line-anchored guard misses this one.
            'fn f(s: &str) { #[allow(clippy::disallowed_methods)] let _ = s.len(); }',
            # A `]` before the `allow`, which truncates a `[^]]*` match into two
            # harmless halves.
            '#[cfg_attr(all(), doc = "[x]", allow(clippy::disallowed_methods))]',
        ]
        for src in SILENCERS:
            assert any(silences(a, lints) for a in attributes(src)), src

        # The negatives, or the check is "does this file contain an attribute".
        for src in [
            "#[deny(clippy::all)]",
            "#[warn(clippy::disallowed_methods)]",
            "#[allow(unused)]",
            "#[allow(clippy::pedantic)]",
            "#[derive(Debug, Clone)]",
            "#[cfg(test)]",
        ]:
            assert not any(silences(a, lints) for a in attributes(src)), src

        # A fake tree, one silencer per root, because a guard is proven only for
        # the shapes it was fed (NOTES § D29). `src/rules_tests/pod.rs` is the
        # `#[path]` child module case: it compiles into this crate and an inner
        # attribute in it silences the lint for that module (measured).
        tree = fake / "tree"
        for rel in ("src", "src/rules_tests", "tests", "examples", "benches"):
            (tree / rel).mkdir(parents=True)
        (tree / "Cargo.toml").write_text('[package]\nname = "k"\n')
        ops = tree / EXCEPTION
        ops.write_text("//! doc\n#![allow(clippy::disallowed_methods)]\n")
        assert carriers(tree, lints) == {EXCEPTION: 1}, carriers(tree, lints)
        assert not exception_drift(carriers(tree, lints))

        for rel in ("src/analysis.rs", "src/rules_tests/pod.rs", "tests/binary.rs",
                    "examples/spike.rs", "benches/b.rs", "build.rs"):
            other = tree / rel
            other.write_text("#![allow(clippy::all)]\nfn f() {}\n")
            got = carriers(tree, lints)
            assert set(got) == {EXCEPTION, rel}, got
            found = exception_drift(got)
            assert len(found) == 1 and found[0].startswith(rel), found
            other.unlink()
        assert carriers(tree, lints) == {EXCEPTION: 1}, carriers(tree, lints)

        # A file with no silencer is not a carrier — the check is not "this root
        # has a file in it".
        (tree / "src" / "rules.rs").write_text(
            "#[allow(dead_code)]\nfn f() {}\n#[deny(clippy::all)]\nfn g() {}\n")
        assert carriers(tree, lints) == {EXCEPTION: 1}, carriers(tree, lints)

        # Empty is a failure, not a pass: a parser that stopped finding
        # attributes reports the same clean tree as a clean tree does.
        ops.write_text("//! doc\n")
        gone = exception_drift(carriers(tree, lints))
        assert len(gone) == 1 and "no file silences" in gone[0], gone

        # …and one file may not carry two of them.
        ops.write_text("#![allow(clippy::disallowed_methods)]\n"
                       "#[allow(clippy::all)]\nfn f() {}\n")
        twice = exception_drift(carriers(tree, lints))
        assert len(twice) == 1 and "2 silencing" in twice[0], twice
        ops.write_text("#![allow(clippy::disallowed_methods)]\n")

        # Outside a `.rs` file entirely — both measured, both green in clippy.
        assert not manifest_holes(tree), manifest_holes(tree)
        (tree / "Cargo.toml").write_text(
            '[package]\nname = "k"\n\n[lints.clippy]\n'
            'disallowed_methods = "allow"\n')
        assert len(manifest_holes(tree)) == 1, manifest_holes(tree)
        (tree / "Cargo.toml").write_text(
            '[package]\nname = "k"\n\n[lints.clippy]\n'
            'all = { level = "allow", priority = -1 }\n')
        assert len(manifest_holes(tree)) == 1, manifest_holes(tree)
        # A table that sets a level *up* is not a hole.
        (tree / "Cargo.toml").write_text(
            '[package]\nname = "k"\n\n[lints.rust]\nunsafe_code = "forbid"\n')
        assert not manifest_holes(tree), manifest_holes(tree)
        (tree / ".cargo").mkdir()
        for name in ("config.toml", "config"):
            conf = tree / ".cargo" / name
            conf.write_text('[build]\nrustflags = ["-Aclippy::disallowed_methods"]\n')
            assert len(manifest_holes(tree)) == 1, manifest_holes(tree)
            conf.unlink()
        # An empty `.cargo/` is not a finding.
        assert not manifest_holes(tree), manifest_holes(tree)

        # A `-A` on a committed rustc command line: the switch that turns all of
        # the above off, in the two files that already set `-D warnings`
        # job-wide. `guards.sh` is here because it is what CI runs this from.
        (tree / ".github" / "workflows").mkdir(parents=True)
        (tree / "scripts").mkdir()
        assert not flag_holes(tree, lints), flag_holes(tree, lints)
        for rel, text in (
            ("justfile", 'export RUSTFLAGS := "-D warnings %s"\n'),
            (".github/workflows/ci.yml", "env:\n  RUSTFLAGS: -D warnings %s\n"),
            ("scripts/guards.sh", "cargo clippy -- -D warnings %s\n"),
        ):
            for flag in ("-Aclippy::disallowed_methods",   # no space
                         "-A clippy::disallowed-methods",  # hyphen: live on a
                                                           # command line, unlike
                                                           # in an attribute
                         "-A clippy::all",                 # the group
                         "--allow warnings",               # rustc's own
                         "--allow=clippy::style"):         # `=` form
                (tree / rel).write_text(text % flag)
                got = flag_holes(tree, lints)
                assert len(got) == 1 and got[0].startswith(rel), (rel, flag, got)
            # `-D` is not `-A`, and the real files must go through clean.
            (tree / rel).write_text(text % "-D clippy::all")
            assert not flag_holes(tree, lints), (rel, flag_holes(tree, lints))
        # The shapes that are all over this repo and are not lint flags: a bash
        # associative array and kubectl's all-namespaces. A guard red for these
        # is one people learn to wave through.
        (tree / "scripts" / "guards.sh").write_text(
            "declare -A want\nkubectl get pods -A -o json\n"
            "cargo mutants -A left --all-features\n")
        assert not flag_holes(tree, lints), flag_holes(tree, lints)
        # --- the single exception is singular END ---

    print(f"write-guard: self-test passed — {len(TYPES)} types are derived from "
          f"kube's signature shapes without reading its trait impls over "
          f"http::Request, and the clippy.toml check fails on a missing Api "
          f"method, on a missing Request method, on a method kube does not have, "
          f"on a wrong path prefix, on an `allow-invalid` outside the "
          f"feature-gated set, and on any single hop of a re-export chain going "
          f"away; and the single-exception check fails on a silencer in any of "
          f"{len(ROOTS) + 1} cargo roots, on each of {len(SILENCERS)} attribute "
          f"shapes clippy was watched accept, on {EXCEPTION} carrying two or "
          f"none, on a lint level set in Cargo.toml or .cargo/config.toml, and "
          f"on an `-A` naming any of them on a rustc command line in the "
          f"justfile, a workflow or a script — without firing on `declare -A` "
          f"or `kubectl get -A`")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
        sys.exit(0)

    roots = kube_packages()
    if not roots:
        # Until 2026-08-22 this branch printed "nothing to contain" and exited
        # 0 — which is how the guard passed vacuously for the entire project
        # (NOTES § D141). kube has been a dependency since Phase 5 (D140) and
        # cannot stop being one, so its absence is a broken manifest.
        sys.exit(f"write-guard: kube is not in Cargo.lock, so no ban list can be "
                 f"derived and {CLIPPY} is vouched for by nothing. kube has been a "
                 f"dependency since Phase 5 — this is a broken manifest, not a "
                 f"phase that has not started (NOTES § D141).")

    dead = dead_prefixes(roots)
    if dead:
        sys.exit("write-guard: a path prefix no longer resolves, so every entry "
                 "under it in " + CLIPPY + " bans nothing while looking like it "
                 "does. clippy reports that as a warning `-D warnings` does not "
                 "promote, so it has to be caught here (NOTES § D141):\n  "
                 + "\n  ".join(dead))

    found = derive(kube_sources(roots))
    for prefix, methods in found.items():
        if not methods:
            sys.exit(f"write-guard: found kube but extracted no {prefix}… methods "
                     f"— the parser broke, and a guard that finds nothing is worse "
                     f"than none")
        gap = sorted(CANARIES[prefix] - methods)
        if gap:
            sys.exit(f"write-guard: kube parsed, but {gap} are not in the derived "
                     f"{prefix}… list — the signature parser is missing methods, so "
                     f"the containment is partial. Fix the parser before trusting it.")

    lints = silencing_lints(clippy_help())
    if not GROUP_CANARIES <= lints:
        sys.exit(f"write-guard: clippy-driver printed a group table this could not "
                 f"read — {sorted(GROUP_CANARIES - lints)} do not contain {LINT} "
                 f"according to it, and they have for the crate's whole life. Fix "
                 f"the parser: a short list here means an `#[allow]` shape nobody "
                 f"is looking for.")

    want = wanted(found)
    listing = drift(want, listed(ROOT / CLIPPY)) + hatch_drift(silenced(ROOT / CLIPPY))
    holes = (exception_drift(carriers(ROOT, lints)) + manifest_holes(ROOT)
             + flag_holes(ROOT, lints))
    for line in listing + holes:
        print(f"FAIL {line}", file=sys.stderr)
    # Only when the *list* is wrong. Printing "paste this into clippy.toml"
    # under a finding about an `#[allow]` in some other file would be advice
    # that fixes nothing — the same defect `render`'s docstring already names,
    # in the other direction.
    if listing:
        print(f"\n{CLIPPY} is generated, not hand-written. Replace its "
              f"`disallowed-methods` with exactly this:\n\ndisallowed-methods = [",
              file=sys.stderr)
        print(render(want) + "]", file=sys.stderr)
    if listing or holes:
        sys.exit(1)
    known = sum(len(m) for m in found.values())
    print(f"write-guard: {known} methods known across {len(TYPES)} types "
          f"({', '.join(f'{p[:-2]} {len(m)}' for p, m in sorted(found.items()))}), "
          f"{len(want)} banned outside src/ops.rs, {CLIPPY} names exactly "
          f"those, and {EXCEPTION} is the only file in "
          f"{len(ROOTS) + 1} cargo roots that silences any of the "
          f"{len(lints)} lints whose `allow` would turn the ban off — with no "
          f"`-A` naming one of those on any committed rustc command line "
          f"either — OK")
