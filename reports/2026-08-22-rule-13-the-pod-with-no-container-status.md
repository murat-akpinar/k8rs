# Rule 13 — the pod with no container status: who writes `PodScheduled: True`, and does the shape survive

Measured 2026-08-22 on an ephemeral review cluster, for todo.md line 611 (Phase 3,
re-opened by D155). Every command below carries `--context kind-review`; the
fixture cluster `k8rs` was not touched.

## The cluster

```
$ K8RS_CLUSTER=review K8RS_APISERVER_PORT=6444 K8RS_WORKERS=1 scripts/cluster.sh up
Creating cluster "review" ...
 ✓ Ensuring node image (kindest/node:v1.36.1) 🖼
 ✓ Preparing nodes 📦 📦
 ✓ Starting control-plane 🕹️
 ✓ Installing CNI 🔌
 ✓ Installing StorageClass 💾
 ✓ Joining worker nodes 🚜
Set kubectl context to "kind-review"
node/review-control-plane condition met
node/review-worker condition met
```

Port 6444 because the fixture cluster holds 6443. One worker, not three: nothing
here needs `break-nodes`.

## 1 — A hand-set `spec.nodeName` naming a node that does not exist

Pod `q1-ghost-node`, `spec.nodeName: node-that-does-not-exist`, no other trick.

```
$ kubectl --context kind-review get nodes -o name
node/review-control-plane
node/review-worker

$ kubectl --context kind-review apply -f q1.yaml
pod/q1-ghost-node created

$ kubectl --context kind-review get pods -o wide
NAME            READY   STATUS    RESTARTS   AGE   IP       NODE                       NOMINATED NODE
q1-ghost-node   0/1     Pending   0          6s    <none>   node-that-does-not-exist   <none>

$ kubectl --context kind-review get pod q1-ghost-node -o json | jq .status
{
  "phase": "Pending",
  "qosClass": "BestEffort"
}

$ kubectl --context kind-review describe pod q1-ghost-node | sed -n '/Events:/,$p'
Events:                      <none>
```

**No `conditions` key at all** — not `PodScheduled: True`, not
`PodScheduled: False`. Nothing on the create path writes the condition, whatever
`spec.nodeName` says. `tests/fixtures/overhead.json` carries `PodScheduled: True`
on a hand-set `nodeName`; on this cluster that same spec produced no condition.

## 2 — What does write it: the `binding` subresource

Pod `q2-bound-to-ghost`, `spec.schedulerName: does-not-exist` (so the real
scheduler never looks at it), then one POST:

```
$ cat binding.json
{"apiVersion":"v1","kind":"Binding","metadata":{"name":"q2-bound-to-ghost","namespace":"default"},"target":{"apiVersion":"v1","kind":"Node","name":"node-that-does-not-exist"}}

$ kubectl --context kind-review get pod q2-bound-to-ghost -o json | jq .status   # before
{
  "phase": "Pending",
  "qosClass": "BestEffort"
}

$ kubectl --context kind-review create -f binding.json \
    --raw /api/v1/namespaces/default/pods/q2-bound-to-ghost/binding
{"kind":"Status","apiVersion":"v1","metadata":{},"status":"Success","code":201}

$ kubectl --context kind-review get pod q2-bound-to-ghost -o json | jq .status   # after
{
  "conditions": [
    {
      "lastProbeTime": null,
      "lastTransitionTime": "2026-08-22T14:20:24Z",
      "status": "True",
      "type": "PodScheduled"
    }
  ],
  "phase": "Pending",
  "qosClass": "BestEffort"
}

$ kubectl --context kind-review get pod q2-bound-to-ghost -o jsonpath='{.spec.nodeName}'
node-that-does-not-exist
```

The binding handler accepts a target node that does not exist (201), sets
`spec.nodeName`, and writes `PodScheduled: True` with a `lastTransitionTime`.
The only difference between `q1` and `q2` is that POST.

## 2b — The ghost-node shape is reaped by PodGC in under a minute

`q1` and `q2` both vanished while the first sample loop was running. Timed on a
re-creation:

```
$ T0=$(date -u +%s); kubectl --context kind-review create -f binding.json \
    --raw /api/v1/namespaces/default/pods/q2-bound-to-ghost/binding >/dev/null
bound at 2026-08-22T14:22:17Z
GONE after 51s (from binding)

$ kubectl --context kind-review get events -n default | grep -i q2
(no events)

$ kubectl --context kind-review logs -n kube-system -l component=kube-controller-manager \
    --tail=2000 | grep -i 'orphaned\|force deleting'
I0822 14:20:44.133275  1 gc_controller.go:348] "PodGC is force deleting Pod" pod="default/q1-ghost-node"
I0822 14:20:44.148797  1 gc_controller.go:264] "Forced deletion of orphaned Pod succeeded" pod="default/q1-ghost-node"
I0822 14:20:44.148838  1 gc_controller.go:348] "PodGC is force deleting Pod" pod="default/q2-bound-to-ghost"
I0822 14:20:44.171203  1 gc_controller.go:264] "Forced deletion of orphaned Pod succeeded" pod="default/q2-bound-to-ghost"
I0822 14:23:04.184038  1 gc_controller.go:348] "PodGC is force deleting Pod" pod="default/q2-bound-to-ghost"
I0822 14:23:04.199657  1 gc_controller.go:264] "Forced deletion of orphaned Pod succeeded" pod="default/q2-bound-to-ghost"
```

Force-deleted, silently, with no Event on the pod, both for the hand-set
`nodeName` pod and the bound one. 51 s is the 5 s poll's resolution; the
controller's own stamp puts the delete at 14:23:04 against a bind at 14:22:17,
so 47 s.

## 2c — A real node with a dead kubelet: the pod is evicted at 300 s

Pod `q4-bound-to-dead-kubelet`, `schedulerName: does-not-exist`, bound to
`review-worker` after `docker exec review-worker systemctl stop kubelet`.

```
$ docker exec review-worker systemctl stop kubelet   # 2026-08-22T14:21:05Z
$ kubectl --context kind-review create -f binding4.json \
    --raw /api/v1/namespaces/default/pods/q4-bound-to-dead-kubelet/binding
{"kind":"Status","apiVersion":"v1","metadata":{},"status":"Success","code":201}   # 14:21:11Z
```

Sampled once a minute (`watch.sh`, the loop is in the scratchpad):

```
### t=0m  q4 :: {"phase":"Pending","conds":[{"type":"PodScheduled","status":"True","lastTransitionTime":"2026-08-22T14:21:11Z"}],"cs":"ABSENT"}
### t=5m  q4 :: {"phase":"Pending","conds":[{"type":"PodScheduled","status":"True","lastTransitionTime":"2026-08-22T14:21:11Z"}],"cs":"ABSENT"}
### t=6m  q4 :: {"phase":"Pending","conds":[{"type":"PodScheduled","status":"True",...},{"type":"DisruptionTarget","status":"True","lastTransitionTime":"2026-08-22T14:26:49Z"}],"cs":"ABSENT"}
```

```
$ kubectl --context kind-review get pod q4-bound-to-dead-kubelet -o jsonpath='{.spec.tolerations}' | jq -c
[{"effect":"NoExecute","key":"node.kubernetes.io/not-ready","operator":"Exists","tolerationSeconds":300},
 {"effect":"NoExecute","key":"node.kubernetes.io/unreachable","operator":"Exists","tolerationSeconds":300}]

$ kubectl --context kind-review get pods -o wide
q4-bound-to-dead-kubelet   0/1   Terminating   0   7m13s   <none>   review-worker
```

`DefaultTolerationSeconds` gives every pod 300 s of NoExecute toleration. The
node went `Ready: Unknown` at 14:21:49; the taint manager set
`DisruptionTarget: True` and a `deletionTimestamp` at 14:26:49, 300 s later. The
pod then sat in `Terminating` forever, because the kubelet that would confirm the
delete is the one that is stopped.

## 3 — The shape that does survive, and its full status

Two pods reached 15 minutes unchanged. Both have `schedulerName: does-not-exist`
and were placed by a `binding` POST.

- **`q6-forever-tolerant`** — bound to `review-worker` (kubelet stopped), with the
  two NoExecute tolerations written out **without** `tolerationSeconds`, so the
  taint manager never evicts it.
