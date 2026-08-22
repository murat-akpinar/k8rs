# 2026-08-22 — Phase 4 close, the seven analysis reports read together

Operator review at Phase 4 close (`CLAUDE.md` § Phase close, step 4 — the family
read, D103). Everything below was run on the dev machine against the committed
corpus and against synthetic inputs written to a scratchpad, never committed. **No
cluster was brought up**; the tree is mid-close and no capture ran beside this.

Machine and build:

```
$ cargo build --release
   Compiling k8rs v0.0.0 (/home/shyuuhei/GIT/k8rs)
    Finished `release` profile [optimized] target(s) in 1m 06s
```

The baseline run reproduces the LAN host's run byte for byte, so every line quoted
below is the same output the phase-close step 2 read:

```
$ ./target/release/k8rs --analysis tests/fixtures/*.json > local-run.txt; echo "EXIT=$?"
EXIT=0
$ diff <(sed -e '1d' -e '$d' host-run.txt) local-run.txt && echo "IDENTICAL to host run"
IDENTICAL to host run
```

---

## 1. A `SyncFailed` PodDisruptionBudget that has never synced draws the pane's quietest row

`src/analysis.rs:914-919` asks `has_not_caught_up` before `blocks_a_drain`, so a
budget whose `status.observedGeneration` is behind `metadata.generation` never
reaches `blocks_a_drain`'s `SyncFailed` branch (`src/analysis.rs:1482-1494`).

**Upstream, `release-1.34`, both halves fetched rather than recalled.**

`pkg/controller/disruption/disruption.go`, the function that writes the
`SyncFailed` condition:

```go
func (dc *DisruptionController) failSafe(ctx context.Context, pdb *policy.PodDisruptionBudget, err error) error {
	newPdb := pdb.DeepCopy()
	newPdb.Status.DisruptionsAllowed = 0
	...
	apimeta.SetStatusCondition(&newPdb.Status.Conditions, metav1.Condition{
		Type:               policy.DisruptionAllowedCondition,
		Status:             metav1.ConditionFalse,
		Reason:             policy.SyncFailedReason,
		Message:            err.Error(),
		ObservedGeneration: newPdb.Status.ObservedGeneration,
	})
	return dc.getUpdater()(ctx, newPdb)
}
```

It does **not** advance `Status.ObservedGeneration`; only `updatePdbStatus` does,
with `ObservedGeneration: pdb.Generation`.

`pkg/registry/core/pod/storage/eviction.go`:

```go
func (r *EvictionREST) checkAndDecrement(namespace string, podName string, pdb policyv1.PodDisruptionBudget, dryRun bool) error {
	if pdb.Status.ObservedGeneration < pdb.Generation {
		return createTooManyRequestsError(pdb.Name)
	}
```

So a budget whose first sync failed sits at `generation: 1`,
`observedGeneration: 0`, `disruptionsAllowed: 0`, condition reason `SyncFailed`,
and the eviction API refuses every eviction of its pods until a human fixes it.

**Measured, two runs one field apart.** Both inputs are a synthetic budget in the
scratchpad selecting the committed Deployment's pods (`app: healthy-deploy`,
`minAvailable: 2`), with the committed `healthy-deploy-pods.json` and
`nodes.json`. The only difference between A and B is `status.observedGeneration`.

