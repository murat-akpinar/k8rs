# `restart` round two — the paused flag on the wire, the `/scale` strict `422`, and a check that was never sent (2026-09-04)

One ephemeral `K8RS_CLUSTER=review` kind cluster, single node, apiserver bound to
`127.0.0.1:6444` so the PM's fixture cluster on `:6443` was untouched. Created,
measured, and deleted from a `trap … EXIT` (NOTES § D185). Everything below ran
on the dev machine. No committed artifact was produced; nothing was written into
`tests/`. The binary is `cargo build` from the uncommitted working tree, with its
own `CARGO_TARGET_DIR` under `$HOME` and its own `XDG_STATE_HOME`.

```
$ kind version
kind v0.32.0 go1.26.5-X:nodwarf5 linux/amd64
$ kubectl version -o json | jq -r '"server \(.serverVersion.gitVersion)  client \(.clientVersion.gitVersion)"'
server v1.36.1  client v1.36.3
```

The object under test: a 3-replica Deployment `payments/web` created with
`kubectl apply`, one container, carrying a **planted literal** in
`spec.template.spec.containers[0].env[0].value` — so `kubectl apply` also put a
copy of it inside the annotation `kubectl apply` writes to remember the last
manifest it sent (371 bytes; the key is not spelled here — see
`reports/README.md`). Beside it a 1-replica StatefulSet `payments/db`.

---

## 1. The dry-run `PATCH` response carries `spec.paused`

Raw `PATCH` through `kubectl proxy`, with exactly what `ops::restart` sends —
`Content-Type: application/strategic-merge-patch+json`, `?dryRun=All&fieldValidation=Strict`,
body `{"spec":{"template":{"metadata":{"annotations":{"kubectl.kubernetes.io/restartedAt":"<stamp>"}}}}}`.

**Before the pause:**

```
http body bytes: 6654
kind=Deployment  spec.paused=null  spec.replicas=3
top-level keys: apiVersion,kind,metadata,spec,status
spec keys: progressDeadlineSeconds,replicas,revisionHistoryLimit,selector,strategy,template
```

`spec.paused` is **absent**, not `false`.

**After `kubectl -n payments rollout pause deployment/web`:**

```
$ kubectl -n payments get deployment web -o jsonpath='{.spec.paused}'
true

http body bytes: 6919
kind=Deployment  spec.paused=true  type=boolean
spec keys: paused, progressDeadlineSeconds, replicas, revisionHistoryLimit, selector, strategy, template
```

The value at the JSON pointer `ops::paused` reads — `/spec/paused` — is present
on the **check pass** and is the JSON boolean `true`. Selected fields of that
same response, nothing else pasted:

```
apiVersion=apps/v1  kind=Deployment  metadata.name=web
spec.paused=true  spec.replicas=3
spec.template.metadata.annotations["kubectl.kubernetes.io/restartedAt"] = <the stamp k8rs's patch sent>
```

**Same request against the StatefulSet:**

```
kind=StatefulSet  spec.paused=null
spec keys: persistentVolumeClaimRetentionPolicy, podManagementPolicy, replicas,
           revisionHistoryLimit, selector, serviceName, template, updateStrategy
```

**What else that 6919-byte response holds** — the reason the closure maps to a
`bool`:

```
grep -c <the planted literal>   in the dry-run response body: 2
grep -c managedFields                                        : 1
spec.template.spec.containers[0].env[0]: name=PLANTED_SECRET, value=<the planted literal>
metadata.annotations[<the annotation kubectl apply writes>]: present, 371 bytes,
  and it contains the planted literal (grep -c = 1)
```

## 2. The real binary, paused and unpaused

Paused Deployment (stderr, verbatim, one line unwrapped per line here):

```
$ echo 'yes' | k8rs ops restart deploy/web -n payments
deployment/web in payments
This asks Kubernetes to replace every copy of your app with a new one. How many stop at the same time is a setting on this deployment — it can be a few, or all of them at once. A paused deployment will not start until you resume it.
$ kubectl rollout restart deployment/web -n payments
This deployment is paused, so nothing will be replaced until somebody resumes it with kubectl rollout resume — and the command above will refuse to run until then.
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
  exit=0
```

Pod names before and 10 s after: identical, three of three.

After `kubectl -n payments rollout resume deployment/web`
(`spec.paused` absent again):

```
$ echo 'yes' | k8rs ops restart deploy/web -n payments
deployment/web in payments
This asks Kubernetes to replace every copy of your app with a new one. How many stop at the same time is a setting on this deployment — it can be a few, or all of them at once. A paused deployment will not start until you resume it.
$ kubectl rollout restart deployment/web -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
  exit=0
```

No warning line. Same for the StatefulSet:

