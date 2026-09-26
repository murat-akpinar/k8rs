# 2026-09-20 — the four-behaviours box, measured

Operator review (step 6) of the uncommitted Phase 12 change to `src/ui.rs`,
`src/ui_tests.rs`, `src/views.rs`, `src/views_tests.rs`. Evidence only; the
rulings belong in `NOTES.md`.

No cluster was used. Everything below is the renderer over constructed input,
plus two reads of the vendored `kube-client-4.2.0` source.

## How it was run

Nothing builds on the dev machine ([D267](../NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)).

```
rsync -a --delete --exclude=/target --exclude='/mutants.out*' ~/GIT/k8rs/ ubuntu:k8rs-review/
ssh ubuntu 'cd ~/k8rs-review && cat ~/k8rs-review-probes/probe_ui.rs >> src/ui_tests.rs && touch src/*.rs'
ssh ubuntu 'cd ~/k8rs-review && export PATH=$HOME/.cargo/bin:$PATH \
  && export CARGO_TARGET_DIR=$HOME/k8rs-src/target \
  && cargo test --bin k8rs probe_ -- --nocapture --test-threads=1'
```

The probes were `#[test]` functions appended to the **host copy only** of
`src/ui_tests.rs` and `src/views_tests.rs`; no file in the repo was written.
`CARGO_TARGET_DIR` was the existing `~/k8rs-src/target` because `df` reported
15G free against a 15G target dir. **Teardown:** `~/k8rs-review`,
`~/k8rs-review-probes` and the temp backups removed; `~/k8rs-src/target/debug/k8rs`
deleted by name, because the last build in that target dir was uplifted from the
scratch tree and a binary no tree held must not sit at the path `just` runs
([D267](../NOTES.md#d267--nothing-builds-on-the-dev-machine-the-gate-the-sweep-and-the-binary-move-to-the-test-host-2026-09-17)).
Both `#![cfg_attr(not(test), expect(dead_code, …))]` attributes were verified
restored before the copy was deleted (`grep -c "PROBE: expectation removed"` → 0
in both files).

## 1. `j` at the tail: the two frames, and the next four lines

`App { tab: Tab::Logs, following: true }`, 40 stream lines, 80×24, then one
`App::scroll_by(1)`, then four more lines arrive.

```
P1 follow on : window=[29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39] scroll=29 following=true
P1 after `j` : window=[29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39] scroll=29 following=false
P1 the frame is byte-identical before and after `j`: true
P1 +4 lines  : window=[29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39] scroll=29 following=false
```

At the same input the pane's own header and footer are identical in both follow
states:

```
P8 following=true  header : "│   network          │  container: app                      previous log: off  │"
P8 following=true  footer : "│ [ ] tabs  f follow  esc back  ? all keys  q quit                             │"
P8 following=false header : "│   network          │  container: app                      previous log: off  │"
P8 following=false footer : "│ [ ] tabs  f follow  esc back  ? all keys  q quit                             │"
```

Key map (`screens/help.md:10`): the only scroll keys are `↑ ↓ / j k`. There is
no `g`, no `G`, no page key. `src/views.rs` has exactly one writer of
`App::following` — `scroll_by`, `views.rs:3187` — and `App::scroll` is written
in exactly two places in the whole product: `views.rs:3188` (`scroll_by`) and
`ui.rs:5096` (`scrolled`'s write-back). It is
reset by nothing: `grep -n "self.scroll" src/views.rs` returns two lines.

## 2. Three draws at two heights, and the control at one

Same 40 lines, `scroll: 25`, follow off. Three draws: 24 rows, 44 rows, 24 rows.

```
P2 24 rows   : window=[25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35] scroll=25
P2 44 rows   : window=[9, 10, … , 39] scroll=9
P2 24 again  : window=[9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19] scroll=9
P2 control   : [25, …, 35] then [25, …, 35] scroll=25
```

The control is the same `App` drawn twice at 24 rows: identical window,
`scroll` unchanged. Two draws at one geometry are idempotent; two draws at two
geometries are not.

## 3. The same offset against a buffer that evicted 30 lines

`LOG_LINES = 5_000` (`src/k8s.rs:5445`), eviction is `pop_front`
(`src/k8s.rs:5504`). Follow off at `scroll: 2000`; the same `App` drawn against
a buffer that has since taken 30 more lines.

```
P3 frozen at : window=[2000, 2001, … , 2010] scroll=2000 dropped=0
P3 30 evicted: window=[2030, 2031, 2032, 2033, 2034, 2035, 2036, 2037] scroll=2000 dropped=30
P3 banner    : Some("30 lines were dropped from the top to keep this pane bounded.")
```

The window is three rows shorter in the second frame because the dropped-lines
banner takes rows from the pinned block above the scrolling half.

## 4. The pending confirmation — what the box draws and what the footer says

`scaling()` with `verdict: None`, rendered at 80×24:

```
P4 |│   networ┌ Scale payments/web ──────────────────────────────────────┐         │
P4 |│         │  $ kubectl scale deployment/web --replicas=3 -n payments │         │
P4 |│         │                                                          │         │
P4 |│         │               [ ⏎ do it ]    [ esc cancel ]              │         │
P4 |│         └──────────────────────────────────────────────────────────┘         │
P4 |├────────────────────┴─────────────────────────────────────────────────────────┤
P4 |│ waiting for the cluster                                                      │
P4 |└──────────────────────────────────────────────────────────────────────────────┘
```

`App::footer` and `App::escape` over four dialog states:

```
P6 scale, check on the wire                   waiting=true  armed=false footer="waiting for the cluster" quit=""
P6                                            esc -> modal still open   ends the run: false
P6 drain-shaped: check out AND a name to type waiting=true  armed=false footer="waiting for the cluster" quit=""
P6                                            esc -> modal still open   ends the run: false
P6 scale, verdict landed                      waiting=false armed=true  footer="⏎ do it  esc cancel" quit=""
P6                                            esc -> modal closed   ends the run: false
P6 delete, verdict is Some from frame 1       waiting=false armed=false footer="type the name to enable  esc cancel" quit=""
P6                                            esc -> modal closed   ends the run: false
```

The quit slot is `""` in all four. In the two `waiting=true` rows the footer
names no key and `esc` is refused.

### What bounds the wait

`kube-client-4.2.0/src/config/mod.rs`:

```
418:const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
190,272,338:            connect_timeout: Some(DEFAULT_CONNECT_TIMEOUT),
191,273,339:            read_timeout: None,
```

`read_timeout` is `None` in all three `Config` constructors. `ops::perform`
(`src/ops.rs:938`) is `call(DRY_RUN).await` with no `tokio::time::timeout`
around it; `src/main.rs` wraps every other cluster read it makes
(`3102, 3640, 3645, 4373, 4498, 4916, 4947, 5222`).

## 5. `command_cut` over the lines the product composes, at their drawn widths

Widths: the command log strip is 76 columns at the floor (`ui.rs:1861`); a
`Confirm`'s `$` line is `room(box) - 2` — **54** in the 58-column box and **57**
in the 61-column box (`ui.rs:187, 192, 1991, 207, 2233`).

```
P10  53 cols: kubectl scale deployment/web --replicas=3 -n payments
P10   @76 -> kubectl scale deployment/web --replicas=3 -n payments
P10   @57 -> kubectl scale deployment/web --replicas=3 -n payments
P10   @54 -> kubectl scale deployment/web --replicas=3 -n payments
P10  76 cols: kubectl scale deployment/checkout-worker --replicas=3 -n payments-production
P10   @76 -> kubectl scale deployment/checkout-worker --replicas=3 -n payments-production
P10   @57 -> kubectl scale deployment/checkout-worker --replicas=3…
P10   @54 -> kubectl scale deployment/checkout-worker --replicas=3…
P10  73 cols: kubectl rollout restart deployment/checkout-worker -n payments-production
P10   @57 -> kubectl rollout restart deployment/checkout-worker -n pa…
P10   @54 -> kubectl rollout restart deployment/checkout-worker…
P10  73 cols: kubectl delete pod/checkout-worker-7d9f4b6c8-x2k9w -n payments-production
P10   @57 -> kubectl delete pod/checkout-worker-7d9f4b6c8-x2k9w -n pa…
P10   @54 -> kubectl delete pod/checkout-worker-7d9f4b6c8-x2k9w…
P10  94 cols: kubectl --context staging scale deployment/checkout-worker --replicas=3 -n payments-production
P10   @76 -> kubectl --context staging scale deployment/checkout-worker --replicas=3…
P10   @57 -> kubectl --context staging scale deployment/…eckout-worker
P10   @54 -> kubectl --context staging scale deployment/…out-worker
P10  94 cols: kubectl scale deployment/checkout-worker --replicas=3 -n payments-production --context staging
P10   @76 -> kubectl scale deployment/checkout-worker --replicas=3 -n payments-productio…
P10   @57 -> kubectl scale deployment/checkout-worker --replicas=3…
P10   @54 -> kubectl scale deployment/checkout-worker --replicas=3…
```

Which width each operation actually gets. `box_width` (`ui.rs:2010`) reads
`dialog.asks` and the wrapped length of `dialog.consequence`, and nothing about
`dialog.kubectl`. The three real dialogs, each handed the long line above and
rendered at 80×24:

```
P11 scale   consequence wraps to 2 lines at 56 cols -> box 58
P11         $ kubectl scale deployment/checkout-worker --replicas=3…
P11 restart consequence wraps to 5 lines at 56 cols -> box 61
P11         $ kubectl rollout restart deployment/checkout-worker -n pa…
P11 delete  consequence wraps to 2 lines at 56 cols -> box 61
P11         $ kubectl delete pod/checkout-worker-7d9f4b6c8-x2k9w -n pa…
```

`delete` is at 61 for its typed-name field, not for its consequence.

Rendered, the scale box for an ordinary long name:

```
P9 |│   networ┌ Scale payments-production/checkout-worker ───────────────┐         │
P9 |│         │  $ kubectl scale deployment/checkout-worker --replicas=3…│         │
P9 |│         │               [ ⏎ do it ]    [ esc cancel ]              │         │
P9 |│ $ kubectl scale deployment/web --replicas=3 -n payments                      │
```

## 6. The two `dead_code` expectations

Measured by removing the `#![cfg_attr(not(test), expect(dead_code, …))]` block
in a host copy and building.

```
$ cargo build --bin k8rs                       # views.rs's removed, ui.rs's in place
warning: `k8rs` (bin "k8rs") generated 27 warnings

$ cargo build --bin k8rs                       # both removed
warning: `k8rs` (bin "k8rs") generated 244 warnings
```

Five of the 27 named in the first run — `may_mutate`, `escape`, `pick_pods`,
`open`, `scroll_by` (`views.rs:2618, 3056, 3128, 3153, 3186` in the unpatched
file; the build's own numbers are shifted by the removed attribute block).

## 7. Where a `Scrollbar` is drawn

```
$ grep -n "scrollbar(" src/ui.rs | grep -v "///"
2558:    scrollbar(frame, list, shown.len(), first, screen);
2743:    scrollbar(frame, list, rows.len(), first, screen);
2752: fn scrollbar(frame: &mut Frame, area: Rect, content: usize, first: usize, screen: &Screen) {
4017:    scrollbar(frame, margin, above.len(), at, screen);
4908:    scrollbar(frame, margin, tall, state.offset(), screen);
```

`scrolled`'s five call sites — `leads` (4013), `stream` (5285), `describe`
(5367), `rows_into` (5501), `yaml` (5617) — pass their return value to a
`scrollbar` in exactly one of the five, `leads`, which draws the *pinned block*
of a Loading or Empty tab. The four tab bodies draw none.
