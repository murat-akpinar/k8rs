#!/usr/bin/env python3
"""Enforce invariant 1: mutations live in `src/ops.rs` and nowhere else.

**The matching is clippy's.** `clippy.toml`'s `disallowed-methods` resolves
paths, so it tells `kube::Api::replace` from `str::replace`; with `-D warnings`
it is the containment. This script owns the half clippy cannot do — proving that
list is still *complete* against the kube actually in `Cargo.lock`.

Written as an **allowlist**, deliberately. Outside `ops.rs` only these kube
`Api` methods may appear:

    get*  ·  list*  ·  watch*  ·  logs  ·  log_stream  ·  apiserver_version

A denylist would have to know about `delete`, `patch`, `replace`, `create`,
`cordon`, `uncordon`, `restart`, `evict`, `exec`, `attach`, `portforward`,
`entry`, `patch_scale` — and about whatever kube-rs adds next release, which
is exactly the thing nobody will remember to update. So the ban list is
*derived*: every `&self` method of `Api<K>` in the kube version actually in
`Cargo.lock`, minus the allowlist above. `clippy.toml` must name exactly that
set, so a kube bump that adds a method is red in the commit that bumps it.

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

# One spelling, so the comparison below can be a string set. `kube::api::Api::x`
# resolves to the same def-id and would work identically; the short one is
# picked because `kube` is the dependency we declare (invariant 10) and
# `kube_client` is transitive.
PREFIX = "kube::Api::"
# …and what makes that spelling real, checked rather than assumed. Measured
# 2026-08-22 (NOTES § D141): clippy warns about a `disallowed-methods` path it
# cannot resolve, but that warning is **not** promoted by `-D warnings`, and it
# is not emitted at all by a crate that does not link kube. So a `clippy.toml`
# whose every path resolved to nothing would leave CI green. This is a proxy for
# compiling a call — it catches kube moving `Api` out of its root, which is the
# way the spelling realistically dies.
ROOT_EXPORT = re.compile(r"^\s*pub use api::Api\s*;", re.M)

# kube's `ws` feature, which D140 leaves off. Their `impl<K> Api<K>` block is
# behind `#[cfg(feature = "ws")]`, so they are in the source this script parses
# and *not* in the crate clippy compiles: their entries resolve to nothing and
# contain nothing. They stay listed so that turning `ws` on needs no one to
# remember, and they carry `allow-invalid` so the unresolved-path warning has a
# silent baseline — measured 2026-08-22, they were three of the twenty-nine and
# a fourth, real one would have read exactly like them (NOTES § D141).
#
# Pinned rather than derived because clippy's own help text offers
# `allow-invalid = true` as *the* fix for that warning, so the plausible next
# edit is someone silencing a genuine hole with it.
# `exec` is also a CANARY below, and the two are not in conflict: a canary
# proves the *parser* still reads a signature shape out of kube's source, which
# is true whether or not the feature that compiles it is on.
FEATURE_GATED = {"attach", "exec", "portforward"}

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
# present — so the canary set passed while 17 of 45 methods, including every
# `patch*`, went uncontained. One canary per signature form, not per method:
#   delete_collection — receiver wrapped onto its own line
#   exec              — generic list between the name and the paren
#   entry             — lifetime on the receiver (`&'a self`)
CANARIES = {"delete", "patch", "replace", "create",
            "delete_collection", "exec", "entry"}

IMPL_API = re.compile(r"^[ \t]*impl(?:<[^>]*>)?\s+Api<", re.M)
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
assert not [c for c in CANARIES if allowed(c)], "the allowlist exempts a known mutation"


def api_methods(sources: list[Path]) -> set[str]:
    """Every `&self` method declared in an `impl ... Api<K>` block."""
    found: set[str] = set()
    for path in sources:
        text = path.read_text(encoding="utf-8", errors="replace")
        for m in IMPL_API.finditer(text):
            found.update(mm.group("name") for mm in METHOD.finditer(impl_body(text, m.end())))
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
    """`kube` and `kube-client`'s source roots, as cargo resolved them."""
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
        if p["name"] in ("kube-client", "kube")
    }


def kube_sources(roots: dict[str, Path]) -> list[Path]:
    return [f for root in roots.values() for f in root.rglob("*.rs")]


def prefix_is_real(roots: dict[str, Path]) -> bool:
    """Is `kube::Api` still a path? See PREFIX for why this is not clippy's job."""
    root = roots.get("kube")
    if root is None:
        return False
    lib = root / "src" / "lib.rs"
    return lib.is_file() and bool(ROOT_EXPORT.search(lib.read_text(encoding="utf-8")))