- **`q5-bound-to-phantom`** — bound to `phantom-node`, a bare `Node` object with
  nothing but a name, created with `kubectl apply`. Its default 300 s tolerations
  were never spent because the node was held `Ready: True` by a heartbeat loop
  (§ 5).

```
$ kubectl --context kind-review get pod q6-forever-tolerant -o json | jq .status
{
  "conditions": [
    {
      "lastProbeTime": null,
      "lastTransitionTime": "2026-08-22T14:25:45Z",
      "status": "True",
      "type": "PodScheduled"
    }
  ],
  "phase": "Pending",
  "qosClass": "BestEffort"
}

$ kubectl --context kind-review get pod q6-forever-tolerant -o json \
    | jq '{nodeName:.spec.nodeName, schedulerName:.spec.schedulerName, tolerations:.spec.tolerations}'
{
  "nodeName": "review-worker",
  "schedulerName": "does-not-exist",
  "tolerations": [
    {"effect": "NoExecute", "key": "node.kubernetes.io/not-ready", "operator": "Exists"},
    {"effect": "NoExecute", "key": "node.kubernetes.io/unreachable", "operator": "Exists"}
  ]
}

$ kubectl --context kind-review describe pod q6-forever-tolerant | sed -n '/^Events:/,$p'
Events:                      <none>

$ kubectl --context kind-review get pods -o wide
NAME                  READY   STATUS    RESTARTS   AGE     IP       NODE
q6-forever-tolerant   0/1     Pending   0          3m21s   <none>   review-worker
```

`q5-bound-to-phantom` is byte-identical in `.status` apart from the stamp
(`2026-08-22T14:23:23Z`) and `nodeName: phantom-node`.

Field by field, for both:

| asked | measured |
|---|---|
| `status.containerStatuses` | **absent** — not `[]`, the key is not present |
| `status.phase` | `Pending` |
| `PodReadyToStartContainers` | **no such condition** — `conditions` holds exactly one entry |
| `Ready` condition | **absent** |
| `Initialized`, `ContainersReady` | absent |
| `kubectl describe` Events | `Events: <none>` |
| `kubectl get pods` STATUS column | `Pending` |
| `status.reason` / `status.message` | both absent |

## 4 — Stability over the capture window

`watch2.sh` sampled both pods and all nodes every two minutes.

```
### 2026-08-22T14:29:52Z
q5-bound-to-phantom :: {"phase":"Pending","conds":[{"lastProbeTime":null,"lastTransitionTime":"2026-08-22T14:23:23Z","status":"True","type":"PodScheduled"}],"cs":"ABSENT","del":null}
q6-forever-tolerant :: {"phase":"Pending","conds":[{"lastProbeTime":null,"lastTransitionTime":"2026-08-22T14:25:45Z","status":"True","type":"PodScheduled"}],"cs":"ABSENT","del":null}
nodes :: [{"n":"phantom-node","ready":"True"},{"n":"review-control-plane","ready":"True"},{"n":"review-worker","ready":"Unknown"}]
### 2026-08-22T14:31:52Z
q5-bound-to-phantom :: {"phase":"Pending","conds":[{"lastProbeTime":null,"lastTransitionTime":"2026-08-22T14:23:23Z","status":"True","type":"PodScheduled"}],"cs":"ABSENT","del":null}
q6-forever-tolerant :: {"phase":"Pending","conds":[{"lastProbeTime":null,"lastTransitionTime":"2026-08-22T14:25:45Z","status":"True","type":"PodScheduled"}],"cs":"ABSENT","del":null}
nodes :: [{"n":"phantom-node","ready":"True"},{"n":"review-control-plane","ready":"True"},{"n":"review-worker","ready":"Unknown"}]
### 2026-08-22T14:33:52Z
q5-bound-to-phantom :: {"phase":"Pending","conds":[{"lastProbeTime":null,"lastTransitionTime":"2026-08-22T14:23:23Z","status":"True","type":"PodScheduled"}],"cs":"ABSENT","del":null}
q6-forever-tolerant :: {"phase":"Pending","conds":[{"lastProbeTime":null,"lastTransitionTime":"2026-08-22T14:25:45Z","status":"True","type":"PodScheduled"}],"cs":"ABSENT","del":null}
nodes :: [{"n":"phantom-node","ready":"True"},{"n":"review-control-plane","ready":"True"},{"n":"review-worker","ready":"Unknown"}]
### 2026-08-22T14:35:53Z
### 2026-08-22T14:37:53Z
### 2026-08-22T14:39:53Z
### 2026-08-22T14:41:53Z
(these four samples are identical to 14:33:52Z in every field, including both
`lastTransitionTime` stamps, `cs: "ABSENT"` and `del: null`)
```

