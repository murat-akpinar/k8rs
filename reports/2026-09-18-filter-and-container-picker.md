# 2026-09-18 — the filter row, the zero-match pane and the container picker, drawn

`k8s-admin`, operator review of the Phase 12 box *A footer key has something
behind it*. No cluster: every frame below is `src/ui_tests.rs`'s own renderer at
the 80×24 floor over committed captures. Probes were appended to a **copy** of
the tree on the test host, run, and the copy restored by re-`rsync`; nothing in
this repo was edited.

## How every frame below was produced

```
rsync -a --delete --exclude=/target --exclude='/mutants.out*' \
      /home/shyuuhei/GIT/k8rs/ ubuntu:k8rs-src/
ssh ubuntu 'cat /tmp/probe.rs >> ~/k8rs-src/src/ui_tests.rs && cd ~/k8rs-src \
      && export PATH=$HOME/.cargo/bin:$PATH \
      && cargo test --locked probe_ -- --nocapture --test-threads=1'
```

Each probe builds a `Detail`/`Screen` exactly as the file's own tests do, renders
at `MIN_WIDTH`×`MIN_HEIGHT`, and prints every row between `|`…`|`.

## 1 — the container picker with a 47-column state word

Probe: `Picked::of("gang")` (two containers, both `running`, `restartCount: 3`)
with the second container's `state` taken from `Picked::of("config")` — the
committed rule-4 capture, whose container waits in `CreateContainerConfigError`.
Neither capture edited; the two combined in the test, the way
`the_picker_counts_restarts_and_names_the_one_worth_pressing_previous_on`
already combines `gang` with a changed `restarts`.

```
| nodes 3/3                            k8rs           ctx: prod-eu · live · admin|
|┌────────────────────┬─────────────────────────────────────────────────────────┐|
|│▸ ALERTS         1 ●│  payments/web-7d9f4                                     │|
|│  RESOURCES         │  ‹ logs ›   describe   yaml   events                    │|
|│   workloads        │  ──────                                                 │|
|│   netwo┌ payments/web-7d9f4 — pick a container ──────────────────────┐: off  │|
|│   stora│                                                             │       │|
|│   confi│  ▸     running                                           3 r│       │|
|│   clust│        needs a ConfigMap or Secret that does not exist   3 r│       │|
|│  ANALYS│                                                             │       │|
|│        │  trigger restarted 3 times. ⇧p on it shows the log from just│       │|
|│        │  before its last crash.                                     │       │|
|│        │                                                             │       │|
|│        │                [ ⏎ pick ]    [ esc cancel ]                 │       │|
|│        │                                                             │       │|
|│        └─────────────────────────────────────────────────────────────┘       │|
|│                    │                                                         │|
|│                    │                                                         │|
|├────────────────────┴─────────────────────────────────────────────────────────┤|
|│                    ...                                                       │|
|├──────────────────────────────────────────────────────────────────────────────┤|
|│ ↑↓ move  ⏎ pick  esc cancel                                                  │|
|└──────────────────────────────────────────────────────────────────────────────┘|
```

Probe output for the two container names against the frame:

```
name "trigger"   on its row: Some("│        │  trigger restarted 3 times. ⇧p on it shows the log from just│       │")
name "bystander" on its row: None
```

Field values the arithmetic turns on, read off `src/ui.rs`:

| term | value |
|---|---|
| `room(CROWDED_BOX)` | `61 - 2` = 59 |
| `width(MARKER)` | 2 |
| `NAMES_GAP` | 3 |
| `word` (max state word) | 47 — `needs a ConfigMap or Secret that does not exist` |
| `counted` (max restart count) | 10 — `3 restarts` |
| `slot = 59 - (2 + 3 + 47 + 3 + 10)` | `-6` → `.max(1)` = **1** |
| `front(name, 1, "…")` (`src/ui.rs:4585`) | `""` — the loop breaks before its first assignment |
| row width `2 + 2 + (1+3) + (47+3) + 10` | **68**, into a 61-column `inner` |

