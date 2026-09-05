# Every operation against a real cluster — scale, restart, delete, may-i

`k8s-admin`, 2026-09-05. Working tree at `56222af`, built release in an isolated
copy under `$HOME` with its own `CARGO_TARGET_DIR`; nothing in `/home/shyuuhei/GIT/k8rs`
was written but this file.

The cluster: `K8RS_CLUSTER=review`, `K8RS_WORKERS=1`, `K8RS_APISERVER_PORT=6444`,
`kindest/node:v1.36.1`, its own kubeconfig and its own `XDG_STATE_HOME` under
`$HOME`. Nodes `review-control-plane` and `review-worker` — names
`scripts/sanitize.jq` refuses, so nothing here can become a fixture. Torn down
before this file was finished; the teardown is § 12.

Server strings are written `<loopback>:<port>` and uids are cut to eight
characters — the findings turn on two uids *differing*, not on their values.

---

## 1. `scale` — the change, the watch stream, the audit line

```
$ echo yes | k8rs ops scale deploy/web 4 -n payments
deployment/web in payments
This starts 2 more copies of your app. Right now: 2 copies. After: 4 copies.
$ kubectl scale deployment/web --replicas=4 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
exit=0
```

`kubectl -n payments get deploy web -o jsonpath='{.spec.replicas}'` → `4`.

The audit log, both lines, in file order:

```
2026-09-05T07:43:49.692717753Z attempt · deployment/web · context kind-review · server https://<loopback>:<port> · namespace payments · uid 3c59efb6… · kubectl: kubectl scale deployment/web --replicas=4 -n payments · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale · resourceVersion not sent
result · attempt 2026-09-05T07:43:49.692717753Z · recorded 2026-09-05T07:43:49.700148384Z · deployment/web · dry-run: the cluster checked it first and accepted it · the change was made
```