```
$ date -u +%FT%TZ; kubectl --context kind-review get pods -o wide
2026-08-22T14:43:39Z
NAME                       READY   STATUS        RESTARTS   AGE   IP       NODE
q3-unbound-control         0/1     Pending       0          22m   <none>   <none>
q4-bound-to-dead-kubelet   0/1     Terminating   0          22m   <none>   review-worker
q5-bound-to-phantom        0/1     Pending       0          20m   <none>   phantom-node
q6-forever-tolerant        0/1     Pending       0          17m   <none>   review-worker
```

`q5` 20 minutes and `q6` 17 minutes with nothing in `.status` changing. The
`PodScheduled` stamp is written once, at bind time, and never refreshed — so at
capture time it is as old as the wait.

`q3-unbound-control` (`schedulerName: does-not-exist`, never bound) held no
`PodScheduled` condition for the whole 26 minutes it existed: the control for § 1
and § 2, and rule 14's shape. The sample loop's `jq` renders that as `conds:[]`
because it iterates with `[]?`; the raw `jq .status` on the same shape (§ 1, § 2
"before") shows the `conditions` key absent, not empty.

## 5 — The node side

For `q6-forever-tolerant`, whose node is a real kind worker with a stopped kubelet:

```
$ kubectl --context kind-review get node review-worker -o json \
    | jq -c '{ready:([.status.conditions[]|select(.type=="Ready")|{status,reason,lastTransitionTime}]|first),taints:[.spec.taints[]?|{key,effect}]}'
{"ready":{"status":"Unknown","reason":"NodeStatusUnknown","lastTransitionTime":"2026-08-22T14:21:49Z"},
 "taints":[{"key":"node.kubernetes.io/unreachable","effect":"NoSchedule"},{"key":"node.kubernetes.io/unreachable","effect":"NoExecute"}]}
```

The node **is** in the cluster, with a `Ready` condition whose status is
`Unknown`, 44 s after the kubelet stopped.

For `q5-bound-to-phantom`, whose node is a bare `Node` object:

```
$ kubectl --context kind-review get node phantom-node -o json | jq -c '{conds:(.status.conditions//"ABSENT"),taints:[.spec.taints[]?|{key,effect}]}'
{"conds":"ABSENT","taints":[{"key":"node.kubernetes.io/not-ready","effect":"NoSchedule"}]}   # t+0s
```

61 s later the node-lifecycle-controller had filled it in:

```
$ kubectl --context kind-review get node phantom-node -o json | jq -c '[.status.conditions[]|{type,status,reason}]'
[{"type":"Ready","status":"Unknown","reason":"NodeStatusNeverUpdated"},
 {"type":"MemoryPressure","status":"Unknown","reason":"NodeStatusNeverUpdated"},
 {"type":"DiskPressure","status":"Unknown","reason":"NodeStatusNeverUpdated"},
 {"type":"PIDPressure","status":"Unknown","reason":"NodeStatusNeverUpdated"}]
```

(`message` on each: `Kubelet never posted node status.`; `lastTransitionTime`
`2026-08-22T14:24:24Z` against a node created at `14:23:23Z`.)

So **"a node with no `Ready` condition" is a window about 60 s wide**, closed by
the node-lifecycle-controller, on a node nothing ever heartbeats.

Holding that node `Ready: True` needs a writer. Patched once:

```
$ NOW=$(date -u +%FT%TZ); kubectl --context kind-review patch node phantom-node --subresource=status --type=merge \
    -p "{\"status\":{\"conditions\":[{\"type\":\"Ready\",\"status\":\"True\",\"reason\":\"KubeletReady\",\"lastHeartbeatTime\":\"$NOW\",\"lastTransitionTime\":\"$NOW\"}]}}"
patched at 2026-08-22T14:26:37Z

# 15s later — taints removed by the node-lifecycle-controller
{"ready":{"status":"True","reason":"KubeletReady"},"taints":[]}

# 52s later, with no fresh heartbeat
{"ready":{"status":"Unknown","reason":"NodeStatusUnknown","lastTransitionTime":"2026-08-22T14:27:29Z"}}
```

