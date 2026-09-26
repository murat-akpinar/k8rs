# The three RBAC pairs a refused key names, against a real cluster

`k8s-admin`, 2026-09-12. Subject: the uncommitted Phase 11 box that marks a refused
key — `src/ui.rs::key_map`'s three clauses (`patch <plural>/scale`,
`patch <plural>`, `delete <plural>`), `src/views.rs::Refused`, and
`screens/help.md` § *When a key is refused* — read against the API that has to
agree with them.

Method starts where `reports/2026-09-05-may-i-against-a-real-cluster.md` left off:
the same impersonation technique, the same `SubjectAccessReview`-as-ground-truth,
the same side-by-side with `kubectl auth can-i`.

Cluster: **ephemeral**, `K8RS_CLUSTER=review`, `K8RS_WORKERS=0`,
`kindest/node:v1.36.1`, API on `127.0.0.1:6444`. Built and destroyed inside this
run; teardown is § 9. The PM's fixture cluster `k8rs` was running beside it and was
never addressed. `kubectl` v1.36.4. Binary: `cargo build` over a copy of the
working tree with this box's uncommitted diff, with its own `CARGO_TARGET_DIR`
outside the repo. Identities are `k8rs-probe` in one of eight groups, each bound to
one ClusterRole written for this run; nothing pre-existing in the cluster was
edited.

## 1. Which resources have a `scale` subresource, from the API's own discovery

```
$ kubectl get --raw /apis/apps/v1        # one line per resource, trimmed to the columns that matter
controllerrevisions      namespaced=True  verbs=create,delete,deletecollection,get,list,patch,update,watch
daemonsets               namespaced=True  verbs=create,delete,deletecollection,get,list,patch,update,watch
daemonsets/status        namespaced=True  verbs=get,patch,update
deployments              namespaced=True  verbs=create,delete,deletecollection,get,list,patch,update,watch
deployments/scale        namespaced=True  verbs=get,patch,update
deployments/status       namespaced=True  verbs=get,patch,update
replicasets              namespaced=True  verbs=create,delete,deletecollection,get,list,patch,update,watch
replicasets/scale        namespaced=True  verbs=get,patch,update
replicasets/status       namespaced=True  verbs=get,patch,update
statefulsets             namespaced=True  verbs=create,delete,deletecollection,get,list,patch,update,watch
statefulsets/scale       namespaced=True  verbs=get,patch,update
statefulsets/status      namespaced=True  verbs=get,patch,update

$ kubectl get --raw /api/v1              # same, nodes and pods only
nodes                    namespaced=False verbs=create,delete,deletecollection,get,list,patch,update,watch
pods                     namespaced=True  verbs=create,delete,deletecollection,get,list,patch,update,watch
```

`daemonsets` carries no `/scale`; the other three workload kinds do, and each lists
**`get`** beside `patch`. `nodes` is `namespaced=False`.

```
$ kubectl get --raw '/apis/apps/v1/namespaces/default/daemonsets/bare-ds/scale'
Error from server (NotFound): the server could not find the requested resource

$ k8rs ops scale daemonset/bare-ds 3 -n default
k8rs: k8rs cannot scale a daemonset — scaling changes how many copies are running, and k8rs
  does that for a deployment, a statefulset and a replicaset
exit=2
```

## 2. `patch <plural>` does not reach `<plural>/scale` — re-measured

`SubjectAccessReview` posted as admin about a subject whose one rule is
`apps` / `deployments/scale` / `patch`, and about one whose one rule is
`apps` / `deployments` / `patch`:

```
rev-scale-patch-only | patch deployments/scale  ->  allowed=True
rev-scale-patch-only | get   deployments/scale  ->  allowed=False
rev-scale-patch-only | patch deployments        ->  allowed=False
rev-scale-full       | get   deployments/scale  ->  allowed=True
rev-scale-full       | patch deployments/scale  ->  allowed=True
rev-restart          | patch deployments        ->  allowed=True
rev-restart          | patch deployments/scale  ->  allowed=False
rev-restart          | get   deployments        ->  allowed=False
```

