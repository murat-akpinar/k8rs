#!/usr/bin/env python3
"""Refuse a secret in reports/ before it is committed.

`reports/` takes an agent's measurements of a real cluster into a *committed*
file (NOTES § D108) — the same path `scripts/sanitize.jq` exists to guard for
fixtures, and until this landed the rule was a paragraph in reports/README.md
enforced by the PM reading the diff. NOTES § D26 is what a promise is worth here.

**Not the fixture sanitizer reused.** That one rewrites JSON; a report is prose,
and a leak in prose is a sentence, a pasted command, a fenced block, or a
base64 blob — so this reads text and refuses, wherever the value sits.

Fed every framing (CLAUDE.md § D31): the value whole, as a substring of a longer
line, inside a fenced block (fences are *not* skipped — a leak in a code block is
still committed), and base64-re-encoded (long base64 runs are decoded and
re-scanned with the same patterns).

**Ceilings, all real, none engineered around.** A hostname is only recognised by
the suffixes and shapes that name a *machine* (`.lan`, `.internal`, AWS's
`ip-a-b-c-d`): a bare `prod-master-01` in a sentence is indistinguishable from a
pod name, and a guard that guessed would be one people learn to wave through. A
shell assignment (`K8RS_NODE_IMAGE=kindest/node:v1.36.1`) is not read as an env
value, because reports/README.md explicitly permits pasting the command — what
is refused is the container `env` entry shape and any assignment whose *name*
says credential. Both are why the README paragraph stays: this is the floor, not
the whole rule.

Usage:
    reports-guard.py              # check reports/
    reports-guard.py <dir>        # check some other tree (the self-test uses this)
    reports-guard.py --self-test  # prove the guard fails when it should
"""
import base64, binascii, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# The two addresses a report may name, because neither identifies a machine:
# kind's own API server, which cluster.sh prints and every trip log therefore
# carries, and the bind-any address. Everything else is somebody's host.
ADDRESS_OK = {"127.0.0.1", "0.0.0.0"}

# reports/README.md's own instruction is *name the field instead and say what it
# held* — so a report that does exactly that must not be refused, or the guard
# teaches people to route around it. Only the explicit markers: no `...`, which
# is a truncated paste rather than a redaction, and no `xxxx`, which a real token
# reaches by chance.
REDACTED = re.compile(r'(?i)(<[^>]{0,24}>|\*{3,}|\bredacted\b|\belided\b|\bomitted\b|\bnot shown\b)')

