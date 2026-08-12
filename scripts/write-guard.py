#!/usr/bin/env python3
"""Enforce invariant 1: mutations live in `src/ops.rs` and nowhere else.

Written as an **allowlist**, deliberately. Outside `ops.rs` only these kube
`Api` methods may appear:

    get*  ·  list*  ·  watch*  ·  logs  ·  log_stream  ·  apiserver_version

A denylist would have to know about `delete`, `patch`, `replace`, `create`,
`cordon`, `uncordon`, `restart`, `evict`, `exec`, `attach`, `portforward`,
`entry`, `patch_scale` — and about whatever kube-rs adds next release, which
is exactly the thing nobody will remember to update. So the ban list is
*derived*: every `&self` method of `Api<K>` in the kube version actually in
`Cargo.lock`, minus the allowlist above.

Until `kube` is a dependency there is no surface to contain and the check says
so instead of passing silently — `--self-test` proves the logic either way.

Usage:
    write-guard.py             # check src/
    write-guard.py --self-test # prove the guard fails when it should
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OPS = "ops.rs"

ALLOWED_PREFIXES = ("get", "list", "watch")
ALLOWED_EXACT = {"logs", "log_stream", "apiserver_version"}

# The ban list is *derived*, so the failure that matters is under-extraction:
# a signature the parser did not recognise (a `&self` wrapped onto its own line,
# an impl block behind a cfg) drops a mutation off the list silently, and a
# guard that under-reports reads exactly like a guard with nothing to report.
# These have existed on `Api<K>` for the crate's whole life; if they are
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
# A call is `.name(`, but also `Api::name(`, `<Api<Pod>>::name(`, a turbofish
# `name::<T>(`, and the raw identifier `r#name` — all of which reach the same
# method and none of which start with a dot.
CALL = re.compile(r"(?:\.|::)\s*(?:r#)?(?P<name>\w+)\s*(?:::<[^()]*>)?\s*\(")


def allowed(name: str) -> bool:
    return name in ALLOWED_EXACT or name.startswith(ALLOWED_PREFIXES)


# The allowlist is three bare prefixes, so any future kube method whose name
# begins with one is exempted the day it appears, silently and forever. Nothing
# in kube today trips it — which is exactly when to write the assertion.
assert not [c for c in CANARIES if allowed(c)], "the allowlist exempts a known mutation"


def strip_line_comment(line: str) -> str:
    """Drop `// …`, but only when the slashes are not inside a string.

    A blanket `//.*$` is a hole with no intent required: any line holding a URL
    hides every call after it, because `https://…` starts with a comment marker
    as far as the regex is concerned.
    """
    in_str = False
    i = 0
    while i < len(line):
        c = line[i]
        if in_str:
            if c == "\\":
                i += 2
                continue
            if c == '"':
                in_str = False
        elif c == '"':
            in_str = True
        elif c == "/" and line[i + 1 : i + 2] == "/":
            return line[:i]
        i += 1
    return line


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


def kube_sources() -> list[Path]:
    """The kube-client sources cargo resolved for this build, or []."""
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if out.returncode != 0:
        sys.exit(f"write-guard: cargo metadata failed\n{out.stderr.strip()}")
    meta = json.loads(out.stdout)
    roots = [
        Path(p["manifest_path"]).parent
        for p in meta["packages"]
        if p["name"] in ("kube-client", "kube")
    ]
    return [f for root in roots for f in root.rglob("*.rs")]


def offences(roots: list[Path], banned: set[str]) -> list[str]:
    """Every banned call outside ops.rs, across every root that compiles.

    Recursive, and not only `src/`: `cargo test --all-targets` builds `tests/`,
    `examples/` and `benches/` too, and CLAUDE.md puts learning spikes in
    `examples/`. Invariant 1 does not stop at the library.
    """
    hits = []
    # A root may be a file — `build.rs` is compiled and *executed* at build
    # time, which makes it the most privileged file in the crate, and it sits
    # under none of the directories above.
    exempt = (ROOT / "src" / OPS).resolve()
    candidates = {
        p
        for root in roots
        for p in ([root] if root.is_file() else root.rglob("*.rs") if root.is_dir() else [])
    }
    for path in sorted(candidates):
        # `src/ops.rs`, by path — not "any file called ops.rs". The name test
        # exempted `tests/ops.rs`, `examples/ops.rs` and `benches/ops.rs` too,
        # and `tests/ops.rs` is precisely what Phase 7's write-path integration
        # test would be called: the one file that must not be able to opt out.
        if path.resolve() == exempt:
            continue
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for m in CALL.finditer(strip_line_comment(line)):
                name = m.group("name")
                if name in banned and not allowed(name):
                    hits.append(
                        f"{path.relative_to(ROOT)}:{n}  .{name}() — writes belong "
                        f"in src/{OPS} (invariant 1)"
                    )
    return hits


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
        # the ban list — which is what happened for the whole of Phase 1.
        assert CANARIES & shapes == shapes, CANARIES & shapes

        src = fake / "src"
        src.mkdir()
        (src / "k8s.rs").write_text(
            "async fn f(api: Api<Pod>) {\n"
            "    let p = api.get(\"web\").await;\n"
            "    let d = api.delete(\"web\").await;   // the violation\n"
            "}\n"
        )
        (src / OPS).write_text("async fn scale(api: Api<Deployment>) { api.patch_scale(\"web\").await; }\n")
        global ROOT
        keep, ROOT = ROOT, fake
        try:
            hits = offences([src], banned)

            # Call forms that reach the same method without a leading dot, and
            # the one that needs no intent at all: a `//` inside a string
            # literal used to blank the rest of the line, so any URL on it hid
            # every write that followed.
            (src / "sneaky.rs").write_text(
                "async fn g(api: Api<Pod>) {\n"
                "    let doc = \"https://kube.rs\"; api.delete(\"a\").await;\n"
                "    Api::delete(&api, \"b\").await;\n"
                "    api.r#delete(\"c\").await;\n"
                "    // api.delete(\"d\") — a real comment, and not a violation\n"
                "}\n"
            )
            sneaky = [h for h in offences([src], banned) if "sneaky.rs" in h]
            assert len(sneaky) == 3, sneaky

            # --- the exemption is a path, not a name START ---
            # `src/ops.rs` is the containment. Every other file that happens to
            # be *called* ops.rs walked free, and `tests/ops.rs` is the obvious
            # name for Phase 7's write-path integration test — the one file that
            # must not be able to opt out of the invariant it is testing.
            write = 'async fn f(api: Api<Pod>) { api.delete("web").await; }\n'
            for rel in ("tests/ops.rs", "examples/ops.rs", "benches/ops.rs"):
                p = fake / rel
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(write)
            (fake / "build.rs").write_text(write)
            elsewhere = offences(
                [fake / d for d in ("tests", "examples", "benches")] + [fake / "build.rs"],
                banned,
            )
            assert len(elsewhere) == 4, f"a write named ops.rs escaped containment: {elsewhere}"

            # …and the negative half: the real one is still exempt, or the fix
            # above is just a guard that fails on everything.
            assert not offences([src / OPS], banned), "src/ops.rs is the containment, not a violation"
            # --- the exemption is a path, not a name END ---
        finally:
            ROOT = keep
        assert len(hits) == 1 and "delete" in hits[0], hits
    print("write-guard: self-test passed — a write outside src/ops.rs is caught, "
          "including in a file merely named ops.rs and in build.rs; the same "
          "call inside src/ops.rs is not")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
        sys.exit(0)

    kube = kube_sources()
    if not kube:
        print("write-guard: kube is not a dependency yet — nothing to contain. "
              "The guard arrives with the client (Phase 5); its logic is proven "
              "by --self-test until then.")
        sys.exit(0)

    banned = api_methods(kube)
    if not banned:
        sys.exit("write-guard: found kube but extracted no Api methods — the "
                 "parser broke, and a guard that finds nothing is worse than none")
    missing = sorted(CANARIES - banned)
    if missing:
        sys.exit(f"write-guard: kube parsed, but {missing} are not in the derived "
                 f"ban list — the signature parser is missing methods, so the "
                 f"containment is partial. Fix the parser before trusting it.")
    problems = offences(
        [ROOT / d for d in ("src", "tests", "examples", "benches")] + [ROOT / "build.rs"],
        banned,
    )
    for line in problems:
        print(f"FAIL {line}", file=sys.stderr)
    if problems:
        sys.exit(1)
    print(f"write-guard: {len(banned)} Api methods known, "
          f"{len([m for m in banned if not allowed(m)])} banned outside {OPS} — OK")
