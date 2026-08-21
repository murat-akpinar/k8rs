> **Superseded corpus, and a later reader will grep for exactly the wrong line.**
> Sections 1–12 measure the **third** capture trip. The review they fed changed the
> tree: `broken-overhead` was pinned to `k8rs-worker`, `DisruptionBudgetSnapshot`
> grew `generation` / `observed_generation` / `conditions`, and the trip was re-run
> a **fourth** time. So `overhead: NOW=k8rs-worker2` in § 2 is true of what was
> measured and **false of the tree now** — it is `k8rs-worker`. Section 13 measures
> the fourth trip; where the two disagree, section 13 is the current one.

# 2026-08-21 — the Family C corpus read as a cluster: drain, capacity, and what the break sequence deletes

Operator review (step 6) of the uncommitted 2026-08-20/21 capture trip. Every
command below was run on the dev machine against the **PM's** fixture cluster
`kind-k8rs`, which was up and broken at the time, and against the working tree.
Nothing was written to the cluster; no second cluster was created.

Under [`reports/README.md`](README.md)'s rule: field values and counts only, no
object dumps, no uids, no addresses.

---

## 1. Where the two new join objects landed, and what is on those nodes now

```
$ kubectl --context kind-k8rs get pods -l app=healthy-deploy \
    -o custom-columns='NAME:.metadata.name,PHASE:.status.phase,NODE:.spec.nodeName,READY:.status.conditions[?(@.type=="Ready")].status'
NAME                              PHASE     NODE           READY
healthy-deploy-7f84bdfb9b-8qxh5   Pending   <none>         <none>
healthy-deploy-7f84bdfb9b-lljgg   Running   k8rs-worker3   False
healthy-deploy-7f84bdfb9b-mhnw8   Pending   <none>         <none>
```

The committed fixture, same selector, taken at 21:02:

```
$ jq -c '.items[] | {name:.metadata.name, node:.spec.nodeName, phase:.status.phase}' \
    tests/fixtures/healthy-deploy-pods.json
{"name":"healthy-deploy-7f84bdfb9b-lljgg","node":"k8rs-worker3","phase":"Running"}
{"name":"healthy-deploy-7f84bdfb9b-rq6bn","node":"k8rs-worker2","phase":"Running"}
```

Their tolerations, from the same fixture — the two the
`DefaultTolerationSeconds` plugin adds and nothing else:

```
$ jq -c '[.items[0].spec.tolerations[] | {key,effect,tolerationSeconds}]' \
    tests/fixtures/healthy-deploy-pods.json
[{"key":"node.kubernetes.io/not-ready","effect":"NoExecute","tolerationSeconds":300},
 {"key":"node.kubernetes.io/unreachable","effect":"NoExecute","tolerationSeconds":300}]
```

`nodes.json`, captured after `break-nodes` in the same trip:

```
$ jq -c '.items[] | {name:.metadata.name, unschedulable:.spec.unschedulable,
                     taints:[.spec.taints[]?|{key,value,effect}],
                     ready:[.status.conditions[]|select(.type=="Ready")|.status][0]}' \
    tests/fixtures/nodes.json
{"name":"k8rs-control-plane","unschedulable":null,"taints":[{"key":"node-role.kubernetes.io/control-plane","value":null,"effect":"NoSchedule"}],"ready":"True"}
{"name":"k8rs-worker","unschedulable":true,"taints":[{"key":"node.kubernetes.io/unschedulable","value":null,"effect":"NoSchedule"}],"ready":"True"}
{"name":"k8rs-worker2","unschedulable":null,"taints":[{"key":"dedicated","value":"gpu","effect":"NoExecute"}],"ready":"True"}
{"name":"k8rs-worker3","unschedulable":null,"taints":[{"key":"node.kubernetes.io/unreachable","value":null,"effect":"NoSchedule"},{"key":"node.kubernetes.io/unreachable","value":null,"effect":"NoExecute"}],"ready":"Unknown"}
```

The live budget counters, hours after the capture:

```
$ kubectl --context kind-k8rs get pdb -A \
    -o custom-columns='NAME:.metadata.name,MIN:.spec.minAvailable,ALLOWED:.status.disruptionsAllowed,CURRENT:.status.currentHealthy,DESIRED:.status.desiredHealthy,EXPECTED:.status.expectedPods'
NAME               MIN   ALLOWED   CURRENT   DESIRED   EXPECTED
broken-pdb-floor   2     0         0         2         3
healthy-pdb-room   1     0         1         1         5
```

