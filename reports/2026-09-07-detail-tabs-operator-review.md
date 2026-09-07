# The four detail tabs, measured — the Phase 11 operator review

`k8s-admin`, 2026-09-07. Review of the uncommitted detail-tabs box: `ui.rs`
`// --- THE DETAIL TABS ---`, `views.rs`
`// --- THE WORDING A DETAIL TAB DRAWS ---`, `main.rs`'s headless printer, and
`screens/detail.md` §§ *The describe tab* / *The events tab*, read together with
the frozen `k8s.rs` events fetch they all reach.

**No cluster was brought up.** The PM's `k8rs` fixture cluster is running, so a
second one beside it is refused (NOTES § D84, § D92); every measurement below is
the landed code run in a throwaway copy of the tree. One claim genuinely needs a
cluster and is marked *unverified* with the command that settles it.

## How it was run

The working tree is the PM's, so the probes were appended to a copy with its own
target directory, and both were removed afterwards.

```
$ mkdir -p /tmp/k8rs-review && tar -C /home/shyuuhei/GIT/k8rs -cf - \
    --exclude=./target --exclude=./.git . | tar -C /tmp/k8rs-review -xf -
$ du -sh /tmp/k8rs-review
20M     /tmp/k8rs-review
$ cd /tmp/k8rs-review && CARGO_TARGET_DIR=/home/shyuuhei/.cache/k8rs-review-target \
    cargo test --quiet scratch_ -- --nocapture
```

Probes live only in the copy's `src/ui_tests.rs`. Nothing under `src/`,
`tests/` or `scripts/` in the real tree was touched, and no git command was run
that can discard work. Teardown:

```
$ cargo clean --manifest-path /tmp/k8rs-review/Cargo.toml \
    --target-dir /home/shyuuhei/.cache/k8rs-review-target
     Removed 4422 files, 3.9GiB total
$ find /tmp/k8rs-review -mindepth 1 -delete; rmdir /tmp/k8rs-review
$ kind get clusters
k8rs
```

A first attempt put the scratch target under `/tmp` and filled the 12 GiB tmpfs
the harness shares — NOTES § D133's box, from the other end:

```
$ df -h /tmp | tail -2
Filesystem      Size  Used Avail Use% Mounted on
tmpfs            12G   11G  981M  92% /tmp
```

## 1. `views::events_heading` on the describe tab, cut read, 80×24

Input: the committed `oom` capture, `Happened { lines: <the two measured
events>, cut: true }`, `Tab::Describe`.

```
$ cargo test --quiet scratch_describe_cut_heading -- --nocapture
```

```
│   workloads        │         ──────────                                      │
│   network          │  Pod · running · created 4 days ago                     │
│   storage          │                                                         │
│   config           │  containers                                             │
│   cluster          │    hog   failed                                         │
│  ANALYSIS          │      container exceeded its memory limit — exit 137,    │
│                    │      10 restarts                                        │
│                    │                                                         │
│                    │  events (the first 500 k8rs was given — there are more  │
│                    │  3 min ago    the health check failed                   │
```

```
heading string  = "events (the first 500 k8rs was given — there are more, and these are not the newest)"
holds("not the newest") on describe = false
holds("there are")      on describe = true
```

The heading is 84 columns; the pane is 55. `ui.rs:1931` builds it as a single
`Line::styled(...)` and `Paragraph` has no `Wrap`, so it is hard-cut at the pane
edge with no `…`. Every other free-text block in the same function goes through
`set` / `wrapped` (`ui.rs:1942`, `1950`).

`screens/detail.md:1004` states what the heading is for: *"The heading carries
the withdrawal, because the heading is the only place the claim was made."*

## 2. The same withdrawal on the events tab, under scroll

Input: the two measured events plus twelve `BackOff` rows so the pane overflows,
`cut: true`, `Tab::Events`, at four scroll offsets.

```
$ cargo test --quiet scratch_the_cut_withdrawal_scrolls_away -- --nocapture
```

`scroll: 0` — the heading wraps correctly here (`rows_into` uses `set`):

```
│                    │  events (the first 500 k8rs was given — there are more, │
│                    │  and these are not the newest):                         │
```

`scroll: 1`:

```
│   network          │  more, and these are not the newest):                   │
│   storage          │                                                         │
│   config           │  3 min ago    the health check failed                   │
holds("there are") = false   holds("not the newest") = true
```

`scroll: 3`:

```
│   network          │  3 min ago    the health check failed                   │
holds("there are") = false   holds("not the newest") = false
```

`scroll: 6`:

```
holds("there are") = false   holds("not the newest") = false
```

The heading is inside the scrolling `Paragraph` (`ui.rs:2002–2011`). The logs
pane's equivalent sentence is outside it — `ui.rs:1769` states the rule: *"a
sentence that scrolled away with the content would be pointing at nothing."*
`screens/detail.md:1052` specifies the scrolling behaviour the code implements.

## 3. A container waiting on a reason no table names

Input: the `oom` capture with `containers[0].state` replaced by what a real
kubelet writes for an unparseable image reference — `InvalidImageName`, one of
rule 3's seven and not in `views::WAITING_REASONS`.

```
$ cargo test --quiet scratch_a_waiting_reason_no_table_names -- --nocapture
```

```
│   cluster          │    hog   InvalidImageName, 10 restarts                  │
```

```
views::container_state = ("InvalidImageName", None)
the kubelet's message reaches the screen: false
```

`views.rs:1099` destructures `ContainerState::Waiting { reason, .. }`. The
`message` arm of that variant exists and is bounded at ingest
(`k8s.rs:386–388`, `FREE_TEXT`); it is discarded here.

Two functions away in the same region, the events fall-through keeps it —
measured on the same run:

```
│                    │  (BackOff) Back-off restarting failed container app     │
```

`screens/detail.md:465` says the container fall-through is *"the same
safe-fallback rule the events table below uses"*.

## 4. What the events pane costs per frame

Input: `EVENTS_KEPT` events, each carrying a `FREE_TEXT`-length message — the
widest object the type permits to reach this pane — then the same count at a
realistic message length. Ten frames each, `--release`, 80×24.

```
$ cargo test --release --quiet scratch_worst_case_events_frame_cost -- --nocapture
message bytes held for one object = 2047500
one 80x24 frame of the events tab  = 35.815093ms
500 ordinary-length events, one frame = 975.358µs
```

`ui::rows` wraps the whole list on every draw; the pane shows 14 rows.

## 5. `kubectl events --for` against the line the command log draws

`screens/detail.md:873` draws `kubectl events --for pod/web-7d9f4 -n payments`.
Nothing in `src/` builds this string yet — `grep -rn '"kubectl' src/` returns
`ops.rs`'s three mutation lines, `rules.rs`'s card lines and `main.rs`'s
`kubectl_get`, and no events line. The pane's `log` field is caller-supplied
(`ui.rs:215`), so this is Phase 12's wiring measured against Phase 11's spec.

The subcommand and the flag, on this machine:

```
$ kubectl version --client
Client Version: v1.36.3
$ kubectl events --help
    --for='':
	Filter events to only those pertaining to the specified resource.
Usage:
  kubectl events [(-o|--output=)json|yaml|...] [--for TYPE/NAME] [--watch] [--types=Normal,Warning] [options]
```

`-n` is a global flag and `--for TYPE/NAME` is the documented spelling, so the
line parses. What it *sends* and *prints*, read off `kubectl`'s own
`pkg/cmd/events/events.go`:

| | k8rs (`k8s.rs:6226–6253`) | `kubectl events --for` |
|---|---|---|
| selector | `involvedObject.kind`, `.name`, `.uid` | `involvedObject.kind`, `.apiVersion`, `.name` — **no uid** |
| bound | `limit(EVENTS_KEPT)` = 500 | `Limit: DefaultChunkSize` with `FollowContinue()` — pages the lot |
| order | sorted newest first, stable | `sort.Sort(SortableEvents(...))`, `eventTime(i).Before(eventTime(j))` — **oldest first** |
| type column | not carried (`Happening` has five fields) | `TYPE` printed |

**Unverified without a cluster:** that a StatefulSet pod recreated under the
same name shows its predecessor's events through the printed line and not
through the pane. What settles it: create `web-0`, let it emit events, delete
and recreate it under the same name inside the event TTL, then compare
`kubectl events --for pod/web-0 -n <ns>` against the tab.

