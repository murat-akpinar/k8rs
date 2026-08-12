#!/usr/bin/env python3
"""Verify every relative Markdown link (file + #anchor) in the repo resolves."""
import re, sys, unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# tmp/ is downloaded upstream documentation, target/ is build output, and the
# changelog is generated — none of them are ours to keep link-clean.
SKIP = ("tmp", "target", "CHANGELOG.md", "tests/fixtures")
LINK = re.compile(r'\[([^\]]*)\]\(([^)\s]+)\)')
# `[label]: ./target.md#anchor` — the other half of Markdown's link syntax, and
# one this script did not look at at all, so a reference-style link to a file
# that does not exist was never checked.
REF_DEF = re.compile(r'^\s{0,3}\[([^\]]+)\]:\s*(\S+)')
# Both fence markers. Matching only ``` meant a heading inside a ~~~ block
# became an anchor here and did not on GitHub — the one way this script could
# call a genuinely broken link green.
FENCE = re.compile(r'^\s*(`{3,}|~{3,})')


def outside_fences(text):
    """(line number, line) for every line that is not inside a code fence.

    The opening marker is remembered rather than toggled, so a ``` written
    inside a ~~~ block does not close it — which is the whole reason someone
    reaches for the other marker.
    """
    opener = None
    for n, line in enumerate(text.splitlines(), 1):
        m = FENCE.match(line)
        if m:
            tok = m.group(1)[0]
            if opener is None:
                opener = tok
            elif opener == tok:
                opener = None
            continue
        if opener is None:
            yield n, line

def slug(text):
    t = text.strip().lower()
    t = re.sub(r'`([^`]*)`', r'\1', t)
    t = re.sub(r'\[([^\]]*)\]\([^)]*\)', r'\1', t)
    t = re.sub(r'[*_~]', '', t)
    out = []
    for ch in t:
        if ch.isalnum() or ch in ' -_':
            out.append(ch)
        elif unicodedata.category(ch).startswith('M'):
            out.append(ch)
    # GitHub hyphenates each space separately: "a — b" -> "a--b" (the dash is
    # dropped above, both surrounding spaces are not). Do not collapse runs.
    return re.sub(r'\s', '-', ''.join(out).strip())

def anchors(path):
    found = set()
    for _, line in outside_fences(path.read_text(encoding='utf-8')):
        m = re.match(r'^(#{1,6})\s+(.*)$', line)
        if m:
            found.add(slug(m.group(2)))
    return found


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        p = Path(tmp) / "d.md"
        p.write_text(
            "# Real Heading\n"
            "~~~\n"
            "# Not A Heading\n"
            "~~~\n"
            "```\n"
            "# Also Not A Heading\n"
            "```\n"
        )
        got = anchors(p)
        assert got == {"real-heading"}, got

        # A ``` inside a ~~~ block must not end it, or everything after the
        # inner marker is read as prose.
        p.write_text("~~~\n```\n# Hidden\n~~~\n# Visible\n")
        got = anchors(p)
        assert got == {"visible"}, got
    print("check-docs: self-test passed — headings inside either fence are not anchors")

if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

files = sorted(
    p for p in ROOT.rglob('*.md')
    if '.git' not in p.parts
    and not any(str(p.relative_to(ROOT)).startswith(s) for s in SKIP)
)
anchor_cache = {p: anchors(p) for p in files}
errors = []

for path in files:
    for n, line in outside_fences(path.read_text(encoding='utf-8')):
        found = LINK.findall(line)
        ref = REF_DEF.match(line)
        if ref:
            found = found + [ref.groups()]
        for label, target in found:
            if target.startswith(('http://', 'https://', 'mailto:')):
                continue
            filepart, _, anchor = target.partition('#')
            dest = path.parent / filepart if filepart else path
            dest = dest.resolve()
            if not dest.exists():
                errors.append(f"{path.relative_to(ROOT)}:{n}  missing file  -> {target}")
                continue
            if anchor and dest.suffix == '.md':
                if anchor not in anchor_cache.get(dest, anchors(dest)):
                    errors.append(f"{path.relative_to(ROOT)}:{n}  missing anchor -> {target}")

print(f"checked {len(files)} markdown files")
for e in errors:
    print("FAIL", e)
print("OK — all relative links resolve" if not errors else f"{len(errors)} broken link(s)")
sys.exit(1 if errors else 0)