A rule spelled `*/scale` is honoured by the API server and read the same way by
k8rs:

```
SAR patch deployments/scale under a rule granting apps `*/scale` -> allowed=True
$ KUBECONFIG=<star-scale> k8rs ops may-i patch deployments.apps --subresource=scale -n default
k8rs: yes — this login is allowed to do that                                        exit=0
```

## 3. `patch <plural>/scale` alone does not scale — neither through k8rs nor through the taught line

Login: `apps` / `deployments/scale` / **`patch`** and nothing else — exactly the
pair `screens/help.md` names as the reason `s` is refused.

```
$ KUBECONFIG=<patch-only> k8rs ops may-i patch deployments.apps --subresource=scale -n default
k8rs: may this login patch deployments.apps (subresource: scale) in default?
k8rs: yes — this login is allowed to do that                                        exit=0

$ echo y | KUBECONFIG=<patch-only> k8rs ops scale deployment/web 3 -n default
k8rs: k8rs could not read how many copies of deployment/web in default are running right now
  — the cluster would not allow it: deployments.apps "web" is forbidden: User "k8rs-probe"
  cannot get resource "deployments/scale" in API group "apps" in the namespace "default"
exit=2

$ KUBECONFIG=<patch-only> kubectl scale deployment/web --replicas=4 -n default
Error from server (Forbidden): deployments.apps "web" is forbidden: User "k8rs-probe"
  cannot get resource "deployments" in API group "apps" in the namespace "default"
exit=1
```

The other direction — the login the screen would draw `s no scale` for, pressing
`s` anyway. The screen's clause names `patch`; the wall names `get`:

```
$ KUBECONFIG=<restart-only> k8rs ops may-i patch deployments.apps --subresource=scale -n default
k8rs: no — this login is not allowed to do that                                     exit=1

$ echo yes | KUBECONFIG=<restart-only> k8rs ops scale deployment/web 5 -n default
k8rs: k8rs could not read how many copies of deployment/web in default are running right now
  — the cluster would not allow it: ... cannot get resource "deployments/scale" ...
exit=2
```

With `get` **and** `patch` on the subresource, the same call reaches its dialog:

```
$ echo y | KUBECONFIG=<get+patch> k8rs ops scale deployment/web 3 -n default
deployment/web in default
This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies.
$ kubectl scale deployment/web --replicas=3 -n default
the cluster checked it first and accepted it
```

`kubectl scale` is a `patch` on the subresource too, so the pair does not diverge
from the taught line on the verb — a login with `get,update` and no `patch` is
refused by both:

```
$ KUBECONFIG=<get+update> kubectl scale deployment/web --replicas=4 -n default
Error from server (Forbidden): ... cannot patch resource "deployments/scale" ...   exit=1
$ echo yes | KUBECONFIG=<get+update> k8rs ops scale deployment/web 6 -n default
$ kubectl scale deployment/web --replicas=6 -n default
k8rs: the change was never sent — the cluster would not allow it: ... cannot patch
  resource "deployments/scale" ...                                                  exit=2
```

## 4. `patch <plural>` is correct and sufficient for restart

Login: `apps` / `deployments` / **`patch`** only — no `get` on anything.

```
$ echo yes | KUBECONFIG=<restart-only> k8rs ops restart deployment/web -n default
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made                                                           exit=0

$ kubectl get deploy web -o jsonpath='{.spec.template.metadata.annotations}'
{"kubectl.kubernetes.io/restartedAt":"<RFC3339 stamp>"}
```

And the refusal, for a login without it — the message names the same pair the
screen does:

```
$ KUBECONFIG=<scale-only> k8rs ops may-i patch deployments.apps -n default
k8rs: no — this login is not allowed to do that                                     exit=1
$ echo yes | KUBECONFIG=<scale-only> k8rs ops restart deployment/web -n default
k8rs: the change was never sent — the cluster would not allow it: deployments.apps "web"
  is forbidden: User "k8rs-probe" cannot patch resource "deployments" in API group "apps"
  in the namespace "default"                                                        exit=2
```

