# 2026-09-28 — the four unwired detail tabs, `ui::unbuilt`, and the collapsed `esc`

Operator review, step 6, over the working tree at `1a9a35f` plus the uncommitted
family (`src/ui.rs`, `src/views.rs`, `src/main.rs` and the three test files).
Read-only: no cluster was started, nothing was built, the test host was not
touched. Every command below ran on the dev machine against the source tree.

## 1. How many places spell the two sentences

```
$ grep -rn 'reading the cluster' src/*.rs | grep -v '_tests.rs'
src/ui.rs:157:/// **`reading the cluster…`, spelled once** — [`note`] centres it on a pane with nothing else on
src/ui.rs:166:/// paragraph, `main.rs`'s `reading the cluster… N pods`, which this constant cannot serve because
src/ui.rs:168:const WAITING: &str = "reading the cluster…";
src/ui.rs:780:    /// *"reading the cluster… 2,140 pods"* — which is a fact about the store and not about the
src/ui.rs:3996:/// one screen (PRIOR-ART § C2). `reading the cluster…` is `screens/states.md`'s own line without
src/ui.rs:4748:    // drew `filter: "web"   esc clears it` over *reading the cluster…*, claiming a list had been
src/main.rs:7410:// `ui::unwired`; NOTES § D313, `ui::unread`): a pane that draws *reading the cluster…* for a fetch
src/main.rs:9223:    // both** — the defect this closes is a body that said *reading the cluster…* under a header
src/main.rs:9713:    // renewed*. Neither may say *reading the cluster…*, and under `Expired` that sentence is not
src/main.rs:9719:        // **Nothing is being read, so *reading the cluster…* is a false sentence** — the two
src/main.rs:9775:                    "reading the cluster… {} pods",
```

Of the eleven, one is the constant (`ui.rs:168`), one is `main.rs:9775`'s own
counted paragraph, and the other nine are comments.

```
$ grep -n 'WAITING' src/ui.rs
164:/// copy as a bare literal, which is why neither this doc nor a sweep over `WAITING`'s call sites
168:const WAITING: &str = "reading the cluster…";
4028:    let waiting = [Stripped::of(WAITING)];
4548:/// **A detail tab whose read was never wired says so, where [`WAITING`] would have claimed
4550:/// read anything). **The four tabs', as [`unwired`] is the browser's** — `WAITING` still serves
4556:/// `Pane::Loading` arms reached [`note`] for that second half, and `note` draws [`WAITING`] for a
4844:/// [`WAITING`] still serves the Alerts pane, whose read is real and does finish (NOTES § D313).
5964:        // **The fifth site D313 reaches, and it was a literal rather than [`WAITING`]** — this is
```

One use of the constant outside its own definition and doc: `ui.rs:4028`, inside
`fn note`.

```
$ grep -n '[^a-z_]note(' src/ui.rs | grep -v '_tests'
3611:            None => note(frame, area, screen, false, None, None),
3631:            Pane::Loading => note(
3650:                note(
4012:fn note(
```

