#!/usr/bin/env python3
"""Fail unless the two copies a frozen file forced are still the same copy.

`twin-guard.py` does this for a `SignedDuration`. These two pairs are a
**sentence** and a **table**, which it cannot read, and they exist for the same
reason it does: `rules.rs` and `ops.rs` closed at the end of Phase 3 and Phase
7, their constants are private, and a later file needed what they hold. Nothing
in the language ties either pair, and in both cases the second copy was written
by hand from the first.

**Pair 1 — the kind sentences.** `ops::SCALABLE` and `ops::RESTARTABLE` are the
*"k8rs does that for a deployment, a statefulset and a replicaset"* half of a
refusal (invariant 14). `?`'s key map needs the same kinds on screen, so `ui.rs`
carries `SCALE_KINDS` / `RESTART_KINDS` — twelve spaces, `works on `, then that
sentence character for character — and `screens/help.md` draws the result in
three mockups. Three copies of two sentences, none able to import the others.

**Each adjacent pair is pinned and the two ends are not, which is the gap this
closes.** Measured, 2026-09-18, by planting each rewording in the real tree:
rewording `ops::SCALABLE` reddens `ops_tests::scale_takes_the_three_kinds_…`
and `main_tests::a_kind_an_operation_does_not_work_on_…`, and both of those
pin `ops.rs` against **its own** quoted sentence; rewording `ui::RESTART_KINDS`
reddens three `ui_tests`, and all three pin `ui.rs` against **the page**. So a
rewording is never green — it is *satisfiable in the wrong direction*: update
the test's expected string, or the page, and every red clears while the copy at
the other end is never consulted. That is CLAUDE.md's *assert what the
requirement says, not what the implementation returns*, one file apart.
`ops.rs` could gain a fourth kind, `ops_tests` could be updated to match, and
`?` would go on naming three. Nothing but this compares the ends.

**Pair 2 — the kind-to-group table.** `rules::ObjectKind::from_api` maps
`(group, kind)` to a variant; `ui::addressed` maps the variant back, because
`rules.rs` is frozen and the inverse could not live beside it (NOTES § D51).
A variant added to the enum is a compile error in both, so they cannot come
apart by *addition* — what they can do is disagree about a **group** or a
**spelling**, and **that is silent right now**. Measured the same day, in the
real tree: filing `Job` under `apps` instead of `batch`, and spelling
`cronjob` as `cronjobs`, each leave `cargo test --all-targets` **fully green**
while pointing an operation at an address nothing serves. This is the only
thing that looks.

**Text, not values.** Both pairs are compared as the characters they are,
because that is what they are: there is no normal form for an English sentence,
and `("apps", "deployment")` differs from `("apps", "Deployment")` only by the
`to_ascii_lowercase` its caller applies. That lowercasing is the single
transformation this makes, and it is the product's own.

**A spelling this cannot read is a copy it did not find, and that fails.** Every
extraction below is counted and every count is asserted, because *the copies
agree* and *I read no copies* print the same line otherwise (CLAUDE.md § A
derived list asserts it found something).

Usage:
    copy-guard.py               # both pairs, against the tree
    copy-guard.py --self-test   # prove the guard fails when it should
"""
import contextlib, io, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

OPS, UI, RULES, PAGE = "src/ops.rs", "src/ui.rs", "src/rules.rs", "screens/help.md"

# --- PAIR 1 — THE KIND SENTENCES START ---

# `ui.rs`'s constant is its indent, this word, then `ops.rs`'s sentence. The
# indent is `HELP`'s own — twelve columns, under the key's label.
INDENT = " " * 12
MARK = "works on "
# (the sentence in `ops.rs`, the drawn line in `ui.rs`). A third pair is one row.
SENTENCES = (("SCALABLE", "SCALE_KINDS"), ("RESTARTABLE", "RESTART_KINDS"))


