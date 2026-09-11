# 2026-09-10 — the dialog family: three measurements

Operator review of Phase 11 boxes 1 and 2 (`src/ui.rs` § THE DIALOGS,
`src/views.rs` § THE MODAL LAYER). Commands and their real output. No
conclusions here — see the review findings and, if one is taken, a `D##`.

Host: dev machine. No cluster was created or torn down: the PM fixture cluster
`k8rs` was already up and every cluster command below is a read.

## 1. The eight dialogs, as they render

Run against a copy of the tree at HEAD+working-tree, with its own
`CARGO_TARGET_DIR` (the repo `target/` was left alone):

```
$ cd <copy of the tree>
$ CARGO_TARGET_DIR=<own> cargo test every_dialog_on_the_screen_it_belongs_to -- --nocapture
test ui::tests::every_dialog_on_the_screen_it_belongs_to ... ok
```

Eight boxes printed: Scale, Restart, Restart · paused, Delete · pod,
Delete · node, Refused, Gone · pod, Gone · deployment. Two of them, verbatim:

```
--- Delete · pod ---
 nodes 3/3                            k8rs           ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     1 ● 1 ▲│  ● payments/web  ·  3 of 5 pods              4 min ago  │
│  RESOURCES         │    Containers exceeded their memory limit and were      │
│   workl┌ Delete payments/web-7d9f4 ──────────────────────────────────┐       │
│   netwo│                                                             │       │
│   stora│  This removes the pod. Whatever created it will normally    │       │
│   confi│  replace it — k8rs has not checked whether anything did.    │       │
│   clust│  k8rs did not check this one with the cluster first.        │n ago  │
│  ANALYS│                                                             │       │
│        │  Type the pod's name to confirm:                            │       │
│        │  ┌───────────────────────────────────────────────────────┐  │       │
│        │  │ web-7d9f_                                             │  │       │
│        │  └───────────────────────────────────────────────────────┘  │       │
│        │                                                             │       │
│        │                [ delete ]    [ esc cancel ]                 │       │
│        └─────────────────────────────────────────────────────────────┘       │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get statefulsets -A --watch                                        │
│ $ kubectl scale deployment/web --replicas=3 -n payments                      │
├──────────────────────────────────────────────────────────────────────────────┤
│ type the name to enable  esc cancel                                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

```
--- Refused ---
 nodes 3/3                            k8rs           ctx: prod-eu · live · admin
┌────────────────────┬─────────────────────────────────────────────────────────┐
│▸ ALERTS     1 ● 1 ▲│  ● payments/web  ·  3 of 5 pods              4 min ago  │
│  RESOURCES┌ The cluster refused this ────────────────────────────┐ were      │
│   workload│                                                      │           │
│   network │  Nothing was changed.                                │           │
│   storage │                                                      │           │
│   config  │  The cluster's own words:                            │           │
│   cluster │    admission webhook 'limits.example.com' denied the │4 min ago  │
│  ANALYSIS │    request: replicas may not exceed 5 in this        │           │
│           │    namespace                                         │           │
│           │                                                      │           │
│           │  This is the check that runs before the real change —│           │
│           │  it stopped this one.                                │           │
│           │                                                      │           │
│           │                    [ esc dismiss ]                   │           │
│           └──────────────────────────────────────────────────────┘           │
│                    │                                                         │
├────────────────────┴─────────────────────────────────────────────────────────┤
│ $ kubectl get statefulsets -A --watch                                        │
│ $ kubectl scale deployment/web --replicas=3 -n payments                      │
├──────────────────────────────────────────────────────────────────────────────┤
│ esc dismiss  ⏎ open                                                          │
└──────────────────────────────────────────────────────────────────────────────┘
```

The two log-strip lines in both boxes are the test fixture's
(`src/ui_tests.rs:4929-4932`), not the modal's own command.

Rows counted off the render: Delete · pod is 11 content rows inside the nested
box; Delete · node is 13; both frames are 24 rows.

The quote window in the Refused box: `MODAL_ROWS` 13 less 3 rows above, 1
heading row and 5 tail rows leaves 4 rows at `DISMISS_BOX` 54 less
`MODAL_MARGIN` 2 less 2 = 50 columns — 200 characters of `Status.message`.
The 101-character message above used 3 of those 4 rows.

## 2. Does a server-side Table row carry `metadata.uid`?

The question behind `views::Object::uid: Option<String>` on the browser path.
k8rs sends no `includeObject`, so the server default applies
(`src/k8s.rs` § THE BROWSER'S ROWS). Read-only, through a local
`kubectl proxy` against the already-running `k8rs` cluster, with the same
Accept header `src/k8s.rs`'s `TABLE_ACCEPT` sends. Server v1.36.1,
client v1.36.4.

```
$ kubectl --context kind-k8rs proxy --port=8899 &
$ ACC=application/json;as=Table;g=meta.k8s.io;v=v1,application/json
$ curl -s -H "Accept: $ACC" http://127.0.0.1:8899<path> | jq -c "{kind, rows:(.rows|length), objects:[.rows[]?.object|{kind, apiVersion, uid_present:(.metadata.uid!=null), name:(.metadata.name!=null)}]}"

