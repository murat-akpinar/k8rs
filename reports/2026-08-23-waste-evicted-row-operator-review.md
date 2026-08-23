# Waste's evicted row — what the committed corpus says (2026-08-23)

Operator review of the re-opened Phase 4 Waste box
([D155](../NOTES.md#d155--a-whole-project-review-found-two-boxes-checked-over-work-their-own-text-does-not-describe-2026-08-22)),
measured on the working tree at the review dispatch. **No cluster was created**:
the PM held `k8rs` for the capture this turn, so everything below is the
committed corpus, the real binary, and upstream source read at HEAD.

## 1 — the pane the binary prints

```
$ cargo build --quiet
$ ./target/debug/k8rs --analysis \
    tests/fixtures/evicted.json tests/fixtures/failed.json \
    tests/fixtures/succeeded.json tests/fixtures/services.json \
    tests/fixtures/endpointslices.json tests/fixtures/persistentvolumeclaims.json \
    tests/fixtures/nodes.json
```

```
3 pods · 4 nodes

● k8rs-worker3 · 47 hours ago
  This node has stopped responding — nothing on it can be trusted until it does
  → check the node itself: is it powered on and reachable?

1 critical
...
[waste]
  Things that cost you something for nothing
  ● default/broken-noendpoints matches no pod
      This Service points at nothing. Anything calling it gets a 503.
      → fix its selector, or delete it
  ▲ default/broken-unused-disk is 128Mi nobody is using
      A disk was reserved for it and no pod is mounting it. …
  ▲ default/healthy-disk is 64Mi nobody is using
      A disk was reserved for it and no pod is mounting it. …
  ▲ 1 pod was removed by a node and remains
      A node does this when it runs out of room. It is often the only sign left once the node recovers.
      → check Alerts for what a node is low on
  ○ 2 pods finished and were never removed
      Kubernetes keeps a few finished Jobs by default, so some of this is normal. …
  Replicasets parked at 0 replicas are not checked. …
```

Exit status 0. The Alerts half of the same run draws **one** card, about
`k8rs-worker3` being unreachable, and **no** N3 card.

## 2 — the fields on the capture behind that row

```
$ python3 -c "import json; d=json.load(open('tests/fixtures/evicted.json')); s=d['status']; \
print(s['phase'], '|', s['reason'], '|', s['message']); \
print([c['type']+'='+c['status'] for c in s['conditions']]); \
print(d['spec']['nodeName'], '|', [ (c['name'], list(c['state']), c['state']['terminated']['exitCode'], c['state']['terminated']['reason']) for c in s['containerStatuses'] ])"
```

| field | value |
|---|---|
| `status.phase` | `Failed` |
| `status.reason` | `Evicted` |
| `status.message` | `Pod ephemeral local storage usage exceeds the total limit of containers 8Mi. ` |
| `status.conditions[].type` | `PodReadyToStartContainers`, `Initialized`, `Ready`, `ContainersReady`, `PodScheduled` |
| `DisruptionTarget` condition | **absent** |
| `spec.nodeName` | `k8rs-worker` |
| `status.containerStatuses[0].state.terminated` | `exitCode: 137`, `reason: "Error"` |

## 3 — the node that pod ran on, in the capture taken beside it

```
$ python3 -c "import json; d=json.load(open('tests/fixtures/nodes.json')); \
[print(n['metadata']['name'], {c['type']:c['status'] for c in n['status']['conditions'] if 'Pressure' in c['type'] or c['type']=='Ready'}) for n in d['items']]"
```

```
k8rs-control-plane {'MemoryPressure': 'False', 'DiskPressure': 'False', 'PIDPressure': 'False', 'Ready': 'True'}
k8rs-worker        {'MemoryPressure': 'False', 'DiskPressure': 'False', 'PIDPressure': 'False', 'Ready': 'True'}
k8rs-worker2       {'MemoryPressure': 'False', 'DiskPressure': 'False', 'PIDPressure': 'False', 'Ready': 'True'}
k8rs-worker3       {'MemoryPressure': 'Unknown', 'DiskPressure': 'Unknown', 'PIDPressure': 'Unknown', 'Ready': 'Unknown'}
```

`scripts/broken.yaml` § `broken-evicted` records the same thing from the trip:
*"Measured after the eviction below: `DiskPressure` stayed `False` on all four
nodes."*

## 4 — upstream, at HEAD, on what writes `status.reason: Evicted`

```
$ curl -sS https://raw.githubusercontent.com/kubernetes/kubernetes/master/pkg/kubelet/eviction/helpers.go | grep -n 'Reason = \|MessageFmt = '
44:	Reason = "Evicted"
46:	nodeLowMessageFmt = "The node was low on resource: %v. "
48:	nodeConditionMessageFmt = "The node had condition: %v. "
50:	containerMessageFmt = "Container %s was using %s, request is %s, has larger consumption of %v. "
56:	podEphemeralStorageMessageFmt = "Pod ephemeral local storage usage exceeds the total limit of containers %s. "
58:	emptyDirMessageFmt = "Usage of EmptyDir volume %q exceeds the limit %q. "
```

```
$ curl -sS .../pkg/kubelet/eviction/eviction_manager.go | grep -n 'localStorageEviction\|DisruptionTarget\|PodReasonTerminationByKubelet\|status.Reason = Reason'
371:	// If eviction happens in localStorageEviction function, skip the rest of eviction action
373:		if evictedPods := m.localStorageEviction(logger, activePods, statsFunc); len(evictedPods) > 0 {
441:		condition := &v1.PodCondition{
442:			Type:               v1.DisruptionTarget,
445:			Reason:             v1.PodReasonTerminationByKubelet,
648:		status.Reason = Reason
```

Two producers of `Evicted`, both reaching the same `evictPod` at line 633:

- **node-pressure** (line ~429): passes the `DisruptionTarget` /
  `TerminationByKubelet` condition and a message beginning
  `The node was low on resource: …`.
- **localStorageEviction** (lines 514–630: `emptyDirLimitEviction`,
  `podEphemeralStorageLimitEviction`, `containerEphemeralStorageLimitEviction`):
  passes `nil` for the condition, and runs *before* any threshold is evaluated.

## 5 — other `status.reason` values the kubelet writes

```
$ curl -sS .../pkg/kubelet/lifecycle/predicate.go   # admission rejection
OutOfCPU = "OutOfcpu" · OutOfMemory = "OutOfmemory"
OutOfEphemeralStorage = "OutOfephemeral-storage" · OutOfPods = "OutOfpods"
UnexpectedAdmissionError · PodOSNotSupported · InvalidNodeInfo · UnknownReason

$ curl -sS .../pkg/kubelet/nodeshutdown/nodeshutdown_manager.go | grep -n 'Reason\s*=\|Message\s*='
84:	NodeShutdownNotAdmittedReason  = "NodeShutdown"
85:	nodeShutdownNotAdmittedMessage = "Pod was rejected as the node is shutting down."
88:	nodeShutdownReason  = "Terminated"
89:	nodeShutdownMessage = "Pod was terminated in response to imminent node shutdown."
```

`Shutdown` is **not** among them. Graceful node shutdown of a *running* pod
writes `Terminated`; rejection during shutdown writes `NodeShutdown`.
`screens/analysis.md` § *The pileup splits in two, one per cause* (line 1205)
and `src/analysis_tests/waste.rs` § `only_the_pods_a_node_removed_leave_the_completed_row`
both name `Shutdown`.

## 6 — what `kubectl get pods` prints in STATUS for this object

```
$ curl -sS .../pkg/printers/internalversion/printers.go | sed -n '974,1080p'
975:	podPhase := pod.Status.Phase
976:	reason := string(podPhase)
977:	if pod.Status.Reason != "" { reason = pod.Status.Reason }
...
1071:		case container.State.Terminated != nil:
1072:			if len(container.State.Terminated.Reason) > 0 { reason = container.State.Terminated.Reason }
```

`pod.Status.Reason` is set first and then **overwritten** by the container's
terminated reason when one exists. This capture's container carries
`reason: "Error"`, so `kubectl get pods` prints `Error`, not `Evicted`, for it.
`--field-selector` accepts `status.phase`; it has no `status.reason` key, so the
reason is reachable only through `-o custom-columns` / `-o jsonpath`.

## 7 — the corpus-wide checks that were run

```
$ cargo test --quiet waste
test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 501 filtered out

$ cargo test --quiet the_whole_capture_through_the_rules_at_once
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 517 filtered out
```

Every place `analysis.rs` and the N-series read pods, checked by hand for the
`finished` gate:

| site | file:line | gate |
|---|---|---|
| Capacity node rows | `analysis.rs:436` | `pods_on` → `!finished` |
| Capacity uncapped workloads | `analysis.rs:649` | `!finished(pod)` |
| Drain safety moving list | `analysis.rs:875` | `pods_on` → `!finished` |
| Waste disks nobody mounts | `analysis.rs:1864` | **no gate, deliberate** (doc: a finished Job pod is evidence a disk is mounted every run) |
| Waste pileups | `analysis.rs:1963` | `finished(pod)`, then partitioned |
| Posture host paths | `analysis.rs:2240` | `!finished(pod)` |
| Restarts | `analysis.rs:2583` | `ContainerState::Running` |
| every node rule | `rules.rs:6580` | `pods_on` → `!finished` |
| `analyze` pod loop | `rules.rs:2441` | `if finished(pod) { continue }` after rule 12 |

## 8 — the shape of the diff against D155's re-opened window

```
$ git diff --stat src/
 src/analysis.rs             | 132 ++++++++++++------
 src/analysis_tests/waste.rs | 323 ++++++++++++++++++++++++++++++++++++++------
 src/k8s.rs                  |   1 +
 src/rules.rs                |  18 +++
 src/rules_tests.rs          |   3 +-
 src/rules_tests/pod.rs      |  20 +++
 src/rules_tests/snapshot.rs |  55 ++++++++
```

`src/rules.rs`: one field (`pub reason: Option<String>`), one decode line
(`reason: status.reason`), one doc comment. `finished()` byte-identical.
`analyze` byte-identical. `src/k8s.rs`: one line,
`maybe(&mut self.reason, IDENTIFIER)` in `impl Bounded for PodSnapshot`.
No other snapshot field arrived.
