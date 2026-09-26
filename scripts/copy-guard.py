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

**Pair 3 — the two verdict lines and the delete hedges.** `ops::ACCEPTED`,
`ops::UNCHECKABLE` and the clauses `ops::removal` hedges a consequence with are
private to the same frozen file, and `src/ui_tests.rs` retypes them so a
`views::Dialog` fixture carries what `ops.rs` really returns. The copy stays
and there is no reversal (NOTES § D282): the sentence lives in **three**
artifacts, and a `pub(crate)` const would unify two of them and leave
`screens/dialogs.md` — the artifact the next reader builds against — pinned by
nothing. Here too each adjacent pair has a keeper and the ends have none:
`ops_tests` compares `ops.rs` against its own quoted strings, and `ui_tests`'s
box tests compare `ui_tests.rs` against the page. Nothing compares `ops.rs`
with `ui_tests.rs`.

**A spelling this cannot read is a copy it did not find, and that fails.** Every
extraction below is counted and every count is asserted, because *the copies
agree* and *I read no copies* print the same line otherwise (CLAUDE.md § A
derived list asserts it found something).

Usage:
    copy-guard.py               # every pair, against the tree
    copy-guard.py --self-test   # prove the guard fails when it should
"""
import contextlib, io, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

OPS, UI, RULES, PAGE = "src/ops.rs", "src/ui.rs", "src/rules.rs", "screens/help.md"
UI_TESTS, DIALOGS = "src/ui_tests.rs", "screens/dialogs.md"

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

# --- PAIR 3 — THE VERDICT LINES AND THE DELETE HEDGES START ---

# Each verdict, with two fragments of itself. **Only the page is counted**:
# `screens/dialogs.md` draws each verdict over and over — in its mockups and in
# the prose that quotes them — while `ops.rs` and `ui_tests.rs` declare it once
# and `str_const` pins that one against the other, which a count would repeat.
# The run prints how many the page holds, so the number is never in this file.
VERDICTS = (
    ("ACCEPTED", ("the cluster checked it first", "and accepted it")),
    ("UNCHECKABLE", ("k8rs did not check this one", "with the cluster first")),
)

# Every clause `ops::removal` hedges a consequence with, each with two fragments
# of itself. Three clauses, not one: the three workload kinds share a finalizer
# hedge, the replicaset and the pod share the unread-`ownerReferences` one word
# for word, and the node words the same fact its own way (`screens/dialogs.md`
# § Delete, NOTES § D224). Both tails name their own clause — *may delay this or
# act first* alone belongs to two of them and would count six copies of three.
HEDGES = (
    ("k8rs has not read what may be attached to it, and something there may delay this or "
     "act first",
     ("k8rs has not read what may be attached to it,",
      "and something there may delay this or act first")),
    ("Whatever created it will normally replace it — k8rs has not checked whether anything did",
     ("whatever created it will normally replace it",
      "k8rs has not checked whether anything did")),
    ("Something attached to it, unread by k8rs, may delay this or act first",
     ("something attached to it, unread by k8rs,", "k8rs, may delay this or act first")),
)

# The product's own name, and `ui::spoken`'s one exception to raising a letter.
NAME = "k8rs"

# Each arm of `removal`'s match opens on one of these, and the consequence it
# builds is the text between this one and the next.
ARM = "ApiResource::erase::<"

# A box frame, a markdown emphasis, and Rust's end-of-line continuation — none of
# them part of any sentence, all of them sitting inside one.
FRAME = str.maketrans("│┌┐└┘─├┤*`\\", " " * 11)


def packed(text):
    """One text with its frame gone, its spaces **removed** and its case folded.

    **Removed, not collapsed, and that is the whole reason this is not `drawn`
    above.** The same sentence arrives here in four shapes: continued across Rust
    source lines with a `\\`, wrapped at a bullet margin, broken across box rows
    between `│`, and — in the mockups of 80-column terminal output — cut
    **mid-word**, `k8rs has no` / `t checked whether anything did`. Only removal
    puts all four back into one string. The fold is `ui::spoken`'s doing: the
    page draws a verdict lower case headlessly and raised in a box, and both are
    copies of the one constant."""
    return "".join(text.translate(FRAME).split()).casefold()


def rows(text):
    """Every line of a drawn block, frame and padding gone, exactly as drawn.

    The one place a verdict is compared as the page draws it — case and inner
    spacing kept, because this is what asserts both shapes are there: a boxed row and a
    headless one reduce to the same sentence, and a line is compared **whole** —
    `k8rs did not check this one with the cluster first` is a prefix of the drawn
    form, so a page that had stopped printing the headless shape would satisfy a
    substring test with the drawn one."""
    return [line.replace("│", " ").strip() for line in fenced(text).splitlines()]


def spoken(line):
    """`ui::spoken` — a verdict line as a dialog draws it.

    First letter raised and a full stop added, except that a line opening on the
    product's own name keeps it: *"K8rs"* is a word this product never spells
    (`src/ui.rs`, NOTES § D260 item 6)."""
    raised = line if line.startswith(NAME) else line[:1].upper() + line[1:]
    return raised if raised.endswith(".") else raised + "."


def copies(where, text, said, fragments):
    """How many copies of one sentence a file holds — or `None`, having said why not.

    **A fragment is counted against the whole, and that is what sees a stale
    copy.** Presence alone cannot: four `ui_tests.rs` fixtures carry the workload
    hedge and the page draws `ACCEPTED` six times, so rewording all but one
    leaves the copies that did move answering for the one that did not. A copy
    reworded anywhere keeps whichever fragment does not cover the reword, so the
    two counts come apart.

    **What it does not see, stated rather than implied.** A copy rewritten end to
    end moves out of both counts together, and no comparison of text catches that
    one. Neither does a reword inside a word both fragments cover — every pair
    below is disjoint but the node clause's, whose two share `k8rs,`, and the
    product's own name is not a word anyone rewords."""
    whole = text.count(packed(said))
    if not whole:
        print(f"FAIL {where} does not say:")
        print(f"  {said!r}")
        print(f"this sentence is one text in three artifacts — `{OPS}` holds it, `{UI_TESTS}` "
              f"retypes it into the fixture a box is drawn from, and `{DIALOGS}` is the one that "
              f"owns the words (invariant 14, NOTES § D282). It is not in this one to pin, so "
              f"this guard was about to vet nothing")
        return None
    for fragment in fragments:
        if packed(fragment) not in packed(said):
            print(f"FAIL this guard's own table pairs {fragment!r}")
            print(f"with a sentence that does not contain it:")
            print(f"  {said!r}")
            print(f"the fragment counts nothing it claims to, so the row was about to vet nothing")
            return None
        near = text.count(packed(fragment))
        if near != whole:
            print(f"FAIL {where} holds {near} copy/copies of:")
            print(f"  {fragment!r}")
            print(f"and {whole} of the sentence it belongs to:")
            print(f"  {said!r}")
            print(f"one copy was reworded and its siblings were not, which is the drift a search "
                  f"for the sentence cannot see — the copies that did move still answer it "
                  f"(NOTES § D282)")
            return None
    return whole


