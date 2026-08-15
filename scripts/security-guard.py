#!/usr/bin/env python3
"""The mechanizable half of CLAUDE.md § Security gate, as a gate.

A checklist item is the thing that gets skipped — every list in this repo that
mattered became a script (`write-guard.py` for invariant 1, `test-guard.py` for
"the suite ran"). Four items of the security gate are decidable on this tree
*today*, so they stop being read and start being run:

1. **Workflow hygiene** — a top-level `permissions:` block that defaults to
   read, every `uses:` pinned to a 40-hex commit SHA rather than a tag (a tag
   is a moving pointer somebody else controls), and no `pull_request_target`,
   which runs with the base repo's secrets against a fork's code.
2. **No API string ever reaches a shell** (invariant 9). `src/` spawns nothing
   at all today; the guard keeps the *class* empty, so it is already standing
   when `$EDITOR` arrives with `e` in v0.4 — the one place this repo will ever
   spawn a process, and the one that must use an argument vector.
3. **No telemetry, no second outbound path** — the dependency list is the ten
   invariant 10 allows and nothing else (an allowlist, because a denylist of
   HTTP crates would need to know about `reqwest`, `hyper`, `ureq`, `isahc`,
   `attohttpc`… and whatever ships next month), and no hardcoded host in the
   code.
4. **Token hygiene** — a type that can hold a kube `Config`/`Client` may not
   *derive* `Debug`: the derived one prints the auth info, and the gate's line
   is "any type that can hold config has a wrapped `Debug`". `kube` is not a
   dependency until Phase 5, so the run says so out loud rather than passing in
   silence, in `write-guard.py`'s own pattern — the logic is proven by
   `--self-test` until the surface exists.

What is deliberately **not** here: every gate item about code that does not
exist yet (the write path and `dryRun`, the audit log's mode, `--read-only`
being structurally unreachable, the edit temp file at 0600, control-character
stripping, Secret reveal, the panic path, `SHA256SUMS`). A check written now
against an empty tree passes forever and reads like coverage — the worse of the
two failures. They stay human-checked until the code lands.

Usage:
    security-guard.py             # check this repository
    security-guard.py --self-test # prove every check fails when it should
"""

import re
import sys
import tomllib
from functools import lru_cache
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# --- what each check is allowed to see -----------------------------------

# The ten of invariant 10 (CLAUDE.md § Hard invariants). An eleventh is a
# recorded decision, so it lands here and in CLAUDE.md in the same change —
# which is exactly the review this line exists to force.
ALLOWED_CRATES = {
    "kube", "k8s-openapi", "ratatui", "crossterm", "tokio",
    "anyhow", "serde_json", "serde_yaml_ng", "x509-parser", "similar",
}

# Reserved names that cannot resolve to a real service (RFC 2606 / RFC 6761),
# so a URL under one is documentation or a test double, never an outbound path.
# `localhost` is *not* here on purpose: a hardcoded loopback URL in product code
# is a second outbound path, and usually a dev leftover.
RESERVED_HOSTS = re.compile(
    r"(?:^|\.)(?:example\.(?:com|net|org)|test|example|invalid)$", re.I
)

SHELLS = {"sh", "bash", "zsh", "dash", "ksh", "fish", "csh", "tcsh",
          "cmd", "cmd.exe", "command.com", "powershell", "powershell.exe",
          "pwsh", "env"}

# Types that carry, or can carry, a bearer token / client certificate.
TOKEN_TYPES = ("Config", "Client", "AuthInfo", "ExecConfig", "TokenFile")

# --- shared scanning ------------------------------------------------------

RAW_STR = re.compile(r'r(#*)"')
# A char literal, so `trim_matches('"')` does not open a string that never
# closes and blank the rest of the file. That call is real: it is in
# src/rules_tests/certificate.rs, and it is why this branch exists.
CHAR_LIT = re.compile(r"'(?:\\.|[^\\'])'")


def sources(root: Path) -> list[Path]:
    """Every .rs file cargo compiles — the same roots write-guard.py scans."""
    return sorted(
        p
        for r in ("src", "tests", "examples", "benches")
        for p in (root / r).rglob("*.rs")
    ) + [p for p in [root / "build.rs"] if p.is_file()]


