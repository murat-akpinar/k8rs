# Family B — `restartPolicyRules` at rule 15 and `status.terminatingReplicas` (2026-08-21)

Measurements taken for the operator review of Phase 4 Family B, on the dev
machine, against the working tree at `git status` = `M NOTES.md backlog.md
src/rules.rs src/rules_tests*.rs todo.md`.

**No cluster was created.** The PM's fixture cluster was up (`kind get clusters`
→ `k8rs`, four nodes, `Up 18 hours`), and one cluster at a time is the rule, so
no `K8RS_CLUSTER=review` cluster was started beside it. The API-validation rows
in § 1 were taken against the running cluster with `kubectl explain` and
`--dry-run=server`, which persist nothing; `kubectl get pods -A -o name | wc -l`
read 53 before and after and no object named `k8s-admin-*` exists.

## 1 — What the API server accepts, v1.36.1

```
$ kubectl version
Client Version: v1.36.3
Server Version: v1.36.1
```

`kubectl explain pod.spec.containers.restartPolicyRules`, verbatim from the
DESCRIPTION:

```
Represents a list of rules to be checked to determine if the container should be
restarted on exit. The rules are evaluated in order. Once a rule matches a
container exit condition, the remaining rules are ignored. If no rule matches the
container exit condition, the Container-level restart policy determines the
whether the container is restarted or not. Constraints on the rules: - At most 20
rules are allowed. - Rules can have the same action. - Identical rules are not
forbidden in validations. When rules are specified, container MUST set
RestartPolicy explicitly even it if matches the Pod's RestartPolicy.
```

`action` field description at this pin: *"The only possible value is "Restart" to
restart the container."* — which the validator below contradicts.

Server dry-run over one regular container under pod `restartPolicy: Never`,
container `restartPolicy: Never`, one rule each:

```
$ kubectl apply --dry-run=server -f d.yaml     # one row per rules value
[{action: Restart}]                                => spec.containers[0].restartPolicyRules[0].exitCodes: Required value: must be specified
[{action: Restart, exitCodes:{operator: In, values: []}}]      => pod/k8s-admin-dryrun created (server dry run)
[{action: Restart, exitCodes:{operator: NotIn, values:[3]}}]   => pod/k8s-admin-dryrun created (server dry run)
[{action: DoNotRestart, exitCodes:{operator: In, values:[3]}}] => spec.containers[0].restartPolicyRules[0].action: Unsupported value: "DoNotRestart": supported values: "Restart", "RestartAllContainers"
[{action: Restart, exitCodes:{operator: Equals, values:[3]}}]  => spec.containers[0].restartPolicyRules[0].exitCodes.operator: Unsupported value: "Equals": supported values: "In", "NotIn"
[{action: RestartAllContainers, exitCodes:{operator: NotIn, values:[0]}}] => pod/k8s-admin-dryrun created (server dry run)
```

Three-container pod, `RestartAllContainers` on a **regular** container:

```
$ kubectl apply --dry-run=server -f dryrun-gang.yaml
pod/k8s-admin-dryrun-gang created (server dry run)
```

Init containers:

```
sidecar (restartPolicy: Always) + RestartAllContainers rule => created (server dry run)
init    (restartPolicy: Never)  + RestartAllContainers rule => created (server dry run)
init    with rules and no restartPolicy                     => spec.initContainers[0].restartPolicy: Required value: must specify restartPolicy when restart rules are used
```

## 2 — Where the replica counters come from

`k8s-openapi 0.28.0`, the generated doc line for `DeploymentStatus::ready_replicas`,
one feature per row:

```
v1_32  readyReplicas is the number of pods targeted by this Deployment with a Ready Condition.
v1_33  Total number of non-terminating pods targeted by this Deployment with a Ready Condition.
v1_34  (same as v1_33)
v1_35  (same as v1_33)
v1_36  (same as v1_33)
```

`terminating_replicas` in the same file:

```
v1_33  This is an alpha field. Enable DeploymentReplicaSetTerminatingReplicas to be able to use this field.
v1_34  This is an alpha field. Enable DeploymentReplicaSetTerminatingReplicas to be able to use this field.
v1_35  This is a beta field and requires enabling DeploymentReplicaSetTerminatingReplicas feature (enabled by default).
v1_36  This is a beta field and requires enabling DeploymentReplicaSetTerminatingReplicas feature (enabled by default).
```

`v1_32`'s `replicas` and `updated_replicas` already read *"Total number of
**non-terminated** pods targeted by this deployment"*.

Upstream at **release-1.32**, `pkg/controller/controller_utils.go`:

```go
func IsPodActive(p *v1.Pod) bool {
	return v1.PodSucceeded != p.Status.Phase &&
		v1.PodFailed != p.Status.Phase &&
		p.DeletionTimestamp == nil
}
```

`pkg/controller/replicaset/replica_set.go`, same release, in `syncReplicaSet`:

```go
filteredPods := controller.FilterActivePods(logger, allPods)
...
newStatus := calculateStatus(rs, filteredPods, manageReplicasErr)
```

and `calculateStatus` counts `Replicas = len(filteredPods)`, `ReadyReplicas`
over `podutil.IsPodReady(pod)` of those same pods.

Upstream at **release-1.36**, `pkg/controller/statefulset/stateful_set_utils.go`:

```go
func isRunningAndReady(pod *v1.Pod) bool {
	return pod.Status.Phase == v1.PodRunning && podutil.IsPodReady(pod)
}
func isTerminating(pod *v1.Pod) bool { return pod.DeletionTimestamp != nil }
```

## 3 — The binary over the captures, and over three scratch probes

Built at the working tree: `cargo build --release`. The probes are derived with
`jq` from committed captures into the scratchpad; **none of them is committed and
none is in `tests/`**.

Committed, unchanged:

```
$ ./target/release/k8rs tests/fixtures/neverback.json
● default/broken-neverback · 17 hours ago
  This container has stopped and nothing is starting it again
  container broke · exit 1 (the application's own error) · ran for under a second
1 critical

$ ./target/release/k8rs tests/fixtures/gang.json
(no card)

$ ./target/release/k8rs tests/fixtures/neverrules.json
▲ default/broken-neverrules · 17 hours ago
  The last run on record failed — exit 1 (the application's own error)
1 warning
```

### Probe A — a sibling that has already stopped

```
$ jq '(.spec.containers[]|select(.name=="done")) |= (.restartPolicy="Never" |
   .restartPolicyRules=[{"action":"RestartAllContainers","exitCodes":{"operator":"In","values":[3]}}])' \
   tests/fixtures/neverback.json > probe-dead-gang-sibling.json
$ ./target/release/k8rs probe-dead-gang-sibling.json
1 pod · 0 nodes · 0 workloads

○ nothing is broken
```

Field values the probe turns on, all off the committed capture except the one
line above: pod `restartPolicy: Never`, `phase: Running`; `broke` —
`state.terminated exit 1 reason Error`, `restartCount 0`, no `lastState`;
`done` — `state.terminated exit 0 reason Completed`, `restartCount 0`;
`keeper` — `state.running`.

### Probe B — a sibling rule that matches no exit code

Same capture, same edit, `values: []` instead of `[3]` (§ 1 shows the API server
accepts that shape):

```
$ ./target/release/k8rs probe-empty-values-sibling.json
1 pod · 0 nodes · 0 workloads

○ nothing is broken
```

### Probe C — `neverrules/retry` sitting in the exit its own rule names

```
$ jq '(.status.containerStatuses[]|select(.name=="retry")) |= (.state = .lastState | .restartCount = 1)' \
   tests/fixtures/neverrules.json > probe-covered-exit-midflight.json
retry: restarts=1 state=terminated exit=3 last=3
$ ./target/release/k8rs probe-covered-exit-midflight.json
▲ default/broken-neverrules · 17 hours ago
  The last run on record failed — exit 3
  container retry · ran for under a second
  → read the last run's log — it holds the last thing written before that run ended, from the
    program or from the shell that started it. The command below is what fetches it
1 warning
```

