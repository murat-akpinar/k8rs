#!/usr/bin/env python3
"""Enforce invariant 1: mutations live in `src/ops.rs` and nowhere else.

**The matching is clippy's.** `clippy.toml`'s `disallowed-methods` resolves
paths, so it tells `kube::Api::replace` from `str::replace`; with `-D warnings`
it is the containment. This script owns the half clippy cannot do — proving that
list is still *complete* against the kube actually in `Cargo.lock`.

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

    print(f"write-guard: self-test passed — {len(TYPES)} types are derived from "
          f"kube's signature shapes without reading its trait impls over "
          f"http::Request, and the clippy.toml check fails on a missing Api "
          f"method, on a missing Request method, on a method kube does not have, "
          f"on a wrong path prefix, on an `allow-invalid` outside the "
          f"feature-gated set, and on any single hop of a re-export chain going "
          f"away")


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

    want = wanted(found)
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
    known = sum(len(m) for m in found.values())
    print(f"write-guard: {known} methods known across {len(TYPES)} types "
          f"({', '.join(f'{p[:-2]} {len(m)}' for p, m in sorted(found.items()))}), "
          f"{len(want)} banned outside src/ops.rs, and {CLIPPY} names exactly "
          f"those — OK")
