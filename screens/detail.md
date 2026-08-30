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
| **events** | this object's events, newest first | Plain-language reasons: `Unhealthy` reads "the health check failed". |

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
