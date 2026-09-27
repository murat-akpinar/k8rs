# 2026-09-27 — the first-launch pane: which link value reaches which paragraph

Operator review of Phase 13 box 3 (`notes`'s `ui::Link` discriminator, `offered`'s
`stranded`, `screens/states.md` § *Over a pane with nothing to show yet*). **No
cluster and nothing on the test host** — `tester` held the mirror. Everything below
is read off the working tree, off the vendored dependency source, or arithmetic.

Tree: `development`, working tree modified (`NOTES.md backlog.md screens/states.md
screens/widgets.md src/main.rs src/main_tests.rs src/ui.rs src/ui_tests.rs
src/views.rs`) on top of `e0fdb7b`.

## 1. The four link values against the four paragraph arms

`src/main.rs`'s `notes` matches `(link, snapshot)`:

| `link` | `snapshot` | first paragraph drawn |
|---|---|---|
| `Unconnected` | any | `⚠ Not connected to the cluster right now.` |
| `Lost` | `None` | `⚠ k8rs asked, and nothing came back.` |
| `Connecting` | `None` | `reading the cluster… N pods` |
| **`Expired`** | **`None`** | **`reading the cluster… N pods`** |
| `Live` / any | `Some` | `N pods and M nodes checked, none of them is in trouble right now.` |

The fourth row is asserted, with a non-zero count, by the box's own new test:

```
$ grep -n "ui::Link::Expired, \"reading the cluster" -B 6 src/main_tests.rs
    for (link, first) in [
        (ui::Link::Connecting, "reading the cluster… 2,140 pods"),
        (ui::Link::Expired, "reading the cluster… 2,140 pods"),
```

`screens/states.md` § *First launch* states the rule that row contradicts:

> A number that has stopped moving and cannot be trusted to still be current is
> exactly the *"reading the cluster… N pods"* lie this section exists to retire,
> whichever number N happens to be.

Reachability of `(Expired, None)`, read rather than run: `k8s::fault` maps a `401`
through `answer(status)` to `Fault::Expired`; `linked` (`src/main.rs:9600-9652`)
returns `ui::Link::Expired` on any watch carrying it, before the `Lost` test; and
`Store::snapshot` is `None` while any watch is inside its first LIST, which every
watch is when the credential is dead at launch. `Fault::standing` is true of
`Expired`, so retrying cannot clear it.

## 2. What `Lost` + `snapshot: None` actually covers

`linked`'s `Lost` needs `dropped && !answering`: some watch carrying
`Unanswered`/`Unfinished`, and **no** `WATCHED` kind without a trouble row. With
`Store::snapshot` `None` until every initial LIST lands, the pair holds in three
shapes, not one:

- nothing ever arrived (the box's subject);
- the pods LIST had decoded *n* objects into `filling` and the socket died — `n` is
  `Listing::so_far`, which `notes` receives and does not read on this arm;
- four kinds complete, the fifth still listing, then the link drops — the
  ordinary shape of a mid-read drop on a large cluster, since pods is the slow LIST.

`screens/states.md` § *First launch* concedes the second: *"the pods `LIST` behind
this screen may genuinely have read some pages before the connection went —
`so_far` need not be zero"*, and rules the trigger as *nothing is currently
answering*, not *nothing has ever arrived*.

## 3. `Unfinished`, and whether the retry claim holds

```
$ grep -rn "stop_waiting" src/main.rs | grep -v "^src/main.rs:[0-9]*: *///"
src/main.rs:3975:                            store.stop_waiting();
```

`src/main.rs:4033` — *"**Which is why nothing here calls `k8s::Store::stop_waiting`**"*
(the `--live`/console path); `src/main_tests.rs:3487` — *"`k8s::Store::stop_waiting`
is unreachable outside `--once`"*. `Trouble::fault` (`src/k8s.rs:1315-1319`) produces
`Fault::Unfinished` only from `unfinished`, which only `stop_waiting` sets. So in the
console `Lost` comes only from `Fault::Unanswered`, which `Fault::standing` answers
`false` for — kube re-lists.

## 4. What a dead port and a blackholed address produce

`src/k8s.rs:1244-1275` — `fault()` ends `_ => Fault::Unanswered`, so ECONNREFUSED (a
released port) and a connect timeout are the same arm. That is why the unit
fixture's `ErrorKind::TimedOut` and the binary run's RST exercise one path.

From the vendored dependency source, not reasoned:

```
$ cd ~/.cargo/registry/src/*/kube-client-4.2.0 && grep -rn "DEFAULT_CONNECT_TIMEOUT" src/config/mod.rs src/client/builder.rs
src/config/mod.rs:190:            connect_timeout: Some(DEFAULT_CONNECT_TIMEOUT),
src/config/mod.rs:272:            connect_timeout: Some(DEFAULT_CONNECT_TIMEOUT),
src/config/mod.rs:338:            connect_timeout: Some(DEFAULT_CONNECT_TIMEOUT),
src/config/mod.rs:418:const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
src/client/builder.rs:229:        connector.set_connect_timeout(config.connect_timeout);
```

So an address whose packets are dropped rather than refused produces no watch
failure for ~30 s; with no failure, `linked`'s `dropped` is false and the frame is
`Connecting`.

## 5. Which pane gets which half of the fix

- `src/main.rs:9199-9206` — `alerts_pane = view == Alerts && opened.is_none()`;
  `notes(...)` runs only then, and every other pane is handed `Vec::new()`.
- `src/ui.rs:3874-3886` — `note()` with an empty `screen.note` substitutes
  `WAITING`; `src/ui.rs:161` — `const WAITING: &str = "reading the cluster…";`
- `src/ui.rs:1641` — `View::Resources`'s `Pane::Loading` arm returns
  `Offer::Nothing { switch: stranded }`, so the browser's loading footer *does* gain
  `X switch cluster` on `Link::Lost`.

## 6. Whether the withheld keys are withheld

```
$ sed -n 9910,9915p src/main.rs
        KeyCode::Up | KeyCode::Char('k') => moved(console, cards, open, Step::Up),
        KeyCode::Down | KeyCode::Char('j') => moved(console, cards, open, Step::Down),
        KeyCode::Enter => entered(console, cards, open),
        KeyCode::Tab => {
```

`moved` (`:10202`) and `entered` (`:10322`) read `console.app.focus`, the sidebar rows
and the cards; neither reads `screen.link`, `views::Offer` or the pane's state.
`views::sidebar` rows are the static panes plus `console.kinds`, so the sidebar has
rows to step over with `kinds` empty.

## 7. Redraw while nothing arrives

```
$ grep -n "STALE\|interval\|sleep(" src/main.rs
(no output)
```

`src/k8s.rs:1738-1744` — *"Invariant 7 blocks when idle, so a screen that draws only
on events never redraws during exactly the silence this type describes … A redraw on
a timer while a bootstrap is outstanding is `ui.rs`'s to write, and until it exists
these two facts are only as fresh as whatever else caused the last draw."*

## 8. Not measured here, and the commands that would settle it

- **The drawn frames.** Nothing was run: `ssh ubuntu 'cd ~/k8rs-src && export
  PATH=$HOME/.cargo/bin:$PATH && cargo test --locked --all-targets -- --nocapture
  main::tests::a_first_launch_that_never_answered_and_a_pane_still_reading_draw_different_paragraphs
  ui::tests::every_state_draws_the_body_and_the_footer_its_own_mockup_gives_it'`.
- **`(Expired, None)` on the real binary.** A kubeconfig whose `exec` block prints an
  expired credential, or a kubeconfig whose bearer field holds an expired one, against a
  live kind cluster — driven on a pty
  at 80×24: read the header, the body's first paragraph and the footer.
- **A blackholed address.** `K8RS_CLUSTER=review`, then a kubeconfig `server:` on an
  address a firewall `DROP`s (not `REJECT`s), and the wall clock until the body
  changes — the claim is ~30 s of `reading the cluster…` first.
- **The browser's loading body under `Link::Lost`.** `j`/`⏎` into `RESOURCES` from
  the first-launch pane on the real binary, and read what the content pane says.
