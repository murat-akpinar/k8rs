# Screen — Analysis (the reports)

Whole-cluster answers no per-object rule can give, computed when opened. This
is where *risky, wasteful and expiring* live — Alerts keeps only *broken right
now* ([NOTES § D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)).

## Capacity

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  What each node promised, and what it has     │
│ RESOURCES          │                                               │
│   workloads        │                                               │
│   network          │  NODE      PROMISED   USABLE       IN USE     │
│   storage          │  node-1    7.4 cpu    8 cpu        2.1 cpu    │
│   config           │  node-2    9.1 cpu ▲  8 cpu        3.4 cpu    │
│   cluster          │  node-3    1.2 cpu    8 cpu        0.4 cpu    │
│ ANALYSIS           │                                               │
│▸  capacity      1 ▲│  node-2 has promised more CPU than it has.    │
│   certificates  30d│  Nothing new can start there.                 │
│   drain safety     │                                               │
│   waste            │  No CPU/memory limit: 34 workloads — ⏎ to list│
│   versions         │  (needs metrics-server for the IN USE column) │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get nodes -o json                                        │
│ $ kubectl top nodes                                                │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

`No CPU/memory limit` is the old rule 9. It is a row here, not an alarm: a
cluster has hundreds of them and none of them is broken.

**Pending, not drawn:** the *parked* node — cordoned, with nothing left on it
that a drain would move. It is a finished drain nobody turned back on, so it is
capacity being paid for and not used rather than an outage, and N2 deliberately
does not raise it on Alerts
([alerts.md](alerts.md#every-count-this-card-can-have)). Note that a node still
running kindnet, kube-proxy or four static pods is *parked*, not busy — a drain
never moves those ([NOTES § D46](../NOTES.md#d46--nine-fields-the-contract-dropped-and-the-drain-that-does-not-drain-2026-08-12)).
Where it lands on this report is a Phase 4 decision and no row is designed for
it yet.

### Capacity when you can only see one namespace

The `PROMISED` column adds up every pod's requests on a node. A view scoped to
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
│   waste            │  No CPU/memory limit: 6 workloads — ⏎ to list │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -n payments --watch                             │
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
- **No `—` in the PROMISED column, because there is no table.** A table of
  dashes invites the reader to look for the one row that does have a number.
  Nothing is drawn where nothing was computed.
- The same wording rule as everywhere else: it names the check, why it could not
  run, and what to ask for. It does not say `403`, `RBAC` or `namespace-scoped
  snapshot`.

## Drain safety

The report that pays for itself — admins normally discover a stuck drain
forty minutes in.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  If you drained each node, what happens?      │
│ RESOURCES          │                                               │
│   workloads        │  node-1   ok        18 pods move              │
│   network          │  node-2   ● BLOCKS  never finishes            │
│   storage          │             payments/web wants at least 5     │
│   config           │             copies and has exactly 5. Draining│
│   cluster          │             would take one away, so it waits  │
│ ANALYSIS           │             forever.                          │
│   capacity      1 ▲│             → run one more copy, or relax the │
│   certificates  30d│               disruption budget first         │
│▸  drain safety     │  node-3   ▲ 2 pods nothing would restart      │
│   waste            │             (started by hand, no Deployment)  │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pdb -A                                               │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

## Waste

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  Things that cost you something for nothing   │
│ RESOURCES          │                                               │
│   workloads        │  ● shop/api-svc     matches no pod            │
│   network          │      This Service points at nothing. Anything │
│   storage          │      calling it gets a 503.                   │
│   config           │  ▲ data/pgdata-old  reserved, unused, 100Gi   │
│   cluster          │  ▲ 47 pods          Evicted / Completed       │
│ ANALYSIS           │  ○ 12 replicasets   parked at 0 replicas      │
│   capacity      1 ▲│                                               │
│   certificates  30d│  Worth knowing (not broken):                  │
│   drain safety     │  ○ 9 pods mount a path from the node          │
│▸  waste            │                                               │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get svc,endpointslices -A                                │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

The Service-with-no-endpoints row is first on purpose: it is the 503 nobody
can explain. It stays a report row rather than an alert because promoting it
would cost a permanent Services + EndpointSlices watch, and the watch budget
is why k8rs is lighter than k9s
([NOTES § D9](../NOTES.md#d9--one-rule-added-to-v1-the-rest-recorded-not-built)).

## Certificates and Versions

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  What expires, soonest first                  │
│ RESOURCES          │                                               │
│   workloads        │  ▲ your kubeconfig certificate    30 days     │
│   network          │      After that, kubectl stops working for    │
│   storage          │      you until it is renewed.                 │
│   config           │  ○ API server certificate         210 days    │
│   cluster          │  ● 2 kubelets waiting to join     pending CSR │
│ ANALYSIS           │      Two nodes cannot join until someone      │
│   capacity      1 ▲│      approves them.                           │
│▸  certificates  30d│                                               │
│   drain safety     │  Versions:  control plane 1.34 · kubelets     │
│   waste            │  1.34 (2) · 1.31 (1) ▲ too far behind         │
│   versions         │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get csr                                                  │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  esc back  ? all keys  q quit                      │
└────────────────────────────────────────────────────────────────────┘
```

A capability that is missing is **stated, never hidden**: with no
metrics-server the IN USE column reads *"needs metrics-server — not installed
in this cluster"*, and with no cert-manager that section says so. A feature
that silently disappears teaches a beginner the tool is unreliable.

**A permission that is missing is the same case, and takes the same treatment**
— a report that cannot be computed from what this user is allowed to read says
which check is off and what it needed, where the answer would have been
([§ Capacity when you can only see one namespace](#capacity-when-you-can-only-see-one-namespace)).
Missing capability, missing permission, missing scope: three causes, one
sentence shape, one slot on the screen.

## What each report needs, and what it says when it does not have it

Capacity is not the only one built on a cluster-wide read. Every report on this
screen answers a question about the whole cluster, so every one of them has a
state where it cannot. The drawn example is
[§ Capacity](#capacity-when-you-can-only-see-one-namespace); the rest take the
same shape, in their own words, in their own pane — never a shared notice
([states.md](states.md#the-second-paragraph-is-the-point-of-this-screen)).

| Report | Needs | Without it |
|---|---|---|
| **Capacity** | every pod on a node, plus the nodes | the promised/usable answer is not computed; the limits row still counts, labelled *from what you can see* |
| **Drain safety** | every pod on a node, plus PodDisruptionBudgets across namespaces | not computed. This is the same join N2 and N5 use, and a partial answer here is the worst of the three: *"18 pods move, node-1 is ok"* is a green light for an operation that then hangs on a pod the report could not see |
| **Waste** | Services and EndpointSlices — namespaced, like the rest | **runs unchanged**, scoped to what is visible. Its rows are per-object facts, not sums, so a partial view gives a shorter list rather than a wrong number. The pane title says which namespace |
| **Certificates** | the kubeconfig for C1; a cluster-wide CSR list for the pending-kubelet row | C1 always runs — it reads a file on disk and needs no cluster permission at all. The CSR row is dropped and named; `list certificatesigningrequests` is a cluster-scoped verb most namespaced roles do not have |
| **Versions** | the node list | not computed. The control-plane version is a separate read and stands on its own, so the section shows it and says the kubelet comparison is missing |

- **A report that still works must not be made to look broken**, which is why
  Waste is in this table saying *runs unchanged*. The instinct to grey out the
  whole Analysis screen under a partial view would hide three answers that are
  completely true.
- **The distinction is sums versus facts.** A sum over objects you cannot all
  see is wrong and silent about it; a list of objects you can see is short and
  honest. Every "not computed" row above is a sum.