`boxed` renders `Paragraph::new(Text::from(lines))` with no `Wrap`
(`src/ui.rs:1543`), so the 7 columns past 61 are dropped with no mark.

## 2 — the tallest box the picker can build (12 containers, no restarts)

```
|│▸ ALERTS         1 ●│  payments/web-7d9f4                                     │|
|│  RESOUR┌ payments/web-7d9f4 — pick a container ──────────────────────┐       │|
|│   workl│                                                             │       │|
|│   netwo│  ▸ app                                            running  █│: off  │|
|│   stora│    proxy                                          running  █│       │|
|│   confi│    sidecar-0                                      running  █│       │|
|│   clust│    sidecar-1                                      running  █│       │|
|│  ANALYS│    sidecar-2                                      running  █│       │|
|│        │    sidecar-3                                      running  █│       │|
|│        │    sidecar-4                                      running  █│       │|
|│        │    sidecar-5                                      running  ║│       │|
|│        │    sidecar-6                                      running  ║│       │|
|│        │                                                             │       │|
|│        │                [ ⏎ pick ]    [ esc cancel ]                 │       │|
|│        │                                                             │       │|
|│        └─────────────────────────────────────────────────────────────┘       │|
```

Nine list rows, buttons drawn, box 15 rows inside a body that holds it.

## 3 — the scrollbar and the restart column, 7 containers with restarts

`Picked::of("init")` plus five copies of its first container at
`restarts: 4`; `migrate` carries the capture's own `restartCount: 10`.

```
|│   netwo│  ▸ app                             not started             █│: off  │|
|│   stora│    migrate                         failed        10 restart█│       │|
|│   confi│    sidecar-0                       failed        4 restarts█│       │|
|│   clust│    sidecar-1                       failed        4 restarts█│       │|
|│  ANALYS│    sidecar-2                       failed        4 restarts█│       │|
|│        │    sidecar-3                       failed        4 restarts║│       │|
|│        │                                                             │       │|
|│        │  migrate restarted 10 times. ⇧p on it shows the log from    │       │|
|│        │  just before its last crash.                                │       │|
```

`views::restart_count` returns `10 restarts`; the row draws `10 restart`.
`scrollbar(frame, list, …)` is handed `Rect { .. inner }` (`src/ui.rs:2337`), so
its column is `inner`'s last, which the widest row already occupies —
`the_picker_counts_restarts_and_names_the_one_worth_pressing_previous_on`
asserts `width(row) == CROWDED_BOX` for exactly that row.

## 4 — the at-rest filter row beside the namespace scope

`table-pods`, `screen.namespace = Some("kube-system")`, `/ kube` and
`n kube-sys` typed through `Input::push`:

```
|│▸ ALERTS            │  pods  ns: kube-system                                  │|
|│  RESOURCES         │  filter: "…be"   namespace: "…ys"   esc clears filter   │|
|│   workloads        │                                                         │|
|│   network          │  NAME                   READY  STATUS   RESTARTS     AGE│|
|│   storage          │▸ kube-apiserver-k8rs-c  1/1    Running  0            34h│|
|│   config           │  kube-controller-manag  1/1    Running  0            34h│|
```

Arithmetic, off `narrowed()` (`src/ui.rs:2858`) at the floor:

| term | value |
|---|---|
| `row.width` (`padded` of the 57-column pane) | 53 |
| `width("esc clears filter") + FILTER_GAP.len()` | 17 + 3 |
| `room` | 33 |
| `fixed` = `width("filter: \"\"") + width("namespace: \"\"") + gap` | 10 + 13 + 3 = 26 |
| `each = (33 - 26) / 2` | **3** |

The screen file's own mockup value fits by one column:

```
|│▸ ALERTS         1 ●│  filter: "web"   namespace: "pay"   esc clears filter   │|
|│  RESOURCES         │  ● payments/web  ·  3 of 5 pods          1014 days ago  │|
```