`retry`'s declaration in the capture: `restartPolicy: Never`,
`restartPolicyRules: [{action: Restart, exitCodes: {operator: In, values: [3]}}]`.

## 4 — The evidence line against the three-line cut

Rendered from the committed captures:

```
$ ./target/release/k8rs tests/fixtures/quota-replicasets.json tests/fixtures/quota-deployment.json
  0 of 1 pod ready · the reason Kubernetes gave: pods "broken-quota-59654c756-wzr9s" is forbidden:
  exceeded quota: deny-all-pods, requested: pods=1, used: pods=0, limited: pods=0     (W1)

$ ./target/release/k8rs tests/fixtures/deployments.json
  0 of 1 pod ready · the reason Kubernetes gave: ReplicaSet "broken-quota-59654c756" has timed
  out progressing.                                                                    (W2)
```

Lengths and the cut, `screens/alerts.md` § The height (evidence capped at 3
lines):

```
W1 evidence 176 chars; at 51 columns 3 lines hold ~152 → cut inside
   "…requested: pods=1, used:"; a 22-char clause appended after it is not drawn.
W2 evidence 110 chars; + "· 1 pod on the way out" = 134 → fits.
```

## 5 — Test suite at the working tree

```
$ cargo test --release
test result: ok. 405 passed; 0 failed; 0 ignored (unit)
test result: ok. 7 passed; 0 failed; 0 ignored (tests/binary.rs)
```

---

# Round 2 — the same tree after the fix (2026-08-21, later)

Same machine, same conditions: `kind get clusters` → `k8rs` only, no review
cluster started, dry-runs against the running cluster persist nothing.

## 6 — The value bounds the API server does *not* enforce

Continuing § 1's matrix, one regular container, `restartPolicy: Never`:

```
$ kubectl apply --dry-run=server -f d.yaml
[{action: Restart, exitCodes: {operator: In,    values: [-1]}}]  => pod/k8s-admin-dryrun2 created (server dry run)
[{action: Restart, exitCodes: {operator: In,    values: [255]}}] => pod/k8s-admin-dryrun2 created (server dry run)
[{action: Restart, exitCodes: {operator: In,    values: [256]}}] => pod/k8s-admin-dryrun2 created (server dry run)
[{action: Restart, exitCodes: {operator: In,    values: [300]}}] => pod/k8s-admin-dryrun2 created (server dry run)
[{action: Restart, exitCodes: {operator: NotIn, values: []}}]    => pod/k8s-admin-dryrun2 created (server dry run)
```

`kubectl explain` for the same field: *"Specifies the set of values to check for
container exit codes. At most 255 elements are allowed."* — a cardinality cap,
not a range.

## 7 — `unavailableReplicas`, from upstream at release-1.36

`pkg/controller/deployment/sync.go`:

```go
func calculateStatus(allRSs []*apps.ReplicaSet, newRS *apps.ReplicaSet, deployment *apps.Deployment) apps.DeploymentStatus {
	availableReplicas := deploymentutil.GetAvailableReplicaCountForReplicaSets(allRSs)
	totalReplicas := deploymentutil.GetReplicaCountForReplicaSets(allRSs)
	unavailableReplicas := totalReplicas - availableReplicas
	if unavailableReplicas < 0 { unavailableReplicas = 0 }
```

`pkg/controller/deployment/util/deployment_util.go`:

```go
func GetReplicaCountForReplicaSets(replicaSets []*apps.ReplicaSet) int32 {
	totalReplicas := int32(0)
	for _, rs := range replicaSets {
		if rs != nil { totalReplicas += *(rs.Spec.Replicas) }
	}
	return totalReplicas
}
func GetAvailableReplicaCountForReplicaSets(replicaSets []*apps.ReplicaSet) int32 {
	totalAvailableReplicas := int32(0)
	for _, rs := range replicaSets {
		if rs != nil { totalAvailableReplicas += rs.Status.AvailableReplicas }
	}
	return totalAvailableReplicas
}
```

