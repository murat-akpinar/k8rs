# Namespace scoping measured under a real RBAC-limited kubeconfig — 2026-08-29

`k8s-admin`, operator review of the namespace-scoping box (todo.md § Phase 5).
Every 403 in that box's own tests is simulated; these runs are against a real
API server with a real `Role`, which is the one thing it could not be proven
against.

## The cluster, and what was done to it

The previous review's cluster was still present and stopped. It was **restarted
rather than recreated**, used, and **deleted** at the end.

```
$ docker ps -a --format '{{.Names}}|{{.Status}}|{{.Image}}'
k8rs-review-worker|Exited (130) 5 hours ago|kindest/node:v1.36.1
k8rs-review-control-plane|Exited (137) 5 hours ago|kindest/node:v1.36.1
```

```
$ kind delete cluster --name k8rs-review
Deleting cluster "k8rs-review" ...
delete exit=0
$ kind get clusters
k8rs
```

The PM's fixture cluster was running throughout and was neither read nor
written. Two clusters were therefore up at once for ~40 minutes; no capture was
running beside it (this review is the gate the PM is blocked on).

Server `v1.36.1`. Binary: `target/debug/k8rs`, rebuilt from the working tree
(`Finished dev profile ... in 43.44s`).

### The principals

Built by `kubectl create token` into throwaway kubeconfigs under `$HOME`, mode
0600, deleted after the run. Tokens are not reproduced here.

| Principal | Grant |
|---|---|
| `dev` | namespaced `Role` in `payments`: `get,list,watch` on pods, deployments, statefulsets, daemonsets, replicasets |
| `getonly` | namespaced `Role` in `payments`: `get` on pods only |
| `nonodes` | `ClusterRole`: cluster-wide `get,list,watch` on pods/services/pvc/deployments/statefulsets/daemonsets/replicasets/endpointslices/pdb — **no nodes** |

```
$ KUBECONFIG=.../dev.kubeconfig kubectl auth can-i list pods --all-namespaces
no
$ KUBECONFIG=.../dev.kubeconfig kubectl auth can-i list pods -n payments
yes
$ KUBECONFIG=.../dev.kubeconfig kubectl auth can-i list nodes
Warning: resource 'nodes' is not namespace scoped

no
$ KUBECONFIG=.../dev.kubeconfig kubectl get pods -n payments
NAME                      READY   STATUS             RESTARTS   AGE
broken-...                0/1     ImagePullBackOff   0          24s
web-...                   1/1     Running            0          24s
web-...                   1/1     Running            0          24s
```

## R1 — namespaced `Role`, no `--namespace`, context names no namespace

```
$ KUBECONFIG=.../dev.kubeconfig timeout 25 ./target/debug/k8rs --live
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
k8rs: the role this kubeconfig uses needs to `list` pods across the whole cluster — so k8rs is watching one namespace instead: default. Pass --namespace <name> for a different one, or ask for cluster-wide read access
```

last report printed before the timeout, stdout:

```
▲ k8rs is not getting pods from this cluster: the role this kubeconfig uses needs to `list` and `watch` pods. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting nodes from this cluster: ... `list` and `watch` nodes ...
▲ k8rs is not getting Deployments from this cluster: ... `list` and `watch` deployments ...
▲ k8rs is not getting StatefulSets from this cluster: ... `list` and `watch` statefulsets ...
▲ k8rs is not getting DaemonSets from this cluster: ... `list` and `watch` daemonsets ...

ns: default · 0 pods · 0 nodes

○ nothing is broken

One node check is off: spotting a node someone started emptying and did not finish needs every pod in the cluster.
```

Field values the finding turns on: header `ns: default · 0 pods · 0 nodes`;
claim line `○ nothing is broken`; actual cluster state in `payments` at that
moment — 3 pods, 1 in `ImagePullBackOff`.

## R2 — same kubeconfig, `--namespace payments`

```
$ KUBECONFIG=.../dev.kubeconfig timeout 25 ./target/debug/k8rs --live --namespace payments
▲ k8rs is not getting nodes from this cluster: the role this kubeconfig uses needs to `list` and `watch` nodes. It keeps asking, and until that works nothing here about them can be trusted

ns: payments · 3 pods · 0 nodes

● payments/broken-...
  Container image is not usable, so the container never started (ImagePullBackOff)
  [evidence line, verbatim controller message, trimmed here]
  → check the image name and tag, whether this namespace has a pull secret for that registry, and whether the pull policy lets the node fetch it at all

1 critical

One node check is off: spotting a node someone started emptying and did not finish needs every pod in the cluster.
```

