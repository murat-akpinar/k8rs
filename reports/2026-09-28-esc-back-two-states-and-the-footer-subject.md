# 2026-09-28 — the two focus states of the unwired kind pane, and what a footer is a fact about

Second operator read of the same box, after [D312](../NOTES.md#d312--esc-back-was-an-identity-write-and-the-key-that-got-the-reader-out-was-never-on-the-footer-2026-09-28)
and its amendment. **Nothing compiled, no cluster, no test host** — `tester` held the
mirror. Read off the working tree at `/home/shyuuhei/GIT/k8rs`. First read:
[`2026-09-28-the-unwired-kind-pane-and-the-one-dash-flag.md`](2026-09-28-the-unwired-kind-pane-and-the-one-dash-flag.md).

## 1 — which keys answer in each of the two focus states

Read off `src/main.rs`. `moved()`'s routing:

```
10332:    match console.app.focus {
10333:        views::Panel::Sidebar => {            // steps app.nav, no reference to screen.browser
...
10348:        views::Panel::Content if console.app.view == views::View::Alerts => {
...
10353:        views::Panel::Content => return Did::Nothing,
```

`entered()`'s routing, and the guard that decides its `Panel::Content` arm:

```
10426: fn selected(console: &Console<'_>, cards: &[views::Card]) -> Option<views::Card> {
10427:     if console.app.view != views::View::Alerts {
10428:         return None;
10429:     }
...
10454:        views::Detailing::Closed => match console.app.focus {
10455:            views::Panel::Sidebar => { ... console.app.open(*item); Did::Changed }
10469:            views::Panel::Content => {
10470:                let Some(card) = selected(console, cards) else {
10471:                    return Did::Nothing;
```

`tab`:

```
10015:        KeyCode::Tab => {
10016:            console.app.focus = console.app.focus.next();
```

`Panel::next` (`src/views.rs:2184`–`:2187`) is a two-way toggle:
`Sidebar => Content`, `Content => Sidebar`.

Resulting table for `View::Resources(at)` with `screen.browser == Pane::Loading`:

| key on the footer | `focus == Sidebar` (entry) | `focus == Content` (after `tab`) |
|---|---|---|
| `↑↓ move` | steps `app.nav` | `Did::Nothing` |
| `⏎ open` | `App::open(item)` | `selected() -> None` ⇒ `Did::Nothing` |
| `esc back` | `open(NavItem::Alerts)` — view changes | `focus = Sidebar` — nothing drawn changes |
| `tab` (on `?`, not on the footer) | → Content | → Sidebar, arrows live again |

Footer literals the code draws (`src/views.rs:3211`–`:3222`), display columns:

```
$ python3 -c 'measure the two new literals'
 45 |↑↓ move  ⏎ open  esc back  ? all keys  q quit|
 63 |X switch cluster  ↑↓ move  ⏎ open  esc back  ? all keys  q quit|
```

Both are now pinned by a screen file:

```
$ grep -rlF "X switch cluster  ↑↓ move  ⏎ open  esc back" screens/
screens/states.md
```

## 2 — what `offered` reads, and what every footer on the product is a fact about

```
$ grep -c "app.focus" src/ui.rs
0
```

`src/ui.rs`'s `offered` takes `(app: &App, screen: &Screen)` and never reads
`app.focus`. `wanting()` (`src/main.rs:10525`) reaches `selected()`, whose only
guard is `view != View::Alerts` — not `focus`. So on `View::Alerts` with
`focus == Panel::Sidebar`, `s`, `r` and `ctrl-d` act on the card under
`app.content` while `↑↓` are walking `app.nav`.

`theme::FOCUS`:

```
$ grep -n "FOCUS" src/theme.rs scripts/signal-guard.py | head
src/theme.rs:  FOCUS ... Signal::Reverse
scripts/signal-guard.py:  FOCUS in the signal table
$ grep -rlF "FOCUS" screens/ | wc -l
0
```

## 3 — the new section's seven fenced blocks, and which one carries no footer

```
$ python3 - <<'EOF'  # blocks of § A kind the browser cannot list yet, in file order
index 0 (block 1): rows=21  footer=YES  ↑↓ move  ⏎ open  esc back  ? all keys  q quit
index 1 (block 2): rows=21  footer=YES  X switch cluster  ↑↓ move  ⏎ open  esc back  ? all keys  q quit
index 2 (block 3): rows= 6  footer=NONE
index 3 (block 4): rows=21  footer=YES  ↑↓ move  ⏎ open  esc back  ? all keys  q quit
index 4 (block 5): rows=21  footer=YES  ↑↓ move  ⏎ open  esc back  ? all keys  q quit
index 5 (block 6): rows=21  footer=YES  ↑↓ move  ⏎ open  esc back  ? all keys  q quit
index 6 (block 7): rows=21  footer=YES  ↑↓ move  ⏎ open  esc back  ? all keys  q quit
EOF
```

Frame widths per block: `[68,70] [70] [49] [70,72] [68,70] [80] [80]`.
`src/ui_tests.rs`'s sweep loop runs `for nth in 0..6` over the footer-bearing
six and maps `1 => Link::Expired`, `2 => Writes::Unaudited`; filtered index 1 is
file index 1 (expired) and filtered index 2 is file index 3 (audit banner). The
loop's own comment names the footer-less block **the second**; it is index 2, the
**third**, and the same comment's index list `0, 1, 3, 4, 5 and 6` is right.

## 4 — the two wrapped mockups, against ratatui's own centring arithmetic

`unwired` is handed the unpadded `body`, so `area.width` is the content pane's
own: 47 at the 70-column page, 57 at the 80-column floor. `dimly` calls
`wrapped(said, area.width)`, maps each line through `Line::centered()`, and
hands them to `centred`, which is
`area.centered(Length(widest.max(BLOCK=39).min(area.width)), Length(lines))`.

Predicted left margin inside the content pane, against the margin the mockups
draw:

| mockup | pane | line | width | block | predicted lead | drawn lead |
|---|---|---|---|---|---|---|
| index 0/1/3 `jobs` | 47 | 1 of 1 | 37 | 39 | 4 + 1 = **5** | 5 |
| index 4 `persistentvolumeclaims` | 47 | 1 of 2 | 32 | 39 | 4 + 3 = **7** | 7 |
| index 4 | 47 | 2 of 2 | 22 | 39 | 4 + 8 = **12** | 12 |
| index 5 `jobs` | 57 | 1 of 1 | 37 | 39 | 9 + 1 = **10** | 10 |
| index 6 `customresourcedefinitions` | 57 | 1 of 2 | 32 | 39 | 9 + 3 = **12** | 12 |
| index 6 | 57 | 2 of 2 | 25 | 39 | 9 + 7 = **16** | 16 |

Break point predicted from `wrapped` (`src/ui.rs:6548`, breaks at the first
whitespace once the next word would exceed `columns`): after `list`, at 32
columns, for both.

## 5 — `--context=-foo`, the escape hatch the new doc clause names

`known` (`src/main.rs:2389`–`:2406`) accepts
`arg.strip_prefix(flag).is_some_and(|r| r.starts_with('='))` for `CONTEXT`, so
`--context=-foo` is not refused by the dashed-word find at `:2418`. The
three-spellings-of-nothing check above it (`:2241`–`:2252`) tests
`rest == "="`, which `"=-foo"` is not. The pair check at `:2216` needs two
words. So the value reaches `live_context`.
