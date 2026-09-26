# 2026-09-24 — the console event loop, read at HEAD + working tree

`k8s-admin`, operator review of the Phase 12 `main.rs` `tokio::select!` box and the
coalescing test beside it. **No cluster was brought up and no binary was built** —
`tester` held the test host for `just check` and nothing builds on this machine
([D267](../NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)).
Everything below is either a `grep`/`sed` over the working tree or arithmetic over a
rule the code states; each is labelled. The measurements a run still owes are in
§ 7.

Tree: `development`, working tree dirty — `src/main.rs`, `src/main_tests.rs`,
`src/ui.rs`, `src/ui_tests.rs`, `src/views.rs`, `src/views_tests.rs`.

```
$ git diff --stat
 src/main.rs       | 1668 +++++++++++++++++++++++++
 src/main_tests.rs | 1584 ++++++++++++++++++++++
 src/ui.rs         |   76 +-
 src/ui_tests.rs   |   18 +-
 src/views.rs      |  244 ++++-
 src/views_tests.rs|   56 +-
```

## 1. Which keys the router binds, read off `pressed` (`src/main.rs:8022-8163`)

Read by hand against `screens/help.md`'s map. `tab`, `esc`, `↑↓`/`jk`, `⏎`, `[`,
`]`, `f`, `/`, `n`, `l`, `d`, `y`, `r`, `ctrl-d`, `X`, `?`, `q`, `ctrl-c` are
bound. `s` is not: `KeyCode::Char('s')` reaches `_ => Did::Nothing`.

```
$ grep -n "KeyCode::Char('s')" src/main.rs
(no output)
```

`views::Offer::act` (`src/views.rs:2648`) answers `Act { scalable: true, .. }` for
`("apps","deployment")`, and `App::footer`'s literal for that pair is
`↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit`
(`src/views.rs:3114`).

Modifier handling inside the same function: the typing branch filters control
(`KeyCode::Char(typed) if !control`, `src/main.rs:8061`); the command branch does
not. So `key.code` alone decides these arms, whatever `key.modifiers` holds:

| pressed | arm reached |
|---|---|
| `ctrl-r` | `wanting(console, store, RESTART)` — `src/main.rs:8160` |
| `ctrl-l` | `tabbed(.., Tab::Logs)` — `:8157` |
| `ctrl-y` | `tabbed(.., Tab::Yaml)` — `:8159` |
| `ctrl-n` | `typing = Some(Typing::Namespace)` — `:8152` |
| `ctrl-f` | `following = !following` (tab open) — `:8143` |
| `ctrl-j` / `ctrl-k` | `moved(..)` — `:8126-8127` |

In `over_modal`'s `ContextPick` arm (`src/main.rs:8239`) every `KeyCode::Char` is
pushed into `picker.filter`, `'/'` included. `screens/context.md:46` draws that
box's footer as `↑↓ move  / filter  ⏎ switch  esc cancel`, and
`screens/context.md:1149` states *nothing on this screen echoes the typed filter
back as it is typed*.

## 2. Where the router's `Offer` comes from, against `ui::offered`'s five facts

`wanting` (`src/main.rs:8476-8504`) builds its own value:

```
let (group, kind) = ui::addressed(&card.owner.kind);
let offer = views::Offer::act(group, kind);
if !console.app.may_mutate(offer, op) { return Did::Nothing; }
```

`ui::offered` (`src/ui.rs:1494-1597`) — the value the footer is drawn from — folds
five facts, and the last line is `Some((group, kind)) if withheld(screen).is_none()
=> Offer::act(group, &kind)`. `withheld` (`src/ui.rs:1651-1675`) answers `Some` for:

| fact | source | router reads it? |
|---|---|---|
| nothing selected / empty pane | pane + cursor | yes, via `selected()` returning `None` |
| filter hid every row | `shown_cards` | yes, same route |
| Analysis / browser view | `app.view` | yes, `selected()` guards `view != Alerts` |
| a detail slot is open | `screen.detail.is_some()` → `Offer::Move` (`src/ui.rs:1502`) | **no** |
| writes dead (`--read-only`, `Unaudited`) | `screen.writes.why()` | **no** |
| `Link::Connecting` / `Lost` / `Expired` | `screen.link` | **no** |
| clock untrusted | `ui::clock` | **no** |