def strip_comments(text: str) -> str:
    """`text` with every comment blanked to spaces, offsets preserved.

    String-aware in both directions, and that is the whole difficulty: a URL is
    written `https://…`, so a scanner that blanks from the first `//` hides
    every line that mentions one (the hole `write-guard.py`'s own stripper
    documents), while a scanner that ignores strings reads `"// not a comment"`
    as code. Blanking rather than deleting keeps `count('\\n')` a line number.
    """
    out = list(text)
    n = len(text)
    i = 0

    def blank(a: int, b: int) -> None:
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = text[i]
        if c == "/" and text[i + 1 : i + 2] == "/":
            j = text.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
        elif c == "/" and text[i + 1 : i + 2] == "*":
            # Rust nests block comments, so this is a depth count, not a find.
            depth, j = 1, i + 2
            while j < n and depth:
                if text[j : j + 2] == "/*":
                    depth, j = depth + 1, j + 2
                elif text[j : j + 2] == "*/":
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
        elif c == "r" and (m := RAW_STR.match(text, i)):
            close = '"' + "#" * len(m.group(1))
            j = text.find(close, m.end())
            i = n if j < 0 else j + len(close)
        elif c == '"':
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
        elif c == "'" and (m := CHAR_LIT.match(text, i)):
            i = m.end()
        else:
            i += 1
    return "".join(out)


@lru_cache(maxsize=None)
def code(path: Path) -> str:
    """Stripped source, cached: six checks read the same 22k lines."""
    return strip_comments(path.read_text(encoding="utf-8", errors="replace"))


def at(text: str, pos: int) -> int:
    return text.count("\n", 0, pos) + 1


def rel(root: Path, path: Path) -> str:
    return str(path.relative_to(root))


# --- 1. workflow hygiene --------------------------------------------------

USES = re.compile(r"^\s*(?:-\s*)?uses:\s*(?P<ref>\S+)", re.M)
PINNED = re.compile(r"^[\w.-]+/[\w.-]+(?:/[\w./-]+)?@[0-9a-fA-F]{40}$")
WRITE_PERM = re.compile(r"\bwrite(?:-all)?\b")


def top_level_permissions(text: str) -> list[str]:
    """The `permissions:` block written at column 0, as `key: value` strings.

    `None` when the workflow has none at all — which is the failure, because
    the repository default is what applies then, and nothing in this repo
    controls it.
    """
    # A trailing YAML comment is not a grant: `permissions: read-all  # writes
    # go in the release job` holds the word "write" and nothing else.
    def value(s: str) -> str:
        return s.split(" #", 1)[0].strip()

    lines = text.splitlines()
    for n, line in enumerate(lines):
        if not line.startswith("permissions:"):
            continue
        inline = value(line.split(":", 1)[1])
        if inline:
            return [inline]
        block = []
        for follow in lines[n + 1 :]:
            if not follow.strip() or follow.strip().startswith("#"):
                continue
            if not follow[:1].isspace():
                break
            block.append(value(follow))
        return block
    return []


def check_workflows(root: Path) -> tuple[list[str], str]:
    files = sorted(
        p for p in (root / ".github" / "workflows").glob("*.y*ml")
        if p.suffix in (".yml", ".yaml")
    )
    if not files:
        return ([".github/workflows/ holds no workflow — either CI is gone, or "
                 "this scan is looking in the wrong place and every check under "
                 "it just passed on an empty list"], "")

    problems, uses = [], []
    for path in files:
        text = path.read_text(encoding="utf-8")
        where = rel(root, path)

        perms = top_level_permissions(text)
        if not perms:
            problems.append(f"{where}  no top-level `permissions:` — the "
                            f"repository default applies, and it is not read")
        for entry in perms:
            if WRITE_PERM.search(entry):
                problems.append(f"{where}  top-level permissions grant write "
                                f"(`{entry}`) — the default is read; escalate "
                                f"in the one job that needs it")

        for m in USES.finditer(text):
            ref = m.group("ref").strip("'\"")
            uses.append(ref)
            if ref.startswith("./"):
                continue  # an action in this repo, reviewed with this repo
            if not PINNED.match(ref):
                problems.append(f"{where}:{at(text, m.start())}  uses: {ref} — "
                                f"pin to a 40-hex commit SHA; a tag is a "
                                f"pointer somebody else can move")

        for n, line in enumerate(text.splitlines(), 1):
            if "pull_request_target" in line:
                problems.append(f"{where}:{n}  pull_request_target runs a "
                                f"fork's code with this repo's secrets")

    # "Matched no uses: line" and "there are no actions to check" print the
    # same pass (CLAUDE.md § A derived list asserts it found something).
    # actions/checkout is in every job here; if it went on purpose, move this
    # line to whatever replaced it rather than deleting it.
    if not any(u.startswith("actions/checkout@") for u in uses):
        problems.append("no `uses: actions/checkout@…` found in any workflow — "
                        "either the scan broke and this check just passed on "
                        "nothing, or checkout was dropped and this canary moves")
    # Not "pinned action(s)": on a red run some of them are the opposite, and a
    # note that contradicts the FAIL above it is the line people believe.
    return problems, f"{len(files)} workflow(s), {len(uses)} action(s)"


