# `may_i` against a real cluster: the two reviews, the RBAC grant, and `kubectl auth can-i` side by side

`k8s-admin`, 2026-09-05. Subject: the uncommitted `// --- MAY I START/END ---` region of
`src/ops.rs` and the `k8rs ops may-i` driver in `src/main.rs`, briefed against
[D229](../NOTES.md#d229--the-four-rulings-mayi-could-not-be-briefed-without-and-the-boxs-arithmetic-that-went-stale-under-it-2026-09-05)
and [D23](../NOTES.md#d23--permissions-are-discovered-by-failing-and-that-is-backwards).

Cluster: **ephemeral**, `K8RS_CLUSTER=review`, one control-plane node
(`K8RS_WORKERS=0`), `kindest/node:v1.36.1`, API on `127.0.0.1:6444`, its own
kubeconfig — never `~/.kube/config`. Built and destroyed inside this run; teardown
is § 9. The PM's fixture cluster `k8rs` was running beside it and was never
addressed. Binary: `cargo build` over a copy of the working tree at `8b6ecc8` +
the uncommitted `may_i` diff, with its own `CARGO_TARGET_DIR` outside the repo.

Impersonation is how every identity below is reached; nothing in the cluster was
deleted and the two documented ClusterRoles were applied verbatim, never edited.
The identities are `k8rs-probe` in one of six groups. kube 4.2.0 reads
`as` / `as-groups` from the kubeconfig and `kubectl` reads `act-as` /
`act-as-groups`; both key pairs were written into each file so the two tools
address the same subject.

## 0. The chain from the doc to the cluster

The two roles under test are the blocks in `docs/security.md`, extracted by
pattern rather than retyped.

```
$ python3 -c "...re.findall(r'```yaml\n(apiVersion: rbac...)```', docs/security.md)"
blocks found: 2
['  name: k8rs-readonly']
['  name: k8rs-admin']
$ kubectl apply -f <block 0> ; kubectl apply -f <block 1>
clusterrole.rbac.authorization.k8s.io/k8rs-readonly created
clusterrole.rbac.authorization.k8s.io/k8rs-admin created
```

Groups and what they are bound to:

| group | bound to |
|---|---|
| `k8rs-ro` | `k8rs-readonly` |
| `k8rs-rw` | `k8rs-readonly` + `k8rs-admin` |
| `k8rs-noscale` | one rule: `apps` / `deployments` / `get,patch` — **no** `deployments/scale` |
| `k8rs-wild` | one rule: `apiGroups: ["*"] resources: ["*"] verbs: ["*"]` |
| `k8rs-named` | `pods delete` with `resourceNames: [only-this-pod]`; `configmaps get` with `resourceNames: ["*"]` |
| `system:nodes` (as `system:node:review-control-plane`) | the cluster's own, untouched |

## 1. Measurement 1 — the default case

```
$ kubectl auth whoami
Username   kubernetes-admin
Groups     [kubeadm:cluster-admins system:authenticated]
Extra: authentication.kubernetes.io/credential-id   <elided>

$ kubectl get clusterrole system:basic-user -o jsonpath='{range .rules[*]}{.apiGroups} {.resources} {.verbs}{"\n"}{end}'
["authorization.k8s.io"] ["selfsubjectaccessreviews","selfsubjectrulesreviews"] ["create"]
["authentication.k8s.io"] ["selfsubjectreviews"] ["create"]

$ kubectl get clusterrolebinding system:basic-user -o jsonpath='{.roleRef.kind}/{.roleRef.name} -> {range .subjects[*]}{.kind}:{.name}{end}'
ClusterRole/system:basic-user -> Group:system:authenticated
```

Both reviews answer:

```
$ k8rs ops may-i delete nodes
k8rs: may this login delete nodes?
k8rs: yes — the cluster says this login is allowed to do that
exit=0

$ k8rs ops may-i list pods -n default
k8rs: may this login list pods in default?
k8rs: yes — the cluster says this login is allowed to do that
exit=0
```

## 2. Measurement 2 — the D160 condition

**The technique in `reports/2026-08-30-the-read-only-clusterrole-under-itself.md` § 2
does not reach this condition on v1.36.1.** An explicit `as-groups` no longer drops
the auto-added `system:authenticated`:

```
$ kubectl auth whoami --as=k8rs-probe --as-group=k8rs-ro -o jsonpath='{.status.userInfo.groups}'
["k8rs-ro","system:authenticated"]
$ kubectl auth whoami --as=k8rs-probe -o jsonpath='{.status.userInfo.groups}'
["system:authenticated"]
```

Naming `system:unauthenticated` in the group list does drop it, and nothing is
deleted to get there:

```
$ kubectl auth whoami --as=k8rs-probe --as-group=k8rs-ro --as-group=system:unauthenticated
error: the selfsubjectreviews API is not enabled in the cluster or you do not have permission to call it

$ kubectl auth can-i create selfsubjectaccessreviews.authorization.k8s.io --as=k8rs-probe --as-group=k8rs-ro --as-group=system:unauthenticated
Error from server (Forbidden): selfsubjectaccessreviews.authorization.k8s.io is forbidden:
  User "k8rs-probe" cannot create resource "selfsubjectaccessreviews" in API group "authorization.k8s.io" at the cluster scope
```

k8rs under that kubeconfig — the cluster-scoped question first, then the
namespaced one:

```
$ k8rs ops may-i delete nodes
k8rs: may this login delete nodes?
k8rs: k8rs tried to ask this cluster what this login is allowed to do — the cluster would not allow it:
  selfsubjectaccessreviews.authorization.k8s.io is forbidden: User "k8rs-probe" cannot create resource
  "selfsubjectaccessreviews" in API group "authorization.k8s.io" at the cluster scope. That is not a no —
  k8rs hides nothing and refuses nothing because of it, and the operation is still there to run
exit=2

$ k8rs ops may-i list pods -n default
k8rs: may this login list pods in default?
k8rs: k8rs tried to ask this cluster what this login is allowed to do — the cluster would not allow it:
  selfsubjectrulesreviews.authorization.k8s.io is forbidden: ... at the cluster scope. That is not a no —
  k8rs hides nothing and refuses nothing because of it, and the operation is still there to run
exit=2
```

The word `no` never appears as a verdict on either line. That the impersonation
reaches k8rs and not only kubectl, measured against the same question under two
identities:

```
$ KUBECONFIG=<admin>  k8rs ops may-i patch deployments.apps -n default   -> yes   exit=0
$ KUBECONFIG=<ro>     k8rs ops may-i patch deployments.apps -n default   -> no    exit=0
```

## 3. Measurement 3 — subresource, and what the `/` means to each tool

### 3a. Ground truth from the API server's own matcher

`SubjectAccessReview` posted as admin about the `k8rs-noscale` subject, whose one
rule is `patch deployments` and which has no `deployments/scale`:

```
$ kubectl create -f - <<< 'SubjectAccessReview{user: k8rs-probe, groups:[k8rs-noscale],
    resourceAttributes:{group: apps, resource: deployments, subresource: "",      namespace: default, verb: patch}}'
subresource=''      allowed=true  reason=RBAC: allowed by ClusterRoleBinding "k8rs-review-noscale" of ClusterRole "k8rs-review-noscale" to Group "k8rs-noscale"
subresource='scale' allowed=false reason=
```

A rule granting `patch deployments` does **not** grant `patch deployments/scale`.

### 3b. The two spellings through `kubectl auth can-i`

```
$ KUBECONFIG=<noscale> kubectl auth can-i patch deployments.apps/scale -n default
yes
$ KUBECONFIG=<noscale> kubectl auth can-i patch deployments.apps --subresource=scale -n default
no
```

### 3c. The `/` is a NAME to `kubectl`, a SUBRESOURCE to k8rs

Under `k8rs-named` (`delete pods`, `resourceNames: [only-this-pod]`), in `default`:

| string | `kubectl auth can-i delete <string>` | `k8rs ops may-i delete <string>` |
|---|---|---|
| `pods` | **no** | **yes** |
| `pods/only-this-pod` | **yes** | **no** |
| `pods/some-other-pod` | no | no |

### 3d. The head-to-head the brief asked for

`-n default` throughout, so k8rs is on its `may_i_in` + `Permits::may` path.

| role | question | `kubectl auth can-i` | `k8rs ops may-i` |
|---|---|---|---|
| rw | `patch deployments.apps` | yes | yes |
| rw | `patch deployments.apps/scale` | yes | yes |
| noscale | `patch deployments.apps` | yes | yes |
| noscale | `patch deployments.apps/scale` | **yes** | **no** |
| ro | `patch deployments.apps` | no | no |
| ro | `patch deployments.apps/scale` | no | no |
| wild | `patch deployments.apps` | yes | yes |
| wild | `patch deployments.apps/scale` | yes | yes |

The one disagreement is 3b/3c's: `kubectl` read `scale` as an object *name*.
Against § 3a's `SubjectAccessReview`, k8rs's answer is the API server's.

### 3e. The spellings an operator types, all under `k8rs-rw`, all `-n default`

| string | `kubectl auth can-i` | `k8rs ops may-i` |
|---|---|---|
| `delete pods` | yes | yes |
| `delete pod` | yes | **no** |
| `delete po` | yes | **no** |
| `patch deployments.apps` | yes | yes |
| `patch deployments` | yes | **no** |
| `patch deployment` | yes | **no** |
| `patch deploy` | yes | **no** |

### 3f. Wildcards

`k8rs-wild` = `apiGroups:["*"] resources:["*"] verbs:["*"]`:

| question | `kubectl` | `k8rs` |
|---|---|---|
| `delete nodes` (no `-n`) | yes | yes |
| `delete nodes -n default` | yes | yes |
| `create pods.something.invented -n default` | yes | yes |
| `frobnicate pods -n default` | yes | yes |

`resourceNames`, under `k8rs-named`:

| question | `kubectl` | `k8rs` |
|---|---|---|
| `delete pods -n default` | no | **yes** |
| `delete pods` (no `-n`, so k8rs is on the `SelfSubjectAccessReview` path) | no | **no** |
| `get configmaps -n default` (rule has `resourceNames: ["*"]`) | no (`No Object name found`) | **yes** |
| `get configmaps/* -n default` | **yes** | **no** |

## 4. An authorizer the rules review cannot enumerate

kind runs `--authorization-mode=Node,RBAC`. Under `system:node:review-control-plane`
in group `system:nodes`:

```
$ SelfSubjectAccessReview {group:"", resource: nodes, name: review-control-plane, verb: get}
allowed=true denied= evalErr=

$ SelfSubjectRulesReview {namespace: kube-system}
incomplete= True   evaluationError= 'node authorizer does not support user rule resolution'
  rule: ['certificates.k8s.io'] ['certificatesigningrequests/selfnodeclient'] ['create'] None
  rule: ['authorization.k8s.io'] ['selfsubjectaccessreviews','selfsubjectrulesreviews'] ['create'] None
  rule: ['authentication.k8s.io'] ['selfsubjectreviews'] ['create'] None
  rule: [''] ['configmaps'] ['get'] ['kubeadm-config']
  rule: [''] ['configmaps'] ['get'] ['kubelet-config']
```

Through k8rs, same identity, `-n kube-system`:

```
$ k8rs ops may-i get nodes -n kube-system
k8rs: this cluster could not work the whole answer out: node authorizer does not support user rule
  resolution. That is not a no — ...                                                   exit=2
$ k8rs ops may-i delete namespaces -n kube-system
k8rs: this cluster could not work the whole answer out: node authorizer does not support user rule
  resolution. That is not a no — ...                                                   exit=2
$ k8rs ops may-i get configmaps -n kube-system
k8rs: yes — the cluster says this login is allowed to do that                          exit=0
```

For comparison, `kubectl` on the same identity: `get nodes` → `no - node
'review-control-plane' cannot read all nodes, only its own Node object`;
`get configmaps` → `no - No Object name found`.

## 5. Measurement 4 — the RBAC grant, before and after

Same `unauth` identity as § 2, with one extra ClusterRole bound to its group:

```yaml
- apiGroups: ["authorization.k8s.io"]
  resources: ["selfsubjectrulesreviews", "selfsubjectaccessreviews"]
  verbs: ["create"]
```

```
### BEFORE
$ k8rs ops may-i list pods -n default   -> k8rs tried to ask this cluster ... the cluster would not allow it
$ k8rs ops may-i delete nodes           -> k8rs tried to ask this cluster ... the cluster would not allow it
### AFTER
$ k8rs ops may-i list pods -n default   -> yes — the cluster says this login is allowed to do that   exit=0
$ k8rs ops may-i delete nodes           -> no — the cluster says this login is not allowed to do that
```

## 6. Exit codes and the refusals

```
exit=2 : k8rs ops may-i list pods -n default          (dead socket, kubeconfig server -> 127.0.0.1:6599)
exit=2 : k8rs ops may-i list pods -n default          (KUBECONFIG=/nonexistent/kubeconfig)
exit=2 : k8rs --read-only ops may-i list pods -n default
exit=2 : k8rs ops may-i list pods --context kind-review -n default
exit=2 : k8rs ops may-i
exit=2 : k8rs ops may-i list
exit=2 : k8rs ops may-i list pods extra -n default
exit=2 : k8rs ops may-i list pods/ -n default
exit=2 : k8rs ops may-i list .apps -n default
exit=2 : k8rs ops may-i list pods -n "NOT A NAMESPACE"
exit=0 : k8rs ops may-i list pods -n default          (yes)
exit=0 : k8rs ops may-i delete pods -n default        (no)
exit=0 : k8rs ops may-i list pods --namespace default
```

The sentences those refusals carry:

```
$ k8rs --read-only ops may-i list pods -n default
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation

$ k8rs ops may-i list pods --context kind-review -n default
k8rs: --context is not a flag `k8rs ops` has — the only one it takes is -n or --namespace, which says
  which namespace the object is in

$ KUBECONFIG=<current-context: does-not-exist> k8rs ops may-i list pods -n default
k8rs: nothing was changed — this kubeconfig has no such context — check the `--context` you gave,
  or the `current-context` line in the file
```

Fault taxonomy on the probe, one line each:

| condition | the clause after `k8rs tried to ask this cluster what this login is allowed to do —` |
|---|---|
| 403 on the review | `the cluster would not allow it: <server's own sentence>` |
| junk bearer token | `the login k8rs was using had run out: Unauthorized` |
| nothing listening on the port | `k8rs could not reach the cluster` |
| host that does not resolve | `k8rs could not reach the cluster` |

## 7. Invariant 9, bounds, and the state directory

```
$ k8rs ops may-i list $'pods\033[2J\033]0;pwned\007' -n default        | cat -v
k8rs: may this login list pods[2J]0;pwned (with what cannot print removed) in default?

$ k8rs ops may-i list $'pods\nfake: line' -n default                   | cat -v
k8rs: may this login list podsfake: line (with what cannot print removed) in default?

$ k8rs ops may-i list <600 'a' characters> -n default
k8rs: may this login list aaa…  (truncated at k8s::NAME_MAX)
```

```
$ XDG_STATE_HOME=<fresh> k8rs ops may-i list pods -n default ; find <fresh> -mindepth 1
(nothing)
$ echo no | XDG_STATE_HOME=<same> k8rs ops delete pod/nothing -n default ; find <fresh> -mindepth 1
<fresh>/k8rs
<fresh>/k8rs/audit.log
-rw------- 1 ... 498 audit.log
```

## 8. D23's scenario, reproduced live

A real pod, and an identity with no `delete pods`:

```
$ printf 'probe-pod\n' | KUBECONFIG=<ro> k8rs ops delete pod/probe-pod -n default
pod/probe-pod in default
This removes the pod. Whatever created it will normally replace it — k8rs has not checked whether anything did.
$ kubectl delete pod/probe-pod -n default
k8rs did not check this one with the cluster first
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: nothing was changed — the cluster would not allow it: pods "probe-pod" is forbidden:
  User "k8rs-probe" cannot delete resource "pods" in API group "" in the namespace "default"
exit=2

$ KUBECONFIG=<ro> k8rs ops may-i delete pods -n default
k8rs: no — the cluster says this login is not allowed to do that
```

The eight words `the cluster would not allow it` appear in both records: in the
`delete` refusal above they are about the delete, and in § 2's `CouldNotTell` they
are about the review.

## 9. Teardown

```
$ K8RS_CLUSTER=review scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-control-plane"]

$ kind get clusters
k8rs

$ docker ps --format '{{.Names}}\t{{.Image}}'
k8rs-worker3         kindest/node:v1.36.1
k8rs-worker          kindest/node:v1.36.1
k8rs-control-plane   kindest/node:v1.36.1
k8rs-worker2         kindest/node:v1.36.1

$ ps -eo args | grep -E "sleep 3000|kind delete"
(none — the teardown watchdog this run armed had already been stopped)
```

The PM's `k8rs` is the only cluster left and its four containers are the only ones
running. Every impersonating kubeconfig this run wrote was mode 0600 outside the
repo and was deleted with the cluster; nothing from it entered `tests/` or `git`.
