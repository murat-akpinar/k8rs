# The read-only `ClusterRole`, run under itself with `system:discovery` out of reach

`k8s-admin`, 2026-08-30. Subject: the `k8rs-readonly` block in
[`docs/security.md`](../docs/security.md) and the Phase 5 box that asks for it to be
"verified by running v0.0.1 against kind under exactly that role and nothing more".

Cluster: the PM's fixture cluster `kind-k8rs`, four nodes, server v1.36.1, 41 pods.
Binary: `target/debug/k8rs` at `db1975c`. No cluster was created or destroyed by this
run; every role edit was restored from a `trap`
([D185](../NOTES.md#d185--cleanup-on-the-last-line-is-not-cleanup-and-the-resource-is-not-always-a-file-2026-08-30)).

## 0. The chain from the doc to the cluster

Both links checked before anything was measured, so a later "the role does X" is about
the block in `docs/security.md` and not about a retyped copy.

```
$ diff <(kubectl get clusterrole k8rs-readonly -o jsonpath='{.rules}' \
          | python3 -c "import json,sys; print(json.dumps(json.load(sys.stdin),indent=1,sort_keys=True))") \
       <(python3 -c "import yaml,json; print(json.dumps(yaml.safe_load(open('$HOME/.cache/k8rs-rbac/role.yaml'))['rules'],indent=1,sort_keys=True))")
APPLIED ROLE == role.yaml

$ awk '/^apiVersion: rbac/{f=1} f{print} /^```$/{if(f)exit}' docs/security.md | sed '/^```$/d' > doc-role.yaml
$ diff doc-role.yaml $HOME/.cache/k8rs-rbac/role.yaml
role.yaml == docs/security.md block
```

## 1. Isolating the role from `system:discovery`, without touching it

A ServiceAccount is in `system:authenticated`, which the default `system:discovery`
ClusterRoleBinding grants discovery to. A `SubjectAccessReview` takes the group list as
a field, so the role can be asked about alone. The RBAC authorizer's `status.reason`
names the binding that allowed it.

```
$ kubectl create -f - -o jsonpath='{.status.allowed}  reason={.status.reason}'
  apiVersion: authorization.k8s.io/v1
  kind: SubjectAccessReview
  spec:
    user: system:serviceaccount:default:k8rs-reader
    groups: []            # or [system:authenticated]
    nonResourceAttributes: {path: <path>, verb: get}
```

| path | `groups: []` | `groups: [system:authenticated]` |
|---|---|---|
| `/api` | true — `ClusterRoleBinding "k8rs-reader-binding" of ClusterRole "k8rs-readonly"` | true — same |
| `/apis` | true — `k8rs-reader-binding` of `k8rs-readonly` | true — same |
| `/apis/apps/v1` | true — `k8rs-reader-binding` of `k8rs-readonly` | true — `ClusterRoleBinding "system:discovery"` |
| `/version` | true — `k8rs-reader-binding` of `k8rs-readonly` | true — `k8rs-readonly` |

With the role's `nonResourceURLs` rule removed (restored from a `trap`):

| path | `groups: []` | `groups: [system:authenticated]` |
|---|---|---|
| `/api` | **false**, `reason` empty | true — `system:discovery` |
| `/apis` | **false**, `reason` empty | true — `system:discovery` |
| `/apis/apps/v1` | **false**, `reason` empty | true — `system:discovery` |
| `/version` | **false**, `reason` empty | true — `system:discovery` |

`system:discovery` was read but never edited:

```
$ kubectl get clusterrolebinding system:discovery -o jsonpath='{.roleRef.name}{" -> "}{.subjects[*].name}'
system:discovery -> system:authenticated
```

## 2. Running the binary under the role and nothing more

Impersonation drops the auto-added `system:authenticated` when a group is named
explicitly, so an impersonating kubeconfig reaches the role plus
`system:public-info-viewer`'s five health paths and nothing else. kube 4.2.0 reads
`as` / `as-groups` from the kubeconfig (`kube-client-4.2.0/src/client/config_ext.rs:366-378`).

That the group really is excluded, measured rather than assumed — `selfsubjectreviews`
comes from `system:basic-user`, bound to `system:authenticated`:

```
$ KUBECONFIG=<impersonating> kubectl auth whoami
error: the selfsubjectreviews API is not enabled in the cluster or you do not have permission to call it
```

```
$ KUBECONFIG=<impersonating> timeout 25 ./target/debug/k8rs --live --analysis
exit=124 (the timeout)
stderr (1 line):
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
stdout (584 lines), first line:
41 pods · 4 nodes
panes: [capacity] [certificates] [drain safety] [posture] [restarts] [versions] [waste]
refusal grep (forbidden|cannot list|permission|not allowed|not getting): 0 in stdout, 0 in stderr
capacity carried live usage, e.g.  k8rs-control-plane  0.95 of 12 cpu · 290Mi of 23.1Gi / using 0.148 cpu and 1.1Gi
```

## 3. Which grants are reachable from the code

Every typed Kubernetes import in `src/k8s.rs:124-137`:

```
DaemonSet, Deployment, ReplicaSet, StatefulSet          (apps)
CertificateSigningRequest                               (certificates.k8s.io)
Node, PersistentVolumeClaim, Pod, Service               (core)
EndpointSlice                                           (discovery.k8s.io)
PodDisruptionBudget                                     (policy)
```

Greps over product code, `_tests` excluded:

```
$ grep -rn "\.logs(\|log_stream\|LogParams" src/*.rs        -> no match
$ grep -rn "Api::<Event\|events::v1\|core::v1::Event" src/*.rs -> no match
$ grep -rn "ConfigMap>" src/*.rs                            -> no match
$ grep -rn "ObjectKind::Job\|ObjectKind::CronJob" src/*.rs  -> no match
$ grep -rnoE '"/[a-z0-9./{}*-]+"' src/k8s.rs src/main.rs    -> one hit:
    src/k8s.rs:2480: const METRICS_NODES = "/apis/metrics.k8s.io/v1beta1/nodes"
```

`src/rules.rs:1907-1908` maps the `batch` group off a pod's `ownerReference` string;
`src/k8s.rs:2809` (`owner_uid`) returns `None` for any owner that is not a ReplicaSet,
so no Job object is ever fetched. `src/k8s.rs:2832` (`unresolved_owners`) and `:2872`
(`owner_fetched`) are `pub` and called from no driver — `main.rs:2194` is a comment.
`src/k8s.rs:5018-5057` (`coverage`) probes with `Api::<Pod>` and never reads a
`Namespace` object. No `src/ops.rs` exists.

## 4. A/B: the documented role against the same role minus four grants

Removed: `batch: ["jobs"]`, and `configmaps`, `events`, `pods/log` from the core rule.
Back to back, same impersonated identity, restored from a `trap`.

```
=== A: documented role (7 rules) ===
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
41 pods · 4 nodes
13 critical, 3 warnings
[capacity] [certificates] [drain safety] [posture] [restarts] [versions] [waste]
refusals: 0 / 0

=== B: minus batch/jobs, configmaps, events, pods/log ===
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
41 pods · 4 nodes
13 critical, 3 warnings
[capacity] [certificates] [drain safety] [posture] [restarts] [versions] [waste]
refusals: 0 / 0
[trap] role restored
```

A further run with every unused **verb** also stripped — nine rules, `list`+`watch` only
on the five watched kinds, `list` only on the six report lists and on metrics,
`get`+`list` on replicasets:

```
minimum-verb role applied (9 rules, no unused resource, no unused verb)
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
41 pods · 4 nodes
13 critical, 3 warnings
panes: [capacity] [certificates] [drain safety] [posture] [restarts] [versions] [waste]
Ask-for lines: 0
using lines:   20
[trap] role restored
```

## 5. Every degraded sentence the role can produce, on one screen

Removed in one edit: `poddisruptionbudgets`, `certificatesigningrequests`,
`endpointslices`, the whole `metrics.k8s.io` rule, `replicasets`, `services`,
`persistentvolumeclaims`. Restored from a `trap`.

```
$ KUBECONFIG=<impersonating> timeout 20 ./target/debug/k8rs --live --analysis
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
41 pods · 4 nodes
13 critical, 3 warnings
panes: [capacity] [certificates] [drain safety] [posture] [restarts] [versions] [waste]

 90:  You are not allowed to read what each node is using.
 91:  Ask for read access to node metrics.
 98:  Ask for permission to list certificatesigningrequests across the whole cluster.
102:  Not checked. Working out whether a drain finishes needs the rules that say how many
      copies of a workload must stay up, and k8rs could not read them — without them every
      node would look safe.
103:  Ask for permission to list poddisruptionbudgets across the whole cluster.
223:  Ask for permission to list services and endpointslices.
225:  Ask for permission to list persistentvolumeclaims.
232:  Ask for permission to list replicasets.
```

The metrics rule alone, removed, gives the capacity pane verbatim:

```
[capacity]
  What each node promised, and what it has
    k8rs-control-plane   0.95 of 12 cpu · 290Mi of 23.1Gi
    k8rs-worker   0.47 of 12 cpu · 378Mi of 23.1Gi
    k8rs-worker2   0.1 of 12 cpu · 50Mi of 23.1Gi
    k8rs-worker3   0.22 of 12 cpu · 282Mi of 23.1Gi
  You are not allowed to read what each node is using.
  Ask for read access to node metrics.
    19 workloads have no memory or CPU limit
      Nothing stops one taking a whole node.
```

The ten permission-shaped `ask_for` strings in `src/analysis.rs`, in file order:

| line | string |
|---|---|
| 414 | `Ask for permission to list nodes across the whole cluster.` |
| 569 | `Ask for read access to node metrics.` |
| 813 | `Ask for permission to list nodes across the whole cluster.` |
| 824 | `Ask for permission to list poddisruptionbudgets across the whole cluster.` |
| 1696 | `Ask for permission to list services, endpointslices, persistentvolumeclaims and replicasets.` |
| 1792 | `Ask for permission to list services and endpointslices.` |
| 1895 | `Ask for permission to list persistentvolumeclaims.` |
| 2086 | `Ask for permission to list replicasets.` |
| 2844 | `Ask for permission to list nodes across the whole cluster.` |
| 3239 | `Ask for permission to list certificatesigningrequests across the whole cluster.` |

And the watch-trouble line, `src/main.rs:1826`:
`the role this kubeconfig uses needs to ` + "`list` and `watch` pods".

## 6. Discovery refused, end to end

The role's `nonResourceURLs` rule removed *and* the identity outside
`system:authenticated` — the state the PM's SA-token run could not reach.

```
$ KUBECONFIG=<impersonating> kubectl get --raw /apis
Error from server (Forbidden): forbidden: User "system:serviceaccount:default:k8rs-reader" cannot get path "/apis"

$ KUBECONFIG=<impersonating> timeout 20 ./target/debug/k8rs --live --analysis
exit=124
stderr (1 line):
k8rs: watching — server v1.36.1 · could not list what this cluster serves, so k8rs cannot
show you what is in it or tell which add-ons it has (the role this kubeconfig uses needs
to `get /apis`)
stdout (756 lines): 41 pods · 4 nodes, all seven panes drawn, capacity still carrying
`using 0.154 cpu and 1.1Gi`, versions still `Control plane v1.36.1 · 4 of 4 kubelets match`
```

## 7. Writes and Secrets, by real attempt rather than by `can-i`

Server dry-run where the API takes one. Under the SA token, not impersonation.

```
$ kubectl delete pod broken-config -n default --dry-run=server
Error from server (Forbidden): pods "broken-config" is forbidden: User "system:serviceaccount:default:k8rs-reader" cannot delete resource "pods" in API group "" in the namespace "default"
$ kubectl patch deployment broken-rollout -n default --type=merge -p '{}' --dry-run=server
Error from server (Forbidden): deployments.apps "broken-rollout" is forbidden: ... cannot patch resource "deployments" in API group "apps" in the namespace "default"
$ kubectl delete node k8rs-worker3 --dry-run=server
Error from server (Forbidden): nodes "k8rs-worker3" is forbidden: ... cannot delete resource "nodes" in API group "" at the cluster scope
$ kubectl get secrets -n kube-system
Error from server (Forbidden): secrets is forbidden: ... cannot list resource "secrets" in API group "" at the cluster scope
```

Every read the role names answers:

```
get pods -n default      -> pod/broken-config …
get nodes                -> node/k8rs-control-plane …
get csr                  -> (empty list, allowed)
get pdb -A               -> poddisruptionbudget.policy/broken-pdb-floor …
get endpointslices -A    -> endpointslice.discovery.k8s.io/broken-noendpoints-… …
get pvc -A               -> persistentvolumeclaim/broken-unused-disk …
get rs -A                -> replicaset.apps/broken-owned-… …
get --raw /apis/metrics.k8s.io/v1beta1/nodes -> {"kind":"NodeMetricsList","apiVersion":"metrics.k8s.io/v1beta1",…}
```

## 8. State on exit

```
$ diff <(kubectl get clusterrole k8rs-readonly -o jsonpath='{.rules}' | …) <(role.yaml rules)
RESTORED — byte-identical to role.yaml, which is byte-identical to docs/security.md
$ kubectl get clusterrolebinding system:discovery -o jsonpath='{.roleRef.name}'
system:discovery
$ kubectl get nodes --no-headers | wc -l      -> 4
$ kubectl get pods -A --no-headers | wc -l    -> 41
$ kubectl get clusterroles -o name | grep -c k8rs -> 1
```

`SubjectAccessReview` is not persisted (`kubectl get subjectaccessreviews` answers
`MethodNotAllowed`), so nothing this run created outlived it. The impersonating
kubeconfig was written mode 0600 into the session scratchpad and shredded.
