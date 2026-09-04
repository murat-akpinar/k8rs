# `restart` against a real apiserver — operator review measurements (2026-09-04)

Three ephemeral `K8RS_CLUSTER=review` clusters, kind v0.32.0, node image
`kindest/node:v1.36.1`, server `v1.36.1`, `kubectl` client `v1.36.3`, apiserver
bound to `127.0.0.1:6444` so the PM's fixture cluster on `:6443` was untouched.
Each run created the cluster, measured, and deleted it from a `trap … EXIT`
(NOTES § D185). Every measurement below was taken on the dev machine. No
committed artifact was produced; nothing was written into `tests/`.

Round 3 ran **the real binary** (`cargo build`, own `CARGO_TARGET_DIR`, own
`XDG_STATE_HOME`) against the cluster. Rounds 1 and 2 used `kubectl` and raw
`PATCH` through `kubectl proxy`.

---

## 1. `kubectl rollout restart` on a `strategy: Recreate` Deployment

3 replicas, `pause:3.10`, sampled every 0.4 s.

```
$ kubectl -n payments get deployment recreate-web -o jsonpath='{.spec.strategy.type} ...'
Recreate replicas=3 ready=3 avail=3
$ kubectl -n payments rollout restart deployment/recreate-web
deployment.apps/recreate-web restarted
sample=1 replicas=3 ready= avail= updated=3 unavail=3
sample=2 replicas=3 ready= avail= updated=3 unavail=3
sample=3 replicas=3 ready=3 avail=3 updated=3 unavail=
```

`readyReplicas` and `availableReplicas` are absent (0) for ~0.8 s with
`unavailableReplicas=3`. **Every copy stops before any starts.**

Same object shape with the default `RollingUpdate` (`maxUnavailable=25%`,
`maxSurge=25%`), for comparison:

```
sample=1 replicas=4 ready=3 avail=3
sample=3 replicas=4 ready=3 avail=3
```

`availableReplicas` never left 3; `replicas` surged to 4.

## 1b. A `RollingUpdate` Deployment with `maxSurge: 0`

Not `Recreate`. `strategy=RollingUpdate maxSurge=0 maxUnavailable=3`:

```
$ kubectl -n payments rollout restart deployment/nosurge
deployment.apps/nosurge restarted
s=1 replicas=3 ready= avail= unavail=3
s=2 replicas=3 ready= avail= unavail=3
s=3 replicas=3 ready=3 avail=3 unavail=
```

`availableReplicas` 0 on a Deployment whose `strategy.type` is `RollingUpdate`.

## 1c. A paused Deployment

```
$ kubectl -n payments rollout pause deployment/web
$ kubectl -n payments rollout restart deployment/web
error: deployments.apps "web" can't restart paused deployment (run rollout resume first)
   kubectl exit=1
```

The same object, patched the way `ops::restart` patches it (raw `PATCH`,
strategic merge, `fieldValidation=Strict`, both passes):

```
check pass  http=200 kind=Deployment
real  pass  http=200 kind=Deployment
generation 3 -> 4 ; spec.paused=true
pods before: web-596449b4d-8j727 web-596449b4d-djnf9 web-596449b4d-z9nsc
pods  after: web-596449b4d-8j727 web-596449b4d-djnf9 web-596449b4d-z9nsc
```

Identical pod names 12 s later: not one copy replaced.

## 2. `updateStrategy: OnDelete`

StatefulSet, 2 replicas:

```
$ kubectl -n payments rollout restart statefulset/ondelete-sts     # exit 0
pods before restart:  ondelete-sts-0 created=2026-09-04T14:42:28Z
pods 15s after:       ondelete-sts-0 created=2026-09-04T14:42:28Z
                      ondelete-sts-1 created=2026-09-04T14:42:29Z
updateStrategy=OnDelete currentRevision=…-7f9cd54644 updateRevision=…-f88864884 updated=1 ready=2
```

`currentRevision != updateRevision`; the pod that existed before the restart is
byte-for-byte the same pod afterwards.

DaemonSet, 3 nodes:

```
$ kubectl -n payments rollout restart daemonset/ondelete-ds        # exit 0
pods before: 3 pods, all created=2026-09-04T14:42:44Z
pods 15s after: the same 3 names, the same creationTimestamps
updateStrategy=OnDelete desired=3 updated= ready=3
```

`updatedNumberScheduled` is absent (0).

The command an operator would run next after the taught line:

```
$ kubectl -n payments rollout status statefulset/ondelete-sts --timeout=20s
error: rollout status is only available for RollingUpdate strategy type
  exit=1
```

