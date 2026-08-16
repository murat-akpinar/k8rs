# 2026-08-16 — what a `lastState.terminated` record actually carries: stamps, author, exit code

Measured for the Phase 3 Family A operator review (boxes 931 / 966 / 991 / 1006 / 1380).
Ephemeral kind cluster, `K8RS_CLUSTER=review`, `kindest/node:v1.36.1`, one worker, brought up
and torn down inside this run. No fixture was produced.

```
$ K8RS_CLUSTER=review K8RS_WORKERS=1 ./scripts/cluster.sh up
Set kubectl context to "kind-review"
node/review-control-plane condition met
node/review-worker condition met
API: https://127.0.0.1:6443   context: kind-review
```

Two pods on the worker, both written for this run: `reboot-probe` (`busybox`, `sleep 100000`,
`restartPolicy: Always`), `start-failure` (`busybox`, `command: ["/definitely-not-here"]`),
and later `never-pair` (two `sleep` containers, `restartPolicy: Never`).

## 1 — a container that fails to start: `startedAt` is the epoch, and `message` is the runtime's

```
$ kubectl --context kind-review get pod start-failure \
    -o jsonpath='{.status.containerStatuses[0].lastState}'
```

Field values (`lastState.terminated`):

| field | value |
|---|---|
| `exitCode` | `128` |
| `reason` | `StartError` |
| `startedAt` | `1970-01-01T00:00:00Z` |
| `finishedAt` | `2026-08-16T01:19:26Z` (real) |
| `containerID` | present |
| `message` | `failed to create containerd task: failed to create shim task: OCI runtime create failed: runc create failed: unable to start container process: error during container init: exec: "/definitely-not-here": stat /definitely-not-here: no such file or directory` |

`state.waiting` at the same moment: `reason: RunContainerError`, same `message`.
`kubectl get pods` reported `CrashLoopBackOff` for this pod 13 s after creation, and
`RunContainerError` between backoff windows; the `lastState` above is identical in both.

Re-read six minutes and seven restarts later — `startedAt` is still `1970-01-01T00:00:00Z`:

```
$ kubectl --context kind-review get pod start-failure -o jsonpath='{.status.containerStatuses[0].restartCount}{"  "}{...lastState.terminated.startedAt}{"  "}{...finishedAt}{"  "}{...exitCode}{"  "}{...reason}'
7  1970-01-01T00:00:00Z  2026-08-16T01:24:30Z  128  StartError
```

Elapsed between those two stamps, in the rungs `lasted()` uses:

```
$ python3 -c "from datetime import datetime,timezone; s=datetime(1970,1,1,tzinfo=timezone.utc); f=datetime(2026,8,16,1,19,26,tzinfo=timezone.utc); h=int((f-s).total_seconds()//3600); print('hours',h,'-> days',h//24)"
hours 496345 -> days 20681
```

Source for the same fields, containerd `main`,
`internal/cri/server/container_start.go:67-73` — the start-failure path sets `FinishedAt`,
`ExitCode`, `Reason` and `Message` and leaves `StartedAt` at `0`:

```go
status.Pid = 0
status.FinishedAt = time.Now().UnixNano()
status.ExitCode = errorStartExitCode
status.Reason = errorStartReason        // "StartError"
status.Message = retErr.Error()
```

and `pkg/kubelet/kuberuntime/kuberuntime_container.go:754-764` at `release-1.36` copies
`Reason`, `Message`, `ExitCode` and `FinishedAt` out of the CRI status, with `StartedAt` set
on a different branch (`status.State != CONTAINER_CREATED`).

## 2 — a node restart: the `(255, "Unknown")` pair, and where `finishedAt` comes from

Short restart (`docker restart`, node away ~11 s):

```
$ date -u +%FT%TZ ; docker restart review-worker ; date -u +%FT%TZ
2026-08-16T01:20:01Z
2026-08-16T01:20:12Z
$ kubectl --context kind-review get pod reboot-probe -o jsonpath='...'
restartCount=1
lastState={"terminated":{"containerID":"containerd://…","exitCode":255,
  "finishedAt":"2026-08-16T01:20:13Z","reason":"Unknown","startedAt":"2026-08-16T01:19:11Z"}}
```

Long outage — the same container, stopped and started again three minutes later:

```
container started at: 2026-08-16T01:20:16Z
stop issued:          2026-08-16T01:20:56Z
node stopped at:      2026-08-16T01:21:06Z
start issued:         2026-08-16T01:24:12Z
```

```
restartCount=2
lastState={"terminated":{"containerID":"containerd://…","exitCode":255,
  "finishedAt":"2026-08-16T01:24:13Z","reason":"Unknown","startedAt":"2026-08-16T01:20:16Z"}}
```

