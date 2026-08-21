# 2026-08-21 — round two: the two new pod decodes, and the drain pane read against a bare `kubectl drain`

Second operator pass (step 6) over the Family C delta: `PodSnapshot::local_storage`,
the ephemeral half of `PodSnapshot::claims`, the rewritten
`// --- THE DRAIN SAFETY REPORT ---` region, and `main.rs`'s `reports` / `pane`.
Round one is
[`2026-08-21-family-c-analysis-report-family-review.md`](2026-08-21-family-c-analysis-report-family-review.md)
and is not repeated here; this file is a **second** measurement rather than an
append to that one, because product doc comments cite that file by section number.

Everything below ran on the **dev machine**. **No cluster was created and nothing
was written to `kind-k8rs`**, which was up: `kubectl drain --dry-run=client` is
client-side and cordons nothing, and every other cluster command is a read.
`K8RS_CLUSTER=review` was therefore not used — CLAUDE.md § *The one hard rule of
concurrency* allows one cluster at a time and the PM's was running.

Under [`reports/README.md`](README.md)'s rule: commands, counts and the specific
field values a finding turns on. No object dumps.

---

## 1. `<pod name>-<volume name>` off the API server's own validation, not off a doc string

Round one § 4 quoted `kubectl explain`. The derivation is also **pre-validated by
the API server at pod-create time**, which answers the *is it right for every
shape* question in a way `explain` cannot:

```
$ curl -sSL -o validation.go \
    https://raw.githubusercontent.com/kubernetes/kubernetes/master/pkg/apis/core/validation/validation.go
$ grep -n 'podMeta.Name + "-" + volName' -B 6 validation.go
790:      numVolumes++
791:      allErrs = append(allErrs, validateEphemeralVolumeSource(...))
792:      // Check the expected name for the PVC. ...
795:      if podMeta != nil && podMeta.Name != "" && volName != "" {
796:              pvcName := podMeta.Name + "-" + volName
797:              for _, msg := range ValidatePersistentVolumeName(pvcName, false) {
```

Three shapes settled by the same file:

```
$ grep -n 'func ValidateTemplateObjectMeta' -A 16 validation.go
1942:  allErrs = append(allErrs, validateFieldAllowList(*objMeta,
1942:      allowedTemplateObjectMetaFields, "cannot be set", fldPath)...)
1946:  var allowedTemplateObjectMetaFields = map[string]bool{
1947:      "Annotations": true,
1948:      "Labels":      true,
1949:  }
```

- **an explicit `volumeClaimTemplate.metadata.name` is rejected**, not honoured
  and not ignored — only `Annotations` and `Labels` may be set;
- **a derived name too long is rejected at the pod**, line 796 above, so it is
  never silently truncated;
- **`ephemeral` and `persistentVolumeClaim` are mutually exclusive** on one
  entry (lines 786-791, `may not specify more than 1 volume type`), so the
  `match` in `PodSnapshot::from` can never lose one to the other.

Controller side, same repo:

```
$ (pkg/controller/volume/ephemeral/controller.go)
pvcName := ephemeral.VolumeClaimName(pod, &vol)
pvc = &v1.PersistentVolumeClaim{
    ObjectMeta: metav1.ObjectMeta{ Name: pvcName, OwnerReferences: [...],
        Annotations: vol.Ephemeral.VolumeClaimTemplate.Annotations,
        Labels:      vol.Ephemeral.VolumeClaimTemplate.Labels },
    Spec: vol.Ephemeral.VolumeClaimTemplate.Spec }
```

**Not measured on a cluster** — no cluster was created, and no pod on `kind-k8rs`
declares an ephemeral volume:

```
$ for f in tests/fixtures/*.json; do jq '<pods with spec.volumes[].ephemeral>' "$f"; done
(no output — the corpus has none either; the only claim-mounting pod is
 default/healthy-disk, via persistentVolumeClaim.claimName)
```

## 2. `local_storage` against `kubectl/pkg/drain/filters.go`