The committed fixture for the same two objects:

```
$ jq -c '.items[] | {name:.metadata.name, minAvailable:.spec.minAvailable,
                     allowed:.status.disruptionsAllowed, current:.status.currentHealthy,
                     desired:.status.desiredHealthy, expected:.status.expectedPods,
                     reason:[.status.conditions[]|select(.type=="DisruptionAllowed")|.reason][0]}' \
    tests/fixtures/poddisruptionbudgets.json
{"name":"broken-pdb-floor","minAvailable":2,"allowed":0,"current":2,"desired":2,"expected":2,"reason":"InsufficientPods"}
{"name":"healthy-pdb-room","minAvailable":1,"allowed":1,"current":2,"desired":1,"expected":3,"reason":"SufficientPods"}
```

## 2. How many committed pod captures the trip's own break sequence deletes

Each single-object `Pod` fixture looked up by name on the live cluster:

```
$ for f in tests/fixtures/*.json; do n=$(jq -r 'select(.kind=="Pod")|.metadata.name' "$f"); \
    [ -z "$n" ] && continue; node=$(jq -r '.spec.nodeName // "-"' "$f"); \
    kubectl --context kind-k8rs get pod "$n" -o name >/dev/null 2>&1 \
      || echo "gone: $n (fixture node=$node)"; done
gone: broken-config(k8rs-worker2)          gone: broken-exit0(k8rs-worker2)
gone: broken-failed(k8rs-worker3)          gone: healthy-podlevel(k8rs-worker2)
gone: healthy-sidecar(k8rs-worker2)        gone: broken-image(k8rs-worker2)
gone: broken-neverback(k8rs-worker2)       gone: broken-overhead(k8rs-worker2)
gone: broken-podlimit(k8rs-worker2)        gone: broken-restarts10serving(k8rs-worker2)
gone: broken-restarts(k8rs-worker2)        gone: broken-succeeded(k8rs-worker2)

present=26  gone=12
```

Placement is not pinned and moves between trips:

```
$ for f in overhead restarts config exit0 image healthy-podlevel healthy-sidecar; do \
    echo "$f: HEAD=$(git show HEAD:tests/fixtures/$f.json 2>/dev/null | jq -r '.spec.nodeName // "-"') NOW=$(jq -r '.spec.nodeName // "-"' tests/fixtures/$f.json)"; done
overhead:         HEAD=(absent)      NOW=k8rs-worker2
restarts:         HEAD=k8rs-worker2  NOW=k8rs-worker2
config:           HEAD=k8rs-worker   NOW=k8rs-worker2
exit0:            HEAD=k8rs-worker2  NOW=k8rs-worker2
image:            HEAD=k8rs-worker2  NOW=k8rs-worker2
healthy-podlevel: HEAD=k8rs-worker2  NOW=k8rs-worker2
healthy-sidecar:  HEAD=k8rs-worker2  NOW=k8rs-worker2
```

## 3. What `kubectl drain` actually does on this cluster

Client-side dry run, no writes:

```
$ kubectl --context kind-k8rs drain k8rs-worker --dry-run=client --ignore-daemonsets --delete-emptydir-data
node/k8rs-worker already cordoned (dry run)
error: unable to drain node "k8rs-worker" due to error: cannot delete Pods that
declare no controller (use --force to override): default/broken-crashloop,
default/broken-hostpath, default/broken-nolimits, default/broken-notfound,
default/broken-probe0, default/broken-reboot, default/broken-restarts10,
default/broken-sigterm, default/broken-wedged, continuing command...

$ kubectl --context kind-k8rs drain k8rs-worker3 --dry-run=client --ignore-daemonsets --delete-emptydir-data
node/k8rs-worker3 cordoned (dry run)
error: unable to drain node "k8rs-worker3" due to error: cannot delete Pods that
declare no controller (use --force to override): default/broken-init,
default/broken-neverrules, default/broken-oom, default/broken-oomserving,
default/broken-readiness, default/broken-resize, default/broken-socket,
default/broken-startup, default/healthy, default/healthy-disk,
default/healthy-hostpath, default/healthy-retry, default/healthy-unreadysidecar,
continuing command...
```

