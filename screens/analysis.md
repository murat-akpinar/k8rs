# Screen — Analysis (the reports)

Whole-cluster answers no per-object rule can give, computed when opened. This
is where *risky, wasteful and expiring* live — Alerts keeps only *broken right
now* ([NOTES § D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)).

Six reports, six sidebar entries, five panes: **Versions** is drawn at the foot
of the Certificates pane and still has its own entry
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
| `▲ node-2   6.2 of 8 cpu · 30 of 16 GiB` | `Row::Answer` with a `severity` | lands |
| `  node-1   7.4 of 8 cpu · 11 of 16 GiB` | `Row::Answer`, `severity: None` — a fact that makes no judgement | lands |
| the indented sentence under a row | that row's `detail` | — |
| the `→ ` line under it | that row's `action`; `views.rs` draws the arrow | — |
| `Versions` · `Still counted, from what you can see:` · Posture's opening paragraph | `Row::Prose` | skipped |
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
│   workloads        │  ▲ node-2   6.2 of 8 cpu · 30 of 16 GiB       │
│   network          │      using 3.4 cpu and 12 GiB                 │
│   storage          │      Almost twice the memory is promised as   │
│   config           │      node-2 has. If these pods use what they  │
│   cluster          │      asked for, one of them is killed.        │
│ ANALYSIS           │      → move a workload off, or ask for less   │
│▸  capacity      1 ▲│    node-1   7.4 of 8 cpu · 11 of 16 GiB       │
│   certificates  30d│      using 2.1 cpu and 6 GiB                  │
│   drain safety     │    node-3   1.2 of 8 cpu · 3 of 16 GiB        │
│   posture          │      using 0.4 cpu and 1 GiB                  │
│   waste            │                                               │
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
<usable> GiB`. Under it, when the cluster can answer, `using <cpu> and
<memory>`.

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
▲ node-2   6.2 of 8 cpu · 30 of 16 GiB
    Almost twice the memory is promised as node-2
    has. If these pods use what they asked for, one
    of them is killed.
    → move a workload off, or ask for less
  node-1   7.4 of 8 cpu · 11 of 16 GiB
  node-3   1.2 of 8 cpu · 3 of 16 GiB

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

| The cluster | What the pane draws |
|---|---|
| metrics-server answers | every node row gets its `using …` line, and **nothing names metrics-server**. A dependency that is working is not news |
| no metrics-server | the row above, wording as drawn |
| metrics-server installed but not answering | the same row in the same slot; only the sentence changes — *"metrics-server is installed here but did not answer."* → *"Check that its pods are running."* |
| you may read nodes but not what they are using (a 403 on the metrics API) | the same row again — *"You are not allowed to read what each node is using."* → *"Ask for read access to node metrics."* |
| the node section itself did not run — one namespace only, or no permission to list nodes | **no metrics row at all.** The section is one `NotComputed` and that is the whole of it (rule 7 above). A usage number with nothing to compare it against is [PRIOR-ART § F2](../PRIOR-ART.md#f2--a-number-that-cannot-be-defended)'s number with no denominator |

**Missing capability, missing permission, missing scope: three causes, one
sentence shape, one slot on the screen.** A feature that silently disappears
teaches a beginner the tool is unreliable; four different ways of saying it is
missing teaches them it is arbitrary.

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
│   waste            │      Nothing stops one taking a whole node.   │
│   versions         │                                               │
│                    │                                               │
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
  node-1   2.1 of 8 cpu · 4 of 16 GiB
    using 0.8 cpu and 2 GiB
  node-2   1.9 of 8 cpu · 3 of 16 GiB
    using 0.6 cpu and 2 GiB

Every node has room to spare, and every workload here
has a memory and CPU limit set. Nothing to do.
```

**One node** — a laptop cluster, and the shape that must not look broken:

```
  kind-control-plane   0.9 of 4 cpu · 2 of 7 GiB
    using 0.3 cpu and 1 GiB

  6 workloads have no memory or CPU limit
    Nothing stops one taking a whole node.
```

**A node name longer than the region.** The row wraps at a space and keeps both
halves whole — the name is what you act on and the numbers are the answer, so
neither may be cut:

```
▲ ip-10-0-134-201.eu-west-1.compute.internal
  9.1 of 8 cpu · 30 of 16 GiB
    Almost twice the memory is promised as this node
    has. If these pods use what they asked for, one
    of them is killed.
    → move a workload off, or ask for less
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
forty minutes in.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  If you drained each node, what happens?      │
│ RESOURCES          │                                               │
│   workloads        │  ● node-2 would never finish draining         │
│   network          │      payments/web wants at least 5 copies and │
│   storage          │      has exactly 5. A drain takes one away,   │
│   config           │      so it waits forever for a sixth.         │
│   cluster          │      → run one more copy, or lower the        │
│ ANALYSIS           │        minimum it must keep                   │
│   capacity      1 ▲│  ▲ node-3 has 2 pods nothing would restart    │
│   certificates  30d│      They were started by hand, with no       │
│▸  drain safety     │      Deployment behind them. A drain deletes  │
│   posture          │      them and nothing brings them back.       │
│   waste            │      → save what you need off them first      │
│   versions         │    node-1 is ready to drain — 18 pods move    │
│                    │                                               │
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
- **Nothing is computed here under one namespace**, and this report says so
  more loudly than the others: a partial answer is *"18 pods move, node-1 is
  ok"*, which is a green light for an operation that then hangs on a pod the
  report could not see. One `Row::NotComputed` is the whole pane.
- **Empty is a real and good state:**

```
Every node could be drained right now. Nothing on
this cluster is protected by a rule a drain would
wait on, and nothing on it was started by hand.
```

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
│   cluster          │  ▲ data/pgdata-old is 100 GiB nobody is using │
│ ANALYSIS           │      A disk was reserved for it and no pod    │
│   capacity      1 ▲│      ever mounted it. You are billed anyway.  │
│   certificates  30d│  ▲ 47 pods finished and were never removed    │
│   drain safety     │      They use no CPU or memory — they only    │
│   posture          │      make every pod list longer.              │
│▸  waste            │  ○ 12 replicasets are parked at 0 replicas    │
│   versions         │      Left behind when deployments moved on.   │
│                    │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get svc,endpointslices -A                               │
│  $ kubectl get pvc,replicasets -A                                  │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **Every row on this pane is a `Row::Answer`**, and there is no `Prose` on it
  at all. The per-object rows — a Service that matches no pod, a disk nobody
  mounted — record a destination (`Jump::Object`); the counted rows do not, and
  that is [owed](#what-this-screen-owes-and-what-it-deliberately-leaves-off).
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
a pod, every disk that was reserved is mounted, and
nothing finished is lying around.
```

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
▲ 6 pods finished and were never removed
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
│   cluster          │  ○ /var/run/containerd/containerd.sock        │
│ ANALYSIS           │      Read-only, mounted by 9 pods in          │
│   capacity      1 ▲│      kube-system and monitoring.              │
│   certificates  30d│  ○ /var/lib/kubelet                           │
│   drain safety     │      Read-only, mounted by 3 pods in          │
│▸  posture          │      kube-system.                             │
│   waste            │  ○ /etc/cni/net.d                             │
│   versions         │      Read-only, mounted by 3 pods in          │
│                    │      kube-system.                             │
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
- **A row stands for a set of pods, so it records no destination** —
  `jump: None`, the same state Waste's counted rows are in and the same answer
  owed, below.
- **Scoped, it runs unchanged** — hostPath is a pod field — and the title names
  the namespace: `Pods in payments that can read the node's own filesystem`.
- **Empty, and worth a sentence rather than a blank pane:**

```
Nothing here mounts a path from the node it runs on.
That is rarer than it sounds — most clusters run a
network or storage agent that does.
```

## Certificates and Versions

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  What expires, soonest first                  │
│ RESOURCES          │                                               │
│   workloads        │  ▲ your kubeconfig certificate · 30 days left │
│   network          │      After that, kubectl stops working for    │
│   storage          │      you until someone renews it.             │
│   config           │  ○ the API server certificate has 210 days    │
│   cluster          │  ● 2 kubelets are waiting to be let in        │
│ ANALYSIS           │      Two nodes cannot join the cluster until  │
│   capacity      1 ▲│      someone approves their request.          │
│▸  certificates  30d│                                               │
│   drain safety     │  Versions                                     │
│   posture          │  Control plane 1.34 · 2 of 3 kubelets match   │
│   waste            │  ▲ node-3 runs kubelet 1.30                   │
│   versions         │      Four releases behind the control plane — │
│                    │      one more than Kubernetes supports.       │
├────────────────────┴───────────────────────────────────────────────┤
│  $ kubectl get csr                                                 │
│  $ kubectl version                                                 │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

- **The kubeconfig row is C1**, and it is the one row on this page whose `⏎`
  goes to a finding rather than an object (`Jump::Finding`) — the rule already
  produced it, and the sidebar badge `certificates  30d` is its alerting
  mechanism.
- **The expiry is inside the sentence, not right-aligned in a column.** The
  earlier sketch put `30 days` and `210 days` at the pane's right edge, which
  is a column `analysis.rs` would have had to pad.
- **`2 kubelets are waiting to be let in` is a counted row** — a set, so no
  destination, and it is dropped entirely when the CSR list cannot be read.
- **Versions is drawn at the foot of this pane and keeps its own sidebar
  entry.** One pane, two reports, and the `Report` type says nothing about
  either — which panes exist is this file's ruling.
