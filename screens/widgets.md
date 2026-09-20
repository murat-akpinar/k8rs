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
- **"Connection state" is `Screen::link`'s four words, and one frame does not
  use them at all.** `connecting…`, `live`, `⚠ disconnected, retrying` and `⚠
  login expired` are what the zone draws whenever there is a connection —
  arriving, live or recently lost — for one of them to describe. **A mid-session
  switch that has been refused, has expired or has otherwise failed leaves
  nothing for any of the four to describe**, from the moment its own box
  first draws through however long the reader leaves it dismissed: the slot
  instead carries the fault's own short word, the same one
  [context.md § When the new cluster does not
  work](context.md#when-the-new-cluster-does-not-work) writes into the box
  itself (`⚠ not allowed` for a `Refused`) — not a fifth connection word, and
  not a blank segment either, for as long as nothing is connected
  ([context.md § After `esc dismiss`, on a switch that failed with a cluster
  already live](context.md#after-esc-dismiss-on-a-switch-that-failed-with-a-cluster-already-live),
  `reports/2026-09-19-the-strip-and-the-connection-word.md` § M1). **This
  is a session fact, not a modal one — it does not end when the box
  closes.** The startup picker's own failure is not this case: dismissing it
  reopens the picker itself, whose header reads `choose a cluster`, not a
  context name at all ([context.md § Opening at
  startup](context.md#opening-at-startup)).
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
| Filter / namespace typing, in the footer (Alerts, Resources) | `Paragraph` + `Frame::set_cursor_position` | the `Input` `/`/`n` already own ([`views::Filters`]) | same mechanism as the typed-name input above, not a second one — [§ 2b](#2b-typing-into-a-filter) |
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

**Withheld now has a per-object cause as well as its run-level ones, and it
draws exactly the same as every cause already listed for it: nothing
selected, disconnected, `--read-only` and the rest all read as "not on the
line," and so does this.** A Node cannot be scaled by anyone and a
DaemonSet has no `/scale` subresource — this is a fact about the selected
object's *kind*, true whatever the session state and whatever this login
may do, and it is not a permission question, so `may_i_in` is never asked
and the key is never marked `no` — `s no scale` on a Node would claim a
verdict nobody was asked to give
([D261 ruling 8](../NOTES.md#d261--the-refused-keys-round-a-permission-that-is-two-questions-and-was-counted-as-one-a-reason-that-did-not-fit-the-line-it-was-promised-to-and-a-row-rewritten-by-arithmetic-another-box-would-have-moved-2026-09-12)).
**`c container` is this rule's own precedent, already shipped**: it drops
from the footer entirely on a single-container pod rather than sitting
there unusable
([detail.md § Choosing a container](detail.md#choosing-a-container-and-when-there-is-nothing-to-choose)) —
a key that cannot work is a key that is not drawn, never a key drawn dim or
marked, and `s`/`r` on a kind that does not support them follow the same
rule. A reader still tells *refused* from *unsupported* apart at a glance,
because the two are different shapes and not the same shape twice: refused
is the ordinary line plus one word (`no`) per key; unsupported is the
ordinary line minus one key. Nothing on this screen is ever both. The rows
are counted below, after the refused table they extend.

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

  **The four rows above all assume a kind that supports both operations — a
  Deployment or a StatefulSet, the running example everywhere else in this
  product.** A kind that supports only one, or neither, is a shorter footer,
  never a `no`-marked key for the one it lacks — the key that does not apply
  is not on the line at all. Measured off the same two operations
  [D261 ruling 8](../NOTES.md#d261--the-refused-keys-round-a-permission-that-is-two-questions-and-was-counted-as-one-a-reason-that-did-not-fit-the-line-it-was-promised-to-and-a-row-rewritten-by-arithmetic-another-box-would-have-moved-2026-09-12)
  already counted against the API: `scale` reaches a Deployment, a
  StatefulSet or a bare ReplicaSet, never a DaemonSet; `restart` reaches a
  Deployment, a StatefulSet or a DaemonSet, never a bare ReplicaSet; neither
  reaches a Pod, a ConfigMap or a Node.

  | State | Alerts / Resources footer | Columns |
  |---|---|---|
  | only `r` supported (a DaemonSet), not refused | `↑↓ move  ⏎ open  r restart  / filter  ? all keys  q quit` | 56 |
  | only `r` supported (a DaemonSet), refused | `↑↓ move  ⏎ open  r no restart  / filter  ? all keys  q quit` | 59 |
  | only `s` supported (a bare ReplicaSet), not refused | `↑↓ move  ⏎ open  s scale  / filter  ? all keys  q quit` | 54 |
  | only `s` supported (a bare ReplicaSet), refused | `↑↓ move  ⏎ open  s no scale  / filter  ? all keys  q quit` | 57 |
  | neither supported (a Node, a bare Pod, a ConfigMap, …) | `↑↓ move  ⏎ open  / filter  ? all keys  q quit` | 45 |

  None of these approaches the 76-column ceiling — the widest line on this
  page is still the *both refused* row above, at 71; dropping a key can only
  shorten a line that adding `no` to two keys already proved fits. Where the
  key that is missing is the one refused, there is nothing to mark: the
  unsupported key was never asked about, so it has no verdict to draw and no
  row in this table needs one.

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
inside it — `⏎ do it  esc cancel`, `waiting for the cluster` for a `Confirm`
whose check has not answered
([dialogs.md § While the check is still on the wire](dialogs.md#while-the-check-is-still-on-the-wire)),
`type the name to enable  esc cancel`,
`esc dismiss  ⏎ open` for `Refused`, the bare `esc dismiss` for `Gone` (it has
nothing to reopen — the object is already gone), `esc stop draining`,
`↑↓ move  / filter  ⏎ switch  esc cancel` — because a modal this small has
nothing left for `?` to reveal, and stacking
`Help` over it is what the single-value enum already makes unrepresentable
(§5): opening one would silently drop whatever the modal underneath was
confirming. `q` is absent the same way — `esc` is always the way out of a
modal, and a global quit sitting beside it on a pending mutation is a second,
riskier way to leave that buys nothing `esc` does not. **`waiting for the
cluster` is this rule's own extreme, not an exception to it: the closed set
there is empty**, because for that one window `esc` is not merely undrawn,
it is inert — the one modal state on this product where the promise this
paragraph just made is not yet true, and it is closed by the wire being
bounded, not by a key
([dialogs.md, same section](dialogs.md#while-the-check-is-still-on-the-wire)).
**`Help` is the one
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
| Alerts, a filter hides every row | ordinary, `↑↓ move` and `⏎ open` kept (the sidebar's own precedent, [states.md § Nothing is broken](states.md#nothing-is-broken)), `s scale`/`r restart` dropped, `esc clear filter` gained | [states.md § The filter hides every row](states.md#the-filter-hides-every-row) |
| Resources, a filter hides every row | ordinary, narrowed to `/ filter` and the anchor, `esc clear filter` gained — the empty-kind-in-the-browser shape, above, not Alerts' | [states.md § The filter hides every row](states.md#the-filter-hides-every-row) |
| Disconnected · login expired · clock skew · namespace-scoped · nothing-is-broken · the audit log could not be opened | ordinary, mutations withheld, anchor always present | [states.md](states.md), each state's own mockup |
| While a call is running, over Alerts or Resources | not a modal — see the rule above | [dialogs.md § While the call is running](dialogs.md#while-the-call-is-running) |
| Alerts / Resources, typing into `/` or `n` | not a modal, and not curated either — the whole line is replaced, the same move as the row above for a different reason | [§ 2b](#2b-typing-into-a-filter) |
| Confirm · Restart · Delete (typed name) · The cluster said no · Already gone · Drain | modal — closed local set, no anchor | [dialogs.md](dialogs.md) |
| The cluster picker, at `X` and at startup | modal — closed local set, no anchor; `esc` itself reads `quit` at startup, or `clear filter` on either variant while `/` holds text, in place of `cancel` or `quit` ([NOTES § D264 ruling 27](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)); **`⏎` itself drops from the line — not merely dimmed like a refused `s`/`r` — on a shadowed row and with no row selected**, because there `⏎` is not refused, it is inert; **a list with no row showing counts as having no landable row, the same as one with every row undefined: `↑↓ move` drops from the line too**, leaving `/ filter  esc cancel` alone for a kubeconfig with no contexts left in it at all, or `/ filter  esc clear filter` where the cause is a filter that hides every row instead, `esc`'s word still following the same rule — there is nowhere for either moving key to go ([NOTES § D264 rulings 16, 18 and 27](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13), `context.md` §§ The picker, No row is both current and landable, The filter hides every row, The kubeconfig has no contexts at all) | [context.md](context.md) |
| A picked context that did not connect (`Unconnected`) | modal — closed local set, no anchor; `esc dismiss` mid-session, `esc back to the list` at startup — the same two words `Before::leave` returns, never a third spelling for the button and the footer | [context.md § When the new cluster does not work](context.md#when-the-new-cluster-does-not-work) |
| The container picker | modal — closed local set, no anchor | [detail.md § Choosing a container, and when there is nothing to choose](detail.md#choosing-a-container-and-when-there-is-nothing-to-choose) |
| The secret-reveal modal | modal — closed local set (`esc close`) | [detail.md § A Secret, values hidden behind an explicit reveal](detail.md#a-secret-values-hidden-behind-an-explicit-reveal) |
| The help modal (`?`) | its own fixed footer — the one modal keeping `q quit`, never `? all keys` — except while a call is running underneath it, when `q quit` itself drops and two of its sixteen body rows carry a `paused` clause instead | [help.md](help.md) |

## 2b. Typing into a filter

`/` and `n` on Alerts and Resources open the same kind of typing session the
cluster picker already has — one `Input` with focus, live-narrowing the list
on every keystroke — and until now nothing said what the footer looks like
while it runs, or what `esc` and `⏎` do in it: `views::Filters` existed with
no typing state anywhere in `views.rs` or `ui.rs`, and the reader could not
tell whether `esc` was about to clear a filter or leave the screen because the
filter itself was drawn nowhere (`backlog.md`, found by `k8s-admin`,
2026-09-13; NOTES § D266 ruling 3). This section closes that gap by reusing
[context.md § The picker](context.md#the-picker)'s own words and mechanism
rather than inventing a second vocabulary — `Filters::text` and the picker's
own filter already share one `holds()` (NOTES § D264 ruling 7) — and departs
from it only where the picker's shape genuinely cannot answer: a picker has
nothing to do besides move, filter and choose a row; Alerts and Resources are
where `s`, `r`, `l`, `d` and `y` live, and typing has to leave those keys
exactly where it found them.

**Opening.** `/` opens typing on [`Filters::text`], `n` on
[`Filters::namespace`] — the same two fields this page already has, never a
third. Either opens with whatever that field already holds, cursor at the
end, so pressing `/` again on an already-filtered list edits the filter
rather than starting it over. Only one of the two can have focus at once:
pressing `n` while `/` already has it does not switch focus — `n` is a
printable character like any other, and is appended to the buffer that
already has focus, the same as every other letter (below).

**While a filter has focus, every printable key is text and nothing else is
a command.** `s`, `r`, `l`, `d`, `y`, `[`, `]`, `q`, `X`, `tab`, `?` are all
letters or symbols this filter can legitimately contain, so all of them are
appended rather than bound — `?` cannot open Help here for the same reason
`q` cannot quit: both are just characters while a filter has focus, not
commands. There is nothing to draw for *Help while typing*, because that
combination cannot happen, not because it was left out. `⌫` removes one
character
([`Input::pop`] — one `char`, never one byte); nothing else edits the
buffer, because [`Input`] has no notion of a cursor anywhere but the end —
the same shape the typed-name field on a delete already has
([dialogs.md § Delete](dialogs.md#delete--the-name-has-to-be-typed-and-nothing-is-checked-first)).
`↑` / `↓` still move the selection over whatever rows the live-narrowed list
is currently showing, exactly as they do outside typing — the picker already
proves this does not collide with a letter key, because an arrow is never
one.

**The footer is fully replaced, not curated, while a filter has focus** — the
same move the in-flight-call footer already makes, for a different reason
(above): the ordinary key set is not valid right now, so naming it would be a
promise this state cannot keep. It shows the word [help.md](help.md) already
teaches for the key being used — its own `/ n   filter · namespace` row —
the text typed so far, and the cursor:

```
filter: payments_  ⏎ done  esc clear filter
```
```
namespace like: pay_  ⏎ done  esc clear namespace
```

Empty, the second half of `esc`'s word changes, for the same reason the
picker's own does
([context.md § The picker](context.md#the-picker),
[NOTES § D264 rulings 27 and 31](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13)):

```
filter: _  ⏎ done  esc cancel
```
```
namespace like: _  ⏎ done  esc cancel
```

The cursor is [`Frame::set_cursor_position`], the same mechanism [§2](#2-element--widget)
already names for the delete dialog's typed-name field — not a drawn
character. The trailing `_` above is this page's own stand-in for it, the
same convention every other typed-input mockup on this page already uses.

**A buffer long enough to run past the label and the hints keeps the cursor
in view by losing its own front, not by losing the hints.** `⏎ done  esc
clear filter` is what tells the reader the two keys that get them out; a
buffer that grew past the ceiling and pushed them off the line would answer
*what did I type* at the cost of *how do I leave*, which is the wrong trade
on the one line that is also the only way out. So the label and the hints
stay fixed width and the typed text is what gives way — front-cut, one `…`,
the cursor always the last character shown, the same shape a shell's own
line editor already uses when a command outgrows the terminal. Realistic
filters never reach it (`IDENTIFIER`'s 512-byte bound is a beginner's
safety rail, not a length anyone types on purpose), so this is named once
here for completeness and not drawn as its own mockup.

**`esc` while typing acts on the field that has focus — the field the reader
is looking at, not a global order.** The buffer `/` or `n` opened is not
empty → `esc` empties it, the list re-widens to whatever now matches, and
typing stays open on it, footer and all; that buffer is already empty →
`esc` closes the typing session and returns to the ordinary footer, leaving
the *other* field exactly as it was. This is a genuine branch on which
field has focus, and it costs nothing extra to write, because the typing
state is what this box adds in the first place — there is no existing
global-order method to reuse here the way the at-rest case below already
has one, so a focus-aware rule is not a second rule competing
with a first one, it is the only one this state needs. The footer's own
word already follows the field with focus, exactly as drawn above: `esc
clear filter` while typing `/` with something in it, `esc clear namespace`
while typing `n` with something in it, `esc cancel` on either once its own
buffer is empty. One key is never spelled two ways in one frame, which is
why the word changes with the state instead of staying `cancel` throughout.

**`esc` outside typing is the same method, and today nothing on screen says
so.** `App::escape`'s `None`-modal arm runs with **no modal open and no
typing session either** — at rest, on an ordinary Alerts or Resources
screen, `esc` already clears a committed filter one field at a time, exactly
as above, and did before this box existed. This is [backlog.md:2538](../backlog.md)'s
own complaint in a second place: a key that is genuinely bound and does
something is drawn nowhere. **It is not added to the ordinary at-rest
footer** — measured, not assumed: `↑↓ move  ⏎ open  s scale  r restart  /
filter  ? all keys  q quit` is 65 columns, both `s`/`r` refused already
reaches 71 ([§ 2a](#2a-the-footer)), and the shortest honest label,
`esc clear filter`, costs 18 more with its gap — 89 in the worst case,
against the same 76 ceiling every other footer on this page answers to.
There is no wording short enough to fit the compound case, so `esc`'s
filter-clearing joins `l logs`, `d describe`, `y view as YAML` and
`ctrl-d delete` in the bucket [§ 2a](#2a-the-footer) already has for a key
that is bound, real, and not the one this crowded line spends room naming —
discoverable the same way those are, and named on the pane itself
([§ A committed filter is drawn at rest, too, below](#a-committed-filter-is-drawn-at-rest-too)), not repeated a
second time on a line that cannot afford it. **The one footer that *can*
afford it is [states.md § The filter hides every row](states.md#the-filter-hides-every-row)'s**:
`⏎ open`, `s scale` and `r restart` are already gone there because nothing
is selected, which is exactly the room `esc clear filter` needs — and it is
the same wording the picker's own equivalent state already uses, so the two
screens stop being two vocabularies for one fact.

**`⏎` commits and returns to browsing — it does not open the selected row.**
This is the one place typing here departs from the picker on purpose: the
picker's `⏎` can only ever mean *choose this row*, because choosing a row is
the whole of what the picker is for. Alerts and Resources are where `s` and
`r` live, and a `⏎` that jumped to Detail every time a filter was confirmed
would make *narrow the list, then act on a row in it* impossible without
opening Detail and coming back first. `⏎` here means *stop typing, keep what
was typed*; the ordinary footer's own `⏎ open` returns the moment typing
ends, and pressed again, on a row, it opens exactly as it always does.

**Leaving typing changes nothing about the list itself.** The filter was
already live while it was being typed — that is what let the reader watch
the list narrow one keystroke at a time, the same as the picker's own list —
so committing is a footer change, not a fetch or a second pass over the
rows. The cursor was already following the narrowed list throughout
([`Cursor::follow`]), so it does not move when typing ends either.

**A filter is kept over anything drawn *on top of* the list it narrows, and
cleared the moment the list itself changes underneath it — two different
facts, and the first draft of this section only had the first one.** Detail,
Help, every dialog on [dialogs.md](dialogs.md) and the container picker have
no `/ filter` or `n namespace` on their own closed footers, but none of them
touch `App::filters` to get there — Detail in particular is drawn *over*
whatever view was already open, never a fourth [`View`], precisely so that
going back needs no state re-derived to return to. So a filter set before
`⏎` opens Detail, before `?` opens Help, or before a dialog or the container
picker opens on the selected row is simply not drawn for as long as that
screen is — and the list is exactly as narrow as it was the moment any of
them closes.

**Analysis is not one of these, and neither is a different kind within
Resources — both are the list *changing*, not something drawn over it, and
both clear both filters.** `web` typed for Alerts' cards means nothing
against a ConfigMap table; `pay` typed for one kind's rows is not a claim
about the next kind the sidebar opens. `App::open` — the one method every
sidebar selection already goes through, `NavItem::Kind` and `NavItem::Report`
included — is where this belongs: it already resets `App::content`'s cursor
when `self.view` actually changes, for the same reason (`src/views.rs`), and
it did not yet reset `App::filters` alongside it — filed as its own defect,
*`App::open` keeps the filter across views*
(`reports/2026-09-13-phase-11-close-family-read.md` line 96). One `View`
change, from anywhere to anywhere, clears both fields; re-opening the
*same* kind or the *same* top-level view is not a change (`self.view !=
before` is already the test) and touches neither.

`/ web` on Alerts, `⏎` to open the one card it leaves, `esc` back from
Detail — the filter is still there because Detail is an overlay, not a
different list:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  filter: "web"   esc clears it                │
│  RESOURCES         │                                               │
│   workloads        │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   network          │    Containers exceeded their memory limit and │
│   storage          │    were killed by the kernel (OOMKilled)      │
│   config           │    limit 256Mi · exit 137 · 47 restarts       │
│   cluster          │    → raise limits.memory, or find the leak    │
│  ANALYSIS          │                                               │
│   capacity      1 ▲│                                               │
│   certificates  30d│                                               │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get statefulsets -A --watch                              │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit  │
└────────────────────────────────────────────────────────────────────┘
```

is what is on screen right before `⏎`, and it is what is on screen again the
instant `esc` returns from Detail — `shop/api` and `node-3` from
[alerts.md](alerts.md)'s own three-card mockup stay gone, because `web` still
does not hold either of them, and the `filter: "web"` line above the one
remaining card is [§ A committed filter is drawn at rest, too](#a-committed-filter-is-drawn-at-rest-too),
below — without it this was exactly backlog.md:2538's own complaint one
level up: a reader back from Detail sees one card and an ordinary footer,
with nothing on screen saying the list is narrowed at all. Opening
`workloads → statefulsets` from here, or pressing `⏎` on `ANALYSIS →
capacity`, drops `filter: "web"` outright — a different list, not an
overlay.

### A committed filter is drawn at rest, too

Typing draws the filter in the footer, [above](#2b-typing-into-a-filter); the moment
typing ends, that line reverts to the ordinary footer and, until now,
nothing else on either screen said the list was narrowed at all —
[backlog.md:2538](../backlog.md)'s exact complaint, and it is real whether
or not the footer could also afford `esc clear filter`
([§ 2b](#2b-typing-into-a-filter)):
a reader cannot tell a short list from a filtered one, or `esc`'s next
target, from anything on screen once the cursor leaves the footer.

**Resources already draws a title row with room in it — `deployments  ns:
payments` — and a filter joins it as a second row underneath, not a third
segment crammed onto the first.** Measured before deciding: the widest
realistic case — a long kind plural, a scoped namespace, and both filters
set — is `validatingadmissionpolicybindings   ns: openshift-cluster-node-tuning-operator   filter: "pay"   namespace like: "prod"`,
which does not fit the 57-column content pane at the 80×24 floor even
before either filter joins it. One line trying to hold four independent
facts is the wrong shape regardless of the filter question, so the filter
gets the same slot [§ 2](#2-element--widget) already names for a pane-level
fact that is not a per-row property — *"Banner above a list… one slot, two
occupants"* — as a third occupant, drawn calm (dim, no glyph: this is
neither a severity nor a connection problem, [README rule
4](README.md#the-five-rules-every-screen-obeys)), directly under the title
row it is already grouped with:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│  ALERTS     3 ● 7 ▲│  deployments          ns: payments            │
│  RESOURCES         │  filter: "web"   esc clears it                │
│▸  workloads        │                                               │
│     deployments  12│    NAME      READY  UP-TO-DATE  AVAILABLE  AGE│
│     statefulsets  3│▸ ● web       3/5    5           3          12d│
│     daemonsets    5│                                               │
│     pods         84│                                               │
│     jobs          7│                                               │
│   network          │                                               │
│   storage          │                                               │
│   config           │                                               │
│   cluster          │                                               │
│  ANALYSIS          │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get deployments -n payments                              │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit  │
└────────────────────────────────────────────────────────────────────┘
```

**`filter: "web"` is not an arbitrary example — it is the one value that
actually survives against the row drawn beside it.** `views::holds` matches
only what a row draws, and this row draws `web` in its `NAME` cell and
nothing that holds `pay`; a scoped `ns: payments` in the *title* is not a
cell `deployments`' own row redraws, `Filters::text`'s own contract
(`src/views.rs` § THE TWO FILTERS). A filter value that could
not have produced the row shown beside it would be exactly the kind of claim
this page forbids everywhere else it draws a command or a count.

**The namespace half of this line is never called `namespace:` on its
own** — measured beside the title row's own `ns: payments`, a bare
`namespace:` reads as a second scope changing, not a substring filter over
rows already held, and an operator has no reason to expect the difference
just from two labels stacked one row apart
(`reports/2026-09-18-filter-and-container-picker.md` § 4). The zero-match
sentence already had the fix — *"a namespace like `pay`"* — so the label
takes the same word: `namespace like:`, never `namespace:`, here, in the
typing footer above, and nowhere else on this page a third spelling could
grow back from.

**`esc clears it` is this line's own way out, named where the fact already
lives, so no `?`/help row has to carry it and the footer's own budget
argument above is untouched — the crowded line still cannot afford it, this
uncrowded one already could.** It shows only while one field is set — there
is only one thing `esc` could mean then, so `esc clears it` needs to name
nothing further.

**Measured, not assumed, and the first draft of this row got it backwards:
once both fields are set, the hint is never shown at all — not reworded,
dropped — and it is the hint that gives way, never the values.** `filter:
""` and `namespace like: ""` alone already cost 10 and 18 columns before
either holds a character; add the 3-column gap between them and a hint
naming which one `esc` reaches first (`esc clears filter`, 18 columns) with
its own gap, and two real values are fighting over **two columns**,
combined, on the 53-column row this pane actually has
(`reports/2026-09-18-filter-and-container-picker.md` § 4) — `/ kube` beside
`n kube-sys`, eight typed characters between them, already clears the
ceiling by one column with the hint still on the line, and no value that
means anything fits in two. **The hint is what a reader can find again by
pressing `?` or reopening `/`/`n`; what they typed is not written down
anywhere else** — so it is the hint that gives way first, unconditionally
once both fields hold something, not the values, the opposite of what the
first draft of this row did. Both values are shown in full instead — `kube`
and `kube-sys` fit inside the 22 columns two full values now split between
them without any hint on the line at all:

```
filter: "kube"   namespace like: "kube-sys"
```

Only a value long enough to still overrun that room front-cuts, the same
way the typing footer's own growing buffer already does
([§ 2b](#2b-typing-into-a-filter)), because it is the same value drawn
twice, once live and once at rest, not two strings with two rules; the
eleven closed cuts of [§ 7](#7-text-that-came-from-the-api) do not govern
it either way, because a typed filter is never API text.

**Alerts has no title row to join, so the same line opens one of its own,
above the first card** — the exact line already drawn a section above, in
`/ web`'s own mockup: `filter: "web"   esc clears it`, alone when only `/`
is set; `filter: "web"   namespace like: "pay"` — the hint already gone —
when both are. It costs one row out of the card list's own budget while it
is drawn, the same way a disconnected banner already does, and nothing
while no filter is set — silent below the threshold, exact at it, the rule
this page already states for the dropped-lines line
([detail.md § When the buffer fills](detail.md#when-the-buffer-fills-the-dropped-lines-line)).

**The zero-match state does not repeat this line** — [states.md § The filter
hides every row](states.md#the-filter-hides-every-row)'s own sentence
already names what was typed, in a full sentence, in the same slot a card or
a table row would otherwise occupy, and that state's own footer already
carries `esc clear filter` on the crowded line, because nothing else is
competing for it there. A `filter: "web"` banner above a sentence that
already says `"web"` and a footer that already says `esc` would be the same
two facts said three times. This line is for the case that sentence cannot
cover: rows are still showing, and nothing else said why there are fewer of
them.

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

- Lists and tables scroll through `ListState` / `TableState`, which is what
  keeps an offset from going stale: nothing here stores one between frames,
  so a selection that moves can never leave it pointing at the wrong window.
  **The cluster picker draws its rows as `Paragraph` lines, not a `List`, and
  still obeys this** — it derives its own offset fresh from the selection
  every frame rather than keeping one anywhere, the same value a fresh
  `ListState` of that height would give, measured at 1 through 300 rows
  ([NOTES § D264 ruling 26](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13),
  [`context.md` § More contexts than fit](context.md#more-contexts-than-fit)).
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
    Unconnected,      // a picked context that did not connect — carries
                      // `Before`, which says what `esc` goes back to
                      // (context.md § When the new cluster does not work)
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
**`Unconnected` is new, and it is the same terminal shape as `Refused` and
`Gone`, not a third one** — it never arms and offers only `esc`, with the
one difference that its `esc` reads two different words depending on what it
carries: `Before::Connected` dismisses back to a running app, `Before::Picking`
reopens the picker it came from
([NOTES § D264](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13),
`context.md` § When the new cluster does not work).

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
  | 58 | the default for a `Confirm` box with a `$ kubectl …` line — used whenever the content fits: the consequence wraps to `CONSEQUENCE_LINES` or fewer at this room **and** the `$` line itself needs no cut at all — kind/name, every flag, `-n`'s value, whole — at this room ([dialogs.md § Scale](dialogs.md#scale--confirm-with-dry-run), [§ The namespace flag never disappears without a trace](dialogs.md#the-namespace-flag-never-disappears-without-a-trace)) | 4 / 4 |
  | 61 | 58 does not fit — a longer consequence sentence, the typed-name field, or a `$ kubectl …` line that would need any cut at all at 58's own room ([§ Restart](dialogs.md#restart--confirm-with-dry-run), [§ Delete](dialogs.md#delete--the-name-has-to-be-typed-and-nothing-is-checked-first), [§ The namespace flag never disappears without a trace](dialogs.md#the-namespace-flag-never-disappears-without-a-trace)) | 3 / 2 — 5 columns split as evenly as an odd number allows |
  | 54 | `Refused`, `Gone` or `Unconnected` — no `$ kubectl …` line and no typed-name field inside the box, so there is consistently less to fit ([§ The cluster said no](dialogs.md#the-cluster-said-no), [§ The object went away](dialogs.md#the-object-went-away-while-the-dialog-was-open), [§ Drain](dialogs.md#drain-which-takes-minutes), [context.md § When the new cluster does not work](context.md#when-the-new-cluster-does-not-work)) | 6 / 6 |

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
- `esc` closes exactly one level, always — with one named exception, and it
  is not a trap: a `Confirm` whose check has not answered yet
  (`Dialog::waiting`) puts the same dialog straight back rather than
  closing it, because the wait it is refusing to leave is bounded by the
  wire, not by this key
  ([dialogs.md § While the check is still on the wire](dialogs.md#while-the-check-is-still-on-the-wire),
  NOTES § D214).
- The confirm button is a `Span` with a reversed style; it is **not** live
  until the dry-run has returned and, for a typed-name dialog, until the typed
  string equals the object name ([dialogs.md](dialogs.md)). **The cancel
  button beside it is not exempt from the same wait**: it draws `theme::DIM`
  for exactly the window `Dialog::waiting` names and `theme::TEXT` from the
  moment a verdict exists, independently of whether the confirm side ever
  arms
  ([dialogs.md § While the check is still on the wire](dialogs.md#while-the-check-is-still-on-the-wire)).
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
- **Eleven cuts are deliberate, and this list is closed — a truncation found
  anywhere else on a screen is a bug, not a twelfth entry**
  ([NOTES § D266](../NOTES.md#d266--the-phase-11-close-six-screens-that-draw-something-false-and-a-freeze-set-one-phase-before-its-consumer-2026-09-13)).
  Five cut from the **back**; six cut from the **front**, and one of those six
  is one rule reused at six call sites rather than six separate rules — **the
  identity cut**, defined once below because it is the one most screens on
  this page now share.

  **Back-cuts.** The full text is one `⏎` away in three of the five, which is
  what makes cutting them legitimate — the reader loses nothing, only a
  keypress:

  1. The Alerts card's evidence line, capped at three wrapped lines with `…`
     at the cut
     ([alerts.md § How wide a card is, and how tall](alerts.md#how-wide-a-card-is-and-how-tall)).
     **The container picker's own state word is a second call site of this
     same cut**, word-boundary walk-back and all — not one `⏎` away but one
     `esc` then `d` away, on the object's own `describe` tab, which is the
     same *the reader loses nothing but a keypress* reasoning one hop
     longer ([detail.md § When a container's own state is what does not
     fit](detail.md#when-a-containers-own-state-is-what-does-not-fit)).
  2. The Resources browser's one-line summary under the table, whose name
     gives way to the sentence around it and is marked the same way
     ([resources.md § The line under the table](resources.md#the-line-under-the-table)).
  3. **The command log strip's own command text** — its real 76-column
     budget at the 80×24 floor (pane width minus the outer border minus the
     one-column margin `indented()` reserves on each side, §1) cannot always
     hold the whole teaching line. It walks back to a whole word first
     (below), and what it may never do is walk back far enough to drop the
     command's own `kind/name` word, or the running mark / outcome word that
     may follow it — see the strip's own rule, next bullet but one.
  4. **A `Confirm` dialog's `$` line** — the same 76-column-style budget, much
     narrower inside a nested box, and the same rule as the strip: trailing
     flags give way before the object's own `kind/name` word does. **One flag
     is not an equal trailing candidate here: `-n`.** Every other trailing
     flag still gives way whole, in order; `-n` gives way last, and past the
     point where the strip would drop it whole too, this line keeps
     degrading its **value** instead — down to a bare `-n…`, if it must —
     because this is the line a reader reads *before* anything is sent, and
     a namespace silently missing is the one loss on this page that can put
     a retyped command against the wrong object
     ([dialogs.md § The namespace flag never disappears without a
     trace](dialogs.md#the-namespace-flag-never-disappears-without-a-trace)).
     `box_width` reads this line too, for the same reason
     ([§5](#5-the-modal-layer)).
  5. **An Analysis row's own detail text**, when what `analysis.rs` built for
     it is taller than the pane — [analysis.md § A row taller than the
     pane](analysis.md#a-row-taller-than-the-pane-and-the-cut-that-keeps-the-cursors-row-on-screen).

  All five are marked with a character the reader can see and step by whole
  characters, never a byte. 1, 2 and 5 walk back to a whole word before they
  cut, because each is made of more than one token and a word with its last
  character sheared off would still look like a real one —
  `--show-managed-fiel` is not a flag a reader would notice was wrong
  ([detail.md's yaml tab](detail.md#the-yaml-tab) draws the case: the whole
  flag gives way, `…` lands right after `yaml`, and deleting just the `…`
  leaves a real command — the one `kubectl get -o yaml` already runs by
  default). 3 and 4 walk back the same way for a **flag**, and differ from
  the other three in one respect only: the object's own `kind/name` token —
  the one thing on the line that says *which* object this command runs
  against — is never a whole-word drop candidate. Where even the flag's own
  **value** would otherwise be dropped whole though part of it would still
  fit — a `-n <namespace>` whose namespace is the last thing standing between
  the command and the budget — the cut lands inside that value instead, at a
  character boundary, keeping as much of it as the room allows, behind the
  mark: dropping a value few readers would misread for a different one costs
  less than dropping it whole. **3 carries a second, distinct mark of its
  own**, `...` (three literal periods, three columns), never the single `…`
  glyph — because on this one strip `…` already means something else, the
  running mark ([`views::RUNNING`](../src/views.rs)) and the same character
  the outcome arrow replaces it with. The two used to be the same glyph, so a
  cut command and a command still running could draw the identical trailing
  character with nothing to tell them apart; they no longer can, on this
  strip only. Every other cut on this page keeps `…`.

  **4 goes one floor further than 3, for `-n` alone.** Once not even one
  character of `-n`'s value would fit, 3 still falls back to dropping the
  flag whole — cosmetic there, because the real call already carries the
  right namespace by the time that line is drawn — while 4 keeps the bare
  flag name standing, `-n…`, because it is read before anything is sent
  ([dialogs.md § The namespace flag never disappears without a
  trace](dialogs.md#the-namespace-flag-never-disappears-without-a-trace)).

  **Front-cuts.** None of these six promises the full text is one `⏎` away —
  what justifies the cut instead is the header's own reasoning
  ([§ 1a](#1a-the-header-row), [NOTES § D249](../NOTES.md#d249--the-layout-box-lands-from-a-second-session-the-header-gives-way-from-its-front-and-a-refusal-keeps-the-list-it-is-about-2026-09-06)):
  what two of these strings share with their neighbours sits at the front, so
  the front is the part each can afford to lose.

  6. The cluster picker's own name slot, where a fleet of contexts named
     after one cloud provider's naming convention share a long prefix and
     differ only near the end. **The container picker's own name column is a
     second call site of this same rule, not a twelfth cut** — it is drawn as
     the same list-picker shape on purpose
     ([detail.md § Choosing a container](detail.md#choosing-a-container-and-when-there-is-nothing-to-choose)),
     a flexible name column beside a fixed state column, and a pod that runs
     two sidecars of the same base image (`istio-proxy`, `istio-init`) is the
     same *shares a prefix, differs at the tail* shape a fleet of contexts is.
  7. The same row's own server-line label, in front of the address it
     introduces.
  8. The one-context sentence's own name, when there is only one row to say
     so about.
  9. The failure box's two names — the context that did not connect and the
     one `X` would take the reader back to
     ([NOTES § D264 ruling 5](../NOTES.md#d264--the-picker-round-a-failure-box-with-a-second-vocabulary-a-current-row-that-could-not-be-retried-and-a-cursor-on-a-context-nobody-chose-2026-09-13),
     `context.md` §§ The picker, The tag column, When the new cluster does not
     work).
  10. **The sidebar's own kind row**, for a discovery plural too long for its
      column — `persistentvolumeclaims`, `validatingadmissionpolicybindings`.
      Every row in the `List` reserves `List::highlight_symbol`'s own two
      columns whether or not it is the selected one (ratatui-widgets 0.3.2,
      `list/rendering.rs`, measured), so a kind row's own 15 columns are 20
      (the sidebar, fixed, §1) less 2 (the gutter) less 3 (a kind row's own
      indent, §2). Front-cut, one `…`,
      character boundary — a discovery plural is one token, so there is no
      word to walk back to: `persistentvolumeclaims` → `…ntvolumeclaims`,
      `validatingadmissionpolicybindings` → `…policybindings`. **§ 1's own
      reason for a fixed-width sidebar — "the labels are fixed-length
      strings" — is true of the product's own words (`ALERTS`, `workloads`,
      …) and was never true of a discovery plural**, the one sidebar label
      whose length the cluster chooses; this is its cut.
  11. **The identity cut** — one rule, six call sites, because a screen this
      product draws is a `namespace/name` or a bare cluster-scoped `name`
      (README rule 5) far more often than it is a browser row or a picker
      slot, and every one of the six needed the same fix at once
      ([NOTES § D266](../NOTES.md#d266--the-phase-11-close-six-screens-that-draw-something-false-and-a-freeze-set-one-phase-before-its-consumer-2026-09-13)):
      a `Confirm` dialog's own title
      ([dialogs.md §§ Scale, Restart, Delete](dialogs.md)); the same dialog's
      `$` line, where the object's own `kind/name` token has to give way to
      fit and does so this way rather than being dropped (back-cut 4, above);
      the *Already gone* body
      ([dialogs.md § The object went away while the dialog was
      open](dialogs.md#the-object-went-away-while-the-dialog-was-open)); the
      in-flight footer's own reason
      ([dialogs.md § While the call is
      running](dialogs.md#while-the-call-is-running) — **this replaces that
      section's old back-cut of the name**, which is D266's own reversal of
      that page's earlier rule 2); the Alerts card's identity row
      ([alerts.md § The age, and what it costs the
      name](alerts.md#the-age-and-what-it-costs-the-name)); and the detail
      tabs' own heading ([detail.md § The heading, when the name does not
      fit](detail.md#the-heading-when-the-name-does-not-fit)).

      **Why an identity front-cuts when nothing else on a card or a dialog
      does: two objects that share almost their whole name differ at its
      end**, not its front — `checkout-worker-service-canary` and
      `…-stable`, `payments/checkout-worker-service-canary-7d9f4bc86d-x2k9p`
      and its `-stable` sibling, `openshift-cluster-node-tuning-operator`'s
      `tuned` and its `node-tuning-operator`. A tail-cut of the combined
      string — this page's own rule until this round — lands inside the
      *name*, at whichever point the budget runs out, and the two names
      above collide there: both cut to `checkout-worker-servi…` at 80×24,
      naming two different Deployments with one identical dialog. **The
      rule, so a reviewer can check any width against it rather than a
      drawing:**

      1. `namespace/name` fits in `room` → drawn whole, no mark.
      2. It does not → the **namespace** gives way first, cut from its own
         front behind a leading `…`, however much of it the room after the
         mark, the `/` and the name in full still leaves. The `/` and the
         name are never touched here. Where nothing at all of the namespace
         fits, the visible string is `…/<name>` — the mark stands for the
         whole namespace, and the `/` still says there is one.
      3. Even `…/<name>` does not fit — the name alone, plus the mark and
         the `/`, is wider than `room` — only then does the **name** also
         give way, cut the same way, from its own front:
         `…/…<tail of the name>`. Reached at the in-flight footer's own
         33-column room by a name long enough on its own —
         `payments/checkout-worker-service-account-token-projector`
         ([dialogs.md § While the call is
         running](dialogs.md#while-the-call-is-running)) — and written down
         for every other surface regardless, because the rule has to hold at
         any width, not only the ones a mockup happens to draw.
      4. A bare, cluster-scoped name (a node) — case 2's plain front-cut,
         with no `/` to protect.

      No word-boundary walk-back in any case, the same reasoning the
      browser's own row-name cut already gives: a name is one token (or two
      joined by one `/`), so there is no word to find.

  Everything else on a card, every other string in the browser, and every
  command log line that fits is drawn whole and clips at the pane edge like
  any other string, if it clips at all.
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