## 4. Per-node CPU request sums over the whole committed corpus

Every fixture that holds a `Pod` or a pod `List`, Running or Pending, pod-level
request preferred over the container sum, native sidecars added, plain init
containers excluded — the shape `rules.rs` § `charged` uses — beside the same
sum with `spec.overhead` added:

```
$ jq -n -f /tmp/.../sum.jq tests/fixtures/*.json
[{"node":"(unscheduled)",     "spec_only":0,   "with_overhead":0,   "pods":2},
 {"node":"k8rs-control-plane","spec_only":950, "with_overhead":950, "pods":8},
 {"node":"k8rs-worker",       "spec_only":100, "with_overhead":100, "pods":12},
 {"node":"k8rs-worker2",      "spec_only":340, "with_overhead":590, "pods":14},
 {"node":"k8rs-worker3",      "spec_only":170, "with_overhead":170, "pods":17}]
```

Whole-corpus totals: **spec-only 1560m · with overhead 1810m**.

The `kube-system` half of that, per node:

```
$ jq -r '... select(.status.phase=="Running") ... .spec.containers[].resources.requests.cpu ...' \
    tests/fixtures/kube-system-pods.json
k8rs-control-plane  coredns ×2                 100m each
k8rs-control-plane  etcd                       100m
k8rs-control-plane  kube-apiserver             250m
k8rs-control-plane  kube-controller-manager    200m
k8rs-control-plane  kube-scheduler             100m
k8rs-control-plane  kindnet                    100m
k8rs-worker         kindnet                    100m
k8rs-worker2        kindnet                    100m
k8rs-worker3        kindnet                    100m
TOTAL kube-system cpu requests: 1250m
```

The overhead object itself:

```
$ jq -c '{rc:.spec.runtimeClassName, overhead:.spec.overhead, node:.spec.nodeName,
          req:.spec.containers[0].resources.requests}' tests/fixtures/overhead.json
{"rc":"broken-overhead","overhead":{"cpu":"250m","memory":"120Mi"},
 "node":"k8rs-worker2","req":{"cpu":"100m","memory":"64Mi"}}
```

Node allocatable, all four:

```
$ jq -r '.items[] | "\(.metadata.name)\tcpu=\(.status.allocatable.cpu)\tmem=\(.status.allocatable.memory)"' tests/fixtures/nodes.json
k8rs-control-plane  cpu=12  mem=24277416Ki
k8rs-worker         cpu=12  mem=24277416Ki
k8rs-worker2        cpu=12  mem=24277416Ki
k8rs-worker3        cpu=12  mem=24277416Ki

$ nproc
12
```

## 5. The Capacity report's usage column, on this cluster

```
$ kubectl --context kind-k8rs top nodes
error: Metrics API not available
```

## 6. Every command-log line the analysis screens draw, run as typed

```
$ kubectl --context kind-k8rs get pdb -A                 → exit 0
$ kubectl --context kind-k8rs get svc,endpointslices -A  → exit 0
$ kubectl --context kind-k8rs get pvc,replicasets -A     → exit 0
$ kubectl --context kind-k8rs get csr                    → exit 0
$ kubectl --context kind-k8rs top nodes                  → exit 1, "Metrics API not available"
```

## 7. Fixture shape drift between HEAD and this tree

Pod phase, container state key, restart count and readiness compared for every
changed fixture; only three moved:

```
== init      HEAD ics migrate terminated r=10   NOW ics migrate waiting r=9
== notfound  HEAD cs  app     terminated r=10   NOW cs  app     waiting r=9
== oom       HEAD cs  hog     terminated r=10   NOW cs  hog     waiting r=9
```

Corpus timestamp spread (every pod capture, both namespaces):

```
$ jq -r '... .metadata.creationTimestamp' tests/fixtures/*.json | sort | sed -n '1p;$p'
2026-08-20T21:02:04Z
2026-08-20T21:02:36Z
```

## 8. Guards and tests actually run here