```
$ ./target/release/k8rs --analysis pdb-syncfailed.json \
    tests/fixtures/healthy-deploy-pods.json tests/fixtures/nodes.json

=== A: SyncFailed, never synced (observedGeneration 0 < generation 1) ===
[drain safety]
  If you drained each node, what happens?
  A drain below assumes --ignore-daemonsets, so DaemonSet pods never count as moving.
  ● k8rs-worker3 would never finish draining
      This node has stopped responding. A drain cannot confirm a pod is gone until it answers again, so it waits forever.
      and default/never-synced's numbers have not caught up yet — check again in a moment
      → check the node itself: is it powered on and reachable?
    k8rs-control-plane is ready to drain — nothing on it would move
    k8rs-worker is ready to drain — nothing on it would move
    k8rs-worker2 needs a moment before it can be checked
      default/never-synced was just changed and Kubernetes has not finished counting its healthy pods — the change is version 1, the count is from version 0.
      → wait a few seconds and look again — if it never catches up, check that the cluster's controller manager is running

=== B: same condition, observedGeneration 1 == generation 1 ===
[drain safety]
  If you drained each node, what happens?
  A drain below assumes --ignore-daemonsets, so DaemonSet pods never count as moving.
  ● k8rs-worker2 would never finish draining
      Kubernetes could not work out how many copies of the pods default/synced-then-failed protects are healthy, so it will not let any of them be moved. The numbers on it are not a measurement of anything.
      → check what default/synced-then-failed points at — this happens when it names something Kubernetes cannot count copies of
  ● k8rs-worker3 would never finish draining
      This node has stopped responding. A drain cannot confirm a pod is gone until it answers again, so it waits forever.
      Kubernetes could not work out how many copies of the pods default/synced-then-failed protects are healthy, so it will not let any of them be moved. The numbers on it are not a measurement of anything.
      → check the node itself: is it powered on and reachable?
    k8rs-control-plane is ready to drain — nothing on it would move
    k8rs-worker is ready to drain — nothing on it would move
```

In A the row carries **no band** (`severity: None`, `src/analysis.rs:1135`) and
sorts at `band: 0` **with the ready nodes** (`src/analysis.rs:1128-1131`).

**Which shapes the tests were fed.** `src/analysis_tests/drain.rs:1447`
(`a_budget_the_controller_could_not_compute_says_so_instead_of_inventing_the_numbers`)
plants the `SyncFailed` reason onto a *captured* budget, and
`src/analysis_tests.rs:155-161` states that a captured budget always has its status
caught up. `src/analysis_tests/drain.rs:1279-1282` plants the generation gap on a
budget with `disruptions_allowed: Some(1)` and no `SyncFailed`. No test in
`src/analysis_tests/drain.rs` feeds both fields at once — checked by listing every
test in the module and grepping `SyncFailed`.

The committed budgets both sit at generation 1 / observedGeneration 1:

```
$ jq -r '.items[] | "\(.metadata.namespace)/\(.metadata.name)\tgeneration=\(.metadata.generation)\tobservedGeneration=\(.status.observedGeneration)\tdisruptionsAllowed=\(.status.disruptionsAllowed)\tcond=\([.status.conditions[]?|.type+"="+.reason]|join(","))"' \
    tests/fixtures/poddisruptionbudgets.json
default/broken-pdb-floor	generation=1	observedGeneration=1	disruptionsAllowed=0	cond=DisruptionAllowed=InsufficientPods
default/healthy-pdb-room	generation=1	observedGeneration=1	disruptionsAllowed=1	cond=DisruptionAllowed=SufficientPods
```

**What it would take for this to be wrong:** `failSafe` advancing
`Status.ObservedGeneration` on some path not shown above, or the disruption
controller writing a successful status before the first failed sync. Neither is in
the fetched source.

---

## 2. `34 workloads` on a screen whose header says `16 workloads`

`src/analysis.rs:644-660` (`uncapped_workloads`) keys on
`pod.owner.group_key()`; `src/rules.rs:44` discards a `Node` ownerReference, so a
static pod's owner is the pod itself and each one is its own key. `src/main.rs:385`
prints `snapshot.workloads.len()` under the same noun.

Baseline run, the two lines fifteen apart in one output:

```
$ ./target/release/k8rs --analysis tests/fixtures/*.json | sed -n '1p'
55 pods · 4 nodes · 16 workloads

$ ./target/release/k8rs --analysis tests/fixtures/*.json | grep "have no memory"
    34 workloads have no memory or CPU limit
```

The header's 16 is the controller count:

