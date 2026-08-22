# The browser's rows — what the API server actually sends (2026-08-22)

Operator review of the Phase 5 family *"the browser's rows"* (`src/k8s.rs`
§ THE BROWSER'S ROWS, § KEEPING A BROWSER VIEW FRESH). Every claim the family
rests on that a running cluster can answer, asked of one.

**Where.** The dev machine, against the running fixture cluster (`kind-k8rs`,
`kindest/node:v1.36.1`, server `v1.36.1`, 4 nodes), **read-only**: one
`kubectl proxy --port=0` and `curl` GETs through it, torn down at the end. No
object was created, changed or deleted; nothing was captured into `tests/`.
No `K8RS_CLUSTER=review` cluster was brought up — this needed no cluster of its
own, only reads of the one already running.

```
kubectl --context kind-k8rs proxy --port=0
Starting to serve on 127.0.0.1:39799
```

Header sent on every request below, byte for byte the `TABLE_ACCEPT` constant:

```
Accept: application/json;as=Table;g=meta.k8s.io;v=v1,application/json
```

---

## 1. A Table watch sends `columnDefinitions` once per stream, not once per event

Replay of an existing watch window (no object was touched to produce events):

```
curl -s --max-time 5 -H "$TA" \
  "$P/api/v1/namespaces/default/pods?watch=true&resourceVersion=$((rv-800))"
```

18 events. Per event, `.object.columnDefinitions | length` and the byte length
of the whole event line:

```
 1 {"type":"MODIFIED","cols":9,"rows":1}   5764
 2 {"type":"MODIFIED","cols":0,"rows":1}   3257
 3 {"type":"MODIFIED","cols":0,"rows":1}   3258
 4 {"type":"MODIFIED","cols":0,"rows":1}   2756
 5 {"type":"MODIFIED","cols":0,"rows":1}   2760
 6 {"type":"MODIFIED","cols":0,"rows":1}   3268
 7 {"type":"MODIFIED","cols":0,"rows":1}   2777
 8 {"type":"MODIFIED","cols":0,"rows":1}   2833
 9 {"type":"MODIFIED","cols":0,"rows":1}   2845
10 {"type":"MODIFIED","cols":0,"rows":1}   2615
11 {"type":"MODIFIED","cols":0,"rows":1}   2671
12 {"type":"MODIFIED","cols":0,"rows":1}   2682
13 {"type":"MODIFIED","cols":0,"rows":1}   2756
14 {"type":"MODIFIED","cols":0,"rows":1}   2760
15 {"type":"MODIFIED","cols":0,"rows":1}   3259
16 {"type":"MODIFIED","cols":0,"rows":1}   2778
17 {"type":"MODIFIED","cols":0,"rows":1}   2834
18 {"type":"MODIFIED","cols":0,"rows":1}   3256
```

The same shape on a second kind, `/apis/apps/v1/namespaces/default/deployments`:

```
     10 {"type":"MODIFIED","cols":0}
      1 {"type":"MODIFIED","cols":8}
```

Field values: `columnDefinitions` on the first event of a pods stream is
**3087 bytes / 9 entries**; on events 2..18 it is **`[]`**. Mean event size
over the 18: **3062 bytes**.

The same 18-event window asked for as a metadata watch, header
`application/json;as=PartialObjectMetadata;g=meta.k8s.io;v=v1,application/json`:

```
--- 18 events, 47250 bytes total
    mean 2624 bytes/event
    first event: type=MODIFIED kind=PartialObjectMetadata
```

And a Table watch event's row carries the same object the metadata watch sends:

```
head -1 wt.jsonl | jq -r '.object.rows[0].object.kind'
PartialObjectMetadata
```

The paragraph these numbers bear on:
[NOTES § Verified against a real cluster](../NOTES.md#verified-against-a-real-cluster-2026-08-11)
item 1, `screens/resources.md:58-64`, `src/k8s.rs` § KEEPING A BROWSER VIEW
FRESH — *"every event re-sends the entire column schema, 3086 bytes of
`columnDefinitions` to deliver an 82-byte row. A 37× overhead is the
argument."*

## 2. A cross-namespace Table has no Namespace column

```
curl -s -H "$TA" "$P/api/v1/pods" | jq -r '[.columnDefinitions[].name]|join(" | ")'
Name | Ready | Status | Restarts | Age | IP | Node | Nominated Node | Readiness Gates
rows: 53
```

Identical column list to the single-namespace call. The 53 rows are drawn from
three namespaces, read off `.rows[].object.metadata.namespace` and not off any
cell:

```
default, kube-system, local-path-storage
```

`kubectl get pods -A` prints a `NAMESPACE` column; the server did not send one,
so kubectl adds it client-side.

A kind where the collision is guaranteed, `/api/v1/configmaps`:

```
Name(p0) Data(p0) Age(p0)
rows: 14
kube-root-ca.crt x6
```

The six rows, cells at `priority: 0` — what `screens/resources.md` draws:

```
kube-root-ca.crt   1   36h
kube-root-ca.crt   1   36h
kube-root-ca.crt   1   36h
kube-root-ca.crt   1   36h
kube-root-ca.crt   1   36h
kube-root-ca.crt   1   36h
```

The committed fixture is the same shape: `tests/fixtures/table-deployments.json`
was captured from `/apis/apps/v1/deployments` (no namespace) and its six rows
live in four namespaces — `broken-owned`, `broken-rollout`, `healthy-deploy`
in one, `broken-quota`, `coredns`, `local-path-provisioner` each in another.
It was captured with `?includeObject=None`, so `Row::namespace` is `None` on
every row as well.

## 3. `includeObject` — the ratio, and what the 19× is made of

```
/api/v1/pods, default includeObject : 363162 bytes, 53 rows  (6852 bytes/row)
/api/v1/pods?includeObject=None      :  18249 bytes
ratio                                : 19.9
```

kube-system alone, with the pieces separated:

```
Table (default includeObject): 142584 bytes, 14 rows
  columnDefinitions:           3087 bytes
  one row (cells + object):    4504 bytes
  one row, cells only:          107 bytes
PartialObjectMetadataList:     128136 bytes, 14 items
  one PartialObjectMetadata:   4378 bytes
  managedFields share:        46691 bytes
```

## 4. A Table can be paged, and the response says how much is left

```
curl -s -H "$TA" "$P/api/v1/pods?limit=5" | jq -c '{rows:(.rows|length), metadata:...}'
rows: 5
metadata.resourceVersion : "230562"
metadata.continue        : present (a base64 token, not reproduced here)
metadata.remainingItemCount : 48
```

Without `limit` the same call answers `metadata: {"resourceVersion":"230562"}`
and 53 rows. `TableResponse` in `src/k8s.rs` names no `metadata` field.

## 5. A kind that can be listed and cannot be watched, on a bare cluster

```
kubectl --context kind-k8rs api-resources -o wide --no-headers \
  | awk '{v=$NF} v ~ /list/ && v !~ /watch/ {print $1, v}'
componentstatuses get,list
```

42 of the cluster's resources advertise `list`; one of them does not advertise
`watch`. What the server answers when one is watched anyway:

```
timeout 10 kubectl --context kind-k8rs get componentstatuses -w
Warning: v1 ComponentStatus is deprecated in v1.19+
NAME                 STATUS    MESSAGE   ERROR
controller-manager   Healthy   ok
scheduler            Healthy   ok
etcd-0               Healthy   ok
Error from server (MethodNotAllowed): watch is not supported on resources of kind "componentstatuses"
```

`browsable()` filters on `verbs::LIST` only, so this kind is offered.

## 6. `kube::runtime::metadata_watcher` is deprecated in the pinned kube 4.2.0

Read off the pinned source, then compiled. `kube-runtime-4.2.0/src/watcher.rs:850`:

```
#[deprecated(
    since = "3.1.0",
    note = "Use `watcher(Api::<PartialObjectMeta<K>>::all(client), config)` instead. \
            `Api<PartialObjectMeta<K>>` now automatically uses metadata-only requests."
)]
pub fn metadata_watcher<K: ...>(api: Api<K>, watcher_config: Config) -> ...
```

A throwaway crate outside the repo, with this repo's exact dependency lines,
holding only the line `src/k8s.rs` § KEEPING A BROWSER VIEW FRESH names:

```
cargo clippy   # scratch crate, k8s-openapi 0.28 v1_36 + kube 4.2.0 client/runtime/rustls-tls

warning: use of deprecated function `kube::kube_runtime::metadata_watcher`: Use
`watcher(Api::<PartialObjectMeta<K>>::all(client), config)` instead. ...
  --> src/main.rs:10:20
   |
10 |     kube::runtime::metadata_watcher(api, watcher::Config::default())
   |                    ^^^^^^^^^^^^^^^^
   = note: `#[warn(deprecated)]` on by default
```

`just check` runs `cargo clippy --locked --all-targets --all-features -- -D warnings`.

The replacement, for a *dynamically discovered* kind, in the same scratch crate:

```rust
let api: Api<PartialObjectMeta<DynamicObject>> = Api::all_with(client, resource);
watcher::watcher(api, watcher::Config::default())
```

```
cargo clippy
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
```

No warning. `PartialObjectMeta<DynamicObject>` accepts an `ApiResource` as its
dynamic type, so `all_with` / `namespaced_with` work unchanged.

Backoff, read off the same file: `watcher.rs:26` — *"To avoid constantly
looping errors, make sure backoff is applied."* — and `:806`, inside
`metadata_watcher`'s own doc — *"You can apply your own backoff by not polling
the stream for a duration after errors."* There is no backoff inside either
entry point.

## 7. What a refusal and a missing kind look like on this path

403, with the Table Accept header, impersonating a principal with no RBAC:

```
{"kind":"Status","code":403,"reason":"Forbidden",
 "message":"secrets is forbidden: User \"nobody\" cannot list resource \"secrets\"
            in API group \"\" in the namespace \"kube-system\""}
```

404 for a resource inside a group the server serves
(`/apis/apps/v1/namespaces/default/widgets`):

```
{"kind":"Status", "message":"the server could not find the requested resource",
 "reason":"NotFound", "code":404}   <- http 404
```

404 for a group nobody serves — the shape a deleted CRD leaves
(`/apis/example.com/v1/namespaces/default/widgets`):

```
404 page not found
 <- http 404
```

Not a `Status`. `kube-client-4.2.0/src/client/mod.rs:551-558` then builds
`Status::failure(&text, "Failed to parse error data").with_code(404)`, so
`.message` is the literal string above and `.reason` is
`"Failed to parse error data"`.

## 8. The Accept header kubectl itself sends

```
kubectl --context kind-k8rs get pods -n kube-system --v=8 | grep -i -m3 'Accept:'
	Accept: application/json;as=Table;v=v1;g=meta.k8s.io,application/json;as=Table;v=v1beta1;g=meta.k8s.io,application/json
```

k8rs sends the same three parameters in a different order and offers no
`v1beta1` Table.

## 9. The guard's predicate against the characters `screens/context.md` names

`src/k8s.rs` `text()` keeps every character for which `char::is_control()` is
false. Compiled and run:

```
                        U+001B ESC  is_control = true
                        U+0000 NUL  is_control = true
                        U+007F DEL  is_control = true
                   U+009B CSI (C1)  is_control = true
     U+202E RIGHT-TO-LEFT OVERRIDE  is_control = false
     U+202D LEFT-TO-RIGHT OVERRIDE  is_control = false
                        U+2066 LRI  is_control = false
           U+200B ZERO WIDTH SPACE  is_control = false
                U+00AD SOFT HYPHEN  is_control = false
            U+FEFF ZERO WIDTH NBSP  is_control = false

tag in  : "prod\u{202e}reversed"
tag kept: "prod\u{202e}reversed"   U+202E survived = true
```

The sentence this bears on is `screens/context.md:141-145`.

## 10. What the family's own test prints

```
cargo test --locked --bin k8rs -- --nocapture the_columns_come_from_the_server
```

```
what a screen gets, deployments:
NAME                    READY  UP-TO-DATE  AVAILABLE  AGE
broken-owned            0/1    1           0          34h
broken-rollout          0/2    1           0          34h
healthy-deploy          0/2    2           0          34h
broken-quota            0/1    0           0          34h
coredns                 2/2    2           2          34h
local-path-provisioner  1/1    1           1          34h
pods: 9 columns, 5 at priority 0
deployments: 8 columns, 5 at priority 0
```

## Teardown

```
pkill -f "kubectl --context kind-k8rs proxy"
kind get clusters   -> k8rs
kubectl --context kind-k8rs get nodes --no-headers | wc -l   -> 4
```

The fixture cluster was left running and unchanged.