```
$ bash scripts/fixture-audit.sh
fixture-audit: 59 committed fixtures (55 parsed as JSON) — no annotations, no env
values, no addresses; no key material in any framing (armoured, base64-wrapped,
DER, mislabeled); node names intact; scripts/sanitize.jq leaves every one of them
byte-identical; and the k8s-openapi pin is not below the cluster they came from

$ cargo test --locked -- --nocapture the_pods_the_blocking_budget the_bound_claim \
    the_service_that_reaches the_runtime_class_charged the_blocking_disruption what_family_cs_inputs
budgets: broken-pdb-floor allows Some(0) (healthy Some(2)/Some(2)) for {"app": "healthy-deploy"} · healthy-pdb-room allows Some(1) (healthy Some(2)/Some(1)) for {"app": "broken-rollout"}
slices: broken-noendpoints-cxr9c -> Some("broken-noendpoints") (0 endpoints) · broken-sts-x2wtg -> Some("broken-sts") (2 endpoints) · kubernetes -> Some("kubernetes") (1 endpoints) · kube-dns-mj97d -> Some("kube-dns") (2 endpoints)
fetched: [("replica_sets", 1), ("services", 4), ("endpoint_slices", 4), ("claims", 2), ("disruption_budgets", 2)] · certificate_requests: not fetched
claims ["broken-unused-disk Some(\"Bound\") Some(\"128Mi\")", "healthy-disk Some(\"Bound\") Some(\"64Mi\")"] · mounted by ["healthy-disk:[\"healthy-disk\"]"] · unused ["broken-unused-disk"]
broken-overhead: overhead Some("250m")/Some("120Mi") on top of container Some("100m")/Some("64Mi")
55 pods · broken-pdb-floor {"app": "healthy-deploy"} -> ["healthy-deploy-7f84bdfb9b-lljgg", "healthy-deploy-7f84bdfb9b-rq6bn"] · healthy-pdb-room {"app": "broken-rollout"} -> []
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 285 filtered out
```

## 9. Fields the new snapshot types do not carry

Read off `src/rules.rs` § SNAPSHOT TYPES at this tree:

```
DisruptionBudgetSnapshot { id, selector, disruptions_allowed, current_healthy, desired_healthy }
```

Not carried, and present on the committed capture:
`metadata.generation`, `status.observedGeneration` (both `1` on both budgets),
`status.conditions[DisruptionAllowed].reason` (`InsufficientPods` /
`SufficientPods`), `status.expectedPods`, `status.disruptedPods`.

Committed EndpointSlices: four slices, four Services, **one slice per Service**.
No captured Service has more than one slice, and no captured slice carries
`conditions.terminating: true`.

## 10. Who reads `state.waiting.message`

```
$ grep -n "waiting" src/rules.rs | grep -i "message\|fn waiting"
2474:fn waiting(c: &ContainerSnapshot) -> Option<(&str, Option<&str>)> {
3993:    let (reason, message) = waiting(c)?;      # image_not_pulled          (rule 3)
4020:    let (reason, message) = waiting(c)?;      # container_config_missing  (rule 4)
5328:    let (reason, message) = waiting(c)?;      # stuck_at_the_starting_line (rule 13)

$ grep -rn 'back-off\|back_off' src/rules.rs src/rules_tests/pod.rs
src/rules.rs:2398:/// ... In the settled `CrashLoopBackOff` back-off the flag      (a doc comment)

$ jq -r '.status.containerStatuses[0].state.waiting.message' tests/fixtures/crashloop.json
back-off 5m0s restarting failed container=quitter pod=broken-crashloop_default(<uid>)
```

## 11. The read-only Role against the three new kinds

`docs/security.md` § `k8rs-readonly`, read at this tree:

```
  - apiGroups: [""]                  resources: [... "persistentvolumeclaims" ...]  verbs: [get, list, watch]
  - apiGroups: ["policy"]            resources: ["poddisruptionbudgets"]            verbs: [get, list, watch]
  - apiGroups: ["discovery.k8s.io"]  resources: ["endpointslices"]                  verbs: [get, list, watch]
```

`node.k8s.io/runtimeclasses` is absent and is not needed: `spec.overhead` and
`spec.runtimeClassName` are both fields of the Pod.

## 12. `src/rules.rs` at this tree

```
$ git status --short src/
 M src/rules_tests.rs
 M src/rules_tests/certificate.rs
 M src/rules_tests/node.rs
 M src/rules_tests/pod.rs
 M src/rules_tests/snapshot.rs
```

---

# 13. Round 2 — the fourth trip, measured against the same cluster