```
$ jq -s -r '[ .[] | if .kind=="List" then .items[] else . end ] | map(.kind) | group_by(.) | map({k:.[0],n:length}) | .[] | "\(.k)\t\(.n)"' tests/fixtures/*.json
CertificateSigningRequest	1
DaemonSet	3
Deployment	7
EndpointSlice	4
Node	4
PersistentVolumeClaim	2
PodDisruptionBudget	2
Pod	55
ReplicaSet	5
Service	4
StatefulSet	1
```

3 + 7 + 5 + 1 = 16.

The row's 34 is a count of distinct pod-owner keys. Over the same corpus there are
45 such keys, 40 of which are one individual pod:

```
$ jq -s -r '[ .[] | if .kind=="List" then .items[] else . end ] | map(select(.kind=="Pod"))
 | map(select((.status.phase//"") as $p | $p!="Succeeded" and $p!="Failed"))
 | map({key:((.metadata.ownerReferences//[])|map(select(.kind!="Node"))| if length>0 then (.[0].kind+"/"+.[0].name) else "BAREPOD" end), ns:(.metadata.namespace//"-"), name:.metadata.name})
 | map(if .key=="BAREPOD" then {kind:"bare/mirror pod", id:(.ns+"/"+.name)} else {kind:(.key|split("/")[0]), id:(.ns+"/"+.key)} end)
 | unique_by(.id) | group_by(.kind) | map({k:.[0].kind,n:length}) | .[] | "\(.k)\t\(.n)"' tests/fixtures/*.json
DaemonSet	2
ReplicaSet	3
bare/mirror pod	40
```

**The minimal case: a stock control plane, no user workload at all.**

```
$ ./target/release/k8rs --analysis tests/fixtures/nodes.json tests/fixtures/kube-system-pods.json \
    | sed -n '1p;/\[capacity\]/,/^$/p'
14 pods · 4 nodes · 0 workloads
[capacity]
  What each node promised, and what it has
    k8rs-control-plane   0.95 of 12 cpu · 290Mi of 23.1Gi
    k8rs-worker   0.1 of 12 cpu · 50Mi of 23.1Gi
    k8rs-worker2   0.1 of 12 cpu · 50Mi of 23.1Gi
    k8rs-worker3   0.1 of 12 cpu · 50Mi of 23.1Gi
  What each node is actually using is not shown. That number comes from metrics-server, and k8rs does not read it.
  Nothing to ask for — the numbers above are complete without it.
    6 workloads have no memory or CPU limit
      Nothing stops one taking a whole node.
```

`0 workloads` in the header and `6 workloads have no memory or CPU limit` in the
body. The six, read off the capture:

```
$ jq -r '.items[] | . as $p | ((.metadata.ownerReferences//[])|if length>0 then (.[0].kind+"/"+.[0].name) else "BARE" end) as $own | ([ (.spec.containers//[])[], (.spec.initContainers//[])[] ] | map(.resources.limits//{}) ) as $lim | "\(.metadata.name)\towner=\($own)\tlimits=\($lim)"' tests/fixtures/kube-system-pods.json
coredns-589f44dc88-hdrv5	owner=ReplicaSet/coredns-589f44dc88	limits=[{"memory":"170Mi"}]
coredns-589f44dc88-lbkj6	owner=ReplicaSet/coredns-589f44dc88	limits=[{"memory":"170Mi"}]
etcd-k8rs-control-plane	owner=Node/k8rs-control-plane	limits=[{}]
kindnet-bhzgd	owner=DaemonSet/kindnet	limits=[{"cpu":"100m","memory":"50Mi"}]
kindnet-h2st9	owner=DaemonSet/kindnet	limits=[{"cpu":"100m","memory":"50Mi"}]
kindnet-qwlg5	owner=DaemonSet/kindnet	limits=[{"cpu":"100m","memory":"50Mi"}]
kindnet-szmvh	owner=DaemonSet/kindnet	limits=[{"cpu":"100m","memory":"50Mi"}]
kube-apiserver-k8rs-control-plane	owner=Node/k8rs-control-plane	limits=[{}]
kube-controller-manager-k8rs-control-plane	owner=Node/k8rs-control-plane	limits=[{}]
kube-proxy-5d9xj	owner=DaemonSet/kube-proxy	limits=[{}]
kube-proxy-kvqmm	owner=DaemonSet/kube-proxy	limits=[{}]
kube-proxy-n8hgn	owner=DaemonSet/kube-proxy	limits=[{}]
kube-proxy-nx52c	owner=DaemonSet/kube-proxy	limits=[{}]
kube-scheduler-k8rs-control-plane	owner=Node/k8rs-control-plane	limits=[{}]
```