`q5`'s node was then held `Ready: True` by a 15 s heartbeat loop for the whole of
§ 4, which is what kept its default 300 s tolerations from being spent:

```
$ kubectl --context kind-review get nodes
NAME                   STATUS     ROLES           AGE   VERSION
phantom-node           Ready      <none>          20m
review-control-plane   Ready      control-plane   24m   v1.36.1
review-worker          NotReady   <none>          24m   v1.36.1
```

`phantom-node` prints an empty VERSION column, because nothing ever wrote
`status.nodeInfo`.

## The "deleted node" case

`q5` bound to `phantom-node`, node object then removed:

```
$ date -u +%FT%TZ; kubectl --context kind-review delete node phantom-node
2026-08-22T14:47:18Z
node "phantom-node" deleted
q5 gone after 46s from node delete

I0822 14:48:04.264811 1 gc_controller.go:348] "PodGC is force deleting Pod" pod="default/q5-bound-to-phantom"
I0822 14:48:04.284014 1 gc_controller.go:264] "Forced deletion of orphaned Pod succeeded" pod="default/q5-bound-to-phantom"
```

Same 40–60 s reaping as § 2b, from the moment the node leaves the cluster.

## Undoing it

A plain `kubectl delete pod` does **not** remove either shape — it returns, prints
`deleted`, and blocks, and the pod stays with a `deletionTimestamp`:

```
$ date -u +%FT%TZ; timeout 40 kubectl --context kind-review delete pod q6-forever-tolerant; echo "rc=$?"
2026-08-22T14:44:00Z
pod "q6-forever-tolerant" deleted from default namespace
rc=124

$ kubectl --context kind-review get pod q6-forever-tolerant -o json | jq -c '{del:.metadata.deletionTimestamp,phase:.status.phase}'
{"del":"2026-08-22T14:44:30Z","phase":"Pending"}

$ kubectl --context kind-review get pods -o wide   # 60s later
q4-bound-to-dead-kubelet   0/1   Terminating   0   24m   <none>   review-worker
q6-forever-tolerant        0/1   Terminating   0   20m   <none>   review-worker
```

The kubelet that would confirm the delete is the one that was stopped. Restarting
it reaps both immediately:

```
$ date -u +%FT%TZ; docker exec review-worker systemctl start kubelet
2026-08-22T14:45:59Z
both terminating pods gone at 2026-08-22T14:46:04Z

$ kubectl --context kind-review get nodes
review-control-plane   Ready   control-plane   26m   v1.36.1
review-worker          Ready   <none>          26m   v1.36.1
```

After both undos the cluster was intact:

```
$ kubectl --context kind-review get pods -n kube-system --no-headers | awk '{print $2, $3}' | sort | uniq -c
     10 1/1 Running
```

(coredns ×2, etcd, kindnet ×2, kube-apiserver, kube-controller-manager,
kube-proxy ×2, kube-scheduler.)

## What was measured about the rule's own inputs

Read off `src/rules.rs` at HEAD, against the shapes above — the values, not a judgement:

| rule / helper | field it reads | value in `q5` / `q6` |
|---|---|---|
| `placed_but_never_started` | `pod.scheduled.status` | `"True"` |
| `placed_but_never_started` | `pod.scheduled.last_transition` | present, set once at bind, never refreshed |
| `placed_but_never_started` | `pod.containers` (from `status.containerStatuses`) | key absent → empty vector |
| `placed_but_never_started` | `pod.deletion_timestamp` | absent, until an undo sets it |
| `placed_but_never_started` | `pod.ready_to_start_containers` | **no such condition on the object** → `None`, so the `is_some_and(status == "False")` test is false and the `else` branch is the one reached |
| `node_stopped_being_ready` (N1) | `node_condition(node, "Ready")` | `Unknown` on `review-worker`; `True` on `phantom-node` while heartbeated; the node is absent entirely for the ghost-node shape, which does not survive 51 s |

## Teardown

```
$ K8RS_CLUSTER=review scripts/cluster.sh down
```