## R3 — the fallback where the context **does** name its namespace

`kubectl config set-context review --namespace=payments`, then no flag:

```
$ KUBECONFIG=.../dev-ns.kubeconfig timeout 25 ./target/debug/k8rs --live
k8rs: the role this kubeconfig uses needs to `list` pods across the whole cluster — so k8rs is watching one namespace instead: payments. Pass --namespace <name> for a different one, or ask for cluster-wide read access

ns: payments · 3 pods · 0 nodes

● payments/broken-...   (ImagePullBackOff)
● payments/broken · 4 min ago   (This rollout gave up ...)

2 critical

One node check is off: ...
```

## R4 — three more scopes that produce nothing readable

Same last four lines in every case: header, `○ nothing is broken`, the
node-check line.

| Run | Header printed |
|---|---|
| `dev` + `--namespace kube-system` | `ns: kube-system · 0 pods · 0 nodes` |
| `dev` + `--namespace nosuchns` | `ns: nosuchns · 0 pods · 0 nodes` |
| `getonly` (`get` only) + `--namespace payments` | `ns: payments · 0 pods · 0 nodes` |

## R5 — cluster-wide reader that cannot list nodes

```
$ KUBECONFIG=.../nonodes.kubeconfig timeout 25 ./target/debug/k8rs --live
▲ k8rs is not getting nodes from this cluster: the role this kubeconfig uses needs to `list` and `watch` nodes. ...

17 pods · 0 nodes

● payments/broken-...
● payments/broken · 3 min ago
▲ default/web-... · 5 hours ago  (Terminating, held by a finalizer)

2 critical, 1 warning
```

No `ns:` fragment and no *check is off* line — `namespace_scope` is `None` on
this shape. `--analysis` on the same principal:

```
[capacity]
  Not checked. Reading what a node has needs permission to list nodes, and this login does not have it.
  Ask for permission to list nodes across the whole cluster.
[drain safety]
  Not checked. This report answers one question per node, and this login cannot list the nodes.
[versions]
  Control plane v1.36.1
  Which machines are behind is not checked. That needs the list of nodes, and this login cannot read it.
```

## R6 — the retry interval under a standing refusal

Counter: `authorization_attempts_total{result="no-opinion"}` read from the
control plane's `/metrics` (the union authorizer's denial result). One refused
watch (`nodes`), `--namespace payments`, so the other four watches are healthy.

```
== BEFORE ==   authorization_attempts_total{result="no-opinion"} 130    t=1788029740
== AFTER 60s == authorization_attempts_total{result="no-opinion"} 136   t=1788029801
```

```
== BEFORE ==    authorization_attempts_total{result="no-opinion"} 136   t=1788029854
== AFTER 180s == authorization_attempts_total{result="no-opinion"} 145  t=1788030034
```

6 denials in 61 s (the ramp), 9 in 180 s. The 3 denials after the ramp are
~43 s apart, inside `StandingBackoff`'s documented 30–60 s plateau.

## R7 — a refused watch recovers when the grant arrives mid-run

k8rs started at `22:04:18` with `--namespace payments`; `nodes` refused.

```
[22:04:58] granting node read to the dev SA
clusterrole.rbac.authorization.k8s.io/nodereader created
clusterrolebinding.rbac.authorization.k8s.io/nodereader created
[22:06:48] k8rs exited
```

Reports in order, header line only:

```
ns: payments · 3 pods · 0 nodes      (▲ nodes line present)
ns: payments · 3 pods · 0 nodes      (▲ nodes line present)
ns: payments · 3 pods · 2 nodes      (▲ nodes line gone)
ns: payments · 3 pods · 2 nodes
```

The process was not restarted. The grant landed at 22:04:58; the third report
is the first with a node count and no `▲ nodes` line.

## R8 — the `nonResourceURL` refusal on `/apis`

`system:discovery` and `system:basic-user` ClusterRoleBindings deleted, run,
then restored (`restored: kubectl get --raw /apis exit=0`).

```
$ KUBECONFIG=.../dev.kubeconfig kubectl get --raw /apis
Error from server (Forbidden): forbidden: User "system:serviceaccount:payments:dev" cannot get path "/apis"
```