## 5. `delete <plural>` is correct and sufficient for delete

Login: `apps` / `deployments` / **`delete`** only.

```
$ echo doomed | KUBECONFIG=<delete-only> k8rs ops delete deployment/doomed -n default
k8rs did not check this one with the cluster first
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: the change was made                                                           exit=0

$ kubectl get deploy doomed
Error from server (NotFound): deployments.apps "doomed" not found                    exit=1
```

Cluster-scoped, for the node arm — the answer does not move when a namespace is
bolted onto a cluster-scoped question:

```
$ KUBECONFIG=<node-delete> k8rs ops may-i delete nodes.
k8rs: may this login delete nodes?      k8rs: yes — this login is allowed to do that  exit=0
$ KUBECONFIG=<node-delete> k8rs ops may-i delete nodes. -n default
k8rs: may this login delete nodes in default?  k8rs: yes ...                          exit=0
$ KUBECONFIG=<no-node-grant> k8rs ops may-i delete nodes.
k8rs: no — this login is not allowed to do that                                       exit=1
$ KUBECONFIG=<node-delete> kubectl auth can-i delete nodes
Warning: resource 'nodes' is not namespace scoped
yes
```

## 6. What the screen's own string does in the two tools an operator checks it with

The string on screen is `patch deployments/scale`. The login below has
`patch deployments` and no `deployments/scale`, so k8rs's verdict is `no`:

```
$ KUBECONFIG=<restart-only> k8rs ops may-i patch deployments.apps --subresource=scale -n default
k8rs: no — this login is not allowed to do that                                       exit=1

$ KUBECONFIG=<restart-only> kubectl auth can-i patch deployments/scale -n default
yes                                                                                   exit=0
$ KUBECONFIG=<restart-only> kubectl auth can-i patch deployments.apps/scale -n default
yes                                                                                   exit=0
$ KUBECONFIG=<restart-only> kubectl auth can-i patch deployments.apps --subresource=scale -n default
no                                                                                    exit=1

$ KUBECONFIG=<restart-only> k8rs ops may-i patch deployments.apps/scale -n default
k8rs: may this login patch deployments.apps/scale in default?
k8rs: yes — this login is allowed to do that                                          exit=0
```

The bare plural the screen prints is refused by k8rs's own question command:

```
$ k8rs ops may-i patch deployments/scale -n default
k8rs: `ops may-i` needs the API group as well as the resource, because it cannot look one
  up — write `deployments.apps`, or `pods.` with nothing after the dot for the core group
  that `pods`, `nodes` and `services` are in                                          exit=2
$ k8rs ops may-i delete deployments -n default        (same sentence)                 exit=2
$ k8rs ops may-i delete nodes                         (same sentence)                 exit=2
```

## 7. The two kind-set exclusions, as Kubernetes behaves

A bare ReplicaSet's `scale` is real — k8rs moved it:

```
$ echo yes | k8rs ops scale replicaset/bare-rs 3 -n default
$ kubectl scale replicaset/bare-rs --replicas=3 -n default
k8rs: the change was made
$ kubectl get rs bare-rs --no-headers
bare-rs   3     3     3     25s
```

A bare ReplicaSet's *restart* is not — the annotation lands and nothing is
replaced, and kubectl refuses the same operation outright:

```
$ kubectl get pods -l app=bare-rs -o custom-columns=NAME:.metadata.name,AGE:.metadata.creationTimestamp
bare-rs-<a>   2026-09-12T00:37:34Z
bare-rs-<b>   2026-09-12T00:37:14Z
bare-rs-<c>   2026-09-12T00:37:14Z

$ kubectl patch rs bare-rs --type=strategic \
    -p '{"spec":{"template":{"metadata":{"annotations":{"kubectl.kubernetes.io/restartedAt":"…"}}}}}'
replicaset.apps/bare-rs patched

  (20 s later — the same three pods, the same three creationTimestamps)
bare-rs-<a>   2026-09-12T00:37:34Z
bare-rs-<b>   2026-09-12T00:37:14Z
bare-rs-<c>   2026-09-12T00:37:14Z

$ kubectl rollout restart rs/bare-rs
error: replicasets.apps "bare-rs" restarting is not supported                          exit=1
```