Everything below is the tree after the two blocker fixes and the fourth capture.
Same rules: the cluster was read, never written; no second cluster.

## 13.1 Node states, and where the pinned pod landed

```
$ jq -c '.items[] | {name:.metadata.name, unschedulable:.spec.unschedulable,
                     taints:[.spec.taints[]?|{key,value,effect}],
                     ready:[.status.conditions[]|select(.type=="Ready")|.status][0]}' tests/fixtures/nodes.json
k8rs-control-plane  unschedulable=null  NoSchedule node-role.kubernetes.io/control-plane   ready=True
k8rs-worker         unschedulable=true  NoSchedule node.kubernetes.io/unschedulable        ready=True
k8rs-worker2        unschedulable=null  NoExecute  dedicated=gpu                           ready=True
k8rs-worker3        unschedulable=null  NoSchedule + NoExecute node.kubernetes.io/unreachable  ready=Unknown

$ jq -c '{node:.spec.nodeName, oh:.spec.overhead, req:.spec.containers[0].resources.requests}' tests/fixtures/overhead.json
{"node":"k8rs-worker","oh":{"cpu":"250m","memory":"120Mi"},"req":{"cpu":"100m","memory":"64Mi"}}
```

## 13.2 Per-node CPU requests: the corpus against the cluster it came from

Corpus (same jq as § 4):

```
control-plane 950m/950m (8 pods) · worker 200m/450m (12) · worker2 190m/190m (16) · worker3 220m/220m (15) · unscheduled 0m (2)
```

Live, same node, Running+Pending, pod-level request preferred:

```
$ for n in k8rs-worker k8rs-worker2 k8rs-worker3; do kubectl --context kind-k8rs get pods -A \
    --field-selector=spec.nodeName=$n -o json | jq '... sum of cpu requests ...'; done
k8rs-worker   200m
k8rs-worker2  100m
k8rs-worker3  220m

$ kubectl --context kind-k8rs get pods -A --field-selector=status.phase!=Succeeded \
    -o custom-columns='NODE:.spec.nodeName' --no-headers | sort | uniq -c
      9 k8rs-control-plane
     15 k8rs-worker
      3 k8rs-worker2
     19 k8rs-worker3
      7 <none>
```

`k8rs-worker` and `k8rs-worker3` agree exactly (200m, 220m). `k8rs-worker2`:
corpus **190m over 16 pods**, cluster **100m over 3 pods** — and 100m is kindnet
alone.

## 13.3 Which captured pods the break sequence deletes, fourth trip

```
$ (same loop as § 2)
present=24  gone=14
gone: broken-exit0(k8rs-worker2)   healthy-disk(k8rs-worker2)   healthy-hostpath(k8rs-worker2)
gone: healthy-retry(k8rs-worker2)  healthy-sidecar(k8rs-worker2) healthy-unreadysidecar(k8rs-worker2)
gone: broken-image(k8rs-worker2)   broken-init(k8rs-worker2)     broken-neverback(k8rs-worker2)
gone: broken-podlimit(k8rs-worker2) broken-readiness(k8rs-worker2) broken-restarts10serving(k8rs-worker2)
gone: broken-succeeded(k8rs-worker2) broken-neverrules(k8rs-worker3)
```

Distribution of the single-object `Pod` fixtures:

```
$ for f in tests/fixtures/*.json; do jq -r 'select(.kind=="Pod")|.spec.nodeName // "-"' "$f"; done | sort | uniq -c
      2 -
     10 k8rs-worker
     14 k8rs-worker2
     12 k8rs-worker3
```

## 13.4 The Drain-safety join, corpus against cluster

```
$ jq -c '.items[] | {name:.metadata.name, node:.spec.nodeName, phase:.status.phase,
                     ready:[.status.conditions[]|select(.type=="Ready")|.status][0]}' \
    tests/fixtures/healthy-deploy-pods.json
{"name":"healthy-deploy-7f84bdfb9b-6hlph","node":"k8rs-worker3","phase":"Running","ready":"True"}
{"name":"healthy-deploy-7f84bdfb9b-lb6wt","node":"k8rs-worker2","phase":"Running","ready":"True"}

$ kubectl --context kind-k8rs get pod healthy-deploy-7f84bdfb9b-6hlph -o jsonpath='...'
Running on k8rs-worker3 ready=False
$ kubectl --context kind-k8rs get pod healthy-deploy-7f84bdfb9b-lb6wt -o jsonpath='...'
Error from server (NotFound)

$ kubectl --context kind-k8rs get pdb -A -o custom-columns='NAME,ALLOWED,CURRENT,DESIRED,EXPECTED,GEN,OBS'
NAME               ALLOWED   CURRENT   DESIRED   EXPECTED   GEN   OBS
broken-pdb-floor   0         0         2         3          1     1
healthy-pdb-room   0         0         1         6          1     1
```