```
$ KUBECONFIG=.../dev-ns.kubeconfig timeout 20 ./target/debug/k8rs --live
k8rs: watching — server v1.36.1 · could not list what this cluster serves, so k8rs cannot show you what is in it or tell which add-ons it has (the role this kubeconfig uses needs to `get /apis`)
k8rs: the role this kubeconfig uses needs to `list` pods across the whole cluster — so k8rs is watching one namespace instead: payments. ...

ns: payments · 3 pods · 2 nodes

● payments/broken-...   (ImagePullBackOff)
```

The sentence names the path. The session continued and produced findings.

## R9 — the six on-demand report lists under a scope

The `dev` `Role` was widened to grant `get,list,watch` on services,
persistentvolumeclaims, endpointslices and poddisruptionbudgets **in
`payments`**, and two wasteful objects were created there.

```
$ KUBECONFIG=.../dev.kubeconfig kubectl get services -A
Error from server (Forbidden): services is forbidden: User "system:serviceaccount:payments:dev" cannot list resource "services" in API group "" at the cluster scope
$ KUBECONFIG=.../dev.kubeconfig kubectl get services -n payments
NAME     TYPE        CLUSTER-IP   EXTERNAL-IP   PORT(S)   AGE
orphan   ClusterIP   <elided>     <none>        80/TCP    1s
$ KUBECONFIG=.../dev.kubeconfig kubectl get pvc -n payments
NAME     STATUS    VOLUME   CAPACITY   ACCESS MODES   STORAGECLASS   AGE
unused   Pending                                      standard       1s
```

k8rs, same principal, `--analysis --namespace payments`:

```
[waste]
  Things in payments that cost you something for nothing
  Not checked. Working out what is going to waste needs the lists of what this cluster has — its Services, the addresses behind them, the disk reservations and the replicasets — and this login could not read any of them.
  Ask for permission to list services, endpointslices, persistentvolumeclaims and replicasets.
```

The admin kubeconfig, same namespace, same objects, same flag:

```
[waste]
  Things in payments that cost you something for nothing
  ● payments/orphan matches no pod
      This Service points at nothing. Anything calling it gets a 503.
      → fix its selector, or delete it
  ○ 1 replicaset is parked at 0 replicas
```

## R10 — `--namespace` value handling

Exit codes measured directly (`$?` of the binary, not of a pipeline):

```
$ ./target/debug/k8rs --live --namespace ../secrets  ; echo $?
k8rs: --namespace needs the name of a namespace, and ../secrets is not one
2
$ ./target/debug/k8rs --live --namespace ; echo $?
k8rs: --namespace needs the name of a namespace
2
$ ./target/debug/k8rs --live --nope ; echo $?     # baseline
2
```

Accepted shapes: `--namespace NAME`, `--namespace=NAME`, `-n NAME`, `-n=NAME`.

`-npayments`, attached short form, is **not refused and not read**:

```
$ KUBECONFIG=.../dev-ns.kubeconfig ./target/debug/k8rs --live -npayments
k8rs: the role this kubeconfig uses needs to `list` pods across the whole cluster — so k8rs is watching one namespace instead: payments. ...
ns: payments · 3 pods · 2 nodes
```

The scope came from the kubeconfig context's own `namespace:`, via the 403
fallback. The word `-npayments` reached neither `namespace_arg` nor `mistyped`.

Two values that cannot be a Kubernetes namespace name pass `path_safe` and are
accepted. Against a principal that can list pods cluster-wide:

```
$ ... k8rs --live --namespace PAYMENTS
ns: PAYMENTS · 0 pods · 0 nodes

○ nothing is broken

$ ... k8rs --live --namespace foo.bar
ns: foo.bar · 0 pods · 0 nodes

○ nothing is broken
```

What the API server answers on those two paths:

```
$ kubectl get --raw "/api/v1/namespaces/PAYMENTS/pods?limit=1"
{"kind":"PodList","apiVersion":"v1","metadata":{"resourceVersion":"5827"},"items":[]}
$ kubectl get --raw "/api/v1/namespaces/foo.bar/pods?limit=1"
{"kind":"PodList","apiVersion":"v1","metadata":{"resourceVersion":"5827"},"items":[]}
```

## Not measured

* A 403 whose body is JSON that is not a `Status` (an authorizing proxy). No
  proxy was stood up; `k8s.rs`'s `answer` doc carries the 2026-08-27 run.
* Behaviour at cluster size. Nothing here exceeded 17 pods.
* `just check`. Not run — this was a cluster measurement, not a gate.
