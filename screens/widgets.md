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
| right | context · namespace scope · connection state · `admin` / `read-only` · a TLS warning | right |

- **The context is never truncated and never clipped.** `prod-eu` and
  `prod-eu-2` differ by one character, and the header is what tells you which
  cluster a `ctx`+`s` keypress is about to scale. It is laid out first and
  keeps its full width.
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

## 2. Element → widget

| Screen element | Widget | State object | Notes |
|---|---|---|---|
| Header row | three `Paragraph`s in a `Layout::horizontal` | — | right zone laid out first at its full width, left next, `k8rs` drawn into what is left only if ≥2 blank columns remain each side (§1a) |
| Outer frame | `Block::bordered()` | — | no titles — the header is its own row |
| Sidebar (ALERTS / RESOURCES / ANALYSIS + children) | `List` | `ListState` | flat `Vec<NavItem>`; group headers are unselectable rows, `↑↓` skips them |
| Sidebar counts (`3 ● 7 ▲`, `1 ▲`, `30d`, `12`) | right-aligned `Span` in the same `ListItem` | — | part of the row, not a second column |
| Finding card (Alerts) | `List` of **multi-line** `ListItem` | `ListState` | one `ListItem` = one card = 3–5 `Line`s + a blank; selection highlights the whole card, and `ListState` does the scrolling for free |
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

Nothing here is a custom widget. If a screen seems to need one, the screen is
wrong before the widget set is.

## 3. Where the state lives

Every `ListState`, `TableState`, `ScrollbarState`, tab index and scroll offset
lives in **`views.rs`**, which
[NOTES § File layout](../NOTES.md#file-layout) already defines as "per-view
state: selection, filters, tabs, scroll". `ui.rs` receives `&mut ViewState` and
draws it.

The `&mut` is unavoidable — `render_stateful_widget` writes the resolved offset
back — but that is the only mutation `ui.rs` performs. It computes nothing,
stores nothing, and decides nothing that survives the frame. This is what keeps
`ui.rs` under the ~800-line line that would trigger `dialog.rs`
([NOTES § D11](../NOTES.md#d11--the-ninth-file-pre-approved)), and it is what
makes a view's behaviour testable without a terminal.

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
    None, Confirm(..), TypedDelete(..), Refused(..),
    Help, ContainerPick(..), ContextPick(..),
}
```

- **One modal at a time — the enum makes stacking unrepresentable.** No modal
  stack, no z-index. A dialog that could open over a dialog is how a
  confirmation ends up applying to the wrong object.
- Draw order is `Clear` over the centered `Rect`, then the block, then the
  content. Without `Clear` the pane underneath shows through — ratatui does
  not clear for you.
- The centered rect comes from one helper (`Layout` twice, vertical then
  horizontal), used by every modal. Not six hand-computed rectangles.
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
| Colours, styles, the 16-colour fallback | `theme.rs` · [docs/tech-stack § Visual identity](../docs/tech-stack.md#visual-identity) |
| The key map | [NOTES § D12](../NOTES.md#d12--the-key-map-and-two-keys-deleted) · [help.md](help.md) |
| What each screen says | the mockups in this directory |
| Which findings exist | [NOTES § v1 rule set](../NOTES.md#v1-rule-set) |
| The order things get built | [todo.md](../todo.md) |
