# Family D — the Restarts pane, measured against a real cluster

Operator review of Phase 4 Family D (`analysis.rs` § THE RESTARTS REPORT, and the
three `rules.rs` helpers widened to `pub(crate)` for it). Everything below is
command + real output. No conclusion is drawn here; the findings that cite this
file live in the review, and anything settled by it is the PM's `D##`.

- Cluster: `kind`, one node, `kindest/node:v1.36.1`, `K8RS_CLUSTER=review`,
  created and deleted inside this run. No fixture, no capture, nothing written
  into `tests/`.
- Binary: `cargo build --release` at the reviewed tree, run as
  `k8rs --analysis <pods.json>` — the temporary driver, which reads pod JSON and
  prints the cards followed by the seven panes.
- Snapshots were taken with `kubectl get pods -A -o json` into the scratchpad and
  deleted with the cluster. Static-pod names carrying the node name are written
  `<node>` below.
- UTC stamps: t1 `2026-08-21T20:33:35Z` · t2 `20:42:42Z` · reboots `20:44–20:47Z`
  · t3 `20:47:12Z` · t4 `20:57:59Z`.

## 0. Setup

Eight pods, all `busybox`, each counting its restarts in an `emptyDir` that
survives a container restart:

- `settled-a..e` — fail five times, then `sleep 100000`. No probes, so `ready`
  goes true the moment the sixth run starts.
- `cycler` — fail three times, then a 600 s run ending `exit 137`, for ever.
- `notready-regular` — fails four times, then runs for ever, with an
  `exec: /bin/false` readiness probe.
- `notready-sidecar` — the same container as a **native sidecar**
  (`initContainers[].restartPolicy: Always`) with the same failing readiness
  probe, beside a healthy regular container.

```
$ kubectl --context kind-review get pods --no-headers \
    -o custom-columns='N:.metadata.name,R:.status.containerStatuses[*].restartCount,IR:.status.initContainerStatuses[*].restartCount'
cycler             3     <none>
notready-regular   4     <none>
notready-sidecar   0     4
settled-a          5     <none>
settled-b          5     <none>
settled-c          5     <none>
settled-d          5     <none>
settled-e          5     <none>
```

## 1. The healthy cluster is quiet

The pre-restart half of t1 (every namespace but `default`; `restartCount` sums to
`0`):

```
$ jq '{apiVersion,kind,items:[.items[]|select(.metadata.namespace!="default")]}' pods-t1.json > pods-healthy.json
$ jq '[.items[]|.status.containerStatuses[]?.restartCount]|add' pods-healthy.json
0
$ k8rs --analysis pods-healthy.json
9 pods · 0 nodes · 0 workloads

○ nothing is broken
...
[restarts]
  Containers that keep restarting
  Nothing here has restarted enough to matter. Every container serving right now has restarted 2 or fewer times since its pod started.
```

The same run, with no `Node` objects in the file, degraded per report and named
the verb and resource each time — Restarts itself was unaffected:

```
[capacity]
  Not checked. Reading what a node has needs permission to list nodes, and this login does not have it.
  Ask for permission to list nodes across the whole cluster.
[drain safety]
  Not checked. This report answers one question per node, and this login cannot list the nodes.
  Ask for permission to list nodes across the whole cluster.
[certificates]
  Machines waiting to join are not checked. Seeing them takes a cluster-wide list of joining requests, and k8rs does not have one.
  Ask for permission to list certificatesigningrequests across the whole cluster.
```

## 2. One snapshot, one process: a WARN card and "Nothing below is broken" about the same container

t4, a single invocation, output kept whole in `out-t4.txt`:

```
$ k8rs --analysis pods-t4.json > out-t4.txt; echo "exit=$?"
exit=0
$ grep -n -A3 "default/cycler" out-t4.txt
3:▲ default/cycler · 2 min ago
4-  Container has been restarted 8 times — it is serving now, but something keeps killing it
5-  container app · exit 137 (killed with SIGKILL — a stop the program cannot refuse, and the code does not say what sent it) · ran for 10 min · docker.io/library/busybox:latest
6-  → check the liveness and startup probes, whether it stops when asked to, and the memory limit: ...
--
75:  ○ default/cycler · container app
76-      Restarted 8 times since this pod started.
77-      This run started 2 min ago.
```

Line 74 of the same file is the pane's opening paragraph, directly above line 75:

```
$ sed -n '72,78p' out-t4.txt
[restarts]
  Containers that keep restarting
  Nothing below is broken. Every container here is running fine right now, and a restart count just stays on the record.
  ○ default/cycler · container app
      Restarted 8 times since this pod started.
      This run started 2 min ago.
  ○ default/settled-a · container app
```

The fields the two screens read: `restartCount: 8`, `ready: true`,
`state.running.startedAt` 2 minutes before `now`. Rule 5's serving card is
suppressed only once that run age passes `NOT_READY_GRACE` (10 min), measured
here as the `settled-*` cards present at t1 (`This run started 3 min ago`) and
absent at t2 (`This run started 13 min ago`) while their rows stayed on the pane
throughout.

## 3. The cap and the sort, on the container the pane was built for

t2. `cycler` was on a live 10-minute cycle; `settled-a..e` had stopped restarting
for good four minutes earlier.