Four static control-plane pods (one key each), `kube-proxy` (one key), `coredns`
(one key, memory limit only). `kindnet` is capped. Each further control-plane node
adds four more keys.

The claim this contradicts is written into the test that pins the noun,
`src/analysis_tests/capacity.rs:890`: *"`workload` means a controller everywhere
else in this product"*.

Consequence measured on the same run: `src/analysis.rs:362-372` only reaches its
rule-8 *nothing to do* sentence when `uncapped == 0`, so on any kubeadm-shaped
cluster that sentence is unreachable.

**What it would take for this to be wrong:** a cluster whose static pods declare
limits, or a reading in which the header's noun and the row's noun are not the same
word to a reader.

---

## 3. `endpoints_behind` is a nested scan, and the cost is quadratic in Services

`src/analysis.rs:1778-1787` scans every EndpointSlice for every Service
(`src/analysis.rs:1748`). Measured against synthetic snapshots in the scratchpad —
200 nodes, 5000 pods, 50 budgets, and N Services each with one EndpointSlice
holding one endpoint. `cards-only` is the same binary on the same file without
`--analysis`, so the difference is the seven reports.

```
$ for n in 2500 10000; do
    a=$(s=$(date +%s%N); ./target/release/k8rs --analysis big-svc$n.json >/dev/null; e=$(date +%s%N); echo $(( (e-s)/1000000 )))
    b=$(s=$(date +%s%N); ./target/release/k8rs big-svc$n.json >/dev/null; e=$(date +%s%N); echo $(( (e-s)/1000000 )))
    echo "services=$n  with-reports=${a}ms  cards-only=${b}ms  reports=$((a-b))ms"
  done
services=2500  with-reports=147ms  cards-only=112ms  reports=35ms
services=10000  with-reports=1547ms  cards-only=192ms  reports=1355ms
```

And at 5000:

```
$ for i in 1 2 3; do ... ./target/release/k8rs --analysis big-svc.json ...
with --analysis: 390 ms
with --analysis: 367 ms
with --analysis: 403 ms
cards only:     139 ms
cards only:     154 ms
```

| Services / EndpointSlices | the seven reports |
|---|---|
| 0 | ~25 ms |
| 2 500 | 35 ms |
| 5 000 | ~230 ms |
| 10 000 | 1 355 ms |

4× the input, ~39× the cost.

**The other joins on the same shape are fine.** The same snapshot with no Services
and no slices:

```
$ ./target/release/k8rs --analysis big.json | head -1
5000 pods · 200 nodes · 0 workloads
$ ...three timed runs...
with --analysis: 121 ms / 103 ms / 98 ms
cards only:      80 ms /  77 ms /  81 ms
```

~25 ms for all seven reports at 200 nodes and 5000 pods, despite `pods_on`
(`src/rules.rs:6417`) being a full pod scan called once per node by
`src/analysis.rs:436` and again per node inside `node_overcommitted`
(`src/rules.rs:6753`).

Budgets scale acceptably too:

```
budgets=500   with-reports=128ms  cards-only=83ms  reports=45ms
budgets=2000  with-reports=215ms  cards-only=99ms  reports=116ms
```

**What it would take for this to be wrong:** a caller that computes the Waste
report once rather than per redraw, or a cluster where the Service count stays
small. `MOST_ROWS_PER_SECTION` (`src/analysis.rs:1699`) caps the rows drawn, not
the objects visited.