def str_const(text, name):
    """Every `const NAME: &str = "…";` in one file, as its value.

    **No escape may appear inside.** These are plain sentences; a `\\n` or a
    `\\"` in one is a shape this cannot compare, so it reads as zero
    declarations and fails as *vetted nothing* rather than comparing half a
    string."""
    pattern = re.compile(
        rf"^(?:pub(?:\([a-z(). ]+\))?\s+)?const\s+{re.escape(name)}\s*:\s*"
        rf"&(?:'static\s+)?str\s*=\s*\"([^\"\\]*)\"\s*;",
        re.M,
    )
    return [m.group(1) for m in pattern.finditer(text)]


def help_literal(text):
    """`ui.rs`'s `HELP` — the block `?` actually draws, not the prose about it.

    Read as its own region so a doc comment carrying the words `works on`
    cannot stand in for a row. That is exactly how this guard would stop
    vetting the thing it is named for and go on printing OK."""
    m = re.search(r"^const HELP: &str = \"(.*?)\";$", text, re.M | re.S)
    return m.group(1) if m else ""


def fenced(text):
    """Every ``` block of a screen file — the mockups, and nothing around them.

    `screens/help.md` names `works on …` in four sentences of prose to explain
    it; a guard that counted those would pass over a page that had stopped
    drawing the row at all."""
    return "\n".join(re.findall(r"^```.*?^```", text, re.M | re.S))


def drawn(text):
    """The `works on …` rows of a drawn block, frame and trailing pad removed —
    so the boxed mockup, the bare one and the Rust literal normalise alike."""
    return [line.strip("│").rstrip() for line in text.splitlines() if MARK in line]


def sentences(ops_text, ui_text, page_text) -> int:
    """`ops.rs`'s sentence, `ui.rs`'s line, and every row the page draws."""
    want = {}
    for ops_name, ui_name in SENTENCES:
        found = str_const(ops_text, ops_name)
        if len(found) != 1:
            print(f"FAIL {OPS} declares {ops_name} {len(found)} time(s), expected 1 — renamed, "
                  f"deleted, duplicated, or written with an escape this guard cannot read. It "
                  f"was about to vet nothing")
            return 1
        mirror = str_const(ui_text, ui_name)
        if len(mirror) != 1:
            print(f"FAIL {UI} declares {ui_name} {len(mirror)} time(s), expected 1 — renamed, "
                  f"deleted, duplicated, or written with an escape this guard cannot read. It "
                  f"was about to vet nothing")
            return 1
        expected = INDENT + MARK + found[0]
        print(f"copy-guard: {ops_name} = {found[0]!r}")
        if mirror[0] != expected:
            print(f"FAIL {UI}'s {ui_name} is not {OPS}'s {ops_name}:")
            print(f"  {ui_name:<13} {mirror[0]!r}")
            print(f"  expected      {expected!r}")
            print(f"one of them is now naming kinds the other does not. `{ui_name}` is "
                  f"`{ops_name}` verbatim behind {len(INDENT)} spaces and {MARK!r}; change one "
                  f"and the other moves in the same commit (invariant 14)")
            return 1
        want[expected] = (ops_name, ui_name)

    # Two sentences that had become one would make every row below match, so the
    # comparison would hold while `?` told a reader `r` works on a replicaset.
    if len(want) != len(SENTENCES):
        print(f"FAIL the {len(SENTENCES)} kind sentences are not {len(SENTENCES)} distinct "
              f"lines — two of them are now the same text, so every drawn row matches whichever "
              f"key it is under and this guard was about to vet nothing")
        return 1

    # Every row that IS drawn has to be one of them — in `HELP` and on the page.
    # **Not every mockup draws them**: `key_map` blanks both rows under a
    # dead-writes run and `screens/help.md` has that screen, so the rule is
    # *every row drawn is one of these*, plus *each sentence is drawn somewhere
    # in each file* below.
    seen = {UI: drawn(help_literal(ui_text)), PAGE: drawn(fenced(page_text))}
    for where, rows in seen.items():
        for row in rows:
            if row not in want:
                print(f"FAIL {where} draws a `{MARK.strip()}` row that is neither sentence:")
                print(f"  drawn     {row!r}")
                for text, (ops_name, _) in want.items():
                    print(f"  {ops_name:<9} {text!r}")
                return 1
        # The canary. An unreadable `HELP`, a page whose fences moved, a regex
        # that stopped matching: each yields zero rows, and zero rows passes the
        # loop above without a word.
        missing = [want[t][0] for t in want if t not in rows]
        if missing:
            print(f"FAIL {where} draws no `{MARK.strip()}` row for {', '.join(missing)} "
                  f"({len(rows)} row(s) read in all) — the copy this guard pins is not there to "
                  f"pin, so it was about to vet nothing")
            return 1
        print(f"copy-guard: {where} draws {len(rows)} `{MARK.strip()}` row(s), every one a "
              f"verbatim {OPS} sentence")
    return 0