# --- 2. no API string ever reaches a shell --------------------------------

SPAWN = re.compile(r"Command::new\s*\(\s*(?P<lit>\"(?P<prog>[^\"]*)\")?")
# `"-c"` as an argument in a file that spawns: the shell-string form.
SHELL_FLAG = re.compile(r"\"(-c|/[cC]|-[cC]ommand)\"")
# A whole command line inside one literal — an argument vector's opposite.
COMMAND_STRING = re.compile(
    r"\"[^\"]*\b(?:sh|bash|zsh|cmd|powershell|pwsh)\s+[-/][cC]\b[^\"]*\""
)


def check_shell(files: list[Path], root: Path) -> tuple[list[str], str]:
    problems, spawners = [], 0
    for path in files:
        text = code(path)
        if "Command::new" not in text:
            # Scoped to files that can actually spawn, on purpose: `-c` is an
            # ordinary token in the command log's display text (`kubectl logs
            # -c app`), which invariant 4 requires and nothing executes.
            continue
        spawners += 1
        where = rel(root, path)
        for m in SPAWN.finditer(text):
            prog = m.group("prog")
            if prog is not None and Path(prog).name in SHELLS:
                problems.append(f"{where}:{at(text, m.start())}  "
                                f"Command::new(\"{prog}\") — a shell, so every "
                                f"argument after it is parsed, not passed")
        for pattern, why in (
            (SHELL_FLAG, "a shell's command-string flag — spawn with an "
                         "argument vector instead"),
            (COMMAND_STRING, "a whole command line in one string — an API "
                             "value inside it becomes syntax"),
        ):
            for m in pattern.finditer(text):
                problems.append(f"{where}:{at(text, m.start())}  "
                                f"{m.group(0)} — {why}")
    return problems, (f"{spawners} file(s) spawn a process"
                      if spawners else "nothing in the tree spawns a process")


# --- 3. no telemetry, no second outbound path -----------------------------

# The path is captured only so the message names the whole address — a report
# that prints `https://github.com` for a URL three segments long tells the
# reader nothing about which line to open.
URL = re.compile(r"https?://(?P<host>[A-Za-z0-9.\-]+)[^\s\"'`)\\]*")


def check_outbound(files: list[Path], root: Path) -> tuple[list[str], str]:
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    problems: list[str] = []
    crates: list[str] = []
    for table in ("dependencies", "dev-dependencies", "build-dependencies"):
        for name in manifest.get(table, {}):
            crates.append(name)
            if name not in ALLOWED_CRATES:
                problems.append(f"Cargo.toml [{table}]  {name} — not one of the "
                                f"ten crates invariant 10 allows. A new "
                                f"dependency is a recorded decision: NOTES.md "
                                f"and CLAUDE.md first, this list with them")

    # Our own repository URL is display text (`main.rs` prints it) and is read
    # from the manifest rather than copied here, so it moves when the repo does.
    # Anchored at a boundary: a bare prefix test also exempts
    # `…/k8rs.attacker.example`, which is the whole class this check is for.
    home = manifest.get("package", {}).get("repository", "")
    own = re.compile(rf"{re.escape(home)}(?![\w.-])") if home else None
    lines = 0
    for path in files:
        text = code(path)
        lines += sum(1 for line in text.splitlines() if line.strip())
        for m in URL.finditer(text):
            if own and own.match(text, m.start()):
                continue
            if RESERVED_HOSTS.search(m.group("host")):
                continue
            problems.append(f"{rel(root, path)}:{at(text, m.start())}  "
                            f"{m.group(0)} — the only outbound connection is "
                            f"the API server in the user's kubeconfig")
    # The comment stripper blanks whole regions, so a scanner that broke reads
    # as a clean tree. Nothing but an empty repo has no code lines.
    if not lines:
        problems.append("no code left after stripping comments — the scanner "
                        "broke, and both code checks above just passed on air")
    return problems, f"{len(crates)} direct dependencies, {lines} code lines read"


