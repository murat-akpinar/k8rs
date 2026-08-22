# 2026-08-20 — `settled` against a live retry rule, and the two commands one container gets

Measured for the operator review of the rule 6 / rule 15 family (`settled`,
`last_run_on_record`, `previous_run_failed`, `stopped_for_good`, and rule 5
`restarting_repeatedly`, which prints the same shared action sentence). Ephemeral kind
cluster, `K8RS_CLUSTER=review`, `kindest/node:v1.36.1`, one worker, brought up and torn
down inside this run. **No fixture was produced**; every pod below was written for this
run, in a namespace `probe`, with no `demo=broken` label. Container IDs are redacted to
`containerd://…`.

```
$ K8RS_CLUSTER=review K8RS_WORKERS=1 ./scripts/cluster.sh up
node/review-control-plane condition met
node/review-worker condition met
API: https://127.0.0.1:6443   context: kind-review

$ kubectl --context kind-review version -o json | jq -r '"client \(.clientVersion.gitVersion) server \(.serverVersion.gitVersion)"'
client v1.36.3 server v1.36.1
```

Two binaries were built from the same tree into the scratchpad, each with one line added to
`main.rs`'s `card()` so the finding's `kubectl_cmd` prints under the action — the shipped
driver does not print it (§ 6). **NEW** is the working tree; **OLD** is `git show
HEAD:src/rules.rs` — i.e. the code before this diff — dropped into an otherwise identical
copy. Neither build touched the repo.

## 1 — the API requires a `restartPolicy` beside `restartPolicyRules`

```
$ kubectl --context kind-review apply -f gangwait.yaml     # container with rules, no restartPolicy
The Pod "gangwait" is invalid: spec.containers[1].restartPolicy: Required value: must specify restartPolicy when restart rules are used
(exit 2)
```

The pod was rejected until `restartPolicy: Never` was added beside the rules on that
container; the version below carries it.

## 2 — a `Restart` rule keeps a container in `state.terminated` for most of the backoff

`retryloop`: pod `restartPolicy: Never`; container `retry` with its own `restartPolicy:
Never` and one rule `{action: Restart, exitCodes: {operator: In, values: [3]}}`, `sh -c`
writing a per-run counter to an `emptyDir` and always `exit 3`; a second container `keeper`
running `sleep 86400` so the pod stays `Running`.

```
$ kubectl --context kind-review -n probe get pod retryloop -o json | jq -r '.status.containerStatuses[]|select(.name=="retry")| "restarts=\(.restartCount) state=\(.state|keys[0]) reason=\(.state.waiting.reason // .state.terminated.reason) exit=\(.state.terminated.exitCode // "-") last=\(.lastState.terminated.exitCode // "none")"'
```

| sample | output |
|---|---|
| 12:31:42 | `restarts=0 state=waiting reason=ContainerCreating exit=- last=none` |
| 12:31:52 | `restarts=1 state=waiting reason=CrashLoopBackOff exit=- last=3` |
| 12:32:02 | `restarts=2 state=terminated reason=Error exit=3 last=3` |
| 12:32:12 | `restarts=2 state=terminated reason=Error exit=3 last=3` |
| 12:32:23 | `restarts=2 state=terminated reason=Error exit=3 last=3` |
| 12:32:33 | `restarts=3 state=terminated reason=Error exit=3 last=3` |
| 12:32:43 | `restarts=3 state=terminated reason=Error exit=3 last=3` |
| 12:32:53 | `restarts=3 state=terminated reason=Error exit=3 last=3` |
| 12:33:03 | `restarts=3 state=terminated reason=Error exit=3 last=3` |
| 12:33:13 | `restarts=3 state=terminated reason=Error exit=3 last=3` |
| 12:33:23 | `restarts=4 state=running reason=- exit=- last=3` |
| 12:33:33 | `restarts=4 state=terminated reason=Error exit=3 last=3` |
| 12:33:43 | `restarts=4 state=terminated reason=Error exit=3 last=3` |
| 12:33:53 | `restarts=4 state=terminated reason=Error exit=3 last=3` |

Twelve of fourteen samples over 2m11s are `state=terminated` on a container the kubelet
restarted five times. The `restartCount` moved 0 → 5 while the effective policy read `Never`
throughout, and the `Restart` action left no reason of its own anywhere in the status —
`lastState.terminated.reason` is the bare `Error` a plain bad exit gets.

## 3 — the same container, ten seconds apart, two card counts and two commands

Same pod, the terminated half and the waiting half of one backoff.

