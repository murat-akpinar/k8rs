# Phase 12 close — the cross-box review (2026-09-26)

`k8s-admin`, CLAUDE.md § Phase close step 4. The phase added 20 019 lines across
8 files and **every box in it already had its own operator round**, so this pass
read only the class those rounds structurally could not see: the seams where two
boxes met. Five seams, named by the PM, read *against each other* rather than one
at a time.

**The settlement is NOTES § D289.** This file is the evidence that decision
cites — what was read, the command that read it, and the behaviour each reading
shows. The severities below are recorded as *what the review reported*, which is a
fact about the review; the triage itself is D289's
([D108](../NOTES.md#d108--work-with-no-phase-gets-a-file-and-measurements-get-a-directory-2026-08-16)).

## No cluster was brought up, and nothing ran on the test host

The PM held the test host for the owed pty gates and a live kind run, and asked
for no command to be run there. **Every reading in this file is a source read of
`src/`, `screens/`, `scripts/` and `backlog.md` at commit `a5bf413`.** No `cargo`,
no `just`, no guard, no binary, no `kubectl`.

**Every `file:line` below resolves against `a5bf413` and nothing later.** By the
time this file was finished a fix for findings 1 and 2 was already in flight —
`src/main.rs`, `src/main_tests.rs` and `src/views.rs` modified, +314/−72 — and
**every `src/` citation here had drifted in the working tree while still being
correct at that commit** (checked, all six spot-checks). A reader who finds a cited
line showing something else should read
`git show a5bf413:src/main.rs | sed -n '9673p'` rather than the current file. The
findings are what drifted the numbers; the numbers are not wrong.

So:

- **There is no cluster output anywhere in this file.** No object dump, no node
  name, no address, no credential-shaped value — the sanitization rule
  (`reports/README.md`) has nothing here to catch, and that is stated rather than
  left to be inferred.
- Every claim below is reproducible from the pasted command on any checkout,
  which is why the greps are pasted whole rather than summarised.
- **Three questions only a cluster can settle are open** and are listed at the
  end. The PM has a 4-node `kind-k8rs` up with `scripts/broken.yaml` and
  `scripts/healthy.yaml` applied and will run journey 1 rather than have anyone
  guess it.

---

## The seven findings, as reported, most severe first

### 1 — reported as a close blocker · D22's *Already gone* guard has no product caller

`src/main.rs:10002-10021` (`mutating`'s `ask` closure) · `src/ops.rs:495-500`,
`src/ops.rs:776-787`, `src/ops.rs:186-191` · `src/main.rs:10175-10183`

```
$ cd /home/shyuuhei/GIT/k8rs && grep -rn "Answer::Gone\|Answer::Changed" src/main.rs src/ui.rs src/views.rs src/ops.rs
src/ops.rs:182:    /// **It is what makes [`Answer::Gone`] and [`Answer::Changed`] checkable after the fact**
src/ops.rs:779:    /// with the guard the design leans on ([`Answer::Gone`]) switched off in the only shape that
src/ops.rs:1090:                Answer::Gone => Outcome::Gone,
src/ops.rs:1091:                Answer::Changed => Outcome::Changed,
```

Four hits over the four product files that could build one: two doc comments, and
`src/ops.rs:1090-1091`, which is the `match` that *consumes* an `Answer` and is not
a construction. The only constructions of either variant in the tree are
`src/ops_tests.rs:281,282,453,454,4335,4340`. **Nothing in the product builds
`ops::Answer::Gone` or `ops::Answer::Changed`.**

The console's `ask` closure, `src/main.rs:10007-10020`:

```rust
match answered.recv().await {
    Some(Some(typed)) => match checked.asks() {
        Some(_) => checked.typed(&typed),
        None => checked.pressed(),
    },
    Some(None) | None => ops::Answer::Cancelled,
}
```

No store lookup, no uid comparison, no `Gone`.

`src/ops.rs:779-782` names the intended producer in so many words — *"the guard
the design leans on (`Answer::Gone`) switched off in the only shape that exists
today, because a headless script has no watch to notice with … The value comes
from a caller that already has it — Phase 11's dialog off the watch running
behind the modal, which is where NOTES § D22 put it."* That caller is Phase 12's
wiring, and it did not arrive.

**What the two verbs then do.**

| key | precondition on the wire | what protects the operator | state today |
|---|---|---|---|
| `ctrl-d` delete | `preconditions.uid` (NOTES § D235) | the server answers `409` | contained; the *screen* is a raw 409, not D22's box |
| `r` restart | **none** — `PatchParams` has no `preconditions` field (`src/ops.rs`'s own doc: *"`PatchParams` has none, which is why `scale`'s own case is answered by the label and not by a guard"*) | D22's watch-side guard **only** | unprotected |

**The concrete failure.** `r` on a Deployment card. The dialog opens, the dry-run
goes out, the reader reads the consequence and presses `⏎` — a real round trip
plus human reading time. In that window a CI apply or an ArgoCD prune-and-sync
deletes and recreates `payments/web`. The patch lands on the new instance, and the
audit line names the uid k8rs *read*. `src/ops.rs:186-191` records that exact
measurement against a real cluster: a Deployment deleted and recreated between the
dry-run and the yes left the record naming a uid that nothing changed, beside a
`PATCH` that landed on a different instance — *"The value is honest; the label was
not."*

For `ctrl-d`, the StatefulSet-name-reuse journey that the box's own evidence line
says was reproduced twice against kind now produces a different screen than
`screens/dialogs.md` § *The object went away while the dialog was open* draws: the
cluster's 409 inside `Modal::Refused`, not the *Already gone* box.

**This is not NOTES § D228's reversal, and the objection is answered here because
that is where a future reader will look for it.** D228 removed a
**resourceVersion** precondition, because that field moves when nothing about the
object changed — measured, a `CrashLoopBackOff` Deployment whose spec never moved
wrote 20 times in 99.4 s. What is missing here is **uid identity**, which is
immutable, differs only when the object genuinely is a different one, and which
`delete` already sends as `preconditions.uid` under NOTES § D235. The two are
different questions about different fields.

**Why the obvious fix does not fit, recorded so the box is not priced as bigger
than it is.** `pump` holds `store: &mut k8s::Store`, so the `mutating` future
cannot also hold `&Store`. But `keyed` already takes `store: &k8s::Store` and
derives `cards` from it, and `over_modal`'s confirm arm is where `Did::Answered`
is built — so the uid-vs-store answer can be decided there and travel on
`Did::Answered`, with `ask` returning `ops::Answer::Gone` (a freely constructible
variant) when it says gone. That is `main.rs` only; no frozen file is touched.

### 2 — reported as a close blocker · the refusal box draws `⏎ open` and `⏎` reaches nothing

`src/views.rs:3015` vs `src/main.rs:9671-9682` · `src/main_tests.rs:17099-17127` ·
`screens/widgets.md:505`, `screens/dialogs.md:1448`, `screens/dialogs.md:1590`

The footer side, `src/views.rs:3015`:

```rust
Some(Modal::Refused { .. }) => return (Cow::Borrowed("esc dismiss  ⏎ open"), ""),
```

pinned by `src/views_tests.rs:3336` and `:3576`, specified twice in
`screens/dialogs.md` (state 0, the `409`; state 1c, the cluster's own refusal) and
once in `screens/widgets.md:505`. The arm's own doc says why: *"`Refused` reports
on a write that was already sent, so the object it was about is still selected
underneath and `⏎` still opens it."*

The router side, `src/main.rs:9671-9682`:

```rust
// Every other box is dismiss-only (`views::Modal::Refused`, `Gone`, `Unconnected`, and the
// container picker, which has nothing to pick until the pod read is wired).
Some(_) => match key.code {
    KeyCode::Esc => {
        if console.app.escape(open) { Did::Quit } else { Did::Changed }
    }
    _ => Did::Nothing,
},
```

`Enter` is `Did::Nothing` — no frame, no state change, nothing said.

**Live on a shipped journey.** `reports/2026-09-26-the-error-state-pass.md`
produced `Modal::Refused` for a real 403, a real 401 **and a real 409**. Restart a
Deployment on a login that lacks `patch deployments`; the box opens; the footer
under it reads `esc dismiss  ⏎ open`; `⏎` does nothing, twice, three times.
PRIOR-ART § G1 — *refuses for no visible reason* — at the worst moment to press a
key.

**The 409 is the sharper case, and it is the one the pass already ran.** A server
`409` is `Fault::Conflict`, and `ui::refused` (`src/ui.rs:2431`) takes its first arm
at `src/ui.rs:2471` — `(Fault::Conflict, _)` — drawing the title *The object changed
first* over `views::MOVED` and `views::REREAD` (`src/views.rs:3867`, `:3870`):
*"something else changed this object while k8rs was working on it — reading it
again shows what it looks like now."* **The box's own body names re-reading the
object as the next step, and `⏎ open` — the key that would do it — is the dead
one.** So this finding's blast radius is not only the permission journeys: it
includes the 409 journey `reports/2026-09-26-the-error-state-pass.md` recorded as
behaving as specified, because the footer it drew was read and the key under it was
not pressed.

**The cross-box half, which is why no round saw it.** `src/main_tests.rs:17099`:

```rust
/// **A dismiss-only box closes on `esc` and on nothing else** (`views::Modal::Refused`, `Gone`: …)
#[test]
fn a_terminal_box_closes_on_esc_and_ignores_every_other_key() {
    …
    for ignored in ['q', '?', 'r', 'X'] {
```

The title says *ignores every other key*; the doc says the box *"offers only `esc
dismiss`"*, which is false of `src/views.rs:3015`; and the loop never presses
`Enter`, the one key the footer names. Two boxes answered one question
differently, and the Phase 12 test wrote the Phase 12 answer down as the
specification.

**`screens/` is not part of this finding.** The review's first reading had
`screens/widgets.md:505` (`Gone` → bare `esc dismiss`) contradicting
`screens/dialogs.md:1448` (the 409 box → `esc dismiss  ⏎ open`), and called
widgets.md's row wrong. **It is not wrong and the claim is withdrawn** — the two
rows are about two different boxes, and a 409 reaches the one widgets.md gives
`⏎ open` to. Finding 7 carries the withdrawal and the reason, kept visible so the
same suspicion is not re-derived. **Both pages already specify this footer
correctly; the defect is entirely in the router.**

### 3 — reported as should-fix, high · `/` and `n` are bound with no detail guard, so the list filter is edited and destroyed from a screen the list is not on

`src/main.rs:9546-9553` vs `screens/widgets.md:743-750`, `screens/detail.md:44`,
`src/views.rs:3068`

Every key arm reachable with no modal open and no filter focused:

```
$ cd /home/shyuuhei/GIT/k8rs && awk 'NR>=9483 && NR<=9561' src/main.rs | grep -oE "KeyCode::(Char\('.'\)|[A-Z][a-z]+)" | sort -u | tr '\n' ' '
KeyCode::Char('/') KeyCode::Char('?') KeyCode::Char('[') KeyCode::Char(']') KeyCode::Char('d') KeyCode::Char('f') KeyCode::Char('j') KeyCode::Char('k') KeyCode::Char('l') KeyCode::Char('n') KeyCode::Char('r') KeyCode::Char('X') KeyCode::Char('y') KeyCode::Down KeyCode::Enter KeyCode::Esc KeyCode::Tab KeyCode::Up
```

The range is `pressed`'s mode `match` only. `q`, `ctrl-c`, `ctrl-d` and `ctrl-z`
are answered above it (`src/main.rs:9418`, `:9471`, `:9478`) and are absent here
for that reason, not for being unbound.

`[`, `]` and `f` each carry `if open != views::Detailing::Closed`. `/` and `n`
carry **no guard at all**, and they write `App::filters` — the filter that narrows
the Alerts cards and the browser rows, i.e. the list the detail is drawn *over*.

What the screens rule. `screens/widgets.md:743`: *"Detail, Help, every dialog on
dialogs.md and the container picker have no `/ filter` or `n namespace` on their
own closed footers, **but none of them touch `App::filters` to get there**."*
`src/views.rs:3068` refuses `/` on the which-pods step by name.
`screens/detail.md:44` describes `/` in the logs tab as *"the same `/` every other
pane already carries silently"* — that is, a **text search over the log**, which
does not exist.

**Concrete failure A — the reader's committed filter is destroyed.**

1. `/ web` `⏎` on Alerts. One card left, `filter: "web"` drawn above it.
2. `⏎` opens the detail.
3. In the logs tab press `/`. The footer is replaced by
   `filter: web  ⏎ done  esc clear filter` — `views::App::footer`'s typing arm
   answers *before* its `Detailing` arms, so the detail's own footer is gone.
4. `esc` → `App::escape` (`src/views.rs:3291-3297`) reaches `buffer.clear()`.
5. `esc`, `esc` → typing off, detail closed.
6. Alerts is the full three-card list with no `filter:` line.

The filter is gone and the reader never saw it go, because `narrowed()` — the
at-rest `filter: "web"` row — is only called from the Alerts and browser arms
(`src/ui.rs:3954`, `src/ui.rs:4463`), and `content()` returns at
`src/ui.rs:3400` the moment `screen.detail` is `Some`.

**Concrete failure B — the list is silently narrowed.** No filter; open a detail;
press `/`, type `panic` (a word off the log), `⏎ done`. Nothing in the detail
changes. `esc back` → Alerts draws *No problems match "panic".* from a string
typed to search a log.

Both fire on the which-pods step too, which `screens/detail.md` § *Picking a pod*
refuses `/` on by name.

### 4 — reported as should-fix · NOTES § D282's behavioural pin is absent, and a spelling of one pinned sentence lives where `copy-guard.py` does not read

`src/main_tests.rs:16971`, `:17020`, `:17385` · `src/ops.rs:976`, `:982` ·
`scripts/copy-guard.py:74-75`

Direct answer to the seam-4 question: **the pin fell between D282 and the wiring
box.** What does exist:

- `ops.rs`'s own branch is pinned behaviourally by `src/ops_tests.rs:400`
  (checkable → `ACCEPTED`), `:1227` and `:4592` (uncheckable → `UNCHECKABLE`).
- The *headless driver's* transport is pinned by `src/main_tests.rs:12072`
  (`confirmation`, `checkable: false`, asserting `UNCHECKABLE` verbatim and that
  it lands before the prompt) and `:13197` / `:13431` (the scale path, `ACCEPTED`
  verbatim).
- The **console dialog's** transport — `ops::Checked::verdict()` →
  `Published::Checked.verdict` → `Dialog::verdict` → the drawn box — is pinned by
  nothing. `each_verb_shows_its_own_command_and_nothing_is_sent_without_an_answer`
  (`src/main_tests.rs:17177`) drives `mutating` for real for `RESTART` and
  `DELETE` against a refusing client and reads `published.first()` only; it
  asserts `kubectl`, `verb`, `object.name()`, `object.namespace()` and
  `object.kind`, and never the `Published::Checked` that follows.

And the fixtures used in its place invented a sentence:

```
$ cd /home/shyuuhei/GIT/k8rs && grep -n "cannot check this one first" src/main_tests.rs
16971:    dialog.verdict = Some("k8rs cannot check this one first.");
17020:    dialog.verdict = Some("k8rs cannot check this one first.");
17385:                verdict: "k8rs cannot check this one first.",
```

`ops::UNCHECKABLE` (`src/ops.rs:982`) is
`"k8rs did not check this one with the cluster first"`, and `ui::spoken` leaves
`k8rs` lower case and appends a stop, so the drawn form is
`"k8rs did not check this one with the cluster first."` —
`"k8rs cannot check this one first."` is therefore **a spelling of that sentence
that nothing in the product produces** — the second vocabulary for one string that
D282's guard exists to refuse. No count is given here on purpose: the number of
copies is the thing that has gone stale four times in this repo, and `copy-guard.py`
prints it on every run. And:

```
$ cd /home/shyuuhei/GIT/k8rs && grep -n "^OPS\|^UI_TESTS" scripts/copy-guard.py
74:OPS, UI, RULES, PAGE = "src/ops.rs", "src/ui.rs", "src/rules.rs", "screens/help.md"
75:UI_TESTS, DIALOGS = "src/ui_tests.rs", "screens/dialogs.md"
```

`src/main_tests.rs` is not in the guard's file list.

**The one consequence that would have made this a blocker was checked and is not
one.** `ui::confirm`'s row budget counts the verdict's wrapped rows
(`src/ui.rs:2287-2296`, `hard = 1 + verdict.len() + 1 + field + 1`). At
`room(CROWDED_BOX)` = 61 − 2 = **59 columns**, the real spoken sentence is **51
characters** and the invented one **33** — both wrap to one row, `hard` is
unchanged, and the 80×24 frame NOTES § D284 ruling 3 was measured on is *not* a
row short of the real extreme.

`src/main_tests.rs:14775`'s `verdict: "Type the name to confirm."` is a prompt in
a verdict slot — harmless to what that test asserts (the `⌫` guard), and another
string in that slot the product cannot produce.

Not flagged: the `"The cluster checked it first and accepted it."` fixtures in
`src/views_tests.rs` and `src/main_tests.rs` are the correct **drawn** form of
`ops::ACCEPTED` under `ui::spoken`, not an invention.

### 5 — reported as should-fix · `?` names `c container` and `⇧p previous`; neither key is bound anywhere

`src/ui.rs:418` (`HELP`) · `src/main.rs:9483-9560` · `src/views.rs:1029-1036`

```
$ cd /home/shyuuhei/GIT/k8rs && sed -n '418p' src/ui.rs
       in the log tab:  f follow · c container · ⇧p previous

$ cd /home/shyuuhei/GIT/k8rs && grep -n "KeyCode::Char('c')\|KeyCode::Char('P')" src/main.rs
9471:    if (key.code == KeyCode::Char('q') && !control) || (key.code == KeyCode::Char('c') && control) {

$ cd /home/shyuuhei/GIT/k8rs && grep -rn "ContainerPick" src/main.rs
(no output)
```

**One hit, and it is `ctrl-c`** — guarded by `&& control`, so it is `q` and not the
container picker (`src/main.rs:9471`). `⇧p` has no occurrence at all. Neither key
appears in the mode `match` at all:

```
$ cd /home/shyuuhei/GIT/k8rs && awk 'NR>=9483 && NR<=9561' src/main.rs | grep -c "KeyCode::Char('c')\|KeyCode::Char('P')"
0
```

`c` can therefore only reach `over_modal`, which runs when a modal is *already*
open, so nothing in the product can **open** `views::Modal::ContainerPick` — which
the third grep above confirms from the other end. The key inventory in finding 3
has no `Char('c')` and no `Char('P')`.

Two things are already honest and are recorded here so a reader does not re-derive
them:

- `src/views.rs:1029-1036`'s `#[expect(dead_code, reason = …)]` on the variant
  says *"`c` opens it off the pod read, and no box fetches the four detail tabs
  yet"* — correct.
- The **footer** does not lie. `ui::containers` (`src/ui.rs:1274-1279`) reads
  `logs.pod.containers` off a pane `drawn` always fills with
  `views::Pane::Loading` (`src/main.rs:9044-9050`), so `containers(screen).len()`
  is 0, `picking(open)` is false, and `c container` never reaches the line.

`?` is the one surface promising both keys unconditionally.
`backlog.md:3309` holds half of this — `c` on a single-container pod, framed on
2026-09-18 as a footer/Help disagreement, when `c` was assumed bound in the
multi-container case. The `⇧p` half is in no file. The true statement is stronger
than either: both keys are unbound in every state. **The shape of an answer already
exists in the same `const`**: `src/ui.rs:422-423` is `s`'s own pair of rows, *not
built yet — there is no way yet to type a copy count*, which is what this screen
already says about a key `screens/` has specified and no box has built.

### 6 — reported as a nit · a doc comment describes a `c` handler that does not exist

`src/main.rs:9352-9356`

```rust
/// **`containers: 0` is *there is nothing to pick*, and it is the honest answer today**: the read
/// that would know how many a pod has is not wired, so `c` opens nothing rather than a picker over
/// a list nobody has …
```

`c` opens nothing because there is no `c` arm, not because `containers` is 0. A
reader of this comment concludes the key is wired and gated. The day the pod read
lands, `containers` becomes non-zero, the footer starts drawing `c container`, and
`c` is still dead.

### 7 — reported as a nit · `settled`'s `Changed` arm would draw *Already gone* — latent, and by NOTES § D289 it stays latent

`src/main.rs:10177` · `src/ui.rs:2596-2618` (`ui::gone`) · `src/ui.rs:2471`
(`ui::refused`'s `Conflict` arm)

```rust
ops::Outcome::Gone | ops::Outcome::Changed => object.map(|object| views::Modal::Gone { … })
```

One arm, two outcomes, and only one of them is what the box says. `ui::gone`
(`src/ui.rs:2596-2618`) draws the title `Already gone` and the sentence *"This
deployment is already gone — something else removed it while this was open. Nothing
will take its place on its own."* — true of `Outcome::Gone` and false of
`Outcome::Changed`, whose object still exists.

**The right sentence for a `Changed` already exists in the product**, one box over:
`ui::refused`'s `(Fault::Conflict, _)` arm at `src/ui.rs:2471` draws *The object
changed first* over `views::MOVED` / `views::REREAD`. So this is not a missing
screen — it is `settled` folding two outcomes into the one of two existing boxes
that can only describe the first.

**It is unreachable today** (finding 1: nothing produces `Outcome::Changed`), and
the review first reported it as *becoming live when finding 1 is fixed*. **NOTES
§ D289 rules otherwise, and the ruling is the reason it stays here as a nit:** the
fix in finding 1 answers one question — is the uid still in the store. *Absent* is
`Gone`; *present but moved* is exactly what NOTES § D228 says must not stop a
`scale` or a `restart`, so **nothing will produce `Answer::Changed`**. That
variant's own doc calls it *the `409` mechanic*, and the security gate's 409 row is
explicit that "applies" means a read-modify-write, which today is only v0.4's
`edit`. So the second box is `edit`'s to bring, and `views.rs` reopening for it
then is the ordinary recorded reversal (NOTES § D273, § D278 ruling 5) rather than
something this close must pre-build.

#### Withdrawn: the `screens/` pair this finding claimed was owed

**The review also reported that `screens/widgets.md:505` and `screens/dialogs.md`
§ state 0 contradict each other about the 409 box's footer, and that widgets.md's
row was the wrong one. That is withdrawn — nothing in `screens/` is owed.** It is
kept here rather than deleted because the suspicion is easy to re-derive and a
future reader should find out here why it does not hold. Both the review and the
PM's first draft of D289 made the same reading.

**They are two different boxes.** A server `409` never travels as
`Outcome::Changed`. It arrives as `Fault::Conflict` inside `Outcome::NotSent` or
`Outcome::Failed`, `settled` turns both into a **`Modal::Refused`**
(`src/main.rs:10184-10193`), and `ui::refused`'s first arm — `(Fault::Conflict, _)`
at `src/ui.rs:2471` — draws the title *The object changed first* over
`views::MOVED` / `views::REREAD`. That is `screens/dialogs.md` § state 0 word for
word, and it is what `reports/2026-09-26-the-error-state-pass.md` observed off a
real 409.

So `screens/dialogs.md` state 0's `esc dismiss  ⏎ open` **is** a `Refused` footer —
the row `screens/widgets.md:505` itself gives `Refused`. widgets.md's bare `esc
dismiss` is about `Modal::Gone`, which is a different box, reached only by the
client-side guard finding 1 shows has no caller. The two pages agree; what does not
work is the key, which is finding 2.

**What is left of this finding is therefore code-only and latent:** `settled`'s
`Gone | Changed` arm (`src/main.rs:10177`) would send a `Changed` to a box titled
*Already gone*, and by the ruling above no `Changed` will ever arrive.

---

## What was checked and found clean, and how

This is the half a future reader does not need to re-derive. Each entry names what
was read and what makes the reading hold.

### Seam 1 — the strip and its doors: complete

Question asked: *is every string that reaches a dialog or a cell actually
stripped, or does one of the doors NOTES § D284 added have a sibling nobody
named?*

- Every string reaching `ui::Screen` and the modal layer was enumerated against
  the leftover list at `src/ui.rs:632-673`. **The list is complete.**
- `views::Object::new` (`src/views.rs:1396-1414`) is the only setter of both
  fields. Since D284 ruling 2 `namespace` (`src/views.rs:1348`) and `name`
  (`:1351`) carry **no `pub` at all**, so `object.name = value` no longer compiles
  from outside the file — which is the half the first mechanism lacked, when it
  rested on `uid` being private and two tests were assigning the field.
- The **19** `views::Stripped::of` / `views::Object::new` call sites in `main.rs`
  cover every string the console assembles: `grep -n "Object::new\|Stripped::of"
  src/main.rs` answers 20 lines, of which `src/main.rs:8679` is a doc comment.
- `views::Log::push` (`src/views.rs:1867`) spends the strip internally, so
  `log.ran` and `log.outcome` cannot route around it.
- The four modal-layer strings that carry no `Stripped` were each traced to a
  strip one layer down:
  - `Modal::Refused::said`, `Modal::Unconnected::said` → `k8s::said` /
    `k8s::watch_said` at `k8s::FREE_TEXT`;
  - `Modal::Unconnected::to`, `Screen::contexts` rows → `k8s::drawable` at
    `k8s::IDENTIFIER`;
  - `Modal::Unconnected::renewal` → `k8s::renewal` (`src/k8s.rs:8089`), which is
    `drawable(auth.exec.command)` and **not** a fifth hand-written copy of the
    filter;
  - `Modal::Unconnected::coverage` → every arm's string has passed
    `k8s::namespace_name`.
- Two doors the list does not name with that word are covered by its
  `Screen::detail` paragraph, and both were checked: the container-picker rows are
  `logs.pod.containers` off a `rules::PodSnapshot` (ingest), and `Screen::kinds`
  is `k8s::Browsable`, which `src/k8s.rs:4278` routes through `k8s::ingest`.

**No unnamed sibling door.**

### Seam 2 — the `any`-vs-all root cause: NOTES § D285's defect exists in one place only

Question asked: *does any other reader of watch health have the `any`-vs-all
defect D285 fixed in one place?*

Every reader of watch health was read:

| reader | shape | verdict |
|---|---|---|
| `linked()` `src/main.rs:9237` | `dropped && !answering`, `answering` = some watched kind has no row | fixed; both halves hold |
| `vitals()` `src/main.rs:9176` | `any(kind == Node && !listed)` | per-kind and correct — a node vital that cannot be read is blank, never guessed |
| `drawn`'s banner `src/main.rs:9010` | `unreadable(&troubles, …)` then `said.first()` into `Pane::Denied` | one sentence per trouble in `Store::troubles`' declared order, so a pod drop takes the banner from a permanently-refused node watch rather than the reverse |
| `ui.rs:3436` `○ nothing is broken` | `Pane::Ready(cards) if empty && link == Live` | interlocked: a non-empty `troubles` makes `alerts` `Denied`, so `Ready` implies no troubles |
| `ui.rs:3736` the clock sentence | `.filter(\|_\| screen.link == Live)` | same interlock |
| `unreadable`, `pods_unread`, `read_so_far`, `too_slow` | `src/main.rs:2793`, `:4100`, `:4154`, `:4195` | the `--live` / `--once` driver's, outside the console — `unreadable` is the one the console also calls, in the row above |

**The one other `any` over a should-be-unique row was found and is safe.**
`tls_unverified` (`src/main.rs:8656`) is
`contexts.iter().any(|row| row.current && row.insecure)`. Two contexts can share a
name, so the question was whether two rows can be `current` and the `any` read the
wrong one's TLS setting. `src/k8s.rs:7709` and `:7715` set **both** `current` and
`insecure` to `false` on a `shadowed` row, so at most one row is `current` and the
`any` can only reach it.

**No other reader has D285's defect.**

### Seam 3 — scroll, modal replacement, the parked halt: consistent

Question asked: *is `App::scroll`, the filter, the container-picker state and the
command log each either reset or deliberately kept across a switch, and does the
code agree with NOTES § D268 about a filter not outliving the list it narrows?*

- `views::App::switched()` (`src/views.rs:2798`) is `*self = App::default()`,
  which resets `scroll`, `filters`, `typing`, all four cursors, `tab`,
  `following` and `modal` — so the container-picker state goes with them.
- `main::switched` (`src/main.rs:8312-8320`) then resets `opened`, `contexts`,
  `insecure`, `clock`, `kinds`, `unconnected` and `context`, and on **failure**
  leaves `log` alone. That is D280 item 2's ruling; `connected()`
  (`src/main.rs:8230`) empties it on **success** instead, beside the new context's
  first line.
- `console.writes` is untouched by either, which is NOTES § D279's requirement
  that `--read-only` outlive `X`.
- `App::rewound()` is called on a resize (`src/main.rs:9388`), on `esc` out of a
  detail (`:9521`), and on all three ways a detail opens (`:9842`, `:9879`,
  `:9901`). `following` is deliberately left, per `src/views.rs:3438`'s last
  paragraph.
- `App::open` (`src/views.rs:3395-3403`) clears `filters` on a real view change,
  which is D268's ruling. **The only place the ruling does not hold is finding
  3** — and that is a key with no guard, not a state that survives a view change.
- `parked` / `console.carried` was traced through both `pump` calls: the slot is
  drained at `src/main.rs:8851` *before* a key is read or a frame is drawn, both
  park sites `return` immediately (`:8892`, `:8898`), and the inner mutating
  frame's `continue` (`:8125`) reaches the outer `pump`. So the
  `debug_assert!(carried.is_none())` at `src/main.rs:7652` cannot be reached from
  this loop.
- The three `continue` guards in the `Halt::Mutate` arm (`src/main.rs:8080-8087`)
  were checked for a stranded `App::changing`, which would freeze the footer on
  `changing … first` and make `may_quit` false for the life of the run. **All
  three are unreachable**: `changing` is only set in `over_modal`'s confirm arm,
  *after* the dialog exists, and each guard's precondition already makes
  `may_mutate` false — no audit `File` means `Writes::ReadOnly` or
  `Writes::Unaudited`, and no session means an empty store and therefore no cards.
- `Owing` (`src/main.rs:7562-7592`) is a throttle, not a debounce: `owed()` keeps
  `already.min(at)`, so a storm cannot push the deadline out and the last event of
  a burst is inside the window by construction. That is invariant 7 and
  PRIOR-ART § A5's own manoeuvre.

### Seam 5 — `--read-only` and the write path end to end: two independent layers

Question asked: *after a cluster switch, and after a failed connect that builds
`Modal::Unconnected`, is every mutation still unreachable — and is `may_i` still
reachable?*

- `audit_log_for(read_only, ops::audit_log)` answers `None` under the flag
  (`src/main.rs:7874`), so `writes` is `ui::Writes::ReadOnly` and **no audit log
  is opened at all** (NOTES § D278).
- Layer one: `ui::withheld` answers `Some(Held::Off(..))`, `ui::offered` returns
  `Offer::Move`, and `views::App::may_mutate` is false **per key** (NOTES § D269),
  so no press produces `Did::Mutate`.
- Layer two: the `Halt::Mutate` arm has no `File` and `continue`s.
- `Screen::writes` lives on `Console`, not `App`, so it survives
  `App::switched()` — `--read-only` outlives `X`.
- After a failed connect, `Modal::Unconnected` makes `modal.is_none()` false. After
  `esc` dismisses it (`src/views.rs:3307-3319`) the store is
  `nothing_connected`'s empty one, so there are no cards, `offered` returns
  `Offer::Nothing`, and `cluster.session` is `None` at the `Halt::Mutate` arm.
- `ops::may_i` is the headless `k8rs ops may-i` path through `ops_line`; the
  console does not call it, so NOTES § D230 ruling 3 is untouched by anything in
  this phase.

### Also read and deliberately not re-raised

- **`SCALE_IS_BUILT = false` is coherent end to end.** `src/views.rs:2611` is
  `false`, `Offer::act` withholds `s` for every kind, `s` is absent from the key
  inventory in finding 3, and `HELP`'s `s` rows (`src/ui.rs:422-423`) read *not
  built yet — there is no way yet to type a copy count*. Footer, router and help
  agree — which is what findings 5 and 6 do **not** have for `c` and `⇧p`.
- **The browser `Table` fetch, the four detail reads, the log stream and
  `may_i_in` are wired and boxed nowhere.** This is already on record at
  `NOTES.md` § *What this box did not wire, and why the phase cannot close on it*
  (inside the D274 material), which rules it into phase-close triage. Not
  re-reported.
- `docs/maps.md:54`'s self-contradiction about when `views.rs` freezes was named
  by the PM as already owned by the docs-sync step; no time spent.

---

## The three questions only a cluster can settle — open

1. **Does `r restart` on a recreated Deployment land on the new instance?**
   Finding 1 rests on `PatchParams` carrying no `preconditions` field and on
   `src/ops.rs:186-191`'s recorded measurement, not on a run made here. The
   journey:

   ```
   kubectl create deploy web -n payments --image=nginx
   # in the console: press r on the web card, leave the dialog open
   kubectl delete deploy web -n payments
   kubectl create deploy web -n payments --image=httpd
   # then press ⏎
   ```

   Expected if the finding holds: exit 0; the audit line names the uid read before
   the dialog opened; `kubectl get deploy web -n payments` shows a `.metadata.uid`
   that differs from it; and the **new** object's
   `.spec.template.metadata.annotations` carries a `restartedAt` stamp. Expected
   if the finding is wrong: a 409 or a 404.

   *The PM has a 4-node `kind-k8rs` up with `scripts/broken.yaml` and
   `scripts/healthy.yaml` applied and will run this rather than have anyone guess
   it.*

2. **Does `just mutants-diff` over the wiring diff kill a mutant in `mutating`'s
   `ask` closure?** If replacing `checked.verdict()` with a literal survives,
   finding 4's missing pin is stated by a tool with no incentive rather than by a
   reading.

3. **Does `k8s::Store::troubles()` list a watched kind before its first LIST
   returns?** `linked()`'s `answering` predicate reads absence-of-row as evidence
   that the cluster is live, so at t=0 with five watches spawned and none answered,
   `answering` is `true` and the link is `Connecting` because the snapshot is
   `None`. That is the right word, but it is reasoned from `troubles`' contract
   rather than measured. A probe printing `store.troubles().len()` on the first
   three frames of a launch settles it.
