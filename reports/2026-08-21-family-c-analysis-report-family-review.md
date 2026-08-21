# 2026-08-21 — the six analysis reports read as an operator: what a bare `kubectl drain` does, and four numbers weighed against it

Operator review (step 6) of the Family C producers in `src/analysis.rs`, their
tests, and the `src/rules.rs` helpers they share. Every command below was run on
the **dev machine** against the PM's fixture cluster `kind-k8rs`, which was up at
the time, and against the working tree.

**Nothing was written to the cluster and no second cluster was created.** The
brief opened the `K8RS_CLUSTER=review` slot; `kind get clusters` showed `k8rs`
already up, and CLAUDE.md § *The one hard rule of concurrency* says one cluster
at a time, so the two shapes that would have needed a create — a generic
ephemeral volume and a `PodDisruptionBudget` caught mid-lag — are argued from the
live API server's own field documentation instead (§ 4, § 5). Everything else is
read-only against the cluster that was already there: `kubectl drain
--dry-run=client` is client-side and cordons nothing.

Under [`reports/README.md`](README.md)'s rule: field values and counts only, no
object dumps, no uids, no addresses.

---

## 1. What a **bare** `kubectl drain` does on this cluster

`reports/2026-08-21-family-c-corpus-drain-and-capacity.md` § 3 ran this with
`--ignore-daemonsets --delete-emptydir-data`. Bare, as a reader who has just been
told a node *is ready to drain* would type it:

```
$ kubectl drain k8rs-control-plane --dry-run=client
node/k8rs-control-plane cordoned (dry run)
error: unable to drain node "k8rs-control-plane" due to error: cannot delete
DaemonSet-managed Pods (use --ignore-daemonsets to ignore):
kube-system/kindnet-bhzgd, kube-system/kube-proxy-n8hgn, continuing command...
There are pending nodes to be drained:
 k8rs-control-plane
cannot delete DaemonSet-managed Pods (use --ignore-daemonsets to ignore):
kube-system/kindnet-bhzgd, kube-system/kube-proxy-n8hgn
```

```
$ kubectl drain k8rs-worker --dry-run=client
node/k8rs-worker already cordoned (dry run)
error: unable to drain node "k8rs-worker" due to error: [cannot delete Pods that
declare no controller (use --force to override): default/broken-crashloop,
default/broken-nolimits, default/broken-overhead, default/broken-probe0,
default/broken-socket, default/broken-startup, default/broken-wedged, cannot
delete DaemonSet-managed Pods (use --ignore-daemonsets to ignore):
default/broken-ds-l698x, kube-system/kindnet-h2st9, kube-system/kube-proxy-5d9xj,
cannot delete Pods with local storage (use --delete-emptydir-data to override):
default/broken-gang, default/broken-restarts], continuing command...
```

Three refusal classes, not one. `kubectl` aborts on all three **after cordoning**.

What is on the control plane, and what a drain would relocate:

```
$ kubectl get pods -A --field-selector=spec.nodeName=k8rs-control-plane -o json \
  | jq -r '.items[] | "\(.metadata.namespace)/\(.metadata.name)\towner=..."'
kube-system/coredns-589f44dc88-hdrv5                      owner=ReplicaSet
kube-system/coredns-589f44dc88-lbkj6                      owner=ReplicaSet
kube-system/etcd-k8rs-control-plane                       owner=Node
kube-system/kindnet-bhzgd                                 owner=DaemonSet
kube-system/kube-apiserver-k8rs-control-plane             owner=Node
kube-system/kube-controller-manager-k8rs-control-plane    owner=Node
kube-system/kube-proxy-n8hgn                              owner=DaemonSet
kube-system/kube-scheduler-k8rs-control-plane             owner=Node
local-path-storage/local-path-provisioner-855c7b7774-6lqqs owner=ReplicaSet
```

Three pods pass `a_drain_would_move`. The pane the committed corpus produces for
the same node:

```
$ cargo test --locked -- --nocapture --test-threads=1 analysis
  k8rs-control-plane is ready to drain — 2 pods move
```

(2 rather than 3 because `kube-system-pods.json` is a capture of one namespace
and `local-path-provisioner` sits in `local-path-storage`; the two DaemonSet pods
that cause the refusal *are* in it — `kindnet-bhzgd` and `kube-proxy-n8hgn`,
listed above.)