# --- 4. token hygiene -----------------------------------------------------

# `#\[.*?\]` across newlines, not `[^\n]*`: rustfmt wraps a long derive list
# onto its own lines, and a one-line attribute pattern captures no attrs at all
# for exactly the structs whose derive list grew — which reads as "derives no
# Debug" and is the silent half of this check.
STRUCT = re.compile(
    r"(?P<attrs>(?:^[ \t]*#\[.*?\]\s*?\n)*)"
    r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?struct\s+(?P<name>\w+)"
    r"(?P<generics><[^{;(]*>)?\s*(?P<body>[{(])",
    re.M | re.S,
)
# To end of line, not to the next comma: `map: HashMap<String, Client>` splits on
# that comma, and the half the field name is on is the half without the token in
# it — an under-extraction that reads exactly like a clean tree.
FIELD = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?\w+\s*:\s*(?P<ty>.+)$", re.M)
DERIVES_DEBUG = re.compile(r"#\[derive\([^)]*\bDebug\b")


def balanced(text: str, open_at: int) -> str:
    pairs = {"{": "}", "(": ")"}
    close = pairs[text[open_at]]
    depth = 0
    for i in range(open_at, len(text)):
        if text[i] == text[open_at]:
            depth += 1
        elif text[i] == close:
            depth -= 1
            if depth == 0:
                return text[open_at + 1 : i]
    return text[open_at + 1 :]


def check_token_debug(files: list[Path], root: Path) -> tuple[list[str], str]:
    """A type that can hold a token may not *derive* Debug.

    Holding a `Client` is what `k8s.rs` is for, so the ban is on the derived
    `Debug` — the one that prints the auth info — and it follows a field into
    the struct that owns it: `App { k8s: K8s }` leaks exactly as well.
    """
    structs = {}  # name -> (where, field types, derives Debug)
    for path in files:
        text = code(path)
        for m in STRUCT.finditer(text):
            body = balanced(text, m.end("body") - 1)
            types = [f.group("ty") for f in FIELD.finditer(body)] \
                if m.group("body") == "{" else body.split(",")
            structs[m.group("name")] = (
                f"{rel(root, path)}:{at(text, m.end('attrs'))}",
                types,
                bool(DERIVES_DEBUG.search(m.group("attrs"))),
            )

    tainted = {n for n, (_, types, _) in structs.items()
               if any(re.search(rf"\b{t}\b", ty) for ty in types for t in TOKEN_TYPES)}
    # A field of a tainted type taints its owner, to a fixpoint: the leak is one
    # `{:?}` away however many structs deep the client sits.
    while True:
        grown = tainted | {
            n for n, (_, types, _) in structs.items()
            if any(re.search(rf"\b{t}\b", ty) for ty in types for t in tainted)
        }
        if grown == tainted:
            break
        tainted = grown

    problems = [
        f"{structs[n][0]}  struct {n} can hold a token and derives Debug — "
        f"write it by hand and print no auth info (§ Token hygiene)"
        for n in sorted(tainted) if structs[n][2]
    ]
    if not structs:
        problems.append("no struct found in the whole tree — the parser broke, "
                        "and this check just passed on nothing")
    return problems, f"{len(structs)} structs, {len(tainted)} can hold a token"


# --- 5 & 6. credentials and transport -------------------------------------

