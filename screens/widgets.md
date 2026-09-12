# screens/widgets.md — how the screens are actually drawn

The other files in this directory say **what a screen looks like**. This one
says **what draws it**: which ratatui widget, which state object, who owns that
state. It is the bridge between the mockups and `views.rs` / `ui.rs`, written
before the code phase so that Phase 11 is transcription and not design.

Target: **ratatui 0.30** with the crossterm backend
([docs/tech-stack.md](../docs/tech-stack.md)). The version matters — 0.30 split
the crate into `ratatui-core` / `ratatui-widgets` and the convenience
initialisers below only exist from 0.28.1.

Colours are **not** here (they are `theme.rs`), keys are **not** here (they are
[NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted) and
[help.md](help.md)), content is **not** here (that is the mockups). One fact,
one place.

## 1. The frame

Every screen is the same frame. Only the content pane changes, which is why
there is one layout function and not seven.

```
Layout::vertical([
    Constraint::Length(1),     // header    — vitals · k8rs · context
    Constraint::Min(0),        // body      — sidebar + content pane
    Constraint::Length(4),     // command log — bordered block, 2 lines inside
    Constraint::Length(1),     // footer    — the keys valid right now
])

body → Layout::horizontal([
    Constraint::Length(20),    // sidebar — fixed, never proportional
    Constraint::Min(0),        // content pane — takes the rest
])
```

- **The sidebar is a fixed 20 columns, not a percentage.** A percentage makes
  the nav labels reflow every time the terminal is resized; the labels are
  fixed-length strings and the eye should find them in the same place.
- The mockups are drawn 70 columns wide so they fit on a page and inside the
  80×24 minimum. At 80 columns the extra 10 go to the **content pane** — the
  sidebar does not grow.
- **The header is its own row above the frame, not titles on the border.**
  Three zones on one line: cluster vitals left, `k8rs` centred, context right.
  Two border titles could not hold three zones — the sidebar's `┬` pins the
  left title to 20 columns, and the name has to sit in the middle of the whole
  width, not the middle of the content pane.