The minuend is `rs.Spec.Replicas`; the subtrahend is `rs.Status.AvailableReplicas`.

## 8 — The binary after the fix

Rebuilt at the working tree. Same three probes as § 3, unchanged bytes:

```
$ ./target/release/k8rs probe-dead-gang-sibling.json          # § 3 probe A
  container broke · exit 1 (the application's own error) · ran for under a second
1 critical

$ ./target/release/k8rs probe-empty-values-sibling.json       # § 3 probe B
  container broke · exit 1 (the application's own error) · ran for under a second
1 critical

$ ./target/release/k8rs probe-covered-exit-midflight.json     # § 3 probe C
▲ default/broken-neverrules · 18 hours ago
  The last run on record failed — exit 3
1 warning
```

### Probe D — the gang-rule sibling is `Waiting`, not `Terminated`

`neverback.json` with `keeper` given the same rule as probe A and put into
`state.waiting {reason: ImagePullBackOff}`:

```
$ jq -r '.status.containerStatuses[]|"\(.name): state=\(.state|keys[0]) reason=\(.state.waiting.reason // "-")"' probe-waiting-gang-sibling.json
broke:  state=terminated reason=-
done:   state=terminated reason=-
keeper: state=waiting     reason=ImagePullBackOff

$ ./target/release/k8rs probe-waiting-gang-sibling.json
1 pod · 0 nodes · 0 workloads

● default/broken-neverback
  Container image is not usable, so the container never started (ImagePullBackOff)
  container keeper · image docker.io/library/busybox:latest · Back-off pulling image
  → check the image name and tag, whether this namespace has a pull secret for that registry, and whether the pull policy lets the node fetch it at all

1 critical
```

`broke` draws no card here; the committed capture draws one for it.

### The readiness fact as drawn

`deployments.json` with `broken-quota`'s `status.terminatingReplicas` planted,
`spec.replicas` is 1 on that object:

```
$ ./target/release/k8rs w2-1.json
  0 of 1 pod ready, 1 shutting down · the reason Kubernetes gave: ReplicaSet "broken-quota-59654c756" has timed out progressing.

$ ./target/release/k8rs w2-3.json
  0 of 1 pod ready, 3 shutting down · the reason Kubernetes gave: ReplicaSet "broken-quota-59654c756" has timed out progressing.
```

## 9 — Gates at the round-2 tree

```
$ cargo test --release
test result: ok. 405 passed; 0 failed; 0 ignored (unit)
test result: ok. 7 passed; 0 failed; 0 ignored (tests/binary.rs)
$ python3 scripts/check-docs.py
OK — all relative links resolve and every decision is indexed
$ python3 scripts/screens-check.py
screens-check: 100 mockups fit 80x24 — OK
```

## 10 — A file that moved during the review

`screens/alerts.md` was read twice. At the first read it carried
`### A third fact, appended last: pods already on their way out`, naming
`on_the_way_out` four times and citing
`a_workload_draining_pods_says_so_beside_the_count_that_stopped_seeing_them`.
Both were gone at the second read:

```
$ date
Cum 21 Ağu 2026 20:20:43 +03
$ ls -l --time-style=full-iso screens/alerts.md src/rules.rs NOTES.md
2026-08-21 20:19:55  screens/alerts.md
2026-08-21 20:13:48  NOTES.md
2026-08-21 20:03:25  src/rules.rs
$ grep -rn "on_the_way_out" screens/ docs/ NOTES.md src/
screens/alerts.md:1152:`on_the_way_out` no longer exists
$ grep -n "beside_the_count" src/rules_tests/workload.rs
(no match; the test is now …_inside_the_count_that_stopped_seeing_them)
```