And the test that fixes that sentence as correct:

```
$ grep -n 'fn a_node_carrying_only_static_and_daemonset_pods_is_ready_to_drain' \
    -A 2 src/analysis_tests/drain.rs
250:fn a_node_carrying_only_static_and_daemonset_pods_is_ready_to_drain() {
251:    // **`a_drain_would_move` is the whole narrowing.** `k8rs-control-plane` runs
252:    //   four static pods and two DaemonSet pods that a drain never evicts, ...
```

## 2. Where the "ready to drain" sentence comes from, and what it reads

```
$ sed -n '779,800p;859,876p' src/analysis.rs   (drain_row)
fn drain_row<'a>(snapshot, budgets, node: &'a NodeSnapshot) -> DrainLine<'a> {
    let name = node.id.name.as_str();
    ...
    text: match moving.len() {
        0 => format!("{name} is ready to drain — nothing on it would move"),
```

`node` is read for `id.name` and `id` only. No node field is consulted:
`conditions[Ready]`, `spec.unschedulable` and `spec.taints` are all on
`NodeSnapshot` and none is read.

The corpus carries a node whose kubelet is gone:

```
$ jq -c '.items[] | {name:.metadata.name,
    ready:[.status.conditions[]|select(.type=="Ready")|.status][0]}' tests/fixtures/nodes.json
k8rs-control-plane ready=True   k8rs-worker ready=True
k8rs-worker2       ready=True   k8rs-worker3 ready=Unknown
```

and the pane, on a snapshot with no blocking budget, prints:

```
  k8rs-worker3 is ready to drain — nothing on it would move
```

`k8rs-worker3` is the node N1 draws *"This node has stopped responding"* about.

## 3. Every command-log line the analysis panes claim, run as typed

```
$ kubectl get pdb -A                 -> exit 0   (2 rows)
$ kubectl get svc,endpointslices -A  -> exit 0   (4 services, 4 slices)
$ kubectl get pvc,replicasets -A     -> exit 0   (2 pvc, 3 replicasets)
$ kubectl get csr                    -> exit 0   "No resources found"
$ kubectl get nodes -o json          -> exit 0
$ kubectl version                    -> exit 0   Client v1.36.3 / Server v1.36.1
$ kubectl top nodes                  -> exit 1   "error: Metrics API not available"
```

All seven resolve as typed. What `kubectl get pdb -A` prints:

```
NAMESPACE  NAME              MIN AVAILABLE  MAX UNAVAILABLE  ALLOWED DISRUPTIONS
default    broken-pdb-floor  2              N/A              0
default    healthy-pdb-room  1              N/A              0
```

No `observedGeneration` column, no `DisruptionAllowed` reason — the two fields
the new blocking branches key on.

## 4. `pod.spec.volumes[].ephemeral`, off the live API server

```
$ kubectl explain pod.spec.volumes.ephemeral.volumeClaimTemplate
DESCRIPTION:
    Will be used to create a stand-alone PVC to provision the volume. The pod in
    which this EphemeralVolumeSource is embedded will be the owner of the PVC,
    i.e. the PVC will be deleted together with the pod.  The name of the PVC
    will be `<pod name>-<volume name>` ...
```

`ephemeral` is a sibling of `persistentVolumeClaim` on the same
`spec.volumes[]` entry. What the snapshot keeps:

```
$ grep -n 'persistentVolumeClaim.claimName' src/rules.rs
1064:    /// **Prune line: `spec.volumes[].persistentVolumeClaim.claimName`.**
```

No live pod on this cluster uses one, so the row was not observed firing; the
field's own documentation is the evidence that the PVC exists, is `Bound`, and is
named by no `persistentVolumeClaim.claimName` anywhere.

## 5. Uncapped pods against their controllers, on this cluster

```
$ kubectl get pods -A -o json | jq -r '<pods not Succeeded/Failed, missing a cpu
    or memory limit on any container, grouped by controlling ownerReference>'
uncapped pods: 41   distinct controllers: 10
```

The row `analysis.rs` builds from the same set:

```
$ sed -n '591,604p' src/analysis.rs
    text: format!("{count} {noun} {verb} no memory or CPU limit"),
      ...  ("workload", "has") / ("workloads", "have")
```

`count` is [`uncapped_workloads`], which counts pods. The word `workload` is
`ClusterSnapshot::workloads` / `WorkloadSnapshot` everywhere else in the product
— Deployments, StatefulSets, DaemonSets.

## 6. The one host path a pod can put on both screens, and whether the corpus has it

`rules.rs` § rule 8 and `analysis.rs` § `left_by_rule_8` are exact complements
**per mount**:

```
rule 8 fires  <=>  path == "/"  ||  is_runtime_socket(path)  ||  (!read_only && !node_agent)
posture keeps <=>  path != "/" && !is_runtime_socket(path) && ( read_only ||  node_agent)
```

`posture`'s row is per **path**, and its `writable` bit is OR-ed only over the
mounts that survived the filter. So a pod mounting one hostPath twice — once
read-only, once writable — puts the writable mount on Alerts and the read-only
one on Posture, and Posture's sentence for that path reads `Read-only, mounted by
1 pod in <ns>.`

The partition test walks mounts, not (pod, path):

```
$ sed -n '87,89p' src/analysis_tests/posture.rs
    let (mine, rule_8): (Vec<_>, Vec<_>) = mounts
        .iter()
        .partition(|(pod, mount)| super::left_by_rule_8(pod, mount));
