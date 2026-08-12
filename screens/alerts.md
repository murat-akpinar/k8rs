# Screen — Alerts

The default view. k8rs never opens on a pod list; it opens on what is broken.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│▸ ALERTS     3 ● 7 ▲│                                               │
│ RESOURCES          │  ● payments/web  ·  3 of 5 pods    4 min ago  │
│   workloads        │    Containers exceeded their memory limit and │
│   network          │    were killed by the kernel (OOMKilled)      │
│   storage          │    limit 256Mi · exit 137 · 47 restarts       │
│   config           │    → raise limits.memory, or find the leak    │
│   cluster          │                                               │
│ ANALYSIS           │  ▲ shop/api  ·  2 of 6 pods       12 min ago  │
│   capacity      1 ▲│    Running, but not receiving traffic — the   │
│   certificates  30d│    readiness check is failing                 │
│   drain safety     │    → check the app's /healthz endpoint        │
│   waste            │                                               │
│   versions         │  ▲ infra/node-3                     6 days ago│
│                    │    Set to refuse new pods (cordoned)          │
│                    │    → someone's maintenance window never closed│
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get pods -A --watch                                      │
│ $ kubectl get nodes --watch                                        │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  l logs  ? all keys  q quit    │
└────────────────────────────────────────────────────────────────────┘
```

## What each part is

| Part | Rule |
|---|---|
| Sidebar | `ALERTS` is selected on startup. Counts are **owners**, not pods. |
| Header row | Three zones on one line: cluster vitals left, the name centred, context · connection state · `admin` or `read-only` right. The right zone also carries the namespace scope when it is not cluster-wide, and a warning if the kubeconfig disables TLS verification. It is never truncated ([widgets.md § The header row](widgets.md#1a-the-header-row)). |
| Finding card | four lines at most: what happened · the evidence · what to do. Title bright, evidence dim. Blank line between cards — half the design. |
| Command log | every command k8rs ran, as the user would have typed it. |
| Footer | the keys valid **right now**; `?` opens the full map. |

## The rules this screen obeys

- **One card per owner, never per pod.** `payments/web · 3 of 5 pods`, not
  three cards. A DaemonSet on forty nodes is still one card
  ([NOTES § D3](../NOTES.md#d3--findings-group-by-owner-not-by-pod)).
- **Only what is broken right now.** No "this pod has no limits", no read-only
  hostPath list — those are Analysis rows
  ([NOTES § D2](../NOTES.md#d2--the-dividing-line-broken-now-vs-risky-later)).
- **No number we cannot produce.** Evidence is `limit 256Mi · exit 137 · 47
  restarts` — the memory a container was using at the moment the kernel killed
  it is not retrievable, so it is not shown.
- Ordering: severity, then recency. `●` critical, `▲` warning — symbol *and*
  colour, never colour alone.
- Every string here passes the glossary test: a newcomer reads it without
  looking anything up.

## Empty state

See [states.md](states.md) — an empty Alerts screen says *"nothing is broken
right now"*, and it has to be true, or the whole product is noise.