**Unverified without a cluster:** that a cluster-scoped object's events land in
`default`, so `kubectl events --for node/<name>` with no `-n` returns nothing
from any other current namespace. What settles it:
`kubectl config set-context --current --namespace=kube-system` then
`kubectl events --for node/<name>` against `kubectl events --for node/<name> -n default`.

## 6. The age column, code against mockup

`ui.rs:2039` pads the age to the widest age on the pane plus `GAP`. Measured:

```
│                    │  3 min ago    the health check failed                   │
│                    │  4 hours ago  the image is ready                        │
```

`screens/detail.md:860` and `:865` draw two spaces after both, so the phrases
start in different columns:

```
│   network          │  3 min ago  the health check failed           │
│   capacity      1 ▲│  4 hours ago  the image is ready              │
```

The same section's prose (`:895`) says the opposite of its own mockup: *"The age
column still pads to the widest age actually on the pane."* The committed test
`an_event_that_repeated_says_how_often_and_one_that_did_not_says_nothing_extra`
asserts the padded form. `screens/detail.md:570` and `:431` carry the same
mockup shape.

## 7. What was checked and found nothing

- **Invariant 9, every path into the four panes.** `Happening.reason` /
  `.message` (`k8s.rs:711`), `PodSnapshot.phase` / `.reason` (`:446`),
  `ContainerState::Waiting` and `Terminated` (`:376`, `:383`),
  `ContainerSnapshot.name` (`:403`) — all stripped at ingest, and
  `k8s::events` builds every `Happening` through `ingest` (`:6235`).
  `views.rs` re-strips nothing, which is what its region doc claims.
- **Bounded sizes.** Message cap `FREE_TEXT` = 4096, list cap `EVENTS_KEPT` =
  500; measured ceiling 2 047 500 bytes for one object (§ 4).
- **Invariant 1.** `git diff src/ui.rs src/views.rs src/main.rs` grepped for
  `.delete(` `.patch(` `.create(` `.replace(` `.evict(` `patch_scale`
  `delete_collection` on added lines — no match.
- **The phrase table against the two D198 measurements.** `Pulled` → *the image
  is ready* is true of the cached-image message as well as the pull;
  `Unhealthy` → *the health check failed* is true of both probe kinds and the
  message still says which; `Killing` → *the container is being stopped* is
  true of a graceful stop and of a probe-driven kill.
- **`views::container_state` against the Phase 6 measurement.** Exit 0 →
  `done`, exit 255 → `failed` / `exit 255`, and `main.rs:4464`'s picker now
  calls the same function, so the *(done)* / `failed` disagreement is gone.
- **`WAITING_REASONS` against the cards.** rule 1 `rules.rs:4338`, rule 3
  `:4480`, rule 4 `:4507` — no surface disagrees with another about one
  container.
- **`repeated`, `restarts`, `no_events`, `no_previous_run` on absent and
  negative inputs.** `count: None` → no line; negative `count` → no line;
  negative `restartCount` → no count text; `first: None` → the count without a
  span; a `first` more than `SKEW_ALLOWANCE` ahead → `age` returns `None` and
  the span is dropped.
- **`Pane::Loading` / `Denied` / `Ready` on all four tabs.** Each tab writes
  the three-way match out rather than sharing it (`ui.rs:1753`); measured that
  a refused events read draws its banner and never `○ none right now`
  (committed test `a_refused_events_read_draws_the_sentence_and_not_the_empty_state`).
- **The tab row and its underline** at all four positions, against
  `screens/detail.md`'s own columns — describe 10 at 9, yaml 6 at 20, events 8
  at 27, all read off the labels rather than written as numbers.

## 8. Two doc lines that do not match the code they sit on

`views.rs:1260` (`no_previous_run`) says a negative restart count *"is `!= 0`
here exactly as it is in the display sites, so a value no API server produces
cannot make two screens say no restarts while this one says it has restarted."*
`views::restarts` (`:1121`) clamps a negative to `0` with `usize::try_from`,
which prints no count; `no_previous_run` compares `restarts != 0`, which for
`-1` suppresses the sentence. The two do differ, on an input no API server
produces.

`screens/detail.md:430` draws the describe events heading with no blank line
above it; `:527` draws one. `ui.rs:1914` pushes the blank, so the code follows
`:527`.
