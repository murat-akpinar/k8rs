# Screen — Object detail (the tabs)

`⏎` on anything opens it. Four tabs, `[` and `]` to move between them — the
whole debugging loop without a typed command.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│ RESOURCES          │  ‹ logs › describe   yaml   events            │
│   workloads        │  ───────                                      │
│   network          │  container: app ▾          previous log: on   │
│   storage          │                                               │
│   config           │  14:21:58  starting worker pool               │
│   cluster          │  14:22:01  connected to postgres              │
│ ANALYSIS           │  14:22:06  allocating 240MB cache             │
│   capacity      1 ▲│  14:22:07  --- killed here ---                │
│   certificates  30d│                                               │
│   drain safety     │  This is the log from before the last crash,  │
│   posture          │  which is usually the one you want.           │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app --previous             │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  ⇧p previous  / search  esc back   │
└────────────────────────────────────────────────────────────────────┘
```

| Tab | Shows | Notes |
|---|---|---|
| **logs** | follow, container picker, `--previous` | The most-typed kubectl command there is. `--previous` is one keypress because that is the log a crash loop needs. |
| **describe** | the object plus its events | Assembled from what we already hold; the event list is fetched for this object only, never a global Events watch. |
| **yaml** | the object as YAML | Key order is the API's, not alphabetised. Secret values are hidden behind an explicit reveal, and a revealed value never enters the command log, the audit log or this pane's copy buffer. |
| **events** | this object's events, newest first | Plain-language reason word, the controller's own message kept beside it: `Unhealthy` reads "the health check failed" next to "Readiness probe failed: …", never instead of it. |

## The logs tab

### The buffer: 2 MB retained, 5,000 lines, 4,096 bytes per line

Three numbers, not two, and only one of them is load-bearing. A line count
and a per-line byte cap multiply into a worst case nobody budgeted and no
reader of a table could predict — **~19.5 MB** was this section's first
draft, silently a product of the other two rather than a number anyone had
chosen. **The retained-bytes ceiling is the fix**: it is the one figure that
is true in both the common case and the worst case, because whichever of the
three limits below is hit first is the one that evicts.

| Bound | Value | What actually binds it |
|---|---|---|
| **Retained bytes per open pane** | **2 MB** | the load-bearing ceiling — the other two vary, this one does not |
| Lines kept per open pane | **5,000**, oldest dropped first | the common case: short lines fill 5,000 slots at well under 2 MB |
| Length of one line before it is cut | **4,096 bytes** | a single line, on its own, whatever the other two are doing |

**2 MB is the number to defend, and it does not lean on borrowed headroom.**
The security gate's *"a 50MB annotation or an endless log line must not be
held whole in memory"* is a **prohibition** on one unbounded value, not a
budget a feature may spend up to — this file's first draft read it backwards.
The figure that actually is a budget is a different 50 MB entirely:
`REQUIREMENTS.md`'s **whole-process** `< 50MB RSS at ~1000 pods`, and it is
already measured over — **58 752 KiB at 1 011 pods**, peak and steady the
same value, with the ruling that the target stays written as missed rather
than moved to match
([NOTES § D171](../NOTES.md#d171--the-resident-set-measured-at-four-sizes-the-budget-it-broke-and-the-ruling-that-the-budget-stays-2026-08-28)).
So 2 MB is not "spare room" in either figure — there is none to spend, one of
them prohibits the framing outright and the other already measures over it —
**60.2 MB against the 50 MB the target names** — before a log pane exists.
It is defended on its own terms instead: a **fixed** addition, small next to
the process's other costs (a single decoded page of objects alone runs
several MB, per D171's own arithmetic), that does not grow with session
length. Eight minutes or eight days, it is the same 2 MB — the property A6
lacked, where nothing was measuring what the log stream held over time and
it reached 21.5 GB resident before the node's own OOM killer acted
([PRIOR-ART § A6](../PRIOR-ART.md#a6--unbounded-memory-in-the-field-for-8-days)).
A byte ceiling is what makes that true regardless of line length; a line
count alone is not, which is exactly the gap the product-of-two left open.

**5,000 lines is what actually evicts in the ordinary case**, and is why the
dropped-lines count in the pane usually tracks something a reader can
picture: at a generous 256 bytes per line — a timestamped, short structured
message, not a stack trace — 5,000 lines is about **1.2 MB**, comfortably
under the 2 MB ceiling and enough to scroll back through a crash's run-up,
which is rarely more than a few hundred lines. **Only when lines run long
does the byte ceiling take over from the line count** — stuff every line to
the 4,096-byte cap and the pane holds roughly 500 of them, not 5,000, because
2 MB runs out first. Either way the pane never exceeds 2 MB; only how many
lines that buys changes.

**4,096 bytes is not a fresh number** — it is the same `FREE_TEXT` figure
`k8s::ingest` already uses to bound a message field on the way into the
snapshot, chosen there from a census of real captures. Reusing it here is a
`tui-designer` call, not a re-derivation: one number is one fewer to explain,
and it is independently generous for a log line (most are well under it; a
line that reaches it is already unusual). Whether the Rust constant is
literally shared or a same-valued sibling is `dev-core`'s call, not this
file's. Whether the **2 MB retained-bytes ceiling** is a third constant or
computed as `5000 × FREE_TEXT` headroom is likewise `dev-core`'s call — this
file specifies the observable behaviour (never more than 2 MB retained,
whichever bound gets there first), not the Rust shape underneath it.

### When the buffer fills: the dropped-lines line

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│ RESOURCES          │  ‹ logs › describe   yaml   events            │
│   workloads        │  ───────                                      │
│   network          │  container: app ▾          previous log: off  │
│   storage          │                                               │
│   config           │  142 lines were dropped from the top to keep  │
│ ANALYSIS           │  this pane bounded.                           │
│   capacity      1 ▲│                                               │
│   certificates  30d│  14:23:41  connected to postgres              │
│   drain safety     │  14:23:44  allocating 240MB cache             │
│   posture          │  14:23:47  writing checkpoint                 │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app -f                     │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  ⇧p previous  / search  esc back   │
└────────────────────────────────────────────────────────────────────┘
```