Committed: `broken-pdb-floor` `allowed 0 · current 2 · desired 2 · expected 2 ·
gen 1 · obs 1 · reason InsufficientPods`.

## 13.5 `unhealthyPodEvictionPolicy`, read off the live v1.36 API server

```
$ kubectl --context kind-k8rs explain pdb.spec.unhealthyPodEvictionPolicy
FIELD: unhealthyPodEvictionPolicy <string>
ENUM: AlwaysAllow, IfHealthyBudget
  ... If no policy is specified, the default behavior will be used, which
  corresponds to the IfHealthyBudget policy.
  IfHealthyBudget policy means that running pods (status.phase="Running"), but not
  yet healthy can be evicted only if the guarded application is not disrupted
  (status.currentHealthy is at least equal to status.desiredHealthy). Healthy pods
  will be subject to the PDB for eviction.
  AlwaysAllow policy means that all running pods ... can be evicted regardless of
  whether the criteria in a PDB is met.
```

Neither value makes the eviction API *stricter* than the counters. Both make it
looser for Running-but-not-Ready pods.

## 13.6 The two D40 plants, exercised

```
$ cargo test --locked -- --nocapture the_blocking_disruption
budgets: broken-pdb-floor allows Some(0) (healthy Some(2)/Some(2)) for {"app": "healthy-deploy"}
         — gen Some(1)/observed Some(1), Some("InsufficientPods")
       · healthy-pdb-room allows Some(1) (healthy Some(2)/Some(1)) for {"app": "broken-rollout"}
         — gen Some(1)/observed Some(1), Some("SufficientPods")
planted: healthy-pdb-room allows Some(1) but gen Some(2)/observed Some(1)
       · broken-pdb-floor allows Some(0) because Some("SyncFailed")

$ cargo test --locked
running 291 tests ... ok. 291 passed; 0 failed
running 7 tests   ... ok. 7 passed; 0 failed
```

## 13.7 Corpus timestamps against the pin

```
$ jq -r '... .metadata.creationTimestamp' tests/fixtures/*.json | sort | sed -n '1p;$p'
2026-08-20T22:42:25Z
2026-08-20T22:43:04Z

$ jq -r '.. | strings | select(test("^20..-..-..T[0-9:]+Z$"))' tests/fixtures/*.json | sort | tail -1
2026-08-20T23:12:59Z

$ grep -n 'time("2026' src/rules_tests.rs
232:    time("2026-08-21T00:00:00Z")
```

47 minutes between the newest capture and the pin. The guard that reads it is
`src/rules_tests/snapshot.rs:2070`, over `every_captured_pod()`.

## 13.8 Container-face drift, HEAD to the fourth trip

```
== crashloop  HEAD waiting r=9    NOW waiting r=10
== exit0      HEAD waiting r=9    NOW waiting r=10
== notfound   HEAD terminated r=10  NOW waiting r=9
== sigterm    HEAD waiting r=13   NOW waiting r=15
$ jq '.status.containerStatuses[0].state|keys[0]' tests/fixtures/oom.json       → "terminated"
$ jq '.status.initContainerStatuses[0].state|keys[0]' tests/fixtures/init.json  → "terminated"
```

Both faces of `oom` and `init` moved between the third trip and the fourth
(`waiting` → `terminated`); both are on the face-independent list
`restarted_after_a_bad_run()` asserts, `src/rules_tests/pod.rs:12594`.

## 13.9 Where rule 1 reads the waiting sentence

```
$ grep -n "fn crash_looping" -A 12 src/rules.rs | grep "waiting(c)"
3845:    let (reason, _) = waiting(c)?;
```

`src/rules_tests/snapshot.rs:431` asserts `message…contains("back-off")`; its
failure message at `:433` reads *"rule 1 shows the kubelet's own sentence"*.