Same for the DaemonSet.

## 3. `maxUnavailable` above 1, and two knobs nobody named

**DaemonSet, `rollingUpdate.maxUnavailable: 3`** (GA, no feature gate),
3 nodes:

```
$ kubectl -n payments get daemonset fast-ds -o jsonpath='...'
maxUnavailable=3 desired=3
$ kubectl -n payments rollout restart daemonset/fast-ds
sample=1 desired=3 ready=0 avail= unavail=3 updated=3
sample=2 desired=3 ready=0 avail= unavail=3 updated=3
sample=3 desired=3 ready=3 avail=3 unavail= updated=3
```

`numberReady=0` on every node at once.

**StatefulSet, `rollingUpdate.maxUnavailable: 3`.** On a default cluster the
apiserver silently drops the field:

```
maxUnavailable= partition=0        # field absent after apply
```

On a cluster created with `featureGates: {MaxUnavailableStatefulSet: true}` it
is kept and honoured:

```
maxUnavailable=3 partition=0
$ kubectl -n payments rollout restart statefulset/db
s=1 replicas=1 ready= avail=0 updated=1
s=2 replicas=2 ready=1 avail=1 updated=2
```

`availableReplicas=0` against 3 desired.

**StatefulSet `partition`** (GA, no gate), 3 replicas, `partition: 2`:

```
pods before:    db-0=…T14:46:18Z  db-1=…T14:46:19Z  db-2=…T14:46:19Z
pods 25s after: db-0=…T14:46:18Z  db-1=…T14:46:19Z  db-2=…T14:46:32Z
partition=2 updated=1 current=2 ready=3
```

Only the highest ordinal was replaced. `kubectl rollout status` reports
`partitioned roll out complete` and exits 0.

**DaemonSet with a `nodeSelector`**, 3-node cluster:

```
nodes in cluster: 3
desiredNumberScheduled=1 numberReady=1
```

## 4. `fieldValidation=Strict` — what a 422 hands back, per media type

One Deployment carrying a container `env` entry with a planted literal, plus a
second entry sourced from a Secret. Raw `PATCH` through `kubectl proxy`, unknown
field `spec.wat`, `fieldValidation=Strict`. Byte counts are of `.message` on the
returned `Status`.

| Content-Type | HTTP | `len(.message)` | carries the planted env literal | carries `managedFields` | carries `containers` |
|---|---|---|---|---|---|
| `application/strategic-merge-patch+json` | 422 | **109** | no | no | no |
| `application/merge-patch+json` | 422 | **4895** | **yes** | **yes** | **yes** |
| `application/json-patch+json` | 422 | **4895** | **yes** | **yes** | **yes** |

The strategic message, verbatim:

```
 "" is invalid: patch: Invalid value: "map[spec:map[wat:1]]": strict decoding error: unknown field "spec.wat"
```

The merge message begins with the whole patched object
(`{\"apiVersion\":\"apps/v1\",\"kind\":\"Deployment\",\"metadata\":{...`).

Dry-run is not what shortens it — the strategic 422 is **109 bytes on both
passes**, `dryRun=All` and real. Deeper unknown fields under strategic merge:

```
unknown field spec.template.spec.nope                     len(.message)=235
unknown field spec.template.spec.containers[0].bogusField len(.message)=256
```

The 235-byte message quotes only k8rs's own patch. The 256-byte one **does
carry a literal that was in the patch k8rs sent** — an `env` value placed in the
patch body for the test. Nothing from the server's copy of the object appeared
in any strategic-merge message.

Not measured: a strict 422 on the `autoscaling/v1` `/scale` subresource.
`scale`'s `Patch::Merge` safety is still read off `storage.go`.

## 5. Equivalence — what `kubectl rollout restart` sends

```
$ kubectl -n payments rollout restart deployment/rolling-web --v=8
"Request Body" body="{\"spec\":{\"template\":{\"metadata\":{\"annotations\":{\"kubectl.kubernetes.io/restartedAt\":\"2026-09-04T17:43:24+03:00\"}}}}}"
	Content-Type: application/strategic-merge-patch+json
```

Local offset, not `Z`. Same key, same path, same media type k8rs uses.

Sending k8rs's own patch (same key, `Z`, nanosecond precision, strategic,
strict) after a kubectl restart:

```
generation before=4 after=5
template annotation keys afterwards: exactly one restartedAt, holding k8rs's value
replicaset count before=4 after=5
```

Re-sending the identical stamp:

```
generation 5 -> 5
```

