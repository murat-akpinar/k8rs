# The scale precondition under churn — how often a Deployment moves, and what that does to `scale`

`k8s-admin`, 2026-09-05, second run of the day. Measured for the operator review
of Phase 7's box *"Every call sends the resourceVersion that was read; a `409`
offers a re-read, never a blind overwrite"* — specifically for the question
[D227](../NOTES.md#d227--the-resourceversion-goes-only-where-a-read-already-happened-and-the-metadata-read-that-leaks-what-a-get-was-refused-for-2026-09-05)
ruling 3 turns on: whether the precondition it adds fires on workloads that are
healthy or merely busy.

The companion file
[reports/2026-09-05-resourceversion-and-409-on-the-wire.md](2026-09-05-resourceversion-and-409-on-the-wire.md)
established the wire mechanics. This one measures rates and the end-to-end
sequence through the built binary.

**The cluster.** One ephemeral `K8RS_CLUSTER=review` kind cluster, `kind
v0.32.0`, node image `kindest/node:v1.36.1`, server **`v1.36.1`**, `kubectl`
client **`v1.36.3`**, single node, its own kubeconfig in a scratch file so the
user's current context never moved. Created and torn down by a watchdog script
whose `kind delete` is in a `trap … EXIT` and not on a last line
([D185](../NOTES.md#d185--cleanup-on-the-last-line-is-not-cleanup-and-the-resource-is-not-always-a-file-2026-08-30)).
Afterwards:

```
$ kind get clusters
k8rs
$ docker ps --format '{{.Names}}'
k8rs-worker3
k8rs-worker
k8rs-control-plane
k8rs-worker2
```

The PM's four containers ran throughout and were not touched. No committed
artifact was produced and nothing was written into `tests/`.

**Two tools.** Rates and raw requests went through `kubectl proxy --port=8877`
and `curl`, because only that gives byte control over the body. The end-to-end
runs used the **built debug binary** at `target/debug/k8rs`, verified before use
to carry this box's strings (`strings … | grep "so look at it again before
deciding"` → 1 match), with `XDG_STATE_HOME` pointed at a scratch directory so
the user's own audit log was never appended to.

**The subjects.** Namespace `payments`. `web`, a Deployment of a pause image,
3–9 replicas, healthy. `crashy`, a Deployment of three replicas whose one
container runs `sleep 2; exit 1`, i.e. `CrashLoopBackOff`. `db`, a StatefulSet, 2 replicas.

**Redactions, named.** Object UIDs are `<uid>`. One `Status.message` in § 6 is
described by size and canary count only; it is an object echo and
`reports/README.md` refuses those outright. Nothing else was altered.

## 1. How often a Deployment's own `metadata.resourceVersion` moves

Counted off a watch, one line per write to the object:

```
$ curl -sN "http://127.0.0.1:8877/apis/apps/v1/namespaces/payments/deployments\
?watch=1&fieldSelector=metadata.name=web&resourceVersion=$RV"
```

| state | window | writes | gaps between writes (s) |
|---|---|---|---|
| **steady** — 3/3 ready, nothing happening | 180 s | **0** | — |
| **rolling** — one `kubectl set image` | 3.46 s | **15** | min 0.010 · median 0.022 · max 2.246 |
| **one pod deleted** | 0.57 s | **4** | 0.009 · 0.145 · 0.415 |
| **`CrashLoopBackOff`**, three pods | 99.4 s | **20** | min 0.006 · median 2.451 · max 35.557 |
| **an HPA attached that cannot compute** (no metrics server) | 90 s | **0** | — |
| one **healthy pod** object | 90 s | **0** | — |
| one **crashlooping pod** object, backoff grown to ~5 min | 90 s | **1** | — |

Steady, 180 s:

```
start rv=541 at 2026-09-05T03:34:10+03:00
events: 0
end 2026-09-05T03:37:10+03:00
```

Rolling, first four and last three of fifteen (`gen` never moves — these are
status writes):

```
1788568649.360017599 MODIFIED rv=858 gen=2 specRep=3 ready=3 upd=3
1788568649.370448857 MODIFIED rv=860 gen=2 specRep=3 ready=3 upd=3
1788568649.384351960 MODIFIED rv=866 gen=2 specRep=3 ready=3 upd=0
1788568649.395848237 MODIFIED rv=869 gen=2 specRep=3 ready=3 upd=1
…
1788568652.701297244 MODIFIED rv=945 gen=2 specRep=3 ready=4 upd=3
1788568652.714420911 MODIFIED rv=951 gen=2 specRep=3 ready=3 upd=3
1788568652.816826451 MODIFIED rv=954 gen=2 specRep=3 ready=3 upd=3
```

`CrashLoopBackOff`, first four of twenty:

```
1788568837.679531305 MODIFIED rv=1325 gen=1 ready=1 avail=1
1788568838.692478683 MODIFIED rv=1330 gen=1 ready=0 avail=0
1788568847.442889540 MODIFIED rv=1351 gen=1 ready=1 avail=1
1788568849.751554540 MODIFIED rv=1357 gen=1 ready=0 avail=0
```

HPA attached, `cpu: <unknown>/80%`, 90 s: `count: 0`.

## 2. A status-only write bumps the scale subresource's version and voids the precondition

The `Scale` returned by `GET …/scale` carries the Deployment's own
`metadata.resourceVersion` (§ 1 of the companion report). This section
establishes that a write which touches **only `status`** moves it.

```
=== before ===
scale rv=954 replicas=3
deploy rv=954 gen=2 collisionCount=absent

=== a STATUS-ONLY write (no spec touched) ===
$ curl -s -X PATCH -H 'Content-Type: application/merge-patch+json' \
    --data '{"status":{"collisionCount":1}}' \
    http://127.0.0.1:8877/apis/apps/v1/namespaces/payments/deployments/web/status
http=200

=== after ===
scale rv=1102 replicas=3
deploy rv=1102 gen=2 collisionCount=1

=== now scale, with the resourceVersion read BEFORE the status write ===
$ curl -s -X PATCH -H 'Content-Type: application/merge-patch+json' \
    --data '{"metadata":{"resourceVersion":"954"},"spec":{"replicas":4}}' \
    http://127.0.0.1:8877/apis/apps/v1/namespaces/payments/deployments/web/scale
{"kind":"Status","reason":"Conflict","code":409,
 "message":"Operation cannot be fulfilled on deployments.apps \"web\": the object has been modified; please apply your changes to the latest version and try again"}
```

`metadata.generation` is `2` before and after; `spec.replicas` is `3` before and
after. The only thing that changed is `status`.

Same on a StatefulSet's scale subresource:

```
scale rv=4604 generation=1
status-only write: http=200
after: scale rv=4605 generation=1
PATCH …/statefulsets/db/scale with rv 4604 ->
{"kind":"Status","reason":"Conflict","code":409}
```

## 3. The full k8rs sequence, by curl — `GET` → `dryRun=All` PATCH → window → real PATCH

All three requests carry the `resourceVersion` the `GET` returned, and both
PATCHes carry `fieldValidation=Strict`, which is what `Pass::patch` sends.
Replicas alternate 4/3 so every patch is a real change.

**Control — healthy `web`, settled before every trial (`rollout status`, then 12 s
of quiet), 8 s window:**

```
web trial 1 rv=1770 replicas=4  dry-run=200 at +.011253879s  real=200 at +8.036396383s
web trial 2 rv=1879 replicas=3  dry-run=200 at +.015292473s  real=200 at +8.030610700s
web trial 3 rv=1923 replicas=4  dry-run=200 at +.015105649s  real=200 at +8.039820643s
web trial 4 rv=1973 replicas=3  dry-run=200 at +.016012583s  real=200 at +8.038548039s
web trial 5 rv=2016 replicas=4  dry-run=200 at +.015222388s  real=200 at +8.037785136s
```

5 of 5 succeeded.

**Healthy `web`, back to back with no settle (a second scale seconds after the
first), 8 s window:**

```
web trial 1 rv=2075 replicas=4  dry-run=200 at +.010859295s  real=200 at +8.032970910s
web trial 2 rv=2075 replicas=3  dry-run=200 at +.012255287s  real=200 at +8.037266681s
web trial 3 rv=2640 replicas=4  dry-run=409 at +.010410282s  real=409 at +8.027884909s
web trial 4 rv=2649 replicas=3  dry-run=200 at +.013379459s  real=200 at +8.030195845s
web trial 5 rv=2649 replicas=4  dry-run=200 at +.009166626s  real=200 at +8.034813814s
web trial 6 rv=2701 replicas=3  dry-run=409 at +.016066578s  real=409 at +8.037503836s
```

2 of 6 conflicted; both on the dry-run.

**`CrashLoopBackOff` `crashy`, 8 s window:**

```
crashy trial 1 rv=2099 replicas=4  dry-run=200 at +.009335417s  real=200 at +8.034280519s
crashy trial 2 rv=2124 replicas=3  dry-run=409 at +.011557789s  real=409 at +8.033161099s
crashy trial 3 rv=2164 replicas=4  dry-run=200 at +.013123666s  real=409 at +8.035785837s
crashy trial 4 rv=2172 replicas=3  dry-run=200 at +.011917348s  real=409 at +8.031567716s
crashy trial 5 rv=2198 replicas=4  dry-run=200 at +.009789625s  real=200 at +8.032181260s
crashy trial 6 rv=2198 replicas=3  dry-run=200 at +.014441841s  real=200 at +8.039070240s
crashy trial 7 rv=2233 replicas=4  dry-run=409 at +.009297257s  real=409 at +8.028222903s
crashy trial 8 rv=2262 replicas=3  dry-run=200 at +.015216823s  real=200 at +8.033794561s
```

4 of 8 conflicted; **2 of those had a `200` dry-run and a `409` real call.**

**`CrashLoopBackOff` `crashy`, 30 s window:**

```
crashy trial 1 rv=2262 replicas=4  dry-run=200 at +.014328258s  real=409 at +30.035505074s
crashy trial 2 rv=2346 replicas=3  dry-run=200 at +.010246914s  real=200 at +30.030627741s
crashy trial 3 rv=2346 replicas=4  dry-run=200 at +.010032043s  real=200 at +30.025737548s
crashy trial 4 rv=2449 replicas=3  dry-run=409 at +.020451020s  real=409 at +30.040147223s
crashy trial 5 rv=2529 replicas=4  dry-run=200 at +.012292766s  real=409 at +30.026704487s
```

3 of 5 conflicted; **2 of those after a `200` dry-run.**

The dry-run lands between **9 ms and 21 ms** after the `GET` in every trial
above; the real call lands at the window.

## 4. The same, through the built binary

`XDG_STATE_HOME` in a scratch directory; the operator's `yes` fed on stdin after
a measured delay.

**Healthy, settled, confirmed at once:**

```
$ echo yes | ./target/debug/k8rs ops scale deployment/web 3 -n payments
deployment/web in payments
This stops 1 copy of your app. Right now: 4 copies. After: 3 copies.
$ kubectl scale deployment/web --replicas=3 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
exit=0
```

Its two audit lines (`server` and `uid` as written, uid masked here):

```
2026-09-05T00:52:34.981268994Z attempt · deployment/web · context kind-review · server https://127.0.0.1:46307 · namespace payments · uid <uid> · kubectl: kubectl scale deployment/web --replicas=3 -n payments · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale · resourceVersion 2719
result · attempt 2026-09-05T00:52:34.981268994Z · recorded 2026-09-05T00:52:34.988823072Z · deployment/web · dry-run: the cluster checked it first and accepted it · the change was made
```

**`CrashLoopBackOff`, confirmed after 10 s, three runs.** Run 2:

```
deployment/crashy in payments
This starts 1 more copy of your app. Right now: 1 copy. After: 2 copies.
$ kubectl scale deployment/crashy --replicas=2 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: nothing was changed — the object had already been changed by something else, so look at it again before deciding whether you still want this change: Operation cannot be fulfilled on deployments.apps "crashy": the object has been modified; please apply your changes to the latest version and try again
exit=2
```

Its result line:

```
result · attempt 2026-09-05T00:52:55.651259854Z · recorded 2026-09-05T00:53:05.588182929Z · deployment/crashy · dry-run: the cluster checked it first and accepted it · nothing was changed — the object had already been changed by something else, so look at it again before deciding whether you still want this change: Operation cannot be fulfilled on deployments.apps "crashy": the object has been modified; please apply your changes to the latest version and try again
```

**Mid-rollout** (`minReadySeconds: 10`, 4 replicas, one `set image` issued
immediately before), confirmed after 10 s, three runs — runs 1 and 2 failed
after the confirmation, run 3 succeeded:

```
--- run 1 ---
This starts 1 more copy of your app. Right now: 4 copies. After: 5 copies.
$ kubectl scale deployment/web --replicas=5 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: nothing was changed — the object had already been changed by something else, so look at it again before deciding whether you still want this change: Operation cannot be fulfilled on deployments.apps "web": the object has been modified; please apply your changes to the latest version and try again
exit=2
```

**The taught line, run immediately after k8rs stopped.** Three attempts, each a
`set image` followed by a 6 s-window `k8rs ops scale`, followed by the exact
line k8rs had just printed:

```
======== attempt 2 ========
$ kubectl scale deployment/web --replicas=8 -n payments
k8rs: the change was never sent — the object had already been changed by something else, so look at it again before deciding whether you still want this change: …
-- the line k8rs printed, run right after:
deployment.apps/web scaled
kubectl exit=0

======== attempt 3 ========
$ kubectl scale deployment/web --replicas=9 -n payments
k8rs: the change was never sent — the object had already been changed by something else, so look at it again before deciding whether you still want this change: …
-- the line k8rs printed, run right after:
deployment.apps/web scaled
kubectl exit=0
```

Two of three: k8rs declined, and the command it had displayed one line above
succeeded on the next keystroke.

## 5. `fieldValidation=Strict` beside `metadata.resourceVersion`

The combination `Pass::patch` sends, never measured together before.

```
rv=3411 replicas=7
-- current rv + Strict + dryRun=All
{"kind":"Scale","replicas":7}   code=200
-- stale rv + Strict + dryRun=All   (object bumped with `kubectl label … --overwrite`)
{"kind":"Status","reason":"Conflict","code":409}
-- Strict + an unknown field (`spec.notAField`) beside a CURRENT rv
{"kind":"Status","reason":"Invalid","code":422}   strict decoding error: unknown field "spec.notAField"
```

`Strict` does not reject `metadata.resourceVersion` in a merge patch on the
scale subresource, does not mask the `409`, and still fires on an unknown field
when the version is current.

## 6. The `422` object echo, by media type

Deployment `web` given one container environment variable `PLANTED` holding the
fake literal `ZZZ-PLANTED-CANARY-ZZZ`. An unknown field under
`fieldValidation=Strict`, `dryRun=All`, on the **object** (not the subresource),
one request per media type. Sizes and canary counts only — the bodies are object
echoes and are not pasted:

```
Content-Type: application/merge-patch+json           -> body 13218 bytes, message 5132 bytes, canary 2
Content-Type: application/strategic-merge-patch+json -> body   514 bytes, message  122 bytes, canary 0
```

## 7. A `json-patch` `test` op as a precondition on `spec.replicas`

```
spec.replicas now = 7
first, bump the object with a status-only write: http=200
$ curl -X PATCH -H 'Content-Type: application/json-patch+json' \
   --data '[{"op":"test","path":"/spec/replicas","value":7},
            {"op":"replace","path":"/spec/replicas","value":8}]' \
   '…/deployments/web/scale?fieldValidation=Strict'
{"kind":"Scale","replicas":8}

-- now test a value that is NOT there:
$ … '[{"op":"test","path":"/spec/replicas","value":99}, {"op":"replace","path":"/spec/replicas","value":1}]'
{"kind":"Status","reason":"Invalid","code":422,
 "message":"the server rejected our request due to an error in our request"}
final replicas: 8
```

The `test` op survives a status-only write. When the tested value has moved the
answer is `422 Invalid` with a fifty-character generic message, not `409
Conflict`, and the message names neither the field nor either value.

## 8. RBAC — the documented `k8rs-admin` role, and `403` against `409`

The ClusterRole was copied verbatim from
[docs/security.md](../docs/security.md), bound to a ServiceAccount in
`payments`, and the binary run under a kubeconfig holding that identity.

```
$ kubectl auth whoami
Username   system:serviceaccount:payments:k8rs-ops
```

**With the documented role, the new body:**

```
This stops 5 copies of your app. Right now: 8 copies. After: 3 copies.
$ kubectl scale deployment/web --replicas=3 -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
exit=0
```

**`deployments/scale` verbs reduced to `["get"]`:**

```
k8rs: the change was never sent — the cluster would not allow it: deployments.apps "web" is forbidden: User "system:serviceaccount:payments:k8rs-ops" cannot patch resource "deployments/scale" in API group "apps" in the namespace "payments"
exit=2
```

**Reduced to `["patch"]` — the read is refused before the dialog:**

```
k8rs: k8rs could not read how many copies of deployment/web are running right now — the cluster would not allow it: deployments.apps "web" is forbidden: User "system:serviceaccount:payments:k8rs-ops" cannot get resource "deployments/scale" in API group "apps" in the namespace "payments"
exit=2
```

**A `409` under the same identity, for comparison:**

```
k8rs: nothing was changed — the object had already been changed by something else, so look at it again before deciding whether you still want this change: Operation cannot be fulfilled on deployments.apps "web": the object has been modified; please apply your changes to the latest version and try again
exit=2
```

**`--read-only`:**

```
$ echo yes | ./target/debug/k8rs --read-only ops scale deployment/web 2 -n payments
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation
exit=2
```

## What I could not measure

- **A conflict rate on a production-sized cluster.** Every number above is one
  kind node with three to nine pods. The write *rate* of a Deployment is driven
  by pod transitions, so a bigger workload has more of them, not fewer; I did
  not measure that and the direction is inferred, not observed.
- **An HPA that actually scales.** No metrics server was installed, so the HPA
  measured is one that reports `cpu: <unknown>` and never writes. What an
  actively scaling HPA does to `/scale` was not observed; every other write to
  `/scale` in this run bumped the version.
- **The `409` failure rate through the TUI.** Phase 11's dialog does not exist;
  the windows above are 6 s, 8 s, 10 s and 30 s of a headless `stdin` read,
  chosen as plausible, not sampled from anybody's behaviour.
- **Whether the two refusals in `scale` are reachable at all.** The apiserver
  populated `metadata.resourceVersion` on every `GET …/scale` in this run and the
  previous one; I did not construct a server that omits it or that returns one
  the strip would alter.
- **`kubectl scale --current-replicas`.** Still not sent — § 5 of the companion
  report quoted its help text and this run did not exercise it either.

## Machine state

```
$ free -g   # before the cluster
Mem: total 23  used 10  free 4  available 12
$ df -h /tmp
tmpfs  12G  8,9G  2,8G  77% /tmp
$ nproc
12
$ uptime
 03:31:44 up 2 days,  8:19,  2 users,  load average: 2,08, 3,77, 3,86
```

One cluster at a time: `kind get clusters` printed `k8rs` alone after teardown,
and `docker ps` listed exactly the PM's four containers.