```
$ kubectl --context kind-review get pod cycler -o jsonpath='{.status.containerStatuses[0].restartCount}{"  ready="}{.status.containerStatuses[0].ready}{"  startedAt="}{.status.containerStatuses[0].state.running.startedAt}'
4  ready=true  startedAt=2026-08-21T20:37:30Z
$ kubectl --context kind-review get pod settled-a -o jsonpath='...same...'
5  ready=true  startedAt=2026-08-21T20:29:38Z

$ k8rs --analysis pods-t2.json | sed -n '/\[restarts\]/,/^$/p'
[restarts]
  Containers that keep restarting
  Nothing below is broken. Every container here is running fine right now, and a restart count just stays on the record.
  ○ default/settled-a · container app
      Restarted 5 times since this pod started.
      This run started 13 min ago.
  ○ default/settled-b · container app
      Restarted 5 times since this pod started.
      This run started 13 min ago.
  ○ default/settled-c · container app
      Restarted 5 times since this pod started.
      This run started 13 min ago.
  ○ default/settled-d · container app
      Restarted 5 times since this pod started.
      This run started 13 min ago.
  ○ default/settled-e · container app
      Restarted 5 times since this pod started.
      This run started 13 min ago.
  and 1 more container keeps restarting
```

The one folded container is `cycler`.

## 4. A node reboot puts every container on the node above the threshold

Three `docker restart` of the node, waiting for `/readyz` between each:

```
$ for n in 1 2 3; do docker restart review-control-plane; <wait for /readyz>; done
$ kubectl --context kind-review get pods -A --no-headers \
    -o custom-columns='NS:.metadata.namespace,N:.metadata.name,R:.status.containerStatuses[*].restartCount'
default              cycler                                 7
default              notready-regular                       7
default              notready-sidecar                       3
default              settled-a                              8
default              settled-b                              8
default              settled-c                              8
default              settled-d                              8
default              settled-e                              8
kube-system          coredns-589f44dc88-bxr54               3
kube-system          coredns-589f44dc88-pt7kk               3
kube-system          etcd-<node>                            3
kube-system          kindnet-fgl9l                          3
kube-system          kube-apiserver-<node>                  3
kube-system          kube-controller-manager-<node>         3
kube-system          kube-proxy-j82ks                       3
kube-system          kube-scheduler-<node>                  3
local-path-storage   local-path-provisioner-855c7b7774-...  4
```

The pane at t3, ninety seconds after the last reboot:

```
$ k8rs --analysis pods-t3.json | sed -n '/\[restarts\]/,/^$/p'
[restarts]
  Containers that keep restarting
  Nothing below is broken. Every container here is running fine right now, and a restart count just stays on the record.
  ○ default/settled-a · container app
      Restarted 8 times since this pod started.
      This run started 1 min ago.
  ... settled-b, settled-c, settled-d, settled-e, identical ...
  and 11 more containers keep restarting
```

Qualifying-container count before the reboots: 6. After: 17. Nothing on the
cluster failed.

## 5. Rule 7 does not fire for a not-ready sidecar

t4. Both containers are `Running`, `ready: false`, `started: true`, above the
threshold:

```
$ kubectl --context kind-review get pod notready-sidecar -o jsonpath='{range .status.initContainerStatuses[*]}name={.name} ready={.ready} restarts={.restartCount} state={.state} started={.started}{"\n"}{end}'
name=proxy ready=false restarts=7 state={"running":{"startedAt":"2026-08-21T20:45:38Z"}} started=true

$ kubectl --context kind-review get pod notready-regular -o jsonpath='{range .status.containerStatuses[*]}name={.name} ready={.ready} restarts={.restartCount} started={.started} running={.state.running.startedAt}{"\n"}{end}'
name=app ready=false restarts=7 started=true running=2026-08-21T20:45:42Z
```

Rule 7 cards in the whole t4 run:

```
$ grep -c "Running, but not receiving traffic" out-t4.txt
1
$ grep -B1 "Running, but not receiving traffic" out-t4.txt
▲ default/notready-regular · 12 min ago
  Running, but not receiving traffic — the readiness check is failing
```

What the sidecar carries instead, same run:

```
▲ default/notready-sidecar · 12 min ago
  Container has been restarted 7 times, and the exit code is not its own
  sidecar container proxy (it runs beside the app the whole time) · exit 255 (the node found the container dead, so this number stands in for a code nobody read) · docker.io/library/busybox:latest
```

Neither container appears on the Restarts pane at t4.

## 6. Diff of the frozen file

Every non-doc line the turn changed in `rules.rs`:

```
$ git diff HEAD~1 -- src/rules.rs | grep -E '^[+-][^+-]' | grep -v '^[+-]///'
-const RESTARTS_WARN: i32 = 3;
+pub(crate) const RESTARTS_WARN: i32 = 3;
-fn container_fact(c: &ContainerSnapshot) -> String {
+pub(crate) fn container_fact(c: &ContainerSnapshot) -> String {
-fn doing_its_job(c: &ContainerSnapshot) -> bool {
+pub(crate) fn doing_its_job(c: &ContainerSnapshot) -> bool {
```

The rest of the diff is three doc paragraphs added above those three items —
`RESTARTS_WARN` at `rules.rs:2316`, `container_fact` at `:2908`, `doing_its_job`
at `:2950`. No statement inside any function body changed.

## 7. Teardown

```
$ K8RS_CLUSTER=review ./scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-control-plane"]
$ kind get clusters
k8rs
```

The PM's `k8rs` fixture cluster was up throughout (22 h at the start, idle at
13%/2.6%/2.3%/3.6% CPU and 1.9 GiB total) and was not touched. Host had 16 GiB
free of 23 GiB with the review cluster running, so D84's memory-starvation
mechanism was not in play; no OOM shape was measured here in any case.
