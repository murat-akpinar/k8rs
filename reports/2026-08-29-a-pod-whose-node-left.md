# A pod that names a node the cluster no longer has — measured

`k8s-admin`, 2026-08-29, dev machine, ephemeral kind cluster `k8rs-review`
(`K8RS_CLUSTER=review`; node names `k8rs-review-control-plane` /
`k8rs-review-worker`, which `scripts/sanitize.jq` refuses — NOTES § D94).
Two nodes, `kindest/node:v1.36.1`, server `v1.36.1`, no metrics-server.
Evidence for the review of NOTES § D183.

The workload under test throughout: `kubectl create deployment web
--image=registry.k8s.io/pause:3.10 --replicas=2` with a
`nodeSelector` of `kubernetes.io/hostname` pinned to the worker.

## 1. A pod bound by hand to a node name that never existed

```
kubectl run ghost-bound --image=registry.k8s.io/pause:3.10 --restart=Never \
  --overrides='{"spec":{"nodeName":"ghost-node-that-never-existed"}}'
```

Polled every 5s with
`kubectl get pod ghost-bound -o json | jq -c '{phase, conds:[.status.conditions[]?|{t:.type,s:.status}], cs:(.status.containerStatuses|length)}'`:

```
t+0s   {"phase":"Pending","conds":[],"cs":0,"node":"ghost-node-that-never-existed","del":false}
t+27s  {"phase":"Pending","conds":[],"cs":0,"node":"ghost-node-that-never-existed","del":false}
t+54s  {"phase":"Pending","conds":[],"cs":0,"node":"ghost-node-that-never-existed","del":false}
t+60s  GONE from the API
```

Field values the review turns on: **`status.conditions` is empty** — no
`PodScheduled` line of any status — `status.containerStatuses` is absent, phase
stays `Pending`, and the pod GC removes the object between t+54s and t+60s.
`kubectl get events --field-selector involvedObject.name=ghost-bound` returned
nothing.

## 2. `kubectl delete node` while that machine's kubelet is still running

```
kubectl get pods -o custom-columns=NAME:...,PHASE:...,NODE:...   # before
web-64845cdff-n7jvd    Running     k8rs-review-worker
web-64845cdff-z4wr8    Running     k8rs-review-worker
kubectl delete node k8rs-review-worker      # returned at t+1s
```

```
t+1s   nodes: [k8rs-review-control-plane]  pods: n7jvd/Running/k8rs-review-worker z4wr8/Running/k8rs-review-worker
t+46s  nodes: [k8rs-review-control-plane]  pods: n7jvd/Running/k8rs-review-worker z4wr8/Running/k8rs-review-worker
t+53s  nodes: [k8rs-review-control-plane]  pods: 2s4tl/Pending/<none> kwq4z/Pending/<none>
t+146s nodes: [k8rs-review-control-plane]  pods: 2s4tl/Pending/<none> kwq4z/Pending/<none>
```

- The two pods kept `phase: Running` and their `spec.nodeName` for ~50s while
  no node of that name existed.
- **The node did not come back.** `docker exec k8rs-review-worker systemctl
  is-active kubelet` → `active`, and `kubectl get nodes` still listed only the
  control plane 6 minutes after the delete. Only restarting the container
  brought the node back: after `docker restart`, the node object reappeared
  within 10s and reached `Ready`.

## 3. The machine leaves for real, and one pod is held back from the pod GC

`docker stop k8rs-review-worker`, then `kubectl delete node
k8rs-review-worker`. One of the two pods was first patched with a finalizer
(`metadata.finalizers: ["k8rs.review/hold"]`) so the shape would survive the
collector.

```
t+1s   {"name":"...-2s4tl","phase":"Running","node":"k8rs-review-worker","ready":"True","state":"running","del":false}
       {"name":"...-kwq4z","phase":"Running","node":"k8rs-review-worker","ready":"True","state":"running","del":false}
t+44s  (both unchanged: Running / ready True / state running)
t+55s  {"name":"...-2s4tl","phase":"Failed","node":"k8rs-review-worker","ready":"True","state":"running","del":true}
       {"name":"...-6wf8p","phase":"Pending","node":null,...}
       {"name":"...-9dwv5","phase":"Pending","node":null,...}
t+231s (unchanged from t+55s)
```

