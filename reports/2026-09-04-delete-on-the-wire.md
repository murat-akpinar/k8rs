# `kubectl delete` on the wire — the body, per kind, the dry-run, and what an audit record holds

`k8s-admin`, 2026-09-04. Measured for todo.md 3808's brief, against a real
apiserver: one ephemeral `K8RS_CLUSTER=review` kind cluster, `kind v0.32.0`,
node image `kindest/node:v1.36.1`, server `v1.36.1`, `kubectl` client
`v1.36.3`, apiserver on `127.0.0.1:6444` so the PM's fixture cluster on `:6443`
was untouched. Its own kubeconfig in a scratch file (`kind create cluster
--kubeconfig …`), so the user's current context never moved. Torn down from a
`trap … EXIT` (NOTES § D185); `kind get clusters` afterwards printed `k8rs`
and nothing else.

No committed artifact was produced and nothing was written into `tests/`.
Response bodies were never printed — the extractor below prints only the
request line and the request body.

The apiserver was started with an audit policy file and an audit log path
through `kubeadmConfigPatches`; the policy, whole:

```
apiVersion: audit.k8s.io/v1
kind: Policy
omitStages:
- RequestReceived
rules:
- level: Metadata
```

`§4` runs the same pair of requests against a fresh deployment with that
single `level:` changed to `Request`.

The extractor, run as `./wire.sh "<label>" <kubectl args…>`:

```
kubectl "$@" --v=9 >"$out.stdout" 2>"$out.stderr"
awk '/"Request Body" body=</{getline; sub(/^\t/,""); print "BODY: " $0; next}
     /curl -v -X/{sub(/^\t/,""); print "REQ : " $0}' "$out.stderr"
```

At `--v=9` kubectl v1.36.3 logs the body as structured klog
(`helper.go:202] "Request Body" body=<` then the body on the next line), which
is why a `grep 'Request Body: '` finds nothing.

## 1. What `kubectl delete pod/<name> -n <ns>` puts on the wire

```
$ kubectl -n payments delete pod/web-847f49cc4d-9ws46 --v=9
exit=0
pod "web-847f49cc4d-9ws46" deleted from payments namespace
BODY: {"propagationPolicy":"Background"}
REQ : curl -v -XDELETE  -H "Accept: application/json" -H "Content-Type: application/json" -H "User-Agent: kubectl/v1.36.3 (linux/amd64) kubernetes/0f29094" 'https://127.0.0.1:6444/api/v1/namespaces/payments/pods/web-847f49cc4d-9ws46'
REQ : curl -v -XGET  -H "Accept: application/json" … 'https://127.0.0.1:6444/api/v1/namespaces/payments/pods/web-847f49cc4d-9ws46'
REQ : curl -v -XGET  -H "Accept: application/json" … 'https://127.0.0.1:6444/api/v1/namespaces/payments/pods?allowWatchBookmarks=true&fieldSelector=metadata.name%3Dweb-847f49cc4d-9ws46&resourceVersionMatch=NotOlderThan&sendInitialEvents=true&timeoutSeconds=356&watch=true'
```

Field values this turns on:

| | |
|---|---|
| verb | `DELETE` |
| path | `/api/v1/namespaces/payments/pods/<name>` |
| query string | **none** — the URL ends at the object name |
| body | `{"propagationPolicy":"Background"}`, 34 bytes |
| headers kubectl logged | `Accept: application/json`, `Content-Type: application/json`, `User-Agent` — no dry-run header among them |

`propagationPolicy` is present and it is `Background`. The body carries no
`kind` and no `apiVersion`.

The DELETE is the **first** request kubectl sends: no `GET` precedes it. The
two `GET`s after it are `--wait`'s default poll-then-watch for the object
going away.

The same three headers appeared in every run; their **order in the log varies
between runs** of the same command (`-H "User-Agent" -H "Accept" -H
"Content-Type"` in the statefulset run of §2, the reverse in §1) — that is klog rendering a Go map,
and says nothing about the wire.

## 2. Per kind — all six of the driver's `KINDS`

Each run is `kubectl delete <ref> --v=9` against a real object of that kind.
Only the DELETE line and its body are reproduced.

```
pod          BODY: {"propagationPolicy":"Background"}
             DELETE 'https://127.0.0.1:6444/api/v1/namespaces/payments/pods/web-847f49cc4d-9ws46'
deployment   BODY: {"propagationPolicy":"Background"}
             DELETE 'https://127.0.0.1:6444/apis/apps/v1/namespaces/payments/deployments/dryrunme'
statefulset  BODY: {"propagationPolicy":"Background"}
             DELETE 'https://127.0.0.1:6444/apis/apps/v1/namespaces/payments/statefulsets/db'
daemonset    BODY: {"propagationPolicy":"Background"}
             DELETE 'https://127.0.0.1:6444/apis/apps/v1/namespaces/payments/daemonsets/agent'
replicaset   BODY: {"propagationPolicy":"Background"}
             DELETE 'https://127.0.0.1:6444/apis/apps/v1/namespaces/payments/replicasets/web-847f49cc4d'
node         BODY: {"propagationPolicy":"Background"}
             DELETE 'https://127.0.0.1:6444/api/v1/nodes/review-worker'
```