`Console` holds `writes` (`src/main.rs:7202`) and `linked()` is computed per frame
in `drawn` (`src/main.rs:7833`); neither is read in `wanting`.

Tests fed to the mutating keys (`src/main_tests.rs:13887`, `:14783`, `:14876`):
nothing selected, empty store, browser view, pod (for `ctrl-d`), Deployment (for
`r`). No test opens a detail tab, sets `Writes::Unaudited`, or moves the link.

## 3. What `Log::outcome` is handed, and what the strip then draws

`settled` (`src/main.rs:8658`) calls `console.log.outcome(&performed.plainly())`.
`views::SAID` is 32 bytes (`src/views.rs:1637`) and its doc names the argument as a
*short form* — `rejected`, `not sent`, `refused`, `login expired`.
`ops::Performed::plainly` (`src/ops.rs:629`) returns `verdict(outcome)` joined with
`and_said`; the arms are at `src/ops.rs:1416-1482`.

Arithmetic over `k8s::text`'s own rule (`src/k8s.rs:284-308`: strip, then cut in
`cap-3..=cap` at a char boundary, then append `SHORTENED` =
`"… (shortened by k8rs)"`), for the eight endings `main_tests.rs:14397` enumerates,
against the strip's 76 columns at the 80-column floor and the command
`$ kubectl rollout restart deployment/web -n payments` (51 columns):

```
$ python3  # k8s::text(plainly(), 32), then 3 + len("→ ") + word
Done                   tail= 24 cols   room_for_command= 52
Started                tail= 58 cols   room_for_command= 18
Cancelled              tail= 58 cols   room_for_command= 18
Gone                   tail= 58 cols   room_for_command= 18
Changed                tail= 58 cols   room_for_command= 18
NotSent/Unanswered     tail= 56 cols   room_for_command= 20
Failed/Refused+said    tail= 56 cols   room_for_command= 20
outcome None (D21)     tail= 56 cols   room_for_command= 20
```

The cancelled line, as composed:

```
$ kubectl rollout restart deployment/web -n payments   → nobody confirmed it, so nothing … (shortened by k8rs)
```

`ui::strip` (`src/ui.rs:1912-1927`) sets `room = 76 - width(outcome)` and calls
`command_cut(command, room, STRIP_CUT)`. In `command_cut`
(`src/ui.rs:5849-5906`) the protected head is
`$ kubectl rollout restart deployment/web` = 39 columns; with `room` 18–20 the
`width(&line[..head_end]) <= room` test fails, `kept` stays `None`, then
`columns.checked_sub(width(fixed))` with `fixed` = 36 columns also fails, so the
line falls to `tailed(line, columns, mark)` — a plain back-cut at 18 columns.