## 8. The column counts the screen files state, counted

```
$ python3 -c "print(len(row))"   # per row, the strings src/ui.rs::key_map builds
  76  s      refused, deployments        76  r      refused, deployments
  77  s      refused, statefulsets       77  r      refused, statefulsets
  70  ctrl-d refused, deployments        71  ctrl-d refused, statefulsets
  76  s      refused, deployments, with the clause reading `get+patch deployments/scale`
      and the opening `(` moved left 4 columns (77 for statefulsets)

  65  footer, neither refused            68  footer, s refused
  68  footer, r refused                  71  footer, both refused
```

Body ceiling at the 80×24 floor is 78 (`src/ui_tests.rs`'s `MIN_WIDTH - 2`);
footer ceiling is 76.

The two frames the box draws, from the tests
(`cargo test the_refused_key_map_is_what_the_help_screen_draws -- --nocapture`
and `cargo test every_footer_on_the_screen_it_belongs_to -- --nocapture`):

```
│  Changing things (each one asks first, and shows the command)                │
│    s       run more or fewer copies       (scale — patch deployments/scale)  │
│    r       restart, at its own pace   (rollout restart — patch deployments)  │
│    ctrl-d  delete — you type the name to confirm (delete deployments)        │
```

```
│ ↑↓ move  ⏎ open  s no scale  r no restart  / filter  ? all keys  q quit      │
```

## 9. A namespaced rules review lists cluster-scoped grants

`SelfSubjectRulesReview` for `default`, as the login whose only ClusterRole grants
`nodes` `delete,get,list` plus the two review verbs:

```
incomplete = False   evaluationError = None
  apiGroups=['']                      resources=['nodes']   verbs=['delete','get','list']  names=None
  apiGroups=['authorization.k8s.io']  resources=['selfsubjectaccessreviews','selfsubjectrulesreviews']  verbs=['create']
  apiGroups=['authorization.k8s.io']  resources=['selfsubjectaccessreviews','selfsubjectrulesreviews']  verbs=['create']
  apiGroups=['authentication.k8s.io'] resources=['selfsubjectreviews']  verbs=['create']
```

## 10. Teardown

```
$ K8RS_CLUSTER=review scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: [<the one control-plane node>]

$ kind get clusters
k8rs

$ docker ps --format '{{.Names}}\t{{.Image}}'
k8rs-worker3         kindest/node:v1.36.1
k8rs-worker          kindest/node:v1.36.1
k8rs-control-plane   kindest/node:v1.36.1
k8rs-worker2         kindest/node:v1.36.1

$ kubectl config get-contexts -o name ; kubectl config current-context
kind-k8rs
kind-k8rs

$ ps -eo args | grep -E "[s]leep 3000|[k]ind delete"
(none — the teardown watchdog this run armed was stopped, and its orphaned `sleep`
 killed by pid)
```

`kind` writes the shared `~/.kube/config` and switches `current-context` on
create; the context above was restored by hand after the delete. Every
impersonating kubeconfig this run wrote was mode 0600 outside the repo and was
deleted with the cluster; the scratch build tree and the binary were deleted too.
Nothing from this run entered `tests/` or `git`.

**One process note.** The scratchpad is on this box's 12 GiB `/tmp` tmpfs, and a
`cargo build` of a copy of the tree into it filled the volume mid-run
([D133](../NOTES.md#d133--the-mutation-gate-files-a-failed-build-as-unviable-so-a-full-disk-reads-as-a-pass-2026-08-21)'s
volume, the same 12 GiB). The build's `CARGO_TARGET_DIR` belongs outside `/tmp`.

---

# Round 2 — is `get+patch <plural>/scale` *sufficient*, and for all three kinds

`k8s-admin`, 2026-09-12, second cluster of the day. Round 1 above measured that
`patch` alone is **not enough**. This round measures the other half, which nothing
had: that the pair the rewritten clause names is enough, and that there is no third
verb and no second resource in `ops::scale`'s path.

Subject: the landed tree — `src/views.rs`'s `Refused::SCALE_VERBS = ["get","patch"]`
with `Refused::of` taking one array of answers per operation, and `src/ui.rs`'s
`key_map` interpolating `SCALE_VERBS.join("+")` into the clause.

Same conditions as round 1: **ephemeral**, `K8RS_CLUSTER=review`, `K8RS_WORKERS=0`,
`kindest/node:v1.36.1`, API on `127.0.0.1:6444`, the PM's `k8rs` running beside it
and never addressed. Binary built from a copy of the landed working tree with its
own `CARGO_TARGET_DIR` **under `$HOME`, not under `/tmp`** — round 1's tmpfs
lesson. Teardown is § R2.7.

## R2.1 — the clause as the landed code draws it

```
$ cargo test the_refused_key_map_is_what_the_help_screen_draws -- --nocapture
│  Changing things (each one asks first, and shows the command)                │
│    s       run more or fewer copies   (scale — get+patch deployments/scale)  │
│    r       restart, at its own pace   (rollout restart — patch deployments)  │
│    ctrl-d  delete — you type the name to confirm (delete deployments)        │
```

## R2.2 — the pair is sufficient, on all three kinds `ops::scalable` serves

Three ClusterRoles, one per kind, each holding **one** rule and nothing else:
`apiGroups: ["apps"]`, `resources: ["<plural>/scale"]`, `verbs: ["get","patch"]`.
Reached by impersonation; no other binding was added to any of the three groups.

```
$ echo yes | KUBECONFIG=<dep-scale> k8rs ops scale deployment/web 4 -n default
deployment/web in default
This starts 2 more copies of your app. Right now: 2 copies. After: 4 copies.
$ kubectl scale deployment/web --replicas=4 -n default
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made                                                    EXIT=0
$ kubectl get deploy web -o jsonpath='{.spec.replicas}'
4

$ echo yes | KUBECONFIG=<sts-scale> k8rs ops scale statefulset/store 3 -n default
statefulset/store in default
This starts 1 more copy of your app. Right now: 2 copies. After: 3 copies.
$ kubectl scale statefulset/store --replicas=3 -n default
the cluster checked it first and accepted it
k8rs: the change was made                                                    EXIT=0
$ kubectl get sts store -o jsonpath='{.spec.replicas}'
3

$ echo yes | KUBECONFIG=<rs-scale> k8rs ops scale replicaset/bare-rs 5 -n default
replicaset/bare-rs in default
This starts 3 more copies of your app. Right now: 2 copies. After: 5 copies. If a
deployment manages this replicaset, its controller will put the count back.
$ kubectl scale replicaset/bare-rs --replicas=5 -n default
the cluster checked it first and accepted it
k8rs: the change was made                                                    EXIT=0
$ kubectl get rs bare-rs -o jsonpath='{.spec.replicas}'
5
```

The `dryRun=All` preflight is inside each of those runs — *"the cluster checked it
first and accepted it"* — so the rehearsal needs no grant the real call does not.

## R2.3 — and it is minimal: each half alone fails, and the object does not move

Same object, same two ClusterRoles split into their halves.

```
$ echo yes | KUBECONFIG=<get-only>   k8rs ops scale deployment/web 6 -n default
$ kubectl scale deployment/web --replicas=6 -n default
k8rs: the change was never sent — the cluster would not allow it: deployments.apps
  "web" is forbidden: User "k8rs-probe" cannot patch resource "deployments/scale"
  in API group "apps" in the namespace "default"                             EXIT=2

$ echo yes | KUBECONFIG=<patch-only> k8rs ops scale deployment/web 6 -n default
k8rs: k8rs could not read how many copies of deployment/web in default are running
  right now — the cluster would not allow it: ... cannot get resource
  "deployments/scale" ...                                                    EXIT=2

$ kubectl get deploy web -o jsonpath='{.spec.replicas}'
4                                                        (unmoved by either run)
```

The resource half is per kind — a `deployments/scale` grant reaches no other kind:

```
$ echo yes | KUBECONFIG=<dep-scale> k8rs ops scale statefulset/store 9 -n default
k8rs: k8rs could not read how many copies of statefulset/store in default are running
  right now — the cluster would not allow it: statefulsets.apps "store" is forbidden:
  User "k8rs-probe" cannot get resource "statefulsets/scale" ...             EXIT=2
$ kubectl get sts store -o jsonpath='{.spec.replicas}'
3                                                        (unmoved)
```

## R2.4 — nothing else is in the path, measured with every default binding gone

The runs above all carried `system:authenticated`, which kind binds to
`system:discovery` and `system:basic-user`. To rule out a hidden discovery or
`nonResourceURL` requirement, the same identity was impersonated with
`system:unauthenticated` in its group list — round 1 § 2's technique, which drops
`system:authenticated` — leaving **one** grant in the whole cluster:
`apps` / `deployments/scale` / `["get","patch"]`.

`kubectl` cannot even enumerate the API under that identity:

```
$ KUBECONFIG=<bare> kubectl auth can-i create selfsubjectaccessreviews.authorization.k8s.io
E0912 ... "Unhandled Error" err="couldn't get current server API group list: unknown"
E0912 ... "Unhandled Error" err="couldn't get current server API group list: unknown"
                                                                             exit=1
```

k8rs performs the whole operation anyway:

```
$ echo yes | KUBECONFIG=<bare> k8rs ops scale deployment/web 7 -n default
deployment/web in default
This starts 3 more copies of your app. Right now: 4 copies. After: 7 copies.
$ kubectl scale deployment/web --replicas=7 -n default
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made                                                    EXIT=0
$ kubectl get deploy web -o jsonpath='{.spec.replicas}'
7
```

No discovery call, no `nonResourceURL`, no read of the parent `deployments`, no
`/status`, no review. `get+patch <plural>/scale` is the complete grant.

## R2.5 — both questions the key map is built on are askable, and answer independently

```
$ KUBECONFIG=<get+patch>  k8rs ops may-i get   deployments.apps --subresource=scale -n default  → yes  exit=0
$ KUBECONFIG=<get+patch>  k8rs ops may-i patch deployments.apps --subresource=scale -n default  → yes  exit=0
$ KUBECONFIG=<get-only>   k8rs ops may-i get   deployments.apps --subresource=scale -n default  → yes  exit=0
$ KUBECONFIG=<get-only>   k8rs ops may-i patch deployments.apps --subresource=scale -n default  → no   exit=1
$ KUBECONFIG=<patch-only> k8rs ops may-i get   deployments.apps --subresource=scale -n default  → no   exit=1
$ KUBECONFIG=<patch-only> k8rs ops may-i patch deployments.apps --subresource=scale -n default  → yes  exit=0
```

Against `Refused::of`'s `[Option<&Verdict>; 2]` and `refuses`'s `any(No)`: the
first login draws `s` lit, the other two draw it refused — and each of those two,
measured in § R2.3, cannot complete the operation.

## R2.6 — the objects and their final state

`deployment/web` 2 → 4 → 7, `statefulset/store` 2 → 3, `replicaset/bare-rs` 2 → 5.
All three created by this run, all three destroyed with the cluster.

## R2.7 — teardown

```
$ K8RS_CLUSTER=review scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: [<the one control-plane node>]

$ kind get clusters
k8rs

$ docker ps --format '{{.Names}}\t{{.Image}}'
k8rs-worker3         kindest/node:v1.36.1
k8rs-worker          kindest/node:v1.36.1
k8rs-control-plane   kindest/node:v1.36.1
k8rs-worker2         kindest/node:v1.36.1

$ kubectl config get-contexts -o name ; kubectl config current-context
kind-k8rs
kind-k8rs

$ ps -eo pid,args | grep -E "[s]leep 3000|[k]ind delete"
(none — watchdog stopped, its orphaned `sleep` killed by pid)
```

Every impersonating kubeconfig was mode 0600 under `$HOME`, outside the repo, and
deleted after the run. Nothing from this run entered `tests/` or `git`.
