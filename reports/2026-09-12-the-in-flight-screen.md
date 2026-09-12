# The in-flight screen — operator review measurements (2026-09-12)

Step 6 over the uncommitted `dev-ui` box that turns `views::App::changing` into
`Option<views::Object>`, appends `changing…` to the header zone and gives the footer
its in-flight line. **No cluster was brought up** — nothing in this box calls an API, so
every frame below is `ratatui`'s `TestBackend` over the committed fixture, rendered by the
box's own tests.

Build volume: `CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target` (CLAUDE.md § the one hard rule
of concurrency — `$HOME`, not the 12 GiB tmpfs scratchpad, which was at 64% during this run).

## 1. The tree is green before anything below is read

```
$ CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --quiet
test result: ok. 1325 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.85s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.52s
```

## 2. The three frames the box draws

```
$ CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --quiet \
    every_footer_on_the_screen_it_belongs_to -- --nocapture
```

Alerts, a change in flight (header row and footer row of the frame, verbatim):

```
 nodes 3/3                               ctx: prod-eu · live · admin · changing…
│ ↑↓ move  ⏎ open  ?  ·  finishing the change to payments/web first            │
```

Alerts, a long name in flight:

```
│ ↑↓ move  ⏎ open  ?  ·  finishing the change to payments/checkout-work… first │
```

Help over the same call — all sixteen body rows, then the footer:

```
┌ Keys ────────────────────────────────────────────────────────────────────────┐
│  Moving around                                                               │
│    ↑ ↓ / j k    move            ⏎     open the selected thing                │
│    tab          next panel      esc   back / close                           │
│    X            switch cluster                                               │
│    [ ]          detail tabs     / n   filter · namespace                     │
│                                                                              │
│  Looking at things (always available)                                        │
│    l  logs, with the log from before a crash                                 │
│       in the log tab:  f follow · c container · ⇧p previous                  │
│    d  describe — the object and what happened to it                          │
│    y  view as YAML                                                           │
│                                                                              │
│  Changing things (each one asks first, and shows the command)                │
│    s       run more or fewer copies       (scale)                            │
│    r       restart, at its own pace       (rollout restart)                  │
│    ctrl-d  delete — you type the name to confirm                             │
├──────────────────────────────────────────────────────────────────────────────┤
│ ? or esc to close                                                            │
└──────────────────────────────────────────────────────────────────────────────┘
```

The healthy frame beside it, same run, `changing: None`:

```
 nodes 3/3                            k8rs           ctx: prod-eu · live · admin
│ ↑↓ move  ⏎ open  s no scale  r no restart  / filter  ? all keys  q quit      │
```

## 3. Column budget of the in-flight footer

```
$ CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --quiet \
    the_in_flight_footer_cuts -- --nocapture
↑↓ move  ⏎ open  ?  ·  finishing the change to payments/checkout-work… first
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1324 filtered out
```

Field values the findings turn on, read off that line:

| | columns |
|---|---|
| the row `ui::indented` leaves at the 80×24 floor | 76 |
| fixed prefix `↑↓ move  ⏎ open  ?  ·  finishing the change to ` | 47 |
| fixed suffix ` first` | 6 |
| left for the object | 23 |
| of which the visible cut mark `…` takes | 1 |
| characters of `namespace/name` kept | 22 |

`ui::footer` derives the 23 by rendering the same line with an empty name and subtracting
its width from `row.width`; no number in `src/ui.rs` restates 47 or 6.

`payments/checkout-worker-service-account-token-projector` → `payments/checkout-work…`:
the 22 kept characters are `payments/checkout-work`.

## 4. The header zone cut, with the mark appended

```
$ CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --quiet \
    a_change_in_flight_survives -- --nocapture
…roduction-eu · ns: payments · live · read-only · ⚠ TLS not verified · changing…
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1324 filtered out
```

80 columns, the whole header row. Two `…` on one row: the leading one is `ui::shortened`'s
truncation mark, the trailing one is the last two bytes of `theme::CHANGING`.

## 5. The predicates, and who calls them

```
$ grep -rn "may_mutate\|may_quit\|may_switch_cluster\|may_delete" src/*.rs | grep -v _tests.rs
src/views.rs:1610:    pub fn may_quit(&self) -> bool {
src/views.rs:1617:    pub fn may_switch_cluster(&self) -> bool {
src/views.rs:1623:    pub fn may_mutate(&self) -> bool {
```

Three definitions, zero call sites. There is no `may_delete`.

## 6. What appends a line to the command log while the call is on the wire