---

## 4. Posture: the one row an operator would act on sorts last, and the count folds it in

`src/analysis.rs:2022` sorts by pod count descending, then path. `src/analysis.rs:2027-2031`
opens the pane with *"Nothing here is broken. Network, storage and metrics agents
are supposed to do this."* `left_by_rule_8` (`src/analysis.rs:2204-2209`) sends
**any** read-only host mount here, from any namespace.

Baseline run, the last row of fourteen on the Posture pane:

```
  ○ /var/log
      Read-only, mounted by 1 pod in default.
```

Every other row on that pane says `in kube-system`.

**Measured with a synthetic pod** written to the scratchpad: one container in
namespace `default` on `k8rs-worker`, mounting `/etc/kubernetes/pki` read-only,
alongside the committed `nodes.json` and `kube-system-pods.json`.

```
$ ./target/release/k8rs --analysis probe-pki.json tests/fixtures/nodes.json tests/fixtures/kube-system-pods.json
...
1 critical, 1 warning
...
[posture]
  Pods that can read the node's own filesystem
  Nothing here is broken. Network, storage and metrics agents are supposed to do this — the list says who can, not what to go and fix.
...
  ○ /etc/kubernetes/pki
      Read-only, mounted by 3 pods in default and kube-system.
```

The tally is `1 critical, 1 warning` — the same two node cards the baseline draws
for this input. The pod produces **no Alerts card at all**, and its row is
indistinguishable from the two kube-apiserver mounts beside it. `Mounters`
(`src/analysis.rs:2056-2073`) carries one pod count and a namespace set, so *3
pods in default and kube-system* is the finest the sentence can be.

The same mechanism on a path that already had rows:

```
$ ./target/release/k8rs --analysis probe-pod.json tests/fixtures/nodes.json tests/fixtures/kube-system-pods.json
  ○ /lib/modules
      Read-only, mounted by 9 pods in default and kube-system.
```

(8 in the baseline, 9 with the probe pod; the row keeps its place at the top of
the pane because its count is the largest.)

**What it would take for this to be wrong:** a reading in which a read-only mount
from a user namespace carries no more weight than the same path mounted by
`kube-apiserver` — which is the line NOTES § D2 draws, and is why this is recorded
as a measurement of the pane's ordering rather than of rule 8's partition.

---

## 5. The `hostPath: {path: "."}` shape is not "in neither"

`src/analysis.rs:1970-1972` states that a path normalising to the empty string is
*"deliberately in neither"* screen. Measured: a writable `.` hostPath in a pod
outside `kube-system` does draw a rule 8 card, with the path missing from the
evidence line.

Synthetic pod in the scratchpad — namespace `default`, one container, two mounts:
`/lib/modules` read-only and a `hostPath` of `.` writable.

```
$ ./target/release/k8rs --analysis probe-pod.json tests/fixtures/nodes.json tests/fixtures/kube-system-pods.json
● default/probe
  A container can change files on the machine it runs on
  container app ·  on the node · writable
  → mount it read-only if the container only needs to read it
```

Note the two spaces between `·` and `on the node`: `src/rules.rs:5603` formats
`{path} on the node` with `path` empty. `src/analysis.rs:2166-2168` skips the same
mount, so it is on exactly one screen — Alerts — and not on neither.

The card itself is `rules.rs`'s (Phase 3, frozen). What is measured here is that
the doc claim in `analysis.rs` is false for the writable, non-node-agent case.

---

## 6. Three of the seven reports cannot draw their principal shape through the binary

`src/main.rs:219-221` and `:239` set `server_version`, `context`,
`client_certificate` and `metrics` to `None` structurally.

Consequences in the baseline run:

```
[versions]
  What version everything here is running
  Versions
  Not checked. Every answer on this pane is measured against the version the control plane is running, and k8rs could not read it.
  Check that the cluster's API server is answering — this is the one number it tells anyone who can reach it.

[certificates]
  What expires, soonest first
  Nothing here expires soon, and no machine is waiting to be let in.

[capacity]
  ...
  What each node is actually using is not shown. That number comes from metrics-server, and k8rs does not read it.
  Nothing to ask for — the numbers above are complete without it.
```