def verdicts(ops_text, tests_text, page_text) -> int:
    """`ops.rs`'s verdict constant, `ui_tests.rs`'s retype, and every copy the page draws."""
    want = {}
    for name, fragments in VERDICTS:
        found = str_const(ops_text, name)
        if len(found) != 1:
            print(f"FAIL {OPS} declares {name} {len(found)} time(s), expected 1 — renamed, "
                  f"deleted, duplicated, or written with an escape this guard cannot read. It "
                  f"was about to vet nothing")
            return 1
        mirror = str_const(tests_text, name)
        if len(mirror) != 1:
            print(f"FAIL {UI_TESTS} declares {name} {len(mirror)} time(s), expected 1 — renamed, "
                  f"deleted, duplicated, or written with an escape this guard cannot read. It "
                  f"was about to vet nothing")
            return 1
        print(f"copy-guard: {name} = {found[0]!r}")
        if mirror[0] != found[0]:
            print(f"FAIL {UI_TESTS}'s {name} is not {OPS}'s {name}:")
            print(f"  {UI_TESTS:<15} {mirror[0]!r}")
            print(f"  {OPS:<15} {found[0]!r}")
            print(f"the fixture is built from a sentence `ops::Checked::verdict` does not return, "
                  f"so the dialog is asserted against a string no code path produces "
                  f"(NOTES § D260 item 6, § D282)")
            return 1
        want[found[0]] = (name, fragments)

    # Two verdicts that had become one line would make every drawn row match
    # whichever of them was looked up first, and a dialog could then say *checked*
    # over an operation that sent no check.
    if len(want) != len(VERDICTS):
        print(f"FAIL the {len(VERDICTS)} verdict lines are not {len(VERDICTS)} distinct "
              f"sentences — two of them are now the same text, so every drawn row matches "
              f"either verdict and this guard was about to vet nothing")
        return 1

    # Both shapes, because the page draws both: the dialog's, reshaped by
    # `ui::spoken`, and the headless one `main.rs` prints, which is the constant
    # itself. Each is a whole row of a mockup.
    drawn_rows, packed_page = rows(page_text), packed(page_text)
    for value, (name, fragments) in want.items():
        for shape, text in (("headless", value), ("drawn", spoken(value))):
            if text not in drawn_rows:
                print(f"FAIL {DIALOGS} draws no {shape} row for {name} "
                      f"({len(drawn_rows)} mockup row(s) read in all):")
                print(f"  expected  {text!r}")
                print(f"the copy this guard pins is not there to pin, so it was about to vet "
                      f"nothing. A verdict reworded on the page is the one drift no test sees "
                      f"(NOTES § D282)")
                return 1
        # And every *other* copy on the page — the further mockups and the prose
        # that quotes them — says the same thing as the two rows just found.
        held = copies(DIALOGS, packed_page, value, fragments)
        if held is None:
            return 1
        print(f"copy-guard: {DIALOGS} says {name} {held}× — headless, and as {spoken(value)!r}")
    return 0


