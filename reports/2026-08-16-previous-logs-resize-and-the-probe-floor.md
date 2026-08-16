# 2026-08-16 — `logs --previous`, `lastState` across restarts, in-place resize, the probe floor

Measured for the Phase 3 **Family B** operator review. Ephemeral kind cluster,
`K8RS_CLUSTER=review`, `kindest/node:v1.36.1`, one worker, brought up and torn down inside this
run. No fixture was produced. Container IDs are elided; the pods were written for this run.

```
$ K8RS_CLUSTER=review K8RS_WORKERS=1 ./scripts/cluster.sh up
API: https://127.0.0.1:6443   context: kind-review
$ kubectl --context kind-review get nodes
review-control-plane   Ready   control-plane   v1.36.1
review-worker          Ready   <none>          v1.36.1
```

## 1 — `lastState.terminated` across ordinary restarts

`looper2`: `busybox`, `sh -c "echo MARKER_RUN_STARTED_AT=$(date -u +%H:%M:%S); sleep 25; echo
MARKER_RUN_DIED; exit 1"`, `restartPolicy: Always`. Sampled every 6 s.

```
$ kubectl --context kind-review get pod looper2 -o jsonpath='{.status.containerStatuses[0].restartCount}|{.status.containerStatuses[0].state}|{...lastState.terminated.startedAt}|{...lastState.terminated.containerID}'
$ kubectl --context kind-review logs looper2 -c app --previous
```

| sample | `restartCount` | `state` | `lastState…startedAt` | `--previous` returns |
|---|---|---|---|---|
| 1–3 | 2 | `running` (started `05:33:53`) | `05:33:16` | `MARKER_RUN_STARTED_AT=05:33:16` … |
| 4–7 | 2 | `terminated` (`exitCode: 1`, finished `05:34:18`) | `05:33:16` | `unable to retrieve container logs for containerd://…` |
| 8–10 | 3 | `running` (started `05:34:47`) | `05:33:53` | `MARKER_RUN_STARTED_AT=05:33:53` … |
| 11–12 | 3 | `terminated` (`exitCode: 1`, finished `05:35:12`) | `05:33:53` | `unable to retrieve container logs for containerd://…` |

`lastState…startedAt` moves `05:33:16` → `05:33:53` between `restartCount` 2 and 3; the
`containerID` moves with it, to the container that was current in the previous sample.

The same pod at `restartCount: 6`, in `waiting` with `back-off 5m0s`, and `looper`
(`sleep 2; exit 1`) at `restartCount: 7`, sampled 15 times over 60 s:

```
$ kubectl --context kind-review get pod looper -o jsonpath='{.status.containerStatuses[0].state}'
{"waiting":{"message":"back-off 5m0s restarting failed container=app pod=looper_default(…)","reason":"CrashLoopBackOff"}}
$ kubectl --context kind-review logs looper -c app -n default --previous
RUN_AT=05:40:06                       # 15 of 15 samples, identical
$ kubectl --context kind-review logs looper -c app -n default
RUN_AT=05:40:06                       # same bytes, no flag
```

What is on the node during the `terminated` window, and the kubelet's GC settings:

```
$ docker exec review-worker crictl ps -a --name app -o table
CONTAINER      CREATED         STATE     NAME   ATTEMPT   POD
b1e86047eb7a   58 seconds ago  Exited    app    4         looper
$ docker exec review-worker sh -c 'ls -la /var/log/pods/default_looper_*/app/'
-rw-r----- 1 root root 56 Aug 16 05:30 4.log
$ docker exec review-worker sh -c 'cat /proc/$(pidof kubelet)/cmdline | tr "\0" "\n"'
/usr/bin/kubelet --bootstrap-kubeconfig=… --kubeconfig=… --config=/var/lib/kubelet/config.yaml …
```

No GC flag is set and `/var/lib/kubelet/config.yaml` carries none, so
`--maximum-dead-containers-per-container` is at its default of 1. One container and one log file
per container name survive.

## 2 — exit `126`, `127`, `128`: what ran, and what the log holds

| pod | `command` | `lastState…exitCode` | `reason` | `startedAt` | `logs --previous` |
|---|---|---|---|---|---|
| `notexec` | `sh -c "exec /etc/hostname"` | `126` | `Error` | real | `sh: exec: line 0: /etc/hostname: Permission denied` |
| `notfound127` | `sh -c "exec /usr/bin/nope"` | `127` | `Error` | real | `sh: exec: line 0: /usr/bin/nope: not found` |
| `startfail` | `["/definitely-not-here"]` | `128` | `StartError` | `1970-01-01T00:00:00Z` | *(empty, exit status 0)* |

