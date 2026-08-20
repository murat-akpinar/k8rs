# 2026-08-20 — rule 7's floor across restarts, a paused Deployment's counters, `subPathExpr` in `describe`, a root hostPath on a node agent

Measured for the Phase 3 pod-rule family operator review (rules 1/5/6 via `lasted`/`ran_for`,
rule 2's grace, rule 7's floor, rule 8's exemption, W2's `short_of_pods`). Ephemeral kind
cluster, `K8RS_CLUSTER=review`, `kindest/node:v1.36.1`, one worker, brought up and torn down
inside this run. No fixture was produced; every pod and workload below was written for this run.

```
$ K8RS_CLUSTER=review K8RS_WORKERS=1 K8RS_APISERVER_PORT=6553 ./scripts/cluster.sh up
node/review-control-plane condition met
node/review-worker condition met
API: https://127.0.0.1:6553   context: kind-review

$ kubectl --context kind-review version -o json | ... clientVersion/serverVersion
client v1.36.3 server v1.36.1
```

## 1 — `Ready.lastTransitionTime` against `state.running.startedAt` across restarts

`unready-and-restarting`: `busybox`, `sh -c "sleep 3600"`, `restartPolicy: Always`, a readiness
probe `exec ["false"]` (`periodSeconds: 5`, `failureThreshold: 1`) and a liveness probe
`exec ["false"]` (`initialDelaySeconds: 20`, `periodSeconds: 5`, `failureThreshold: 2`). Sampled
once a minute for twelve minutes. These are the two fields `running_but_not_ready` takes the
later of.

```
$ kubectl --context kind-review get pod unready-and-restarting -o jsonpath='{"restarts="}{.status.containerStatuses[0].restartCount}{"  startedAt="}{.status.containerStatuses[0].state.running.startedAt}{"  Ready="}{range .status.conditions[?(@.type=="Ready")]}{.status}{"@"}{.lastTransitionTime}{end}{"\n"}'
```

| sample | `restartCount` | `state.running.startedAt` | `Ready` |
|---|---|---|---|
| t+00m | 0 | `23:29:37Z` | `False@2026-08-19T23:29:33Z` |
| t+01m | 1 | `23:30:35Z` | `False@2026-08-19T23:29:33Z` |
| t+02m | 2 | `23:31:35Z` | `False@2026-08-19T23:29:33Z` |
| t+03m | 3 | `23:32:35Z` | `False@2026-08-19T23:29:33Z` |
| t+04m | 4 | `23:33:35Z` | `False@2026-08-19T23:29:33Z` |
| t+05m | 5 | `23:34:35Z` | `False@2026-08-19T23:29:33Z` |
| t+06m | 5 | *(absent — `waiting`)* | `False@2026-08-19T23:29:33Z` |
| t+07m | 5 | *(absent — `waiting`)* | `False@2026-08-19T23:29:33Z` |
| t+08m | 6 | `23:37:00Z` | `False@2026-08-19T23:29:33Z` |
| t+09m | 6 | *(absent — `waiting`)* | `False@2026-08-19T23:29:33Z` |
| t+10m | 6 | *(absent — `waiting`)* | `False@2026-08-19T23:29:33Z` |
| t+11m | 7 | `23:40:47Z` | `False@2026-08-19T23:29:33Z` |
| t+12m | 7 | *(absent — `waiting`)* | `False@2026-08-19T23:29:33Z` |

`Ready.lastTransitionTime` holds `2026-08-19T23:29:33Z` across all seven restarts. The largest
`now − startedAt` reachable at any sample where the container was `Running` was the gap between
two restarts: 60 s at `restartCount` 1–5, 3 m 47 s between 6 and 7 (kubelet restart back-off).

Final state, and the record the liveness kill left:

```
$ kubectl --context kind-review get pod unready-and-restarting -o jsonpath='...'
restarts=7 ready=false started=false phase=Running
PodReadyToStartContainers=True@2026-08-19T23:29:34Z
Initialized=True@2026-08-19T23:29:33Z
Ready=False@2026-08-19T23:29:33Z
ContainersReady=False@2026-08-19T23:29:33Z
PodScheduled=True@2026-08-19T23:29:33Z
lastState.terminated: exitCode=137 reason=Error
```

## 2 — a Deployment paused, then its template changed

`paused-mid-change`: two replicas of `busybox`, `sh -c "sleep 3600"`, rolled out healthy first.

