# `delete` against a cluster — RBAC, the six taught lines, five audit records, and the object that was not gone

`k8s-admin`, 2026-09-04. The step-6 operator review of todo.md 3808, measured
rather than reasoned. One ephemeral `K8RS_CLUSTER=review` kind cluster,
`kind v0.32.0`, node image `kindest/node:v1.36.1`, server `v1.36.1`, `kubectl`
client `v1.36.3`, apiserver on `127.0.0.1:6444` so the PM's fixture cluster on
`:6443` was untouched. Its own kubeconfig in a scratch file, so the user's
current context never moved. Torn down through a `trap … EXIT` in a detached
watchdog with a hard deadline, not on a last line (NOTES § D185); afterwards
`kind get clusters` printed `k8rs` and nothing else and `docker ps` listed the
PM's four containers and nothing else.

The binary under test is the uncommitted working tree, copied to a scratch
directory and built with its own `CARGO_TARGET_DIR` — the repo's tree was read
and never written, and no `cargo` ran in it. `md5sum` of `src/ops.rs` matched
between the two before the build.

No committed artifact was produced and nothing was written into `tests/`.

## 1. Can the documented `k8rs-admin` role delete each of the six kinds?

Both ClusterRoles extracted **verbatim** from `docs/security.md` by a script
(two ```yaml blocks containing `kind: ClusterRole`), applied unedited, and both
bound to one ServiceAccount:

```
$ kubectl apply -f roles.yaml
clusterrole.rbac.authorization.k8s.io/k8rs-readonly created
clusterrole.rbac.authorization.k8s.io/k8rs-admin created
$ kubectl create clusterrolebinding jane-k8rs-readonly --clusterrole=k8rs-readonly --serviceaccount=default:jane
$ kubectl create clusterrolebinding jane-k8rs-admin    --clusterrole=k8rs-admin    --serviceaccount=default:jane
```

```
$ kubectl auth can-i delete <resource> --as=system:serviceaccount:default:jane [-n payments]

KIND         RESOURCE                 VERB     can-i
deployment   deployments.apps         delete   no
statefulset  statefulsets.apps        delete   no
daemonset    daemonsets.apps          delete   no
replicaset   replicasets.apps         delete   no
pod          pods                     delete   yes
node         nodes                    delete   no
```

Confirmed by real calls under that identity, not only by the review API:

```
$ curl -X DELETE -d '{"propagationPolicy":"Background"}' …/apis/apps/v1/namespaces/payments/deployments/web
HTTP 403
message : deployments.apps "web" is forbidden: User "system:serviceaccount:default:jane"
          cannot delete resource "deployments" in API group "apps" in the namespace "payments"
details : {"name": "web", "group": "apps", "kind": "deployments"}

$ curl -X DELETE -d '{"propagationPolicy":"Background"}' …/api/v1/nodes/review-worker
HTTP 403
message : nodes "review-worker" is forbidden: User "system:serviceaccount:default:jane"
          cannot delete resource "nodes" in API group "" at the cluster scope
details : {"name": "review-worker", "kind": "nodes"}
```

The cluster-scoped refusal ends *"at the cluster scope"* where the namespaced
one ends *"in the namespace …"*; `details` carries no `namespace` key for the
node.

## 2. The six taught lines, run through the real binary

`XDG_STATE_HOME` fresh per run; the answer piped on stdin.

```
$ echo web-847f49cc4d-qgc8x | k8rs ops delete pod/web-847f49cc4d-qgc8x -n payments
pod/web-847f49cc4d-qgc8x in payments
This removes the pod. Whatever created it will normally replace it — k8rs has not checked whether anything did.
$ kubectl delete pod/web-847f49cc4d-qgc8x -n payments
k8rs did not check this one with the cluster first
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: the change was made
exit=0
```

The `$` line, the audit `call:` path and the exit code for each kind:

| kind | taught line | audit `call:` | exit |
|---|---|---|---|
| pod | `kubectl delete pod/web-847f49cc4d-qgc8x -n payments` | `DELETE /api/v1/namespaces/payments/pods/web-847f49cc4d-qgc8x` | 0 |
| deployment | `kubectl delete deployment/doomed -n payments` | `DELETE /apis/apps/v1/namespaces/payments/deployments/doomed` | 0 |
| statefulset | `kubectl delete statefulset/db -n payments` | `DELETE /apis/apps/v1/namespaces/payments/statefulsets/db` | 0 |
| daemonset | `kubectl delete daemonset/agent -n payments` | `DELETE /apis/apps/v1/namespaces/payments/daemonsets/agent` | 0 |
| replicaset | `kubectl delete replicaset/web-847f49cc4d -n payments` | `DELETE /apis/apps/v1/namespaces/payments/replicasets/web-847f49cc4d` | 0 |
| node | `kubectl delete node/node-3` | `DELETE /api/v1/nodes/node-3` | 0 |

The node line carries no `-n` and the node path carries no `namespaces/` segment.

`kubectl` sending the same request for the same kind, at `--v=9`:

```
$ kubectl delete deployment/reprod -n payments --v=9
  kubectl BODY: {"propagationPolicy":"Background"}
  kubectl REQ : curl -v -XDELETE … 'https://127.0.0.1:6444/apis/apps/v1/namespaces/payments/deployments/reprod'
$ kubectl delete node/node-11 --v=9
  curl -v -XDELETE … 'https://127.0.0.1:6444/api/v1/nodes/node-11'
```

`kubectl` given a `-n` on a cluster-scoped delete does not refuse it:

```
$ kubectl delete node/review-control-plane -n payments --dry-run=client
Warning: deleting cluster-scoped resources, not scoped to the provided namespace
node "review-control-plane" deleted (dry run)
```

k8rs refuses the same line before dialling:

```
$ k8rs ops delete node/node-9 -n payments
k8rs: a node belongs to the whole cluster and is in no namespace, so `ops delete` will not take -n — leave it off
exit=2
```

## 3. Five audit records, whole

Confirmed (namespaced):

```
2026-09-04T20:15:20.422324691Z attempt · deployment/doomed · context kind-review · server https://127.0.0.1:6444 · namespace payments · uid not read · kubectl: kubectl delete deployment/doomed -n payments · call: DELETE /apis/apps/v1/namespaces/payments/deployments/doomed · resourceVersion not sent
result · attempt 2026-09-04T20:15:20.422324691Z · recorded 2026-09-04T20:15:20.426480978Z · deployment/doomed · dry-run: k8rs did not check this one with the cluster first · the change was made
```

Cancelled (`yes` typed where the name was wanted):

```
result · … · deployment/cancelme · dry-run: k8rs did not check this one with the cluster first · nobody confirmed it, so nothing was changed
```

Object absent:

```
result · … · deployment/nosuchthing · dry-run: k8rs did not check this one with the cluster first · nothing was changed — the cluster has no object with that name: deployments.apps "nosuchthing" not found
```

Refused by RBAC (run under the ServiceAccount of § 1):

```
result · … · deployment/forbidden · dry-run: k8rs did not check this one with the cluster first · nothing was changed — the cluster would not allow it: deployments.apps "forbidden" is forbidden: User "system:serviceaccount:default:jane" cannot delete resource "deployments" in API group "apps" in the namespace "payments"
```

Cluster-scoped node — the field that differs is the namespace gap:

```
2026-09-04T20:15:20.53800623Z attempt · node/node-3 · context kind-review · server https://127.0.0.1:6444 · cluster-wide · uid not read · kubectl: kubectl delete node/node-3 · call: DELETE /api/v1/nodes/node-3 · resourceVersion not sent
```

Refusals that happen before a `Mutation` exists write no line and create no file
(`ls` of the state directory after each): a word naming no kind, a namespaced
kind with no `-n`, a `-n` on a node, a name that is not addressable, and
`--read-only`. Five runs, `audit: no file`.

File modes after a run: `600 …/k8rs/audit.log`, `700 …/k8rs`.

## 4. What a failed DELETE hands back — is there an object in it?

Raw `DELETE`s through `kubectl proxy` with admin credentials, so the status is
about the body and not about RBAC. Fields printed, never the record:

```
=== invalid propagationPolicy   body {"propagationPolicy":"Bogus"}
    kind Status  reason Invalid  code 422  msglen 149
    message : DeleteOptions.meta.k8s.io "" is invalid: propagationPolicy: Unsupported value: "Bogus": supported values: "Foreground", "Background", "Orphan", "nil"
    details : {"group": "meta.k8s.io", "kind": "DeleteOptions", "causes": [{"reason": "FieldValueNotSupported", "field": "propagationPolicy"}]}
=== wrong-type field            body {"gracePeriodSeconds":"soon"}
    kind Status  reason BadRequest  code 400  msglen 97
    message : json: cannot unmarshal string into Go struct field DeleteOptions.gracePeriodSeconds of type int64
=== object that is not there    body {"propagationPolicy":"Background"}
    kind Status  reason NotFound  code 404  msglen 40
    message : deployments.apps "nosuchthing" not found
```

The `422`'s `details.kind` is `DeleteOptions`. No object appears in any of them.

`fieldValidation=Strict` on a `DELETE` catches nothing, measured on two live
objects:

```
=== unknown DeleteOptions field, NO fieldValidation      HTTP 200  kind Status status Success
=== unknown DeleteOptions field WITH ?fieldValidation=Strict  HTTP 200  kind Status status Success
=== are s1 and s2 gone?
Error from server (NotFound): deployments.apps "s1" not found
Error from server (NotFound): deployments.apps "s2" not found
```

## 5. What a *successful* DELETE hands back, and whether the object is gone

Pod, graceful deletion in progress — fields off the 200 response:

```
kind                 : Pod
status.phase         : Running
deletionTimestamp    : 2026-09-04T20:05:31Z
deletionGracePeriod  : 30
has spec.containers  : True
has managedFields    : True
```

Deployment, deletion complete — `kind: Status`, `status: Success`, no object.

Node carrying a finalizer:

```
$ curl -X DELETE -d '{"propagationPolicy":"Background"}' …/api/v1/nodes/managed-node-2
HTTP 200
response kind      : Node
deletionTimestamp  : 2026-09-04T20:10:46Z
finalizers         : ['example.com/termination']
$ kubectl get nodes --no-headers
managed-node-1         Unknown   <none>          6s
managed-node-2         Unknown   <none>          0s
review-control-plane   Ready     control-plane   7m4s   v1.36.1
```

The same shape through the real binary:

```
$ echo managed-node-9 | k8rs ops delete node/managed-node-9
node/managed-node-9
This removes the cluster's record of managed-node-9, not the machine. Nothing is drained: its pods are deleted, and replacements may have nowhere to go. The machine keeps running — only a kubelet restart brings it back.
$ kubectl delete node/managed-node-9
k8rs did not check this one with the cluster first
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: the change was made
exit=0
result · … · node/managed-node-9 · dry-run: k8rs did not check this one with the cluster first · the change was made
--- and the node, 3 s later ---
NAME             DELETED                FINALIZERS
managed-node-9   2026-09-04T20:15:49Z   [example.com/termination]
```

The taught line on the same shape:

```
$ timeout 5 kubectl delete node/managed-node-10
node "managed-node-10" deleted
kubectl exit=124 (124 = still waiting)
```

The pod half of the same behaviour, met by accident: a pod held by a finalizer
kept `kubectl delete pod stuck --force --grace-period=0` blocked past a 180 s
tool timeout, and only removing the finalizer freed it.

## 6. Transport failure, with and without a preflight

Nothing listening on `:6499`:

```
$ echo web | k8rs ops delete deployment/web -n payments
…
k8rs: k8rs does not know whether the change was made — k8rs could not reach the cluster
exit=2
audit lines: 2
```

The same fault on a checkable operation reaches `Outcome::NotSent` instead,
whose sentence is *"the change was never sent — k8rs could not reach the
cluster"* (`src/ops.rs` `verdict`, and NOTES § D222's measurement of the same
arm on `scale`).

Missing kubeconfig, and an audit log that cannot be opened:

```
$ KUBECONFIG=<absent> k8rs ops delete deployment/web -n payments
k8rs: nothing was changed — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML
exit=2

$ XDG_STATE_HOME=<dir mode 500> k8rs ops delete deployment/cancelme -n payments
k8rs: k8rs could not open its audit log at …/k8rs/audit.log (from $XDG_STATE_HOME): Permission denied (os error 13) — every change k8rs makes is written to that log before it is sent, so k8rs will not change anything until that is fixed, and reading your cluster still works
exit=2
is cancelme still there?
cancelme   0/1   1     0     95s
```

## 7. Confirmation, streams and leaks

```
$ echo CANCELME | k8rs ops delete deployment/cancelme -n payments
k8rs: nobody confirmed it, so nothing was changed
$ printf '' | k8rs ops delete deployment/cancelme -n payments
k8rs: nobody confirmed it, so nothing was changed
$ k8rs ops delete 'pod/../../nodes/review-control-plane' -n payments
k8rs: ../../nodes/review-control-plane is not the name of an object — a name is letters, digits, dashes and dots, up to 253 characters
$ k8rs --read-only ops delete deployment/cancelme -n payments
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation
```

Streams, one confirmed delete: `stdout bytes: 0`, `stderr bytes: 330`.

A container environment value was planted in the StatefulSet before it was
deleted. `grep` for that literal across the run's stdout, stderr and audit log:
0 occurrences in each. The literal is not reproduced here.

## 8. Node deletion, second run — what goes with the object

```
$ kubectl delete node/review-worker
node "review-worker" deleted     real 0m0,082s

-- t+3s --   node: NotFound   lease: NotFound   container: Up 4 minutes   kubelet: active
-- t+12s --  node: NotFound   lease: NotFound   container: Up 4 minutes   kubelet: active
-- t+30s --  node: NotFound   lease: NotFound   container: Up 5 minutes   kubelet: active
-- t+60s --  node: NotFound   lease: NotFound   container: Up 6 minutes   kubelet: active
```

The `Lease` in `kube-node-lease` carried `ownerReferences: Node/review-worker`
before the delete and answered `NotFound` at the first sample 3 s after it. Why
it went was not measured; the owner reference is the only fact taken. No new CSR appeared in the 60 s watched; the `csr` list was unchanged.

Pods, after the delete:

```
NS         NAME                     NODE            PHASE       DELETED
payments   stuck                    review-worker   Succeeded   2026-09-04T20:05:19Z
payments   web-847f49cc4d-qgc8x     <none>          Pending     <none>
```

The pod carrying a finalizer stayed, with `spec.nodeName` naming a node that no
longer exists.

## 9. Machine state

```
$ free -g   # before the cluster
Mem: total 23  used 9  available 13
$ df -h /tmp
tmpfs  12G  11G  694M  95% /tmp
$ df -h /home
/dev/nvme0n1p2  954G  57G  896G  6% /home
```

`/tmp` was at 95%, so the scratch build and the review cluster's files were
placed under `/home` (NOTES § D133 is the same trap on a different tool). One
cluster at a time: the PM's four `k8rs-*` containers ran throughout, were read
once (`kubectl get nodes`) and never written; `kind get clusters` after teardown
printed `k8rs`.