```
$ (kubectl/pkg/drain/filters.go, tabs replaced with spaces)
func hasLocalStorage(pod corev1.Pod) bool {
    for _, volume := range pod.Spec.Volumes {
        if volume.EmptyDir != nil { return true }
    }
    return false
}
func (d *Helper) localStorageFilter(pod corev1.Pod) PodDeleteStatus {
    if !hasLocalStorage(pod) { return MakePodDeleteStatusOkay() }
    // Any finished pod can be removed.
    if pod.Status.Phase == corev1.PodSucceeded || pod.Status.Phase == corev1.PodFailed {
        return MakePodDeleteStatusOkay() }
    if !d.DeleteEmptyDirData { return MakePodDeleteStatusWithError(localStorageFatal) }
    return MakePodDeleteStatusWithWarning(true, localStorageWarning)
}
```

Presence only: no `medium` read, no check that a container mounts it. The filter
chain short-circuits on the first `Delete == false`, and `daemonSetFilter` and
`mirrorPodFilter` both run *before* `localStorageFilter` — so under
`--ignore-daemonsets` a DaemonSet or static pod's `emptyDir` never reaches this
filter at all. Counting `local` over `moving` is that behaviour, not an
approximation of it.

`medium`'s two legal values, off the live API server:

```
$ kubectl explain pod.spec.volumes.emptyDir.medium
    medium represents what type of storage medium should back this directory.
    The default is "" which means to use the node's default medium. Must be an
    empty string (default) or Memory.
```

Neither the corpus nor the cluster carries one:

```
$ kubectl get pods -A -o json | jq '<pods with an emptyDir, by medium>'
pods_with_emptydir: 4   movable: 4   any_memory_medium: 0
$ for f in tests/fixtures/*.json; do jq '<same>' "$f"; done
8 pods, all medium "(disk)"  (one of them Succeeded)
```

## 3. The drain pane against a bare `kubectl drain`, node by node, on today's cluster

Bare, no flags, as a reader who has just been told a node *is ready to drain*
would type it. Refusal classes per node:

```
$ for n in k8rs-control-plane k8rs-worker k8rs-worker2 k8rs-worker3; do
      kubectl drain $n --dry-run=client; done
k8rs-control-plane   DaemonSet-managed: 2
k8rs-worker          no controller: 7 · DaemonSet-managed: 3 · local storage: 2
k8rs-worker2         DaemonSet-managed: 2
k8rs-worker3         no controller: 9 · DaemonSet-managed: 3 · local storage: 2
```

The same cluster's pod list handed to the binary (the node list is the committed
capture; the pods and budgets are fresh reads, neither committed):

```
$ kubectl get pods -A -o json > live-pods.json
$ kubectl get pdb -A -o json  > live-pdb.json
$ ./target/debug/k8rs --analysis live-pods.json tests/fixtures/nodes.json live-pdb.json
  A drain below assumes --ignore-daemonsets, so DaemonSet pods never count as moving.
  ● k8rs-worker3 would never finish draining
      This node has stopped responding. A drain cannot confirm a pod is gone until
      it answers again, so it waits forever.
      → check the node itself: is it powered on and reachable?
  ● k8rs-worker drains, but throws away files on 2 pods
      2 pods here keep files on this machine's own disk — what Kubernetes calls an
      emptyDir volume — and a drain deletes them with the pods.
      9 pods here were started by hand, with no Deployment behind them. A drain
      deletes them and nothing brings them back.
      → copy what you need off them first — the replacement pods start with an empty disk
    k8rs-control-plane is ready to drain — 3 pods move
    k8rs-worker2 is ready to drain — nothing on it would move
```

| node | bare `kubectl drain` | the pane | agrees |
|---|---|---|---|
| `k8rs-control-plane` | DaemonSet only | ready to drain — 3 pods move | yes |
| `k8rs-worker` | 7 no-controller + 2 local storage | 2 local storage · 9 started by hand | yes — kubectl's two lists are disjoint only because it short-circuits; 7 + 2 = 9 |
| `k8rs-worker2` | DaemonSet only | ready to drain — nothing would move | yes |
| `k8rs-worker3` | 9 no-controller + 2 local storage | **N1 paragraph only** | **no** — § 4 |