--- GET /api/v1/namespaces/kube-system/pods?limit=3
{"kind":"Table","rows":3,"objects":[{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true},{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true},{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true}]}
--- GET /api/v1/nodes?limit=3
{"kind":"Table","rows":3,"objects":[{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true},{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true},{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true}]}
--- GET /apis/apps/v1/namespaces/kube-system/deployments?limit=3
{"kind":"Table","rows":2,"objects":[{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true},{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true}]}
--- GET /apis/apps/v1/namespaces/kube-system/replicasets?limit=3
{"kind":"Table","rows":2,"objects":[{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true},{"kind":"PartialObjectMetadata","apiVersion":"meta.k8s.io/v1","uid_present":true,"name":true}]}
```

`uid_present` is `true` on every row of all four kinds. The proxy was killed
from an `EXIT` trap, not a last line; `kind get clusters` before and after both
print `k8rs` and nothing else.

Not measured: an aggregated APIService with its own Table implementation. This
cluster has none service-backed to probe.

## 3. How far into a Deployment serialization the Refused quote window reaches

NOTES § D217 measured a `fieldValidation=Strict` rejection returning the whole
patched object in `Status.message` (4859 bytes on a trivial Deployment). § 1
above puts the Refused box's window at 200 characters. Where the sensitive
fields sit in that serialization, over every Deployment on the fixture cluster
(read-only, byte offsets only — no object content is reproduced):

```
$ kubectl --context kind-k8rs get deploy <name> -n <ns> -o json | jq -c . | python3 -c "..."

    default/broken-owned                     total=2385  first "env"=-1  last-applied=134
    default/broken-rollout                   total=2225  first "env"=-1  last-applied=134
    default/healthy-deploy                   total=2269  first "env"=-1  last-applied=134
    k8rs-quota/broken-quota                  total=2345  first "env"=-1  last-applied=134
    kube-system/coredns                      total=3211  first "env"=-1  last-applied=-1
    kube-system/metrics-server               total=4461  first "env"=-1  last-applied=134
    local-path-storage/local-path-provisioner total=3867  first "env"=2207  last-applied=134
```

`-1` is Python `str.find` for absent. `last-applied` is the byte offset of the
string `last-applied-configuration`; `first "env"` is the offset of the first
`"env"` key. The one Deployment here that has an `env` block has it at byte
2207 of 3867.

## 4. Where `ask` runs relative to the dry-run

`src/ops.rs:937-950`, read at HEAD (this file is not in the diff):

```rust
let checked = if record.checkable {
    call(DRY_RUN).await.map(Some)
...
    Err(error) => Outcome::NotSent {
...
    Ok(returned) => {
        match ask(Checked {
```

`src/views.rs:990-995`, the command log's own contract for a mutation line:

```
a **mutation** — appended the instant `ops::ask` answers
`ops::Answer::Confirmed`, and never when the dialog opens. `Cancelled`, `Gone`
and `Changed` append nothing.
```