`3611` is `View::Analysis(nth)` with no report; `3631` is `View::Alerts` /
`Pane::Loading`; `3650` is `View::Alerts` / ready-and-empty (`healthy = true`,
which `note`'s `!healthy` guard keeps off the `WAITING` branch).

The Analysis pane's `None` comes from `panes()`:

```
$ grep -n 'fn panes' -A 9 src/main.rs
7631:fn panes(
7632-    snapshot: Option<&ClusterSnapshot>,
7633-    findings: &[Finding],
7634-) -> Vec<(&'static str, Option<analysis::Report>)> {
7635-    PANES
7636-        .iter()
7637-        .map(|(label, produce)| (*label, snapshot.map(|s| produce(s, findings))))
7638-        .collect()
7639-}
```

All seven are `None` exactly when `snapshot` is `None`, i.e. before the store's
first snapshot.

## 2. The four sentences in code against the four in the screen file

```
$ python3 - <<'PY'
import re, pathlib
code = pathlib.Path('src/ui.rs').read_text()
spec = pathlib.Path('screens/detail.md').read_text()
tmpl = 'not built yet — k8rs cannot {}'
calls = re.findall(r'unread\(\s*frame,\s*\n?\s*area,\s*\n?\s*app,\s*\n?\s*screen,\s*\n?\s*&above,\s*\n?\s*"([^"]+)"', code)
calls += re.findall(r'unread\(frame, area, app, screen, &above, "([^"]+)"\)', code)
built = sorted(tmpl.format(c) for c in sorted(set(calls)))
bullets = sorted(m[1] for m in re.findall(r'^- \*\*(\w+):\*\* `([^`]+)`$', spec, re.M))
print("EQUAL AS SETS:", set(built) == set(bullets))
print("only in code :", set(built)-set(bullets))
print("only in spec :", set(bullets)-set(built))
for b in built: print(f"{len(b):3d} cols  {b!r}")
PY
EQUAL AS SETS: True
only in code : set()
only in spec : set()
 48 cols  'not built yet — k8rs cannot describe this object'
 54 cols  "not built yet — k8rs cannot fetch this object's events"
 52 cols  "not built yet — k8rs cannot fetch this object's logs"
 52 cols  'not built yet — k8rs cannot show this object as YAML'
```

(The apostrophe is ASCII `U+0027` in both files; the em-dash is `U+2014`, one
display column under `screens-check.py`'s own width rule.)

Widths, per sentence: logs 52, describe 48, yaml 52, events 54.

```
$ grep -n 'const MIN_WIDTH\|const SIDEBAR\|const PAD\|const BLOCK\|const FLOOR' src/ui.rs
81:const MIN_WIDTH: u16 = 80;
86:const SIDEBAR: u16 = 20;
93:const PAD: u16 = 2;
136:const BLOCK: u16 = 39;
144:const FLOOR: u16 = 3;

$ sed -n '4173,4177p' src/ui.rs
/// The card region: the pane less a two-column pad each side — 53 columns at the floor
/// (`screens/alerts.md` § The columns). [`indented`]'s reason for `Rect::inner`, at two columns.
fn padded(area: Rect) -> Rect {
    area.inner(Margin::new(PAD, 0))
}
```

`unread`'s `leads` half wraps to `padded(area).width` — 53 at the 80-column
floor — and its `dimly` half wraps to `area.width`, 57 at the same floor. Of
the four, only the events sentence (54) exceeds 53.

## 3. Who produces `Detailing::Tabs::stream`, and who reads it

```
$ grep -rn 'Detailing::Tabs' src/*.rs | grep -v '_tests'
src/ui.rs:1123:    /// reached straight off a card (`crate::views::Detailing::Tabs`, NOTES § D270). Nothing on
src/ui.rs:1365:        Some(Detailed::Tabs { open, from_step }) => Detailing::Tabs {
src/main.rs:9867:        Some(Opened::Tabs { from_step, .. }) => views::Detailing::Tabs {
src/main.rs:10021:                    // back` means for a `⏎` that came through it (`views::Detailing::Tabs`'s
src/main.rs:10332:        views::Detailing::Tabs { .. } => {
src/main.rs:10448:        views::Detailing::Tabs { .. } => Did::Nothing,
src/views.rs:2504:    matches!(open, Detailing::Tabs { containers, .. } if containers > 1)
src/views.rs:3181:            (Detailing::Tabs { stream: true, .. }, Tab::Logs, _) if picking(open) => {
src/views.rs:3184:            (Detailing::Tabs { stream: true, .. }, Tab::Logs, _) => {
src/views.rs:3195:            (Detailing::Tabs { .. }, Tab::Logs | Tab::Describe | Tab::Yaml | Tab::Events, _) => {
```

Two producers: `ui::detailing` (`ui.rs:1358`, `stream: !matches!(open.logs, Pane::Loading)`)
and `main::detailing` (`main.rs:9859`, `stream: false`). One reader:
`views::App::footer`, arms `:3181` and `:3184`.

```
$ grep -rn 'picking(' src/main.rs src/ui.rs src/views.rs | grep -v 'views.rs:250'
src/views.rs:3149:            Some(Modal::ContainerPick(_)) if picking(open) => {
src/views.rs:3181:            (Detailing::Tabs { stream: true, .. }, Tab::Logs, _) if picking(open) => {
src/views.rs:3425:        if matches!(self.modal, Some(Modal::ContainerPick(_))) && !picking(open) {
```

`:3149` and `:3181` are reached with `ui::detailing`'s value; `:3425` (inside
`App::escape`) is reached with `main::detailing`'s.

`ui::containers`, the source of the `containers` half, at `ui.rs:1338`:

```rust
match tabbed(screen).map(|open| open.logs) {
    Some(Pane::Ready(logs) | Pane::Denied(_, logs)) => logs.pod.containers,
    _ => &[],
}
```

So `stream == false` implies `containers == 0`; `containers == 0` does not imply
`stream == false` (a `Ready` pane whose pod snapshot has not landed).

Where the logs pane comes from today, `main.rs:9237-9247`:

```rust
ui::Detail {
    object,
    card: card(object),
    logs: &views::Pane::Loading,
    read: &views::Pane::Loading,
    yaml: &views::Pane::Loading,
    events: &views::Pane::Loading,
    secret_without_keys: false,
},
```

`Console` carries no pane field; the four `Pane`s are built in `drawn()`'s own
frame.

## 4. The `f` key against the footer that now withholds it

```
$ grep -n "KeyCode::Char('f')" -A 4 src/main.rs
10049:        KeyCode::Char('f') if open != views::Detailing::Closed => {
10050-            console.app.following = !console.app.following;
10051-            Did::Changed
10052-        }
```

No `stream` or `Pane` guard. `console.app.following` is read in `ui::stream`
(`ui.rs:5858`), which a `Pane::Loading` logs tab does not reach.

There is no `c` arm in `pressed`:

```
$ grep -cn "KeyCode::Char('c') && control" src/main.rs
1
$ grep -n "KeyCode::Char('c')" src/main.rs | grep -v '_tests'
10078:    if (key.code == KeyCode::Char('q') && !control) || (key.code == KeyCode::Char('c') && control) {
```

## 5. `esc` on the browser's unwired kind pane

`back: true` is produced in one place, `ui::offered` at `ui.rs:1709-1721`:

```rust
View::Resources(at) => match screen.browser {
    Pane::Loading => {
        return Offer::Nothing { switch: stranded, back: true };
    }
```

`screen.browser` is `&UNOPENED` on every console frame (`main.rs:9280`), a
`static views::Pane<k8s::Table> = Pane::Loading`.

`App::escape`, `views.rs:3424-3495`, landed arm:

```rust
None if open != Detailing::Closed => {}
...
None if back => self.open(NavItem::Alerts),
None => match self.filters.clears() { ... },
```

`App::open`, `views.rs:3542-3562`, on a view change: `content = Cursor::default()`,
`filters = Filters::default()`, `typing = None`. `self.nav` and `self.focus` are
not written.

```
$ grep -n 'fn next' -A 7 src/views.rs | sed -n '1,9p'
2183:    pub fn next(self) -> Panel {
2184-        match self {
2185-            Panel::Sidebar => Panel::Content,
2186-            Panel::Content => Panel::Sidebar,
2187-        }
2188-    }
```

```
$ sed -n '10353,10373p' src/main.rs
    match console.app.focus {
        views::Panel::Sidebar => {
            ... step(&mut console.app.nav, &keys, towards);
        }
        views::Panel::Content if console.app.view == views::View::Alerts => {
            ... step(&mut console.app.content, &keys, towards);
        }
        views::Panel::Content => return Did::Nothing,
    }
    Did::Changed
```

```
$ sed -n '10475,10490p' src/main.rs
        views::Detailing::Closed => match console.app.focus {
            views::Panel::Sidebar => {
                ... console.app.open(*item);
                Did::Changed
            }
            views::Panel::Content => {
                let Some(card) = selected(console, cards) else {
                    return Did::Nothing;
                };
```

## 6. Which object `l` opens the logs tab on

`main::tabbed`, `main.rs:10518-10533`:

```rust
let object = card
    .pods()
    .first()
    .map_or_else(|| card.owner.clone(), |id| (*id).clone());
```

`views::Card::pods`, `views.rs:464-472`, keeps only
`finding.object.kind == ObjectKind::Pod`. A card whose findings are all about a
Node therefore yields an empty `pods()` and `tabbed` opens `card.owner`, a Node.

`l`, `d`, `y` are bound with no kind guard, `main.rs:10077-10079`:

```rust
KeyCode::Char('l') => tabbed(console, cards, views::Tab::Logs),
KeyCode::Char('d') => tabbed(console, cards, views::Tab::Describe),
KeyCode::Char('y') => tabbed(console, cards, views::Tab::Yaml),
```

## 6b. The clause literals, per call site

```
$ grep -n "fetch this object's events\|fetch this object's logs\|describe this object\|show this object as YAML" src/ui.rs
4835:/// `fetch this object's logs` for a detail tab (NOTES § D310, § D313). Two panes spelling one fact
5760:        Pane::Loading => unread(frame, area, app, screen, &above, "fetch this object's logs"),
5874:            return unread(frame, area, app, screen, &above, "describe this object");
5970:        Pane::Loading => lines.extend(set(&unbuilt("fetch this object's events"), region, dim)),
6005:                "fetch this object's events",
6164:            return unread(frame, area, app, screen, &above, "show this object as YAML");
```

Four clauses, five call sites. `5970` is `ui::block`'s arm over `Detail::events`
and `6005` is `ui::events`'s arm over the same `Pane`.

## 7. The name `unread` elsewhere in the product

```
$ grep -rn '\bunread\b' src/*.rs | grep -v '_tests' | grep -E 'fn |Unread'
src/ops.rs:1998:fn unread(object: &str, namespace: &str, why: &str, message: Option<&str>) -> String {
src/ui.rs:4559:fn unread(
src/views.rs:862:    Unread,
src/ui.rs:3484:                NavItem::Unread => (1, "could not read", theme::DIM, Vec::new()),
```

## 8. Fenced mockups in the new screen section

```
$ awk '/^## Before any of the four tabs has read anything/,/^## The logs tab/' screens/detail.md | grep -c '^```'
0
```

```
$ grep -n 'stay bound and stay named' screens/widgets.md
673:`↑↓ move` all stay bound and stay named — they are viewing and moving, not
```

`screens/widgets.md:669-675` reads, in full:

> Analysis and every detail tab never name `s` or `r` on their own footer, so
> none of them has anything on the line a call in flight makes false — except
> `q quit` … `[ ] tabs`, `f follow`, `c container`, `esc back`, `⏎ open` and
> `↑↓ move` all stay bound and stay named

The footer table 9 lines below it, same file, now carries a `Pane::Loading` row
for the logs tab that draws neither `f follow` nor `c container`.

`screens/detail.md`'s new section states:

> **Out of scope for this fix, and unchanged.** … this is four of `WAITING`'s
> six call sites in `ui.rs`, not all six (NOTES § D313).

## 9. What was not run

Nothing was compiled and no test was executed — `tester` holds the mirror and
`CLAUDE.md § Running it` puts every `cargo` run on the test host. No cluster was
started; the four-node `k8rs` fixture cluster was left alone. The behaviours
above are read off the source and off the screen files.

Two claims could only be settled by running, and are named rather than argued:

- Whether the events sentence wraps onto two rows in the `leads` half at the
  80-column floor. Command that would settle it, on the test host:
  `cargo test --locked --all-targets -- --nocapture a_block_taller_than_the_pane_keeps_the_tabs_own_sentence_and_scrolls_for_the` —
  the test prints the drawn rows.
- Whether the four sentences and the bare logs footer appear on a real binary.
  Command that would settle it, on the test host, one terminal:
  `tmux new-session -d -x 80 -y 24 'target/debug/k8rs --read-only'` then
  `tmux send-keys l` and `tmux capture-pane -p`. `test.md` § K's first row
  records such a run by `dev-ui`; it has not been repeated on the landed tree.