def hedges(ops_text, tests_text, page_text) -> int:
    """Every clause `removal` hedges with, in `ops.rs`, in `ui_tests.rs` and on the page."""
    arms = body(ops_text, "fn removal(")
    if arms is None:
        print(f"FAIL {OPS} has no readable `removal` body — renamed, moved, or its braces no "
              f"longer match. This guard was about to vet nothing")
        return 1
    arms = [packed(arm) for arm in arms.split(ARM)[1:]]
    if not arms:
        print(f"FAIL {OPS}'s `removal` body holds no `{ARM}` arm — the consequences are built "
              f"some way this guard cannot read, so it was about to vet nothing")
        return 1

    # Joined on a space, which packed text never contains: a clause must be found
    # inside one arm and not across the seam between two.
    read = ((OPS, " ".join(arms)), (UI_TESTS, packed(tests_text)), (DIALOGS, packed(page_text)))
    for hedge, fragments in HEDGES:
        held = []
        for where, text in read:
            count = copies(where, text, hedge, fragments)
            if count is None:
                return 1
            held.append(f"{where} {count}×")
        print(f"copy-guard: {hedge[:46]!r}… — {', '.join(held)}")

    # The other direction: a clause above may still be somewhere in `removal`
    # while the arm that used to carry it has been reworded out of all of them.
    homeless = [n for n, arm in enumerate(arms, 1)
                if not any(packed(hedge) in arm for hedge, _ in HEDGES)]
    if homeless:
        for n in homeless:
            print(f"FAIL {OPS}'s `removal` arm {n} hedges with none of them:")
            print(f"  {arms[n - 1][:140]!r}…")
        print(f"every consequence `delete` shows says what k8rs did not read before offering it "
              f"(`{DIALOGS}` § Delete). An arm that hedges with nothing is a dialog promising "
              f"more than the call behind it knows")
        return 1
    print(f"OK — {len(arms)} `removal` arm(s), each hedging with one of {len(HEDGES)} clause(s) "
          f"all three artifacts spell the same way, every copy of every one of them")
    return 0