```
$ kubectl --context kind-review get deployment paused-mid-change -o jsonpath='...'
=== 1. at rest ===
  spec.replicas=2 spec.paused= | status.readyReplicas=2 status.updatedReplicas=2 status.unavailableReplicas= status.replicas=2
  Available=True reason=MinimumReplicasAvailable message="Deployment has minimum availability."
  Progressing=True reason=NewReplicaSetAvailable message="ReplicaSet "paused-mid-change-69954b5b6f" has successfully progressed."
  replicasets: paused-mid-change-69954b5b6f=spec.replicas:2,ready:2

$ kubectl --context kind-review rollout pause deployment/paused-mid-change
$ kubectl --context kind-review set image deployment/paused-mid-change app=busybox:1.36
=== 2. paused + template changed, 15 s later ===
  spec.replicas=2 spec.paused=true | status.readyReplicas=2 status.updatedReplicas= status.unavailableReplicas= status.replicas=2
  Available=True reason=MinimumReplicasAvailable message="Deployment has minimum availability."
  Progressing=Unknown reason=DeploymentPaused message="Deployment is paused"
  replicasets: paused-mid-change-69954b5b6f=spec.replicas:2,ready:2
```

`status.updatedReplicas` and `status.unavailableReplicas` are both absent; `readyReplicas` is 2
against `spec.replicas` 2; the ReplicaSet list still holds one entry, so no second ReplicaSet was
created for the new template.

## 3 — what `kubectl describe pod` prints for `subPath` and for `subPathExpr`

`mount-shapes`: one `hostPath: {path: /}` volume, mounted three ways.

The four lines of `kubectl describe pod mount-shapes` the finding turns on — three `Mounts:`
rows and the volume's `Path` — with the declaration each row came from:

```
$ kubectl --context kind-review describe pod mount-shapes | grep -E 'host-|host/root|Path:'
      /host-expr from root (rw)               <- declared subPathExpr: "$(POD_NAME)"
      /host-sub from root (rw,path="etc")     <- declared subPath: "etc"
      /host/root from root (ro)               <- declared readOnly: true, mountPropagation: HostToContainer
    Path:          /                          <- the single volume, hostPath.path: "/"
```

The `subPath` appears as `path="etc"`. The `subPathExpr` produces no `path=` flag and appears
nowhere else in the output; the volume's `Path` reads `/` for both.

## 4 — the fields rule 8 reads on a node-agent DaemonSet outside `kube-system`

A DaemonSet in namespace `monitoring` carrying the volume set
`prometheus-node-exporter`'s chart declares by default (`hostRootFsMount.enabled: true` in
`charts/prometheus-node-exporter/values.yaml`; the chart's `templates/daemonset.yaml` declares
`- name: root / hostPath: / path: /` mounted at `/host/root` with `readOnly: true`).

```
$ kubectl --context kind-review -n monitoring get pods -l app=node-exporter -o jsonpath='...'
monitoring/node-exporter-nc6l5  owner=DaemonSet/node-exporter  hostPaths=/proc /sys /   mountsReadOnly=/host/proc:true /host/sys:true /host/root:true /var/run/secrets/kubernetes.io/serviceaccount:true
monitoring/node-exporter-p5cfc  owner=DaemonSet/node-exporter  hostPaths=/proc /sys /   mountsReadOnly=/host/proc:true /host/sys:true /host/root:true /var/run/secrets/kubernetes.io/serviceaccount:true
```

Two pods, one per node, each with `hostPath.path: "/"` and the matching mount at
`readOnly: true`, owned by a `DaemonSet` whose namespace is not `kube-system`.

## 5 — the suite as it stands on the reviewed tree

```
$ cargo test --quiet
running 232 tests
test result: ok. 232 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.16s
```

## 6 — how many mutants `cargo mutants` generates for `rules.rs`, either side of the D119 line

`--list` only, no run. The reviewed tree, then the same file with the change stashed out and
restored (`git diff --stat src/rules.rs` reads `9 insertions, 4 deletions` again afterwards, and
`diff -q` against a copy taken first is silent).

```
$ cargo mutants --list --file src/rules.rs | wc -l
553                     # reviewed tree: `started_at.as_ref().map_or(unready_since, |b| b.max(unready_since))`

$ git stash push -- src/rules.rs && cargo mutants --list --file src/rules.rs | wc -l
558                     # the `match started_at { Some(began) if began.0 > unready_since.0 => ... }` it replaced
```

## Teardown

```
$ K8RS_CLUSTER=review ./scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-worker" "review-control-plane"]
$ kind get clusters
No kind clusters found.
```