```
$ k8rs <live retryloop object, state=terminated, restarts=4>          # NEW
▲ probe/retryloop · 1 min ago
  Container has been restarted 4 times, but something keeps killing it
  container retry · exit 3 · ran for under a second · docker.io/library/busybox:latest
  → read the last run's log — it holds the last thing written before that run ended, from the program or from the shell that started it. The command below is what fetches it
  $ kubectl logs retryloop -c retry -n probe --previous

▲ probe/retryloop · 1 min ago
  The last run on record failed — exit 3
  container retry · ran for under a second
  → read the last run's log — it holds the last thing written before that run ended, from the program or from the shell that started it. The command below is what fetches it
  $ kubectl logs retryloop -c retry -n probe

2 warnings

$ k8rs <same object, OLD>
▲ probe/retryloop · 1 min ago
  Container has been restarted 4 times, but something keeps killing it
  container retry · exit 3 · ran for under a second · docker.io/library/busybox:latest
  → read the last run's log — … The --previous flag below is what fetches it
  $ kubectl logs retryloop -c retry -n probe --previous

1 warning

$ k8rs <live retryloop object 3m34s later, state=waiting CrashLoopBackOff, restarts=5>   # NEW
● probe/retryloop · 2 min ago
  Container keeps crashing, and each restart waits longer (CrashLoopBackOff)
  container retry · 5 restarts · ran for under a second · exit 3
  → read the last run's log — … The command below is what fetches it
  $ kubectl logs retryloop -c retry -n probe --previous

1 critical
```

Both commands were run against the cluster in the terminated half:

```
$ kubectl --context kind-review logs retryloop -c retry -n probe
this is run 5 speaking
(rc=0)

$ kubectl --context kind-review logs retryloop -c retry -n probe --previous
unable to retrieve container logs for containerd://…(rc=0)
```

`--previous` prints that line to **stdout** and exits `0`.

## 4 — the settled ladder: `restartCount=4`, `state.terminated exit 1`, `lastState exit 3`

`retryladder`: the same manifest, but the script exits `3` for the first four runs and `1`
after that, so the rule stops matching and the container settles. This is
`tests/fixtures/neverrules.json`'s shape with a longer ladder.

```
$ kubectl --context kind-review -n probe get pod retryladder -o json | jq -r '"podPolicy=\(.spec.restartPolicy) phase=\(.status.phase)", (.status.containerStatuses[]|"\(.name): restarts=\(.restartCount) state=\(.state|keys[0]) exit=\(.state.terminated.exitCode // "-")/\(.state.terminated.reason // "-") last=\(.lastState.terminated.exitCode // "-")/\(.lastState.terminated.reason // "-")")'
podPolicy=Never phase=Running
keeper: restarts=0 state=running exit=-/- last=-/-
retry: restarts=4 state=terminated exit=1/Error last=3/Error

$ kubectl --context kind-review -n probe get pods
NAME          READY   STATUS             RESTARTS        AGE
gangwait      1/2     Error              2               106s
retryladder   1/2     Error              4 (7m27s ago)   8m15s
retryloop     1/2     CrashLoopBackOff   5 (2m18s ago)   5m24s

$ kubectl --context kind-review logs retryladder -c retry -n probe
this is run 5 speaking
(rc=0)

$ kubectl --context kind-review logs retryladder -c retry -n probe --previous
unable to retrieve container logs for containerd://…(rc=0)
```

k8rs over that object:

```
$ k8rs <live retryladder object>          # NEW
▲ probe/retryladder · 1 min ago
  Container has been restarted 4 times, but something keeps killing it
  container retry · exit 3 · ran for under a second · docker.io/library/busybox:latest
  → read the last run's log — … The command below is what fetches it
  $ kubectl logs retryladder -c retry -n probe --previous

▲ probe/retryladder · 49s ago
  The last run on record failed — exit 1 (the application's own error)
  container retry · ran for under a second
  → read the last run's log — … The command below is what fetches it
  $ kubectl logs retryladder -c retry -n probe

2 warnings

$ k8rs <same object>                       # OLD
▲ probe/retryladder · 1 min ago
  Container has been restarted 4 times, but something keeps killing it
  container retry · exit 3 · ran for under a second · docker.io/library/busybox:latest
  → read the last run's log — … The --previous flag below is what fetches it
  $ kubectl logs retryladder -c retry -n probe --previous

1 warning
```

## 5 — a `RestartAllContainers` sibling: rule 15's card, then the restart 48s later

`gangwait`: pod `restartPolicy: Never`; `bystander` (`echo …; exit 1`, no rules, no policy of
its own); `trigger` (`restartPolicy: Never` plus `{action: RestartAllContainers, exitCodes:
{operator: In, values: [3]}}`, `sleep 75` then `exit 3`).

