# Screen — Analysis (the reports)

Whole-cluster answers no per-object rule can give, computed when opened. This
is where *risky, wasteful and expiring* live — Alerts keeps only *broken right
now* ([NOTES § D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)).

Seven reports, seven sidebar entries, six panes: **Versions** is drawn at the
foot of the Certificates pane and still has its own entry
([NOTES § D127](../NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)).

## How a report is drawn — the grammar every pane on this page obeys

**The frames below are drawn 70 columns wide and 15 body lines tall, the page
convention ([README](README.md#how-to-read-them)). The real numbers at the
80×24 floor are wider and one line taller**, and they are what a row is
designed against:

| At 80×24 | Columns / lines | From |
|---|---|---|
| Content pane, inside its borders | 57 | 80 − 2 borders − 20 sidebar − 1 divider |
| **Report region** | **53** | the pane less a two-column pad each side — the same budget an Alerts card gets ([alerts.md](alerts.md#how-wide-a-card-is-and-how-tall)) |
| Row text, after the two-column band gutter | 51 | the region less the gutter §2 below |
| A row's `detail` and its `→ ` action | 49 | they indent two columns under the row text |
| Action continuation | 47 | two more, under the text after `→ ` |
| Body lines | 16 | 24 − header − 4 command log − footer − 2 body borders ([widgets.md § 1](widgets.md#1-the-frame)) |

### Which variant carries which line

Every line drawn on this page comes from one of five places, and for the three
that are rows the *variant* — not a field — is what says whether the cursor may
land on it
([NOTES § D127](../NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)):

| Drawn | Is | Cursor |
|---|---|---|
| the heading on the pane's first line | `Report::title` — **not a row** | — |
| `▲ node-2   6.2 of 8 cpu · 17.8Gi of 16Gi` | `Row::Answer` with a `severity` | lands |
| `  node-1   7.4 of 8 cpu · 9.8Gi of 16Gi` | `Row::Answer`, `severity: None` — a fact that makes no judgement | lands |
| the indented sentence under a row | that row's `detail` | — |
| the `→ ` line under it | that row's `action`; `views.rs` draws the arrow | — |
| `Versions` · `Still counted, from what you can see:` · Posture's and Restarts' opening paragraphs | `Row::Prose` | skipped |
| `Not checked here. …` followed by `Ask for …` | one `Row::NotComputed { reason, ask_for }` | skipped |
| the value beside a name in the sidebar | `Report::badge` | — |

Eight rules follow from that table, and every pane below obeys all eight.

1. **The band is the first thing on a row, never inside it.** Three rows on
   this page used to carry one mid-line — `9.1 cpu ▲`, `node-2   ● BLOCKS`,
   `1.31 (1) ▲ too far behind` — and a row carries one band and one string, so
   none of them could be built. They are gone. `theme.rs` draws the glyph from
   `severity`; no string on this page contains one.
2. **The gutter is two columns whether or not there is a glyph**, so a banded
   row and a plain one start at the same column and the eye reads a straight
   left edge of names.
3. **Nothing on this page is a column.** `analysis.rs` pads nothing and
   `views.rs` never splits a rendered string back into values, which is
   [PRIOR-ART § F1](../PRIOR-ART.md#f1--sorting)'s single stated cause. Every
   number sits inside a sentence that names it.
4. **Rows wrap; they never clip.** A row is one string, so there is nothing to
   lay out separately and nothing to give way: `ip-10-0-134-201.eu-west-1.compute.internal`
   is 42 columns and simply pushes the rest of its row onto a second line. This
   is the one place the page differs from an Alerts card, where an age is
   right-aligned and the name clips ([alerts.md](alerts.md#how-wide-a-card-is-and-how-tall));
   there are two zones there and one here.
5. **The title is not a row, so it does not scroll.** Whatever a reader has
   scrolled to, the sentence that says what they are looking at — and which
   namespace it covers — is still on the first line.
6. **The title names a namespace only where there is one**, which is
   [README rule 5](README.md#the-five-rules-every-screen-obeys) applied to a
   heading. An unscoped pane says nothing about scope; adding *"across the whole
   cluster"* to every title would make the one title that matters invisible.
7. **One `NotComputed` per section, never two.** When two things are switched
   off at once — no permission *and* no metrics-server — the one that switched
   off more is the one drawn. Two reasons stacked over an empty space is two
   ways out for a reader who can only take one.
8. **A report with nothing to say says it in its own words**, as one
   `Row::Prose`, so `views.rs` carries no per-report empty text. The empty state
   of a report is drawn in the section that owns it, below.

## Capacity

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  What each node promised, and what it has     │
│ RESOURCES          │                                               │
│   workloads        │  ▲ node-2   6.2 of 8 cpu · 17.8Gi of 16Gi     │
│   network          │      using 3.4 cpu and 12.1Gi                 │
│   storage          │      Almost twice the memory is promised as   │
│   config           │      node-2 has. If these pods use what they  │
│   cluster          │      asked for, one of them is killed.        │
│ ANALYSIS           │      → move some pods to another node, or     │
│▸  capacity      1 ▲│        lower what they ask for (their         │
│   certificates  30d│        requests)                              │
│   drain safety     │    node-1   7.4 of 8 cpu · 9.8Gi of 16Gi      │
│   posture          │      using 2.1 cpu and 6.4Gi                  │
│   restarts         │    node-3   1.2 of 8 cpu · 3.5Gi of 16Gi      │
│   waste            │      using 0.4 cpu and 950Mi                  │
│   versions         │    34 workloads have no memory or CPU limit   │
│                    │      Nothing stops one taking a whole node.   │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get nodes -o json                                       │
│  $ kubectl top nodes                                               │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

**A node row is one string and reads left to right:**
`name` · two spaces · `<promised> of <usable> cpu` · ` · ` · `<promised> of
<usable>`. Both quantities are spelled by the same formatters every other
number on this screen uses — `cpu_text` and `bytes` — never a fraction the
renderer invents: `cpu_text` prints a trimmed decimal (`6.2`, `0.138`, `8`)
and `bytes` prints the largest binary unit that leaves the value above 1
(`17.8Gi`, `950Mi`). **The two sides of one `of` are not guaranteed to share
a unit** — `bytes` picks a unit per number, not per row — and a lightly
loaded node proves it on a real cluster: `290Mi of 23.1Gi`
(`reports/2026-08-21-family-c-analysis-report-family-review.md` § 9). A row
that assumed one shared unit could not print that node at all. Under it,
when the cluster can answer, `using <cpu> and <memory>` — `using 0.138 cpu
and 1011Mi`, same two formatters, same possible unit split.

- **It is not a table, and that was ruled rather than drafted.** The first
  sketch drew `NODE / PROMISED / USABLE / IN USE`, which showed CPU where
  memory is what kills a workload, left the `IN USE` column empty on every
  cluster without metrics-server, and would have made `analysis.rs` pre-align
  four columns two layers below the renderer
  ([NOTES § D127](../NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)).
- **Both dimensions, on every row, always.** CPU overcommitment stops the next
  pod from starting; memory overcommitment gets a running one killed. A report
  that names one and not the other teaches the wrong lesson about which number
  to watch.
- **One band for both dimensions.** A node over on memory and a node over on
  CPU are the same `▲`: this whole screen is *risky later*, and the kill itself
  is Alerts' rule 2 ([NOTES § D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)).
  The *sentence* under the row is where the two differ, because the consequence
  differs — and that costs no severity nobody has a legend for.
- **A node that is fine carries no band** (`severity: None`), so the one node
  that is not is the only glyph in the pane. It is still a row the cursor lands
  on and `⏎` still opens the node.
- **`34 workloads have no memory or CPU limit` is the old rule 9.** It is a row
  here, not an alarm: a cluster has hundreds of them and none of them is broken.
  It is counted from pods, so it survives every state below in which the node
  section does not.
- **The sidebar badge counts the flagged nodes** and nothing else — `capacity
  1 ▲`, never a percentage ([widgets.md § 1a](widgets.md#1a-the-header-row)).
- **The title never names a namespace, scoped or not — the code is right and
  rule 6 needed a carve-out, not this pane.** Rule 6 as first written implied
  every scoped report should say so in its title; Waste and Posture earn that
  because they keep computing under a scope and the namespace has to appear
  *somewhere* on the pane. Capacity does not compute anything under a scope —
  [§ below](#capacity-when-you-can-only-see-one-namespace) is one
  `Row::NotComputed`, and that row's own first sentence already names the
  namespace (*"you can only see payments"*). A title that repeated it would be
  the second copy of one fact this project keeps paying for. **Rule 6 now
  reads**: a title names its namespace only in a report that keeps answering
  under one — Waste, Posture. A report whose scoped state is one
  `Row::NotComputed` leaves the naming to that row's own sentence, because it
  already carries it. Drain safety is the same shape for the same reason.

**Pending, not drawn:** the *parked* node — cordoned, with nothing left on it
that a drain would move. It is a finished drain nobody turned back on, so it is
capacity being paid for and not used rather than an outage, and N2 deliberately
does not raise it on Alerts
([alerts.md](alerts.md#every-count-this-card-can-have)). Note that a node still
running kindnet, kube-proxy or four static pods is *parked*, not busy — a drain
never moves those ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
Where it lands on this report is a Phase 4 decision and no row is designed for
it yet.

### Live usage, and the one place a missing metrics-server is said

Most clusters have no metrics-server ([widgets.md § 1a](widgets.md#1a-the-header-row)),
so this is the ordinary case and not the exception. The `using` lines are
absent and **one** `Row::NotComputed` sits under the node rows, in the place
their answer would have been — drawn here at the real 53-column region:

```
▲ node-2   6.2 of 8 cpu · 17.8Gi of 16Gi
    Almost twice the memory is promised as node-2
    has. If these pods use what they asked for, one
    of them is killed.
    → move some pods to another node, or lower what
      they ask for (their requests)
  node-1   7.4 of 8 cpu · 9.8Gi of 16Gi
  node-3   1.2 of 8 cpu · 3.5Gi of 16Gi

What each node is actually using is not shown. That
number comes from metrics-server, and this cluster
does not have it installed.

Install metrics-server if you want it — the numbers
above are complete without it.

  34 workloads have no memory or CPU limit
    Nothing stops one taking a whole node.
```

**That is the only rendering of a missing metrics-server on this page.** There
is no per-row `—`, no parenthetical hung off a heading, and no sentence inside
a cell; nothing is drawn where nothing was computed, and the one row that says
so is unselectable and carries no band. `$ kubectl top nodes` appears in the
command log only if k8rs actually called it ([invariant 4](../CLAUDE.md)) — the
strip is not a list of the commands a full report would have run.

**Five states reach this slot, and only four of them draw a row** — read off
`src/analysis.rs`'s `live_usage_row`, which is what settles the exact wording
below; a mockup drawn before the field existed guessed at some of it and
guessed wrong on one count (next bullet):

| The cluster | What the pane draws |
|---|---|
| metrics-server answers | every node row gets its `using …` line, and **nothing names metrics-server**. A dependency that is working is not news |
| k8rs has not asked *(every cluster, through the whole of Phase 4 — this is `Metrics: None`, not a fifth wording choice)* | *"What each node is actually using is not shown. That number comes from metrics-server, and k8rs does not read it."* → *"Nothing to ask for — the numbers above are complete without it."* |
| no metrics-server installed | *"What each node is actually using is not shown. That number comes from metrics-server, and this cluster does not have it installed."* → *"Install metrics-server if you want it — the numbers above are complete without it."* |
| metrics-server installed but not answering | *"metrics-server is installed here but did not answer."* → *"Check that its pods are running."* |
| you may read nodes but not what they are using (a 403 on the metrics API) | *"You are not allowed to read what each node is using."* → *"Ask for read access to node metrics."* |
| the node section itself did not run — one namespace only, or no permission to list nodes | **no metrics row at all.** The section is one `NotComputed` and that is the whole of it (rule 7 above). A usage number with nothing to compare it against is [PRIOR-ART § F2](../PRIOR-ART.md#f2--a-number-that-cannot-be-defended)'s number with no denominator |

**The middle two rows are not one sentence with a swapped clause — they are
the only two that carry the leading `"What each node is actually using is not
shown"` line at all.** *Silent* and *Denied* are one sentence each, with no
lead-in. An earlier draft of this table put the lead-in on all four and would
have printed *"…is not shown. You are not allowed to read what each node is
using."* on the Denied row — two sentences that do not agree about whether
anything is shown. The code never wrote that: `NotInstalled` and the `None`
state are the only two answering *what number, and why is it missing*, so
they are the only two that name what is missing before saying why; `Silent`
and `Denied` each answer a narrower question (*is it running* — *am I
allowed*) and get to it directly.

**Missing capability, missing permission, missing scope: three causes, one
sentence shape, one slot on the screen.** A feature that silently disappears
teaches a beginner the tool is unreliable; four different ways of saying it is
missing teaches them it is arbitrary.

**One state this slot never explains, on purpose: a node absent from an
otherwise-answering metrics-server.** `using()`'s `None` covers three
different clusters identically — nobody probed, the probe failed outright, and
the probe answered without this one node in it, the last being a machine that
joined between polls. All three draw the same nothing: the node's row keeps
its promised/usable line and simply has no `using …` paragraph under it, and
nothing on the pane says why *that one node* is missing it. This is
deliberate and not a sixth state to add here — a per-node reason would be a
second rendering of "metrics-server did not answer for this", and rule 7's
whole point is that this page says a missing dependency exactly once. A
reader who notices one node with no usage line and the others with one is
seeing a stale metrics response, and the fix is the same one this table
already gives for *Silent*.

### Capacity when you can only see one namespace

The promised number adds up every pod's requests on a node. A view scoped to
one namespace holds a fraction of them, so every number in it would come out
low — and a low number here does not read as *missing*, it reads as *fine*,
which is the one wrong answer this report exists to prevent
([NOTES § D43](../NOTES.md#d43--n2-has-no-clock-and-that-makes-a-findings-age-optional-2026-08-12)).
So the check does not run, and the report says so where its own answer would
have been ([states.md](states.md#you-can-only-see-some-namespaces)).

```
 nodes 3/3                    ctx: prod-eu · ns: payments · read-only
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  What each node promised, and what it has     │
│ RESOURCES          │                                               │
│   workloads        │  Not checked here. Adding up what a node has  │
│   network          │  promised needs every pod on it, and you can  │
│   storage          │  only see payments — so every number would    │
│   config           │  come out too low.                            │
│   cluster          │                                               │
│ ANALYSIS           │  Ask for cluster-wide read access, or drop    │
│▸  capacity         │  the  --namespace  flag if you set one.       │
│   certificates  30d│                                               │
│   drain safety     │  Still counted, from what you can see:        │
│   posture          │    6 workloads have no memory or CPU limit    │
│   restarts         │      Nothing stops one taking a whole node.   │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get pods -n payments --watch                            │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **The report is not empty and is not an error** — it is a report with one
  section switched off. The limits row still counts, because it is a fact about
  each workload on its own and is complete for everything in scope; the line
  above it says *from what you can see* so the number is never read as
  cluster-wide.
- **The way out names both causes, because the screen cannot tell them apart
  and does not need to.** Scope arrives from `--namespace` or from the 403
  fallback as one field ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)),
  so *"ask for cluster-wide read access"* alone would be wrong advice for the
  admin who typed the flag, and *"drop the flag"* alone would be wrong for the
  user who never passed one. One sentence covering both is shorter than two
  screens.
- **The sidebar badge is blank in this state** (`capacity` with no `1 ▲`), which
  is exactly why this screen has to speak: a badge has room for a number, not
  for a reason ([widgets.md § 1a](widgets.md#1a-the-header-row)).
- **Nothing is drawn where nothing was computed** — no dash, no placeholder
  row, no greyed-out node list. A list of dashes invites the reader to look for
  the one row that does have a number.
- The same wording rule as everywhere else: it names the check, why it could not
  run, and what to ask for. It does not say `403`, `RBAC` or `namespace-scoped
  snapshot`.

### Capacity's remaining states

Drawn at the 53-column region, without the frame around them.

**Nothing to say — every node has room and every workload has limits.** One
`Row::Prose` after the node rows; the pane is never empty, because a cluster
always has nodes:

```
  node-1   2.1 of 8 cpu · 4.2Gi of 16Gi
    using 0.8 cpu and 2.1Gi
  node-2   1.9 of 8 cpu · 3.6Gi of 16Gi
    using 0.6 cpu and 1.9Gi

Every node has room to spare, and every workload here
has a memory and CPU limit set. Nothing to do.
```

**One node** — a laptop cluster, and the shape that must not look broken:

```
  kind-control-plane   0.9 of 4 cpu · 2.1Gi of 7Gi
    using 0.3 cpu and 1.1Gi

  6 workloads have no memory or CPU limit
    Nothing stops one taking a whole node.
```

**A node name longer than the region.** The row wraps at a space and keeps both
halves whole — the name is what you act on and the numbers are the answer, so
neither may be cut:

```
▲ ip-10-0-134-201.eu-west-1.compute.internal
  9.1 of 8 cpu · 17.8Gi of 16Gi
    Almost twice the memory is promised as this node
    has. If these pods use what they asked for, one
    of them is killed.
    → move some pods to another node, or lower what
      they ask for (their requests)
```

**A node whose numbers cannot be read.** `promised` answers `None` when the
node does not say what it has, or a quantity in the sum is written in a way
k8rs cannot parse — and the node stays on the pane rather than vanishing from
it, which is the defect NOTES § D81 already paid for once on a different
report. No band (nothing was measured, so nothing was judged), no action, and
`⏎` still opens the node:

```
  node-4   could not be worked out
    One of the numbers here — what this node has to
    give, or what a pod on it asked for — is written
    in a way k8rs could not read.
```

**Many nodes.** The pane scrolls, and the scrollbar appears only once the
content is taller than the viewport ([widgets.md § 2](widgets.md#2-element--widget)).
**Flagged nodes first, then node name** — the same order Alerts uses and the
order the restart row is specified in ([todo.md](../todo.md)). On a two-hundred
node cluster the alternative puts the one answer this report exists to give
below the fold, and the sidebar badge that says *there is one in here* gives no
way to find it.

**No permission to list nodes at all** — a real RBAC shape, and distinct from
namespace scope: this login may read pods everywhere and nodes nowhere. The
whole node section is one `NotComputed`, the limits row keeps counting, and the
header's left zone is blank rather than guessed, `nodes 3/3` included
([widgets.md § 1a](widgets.md#1a-the-header-row)):

```
What each node promised, and what it has

Not checked. Reading what a node has needs permission
to list nodes, and this login does not have it.

Ask for permission to list nodes across the whole
cluster.

Still counted, from what you can see:
  34 workloads have no memory or CPU limit
    Nothing stops one taking a whole node.
```

**While it is being computed** there is no `Report` at all — no variant carries
a loading line, and none should. Capacity, Versions and Posture are built from
the watches that are already open and appear the instant the pane does; Waste
and Certificates need a fetch first, and until it lands the pane is the loading
state every screen shares ([states.md](states.md#still-loading)).

## Drain safety

The report that pays for itself — admins normally discover a stuck drain
forty minutes in. **This report answers with the flags a normal drain uses,
never with a flag that destroys data**, and that split is now the shape of
the whole pane: a bare `kubectl drain` on a real cluster refuses on three
separate grounds — DaemonSet-managed pods, pods with no controller, and pods
with local storage — and drains that finish *cordon a node and then hang
forever* on a fourth
(`reports/2026-08-21-family-c-analysis-report-family-review.md` §§ 1–2). The
DaemonSet ground is safe to assume away and is said once, below. The other
three each get a row, because assuming any of them away is either a false
*ready* or a real chance of losing data.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  If you drained each node, what happens?      │
│ RESOURCES          │                                               │
│   workloads        │  A drain below assumes --ignore-daemonsets, so│
│   network          │  DaemonSet pods never count as moving.        │
│   storage          │  ● node-3 would never finish draining         │
│   config           │      This node has stopped responding. A drain│
│   cluster          │      cannot confirm a pod is gone until it    │
│ ANALYSIS           │      answers again, so it waits forever.      │
│   capacity      1 ▲│      → check the node itself: is it powered on│
│   certificates  30d│        and reachable?                         │
│▸  drain safety     │  ▲ node-2 has 2 pods nothing would restart    │
│   posture          │      They were started by hand, with no       │
│   restarts         │      Deployment behind them. A drain deletes  │
│   waste            │      them and nothing brings them back.       │
│   versions         │      → save what you need off them first      │
│                    │    node-1 is ready to drain — 18 pods move    │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get pdb -A                                              │
│  $ kubectl get pods -A --watch                                     │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- Every row is a `Row::Answer` and `⏎` opens the node
  (`Jump::Object`) — including `node-1 is ready to drain`, which carries no
  band because there is nothing to judge.
- **`● BLOCKS` is gone.** The band is the row's `severity` and the row's own
  words say what it means: *would never finish draining*. A reader who has not
  met a PodDisruptionBudget learns what one does from the sentence under it,
  which is the whole point of this report ([invariant 14](../CLAUDE.md)).
- **No jargon in the way out either.** *"lower the minimum it must keep"*
  rather than *"relax the disruption budget"* — the object has a name, and the
  reader can find it from the pod the sentence already named.
- **Seven row kinds now, five bands deep, worst first**: `would never finish
  draining` (Critical) → `would drain, but throws away files` (Critical,
  [below](#a-node-that-would-throw-away-files)) → `has N pods nothing would
  restart` (Warn) → `drains, but needs one more flag` (Info,
  [below](#a-node-that-would-throw-away-files)) → `is ready to drain` /
  `needs a moment before it can be checked` / `can't be checked until it is
  ready again` (no band, three kinds sharing it —
  [above](#a-node-that-has-stopped-responding-and-a-node-that-only-says-it-isnt-ready)).
  Two Critical row *kinds* share the top of the sort
  because they are not the same fact ranked twice: one never completes, the
  other completes and costs the reader data they did not agree to lose — see
  [§ Which Critical wins the row's text](#which-critical-wins-the-rows-text-when-a-node-is-both)
  for what happens when one node is both.
- **A node can carry more than one problem, and the row still shows one
  text.** The pattern already established for a blocked node carrying orphan
  pods too — *"the second problem is not dropped because the first one is
  louder"* — extends to every combination below: the highest-band reason
  supplies the row's single line of text, and every other true reason about
  that node is a further paragraph in `detail`, in the same band-order.
- **Nothing is computed here under one namespace**, and this report says so
  more loudly than the others: a partial answer is *"18 pods move, node-1 is
  ok"*, which is a green light for an operation that then hangs on a pod the
  report could not see. [Three different reads can be the one that is
  missing](#three-reasons-this-pane-goes-dark-not-one), and the widest cause
  is the one drawn.
- **Empty is a real and good state, and the DaemonSet assumption still
  holds inside it** — *"could be drained right now"* means *with
  `--ignore-daemonsets`*, the same as every row above it:

```
Every node could be drained right now. Nothing on
this cluster is protected by a rule a drain would
wait on, nothing on it was started by hand, and
nothing on it keeps its own files, on disk or in
memory.
```

- **The third clause used to name only the disk case, and a node that would
  throw away files read as *all clear*.** One clause now covers both
  emptyDir mediums on purpose, not two: if either kind existed anywhere on
  the cluster, that node's own row would already have won the sort (Critical
  for the disk case, Info for the memory-only one — [below](#a-node-that-would-throw-away-files))
  and this sentence would not be the one drawn. *"Keeps its own files"* is
  therefore true either way without saying which medium, the same way *"was
  started by hand"* does not need to say which controller is missing.
- **It says nothing about a node this pane could not finish checking**, on
  purpose. A `Ready: False` node past its grace period gets its own row —
  [above](#a-node-that-has-stopped-responding-and-a-node-that-only-says-it-isnt-ready)
  — and that row's `DrainLine::ready` is `false` like every other no-band
  row here, so this sentence is never drawn while one exists. Naming *"every
  node answered"* as a fourth clause would be a second sentence saying what
  the row's own absence already says.

### The DaemonSet flag, said once — and why it never becomes a row

**`--ignore-daemonsets` is not optional on a real cluster and it deletes
nothing**, so this pane assumes it the way it already assumes `kubectl drain`
means the eviction API and not `--force`-deleting pods by hand. Naming it
once, as the pane's own opening `Row::Prose`, is the choice this section
draws: the other two candidates the brief raised do not fit. **Not the
command log** — the strip only ever shows a command k8rs actually ran
(`screens/analysis.md` § *How a report is drawn*, invariant 4), and this pane
never calls `kubectl drain` at all; printing a command nobody ran would be
the log lying about what k8rs did. **Not a per-node note** — a CNI DaemonSet
and `kube-proxy` run on nearly every node on nearly every cluster, so a
per-row repetition of the same fact would be the loudest, most repeated line
on the busiest pane in the product, for a fact that is true everywhere and
interesting nowhere. Said once, first, it reads like Posture's opening
paragraph: context for every row under it, never repeated by one of them.

**This is a vocabulary decision, not a new field.** `a_drain_would_move`
already excludes DaemonSet-owned pods from every count on this pane — the
framing line explains what that exclusion has meant all along; it does not
change what any row counts.

### A node that has stopped responding, and a node that only says it isn't ready

**Blocker 2.** `nodes.json`'s `k8rs-worker3` sits at `Ready: Unknown`, and
with no PodDisruptionBudget blocking it the pane used to print `k8rs-worker3
is ready to drain — nothing on it would move` — on the same screen where N1's
Alerts card says *"This node has stopped responding — nothing on it can be
trusted until it does"* about the identical object
(`reports/2026-08-21-family-c-analysis-report-family-review.md` § 2). Two
screens, one node, opposite advice, and the mechanical reason a real drain
agrees with N1 and not with the old row: `kubectl drain` cordons the node,
then asks the eviction API to remove each pod and waits for confirmation.
Confirmation comes from the kubelet, and a kubelet that has stopped posting
answers nothing — the evicted pods sit `Terminating` forever and `kubectl
drain` polls until somebody kills it. That is this pane's own headline row
kind, not a new one:

```
● node-3 would never finish draining
    This node has stopped responding. A drain
    cannot confirm a pod is gone until it
    answers again, so it waits forever.
    → check the node itself: is it powered
      on and reachable?
```

- **The row folds into the existing `would never finish draining` band**
  rather than opening a fifth kind: the mechanism differs from a
  PodDisruptionBudget's (a refusal that never resolves, against a wait that
  never confirms) but the answer to *"if you drained this node, what
  happens?"* is the same sentence either way. `detail` is where they differ,
  same as a node that is both blocked and carries an orphan pod today.
- **The detail and the action are not written fresh — they are the shared
  fact N1 already established**, read off the same `conditions[Ready]` check
  N1 makes (unanswered beyond the five-minute grace `NODE_DOWN_GRACE`,
  [NOTES § Node rules](../NOTES.md#node-rules-n-series)), so the two
  screens cannot disagree about *which* nodes this is true of. The action is
  N1's own sentence verbatim — *"check the node itself: is it powered on and
  reachable?"* — so a reader who has already read the Alerts card meets the
  same instruction here, not a second one to reconcile.
- **This needs a shared helper, not a new field.** `NodeSnapshot.conditions`
  already carries `Ready` (confirmed on the corpus,
  `reports/2026-08-21-family-c-corpus-drain-and-capacity.md` § 13.1), so
  nothing is missing from the snapshot. What is missing is a function next to
  `node_stopped_being_ready` that answers the one question this pane needs —
  *unanswered beyond the grace period, yes or no* — exported the way
  `a_drain_would_move` and `node_overcommitted` already are, so N1 and this
  row read one fact and not two that can drift apart
  ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
  **It answers only the *unresponsive* half of N1** — the *answered and said
  no* half (`Ready: False`) is a different row, drawn next, and the reason it
  is different is the whole of this section's second half.
- **A node that is both unresponsive and blocked by a budget still reads
  `would never finish draining`, once**, with the node's own paragraph first
  (nothing about it can be trusted, counters included) and any budget
  reasons appended after — see
  [§ Which Critical wins](#which-critical-wins-the-rows-text-when-a-node-is-both).
- **Cordon and taints were considered and deliberately left alone.** A drain
  cordons the node itself as its first step, so a node that is already
  cordoned drains exactly the same way — `kubectl drain --dry-run=client`'s
  own `already cordoned (dry run)` line on the corpus is the harmless,
  idempotent case, not a new one this pane has to explain. N2 already owns
  *"this node refuses new pods and still has work on it"* on Alerts; drawing
  it again here would be the divergence NOTES § D46 is about. A taint an
  autoscaler placed to empty a node changes nothing about whether draining it
  by hand would also work, so this pane asks the same hypothetical regardless
  of why the node is tainted — the same non-answer Capacity gives the
  *parked* node, [§ Capacity](#capacity) above.

#### `Ready: False`, reversed

**This file used to rule the opposite of what is drawn below, on the
argument that a kubelet that is running and talking can still carry out an
eviction.** The argument does not survive contact with how a drain actually
works: `kubectl drain` cordons the node, asks the eviction API to delete each
pod, and then waits for the **kubelet** to confirm the container is gone —
that confirmation is the one thing a `False` kubelet's own conditions say
nothing about. `KubeletNotReady: container runtime is down` and `PLEG is not
healthy` are kubelets that post status and cannot stop a container, so the
evicted pods sit `Terminating` forever, identically to the `Unknown` case
above; `NetworkPluginNotReady` is a kubelet that can. `conditions[Ready]`
does not carry which of those a `False` node is, so neither *would never
finish draining* nor *is ready to drain* is a defensible answer about one —
the old ruling picked the second and planting `Ready: False` on a real node
proved it wrong twice over: it is also the **ordinary** shape of a broken
node (dead containerd, a full disk, a CNI that never came up), not the rare
one — `Unknown` needs the machine to have left the network entirely
(`reports/2026-08-21-family-c-drain-rows-and-the-two-new-decodes.md` § 6).

**So this pane says what it actually knows: not enough.** It already has a
row kind for exactly that — the no-band row drawn further down
([§ A budget that has not caught up yet](#a-budget-that-has-not-caught-up-yet--rebanded))
carries no glyph because there is nothing to judge yet, and a `False` node
past N1's own grace period is the same shape: not *safe*, not *doomed*, a
fact k8rs cannot finish computing from here.

```
  node-3 can't be checked until it is ready again
    This node says it cannot run pods right now
    — the same thing its Alerts card says. A
    kubelet that is still talking might still
    confirm an eviction, or it might not. k8rs
    cannot tell which from here, so this pane
    will not guess.
    → check the node's Alerts card for what is
      wrong, then look again once it says ready
```

- **No band, and it sorts with the ready and the not-yet-caught-up nodes**,
  never with the two Critical kinds above it — the same reasoning as the
  stale-budget row: nothing here is urgent by k8rs's own account, because
  k8rs's own account is exactly what is missing. `DrainLine::ready` is
  `false` for it all the same, the same distinction the stale-budget row
  already draws between *no band* and *ready*
  ([§ A budget that has not caught up yet](#a-budget-that-has-not-caught-up-yet--rebanded)).
- **The action is not N1's own action, reused** — unlike the `Unknown` row
  above, N1's `False` action ends *"what the kubelet says is wrong is
  above"*, and there is no *above* here: this pane never repeats the
  kubelet's message, the Alerts card does. Pointing at that card instead is
  what keeps the two screens from carrying two different diagnoses of the
  same node — one screen says what is wrong, this one says what it costs a
  drain, and the way out sends the reader to the one that actually knows.
- **Still N1's own fact, read once, through one function.** `not_ready`
  returns `Option<NotReady>` — `Silent(&Finding)` for the `Unknown` row
  above, `SaidNo` for the row here, no payload, because nothing on this row
  quotes N1's finding back. The `False` branch reads N1's finding for
  *whether the node is wrong, and for how long* — the same grace period N1
  itself already applied, never a second one computed here — and reads the
  node's own `Ready` string only to pick which of the two variants to
  return. So this row is picked out of [`crate::rules::analyze`]'s own
  output the same way the `Unknown` row is, and the two screens read one
  finding-per-node fact and not two independent re-readings of
  `conditions[Ready]` that could drift apart. No new snapshot field:
  `NodeSnapshot.conditions` already carries everything this needs.
- **A node that is both `Ready: False` and carries local storage or orphan
  pods still reads `can't be checked until it is ready again`, once**, with
  this paragraph first and the other true facts about the node folded in
  after it, in the same band order the two Critical rows already use — a
  reader who fixes the node is not then surprised by what draining it would
  have cost:

```
  node-3 can't be checked until it is ready again
    This node says it cannot run pods right now
    — the same thing its Alerts card says. A
    kubelet that is still talking might still
    confirm an eviction, or it might not. k8rs
    cannot tell which from here, so this pane
    will not guess.
    2 pods here keep files on this machine's
    own disk — what Kubernetes calls an
    emptyDir volume — and a drain deletes them
    with the pods.
    → check the node's Alerts card for what is
      wrong, then look again once it says ready
```

- **A `PodDisruptionBudget` genuinely at its floor still wins the row —
  this check never reaches a node the budget check already claimed.** A
  budget refuses at the API server, before the kubelet is asked to confirm
  anything, so *would never finish draining* stays true about that node
  whether or not its kubelet is answering; only a node with **no** genuine
  budget block falls through to this check.
- **Inside `NODE_DOWN_GRACE` — the same five minutes N1 itself waits — this
  row does not appear at all.** No N1 finding exists yet for a node whose
  `Ready` condition flipped moments ago, `False` or `Unknown` either one, so
  the node is read as if nothing were wrong: it still gets a local-storage
  or orphan row on its own merits, or *is ready to drain*, exactly as before
  this section existed. A five-second blip does not stall the whole pane —
  the same restraint N1 already shows on Alerts.

### A node that would throw away files

**Blocker 1, second refusal class.** `kubectl drain --dry-run=client`
without `--delete-emptydir-data` refuses on `k8rs-worker`'s pods with local
storage the same way it refuses on DaemonSet pods — but unlike
`--ignore-daemonsets`, typing `--delete-emptydir-data` deletes real data the
first time it runs, so this pane may not assume it the way it assumes the
DaemonSet flag. It gets its own row kind, above the bare-pod row and below
`would never finish draining` — completing is not the same danger as never
completing, but it is a worse danger than *nothing recreates this pod*,
because the reader may not even know there was anything on the pod's own
disk to lose:

```
● node-2 drains, but throws away files on 2 pods
    They keep files on this machine's own disk —
    what Kubernetes calls an emptyDir volume — and
    a drain deletes them with the pods.
    → copy what you need off them first — the
      replacement pods start with an empty disk
```

Singular: `node-4 drains, but throws away files on 1 pod` /
`It keeps files on this machine's own disk …` / `deletes it with the pod` /
`copy what you need off it first — the replacement pod starts with an empty
disk`.

#### One volume kind, two mediums, and only one of them loses anything

**`kubectl drain`'s own filter does not read `medium`, which is the round-two
review's own finding.** `hasLocalStorage` in `kubectl/pkg/drain/filters.go`
checks only `volume.EmptyDir != nil` — `emptyDir: {}` and
`emptyDir: {medium: Memory}` refuse a bare drain identically. `medium`'s two
legal values, off the live API server:

```
$ kubectl explain pod.spec.volumes.emptyDir.medium
    Must be an empty string (default) or Memory.
```

The default (unset, or an empty string) backs the volume with the node's
own disk — the row above, unchanged. `Memory` backs it with a tmpfs:
nothing on the machine's disk, nothing to copy off, and empty again the
moment any container in the pod restarts. **Istio's sidecar injector adds
`istio-envoy` to every meshed pod this way**, so a Critical row reading
*throws away files* over an action reading *copy them off first* would fire
on every node of a meshed cluster, about a volume that never held anything
worth keeping
(`reports/2026-08-21-family-c-drain-rows-and-the-two-new-decodes.md` § 2).
**Two facts, and they do not point the same way: the drain still refuses
without the flag, and nothing is lost.** This pane says both, in two
different rows, because a reader who sees only one of them is either warned
about a loss that will not happen or not warned about a refusal they are
about to hit.

**The field splits in two: `PodSnapshot.local_storage_disk: bool` and
`PodSnapshot.local_storage_memory: bool`**, each read the same way the one
field used to be, off `spec.volumes[].emptyDir`, now split on `medium`. A
pod naming both an unset-medium and a `Memory` volume counts once in each —
the same deliberate non-deduplication the orphan and local-storage counts
already practise on each other.

```
○ node-5 drains, but needs one more flag for 3 pods
    They keep files in memory only — what
    Kubernetes calls an emptyDir volume set to
    use memory — and a bare drain refuses to
    touch them. Nothing is lost: that storage
    empties every time the container restarts
    anyway.
    → add --delete-emptydir-data when you drain
      — there is nothing on these pods to copy
      off first
```

Singular: `node-6 drains, but needs one more flag for 1 pod` /
`It keeps files in memory only …` / `refuses to touch it` / `there is
nothing on this pod to copy off first`.

- **`Info`, not Critical, and it sorts below `has N pods nothing would
  restart`** — [§ Drain safety](#drain-safety)'s band order. Nothing here
  is lost, so ranking it above the orphan row (a real, permanent loss)
  would teach the wrong lesson about which glyph means *act now*; ranking
  it beside *is ready to drain* would hide a refusal the reader is about to
  hit for real.
- **`ready: false`, the same as the local-storage-disk row** — a bare
  `kubectl drain --ignore-daemonsets` genuinely refuses on these pods, so
  this node is not *drainable right now* even though nothing on it is at
  risk. The all-clear sentence [above](#drain-safety) folds this into the
  same *"keeps its own files, on disk or in memory"* clause as the disk
  case, for exactly this reason — one clause, because either medium is
  enough to keep the sentence from being drawn.
- **The action is the one thing the old, undifferentiated row got wrong,
  and is the whole reason this is its own row rather than a note under the
  disk one.** *"Copy what you need off it first"* is advice about a volume
  with nothing to copy; *"add `--delete-emptydir-data`"* is the one thing a
  reader can actually do about it.
- **A node can carry both mediums, and the disk row wins the text** — real
  loss outranks a flag reminder, [§ Drain safety](#drain-safety)'s general
  rule. The memory fact becomes a further paragraph, self-contained the way
  [`local_storage_paragraph`] already is — never *"N more"*, because it is
  its own fact and not a continuation of the disk count:

```
● node-2 drains, but throws away files on 2 pods
    They keep files on this machine's own disk —
    what Kubernetes calls an emptyDir volume —
    and a drain deletes them with the pods.
    1 pod here keeps files in memory only — what
    Kubernetes calls an emptyDir volume set to
    use memory — and a drain needs the same
    extra flag to touch it. Nothing is lost: that
    storage empties every time the container
    restarts anyway.
    → copy what you need off them first — the
      replacement pods start with an empty disk
```

#### A paragraph reads differently depending on whether it is the row's own text

**Item 3 of the round-two review: `local_storage_paragraph` and
`orphan_paragraph` were written self-contained on purpose, and that is
right when a louder reason won the row and wrong under their own** — a node
whose row already says *throws away files on 2 pods* does not need its
detail to say *2 pods here keep files* again; the count is said once, not
twice on adjacent lines. Every paragraph that can appear either as a row's
own detail or folded under a row a louder reason won now has two forms:

| Paragraph | As the row's own detail | Folded under a louder row |
|---|---|---|
| local storage, disk | *"They keep files on this machine's own disk — what Kubernetes calls an emptyDir volume — and a drain deletes them with the pods."* (singular: *"It keeps … and a drain deletes it with the pod."*) | *"2 pods here keep files on this machine's own disk — what Kubernetes calls an emptyDir volume — and a drain deletes them with the pods."* (singular: *"1 pod here keeps … and a drain deletes them with the pod."*) |
| local storage, memory | *"They keep files in memory only — what Kubernetes calls an emptyDir volume set to use memory — and a bare drain refuses to touch them. Nothing is lost: that storage empties every time the container restarts anyway."* (singular: *"It keeps … refuses to touch it. Nothing is lost …"*) | *"2 pods here keep files in memory only — what Kubernetes calls an emptyDir volume set to use memory — and a drain needs the same extra flag to touch them. Nothing is lost: that storage empties every time the container restarts anyway."* (singular: *"1 pod here keeps … needs the same extra flag to touch it. Nothing is lost …"*) |
| orphan | *"They were started by hand, with no Deployment behind them. A drain deletes them and nothing brings them back."* (singular: *"It was started by hand, with no Deployment behind it. A drain deletes it and nothing brings it back."*) | *"9 pods here were started by hand, with no Deployment behind them. A drain deletes them and nothing brings them back."* (singular: *"1 pod here was started by hand, with no Deployment behind it. A drain deletes it and nothing brings it back."*) |

- **Two sentences, one per position, is what the `action` strings already
  do** — *"copy what you need off it first"* under the local-storage row
  itself never restates how many pods; the review's own reason for the
  fix, applied here to `detail` as well.
- **The folded form is the one every row already drew before this turn** —
  `local_storage_paragraph` and `orphan_paragraph`'s existing bodies stay
  the *folded* variant verbatim, with one correction: the singular orphan
  form read *"One pod here …"*, the only paragraph on this page that
  spelled a count as a word instead of a digit — now *"1 pod here …"*, the
  digit convention every other counted row uses
  ([README rule 5](README.md#the-five-rules-every-screen-obeys), invariant
  14).
- **The own-row form is the one already drawn under this section and the
  main overview mockup** — this turn only names it as its own variant
  rather than the paragraph's only shape.

- **Counted over `moving` pods only, the same set the orphan row counts.** A
  DaemonSet pod's or a static pod's own emptyDir is never touched by a
  drain, so it is not this row's concern; a pod already `Terminating` is
  already going. This is the same narrowing [`a_drain_would_move`] already
  performs — no second predicate to keep in sync with it.
- **The way out teaches the difference from the orphan row rather than
  reusing its sentence.** An orphan pod never comes back — *nothing brings
  it back* — where a pod with local storage usually does, behind whatever
  controller owns it; what does not come back is only what was sitting on
  this one machine's disk. *"The replacement pod starts with an empty
  disk"* says exactly that, and is wrong to say about an orphan.
- **A pod can be both an orphan and a local-storage pod, of either
  medium**, and every row's facts are true about it at once — the orphan
  count and the two local-storage counts are three independent tallies
  over the same `moving` list, deliberately not deduplicated against each
  other: three different facts a reader needs, not one fact counted
  thrice.
- **`jump: Some(Jump::Object(node.id))`**, the same as every other row on
  this pane — navigation to the node, never to an operation
  ([`Jump::Object`]'s own doc).

### The three more ways a PodDisruptionBudget blocks a drain

`blocks_a_drain` in `src/analysis.rs` already answers four shapes; the
mockup only ever drew one. **Every blocked row names the budget the reader
would act on, never the workload it protects** — `default/broken-pdb-floor`,
not `payments/web`. A Deployment's pods are owned by a hashed ReplicaSet by
Phase 4, so naming the Deployment would be a name the row cannot actually
resolve to without a second join, and the way out — *raise the floor, or run
one more copy* — is an edit to the budget object, not to the workload.

**At its floor** — exactly as many healthy pods as it demands, so a drain
would take the cluster below the line it promised to hold:

```
● node-2 would never finish draining
    default/broken-pdb-floor keeps at least 2
    copies of the pods it protects, and right
    now exactly 2 are healthy. A drain has to
    take one away, so it waits forever.
    → run one more copy of what it protects, or
      lower the minimum it must keep
```

**Below its floor** — the workload is *already* down, so *"a drain takes one
away"* is false about it and *run one more copy* is the wrong advice:

```
● node-2 would never finish draining
    default/broken-pdb-floor keeps at least 2
    copies of the pods it protects, and right
    now 1 is healthy. It will not let any be
    moved until they are back — a drain would
    wait on pods that are already down.
    → get the pods it protects healthy again
      first, then drain
```

*(This mockup used to read `keeps at least 2 of the pods it protects, and
right now exactly 2` on both examples, drawn before `blocks_a_drain` existed
and never brought back in step with it — `copies` names the unit, `are
healthy` completes the sentence the number is answering. The code's wording
is the one kept.)*

**`SyncFailed`** — the controller could not resolve what the budget points
at, so its counters are not a measurement of anything and a sentence built
from them would be invented:

```
● node-2 would never finish draining
    Kubernetes could not work out how many
    copies of the pods default/broken-pdb-floor
    protects are healthy, so it will not let any
    of them be moved. The numbers on it are not
    a measurement of anything.
    → check what default/broken-pdb-floor
      points at — this happens when it names
      something Kubernetes cannot count copies
      of
```

#### More than one budget blocks the same node, and every one of them is named

**Item 4 of the round-two review: the row used to name the loudest budget
and hide the rest behind a count** — *"2 other rules on this node would stop
the drain too"* sent a reader who fixed that one budget straight into a
second budget whose name appeared nowhere on the pane
(`reports/2026-08-21-family-c-drain-rows-and-the-two-new-decodes.md` § 7).
The loudest budget still supplies the row's own paragraph — the fact a
reader acts on first — and every other blocking budget is now named after
it, capped the way N1's own evidence line already caps a list of owners: up
to two named, then a count (`rules.rs`'s `listed`).

Two budgets total, one other:

```
● node-2 would never finish draining
    default/broken-pdb-floor keeps at least 2
    copies of the pods it protects, and right
    now exactly 2 are healthy. A drain has to
    take one away, so it waits forever.
    default/zzz-pdb-below blocks the drain too.
    → run one more copy of what it protects, or
      lower the minimum it must keep
```

Three budgets, both others named:

```
● node-2 would never finish draining
    Kubernetes could not work out how many
    copies of the pods default/aaa-pdb-syncfailed
    protects are healthy, so it will not let any
    of them be moved. The numbers on it are not
    a measurement of anything.
    default/bbb-pdb-floor and default/zzz-pdb-below
    block the drain too.
    → check what default/aaa-pdb-syncfailed
      points at — this happens when it names
      something Kubernetes cannot count copies
      of
```

Six budgets, where the cap starts to matter:

```
    default/bbb-pdb-floor, default/ccc-pdb-low
    and 3 more block the drain too.
```

- **The cap is the one this file already trusts with a list of names, not a
  new convention.** `listed()` is `rules.rs`'s own helper, already the shape
  N1's evidence line uses to name up to two owners and then a count. Reusing
  it here means this detail line never grows past *"first, second and N
  more"* whatever the cluster's own budget count is — the cap question the
  review raised, answered by the function this project already trusts with
  it, made `pub(crate)` so `analysis.rs` can reach it.
- **Named, not re-explained.** The trailing sentence gives identity, not
  the reason each one blocks — a reader who clears the loudest budget and
  looks again meets the next name and can go read that budget for itself;
  three full paragraphs on one row would answer a question nobody has asked
  yet.
- **Singular reads *blocks*, plural reads *block*** —
  *"default/zzz-pdb-below blocks the drain too."* against *"default/bbb-pdb-floor
  and default/zzz-pdb-below block the drain too."*, the same subject-verb
  agreement every other counted sentence on this page already keeps.
- **`Blocked` needs its name back.** It carried none once the sort moved
  onto the budget list itself (`reports/2026-08-21-family-c-analysis-report-family-review.md`
  § 7); naming the *other* budgets needs it again, or `drain_row` zips
  `relevant[1..]` against `blocked[1..]` instead — either way, a logic
  change, not a new field.

### A budget that has not caught up yet — rebanded

**A real defect the review caught: the fourth shape used to share this same
`●` row and the same `would never finish draining` headline**, with an
action that gave it away — *"look again in a moment"* under the pane's
loudest band. `metadata.generation` ahead of `status.observedGeneration` is
the controller not having reached the budget yet, normally for well under a
second; it is upstream's own `TooManyRequests` refusal with no diagnosis
attached, and it resolves by itself far more often than it needs an
operator. **One shape of that gap never closes**: a budget whose very first
sync failed sits at `generation: 1` / `observedGeneration: 0` forever,
because `failSafe` sets `SyncFailed` without ever advancing the counter —
that budget carries the reason this row cannot see, so it never reaches
here at all; it draws *would never finish draining* below instead
([NOTES § D139](../NOTES.md#d139--phase-4s-close-the-budget-whose-first-sync-failed-and-where-the-other-seven-findings-went-2026-08-22)).
That is not the same class of problem as *"this will never finish"*, and
dressing a self-healing state in the pane's most urgent band teaches a
reader to distrust the band the next time it is genuinely urgent.
**Rebanded to no band at all** — a fact k8rs cannot yet answer, the same
family `node   could not be worked out` already sits in on Capacity — and
reworded so the text does not claim the drain would hang:

```
node-2 needs a moment before it can be checked
    default/broken-pdb-floor was just changed and
    Kubernetes has not finished counting its
    healthy pods — the change is version 2, the
    count is from version 1.
    → wait a few seconds and look again — if it
      never catches up, check that the cluster's
      controller manager is running
```

- **Sorts with the ready nodes, not with a band of its own** — no urgency to
  signal, so no reason to outrank *"is ready to drain"* in the list; the two
  differ only in which sentence a reader sees.
- **Stacks under whichever row wins the text, when it is not the only
  problem on the node.** A node that is genuinely blocked by one budget
  while a second budget's counters are merely stale still reads `would never
  finish draining`, with the genuine block's paragraph first and a trailing
  line — *"and default/broken-pdb-floor's numbers have not caught up yet —
  check again in a moment"* — so the transient fact is not lost, only never
  the loudest thing on the row.

### Which Critical wins the row's text, when a node is both

Two Critical row kinds can each be true of one node: `would never finish
draining` (a budget genuinely at or below its floor, `SyncFailed`, or the
node itself not answering) and `would drain, but throws away files` (a
local-storage pod would be evicted). **`would never finish draining` wins
the text every time**, because it is the more fundamental fact — a drain
that never completes never reaches the point of deleting anything, so the
data-loss warning is true but not yet actionable. Its paragraph stays in
`detail` as a secondary fact so a reader who clears the block is not then
surprised by the second problem the moment it becomes real, the same
guarantee the orphan row already gives a blocked node.

### Three reasons this pane goes dark, not one

`drain_not_computed` already returns three different `Row::NotComputed`s —
the mockup only ever drew the first. **The widest cause wins** (rule 7):

```
Not checked here. Working out whether a drain
finishes needs every pod on every node, and the
rules that say how many copies must stay up — you
can only see payments, and a half-answer here
would call a node safe that is not.

Ask for cluster-wide read access, or drop the
--namespace flag if you set one.
```

```
Not checked. This report answers one question per
node, and this login cannot list the nodes.

Ask for permission to list nodes across the whole
cluster.
```

```
Not checked. Working out whether a drain finishes
needs the rules that say how many copies of a
workload must stay up, and k8rs could not read
them — without them every node would look safe.

Ask for permission to list poddisruptionbudgets
across the whole cluster.
```

### The zero-pods case

*"18 pods move"* was the only count this file ever drew. At zero, the row
says so in words rather than printing a number that reads as an error:

```
  node-1 is ready to drain — nothing on it would move
```

A node running only static pods and DaemonSet pods — a control-plane node is
the ordinary example — is exactly this row: nothing a drain evicts, so
nothing moves, and `0 pods move` would read as a defect rather than as the
answer.

## Waste

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  Things that cost you something for nothing   │
│ RESOURCES          │                                               │
│   workloads        │  ● shop/api-svc matches no pod                │
│   network          │      This Service points at nothing. Anything │
│   storage          │      calling it gets a 503.                   │
│   config           │      → fix its selector, or delete it         │
│   cluster          │  ▲ data/pgdata-old is 128Mi nobody is using   │
│ ANALYSIS           │      A disk was reserved for it and no pod is │
│   capacity      1 ▲│      mounting it. It stays reserved until     │
│   certificates  30d│      somebody deletes it.                     │
│   drain safety     │  ○ 4 pods were removed by a node and remain   │
│   posture          │      Either the node was short, or the pod    │
│   restarts         │      went over its own disk limit (Evicted).  │
│▸  waste            │      → look at one of the pods — its own      │
│   versions         │        message names what ran out             │
│                    │  ○ 47 pods finished and were never removed    │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get svc,endpointslices,pvc,replicasets -A               │
│  $ kubectl get pods -A --watch                                     │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **The size is what was provisioned, spelled by the same `bytes()` every
  other size on this screen uses — `128Mi`, never `100 GiB` invented for a
  mockup.** A claim whose capacity k8rs cannot parse keeps its row and drops
  only the number: `data/pgdata-old is reserved and nobody is using it`.
- **The disk row's sentence does not claim what k8rs cannot prove.** *"No
  pod ever mounted it"* and *"you are billed anyway"* are both stronger than
  the evidence: k8rs sees the claim's whole *life* no more than it watches a
  bill. What it can say is what it read — nobody is mounting it now, and it
  stays reserved until somebody deletes it — and that is the sentence drawn.
- **Item 2 of the round-two review: a StatefulSet's own disks are the
  commonest shape this row takes, and the row used to read as an accusation
  about them.** `kubectl explain
  statefulset.spec.persistentVolumeClaimRetentionPolicy.whenScaled`
  defaults to `Retain`, so scaling a StatefulSet down for the weekend, or
  catching one mid rolling-update, is enough to put its pods' own database
  volumes on this pane. Deleting one is the classic irrecoverable
  Kubernetes mistake, and *technically true, operationally a trap* is not a
  sentence this report may leave standing — the same shape the completed
  row [below](#the-pileup-splits-in-two-one-per-cause) already solves gets
  one more sentence here, on every row of this kind:

```
▲ data/pgdata-old is 128Mi nobody is using
    A disk was reserved for it and no pod is
    mounting it. It stays reserved until
    somebody deletes it. A StatefulSet keeps
    its pods' disks by default, even after it
    is scaled down, so some of this is normal.
```

  It stays `▲` and not `○`, unlike the completed row
  [below](#the-pileup-splits-in-two-one-per-cause): nothing about a
  claim k8rs can see tells it whether the StatefulSet that made this one is
  still around, still scaled down on purpose, or gone — an idle disk with a
  real cost is still worth a look, the caveat only stops the sentence from
  pushing a reader toward the delete key before they have checked.
- **Every per-object row on this pane is a `Row::Answer`**; the counted rows
  (`4 pods were removed`, `47 pods`, `12 replicasets`) are `Row::Answer`s
  too, with no destination
  recorded (`jump: None`) — [owed](#what-this-screen-owes-and-what-it-deliberately-leaves-off).
  **`Row::Prose` does appear on this pane now**, and only as the cap's
  overflow line — [below](#a-pane-that-caps-and-a-pane-that-folds).
- **The Service that matches no pod is first on purpose:** it is the 503 nobody
  can explain. It stays a report row rather than an alert because promoting it
  would cost a permanent Services + EndpointSlices watch, and the watch budget
  is why k8rs is lighter than k9s
  ([NOTES § D9](../NOTES.md#d9--one-rule-added-to-v1-the-rest-recorded-not-built)).
- **The two-column look is gone.** `shop/api-svc     matches no pod` was a
  table with the header left off, and `analysis.rs` pads nothing; the row is
  one sentence beginning with the object's name.
- **`○ 9 pods mount a path from the node` has moved to Posture**, and the
  `Worth knowing (not broken):` heading went with it — there was nothing left
  under it. That heading was the one Family D was told to re-read
  ([NOTES § D101](../NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15));
  it is not on this pane any more, and where the restart row lands is that
  box's answer, not this one's.
- **Empty:**

```
Nothing here is going to waste. Every Service reaches
a pod, every disk that was reserved is mounted, and no
pod — finished or removed by a node — is left lying
around.
```

### The pileup splits in two, one per cause

**Re-opened 2026-08-22: the box asked for *"Evicted and Completed pod
pileups"* and one row shipped, over both at once —
[NOTES § D155](../NOTES.md#d155--a-whole-project-review-found-two-boxes-checked-over-work-their-own-text-does-not-describe-2026-08-22).**
`finished()` — `Succeeded | Failed` — stays exactly what it was and still
decides whether a pod reaches this section at all. Inside that gate, the
discriminator is `status.reason` alone, not the phase: a pod whose reason is
the literal string `Evicted` draws the removed row whether it is `Failed` or
`Succeeded` — `finished()` already narrowed the set once, and a second,
phase-shaped narrowing on top of it would strand a pod that passed the gate
on neither row, which is the one thing this partition may never do. Every pod
whose reason is anything else — `DeadlineExceeded`, `NodeAffinity`,
`Terminated`, `NodeShutdown`, `OutOfcpu`, anything else this report has no
capture of, or no reason at all — stays in the row below, unmoved: this pane
draws a row for a shape it has measured, not one it is guessing at. The two
rows always sum to the count the one row used to draw; no pod lands on both,
and none falls through and lands on neither.

```
○ 4 pods were removed by a node and remain
    Either the node was short, or the pod went over
    its own disk limit (Evicted).
    → look at one of the pods — its own message names
      what ran out
○ 47 pods finished and were never removed
    Kubernetes keeps a few finished Jobs by default,
    so some of this is normal. They use no CPU or
    memory — they only make every pod list longer.
```

Singular: `1 pod was removed by a node and remains` /
`Either the node was short, or the pod went over its own disk limit
(Evicted).` / `look at one of the pods — its own message names what ran
out` (detail and action do not change with the count).

- **`○`, the same band as the completed row — the earlier ruling for `Warn`
  argued from a correlation, not a constraint.** *"This pane's two `Info`
  rows carry no action"* is true of both, but `severity` and `action` are
  independent fields on `Row::Answer`, and the renderer proves it:
  `src/main.rs:560-579` prints `→ {action}` inside the `Row::Answer` arm with
  no reference to `severity` at all. Three reasons carry the reband on their
  own ground, and a fourth explains why the row still leads:
  - **Waste's charter is cost, and this row's cost is the completed row's own
    cost** — an etcd entry and a longer `kubectl get pods`, nothing more. The
    *killing* is what deserved a look, and it already happened, possibly
    months ago, on a node that has likely since recovered; what is left
    behind today is exactly as cheap as what the completed row already calls
    `Info`.
  - **Evicted pods are garbage-collected only once a node's finished-pod
    count passes 12 500**
    ([NOTES § D71](../NOTES.md#d71--nine-rules-three-blockers-and-the-two-that-were-decisions-not-code-2026-08-13)),
    so a `Warn` here would stay lit for good after one bad half-hour,
    clearable only by deleting the very pods that are this pane's own
    evidence. A warning with no way to clear it except destroying what it
    warns about is what makes an alert screen stop being believed.
  - **It is the completed row's own argument, applied to the row that used
    to be exempt from it.** *"`▲` over a fact that is often deliberate
    teaches the wrong lesson the first time a reader chases it and finds
    nothing to fix"* — the completed row's own reason for staying `Info`,
    below. A pod a node removed is exactly that kind of fact: sometimes a
    real problem, sometimes a pod that overran a limit it declared for
    itself and nothing else, and neither this pane nor N3 can tell the two
    apart from here.
  - **Still first of the two, on different ground.** Both rows are `Info`
    now, so *louder first* no longer orders them. This row leads because it
    is the more specific statement — it names a cause, where the completed
    row names the absence of one — and because it is the row that carries an
    action; the completed row carries none.
- **The word this pane names once, in parentheses, is `Evicted`.** NOTES'
  own translation of the term is *"removed by the node because it ran out of
  room"* ([NOTES § Positioning, item 4](../NOTES.md#positioning--lazygit-for-kubernetes-user-2026-08-11),
  invariant 14), and the detail sentence is built from that translation
  first — the API's own word comes after, in parentheses, the shape
  `rules.rs` already uses for every other term this project translates
  (`… (CrashLoopBackOff)`, `… (OOMKilled)`). It has to be said somewhere:
  `kubectl get pods` never prints `Evicted` for this object at all —
  `printPod` overwrites `status.reason` with the container's own terminated
  reason (`Error`, on the capture this row is measured against), so the
  parenthetical here is the only place on the whole screen the word appears.
  Naming it is what lets a reader who already knows the term, or who needs to
  hand this off to a runbook written against it, connect the row to
  something they can search for — both halves of invariant 13, not the one
  this bullet used to keep.
- **Nothing on the pod says which resource or which node, so the row never
  claims to know — and the action no longer pretends another screen does.**
  The API object behind this row carries a message like *"Pod ephemeral
  local storage usage exceeds the total limit of containers 8Mi"* — one
  pod's disk, one moment — and the row is a count over many pods, possibly
  many nodes, so no single sentence here could name either. The action used
  to send the reader to N3, which answers *which node, and whether it is
  memory, disk or process IDs* — but only for a node under pressure right
  now ([alerts.md § N3](alerts.md#n3--a-node-running-low-on-something)), and
  the corpus this row is measured against proves the commoner cause never
  trips that condition at all: a pod's own disk limit, not the node's
  (`reports/2026-08-23-waste-evicted-row-operator-review.md` §§ 2–4). Sending
  the reader to a screen that is silent for the ordinary case is worse than
  sending them nowhere, so the action points at the object itself instead:
  the pod's own message names the exact resource and moment, a field this
  pane does not decode
  ([D155](../NOTES.md#d155--a-whole-project-review-found-two-boxes-checked-over-work-their-own-text-does-not-describe-2026-08-22)
  reopened only the reason string, not the message).
- **The action is the one thing the completed row still does not carry.**
  These pods did not finish; something killed them, and that is worth
  a look. A pod that ran to completion on its own is not.
- **The completed row keeps its own wording, its own `○` and no action** —
  narrowed only in what it now counts: every `Failed` pod that is not this
  one, plus every `Succeeded` one. `kubectl explain
  cronjob.spec.successfulJobsHistoryLimit` defaults to keeping **three**
  finished Jobs *forever*
  (`reports/2026-08-21-family-c-analysis-report-family-review.md` § 11), so
  any cluster running a CronJob carries some of these on purpose — the same
  shape Posture already solved by pairing `Info` with *"Nothing here is
  broken."* `▲` over a fact that is often deliberate teaches the wrong
  lesson the first time a reader chases it and finds nothing to fix. The
  count still matters at scale — a genuine pileup of thousands is still
  worth a look — so the row stays, quieter and honest about the normal case.
- **Both are counted rows, not per-object.** No cap, no `jump`, no per-pod
  name: the pane says how many, not which ones, the same shape the row had
  before the split and the same reason Capacity's node list and every
  Posture row do not cap either
  ([§ A pane that caps, and a pane that folds](#a-pane-that-caps-and-a-pane-that-folds)).
- **The main mockup above scrolls past this point.** At the 80×24 floor the
  Service, the disk and this row leave room for only the completed row's own
  headline; its `detail` and the ReplicaSets row below it are the first
  things a reader scrolls to, the same way a busy Drain safety pane scrolls
  past its own fourth row kind.

### Waste under one namespace — and the number a reader must not misread

Waste **runs unchanged** when the view is scoped, because every input it has is
namespaced. What changes is the title:

```
Things in payments that cost you something for
nothing

● payments/api-svc matches no pod
    This Service points at nothing. Anything calling
    it gets a 503.
    → fix its selector, or delete it
○ 1 pod was removed by a node and remains
○ 6 pods finished and were never removed
```

- **Unscoped, the title is `Things that cost you something for nothing`** and
  says nothing about scope — rule 6 above. The dangerous state is the narrow
  one, so it is the labelled one.
- **The title is not a row, so it cannot scroll away from the number under
  it.** A reader forty rows down still has *in payments* on the first line of
  the pane.
- **No number on this pane is measured against something the reader cannot
  see.** `47 pods` is the length of a list, not a share of a total; scoped, it
  is a shorter list and still exactly true. That is the difference from
  Capacity, whose promised number is a sum weighed against a node's capacity
  and comes out silently low — [PRIOR-ART § F2](../PRIOR-ART.md#f2--a-number-that-cannot-be-defended)'s
  number with no complete denominator is the one this pane never prints.
- **Per-object rows carry their own scope** — `payments/api-svc` says which
  namespace without being told to ([README rule 5](README.md#the-five-rules-every-screen-obeys)).

### A pane that caps, and a pane that folds

**This pane's earlier claim that it draws no `Row::Prose` at all was written
before `analysis.rs` had anything to cap, and the code has since grown a
cap.** A per-*object* row here — a Service, a disk — is unbounded in the
cluster's own object count, so on a large cluster it is cut to **five**, the
most one section can spend out of this pane's sixteen-line body budget
before a per-object list starts crowding out the other three sections
entirely. The sixth-and-on row becomes one `Row::Prose` overflow line:

```
● shop/api-svc matches no pod
    This Service points at nothing. Anything calling
    it gets a 503.
    → fix its selector, or delete it
● shop/legacy-svc matches no pod
    This Service points at nothing. Anything calling
    it gets a 503.
    → fix its selector, or delete it
and 810 more Services match no pod
```

- **Five is not a round number picked for looks — it is read off the pane's
  own body budget**, [§ *How a report is drawn*](#how-a-report-is-drawn--the-grammar-every-pane-on-this-page-obeys)'s
  16 body lines: a per-object row here runs three to four lines (text, one
  or two `detail` lines, sometimes an action), so five is one section's
  worth of the pane before a second section would be pushed off entirely.
- **Per section, not per pane** — a cluster with 812 broken Services still
  shows its orphaned disks in full: one loud section may not starve the
  others.
- **The overflow line is `Row::Prose` and not another `Row::Answer`**,
  because the cursor landing on it would advertise `⏎` over nothing
  openable — not one object, and not a *set* [`Jump`] has a case for either,
  since what it names is the *remainder* of a list and has no identity of
  its own. The counted rows of this pane (`4 pods were removed by a node`,
  `47 pods finished`, `12 replicasets`) are different and stay
  `Row::Answer`s, `jump: None`: each is the report's own complete answer to
  its own question, not a truncated list.
- **The three counted rows do not cap, and neither does Capacity's node list
  or any Posture row**, for the same reason as each other: a counted row is
  the length of a list, one row whatever the number is, never one row per
  object. What grows unboundedly with the cluster is what gets cut; what is
  already an aggregate scrolls (Capacity) or simply keeps counting (Waste's
  three counted rows, every Posture row).

**When every section is unread, the pane folds three `NotComputed`s into
one**, the way [§ Capacity](#capacity)'s namespace scope already folds two
causes into the widest — except here the fold is a *count*, not a width
comparison: this is one ordinary namespaced role with none of the three
cluster-scoped verbs (`services`, `endpointslices`, `persistentvolumeclaims`,
`replicasets`), not a corner case.

```
Not checked. Working out what is going to waste
needs the lists of what this cluster has — its
Services, the addresses behind them, the disk
reservations and the replicasets — and this login
could not read any of them.

Ask for permission to list services, endpointslices,
persistentvolumeclaims and replicasets.
```

- **Only when all three unreadable sections drew nothing but a
  `NotComputed`, and only when there are exactly three of them.** The
  fourth section — the two finished-pod rows — is counted straight off
  pods, which are always in scope, so it can never contribute a fourth
  `NotComputed` to fold; if even one of the three answered (found
  something, or found nothing), that section's real rows stay and the fold
  does not happen, because a pane with one true answer on it is not a pane
  that could not check anything.
- **Three excuses stacked over an empty pane is three ways out for a reader
  who can only take one** — the same reasoning rule 7 gives one report
  section; here it is applied once across the whole pane instead of once
  per section, because Waste is the one report built from more than one
  independently-fetched list.

## Posture

The read-only host mounts. Rule 8 keeps the escalated case — `/`, a container
runtime socket, or anything writable — and everything it leaves is here: a list
to review, not an alarm to answer
([NOTES § D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later) ·
[§ D14](../NOTES.md#d14--three-plan-corrections)).

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  Pods that can read the node's own filesystem │
│ RESOURCES          │                                               │
│   workloads        │  Nothing here is broken. Network, storage and │
│   network          │  metrics agents are supposed to do this — the │
│   storage          │  list says who can, not what to go and fix.   │
│   config           │                                               │
│   cluster          │  ○ /lib/modules                               │
│ ANALYSIS           │      Read-only, mounted by 8 pods in          │
│   capacity      1 ▲│      kube-system.                             │
│   certificates  30d│  ○ /var/lib/kubelet                           │
│   drain safety     │      Read-only, mounted by 3 pods in          │
│▸  posture          │      kube-system.                             │
│   restarts         │  ○ /etc/cni/net.d                             │
│   waste            │      Read-only, mounted by 3 pods in          │
│   versions         │      kube-system.                             │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get pods -A --watch                                     │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **One row per host path, not one per pod.** A DaemonSet that mounts
  `/var/lib/kubelet` on two hundred nodes is one line here, not two hundred
  identical ones. The path is the thing being exposed and the count is how
  widely; that is the review this pane is for.
- **The opening paragraph is a `Row::Prose` and is part of the report**, not a
  caption `views.rs` adds. Without it the pane reads as an accusation, and
  every row on it is something the cluster is supposed to have.
- **`○` on every row, and no badge, ever.** A permanent number beside `posture`
  in the sidebar would nag about a list that is correct — `drain safety` and
  `waste` badge nothing for the same reason
  ([NOTES § D127](../NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)).
- **Each row is a `Row::Answer` with `severity: Some(Info)`, and its detail
  names up to three namespaces, then `and N more`.** Which namespaces can read
  a path is the half of this an operator acts on; a list of every one of them
  is the half that makes the pane unreadable.
- **A row that has a pod outside `kube-system` sorts above every row that
  does not; within each of the two groups the existing key is unchanged.**
  The group boundary is the same shape Capacity's flagged-nodes-first and
  Drain safety's worst-first already use — a priority group ahead of
  everything else, tie-broken inside it — applied here for the first time
  because until now every pod on every row already cleared the check. The
  check itself is narrow: a mounting pod counts only if it runs in the
  namespace `kube-system` as a DaemonSet or a mirror pod — the same test
  `left_by_rule_8` already applies to decide whether a writable mount is
  escalated, read here off every contributor to a row, read-only included.
  Because the namespace half of the check is exact equality, a row naming
  more than one namespace has already left this group — two namespaces are
  only possible once at least one of them is not `kube-system`. **It is a
  real check, not a verdict on the pod**
  ([NOTES § D70](../NOTES.md#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)):
  Rook in `rook-ceph`, Longhorn in `longhorn-system`, Cilium wherever it
  installs, and the whole monitoring class — node-exporter, promtail,
  fluent-bit — are real node agents that fail it too, because none of them
  run in `kube-system`. Within a group, most widely mounted first, then the
  path: how widely a path is exposed is the review this pane is for, and
  the alternative buries it below the fold on the cluster that has most of
  it. **A row leaves the `kube-system` group the moment one contributing pod
  fails the check**, whatever else mounts the same path — not because that
  pod is guilty of anything, but because it is the one thing on the row the
  check cannot clear, and pod count must not bury it under paths the check
  already did clear. A tie is not a coin flip within either group: two
  paths mounted by the same number of pods sort alphabetically, so a
  re-render of an unchanged cluster never reorders them.
- **A row stands for a set of pods, so it records no destination** —
  `jump: None`, the same state Waste's counted rows are in and the same answer
  owed, below.
- **Scoped, it runs unchanged** — hostPath is a pod field — and the title names
  the namespace: `Pods in payments that can read the node's own filesystem`.
- **A writable row is a shape this file never drew.** `○` still — the band
  makes no judgement about this row either, [above](#posture) — and the
  sentence says what the read-only sentence does not need to: that writing
  is possible, and that it is expected here.

```
○ /var/lib/containerd
    Mounted by 6 pods in kube-system, and at least
    one of them can write to it. Kubernetes runs its
    own node agents this way.
```

  One pod, one namespace: `Mounted by 1 pod in kube-system, which can write
  to it. Kubernetes runs its own node agents this way.` — singular through
  the whole sentence, the same care every counted noun on this screen takes.
  **The only writable mounts that reach this pane at all are node
  infrastructure** — `kube-system`, a DaemonSet or a mirror pod — because
  rule 8 already took every other writable mount to Alerts; the *"Kubernetes
  runs its own node agents this way"* clause is what stops this row reading
  like the one rule 8 missed.
- **A row does not "have a pod outside `kube-system`" because a mount
  escalated** — that pod already has rule 8's card on Alerts and
  contributes nothing here ([`left_by_rule_8`], above). It is this instead:
  **at least one pod contributing to the row runs outside `kube-system`, or
  inside it without being a DaemonSet or a mirror pod.** k8rs cannot say
  more than that about the pod itself — it could be a plain workload
  reading a path it has no real reason to, or it could be exactly the kind
  of agent every other row on this pane is, just installed somewhere the
  one check this pane runs does not look
  ([NOTES § D70](../NOTES.md#d70--rule-8-is-narrowed-to-kube-system-and-every-storage-operator-lives-outside-it-2026-08-13)).
  `/var/log`, read by one pod in `default` on the combined
  `tests/fixtures/healthy-hostpath.json` + `nodes.json` +
  `kube-system-pods.json`, is the first shape: nothing rule 8 escalates,
  nothing [D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)
  sends to Alerts, and — before this box — a row indistinguishable from
  `/lib/modules` two lines above it. **Rule 8 is not touched by this box**:
  `left_by_rule_8` still decides who reaches this pane at all; what changes
  is only where a row it already computed lands, and what its sentence says.
- **The opening paragraph stops asserting "nothing here is broken" when at
  least one pod on the pane runs outside `kube-system`.**
  [D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later) still
  keeps a plain read-only hostPath off Alerts — this is not a reversal of
  that, and the row stays `○` / `Info`, [never a badge](#posture) — but a
  pane that opens by saying nothing is broken while holding a row it cannot
  actually vouch for is telling two stories at once. When every pod on the
  pane clears the check, the paragraph is unchanged (wrapped as the mockup
  at the top of this section already shows it):

```
Nothing here is broken. Network, storage and
metrics agents are supposed to do this — the
list says who can, not what to go and fix.
```

  When at least one pod does not, it opens with this instead:

```
Network, storage and metrics agents are
supposed to do this. The top row has a pod
outside kube-system, so k8rs cannot tell
what it is. Nothing is marked broken; it
still says who can, not what to go and fix.
```

  Both keep the pane's reason for existing — *"who can, not what to go and
  fix"* — because this box does not turn Posture into a second Alerts; it
  only stops one sentence from claiming a certainty the check cannot give
  it. **The second wording is written to stay true whichever fraction of
  the pane it is**, on purpose: it names no proportion, because a namespace
  scope that is not `kube-system` will show it on almost every render — an
  ordinary app namespace has no pods running in `kube-system` at all, so
  nothing in it can clear the check, and a Posture pane scoped to `payments`
  is routinely *every* row, not just the top one. The wording stays honest
  about that render too, because it only ever claims *at least one*.

  The frame is unchanged — 70 columns, 20 (sidebar) + 47 (content) plus the
  borders, the [README](README.md#how-to-read-them) split for the 80×24
  floor. Every content line in this file's mockups keeps at least one
  trailing space before the border, so the real wrap budget is one column
  short of the margin's arithmetic: **44** after the 2-space prose margin
  (47 − 2 − 1), **40** after a row detail's 6-space indent (47 − 6 − 1) —
  confirmed by counting every content line of both this and the frame above
  and checking each is 47 wide and ends in a space:

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  Pods that can read the node's own filesystem │
│ RESOURCES          │                                               │
│   workloads        │  Network, storage and metrics agents are      │
│   network          │  supposed to do this. The top row has a pod   │
│   storage          │  outside kube-system, so k8rs cannot tell     │
│   config           │  what it is. Nothing is marked broken; it     │
│   cluster          │  still says who can, not what to go and fix.  │
│ ANALYSIS           │                                               │
│   capacity      1 ▲│  ○ /var/log                                   │
│   certificates  30d│      Read-only, mounted by 1 pod in default — │
│   drain safety     │      outside kube-system, so k8rs cannot tell │
│▸  posture          │      what it is.                              │
│   restarts         │  ○ /lib/modules                               │
│   waste            │      Read-only, mounted by 8 pods in          │
│   versions         │      kube-system.                             │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get pods -A --watch                                     │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

  `/var/log` is the committed fixture's own row, moved from last of
  fourteen to first, next to the same `/lib/modules` line the unmodified
  render already draws — the other twelve rows are unchanged and keep
  scrolling below it exactly as the pane already does, below.

- **The row's own sentence says which kind it is — the reorder alone is
  not legible**, because "this row is near the top" means nothing to a
  reader who does not already know the sort key. Read-only, one pod,
  outside `kube-system`:

  `Read-only, mounted by 1 pod in default — outside kube-system, so k8rs
  cannot tell what it is.`

  Read-only, several pods, at least one of them outside `kube-system`:

  `Read-only, mounted by 3 pods in default and kube-system. At least one
  of them is outside kube-system, so k8rs cannot tell what it is.`

  The same clause holds when nothing mounting the path runs in
  `kube-system` at all — `namespaces` can list only `default` and the
  sentence is unchanged, because *"at least one"* is true whether one pod
  of three fails the check or all three do, and the pane draws no third
  sentence for the difference: the binary this box decided is *has a pod
  outside `kube-system`* or *does not*, not *how many of them are*.

  A writable row's reassurance clause no longer claims more than the code
  checked. **Every writer on this pane runs in `kube-system` as a DaemonSet
  or mirror pod** — [above](#posture), `left_by_rule_8` only lets a
  writable mount through when `node_agent` already held, and rule 8 took
  every other writable mount to Alerts — but the sentence cannot point at
  *that one* the way a single-writer row does, because two DaemonSets can
  write to the same path at once. It names the writers as a group instead,
  then says plainly that the row holds more than them:

  `Mounted by 6 pods in default and kube-system, and at least one of them
  can write to it. The ones that write are in kube-system; not every pod
  here is.`

  **There is no writable, one-pod, outside-`kube-system` sentence, and
  there cannot be one.** A lone writable pod that runs outside
  `kube-system` — or inside it without being a DaemonSet or a mirror pod —
  is escalated by rule 8 before this pane ever sees it, so a row with
  exactly one contributing pod either is the pod the writable clause above
  already describes, or it is on Alerts instead of here: `pods == 1`,
  `writable`, and "has a pod outside `kube-system`" never hold of the same
  row at once.
- **This pane never caps, and it is not an oversight — it is the same rule
  Capacity's node list already follows.** A live cluster with four nodes
  already produces 14 rows for this pane, and a real one runs 30 to 60
  (`reports/2026-08-21-family-c-corpus-drain-and-capacity.md`); on Waste,
  that many rows would be a cap-and-overflow line, because Waste's per-object
  rows grow with the *cluster's object count* and nothing bounds a Service
  count. **Every row here is already an aggregate** — one path, with the pod
  count folded into its own sentence — so growing the cluster grows the
  count inside a row, never the number of rows past what the node fleet
  itself has host paths worth naming. The pane simply scrolls, the way
  Capacity's node list already does:

```
○ /etc/ca-certificates
    Read-only, mounted by 41 pods in kube-system,
    monitoring and 2 more. At least one of them is
    outside kube-system, so k8rs cannot tell what
    it is.
○ /var/lib/kubelet
    Read-only, mounted by 38 pods in kube-system.
○ /etc/cni/net.d
    Read-only, mounted by 38 pods in kube-system.
○ /var/lib/containerd
    Mounted by 12 pods in kube-system, and at least
    one of them can write to it. Kubernetes runs its
    own node agents this way.
    ⋮ (10 more paths, most widely mounted first)
```

- **Empty, and worth a sentence rather than a blank pane:**

```
Nothing here mounts a path from the node it runs on.
That is rarer than it sounds — most clusters run a
network or storage agent that does.
```

## Restarts

**The row [D101](../NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15)
named and left homeless.** Four rules stand down on a container that is
serving right now, so a container that OOMs every thirty minutes or dies on
the nightly batch draws a card for a few minutes after each restart and
nothing the rest of the time. A card cannot say more without lying about
whether it is broken *now* — but a report can print two facts and assert
nothing: how many times, and how long the current run has lasted. This pane
is that report, one row per container.

**Why its own pane, and not a row bent into one of the other five.** Waste's
title is *Things that cost you something for nothing* — nothing here costs
anything, the container is running — and Capacity, Drain safety, Posture and
Certificates each answer one different, unrelated question. Widening any of
their titles to also cover *"has this container been dying"* would blur the
one thing each already answers cleanly, which is the same invariant-14
problem the box was raised to fix, just moved to a different heading. One
report, one question, matches every pane already on this screen.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  Containers that keep restarting              │
│ RESOURCES          │                                               │
│   workloads        │  Every container below is serving right       │
│   network          │  now. A restart count never clears itself     │
│   storage          │  — the second number, how long this run       │
│   config           │  has lasted, is the signal.                   │
│   cluster          │                                               │
│ ANALYSIS           │  ○ payments/worker-7f9c · container api       │
│   capacity      1 ▲│    Restarted 9 times since this pod started.  │
│   certificates  30d│    This run started 6 hours ago.              │
│   drain safety     │                                               │
│   posture          │  ○ shop/api · sidecar container proxy (it     │
│▸  restarts         │  runs beside the app the whole time)          │
│   waste            │    Restarted 4 times since this pod started.  │
│   versions         │    This run started 2 days ago.               │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get pods -A --watch                                     │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **The opening paragraph does not say nothing is broken, and that is a
  correction, not a style choice.** Rule 5's *serving* card is suppressed
  only once a container's current run is older than `NOT_READY_GRACE` (ten
  minutes), and this pane qualifies a container the moment it is serving and
  above the threshold — so for the first ten minutes after every restart the
  two sets overlap, measured on a real cluster: `▲ default/cycler · 2 min
  ago` — *"Container has been restarted 8 times — it is serving now, but
  something keeps killing it"* — on Alerts, at the same moment this pane
  drew `default/cycler` with nothing wrong in its own words. The reader most
  likely to open this pane is the one who just came from that card. The
  paragraph may say what the pane is and which of its two numbers is the
  signal; it may not tell the reader nothing is broken, because a point
  sample cannot know that ([D101](../NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15)).
- **Both numbers, never divided** ([PRIOR-ART § F2](../PRIOR-ART.md#f2--a-number-that-cannot-be-defended),
  D101). `restarts` and the current run's age sit one under the other in
  `detail`, side by side on the row, and are never combined into a rate.
- **The row's identity is `container_fact(c)`, verbatim — never a second
  spelling.** `container api` for the ordinary case; `sidecar container
  proxy (it runs beside the app the whole time)` and `init container migrate
  (the app starts only after this one finishes)` for the other two roles —
  the exact three strings [`rules.rs`'s `container_fact`](../src/rules.rs)
  already produces for every Alerts card that names a container, gloss and
  all. `ContainerRole`'s own doc names a second, differently-worded spelling
  of a role wrong in these terms — *"the init container `istio-proxy` is
  crashlooping" is wrong, not merely unclear* — so this row calls the same
  function rather than re-describing what a sidecar or an init container is,
  and it always calls it, even for the ordinary one-container pod where the
  role never shows: one function, one place the words come from, never a
  conditional over whether the pod happens to have more than one.
  **The gloss does not sit mid-sentence.** *"sidecar container proxy (it runs
  beside the app the whole time) restarted 9 times"* asks a reader to parse a
  parenthetical about when the app starts and then keep going, in one
  breath — so the row is built around the gloss instead of the gloss being
  squeezed into a sentence about restarts: `container_fact`'s string, prefixed
  with the object's name, is the whole of `text`; the count and the age move
  to `detail`, one paragraph each, the same split Capacity already uses to
  keep a raw measurement off its own text line.
- **"since this pod started" is not decoration — it is the qualifier D101
  requires**, because `restarts` resets to 0 on a new pod (a rollout, an
  eviction, a drain). Without it a reader could take the count as the
  container's whole history and misjudge a young pod as calm.
- **The current run's age is a second clock, not the same one, and the
  wording keeps them apart on purpose.** "Since this pod started" answers
  for the *count*, in the first `detail` paragraph; "This run started 6
  hours ago" answers for the *current run*, in the second — which began at
  the last restart, not at the pod's creation. Reusing the word "started"
  for both in one breath is what would make the row misleading, so the two
  paragraphs name what is doing the starting each time — "this pod" against
  "this run" — and the run's own age is drawn from `rules::age`, the one
  ladder every age on every screen already uses
  ([widgets.md § 1b](widgets.md#1b-how-long-ago-it-happened--one-ladder-every-screen)),
  appended after "started" exactly as the header and every Alerts card
  already do.
- **What may not appear: how the run ended.** `ending` and `exit_meaning`
  are private to `rules.rs`
  ([D101](../NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15)),
  and no row here spells `exit 137` or a reason. There is nothing to fix in
  this row's own words — that is what a card is for, and this container has
  none right now.
- **Qualifies at `matches!(state, Running { .. }) && doing_its_job(c) &&
  restarts ≥ RESTARTS_WARN` — two clauses about health, doing two different
  jobs, not one rule reopened as two.** `doing_its_job` is the one reader of
  *is this container healthy*, and this pane never re-derives that question
  for itself — it is the suppressor `restarting_repeatedly`,
  `previous_run_failed` and `out_of_memory` already share, widened to
  `pub(crate)` the same one-keyword way `RESTARTS_WARN` was. But *healthy*
  is not *healthy in a run right now*: `doing_its_job`'s one `Terminated`
  arm answers for an init container that failed and then finished cleanly —
  `healthy-retry`'s `wait-for-db` in the corpus, three failures then `exit
  0` — and that is the health of a run already over, not a current one. The
  `Running` clause is what keeps this pane to a *current* run, because its
  second number is that run's age; a finished init container has no current
  run to put one on, and this pane has nothing to say about a container that
  already did its job and stopped — that is what `doing_its_job` answering
  *yes* means for it, and it is a different question from the one this pane
  asks. A container that is `Running` but failing its readiness check is
  excluded, and the guarantee that it is not excluded into silence is rule
  5's own non-serving branch, named because it was measured and not assumed:
  `restarting_repeatedly` fires for any `Running && !ready` container at or
  above `RESTARTS_WARN`, whatever its role, and never ages out — so the
  exact set this pane declines to draw a row for is a set rule 5 already
  covers permanently. Rule 7's card, *"Running, but not receiving traffic,"*
  is the extra one a *regular* container also gets, not the reason this
  exclusion is safe: `running_but_not_ready` opens with `if c.role !=
  ContainerRole::Regular { return None; }`, so a native sidecar failing the
  identical probe carries no rule 7 card at all — the Istio/Linkerd shape
  the whole role split exists for — and rule 5's branch is what still
  catches it. `RESTARTS_WARN` (3) is reused rather than invented, the same
  number the suppressed rule-5 card already used.
- **A set that qualifies but cannot yet print an age draws no row, and the
  empty sentence is not drawn either.** `state.running.started_at` reads
  `None` for under eight seconds after a restart (D100), and `rules::age`
  itself declines a moment past its future-skew allowance — either way, a
  container is serving and above the threshold with nothing yet for the
  second `detail` paragraph. The pane's empty sentence claims that nothing
  qualifies by serving and count, which would be false here — a container
  above the line is above the line — so this state keeps the opening
  paragraph and draws nothing under it: no row, no claim either way. It
  clears on the next redraw once Kubernetes reports the timestamp, which
  costs nothing on a screen that draws on events
  ([invariant 7](../CLAUDE.md)).
- **`findings` is unread, for its own reason and not Capacity's.** Capacity's
  producer ignores it because N5 never reaches `analyze`'s slice at all; this
  one ignores it on purpose — the pane does not cross-check against what
  Alerts is currently showing, and a container can appear here whether or not
  it also carries a live card. The row's claim is narrower than a card's —
  count and age, nothing about current health — so there is nothing to
  reconcile.
- **Worst first: highest restart count first, and a tie no longer throws
  away the second number to get there.** `restarts` is the pane's subject
  and D101's own *worst*, so it stays primary — but a tie now breaks on the
  younger current run first (the one that started more recently, still
  mid-cycle rather than long settled), then `namespace/pod`, then the
  container name, alphabetically. Alerts sorts severity then recency; this
  pane had the recency already computed one line above the comparator, for
  the row's own second number, and was throwing it away at the tie-break
  instead of reading it.
- **`⏎` jumps to the pod, and the type already has the case.**
  [`Jump::Object`]'s own doc names this exact row as
  its reason to exist — *"the container that keeps dying between its
  restarts"* — so `jump: Some(Jump::Object(pod_id))` on every row here is
  not a new case, it is the case that was already written with this pane in
  mind. Two qualifying containers in one pod jump to the same pod; the
  reader sees both containers from there.
- **No badge, and the reasoning is Posture's, extended rather than
  repeated.** Every row on this pane is `severity: Some(Info)` — a point
  sample cannot tell a container that is still cycling from one that hiccuped
  once and has been solid for a month, so the pane refuses to imply either
  ([D101](../NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15)).
  A badge that is a count draws its band as a glyph
  ([widgets.md § 2](widgets.md#2-element--widget)), and the only band this
  pane could ever offer is `○` — a glyph that says *no judgement* sitting
  beside a number, which teaches nothing a reader can act on and would be
  the first `○` badge on this screen. Worse, the *count of qualifying
  containers* only grows: a settled restart from a node reboot last month
  never leaves the tally until its pod is replaced, so on any cluster with
  real age the badge would read nonzero most of the time — Posture's own
  reason for refusing one ("a permanent number beside `posture` in the
  sidebar would nag about a list that is correct") applies here at least as
  strongly, on a badge with even less reason to move. `drain safety`, `posture` and
  `waste` already badge nothing; `restarts` joins them.
- **No `NotComputed` state.** This report reads only pod data, which is
  already permanently watched and needs no permission Alerts does not
  already have — the same reason Posture has none. A namespace scope
  narrows the list; it never turns the check off.
- **One container is one row, nothing else on the pane changes.** Unlike
  Capacity's single-node laptop cluster, there is no table to collapse and no
  neighbouring column to hide — the mockup above already shows what a second
  row looks like next to a first, and a pane with only one just stops there.
- **This pane scrolls, and does not cap — the earlier cap did not survive a
  real cluster.** Waste's five-row cap is a *per-section* budget: four
  sections share Waste's sixteen lines, and cutting the loudest one is what
  stops it starving the other three. This pane has exactly one section, so
  there is nothing left for an unbounded list to starve — the cap was reused
  from Waste's number without re-deriving Waste's reason, and it broke on a
  one-node kind cluster where three node reboots took the qualifying set
  from 6 to 17: the five slots it would have kept went to five containers
  that had already stopped restarting, and the one still on a live
  ten-minute cycle — the exact container this pane exists for — fell into
  `and 1 more`. `Row::Prose` is not selectable, so a folded row is not one
  keypress away, it is gone from the screen. The pane simply scrolls, the
  way Capacity's node list and every Posture row already do.

### Restarts under one namespace

Runs unchanged, scoped to what is visible — pod data is namespaced like the
rest. The title carries the scope, and so does every row, in its own
`namespace/pod` prefix:

```
Containers in payments that keep restarting

Every container below is serving right now. A
restart count never clears itself — the second
number, how long this run has lasted, is the
signal.

○ payments/worker-7f9c · container api
    Restarted 9 times since this pod started.
    This run started 6 hours ago.
```

- **Per-object rows carry their own scope, and keep the namespace prefix
  even under one** — `payments/worker-7f9c`, unchanged from the unscoped
  pane, the same rule Waste and Posture's own rows already follow
  ([README rule 5](README.md#the-five-rules-every-screen-obeys)).
- **The empty sentence has no row to carry the scope, so it says the
  namespace itself — the same rule the title already follows, on the one
  line that would otherwise quantify over the whole cluster.** Unscoped,
  the sentence claims something about every serving container k8rs can see;
  under `--namespace payments`, or the 403 fallback that fills the same
  field, only `payments` was ever read, and `kube-system/etcd` sitting at
  forty restarts and serving makes the unscoped wording false while the
  title above it says `payments`:

```
Nothing here has restarted enough to matter. Every
container serving right now in payments has
restarted 2 or fewer times since its pod started.
```

### Empty, and nothing qualifies

**One sentence, and it has to stay true across every cluster it can be drawn
on.** Nothing has restarted at all; something has but stayed under the
threshold; or a container is not serving right now — crash-looping and
already carrying a card, or `Running` but failing its readiness check and
already carrying rule 5's own non-serving card, which fires for that shape
regardless of role — so it was never in this pane's set to begin with. The
pane's filter is the qualifying rule above, and the sentence has to
quantify over exactly that and nothing wider, or a cluster in the third
state reads a claim about a container it can see a card for one screen
over:

```
Nothing here has restarted enough to matter. Every
container serving right now has restarted 2 or fewer
times since its pod started.
```

- **"Every container" was too wide, and a not-ready container makes the gap
  visible on top of a crash-looping one.** The pane only ever draws a
  container that is `doing_its_job`, so its empty sentence may only ever
  claim something about *those* — "every container serving right now," not
  "every container running right now" and not "every container." `Running`
  alone is not enough: a container that is `Running` but not `ready` can sit
  at thirteen restarts, already carry rule 5's non-serving card — the one
  that fires on this exact shape for any role and never ages out — and never
  appear in this pane's set at all. "Running right now" would have swept it
  into "has had two or fewer" exactly the way "every container" once did,
  just one exclusion later. Scoping the sentence to the pane's own filter —
  serving, not merely running — is what keeps it true across every cluster,
  not only the ones this box happened to check.
- **`2 or fewer`, digits, not the word — the same rule
  [§ Certificates and Versions](#certificates-and-versions) already states
  for this page: every count here is a digit, and spelling one out is the
  inconsistency invariant 14 exists to catch.** The number is still
  `RESTARTS_WARN - 1`, derived by the producer off the constant it already
  reads for the qualifying test, never retyped — the digit is what changes if
  rule 5's threshold ever moves, not the word.

## Certificates and Versions

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  What expires, soonest first                  │
│ RESOURCES          │                                               │
│   workloads        │  ▲ Your kubeconfig certificate expires in 30  │
│   network          │  days                                         │
│   storage          │      valid until 2026-09-20T00:00:00Z · this  │
│   config           │      is the file on your own machine that     │
│   cluster          │      proves who you are — nothing in the      │
│ ANALYSIS           │      cluster is broken                        │
│   capacity      1 ▲│      → ask whoever gave you access for a new  │
│▸  certificates  30d│        kubeconfig before that date — k8rs     │
│   drain safety     │        cannot renew it, and after it kubectl  │
│   posture          │        stops working for you too              │
│   restarts         │                                               │
│   waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get csr                                                 │
│  $ kubectl version                                                 │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

**The CSR row sits directly under C1 on a real pane** — cut from the frame
above only because C1's own wording already fills it; drawn here at the
real 53-column region, in the exact words the row and its action carry:

```
● 2 kubelets are waiting to be let in
    2 machines cannot join the cluster until
    someone approves their requests.
    → approve each request once you know which
      machine it came from
```

- **The kubeconfig row is C1, drawn in the rule's own words and not a second
  copy of them.** `text` is `Finding::title` verbatim, `detail` is
  `Finding::evidence` verbatim — ISO timestamp included, because that
  evidence line is what the rule already decided a reader needs — and
  `action` is `Finding::action` verbatim. A report and the rule behind it
  saying two different things about one certificate is the divergence
  NOTES § D46 is about, and it is the one row on this page whose `⏎` goes to
  a finding rather than an object (`Jump::Finding`). The sidebar badge
  `certificates  30d` is C1's own countdown and its alerting mechanism.
- **The API server's own certificate has no row, and that is not an
  omission.** The earlier sketch drew `○ the API server certificate has 210
  days`; C2's read landed in Phase 5, but only as a `Session` field spelled
  by `--once` ([once.md § When the API server's own certificate is running
  out](once.md#when-the-api-servers-own-certificate-is-running-out)) — a
  Certificates-pane row needs `analysis::certificates` to grow a third
  source beside `c1_row` and `kubelets_waiting_to_join`, and `analysis.rs`
  froze at Phase 4 close, one phase before this box could reach it. The row
  is not undrawn for lack of a phase to hold it; it is undrawn because the
  one file that could draw it is frozen and nobody has granted the unfreeze
  that would change that
  ([NOTES § D178](../NOTES.md#d178--c3-lands-whole-c2s-row-cannot-be-drawn-in-a-frozen-pane-and-the-twelfth-crate-was-already-compiled-2026-08-28),
  [backlog.md § From the C2/C3 certificate box](../backlog.md#from-the-c2c3-certificate-box-2026-08-28)).
  A row this screen cannot yet draw is not drawn a placeholder value; it is
  simply not drawn, the same rule that keeps a `—` off every other empty
  cell on this page.
- **The expiry is inside the sentence, not right-aligned in a column.** The
  earlier sketch put `30 days` and `210 days` at the pane's right edge, which
  is a column `analysis.rs` would have had to pad.
- **`2 kubelets are waiting to be let in` names machines in digits, not
  words, and says *machines* rather than *nodes*** — a node that has not
  joined yet is not in `ClusterSnapshot::nodes` to be called one, and
  spelling out `Two` is the inconsistency invariant 14 exists to catch: every
  other count on this page is a digit. It is a counted row — a set, so no
  destination — and it is dropped entirely when the CSR list cannot be read.
- **Versions is a second report drawn at the foot of this same pane, and its
  own heading is a `Row::Prose`** — the literal word `Versions`, emitted by
  the Versions producer itself because nothing else can: two reports share
  one pane, `views.rs` draws the pane's own title from the *first* report's
  `title`, and the second report's `title` is never drawn at all. Keeping
  its own sidebar entry despite sharing a pane is this file's ruling, not
  the `Report` type's.

```
Versions
Control plane v1.34.2 · 2 of 3 kubelets match
▲ node-3 runs kubelet v1.30.5
    4 releases behind the control plane, which is
    further back than Kubernetes supports.
    → upgrade the kubelet on this machine —
      Kubernetes supports a kubelet at most 3
      minor versions older than the control plane
```

- **`▲ node-3 runs kubelet v1.30.5` replaces `1.31 (1) ▲ too far behind`,
  which was wrong, and both version strings carry their own `v` because that
  is what `kubectl version` and `kubectl get nodes` print.** The supported
  skew is **three** minor versions behind the control plane, not two, so
  1.31 against a 1.34 control plane is fine and the old drawing flagged a
  healthy cluster mid-upgrade ([NOTES § N4](../NOTES.md#node-rules-n-series),
  corrected by
  [§ D81](../NOTES.md#d81--the-node-rules-and-the-four-things-a-real-cluster-said-about-them-2026-08-13)).
  1.30 is four behind and is the case N4 exists for.
- **The detail names the gap in digits and cites no delta of its own** —
  `4 releases behind the control plane, which is further back than
  Kubernetes supports`, not *"one more than Kubernetes supports"*: the
  window is upstream's `SUPPORTED_SKEW`, a private constant, and a sentence
  that computed *4 − 3 = one more* would be teaching a number this file has
  no business deriving. The **action**, by contrast, is allowed to cite the
  window directly — *"a kubelet at most 3 minor versions older"* — because
  that sentence is upstream's own documented support window, quoted rather
  than subtracted from anything on this row.
- **The badge reads `certificates  30d`, with no glyph — the duration shape
  of the one badge-glyph rule every screen shares**, which now lives in
  [widgets.md § 2](widgets.md#2-element--widget) rather than here: this
  section drew the rule once for this one badge, and it applied to every
  badge on every screen, which is exactly the second copy of one fact this
  project keeps paying for. This is that rule's one worked example, not the
  rule's home. It is also exactly 20 columns, the whole sidebar
  ([widgets.md § 1](widgets.md#1-the-frame)), so a `▲` would not fit even if
  the rule allowed one — `capacity` only fits its `1 ▲` because the label is
  four characters shorter — but the width is not the reason it has none.
  **A test-only defect, named here so the fix lands in the right file:**
  `src/analysis_tests.rs`'s pane printer draws `30d▲` and `out●` — a glyph on
  a duration badge, which is exactly the shape the rule forbids. The rule
  stands as written; the printer is `dev-core`'s to fix, not this file's to
  relax.
- **The expired badge reads `out`, with no digits at all — confirmed rather
  than invented.** `in_days` drops the sign because the *card's sentence*
  already carries direction — *"expired 14 days ago"* — and a badge has no
  sentence beside it: `0d` would read as *expires today* (still valid),
  `14d` would be indistinguishable from fourteen days left, and `-14d` teaches
  a minus sign to a reader this screen is written for
  ([invariant 14](../CLAUDE.md)). `out` is the one spelling that cannot be
  misread in either direction, and it is what the C1 card itself already
  says in its own three columns:

```
▲ certificates  30d          (30 days or fewer left — the ordinary warning)
● certificates  out          (the certificate has already expired)
```

- **And it is C1's band, not the worst row on the pane.** `certificates  30d`
  sits beside a pane whose CSR row is `●`, and that is correct rather than a
  contradiction: the badge is the alerting mechanism for the one finding that
  has no other home, and the sidebar has room for a number and not for a reason
  ([widgets.md § 1a](widgets.md#1a-the-header-row)). What it should read when
  the CSR section could not be checked at all is the Certificates box's, not
  this file's
  ([NOTES § D127](../NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)).
- **Without a node list the version comparison is one `NotComputed`, and the
  control-plane line stays** — it is a separate read that stands on its own.
- **The denominator used to conflate *does not match* with *could not be
  checked*, and that is fixed at the sentence, not the number.** `Control
  plane v1.34.2 · 3 of 4 kubelets match` beside *"could not work out how far
  behind some of these machines are"* reads as two claims about the same
  fourth node — the first counts it as a non-match, the second says nothing
  is known about it. **When every node was measured**, `N of M kubelets
  match` stays exactly as drawn above — the denominator is the measured
  count and carries no ambiguity. **When one or more could not be measured**
  (missing version, unparseable, or on another Kubernetes major — the three
  shapes `kubelet_too_far_behind` does not compare across), the line
  separates the two facts instead of folding an unknown into a
  non-match:

```
Control plane v1.34.2 · 2 kubelets match, 1 could
not be checked
```

  and the one-node cluster gets the same missing case it never had a
  sentence for: `Control plane v1.34.2 · its kubelet could not be checked`.
  This is a code change, not a mockup redraw: `control_plane_line` gains the
  unmeasured count it does not read today, alongside `matching` and `total`.
- **Two empty states, in two reports' own words — not one combined
  sentence.** The pane is two reports and each closes on its own terms:

```
Nothing here expires soon, and no machine is
waiting to be let in.
```

```
Versions
Control plane v1.34.2 · its kubelet is the same
version
```

  or, on a multi-node cluster with nothing to flag, one of three sentences
  depending on what could actually be measured — never a fourth invented to
  cover all three:
  - **Everything measured and every kubelet at the control plane's own
    version:** *"Every machine is running the same version as the control
    plane. Nothing to do."*
  - **Everything measured and some kubelets are behind, but inside the
    supported window:** *"Every machine is inside the window Kubernetes
    supports. Nothing to do."*
  - **Some machine could not be measured at all**, so the pane may not claim
    to know its status: *"Nothing k8rs could measure is outside the window
    Kubernetes supports. It could not work out how far behind some of these
    machines are."*
  - **The control plane's own version could not be compared against
    anything**, which is a different absence from *some kubelet* being
    unmeasured — nothing on the pane was measured, not just one machine:
    *"Nothing here could be measured. The version the control plane
    reported is not written in a way k8rs can compare against, so how far
    behind each machine is could not be worked out."*
  The earlier single sentence — *"Nothing here expires soon, and every
  kubelet matches the control plane"* — was one screen speaking for two
  reports and was never what the code could say once a real cluster could
  be mid-upgrade and still healthy.

## What each report needs, and what it says when it does not have it

Capacity is not the only one built on a cluster-wide read. Every report on this
screen answers a question about the whole cluster, so every one of them has a
state where it cannot. The drawn examples are
[§ Capacity](#capacity-when-you-can-only-see-one-namespace) and
[§ Waste](#waste-under-one-namespace--and-the-number-a-reader-must-not-misread);
the rest take the same shape, in their own words, in their own pane — never a
shared notice ([states.md](states.md#the-second-paragraph-is-the-point-of-this-screen)).

| Report | Needs | Without it |
|---|---|---|
| **Capacity** | every pod on a node, plus the nodes; metrics-server for the `using` line | the promised/usable answer is not computed; the limits row still counts, labelled *from what you can see*. The usage line has [its own five states](#live-usage-and-the-one-place-a-missing-metrics-server-is-said) |
| **Drain safety** | every pod on a node, plus PodDisruptionBudgets across namespaces | not computed. This is the same join N2 and N5 use, and a partial answer here is the worst of the three: *"18 pods move, node-1 is ok"* is a green light for an operation that then hangs on a pod the report could not see |
| **Waste** | Services and EndpointSlices — namespaced, like the rest | **runs unchanged**, scoped to what is visible: a shorter list, never a wrong number. The title says which namespace |
| **Posture** | pod specs — namespaced, and already watched | **runs unchanged**, scoped, title and all, for the same reason Waste does. It needs no permission Alerts does not already have |
| **Restarts** | pod specs — namespaced, and already watched | **runs unchanged**, scoped, title and all, for the same reason Posture does. No `NotComputed` state exists for it |
| **Certificates** | the kubeconfig for C1; a cluster-wide CSR list for the pending-kubelet row | C1 always runs — it reads a file on disk and needs no cluster permission at all. The CSR row is dropped and named; `list certificatesigningrequests` is a cluster-scoped verb most namespaced roles do not have |
| **Versions** | the node list | not computed. The control-plane version is a separate read and stands on its own, so the section shows it and says the kubelet comparison is missing |

- **A report that still works must not be made to look broken**, which is why
  Waste, Posture and Restarts are in this table saying *runs unchanged*. The
  instinct to grey out the whole Analysis screen under a partial view would
  hide answers that are completely true.
- **The distinction is not sums versus facts — a count is a sum.** It is
  whether the number is measured against something the reader cannot see.
  `47 pods` is the length of a list they can see, and it is honest at any
  scope. Capacity's promised total is weighed against a node's capacity, so a
  view holding a fraction of the pods makes it come out low and say *fine*.
  Every "not computed" row above needs objects outside the reader's view to be
  right at all ([PRIOR-ART § F2](../PRIOR-ART.md#f2--a-number-that-cannot-be-defended)).

## What this screen owes, and what it deliberately leaves off

- **`— ⏎ to list` is not drawn on any counted row, and that is still
  deliberate — but the cap-and-overflow mechanism it was waiting on has since
  landed for Waste's per-*object* rows** (`and 810 more Services match no
  pod`,
  [§ A pane that caps, and a pane that folds](#a-pane-that-caps-and-a-pane-that-folds)).
  What remains owed is narrower than it was: `34 workloads`,
  `4 pods were removed by a node`, `47 pods`, `12 replicasets`,
  `2 kubelets`, and every Posture row are *counted* rows,
  not capped lists — each is already the report's one complete answer to its
  own question, and `Jump` still has a case for one object and one for one
  finding and none for a set. The suffix comes back on every counted row in
  one edit, the day `Jump` gets that third case
  ([NOTES § D127](../NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)).
  A key this page draws is a key that does something, and the help screen lists
  exactly the keys the screen has ([help.md](help.md)).
- **The restart row has landed, as its own pane** — [§ Restarts](#restarts) —
  rather than bent into the Waste heading it was originally measured against,
  which is gone
  ([NOTES § D101](../NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15)).
- **No key changed.** The footer is the same on every pane, `?` opens the same
  help, and this page adds nothing to the key map
  ([help.md](help.md)) — a seventh report is a seventh sidebar entry, not a
  seventh keystroke.
- **The badge glyph rule has moved to
  [widgets.md § 2](widgets.md#2-element--widget)**, beside the `3 ● 7 ▲` ·
  `1 ▲` · `30d` · `12` list that is the only place every badge on every screen
  is written down now — this turn made the move the earlier draft of this
  bullet only pointed at.
- **No live usage in the header, and no percentage in a badge.** Settled in
  [widgets.md § 1a](widgets.md#1a-the-header-row) and not reopened here: the
  `capacity` badge is what replaced it, and it counts nodes.
- **`PodSnapshot.local_storage: bool`, the shared helper beside
  `node_stopped_being_ready`, `control_plane_line`'s unmeasured count and
  `finished_pods_left_behind`'s reband — the four items this bullet used to
  list as owed — are landed.** What is owed now is the second round's own
  delta, named here the same way rather than left implicit; this pass
  authorises drawing against every item below, and the drawing is the
  authorisation, the same rule every earlier round followed:
  - **`PodSnapshot.local_storage` splits into `local_storage_disk: bool` and
    `local_storage_memory: bool`**, both still read off `spec.volumes[]`
    holding an `emptyDir` entry, now split on `medium` — `kubectl drain`'s
    own filter reads presence only and refuses on either, but only the
    unset-medium one loses data. No pod is double-counted away: one naming
    both mediums counts once in each. Feeds
    [§ A node that would throw away files](#a-node-that-would-throw-away-files).
  - **The shared helper beside `node_stopped_being_ready` widens to the
    *answered* half of N1 too** — still no new field, `NodeSnapshot.conditions`
    still carries everything this needs — so it answers *has this node gone
    quiet* **and** *has this node said `Ready: False` past the grace period*
    from the one finding-per-node N1 already computes, rather than two
    independent re-readings of `conditions[Ready]` that could drift apart.
    Feeds
    [§ A node that has stopped responding, and a node that only says it
    isn't ready](#a-node-that-has-stopped-responding-and-a-node-that-only-says-it-isnt-ready).
  - **`the_other_problems` gains a `local_memory` parameter** (a pure logic
    change, no new field) — the fourth independent fact this pane can fold
    under whichever reason won a row's text, alongside `local`, `orphans`
    and `stale`.
  - **Every budget beyond the first that blocks a drain gets named, not
    just counted** — `Blocked` needs the budget's `namespace/name` back (or
    `drain_row` zips `relevant[1..]` against `blocked[1..]`, since the name
    is already in `relevant`), and `listed()` (`rules.rs`) becomes
    `pub(crate)` so this row's *"and N more"* cap is the one N1's own
    evidence line on Alerts already uses, not a second convention. See
    [§ The three more ways a PodDisruptionBudget blocks a drain](#the-three-more-ways-a-poddisruptionbudget-blocks-a-drain).
  - **Wording only, no field and no helper**: the all-clear sentence
    ([above](#drain-safety)), the Waste PVC-retention caveat
    ([below](#waste)), and `local_storage_paragraph` / `orphan_paragraph`
    each splitting into an own-row and a folded form, the latter fixing
    `orphan_paragraph`'s singular *"One pod"* to *"1 pod"* on the way
    ([above](#a-paragraph-reads-differently-depending-on-whether-it-is-the-rows-own-text)).
