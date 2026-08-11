#!/usr/bin/env python3
"""Verify every relative Markdown link (file + #anchor) in the repo resolves."""
import re, sys, unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# tmp/ is downloaded upstream documentation, target/ is build output, and the
# changelog is generated — none of them are ours to keep link-clean.
SKIP = ("tmp", "target", "CHANGELOG.md", "tests/fixtures")
LINK = re.compile(r'\[([^\]]*)\]\(([^)\s]+)\)')
FENCE = re.compile(r'^\s*```')

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
    found, in_fence = set(), False
    for line in path.read_text(encoding='utf-8').splitlines():
        if FENCE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        m = re.match(r'^(#{1,6})\s+(.*)$', line)
        if m:
            found.add(slug(m.group(2)))
    return found

files = sorted(
    p for p in ROOT.rglob('*.md')
    if '.git' not in p.parts
    and not any(str(p.relative_to(ROOT)).startswith(s) for s in SKIP)
)
anchor_cache = {p: anchors(p) for p in files}
errors = []

for path in files:
    in_fence = False
    for n, line in enumerate(path.read_text(encoding='utf-8').splitlines(), 1):
        if FENCE.match(line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        for label, target in LINK.findall(line):
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