So the run that step 2 of the phase-close ritual read exercised: 1 of the six
shapes `versions` can produce; neither C1's row nor the sidebar badge — the pane's
only `Jump::Finding` (`src/analysis.rs:2869`) and the product's only duration badge
(`src/analysis.rs:2915`); and none of Capacity's `using …` paragraphs
(`src/analysis.rs:587`).

The corpus does carry a version string, in a file the glob does not reach:

```
$ cat tests/fixtures/K8S_VERSION
v1.36.1
```

`src/analysis_tests.rs:468-652` covers all of these states under `cargo test`.

---

## 7. `Info` and *no band* each carry two meanings across the family

Read off the producers:

| row | file:line | band |
|---|---|---|
| Drain — *needs one more flag for N pods* | `src/analysis.rs:1086` | `Info` |
| Posture — every row | `src/analysis.rs:2035` | `Info` |
| Restarts — every row | `src/analysis.rs:2420` | `Info` |
| Waste — finished pods, parked replicasets | `src/analysis.rs:1914`, `:1942` | `Info` |
| Drain — *is ready to drain* | `src/analysis.rs:1150` | `None` |
| Drain — *needs a moment before it can be checked* | `src/analysis.rs:1135` | `None` |
| Drain — *can't be checked until it is ready again* | `src/analysis.rs:995` | `None` |
| Capacity — a healthy node, and a node whose numbers could not be read | `src/analysis.rs:518` | `None` |

Posture and Restarts document `Info` as *the pane makes no judgement*
(`src/analysis.rs:2033-2034`, `:2418-2419`). Drain's `Info` row is a judgement: a
bare `kubectl drain --ignore-daemonsets` refuses on those pods
(`src/analysis.rs:1080-1083`).

*No band* covering both *fine* and *k8rs cannot answer* is D128's recorded refusal
of a per-row unknown marker (`src/analysis.rs:255-257`), and is recorded here as
checked rather than as a divergence.

---

## 8. The one unglossed sentence on the screen

`src/analysis.rs:758-761`:

```
  A drain below assumes --ignore-daemonsets, so DaemonSet pods never count as moving.
```

It is the first line of the Drain safety pane in the baseline run. The code matches
`screens/analysis.md:418-419`, which draws the sentence verbatim inside the mockup,
and `screens/analysis.md:503-516` argues for it.

Every neighbouring sentence in the same family glosses its jargon:
`src/analysis.rs:1302` — *"what Kubernetes calls an emptyDir volume"*;
`src/analysis.rs:1360` — *"started by hand, with no Deployment behind them"*;
`src/analysis.rs:552-554` — names metrics-server and says what it is for.

---

## What was checked and found sound

**The rule 8 / Posture partition, both ways.** `left_by_rule_8`
(`src/analysis.rs:2208`) is `path != "/" && !is_runtime_socket(&path) && (read_only
|| node_agent)`; rule 8 (`src/rules.rs:5576-5597`) fires on `path == "/"`,
`is_runtime_socket(path)`, or `!read_only && !node_agent`. Exact complement.
`analyze` gates the pod rules on `!finished(pod)` (`src/rules.rs:2418`) and
`host_paths` filters the same predicate (`src/analysis.rs:2147`).
`src/analysis_tests/posture.rs:72` asserts it over every captured mount in both
directions, asserts the corpus still carries more than 30 mounts, and names entries
on each side.

**Pods per node, Capacity against Drain safety.** Both go through `pods_on`
(`src/rules.rs:6417`); Drain narrows with `a_drain_would_move`
(`src/rules.rs:6444`). Every count in the baseline run reproduces by hand:

```
$ jq -s -r '...' tests/fixtures/*.json
-	moving=2	orphans=2	disk=0	mem=0
k8rs-control-plane	moving=2	orphans=0	disk=0	mem=0
k8rs-worker	moving=10	orphans=9	disk=2	mem=0
k8rs-worker2	moving=13	orphans=12	disk=2	mem=0
k8rs-worker3	moving=13	orphans=12	disk=3	mem=0
```

against the run's `9 pods here were started by hand` / `throws away files on 2
pods` (k8rs-worker), `12` / `2` (k8rs-worker2), `12` / `3` (k8rs-worker3),
`2 pods move` (k8rs-control-plane). N2's own card on the same run says *10 pods
here would still have to move* about k8rs-worker, the same `moving` figure.

`mem=0` on every node: the memory-emptyDir row (`src/analysis.rs:1072-1108`) is
reached by no capture in the corpus, only by the plants in
`src/analysis_tests/drain.rs:588`.

**Containers, Restarts against the Alerts rules.** Disjoint on the corpus:

```
$ ...cards section vs the restarts pane...
on Alerts: default/broken-config default/broken-crashloop default/broken-exit0 default/broken-hostpath default/broken-image default/broken-init default/broken-neverback default/broken-neverrules default/broken-notfound default/broken-oom default/broken-owned-7bdb7645c8-bwdfd default/broken-pending default/broken-probe0 default/broken-readiness default/broken-restarts10 default/broken-sigterm default/broken-socket default/broken-stuck default/broken-unjudged default/broken-wedged
on the restarts pane: default/broken-gang default/broken-reboot default/broken-restarts default/broken-restarts10serving
on both:
```

Both sides read `doing_its_job` and `RESTARTS_WARN` from `rules.rs` rather than
re-deriving them (`src/analysis.rs:2490-2493`).

**No producer returns an empty `Vec`.** Capacity always has a node row or the
section-off row; Drain always emits the flag `Prose`; Waste, Certificates, Posture
and Restarts each have an `is_empty()` fallback; Versions always emits the
`Versions` heading `Prose`. `src/analysis_tests.rs:681` asserts it over 27 states.

**Denominators.** Versions guards `N of M` on `unmeasured == 0`
(`src/analysis.rs:2679-2698`); Waste's numbers are list lengths; Restarts prints
two numbers and divides nowhere (`src/analysis.rs:2437-2443`); Capacity's `X of Y`
denominator is the node's own allocatable and `promised` refuses the whole
dimension on an unparseable quantity (`src/rules.rs:6824-6837`). The only bare
count whose noun collides with another producer's is § 2 above.

**The selector matcher against upstream.** `satisfies` (`src/analysis.rs:1588`)
returns `false` on an unknown operator, and `selects` returns `false` for a `null`
selector. `pkg/registry/core/pod/storage/eviction.go`:

```go
selector, err := metav1.LabelSelectorAsSelector(pdb.Spec.Selector)
if err != nil {
	// This object has an invalid selector, it does not match the pod
	continue
}
```

Same answer at the API server.

**N1 is the only `Critical` node finding**, which is what makes `not_ready`'s
three-field identity (`src/analysis.rs:1209-1213`) enough:

```
$ for l in 6540 6601 6654 6716 6809; do ... done
=== around 6540 === node_stopped_being_ready ... Severity::Critical
=== around 6601 === cordoned_with_work_left_on_it ... Severity::Warn
=== around 6654 === node_running_low ... Severity::Warn
=== around 6716 === ... Severity::Info
=== around 6809 === ... Severity::Info
```

**C1's row and C1's badge cannot disagree.** Both go through `c1(findings)`
(`src/analysis.rs:2875`) and `band` (`src/analysis.rs:2885`), and the badge's extra
reads — `client_certificate` and `expires_at` — are the same two `?`s C1 itself
takes (`src/rules.rs:7667-7668`), so the badge is present whenever the row is.

**The certificates pane's silence about the pending CSR is correct.**
`tests/fixtures/csr-pending.json` carries the `…client` signer, not
`…client-kubelet`, and `src/analysis.rs:2973` filters on the kubelet signer.