CHECKS = [
    # Armour is the whole signal — a report has no reason to hold either half of
    # a keypair, and a certificate identifies the cluster that issued it.
    ("key material or a certificate",
     re.compile(r'-----BEGIN [A-Z0-9 ]+-----')),

    # A ServiceAccount token and anything else JWT-shaped: three base64url
    # segments, the first of which decodes to `{"alg"…`.
    ("a token",
     re.compile(r'\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}')),
    ("a token",
     re.compile(r'(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}')),
    # kubeadm's bootstrap token: `<6 chars>.<16 chars>`, id and secret. kind pins
    # the id to `abcdef`, which does not make the pair less of a credential.
    ("a token",
     re.compile(r'\b[a-z0-9]{6}\.[a-z0-9]{16}\b')),
    # A field whose *name* says credential, carrying something long enough to be
    # one. This is also what catches an env value worth catching.
    # No `\b` before the word: an env var is `DATABASE_PASSWORD`, and a word
    # boundary never matches between `_` and `P` — which is how this class was
    # green over the one framing it exists for. The word is matched wherever it
    # ends a name, which is where it always is (CLAUDE.md § D31).
    ("a token",
     re.compile(r'(?i)(token|password|passwd|secret|api[_-]?key|credential)s?'
                # The closing quote of a JSON key sits between the name and the
                # colon, and a report pastes JSON as often as YAML.
                r'["\']?\s*[:=]\s*["\']?[^\s"\'`,;]{6,}')),
    # A credential in a URL's userinfo — the framing that hides inside a
    # connection string nobody reads to the end.
    ("a token",
     re.compile(r'\b[a-z][a-z0-9+.-]*://[^/\s:@]+:[^/\s@]+@')),

    # The value is required, not just the key: naming the field is what
    # reports/README.md asks a report to do instead of pasting it, and it also
    # puts a redaction marker inside the match where REDACTED can see it.
    ("kubeconfig contents",
     re.compile(r'(client-certificate-data|client-key-data|certificate-authority-data)'
                r'\s*:\s*\S+')),
    ("kubeconfig contents",
     re.compile(r'(?m)^\s*(current-context|preferences)\s*:\s*\S|^\s*kind\s*:\s*Config\s*$')),

    # The container env entry, in YAML and in JSON. A `value:` under a `name:` is
    # an object dump, which reports/README.md refuses outright — those go through
    # `just fixtures` and the sanitizer, into tests/.
    ("an environment variable value",
     re.compile(r'(?m)^[ \t-]*name\s*:\s*\S.*\n[ \t]*value\s*:\s*\S')),
    ("an environment variable value",
     re.compile(r'"name"\s*:\s*"[^"]*"\s*,\s*"value"\s*:\s*"[^"]')),

    ("an annotation payload",
     re.compile(r'(?m)^\s*"?annotations"?\s*:|kubectl\.kubernetes\.io/last-applied-configuration')),
    # A domain-keyed annotation carrying its value — `internal.example.com/oncall:
    # someone@example.com` is the shape, and the payload is the part after the
    # colon.
    ("an annotation payload",
     re.compile(r'(?m)^\s*"?[a-z0-9-]+(\.[a-z0-9-]+)+/[A-Za-z0-9._-]+"?\s*:\s*\S')),
    # An address a human can be reached at is a payload wherever it sits.
    ("an annotation payload",
     re.compile(r'\b[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+\b')),

    ("a node IP", re.compile(r'\b(?:\d{1,3}\.){3}\d{1,3}\b')),

    # A name that identifies a machine rather than an object in the cluster.
    ("a hostname",
     re.compile(r'(?i)\b[a-z0-9][a-z0-9-]*(\.[a-z0-9-]+)*\.(lan|local|internal|corp|home|localdomain)\b')),
    ("a hostname",
     re.compile(r'\bip-\d{1,3}-\d{1,3}-\d{1,3}-\d{1,3}\b')),
]

# Base64 with either alphabet, long enough that a word cannot be one by accident.
B64 = re.compile(r'[A-Za-z0-9+/_-]{12,}={0,2}')


def decoded(text: str):
    """Every long base64 run in `text`, decoded, where it decodes to text.

    The framing this exists for is `base64 -w0 admin.key.pem` pasted into a
    report: armour and all, invisible to every pattern above. Runs that decode to
    bytes are dropped rather than scanned — an ordinary word like
    `ContainerStateTerminated` is valid base64, and scanning its binary garbage
    is how a guard invents a finding.

    Floor: twelve encoded characters, so nothing shorter than nine bytes is
    decoded at all. It is set by the shortest thing worth catching — an IPv4
    address encodes to fifteen — and not by taste; below it every ordinary word
    in the file becomes a decode, and a secret of eight bytes is not one.
    """
    for m in B64.finditer(text):
        blob = m.group(0)
        for alphabet in (base64.b64decode, base64.urlsafe_b64decode):
            try:
                raw = alphabet(blob + '=' * (-len(blob) % 4))
            except (binascii.Error, ValueError):
                continue
            if not raw:
                continue
            printable = sum(32 <= b < 127 or b in (9, 10, 13) for b in raw)
            if printable / len(raw) >= 0.9:
                yield raw.decode('ascii', 'replace'), blob
                break


def findings(text: str):
    """(class, what matched, a string that is in `text`) for everything refused.

    The third element is what the caller turns into a line number: for a direct
    match it is the match, and for a base64 hit it is the *blob*, which is the
    thing actually sitting in the file. Reporting the decoded value's position
    gives line 0 — a leak nobody can find is a leak reported badly.
    """
    out = []
    bodies = [('', text, None)] + [(' (base64-encoded)', d, blob) for d, blob in decoded(text)]
    for framing, body, blob in bodies:
        for label, pattern in CHECKS:
            for m in pattern.finditer(body):
                hit = m.group(0).strip().splitlines()[0]
                if label == "a node IP" and hit in ADDRESS_OK:
                    continue
                if REDACTED.search(hit):
                    continue
                out.append((label + framing, hit, blob or hit))
    return out


def line_of(text: str, needle: str) -> int:
    i = text.find(needle)
    return text.count('\n', 0, i) + 1 if i >= 0 else 0