```

Whether any committed pod carries that shape — grouping every hostPath mount of
every captured pod by resolved path and selecting the groups with more than one
`readOnly` value:

```
$ for f in tests/fixtures/*.json; do jq -r '<group hostPath mounts by path+subPath,
    select (map(.ro)|unique|length) > 1>' "$f"; done
(no output)

$ (the same query without the select, one file, as its own canary)
etcd-k8rs-control-plane: [{"p":"/etc/kubernetes/pki/etcd","modes":[false]},
                          {"p":"/var/lib/etcd","modes":[false]}]
kindnet-bhzgd:           [{"p":"/etc/cni/net.d","modes":[false]},
                          {"p":"/lib/modules","modes":[true]}, ...]
```

The query finds mounts; no captured pod has the mixed shape.

## 7. Sort keys across the six producers

```
$ grep -n 'sort_by' src/analysis.rs
351:  lines.sort_by(|a, b| b.over.cmp(&a.over).then_with(|| a.name.cmp(b.name)));       bool, &str
698:  lines.sort_by(|a, b| b.band.cmp(&a.band).then_with(|| a.name.cmp(b.name)));      u8,   &str
804:  blocked.sort_by(|a, b| a.budget.cmp(&b.budget));                                 String "ns/name"
1250: orphans.sort_by(|a, b| (&a.id.namespace, &a.id.name).cmp(&(&b.id.namespace, &b.id.name)));
1329: idle.sort_by(   |a, b| (&a.id.namespace, &a.id.name).cmp(&(&b.id.namespace, &b.id.name)));
1500: paths.sort_by(  |a, b| b.1.pods.cmp(&a.1.pods).then_with(|| a.0.cmp(b.0)));      usize, &String
1761: behind.sort_by( |a, b| b.gap.cmp(&a.gap).then_with(|| a.name.cmp(b.name)));      u32,  &str
```

Every key is typed; none is a rendered quantity. Line 804 is the one that
differs in shape from 1250/1329 — it sorts the joined `namespace/name`, whose
doc claims *"the same order the reader's own `kubectl get pdb -A` prints"*:

```
$ printf "team/web\nteam-a/api\n" | LC_ALL=C sort
team-a/api
team/web
$ printf "team-a api\nteam web\n" | LC_ALL=C sort -k1,1 -k2,2   # namespace, then name
team web
team-a api
```

`'-'` (0x2D) sorts before `'/'` (0x2F), so the joined form reverses the pair
whenever one namespace is a prefix of another.

## 8. Where a report is rendered outside the test harness

```
$ grep -rn 'capacity(\|drain_safety(\|waste(\|posture(\|versions(\|certificates(' src/*.rs \
    | grep -v _tests
src/analysis.rs:320:pub fn capacity(...)        src/analysis.rs:1114:pub fn waste(...)
src/analysis.rs:677:pub fn drain_safety(...)    src/analysis.rs:1472:pub fn posture(...)
src/analysis.rs:1695:pub fn versions(...)       src/analysis.rs:1974:pub fn certificates(...)

$ grep -n 'analysis' src/main.rs
17:mod analysis;
```

`main.rs` declares the module and calls nothing in it. Every string in the six
reports has therefore only ever reached a screen through
`src/analysis_tests.rs`'s `pane()`, which is `#[cfg(test)]` and applies no
`sanitize`:

```
$ grep -n 'sanitize' src/main.rs | tail -3
348:    lines.push(format!("  → {}", sanitize(&finding.action)));
366:        Some(namespace) => format!("{}/{}", sanitize(namespace), sanitize(&id.name)),
367:        None => sanitize(&id.name),
```

All three call sites are the findings printer.

## 9. Panes read as printed

```
$ cargo test --locked -- --nocapture --test-threads=1 analysis
running 93 tests ... 93 passed
```

Lines this review turned on, quoted from that run:

```
Control plane v1.36.1 · 3 of 4 kubelets match
Nothing k8rs could measure is outside the window Kubernetes supports. It could
not work out how far behind some of these machines are.
```

```
  k8rs-control-plane   0.95 of 12 cpu · 290Mi of 23.1Gi
      using 0.95 cpu and 290Mi
  k8rs-worker2   0.11 of 12 cpu · 178Mi of 23.1Gi          <- no `using` line, and
  k8rs-worker3   0.25 of 12 cpu · 130Mi of 23.1Gi             nothing on the pane
      using 0.25 cpu and 130Mi                                says why
```

```
● k8rs-worker2 would never finish draining
      default/broken-pdb-floor was changed and its numbers have not caught up —
      the change is version 2, the numbers are from version 1. ...
      → look again in a moment — if the numbers never catch up, check that the
        cluster's controller manager is running
```

```
[sidebar]  15d▲
[sidebar]  out●
```

`screens/analysis.md` § *Certificates and Versions* rules that *"a badge which is
a count draws its band as a glyph and a badge which is a duration does not"*; the
test printer draws one on both.

## 10. The read-only role against the six reports

`docs/security.md` § `k8rs-readonly`, read at this tree, against what each
producer reads:

```
capacity      pods, nodes                          [""] pods, nodes                 present
              metrics                              ["metrics.k8s.io"] nodes         present
drain safety  pods, nodes, poddisruptionbudgets    ["policy"] poddisruptionbudgets  present
waste         services, endpointslices,            [""] services                    present
              persistentvolumeclaims, replicasets  ["discovery.k8s.io"] endpointslices present
                                                   [""] persistentvolumeclaims      present
                                                   ["apps"] replicasets             present
posture       pods                                 [""] pods                        present
versions      nodes + /version                     [""] nodes                        present
certificates  kubeconfig (no verb),                ["certificates.k8s.io"]
              certificatesigningrequests             certificatesigningrequests     present
```

Nothing the six read is missing from the documented role.

## 11. Two field defaults the rows turn on

```
$ kubectl explain cronjob.spec.successfulJobsHistoryLimit
DESCRIPTION:
    The number of successful finished jobs to retain. Value must be non-negative
    integer. Defaults to 3.

$ kubectl explain pdb.spec.unhealthyPodEvictionPolicy
ENUM: AlwaysAllow, IfHealthyBudget
    AlwaysAllow policy means that all running pods (status.phase="Running") will
    be considered for eviction regardless of whether the criteria in a PDB is
    met.
```

Waste counts every `Succeeded`/`Failed` pod as *finished and never removed*; a
CronJob retains three by default. Drain safety keys *would never finish draining*
on the three status counters, and `AlwaysAllow` lets the eviction API ignore them
for a Running-but-unhealthy pod — `unhealthyPodEvictionPolicy` is on no snapshot
type and in no prune line.


## 12. Environment

```
$ kubectl version --output=json | jq -r '.serverVersion.gitVersion'
v1.36.1
$ kind get clusters
k8rs
$ free -g | head -2
               total   used   free   available
Mem:              23      4     11          19
$ nproc
12
```