`filter: "web"` (13) + 3 + `namespace: "pay"` (16) + 3 + `esc clears filter`
(17) = 52, against `row.width` 53.

## 5 — the zero-match pane, both panes, at the floor

Alerts, one `oom()` card, `/ prodeu` and `n pay`:

```
|│  ANALYSIS          │                                                         │|
|│                    │  No problems match "prodeu" in a namespace like "pay".  │|
|├────────────────────┴─────────────────────────────────────────────────────────┤|
|│                    ...                                                       │|
|├──────────────────────────────────────────────────────────────────────────────┤|
|│ / filter  esc clear filter  ? all keys  q quit                               │|
```

Browser, `deployments`, scope `payments`, `/ web` and `n pay`:

```
|│▸ ALERTS            │  deployments  ns: payments                              │|
|│                    │  No deployments match "web" in a namespace like "pay".  │|
|├──────────────────────────────────────────────────────────────────────────────┤|
|│ / filter  esc clear filter  ? all keys  q quit                               │|
```

For comparison, the footer of the *other* empty Alerts pane, from
`screens/states.md` § Nothing is broken and `ui::offered`'s
`View::Alerts` arm:

```
↑↓ move  ⏎ open  / filter  ? all keys  q quit
```

## 6 — the typing footer

`filters.namespace = "pay"`, `typing = Some(Typing::Namespace)`:

```
|│▸ ALERTS         1 ●│  ● payments/web  ·  3 of 5 pods          1014 days ago  │|
|│  RESOURCES         │    Containers exceeded their memory limit and were      │|
|│   workloads        │    killed by the kernel (OOMKilled)                     │|
|│   network          │    limit 256Mi · exit 137 · 47 restarts                 │|
|│   storage          │    → raise limits.memory, or find the leak              │|
|├──────────────────────────────────────────────────────────────────────────────┤|
|│ namespace: pay  ⏎ done  esc clear namespace                                  │|
```

## 7 — what `/` matches, per pane

Browser, `/ worker3`, against `table-pods` whose `Node` column is
`priority: 1` and is dropped by `grid`:

```
|│▸ ALERTS            │  pods                                                   │|
|│  RESOURCES         │  filter: "worker3"   esc clears it                      │|
|│   workloads        │                                                         │|
|│   network          │  NAME                   READY  STATUS   RESTARTS     AGE│|
|│   storage          │▸ kube-system/kindnet-s  1/1    Running  3 (34h ago)  34h│|
|│   config           │  kube-system/kube-prox  1/1    Running  3 (34h ago)  34h│|
```

Alerts, the same `oom()` card, four strings typed into `/`, each rendered and
the card looked for:

```
typed "3 of 5 pods"  -> card kept: false
typed "days ago"     -> card kept: false
typed "payments/web" -> card kept: true
typed "OOMKilled"    -> card kept: true
```

The card's identity line as drawn is
`● payments/web  ·  3 of 5 pods          1014 days ago`.

## Cleanup

```
rsync -a --delete --exclude=/target --exclude='/mutants.out*' \
      /home/shyuuhei/GIT/k8rs/ ubuntu:k8rs-src/
ssh ubuntu 'rm -f /tmp/probe*.rs; cd ~/k8rs-src && grep -c "k8s-admin review probe" src/ui_tests.rs'
0
```

`git status --short` in this repo, after: the same twelve modified files it held
before, no thirteenth.

---

# Round two — the same frames after the fixes

Same method, same host, the tree as it stands after the triage build.

## 1r — the container picker, both shapes

`gang` + `config.json`'s `CreateContainerConfigError` state, unchanged probe:

```
|│   confi│  ▸ trigger                running                 3 restarts│       │|
|│   clust│    bystander              needs a ConfigMap or…   3 restarts│       │|
|│        │  trigger restarted 3 times. ⇧p shows the log from before    │       │|
|│        │  that restart, if the kubelet still has it.                 │       │|
|│        │                [ ⏎ pick ]    [ esc cancel ]                 │       │|
```

