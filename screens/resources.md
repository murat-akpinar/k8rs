# Screen — Resources (the browser)

Every kind the cluster serves, including CRDs, with **no per-kind code**. The
sidebar comes from `kube::discovery`; the columns come from the API server's
own `Table` printing — the exact columns `kubectl get` would show.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│ ALERTS      3 ● 7 ▲│  deployments          ns: payments            │
│ RESOURCES          │                                               │
│▸  workloads        │    NAME      READY  UP-TO-DATE  AVAILABLE  AGE│
│     deployments  12│▸ ● web       3/5    5           3          12d│
│     statefulsets  3│    api       6/6    6           6          40d│
│     daemonsets    5│    worker    2/2    2           2          8d │
│     pods         84│    cron-sync 1/1    1           1          3d │
│     jobs          7│                                               │
│   network          │  ● web has 3 pods with problems — ⏎ to see    │
│   storage          │                                               │
│   config           │                                               │
│   cluster          │                                               │
│ ANALYSIS           │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get deployments -n payments                              │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  ctrl-d delete  / filter       │
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
  what `-o wide` adds. Pods come back with nine columns, **five** of them
  priority 0 — the ones this screen draws — and four priority 1, what `-o wide`
  adds; without the filter every screen is the wide view. Measured off
  `tests/fixtures/table-pods.json`:
  `jq -r '.columnDefinitions[]|"\(.name)|prio=\(.priority)"' tests/fixtures/table-pods.json`
  — Name, Ready, Status, Restarts, Age at priority 0; IP, Node, Nominated Node,
  Readiness Gates at priority 1. Deployments split the same way, 8 columns
  (5 + 3): `tests/fixtures/table-deployments.json`.
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
- **Table can be watched, and is still not watched here — but not because of
  what this file used to say.** `?watch=true` with the Table Accept header
  returns 200 and streams `Table` objects, and `columnDefinitions` is sent
  **once per stream, on the first event, and never again**: measured against
  the same cluster image the old claim was written against, a pods Table watch
  gave one `cols: 9` event at 5 764 bytes and seventeen `cols: 0` events
  averaging 3 062 bytes; a deployments watch gave one `cols: 8` event and ten
  `cols: 0` events. There is no 37× to pay per event.
  **The reason the design stays is a different one.** `kube::runtime::watcher`
  needs `K: Resource + DeserializeOwned`, and a `Table` is neither — watching
  one means a hand-rolled `Client::request_stream` carrying its own
  `resourceVersion` bookkeeping, its own `410 Gone` relist and its own init
  event: three things the metadata path gets from `kube` for free. **It is not
  even the cheaper choice on the wire**: a Table watch event (~3 062 bytes)
  already carries the row's identity, where a metadata event (~2 624 bytes,
  14% smaller) still owes a whole Table re-fetch at ~6 852 bytes per row — on a
  500-row namespace, roughly 3 KB per change watching the Table against 2.6 KB
  plus 3.4 MB re-fetching it. These views watch `watch_metadata` (tiny) to
  learn *that* something changed, then re-fetch the Table, debounced, because
  the engineering the Table path owes is worth more than the bytes it would
  save
  ([NOTES § D154](../NOTES.md#d154--the-browsers-rows-a-37-that-was-one-event-a-floor-measured-from-the-answer-and-a-guard-that-stopped-at-cc-2026-08-22),
  [§ Verified against a real cluster](../NOTES.md#verified-against-a-real-cluster-2026-08-11)).

## Rules

- **Alerts bleed through.** A row whose object has a finding is marked (`●`),
  so the browser never disagrees with the Alerts view.
- **The `ns:` label follows the kind and disappears for the cluster-wide
  ones — and for a namespaced kind with no scope in effect.** The pane title
  reads `deployments` with `ns: payments` beside it for a kind that lives in
  namespaces **and is currently scoped to one** (`--namespace`, or the 403
  fallback, [states.md](states.md#you-can-only-see-some-namespaces)). For
  nodes, persistent volumes and certificate requests the label is **absent** —
  not blank, not `ns: -` — and so is the title just the kind. Whether to draw
  it is two conditions, not one: discovery's own `namespaced` flag, **and**
  whether a namespace scope is currently in effect
  ([invariant 12](../CLAUDE.md) — still one condition per fact, never a list of
  kinds). A namespaced kind browsed with no scope reads the same as a
  cluster-wide one: no label on the title, and the row underneath carries the
  namespace instead — [§ Browsing every namespace](#browsing-every-namespace)
  is that case, and it is the *default* one, not a rare one, until a
  namespace picker exists to set a scope from inside the app. Same rule as the
  identity line
  ([README § the five rules](README.md#the-five-rules-every-screen-obeys)): no
  namespace is shown where there is no namespace.
- Only the Alerts view's inputs are watched permanently — Pods, Nodes and the
  three workload kinds. Opening this view starts a watch; closing it stops one.
  Forty permanent streams is the problem this architecture exists to avoid.
- Operations live here, on the selected object — see [dialogs.md](dialogs.md).
  Nothing is ever applied to a selection of more than one object.

## Browsing every namespace

`Fetch::table(kind, None)` on a namespaced kind lists **every namespace**, and
this is the ordinary state of the browser today, not an edge case: without
`--namespace/-n` and without a 403 narrowing the scope, there is nothing else
to pass. It stays the ordinary state until a namespace picker exists to set a
scope from inside the app — none does yet ([todo.md § Phase 5](../todo.md)).

**The server sends no `NAMESPACE` column for it.** Measured 2026-08-22:
`/api/v1/pods` — 53 rows drawn from three namespaces — comes back with the
same nine columns as `/api/v1/namespaces/kube-system/pods`
(`reports/2026-08-22-browser-rows-table-watch-and-refresh.md` § 2). `kubectl
get pods -A` prints `NAMESPACE` because **kubectl prepends it client-side**;
the server never sends one, so a screen that draws only the `priority: 0`
cells has nothing of its own to show either. Left as it was, a kind with one
popular name in every namespace — `configmaps`, `kube-root-ca.crt` — draws
identical rows with a cursor resting on one of them, which is invariant 2's
*explicitly selected object* satisfied in the letter and defeated in the
intent.

**The fix spends no new column.** `Row` already carries the identity a Table
cell cannot: name, namespace and uid, off the same `PartialObjectMetadata`
the `●` finding marker is matched against. So the browser draws
**`namespace/name`** in the first cell instead of the bare name — the exact
identity-line rule the rest of this app already uses
([README § the five rules, item 5](README.md#the-five-rules-every-screen-obeys)),
applied here the same way the finding marker already is: a value prepended to
the first `Cell`, not a column of its own
([widgets.md § 2](widgets.md#2-element--widget)).

- **Only when the view carries no namespace scope.** A scoped view
  (`ns: payments`) already names its one namespace in the title, so every row
  sharing it there would be noise the reader has already been told; the
  scoped mockup at the top of this file is unchanged.
- **Only namespaced kinds.** A cluster-wide kind (nodes, persistent volumes,
  certificate requests) never grows a namespace it does not have — `Row`'s own
  `namespace` field decides this, not a kind check, so the rule reads one
  condition (`row.namespace.is_some()`) the same way the title's does.
- **A row whose `namespace` is genuinely absent draws the bare name, not
  `None/name` and not a bare slash.** This is `Row::namespace` on a Table
  fetched with `?includeObject=None` — never what a running k8rs asks for, but
  the shape `tests/fixtures/table-deployments.json` was captured in, so the
  decode has to survive it rather than assume one includeObject shape
  (`src/k8s.rs` § THE BROWSER'S ROWS). Same governing rule either way: no
  namespace is shown where the row does not carry one.
- **The clip point leaves one blank column before the next cell.** A
  namespace prefix makes the widest name in the column longer on average, and
  [widgets.md § 7](widgets.md#7-text-that-came-from-the-api) already clips an
  over-long string at the cell boundary rather than truncating it by hand — but
  clipping flush to the boundary would fuse a cut-off name onto the number
  beside it. The name cell reserves its last column as blank for exactly this,
  so a clipped identity and a clipped number are never read as one token.

`configmaps`, no namespace scope in effect — six of its fourteen rows share
one name. Measured 2026-08-22 off `/api/v1/configmaps` on the same cluster:
14 rows, six named `kube-root-ca.crt`
(`reports/2026-08-22-browser-rows-table-watch-and-refresh.md` § 2); the
namespaces below are illustrative, the row count and the collision are not:

```
┌───────────────────────────────────────────────┐
│  NAME                              DATA  AGE  │
│▸ default/kube-root-ca.crt          1     36h  │
│  kube-node-lease/kube-root-ca.crt  1     36h  │
│  kube-public/kube-root-ca.crt      1     36h  │
│  kube-system/kube-root-ca.crt      1     36h  │
│  local-path-storage/kube-root-ca.c 1     36h  │
│  payments/kube-root-ca.crt         1     4h   │
└───────────────────────────────────────────────┘
```

Six rows, six distinct strings, a cursor that names exactly one object —
`local-path-storage/kube-root-ca.c` is clipped, but it is still the only row
that starts with `l`, and `⏎` opens the object [detail.md](detail.md) shows
the rest of the name on, the same escape hatch every other over-long string on
this screen already has.
