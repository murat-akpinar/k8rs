# The kind word is not a key — measurements behind the `Offer::act` review (2026-09-18)

Read-only queries against the running fixture cluster (`kind-k8rs`, server v1.36.1).
**No cluster was created and nothing was written** — `api-resources`, `get --raw` and
`get -o json` only, per D92's *a dev may read the fixture cluster*. No second cluster
was brought up beside it (D84, D267).

## 1. Two kinds share one `kind` word on a stock cluster, with no CRD installed

```
$ kubectl --context kind-k8rs api-resources -o wide | awk 'NR==1 || /Event|StatefulSet|Deployment|DaemonSet|ReplicaSet|Node |Pod /'
NAME            SHORTNAMES  APIVERSION        NAMESPACED  KIND         VERBS
events          ev          v1                true        Event        create,delete,deletecollection,get,list,patch,update,watch
nodes           no          v1                false       Node         create,delete,deletecollection,get,list,patch,update,watch
pods            po          v1                true        Pod          create,delete,deletecollection,get,list,patch,update,watch
daemonsets      ds          apps/v1           true        DaemonSet    create,delete,deletecollection,get,list,patch,update,watch
deployments     deploy      apps/v1           true        Deployment   create,delete,deletecollection,get,list,patch,update,watch
replicasets     rs          apps/v1           true        ReplicaSet   create,delete,deletecollection,get,list,patch,update,watch
statefulsets    sts         apps/v1           true        StatefulSet  create,delete,deletecollection,get,list,patch,update,watch
events          ev          events.k8s.io/v1  true        Event        create,delete,deletecollection,get,list,patch,update,watch
```

Both `Event` rows carry `list`, so `k8s::browsable` keeps both (its own doc: *"Nothing is
de-duplicated … the same plural under two groups is two resources"*), and both lowercase to
the single word `event`. The field the finding turns on is `APIVERSION`: it is the only
column that tells the two rows apart, and it is the column
`ui::offered`'s browser arm drops.

## 2. The `/scale` subresources, per group

```
$ kubectl --context kind-k8rs get --raw /apis/apps/v1 | (resources with /scale, plus the four workload kinds)
daemonsets      | kind=DaemonSet   | verbs=create,delete,deletecollection,get,list,patch,update,watch
deployments     | kind=Deployment  | verbs=create,delete,deletecollection,get,list,patch,update,watch
deployments/scale  | kind=Scale | verbs=get,patch,update
replicasets     | kind=ReplicaSet  | verbs=create,delete,deletecollection,get,list,patch,update,watch
replicasets/scale  | kind=Scale | verbs=get,patch,update
statefulsets    | kind=StatefulSet | verbs=create,delete,deletecollection,get,list,patch,update,watch
statefulsets/scale | kind=Scale | verbs=get,patch,update

$ kubectl --context kind-k8rs get --raw /api/v1 | (resources with /scale)
['replicationcontrollers/scale']
```

`ops::scalable`'s three kinds are exactly the three `apps/v1` resources carrying a `/scale`
subresource, and `daemonsets` carries none. The only other `/scale` in a stock cluster is
`v1 replicationcontrollers`, which `ops::scalable` refuses — so the footer drops the key
rather than offering one that would fail.

The `/scale` subresource is declared **per group/version**, not per kind word: `apps/v1`
declares three, and a CRD in another group declares its own or none.

## 3. What a card's owner kind actually is, right now, on that cluster

```
$ kubectl --context kind-k8rs get pods -A -o json | (count controlling ownerReferences)
controlling ownerReferences on pods, by (apiVersion, kind):
  ('apps/v1', 'DaemonSet') 10
  ('apps/v1', 'ReplicaSet') 10
  ('apps/v1', 'StatefulSet') 2
  ('v1', 'Node') 4
pods with no controller: 15

$ kubectl --context kind-k8rs get replicasets -A -o json | (count controllers)
replicasets total: 8 | controlled by a Deployment: 8 | bare (no controller): 0
```

10 of 41 pods name an `apps/v1 ReplicaSet` as their controller. All 8 ReplicaSets on the
cluster are Deployment-controlled; **zero are bare.**

`k8s.rs` publishes those pods with the ReplicaSet as the owner until an on-demand `get`
resolves it (`Store::with_owner`, and the `owner()` doc: *"the snapshot is published with the
ReplicaSet as the owner and never held back"*), and `Store::unresolved_owners`' doc states
that a failed fetch is never retried: *"a socket that died once leaves the heading at
ReplicaSet for the life of the process."*

## 4. Help's three `Changing things` rows, as widths

```
50  '    s       run more or fewer copies       (scale)'
60  '    r       restart, at its own pace       (rollout restart)'
49  '    ctrl-d  delete — you type the name to confirm'
76  '    s       run more or fewer copies   (scale — get+patch deployments/scale)'
```

The first three are `screens/help.md`'s static body; the fourth is the refused rewrite
`ui::key_map` builds. None of the three static rows names a kind.