Both names whole, both counts whole, the state back-cut at a word boundary
behind one `…`.

A 512-byte `Waiting` reason, one token with no whitespace:

```
|│   confi│  ▸ trigger                running                 3 restarts│       │|
|│   clust│    bystander              WWWWWWWWWWWWWWWWWWWW…   3 restarts│       │|
```

`wrapped` hard-breaks a word longer than its column, so `marked` gets more than
one line and the cut is marked rather than falling through `slotted`'s unmarked
`fits`.

Arithmetic after the change, both frames: `around` = `2 + 3 + 3 + counted`,
`avail` = `columns - around`, `slot` = `max(avail - widest, NAME_FLOOR.min(avail))`,
`word` = `avail - slot`. With `counted` at its ceiling — `restart_count` of
`i32::MAX` is `2147483647 restarts`, 19 — `avail` is 32, so `word` is never 0
and `cut(state, 0, 1)`'s unmarked path is unreachable.

## 3r — the scrollbar's own column

Seven containers, restarts on all:

```
|│   netwo│  ▸ app                            not started              █│: off  │|
|│   stora│    migrate                        failed        10 restarts█│       │|
|│   confi│    sidecar-0                      failed        4 restarts █│       │|
```

`10 restarts` whole. The hint below the list is outside the scrollbar's `Rect`
(`y: inner.y + 1, height: listed`) and is unaffected.

## 4r — the at-rest row

`table-pods`, scope `kube-system`, `/ kube` + `n kube-sys`:

```
|│▸ ALERTS            │  pods  ns: kube-system                                  │|
|│  RESOURCES         │  filter: "kube"   namespace like: "kube-sys"            │|
```

`/ kube` alone:

```
|│  RESOURCES         │  filter: "kube"   esc clears it                         │|
```

New thresholds at the floor (`row.width` 53): one field set, hint drawn —
`53 - (13 + 3) - 10` = **27** columns of value; both set, hint dropped —
`53 - (10 + 18 + 3)` = **22** columns shared, 11 each once over.

## 5r — the zero-match footers

Alerts, `/ prodeu`, `Link::Live`:

```
|│                    │               No problems match "prodeu".               │|
|│ ↑↓ move  ⏎ open  / filter  esc clear filter  ? all keys  q quit              │|
```

The same `App`, `screen.link = Link::Expired`, everything else held:

```
|│                    │               No problems match "prodeu".               │|
|│ ↑↓ move  ⏎ open  / filter  esc clear filter  ? all keys  q quit              │|
```

Byte-identical. `screen.context` in the test helper is the literal
`ctx: prod-eu · live`; in the product that string is the caller's and carries
the connection word — `Screen::context`'s own doc: *"The header's right zone up
to the connection state, already joined"*, and `screens/states.md` § Your login
expired draws it as `ctx: prod-eu · ⚠ login expired · admin`.

Footer widths, counted:

| line | columns |
|---|---|
| `↑↓ move  ⏎ open  / filter  esc clear filter  ? all keys  q quit` | 63 |
| the same with `X switch cluster` in front | 81 |
| the same with `esc clear namespace` | 84 |
| `ui::indented` at the 80-column floor | 76 |

What else names the fact and the key on that screen, per
`screens/states.md` § Your login expired: the header segment
`⚠ login expired`, the pane banner *"Renew it, then press X and pick this
cluster again"*, and the command-log strip's `→ login expired`.
`screens/help.md`'s key map row is `esc   back / close` — it names no filter.

## Cleanup, round two

```
rsync -a --delete --exclude=/target --exclude='/mutants.out*' \
      /home/shyuuhei/GIT/k8rs/ ubuntu:k8rs-src/
ssh ubuntu 'rm -f /tmp/round2.rs /tmp/probe*.rs; cd ~/k8rs-src && grep -c "review probe" src/ui_tests.rs'
```