Field values: through the whole window the pods read `phase: Running`,
`conditions[Ready].status: "True"` and `containerStatuses[].state: running`
while `spec.nodeName` names a node that is not in the node list. The held pod
then reads `phase: Failed` **with `Ready` still `True` and its container state
still `running`**, plus a `deletionTimestamp`; `status.reason` was `null`.
The unheld pod was removed from the API and replaced by the ReplicaSet.

## 4. What the Deployment reports during the window

Same shape again, polling `kubectl get deploy web -o
custom-columns=READY:.status.readyReplicas,AVAIL:.status.availableReplicas,REPLICAS:.status.replicas`:

```
t+0s   ready/avail/replicas: 2 2 2   pods: 6wf8p=Running@k8rs-review-worker 9dwv5=Running@k8rs-review-worker
t+18s  ready/avail/replicas: 2 2 2   pods: 6wf8p=Running@k8rs-review-worker 9dwv5=Running@k8rs-review-worker
t+24s  ready/avail/replicas: <none> <none> 2   pods: 66dfv=Pending@<none> crcgl=Pending@<none>
t+78s  ready/avail/replicas: <none> <none> 2   pods: 66dfv=Pending@<none> crcgl=Pending@<none>
```

The pod list in those four lines is quoted without the finalizer-held pod from
section 3, which was still in it as `2s4tl=Failed@k8rs-review-worker`.

The window measured three times: **~20s, ~46–52s and ~44–55s** from the node
delete to the pod GC acting.

## 5. The built binary, inside the window and after it

`cargo build` then
`k8rs --live --analysis --context kind-k8rs-review`, started at t+1s after
`docker stop` + `kubectl delete node`. Trimmed to the lines the review turns on;
the panes not quoted were byte-identical to the healthy run.

```
14 pods · 1 node

▲ default/web-64845cdff-2s4tl · 19 min ago
  This pod was asked to shut down and is still here (it shows as Terminating)
  on node k8rs-review-worker · held by a finalizer: k8rs.review/hold
  → nothing can delete this pod while that list has anything in it — find what put it there

1 warning

[capacity]
  What each node promised, and what it has
    k8rs-review-control-plane   0.95 of 12 cpu · 290Mi of 23.1Gi
    8 workloads have no memory or CPU limit

[drain safety]
  If you drained each node, what happens?
    k8rs-review-control-plane is ready to drain — 3 pods move
  Every node could be drained right now. Nothing on this cluster is protected by a rule a drain would wait on, nothing on it was started by hand, and nothing on it keeps its own files, on disk or in memory.
```

The healthy run one minute earlier, same binary, same flags:

```
14 pods · 2 nodes
[capacity]
    k8rs-review-control-plane   0.95 of 12 cpu · 290Mi of 23.1Gi
    k8rs-review-worker   0.1 of 12 cpu · 50Mi of 23.1Gi
    8 workloads have no memory or CPU limit
[drain safety]
    k8rs-review-control-plane is ready to drain — 3 pods move
    k8rs-review-worker is ready to drain — 2 pods move
```

So inside the window: the two `Running` pods are in the `14 pods` header count
and in the `8 workloads` limits row, they are in no node row of either pane, and
no card is drawn about them. The only card is rule 12's, about the pod the
finalizer held.

Once the pod GC had acted, the replacements drew rule 10:

```
▲ default/web-64845cdff-nnmnt · just now
  No machine in the cluster will take this pod, so it has never started (it shows as Pending)
  it asks for a node labelled kubernetes.io/hostname=k8rs-review-worker, and the cluster's one node does not have that label · the scheduler's own words (a node is one machine): 0/1 nodes are available: 1 node(s) had untolerated taint(s). ...
  → change the nodeSelector, or label a node kubernetes.io/hostname=k8rs-review-worker
```

## Teardown

`docker stop k8rs-review-control-plane k8rs-review-worker` ran; **`kind delete
cluster --name k8rs-review` was refused by the permission system** and is owed:

```
kind delete cluster --name k8rs-review
```

`kubectl config use-context kind-k8rs` was run to put the default kubeconfig
back on the fixture cluster, which `kind create` had switched.