```
$ kubectl --context kind-review -n probe get pod gangwait -o json | jq -r '"phase=\(.status.phase)", (.status.containerStatuses[]|"\(.name): restarts=\(.restartCount) state=\(.state|keys[0]) exit=\(.state.terminated.exitCode // "-")/\(.state.terminated.reason // "-") last=\(.lastState.terminated.reason // "none")")'
phase=Running
bystander: restarts=0 state=terminated exit=1/Error last=none
trigger:   restarts=0 state=running  exit=-/-     last=none

$ k8rs <that object>                       # NEW
● probe/gangwait · 24s ago
  This container has stopped and nothing is starting it again
  container bystander · exit 1 (the application's own error) · ran for under a second
  → read its log — that is where it says why it stopped. Nothing is waiting to start it again, so the pod has to be replaced; until it is, whatever needed this container is still without it
  $ kubectl logs gangwait -c bystander -n probe

1 critical
```

48 seconds later the trigger fired:

| sample | output |
|---|---|
| 12:35:56 | `bystander:r0/terminated/-  trigger:r0/running/-` |
| 12:36:08 | `bystander:r0/terminated/-  trigger:r0/running/-` |
| 12:36:20 | `bystander:r0/terminated/-  trigger:r0/running/-` |
| 12:36:32 | `bystander:r0/terminated/-  trigger:r0/running/-` |
| 12:36:44 | `bystander:r1/terminated/RestartingAllContainers  trigger:r1/running/RestartingAllContainers` |

```
$ kubectl --context kind-review -n probe get pod gangwait -o json | jq -r …
phase=Running
bystander: restarts=1 state=terminated exit=1/Error last=137/RestartingAllContainers
trigger:   restarts=1 state=running    exit=-/-     last=137/RestartingAllContainers

$ kubectl --context kind-review -n probe get pods | grep gangwait
gangwait   1/2   Error   2   106s

$ k8rs <that object>                       # NEW and OLD, identical
1 pod · 0 nodes · 0 workloads

○ nothing is broken
```

The restart count moved `0 → 1` on both containers when the gang rule fired, so
`restartCount == 0` beside a `RestartingAllContainers` record was not produced by this run.

## 6 — the shipped driver prints no per-finding command

Unpatched binary, working tree, over the committed capture:

```
$ cargo build --release && ./target/release/k8rs tests/fixtures/neverrules.json
1 pod · 0 nodes · 0 workloads

▲ default/broken-neverrules · 3 days ago
  The last run on record failed — exit 1 (the application's own error)
  container retry · ran for under a second
  → read the last run's log — it holds the last thing written before that run ended, from the program or from the shell that started it. The command below is what fetches it

1 warning
```

`Finding::kubectl_cmd` is not rendered by `main.rs`'s `card()`; `screens/once.md`'s mockup
carries no per-card command either, and its stderr block holds the commands k8rs ran
(`kubectl get pods -A`). `screens/alerts.md`'s card is four parts with no command line, and
its command-log strip is the bottom pane of the whole screen.

## 7 — the whole committed corpus, OLD vs NEW

```
$ k8rs tests/fixtures/*.json    # OLD and NEW, diffed
```

Nine cards carry the reworded action sentence — `The --previous flag below is what fetches
it` became `The command below is what fetches it` — and eight of the nine still ship a
`--previous` command:

```
$ grep -c "The command below is what fetches it" corpus-new.txt
9
$ grep -A1 "The command below is what fetches it" corpus-new.txt | grep -c -- --previous
8
```

The ninth is the one card that changes substance, `default/broken-neverrules`:

```
-  The last run on record failed — exit 3
-  $ kubectl logs broken-neverrules -c retry -n default --previous
+  The last run on record failed — exit 1 (the application's own error)
+  $ kubectl logs broken-neverrules -c retry -n default
```

No other card in the 51 files changes.

## 8 — plants off `neverrules.json`, run through both binaries

Decoded plants written into the scratchpad, never into `tests/`; each is the committed
capture with the named fields changed.