# Both are check 2's shape: the class is empty today, and the guard keeps it
# empty from the moment `kube` lands. Both ban *the call*, never the word — the
# difference matters most for TLS, where Phase 5 has to read a kubeconfig that
# sets `insecure-skip-tls-verify` and Phase 11 has to show it in the header.
# Reading the field and printing it is honouring the user's config; assigning it
# is us turning verification off, and only the second one is here.
BANNED_CALLS = {
    "credentials come from the kubeconfig": (
        re.compile(
            r"\b(?:Config\s*::\s*(?:incluster\w*|infer)"
            r"|Client\s*::\s*try_default"
            r"|incluster_(?:env|dns|config))\s*\("
        ),
        "there is no in-cluster ServiceAccount path and this opens one — "
        "credentials come from the kubeconfig current context and nowhere else "
        "(§ Identity and transport). Config::infer() and Client::try_default() "
        "try the in-cluster environment first; Config::from_kubeconfig does not",
    ),
    "TLS verification is never disabled by us": (
        re.compile(
            # `= true` and not merely `=`: assigning the flag *from the
            # kubeconfig's own parsed value* is how Phase 5 honours it, and
            # that is the one assignment that must stay writable.
            r"(?:\baccept_invalid_(?:certs|hostnames)\s*(?:=\s*true|:\s*true|\(\s*true)"
            r"|\bdanger\w*\s*\("  # covers dangerous() and danger_accept_*()
            r"|\bSslVerifyMode\s*::\s*NONE)"
        ),
        "this turns certificate verification off — honouring a kubeconfig that "
        "sets insecure-skip-tls-verify is the user's choice and is shown in the "
        "header; disabling it ourselves is never one (§ Identity and transport)",
    ),
}


def check_banned(files: list[Path], root: Path,
                 pattern: re.Pattern, why: str) -> tuple[list[str], str]:
    problems = [
        f"{rel(root, path)}:{at(text, m.start())}  {m.group(0).strip()} — {why}"
        for path in files
        for text in [code(path)]
        for m in pattern.finditer(text)
    ]
    # Never "the class is empty" on a red run: a note that contradicts the FAIL
    # above it is the line people believe.
    return problems, f"{len(problems)} call(s)" if problems else "the class is empty"


# --- the run --------------------------------------------------------------

def run(root: Path) -> dict[str, tuple[list[str], str]]:
    # Per run, not per process: the cache exists so six checks strip the tree
    # once, and `--self-test` rewrites the same paths between runs — a cache
    # that outlived a run would hand the next check the file before the plant.
    code.cache_clear()
    files = sources(root)
    return {
        "workflows": check_workflows(root),
        "no shell": check_shell(files, root),
        "no second outbound path": check_outbound(files, root),
        "token hygiene": check_token_debug(files, root),
        **{name: check_banned(files, root, pattern, why)
           for name, (pattern, why) in BANNED_CALLS.items()},
    }