| what | value |
|---|---|
| container ran | `01:20:16` → node stopped `01:21:06` = **50 s** |
| node unavailable | `01:21:06` → `01:24:12` = **3 min 6 s** |
| `startedAt` | `01:20:16` (the real run start) |
| `finishedAt` | `01:24:13` — one second after `docker start`, i.e. when containerd recovered |
| `finishedAt − startedAt` | **3 min 57 s** |

Source, containerd `main`, `internal/cri/server/restart.go:353-357` (task not found for a
container the checkpoint says was `RUNNING`):

```go
status.FinishedAt = time.Now().UnixNano()
status.ExitCode = unknownExitCode      // 255
status.Reason = unknownExitReason      // "Unknown"
```

`internal/cri/server/helpers.go:250-266` is where those two constants live.
`unknownContainerStatus()` itself (`CreatedAt/StartedAt/FinishedAt: 0, Unknown: true`) reports
CRI state `CONTAINER_UNKNOWN`, not `CONTAINER_EXITED`
(`internal/cri/store/container/status.go:104-118`), so it does not reach the API as this pair.

## 3 — what `kubectl describe pod` prints after each of the two

After the 11-second restart (`Events:` section only):

```
Normal  SandboxChanged  19s  kubelet  Pod sandbox changed, it will be killed and re-created.
Normal  Pulling/Pulled/Created/Started …
```

After the 3-minute outage:

```
Warning  NodeNotReady    2m45s  node-controller  Node is not ready
Normal   SandboxChanged  18s    kubelet          Pod sandbox changed, it will be killed and re-created.
```

`logs --previous` on the same record:

```
$ kubectl --context kind-review logs reboot-probe -c app --previous
hello-from-the-app        # exit status 0
```

## 4 — `restartPolicy: Never` under a node restart

```
$ kubectl --context kind-review get pod never-pair -o jsonpath='phase={.status.phase}…'
phase=Failed
a restarts=0 state={"terminated":{…,"exitCode":255,"finishedAt":"2026-08-16T01:25:37Z",
  "reason":"Unknown","startedAt":"2026-08-16T01:25:24Z"}}
b restarts=0 state={"terminated":{…,"exitCode":255,"finishedAt":"2026-08-16T01:25:37Z",
  "reason":"Unknown","startedAt":"2026-08-16T01:25:25Z"}}
```

Both containers carry the pair; the pod is `phase: Failed`.

## 5 — CRI-O writes a different pair for the same event

Not run here — no CRI-O node on this host. Read at `cri-o/cri-o` `main`,
`server/container_status.go:107-130`: for a stopped container whose exit code could not be
determined, CRI-O reports `ExitCode: -1` and `Reason: "Error"` (`errorReason`), with
`Message: cState.Error` — the runtime's own error string — beside a real `FinishedAt`.
`"Unknown"` does not appear in that file; the constants are `oomKilledReason`,
`seccompKilledReason`, `completedReason`, `errorReason`.

## 6 — the lost init-container literal, at `release-1.36`

`pkg/kubelet/kubelet_pods.go:2705-2723`, quoted for the gate and the fields:

```go
isSidecar := container.RestartPolicy != nil && *container.RestartPolicy == v1.ContainerRestartPolicyAlways
if s == nil &&
    kuberuntime.HasAnyRegularContainerCreated(pod, podStatus) &&
    !isSidecar &&
    statuses[container.Name].State.Waiting != nil {
    statuses[container.Name].State = v1.ContainerState{
        Terminated: &v1.ContainerStateTerminated{
            Reason:   "Completed",
            Message:  "Unable to get init container status from container runtime and pod has been initialized, treat it as exited normally",
            ExitCode: 0,
        },
    }
```

`HasAnyRegularContainerCreated` (`kuberuntime/kuberuntime_container.go:1034-1049`) returns true
for a regular container in `Created`, `Running` **or `Exited`**.
`computeInitContainerActions` (`kuberuntime_container.go:1071-1090`) computes its own
`podHasInitialized` from the same list and excludes `Exited`, with the comment *"If the node is
rebooted, all containers will be in the exited state … the kubelet should not mistakenly think
that the newly created podSandbox has been initialized."*

`convertContainerStatus` (`kubelet_pods.go:2294-2306`), under the `RestartAllContainersOnContainerExits`
gate:

```go
if oldStatus.ContainerID != status.ContainerID && oldStatus.State.Terminated != nil {
    status.LastTerminationState.Terminated = oldStatus.State.Terminated
} else if oldStatus.LastTerminationState.Terminated != nil {
    status.LastTerminationState.Terminated = oldStatus.LastTerminationState.Terminated
}
```

## Teardown

```
$ K8RS_CLUSTER=review ./scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-worker" "review-control-plane"]
$ kind get clusters
No kind clusters found.
```