```
$ grep -n "kubectl logs\|kubectl describe\|kubectl get .*-o yaml" src/views.rs
src/views.rs:1269:        "$ kubectl describe pod {}{}",
src/views.rs:1330:        "$ kubectl get {resource} {}{} -o yaml --show-managed-fields",
```

```
$ grep -n "const LOG_LINES" src/ui.rs
src/ui.rs:72:const LOG_LINES: u16 = 2;
```

`src/views.rs:1111-1120` (`Log::waiting`) records the same interleaving from the other side:
*"a reader who opens a detail tab meanwhile appends a read line **after** the one still
waiting."*

## 7. The kinds a dialog can open on

```
$ grep -n "const KINDS" src/main.rs
src/main.rs:5722:const KINDS: [Kind; 6] = [
```

Six, per `screens/dialogs.md` § Delete: `deployment`, `statefulset`, `daemonset`,
`replicaset`, `pod`, `node` — one of which is cluster-scoped.

---

# Second round — the same tree after the three findings were fixed (2026-09-12)

Still uncommitted, still no cluster. The strings measured above are the **old** ones; this
section is what the same `render` helpers draw after `dev-ui`'s fixes.

## 8. The tree is still green

```
$ CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --quiet
test result: ok. 1330 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.03s
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.40s
```

1325 → 1330 unit tests.

## 9. The five in-flight frames now drawn

```
$ CARGO_TARGET_DIR=$HOME/.cache/k8rs-review-target cargo test --quiet \
    every_footer_on_the_screen_it_belongs_to -- --nocapture
```

Footer rows, verbatim, at the 80×24 floor:

```
│ ↑↓ move  ⏎ open  ? keys  ·  changing payments/web first                      │
│ ↑↓ move  ⏎ open  ? keys  ·  changing payments/checkout-worker-service… first │
│ ↑↓ move  ⏎ open  ? keys  ·  changing team-alpha-payments-platform-an/… first │
│ [ ] tabs  f follow  c container  esc back  ? all keys                        │
│ ? or esc to close                                                            │
```

Help's body, the two rewritten rows of the same sixteen:

```
│    X            switch cluster (paused until the change below finishes)      │
│  Changing things (paused until the change below finishes)                    │
│    s       run more or fewer copies       (scale)                            │
│    r       restart, at its own pace       (rollout restart)                  │
│    ctrl-d  delete — you type the name to confirm                             │
```

The command log strip under that Help frame, in the same render:

```
│ $ kubectl get statefulsets -A --watch                                        │
│ $ kubectl get daemonsets -A --watch                                          │
```

## 10. The new column budget

| | columns |
|---|---|
| the row `ui::indented` leaves at the floor | 76 |
| fixed prefix `↑↓ move  ⏎ open  ? keys  ·  changing ` | 37 |
| fixed suffix ` first` | 6 |
| left for the object (`room`) | **33** (was 23) |

Read off the drawn lines: `payments/checkout-worker-service` is 32 characters + `…` = 33;
`team-alpha-payments-platform-an` is 31 characters + `/…` = 33.

## 11. `name_cut`'s case 3, as a rule

`src/ui.rs`, `fn name_cut`: when a plain `clipped` loses the `/`, the result is
`fits(namespace, columns - 2)` + the literal `/…`. At `room = 33` that is the first **31**
characters of the namespace and **none** of the object's own name.

Namespaces at or above 32 characters therefore draw identically for every object in them.
Real shipped examples, character counts taken with `printf %s … | wc -m`:

```
$ printf 'openshift-cluster-node-tuning-operator' | wc -m
38
$ printf 'openshift-controller-manager-operator' | wc -m
37
```

## 12. Three stale claims the fix left behind

```
$ grep -n "never gives way is the pair\|are what$" screens/widgets.md | head -3
299:**`? all keys` and `q quit` are one closed pair, drawn last, and they are what
300:never gives way.** ...
```

```
$ grep -n "What never gives way is the pair" src/views.rs
1639:    /// and its footer names neither. **What never gives way is the pair `? all keys  q quit`**,
```

`src/views.rs:1799-1803` is the `strip_suffix("  q quit")` those two sentences describe as
impossible.

```
$ git diff --stat screens/detail.md
 screens/detail.md | 5 +++++
```

`screens/dialogs.md` § *Detail tabs and Analysis keep their own footer* says of that edit:
*"that edit is not this round's, `detail.md` being outside the three files this pass may
touch"* — while `screens/detail.md:38-42` carries it in the same working tree.

## 13. The permission clauses under a call in flight

`src/ui.rs`, `fn key_map`: `if changing { … } else { … }` — while `changing` is set the three
`Refused` clauses are not built at all. Asserted for all eight combinations by
`ui_tests::a_call_in_flight_draws_no_permission_clause_on_any_row`.