```
$ echo 'yes' | k8rs ops restart statefulset/db -n payments
statefulset/db in payments
This asks Kubernetes to replace every copy of your app with a new one, working down from the highest-numbered copy. How many stop at the same time, how far down it goes, and whether it waits for you to delete a copy yourself are all settings on this statefulset.
$ kubectl rollout restart statefulset/db -n payments
the cluster checked it first and accepted it
type yes and press enter to go ahead — anything else stops it:
k8rs: the change was made
  exit=0
```

## 3. The audit log those three runs produced

Server URL replaced here; mode `600`; the planted literal appears 0 times.

```
… attempt · deployment/web · context kind-review · server <url> · namespace payments · uid not read · kubectl: kubectl rollout restart deployment/web -n payments · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web · resourceVersion not sent
result · … · deployment/web · dry-run: the cluster checked it first and accepted it · the change was made      ← the paused run
… attempt · deployment/web · …
result · … · deployment/web · dry-run: the cluster checked it first and accepted it · the change was made      ← the resumed run
… attempt · statefulset/db · …
result · … · statefulset/db · dry-run: the cluster checked it first and accepted it · the change was made
```

```
$ grep -ci 'paus' audit.log
0
```

The paused run and the resumed run are byte-identical on every field of both
lines except the two timestamps.

## 4. A strict `422` on the `autoscaling/v1` `/scale` subresource

Raw `PATCH` to `…/deployments/web/scale?dryRun=All&fieldValidation=Strict`, body
carrying an unknown field. Byte counts are of `.message` on the returned
`Status`.

| Content-Type | HTTP | `len(.message)` | planted env literal | `managedFields` | `containers` | annotations |
|---|---|---|---|---|---|---|
| `application/merge-patch+json` (what `scale` sends) | 422 | **646** | no | **yes** | no | no |
| `application/strategic-merge-patch+json` | 422 | **120** | no | no | no | no |

The merge-patch message, with the two identifiers it carries redacted:

```
 "" is invalid: patch: Invalid value: "{\"apiVersion\":\"autoscaling/v1\",\"kind\":\"Scale\",
 \"metadata\":{\"name\":\"web\",\"namespace\":\"payments\",\"uid\":\"<uid>\",
 \"resourceVersion\":\"<rv>\",\"creationTimestamp\":\"<ts>\",
 \"managedFields\":[{\"manager\":\"kubectl-client-side-apply\",\"operation\":\"Update\",
 \"apiVersion\":\"autoscaling/v1\",\"time\":\"<ts>\",\"fieldsType\":\"FieldsV1\",
 \"fieldsV1\":{\"f:spec\":{\"f:replicas\":{}}}}]},
 \"spec\":{\"replicas\":2,\"wat\":1},\"status\":{\"replicas\":3,\"selector\":\"app=web\"}}":
 strict decoding error: unknown field "spec.wat"
```

The strategic-merge message, whole:

```
 "" is invalid: patch: Invalid value: "map[spec:map[replicas:2 wat:1]]": strict decoding error: unknown field "spec.wat"
```

An unknown field one level deeper, under merge-patch (`metadata.bogusField`):

```
len(.message) = 668
```

The object the merge-patch message quotes, decoded:

```
embedded object bytes = 485   (of the 646-byte .message)
metadata keys : creationTimestamp, managedFields, name, namespace, resourceVersion, uid
spec keys     : replicas, wat
status keys   : replicas, selector
managedFields entries: 1  (manager "kubectl-client-side-apply", fieldsV1 {"f:spec":{"f:replicas":{}}})
```

The Deployment carried an annotation holding the planted literal
(§ 1) and a container environment holding it, and
**neither appeared in any `/scale` message** — grep count 0 on both media types.
`managedFields` **did**.

## 5. `Outcome::NotSent` against a server that was never reached

Two kubeconfigs, each `current-context: dead`, `namespace: payments`, pointing at
a loopback port.

**A — nothing listening on the port.**

```
$ ss -ltn | grep -c ':6555'
0
$ echo 'yes' | k8rs ops restart deploy/web -n payments
deployment/web in payments
This asks Kubernetes to replace every copy of your app with a new one. …
$ kubectl rollout restart deployment/web -n payments
k8rs: the change was never sent — k8rs could not reach the cluster
  exit=2
```

audit log:

```
2026-09-04T16:35:28.165602655Z attempt · deployment/web · context dead · server <url> · … · call: PATCH /apis/apps/v1/namespaces/payments/deployments/web · resourceVersion not sent
result · attempt 2026-09-04T16:35:28.165602655Z · recorded 2026-09-04T16:35:28.165908013Z · deployment/web · dry-run: the check was sent and did not pass · the change was never sent — k8rs could not reach the cluster
```

Attempt to result: **305 µs** (`.165602655` → `.165908013`). No TCP connection was established, so no
`?dryRun=All` request left the machine.

