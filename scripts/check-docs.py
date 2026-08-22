#!/usr/bin/env python3
"""Verify every relative Markdown link (file + #anchor) in the repo resolves."""
import re, sys, unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# tmp/ is downloaded upstream documentation, target/ is build output, and the
# changelog is generated — none of them are ours to keep link-clean.
SKIP = ("tmp", "target", "CHANGELOG.md", "tests/fixtures")
NOTES = ROOT / "NOTES.md"
# Matched over the whole file, never line by line. `[^\]]*` already spans
# newlines, so the only thing that ever stopped a wrapped label from matching
# was feeding the regex one line at a time — and this repo wraps its prose at 79
# columns, which makes the wrapped label the shape the house style produces
# most. A link it could not match was not reported, it was skipped: target file
# and anchor both (NOTES § D49).
LINK = re.compile(r'\[([^\]]*)\]\(([^)\s]+)\)')
# `[label]: ./target.md#anchor` — the other half of Markdown's link syntax, and
# one this script did not look at at all, so a reference-style link to a file
# that does not exist was never checked.
# `[ ]{0,3}`, not `\s{0,3}`: Markdown allows three leading *spaces*, and `\s`
# also matches the newline before them — which over a whole file makes the match
# start on the previous line and reports the error one line off.
REF_DEF = re.compile(r'^[ ]{0,3}\[([^\]]+)\]:\s*(\S+)', re.M)
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

def masked(text):
    """`text` with every fenced line blanked out, line count preserved.

    Blanking rather than deleting is the point: the regexes below run over the
    whole string, and a line number is still `count('\\n')` up to the match.
    """
    keep = dict(outside_fences(text))
    return '\n'.join(keep.get(n, '') for n in range(1, len(text.splitlines()) + 1))


def links(text):
    """(line, label, target) for every inline and reference link outside a fence."""
    text = masked(text)
    found = []
    for pattern in (LINK, REF_DEF):
        for m in pattern.finditer(text):
            found.append((text.count('\n', 0, m.start()) + 1, m.group(1), m.group(2)))
    return sorted(found)


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


# NOTES § Decision index is what makes a 150k-token file navigable without
# reading it, and CLAUDE.md requires a new `### D##` to land with its line in the
# same edit. The anchor check above catches a heading that was *renamed* — the
# index line then points at nothing. It cannot catch a heading added with no line
# at all, because there is no link to resolve: the failure degrades in silence,
# which is the hole NOTES § D103 left open and the reason the rule needs a guard
# and not a promise.
#
# Both directions, because they are different failures and only one of them was
# ever reachable: a heading with no line is invisible to anyone reading the index
# instead of the file, and a line with no heading survives whenever someone
# writes it without an anchor and so leaves the anchor check nothing to check.
DECISION = re.compile(r'D(\d+)\b')
INDEX_LINE = re.compile(r'^\s*[-*]\s*\[(D\d+)\]')


def decisions(text):
    """(headings, indexed) — `[(line, "D##")]` for each, read outside fences.

    The index section is delimited by heading *level*, not by a blank line or a
    fixed length: it ends at the next heading of the same or a higher level, so a
    subsection under it would still count as inside it.
    """
    headings, indexed, depth = [], [], None
    for n, line in outside_fences(text):
        h = re.match(r'^(#{1,6})\s+(.*)$', line)
        if h:
            level, title = len(h.group(1)), h.group(2).strip()
            if depth is not None and level <= depth:
                depth = None
            if title.lower() == 'decision index':
                depth = level
            # Level 3 exactly, which is the shape CLAUDE.md names. NOTES.md
            # carries `#### D112 is right and narrow, …` as a *subsection* of the
            # decision it discusses, and reading that as a second D112 heading is
            # how this check would invent a decision number and demand an index
            # line for it. Matching `D\d+` at any level is the fail-open half of
            # the same choice, so the level does the work and the pattern stays
            # loose enough to catch a heading whose dash was typed wrong.
            m = DECISION.match(title) if level == 3 else None
            if m:
                headings.append((n, 'D' + m.group(1)))
        elif depth is not None:
            m = INDEX_LINE.match(line)
            if m:
                indexed.append((n, m.group(1)))
    return headings, indexed