- **Silent below one drop, exact at and above it** — the same rule as every
  other banner on this product: nothing is shown until it is true
  ([states.md](states.md#the-second-paragraph-is-the-point-of-this-screen)).
  A pane that has dropped nothing shows no line at all; the moment the buffer
  evicts its first line, this replaces the blank row above the content and
  never goes away again for that pane — it only counts up.
- **Position: the top of the visible log content**, dim, because that is
  literally where the gap is — the lines missing are the oldest ones, which
  would have been above what is now the first line on screen
  ([widgets.md § 4](widgets.md#4-scrolling), which already promises this line
  and only this file was missing the number and the words).
- **The count is exact and grows live** — "1 line was dropped" (not "1
  lines"), then "2 lines", climbing for as long as the pane stays open and
  the stream outruns it. It is never rounded or bucketed; a beginner counting
  on this tool to tell the truth about what it lost gets the real number.
- **Follow (`f`) and the drop counter are independent.** Turning follow off
  freezes the *view*, not the stream underneath it while the pane is still
  open — dropping can still happen off-screen and the counter still climbs;
  turning follow back on does not "catch up" the dropped lines, because they
  are gone.

### A line longer than the cap, and a line longer than the pane — not the same thing

[widgets.md § 7](widgets.md#7-text-that-came-from-the-api) already rules that
ratatui wraps free text to the pane rather than k8rs clipping it — nothing
here overrides that. **Wrapping is not cutting.**
A perfectly ordinary long line — a connection string, a stack frame — simply
takes more rows in the pane. Nothing is missing and nothing is marked:

```
14:23:50  connecting to postgres://payments-db.svc.cluster
          .local:5432/payments?sslmode=verify-full&connect_
          timeout=10
```

A line that runs past the 4,096-byte cap is a different event: k8rs cut it,
and says so with the same marker the ingest prune already uses for an
over-long field, so the product has one way of saying "we shortened this,"
not two ([NOTES § D146](../NOTES.md#d146--the-ingest-guard-two-bounds-off-a-census-a-visible-marker-and-the-newline-a-real-kubelet-sent-2026-08-22)):

```
14:23:51  {"level":"error","msg":"panic: runtime err…  (shortened by k8rs)
```

(shown short for the page — the real cut lands at 4,096 bytes, or up to three
earlier, stepped back to a whole character so a multi-byte one is never
split.) **Attributed on purpose**, same reasoning as the ingest prune:
without the name on it, the cut reads as the application's own line trailing
off, and a debugging tool that quietly shortens the evidence is lying about
what it saw.

### Choosing a container, and when there is nothing to choose

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ payments/web-7d9f4 — pick a container ─────────────────────┐   │
│   │                                                            │   │
│   │  ▸ app                   running                           │   │
│   │    sidecar-envoy         running        3 restarts         │   │
│   │    init-migrate          done                              │   │
│   │                                                            │   │
│   │  sidecar-envoy restarted 3 times. ⇧p on it shows           │   │
│   │  the log from just before its last crash.                  │   │
│   │                                                            │   │
│   │         [ ⏎ pick ]       [ esc cancel ]                    │   │
│   │                                                            │   │
│   └────────────────────────────────────────────────────────────┘   │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app                        │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move   ⏎ pick   esc cancel                                      │
└────────────────────────────────────────────────────────────────────┘
```

Same list-picker shape as [the cluster picker](context.md#the-picker) —
`▸` for the row that would open, one line per entry, a state word instead of
a tag column. **Restart count is shown next to a container that has one**,
because that is exactly the signal that makes `⇧p` worth pressing, and the
line under the list spells out which key does it and on which container.

**A single-container pod has nothing to pick, so the picker is not offered
at all** — invariant: a key that does nothing is a bug already shipped once
here.

| Element | Multi-container pod | Single-container pod |
|---|---|---|
| Header line | `container: app ▾   previous log: off` | `container: app   previous log: off` — no `▾`, nothing opens |
| Footer | `… c container …` | `c container` is gone from the footer |
| `c` | opens the picker above | not bound; there is only ever one answer |

### No logs yet, no previous run, and the pod disappearing mid-stream

**A container that has produced nothing** is a state, not a hang
([PRIOR-ART § E1](../PRIOR-ART.md#e1--a-stream-ends-for-many-reasons-and-the-viewer-says-one-thing)) —
a `Pending` pod, or a container that just started:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/queue-worker-xk2p9                  │
│ RESOURCES          │  ‹ logs › describe   yaml   events            │
│   workloads        │  ───────                                      │
│   network          │  container: worker ▾       previous log: off  │
│   storage          │                                               │
│   config           │               ○  no logs yet                  │
│ ANALYSIS           │                                               │
│   capacity      1 ▲│        Nothing has been written to this       │
│   certificates  30d│        container's log yet.                   │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs queue-worker-xk2p9 -n payments -c worker -f         │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  ⇧p previous  / search  esc back   │
└────────────────────────────────────────────────────────────────────┘
```

`○` is reused rather than invented — it is already the product's symbol for
*calm, not a problem, just information* ([states.md](states.md#nothing-is-broken)).

**`⇧p` on a container that has never restarted** has no previous run to
show. k8rs does not print the API's refusal and does not leave `previous
log: on` pointed at nothing — it says so in one line and falls back to the
run that does exist:

```
  container: app ▾          previous log: off
  ⇧p — app hasn't restarted, so there's no previous run
       to show. Showing the current run instead.
```

**The stream ends because the pod itself is gone** — deleted while `f` was
following it. This is the one case E1 asks for by name: say why the stream
ended, and do not make the reader wonder whether it was a dropped connection.
It is marked the same way the existing mockup already marks a kill event
in-line (`--- killed here ---`), because that convention already exists on
this exact screen and a second one would be a second thing to learn:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│ RESOURCES          │  ‹ logs › describe   yaml   events            │
│   workloads        │  ───────                                      │
│   network          │  container: app ▾          previous log: off  │
│   storage          │  14:24:58  writing checkpoint                 │
│   config           │  14:25:02  shutting down                      │
│ ANALYSIS           │  14:25:03  --- stream ended: pod deleted ---  │
│   capacity      1 ▲│                                               │
│   certificates  30d│  Not a dropped connection — the pod itself    │
│   drain safety     │  is gone, so there's nothing left to stream.  │
│   posture          │                                               │
│   restarts         │  payments/web is a Deployment, so a           │
│   waste            │  replacement pod is probably starting. esc,   │
│   versions         │  then ⏎ its row opens that one instead.       │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl logs web-7d9f4 -n payments -c app -f                     │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  f follow  c container  ⇧p previous  / search  esc back   │
└────────────────────────────────────────────────────────────────────┘
```

No `⚠` here — that symbol is reserved for a connection or trust problem and
this is neither: the connection is fine, the object it was watching no
longer exists ([README.md § The five rules every screen obeys](README.md#the-five-rules-every-screen-obeys)).
**"Offer resume"** (E1) is not literal for a deleted pod — there is nothing
left to resume — so the offer is the next useful thing instead: open
whatever replaced it, through the same `esc` and `⏎` every other screen
already uses. No new key.

### Printed instead of drawn — logs on the headless surface

Phase 6 has no TUI: the temporary `main.rs` prints this tab's output to
stdout the way [`--once`](once.md) prints findings to it, with the same
[stdout/stderr split](once.md#stdout-and-stderr-are-split-on-purpose) — the
lines are the payload, the kubectl-equivalent line is the teaching device on
stderr. **What flag or argument tells the temporary driver which pod to
stream is not decided by this file** — today's parser only knows the four
flags invariant 10 names, and picking the fifth is a CLI-surface call for
whoever wires this box, not a screen. The bounds above do not carry over
identically regardless of that shape, and the reason is what each one
actually protects:

- **The per-line cut is a property of the line itself**, sanitised and
  capped before either surface sees it — `… (shortened by k8rs)` prints
  byte-for-byte the same whether it lands in a pane or a pipe. Piping this
  stream through `grep` sees exactly what the pane would have shown.
- **The headless path carries no lossy buffer between the stream and
  stdout, and that is a requirement on it, not a description of how it
  happens to be built.** A pane needs the retained-bytes ceiling because it
  has to *redraw* recent lines; a print-as-it-arrives loop does not, provided
  each line is capped, printed and forgotten with nothing buffered in
  between — no line kept past the moment it is written, none ever evicted to
  make room for a newer one. That is what a stdout dump with **no
  dropped-lines counter** is honest about only if nothing upstream of the
  print can silently lose a line instead. A bounded channel between the API
  stream and stdout is an ordinary thing to reach for and nothing here rules
  it out by construction — so if the driver is ever built with one, the same
  dropped-lines wording this file already specifies for the pane goes on
  stdout with it, on its own line, same rule as
  [once.md](once.md#stdout-and-stderr-are-split-on-purpose): it is payload,
  so it belongs on stdout, not stderr. Only a driver with no such buffer gets
  to print nothing here — and, done that way, it holds the same near-zero
  resident memory whether it runs for eight minutes or the eight days that
  grew k9s to 21.5 GB (A6), which is a stronger property than a bounded
  retained buffer, not a weaker one.

Whatever the invocation turns out to be, the printed shape is the teaching
line on stderr followed by the sanitised, capped content on stdout — nothing
else runs between them:

```
$ kubectl logs web-7d9f4 -n payments -c app -f
14:23:41  connected to postgres
14:23:44  allocating 240MB cache
14:23:51  {"level":"error","msg":"panic: runtime err…  (shortened by k8rs)
14:25:03  --- stream ended: pod deleted ---
```

Rules for this screen:

- **Log streams are attacker-controlled text: an open pane retains at most
  2 MB, oldest lines dropped first — up to 5,000 lines in the common case,
  fewer if they run long; a single line is cut at 4,096 bytes and marked.**
  Control characters are stripped before any of the three bounds is applied.
  [§ The logs tab](#the-logs-tab) has the arithmetic and the wording; this
  line used to promise a bound with no number, which is how a bound stays
  unbuilt.
- The finding that brought you here stays visible at the top — you never lose
  the reason you opened the object.
- **That block draws the finding's evidence in full**, and it is the only place
  that does. The Alerts card caps it at three wrapped lines with `…`, because a
  controller's verbatim message runs past any card
  ([alerts.md § the height](alerts.md#the-height)); this is where the rest of it
  is, and the cut is only honest because this screen exists. The block wraps to
  the pane and **scrolls with it** rather than being pinned — a nine-line quote
  pinned above a log pane leaves no log pane.
- On a grouped finding, `⏎` first lists *which* pods of the group are affected,
  then opens the one you pick. **The finding block is on that step too**, for
  the same reason: the full message must never be two keypresses away, or the
  card's `…` is pointing at nothing the reader can find.

## The describe tab

**The object, plus what happened to it — assembled from two reads, never
from the watch store.** `describe` opens with the same fresh, unpruned GET
[the yaml tab](#the-yaml-tab) uses (NOTES §
[D194](../NOTES.md#d194--the-flag-that-names-an-object-and-d17s-threshold-read-against-the-binary-it-was-written-for-2026-08-30)),
then adds one more read this family builds for the first time: this object's
own events, fetched by an `involvedObject` field selector that names it and
nothing else. **Never the global Events watch** — that would mean holding
every event in the cluster just to answer one object's question, and
invariant 6 already draws the line at watching Pods, Nodes and the three
workload kinds.

**This is the only place in Phase 6 that reads events, and it is the whole
reason the fetch is being built now rather than when the events *tab* needs
it in Phase 11.** `k8s.rs` freezes at the end of this phase; a fetch built
only for the tab would have to be reopened later for describe, and a second
version of "this object's events" is exactly the two-places-disagreeing
defect this repo pays most for (invariant 11's own reasoning, one layer up).
One function, two callers, one order — newest first — settled once here.

**What describe needs of events, and what it deliberately does not build.**
The events *tab*'s own drawn layout — its own scrolling, its own columns, the
full reason-to-sentence table — is Phase 11's, out of scope for this file
today. What describe needs is smaller: the same list, oldest to newest
reversed, each line short enough to sit under a container block without
turning the pane into the tab it is not trying to be.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│ RESOURCES          │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  Pod · running · created 3 days ago           │
│   storage          │  containers                                   │
│   config           │    app             failed                     │
│   cluster          │      container exceeded its memory limit —    │
│ ANALYSIS           │      exit 137, 4 restarts                     │
│   capacity      1 ▲│    sidecar-envoy   keeps crashing and         │
│   certificates  30d│      restarting, 12 restarts                  │
│   drain safety     │    init-migrate    done                       │
│   posture          │  events (newest first)                        │
│   restarts         │  3 min ago  the container is being stopped    │
│   waste            │  (Killing) Stopping container app             │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod web-7d9f4 -n payments                       │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back                                                 │
└────────────────────────────────────────────────────────────────────┘
```

- **The containers block reuses the picker's order and its calm words**
  (declared, then init; `running` / `done` / `not started` unchanged) **but
  no longer stops at `waiting` for every container that isn't** — measured
  on a real pod, `done` was printed for a container that exited `0` *and*
  for one that exited `255`, and `waiting` covers `ImagePullBackOff`,
  `CrashLoopBackOff` and `CreateContainerConfigError` alike
  (`k8s-admin`, three containers, 2026-08-31). `done` is still correct for a
  clean exit; everything else now says what actually happened, below.
- **A terminated container that did not exit cleanly says its reason, in
  plain language where one is known, and its exit code always** — `app`
  above is the pod's own OOM story from [the logs tab](#the-logs-tab),
  `container exceeded its memory limit` being invariant 14's own worked
  example (`CLAUDE.md`). Only `OOMKilled` is translated today; every other
  reason — `Error`, `ContainerCannotRun`, or the empty string a real
  container can carry (`k8s-admin` measured `reason=Error, exit=1` and a
  bare `exit=255` with nothing in `reason` on the same pod) — falls through
  to the exit code alone, never a guessed word. **`done` is not renamed to
  `failed` before it earns that word**: a clean `exit 0` stays `done`,
  because that is not a diagnosis, it is the healthy case.
- **A waiting container says its reason in plain language, not the generic
  `waiting`** — `sidecar-envoy` above is `CrashLoopBackOff`, translated;
  `ImagePullBackOff` / `ErrImagePull` and `CreateContainerConfigError` get
  their own short phrases the same way. An ordinary, momentary
  `ContainerCreating` stays the calm `not started` rather than being dressed
  up as a problem, and a reason this table does not recognise falls through
  to the raw word, sanitised — the same safe-fallback rule the events table
  below uses, stated once and reused rather than invented twice.
- **Every event reason is a short plain-language phrase *beside* the
  controller's own message, never instead of it** ([NOTES §
  D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
  `Killing` above reads *"the container is being stopped"* on its own line,
  then `(Killing) Stopping container app` — the raw word and the verbatim
  message together — on the line under it. **This reverses what this file
  said before**: a translated sentence that *replaces* the message can be
  measurably false (`Pulled` translated as "the image finished downloading"
  is false whenever the image was already cached) or can quietly delete the
  one fact the diagnosis turns on (`Unhealthy` covers both a liveness probe,
  which kills the container, and a readiness probe, which only takes it out
  of the Service — the same reason word, two different outcomes, and only
  the message says which). The table below is short **on purpose**: a reason
  not in it prints as its own raw word, the message beside it, and nothing
  invented — the same discipline `BackOff` already had, now applied to every
  reason rather than carved out for one.

  | Raw reason | Phrase |
  |---|---|
  | `Scheduled` | kubernetes placed this pod on a node |
  | `Pulling` | the container started pulling its image |
  | `Pulled` | the image is ready |
  | `Killing` | the container is being stopped |
  | `Unhealthy` | the health check failed |
  | `BackOff`, or anything else | *(no phrase — the raw word and the message, nothing more)* |

- **The age is [the one ladder](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen),
  not a shorthand.** `3 min ago`, `1 hour ago`, `4 days ago` — the same
  strings a card's right edge or `--once`'s title suffix draws, because it is
  one function reached from here too. `1 day ago` never appears, for the same
  reason it never appears anywhere else on this product.
- **No command line of its own for the events fetch.** `kubectl describe`
  is one word the user would have typed for two real reads — the pod and its
  events — and invariant 4 already tells the command log to show the
  *equivalent*, not the two calls underneath it. The audit log, not drawn on
  this page, is where both real reads land. A reveal of any kind costs no
  command line either, the same reasoning [the yaml tab's](#a-secret-values-hidden-behind-an-explicit-reveal)
  does — nothing here is ever sent to the cluster that the object's own read
  did not already send.

### No events at all — a healthy pod is not a broken fetch

A pod up for a week has almost certainly outlived every event it ever had —
Kubernetes keeps them for a while and then drops them, so *nothing left* and
*nothing happened* are different facts wearing the same empty list. Saying
only "nothing happened" would be true the day the pod started and false a
week later, in the one case a reader has no other way to check.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web                                 │
│ RESOURCES          │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  Pod · running · created 8 days ago           │
│   storage          │                                               │
│   config           │  containers                                   │
│   cluster          │    app             running                    │
│ ANALYSIS           │                                               │
│   capacity      1 ▲│  events                                       │
│   certificates  30d│  ○  none right now                            │
│   drain safety     │                                               │
│   posture          │  Kubernetes only keeps events for a while, and│
│   restarts         │  this pod has run long enough that none are   │
│   waste            │  left.                                        │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod web -n payments                             │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back                                                 │
└────────────────────────────────────────────────────────────────────┘
```

`○` is the product's own symbol for *calm, not a problem*
([states.md](states.md#nothing-is-broken)), reused rather than invented —
same rule the no-logs-yet state already follows
([§ No logs yet](#no-logs-yet-no-previous-run-and-the-pod-disappearing-mid-stream)).
The second line is the point of the state, same as everywhere else on this
product ([states.md § the second paragraph is the point of this screen](states.md#the-second-paragraph-is-the-point-of-this-screen)):
it says *why* the list is empty, not just that it is.

### A repeated event — one line for something that happened 2,383 times

The kubelet does not create a new Event object per occurrence; it bumps
`count` on the one it already has. A card that shows only the last
occurrence and the reason word is silently dropping the fact that actually
matters: *"the health check failed 3 minutes ago"* and *"it has failed 2,383
times since the pod started"* are different diagnoses of the same pod, and
only one of them tells a reader whether to look now or to have looked four
days ago.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│ RESOURCES          │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  Pod · running · created 5 days ago           │
│   storage          │  containers                                   │
│   config           │    app             running                    │
│   cluster          │                                               │
│ ANALYSIS           │  events (newest first)                        │
│   capacity      1 ▲│  3 min ago  the health check failed           │
│   certificates  30d│  (Unhealthy) Readiness probe failed:          │
│   drain safety     │  HTTP probe failed with statuscode: 503       │
│   posture          │  happened 2,383 times since 4 days ago        │
│   restarts         │                                               │
│   waste            │  4 hours ago  the image is ready              │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod web-7d9f4 -n payments                       │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back                                                 │
└────────────────────────────────────────────────────────────────────┘
```

- **The fourth line only appears when `count` is more than one.** `4 hours
  ago the image is ready` above carries no such line — a `count` of `1`
  draws exactly what every other event row draws, nothing extra, because a
  thing that happened once needs no sentence saying it happened once.
- **`happened N times since <span> ago`, not `x2383 over 4d8h`.** The number
  is exact — commas at the thousand, never rounded, the same discipline the
  dropped-log-lines counter already keeps
  ([§ When the buffer fills](#when-the-buffer-fills-the-dropped-lines-line)) —
  and the span uses [the one age ladder](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen)
  a second time, on `firstTimestamp` rather than `lastTimestamp`. **Both
  numbers are needed and neither replaces the other**: the count without the
  span is "a lot," of unknown recency; the span without the count is "still
  going," of unknown severity. `kubectl`'s own `3m14s (x2383 over 4d8h)` is
  the same two facts, spelled for someone who already knows what `x` and
  `over` mean here.
- **This is measured, not invented** (`k8s-admin`, 2026-08-31): a real
  readiness probe on an 8-day cluster, `count` 2,383, first seen 4 days
  before the last. The translated line above is this file's own words over
  those real numbers.

### More events than the pane — it scrolls, the same as everything else

Describe does not cap the list and does not add its own "N more" line. The
pane is a `Paragraph` with a scroll offset like every other overflowing pane
on this product ([widgets.md § 4](widgets.md#4-scrolling)), and a busy pod's
events scroll exactly the way the Analysis panes that refuse to cap already
do ([analysis.md § Restarts](analysis.md#restarts)) — no new affordance, no
new key.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                           │
│ RESOURCES          │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  4 hours ago  the container started pulling   │
│   storage          │  its image                                    │
│   config           │  (Pulling) Pulling image "payments/web:2.3.1" │
│   cluster          │                                               │
│ ANALYSIS           │  6 hours ago  kubernetes placed this pod on a │
│   capacity      1 ▲│  node                                         │
│   certificates  30d│  (Scheduled) Successfully assigned            │
│   drain safety     │  payments/web-7d9f4 to node-3                 │
│   posture          │                                               │
│   restarts         │  9 hours ago  BackOff                         │
│   waste            │  Back-off restarting failed container app     │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod web-7d9f4 -n payments                       │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back                                                 │
└────────────────────────────────────────────────────────────────────┘
```

This is the same pod as the main mockup, scrolled past the identity line and
the containers block — the tab row and the underline under it stay pinned
(they are the `Tabs` widget, a separate element from the scrolling
`Paragraph` below it), only the body moves. A message too long for one line
wraps to the pane exactly as a log line does
([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)) — *"the container
started pulling its image"* above is the ordinary case, not a special one.
`BackOff` is the fall-through case drawn for real: no phrase, the raw word
and the message it came with, nothing invented.

### The pod's own reason, when it has one

`status.reason` sits beside `status.phase` and was dropped entirely before
this review — a pod carrying `reason: Evicted` printed `Pod · failed ·
created 8 days ago` and never said why, which is a `Failed` that told a
reader nothing a `Failed` from any other cause would not also have told
them. The identity line now carries it, translated where the word is known,
with `status.message` kept beside it for the same reason an event's message
stays beside its reason — [the Waste report](analysis.md#waste) already
promises this exact page: *"look at one of the pods — its own message names
what ran out."* This is that pod.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/worker-4kd2p                        │
│ RESOURCES          │  logs   ‹ describe ›   yaml   events          │
│   workloads        │         ──────────                            │
│   network          │  Pod · failed · created 8 days ago            │
│   storage          │  removed by the node to take back room        │
│   config           │  (Evicted) The node was low on resource:      │
│   cluster          │  ephemeral-storage.                           │
│ ANALYSIS           │                                               │
│   capacity      1 ▲│  containers                                   │
│   certificates  30d│    worker          not started                │
│   drain safety     │                                               │
│   posture          │                                               │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl describe pod worker-4kd2p -n payments                    │
├────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back                                                 │
└────────────────────────────────────────────────────────────────────┘
```

- **The phrase and the raw word follow the exact same layout as an event**:
  a short line, then `(RawReason) message` under it, wrapped like any other
  pane text. One layout for "a word that explains a state," used for a pod's
  own reason, a container's reason and an event's reason alike, is the point
  — three separate inventions here would be three things to keep agreeing.
  Only `Evicted` is translated today; anything else `status.reason` can hold
  falls through to the raw word beside the message, the same safe fallback
  as everywhere else on this page.
- **`Pod · failed · …` still comes first, unchanged**, because `status.phase`
  and `status.reason` answer different questions — *what state is it in*
  and *why* — and a reader who only wants the first still gets it on one
  short line.

### An object that does not exist

Reachable today only headlessly, because `⏎` in the eventual browser opens a
row that is, by construction, currently in the watched store — a typed
`--object` name has no such guarantee. [§ Printed instead of drawn](#printed-instead-of-drawn--describe-on-the-headless-surface)
below has the exact sentence; it is the same one `logs_run`'s pod-fetch
already prints for a missing pod, reused rather than reworded, because
describe's first read *is* that fetch. **The drawn pane's own version of this
state is Phase 11's** — dialogs.md already has the shape a re-read that finds
the object gone should take
([§ The object went away while the dialog was open](dialogs.md#the-object-went-away-while-the-dialog-was-open)),
and describe/yaml opening on a since-deleted object is the same fact reached
by a different door, not a new one to design here.

### Free text that carried control characters

An event's `message` is exactly the kind of field invariant 9 exists for —
free text a controller wrote, not something k8rs generated. It is stripped
before it is drawn, the same way a log line or a pod name is
([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)): silently,
character by character, with nothing left in its place. A message reading
`FailedMount: secret "prod` + U+202E + `terces" not found` draws as
`FailedMount: secret "prodterces" not found` — the reader sees a shorter,
correctly-ordered sentence and nothing marks that anything was removed,
exactly as `sanitize()` already does for every other free-text field on this
build (`src/main.rs`, `fn sanitize`). Reused, not reinvented: this file does
not ask for a second convention where the first one already holds.

**Unchanged on purpose, checked against [the yaml tab's own
reversal](#free-text-that-carried-control-characters--reversed-for-this-one-pane)
rather than assumed exempt from it** (NOTES §
[D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)
narrows this by the reason it was written, not by a second reading of it).
D198's own distinction is exactly the one that decides this: a **document**
keeps `\n` because the payload *is* the text and a newline prints as itself;
a **cell** does not, because in a cell a newline breaks the layout instead
of printing as itself. Every row on this tab — the identity line, a
container's row, an event's — is a cell: one line in a list, wrapped by
width like any other pane text but never carrying a hard line break of its
own, the same way a table row never does. A `\n` inside an event's message
would not print as a second line of that event, it would open a second
row that looks like a second event — which is worse than losing the
character, not better. So this section's own example — a control character
disappearing with nothing marking the cut — is the correct behaviour here,
and stays exactly as written above.

### Printed instead of drawn — describe on the headless surface

Same split as [the logs tab](#printed-instead-of-drawn--logs-on-the-headless-surface)
and [`--once`](once.md#stdout-and-stderr-are-split-on-purpose): the teaching
line on stderr, the payload on stdout, `--describe` beside the same `--object`
[D194](../NOTES.md#d194--the-flag-that-names-an-object-and-d17s-threshold-read-against-the-binary-it-was-written-for-2026-08-30)
already reserved for this family of verbs. **No new pod-fetch code**: the
namespace check, the 404 sentence, the connect-timeout sentence and the
cluster-unreachable sentence are `logs_run`'s own first steps, read again
rather than rewritten, because describe's first read is the identical
`k8s::pod()` call before either verb touches what makes it different.
`--container`, `--previous` and `--follow` are simply not read by
`--describe` — not specially refused, the same way `--context` without
`--live` is not refused today (`fn mistyped`'s own stated rule).

```
$ kubectl describe pod web-7d9f4 -n payments
Pod · running · created 3 days ago

containers:
  app             failed
    container exceeded its memory limit — exit 137, 4 restarts
  sidecar-envoy   keeps crashing and restarting, 12 restarts
  init-migrate    done

events (newest first):
  3 min ago  the container is being stopped
    (Killing) Stopping container app
```

A pod carrying `status.reason` prints it the same way, right under the
`Pod · … · created …` line — `removed by the node to take back room` then
`(Evicted) The node was low on resource: ephemeral-storage.` — and an event
with `count` above `1` prints one more line under its message: `happened
2,383 times since 4 days ago`. Both are the drawn pane's own wording,
unboxed.

**No events prints no heading, on stdout or stderr** — the same reasoning
`nothing_written` already states for a container with no log yet
(`src/main.rs`): stdout is the payload, and when the payload really is
empty it stays empty rather than dressing itself up, so a reader piping
`k8rs --describe … | wc -l` still gets an honest count and still learns why
from stderr. An empty `events (newest first):` heading over nothing is not
honest payload, it is decoration, so the heading is dropped and the one-line
explanation — *"Kubernetes only keeps events for a while, and this pod has
run long enough that none are left"* — moves to stderr instead, beside the
`$ kubectl describe …` line it belongs with.

**Exit code is the only thing that can carry the difference between *this
object has no events* and *k8rs could not find out*.** Both print the
identical `Pod · … / containers: …` block on stdout with no events
section — the payload looks the same either way, because in both cases
describe has nothing honest to add under the heading it already dropped.
The distinction a reader needs — *calm* versus *broken* — has nowhere left
to live but the exit code: `0` for a read that succeeded and found nothing,
`2` for a read that did not finish. A script branching on `k8rs --describe …
&& echo ok` gets the right answer even though stdout alone could not have
told it.

| Failure | Stream | Exit |
|---|---|---|
| `--object` names no such pod | stderr: `k8rs: there is no pod named ghost in payments — check the name and the namespace` | `2` |
| Cluster unreachable / login expired | stderr: the same `because(...)` sentence `logs_run` prints | `2` |
| The pod fetch does not answer inside the timeout | stderr: `k8rs: this cluster has not answered for the pod … in … after … seconds` | `2` |
| Events fetch fails after the pod read succeeded | stderr: one sentence naming what failed, same `because(...)` shape | `2` |
| Nothing has gone wrong, this object simply has no events | stderr: the one-line explanation above; stdout carries the object and containers with no events section | `0` |
| `--kind` names anything but `pod` | stderr: `k8rs: --describe only knows how to read a pod right now — containers and events don't mean the same thing on a Secret. --kind pod is the only value it accepts` | `2` |
| Success | stdout: the block above | `0` |

**One open question this file does not close**: if a run somehow named more
than one of `--logs` / `--describe` / `--yaml`, which one wins is a tie-break
`dev-core` picks and records — the same way [D194](../NOTES.md#d194--the-flag-that-names-an-object-and-d17s-threshold-read-against-the-binary-it-was-written-for-2026-08-30)
left the flag's exact spelling to them. This file specifies what each verb
shows once chosen, not the precedence between three that all narrow to one
object.

## The yaml tab

**A fresh, unpruned GET — never the watch store.** The store is pruned to the
fields `rules.rs` names (invariant 6); a yaml pane fed from it would show a
partial object and call it the object. So `y` re-reads, the same way `d`
does, and pays the same one extra round trip for the same reason: this is
the one pane on the product where "the object, exactly as the API returned
it" is the entire point, and a stale, pruned stand-in would be lying about
what it is.

**This mockup and the two after it are drawn 80 columns wide, not this
file's usual 70** — the first width exception anywhere in `screens/`, and a
measured one, not a stylistic one. `kubectl`'s own `--show-managed-fields`
is 22 characters on its own, and once it is added to the teaching line
(below), no object name that still reads as one fits inside a 68-column
strip: the shortfall was checked with the actual flag and the actual
namespace, not estimated. 80 is not a wider terminal than this product
promises — it is [the same 80×24
minimum](README.md#how-to-read-them) this whole directory already targets,
drawn at the width it actually has rather than the narrower 70 chosen
elsewhere for the page's own readability; the sidebar stays the fixed 20 it
always is, and the content pane takes the extra 10 columns, exactly the rule
[widgets.md § 1](widgets.md#1-the-frame) already states for any terminal
wider than the minimum. Nothing here works *only* wider than 80×24 — it is
what 80×24 already draws, shown at its own size instead of a narrower one.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/web-7d9f4                                     │
│ RESOURCES          │  logs   describe   ‹ yaml ›   events                    │
│   workloads        │                    ──────                               │
│   network          │apiVersion: v1                                           │
│   storage          │kind: Pod                                                │
│   config           │metadata:                                                │
│   cluster          │  name: web-7d9f4                                        │
│ ANALYSIS           │  namespace: payments                                    │
│   capacity      1 ▲│  labels:                                                │
│   certificates  30d│    app: web                                             │
│   drain safety     │  managedFields:                                         │
│   posture          │    - manager: kubelet                                   │
│   restarts         │      operation: Update                                  │
│   waste            │      …                                                  │
│   versions         │spec:                                                    │
│                    │  containers:                                            │
│                    │    - name: app                                          │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields        │
├──────────────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back                                                           │
└──────────────────────────────────────────────────────────────────────────────┘
```

- **`--show-managed-fields` is on the teaching line because `kubectl` has
  hidden `managedFields` from `get -o yaml` by default since 1.21, and k8rs
  does not** — measured on a real pod: 95 of 246 lines, 39% of the document,
  are `managedFields` (`k8s-admin`, 2026-08-31). Dropping them to match
  `kubectl`'s default was considered and refused: this pane's one claim is
  that it is the object, and pruning a field the object actually carries to
  make the printed line simpler is the exact failure [the yaml pane's own
  intro above](#the-yaml-tab) already refuses for the watch store. The flag
  says, honestly, what the command underneath was always going to return.
- **`managedFields` is real YAML and gets no special treatment beyond
  wrapping** — shown short for the page above (`…`, the same marker every
  other trimmed-for-the-page block in this file uses), not because k8rs cuts
  it. A reader who wants the whole thing has the whole thing; this is a
  page-width choice, not a product one.
- **No two-space reading margin here**, unlike every other tab on this
  screen. The pane's own left edge *is* the document's, so the YAML's own
  indentation is the only indentation drawn — adding a margin on top of it
  would misrepresent what the object's own structure is, which is the one
  thing this pane exists to get right. `yaml` and `logs` are the two panes
  that do not wrap-trim for exactly this reason
  ([widgets.md § 2](widgets.md#2-element--widget)): leading whitespace is
  meaningful in both.
- **Key order is the API's, never alphabetised** — already promised in the
  tab table at the top of this file, restated here because it is this pane's
  whole contract with a reader who already knows what `kubectl get -o yaml`
  looks
  like and is checking that this is the same thing.
- **Nothing here is masked except on a Secret — and there, `data` is not
  the only field that carries the Secret.** Every value under
  `metadata.annotations` is masked too, the same way and for the same
  reason as `data`, below — an annotation on a Secret is treated as a copy
  of the Secret, not as metadata about it ([NOTES §
  D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
  `metadata.labels` is not masked: it is validated to 63 characters by the
  API server itself, nothing writes a Secret's body into one, and the
  review that found the annotation leak looked and found none — a residual
  named on purpose rather than silently assumed safe.
- **A Pod's literal environment values**, if it has any set directly rather
  than through a `secretKeyRef`, are shown exactly as the API returned them,
  unredacted. That reads as a contradiction of *"environment variable values
  are never displayed"* ([REQUIREMENTS §
  DevSecOps](../REQUIREMENTS.md#devsecops-requirements) ·
  [docs/security.md § Data displayed and stored](../docs/security.md#data-displayed-and-stored))
  until the rule that already settled it is read: that line governs what
  *k8rs goes and fetches and interprets on its own initiative* — the same
  words cover Secret data — and it is explicitly not a mandate to hide what
  `kubectl` already shows verbatim
  ([NOTES §
  D37](../NOTES.md#d37--a-controllers-message-is-a-status-field-not-a-payload-2026-08-12) ·
  [D188](../NOTES.md#d188--where-a---once-report-ends-up-and-the-flag-that-is-the-only-reader-three-shipped-rules-have-2026-08-30)).
  A yaml pane that diverged from `kubectl get -o yaml` on an ordinary field
  would be lying by omission about what it is, which is the exact failure
  D37 already ruled against for a controller's own message. Building a
  detector for "this field looks like a secret" is the masking engine
  REQUIREMENTS itself already calls YAGNI — Secret `data` and, now,
  `metadata.annotations` get their own rule because each is a *named,
  structural* field, on a *specific kind*, not a heuristic over free text.

### A Secret, values hidden behind an explicit reveal

`data`'s values are one thing this pane always masks, by key, with the size
the value decodes to rather than the value itself — and, since this review,
so is every value under `metadata.annotations`, by the same rule:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/db-credentials                                │
│ RESOURCES          │  logs   describe   ‹ yaml ›   events                    │
│   workloads        │                    ──────                               │
│   network          │apiVersion: v1                                           │
│   storage          │kind: Secret                                             │
│   config           │metadata:                                                │
│   cluster          │  name: db-credentials                                   │
│ ANALYSIS           │  namespace: payments                                    │
│   capacity      1 ▲│  annotations:                                           │
│   certificates  30d│    kubectl.kubernetes.io/last-applied-configuration:    │
│   drain safety     │    <hidden — 612 bytes>                                 │
│   posture          │  managedFields: …                                       │
│   restarts         │type: Opaque                                             │
│   waste            │data:                                                    │
│   versions         │  username: <hidden — 8 bytes>                           │
│                    │  password: <hidden — 16 bytes>                          │
│                    │  tls.crt: <hidden — 1,172 bytes>                        │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get secret db-credentials -n payments -o yaml --show-managed-fields│
├──────────────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  v reveal  esc back                                                 │
└──────────────────────────────────────────────────────────────────────────────┘
```

**Why an annotation on a Secret gets treated as a copy of the Secret rather
than as metadata about it.** `kubectl apply -f secret.yaml` — the ordinary
way a Secret is created — writes the whole applied body, `data` included,
into `metadata.annotations["kubectl.kubernetes.io/last-applied-configuration"]`,
base64 inside base64; a Secret written through `stringData` puts
**plaintext** there instead, not even base64. Measured: that annotation's
own bytes decode to the same `username`/`password` values the block above
already shows masked, so masking `data` and leaving `metadata.annotations`
alone would tell a reader this document is safe to paste into a ticket while
the second copy sits four lines away, untouched (`k8s-admin`, 2026-08-31;
NOTES §
[D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
**Every key in `metadata.annotations` is masked, not only this one** — a
denylist of `last-applied-configuration` by name is invariant 1's own
allowlist-not-denylist reasoning applied one layer up: every GitOps
controller that reconciles a Secret writes its own reconstruction into its
own annotation key, and a mask that only catches the one `kubectl` happens
to write catches nothing the day a different controller manages the object.
The key is still drawn, so a reader can see *that* something is stored
there and go looking with `kubectl` if they need to; only the value is
replaced by its size.

**`v` reveals `data`, and only `data`** — the modal below lists
`username` / `password` / `tls.crt`, never the annotation. A copy nobody
asked to see is not a value somebody pressed a key to read; revealing it
would be k8rs itself decoding and displaying a controller's own
reconstruction of the Secret, which is one more place the plaintext could
end up on screen for no reader benefit over the three real keys already
there. If a reader genuinely needs what is inside that annotation, `kubectl`
still shows it — k8rs choosing not to make it one keypress easier here is
the whole point of masking it in the first place.

`v` is new — free, not used anywhere else in the key map (`r` already means
*restart*, everywhere, and could not be reused here even though "reveal"
reads just as naturally). It only appears in the footer when the object is a
Secret **and** it has at least one key — a key that does nothing is a bug
this product has already shipped once, and a zero-key Secret has nothing to
reveal (below).

**`v` opens a small modal; it does not rewrite the pane.** This is the
choice the brief leaves to this file, and it is what makes the security
gate's own sentence literally true rather than a matter of trusting the
terminal's mouse selection: *"a revealed value never enters the command log,
the audit log or the YAML shown by `y`"*
([docs/security.md § Data displayed and stored](../docs/security.md#data-displayed-and-stored)).
If reveal rewrote the pane's own text in place, whatever copies that text —
a future export, a future "copy the whole document" key — would carry the
plaintext with it. A modal is a separate, disposable view: what it shows
never becomes part of the document, so the document — the thing `y` is
named for — stays masked no matter how many times a value is looked at.
This reuses the existing modal layer rather than inventing a new mechanism
([widgets.md § 5](widgets.md#5-the-modal-layer)), the same shape the
container picker already is.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│   ┌ payments/db-credentials — revealed ────────────────────────┐   │
│   │                                                            │   │
│   │  username     admin                                        │   │
│   │  password     hunter22                                     │   │
│   │                                                            │   │
│   │  tls.crt      binary — 1,172 bytes, not shown as           │   │
│   │               text                                         │   │
│   │                                                            │   │
│   │  esc closes this — nothing was sent to the                 │   │
│   │  cluster to show it.                                       │   │
│   │                                                            │   │
│   └────────────────────────────────────────────────────────────┘   │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ $ kubectl get secret db-credentials -n payments -o yaml            │
├────────────────────────────────────────────────────────────────────┤
│ esc close                                                          │
└────────────────────────────────────────────────────────────────────┘
```

- **No new command-log line.** The value was already in the object k8rs
  already read; revealing it decodes bytes already in memory and draws them.
  Nothing is sent to the cluster, so the command strip stays on whatever it
  already showed, and the modal's own footer says so in the reader's own
  words rather than leaving the absence of a `$` line to be noticed.
- **A value that is not valid UTF-8 once decoded is never printed as text.**
  `tls.crt` above is the ordinary case for that key — a certificate is DER
  bytes, not a string — and printing arbitrary bytes into a terminal is the
  same class of risk invariant 9 exists to close, worse here because a
  Secret is exactly the content most likely to be adversarial-shaped by
  accident. The reveal names the byte count and says plainly that it is not
  shown as text; it does not attempt a lossy decode that would show
  something that was never actually in the Secret.
- **All keys reveal together, not one at a time.** The pane holds a scroll
  offset and nothing else ([widgets.md § 2](widgets.md#2-element--widget)) —
  no per-line selection exists to point `v` at a single key, and adding one
  only for this would be new state carried for one feature. A Secret with
  many keys is the uncommon case; scrolling the modal like any other
  overflowing one is not a new idea this file has to introduce.

### A Secret with no keys

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│  payments/pending-secret                                │
│ RESOURCES          │  logs   describe   ‹ yaml ›   events                    │
│   workloads        │                    ──────                               │
│   network          │apiVersion: v1                                           │
│   storage          │kind: Secret                                             │
│   config           │metadata:                                                │
│   cluster          │  name: pending-secret                                   │
│ ANALYSIS           │  namespace: payments                                    │
│   capacity      1 ▲│  managedFields: …                                       │
│   certificates  30d│type: Opaque                                             │
│   drain safety     │data: {}                                                 │
│   posture          │                                                         │
│   restarts         │  This Secret holds no keys yet.                         │
│   waste            │                                                         │
│   versions         │                                                         │
│                    │                                                         │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get secret pending-secret -n payments -o yaml --show-managed-fields│
├──────────────────────────────────────────────────────────────────────────────┤
│ [ ] tabs  esc back                                                           │
└──────────────────────────────────────────────────────────────────────────────┘
```

No `metadata.annotations` here — this Secret has none yet, and an empty
section is not drawn any more than an empty `data` map invents keys that
are not there.

`data: {}` is drawn exactly as the API returns it — there is nothing to mask
because there is nothing there — and the one-line explanation under it says
so in a reader's words rather than leaving an empty map to be interpreted.
`v` is absent from the footer, same rule as the multi-container picker not
being offered to a single-container pod: nothing to act on, so no key.

### An object that does not exist, and a very large object

**Does not exist** is the identical state described's has, reached by the
identical first read: [§ An object that does not exist](#an-object-that-does-not-exist)
and the headless sentence below both apply unchanged, because `yaml`'s first
step is the same `k8s::pod()` call.

**Very large** is deliberately *not* bounded the way the log buffer is
([§ The buffer](#the-buffer-2-mb-retained-5000-lines-4096-bytes-per-line)).
A log is a stream with no natural end; one GET of one object is not — its
size is already bounded by what the Kubernetes API server will accept for a
single object in the first place, a ceiling this product does not own and
does not need to police a second time. The pane simply scrolls, the same way
[the Analysis panes that refuse to cap already do](analysis.md#restarts),
holding the whole document once rather than retaining a stream indefinitely.
**This is a deliberate, narrow exception to
[widgets.md § 7](widgets.md#7-text-that-came-from-the-api)'s closing rule —
*"long values are bounded before they are stored, not at draw time"* — and
that section does not currently say so.** Flagged here rather than edited
there, since only `tui-designer` writes `screens/` and `widgets.md` is a
different file: the PM's call whether it gets its own cross-reference.

### Free text that carried control characters — reversed for this one pane

**This file was wrong, and the reversal is measured, not reasoned** (NOTES §
[D198](../NOTES.md#d198--the-two-reversals-the-operator-review-forced-a-secret-keeps-a-second-copy-of-itself-and-the-strip-that-made---yaml-not-the-object-2026-08-31)).
The section this replaces said every control character is stripped here the
same way an event message strips one, silently, no exception — and `\n` is
a control character. Measured against a real ConfigMap:
`kubectl get cm coredns -n kube-system -o yaml` is 33 lines; the same object
through this pane, as this section was written until now, printed **20** —
its 20-line `Corefile` value collapsed onto one, because every newline
inside it had already become a space. A reader who redirected that into a
file and re-applied it would have shipped a different config than the one
running.

**The ruling: on this pane, `\n` and `\t` survive; everything else
`unprintable` refuses still does not.** `ESC`, `U+202E` and `U+200B` are
still stripped, checked again in the same run that found the Corefile bug —
this is not "the yaml pane trusts the cluster now," it is that a newline
and a tab are not the class of thing invariant 9 exists to catch. The class
is characters that *do something* to a terminal instead of printing as
themselves: an escape sequence, a bidi override, a zero-width joiner. A
newline inside a YAML document does exactly what it says — starts a new
line — which is what the document already looked like before k8rs read it.

```
data:
  Corefile: |
    .:53 {
        errors
        health
        ready
    }
```

drawn as four real lines under `Corefile: |`, matching what `kubectl get cm
coredns -o yaml` already shows, not collapsed onto one the way this section
used to specify.

**Read against [describe's own copy of this section](#free-text-that-carried-control-characters),
this narrows it rather than contradicts it — and does not touch it.** Every
row on the describe tab is a cell in a list: one line for a container, one
line for an event, wrapped by width but never carrying a hard line break
of its own, because a `\n` inside an event message would not print as a
second line of that event, it would open a second row that looks like a
second event. The yaml pane is the other case — the payload *is* the
document, `\n` prints as exactly what it is there, and stripping it is what
was wrong. One predicate, `unprintable`, decides what invariant 9 removes
everywhere; what changed here is which surface a newline is judged
*against* — a cell's layout, or a document's own content — and only the
document path reads `\n` and `\t` as printing correctly. `ESC`, `U+202E`
and `U+200B` still have nothing to do with either judgement: they do not
print as themselves in a cell or in a document, so both paths keep
removing them, silently, with nothing marking the cut — the same as any
other free-text field on this build.

### Printed instead of drawn — yaml on the headless surface

Same split, same reused pod-fetch, same `--object`
([§ Printed instead of drawn — describe](#printed-instead-of-drawn--describe-on-the-headless-surface)) —
`--yaml` is the third verb beside it. The payload is the document itself,
whole, on stdout; the teaching line is on stderr.

**`--yaml` also takes `--kind`, defaulting to `pod`.** Every other verb on
this surface only ever reads a pod, so `--object` alone was enough; `--yaml`
is the one that has to say which kind of object it means, because the
Secret masking below has no other caller in this phase and `k8s.rs` freezes
at the end of it — code that ships with no reachable caller can only ever be
unit-tested, never run for real, and this repo's own rule is that something
is run every box. `--kind` resolves through the discovery machinery
`k8s.rs` already built for the browser (`browsable()` /
`ApiResource::from_gvk_with_plural`), so this is not a second, hand-written
notion of what a kind is — invariant 12 still holds. **`--object`'s own
parse does not change**: it stays `[namespace/]name`, one reader shared by
all three verbs, precisely so the kind travels in its own flag instead of a
second parse of `--object` learning to disagree with the first.

**`--kind` takes a bare word, like `secret`** — or, when one word names two
different things this cluster serves, that word followed by a dot and one
more part naming which. This is `kubectl`'s own spelling, the one a reader
already knows if they have ever had to use it. A bare word works whenever it
names only one thing; the dotted form is for the rare case below where it
does not.

The default case is unchanged from every earlier example in this file, other
than one flag — `--show-managed-fields`, kept on every yaml-tab teaching
line for the same reason it is on the drawn pane's: `kubectl` hides
`managedFields` from `-o yaml` by default and k8rs does not, so the line
k8rs prints has to say the one thing that makes it produce the same
document:

```
$ kubectl get pod web-7d9f4 -n payments -o yaml --show-managed-fields
apiVersion: v1
kind: Pod
metadata:
  name: web-7d9f4
  namespace: payments
  managedFields:
    - manager: kubelet
      operation: Update
      …
…
```

(shown short for the page — nothing here is cut by k8rs; the real object,
`managedFields` included, continues to its real end.) A multi-line value
anywhere in this document — a ConfigMap's `Corefile`, a Secret's PEM block
once revealed — prints as the many real lines it is, not squashed onto one;
[the drawn pane's own reversal above](#free-text-that-carried-control-characters--reversed-for-this-one-pane)
is one predicate reading one surface, so stdout gets the same fix the pane
does, not a second copy of it.

**Naming a kind is what makes the Secret masking above provable rather than
merely written.** `k8rs --yaml --object payments/db-credentials --kind secret`
is a real command against a real cluster now, and this is what it prints —
the identical masked `data:` **and** `metadata.annotations` the drawn
mockup shows, because both surfaces call the one masking function:

```
$ kubectl get secret db-credentials -n payments -o yaml --show-managed-fields
apiVersion: v1
kind: Secret
metadata:
  name: db-credentials
  namespace: payments
  annotations:
    kubectl.kubernetes.io/last-applied-configuration: <hidden — 612 bytes>
  managedFields:
    …
type: Opaque
data:
  username: <hidden — 8 bytes>
  password: <hidden — 16 bytes>
  tls.crt: <hidden — 1,172 bytes>
```

**There is no `--reveal` flag, and there will not be one on this surface**:
a reveal is a keypress on a drawn pane and Phase 6 has no pane, so `--yaml`
on a Secret redacts unconditionally, with no way to ask for the plaintext
headlessly. This is a ruling, not an oversight, and it holds regardless of
`--kind` making the masked path reachable — reachable is not the same as
revealable.

| Failure | Stream | Exit |
|---|---|---|
| `--object` names no such object of the given kind | stderr: `k8rs: there is no pod named ghost in payments — check the name and the namespace` (the kind's own singular in place of *pod*) — a cluster-scoped kind drops both the namespace clause and the *namespace* word: `k8rs: there is no node named ghost — check the name`, the same rule README's [§ The five rules every screen obeys](README.md#the-five-rules-every-screen-obeys) already states for a namespace shown only where there is one | `2` |
| Cluster unreachable / login expired | stderr: the same `because(...)` sentence `logs_run` prints | `2` |
| The object fetch does not answer inside the timeout | stderr: `k8rs: this cluster has not answered for the pod … in … after … seconds` (kind's own singular in place of *pod*, same as the row above) | `2` |
| The document could not be serialised to YAML | stderr: one sentence naming what failed | `2` |
| stdout write fails, not `BrokenPipe` | stderr: `k8rs: the report could not be written — …` | `2` |
| stdout write fails with `BrokenPipe` (the reader closed the pipe, e.g. `\| head`) | nothing | `0` |
| `--kind` with nothing after it | stderr: `k8rs: --kind needs the name of a kind` + usage — the same three-shapes-of-nothing check `--namespace` and `--context` already get (`fn mistyped`) | `2` |
| The cluster does not serve a kind by that name | stderr: `k8rs: this cluster does not serve a kind named widget — check the spelling` | `2` |
| The kind word names more than one thing the cluster serves and neither spelling was qualified — `events` is the real example: `core/v1` and `events.k8s.io/v1` both serve it, and `browsable()` keeps both, adjacent, because they are different resources | stderr: `k8rs: --kind events matches two things this cluster serves — the original one, and the one events.k8s.io adds. Say which: --kind 'events.' for the original one, or --kind 'events.events.k8s.io' for the other` | `2` |
| Success | stdout: the document | `0` |