The committed capture for this class, read rather than re-captured:

```
$ python3 -c "import json; d=json.load(open('tests/fixtures/notfound.json')); …"
broken-notfound app  exitCode 127, reason "Error", startedAt 2026-08-13T23:30:50Z, containerID present
  spec command: ['sh', '-c', 'exec /usr/local/bin/server --serve']
```

## 3 — in-place pod resize with `resizePolicy: RestartContainer`

`resizer`: one sidecar (`initContainers[].restartPolicy: Always`), one plain init container, one
regular container; memory `resizePolicy: RestartContainer` on the sidecar and the regular one.

```
$ kubectl --context kind-review patch pod resizer --subresource resize \
    --patch '{"spec":{"containers":[{"name":"app","resources":{"requests":{"memory":"128Mi"},"limits":{"memory":"128Mi"}}}]}}'
pod/resizer patched
$ kubectl --context kind-review get pod resizer -o jsonpath='{range .status.conditions[*]}{.type}={.status} {end}'
PodResizeInProgress=True PodReadyToStartContainers=True Initialized=True Ready=True ContainersReady=True PodScheduled=True
```

`kubectl describe pod resizer` during the resize prints it under `Conditions:`:

```
Conditions:
  Type                        Status
  PodResizeInProgress         True
  PodReadyToStartContainers   True
  …
```

After the restart the condition is gone; what `describe` still prints is in `Events:`

```
  Normal  ResizeStarted    33s   kubelet  Pod resize started: {…"containers":[{"name":"app","resources":{…"memory":"128Mi"}}]…}
  Normal  Killing          33s   kubelet  spec.containers{app}: Container app resize requires restart
  Normal  ResizeCompleted   1s   kubelet  Pod resize completed: {…}
```

`resizePolicy` itself is not in `describe` at all:

```
$ kubectl --context kind-review describe pod resizer | grep -ic "resizePolicy\|Resize Policy"
0
```

The record the restart left, and the same for the sidecar:

| container | `lastState…exitCode` | `reason` | note |
|---|---|---|---|
| `app` (regular, `sh -c "sleep 100000"`) | `137` | `Error` | PID 1 with no `SIGTERM` handler; killed at the grace period |
| `side` (sidecar, `sh -c "sleep 100000"`) | `137` | `Error` | event: `spec.initContainers{side}: Container side resize requires restart` |
| `app` on `politeresize` (`trap 'exit 143' TERM`) | `143` | `Error` | same `Killing … resize requires restart` event |

The two open sub-questions:

```
$ kubectl --context kind-review patch pod resizer --subresource resize \
    --patch '{"spec":{"initContainers":[{"name":"side","resources":{"requests":{"memory":"96Mi"},"limits":{"memory":"96Mi"}}}]}}'
pod/resizer patched
$ kubectl --context kind-review get pod resizer -o jsonpath='{.status.initContainerStatuses[0].restartCount} {.status.initContainerStatuses[0].allocatedResources.memory}'
1 96Mi
```

```
$ kubectl --context kind-review apply -f initresize.yaml     # plain init container, resizePolicy RestartContainer
The Pod "initresize" is invalid: spec.initContainers[0].resizePolicy[0].restartPolicy:
Invalid value: "RestartContainer": must not be set to 'RestartContainer' for non-sidecar initContainers
```

## 4 — a liveness probe at stock settings

`probefail`: `livenessProbe: exec: ["sh","-c","exit 1"]` with no `initialDelaySeconds`,
`periodSeconds` or `failureThreshold` set. Container traps `SIGTERM` and exits `143`.

```
$ kubectl --context kind-review get pod probefail -o jsonpath='{.status.containerStatuses[0].lastState.terminated}'
{"exitCode":143,"finishedAt":"2026-08-16T05:39:19Z","reason":"Error","startedAt":"2026-08-16T05:38:50Z", …}
$ kubectl --context kind-review describe pod probefail | grep -E "Unhealthy|Killing"
  Warning  Unhealthy  3s (x3 over 23s)  kubelet  spec.containers{app}: Liveness probe failed:
  Normal   Killing    3s                kubelet  spec.containers{app}: Container app failed liveness probe, will be restarted
```

`finishedAt − startedAt` = **29 s**; the third `Unhealthy` is at ~23 s after the container started.

## 5 — `--event-ttl`

```
$ kubectl --context kind-review -n kube-system get pod -l component=kube-apiserver \
    -o jsonpath='{.items[0].spec.containers[0].command}' | tr ',' '\n' | grep -i event-ttl
(no output — the flag is not set)
$ docker exec review-control-plane crictl exec <kube-apiserver> kube-apiserver --help | grep -- --event-ttl
      --event-ttl duration    Amount of time to retain events. (default 1h0m0s)
```