- **`▲ node-3 runs kubelet 1.30` replaces `1.31 (1) ▲ too far behind`, which
  was wrong.** The supported skew is **three** minor versions behind the
  control plane, not two, so 1.31 against a 1.34 control plane is fine and the
  old drawing flagged a healthy cluster mid-upgrade
  ([NOTES § N4](../NOTES.md#node-rules-n-series), corrected by
  [§ D81](../NOTES.md#d81--the-node-rules-and-the-four-things-a-real-cluster-said-about-them-2026-08-13)).
  1.30 is four behind and is the case N4 exists for.
- **The badge reads `certificates  30d`, with no glyph** — as every mockup on
  this page draws it. It is exactly 20 columns, the whole sidebar
  ([widgets.md § 1](widgets.md#1-the-frame)), so a `▲` would not fit; `capacity`
  only fits its `1 ▲` because the label is four characters shorter. The width is
  not the reason, though. **The rule is that a badge which is a count draws its
  band as a glyph and a badge which is a duration does not**, and it holds for
  every badge on every screen:
  - `● ▲ ○` never rely on colour alone — colour blindness, and copyability
    ([NOTES § Design](../NOTES.md#design)). On a **count** the glyph is not
    emphasis, it is the *unit*: `1` counts nothing, `1 ▲` counts one warning,
    and a reader who copies `capacity  1` out of the terminal has lost what the
    number was of.
  - A **duration** loses nothing. `30d` states the fact the reader acts on, in
    words that survive being copied into a monochrome terminal, so
    `Badge::severity` colours it and adds nothing to it.
  - A plain count with no band draws neither — the `12` in
    [widgets.md § 2](widgets.md#2-element--widget)'s badge list.
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
- **Empty:** *"Nothing here expires soon, and every kubelet matches the control
  plane."* No number is named, because the only threshold this screen has is
  C1's 30 days and it belongs to the row that has one.

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
| **Certificates** | the kubeconfig for C1; a cluster-wide CSR list for the pending-kubelet row | C1 always runs — it reads a file on disk and needs no cluster permission at all. The CSR row is dropped and named; `list certificatesigningrequests` is a cluster-scoped verb most namespaced roles do not have |
| **Versions** | the node list | not computed. The control-plane version is a separate read and stands on its own, so the section shows it and says the kubelet comparison is missing |

- **A report that still works must not be made to look broken**, which is why
  Waste and Posture are in this table saying *runs unchanged*. The instinct to
  grey out the whole Analysis screen under a partial view would hide four
  answers that are completely true.
- **The distinction is not sums versus facts — a count is a sum.** It is
  whether the number is measured against something the reader cannot see.
  `47 pods` is the length of a list they can see, and it is honest at any
  scope. Capacity's promised total is weighed against a node's capacity, so a
  view holding a fraction of the pods makes it come out low and say *fine*.
  Every "not computed" row above needs objects outside the reader's view to be
  right at all ([PRIOR-ART § F2](../PRIOR-ART.md#f2--a-number-that-cannot-be-defended)).

## What this screen owes, and what it deliberately leaves off

- **`— ⏎ to list` is not drawn on any counted row, and that is deliberate.**
  `34 workloads`, `47 pods`, `12 replicasets`, `2 kubelets`, and every Posture
  row stand for a *set* of objects, and `Jump` has a case for one object and a
  case for one finding and none for a set. The rows stay selectable and record
  no destination (`jump: None`); the suffix comes back on every pane in one
  edit when the Waste box answers what `⏎` opens — which needs a cap and an
  overflow row (`and 812 more`), not merely a destination
  ([NOTES § D127](../NOTES.md#d127--the-report-shape-the-test-that-decided-its-fields-and-the-two-panes-it-cannot-express-2026-08-20)).
  A key this page draws is a key that does something, and the help screen lists
  exactly the keys the screen has ([help.md](help.md)).
- **The restart row is not on this page.** Where it lives is Family D's
  designer box, and the Waste heading it was measured against is gone
  ([NOTES § D101](../NOTES.md#d101--a-point-sample-cannot-separate-a-settled-container-from-one-on-a-long-cycle-so-the-count-becomes-a-report-row-2026-08-15)).
- **No key changed.** The footer is the same on every pane, `?` opens the same
  help, and this page adds nothing to the key map
  ([help.md](help.md)) — a sixth report is a sixth sidebar entry, not a sixth
  keystroke.
- **The badge glyph rule is stated here and belongs in
  [widgets.md § 2](widgets.md#2-element--widget)**, beside the `3 ● 7 ▲` ·
  `1 ▲` · `30d` · `12` list that is the only place every badge on every screen
  is written down. It is a designer turn on a file this one did not touch.
- **No live usage in the header, and no percentage in a badge.** Settled in
  [widgets.md § 1a](widgets.md#1a-the-header-row) and not reopened here: the
  `capacity` badge is what replaced it, and it counts nodes.