## 1a. The header row

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
```

| Zone | Holds | Alignment |
|---|---|---|
| left | cluster vitals — `nodes 3/3`, and how stale they are when the connection is gone | left |
| centre | `k8rs` | centred on the **full** width |
| right | context · namespace scope · connection state · `admin` / `read-only` · a TLS warning · `changing…` | right |

- **The right zone is one string, its segments joined by ` · ` in the table's
  order above, `changing…` last of all** — after `admin`/`read-only` and any
  TLS warning, never ahead of them, matching how
  [dialogs.md](dialogs.md#while-the-call-is-running) already draws it
  (`ctx: prod-eu · live · admin · changing…`). **It shortens from its front
  behind a visible `…` when the row is too narrow — it is never clipped, but
  it is no longer true that it is never truncated.** Which end gives way is a
  security question, not a layout preference: the tail carries `read-only`
  and a TLS warning, what the reader believes they are allowed to do, so the
  cluster's own name is what erodes first and the tail — `changing…`
  included — never does (`ui::shortened`,
  [NOTES § D249](../NOTES.md#d249--the-layout-box-lands-from-a-second-session-the-header-gives-way-from-its-front-and-a-refusal-keeps-the-list-it-is-about-2026-09-06)).
  `prod-eu` and `prod-eu-2` differ by one character, which is why the cut is
  marked rather than silent.
- **The centred name is dropped first when the row fills up.** It is the only
  zone carrying no information; on the disconnected and namespace-scoped
  screens it is already gone. Order of sacrifice: name, then vitals, never the
  context.
- **`nodes 3/3` is free; live CPU/RAM is not.** Node count comes from the Nodes
  watch that is already permanent ([invariant 6](../CLAUDE.md)). Real usage
  needs metrics-server, which most clusters do not have — the header would read
  blank for the majority of users. It also cannot be watched: the polling
  [todo.md](../todo.md) allows for it is deliberately narrow — 30s+, capability
  gated, and *only for what is on screen*. A permanent header widget would
  widen that into an always-on timer, which is the part that is not free
  (§6, [invariant 7](../CLAUDE.md)).
- **What replaces it: the `capacity` sidebar badge.** `capacity  1 ▲` counts
  the nodes that have promised more than they have — computed from data already
  in the store (`Node.status.allocatable` plus the pods' requests), no new API
  call and no new permission. It is a **count, not a cluster-wide percentage**:
  an average of 74% hides the one node at 114%, which is the exact case the
  Capacity report exists to catch ([analysis.md](analysis.md)). The per-node
  numbers stay in the report, where there is room to say whether they mean
  *promised* or *used* — a bare `78%` in the header would be read as usage and
  would be a lie ([invariant 14](../CLAUDE.md)).
- **A vital that cannot be read is blank, never guessed.** While connecting, the
  count reads `nodes …`; a user who cannot list nodes gets an empty left zone.
  Stale vitals stay visible and say how old they are (`nodes 3/3 (40s ago)`),
  the same rule the body obeys ([states.md](states.md)).
- **Before any cluster is picked, there is no context to put in the right
  zone.** The startup picker
  ([context.md § Opening at startup](context.md#opening-at-startup)) is that
  state: the right zone reads `choose a cluster` plus `admin` / `read-only`
  (known from the CLI flag before any connection is made), and the left
  vitals zone is empty for the same reason it is empty while connecting —
  nothing has been read yet.
- **Namespace scope and node access are two different permissions, and the
  header must not conflate them.** Being scoped to one namespace for *pods* says
  nothing about *nodes*: the common namespace-scoped screen keeps `nodes 3/3`
  and loses the `capacity` badge, because the badge needs every pod on a node
  and the node count does not
  ([states.md](states.md#you-can-only-see-some-namespaces)). Two flags, two
  behaviours — an earlier draft of this section had one, and it drew a blank
  vital for the wrong reason.
- **A blank badge is only honest because the screen behind it speaks.** `capacity`
  with nothing beside it cannot distinguish *no overcommitted nodes* from *not
  checked*; the badge has room for a number, not for a sentence, so the report
  itself carries the reason ([analysis.md](analysis.md#capacity-when-you-can-only-see-one-namespace)).
  A fourth symbol meaning "not checked" is not the answer — it needs a legend,
  and the three severity symbols are the whole vocabulary.
- Terminal setup is `ratatui::init()` / `ratatui::restore()`. `init()` already
  installs a panic hook that restores the terminal — but
  [invariant 8](../CLAUDE.md) needs a second guarantee, that no credential
  reaches stderr, so our hook **chains** ratatui's rather than replacing it.
  Replacing it is how the terminal ends up corrupted after a panic.

## 1b. How long ago it happened — one ladder, every screen

Three places draw an age: the header's stale vitals (`nodes 3/3 (40s ago)`,
[states.md](states.md)), the Alerts card's right edge
([alerts.md](alerts.md)), and `--once`'s title-line suffix
([once.md](once.md)). They are one function — `rules::age(now, event)`, low
enough in the pyramid for both renderers to reach
([NOTES § D68](../NOTES.md#d68--the-age-ladder-is-not-the-formatters-choice-and-what-the-brief-still-left-open-2026-08-13))
— so the rungs belong here rather than three times over.

**This table is the ladder. A Rust `if`/`else if` chain is read straight off
it, top to bottom.**

| `now − event` | drawn | widest string on this rung |
|---|---|---|
| more than 5 minutes in the future | **nothing** — no age is drawn at all | — |
| up to 5 minutes in the future, or under one whole second | `just now` | 8 |
| 1 s … 59 s | `40s ago` | `59s ago` — 7 |
| 1 min … 59 min | `4 min ago` | `59 min ago` — 10 |
| 1 h … 47 h | `1 hour ago` · `2 hours ago` | `47 hours ago` — 12 |
| 48 h and up | `2 days ago` · `6 days ago` | `20678 days ago` — 14 |

- **Every rung truncates**, and each rung names one unit. `min` is abbreviated
  and never pluralised, because that is how the screens spell it; `hour` and
  `day` are words and take their singular at one.
- **The hours rung runs to 48, not to 24**
  ([NOTES § D83](../NOTES.md#d83--the-hours-rung-runs-to-48-and-the-age-ladder-gets-one-home-2026-08-14)).
  `1 day ago` used to cover 24h01m through 47h59m — a whole day of
  resolution thrown away in the one band where the reader's question is *"was
  this before or after yesterday's change window?"*. `kubectl`'s own
  `HumanDuration` prints `30h`, `47h`, then `2d3h`, so k8rs was coarser than
  the command it exists to teach, in the band that matters most. Past 48 h the
  question stops being *which* window, and one unit is enough — the days rung
  stays coarse on purpose, and nothing here should be read as inviting
  `2 days 3 hours ago` later.
- **`1 day ago` is therefore not a reachable string.** Neither is `0s ago` —
  the sub-second window says `just now`. Both absences are deliberate; a screen
  that draws either is drawing something this ladder cannot produce.
- **The future bound is a wrong-field guard, not a clock feature**
  ([NOTES § D69](../NOTES.md#d69--the-operator-review-that-reopened-the-box-and-the-prune-line-that-was-never-true-2026-08-13)).
  A moment more than five minutes ahead is a rule reading a deadline instead of
  an event, and *"no number we cannot produce"* answers it with a blank rather
  than with a smaller number.
- **`now` is the caller's moment**, not one global clock: the snapshot's for a
  finding drawn in that pass, a freshly read one for the header's staleness
  age, which has to keep advancing while the snapshot does not.

**The widest string is 14 columns**, and that number is what
[alerts.md](alerts.md#how-wide-a-card-is-and-how-tall) budgets the age column
against. It comes from the days rung's digit count, not from a real cluster:
`20678 days ago` is what the epoch draws, which is the case `Option<Time>`
exists to prevent from ever reaching a screen. A cluster ten years old draws
`3652 days ago` — 13. Nothing is clamped at 14; a wider string simply takes one
more column from the name beside it.

## 2. Element → widget

| Screen element | Widget | State object | Notes |
|---|---|---|---|
| Header row | three `Paragraph`s in a `Layout::horizontal` | — | right zone laid out first at its full width, left next, `k8rs` drawn into what is left only if ≥2 blank columns remain each side (§1a) |
| Outer frame | `Block::bordered()` | — | no titles — the header is its own row |
| Sidebar (ALERTS / RESOURCES / ANALYSIS + children) | `List` | `ListState` | flat `Vec<NavItem>`; group headers are unselectable rows, `↑↓` skips them |
| Sidebar counts (`3 ● 7 ▲`, `1 ▲`, `30d`, `12`) | right-aligned `Span` in the same `ListItem` | — | part of the row, not a second column |
| Finding card (Alerts) | `List` of **multi-line** `ListItem` | `ListState` | one `ListItem` = one card = **three to twelve `Line`s** + a blank, wrapped and capped by [alerts.md § How wide a card is, and how tall](alerts.md#how-wide-a-card-is-and-how-tall). **No card carries a selection marker** — `theme::SELECTION` is the sidebar's, and every mockup on this screen draws a card the same way selected or not; `ListState` is here only for the other half of its job, keeping a tall card's action in view rather than scrolling past it (§4). `ListItem` does not wrap — `ui.rs` wraps the card's four parts into `Line`s itself, at the pane's current width, every frame |
| Resource table | `Table` | `TableState` | rows and header both come from the server's `Table` response; widths `Constraint::Min(len(header))` per column, so nothing is hard-coded per kind ([invariant 12](../CLAUDE.md)) |
| Finding marker in a table row (`●`) | `Span` prepended to the first `Cell` | — | how Alerts bleeds through into the browser |
| Detail tabs (logs · describe · yaml · events) | `Tabs` | `usize` index in the view state | `[` `]` move it |
| Logs / describe / yaml pane | `Paragraph` + `Wrap { trim: false }` | `u16` scroll offset | yaml and logs do **not** wrap-trim: leading whitespace is meaningful |
| Any pane taller than its viewport | `Scrollbar` (`ScrollbarOrientation::VerticalRight`) | `ScrollbarState` | rendered **only** when content exceeds the viewport — a permanent scrollbar in a 3-line pane is noise |
| Command log strip | `Paragraph` inside a `Block` | `VecDeque<Line>`, capped | last 2 lines visible, no wrap — these are copy-paste text and a wrapped command is a lie |
| Footer | `Paragraph` of `Span`s | — | rebuilt per frame from the current mode; there is no stored footer |
| Dialogs, help, container picker | `Clear` → `Block::bordered()` → content | `Modal` enum in the view state | §5 |
| Typed-name input (delete / drain) | `Paragraph` + `Frame::set_cursor_position` | `String` + byte cursor | no input widget exists in ratatui and one line does not need one |
| Empty · loading · disconnected | centered `Paragraph` | — | same frame, different content pane — never a different screen |
| Banner above a list (disconnected · namespace scope) | two to eight `Line`s above the normal list | — | one slot, two occupants: the list stays visible and the banner says what is wrong with it — stale data, or a check that could not run ([states.md](states.md)). Disconnected **while** scoped drops the scope explanation (the header still says `ns: payments`) and keeps the *"one node check is off"* line, which is the half a reader cannot infer from anywhere else |

**The sidebar's indent, stated once rather than read off which rows happen to
be selected today.** Every row — top-level heading, nav group, kind row —
reserves the same two-column gutter `List::highlight_symbol` gives it:
[`theme::SELECTION`](../src/theme.rs) plus the column that keeps it off the
label, blank when the row is not the selected one. **Indent is additive on
top of that gutter and depends only on the row's level, never on whether it or
any other row is selected**: 0 for `ALERTS` / `RESOURCES` / `ANALYSIS`, 1 for a
nav group under one of them (`workloads`, `capacity`, …), 3 for a kind row
nested under an expanded Resources group (`deployments`, …). A selected and an
unselected row at the same level land their label on the same column — compare
`▸  workloads` and `   network` in [resources.md](resources.md) — and that
equality, not any single drawing, is the rule a reviewer checks a new row
against. It cost every mockup on this page one column: an unselected top-level
heading used to draw with a single leading space, one column short of the
gutter its own selected form already spent, which a uniform rule cannot
produce without a second one carved out for exactly three rows. The fix
was the mockups, not a second `List` — `RESOURCES` and `ANALYSIS` gained the
column their gutter already reserved, everywhere they are drawn unselected.

**The badge-glyph rule, stated once for every sidebar count on every
screen**: `3 ● 7 ▲` · `1 ▲` · `30d` · `12` are not one convention, they are
two, and which one a badge follows depends on what kind of value it carries,
never on how it looks.

- **A badge that is a count draws its band as a glyph.** `● ▲ ○` never rely
  on colour alone — colour blindness, and copyability
  ([NOTES § Design](../NOTES.md#design)) — and on a count the glyph is not
  emphasis, it is the *unit*: `1` counts nothing, `1 ▲` counts one warning,
  and a reader who copies `capacity  1` out of the terminal has lost what
  the number was of. `capacity  1 ▲` is this shape.
- **A badge that is a duration draws no glyph.** The value already states
  the fact the reader acts on, in words that survive being copied into a
  monochrome terminal — `30d`, or `out` when the deadline has passed — so
  `Badge::severity` colours the text and adds nothing else to it.
  `certificates  30d` is this shape, and so is `certificates  out`: the
  expired case drops the digits entirely rather than signing them, because a
  badge carries no sentence to give a number direction — `0d` would read as
  *expires today*, and `-12d` would teach a minus sign to a reader this
  product is written for ([invariant 14](../CLAUDE.md); the case is drawn in
  full at [analysis.md § Certificates and Versions](analysis.md#certificates-and-versions)).
- **A plain count with no band draws neither** — the `12` above, a fact with
  nothing to judge.

This is the single point of change for every badge on every screen; a new
report's badge is one of these three shapes and never a fourth. It moved
here from `analysis.md`, which drew it once for `certificates  30d` and
named this file as where it belonged — see that section for the one worked
example.

**A mockup's command log line is read against its real 76-column budget, not
its own drawn width — the number and the cut behaviour live at §7.** Most
mockups in this directory draw their frame 70 columns wide for the
page's own readability
([README § How to read them](README.md#how-to-read-them)), which puts a
stricter 66 on the line they draw — never the real ceiling, only a narrower
one a mockup's own text almost always clears anyway.

**The gap before an outcome word is three columns** — `→ rejected`,
`→ not sent`, `→ refused`, `→ not allowed`, `→ login expired` all sit three
spaces after the command they answer, a beat the eye can find the same way
on every screen that draws one. [dialogs.md](dialogs.md)'s own rejected-scale
line is the one exception, at two: that row already fills all 68 columns of
its 70-column-page pane with the command and the verdict, and there is no
column left to spend on a third space. It is the exception because its pane
is drawn narrower than the real floor above, not because the rule bends —
read the other five as the pattern, not this one.

Nothing here is a custom widget. If a screen seems to need one, the screen is
wrong before the widget set is.

## 2a. The footer

The footer is a `Paragraph` rebuilt every frame from the current mode (§2's
own element table) — there is no stored footer, so nothing here is a runtime
truncation of a string this file already drew wrong. It goes through the same
`ui::indented` pass as the command log strip, so it carries the identical
**76-column** ceiling at the 80×24 floor, never its own number — pane width,
less the outer border, less the one-column margin `indented()` reserves on
each side (§1, §7). A mockup drawn at this file's usual 70-column page width
shows the footer at that page's own stricter budget, the same convention
[the command log line already follows](#2-element--widget).

Most footers of fixed text stay inside the 70-column page's own 66-column
budget — the curation rule below is what keeps it that way. (The one footer
that is *not* fixed text,
[while a call is running](dialogs.md#while-the-call-is-running), is a
different exception, a few paragraphs down, and it reaches the full 76 by
design, not by accident.) **One fixed footer already does not fit that
budget**: the both-refused row in the table below is 71 real columns, past
the 66 a 70-column page allows. A fixed footer that needs the extra room is
drawn at the real 80-column width instead — ten columns more than a
70-column page, not six — the same move
[help.md § When a key is refused](help.md#when-a-key-is-refused) already
makes for its own over-70 rows, and the same move [alerts.md](alerts.md) and
[detail.md](detail.md) make for other lines too wide for 70; never a fifth
truncation invented for the footer alone.

**`? all keys` and `q quit` are one closed pair, drawn last, and they are what
never gives way — with exactly one named exception, below: while a call is
running, every footer it reaches loses `q quit` at least, and two of them
lose the whole line, pair included.** [help.md](help.md)'s own rule — `q`
sits in the footer with the other keys valid right now, the same place every
other screen puts it — is read here as the rule for *this* pair specifically,
not as a mandate that every bound key appear on every footer:
[README rule 2](README.md#the-five-rules-every-screen-obeys) already draws
that line — the footer shows what is valid right now, `?` shows everything —
so a footer has always been a curated subset, never an exhaustive one. What
is new is naming the one subset that is never cut from it, and the one state
where it is.

**The rest of a footer is curated to fit, and a key that does not fit stays
bound — it just is not the one this line spends room naming.** [D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted)
fixes the key set; this is only which of them a given footer's one line
shows, the same way `d describe` and `y view as YAML` were never shown on a
list-view footer even though both work from one. Two places apply that same
choice to a key that used to be shown and no longer is, once the anchor pair
was added back in:

- **Alerts and Resources share one footer** — `↑↓ move  ⏎ open  s scale
  r restart  / filter  ? all keys  q quit` — because both are "a list with a
  selected object" in the same sense. `l logs` and `ctrl-d delete` give way:
  the first is one `⏎` and one `[`/`]` from either list already, and the
  second is no more central to either than `describe`/`yaml` already were —
  both stay reachable through `?`, neither stops working.
- **The logs tab keeps `f follow` and `c container`, and gives up `⇧p
  previous` and `/ search`** — the two kept are what a reader reaches for on
  nearly every open pane; `⇧p` only matters once a container has crashed, and
  the search key is the same `/` every other pane already carries without a
  footer hint.

**A key this login may not use is marked where it is drawn, never omitted —
omission is a different fact and belongs to a different box.**
[D23](../NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards)
exists because a user who cannot `scale` still had to type a pod's name in
full before finding out; [D229](../NOTES.md#d229--the-four-rulings-mayi-could-not-be-briefed-without-and-the-boxs-arithmetic-that-went-stale-under-it-2026-09-05)
built `may_i_in` to answer that before the keystroke, and this is where the
answer reaches the screen. Two facts look alike from a glance and are not:
**withheld** is a key this pane has nothing to act on — no object is
selected, so the key is not drawn at all, the rule the eight states already
follow ([states.md](states.md)); **refused** is a key this pane could act on
if the login were allowed to — the key stays on the line, and what changes is
the word after it. A reader tells the two apart by what is on the line, not
by a colour: one line is shorter than the ordinary footer, the other is
longer by one word.

- **The word is `no`, inserted between the key and its label — `s no scale`,
  `r no restart` — never a symbol.** [`theme.rs`](../src/theme.rs) gives two
  non-colour carriers, `Signal::Mark` and `Signal::Reverse`; `Reverse` already
  means *press this* twice over (`FOCUS`, a modal's live confirm button), and
  reusing it here would tell the reader to press the one key they may not. A
  new `Mark` glyph was considered and does not earn its keep — not for a
  fit-width reason: the *reason* a key is refused lives behind `?` either
  way, `no` does not carry it either (below), so which carrier fits this line
  was never what decided between them. What does not earn its keep is the
  vocabulary cost: `● ▲ ○` and `⚠` are one small set, taught once in
  [rule 4](README.md#the-five-rules-every-screen-obeys) and read the same way
  on every screen since; a fifth glyph invented for this one job — three
  keys, one shared footer — would need its own legend before a reader who
  had not met it before knew what it meant, in colour or out of it. `no` is
  not new
  vocabulary and needs none: it is the same word this product already
  reaches for when there is nothing to soften
  ([states.md](states.md#you-can-only-see-some-namespaces) — *"your user
  can't list them"* is the same fact, spelled in a full sentence where there
  is room for one) — and it is a literal, so it survives a copy-paste the way
  every `Signal::Mark` here already has to.
- **The reason is `?`'s, not this line's — counted, not assumed.** The
  ceiling is 76 columns at the 80×24 floor (above); today's Alerts/Resources
  footer is 65, leaving 11. `no` costs 3 columns a key (`s scale` → `s no
  scale`, `r restart` → `r no restart`); both refused at once costs 6,
  landing at 71 — inside the ceiling with 5 columns to spare. The shortest
  honest reason this box could write for one key —
  `get+patch deployments/scale` — is 27 columns on its own, before the
  punctuation that would introduce it; there is no version of *both refused,
  plus why, for each* that fits 11 columns, so the reason does not try to
  live here. It is one `?` away, drawn
  in [help.md § When a key is refused](help.md#when-a-key-is-refused) — the
  same trade the browser's row and the evidence line already make: the
  crowded surface marks, the roomy one explains
  ([§ 7](#7-text-that-came-from-the-api)).

  | State | Alerts / Resources footer | Columns |
  |---|---|---|
  | neither refused (today's footer, unchanged) | `↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit` | 65 |
  | `s` refused | `↑↓ move  ⏎ open  s no scale  r restart  / filter  ? all keys  q quit` | 68 |
  | `r` refused | `↑↓ move  ⏎ open  s scale  r no restart  / filter  ? all keys  q quit` | 68 |
  | both refused | `↑↓ move  ⏎ open  s no scale  r no restart  / filter  ? all keys  q quit` | 71 |

- **`Verdict::Yes`, `Verdict::CouldNotTell` and *not asked yet* draw the
  ordinary key, unmarked — the *neither refused* row above, exactly.**
  ([D229 ruling 4](../NOTES.md#d229--the-four-rulings-mayi-could-not-be-briefed-without-and-the-boxs-arithmetic-that-went-stale-under-it-2026-09-05):
  a probe that could not be answered may never be the reason a permitted key
  is hidden or dimmed, and the first frame of any run — before `may_i_in` has
  replied at all — is the same case on screen as a probe the cluster refused
  to answer.) Only `Verdict::No` changes the line.
- **`ctrl-d delete` never reaches this line, refused or not.**
  [D259](../NOTES.md#d259--the-footer-is-a-curated-subset-with-one-pair-that-never-gives-way-the-help-screen-is-the-frame-wearing-a-title-rather-than-a-box-drawn-inside-it-and-a-gate-verified-against-a-substituted-tree-is-not-verified-2026-09-10)
  already took it off both list footers for width, before this box existed —
  it is marked only where it is drawn, in
  [help.md § When a key is refused](help.md#when-a-key-is-refused).

**A modal's footer is a closed, complete list, and it never carries the
anchor pair.** Every `Modal` variant but `Help` (§5) draws only the keys valid
inside it — `⏎ do it  esc cancel`, `type the name to enable  esc cancel`,
`esc dismiss  ⏎ open` for `Refused`, the bare `esc dismiss` for `Gone` (it has
nothing to reopen — the object is already gone), `esc stop draining`,
`↑↓ move  / filter  ⏎ switch  esc cancel` — because a modal this small has
nothing left for `?` to reveal, and stacking
`Help` over it is what the single-value enum already makes unrepresentable
(§5): opening one would silently drop whatever the modal underneath was
confirming. `q` is absent the same way — `esc` is always the way out of a
modal, and a global quit sitting beside it on a pending mutation is a second,
riskier way to leave that buys nothing `esc` does not. **`Help` is the one
modal exempt from both halves of this rule**, because nothing is pending
while it is open: it keeps `q quit` and replaces `? all keys` with its own
`? or esc to close` — the map itself, not a pointer to one — drawn once, in
[help.md](help.md), not repeated here.

**That reasoning covers every modal Help could stack over — none is reachable
while it is open (§5) — but not the one screen that is not a modal at all:**
[while a call is running](dialogs.md#while-the-call-is-running) is not a
`Modal`, so `?` still opens Help over it, and there something *is* pending.
Help's footer drops `q quit` in that one state — `? or esc to close` alone —
rather than promise a key the running call has already refused; the omission
matches the one the in-flight footer itself already makes for the same key.
It is not the `no`-marked refusal above, which is a permission this login
lacks — a call finishing is a wait, not a permission, and the two stay two
different facts. Drawn in
[help.md § While the call is running](help.md#while-the-call-is-running).

**One state is neither an ordinary footer nor a modal, and it is the one
place a footer's own text can run long: [while a call is running](dialogs.md#while-the-call-is-running) —
and only on Alerts and Resources.** Those two are the only ordinary footers
that name `s` and `r`, and marking either `no` right now would say the wrong
thing (dialogs.md's own reasoning: `no` means a permission this login lacks,
and a call finishing is a wait). So on Alerts and Resources alone, the whole
line is replaced: `?` still opens Help, but reads `? keys` rather than the
ordinary `? all keys`, to leave room for the reason clause, and `q` is
refused rather than merely unshown. The object name inside that clause is the
one string in any footer this file did not choose the length of, and it is
cut on the same rule as every other truncation in this product
([§7](#7-text-that-came-from-the-api)), not a fifth convention.

**Every other mode keeps its own footer while a call is in flight, less one
word.** Analysis and every detail tab never name `s` or `r` on their own
footer, so none of them has anything on the line a call in flight makes
false — except `q quit`, which every one of them carries and which drops
silently, the same way it already drops from Help's own footer in this same
state: not marked `q no quit`, because a call finishing is a wait, not a
permission. `[ ] tabs`, `f follow`, `c container`, `esc back`, `⏎ open` and
`↑↓ move` all stay bound and stay named — they are viewing and moving, not
mutating (`screens/dialogs.md § Detail tabs and Analysis keep their own
footer`).

**The closed mode list** — every mode's own exact string is drawn once, in
the file that owns it, cited here rather than copied:

| Mode | Footer shape | Drawn in |
|---|---|---|
| Alerts | ordinary, anchor always present — the pair's own named exception applies (above): replaced outright | [alerts.md](alerts.md), [dialogs.md § While the call is running](dialogs.md#while-the-call-is-running) |
| Resources | ordinary, identical to Alerts', the same exception and all | [resources.md](resources.md), [dialogs.md § While the call is running](dialogs.md#while-the-call-is-running) |
| Analysis, any of the seven reports | ordinary, fixed regardless of report — adds nothing to the key map — the pair's own named exception applies (above): `q quit` alone | [analysis.md § How a report is drawn](analysis.md#how-a-report-is-drawn--the-grammar-every-pane-on-this-page-obeys) |
| Detail — logs tab, every state of it | ordinary, anchor always present — the pair's own named exception applies (above): `q quit` alone | [detail.md § The logs tab](detail.md#the-logs-tab) |
| Detail — describe / yaml / events tab | ordinary, anchor always present — the pair's own named exception applies (above): `q quit` alone | [detail.md](detail.md), each tab's own mockup |
| Detail — yaml tab, a Secret with keys | ordinary, anchor always present, adds `v reveal` — the pair's own named exception applies (above): `q quit` alone | [detail.md § A Secret, values hidden behind an explicit reveal](detail.md#a-secret-values-hidden-behind-an-explicit-reveal) |
| Empty kind in the browser | ordinary, narrowed to what there is an object to act on — anchor still present | [states.md § An empty kind in the browser](states.md#an-empty-kind-in-the-browser) |
| Still loading | ordinary, narrowed to the anchor alone — nothing exists yet to move a cursor across | [states.md § Still loading](states.md#still-loading) |
| Disconnected · login expired · clock skew · namespace-scoped · nothing-is-broken | ordinary, mutations withheld, anchor always present | [states.md](states.md), each state's own mockup |
| While a call is running, over Alerts or Resources | not a modal — see the rule above | [dialogs.md § While the call is running](dialogs.md#while-the-call-is-running) |
| Confirm · Restart · Delete (typed name) · The cluster said no · Already gone · Drain | modal — closed local set, no anchor | [dialogs.md](dialogs.md) |
| The cluster picker, at `X` and at startup | modal — closed local set, no anchor; `esc` itself reads `quit` at startup | [context.md](context.md) |
| The container picker | modal — closed local set, no anchor | [detail.md § Choosing a container, and when there is nothing to choose](detail.md#choosing-a-container-and-when-there-is-nothing-to-choose) |
| The secret-reveal modal | modal — closed local set (`esc close`) | [detail.md § A Secret, values hidden behind an explicit reveal](detail.md#a-secret-values-hidden-behind-an-explicit-reveal) |
| The help modal (`?`) | its own fixed footer — the one modal keeping `q quit`, never `? all keys` — except while a call is running underneath it, when `q quit` itself drops and two of its sixteen body rows carry a `paused` clause instead | [help.md](help.md) |

## 3. Where the state lives

**No `ListState`, `TableState` or `ScrollbarState` is stored anywhere.** What
`views.rs` holds is a `Cursor` (a plain `usize` index plus the anchor key it
last pointed at, so a selection survives the list under it changing shape) for
the sidebar and for whatever the content pane is showing, a `u16` scroll offset
for the free-text panes — logs, yaml, describe — which have no selection to
follow, and a tab index: [NOTES § File layout](../NOTES.md#file-layout)'s
"per-view state: selection, filters, tabs, scroll", named in the types that
actually carry it. `ui::draw` takes **`&App`, immutably** — it builds every `ListState`
and `TableState` fresh from a `Cursor` on the spot, hands it to
`render_stateful_widget`, and drops it at the end of the frame. Nothing ratatui
resolves is carried between frames; the `Cursor` is what is, and it is
re-derived every time.

`ui.rs` computes nothing that outlives the frame, stores nothing, and decides
nothing a second frame could see — this is what makes a view's behaviour
testable without a terminal. It does not keep the file short: `ui.rs` has
already passed the ~800-line mark at which
[NOTES § D11](../NOTES.md#d11--the-ninth-file-pre-approved) pre-approves
`dialog.rs` as the ninth file, and that permission is now on the table for
Phase 11's dialog boxes to spend or not
([NOTES § D249](../NOTES.md#d249--the-layout-box-lands-from-a-second-session-the-header-gives-way-from-its-front-and-a-refusal-keeps-the-list-it-is-about-2026-09-06)).

## 4. Scrolling

- Lists and tables scroll through `ListState` / `TableState`. We do not
  compute offsets by hand; ratatui already keeps the selection in view.
- Free text (logs, yaml, describe) keeps its own `u16` offset because
  `Paragraph` has no selection to follow.
- **The scrollbar reports the buffer, not the history.** The log buffer is
  bounded ([invariant 9](../CLAUDE.md) — no unbounded line, no unbounded
  buffer), so the bar shows position within what is *retained*. When the
  buffer has dropped older lines, the pane says so in one dim line rather than
  letting the bar imply the whole stream is there.
- **Follow mode (`f`) pins the offset to the bottom** and any manual scroll
  turns it off — the standard `tail -f` behaviour, and the only way a stream
  and a scrollbar coexist without fighting.

## 5. The modal layer

```rust
enum Modal {
    Help,             // `?` — the key map (help.md)
    Confirm(Dialog),  // every live confirm on dialogs.md — Scale, Restart,
                      // and Delete's typed-name variant, told apart by
                      // Dialog::asks, not by a second variant
    Refused,          // "The cluster said no" (dialogs.md § The cluster
                      // said no)
    Gone,             // "Already gone" (dialogs.md § The object went away)
    ContainerPick,    // pick a container before opening logs (detail.md)
    ContextPick,      // the cluster picker, at startup and on `X` (context.md)
}
```

This replaces the enum this section used to list — `None, Confirm(..),
TypedDelete(..), Refused(..), Help, ContainerPick(..), ContextPick(..)` — which
had drifted from the code in two ways and was missing a screen this file
already draws. **There is no `None` variant**: "nothing open" is
`Option<Modal>::None` at the call site (`App::modal`), not a case inside the
enum itself — the enum only ever names something that *is* open.
**`Confirm` and `TypedDelete` are one variant, `Confirm(Dialog)`**, told apart
by `Dialog::asks: Option<String>` — `None` for a dialog a plain `⏎` confirms,
`Some(name)` for one that needs the name typed back (`src/views.rs` § THE
MODAL LAYER). Folding them costs nothing a second variant would have bought:
both are the same box with the same fields, and `armed()` already reads
`asks` to decide whether a typed match is required.
**`Refused` and `Gone` are their own variants, not `Confirm(Dialog)` wearing a
different message** — the reason is structural, not cosmetic. `Confirm`'s
button goes live once a verdict arrives (`Dialog::armed`); a refused write and
a vanished object are *terminal* states that never arm and offer only
`esc dismiss`, so a `Confirm` that could reach either state would need
`armed()` to somehow stay false forever after a `Some` verdict, which is what
`armed()` today has no way to express. Two dismiss-only screens are two small
variants, not one overloaded one. **Neither `ContainerPick` nor `ContextPick`
changed** — both are real, both are pickers over a list rather than a
confirmation, and neither is this box's to touch; they are named here only so
this list stays complete.

- **One modal at a time — the enum makes stacking unrepresentable.** No modal
  stack, no z-index. A dialog that could open over a dialog is how a
  confirmation ends up applying to the wrong object.
- Draw order is `Clear` over the centered `Rect`, then the block, then the
  content. Without `Clear` the pane underneath shows through — ratatui does
  not clear for you.
- The centered rect comes from one helper (`Layout` twice, vertical then
  horizontal), used by every modal. Not six hand-computed rectangles.
  **`Help` is the one exception, and it is a sizing exception, not a second
  mechanism**: its `Rect` is the whole body region — the same 16 rows and
  full width the sidebar and content pane would otherwise split
  ([§1](#1-the-frame)), not a smaller box floating over a visible sidebar. Only
  two of the three calls apply to that `Rect`: `Clear`, then the content —
  a **borderless** `Paragraph`, never a second `Block::bordered()`, so all 16
  rows are key map and none are spent on a border ratatui would otherwise
  draw at the `Rect`'s own top and bottom row. The border that carries the
  title `Keys` is the frame's own outer `Block::bordered()` — the one every
  screen already has around body+log+footer, titleless everywhere else
  ([§2](#2-element--widget)) — not a block drawn over the body `Rect` itself.
  This is why every other modal's *outer* frame is plain, with the title on
  its own smaller nested box, while Help's title sits on that outer frame
  directly — Help has no nested box, because it has no sidebar or content
  pane left showing to float over. The header, command log strip and footer
  rows are unaffected either way: they are
  siblings of the body in [§1](#1-the-frame)'s layout, not inside the `Rect`
  a modal draws over, which is why Help's own log strip keeps showing real
  commands and its footer is content this file already names
  ([§2a](#2a-the-footer)).
- **`Confirm`, `Refused` and `Gone` pick their box from three standard
  widths, tried in this order, never a bespoke fit per mockup** — read off
  every dialog box in [dialogs.md](dialogs.md), not asserted from a layout
  formula. **The width is the only choice a dialog makes.** The margin is
  never a second, separate choice — it is whatever centring that width
  inside the 68-column body, by the one helper above, leaves on each side:

  | Interior width | Used when | Margin, centred |
  |---|---|---|
  | 58 | the default for a `Confirm` box with a `$ kubectl …` line — used whenever the content fits ([dialogs.md § Scale](dialogs.md#scale--confirm-with-dry-run)) | 4 / 4 |
  | 61 | 58 does not fit — a longer consequence sentence, or the typed-name field ([§ Restart](dialogs.md#restart--confirm-with-dry-run), [§ Delete](dialogs.md#delete--the-name-has-to-be-typed-and-nothing-is-checked-first)) | 3 / 2 — 5 columns split as evenly as an odd number allows |
  | 54 | `Refused` or `Gone` — no `$ kubectl …` line and no typed-name field inside the box, so there is consistently less to fit ([§ The cluster said no](dialogs.md#the-cluster-said-no), [§ The object went away](dialogs.md#the-object-went-away-while-the-dialog-was-open), [§ Drain](dialogs.md#drain-which-takes-minutes)) | 6 / 6 |

  **58 fits with room to spare either side. 61 is as wide as any dialog on
  this page needs, and it is close to the real ceiling** — the smaller
  margin cannot drop below 2 without the box touching the outer frame, and
  61 leaves 2 already, one column short of it. **This file drew the three
  dismiss-only boxes off-centre (4 / 8) until a review measured every box
  against this same rule and found those three did not obey it** — fixed by
  centring them, not by writing a second margin rule to excuse the
  difference. Nothing here chooses a margin directly, on any of the three
  widths.
- **Height follows the same box, not a fourth number to memorise.** Every
  dialog's full frame is 11 fixed rows — the header line, the outer top border,
  one blank row, the nested box's own top and bottom border, one more blank
  row, the two log-strip separators and its one line, the footer, and the outer
  bottom border — plus however many rows sit between the nested box's own
  borders. **24 is the ceiling**, because the whole frame is drawn inside the
  80×24 floor this product supports at all ([README § How to read
  them](README.md#how-to-read-them)), so content between the nested borders
  never runs past 13 rows. **One blank row is kept between the consequence and
  the dry-run verdict whenever there is room for it inside that ceiling, and
  dropped — first, before any sentence is cut — whenever there is not.** §
  Scale keeps it (9 content rows). § Restart and § Delete both need that row
  for something else instead — an extra sentence, the typed-name field — so the
  blank goes rather than a clause
  [D223](../NOTES.md#d223--the-four-rulings-restart-could-not-be-briefed-without-and-the-pod-arm-that-is-deletes-2026-09-04)/[D224](../NOTES.md#d224--the-restart-review-round-two-blockers-a-stand-in-apiserver-could-not-produce-and-the-sentence-that-promised-a-clusters-settings-2026-09-04)/[D225](../NOTES.md#d225--the-five-rulings-delete-could-not-be-briefed-without-and-the-preflight-it-declines-2026-09-04)
  put there on purpose; § Restart's paused variant and § Drain both land on
  exactly 13, the ceiling itself, with no blank left to spend.
- `esc` closes exactly one level, always. A modal never traps the user.
- The confirm button is a `Span` with a reversed style; it is **not** live
  until the dry-run has returned and, for a typed-name dialog, until the typed
  string equals the object name ([dialogs.md](dialogs.md)).
- Under `--read-only` the mutating variants are not constructed anywhere —
  unreachable, not merely unbound ([invariant 2](../CLAUDE.md)).

## 6. When a frame is drawn

There is no frame rate ([invariant 7](../CLAUDE.md)). `terminal.draw()` is
called when, and only when, one of these happened:

| Trigger | Source |
|---|---|
| A key | crossterm event stream |
| A resize | crossterm event stream |
| A watch event changed something on screen | `k8s.rs` channel |
| A modal opened, closed, or got its dry-run verdict | `ops.rs` reply |

Events are coalesced over ~100 ms during a storm — a rollout that restarts 200
pods produces one redraw, not 200. When nothing arrives, the loop blocks on the
channel: **0% CPU idle**, which is the measurable difference from a tool that
polls. No animation, no spinner, no throbber: each of them needs a timer tick,
and a timer tick is a frame rate by another name. "Still loading" is a static
line of text ([states.md](states.md)).

Mouse capture is **off**. It would cost the user their terminal's own text
selection, and the command log exists to be copied.

## 7. Text that came from the API

Every string that originated in the cluster — names, messages, annotations, log
lines, `Table` cells — passes through one `sanitize()` before it becomes a
`Span`. One function, called at the boundary, so no screen can forget it.

- Control characters are stripped ([invariant 9](../CLAUDE.md)). ratatui does
  not do this: an escape sequence in a pod name reaches the terminal and
  rewrites it.
- **We never truncate a string ourselves.** Widgets clip at the cell boundary
  and ratatui measures character width correctly, including wide CJK
  characters; `String::truncate` slices bytes and panics in the middle of a
  multi-byte name. Handing the full `Span` to the widget is both shorter and
  correct.
- **Four places truncate on purpose, and they are the exceptions that prove
  the rule above:** the Alerts card's evidence line, capped at three wrapped
  lines with `…` at the cut
  ([alerts.md § How wide a card is, and how tall](alerts.md#how-wide-a-card-is-and-how-tall));
  the Resources browser's one-line summary under the table, whose name
  gives way to the sentence around it and is marked the same way
  ([resources.md § The line under the table](resources.md#the-line-under-the-table));
  the command log strip, when even its real 76-column budget at the
  80×24 floor — pane width minus the outer border minus the one-column
  margin `indented()` reserves on each side (§1) — cannot hold the whole
  teaching line; and the footer's own in-flight reason, the one footer that
  carries a string this product did not choose — the selected object's own
  name, cut the same way the browser's row name is, on the same 76-column
  budget (§2a, [dialogs.md § While the call is running](dialogs.md#while-the-call-is-running)).
  What § 7 forbids is a *silent* cut and a *byte* cut. None of
  these four is: all are marked with a character the reader can see, and all
  step by whole characters. The evidence line and the command log both walk
  back to a whole word before they cut, because both are made of more than
  one token and a word with its last character sheared off would still look
  like a real one — `--show-managed-fiel` is not a flag a reader would notice
  was wrong. Walking back drops the whole word instead, so the mark lands
  after the word before it, never glued to a maimed one
  ([detail.md's yaml tab](detail.md#the-yaml-tab) draws the case: the whole
  flag gives way, `…` lands right after `yaml`, and deleting just the `…`
  leaves a real command — the one `kubectl get -o yaml` already runs by
  default). The browser's line and the footer's in-flight name do not walk
  back — a name is one token, so there is no word boundary to find, and
  cutting mid-token is what the mark is for there. The full text is one `⏎`
  away in all four cases ([detail.md](detail.md)) — which is what makes
  cutting any of them legitimate at all. Everything else on a card, every
  other string in the browser, and every command log line that fits is drawn
  whole and clips at the pane edge like any other string, if it clips at all.
- Long values are bounded *before* they are stored, not at draw time — a 50 MB
  annotation must never become a `Text`.

## 8. Smaller than 80×24

Previously undefined. Below the minimum, k8rs does **not** attempt the layout:
it clears the screen and draws one centered line —

```
k8rs needs a terminal at least 80×24. This one is 64×18.
```

— and returns to the normal frame as soon as the terminal grows. No collapsing
sidebar, no responsive breakpoints, no horizontal scrolling: a fixed 20-column
sidebar plus a table is not readable at 50 columns, and a squeezed layout that
technically renders is worse than a sentence that says why. Recorded as
[NOTES § D15](../NOTES.md#d15--the-widget-layer-and-what-it-rules-out).

## 9. What this file deliberately does not decide

| Not here | Where |
|---|---|
| The colour and glyph data — a role's Catppuccin value, its 16-colour degrade, a severity's symbol | `theme.rs` · [docs/tech-stack § Visual identity](../docs/tech-stack.md#visual-identity) |
| Turning that data into a ratatui `Style`/`Color` | `ui.rs` ([NOTES § D241](../NOTES.md#d241--the-two-rulings-phase-9-could-not-be-briefed-without-themers-names-no-ratatui-type-and-declaring-a-module-is-part-of-writing-it-2026-09-05)) — `theme.rs` names no ratatui type, so it cannot hold this mapping itself |
| The key map | [NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted) · [help.md](help.md) |
| What each screen says | the mockups in this directory |
| Which findings exist | [NOTES § v1 rule set](../NOTES.md#v1-rule-set) |
| The order things get built | [todo.md](../todo.md) |