The command log line on screen and the `kubectl:` field of the audit line are
byte-identical. `resourceVersion not sent` matches
[D228](../NOTES.md#d228--the-review-round-that-reversed-the-box-a-precondition-on-a-field-that-moves-when-nothing-changed-and-the-dry-run-window-that-was-02-of-what-it-claimed-2026-09-05).

### The watch stream

`k8rs --live -n payments` running in a second process for 45 s, the scale above
fired at t+8 s. Its whole stdout across the run:

```
ns: payments · 4 pods · 2 nodes
○ nothing is broken in payments

ns: payments · 5 pods · 2 nodes
○ nothing is broken in payments

ns: payments · 6 pods · 2 nodes
○ nothing is broken in payments
```

The change k8rs made appears in k8rs's own live view, through the watch, in two
redraws.

### The taught line, run by hand

```
$ kubectl scale deployment/web --replicas=4 -n payments
deployment.apps/web scaled          exit=0   → spec.replicas 4
```

and again from a current context whose namespace is `default`, to prove the `-n`
is carrying the run:

```
$ kubectl scale deployment/web --replicas=3 -n payments
deployment.apps/web scaled          exit=0   → spec.replicas 3
```

### Load: the counters the apiserver keeps

`apiserver_request_total`, summed per resource, before and after one 62-second
`k8rs --live -n payments`:

```
resource       verb   before  after  delta
daemonsets     LIST        5      6     +1
daemonsets     WATCH       5      6     +1
deployments    LIST        6      7     +1
deployments    WATCH      12     13     +1
nodes          LIST        9     10     +1
nodes          WATCH      28     29     +1
pods           LIST       17     18     +1
pods           WATCH      21     22     +1
statefulsets   LIST        5      6     +1
statefulsets   WATCH       8      9     +1
```

One LIST and one WATCH per kind for the whole minute. `ps -o times=` on the live
process after 55 s elapsed: `0` seconds of CPU.

---

## 2. `restart` — generation, annotation, rollout

| object | generation before → after | annotation written |
|---|---|---|
| `deployment/web` | 3 → 4 | one, `kubectl.kubernetes.io/restartedAt` |
| `statefulset/db` | 1 → 2 | one, same key |
| `daemonset/agent` | 1 → 2 | one, same key |

Pod names before and after the deployment restart — a real rollout, not just a
spec write:

```
before: web-bc5d7d6f9-d6gqg  web-bc5d7d6f9-shvf7  web-bc5d7d6f9-v744n
after:  web-7866489d48-9k7sk web-7866489d48-cswtc web-7866489d48-mzdqh
```

The taught line reproduces it on all three kinds (`deployment.apps/web
restarted`, `statefulset.apps/db restarted`, `daemonset.apps/agent restarted`,
each `exit=0`, each generation +1).

### The paused deployment, on a real cluster

```
$ kubectl -n payments rollout pause deployment/web      → spec.paused=true
$ echo yes | k8rs ops restart deploy/web -n payments
…
$ kubectl rollout restart deployment/web -n payments
This deployment is paused, so nothing will be replaced until somebody resumes it
with kubectl rollout resume — and the command above will refuse to run until then.
the cluster checked it first and accepted it
…
k8rs: the change was made
exit=0

$ kubectl rollout restart deployment/web -n payments
error: deployments.apps "web" can't restart paused deployment (run rollout resume first)
exit=1
```

[D224](../NOTES.md#d224--the-restart-review-round-two-blockers-a-stand-in-apiserver-could-not-produce-and-the-sentence-that-promised-a-clusters-settings-2026-09-04)'s warning fires, and the sentence it prints is exactly
what the taught line then does.

### Two restarts inside one second

```
$ kubectl rollout restart deployment/web -n payments && kubectl rollout restart deployment/web -n payments
deployment.apps/web restarted
error: failed to create patch for web: if restart has already been triggered
within the past second, please wait before attempting to trigger another
exit=1                                        → generation moved 5 → 6 (one bump)

$ echo yes | k8rs ops restart deploy/web -n payments   → k8rs: the change was made
$ echo yes | k8rs ops restart deploy/web -n payments   → k8rs: the change was made
                                              → generation moved 6 → 8 (two bumps)
```

`kubectl` refuses the second; k8rs performs it. k8rs stamps the annotation to
nanoseconds (`…T07:45:52.34163431Z`) where `kubectl` writes a second-resolution
local-offset stamp (`2026-09-05T10:46:13+03:00`), so the merge patch is never a
no-op.

---

## 3. `delete` — all six kinds

| kind | typed name | sentence | exit |
|---|---|---|---|
| `pod` (inside a 30 s grace period) | yes | the cluster accepted this and the object is still there | 0 |
| `pod` (planted finalizer) | yes | the cluster accepted this and the object is still there | 0 |
| `replicaset` | yes | the change was made | 0 |
| `daemonset` | yes | the change was made | 0 |
| `statefulset` | yes | the change was made | 0 |
| `deployment` | yes | the change was made | 0 |
| `node` (cluster-scoped) | yes | the change was made | 0 |

### The case that matters — accepted, not finished

Planted, and named as planted: a finalizer `k8rs.review/hold` added to one pod.

```
$ echo web-5dd5784c74-jtzpj | k8rs ops delete pod/web-5dd5784c74-jtzpj -n payments
pod/web-5dd5784c74-jtzpj in payments
This removes the pod. Whatever created it will normally replace it — k8rs has
not checked whether anything did.
$ kubectl delete pod/web-5dd5784c74-jtzpj -n payments
k8rs did not check this one with the cluster first
type the object's own name and press enter to go ahead — anything else stops it:
k8rs: the cluster accepted this and the object is still there — something is
delaying the removal, and the command above waits for that where k8rs does not
exit=0 took 0s
```

The object one second later: `phase=Running deletionTimestamp=2026-09-05T07:48:21Z`.

The taught line on the same object:

```
$ timeout 8 kubectl delete pod/web-5dd5784c74-jtzpj -n payments
pod "web-5dd5784c74-jtzpj" deleted from payments namespace
exit=124 took 8s
```

`kubectl` prints *deleted* and then blocks — killed at 8 s by `timeout`. k8rs's
sentence is the more honest of the two records, and the divergence it names is
real and measured.

The unplanted grace-period case reaches the same outcome: a pod with
`terminationGracePeriodSeconds: 30` answers with the object, k8rs says still
there, `deletionTimestamp` set. Where the container is `pause` and exits on
SIGTERM immediately, `kubectl delete` returns in under a second — so the *wait*
is only visible when something really holds the object.

### The node — the one cluster-scoped path

```
$ echo review-worker | k8rs ops delete node/review-worker
node/review-worker
This asks the cluster to remove its record of review-worker, not the machine.
Something attached to it, unread by k8rs, may delay this or act first. Left
alone, its pods are deleted and the machine keeps running until its kubelet
restarts.
$ kubectl delete node/review-worker
k8rs did not check this one with the cluster first
…
k8rs: the change was made
exit=0
```

Audit line path: `DELETE /api/v1/nodes/review-worker`, namespace slot
`cluster-wide`. The consequence is accurate to the letter — the node did not
come back on its own after two minutes, and `systemctl restart kubelet` in the
container is what re-registered it.

`-n` on a node is refused before anything is sent: *a node belongs to the whole
cluster and is in no namespace, so `ops delete` will not take -n — leave it off*,
exit 2.

### The typed name

```
$ echo not-the-name | k8rs ops delete deployment/web -n payments
…
k8rs: nobody confirmed it, so nothing was changed
exit=2                       → deployment still there, generation unchanged
```

---

## 4. `may-i`

| line | answer | exit |
|---|---|---|
| `ops may-i list pods. -n payments` (admin) | yes | 0 |
| `ops may-i delete nodes.` (admin) | yes | 0 |
| `ops may-i delete pods. -n payments` (pods-read-only login) | no | 1 |
| `ops may-i list pods.` cluster-wide (namespaced Role) | no | 1 |
| `ops may-i list pods. -n payments` (dead socket) | could not put the question | 2 |
| `ops may-i list pods. -n payments` (rejected login) | could not put the question | 2 |

Spellings, all against the same cluster:

```
get pods./web          → may this login get pods/web in payments?           yes
get pods. --subresource log → may this login get pods (subresource: log)…   yes
patch deployments.apps/web  → may this login patch deployments.apps/web…    yes
delete nodes./review-worker → may this login delete nodes/review-worker?    yes
list deployments       → refused: `ops may-i` needs the API group as well as
                         the resource, because it cannot look one up …
```

`kubectl auth can-i` answers `yes` to the same three. No state directory is
created by any `may-i` run — checked by running one first on a fresh
`XDG_STATE_HOME` and finding no directory.

The dead-socket sentence:

```
k8rs: k8rs could not put the question to this cluster — k8rs could not reach the
cluster. That is not a no — k8rs hides nothing and refuses nothing because of it,
and the operation is still there to run
exit=2
```

---

## 5. The object's name reused while the dialog is open

### `scale` — the audit line names the wrong instance

Confirmation held on a fifo; between the dry-run and the yes, the deployment was
deleted and recreated under the same name.

```
uid BEFORE = 8656c3ec…
$ k8rs ops scale deploy/web 5 -n payments < fifo
deployment/web in payments
This starts 3 more copies of your app. Right now: 2 copies. After: 5 copies.
$ kubectl scale deployment/web --replicas=5 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
exit=0
uid AFTER  = 5fbe5cc6…      replicas = 5
```

The audit line for that run:

```
… attempt · deployment/web · … · uid 8656c3ec… · kubectl: kubectl scale deployment/web --replicas=5 -n payments · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale · resourceVersion not sent
result · … · deployment/web · dry-run: the cluster checked it first and accepted it · the change was made
```

`8656c3ec…` was changed by nothing. `5fbe5cc6…` was scaled to 5. The consequence
also described the object that is gone — *Right now: 2 copies* over an object
running 1.

### `delete` — the pod that was deleted is not the pod that was named

A StatefulSet pod, whose name the controller reuses by design. Three pods held
the name `db-0` across one k8rs run:

```
db-0 the operator opened the dialog on and typed the name of = 9c4d2429…
db-0 that held the name when the name was typed               = 081db2a7…
db-0 that exists two seconds after the run                    = db1b8778…
```

k8rs's own output for that run ends:

```
k8rs: the cluster accepted this and the object is still there — something is
delaying the removal, and the command above waits for that where k8rs does not
exit=0
```

and the audit line records `uid not read`, so the trail cannot say which of the
three it was. That `081db2a7…` is the one k8rs removed is read off the third uid:
the StatefulSet only makes a new `db-0` when the one before it is going.

### What the server will do about it, measured

A `DELETE` carrying a uid precondition that does not match, sent by hand:

```
$ curl -X DELETE … -d '{"kind":"DeleteOptions","apiVersion":"v1","propagationPolicy":"Background","preconditions":{"uid":"00000000-0000-0000-0000-000000000000"}}' \
    https://<loopback>:<port>/api/v1/namespaces/payments/pods/web-847f49cc4d-7lztw
HTTP 409
reason: Conflict
message: Operation cannot be fulfilled on Pod "web-847f49cc4d-7lztw": the UID in
the precondition (00000000-…) does not match the UID in record (07497095-…).
The object might have been deleted and then recreated
```

The pod survived (`deletionTimestamp=` empty). `kube-core-4.2.0`'s
`DeleteParams` carries `preconditions: Option<Preconditions>` with a `uid` field;
`ops::Pass::delete` sets `dry_run` and `propagation_policy` and leaves it `None`.

---

## 6. `scale` on a controller-owned ReplicaSet

```
$ echo yes | k8rs ops scale rs/web-847f49cc4d 5 -n payments
replicaset/web-847f49cc4d in payments
This starts 3 more copies of your app. Right now: 2 copies. After: 5 copies.
$ kubectl scale replicaset/web-847f49cc4d --replicas=5 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
exit=0
```

Three seconds later: `spec.replicas=2 status.replicas=2`, two pods. The
Deployment controller reverted it.

Whether k8rs could have known, off data it already has:

```
$ kubectl get --raw /apis/apps/v1/namespaces/payments/replicasets/web-847f49cc4d/scale
keys in metadata: ['creationTimestamp', 'name', 'namespace', 'resourceVersion', 'uid']
ownerReferences present: False
```

The `Scale` subresource strips `ownerReferences`, so the fact is not in the
answer `ops::scale` already reads.

---

## 7. The refusal that does not name the namespace

```
$ echo yes | k8rs ops scale deploy/web 3 -n no-such-ns
k8rs: k8rs could not read how many copies of deployment/web are running right
now — the cluster has no object with that name: deployments.apps "web" not found
exit=2
```

`no-such-ns` does not exist. The whole output of the run names no namespace.
The same mistake on the other two operations does name it, because `show` runs
first:

```
$ echo yes | k8rs ops restart deploy/web -n no-such-ns
deployment/web in no-such-ns
…
k8rs: the change was never sent — the cluster has no object with that name: …

$ echo web | k8rs ops delete deploy/web -n no-such-ns
deployment/web in no-such-ns
…
k8rs: nothing was changed — the cluster has no object with that name: …
```

`ops::unread` takes `object` and no namespace; `ops::scale` holds `namespace`
one line above the call.

---

## 8. The audit log's own properties, on a real run

26 mutations across the session, one file:

```
$ stat -c '%A %a %s bytes' state/k8rs/audit.log
-rw------- 600 14465 bytes
$ awk '/ attempt · /{a++} /^result · /{r++} END{print a, r}'
26 26
```

The state directory and its parent were created `drwx------ 700`. Every outcome
class reached it — 14 `the change was made`, 4 `the cluster accepted this and the
object is still there`, 3 `nothing was changed — … has no object with that name`,
1 `nobody confirmed it, so nothing was changed`, 1 `nothing was changed — the
cluster would not allow it`, 1 `the change was never sent — the login k8rs was
using had run out`, 1 `k8rs does not know whether the change was made`, and every
attempt line carries `resourceVersion not sent`. Nothing JWT-shaped is in the
file.

### Two operations at once

Two `k8rs ops scale` processes started together, confirmations at t+3 s and
t+6 s, one log:

```
07:53:41.687278332Z attempt · deployment/web …
07:53:41.687278351Z attempt · deployment/api …
result · attempt 07:53:41.687278351Z · recorded 07:53:44.670237862Z · deployment/api · … the change was made
result · attempt 07:53:41.687278332Z · recorded 07:53:47.670998036Z · deployment/web · … the change was made
```

Adjacency is wrong and the pairing is still unambiguous: each result names its
attempt's stamp *and* its object. The two attempt stamps differ by 19 ns.

---

## 9. Failure paths

| what | what k8rs said | exit |
|---|---|---|
| dead socket, `scale` | k8rs could not read how many copies … — k8rs could not reach the cluster | 2 |
| dead socket, `delete` | k8rs does not know whether the change was made — k8rs could not reach the cluster | 2 |
| dead socket, `may-i` | could not put the question … That is not a no | 2 |
| context that is not in the file | this kubeconfig has no such context — check the `current-context` line in the file, and any `--context` on the command line | 2 |
| kubeconfig path that does not exist | the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML | 2 |
| valid kubeconfig, login the cluster rejects | the login k8rs was using had run out: Unauthorized | 2 |
| 403 on `scale` (pods-read-only login) | the cluster would not allow it: … cannot get resource "deployments/scale" in API group "apps" in the namespace "payments" | 2 |
| 403 on `delete` | the cluster would not allow it: … cannot delete resource "pods" in API group "" in the namespace "payments" | 2 |

Both 403s name the verb and the resource, because the server's own message does
and k8rs quotes it.

The dead-socket `scale` and the dead-socket `delete` were run back to back and
the log went 36 → 38 lines, so exactly two were written between them; the two are
the delete's, because they name `call: DELETE` and carry `k8rs does not know
whether the change was made`. The scale therefore wrote none — it failed on the
read that builds the dialog, above `ops::perform`. That is an inference from the
pair and not two separate before/after readings.

### The dry-run window, when it loses

Confirmation held on a fifo, object deleted between the dry-run and the yes:

```
deployment/web in payments
This starts 3 more copies of your app. Right now: 2 copies. After: 5 copies.
$ kubectl scale deployment/web --replicas=5 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: nothing was changed — the cluster has no object with that name: deployments.apps "web" not found
exit=2
```

Audit line: `dry-run: the cluster checked it first and accepted it · nothing was
changed — …`, and the `uid` field holds the uid of the object that was there.

---

## 10. `--read-only`

```
$ echo yes | k8rs --read-only ops scale deploy/web 9 -n payments      exit=2
$ echo yes | k8rs --read-only ops restart deploy/web -n payments      exit=2
$ echo web | k8rs --read-only ops delete deploy/web -n payments       exit=2
k8rs: --read-only was asked for, so k8rs will not change anything — run it
without that flag to use an operation
```

`spec.replicas` and `generation` unchanged after all three. The audit log was
read before and after the `scale` line specifically: 34 → 34. The same refusal comes back instantly against a kubeconfig pointing at a
closed port, and against `KUBECONFIG` naming a file that does not exist — so
nothing was dialled and no file was read.

`--read-only ops may-i delete deployments.apps -n payments` answers `yes`,
exit 0.

---

## 11. Two claims in `src/ops.rs` checked against the server

**The webhook that fails a dry-run.** The § THE MUTATION CONTRACT header says a
webhook with `sideEffects: Some | Unknown` fails `dryRun=All`. On
`admissionregistration.k8s.io/v1`, server v1.36.1:

```
$ kubectl apply -f - <<< '…sideEffects: Unknown…'
The ValidatingWebhookConfiguration "k8rs-review-probe" is invalid:
webhooks[0].sideEffects: Unsupported value: "Unknown":
supported values: "None", "NoneOnDryRun"
```

`kubectl explain validatingwebhookconfiguration.webhooks.sideEffects` says the
same and adds *webhooks created via v1beta1 may also specify Some or Unknown* —
and `v1beta1` was removed in 1.22. So the shape is only reachable on a cluster
carrying a webhook config written before 1.22 and never rewritten since;
validation runs on write, not on read.

**`kubectl delete` waits.** Confirmed, but only where something holds the object
(§ 3): against a `pause` container that exits on SIGTERM, `kubectl delete pod`
returned in under one second.

---

## 12. Teardown

```
$ K8RS_CLUSTER=review bash scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-worker" "review-control-plane"]

$ kind get clusters
k8rs

$ docker ps --format '{{.Names}}\t{{.Image}}'
k8rs-worker3	kindest/node:v1.36.1
k8rs-worker	kindest/node:v1.36.1
k8rs-control-plane	kindest/node:v1.36.1
k8rs-worker2	kindest/node:v1.36.1
```

The PM's cluster and its four containers are as they were. `~/.kube/` was never
written to — its mtime is from before this run, and it holds no `config` file at
all. `git status --short` in the repo is empty at `56222af`.

---

## 13. What I could not do

- **An expired client certificate** was not produced. What was measured instead
  is a valid kubeconfig carrying a login the cluster rejects, which reaches
  `Fault::Expired` by a different route.
- **A real admission webhook** was not installed — no endpoint was served, so
  only the registration-time refusal in § 11 was measured, not a webhook
  answering a request.
- **A first attempt at a rejected login was invalid and is not in this report**:
  the kubeconfig was written in YAML flow style and the edit that replaced the
  credential broke the closing brace, so k8rs's *not valid YAML* answer was
  correct and my reading of it was not. § 9's row is the re-run against a
  block-style file `kubectl` parses.
- **Six kinds for `delete`, and a seventh path.** All six of `KINDS` were
  deleted. Nothing was checked about `delete` against an object another
  controller recreates *immediately* other than the StatefulSet pod in § 5.
- **Nothing was measured at 5000 pods.** § 1's LIST/WATCH counters are the
  argument; the resident-set question is
  [2026-09-02-where-the-resident-set-is-at-1000-pods.md](2026-09-02-where-the-resident-set-is-at-1000-pods.md)'s.
