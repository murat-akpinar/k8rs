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
