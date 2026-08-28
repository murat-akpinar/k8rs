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
2. **No API string ever reaches a shell** (invariant 9). Two files spawn today,
   each through an argument vector: `tests/binary.rs` runs the built binary
   (`CARGO_BIN_EXE_k8rs`), and `src/k8s_tests.rs` runs `openssl` on literals and
   temp paths to build the CA and leaf its TLS server needs. The rule does not
   soften for a caller, and will not for `$EDITOR` in v0.4 — in every file that
   spawns at all it refuses a shell program, a `-c` flag and a command string.
3. **No telemetry, no second outbound path** — the dependency list is what
   invariant 10 allows and nothing else (an allowlist, because a
   denylist of HTTP crates would need to know about `reqwest`, `hyper`,
   `ureq`, `isahc`, `attohttpc`… and whatever ships next month), and no
   hardcoded host in the code. Both halves are narrower than the name: the
   allowlist reads the three dependency tables of `Cargo.toml`, and the code
   scan matches a *literal* `https?://host`. A host that is assembled
   (`format!("https://{h}/x")`) or handed to us at runtime is not visible to
   either, and neither is the kubeconfig's own API server — this check never
   sees the one allowed path, it only refuses the ways a second one is
   usually typed.
4. **Token hygiene** — a type that can hold a kube `Config`/`Client`, or a
   kube error, may not *derive* `Debug`, which prints the auth info. The gate's
   line is "no `Debug` is **derived** over a type that can hold config"
   (CLAUDE.md § Security gate), and the fix is to drop the derive rather than
   wrap it: an impl leaves `{:?}` compiling forever and has to be kept correct
   by whoever adds the next field, while no impl turns a stray `{:?}` into a
   compile error — and a hand-written one is in this check's blind spot either
   way (NOTES § D164). The taint follows a field type into the type that owns
   it, to a fixpoint, through `struct` fields, `enum` variant payloads and
   `type` aliases alike, so it catches a foreign type without reading kube.
   **This check is a *derived* `Debug` on a declaration the scan parses, and
   nothing else** — the summary line says so on every run, because a count
   beside the word OK is read as coverage. Outside it, all of it hand-checked
   against `docs/security.md § Token hygiene`: every `{}` / `{:?}` /
   `.to_string()` / `{:#}` **call** on a kube error or a `Config` (this is the
   shape NOTES § D162 measured a bearer token coming out of — `Display`
   interpolates its source down to an `exec` plugin's stdout, and `anyhow` is
   approved but unused, so the whole `?`-then-print path is still ahead);
   a hand-written `Debug` that formats one whole — the last resort the FAIL
   text names, and names as unverifiable, for this reason; a renamed import
   (`use kube::Client as Kc;`); a generic default (`struct Conn<C = Client>`);
   a generic parameter never named in a field; and an unqualified `Error`
   import, which cannot be told from `anyhow::Error` without resolving imports.
   What the check *can* be trusted about is bounded by one number it does not
   derive from itself: a keyword count of `struct`/`enum` that must agree with
   how many declarations the parser reached, and FAILs loudly when it does not.

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