# --- PAIR 1 — THE KIND SENTENCES END ---

# --- PAIR 2 — THE KIND-TO-GROUP TABLE START ---

# A variant that must be on both sides, whatever else is. `write-guard.py`'s
# `CANARIES`: two lists that parsed to nothing agree perfectly, and a group
# typo'd into unreadability drops one row silently rather than loudly.
CANARIES = ("Deployment", "Node", "Pod")

# `("apps", "Deployment") => Self::Deployment,`
FROM_API = re.compile(r'\(\s*"([a-z.]*)"\s*,\s*"([A-Za-z]+)"\s*\)\s*=>\s*Self::([A-Za-z]+)\s*,')
# `ObjectKind::Deployment => ("apps", "deployment"),`
ADDRESSED = re.compile(r'ObjectKind::([A-Za-z]+)\s*=>\s*\(\s*"([a-z.]*)"\s*,\s*"([a-z]*)"\s*\)\s*,')


def body(text, signature):
    """One function's body, brace-matched from its signature.

    **String literals are skipped while counting**, because `from_api`'s
    fallback is `format!("{kind}.{group}")` and a counter that read those two
    braces as code would close the body four lines early — on an arm list that
    still parsed, which is the quiet kind of wrong.

    **Exactly one occurrence, or none is returned.** Taking the first of two
    would read some other function's arms and compare them in silence, which is
    the same *a derived list asserts it found something* this file is otherwise
    careful about (found by this guard's own second pass, before it shipped)."""
    if text.count(signature) != 1:
        return None
    at = text.find(signature)
    if at < 0:
        return None
    at = text.find("{", at)
    if at < 0:
        return None
    depth, i, n = 0, at, len(text)
    while i < n:
        c = text[i]
        if c == '"':
            i += 1
            while i < n and text[i] != '"':
                i += 2 if text[i] == "\\" else 1
        elif c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return text[at : i + 1]
        i += 1
    return None


def addresses(rules_text, ui_text) -> int:
    """Each variant's `(group, kind)`, read from the map and from its inverse."""
    forward = body(rules_text, "fn from_api(")
    inverse = body(ui_text, "fn addressed(")
    for what, found, where in (("from_api", forward, RULES), ("addressed", inverse, UI)):
        if found is None:
            print(f"FAIL {where} has no readable `{what}` body — renamed, moved, or its braces "
                  f"no longer match. This guard was about to vet nothing")
            return 1

    # **`Other` is on neither side, and neither regex above can admit it** — it
    # carries no address to compare (invariant 12). `from_api`'s two `Other`
    # arms match on `_` rather than a kind literal, and `addressed`'s is
    # `ObjectKind::Other(_) =>`, whose payload sits where `ADDRESSED` requires
    # the `=>`. That is the whole exclusion: an explicit `!= "Other"` filter
    # stood here until this guard's own self-test was attacked and could not
    # make it matter — a second mechanism for a job the first already does, and
    # the kind of line that reads as a check while pinning nothing.
    down = {v: (g, k.lower()) for g, k, v in FROM_API.findall(forward)}
    up = {v: (g, k) for v, g, k in ADDRESSED.findall(inverse)}

    for what, table, where in (("from_api", down, RULES), ("addressed", up, UI)):
        absent = [c for c in CANARIES if c not in table]
        if absent:
            print(f"FAIL {where}'s `{what}` parsed {len(table)} arm(s) and "
                  f"{', '.join(absent)} is not among them — either the arms are spelled some way "
                  f"this guard cannot read, or a variant it pins is gone. It was about to vet "
                  f"nothing")
            return 1

    print(f"copy-guard: {RULES} `from_api` maps {len(down)} address(es), {UI} `addressed` maps "
          f"{len(up)} back")
    if down.keys() != up.keys():
        only_down = sorted(down.keys() - up.keys())
        only_up = sorted(up.keys() - down.keys())
        print(f"FAIL the two tables name different variants — "
              f"only in `from_api`: {only_down or 'none'}; only in `addressed`: {only_up or 'none'}")
        print("one direction knows an object the other cannot address (NOTES § D51)")
        return 1
    drift = {v: (down[v], up[v]) for v in down if down[v] != up[v]}
    if drift:
        for variant, (there, back) in sorted(drift.items()):
            print(f"FAIL {variant}:  from_api {there}   addressed {back}")
        print(f"`ui::addressed` is `rules::ObjectKind::from_api` read the other way, so a group "
              f"or a spelling that differs points an operation at an object nothing serves "
              f"(NOTES § D51, invariant 12). `addressed`'s kind word is `from_api`'s lowercased")
        return 1
    print("OK — every variant has one address, spelled the same in both directions: "
          + " · ".join(f"{v}={g or '(core)'}/{k}" for v, (g, k) in sorted(down.items())))
    return 0


