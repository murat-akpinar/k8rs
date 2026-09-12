# Screen — Resources (the browser)

Every kind the cluster serves, including CRDs, with **no per-kind code**. The
sidebar comes from `kube::discovery`; the columns come from the API server's
own `Table` printing — the exact columns `kubectl get` would show.

```
 nodes 3/3                      k8rs     ctx: prod-eu · live · admin
┌────────────────────┬───────────────────────────────────────────────┐
│  ALERTS     3 ● 7 ▲│  deployments          ns: payments            │
│  RESOURCES         │                                               │
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
│  ANALYSIS          │                                               │
├────────────────────┴───────────────────────────────────────────────┤
│ $ kubectl get deployments -n payments                              │
├────────────────────────────────────────────────────────────────────┤
│ ↑↓ move  ⏎ open  s scale  r restart  / filter  ? all keys  q quit  │
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

- **The footer's exact set — and why `ctrl-d delete` is not in it even though
  the key still works — is [widgets.md § The footer](widgets.md#2a-the-footer)**,
  the same rule [alerts.md](alerts.md) draws from. `q quit` and `? all keys`
  are the two keys that never give way; `ctrl-d delete` gives way to them here
  the same way `d describe` and `y view as YAML` already did, on every list
  view, before this footer was ever audited for width.
- **`s`/`r` on the selected row are marked the same way as on Alerts, and no
  differently for being over a table instead of a card** — `s no scale`,
  `r no restart`, from the same `may_i_in` result Alerts reads
  ([widgets.md § The footer](widgets.md#2a-the-footer),
  [help.md § When a key is refused](help.md#when-a-key-is-refused)). One
  mechanism, one place it is spelled out, cited from both list screens rather
  than drawn twice.
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
- **A kind with zero rows is not a finding.** It draws its own dim, centred
  sentence — deliberately not the Alerts `○ nothing is broken` claim, because
  an empty list of one kind carries no severity — covering all three ways the
  pane can be empty: scoped, unscoped, and when the kind that was selected has
  dropped out of the sidebar's own list
  ([states.md § An empty kind in the browser](states.md#an-empty-kind-in-the-browser)).

## The line under the table

One line, drawn under the table about the **selected** row only, when that
row's object owns an Alerts card — never one line per marked row in the pane,
which would be a second list competing with the table above it.

**Which sentence draws is [`Card::count`](../src/views.rs), not a layout
choice.** A card with a real pod count gets the first; a card with none —
every node card, and a card whose owner **is** the pod — gets the second.
Both are literal strings, and there is no third:

```
● web has 3 pods with problems — ⏎ to see
● node-3 has problems — ⏎ to see
```

- `3 pods` is [`Card::affected`](../src/views.rs) — the same number
  `alerts.md`'s `· n of m pods` counts, off the same card, about the same
  object.
- The second form is **not** `0 pods with problems` and **not** a pod counted
  against its own card — both were considered and rejected. A node's card has
  `affected == 0` — that count counts pods, and a node card is about one
  machine
  ([NOTES § D39](../NOTES.md#d39--a-node-owns-pods-and-three-more-things-the-shape-could-not-say-2026-08-12)).
  A bare pod's card has `owner.kind == Pod` — nothing owns it, so a fraction
  of one pod out of itself is not a fact
  ([NOTES § D246 ruling 2](../NOTES.md#d246--the-viewsrs-review-round-a-fraction-whose-halves-count-different-things-a-card-that-draws-a-count-the-screen-ends-without-and-the-freeze-that-was-set-one-phase-too-early-2026-09-06)).
  Both reach [`Card::count`](../src/views.rs)'s same `None`, and `has problems`
  is the one sentence that is true of either without claiming to know which.

### When it does not fit, the name gives way — and now it says so

The line is a fixed prefix (the row marker's own column, the severity glyph)
and a fixed tail — the sentence after the name, in full, one of the two
strings above minus the name — with the name in between. **`⏎ to see` never
gives way.** It is the half of the line that says what to do next, and the
order that already ships is right: a line with a shortened name still tells
the reader who it is about; a line with a shortened instruction tells them
nothing — the same ordering `· n of m pods` already gives way to the name for,
one level up
([alerts.md § the age, and what it costs the name](alerts.md#the-age-and-what-it-costs-the-name)).

The rule, so a reviewer can check any width against it rather than a drawing:

1. `room` is the pane's width, less the prefix, less the tail — the same
   measurement already made off the spans about to be drawn, not a sum
   restated separately.
2. The name fits in `room` → it draws whole. No mark.
3. It does not → it is cut to `room − 1` columns, on a character boundary and
   never inside one — the same rule every other clip on this page already
   follows — and **one `…`** is appended, glued to the last character kept,
   no space before it. Same mark, same rule as the one other place this
   product cuts a string on purpose
   ([widgets.md § 7](widgets.md#7-text-that-came-from-the-api)).
4. `room` is `0` → nothing is drawn where the name would go. Not reached at
   the 80×24 floor by any tail this rule set produces today, and not designed
   past that.

**This is that section's exception extended to a second place, not a second
convention** — [widgets.md § 7](widgets.md#7-text-that-came-from-the-api) now
names both. What makes the cut legitimate here is the same thing that makes
it legitimate there: the whole name is one `⏎` away, on the object's own
detail screen ([detail.md](detail.md)), which was already true of every row
on this pane before this rule existed.

Illustrative, not a captured run — the exact column the cut lands on is the
renderer's own measurement, not this page's:

```
│  ● payments/checkout-worker-servi… has 3 pods with problems — ⏎ to see│
```

The real case this rule exists for is narrower, and already run:
`cargo test the_browser -- --nocapture` draws `kube-system/cored` with no
mark at all today, on a 57-column pane — `kube-system/coredns`, a Deployment
name, with its last two characters silently gone. Nothing about that string
tells a reader it is not `kube-system/cored`, an object that does not exist.
The fix is the rule above; the exact marked string it produces is
`cargo test`'s to show next, not this file's to predict.

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