# --- PAIR 3 — THE VERDICT LINES AND THE DELETE HEDGES END ---


def every() -> int:
    read = {}
    for name in (OPS, UI, RULES, PAGE, UI_TESTS, DIALOGS):
        path = ROOT / name
        if not path.exists():
            print(f"FAIL {name} does not exist — this guard was about to vet nothing")
            return 1
        read[name] = path.read_text(encoding="utf-8")
    return max(
        sentences(read[OPS], read[UI], read[PAGE]),
        addresses(read[RULES], read[UI]),
        verdicts(read[OPS], read[UI_TESTS], read[DIALOGS]),
        hedges(read[OPS], read[UI_TESTS], read[DIALOGS]),
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

    # **Every row below is wired into `every`.** Each case in this function calls
    # its checker directly, so a row written, self-tested and never called
    # against the tree is the one failure the rest of this cannot see — and it
    # is the failure that leaves the copies it names pinned by nothing. Found by
    # attacking this self-test: deleting a call from `every` left it green.
    called = Path(__file__).read_text(encoding="utf-8").split("def every() -> int:")[1]
    called = called.split("\ndef ")[0]
    for row in ("sentences(", "addresses(", "verdicts(", "hedges("):
        assert row in called, f"`every` never calls `{row})`, so nothing runs it against the tree"

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

    # --- pair 3 ---
    # Written the way `ops.rs` writes them — across source lines, continued with
    # a `\`, which is one of the four shapes `packed` exists for and the reason a
    # plain substring search finds none of these.
    verdict_consts = ('const ACCEPTED: &str = "the cluster checked it first and accepted it";\n'
                      'const UNCHECKABLE: &str = "k8rs did not check this one with the cluster '
                      'first";\n')
    removal = (
        'fn removal(kind: &str, name: &str) -> Result<(ApiResource, String, bool), String> {\n'
        '    let (resource, consequence, namespaced) = match kind {\n'
        '        "deployment" => (\n'
        '            ApiResource::erase::<Deployment>(&()),\n'
        '            "This asks the cluster to remove the deployment. \\\n'
        '             k8rs has not read what may be attached to it, and something there may delay '
        'this or \\\n'
        '             act first — left alone, nothing is left running."\n'
        '                .to_string(),\n'
        '            true,\n'
        '        ),\n'
        '        "pod" => (\n'
        '            ApiResource::erase::<Pod>(&()),\n'
        '            "This removes the pod. Whatever created it will normally replace it — k8rs '
        'has not \\\n'
        '             checked whether anything did."\n'
        '                .to_string(),\n'
        '            true,\n'
        '        ),\n'
        '        "node" => (\n'
        '            ApiResource::erase::<Node>(&()),\n'
        '            format!(\n'
        '                "This asks the cluster to remove its record of {name}, not the machine. '
        '\\\n'
        '                 Something attached to it, unread by k8rs, may delay this or act first. '
        'Left \\\n'
        '                 alone, its pods are deleted."\n'
        '            ),\n'
        '            false,\n'
        '        ),\n'
        '        other => return Err(format!("k8rs cannot delete {}", a_kind(other))),\n'
        '    };\n'
        '    Ok((resource, consequence, namespaced))\n'
        '}\n'
    )
    ops3 = verdict_consts + removal
    # Two copies of the pod clause, because one copy is the shape a count can
    # never fail on and the real file carries three.
    tests3 = (
        verdict_consts
        + '        consequence: "This removes the pod. Whatever created it will normally replace '
          'it — k8rs \\\n'
          '                      has not checked whether anything did."\n'
          '            .to_owned(),\n'
          '    let replicaset = "This removes the replicaset and every pod it manages. Whatever '
          'created it \\\n'
          '                      will normally replace it — k8rs has not checked whether anything '
          'did.";\n'
          '    let removed = "This asks the cluster to remove the deployment. k8rs has not read '
          'what may \\\n'
          '                   be attached to it, and something there may delay this or act first '
          '— left \\\n'
          '                   alone, nothing is left running.";\n'
          '        consequence: "This asks the cluster to remove its record of node-3, not the '
          'machine. \\\n'
          '                      Something attached to it, unread by k8rs, may delay this or act '
          'first. \\\n'
          '                      Left alone, its pods are deleted."\n'
          '            .to_owned(),\n'
    )
    # The page in every shape it really uses: two drawn boxes that wrap a
    # sentence between `│`, a **bullet outside every fence** — § Delete specifies
    # four of the six kinds that way and draws neither of them — and a headless
    # mockup of 80-column output, which is the one that cuts a word in half.
    scale_box = ("```\n"
                 "│   │  This starts 1 more copy of your app.                      │  │\n"
                 "│   │  The cluster checked it first and accepted it.             │  │\n"
                 "```\n")
    restart_box = ("```\n"
                   "│   │  This replaces every copy of your app with a new one.      │  │\n"
                   "│   │  The cluster checked it first and accepted it.             │  │\n"
                   "```\n")
    bullet = ("- **deployment** — \"This asks the cluster to remove the deployment.\n"
              "  k8rs has not read what may be attached to it, and something there\n"
              "  may delay this or act first — left alone, nothing is left running.\"\n")
    headless = ("```\n"
                "the cluster checked it first and accepted it\n"
                "This removes the pod. Whatever created it will normally replace it — k8rs has no\n"
                "t checked whether anything did.\n"
                "k8rs did not check this one with the cluster first\n"
                "```\n")
    pod_box = ("```\n"
               "│   │  This removes the pod. Whatever created it will normally    │  │\n"
               "│   │  replace it — k8rs has not checked whether anything did.    │  │\n"
               "│   │  k8rs did not check this one with the cluster first.        │  │\n"
               "```\n")
    node_box = ("```\n"
                "│   │  This asks the cluster to remove its record of node-3, not  │  │\n"
                "│   │  the machine. Something attached to it, unread by k8rs, may │  │\n"
                "│   │  delay this or act first. Left alone, its pods are deleted. │  │\n"
                "│   │  k8rs did not check this one with the cluster first.        │  │\n"
                "```\n")
    page3 = scale_box + "\n" + restart_box + "\n" + bullet + "\n" + headless + "\n" + \
        pod_box + "\n" + node_box

    out = check(verdicts, (ops3, tests3, page3), 0, "the tree's own three copies of each verdict")
    assert "ACCEPTED 3×" in out and "UNCHECKABLE 3×" in out, ("the page copies were not counted",
                                                              out)
    out = check(hedges, (ops3, tests3, page3), 0, "the three hedges in all three artifacts")
    assert "3 `removal` arm(s)" in out, ("the arms were not read one by one", out)
    # **`packed` is load-bearing in all four of its shapes**, and each of these
    # says so against the fixtures above: without it none of the three artifacts
    # holds a clause at all and this guard would fail on a tree perfectly in step
    # — the failure that gets a guard deleted rather than fixed.
    workload, pod_clause, node_clause = (said for said, _ in HEDGES)
    assert workload not in ops3 and packed(workload) in packed(ops3), \
        "the ops.rs fixture does not exercise Rust's `\\` continuation"
    assert pod_clause not in tests3 and packed(pod_clause) in packed(tests3), \
        "the ui_tests.rs fixture does not exercise Rust's `\\` continuation"
    assert node_clause not in page3 and packed(node_clause) in packed(node_box), \
        "the page fixture does not exercise a sentence wrapped between box rows"
    assert packed(pod_clause) in packed(headless), \
        "the headless fixture does not exercise a word cut in half by the 80-column wrap"
    assert packed(workload) not in packed(fenced(page3)), \
        "the page fixture draws the workload clause in a fence, so it proves nothing about the "
    # A framed row with less trailing pad is the frame, not drift.
    check(verdicts, (ops3, tests3, page3.replace("accepted it.             │", "accepted it.  │")),
          0, "a framed verdict row with less trailing pad")

    # The rewording, in each artifact and in each shape each one draws.
    check(verdicts, (ops3.replace("and accepted it", "and allowed it"), tests3, page3), 1,
          "a reworded ops.rs verdict", "is not")
    check(verdicts, (ops3, tests3.replace("with the cluster first", "with the cluster up front"),
                     page3), 1, "a reworded ui_tests.rs verdict", "is not")
    check(verdicts, (ops3, tests3, page3.replace("the cluster checked it first and accepted it\n",
                                                 "the cluster checked it first, and accepted it\n")),
          1, "a reworded headless row on the page", "draws no headless row for ACCEPTED")
    # **One drawn row of several.** The shape check above still finds the other
    # box, so this is the count and nothing else.
    check(verdicts, (ops3, tests3, page3.replace(restart_box,
                                                 restart_box.replace("checked it first",
                                                                     "checked this first"))),
          1, "one drawn verdict row of two reworded", "copy/copies of")
    # **`ui::spoken` does not raise the product's own name**, and this is the case
    # that says so: a page drawing *"K8rs did not check…"* is a page the product
    # cannot produce.
    check(verdicts, (ops3, tests3, page3.replace("│  k8rs did not check", "│  K8rs did not check")),
          1, "a drawn row that raised the product's own name", "draws no drawn row for UNCHECKABLE")
    # **The headless row is compared whole, not as a substring.** `UNCHECKABLE`'s
    # constant is a prefix of its drawn form, so a page that had stopped printing
    # the headless shape would satisfy a `contains` with the drawn one and this
    # guard would vet one copy while claiming two.
    check(verdicts, (ops3, tests3,
                     page3.replace("k8rs did not check this one with the cluster first\n", "", 1)),
          1, "a page that draws UNCHECKABLE only as a dialog does", "draws no headless row")

    check(hedges, (ops3.replace("and something there may delay this or",
                                "and anything there may delay this or"), tests3, page3), 1,
          "the only ops.rs copy of a clause reworded", "src/ops.rs does not say")
    check(hedges, (ops3, tests3.replace("has not checked whether anything did",
                                        "has not checked whether one does", 1), page3), 1,
          "one ui_tests.rs copy of two reworded", "copy/copies of")
    check(hedges, (ops3, tests3, page3.replace(pod_box, pod_box.replace("normally    │",
                                                                        "usually     │"))),
          1, "one page copy of two reworded", "copy/copies of")
    check(hedges, (ops3, tests3, page3.replace("unread by k8rs, may │",
                                               "unread by k8rs, might │")), 1,
          "the only page copy of a clause reworded", "screens/dialogs.md does not say")
    check(hedges, (ops3, tests3, page3.replace(bullet, "")), 1,
          "a page that stopped specifying the workload clause", "screens/dialogs.md does not say")
    # **A fragment that is not part of its own sentence** — the table above
    # mistyped — counts nothing it claims to and would be reported as the file's
    # fault rather than this guard's.
    check(copies, (UI_TESTS, packed(tests3), pod_clause, ("whatever built it",)), None,
          "a fragment this guard's table pairs with the wrong sentence", "vet nothing")

    # **The other direction.** A clause can stay in the file while the arm that
    # carried it is reworded out of every hedge — here a fourth kind that hedges
    # with nothing, which is a dialog promising more than its call knows.
    unhedged = removal.replace(
        '        "pod" => (\n',
        '        "replicaset" => (\n'
        '            ApiResource::erase::<ReplicaSet>(&()),\n'
        '            "This removes the replicaset and every pod it manages.".to_string(),\n'
        '            true,\n'
        '        ),\n'
        '        "pod" => (\n', 1)
    check(hedges, (verdict_consts + unhedged, tests3, page3), 1,
          "an arm that hedges with none of them", "hedges with none of them")

    # Every way an extraction can come back empty, on each of the two new facts.
    for why, args in {
        "a renamed ops verdict": (ops3.replace("ACCEPTED", "AGREED", 1), tests3, page3),
        "a deleted ops verdict": (ops3.replace(verdict_consts.splitlines()[0] + "\n", ""), tests3,
                                  page3),
        "a duplicated ops verdict": (ops3 + verdict_consts.splitlines()[0] + "\n", tests3, page3),
        "a renamed ui_tests verdict": (ops3, tests3.replace("UNCHECKABLE", "UNCHECKED", 1), page3),
        "a duplicated ui_tests verdict": (ops3, tests3 + verdict_consts.splitlines()[1] + "\n",
                                          page3),
        "an escape in a verdict": (ops3.replace("the cluster checked",
                                                "the \\\"cluster\\\" checked", 1), tests3, page3),
        "a page with no fences at all": (ops3, tests3, page3.replace("```", "~~~")),
        "an empty ops.rs": ("", tests3, page3),
        "an empty ui_tests.rs": (ops3, "", page3),
        "an empty page": (ops3, tests3, ""),
    }.items():
        check(verdicts, args, 1, why, "vet nothing")
    # Two verdicts that became one sentence: a dialog could then say *checked*
    # over an operation that sent no check, and every drawn row would still match.
    same = verdict_consts.replace("k8rs did not check this one with the cluster first",
                                  "the cluster checked it first and accepted it")
    # Asked for by its own sentence, not by the `vet nothing` every failure here
    # ends on: this fixture also trips the fragment-table check one step later,
    # so a looser `says` would pass with the distinctness test taken out.
    check(verdicts, (same + removal, same, page3), 1,
          "two verdicts that are now the same sentence", "distinct sentences")

    for why, args, says in (
        ("a renamed removal", (ops3.replace("fn removal(", "fn removed("), tests3, page3),
         "vet nothing"),
        ("a duplicated removal signature", (ops3 + removal, tests3, page3), "vet nothing"),
        ("a removal whose braces do not match",
         (ops3.replace("    };\n    Ok((resource", "    Ok((resource"), tests3, page3),
         "vet nothing"),
        ("a removal whose arms cannot be read",
         (ops3.replace("ApiResource::erase::<", "erased::<"), tests3, page3), "body holds no"),
        ("an empty ops.rs", ("", tests3, page3), "vet nothing"),
        ("an empty ui_tests.rs", (ops3, "", page3), "does not say"),
        ("an empty page", (ops3, tests3, ""), "does not say"),
    ):
        check(hedges, args, 1, why, says)

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
    print("copy-guard: self-test passed — on the verdicts: a line reworded in ops.rs, in "
          "ui_tests.rs, in the headless mockup row or in one drawn box of two is refused, a "
          "drawn row that raised the product's own name is refused, and a page that draws "
          "UNCHECKABLE only as a dialog does is refused rather than satisfied by the prefix; "
          "less trailing pad inside a frame is not a failure. On the hedges: all three clauses "
          "are read out of removal's own arms, one reworded in ops.rs, in ui_tests.rs or on the "
          "page is refused whether it is the only copy or one of several, and so is an arm that "
          "hedges with none of them. The stale-sibling case is the one a search for the sentence "
          "cannot see and it is caught by counting two non-overlapping fragments against the "
          "whole — each of the two is asserted load-bearing on its own, and a fragment this "
          "guard's own table pairs with a sentence that does not contain it is refused as "
          "*vetted nothing* rather than blamed on the file. `packed` is asserted load-bearing in each of its four shapes — Rust's `\\` "
          "continuation, a bullet wrapped at the margin, a sentence broken between box rows, and "
          "a word cut in half by the 80-column mockup — and the page is asserted searched whole "
          "rather than fence by fence, since § Delete specifies four of the six kinds in prose. "
          "A renamed, deleted, duplicated or escape-carrying constant on either side, two "
          "verdicts that became one sentence, a page with no fences, a renamed, duplicated, "
          "brace-broken or unreadable removal, and an empty file each fail as *vetted nothing* "
          "rather than as agreement")


if "--self-test" in sys.argv:
    self_test()
    sys.exit(0)

sys.exit(every())