# --- PAIR 2 — THE KIND-TO-GROUP TABLE END ---


def every() -> int:
    read = {}
    for name in (OPS, UI, RULES, PAGE):
        path = ROOT / name
        if not path.exists():
            print(f"FAIL {name} does not exist — this guard was about to vet nothing")
            return 1
        read[name] = path.read_text(encoding="utf-8")
    return max(
        sentences(read[OPS], read[UI], read[PAGE]),
        addresses(read[RULES], read[UI]),
    )


def self_test():
    """A guard nobody has seen fail is not a guard (todo.md, Phase 1)."""

    def quiet(fn, *args):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = fn(*args)
        return rc, buf.getvalue()

    def check(fn, args, want, why, says=None):
        rc, out = quiet(fn, *args)
        assert rc == want, f"{why}: expected {want}, got {rc}\n{out}"
        if says:
            assert says in out, f"{why}: {says!r} not in\n{out}"
        return out

    # --- pair 1 ---
    ops = ('const SCALABLE: &str = "a deployment, a statefulset and a replicaset";\n'
           'const RESTARTABLE: &str = "a deployment, a statefulset and a daemonset";\n')
    rows = ("            works on a deployment, a statefulset and a replicaset\n"
            "            works on a deployment, a statefulset and a daemonset\n")
    ui = ('/// prose that says works on and must not count as a row\n'
          'const HELP: &str = "  Changing things\n' + rows + '    ctrl-d  delete";\n'
          'const SCALE_KINDS: &str = "            works on a deployment, a statefulset and a replicaset";\n'
          'const RESTART_KINDS: &str = "            works on a deployment, a statefulset and a daemonset";\n')
    page = ("prose naming `works on …` outside a fence\n\n```\n"
            + "".join(f"│{line}   │\n" for line in rows.splitlines())
            + "```\n\n```\n" + rows + "```\n")

    check(sentences, (ops, ui, page), 0, "the tree's own three copies")
    # The boxed mockup and the bare one have to normalise to one string; a page
    # holding only the framed shape still passes.
    boxed = "```\n" + "".join(f"│{line}   │\n" for line in rows.splitlines()) + "```\n"
    check(sentences, (ops, ui, boxed), 0, "a page with only the framed mockup")
    # A mockup that legitimately draws neither row (the dead-writes screen) is
    # not a failure, as long as another one draws both.
    check(sentences, (ops, ui, page + "\n```\n  Changing things (off)\n```\n"), 0,
          "a mockup that blanks both rows")

    # The rewording, in each of the three places it can happen.
    reworded = ops.replace("and a replicaset", "and a bare replicaset")
    check(sentences, (reworded, ui, page), 1, "a reworded ops.rs sentence", "is not")
    check(sentences, (ops, ui.replace("and a daemonset\";", "and a daemon set\";"), page), 1,
          "a reworded ui.rs constant", "is not")
    check(sentences, (ops, ui, page.replace("a statefulset and a daemonset",
                                            "a statefulset, and a daemonset", 1)), 1,
          "a reworded page row", "neither sentence")
    # A kind added on one side only — the drift the box was written for.
    check(sentences, (ops.replace("and a daemonset", "a daemonset and a cronjob"), ui, page), 1,
          "a kind added to ops.rs alone", "is not")
    # Indent and pad: one is real drift, the other is the frame and must not be.
    # Anchored at the constant's own `= "`, because the first `INDENT + MARK` in
    # that text is a `HELP` row and this case is about the constant.
    check(sentences, (ops, ui.replace('= "' + INDENT + MARK, '= "' + INDENT[1:] + MARK, 1), page),
          1, "a constant whose indent moved by one column", "is not")
    check(sentences, (ops, ui, page.replace("replicaset   │", "replicaset       │")), 0,
          "a framed row with more trailing pad")

    # Every way an extraction can come back empty, each of which passes a
    # comparison in silence if it is not asserted.
    blanked = {
        "a renamed ops constant": (ops.replace("SCALABLE:", "SCALES:", 1), ui, page),
        "a deleted ops constant": (ops.replace(ops.splitlines()[0] + "\n", ""), ui, page),
        "a duplicated ops constant": (ops + ops.splitlines()[0] + "\n", ui, page),
        # The same case on the other file. It was missing until this guard's
        # own self-test was attacked: loosening `!= 1` to `< 1` on the `ui.rs`
        # count left the self-test green, so a second `SCALE_KINDS` — the shape
        # a copy-paste leaves behind — was pinned by nothing.
        "a duplicated ui constant":
            (ops, ui + [l for l in ui.splitlines() if "SCALE_KINDS" in l][0] + "\n", page),
        "an escape in the sentence": (ops.replace("a deployment", "a \\\"deployment\\\"", 1), ui, page),
        "a renamed ui constant": (ops, ui.replace("SCALE_KINDS", "SCALE_WORDS", 1), page),
        "a HELP literal that cannot be read": (ops, ui.replace("const HELP: &str = \"", "const HELP: &str = concat!(\"", 1), page),
        "a page with no fences at all": (ops, ui, page.replace("```", "~~~")),
        "a page that stopped drawing one row":
            (ops, ui, page.replace(MARK + "a deployment, a statefulset and a daemonset", "")),
        "an empty ops.rs": ("", ui, page),
        "an empty ui.rs": (ops, "", page),
        "an empty page": (ops, ui, ""),
    }
    for why, args in blanked.items():
        check(sentences, args, 1, why, "vet nothing")
    # Two sentences that became one line: every row then matches either key.
    same = ops.replace("and a daemonset", "and a replicaset")
    check(sentences, (same, ui.replace("and a daemonset\";", "and a replicaset\";"),
                      page.replace("and a daemonset", "and a replicaset")), 1,
          "two sentences that are now the same text", "vet nothing")

    # --- pair 2 ---
    rules = ('fn from_api(api_version: &str, kind: &str) -> Self {\n'
             '    match (group, kind) {\n'
             '        ("apps", "Deployment") => Self::Deployment,\n'
             '        ("batch", "Job") => Self::Job,\n'
             '        ("", "Node") => Self::Node,\n'
             '        ("", "Pod") => Self::Pod,\n'
             '        ("", _) => Self::Other(kind.to_string()),\n'
             '        _ => Self::Other(format!("{kind}.{group}")),\n'
             '    }\n}\n')
    maps = ('fn addressed(kind: &ObjectKind) -> (&\'static str, &\'static str) {\n'
            '    match kind {\n'
            '        ObjectKind::Deployment => ("apps", "deployment"),\n'
            '        ObjectKind::Job => ("batch", "job"),\n'
            '        ObjectKind::Node => ("", "node"),\n'
            '        ObjectKind::Pod => ("", "pod"),\n'
            '        ObjectKind::Other(_) => ("", ""),\n'
            '    }\n}\n')
    out = check(addresses, (rules, maps), 0, "a table and its true inverse")
    assert "4 address(es)" in out, out
    assert "Pod=(core)/pod" in out, out
    # Both fixtures carry an `Other` arm, and neither table may have picked it
    # up: it is the one variant with no address, and a table that held it would
    # compare `("", "")` against `("", "")` and call that agreement.
    assert "Other" not in out, ("Other reached a table that compares addresses", out)
    # **A brace inside a string literal must not be counted**, and this is the
    # case that tells whether the skip is load-bearing. `format!("{kind}.{group}")`
    # alone does not: its braces are balanced, so a counter that read them as
    # code would land in the same place and the assertion above would pass
    # either way. An *unbalanced* one runs the body past the closing `}` and
    # swallows the arms of whatever follows — which parses, and is wrong.
    ran_on = rules.replace('        _ => Self::Other(format!("{kind}.{group}")),\n',
                           '        _ => Self::Other(format!("a { of its own")),\n')
    ran_on += ('fn unrelated() {\n    match x {\n'
               '        ("apps", "StatefulSet") => Self::StatefulSet,\n    }\n}\n')
    out = check(addresses, (ran_on, maps), 0, "an unbalanced brace inside a string literal")
    assert "4 address(es)" in out, ("the body ran past its own closing brace", out)

    check(addresses, (rules, maps.replace('("batch", "job")', '("apps", "job")')), 1,
          "a group that moved on one side", "from_api ('batch', 'job')")
    check(addresses, (rules, maps.replace('"deployment")', '"deploy")')), 1,
          "a kind spelled short on one side", "from_api ('apps', 'deployment')")
    check(addresses, (rules, maps.replace('"deployment")', '"Deployment")')), 1,
          "a kind left PascalCase on one side", "vet nothing")
    # Added *inside* the match, not after the closing brace — appending past the
    # body would leave this green and prove nothing, which is how a case like
    # this passes while looking like a test.
    check(addresses, (rules.replace('        ("batch", "Job") => Self::Job,\n',
                                    '        ("batch", "Job") => Self::Job,\n'
                                    '        ("apps", "StatefulSet") => Self::StatefulSet,\n'),
                      maps), 1, "a variant only `from_api` knows", "different variants")
    check(addresses, (rules, maps.replace('        ObjectKind::Job => ("batch", "job"),\n', "")), 1,
          "a variant only `from_api` knows, by deletion", "different variants")

    for why, args, says in (
        ("a renamed from_api", (rules.replace("fn from_api(", "fn from_api_v2("), maps), "vet nothing"),
        ("a renamed addressed", (rules, maps.replace("fn addressed(", "fn address(")), "vet nothing"),
        ("a from_api whose arms are unreadable", (rules.replace('("apps", "Deployment")', "(APPS, DEPLOYMENT)"), maps), "vet nothing"),
        ("an addressed whose arms are unreadable", (rules, maps.replace("ObjectKind::Node", "Self::Node")), "vet nothing"),
        # Two bodies with one signature: taking the first would compare some
        # other function's arms and say nothing about it.
        ("a duplicated from_api signature", (rules + rules, maps), "vet nothing"),
        ("a duplicated addressed signature", (rules, maps + maps), "vet nothing"),
        ("an empty rules.rs", ("", maps), "vet nothing"),
        ("an empty ui.rs", (rules, ""), "vet nothing"),
    ):
        check(addresses, args, 1, why, says)

    print("copy-guard: self-test passed — a kind sentence reworded in ops.rs, in ui.rs or on the "
          "page is refused, and so is a kind added to one of the three alone, a constant whose "
          "indent moved and two sentences that became one; a framed mockup, a bare one, extra "
          "trailing pad and a mockup that blanks both rows are not failures. A renamed, deleted, "
          "duplicated or escape-carrying constant, an unreadable HELP, a page with no fences or "
          "one row gone, and an empty file each fail as *vetted nothing* rather than as "
          "agreement. On the table: a moved group, a shortened spelling, a kind left PascalCase "
          "and a variant either side lacks are refused, `format!(\"{kind}.{group}\")`'s braces do "
          "not close the body early, and a renamed or unreadable arm list fails as *vetted "
          "nothing* rather than as two empty tables agreeing; and an unbalanced brace inside a "
          "string literal does not run one body into the next, while a signature that appears "
          "twice is refused rather than read as its first occurrence, and `Other` reaches "
          "neither table")


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

sys.exit(every())