def wanted(banned: set[str]) -> set[str]:
    """The exact `disallowed-methods` list `clippy.toml` has to carry."""
    return {PREFIX + m for m in banned if not allowed(m)}


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
        [f"{p} — kube has this Api method and {CLIPPY} does not ban it. "
         f"Nothing stops a call to it outside src/ops.rs (invariant 1)."
         for p in sorted(want - got)]
        + [f"{p} — {CLIPPY} bans this and the kube in Cargo.lock has no such "
           f"Api method. Either kube removed it, or the path is misspelled — "
           f"and a path clippy cannot resolve bans nothing while looking like "
           f"it does (NOTES § D141)."
           for p in sorted(got - want)]
    )


def hatch_drift(got: set[str]) -> list[str]:
    """`allow-invalid` may cover kube's `ws` methods and nothing else.

    A function and not a block inside `main`, because `--self-test` cannot reach
    `main`. It was written there first and this file's own second pass moved it.
    """
    want = {PREFIX + m for m in FEATURE_GATED}
    return (
        [f"{p} carries `allow-invalid`, which silences the one warning clippy "
         f"gives when a path resolves to nothing. Only kube's `ws` methods may "
         f"(see FEATURE_GATED) — if this was added to quiet a warning, the "
         f"warning was the finding (NOTES § D141)."
         for p in sorted(got - want)]
        + [f"{p} is `ws`-gated and needs `allow-invalid`, or its unresolved-path "
           f"warning becomes the baseline noise a real one would hide in."
           for p in sorted(want - got)]
    )