# The twelve of invariant 10 (CLAUDE.md § Hard invariants). A thirteenth is a
# recorded decision, so it lands here and in CLAUDE.md in the same change —
# which is exactly the review this line exists to force. It forced both
# reversals of "no new dependencies", and each is a crate the build already
# linked, so it names something rather than adding compiled code:
# `futures-util` for `Stream`, which `kube-runtime` returns and `std` does not
# have (NOTES § D143), and `tokio-rustls` for the connector C2 needs to drive
# its own handshake and read the API server's certificate (NOTES § D178).
ALLOWED_CRATES = {
    "kube", "k8s-openapi", "ratatui", "crossterm", "tokio",
    "anyhow", "serde_json", "serde_yaml_ng", "x509-parser", "similar",
    "futures-util", "tokio-rustls",
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

# Types that carry, or can carry, a bearer token / client certificate. Each
# entry is a regex fragment, prefixed with `\b` below, because the right-hand
# boundary is not the same on both ends:
#   * `Client` has none — `\bClient\b` does not match `ClientBuilder`, which is
#     the type holding the config it is one call away from building from.
#   * `Config` keeps one, or `ConfigMap` taints everything that names it, and
#     that browser kind is in this tree today (src/k8s.rs, src/rules.rs). It
#     also refuses `watcher::Config`, which is a page size and a label selector
#     and carries no credential at all — named in comments here today, and a
#     field of the driver's the moment the client lands, at which point a FAIL
#     naming a token that is not there is what teaches people to edit the guard.
# The kube **error** family is here for the reason docs/security.md § Token
# hygiene measured (NOTES § D162): a kube error's `Display` interpolates its
# source at every hop down to `AuthError::AuthExecRun`, whose `{out:?}` over a
# `std::process::Output` prints an exec credential plugin's stdout — the
# ExecCredential JSON, token included. Qualified spellings **only**: a bare
# `Error` would match `anyhow::Error` and every `serde_json::Error` in the
# tree, so `use kube_runtime::watcher::Error;` followed by `Option<Error>` is
# a miss this scan cannot close and says so in its summary line.
TOKEN_TYPES = (
    r"(?<!watcher::)Config\b", r"Client", r"AuthInfo\b", r"ExecConfig\b",
    r"TokenFile\b",
    r"kube\s*::\s*Error\b", r"kube_client\s*::\s*Error\b",
    r"kube_runtime\s*::[\s\w:]*Error\b", r"watcher\s*::\s*Error\b",
    r"AuthError\b", r"KubeError\b",
)
TOKEN_TYPE = re.compile("|".join(rf"\b(?:{t})" for t in TOKEN_TYPES))

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


def strip_comments(text: str, strings: bool = False) -> str:
    """`text` with every comment blanked to spaces, offsets preserved.

    String-aware in both directions, and that is the whole difficulty: a URL is
    written `https://…`, so a scanner that blanks from the first `//` hides
    every line that mentions one (the hole `write-guard.py`'s own stripper
    documents), while a scanner that ignores strings reads `"// not a comment"`
    as code. Blanking rather than deleting keeps `count('\\n')` a line number.

    `strings=True` **also** blanks the inside of every string and char literal,
    and it is off by default because two checks need the opposite: the outbound
    scan matches a literal `https://host` and the shell scan matches
    `Command::new("x").arg("sh -c …")`, both inside string literals and both
    proven by plants in `--self-test`. Only the token check asks for it — its
    two bracket matchers count `{`, `}` and `]`, and a `#[serde(rename = "}")]`
    on a field truncated the struct body so the `Client` after it left the field
    list, with the struct still counted
    (reports/2026-08-27-token-hygiene-guard-shape-probe.md).
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
            if strings:
                blank(m.end(), n if j < 0 else j)
            i = n if j < 0 else j + len(close)
        elif c == '"':
            opened = i
            i += 1
            while i < n:
                if text[i] == "\\":
                    i += 2
                    continue
                if text[i] == '"':
                    i += 1
                    break
                i += 1
            if strings:
                blank(opened + 1, min(i, n) - 1)
        elif c == "'" and (m := CHAR_LIT.match(text, i)):
            if strings:
                blank(m.start() + 1, m.end() - 1)
            i = m.end()
        else:
            i += 1
    return "".join(out)


@lru_cache(maxsize=None)
def code(path: Path) -> str:
    """Stripped source, cached: six checks read the same 22k lines."""
    return strip_comments(path.read_text(encoding="utf-8", errors="replace"))


@lru_cache(maxsize=None)
def code_no_strings(path: Path) -> str:
    """The same, with string and char literals hollowed out — the token check's
    view, and only its (see `strip_comments`)."""
    return strip_comments(path.read_text(encoding="utf-8", errors="replace"),
                          strings=True)


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
                problems.append(f"Cargo.toml [{table}]  {name} — not one of "
                                f"the {len(ALLOWED_CRATES)} crates invariant 10 "
                                f"allows. A new dependency is a recorded "
                                f"decision: NOTES.md and CLAUDE.md first, this "
                                f"list with them")

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

# `struct` and `enum` are one shape here, and that is the point of this pattern:
# `enum Conn { Up(kube::Client), Down }` leaks through a derived `Debug` exactly
# as well as the struct that would have wrapped it, and a connection-state enum
# is the natural shape for the code that builds the client. A scan that read
# `struct` only went green on it without having looked (todo.md Phase 5,
# NOTES § D141).
#
# The attribute pattern spans newlines, because rustfmt wraps a long derive list
# onto its own lines and a one-line pattern captures no attrs at all for exactly
# the types whose derive list grew — which reads as "derives no Debug" and is the
# silent half of this check. It is **bracket-balanced** rather than `#\[.*?\]`,
# and that is not tidying: under `re.S` the lazy `.*?` gets backtracked forward
# through the file hunting a `]` that a declaration follows, and everything it
# crosses ends up inside one match and is never scanned. It was happening here —
# `struct Mounters` (src/analysis.rs:2176) matched with an `attrs` group that
# began at line 172, and five declarations in between vanished with it:
# `Watch` (k8s.rs:585, which holds a `watcher::Error`), `NodeLine`, `DrainLine`,
# `NotCaughtUp` and `Blocked`. Measured on this tree, 2026-08-27: the old pattern
# matched 44 structs, this one matches 49.
#
# Whitespace-only lines are allowed *between* the attributes and the declaration:
# `strip_comments` blanks a doc comment to spaces rather than deleting it, so an
# attribute with a `///` under it is separated by a line of spaces, and a pattern
# that demanded adjacency would read that type as deriving nothing. The final
# newline is optional so that `#[derive(Debug)] pub struct S { … }` on one line
# is parsed — rustfmt splits that shape, so `cargo fmt --check` normally removes
# it before this guard runs, but a guard that depends on another step having run
# first is a guard with a precondition nobody states.
ATTRS = r"(?:^[ \t]*#\[(?:[^\[\]]|\[[^\[\]]*\])*\][ \t]*(?:\n(?:[ \t]*\n)*)?)*"
# Line start, or straight after an attribute's `]` on the same line — the second
# branch is only for `#[derive(Debug)] pub struct S { … }`, and it matches
# nothing in this tree today (measured 2026-08-27). What makes an unanchored
# reader safe at all here is `code_no_strings`: a `struct`/`enum` keyword inside
# a string literal is blanked before this runs, which is worth one hit — the
# unanchored keyword count is 63 with strings kept and 62 with them blanked, the
# odd one being prose inside a test assertion at src/rules_tests/pod.rs:3680.
ANCHOR = r"(?:^|(?<=\]))[ \t]*(?:pub(?:\([^)]*\))?\s+)?"
DECL = re.compile(rf"(?P<attrs>{ATTRS}){ANCHOR}(?P<kw>struct|enum)\s+(?P<name>\w+)",
                  re.M)
# The denominator, and it must not come from `DECL`: a count derived from the
# regex being checked reads as coverage and cannot reveal that regex's own miss —
# the same disease as the empty-tree canary below (PRIOR-ART § F2). This one
# counts the keyword and stops. It is the tripwire for a `DECL` that stops
# *matching*; a `DECL` that matches but cannot find the body is the `lost` list
# further down, and the two together are what "parsed" in the summary means.
# Equal on this tree (62 = 49 + 13), and a disagreement is a FAIL, not a note.
NAIVE_DECL = re.compile(rf"{ANCHOR}(?:struct|enum)\s+\w+", re.M)
# An alias declares no fields, so the fixpoint below could not walk through one:
# `type Handle = kube::Client;` made every holder of a `Handle` clean.
ALIAS = re.compile(
    r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?type\s+(?P<name>\w+)"
    r"(?:<[^=;]*>)?\s*=\s*(?P<ty>[^;]+);",
    re.M,
)
# To end of line, not to the next comma: `map: HashMap<String, Client>` splits on
# that comma, and the half the field name is on is the half without the token in
# it — an under-extraction that reads exactly like a clean tree.
FIELD = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?\w+\s*:\s*(?P<ty>.+)$", re.M)
# `\bderive(` and not `#\[derive(`, so the gated forms are seen:
# `#[cfg_attr(test, derive(Debug))]` is the most likely way a connection type in
# *this* repo gets a `Debug` — the tests want to assert on it and the product
# does not need it — and `#![cfg_attr(…)]` is already in the tree (src/k8s.rs:100,
# src/analysis.rs:59). The narrow form parsed the declaration, tainted it, and
# printed no FAIL beside a tainted count that had gone up
# (reports/2026-08-27-token-hygiene-guard-shape-probe.md).
DERIVES_DEBUG = re.compile(r"\bderive\([^)]*\bDebug\b")


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


def body_at(text: str, i: int) -> int | None:
    """Index of a declaration's body opener, scanning from just after its name.

    `-1` is `struct Foo;`, which holds nothing and is a real answer. `None` is
    *this scan lost it* — and the two must not be one value, because a lost
    declaration otherwise reads as a type with no fields, which is a clean type.

    A scan and not part of `DECL`, because what sits between the name and the
    body is not regular. `struct S<F: Fn(&str) -> bool>` puts parens and a `>`
    inside the generics, and rustfmt **canonicalises** an inline `where` into

        pub struct S<T>
        where
            T: Clone,
        {

    so the pattern that demanded `<…>` then `{` missed a shape `cargo fmt` in
    `just check` actively produces — the gate installing the blind spot rather
    than closing it (reports/2026-08-27-token-hygiene-guard-shape-probe.md).
    """
    n = len(text)
    depth = 0
    while i < n:
        c = text[i]
        if text[i : i + 2] == "->":
            i += 2  # a return arrow inside a bound, not a closing angle
        elif c == "<":
            depth += 1
            i += 1
        elif c == ">":
            depth = max(0, depth - 1)
            i += 1
        elif c == "(":
            if depth == 0:
                return i  # a tuple struct's body
            i += len(balanced(text, i)) + 2  # a bound such as `Fn(&str)`
        elif c == "{":
            return i if depth == 0 else None
        elif c == ";":
            return -1  # a unit struct; a tuple struct returned at its `(`
        else:
            i += 1
    return None


def payloads(body: str) -> list[str]:
    """Every variant payload in an enum body — a tuple variant's `(…)` and a
    struct variant's `{…}`, nesting included.

    Variant *names* are left out deliberately: a unit variant holds nothing, and
    `k8s.rs`'s `Capability` is seven of them. Reading the body whole would taint
    any enum with a variant merely *named* after a token type, and a guard that
    fires on a name nobody can hold is a guard people start editing around.
    """
    out, i = [], 0
    while i < len(body):
        if body[i] in "({":
            inner = balanced(body, i)
            out.append(inner)
            i += len(inner) + 2
        else:
            i += 1
    return out


def check_token_debug(files: list[Path], root: Path) -> tuple[list[str], str]:
    """A type that can hold a token may not *derive* Debug.

    Holding a `Client` is what `k8s.rs` is for, so the ban is on the derived
    `Debug` — the one that prints the auth info — and it follows a field into
    the type that owns it: `App { k8s: K8s }` leaks exactly as well.

    **What it does not cover is in the summary line, and that is the point.**
    A regex has no types, so the whole `Display` half — `format!("{e}")`,
    `.to_string()`, `{:#}` on an `anyhow` chain — and every `{:?}` on a bare
    local are outside it, and those are the shapes NOTES § D162 measured a
    token coming out of. They are hand-checked against
    docs/security.md § Token hygiene.
    """
    # Keyed by *name*, because that is the only thing the fixpoint below can
    # match a field type against — a regex has no name resolution. Every
    # declaration of a name is kept as its own site rather than overwriting or
    # merging into one: an assignment silently dropped a declaration whole, and a
    # merge that kept only the first site printed the wrong file and line — the
    # tree already collides once (`Row`: analysis.rs:173 enum, k8s.rs:2722
    # struct), and a FAIL naming a line with no `derive` on it is how a guard
    # earns a suppression at 3am.
    decls: dict[str, list[tuple[str, str, list[str], bool]]] = {}
    kinds = {"struct": 0, "enum": 0, "type": 0}
    naive = 0
    lost: list[str] = []

    def record(name: str, kw: str, where: str, held: list[str], dbg: bool) -> None:
        kinds[kw] += 1
        decls.setdefault(name, []).append((kw, where, held, dbg))

    for path in files:
        # Strings hollowed out for this check alone: `balanced` and `ATTRS` count
        # brackets, and a `#[serde(rename = "}")]` on a field truncated the body
        # so the `Client` below it was never in the field list — with the struct
        # still counted, so the cross-check could not see it either.
        text = code_no_strings(path)
        naive += sum(1 for _ in NAIVE_DECL.finditer(text))
        for m in DECL.finditer(text):
            open_at = body_at(text, m.end())
            if open_at is None:
                lost.append(f"{rel(root, path)}:{at(text, m.start('kw'))}")
                held = []
            elif open_at < 0:
                held = []  # `struct Foo;` holds nothing, and still counts
            else:
                body = balanced(text, open_at)
                if m.group("kw") == "enum":
                    held = payloads(body)
                elif text[open_at] == "{":
                    held = [f.group("ty") for f in FIELD.finditer(body)]
                else:
                    held = body.split(",")
            record(m.group("name"), m.group("kw"),
                   f"{rel(root, path)}:{at(text, m.end('attrs'))}", held,
                   bool(DERIVES_DEBUG.search(m.group("attrs"))))
        # An alias is a propagation node and never a report: it derives nothing.
        for m in ALIAS.finditer(text):
            record(m.group("name"), "type",
                   f"{rel(root, path)}:{at(text, m.start())}",
                   [m.group("ty")], False)

    def holds(name: str, pattern: re.Pattern) -> bool:
        return any(pattern.search(ty) for _, _, held, _ in decls[name] for ty in held)

    tainted = {n for n in decls if holds(n, TOKEN_TYPE)}
    # A field of a tainted type taints its owner, to a fixpoint: the leak is one
    # `{:?}` away however many types deep the client sits.
    while True:
        grown = tainted | {
            n for n in decls
            if any(holds(n, re.compile(rf"\b{t}\b")) for t in tainted)
        }
        if grown == tainted:
            break
        tainted = grown

    problems = []
    for n in sorted(tainted):
        sites = decls[n]
        elsewhere = ""
        if len(sites) > 1:
            elsewhere = (f" — and {n} is declared {len(sites)} times "
                         f"({', '.join(w for _, w, _, _ in sites)}); this scan "
                         f"matches field types by name, so the token may be in "
                         f"one of the others")
        for kw, where, _, dbg in sites:
            if dbg:
                problems.append(
                    f"{where}  {kw} {n} can hold a token and derives Debug — "
                    f"remove the derive. A `{{:?}}` on this prints a bearer "
                    f"token; with no Debug at all, a stray `{{:?}}` is a "
                    f"compile error instead. Only if something truly needs "
                    f"Debug, write one by hand that selects fields — and this "
                    f"check cannot tell whether a hand-written one leaks "
                    f"(§ Token hygiene){elsewhere}")
    # Not "no struct": a tree whose only declarations are enums is a legitimate
    # shape, and a canary that fires on it gets read as the rule above catching
    # something. This one fires only when nothing at all was parsed.
    if not decls:
        problems.append("no struct, enum or type alias found in the whole tree — "
                        "the parser broke, and this check just passed on nothing")
    parsed = kinds["struct"] + kinds["enum"]
    # The other half, and it is the one the reviewer's `where`-clause plant now
    # lands in: `DECL` matches the keyword, so a body it cannot follow keeps the
    # two counts equal while the type is scanned for neither a token nor a
    # derive. A declaration with no readable body is never a pass.
    for where in lost:
        problems.append(
            f"{where}  this declaration's body could not be found, so it was "
            f"scanned for neither a token nor a derive — and an unreadable body "
            f"reads exactly like a type with no fields. Teach `body_at` the "
            f"shape rather than letting it answer nothing")
    if parsed != naive:
        problems.append(
            f"the keyword count found {naive} struct/enum declarations and the "
            f"parser reached {parsed}, a gap of {naive - parsed} — those were "
            f"scanned for neither a token nor a derive, and the count in the "
            f"summary cannot show it because that count comes from the same "
            f"regex. Find the shape (a `where` clause, a bound with a paren, a "
            f"macro) and teach `DECL` about it")
    return problems, (
        f"{kinds['struct']} structs, {kinds['enum']} enums, {kinds['type']} "
        f"aliases ({parsed} of {naive} declarations parsed), {len(tainted)} can "
        f"hold a token — and this check is a *derived* `Debug` on a declaration "
        f"it parses, nothing more: a `{{}}`/`{{:?}}`/`.to_string()` on a kube "
        f"error or a Config, a hand-written Debug that formats one whole, a "
        f"`use kube::Client as Kc` rename, a generic default or a generic never "
        f"named in a field, and an unqualified `Error` import are all outside a "
        f"regex and are hand-checked against docs/security.md § Token hygiene"
    )


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
    code_no_strings.cache_clear()
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

        # 3a — a crate outside the allowlist.
        cargo = (fake / "Cargo.toml").read_text()
        (fake / "Cargo.toml").write_text(cargo + 'reqwest = "0.12"\n')
        planted["a dependency outside the allowlist"] = only("no second outbound path")
        assert any("reqwest" in p for p in planted["a dependency outside the allowlist"])
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

        # 4e — a token in an *enum variant*, and the struct that merely owns the
        # enum. `struct`-only was the whole of this scan until 2026-08-27, so a
        # connection-state enum — the natural shape for the code that builds the
        # client — went green without having been looked at (NOTES § D141).
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub enum Conn {\n"
            "    Up(kube::Client),\n"
            "    Down,\n"
            "}\n"
            "#[derive(Debug)]\n"
            "pub struct Owner {\n"
            "    conn: Conn,\n"
            "}\n"
        )
        planted["a token in an enum variant"] = only("token hygiene")
        for want in ("enum Conn can hold a token", "struct Owner can hold a token"):
            assert any(want in p for p in planted["a token in an enum variant"]), \
                (want, planted["a token in an enum variant"])

        # 4f — the *other* half of an enum body. A payload scan that reads only
        # `(…)` sees a struct variant as a unit one, which is a clean tree again.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub enum Session {\n"
            "    Live { client: Client, since: Instant },\n"
            "    Cold,\n"
            "}\n"
        )
        planted["a token in a struct variant"] = only("token hygiene")
        assert any("enum Session can hold a token" in p
                   for p in planted["a token in a struct variant"]), \
            planted["a token in a struct variant"]

        # 4g — behind a `type` alias, the hop the fixpoint could not walk because
        # an alias declares no fields.
        (fake / "src" / "k8s.rs").write_text(
            "type Handle = kube::Client;\n"
            "#[derive(Debug)]\n"
            "pub struct Aliased {\n"
            "    h: Handle,\n"
            "}\n"
        )
        planted["a token behind a type alias"] = only("token hygiene")
        assert any("struct Aliased can hold a token" in p
                   for p in planted["a token behind a type alias"]), \
            planted["a token behind a type alias"]

        # 4h — `ClientBuilder`. `\bClient\b` has no match inside it, so the type
        # holding the config it is one call away from building from was invisible
        # (NOTES § D31: the framing, not only the value).
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Wiring {\n"
            "    inner: ClientBuilder,\n"
            "}\n"
        )
        planted["a ClientBuilder, which has no word boundary before B"] = \
            only("token hygiene")
        assert any("struct Wiring can hold a token" in p for p in
                   planted["a ClientBuilder, which has no word boundary before B"]), \
            planted["a ClientBuilder, which has no word boundary before B"]

        # 4i — the foreign type that matters. `watcher::Error`'s `Display`
        # interpolates its source down to `AuthError::AuthExecRun`, whose
        # `{out:?}` over a `std::process::Output` prints an exec credential
        # plugin's stdout — the ExecCredential JSON, token included
        # (docs/security.md § Token hygiene, NOTES § D162). This is the live
        # shape: src/k8s.rs:840's `Trouble` is written exactly like this.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Trouble<'a> {\n"
            "    pub kind: ObjectKind,\n"
            "    pub failure: Option<&'a watcher::Error>,\n"
            "}\n"
        )
        planted["a kube watcher::Error, whose Display carries a token"] = \
            only("token hygiene")
        assert any("struct Trouble can hold a token" in p for p in
                   planted["a kube watcher::Error, whose Display carries a token"]), \
            planted["a kube watcher::Error, whose Display carries a token"]

        # 4j — two files, one name. This repo is flat, so two product files may
        # each declare a `Row` and this tree already does (analysis.rs:173,
        # k8s.rs:2723). The dict is keyed by name because the fixpoint has no
        # name resolution, so an assignment drops one declaration whole — and
        # `sources()` is alphabetical, which makes *which* one is dropped an
        # accident of the filename. The tainted half must survive either order.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Row {\n"
            "    client: Client,\n"
            "}\n"
        )
        (fake / "src" / "rules.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Row {\n"
            "    label: String,\n"
            "}\n"
        )
        planted["a collided name whose tainted half must not be dropped"] = \
            only("token hygiene")
        assert any("struct Row can hold a token" in p for p in
                   planted["a collided name whose tainted half must not be dropped"]), \
            planted["a collided name whose tainted half must not be dropped"]
        (fake / "src" / "rules.rs").unlink()

        # 4k — the canary, and it had to be re-thought for this box: it used to
        # read "no *struct* found", so a plant written into a file of pure enums
        # fired the canary instead of the rule, and a canary line reads exactly
        # like a catch. A tree that declares only enums is now a legitimate one.
        (fake / "src" / "main.rs").write_text("fn main() {}\n")
        (fake / "src" / "k8s.rs").write_text("pub enum Only { A, B }\n")
        assert not only("token hygiene"), only("token hygiene")
        (fake / "src" / "k8s.rs").write_text("fn nothing_at_all() {}\n")
        planted["nothing declares a type at all (canary)"] = only("token hygiene")
        assert any("no struct, enum or type alias found" in p
                   for p in planted["nothing declares a type at all (canary)"]), \
            planted["nothing declares a type at all (canary)"]
        (fake / "src" / "main.rs").write_text(main)

        # 4l — a declaration **swallowed by the scan's own attribute pattern**.
        # `#\[.*?\]` under `re.S` backtracks its lazy `.*?` forward through the
        # file looking for a `]` that a struct follows, and everything it crosses
        # is inside one match and never scanned. Reduced from src/analysis.rs,
        # where `struct Mounters` (line 2176) matched with an `attrs` group that
        # began at line 172 and four structs vanished between them.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Clone, Debug, PartialEq, Eq)]\n"
            "pub enum Row {\n"
            "    A,\n"
            "    B,\n"
            "}\n"
            "\n"
            "fn helper(v: &[u8]) -> bool {\n"
            "    v[0] == 1\n"
            "}\n"
            "\n"
            "struct Hidden {\n"
            "    client: Client,\n"
            "}\n"
            "\n"
            "#[derive(Debug)]\n"
            "struct Mounters {\n"
            "    inner: Hidden,\n"
            "}\n"
        )
        planted["a declaration swallowed by the attribute pattern"] = \
            only("token hygiene")
        assert any("struct Mounters can hold a token" in p for p in
                   planted["a declaration swallowed by the attribute pattern"]), \
            planted["a declaration swallowed by the attribute pattern"]

        # 4m — an attribute, a doc comment, then the declaration. Not a shape
        # this box widened the scan to reach: it is the one the bracket-balanced
        # `ATTRS` above could have *lost*. `strip_comments` blanks a `///` to
        # spaces rather than deleting it, so the attribute and the struct are not
        # adjacent lines, and a pattern demanding adjacency reads this type as
        # deriving nothing at all.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "/// What this holds.\n"
            "pub struct Docced {\n"
            "    client: Client,\n"
            "}\n"
        )
        planted["an attribute separated from its struct by a doc comment"] = \
            only("token hygiene")
        assert any("struct Docced can hold a token" in p for p in
                   planted["an attribute separated from its struct by a doc comment"]), \
            planted["an attribute separated from its struct by a doc comment"]

        # Everything below came out of the operator review of the block above
        # (reports/2026-08-27-token-hygiene-guard-shape-probe.md), which fed the
        # check one declaration shape at a time. Each of these walked straight
        # through the version of the scan the plants above were written for.

        # 4n — a `where` clause, and this is the one that made the review
        # blocking rather than academic: `just check` runs `cargo fmt --check`
        # first, and rustfmt *canonicalises* an inline `where` into exactly this
        # multi-line form. The gate was installing the blind spot.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Session<T>\n"
            "where\n"
            "    T: Clone,\n"
            "{\n"
            "    client: kube::Client,\n"
            "    tag: T,\n"
            "}\n"
        )
        planted["a where clause between the generics and the body"] = \
            only("token hygiene")
        assert any("struct Session can hold a token" in p for p in
                   planted["a where clause between the generics and the body"]), \
            planted["a where clause between the generics and the body"]

        # 4o — a bound with a paren in it. The generics group was `<[^{;(]*>`,
        # so `Fn(&str) -> bool` ended the pattern before the body.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Session<F: Fn(&str) -> bool> {\n"
            "    client: kube::Client,\n"
            "    pred: F,\n"
            "}\n"
        )
        planted["a generic bound containing a paren"] = only("token hygiene")
        assert any("struct Session can hold a token" in p
                   for p in planted["a generic bound containing a paren"]), \
            planted["a generic bound containing a paren"]

        # 4p — derive and declaration on one line. rustfmt splits this, so
        # `cargo fmt --check` normally removes it before the guard runs; a guard
        # that is only correct because an earlier step ran is a guard with an
        # unstated precondition.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)] pub struct SameLine { client: kube::Client }\n"
        )
        planted["a derive and its declaration on one line"] = only("token hygiene")
        assert any("struct SameLine can hold a token" in p
                   for p in planted["a derive and its declaration on one line"]), \
            planted["a derive and its declaration on one line"]

        # 4q — a body the scan cannot follow. This one is a hazard `body_at`
        # *introduced* and has to answer for: `DECL` now stops at the name, so a
        # declaration whose body cannot be found would be recorded with an empty
        # field list — indistinguishable from a type that holds nothing, and
        # invisible to the keyword count, which matched it perfectly well. A
        # missing body is a FAIL, never a zero.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Dangling<T\n"
        )
        planted["a declaration whose body could not be found"] = \
            only("token hygiene")
        assert any("body could not be found" in p
                   for p in planted["a declaration whose body could not be found"]), \
            planted["a declaration whose body could not be found"]

        # 4r — a gated derive, in both spellings. `#[cfg_attr(test,
        # derive(Debug))]` is the most likely way a connection type in *this*
        # repo gets one, and the narrow `#\[derive\(` parsed the type, tainted
        # it, and printed no FAIL beside a tainted count that had gone up.
        (fake / "src" / "k8s.rs").write_text(
            "#[cfg_attr(test, derive(Debug))]\n"
            "pub struct Gated { client: kube::Client }\n"
            "#[cfg_attr(feature = \"dbg\", derive(Debug))]\n"
            "pub struct Featured { client: kube::Client }\n"
        )
        planted["a derive behind cfg_attr"] = only("token hygiene")
        for want in ("struct Gated can hold a token", "struct Featured can hold a token"):
            assert any(want in p for p in planted["a derive behind cfg_attr"]), \
                (want, planted["a derive behind cfg_attr"])

        # 4s — a bracket inside a *string* in an attribute, plain and raw. The
        # bracket matchers count characters; `strip_comments` skipped string
        # literals rather than blanking them, so `"press ] to close"` closed the
        # attribute early and the derive was never in `attrs`.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "#[doc = \"press ] to close\"]\n"
            "pub struct Docs { client: kube::Client }\n"
            "#[derive(Debug)]\n"
            "#[doc = r\"a ] b\"]\n"
            "pub struct Raw { client: kube::Client }\n"
        )
        planted["a ] inside a string in an attribute"] = only("token hygiene")
        for want in ("struct Docs can hold a token", "struct Raw can hold a token"):
            assert any(want in p for p in planted["a ] inside a string in an attribute"]), \
                (want, planted["a ] inside a string in an attribute"])

        # 4t — and the same cause in the body: a `}` inside a field attribute's
        # string truncated the struct, so the `Client` under it was not in the
        # field list — with the struct still *counted*, which is why no count
        # could have revealed this one.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug, Clone)]\n"
            "pub struct Neighbour { name: String }\n"
            "\n"
            "#[derive(Debug)]\n"
            "pub struct Conn {\n"
            "    #[serde(rename = \"}\")]\n"
            "    tag: String,\n"
            "    client: kube::Client,\n"
            "}\n"
        )
        planted["a } inside a string in a field attribute"] = only("token hygiene")
        assert any("struct Conn can hold a token" in p
                   for p in planted["a } inside a string in a field attribute"]), \
            planted["a } inside a string in a field attribute"]

        # 4u — the collided name again, and this time what the *message* says.
        # Merging into the first site printed `analysis.rs:1` for a derive that
        # is in `k8s.rs` — nonsense on the line it names at 3am. Every site is
        # kept: the FAIL is on the one that derives, and it names the others,
        # because a name-keyed scan genuinely cannot tell which `Session` a
        # field of type `Session` meant.
        (fake / "src" / "analysis.rs").write_text(
            "pub struct Session { label: String }\n"
        )
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Session { client: kube::Client }\n"
        )
        planted["a collided name, reported on the site that derives Debug"] = \
            only("token hygiene")
        assert any(p.startswith("src/k8s.rs:2  struct Session can hold a token")
                   and "src/analysis.rs:1" in p
                   for p in planted["a collided name, reported on the site that derives Debug"]), \
            planted["a collided name, reported on the site that derives Debug"]
        (fake / "src" / "analysis.rs").unlink()

        # 4v — the denominator, proved by breaking the numerator. `NAIVE_DECL`
        # exists so a `DECL` that stops matching cannot report its own coverage,
        # and on this tree the two agree at 62, which is what a tripwire looks
        # like when nothing has tripped it — indistinguishable from one that is
        # wired to nothing. So narrow `DECL` here the way the reviewed one was,
        # demanding `<…>` then the body, and require the count to say so.
        global DECL
        was, DECL = DECL, re.compile(
            rf"(?P<attrs>{ATTRS}){ANCHOR}(?P<kw>struct|enum)\s+(?P<name>\w+)"
            r"(?:<[^{;(]*>)?\s*[{(]", re.M)
        try:
            (fake / "src" / "k8s.rs").write_text(
                "pub struct Session<T>\n"
                "where\n"
                "    T: Clone,\n"
                "{\n"
                "    client: kube::Client,\n"
                "}\n"
            )
            planted["a DECL that stopped matching, caught by the keyword count"] = \
                only("token hygiene")
            assert any("the parser reached" in p for p in
                       planted["a DECL that stopped matching, caught by the keyword count"]), \
                planted["a DECL that stopped matching, caught by the keyword count"]
        finally:
            DECL = was

        # …and `watcher::Config`, which is a page size and a label selector and
        # carries no credential — beside a `kube::Config` in the same file, so
        # the green below cannot be the rule having died.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Spec { wc: watcher::Config }\n"
        )
        assert not only("token hygiene"), only("token hygiene")
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Debug)]\n"
            "pub struct Spec { wc: watcher::Config }\n"
            "#[derive(Debug)]\n"
            "pub struct Boot { cfg: kube::Config }\n"
        )
        assert [p for p in only("token hygiene") if "struct Boot" in p] \
            and not [p for p in only("token hygiene") if "struct Spec" in p], \
            only("token hygiene")

        # …and the shapes that must NOT fire, each one a boundary this scan had
        # to widen without widening: unit variants carry nothing (k8s.rs's
        # `Capability` is thirteen of them), `ConfigMap` is a browser kind and
        # not a kubeconfig, and `anyhow::Error` is why the kube error family is
        # spelled qualified rather than as a bare `Error`.
        (fake / "src" / "k8s.rs").write_text(
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\n"
            "pub enum Capability { Metrics, DisruptionBudgets, CertManager }\n"
            "#[derive(Debug)]\n"
            "pub struct Cached {\n"
            "    maps: BTreeMap<Key, ConfigMap>,\n"
            "    last: Option<anyhow::Error>,\n"
            "    note: serde_json::Error,\n"
            "}\n"
        )
        assert not only("token hygiene"), only("token hygiene")

        # …and holding one without *deriving* Debug is exactly what k8s.rs is
        # for. This green is the check's limit and not a verdict on the impl:
        # a hand-written Debug that formatted the client whole would pass here
        # too, which is why the FAIL text makes writing one the last resort and
        # says out loud that this check cannot verify it (NOTES § D164).
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