`installed` (`src/main.rs:8611-8624`) appends that line on `Published::Opening`,
i.e. when the dialog opens. `views::Log::sent` (`src/views.rs:1747`) always appends
`RUNNING` (`…`). `ops::delete` sends no check
([D225](../NOTES.md#d225--the-five-rulings-delete-could-not-be-briefed-without-and-the-preflight-it-declines-2026-09-04)
ruling 1), so at that moment zero requests have been sent for a delete.

[D233](../NOTES.md#d233--the-dialogs--line-and-the-command-logs-are-not-the-same-line-and-the-read-side-is-a-manifest-rather-than-a-feed-2026-09-05) ruling 1: *"The log line is appended when `ask`
returns `Answer::Confirmed`… `Cancelled`, `Gone` and `Changed` append nothing."*
`screens/dialogs.md:25` and `:116` both draw the line in the strip under an open
dialog, unmarked.

## 4. What the two unwired panes draw

`Screen::browser` is `&UNOPENED` = `views::Pane::Loading` (`src/main.rs:7818`,
`:7873`). `ui.rs`'s browser arm for `Pane::Loading` is
`note(frame, body, screen, false, None)` (`src/ui.rs:4343`), and `note`
(`src/ui.rs:3656-3692`) draws `screen.note` whenever it is non-empty.
`Screen::note` is `notes(snapshot, &store.still_listing())`
(`src/main.rs:7774`, built at `:7946-7974`), which on a listed store returns one
paragraph: `"{pods} and {nodes} checked, none of them is in trouble right now."`

The four detail panes are `&views::Pane::Loading` (`src/main.rs:7785-7788`), and
each tab's `Loading` arm draws `WAITING` = `"reading the cluster…"`
(`src/ui.rs:158`, arms at `+434`, `+552`, `+647`, `+675`, `+837` inside
§ THE DETAIL TABS), falling back to `note(...)` when there is no pinned block.

`Screen::refused` is `views::Refused::default()` (`src/main.rs:7830`) — every key
unmarked, which is `Refused`'s documented fail-open (`src/views.rs:2268`).

No frame in `main_tests.rs` is drawn with `console.app.view =
View::Resources(..)` or with `console.opened = Some(Opened::Tabs { .. })`:

```
$ grep -n 'View::Resources' src/main_tests.rs | sed -n '1,5p'
13902:    browsing.app.view = views::View::Resources(0);
```

(that one asserts key refusals, it does not call `framed`).

## 5. `linked()` against the fault taxonomy

`linked` (`src/main.rs:7920-7936`): `Expired` → `Link::Expired`; `Unanswered` or
`Unfinished` → `Link::Lost`; anything else leaves `Live`/`Connecting`.

`k8s::Fault` has eleven variants (`src/k8s.rs:838-997`). `Fault::NoCredential` is
reachable on a watch mid-session — `watch_fault` → `fault(error)` →
`kube::Error::Service(boxed) if boxed.downcast_ref::<kube::client::AuthError>()`
(`src/k8s.rs:1269-1273`), the arm whose doc says *measured on the built binary
against a login program that answers once and then exits 1*. `Fault::standing`
(`src/k8s.rs:1023-1049`) is `true` for it.

## 6. `Detailing` is derived twice

`main::detailing(&Console)` (`src/main.rs:7983-7992`) reads `console.opened`;
`ui::detailing(&Screen)` (`src/ui.rs:1243-1252`) reads `screen.detail`. `drawn`
builds `detail` as `card(owner).map(ui::Detailed::Pods)` (`src/main.rs:7800`), so a
card that leaves the list sets `screen.detail` to `None` while `console.opened`
stays `Some(Opened::Pods(..))`. `screens/detail.md:404-409` rules that state:
*"Only the group reaching zero … closes the step and hands back to the view
beneath, the same `esc`-shaped return."*

Per-frame and per-keypress cost, read off the calls: `drawn` runs one
`Store::snapshot` (a deep clone — `src/k8s.rs:2355-2383`), one `analyze`, one
`views::cards` and seven `analysis::*` reports; `carded` (`src/main.rs:8340`) runs
snapshot + `analyze` + `cards` again, and is called from `moved`, `selected`
(hence `entered`, `tabbed`, `wanting`).

`Cursor`'s anchors: the router passes `Vec<Option<&str>>` of all `None` for the
content and sidebar cursors (`src/main.rs:8307`, `:8320`, `:8366`), matching
`ui.rs`'s own alerts list (`src/ui.rs:3863`); the browser's rows pass real uids
(`src/ui.rs:4536`). Nothing calls `App::content.follow(..)`. `views::cards` sorts
by severity then `newest_first(a, b, now)` (`src/views.rs:598-602`), and `now`
moves every frame.

## 7. What a run still has to measure — none of this was run

1. `framed()` with `app.view = View::Resources(0)` on a listed store: read the
   pane's body text.
2. `framed()` with `console.opened = Some(Opened::Tabs { .. })` on each of the four
   tabs.
3. The strip row for each of the eight endings at 80×24 — the drawn cells, not the
   composed `String`.
4. `r` and `ctrl-d` pressed with (a) a tab open, (b) `Writes::Unaudited`, (c) a
   `Trouble` carrying `Fault::Unanswered`, (d) a skew sentence set.
5. `ctrl-r` against the built binary in a real terminal, to confirm crossterm
   delivers `Char('r')` + `CONTROL` on this platform.
6. Idle CPU and one frame's cost at ~1000 and ~5000 pods (todo.md § Phase 12's own
   last box).
