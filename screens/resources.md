# Screen — Resources (the browser)

Every kind the cluster serves, including CRDs, with **no per-kind code**. The
sidebar comes from `kube::discovery`; the columns come from the API server's
own `Table` printing — the exact columns `kubectl get` would show.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  deployments          ns: payments            │
│ RESOURCES          │                                               │
│▸  workloads        │  NAME       READY  UP-TO-DATE  AVAILABLE  AGE │
│     deployments  12│  ▸ web      3/5    5           3          12d │
│     statefulsets  3│    api      6/6    6           6          40d │
│     daemonsets    5│    worker   2/2    2           2          8d  │
│     pods         84│    cron-sync 1/1   1           1          3d  │
│     jobs          7│                                               │
│   network          │  ● web has 3 pods with findings — ⏎ to see    │
│   storage          │                                               │
│   config           │                                               │
│   cluster          │                                               │
│ ANALYSIS           │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get deployments -n payments                              │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ details  s scale  r restart  ctrl-d delete  / filter    │
└────────────────────────────────────────────────────────────────────┘
```

## Why there is no column list in our code

- `kube::discovery` enumerates what this cluster actually serves — built-ins
  and CRDs alike. The sidebar is generated, never hard-coded.
- Each list is fetched with
  `Accept: application/json;as=Table;g=meta.k8s.io;v=v1,application/json`.
  The API server computes the columns. A CRD therefore displays correctly with
  no line written for it.
- **Only the `priority: 0` columns are shown.** The server returns both sets in
  one response: priority 0 is what plain `kubectl get` prints, priority 1 is
  what `-o wide` adds. Pods come back with nine columns, five of them
  priority 1 — without the filter every screen is the wide view.
- Each row's `.object` is a `PartialObjectMetadata`, not the object: metadata
  only, no `spec`, no `status`. Enough to draw the row, and enough for a
  finding to be matched onto it by name/namespace/uid — but a report can never
  be built from a Table row, which is why `analysis.rs` fetches typed lists of
  its own.
- The `,application/json` fallback is **not optional**: aggregated API servers
  may answer `406` to a Table-only Accept header, and a browser that breaks on
  someone's CRD is worse than one with hand-written columns. *(Kept on the
  documented behaviour of aggregated API servers — a bare kind cluster has none
  to test it against, so this one is unverified, not proven.)*
- **Table can be watched, and is still not watched here.** `?watch=true` with
  the Table Accept header returns 200 and streams `Table` objects — but every
  event re-sends the entire column schema: 3086 bytes of `columnDefinitions` to
  carry an 82-byte row. These views watch `watch_metadata` (tiny) to learn
  *that* something changed, then re-fetch the Table, debounced. The mechanism
  was right; "cannot be watched" was not
  ([NOTES § Verified against a real cluster](../NOTES.md#verified-against-a-real-cluster-2026-08-11)).

## Rules

- **Alerts bleed through.** A row whose object has a finding is marked (`●`),
  so the browser never disagrees with the Alerts view.
- Only Pods and Nodes are watched permanently. Opening this view starts a
  watch; closing it stops one. Forty permanent streams is the problem this
  architecture exists to avoid.
- Operations live here, on the selected object — see [dialogs.md](dialogs.md).
  Nothing is ever applied to a selection of more than one object.