| plant | change | OLD | NEW |
|---|---|---|---|
| A | `retry.restartCount` 1 → 5 | 1 card (rule 5, `--previous`, exit 3) | 2 cards: rule 5 exit 3 `--previous`, rule 6 exit 1 plain `logs` |
| B | `state.terminated` → `137/OOMKilled`, count 1 | 1 card: `The last run on record failed — exit 3` | `○ nothing is broken` |
| D | pod `OnFailure`, no container rules, `state.terminated` → `0/Completed`, `lastState` → `1/Error` | 1 card: `The last run on record failed — exit 1` | `○ nothing is broken` |
| E | plant D with `restartCount: 3` | 1 card: `restarted 3 times, but something keeps killing it`, exit 1, `--previous` | same card, unchanged |
| F | pod `Always`, container `restartPolicy: Never`, no rules, count 3, `lastState` → `137/ContainerStatusUnknown` | 1 card: `restarted 3 times, and the record names no ending` | that card **plus** `The last run on record failed — exit 1`, plain `logs` |
| G | plant F with `lastState` → `1/Error` | 1 card: `restarted 3 times, but something keeps killing it`, exit 1, `--previous` | that card **plus** `The last run on record failed — exit 1`, plain `logs` |

## 9 — the corpus, containers sitting in `state.terminated`

```
$ jq over tests/fixtures/*.json for containerStatuses with .state.terminated
failed.json        broken-failed/app        [reg]  phase=Failed   podPol=OnFailure restarts=4  state=137/ContainerStatusUnknown last=1/Error
healthy.json       healthy/migrate          [init] phase=Running  podPol=Always    restarts=0  state=0/Completed               last=none
healthy-retry.json healthy-retry/wait-for-db[init] phase=Running  podPol=Always    restarts=3  state=0/Completed               last=1/Error
init.json          broken-init/migrate      [init] phase=Pending  podPol=Always    restarts=10 state=1/Error                   last=1/Error
neverback.json     broken-neverback/broke   [reg]  phase=Running  podPol=Never     restarts=0  state=1/Error                   last=none
neverback.json     broken-neverback/done    [reg]  phase=Running  podPol=Never     restarts=0  state=0/Completed               last=none
neverrules.json    broken-neverrules/retry  [reg]  phase=Running  podPol=Never     restarts=1  state=1/Error                   last=3/Error
notfound.json      broken-notfound/app      [reg]  phase=Running  podPol=Always    restarts=10 state=127/Error                 last=127/Error
oom.json           broken-oom/hog           [reg]  phase=Running  podPol=Always    restarts=10 state=137/OOMKilled             last=137/OOMKilled
owned-pods.json    …/quitter                [reg]  phase=Running  podPol=Always    restarts=10 state=1/Error                   last=1/Error
succeeded.json     broken-succeeded/migrate [reg]  phase=Succeeded podPol=OnFailure restarts=3 state=0/Completed               last=1/Error
```

No committed container has `restartCount == 0` beside a populated `lastState`:

```
$ jq over tests/fixtures/*.json for restartCount==0 and (lastState|keys|length)>0
(no output)
```

`gang.json`'s two containers are `restarts=3` each with `137/RestartingAllContainers` in
`lastState`, both `state=running`.

## 10 — the settled container's log, 12 minutes on

```
$ date +%T; kubectl --context kind-review -n probe get pods --no-headers
12:41:46
gangwait      1/2   Error              8               6m27s
retryladder   1/2   Error              4 (12m ago)     12m
retryloop     1/2   CrashLoopBackOff   6 (4m16s ago)   10m

$ kubectl --context kind-review logs retryladder -c retry -n probe
this is run 5 speaking
(rc=0)

$ kubectl --context kind-review logs retryladder -c retry -n probe --previous
unable to retrieve container logs for containerd://…(rc=0)
```

Both answers are unchanged from the reading taken one minute after the container settled
(§ 4). Longer horizons were not measured here;
`reports/2026-08-16-previous-logs-resize-and-the-probe-floor.md` and NOTES § D97 carry the
46-minute and node-reboot readings, taken at `restartCount == 0`.

## 11 — the gang pod at five restarts

The `RestartAllContainers` rule kept firing every 75s. At `restartCount == 5` on both
containers:

```
$ k8rs <live gangwait object>
▲ probe/gangwait
  Container has been restarted 5 times, and the record names the pod's rule
  container bystander · exit 137 (removed so Kubernetes could restart every container in the pod, which is what this pod asked for) · docker.io/library/busybox:latest
  → this record does not say which container exited — the one whose spec declares the restart rule (restartPolicyRules) can set it off, and that may be this container
  $ kubectl get pod gangwait -n probe -o yaml

▲ probe/gangwait · 11s ago
  Container has been restarted 5 times — it is serving now, and the record names the pod's rule
  container trigger · exit 137 (…same translation…)
  → …same sentence…
  $ kubectl get pod gangwait -n probe -o yaml

2 warnings
```

Identical on OLD and NEW. The bystander's card carries no age; its record has no stamps.

## Teardown

```
$ K8RS_CLUSTER=review ./scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-control-plane" "review-worker"]

$ kind get clusters
No kind clusters found.
```