Not measured here, read from vendor documentation: GKE and AKS do not expose the flag and keep the
default; EKS defaults to it and, since its advanced control-plane configuration, allows tuning it.

## 6 — the action and title widths, wrapped independently

A greedy wrap that breaks on spaces and by character where a token is wider than the line, over
every action string this family touched, at 49 columns; titles at 51.

```
5 lines, 226 chars, 13 free  stopped_action(Regular|Sidecar)
5 lines, 234 chars, 10 free  stopped_action(Init)
5 lines, 230 chars,  4 free  failed_action(Init)
5 lines, 234 chars,  3 free  no_record_action
5 lines, 238 chars,  7 free  finished_action(Regular, long)
5 lines, 236 chars,  6 free  finished_action(Regular, short)
5 lines, 233 chars, 12 free  finished_action(Sidecar, long)
5 lines, 224 chars, 22 free  finished_action(Sidecar, short)
5 lines, 232 chars,  8 free  finished_action(Init)
5 lines, 238 chars,  4 free  killed_action(Regular|Sidecar)
4 lines, 193 chars,  4 free  unwatched_action
3 lines, 102 chars, 45 free  failed_action(Regular|Sidecar)
2 lines,  80 chars, 15 free  what_the_exit_code_names(126..=128)

3 lines,  6 free  rule 6, CodeUnknown / 255
3 lines,  4 free  rule 6, CodeUnknown / -1
3 lines,  8 free  rule 6, Unwatched / 137
2 lines,  4 free  rule 1, no record at all
2 lines, 13 free  rule 6, Failed / 128
2 lines, 10 free  rule 3, SignatureValidationFailed (longest reason in UNUSABLE_IMAGE)
```

```
$ cargo test
test result: ok. 219 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Teardown

```
$ K8RS_CLUSTER=review ./scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-worker" "review-control-plane"]
$ kind get clusters
No kind clusters found.
```

## 7 — the delta: `failed_run_action` over the committed captures

Run against the fixed tree, no cluster. `cargo test the_whole_capture_through_the_rules_at_once
-- --nocapture`, cards quoted as printed.

`broken-oom` — `state.waiting.reason: CrashLoopBackOff`, `lastState.terminated`
`exitCode: 137` / `reason: OOMKilled`. Two cards, both CRITICAL, adjacent:

```
● default/broken-oom · 2 days ago
  Container keeps crashing, and each restart waits longer (CrashLoopBackOff)
  container hog · 16 restarts · ran for under a second · exit 137 (killed by the kernel for using more memory than it was allowed)
  → check the liveness and startup probes, whether it stops when asked to, and the memory limit: a kill for using too much memory is not always labelled as one, and this kill came from outside the application, so its own logs will not say why
  $ kubectl describe pod broken-oom -n default

● default/broken-oom · 2 days ago
  Container used more memory than it was allowed and the kernel killed it (OOMKilled)
  container hog · limit 64Mi · exit 137 · 16 restarts
  → raise the container's memory limit, or find what is using the memory
  $ kubectl describe pod broken-oom -n default
```

`broken-restarts10` — `restartCount: 10`, `state.running`, `ready: false`, `lastState.terminated`
`exitCode: 1`. Rules 5 and 6 co-fire on one ending with two sentences:

```
● default/broken-restarts10 · 2 days ago
  Container has been restarted 10 times, but something keeps killing it
  container flaky · exit 1 (the application's own error) · docker.io/library/busybox:latest
  → check the memory limit and the liveness probe — those are what restart a container that otherwise runs
  $ kubectl describe pod broken-restarts10 -n default

▲ default/broken-restarts10 · 2 days ago
  The last run on record failed — exit 1 (the application's own error)
  container flaky · ran for under a second
  → read that run's log — that is where the program, or the shell that tried to start it, said what went wrong
  $ kubectl logs broken-restarts10 -c flaky -n default --previous
```

The fold, on two `Ending::Failed` containers that differ only in whether the record carries a
`message`:

| capture | `lastState…message` | cards drawn |
|---|---|---|
| `broken-notfound` (`exit 127`) | absent | **1** — rule 6 folded into rule 1 |
| `broken-crashloop` (`exit 1`) | `panic: dial tcp …: connection refused` | **2** — subset clause refuses the fold |

Every action the rule set draws, re-wrapped independently at 49 columns (30 distinct):
eleven measure 5 lines, tightest `no_record_action` at 3 free columns; none over.

```
$ cargo test
test result: ok. 219 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```