## 4. Why `k8rs-worker3`'s row carries no orphan and no local-storage paragraph

```
$ jq '<pods on k8rs-worker3>' live-pods.json
total: 19   with a deletionTimestamp: 16   phases: {Running: 14, Pending: 5}
```

The node controller's unreachable eviction has stamped 16 of the 19. `moving`
drops every one of them, so `local` and `orphans` are both `0`. A bare
`kubectl drain` does not:

```
$ kubectl drain --help | grep -A2 skip-wait-for-delete-timeout
    --skip-wait-for-delete-timeout=0:
    If pod DeletionTimestamp older than N seconds, skip waiting for the pod.
    Seconds must be greater than 0 to skip.
```

`skipDeletedFilter` is inert at the default. The two facts the row drops are the
ones `kubectl` prints two paragraphs above.

## 5. The corpus, and the shapes it contains

```
$ cargo build && ./target/debug/k8rs --analysis tests/fixtures/*.json
  ● k8rs-worker2 would never finish draining         (budget · 2 local · 12 orphans)
  ● k8rs-worker3 would never finish draining         (N1 · budget · 3 local · 12 orphans)
  ● k8rs-worker drains, but throws away files on 2 pods    (2 local · 9 orphans)
    k8rs-control-plane is ready to drain — 2 pods move
```

emptyDir pods in the corpus, by node — `local` is 2 / 2 / 3 and one pod is
excluded:

```
$ for f in tests/fixtures/*.json; do jq '<pods with an emptyDir>' "$f"; done
broken-gang               k8rs-worker    Running
broken-restarts           k8rs-worker    Running
healthy-retry             k8rs-worker2   Running
broken-restarts10serving  k8rs-worker2   Running
broken-succeeded          k8rs-worker2   Succeeded   <- not counted
broken-neverrules         k8rs-worker3   Running
broken-oomserving         k8rs-worker3   Running
broken-restarts10         k8rs-worker3   Running
```

`broken-succeeded` is `localStorageFilter`'s own *any finished pod can be
removed* branch, reached here through `pods_on`'s `!finished(p)`.

## 6. A node whose kubelet answered and said `Ready: False`

`nodes.json` carries `True True True Unknown`, so the shape needs a plant. One
field moved on a scratch copy — **not committed, not a fixture**:

```
$ jq '(<k8rs-control-plane>.status.conditions[] | select(.type=="Ready"))
      |= (.status="False" | .reason="KubeletNotReady"
                          | .message="container runtime is down")' \
     tests/fixtures/nodes.json > nodes-cp-false.json
$ ls tests/fixtures/*.json | grep -v nodes.json \
      | xargs ./target/debug/k8rs --analysis nodes-cp-false.json
● k8rs-control-plane · 13 hours ago
  This node says it cannot run pods — nothing new will start here until it can
  ...
    k8rs-control-plane is ready to drain — 2 pods move
```

Same run, same plant on `k8rs-worker` instead:

```
● k8rs-worker · 13 hours ago
  This node says it cannot run pods — nothing new will start here until it can
  ● k8rs-worker drains, but throws away files on 2 pods
```

## 7. Three budgets blocking one node

Two extra budgets built from the committed one on a scratch copy — again not
committed:

```
$ jq '<+ aaa-pdb-syncfailed (DisruptionAllowed/SyncFailed)
      + zzz-pdb-below (currentHealthy 1, desiredHealthy 2)>' \
     tests/fixtures/poddisruptionbudgets.json > pdb-three.json
  ● k8rs-worker2 would never finish draining
      Kubernetes could not work out how many copies of the pods
      default/aaa-pdb-syncfailed protects are healthy, ...
      2 other rules on this node would stop the drain too.
      ...
      → check what default/aaa-pdb-syncfailed points at — ...
```

The two other budgets are named nowhere. Which of the three supplies the row's
text and its `action` is decided by `(namespace, name)` order.

## 8. A retained StatefulSet claim on the Waste pane

The default, off the live API server:

```
$ kubectl explain statefulset.spec.persistentVolumeClaimRetentionPolicy.whenScaled
    The default policy of `Retain` causes PVCs to not be affected by a scaledown.
$ kubectl get sts -A
default   broken-sts   1/2   14h
```

One claim renamed on a scratch copy of the committed PVC capture — not committed:

```
$ jq '<healthy-disk renamed to data-broken-sts-1>' \
     tests/fixtures/persistentvolumeclaims.json > pvc-sts.json
  ▲ default/data-broken-sts-1 is 64Mi nobody is using
      A disk was reserved for it and no pod is mounting it. It stays reserved
      until somebody deletes it.
```

## 9. `uncapped_workloads`, the same query on both sides

```
$ kubectl get pods -A -o json | jq '<not Succeeded/Failed, missing a cpu or a
    memory limit on the pod or on every container>'
uncapped_pods: 42   k8rs group keys (controller, or the pod itself): 29
                    distinct controllers only: 9
$ ./target/debug/k8rs --analysis tests/fixtures/*.json
    34 workloads have no memory or CPU limit
```

Round one measured 41 / 10 on the same cluster the day before.

## 10. Node-rule severities, read off the source

```
$ grep -n 'severity: Severity::' src/rules.rs   (inside THE NODE RULES region)
6153  Severity::Critical   node_stopped_being_ready       (N1)
6219  Severity::Warn       cordoned_with_work_left_on_it  (N2)
6266  Severity::Warn       node_running_low               (N3)
6329  Severity::Info       kubelet_too_far_behind         (N4)
6417  Severity::Info       node_overcommitted             (N5)
$ grep -n 'object:' src/rules.rs
... 6170 6231 6284 6346 6439 are the only `object: node.id`; 7277 is
    ObjectKind::Other("kubeconfig"), every other is a pod or a workload
```

`n1_is_the_only_critical_node_rule_which_is_what_makes_the_pick_by_identity_enough`
asserts over the findings *one cluster* produces:

```
$ sed -n '675,690p' src/analysis_tests/drain.rs
    assert!(about_nodes.len() >= 3, ...);
    assert_eq!(critical, vec!["k8rs-worker3"], ...);
```

N4 and N5 do not fire on `drain_corpus()` and are not in `about_nodes`.

## 11. `sanitize` on the Posture row, which is a `hostPath` verbatim

A scratch copy of `healthy-hostpath.json` whose `hostPath.path` was set to
`/data`, one ESC byte (0x1b), then `[31mRED/x` — the shape invariant 9 exists
for:

```
$ jq -r '<the path>' hostpath-esc.json | cat -v
/data^[[31mRED/x
$ ./target/debug/k8rs --analysis hostpath-esc.json tests/fixtures/nodes.json \
    | grep RED | cat -v
  M-bM-^WM-^K /data[31mRED/x
```

The ESC byte is gone; the printable remainder is left whole and nothing is
truncated.

## 12. Full run

```
$ cargo test --locked
test result: ok. 394 passed; 0 failed
test result: ok. 7 passed; 0 failed
```

## 13. Environment

```
$ kubectl get nodes -o custom-columns='NAME:...,READY:...'
k8rs-control-plane True · k8rs-worker True · k8rs-worker2 True · k8rs-worker3 Unknown
$ kind get clusters
k8rs
$ kubectl version --output=json | jq -r '.serverVersion.gitVersion'
v1.36.1
```

## 14. The one count badge the driver can print

Capacity is the only report with a count badge (`flagged.to_string()`). A scratch
copy of `nodes.json` with `k8rs-worker`'s `status.allocatable.cpu` set to `100m`
reaches it:

```
$ ls tests/fixtures/*.json | grep -v nodes.json \
    | xargs ./target/debug/k8rs --analysis nodes-small.json
[capacity] 1
  What each node promised, and what it has
  ▲ k8rs-worker   0.45 of 0.1 cpu · 234Mi of 23.1Gi
```

`screens/widgets.md`'s badge-glyph rule, added in the same turn, says a badge
that is a count draws its band as a glyph because the glyph is the unit.