def render(want: set[str]) -> str:
    """The `disallowed-methods` body, in the exact form this guard accepts.

    The failure path printed bare strings for every entry until this existed,
    so pasting what the guard told you to paste produced a file it then rejected
    for the missing `allow-invalid` — a gate that cannot be passed by following
    its own instructions. The self-test round-trips this back through `listed`
    and `silenced` for that reason.
    """
    gated = {PREFIX + m for m in FEATURE_GATED}
    return "".join(
        f'    {{ path = "{p}", allow-invalid = true }},\n' if p in gated else f'    "{p}",\n'
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
        )
        banned = api_methods([fake / "api.rs"])
        assert banned == {"get", "delete", "patch_scale"}, banned
        # A constructor is not a method, and another type's methods are not ours
        assert "namespaced" not in banned and "request" not in banned, banned

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
        shapes = api_methods([fake / "shapes.rs"])
        assert shapes == {"delete_collection", "patch", "exec", "entry"}, shapes
        # Every one of those shapes is itself a canary, so a parser that stops
        # reading any of them turns the build red instead of quietly shrinking
        # the ban list — which is what happened for the whole of Phase 1, and
        # which `clippy.toml` can no longer contradict now that it is compared
        # against this list rather than against the code.
        assert CANARIES & shapes == shapes, CANARIES & shapes

        # --- the drift check, both directions START ---
        # `banned` is {get, delete, patch_scale}; `get` is allowlisted, so the
        # file must name exactly delete and patch_scale.
        want = wanted(banned)
        assert want == {PREFIX + "delete", PREFIX + "patch_scale"}, want
        assert not drift(want, want), "an exact match is not drift"

        cfg = fake / CLIPPY
        cfg.write_text(
            'disallowed-methods = ["kube::Api::delete", "kube::Api::patch_scale"]\n'
        )
        assert listed(cfg) == want, listed(cfg)
        assert not drift(want, listed(cfg))

        # A method kube has that the file does not ban: the hole this guard
        # exists to catch, and the one clippy cannot report because there is
        # nothing in clippy.toml to report about.
        cfg.write_text('disallowed-methods = ["kube::Api::delete"]\n')
        missing = drift(want, listed(cfg))
        assert len(missing) == 1 and "patch_scale" in missing[0], missing

        # A method kube does not have: a stale entry or a typo, which clippy
        # reports only as a warning `-D warnings` does not promote, and only in
        # a crate that links kube — so it is caught here or nowhere.
        cfg.write_text(
            'disallowed-methods = ["kube::Api::delete", "kube::Api::patch_scale", '
            '"kube::Api::deleet"]\n'
        )
        extra = drift(want, listed(cfg))
        assert len(extra) == 1 and "deleet" in extra[0], extra

        # A uniformly wrong prefix is caught too — the comparison is the whole
        # string, not the last segment, because the last segment of
        # `str::replace` is `replace`.
        cfg.write_text(
            'disallowed-methods = ["kube::api::Api::delete", '
            '"kube::api::Api::patch_scale"]\n'
        )
        assert len(drift(want, listed(cfg))) == 4, drift(want, listed(cfg))

        # The table form of an entry is read, so a reformat cannot shrink the
        # list the guard thinks it is looking at.
        cfg.write_text(
            "disallowed-methods = [\n"
            '  { path = "kube::Api::delete", reason = "invariant 1" },\n'
            '  "kube::Api::patch_scale",\n'
            "]\n"
        )
        assert not drift(want, listed(cfg)), drift(want, listed(cfg))
        # --- the drift check, both directions END ---

        # --- the allow-invalid hatch is pinned START ---
        cfg.write_text(
            "disallowed-methods = [\n"
            '  { path = "kube::Api::delete", allow-invalid = true },\n'
            '  "kube::Api::patch_scale",\n'
            "]\n"
        )
        # `delete` is not ws-gated, so silencing it is the edit clippy's own help
        # text invites and the one that would hide a real hole.
        assert silenced(cfg) == {PREFIX + "delete"}, silenced(cfg)
        assert not drift(want, listed(cfg)), "the hatch must not disturb the name check"
        # Silencing a method that is not ws-gated is caught…
        rogue = hatch_drift(silenced(cfg))
        assert len(rogue) == 1 + len(FEATURE_GATED), rogue
        assert "kube::Api::delete carries `allow-invalid`" in rogue[0], rogue[0]
        # …and a plain list silences nothing, so the check has a real negative —
        # it then reports only the three ws methods that are missing their flag.
        cfg.write_text('disallowed-methods = ["kube::Api::delete"]\n')
        assert silenced(cfg) == set(), silenced(cfg)
        assert len(hatch_drift(silenced(cfg))) == len(FEATURE_GATED)
        # The real file is the only input that satisfies it outright.
        assert not hatch_drift({PREFIX + m for m in FEATURE_GATED})
        # What the failure path prints has to be what this guard accepts, or
        # following its instructions is itself a red build.
        real = wanted({"delete", "patch_scale"} | FEATURE_GATED)
        cfg.write_text("disallowed-methods = [\n" + render(real) + "]\n")
        assert listed(cfg) == real, listed(cfg)
        assert not drift(real, listed(cfg)), drift(real, listed(cfg))
        assert not hatch_drift(silenced(cfg)), hatch_drift(silenced(cfg))
        # --- the allow-invalid hatch is pinned END ---

        # The spelling in PREFIX is only real while kube re-exports `Api` at its
        # root; if it stops, every path in clippy.toml resolves to nothing and
        # clippy says so in a warning nobody's `-D warnings` will catch.
        kube = fake / "kube"
        (kube / "src").mkdir(parents=True)
        (kube / "src" / "lib.rs").write_text("cfg_client! {\n    pub use api::Api;\n}\n")
        assert prefix_is_real({"kube": kube})
        (kube / "src" / "lib.rs").write_text("cfg_client! {\n    pub use api::Klient;\n}\n")
        assert not prefix_is_real({"kube": kube}), "a kube without Api at its root passed"
        assert not prefix_is_real({}), "a missing kube package passed"

    print("write-guard: self-test passed — the derived list is read from all three "
          "kube signature shapes, and the clippy.toml check fails on a missing "
          "entry, on a method kube does not have, on a wrong path prefix, on an "
          "`allow-invalid` outside kube's ws methods, and on a kube that no longer "
          "exports Api at its root")


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

    if not prefix_is_real(roots):
        sys.exit(f"write-guard: kube no longer re-exports `Api` at its root, so "
                 f"every `{PREFIX}…` path in {CLIPPY} resolves to nothing and bans "
                 f"nothing. clippy reports that as a warning `-D warnings` does not "
                 f"promote, so it has to be caught here. Re-check the spelling "
                 f"against kube's lib.rs and update PREFIX (NOTES § D141).")

    banned = api_methods(kube_sources(roots))
    if not banned:
        sys.exit("write-guard: found kube but extracted no Api methods — the "
                 "parser broke, and a guard that finds nothing is worse than none")
    missing = sorted(CANARIES - banned)
    if missing:
        sys.exit(f"write-guard: kube parsed, but {missing} are not in the derived "
                 f"ban list — the signature parser is missing methods, so the "
                 f"containment is partial. Fix the parser before trusting it.")

    want = wanted(banned)
    problems = drift(want, listed(ROOT / CLIPPY))

    problems += hatch_drift(silenced(ROOT / CLIPPY))
    for line in problems:
        print(f"FAIL {line}", file=sys.stderr)
    if problems:
        print(f"\n{CLIPPY} is generated, not hand-written. Replace its "
              f"`disallowed-methods` with exactly this:\n\ndisallowed-methods = [",
              file=sys.stderr)
        print(render(want) + "]", file=sys.stderr)
        sys.exit(1)
    print(f"write-guard: {len(banned)} Api methods known, {len(want)} banned "
          f"outside src/ops.rs, and {CLIPPY} names exactly those — OK")