The body is byte-identical across all six. What differs is the path: the node
is `/api/v1/nodes/<name>` with no `namespaces/<ns>` segment, and
`kubectl delete node/<name>` takes no `-n`.

## 3. `--dry-run=server` against the same object — exactly which bytes differ

Run against the same pod, dry-run first, then the real delete:

```
$ kubectl -n payments delete pod/web-847f49cc4d-9ws46 --dry-run=server --v=9
exit=0
pod "web-847f49cc4d-9ws46" deleted from payments namespace (server dry run)
BODY: {"propagationPolicy":"Background","dryRun":["All"]}
REQ : curl -v -XDELETE  -H "Accept: application/json" -H "Content-Type: application/json" -H "User-Agent: kubectl/v1.36.3 (linux/amd64) kubernetes/0f29094" 'https://127.0.0.1:6444/api/v1/namespaces/payments/pods/web-847f49cc4d-9ws46'
```

Against §1's real call:

| | dry-run | real |
|---|---|---|
| verb | `DELETE` | `DELETE` |
| URL | `…/pods/web-847f49cc4d-9ws46` | `…/pods/web-847f49cc4d-9ws46` |
| query string | none | none |
| headers | the same three names | the same three names |
| body | 51 bytes | 34 bytes |

```
$ python3 -c "…"
real body    34 bytes: {"propagationPolicy":"Background"}
dry-run body 51 bytes: {"propagationPolicy":"Background","dryRun":["All"]}
difference   17 bytes, inserted before the closing brace: ',"dryRun":["All"]'
```

The seventeen bytes `,"dryRun":["All"]` in the body, and the `Content-Length`
that follows from them, are the whole difference. The same pair was run against a
deployment and against a node, and both dry-run bodies were also
`{"propagationPolicy":"Background","dryRun":["All"]}` with an unchanged URL.

kubectl's *stdout* does distinguish them: `… deleted from payments namespace
(server dry run)` against `… deleted from payments namespace`.

## 4. What the cluster's audit record holds

Both requests of §1 and §3, from the cluster's own audit log at `level:
Metadata`, extracted with `python3 audit.py` (prints the fields, never the
record):

```
level=Metadata verb=delete code=200 dryRun-in-uri=False uri=/api/v1/namespaces/payments/pods/web-847f49cc4d-9ws46
level=Metadata verb=delete code=200 dryRun-in-uri=False uri=/api/v1/namespaces/payments/pods/web-847f49cc4d-9ws46
```

Field by field, `python3 pair.py`:

```
records for that pod's DELETE: 3
'dryRun' appears anywhere in record 1 (the --dry-run=server one): False
'dryRun' appears anywhere in record 2 (the real one):           False
top-level keys, record 1: ['annotations', 'apiVersion', 'auditID', 'kind', 'level', 'objectRef', 'requestReceivedTimestamp', 'requestURI', 'responseStatus', 'sourceIPs', 'stage', 'stageTimestamp', 'user', 'userAgent', 'verb']
identical keys: True
fields that differ between the two records:
   auditID                "2c6a28b8-…"   vs   "4446ac5e-…"
   requestReceivedTimestamp "2026-09-04T18:28:59.189368Z"   vs   "2026-09-04T18:29:28.289843Z"
   stageTimestamp         "2026-09-04T18:28:59.191609Z"   vs   "2026-09-04T18:29:28.293280Z"
fields that are byte-identical:
   ['annotations', 'apiVersion', 'kind', 'level', 'objectRef', 'requestURI', 'responseStatus', 'sourceIPs', 'stage', 'user', 'userAgent', 'verb']
objectRef, record 1: {"resource": "pods", "namespace": "payments", "name": "web-847f49cc4d-9ws46", "apiVersion": "v1"}
objectRef, record 2: {"resource": "pods", "namespace": "payments", "name": "web-847f49cc4d-9ws46", "apiVersion": "v1"}
```

`requestObject` is not among the keys at this level. The two `annotations`
entries are the authorization decision and its reason, and they are equal.
The third record is the worker's kubelet confirming the deletion
(`user: review-worker`, `userAgent: kubelet/v1.36.1`), not a second kubectl
call.

**Positive control — the same audit policy records `dryRun` when it is in the
URI.** A `PATCH` with `--dry-run=server`, sent minutes later on the same
cluster:

```
REQ : curl -v -XPATCH … 'https://127.0.0.1:6444/apis/apps/v1/namespaces/payments/deployments/web?dryRun=All&fieldManager=kubectl-patch'