## 6. Missing object and missing namespace, under `dryRun=All`

```
PATCH …/namespaces/payments/deployments/wbe?dryRun=All&fieldValidation=Strict
http=404 reason=NotFound  message='deployments.apps "wbe" not found'
        details={"name":"wbe","group":"apps","kind":"deployments"}

PATCH …/namespaces/nosuchns/deployments/web?dryRun=All&fieldValidation=Strict
http=404 reason=NotFound  message='deployments.apps "web" not found'
```

The message is identical for a wrong name and a wrong namespace.

## 7. RBAC — the documented `k8rs-admin` role

`docs/security.md`'s role applied verbatim, bound to a user with a real client
certificate and nothing else.

`kubectl auth can-i`, impersonated:

```
patch deployments.apps          -> yes
patch statefulsets.apps         -> yes
patch daemonsets.apps           -> yes
get   deployments.apps          -> no
patch deployments.apps/scale    -> yes
get   deployments.apps/scale    -> no
```

Raw `PATCH` — what `ops::restart` sends, no `GET`:

```
check pass, deployment    http=200
real  pass, deployment    http=200
statefulset               http=200
daemonset                 http=200
```

The real binary, running as that user and nothing more:

```
$ echo 'yes' | k8rs ops restart deploy/web -n payments
k8rs: the change was made
  exit=0
```

The command it printed, run by the same user:

```
$ kubectl rollout restart deployment/web -n payments
Error from server (Forbidden): deployments.apps "web" is forbidden: User "opsuser"
cannot get resource "deployments" in API group "apps" in the namespace "payments"
  kubectl exit=1
```

With the binding removed (no rights at all):

```
$ echo 'yes' | k8rs ops restart deploy/web -n payments
k8rs: the change was never sent — the cluster would not allow it: deployments.apps "web"
is forbidden: User "opsuser" cannot patch resource "deployments" in API group "apps" in
the namespace "payments"
  exit=2
```

No crash, no retry, the verb and the resource named.

## 8. The real binary, end to end

```
$ echo 'yes' | k8rs ops restart deploy/web -n payments
deployment/web in payments
This replaces every copy of your app with a new one, a few at a time, so the copies still running keep serving — unless this deployment is set up to stop them all first.
$ kubectl rollout restart deployment/web -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
  exit=0
```

The audit log those two produced (server URL replaced here):

```
2026-09-04T14:49:17.39006164Z attempt · deployment/web · context kind-review · server <url> · namespace payments · uid not read · kubectl: kubectl rollout restart deployment/web -n payments · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web · resourceVersion not sent
result · attempt 2026-09-04T14:49:17.39006164Z · recorded 2026-09-04T14:49:17.4007087Z · deployment/web · dry-run: the cluster checked it first and accepted it · the change was made
```

The value that reached the cluster: the restart annotation held
`2026-09-04T14:49:17.390020991Z` — 41 µs before the attempt line's own stamp,
and on no line of the log.

A 404 through the real binary:

```
$ echo 'yes' | k8rs ops restart deploy/wbe -n payments
k8rs: the change was never sent — the cluster has no object with that name: deployments.apps "wbe" not found
  exit=2
```

and its audit lines:

```
… attempt · deployment/wbe · … · call: PATCH /apis/apps/v1/namespaces/payments/deployments/wbe · resourceVersion not sent
result · … · deployment/wbe · dry-run: not checked · the change was never sent — the cluster has no object with that name: deployments.apps "wbe" not found
```

`dry-run: not checked` on a `dryRun=All` request that went out and was answered
404.

Other paths, all through the real binary:

```
$ echo 'no'  | k8rs ops restart deploy/web -n payments
k8rs: nobody confirmed it, so nothing was changed              exit=2

$ echo 'yes' | k8rs ops restart pod/web-xyz -n payments
k8rs: k8rs will not restart a pod: restarting a pod means deleting it and letting whatever made it start another one, which is not what the word restart does here — k8rs restarts a deployment, a statefulset and a daemonset
                                                               exit=2

$ echo 'yes' | k8rs ops restart rs/web-abc -n payments
k8rs: k8rs cannot restart a replicaset — restarting replaces the copies an object is running, and k8rs does that for a deployment, a statefulset and a daemonset
                                                               exit=2

$ echo 'yes' | k8rs --read-only ops restart deploy/web -n payments
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation
                                                               exit=2
```

Audit log after all of the above: 14 lines / 7 records, mode `600`, one record
per attempt including the cancellation, both 404s and the 403. Grepping it for
the planted env literal returns 0.
