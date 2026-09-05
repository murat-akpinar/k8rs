# scale, measured against a stub API server and against kubectl v1.36.3

`k8s-admin`, 2026-09-04. Operator review of the `scale` box (todo.md 3749).
Everything below was run on the dev machine. **No cluster was created and
nothing was written into any cluster**: the mutations go to a local stub API
server on `127.0.0.1`, and the two cluster reads at the end are `kubectl get`
against built-in Kubernetes RBAC objects in the already-running fixture
cluster.

## The stub

A ~90-line Python `http.server` that answers discovery, answers
`GET .../scale` with an `autoscaling/v1 Scale` at `replicas: 2`, and writes
every request line, `Content-Type` and body to a log. Failure modes are
selected with `FAKE_MODE`. It lives in the scratchpad, not in the repo.

```
$ kubectl version --client
Client Version: v1.36.3
```

## 1. What kubectl scale puts on the wire

```
$ KUBECONFIG=<stub> kubectl scale deployment/web --replicas=3 -n payments
```

Stub request log, four kinds in sequence:

```
GET /api?timeout=32s
GET /apis?timeout=32s
GET /apis/autoscaling/v1?timeout=32s
GET /api/v1?timeout=32s
GET /apis/apps/v1?timeout=32s
GET /apis/apps/v1/namespaces/payments/deployments/web
PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale
    Content-Type: application/merge-patch+json
    body: {"spec":{"replicas":3}}
```

`statefulset/web`, `replicaset/web` and `daemonset/web` produce the same
shape against `statefulsets/web/scale`, `replicasets/web/scale` and
`daemonsets/web/scale`. No `dryRun`, no `fieldValidation`, no query string
at all on the PATCH.

## 2. What k8rs puts on the wire

```
$ echo yes | KUBECONFIG=<stub> XDG_STATE_HOME=<scratch> \
    ./target/debug/k8rs ops scale deployment/web 3 -n payments
```

Stub request log:

```
GET /version
GET /version
GET /apis
GET /api
GET /apis
GET /api
GET /api/v1
GET /apis
GET /apis/apps/v1
GET /apis
GET /apis/autoscaling/v1
GET /apis/apps/v1/namespaces/payments/deployments/web/scale
PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale?&dryRun=All&fieldValidation=Strict
    Content-Type: application/merge-patch+json
    body: {"spec":{"replicas":3}}
PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale?&fieldValidation=Strict
    Content-Type: application/merge-patch+json
    body: {"spec":{"replicas":3}}
```

Verb, path, patch type and body are identical to kubectl. The differences
are the two query parameters, the extra `dryRun=All` pass, and the read:
kubectl reads the whole object, k8rs reads the scale subresource.

Request counts before the first mutating call, against a stub that does not
serve aggregated discovery: kubectl 6, k8rs 12.

stdout was empty. stderr:

```
deployment/web in payments
This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies.
$ kubectl scale deployment/web --replicas=3 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it: k8rs: the change was made
```

exit 0. Audit log:

```
<stamp> attempt · deployment/web · context fake · server http://127.0.0.1:48765 · namespace payments · uid aaaa-bbbb · kubectl: kubectl scale deployment/web --replicas=3 -n payments · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale · resourceVersion not sent
result · attempt <stamp> · recorded <stamp> · deployment/web · dry-run: the cluster checked it first and accepted it · the change was made
```

The prompt has no trailing newline and stdin is a pipe, so the closing
sentence lands on the same physical line as the prompt.


## 3. Ten endings, as the operator sees them

Same invocation, `FAKE_MODE` varied. `stderr` is quoted whole; the audit
line is quoted only where it differs from stderr.

| what happened | exit | stderr, last line |
|---|---|---|
| confirmed, cluster accepts | 0 | `k8rs: the change was made` |
| `echo no` | 2 | `k8rs: nobody confirmed it, so nothing was changed` |
| empty stdin | 2 | `k8rs: nobody confirmed it, so nothing was changed` |
| 403 on the dry-run PATCH | 2 | `k8rs: the change was never sent` |
| 422 strict rejection on the dry-run PATCH | 2 | `k8rs: the change was never sent` |
| 403 on the get of the scale subresource | 2 | `k8rs: k8rs could not read how many copies of deployment/web are running right now — the cluster would not allow it: deployments.apps "web" is forbidden: User "dev" cannot get resource "deployments/scale" in API group "apps" in the namespace "payments"` |
| 404 on the get of the scale subresource | 2 | `k8rs: k8rs could not read how many copies of deployment/wbe are running right now — the cluster has no such object any more: deployments.apps "wbe" not found` |
| 403 on the real PATCH, dry-run accepted | 2 | `k8rs: nothing was changed: the cluster would not allow it` |
| socket closed on the real PATCH | 2 | `k8rs: k8rs does not know whether the change was made — k8rs could not reach the cluster` |
| `--read-only` on the line | 2 | `k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation` |

Four of these are worth their full text.

**403 on the dry-run PATCH.** stderr, whole:

```
deployment/web in payments
This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies.
$ kubectl scale deployment/web --replicas=3 -n payments
k8rs: the change was never sent
```

The audit line for the same run:

```
result · attempt <stamp> · recorded <stamp> · deployment/web · dry-run: not checked, the cluster would not allow it · the change was never sent: deployments.apps "web" is forbidden: User "kubernetes-admin" cannot patch resource "deployments/scale" in API group "apps" in the namespace "payments"
```

**422 strict rejection on the dry-run PATCH.** stderr, last line, byte
identical to the 403 above:

```
k8rs: the change was never sent
```

Audit line:

```
result · … · dry-run: not checked, the cluster would not accept the request k8rs made · the change was never sent: Scale in version "v1" cannot be handled as a Scale: strict decoding error: unknown field "spec.bogus"
```

**403 on the real PATCH.** stderr last line names the fault class and not
the server sentence:

```
k8rs: nothing was changed: the cluster would not allow it
```

Audit line carries `: deployments.apps "web" is forbidden: User "dev" cannot patch resource "deployments/scale" …`.

**Scale to a count that is already running.** `ops scale deployment/web 2`
against a stub reporting `replicas: 2`:

```
deployment/web in payments
This makes no change — web is already running 2 copies. Right now: 2 copies. After: 2 copies.
$ kubectl scale deployment/web --replicas=2 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it: k8rs: the change was made
```

exit 0, both PATCHes sent, audit result line `· the change was made`.

## 4. Where the audit log has nothing in it

`XDG_STATE_HOME` pointed at an empty scratch directory for every run.

| run | state directory | audit.log | lines in it |
|---|---|---|---|
| `--read-only ops scale …` | not made | not made | — |
| `ops scale deployment/web 3` (no `-n`) | not made | not made | — |
| `ops scale pod/web 3 -n payments` | not made | not made | — |
| 403 on the get of the scale subresource | made | made | **0** |
| 404 on the get of the scale subresource | made | made | **0** |
| cancelled | made | made | 2 |
| every ending after the dialog opens | made | made | 2 |

## 5. Performed::plainly, printed by its own test

```
$ cargo test everything_that_is_not_a_change_says_so_and_none_of_it_exits_zero -- --nocapture
running 1 test
nothing was changed — k8rs could not write this to its audit log first, and every change k8rs makes is written to that log before it is sent
nobody confirmed it, so nothing was changed
the object was already gone, so nothing was changed
the object changed while this was open, so nothing was changed
the change was never sent
nothing was changed: the object had already been changed by something else
.
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 977 filtered out
```

The fifth line is `Outcome::NotSent { fault: Fault::Refused, said: None }`.
`verdict` has no arm that reads `fault` or `said` for `NotSent`, and
`plainly` appends neither.


## 6. RBAC: what resource a scale is authorized against

Two `kubectl get` reads against the running fixture cluster, server
v1.36.1. Both objects are upstream Kubernetes ClusterRoles, shipped with
every cluster; nothing cluster-specific is reproduced.

```
$ kubectl get clusterrole edit -o json   # rules mentioning deployments
apiGroups=[apps] verbs=[create, delete, deletecollection, patch, update]
  resources=[deployments, deployments/rollback, deployments/scale]
apiGroups=[apps] verbs=[get, list, watch]
  resources=[deployments, deployments/scale, deployments/status]
```

```
$ kubectl get clusterrole system:controller:horizontal-pod-autoscaler -o json
apiGroups=[autoscaling] resources=[horizontalpodautoscalers] verbs=[get, list, watch]
apiGroups=[autoscaling] resources=[horizontalpodautoscalers/status] verbs=[update]
apiGroups=[*] resources=[*/scale] verbs=[get, update]
apiGroups=[] resources=[pods] verbs=[list, watch]
…
```

Upstream lists `deployments/scale` as its own resource string in both the
read rule and the write rule of `edit`, and the autoscaler controller is
granted `*/scale` and no verb at all on the workloads themselves.

`docs/security.md:221-223`, the documented `k8rs-admin` ClusterRole, as
landed:

```
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets"]
    verbs: ["patch", "update"]       # scale, rollout restart, edit
```

`docs/security.md:184-186`, the documented `k8rs-readonly` ClusterRole:

```
  - apiGroups: ["apps"]
    resources: ["deployments", "statefulsets", "daemonsets", "replicasets"]
    verbs: ["get", "list", "watch"]
```

Neither names any `/scale` resource. The two calls `ops scale` makes are
`get` and `patch` on `<plural>/scale`.

## 7. ReplicaSets in the fixture cluster

```
$ kubectl get replicasets -A -o json   # counted by controlling ownerReference
replicasets owned by a Deployment: 8
replicasets with no controlling Deployment: 0
```

## 8. Machine state during this review

```
$ df -h /tmp
tmpfs            12G   12G     0 100% /tmp
```

`/tmp` was at 100% for the whole session, 7.9G of it one Claude session
scratch directory that is not mine. Every cargo invocation here ran with
`CARGO_TARGET_DIR` and `TMPDIR` under `$HOME`; the first one without them
failed inside `cc` with `No space left on device`.