**B — a TLS server that completes the handshake, reads the request, then closes.**
Self-signed cert, `openssl req -x509`, ALPN `h2, http/1.1`; kubeconfig
`insecure-skip-tls-verify: true`.

```
$ echo 'yes' | k8rs ops restart deploy/web -n payments
k8rs: the change was never sent — k8rs could not reach the cluster
  exit=2
```

server side:

```
[server] handshake done, read 87 bytes, closing
[server] handshake done, read 87 bytes, closing
[server] handshake done, read 253 bytes, closing
[server] handshake done, read 386 bytes, closing
```

audit log:

```
result · … · deployment/web · dry-run: the check was sent and did not pass · the change was never sent — k8rs could not reach the cluster
```

A and B produce the **same** `dry-run:` field. In B the request was sent (386
bytes read off the socket); in A it was not. Both classify as
`Fault::Unanswered` (`k8s.rs:1274`, the `_ =>` arm), so the fault alone does not
separate them either.

Neither run crashed, neither retried, both exited `2`.

## 6. Nothing hostile reaches `ops::a_kind` from a command line

```
$ echo 'yes' | k8rs ops restart $'job\x1b[2J‮/web' -n payments
k8rs: k8rs does not work on a kind called job[2J (with what cannot print removed) — the ones an operation can be pointed at are deployment, statefulset, daemonset, replicaset, pod and node

$ echo 'yes' | k8rs ops restart service/web -n payments
k8rs: k8rs does not work on a kind called service — the ones an operation can be pointed at are deployment, statefulset, daemonset, replicaset, pod and node

$ echo 'yes' | k8rs ops restart node/n1
k8rs: k8rs cannot restart a node — restarting replaces the copies an object is running, and k8rs does that for a deployment, a statefulset and a daemonset
```

`main.rs`'s `known_kind` answers first for anything that is not one of six
canonical singulars, and it **echoes the stripped word with a parenthetical**;
`ops.rs`'s `a_kind` — reached only for a canonical kind the operation does not
serve — **declines to echo**. Both refusals were produced by the same build.

The three other argv paths, same build, kubeconfig pointing at the dead port:

```
$ echo 'yes' | k8rs ops restart pod/web-abc -n payments
k8rs: k8rs will not restart a pod: restarting a pod means deleting it and letting the thing that created it start a replacement. k8rs restarts a deployment, a statefulset and a daemonset — if this pod belongs to one, restart that instead

$ echo 'yes' | k8rs ops restart rs/web-abc -n payments
k8rs: k8rs cannot restart a replicaset: a replicaset is normally made by a deployment, and restarting that deployment is what replaces its copies. k8rs restarts a deployment, a statefulset and a daemonset

$ echo 'yes' | k8rs --read-only ops restart deploy/web -n payments
k8rs: --read-only was asked for, so k8rs will not change anything — run it without that flag to use an operation
```

```
$ find $XDG_STATE_HOME -type f
(nothing)
```

No audit log is opened by any of the six refusals above.

## 7. `screens/dialogs.md` § Restart against the source strings

Extracted from the markdown (box art unwrapped at `│  │`, bullets and the
`Printed instead of drawn` block unwrapped at their own wrap points) and compared
byte for byte against the Rust string literals with line continuations folded:

```
MATCH   deployment consequence (box art)      vs src/ops.rs rollout()
MATCH   deployment consequence (printed)      vs src/ops.rs rollout()
MATCH   statefulset consequence (bullet)      vs src/ops.rs rollout()
MATCH   daemonset consequence (bullet)        vs src/ops.rs rollout()
MATCH   paused warning (box art)              vs src/main.rs while_paused()
MATCH   paused warning (printed)              vs src/main.rs while_paused()
MATCH   pod refusal                           vs src/ops.rs pod_is_a_delete()
MATCH   replicaset refusal                    vs src/ops.rs rollout()
MATCH   service refusal (inline)              vs src/ops.rs rollout() + a_kind()
```

## 8. `screens/help.md`'s `r` row

```
line 22  len=70  '│    s       run more or fewer copies       (scale)                  │'
line 23  len=70  "│    r       restart, at the object's own pace    (rollout restart)  │"
```

Both rows are 70 columns. The parenthetical starts at column 43 on the `s` row
and at column 49 on the `r` row.

## What could not be measured

- **A socket that dies between the request and the response, against a real
  apiserver.** § 5 B is a stand-in TLS server, not Kubernetes; it establishes
  that the request left the machine, not what the apiserver had done with it by
  then.
- **Whether the apiserver ever omits `spec.paused` from a dry-run response it is
  present on the object for.** Measured present on one apiserver version
  (`v1.36.1`) on one paused Deployment; not swept across versions.