level=Metadata verb=patch  code=200 dryRun-in-uri=True  uri=/apis/apps/v1/namespaces/payments/deployments/web?dryRun=All&fieldManager=kubectl-patch
```

**Counterfactual — the same two deletes at `level: Request`.** Policy changed
to `level: Request`, apiserver restarted by moving its static-pod manifest out
and back (`apiserver back after 27s`), one fresh deployment deleted twice:

```
$ kubectl -n payments delete deployment/reqlevel --dry-run=server
$ kubectl -n payments delete deployment/reqlevel
level=Request uri=/apis/apps/v1/namespaces/payments/deployments/reqlevel
   requestObject: {"kind": "DeleteOptions", "apiVersion": "meta.k8s.io/__internal", "propagationPolicy": "Background", "dryRun": ["All"]}
level=Request uri=/apis/apps/v1/namespaces/payments/deployments/reqlevel
   requestObject: {"kind": "DeleteOptions", "apiVersion": "meta.k8s.io/__internal", "propagationPolicy": "Background"}
```

## 5. Where the `--cascade` flag lands

```
$ kubectl -n payments delete deployment/casc1 --cascade=orphan --v=9
BODY: {"propagationPolicy":"Orphan"}
$ kubectl -n payments delete deployment/casc2 --cascade=foreground --v=9
BODY: {"propagationPolicy":"Foreground"}
$ kubectl -n payments delete deployment/casc3 --cascade=background --v=9
BODY: {"propagationPolicy":"Background"}
```

`--cascade=background` and no flag at all produce the same 34 bytes.

## 6. What an empty `DeleteOptions` body does, per kind

Raw `DELETE` through `kubectl proxy`, against objects whose one pod carries a
finalizer nothing removes, so a foreground delete cannot finish:

```
=== deployments/gc1   DELETE body: {}
  http=200
  2s later: deployment object is gone ( NotFound )
  replicasets left: 0
=== deployments/gc2   DELETE body: {"propagationPolicy":"Background"}
  http=200
  2s later: deployment object is gone ( NotFound )
  replicasets left: 0
=== deployments/gc3   DELETE body: {"propagationPolicy":"Foreground"}
  http=200
  2s later: deployment object still present; deletionTimestamp set: True ; finalizers: ['foregroundDeletion']
  replicasets left: 1
```

Whether an empty body orphans dependents, for the other three workload kinds
(pods counted by label, 6 s after the DELETE):

```
  replicaset rs-empty  body={}  http=200  pods before=1  pods 6s after=0
  replicaset rs-bg  body={"propagationPolicy":"Background"}  http=200  pods before=1  pods 6s after=0
  statefulset sts-empty  body={}  http=200  pods before=1  pods 6s after=0
  daemonset ds-empty  body={}  http=200  pods before=1  pods 6s after=0
```

On server `v1.36.1`, for `apps/v1` deployments, statefulsets, daemonsets and
replicasets, an empty body and `Background` were not distinguishable by any of
these observations; `Foreground` was.

## 7. A node whose object is deleted while its kubelet keeps running

```
$ kubectl delete node/review-worker --v=9
exit=0
node "review-worker" deleted
```

The container was never stopped (`docker ps`: `review-worker Up 5 minutes`,
`systemctl is-active kubelet` → `active`). Node list, sampled every 5 s:

```
t+10s: review-control-plane   Ready   control-plane   4m7s   v1.36.1
…
t+65s: review-control-plane   Ready   control-plane   5m3s   v1.36.1
```

and at 2 min 45 s after the delete, still one node. Three of the recurring lines from the kubelet's own log for that window
(grepped, not consecutive):

```
E0904 18:33:22.294270 kubelet_node_status.go:479] "Error updating node status, will retry" err="error getting node \"review-worker\": node \"review-worker\" not found"
E0904 18:33:26.242740 eviction_manager.go:297] "Eviction manager: failed to get summary stats" err="failed to get node info: node \"review-worker\" not found"
E0904 18:33:21.978562 nodelease.go:50] "Failed to get node when trying to set owner ref to the node lease" err="nodes \"review-worker\" not found" node="review-worker"
```

five `Error updating node status` lines per ~10 s cycle, unbroken, with no
registration attempt logged.

The pods that had been on that node did not survive the delete either. Before
it, `deployment/web`'s surviving pod was `web-847f49cc4d-v7qjx`; 55 s after it,
`kubectl -n payments get pods` listed `web-847f49cc4d-5n625` and
`web-847f49cc4d-gpd2c`, both `Pending`, `<none>` for node — different names, so
the old pod was deleted and the ReplicaSet made new ones, which had nowhere to
go (one node left, carrying the control-plane taint).

Restarting the kubelet:

```
$ docker exec review-worker systemctl restart kubelet
node re-registered 2s after the kubelet restart
review-worker   Ready   <none>   2s   v1.36.1
```

Not measured: whether a kubelet left alone re-registers on some longer
timescale than the 2 min 45 s watched here.

## 8. Machine state

```
$ free -g   # before the cluster
Mem: total 23  used 7  available 16
$ df -h /tmp
tmpfs  12G  8,2G  3,5G  70% /tmp
```

One cluster at a time: the PM's four `k8rs-*` containers were running
throughout and were not touched; `docker ps` after teardown listed exactly
those four.