def decision_errors(text, name='NOTES.md'):
    """Every `### D##` missing from the index, and every index line missing a heading."""
    headings, indexed = decisions(text)
    # "Found no unindexed heading" and "found no heading at all" print the same
    # line otherwise, and the second one vets nothing (CLAUDE.md § A derived list
    # asserts it found something). Both halves: an index this could not locate
    # fails the same way a file with no decisions in it does.
    if not headings or not indexed:
        return [f"{name}  read {len(headings)} `### D##` heading(s) and "
                f"{len(indexed)} § Decision index line(s) — this check was about "
                f"to vet nothing"]
    have = {d for _, d in headings}
    listed = {d for _, d in indexed}
    out = [f"{name}:{n}  {d} has a heading and no line in § Decision index — "
           f"the index is part of the entry (CLAUDE.md § Every file here also "
           f"has to get smaller)"
           for n, d in headings if d not in listed]
    out += [f"{name}:{n}  § Decision index lists {d}, which has no `### {d}` heading"
            for n, d in indexed if d not in have]
    return out


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

    # A link whose label wraps. This repo wraps prose at 79 columns, so it is
    # the shape the house style produces most — and the line-by-line scan
    # matched no regex on it and skipped the link entirely, file and anchor
    # both, while printing "OK — all relative links resolve" (NOTES § D49).
    got = links("See [the label that\nwraps](./gone.md#anchor) and done.\n")
    assert got == [(1, "the label that\nwraps", "./gone.md#anchor")], got

    # The same link on one line — the shape that was already caught. Both, or
    # the wrapped assertion above could pass on a regex that lost the plain one.
    got = links("See [one line](./gone.md#anchor) and done.\n")
    assert got == [(1, "one line", "./gone.md#anchor")], got

    # Reference definitions still land, and the line number is still the line
    # the link starts on rather than the offset it was found at.
    got = links("intro\n\n[ref]: ./target.md#a\n")
    assert got == [(3, "ref", "./target.md#a")], got

    # …and a link inside a fence is still not a link, wrapped or not: it is
    # sample text, and its target need not exist.
    assert links("```\n[a](./gone.md)\n```\n") == [], links("```\n[a](./gone.md)\n```\n")
    assert links("~~~\n[wrapped\nlabel](./gone.md)\n~~~\n") == []


    # --- NOTES § Decision index, both directions ---
    # The shape the real file has: a `## Decision index` section of link lines,
    # then the `### D##` sections themselves further down.
    ok = ("# NOTES\n\n## Decision index\n\n"
          "- [D1](#d1--first) — first\n"
          "- [D2](#d2--second) — second\n\n"
          "## Decisions\n\n### D1 — first\n\ntext\n\n### D2 — second\n\ntext\n")
    assert decision_errors(ok) == [], decision_errors(ok)

    # The hole D103 left open: a decision written with no index line at all.
    # There is no link to resolve, so the anchor check above sees nothing to
    # check and the file stays green while the index silently stops being true.
    planted = ok + "\n### D3 — added with no index line\n\ntext\n"
    got = decision_errors(planted)
    assert len(got) == 1 and "D3 has a heading and no line" in got[0], got

    # The other direction: an index line naming a decision nobody wrote. Written
    # without an anchor it is invisible to the anchor check too.
    planted = ok.replace("- [D2](#d2--second) — second\n",
                         "- [D2](#d2--second) — second\n- [D9](#d9) — never written\n")
    got = decision_errors(planted)
    assert len(got) == 1 and "lists D9" in got[0], got

    # `### Design` and `### Dependencies` are headings in the real NOTES.md that
    # start with a D. Neither is a decision, and reading either as `D` + digits
    # would report a decision number that does not exist.
    assert decision_errors(ok + "\n### Design\n\n### Dependencies\n") == []

    # A `### D##` inside a fence is sample text — NOTES.md quotes its own format
    # in code blocks — and an index line inside one is not an index entry.
    assert decision_errors(ok + "\n```\n### D42 — an example\n```\n") == []

    # A `#### D112 is right and narrow, …` subsection under the decision it
    # discusses — a real shape in NOTES.md. It is not a second D112 heading, and
    # reading it as one demands an index line for a decision nobody wrote.
    assert decision_errors(ok + "\n#### D2 is right and narrow\n\ntext\n") == []
    got = decision_errors(ok + "\n#### D7 was never written\n\ntext\n")
    assert got == [], got

    # The index section ends at the next heading of its own level, and it has to:
    # NOTES.md cites its own decisions in prose, in bullets, with anchors — and
    # counting one of those as an index line is how a missing entry would be
    # masked by a cross-reference somewhere else in the file.
    # D2's real index line is removed and the same bullet planted *outside* the
    # section: scoped correctly this is one error, and a bullet in prose standing
    # in for an index line is the whole failure — it would print green.
    outside = (ok.replace("- [D2](#d2--second) — second\n", "")
               + "\n## Something else\n\n- [D2](#d2--second) — cited in prose\n")
    got = decision_errors(outside)
    assert len(got) == 1 and "D2 has a heading and no line" in got[0], got

    # The canary: a file this found no decisions in must fail rather than report
    # nothing wrong, or "the index is complete" and "I could not read the index"
    # print the same line.
    got = decision_errors("# NOTES\n\nno decisions here\n")
    assert len(got) == 1 and "about to vet nothing" in got[0], got
    # Half a canary is still one: headings present, index section unreadable.
    got = decision_errors("# NOTES\n\n### D1 — first\n")
    assert len(got) == 1 and "about to vet nothing" in got[0], got

    print("check-docs: self-test passed — headings inside either fence are not "
          "anchors, a link whose label wraps is still a link, and a decision "
          "with no line in NOTES \u00a7 Decision index (or a line with no "
          "decision) is a failure")

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
    for n, label, target in links(path.read_text(encoding='utf-8')):
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

notes = NOTES.read_text(encoding='utf-8')
errors += decision_errors(notes)

print(f"checked {len(files)} markdown files and "
      f"{len(decisions(notes)[0])} decisions against NOTES \u00a7 Decision index")
for e in errors:
    print("FAIL", e)
print("OK — all relative links resolve and every decision is indexed"
      if not errors else f"{len(errors)} problem(s)")
sys.exit(1 if errors else 0)