def run(tree: Path) -> int:
    files = sorted(tree.rglob('*.md'))
    bad, lines = [], 0
    # Everything else in the directory is refused unread. reports/README.md says
    # one markdown file per measurement and no object dumps; a `kubectl get -o
    # yaml > reports/x.yaml` is exactly the thing this guard exists for and is
    # exactly what globbing `*.md` would never look at.
    for f in sorted(tree.rglob('*')):
        if f.is_file() and f.suffix != '.md':
            bad.append((f, 0, "a file that is not a report",
                        f"{f.name} — reports/ holds one markdown file per "
                        f"measurement; an object dump goes through `just fixtures`"))
    for f in files:
        text = f.read_text(encoding='utf-8')
        lines += len(text.splitlines())
        for label, hit, anchor in findings(text):
            bad.append((f, line_of(text, anchor[:40]), label, hit))

    # The canary. "Nothing to find" and "found nothing because I read the wrong
    # directory" print the same line otherwise, and this guard's whole value is
    # that it keeps printing the first one (CLAUDE.md § A derived list asserts it
    # found something). Proven on the bytes just read, not on a fixture: the real
    # text of a real report, plus one planted line, has to come back refused.
    if not files:
        print(f"FAIL {tree} holds no .md files — this guard was about to vet nothing")
        return 1
    probe = files[0].read_text(encoding='utf-8') + "\nAuthorization: Bearer eyJhbGciOiJSUzI1NiIsImtpZCI6IngifQ.e30.sig\n"
    if not findings(probe):
        print(f"FAIL the canary line planted in {files[0].name} was not refused — "
              f"the patterns are not running over what was read")
        return 1

    print(f"checked {len(files)} report(s), {lines} lines, in {len(CHECKS)} classes "
          f"(canary: a planted bearer token in {files[0].name} was refused)")
    for f, n, label, hit in bad:
        try:
            name = f.relative_to(ROOT)
        except ValueError:
            name = f
        print(f"FAIL {name}:{n}  {label}  -> {hit[:70]}")
    print("OK — no secret in any report" if not bad else
          f"{len(bad)} leak(s) — a leak never leaves git history (reports/README.md)")
    return 1 if bad else 0


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1).

    Every refused class is planted once, in each framing the box names: whole, as
    a substring of a longer line, inside a fence, and base64-re-encoded.
    """
    plants = {
        "key material": "-----BEGIN RSA PRIVATE KEY-----",
        "a certificate": "-----BEGIN CERTIFICATE-----",
        "a ServiceAccount token": "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJhIn0.c2lnbmF0dXJl",
        "a bearer header": "Authorization: Bearer abcdefghijklmnopqrstuvwxyz012345",
        "a bootstrap token": "abcdef.0123456789abcdef",
        "a named credential": "token: s3cr3t-value-here",
        # The framing that was green while the class was broken: the word is not
        # the whole name, it is the end of one, and `\b` never matched there.
        "a credential under a longer name": "DATABASE_PASSWORD=hunter22xyz",
        "a credential under a JSON key": '"bootstrapToken": "0123456789abcdef"',
        "a credential in a URL": "postgres://deployer:hunter2@db/app",
        "a kubeconfig data key": "client-key-data: LS0tLS1CRUdJTg==",
        "a JSON env entry": '{"name": "API_KEY", "value": "sk-live-0123"}',
        "a last-applied annotation": "kubectl.kubernetes.io/last-applied-configuration: {}",
        "an email address": "someone@example.com",
        "a node IP": "192.168.1.130",
        "a public IP": "10.3.44.204",
        "a .lan hostname": "k8rs-worker.lan",
        "an AWS hostname": "ip-10-3-44-204.eu-west-1.compute.internal",
    }
    # The rest are *line* shapes — a YAML key at the start of its line. They get
    # every framing except the mid-sentence one, and deliberately: unanchoring
    # `current-context:` or `annotations:` would refuse a report that merely
    # names the field, which is the one thing reports/README.md says to do
    # instead of pasting the value.
    line_plants = {
        "a kubeconfig document": "current-context: kind-k8rs",
        "an env entry": "- name: DATABASE_URL\n  value: postgres://db/app",
        "an annotations block": "  annotations:",
        "a domain-keyed annotation": "internal.example.com/oncall: pager-duty-schedule",
    }
    for what, value in line_plants.items():
        assert findings(value), f"{what} was not refused whole: {value!r}"
        assert findings(f"text\n\n```\n{value}\n```\n\nmore text\n"), \
            f"{what} was not refused inside a fence"
        blob = base64.b64encode(value.encode()).decode()
        assert findings(f"the blob was {blob}\n"), \
            f"{what} was not refused base64-encoded (as {blob})"
        # …and the anchoring is the reason, not an accident: named mid-sentence,
        # the field is what a report is supposed to say instead of the value.
        assert findings(f"the report named {value.splitlines()[0]} and stopped there\n") == [], \
            f"{what} refused a sentence that only names the field"

    for what, value in plants.items():
        # Whole: the value is the line.
        assert findings(value), f"{what} was not refused whole: {value!r}"
        # Substring: the value sits inside a sentence, which is where a value
        # pasted from a terminal into prose actually lands (D31).
        assert findings(f"The run printed {value} and then exited 0.\n"), \
            f"{what} was not refused as a substring"
        # Inside a fence. Fences are not skipped, on purpose: a leak in a code
        # block is committed exactly like one in a sentence.
        assert findings(f"text\n\n```\n{value}\n```\n\nmore text\n"), \
            f"{what} was not refused inside a fence"
        # Base64-re-encoded — the framing that walks past every pattern above.
        blob = base64.b64encode(value.encode()).decode()
        assert findings(f"the blob was {blob}\n"), \
            f"{what} was not refused base64-encoded (as {blob})"

    # The other half: what a report is *for* has to keep passing, or the guard is
    # a directory nobody may write to. These are lines lifted from the committed
    # reports and from reports/README.md's own list of what may be pasted.
    clean = (
        "# 2026-08-20 — the settled record\n\n"
        "    kind create cluster --name k8rs --image kindest/node:v1.36.1\n"
        "API: https://127.0.0.1:6443   context: kind-k8rs\n"
        "`docker restart` leaves `exitCode: 255, reason: \"Unknown\"`\n"
        "K8RS_CLUSTER=review K8RS_NODE_IMAGE=kindest/node:v1.36.1 scripts/cluster.sh up\n"
        "Never: tokens, certificates, keys, kubeconfig contents, environment\n"
        "variable values, annotation payloads, node IPs or hostnames.\n"
        "restartCount: 3 on k8rs-worker2, and the container was Terminated\n"
        "src/rules.rs:3630 and scripts/sanitize.jq disagree about ContainerStateTerminated\n"
    )
    assert findings(clean) == [], findings(clean)

    # A redaction is what reports/README.md asks for, and refusing it is how a
    # guard trains people to leave the value in instead.
    for ok in ("token: <redacted>", "client-key-data: <elided>",
               "the password was ***", "DATABASE_PASSWORD=<not shown>"):
        assert findings(ok) == [], (ok, findings(ok))
    # …and the marker does not launder a value sitting beside it.
    assert findings("token: s3cr3t-value-here  (<redacted> elsewhere)")

    # A non-markdown file in the directory is refused unread — the object dump
    # this guard would otherwise never open.
    import contextlib, io, tempfile
    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        (d / "r.md").write_text("# a clean report\n")
        (d / "dump.yaml").write_text("apiVersion: v1\n")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = run(d)
        assert rc == 1 and "not a report" in buf.getvalue(), buf.getvalue()
        (d / "dump.yaml").unlink()
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = run(d)
        assert rc == 0, buf.getvalue()

    # The canary, proven the only way it can be: point the guard at a tree with
    # nothing in it and watch it refuse to report success. (That the real
    # reports/ is green is the *next* line of scripts/guards.sh, not this one —
    # a self-test that also audits the tree fails with the wrong message the day
    # a report genuinely leaks.)
    with tempfile.TemporaryDirectory() as tmp:
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = run(Path(tmp))
        assert rc == 1 and "vet nothing" in buf.getvalue(), buf.getvalue()

    print(f"reports-guard: self-test passed — {len(plants) + len(line_plants)} planted values across "
          f"{len(set(l for l, _ in CHECKS))} classes, each refused whole, as a "
          f"substring, inside a fence and base64-encoded; the commands and field "
          f"values a report is for still pass")


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

sys.exit(run(Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "reports"))
