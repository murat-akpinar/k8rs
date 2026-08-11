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
# These four have existed on `Api<K>` for the crate's whole life; if they are
# missing, the parser broke, not kube-rs.
CANARIES = {"delete", "patch", "replace", "create"}

IMPL_API = re.compile(r"^\s*impl(?:<[^>]*>)?\s+Api<")
METHOD = re.compile(r"^\s*pub\s+(?:async\s+)?fn\s+(?P<name>\w+)\s*\(\s*&\s*(?:mut\s+)?self")
CALL = re.compile(r"\.\s*(?P<name>\w+)\s*\(")
LINE_COMMENT = re.compile(r"//.*$")


def allowed(name: str) -> bool:
    return name in ALLOWED_EXACT or name.startswith(ALLOWED_PREFIXES)


def api_methods(sources: list[Path]) -> set[str]:
    """Every `&self` method declared in an `impl ... Api<K>` block."""
    found: set[str] = set()
    for path in sources:
        inside = False
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if IMPL_API.match(line):
                inside = True
                continue
            if inside:
                if line.startswith("}"):
                    inside = False
                    continue
                m = METHOD.match(line)
                if m:
                    found.add(m.group("name"))
    return found


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


def offences(src: Path, banned: set[str]) -> list[str]:
    hits = []
    for path in sorted(src.glob("*.rs")):
        if path.name == OPS:
            continue
        for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for m in CALL.finditer(LINE_COMMENT.sub("", line)):
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

        # The known limitation, proven rather than assumed: a signature whose
        # `&self` wraps onto its own line is not matched, so it never reaches
        # the ban list. That silent hole is exactly what CANARIES turns into a
        # red build — it is the reason that check exists.
        (fake / "wrapped.rs").write_text(
            "impl<K> Api<K> {\n"
            "    pub async fn replace(\n"
            "        &self,\n"
            "        name: &str,\n"
            "    ) -> Result<K> { todo!() }\n"
            "}\n"
        )
        wrapped = api_methods([fake / "wrapped.rs"])
        assert "replace" not in wrapped, "the parser now reads wrapped signatures — tighten this test"
        assert CANARIES - wrapped, "an under-extracted ban list must be visible to the caller"

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
            hits = offences(src, banned)
        finally:
            ROOT = keep
        assert len(hits) == 1 and "delete" in hits[0], hits
    print("write-guard: self-test passed — a write outside ops.rs is caught, "
          "the same call inside ops.rs is not")


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
    problems = offences(ROOT / "src", banned)
    for line in problems:
        print(f"FAIL {line}", file=sys.stderr)
    if problems:
        sys.exit(1)
    print(f"write-guard: {len(banned)} Api methods known, "
          f"{len([m for m in banned if not allowed(m)])} banned outside {OPS} — OK")
