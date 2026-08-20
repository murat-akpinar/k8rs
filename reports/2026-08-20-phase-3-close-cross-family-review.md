# Phase 3 close — cross-family measurements (2026-08-20)

`k8s-admin`, step 4 of CLAUDE.md § Phase close. Binary built from `070b742`
(`cargo build --release`, this machine). Cluster: `K8RS_CLUSTER=review`,
`K8RS_WORKERS=2`, `kindest/node:v1.36.1`, brought up and torn down inside this
run. Node addresses and container IDs are redacted per
[reports/README.md](README.md#the-sanitization-rule--read-it-before-pasting-cluster-output).

## 1. A node whose kubelet stops: what freezes, what moves

```
$ docker exec review-worker systemctl stop kubelet     # 06:46:19Z
```

Node `Ready` condition, polled every 15 s:

```
06:46:25Z node=True    2026-08-20T06:41:34Z
06:46:40Z node=True    2026-08-20T06:41:34Z
06:46:56Z node=True    2026-08-20T06:41:34Z
06:47:11Z node=Unknown 2026-08-20T06:47:09Z
```

Ready → `Unknown` 50 s after the kubelet stopped.

Two pods on that node, polled every 30 s for 6 minutes afterwards
(`kubectl get pod <name> -o jsonpath=...`):

```
06:47:19Z crashloop = Running|CrashLoopBackOff|restarts=5|ready=false|PodReady=False/ContainersNotReady/2026-08-20T06:44:58Z
06:47:19Z healthy   = Running|ready=true|PodReady=False/
...
06:53:21Z crashloop = Running|CrashLoopBackOff|restarts=5|ready=false|PodReady=False/ContainersNotReady/2026-08-20T06:44:58Z
06:53:21Z healthy   = Running|ready=true|PodReady=False/
```

Field by field, over the whole window after the kubelet stopped:

| field | before | 6 min after |
|---|---|---|
| `status.phase` | `Running` | `Running` |
| `containerStatuses[0].state.waiting.reason` | `CrashLoopBackOff` | `CrashLoopBackOff` |
| `containerStatuses[0].restartCount` | `5` | `5` |
| `containerStatuses[0].lastState.terminated.finishedAt` | `2026-08-20T06:44:57Z` | `2026-08-20T06:44:57Z` |
| `containerStatuses[0].ready` (healthy pod) | `true` | `true` |
| pod condition `Ready` (healthy pod) | `True` | `False`, reason empty |
| pod condition `Ready.lastTransitionTime` (crashloop pod) | `06:44:58Z` | `06:44:58Z` |

The node lifecycle controller flips the **pod-level** `Ready` condition and
leaves `containerStatuses[].ready` alone. At ~300 s the same controller set a
`deletionTimestamp` on both pods (`node.kubernetes.io/unreachable:NoExecute`);
nothing removes them, because the kubelet that would is the one that stopped.

## 2. What `analyze` prints over that state

`kubectl get nodes -o json` + `kubectl get pods -n default -o json` into the
scratchpad, then:

```
$ ./target/release/k8rs <nodes.json> <pods.json>
2 pods · 3 nodes · 0 workloads

● review-worker · 7 min ago
  This node has stopped responding — nothing on it can be trusted until it does
  default/review-crashloop and default/review-healthy were running here (2 pods)
  → check the node itself: is it powered on and reachable?

● default/review-crashloop · 9 min ago
  Container keeps crashing, and each restart waits longer (CrashLoopBackOff)
  container quitter · 5 restarts · ran for 2s · exit 1 (the application's own error)
  → read the last run's log — it holds the last thing written before that run ended, from the program or from the shell that started it. The --previous flag below is what fetches it

▲ default/review-crashloop · 2 min ago
  This pod was asked to shut down and is still here (it shows as Terminating)
  on node review-worker
  → nothing is holding the pod, so check the kubelet on that machine

▲ default/review-healthy · 2 min ago
  This pod was asked to shut down and is still here (it shows as Terminating)
  on node review-worker
  → nothing is holding the pod, so check the kubelet on that machine

2 critical, 2 warnings
```

The healthy pod draws no container card.

## 3. The two commands those cards teach, run against that cluster

```
$ kubectl logs review-crashloop -c quitter -n default --previous
Error from server: Get "https://<node-ip>:10250/containerLogs/default/review-crashloop/quitter?previous=true": dial tcp <node-ip>:10250: connect: connection refused

$ kubectl describe pod review-crashloop -n default
Name:                      review-crashloop
Namespace:                 default
Node:                      review-worker/<node-ip>
Status:                    Terminating (lasts 118s)
```

## 4. Rule 8 and rules 1–6 naming one container

Pod on the live node with one init container that mounts
`/run/containerd/containerd.sock` read-only and exits 1:

```
$ ./target/release/k8rs <initmount.json>
● default/review-initmount
  A container can drive the container runtime, which is full control of that machine
  container prep · /run/containerd/containerd.sock on the node · read-only
  → remove the mount, unless this pod's job is to manage or watch the containers on the node — if it is, it already has full control of every node it runs on

▲ default/review-initmount · 37s ago
  The last run on record failed — exit 1 (the application's own error)
  init container prep (the app starts only after this one finishes) · ran for 1s
  → read the last run's log — it holds the last thing written before that run ended, from the program or from the shell that started it. The --previous flag below is what fetches it
```

## 5. Over the committed captures, no cluster

```
$ ./target/release/k8rs tests/fixtures/*.json | grep '^  → ' | sort | uniq -c | sort -rn | head -3
      9   → read the last run's log — it holds the last thing written before that run ended, from the program or from the shell that started it. The --previous flag below is what fetches it
      3   → check the readiness probe: the path, the port, and whether the application answers it yet
      2   → remove the mount, unless this pod's job is to manage or watch the containers on the node — if it is, it already has full control of every node it runs on
```

W1 / W2 over the two quota captures:

```
$ ./target/release/k8rs tests/fixtures/quota-deployment.json
● k8rs-quota/broken-quota · 3 days ago
  This rollout gave up — Kubernetes has stopped waiting for it to finish

$ ./target/release/k8rs tests/fixtures/quota-deployment.json tests/fixtures/quota-replicasets.json
● k8rs-quota/broken-quota-59654c756 · 3 days ago
  Kubernetes refused to create the pods this workload asked for
```

Negative control, the healthy captures alone:

```
$ ./target/release/k8rs tests/fixtures/healthy.json tests/fixtures/healthy-hostpath.json \
    tests/fixtures/healthy-podlevel.json tests/fixtures/healthy-retry.json \
    tests/fixtures/healthy-sidecar.json tests/fixtures/healthy-unreadysidecar.json \
    tests/fixtures/healthy-replicasets.json tests/fixtures/kube-system-pods.json
20 pods · 0 nodes · 1 workload

○ nothing is broken
```

`tests/fixtures/unjudged.json` — the field rule 14 and `explains_a_shortfall`
both read:

```
$ jq '.status.conditions // "NO CONDITIONS"' tests/fixtures/unjudged.json
"NO CONDITIONS"
```

## Teardown

```
$ K8RS_CLUSTER=review ./scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-worker" "review-worker2" "review-control-plane"]
$ kind get clusters
No kind clusters found.
```