def self_test() -> None:
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        fake = Path(tmp)
        wf = fake / ".github" / "workflows"
        wf.mkdir(parents=True)
        (fake / "src").mkdir()
        clean_yml = (
            "name: CI\n"
            "on: [push]\n"
            "permissions:\n"
            "  contents: read\n"
            "jobs:\n"
            "  a:\n"
            "    steps:\n"
            "      - uses: actions/checkout@" + "a" * 40 + "\n"
            "      - run: cargo test\n"
        )
        (wf / "ci.yml").write_text(clean_yml)
        (fake / "Cargo.toml").write_text(
            '[package]\nname = "k"\nversion = "0"\n'
            'repository = "https://github.com/murat-akpinar/k8rs"\n'
            '[dependencies]\nk8s-openapi = "0.28"\n'
        )
        (fake / "src" / "main.rs").write_text(
            '//! See https://kube.rs/docs — a citation, not a connection.\n'
            '/* https://k8s.io/docs is one too */\n'
            '#[derive(Debug)]\n'
            'struct Fine { name: String }\n'
            'fn main() {\n'
            '    println!("Progress: https://github.com/murat-akpinar/k8rs");\n'
            '    let q = "https://registry.invalid/v2/x"; let c = \'"\';\n'
            '}\n'
        )

        def only(check: str) -> list[str]:
            return run(fake)[check][0]

        # The negative half first, or every assertion below is a guard that
        # fails on everything. It also pins the three shapes that must NOT
        # fire: a URL in a line comment, one in a block comment, this repo's
        # own address, and a reserved-name host in a test double.
        assert not any(p for c in run(fake).values() for p in c[0]), run(fake)

        planted = {}

        # 1a — a tag instead of a SHA. The whole class: a tag is a pointer the
        # action's owner can move after review.
        (wf / "ci.yml").write_text(clean_yml.replace("@" + "a" * 40, "@v4"))
        planted["uses: a tag"] = only("workflows")
        assert any("pin to a 40-hex" in p for p in planted["uses: a tag"])

        # 1b — no top-level permissions at all.
        (wf / "ci.yml").write_text(clean_yml.replace("permissions:\n  contents: read\n", ""))
        planted["no permissions block"] = only("workflows")
        assert any("no top-level" in p for p in planted["no permissions block"])

        # 1c — …and one that grants write, which is the same hole with a block
        # present, so the check cannot be satisfied by the block's existence.
        (wf / "ci.yml").write_text(clean_yml.replace("contents: read", "contents: write"))
        planted["permissions: write"] = only("workflows")
        assert any("grant write" in p for p in planted["permissions: write"])

        # 1c' — the same grant written inline, which is a different branch of
        # the reader and the shortest thing a hurried CI fix types.
        (wf / "ci.yml").write_text(
            clean_yml.replace("permissions:\n  contents: read\n", "permissions: write-all\n"))
        planted["permissions: write-all, inline"] = only("workflows")
        assert any("grant write" in p for p in planted["permissions: write-all, inline"])

        # 1d — pull_request_target: a fork's code with this repo's secrets.
        (wf / "ci.yml").write_text(clean_yml.replace("on: [push]", "on: [pull_request_target]"))
        planted["pull_request_target"] = only("workflows")
        assert any("fork's code" in p for p in planted["pull_request_target"])

        # 1e — the canary itself: a scan that matches nothing must not pass.
        (wf / "ci.yml").write_text(clean_yml.replace("uses:", "step:"))
        planted["no uses: matched (canary)"] = only("workflows")
        assert any("canary moves" in p for p in planted["no uses: matched (canary)"])
        (wf / "ci.yml").write_text(clean_yml)

        # 1f — a *second* workflow, spelled .yaml. The release workflow Phase 13
        # adds is the obvious one, and a glob that reads only .yml would check
        # the file that was already clean and skip the new one entirely.
        (wf / "release.yaml").write_text(
            "name: R\non: [push]\njobs:\n  b:\n    steps:\n      - uses: some/action@v1\n")
        planted["a second workflow, spelled .yaml"] = only("workflows")
        assert any("release.yaml" in p for p in planted["a second workflow, spelled .yaml"])
        (wf / "release.yaml").unlink()

        # 1g — no workflow at all. The scan's own list is derived, so an empty
        # one has to be loud: the alternative is four checks reporting OK about
        # a directory they never found.
        (wf / "ci.yml").rename(wf / "ci.yml.bak")
        planted["no workflow file found (canary)"] = only("workflows")
        assert any("passed on an empty list" in p for p in planted["no workflow file found (canary)"])
        (wf / "ci.yml.bak").rename(wf / "ci.yml")

        # …and the negative that fix cost nothing to keep: a trailing comment is
        # not a grant, however many times it says the word.
        (wf / "ci.yml").write_text(
            clean_yml.replace("permissions:\n  contents: read\n",
                              "permissions: read-all  # writes go in the release job\n"))
        assert not only("workflows"), only("workflows")
        (wf / "ci.yml").write_text(clean_yml)

        # 2 — the three shell shapes. All in one file because the check is
        # scoped to files that spawn, and that scoping is what keeps the
        # command log's `kubectl logs -c app` out of it.
        (fake / "src" / "spawn.rs").write_text(
            'fn edit(p: &str) {\n'
            '    Command::new("bash").arg("-c").arg(p);\n'
            '    Command::new("x").arg("sh -c echo hi");\n'
            '}\n'
        )
        planted["shell spawn"] = only("no shell")
        assert len(planted["shell spawn"]) == 3, planted["shell spawn"]

        # …and the same tokens in a file that spawns nothing are display text.
        (fake / "src" / "spawn.rs").write_text(
            'const LOG: &str = "kubectl logs pod -c app";\n'
            'const EX: &str = "kubectl exec pod -- sh -c \'ls\'";\n'
        )
        assert not only("no shell"), only("no shell")

        # …and the shape v0.4 has to be allowed to write: a program that is not
        # a literal shell, with its arguments as a vector.
        (fake / "src" / "spawn.rs").write_text(
            'fn edit(editor: &str, path: &Path) {\n'
            '    Command::new(editor).arg(path).status();\n'
            '}\n'
        )
        assert not only("no shell"), only("no shell")
        (fake / "src" / "spawn.rs").unlink()

        # 3a — a crate outside the ten.
        cargo = (fake / "Cargo.toml").read_text()
        (fake / "Cargo.toml").write_text(cargo + 'reqwest = "0.12"\n')
        planted["dependency outside the ten"] = only("no second outbound path")
        assert any("reqwest" in p for p in planted["dependency outside the ten"])
        (fake / "Cargo.toml").write_text(cargo)

        # 3b — a hardcoded host in code. Written next to a `//` inside a string
        # so the plant also proves the stripper is not blanking from the first
        # slash pair it sees, which would hide the very line under test.
        main = (fake / "src" / "main.rs").read_text()
        (fake / "src" / "main.rs").write_text(
            main + 'const T: &str = "https://telemetry.k8rs.dev/collect";\n'
        )
        planted["hardcoded host"] = only("no second outbound path")
        assert any("telemetry.k8rs.dev" in p for p in planted["hardcoded host"])

        # 3c — a different address wearing ours as a prefix. `startswith` on the
        # repository URL waves every longer name through, and the exemption's
        # framing is the half that gets tested last (NOTES § D31).
        (fake / "src" / "main.rs").write_text(
            main + 'const T: &str = "https://github.com/murat-akpinar/k8rs-metrics/c";\n'
        )
        planted["an address wearing ours as a prefix"] = only("no second outbound path")
        assert planted["an address wearing ours as a prefix"], "k8rs-metrics is not k8rs"
        (fake / "src" / "main.rs").write_text(main)

        # 4 — a derived Debug over a type that can hold a token, and over the
        # struct that merely owns one, which is the leak a one-level check
        # misses.
        (fake / "src" / "k8s.rs").write_text(
            "pub struct K8s {\n"
            "    client: Client,\n"
            "    cfg: kube::Config,\n"
            "}\n"
            "#[derive(Clone, Debug)]\n"
            "pub struct App {\n"
            "    k8s: K8s,\n"
            "}\n"
        )
        planted["derived Debug over a token holder"] = only("token hygiene")
        assert planted["derived Debug over a token holder"], "App leaks through K8s"
        assert any("struct App" in p for p in planted["derived Debug over a token holder"])

        # …and the same field with a comma in its type, which is the shape a
        # field regex that stops at the next comma reads as `HashMap<String`.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Pool {\n"
            "    by_context: HashMap<String, Client>,\n"
            "}\n"
        )
        planted["a token behind a comma in the field type"] = only("token hygiene")
        assert any("struct Pool" in p for p in planted["a token behind a comma in the field type"])

        # …and the same derive after rustfmt wrapped the list, which is what a
        # long one looks like the moment a trait is added to it. Measured on
        # src/rules.rs: a one-line attribute pattern captures no attrs at all
        # here, so the struct reads as deriving nothing.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(\n"
            "    Clone,\n"
            "    Debug,\n"
            ")]\n"
            "pub struct Wrapped {\n"
            "    cfg: Config,\n"
            "}\n"
        )
        planted["a wrapped derive list rustfmt wrote"] = only("token hygiene")
        assert any("struct Wrapped" in p for p in planted["a wrapped derive list rustfmt wrote"])

        # …and holding one without deriving Debug is exactly what k8s.rs is for.
        (fake / "src" / "k8s.rs").write_text(
            "pub struct K8s { client: Client }\n"
            "impl std::fmt::Debug for K8s {\n"
            '    fn fmt(&self, f: &mut Formatter) -> Result { f.write_str("K8s") }\n'
            "}\n"
        )
        assert not only("token hygiene"), only("token hygiene")
        (fake / "src" / "k8s.rs").unlink()

        # 5 — every door into the in-cluster environment. `infer` and
        # `try_default` are here because they are the *convenient* ones: both
        # try the ServiceAccount first and only then the kubeconfig, so the
        # path opens without anybody typing the word "incluster".
        (fake / "src" / "k8s.rs").write_text(
            "async fn connect() -> Result<Client> {\n"
            "    let a = Config::incluster()?;\n"
            "    let b = Config::incluster_env()?;\n"
            "    let c = Config::infer().await?;\n"
            "    let d = Client::try_default().await?;\n"
            "}\n"
        )
        planted["a door into the in-cluster environment"] = \
            only("credentials come from the kubeconfig")
        assert len(planted["a door into the in-cluster environment"]) == 4, \
            planted["a door into the in-cluster environment"]

        # …and the one call that is the whole point: the kubeconfig's own
        # current context, which must stay writable.
        (fake / "src" / "k8s.rs").write_text(
            "async fn connect(opts: &KubeConfigOptions) -> Result<Config> {\n"
            "    Config::from_kubeconfig(opts).await\n"
            "}\n"
        )
        assert not only("credentials come from the kubeconfig"), \
            only("credentials come from the kubeconfig")

        # 6 — us turning verification off, in each spelling kube and its client
        # stack offer.
        (fake / "src" / "k8s.rs").write_text(
            "fn tls(cfg: &mut Config) {\n"
            "    cfg.accept_invalid_certs = true;\n"
            "    let c = Config { accept_invalid_certs: true, ..d };\n"
            "    let b = builder.danger_accept_invalid_certs(true);\n"
            "    ctx.set_verify(SslVerifyMode::NONE);\n"
            "    let r = rustls_cfg.dangerous();\n"
            "}\n"
        )
        planted["us switching certificate verification off"] = \
            only("TLS verification is never disabled by us")
        assert len(planted["us switching certificate verification off"]) == 5, \
            planted["us switching certificate verification off"]

        # …and the negative that has to survive to Phase 11, which is the whole
        # reason the ban is on the call and not on the word: a kubeconfig that
        # sets insecure-skip-tls-verify is read, carried and *shown*. If this
        # fires, the guard has made the header unimplementable.
        (fake / "src" / "k8s.rs").write_text(
            "fn header(cfg: &Config) -> Vec<&str> {\n"
            "    let mut out = vec![];\n"
            "    if cfg.accept_invalid_certs {\n"
            '        out.push("TLS verification is OFF for this cluster");\n'
            "    }\n"
            "    out\n"
            "}\n"
        )
        assert not only("TLS verification is never disabled by us"), \
            only("TLS verification is never disabled by us")
        (fake / "src" / "k8s.rs").unlink()

        # The tree is clean again — every plant above was undone rather than
        # left for the temp directory to swallow, which is what proves the
        # green above is a green and not a plant that happened to miss. The
        # real tree is never written at all: every check takes its root as an
        # argument, so nothing here can reach ROOT.
        assert not any(p for c in run(fake).values() for p in c[0]), run(fake)

    for name, hits in planted.items():
        print(f"  red on {name}:")
        for h in hits:
            print(f"      {h}")
    print(f"security-guard: self-test passed — {len(planted)} planted violations, "
          f"each seen red, and the clean tree green before and after")


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
        sys.exit(0)

    results = run(ROOT)
    failed = False
    for name, (problems, note) in results.items():
        for line in problems:
            print(f"FAIL [{name}] {line}", file=sys.stderr)
        failed = failed or bool(problems)
        print(f"security-guard: {name} — {note or 'nothing to report'}"
              f"{' — OK' if not problems else f' — {len(problems)} problem(s)'}")

    # write-guard.py's pattern: the surface this check contains does not exist
    # yet, and saying so is the difference between a gate that is waiting and a
    # gate that is vacuous.
    if "kube" not in tomllib.loads(
        (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    ).get("dependencies", {}):
        print("security-guard: kube is not a dependency yet — no Config and no "
              "Client exist to leak through a derived Debug. The check arrives "
              "with the client (Phase 5); its logic is proven by --self-test "
              "until then.")
    sys.exit(1 if failed else 0)
