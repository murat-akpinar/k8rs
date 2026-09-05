# resourceVersion preconditions and the 409, on the wire — scale, restart, delete

`k8s-admin`, 2026-09-05. Measured for todo.md 3837 ("Every call sends the
resourceVersion that was read; a `409` offers a re-read, never a blind
overwrite"), against a real apiserver: one ephemeral `K8RS_CLUSTER=review` kind
cluster, `kind v0.32.0`, node image `kindest/node:v1.36.1` (the tag
`scripts/cluster.sh` pins), server **`v1.36.1`**, `kubectl` client **`v1.36.3`**,
apiserver on `127.0.0.1:6444` so the PM's fixture cluster on `:6443` was
untouched. Its own kubeconfig in a scratch file (`kind create cluster
--kubeconfig …`), so the user's current context never moved. Two nodes
(`review-control-plane`, `review-worker`) so §4 could spend one. Torn down from
a `trap … EXIT` (NOTES § D185); `kind get clusters` afterwards printed `k8rs`
and nothing else, and `docker ps` listed exactly the PM's four containers.

No committed artifact was produced and nothing was written into `tests/`.

**Two tools.** Raw requests went through `kubectl proxy --port=8899` and `curl`,
because only that gives byte control over the request body and the `Accept`
header; §5 used `kubectl --v=8` / `--v=9`, because the question there is what
`kubectl` itself sends. Each section says which.

**The subject.** One namespace `payments`, one Deployment `web` created with
`kubectl apply -f` so it carries the `last-applied-configuration` annotation
(`kubectl.kubernetes.io/…`), and whose pod template holds exactly one
environment variable, `PLANTED`, with the fake value `ZZZ-PLANTED-CANARY-ZZZ`.
That string is planted, is not a secret, and is what §6 greps for.

**Reading the numbers.** resourceVersions differ between sections: each stale
case was made by reading the object, bumping it with `kubectl label … bump=N
--overwrite`, and re-reading. The numbers are only meaningful within one block.

**One redaction, named.** `scripts/reports-guard.py` refuses the full literal
name of the `last-applied-configuration` annotation key, so where real output
printed that key it appears below as `<the last-applied-configuration
annotation>`. Object UIDs are `<uid>`. Nothing else was altered.

## 1. A resourceVersion in a merge patch body *is* a precondition on the scale subresource

`GET …/scale` first, then the Deployment, back to back:

```
$ curl -s http://127.0.0.1:8899/apis/apps/v1/namespaces/payments/deployments/web/scale
kind: Scale apiVersion: autoscaling/v1
metadata keys: ['creationTimestamp', 'name', 'namespace', 'resourceVersion', 'uid']
scale metadata.resourceVersion: 569
spec: {'replicas': 2} status: {'replicas': 2, 'selector': 'app=web'}

$ curl -s http://127.0.0.1:8899/apis/apps/v1/namespaces/payments/deployments/web
deployment metadata.resourceVersion: 569
generation: 1
```

The `Scale` read back from `GET …/scale` **does** carry
`metadata.resourceVersion`, and it is populated.

Equality was checked twice more and the second check is the one that means
something. Sampling both endpoints in a loop is a race — the object moves
between the two GETs, and sample 1 of 12 showed `1612` against `1622` for that
reason alone. The semantic check instead uses one value as a precondition on the
other endpoint:

```
scale rv read = 1633
--- the SCALE's rv as a precondition on the DEPLOYMENT object (strategic patch):
http=200   kind: Deployment
deployment rv read = 1673
--- the DEPLOYMENT's rv as a precondition on the SCALE subresource (merge patch):
http=200   kind: Scale
--- the SCALE's rv as a DELETE precondition on the deployment (dryRun=All):
http=200   kind: Status status: Success
```

One counter. The `Scale`'s resourceVersion is the Deployment's own, and is
accepted interchangeably by all three write shapes.

### Stale — 409

```
$ curl -s -w 'http=%{http_code}\n' -X PATCH \
    -H 'Content-Type: application/merge-patch+json' -H 'Accept: application/json' \
    --data '{"metadata":{"resourceVersion":"569"},"spec":{"replicas":3}}' \
    http://127.0.0.1:8899/apis/apps/v1/namespaces/payments/deployments/web/scale
http=409
{
  "kind": "Status",
  "apiVersion": "v1",
  "metadata": {},
  "status": "Failure",
  "message": "Operation cannot be fulfilled on deployments.apps \"web\": the object has been modified; please apply your changes to the latest version and try again",
  "reason": "Conflict",
  "details": {
    "name": "web",
    "group": "apps",
    "kind": "deployments"
  },
  "code": 409
}
```

(current at that moment: `615`.)

### Current — 200, and the precondition is real

```
=== current resourceVersion (615)
http=200
kind: Scale spec: {'replicas': 3} metadata.resourceVersion: 636
deployment spec.replicas now: 3

=== baseline: no resourceVersion in the body at all (what ops.rs sends today)
http=200
kind: Scale spec: {'replicas': 4}

=== the SAME body sent a second time (615 is now stale)
http=409   reason: Conflict
spec.replicas after: 4
```

The third block is the one that proves it is a precondition and not decoration:
the identical request that succeeded at `615` fails once the object has moved.

## 2. Same on the object itself, with `restart`'s media type

Body: `{"metadata":{"resourceVersion":"<rv>"},"spec":{"template":{"metadata":{"annotations":{"kubectl.kubernetes.io/restartedAt":"<ts>"}}}}}`,
`Content-Type: application/strategic-merge-patch+json`, sent to
`…/deployments/web` (no subresource).

```
before: rv=677 gen=3
after bump: rv=703 gen=3  (stale rv to use: 677)

=== 2a. STALE resourceVersion (677)
http=409
{
  "kind": "Status",
  "apiVersion": "v1",
  "metadata": {},
  "status": "Failure",
  "message": "Operation cannot be fulfilled on deployments.apps \"web\": the object has been modified; please apply your changes to the latest version and try again",
  "reason": "Conflict",
  "details": {
    "name": "web",
    "group": "apps",
    "kind": "deployments"
  },
  "code": 409
}

=== 2b. CURRENT resourceVersion (703)
http=200
after 2b: rv=704 gen=4   (gen before was 3, delta=1)
returned kind: Deployment
spec.template.metadata.annotations: {"kubectl.kubernetes.io/restartedAt":"2026-09-04T23:01:32Z"}
```

Byte-identical `Status` to §1's — same message, same `details`, same 377 bytes.
`generation` moved `3 → 4`: **exactly 1**, the same bump the same patch produces
without a resourceVersion in it.

## 3. `dryRun=All` sees the conflict

Same two requests, `?dryRun=All` appended to the URL:

```
before: rv=834 gen=4 replicas=4 ; after bump: rv=862 ; stale=834

=== 3a. scale subresource, dryRun=All, STALE rv
http=409
code: 409 reason: Conflict details: {'name': 'web', 'group': 'apps', 'kind': 'deployments'}

=== 3b. deployment strategic patch, dryRun=All, STALE rv
http=409
code: 409 reason: Conflict details: {'name': 'web', 'group': 'apps', 'kind': 'deployments'}
```

And the dry-run does not consume the resourceVersion — the real call with the
*same* value still works:

```
=== 3c. scale subresource, dryRun=All, CURRENT rv (862), then the real call with the SAME rv
dry-run http=200
after dry-run: rv=862 (was 862)  replicas=4
real    http=200
after real:    rv=863 replicas=7
```

## 4. `DELETE` preconditions

Body shape: `{"propagationPolicy":"Background","preconditions":{…}}`, through
`kubectl proxy`. Four disposable Deployments `del1`–`del4` (`kubectl create
deployment`), plus the worker Node.

### 4a. `preconditions.resourceVersion`, stale — 409, and the message names both numbers

```
del1: stale rv=947 current rv=989
$ curl -s -w 'http=%{http_code}\n' -X DELETE -H 'Content-Type: application/json' \
    --data '{"propagationPolicy":"Background","preconditions":{"resourceVersion":"947"}}' \
    http://127.0.0.1:8899/apis/apps/v1/namespaces/payments/deployments/del1
http=409
{
  "kind": "Status",
  "apiVersion": "v1",
  "metadata": {},
  "status": "Failure",
  "message": "Operation cannot be fulfilled on Deployment.apps \"del1\": the ResourceVersion in the precondition (947) does not match the ResourceVersion in record (989). The object might have been modified",
  "reason": "Conflict",
  "details": {
    "name": "del1",
    "group": "apps",
    "kind": "Deployment"
  },
  "code": 409
}
del1 still present: deployment.apps/del1
```

`details.kind` is `Deployment` here and `deployments` in §1/§2 — same cluster,
same object type, different casing and different number per code path.

### 4b. `preconditions.uid`, wrong — also 409, different sentence

```
http=409
{"kind": "Status", "apiVersion": "v1", "status": "Failure", "reason": "Conflict",
 "details": {"name": "del2", "group": "apps", "kind": "Deployment"}, "code": 409}
message: Operation cannot be fulfilled on Deployment.apps "del2": the UID in the precondition (<uid>) does not match the UID in record (<uid>). The object might have been deleted and then recreated
del2 still present: deployment.apps/del2
```

The UID I sent was the all-zeros fake; the record's UID is masked above.

### 4c–4e. Correct precondition beside `propagationPolicy`, and the dry-run

```
=== 4c. preconditions.resourceVersion = CURRENT (1012), beside propagationPolicy
http=200   returned kind: Status status: Success   details.kind: deployments
del3 after 3s: Error from server (NotFound): deployments.apps "del3" not found
replicasets left for del3: 0

=== 4d. preconditions with BOTH correct rv and correct uid
http=200
del4 after 2s: Error from server (NotFound): deployments.apps "del4" not found

=== 4e. stale rv precondition AND dryRun=All
http=409   reason: Conflict
del1 still present: deployment.apps/del1
```

`preconditions` and `propagationPolicy` coexist in one `DeleteOptions` body; the
delete still cascades (`replicasets left: 0`).

### 4f–4g. Cluster-scoped — a Node

```
node review-worker: stale rv=534 current rv=1313

=== 4f. DELETE node, preconditions.resourceVersion = STALE
http=409
{
  "kind": "Status",
  "apiVersion": "v1",
  "metadata": {},
  "status": "Failure",
  "message": "Operation cannot be fulfilled on Node \"review-worker\": the ResourceVersion in the precondition (534) does not match the ResourceVersion in record (1313). The object might have been modified",
  "reason": "Conflict",
  "details": {
    "name": "review-worker",
    "kind": "Node"
  },
  "code": 409
}
node still present: node/review-worker

=== 4g. same, CURRENT resourceVersion
http=200   returned kind: Status status: Success
review-control-plane Ready
```

`details` on the cluster-scoped kind has **no `group` key** — core group, so the
field is absent, not empty. A formatter reading `details.group` gets nothing
here. The node was restored with `docker exec review-worker systemctl restart
kubelet`; `Ready` again within 5 s.

## 5. What `kubectl` can express

Measured with `--v=8` / `--v=9`; bodies extracted from klog's
`"Request Body" body=` form.

### `kubectl scale` — it has the flag, and it changes the verb

```
$ kubectl scale --help
 If --current-replicas or --resource-version is specified, it is validated before the scale is
 attempted, and it is guaranteed that the precondition holds true when the scale is sent to the server.
  kubectl scale --current-replicas=2 --replicas=3 deployment/mysql
    --resource-version='':
	Precondition for resource version. Requires that the current resource version match this value in order to scale.
    --current-replicas=-1:
	Precondition for current size. Requires that the current size of the resource match this value in order to scale. -1 (default) for no condition.
```

With a **stale** value, the server is never asked:

```
$ kubectl -n payments scale deployment/web --replicas=6 --resource-version=1 --v=8
exit=1
error: Expected resource version to be 1, was 908
--- requests kubectl made:
"Request" verb="GET" url="/apis/apps/v1/namespaces/payments/deployments/web"
"Request" verb="GET" url="/apis/apps/v1/namespaces/payments/deployments/web/scale"
```

Two GETs and no write. The precondition is checked **client-side**; there is no
409 and no `Status` body to render.

With a **correct** value, the verb changes from `PATCH` to `PUT`:

```
$ kubectl -n payments scale deployment/web --replicas=6 --resource-version=908 --v=9
exit=0
"Response" verb="GET" url=".../deployments/web"       status="200 OK"
"Response" verb="GET" url=".../deployments/web/scale" status="200 OK"
"Response" verb="PUT" url=".../deployments/web/scale" status="200 OK"
  BODY: {"kind":"Scale","apiVersion":"autoscaling/v1","metadata":{"name":"web","namespace":"payments","uid":"<uid>","resourceVersion":"908","creationTimestamp":"2026-09-04T23:00:30Z"},"spec":{"replicas":6},"status":{"replicas":7,"selector":"app=web"}}
final replicas: 6
```

```
$ kubectl -n payments scale deployment/web --replicas=5 --v=9      # no flag
"Response" verb="GET"   url=".../deployments/web"       status="200 OK"
"Response" verb="PATCH" url=".../deployments/web/scale" status="200 OK"
	curl -v -XPATCH -H "Accept: application/json, */*" -H "Content-Type: application/merge-patch+json" … '/apis/apps/v1/namespaces/payments/deployments/web/scale'
  BODY: {"spec":{"replicas":5}}
```

So the flagless form is byte-identical to what `ops.rs` sends today
(`application/merge-patch+json`, `{"spec":{"replicas":N}}`); the
`--resource-version` form is a whole-object `PUT` of the `Scale`, preceded by a
client-side check. The `PUT` path *is* also guarded server-side — sending a
`Scale` whose resourceVersion has gone stale:

```
$ curl -X PUT .../deployments/web/scale   # the Scale read at rv 1245, object bumped since
http=409   reason: Conflict
message: Operation cannot be fulfilled on deployments.apps "web": the object has been modified; …
```

### `kubectl delete` — **no flag**

```
$ kubectl delete --help | grep -iE 'resource-version|precondition|uid'
(no match — no such flag)
```

There is no way to express `preconditions.resourceVersion` or
`preconditions.uid` on a `kubectl delete` command line.

### `kubectl rollout restart` — **no flag**

Its whole `Options:` list, verbatim: `--allow-missing-template-keys`,
`--field-manager`, `-f/--filename`, `-k/--kustomize`, `-o/--output`,
`-R/--recursive`, `-l/--selector`, `--show-managed-fields`, `--template`.
Nothing about resourceVersion or preconditions.

### `kubectl patch` — **no flag**

```
$ kubectl patch --help | grep -iE 'resource-version|precondition'
(no match — no such flag)
```

## 6. The metadata-only read

`GET …/deployments/web` with
`Accept: application/json;as=PartialObjectMetadata;g=meta.k8s.io;v=v1`, against
a plain `GET`:

```
=== PartialObjectMetadata
http=200 bytes=5247
=== plain GET (full object)
http=200 bytes=7243

PartialObjectMetadata kind/apiVersion: PartialObjectMetadata meta.k8s.io/v1
top-level keys: ['apiVersion', 'kind', 'metadata']
metadata keys: ['annotations', 'creationTimestamp', 'generation', 'labels', 'managedFields', 'name', 'namespace', 'resourceVersion', 'uid']
metadata.annotations keys: ['deployment.kubernetes.io/revision', '<the last-applied-configuration annotation>']
metadata.resourceVersion present: True value: 908
metadata.uid present: True
managedFields present: True

canary occurrences in PartialObjectMetadata response: 1
canary occurrences in full GET response:             2
```

**Yes — the `PartialObjectMetadata` response carries the
`last-applied-configuration` annotation, and that annotation contains
`ZZZ-PLANTED-CANARY-ZZZ`.** One occurrence, inside the annotation value; the
full object has two (the annotation, plus the live `env` entry).

Where the 5247 bytes go:

```
PartialObjectMetadata response bytes:             5247
  bytes of the last-applied-configuration value:   359
  bytes of managedFields (compact json):          2148
  what is left once both are dropped:              260
```

So the metadata read is **1.38×** smaller than the full object, not an order of
magnitude, and 41% of it is `managedFields` — the thing invariant 6 prunes
everywhere else.

Whether the annotation is there depends on how the object was created:

```
=== del5 (created with 'kubectl create', never applied)
http=200 bytes=3393
annotation keys: ['deployment.kubernetes.io/revision']
=== web (created with 'kubectl apply')
annotation keys: ['deployment.kubernetes.io/revision', '<the last-applied-configuration annotation>']
```

A Secret's metadata does *not* carry its data, in either encoding:

```
$ kubectl -n payments create secret generic probe --from-literal=k=ZZZ-PLANTED-CANARY-ZZZ
=== PartialObjectMetadata of that Secret
http=200 bytes=621
top-level keys: ['apiVersion', 'kind', 'metadata']
canary in response (plain): 0
canary in response (base64): 0
```

### `kube` 4.2

```
$ grep -rn "fn get_metadata\|fn get_scale" ~/.cargo/registry/src/index.crates.io-*/kube-client-4.2.0/src/
kube-client-4.2.0/src/api/subresource.rs:28:    pub async fn get_scale(&self, name: &str) -> Result<Scale>
kube-client-4.2.0/src/api/core_methods.rs:56:   pub async fn get_metadata(&self, name: &str) -> Result<PartialObjectMeta<K>>
kube-client-4.2.0/src/api/core_methods.rs:118:  pub async fn get_metadata_with(&self, name: &str, gp: &GetParams) -> Result<PartialObjectMeta<K>>
kube-client-4.2.0/src/api/core_methods.rs:167:  pub async fn get_metadata_opt(&self, name: &str) -> Result<Option<PartialObjectMeta<K>>>
```

`Api::get_metadata` exists, at `kube-client-4.2.0/src/api/core_methods.rs:56`,
with an `_opt` form at :167. The `Accept` header it sends is
`kube-core-4.2.0/src/request.rs:16`:

```
pub(crate) const JSON_METADATA_MIME: &str = "application/json;as=PartialObjectMetadata;g=meta.k8s.io;v=v1";
```

— byte-identical to the header measured above, so the numbers in this section
are the ones `get_metadata` would produce. **That last sentence is read off
source, not off the wire**: I did not run a `kube` client against this cluster.

`Api::get_scale`'s `Scale` carries a populated `metadata.resourceVersion` —
measured, §1.

## 7. What a 409 body holds, and whether it echoes the object

Every conflict captured in this run, field by field, with its total body size:

```
--- scale subresource, merge-patch, stale rv                        [377 bytes]
   code: 409  reason: 'Conflict'  status: 'Failure'
   details keys: ['group', 'kind', 'name']   retryAfterSeconds: ABSENT   causes: ABSENT
   message: Operation cannot be fulfilled on deployments.apps "web": the object has been modified; please apply your changes to the latest version and try again
--- deployment, strategic-merge-patch, stale rv                     [377 bytes]   identical to the above
--- scale subresource, merge-patch, stale rv, dryRun=All            [377 bytes]   identical to the above
--- PUT scale (kubectl's --resource-version path), stale rv         [377 bytes]   identical to the above
--- DELETE deployment, preconditions.resourceVersion stale          [419 bytes]
   details: {"name": "del1", "group": "apps", "kind": "Deployment"}
   message: … the ResourceVersion in the precondition (947) does not match the ResourceVersion in record (989). The object might have been modified
--- DELETE deployment, preconditions.uid wrong                      [479 bytes]
   message: … the UID in the precondition (<uid>) does not match the UID in record (<uid>). The object might have been deleted and then recreated
--- DELETE node (cluster-scoped), preconditions.resourceVersion     [400 bytes]
   details: {"name": "review-worker", "kind": "Node"}     <- no 'group' key
```

Constant across all seven: `code: 409`, `reason: "Conflict"`,
`status: "Failure"`, top-level keys
`['apiVersion','code','details','kind','message','metadata','reason','status']`.
**`details.retryAfterSeconds` is absent in every one. `details.causes` is absent
in every one.** The `DELETE` preconditions messages name both resourceVersions;
the patch/PUT messages name **neither**.

No 409 body exceeded 479 bytes and none contained the canary — a conflict does
not echo the object.

### Edge cases of the value itself

```
=== empty-string resourceVersion in a PATCH body
http=200   kind: Scale                      <- treated as no precondition
=== a resourceVersion from the future ("99999999") in a PATCH body
http=409   reason: Conflict
=== a non-numeric resourceVersion ("not-a-number") in a PATCH body
http=500   kind: Status   reason: <absent>
           message: strconv.ParseUint: parsing "not-a-number": invalid syntax
=== empty-string resourceVersion in a DELETE precondition
http=409   message: … the ResourceVersion in the precondition () does not match the ResourceVersion in record (1430) …
=== non-numeric resourceVersion in a DELETE precondition
http=409   message: … the ResourceVersion in the precondition (not-a-number) does not match the ResourceVersion in record (1454) …
```

An empty string means *no precondition* on a patch and *a conflict that can
never clear* on a delete. A malformed value on a patch is a **500 with no
`reason` field**, not a 409.

### The 422 that echoes the object — not reproduced here

Four shapes were tried against this server, each with the planted canary in the
object, looking for the response D217 describes:

```
strategic patch, unknown field, fieldValidation=Strict   -> http=422  message 163 bytes  canary: False
PUT (replace), unknown field, fieldValidation=Strict     -> http=400  message 131 bytes  canary: False
POST (create), unknown field, fieldValidation=Strict     -> http=400  message 131 bytes  canary: False
POST (create), selector/label mismatch (Invalid)         -> http=422  message 149 bytes  canary: False   details keys: ['causes','group','kind','name']
PATCH statefulset, forbidden spec update (Invalid)       -> http=422  message 253 bytes  canary: False
```

The largest body among them was 820 bytes. This is **not** evidence against
D217 — it is evidence that I did not find the input shape that produces the
echo, on `v1.36.1`, in five tries.

## What I could not measure

- **The 4859-byte `Status.message` echo of D217.** Five input shapes, none
  reproduced it (above). Finding the shape that does was not in the brief and I
  stopped after five.
- **`Api::get_metadata` on the wire.** The `Accept` header it sends is read off
  `kube-core-4.2.0/src/request.rs:16` and matches the header I sent by hand;
  I did not build a Rust binary to confirm the request `kube` actually emits.
  Marked as source, not wire.
- **A 409 under real concurrency.** Every conflict here was manufactured by
  bumping the object with `kubectl label` between the read and the write. I did
  not run two writers racing, so nothing here says how often a real operator
  would hit one.
- **Whether `details.retryAfterSeconds` ever appears on a 409.** Absent in all
  seven bodies captured; a `Conflict` from some other path (a namespace being
  terminated, a resource-quota race) was not exercised.
- **A conflict during the window between a dry-run and the real call.** §3c
  proves the dry-run does not consume the resourceVersion; it does not measure
  what happens when the object moves *inside* that window. By §1's third block
  the real call would 409, but that specific sequence was not run.
- **`kubectl scale --current-replicas`.** The help text is quoted; I did not
  send it, so I do not know whether it too switches the verb to `PUT`.

## Machine state

```
$ free -g   # before the cluster
Mem: total 23  used 7  available 15
$ df -h /tmp
tmpfs  12G  4,2G  7,4G  37% /tmp
```

One cluster at a time: the PM's four `k8rs-*` containers were running
throughout and were not touched; `docker ps` after teardown listed exactly
those four, and `kind get clusters` printed `k8rs` alone.
