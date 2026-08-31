# `--describe` and `--yaml` against a cluster — measurements

Operator review of Phase 6 family 2 ("one object's own story"), against the
working tree at `1f89b8d` + the uncommitted family diff. `cargo build` (dev
profile) was the binary under test.

Two clusters were used and neither produced a committed artifact:

- **`kind-k8rs`**, the PM's fixture cluster, **read only** — 4 nodes, server
  `v1.36.1`, 8 days old, 25 pods in `default`. Nothing was created, patched or
  deleted on it.
- **`kind-review`**, a single-node cluster this review brought up for the writes
  the measurements needed, and **deleted before this file was written**
  (`kind get clusters` afterwards prints `k8rs` alone; the `kind-review`
  context, cluster and user were removed from the kubeconfig).

---

## 1. `--yaml --kind secret` against a Secret created with `kubectl apply`

### The object shape

`kubectl apply` writes the whole applied body into
`metadata`'s `…/last-applied-configuration` entry, for any kind. Measured first
on the fixture cluster, on a Pod, where the annotation carries the pod's whole
spec:

```
$ kubectl get pod broken-exit0 -n default -o jsonpath='{.metadata.annotations}'
```

returns a single entry whose key ends `/last-applied-configuration` and whose
value is the JSON body of `scripts/broken.yaml`'s entry for that pod.

```
$ kubectl get all -A -o json | grep -c "last-applied-configuration"
26
```

### The same shape on a Secret

On `kind-review`, a two-key `Opaque` Secret was applied with `kubectl apply -f`.
Reading its annotation back:

```
$ kubectl --context kind-review get secret db-credentials -n default \
    -o jsonpath='{.metadata.annotations}'
```

The entry ending `/last-applied-configuration` carries a JSON body with a
top-level `data` object holding **both base64 values verbatim** — the same two
strings that sit in the object's own `data`.

### What k8rs printed

```
$ ./target/debug/k8rs --yaml --kind secret --object default/db-credentials --context kind-review
```

stderr: `$ kubectl get secret db-credentials -n default -o yaml`
exit: `0`

stdout contained, in this order:

- the annotation entry ending `/last-applied-configuration`, whose value is the
  JSON body **including both base64 values, unmasked**;
- eleven lines later, the object's own block, where the same two values read
  `username: <hidden — 5 bytes>` and the second key `<hidden — 8 bytes>`.

The 8-byte value decodes to an 8-character word:

```
$ echo -n '<the base64 the annotation carried>' | base64 -d | wc -c
8
```

A second Secret was applied through `stringData` instead of `data`. The API
server cleared `stringData` and served `data.note: <hidden — 18 bytes>` — and
the annotation carried the **plaintext** `stringData` body, not base64.

### Where masking runs

`src/k8s.rs:5697` `fn mask` reads `document.get("data")` and
`document.get("stringData")` — both top level. Nothing walks
`metadata.annotations`.

### Two Secret shapes that did *not* leak

Both created with `kubectl create` / by the controller, so neither carries the
annotation:

```
$ ./target/debug/k8rs --yaml --kind secret --object default/regcred --context kind-review
data:
  .dockerconfigjson: <hidden — 125 bytes>
type: kubernetes.io/dockerconfigjson
```

```
$ ./target/debug/k8rs --yaml --kind secret --object default/default-token --context kind-review
```

The three keys of a `kubernetes.io/service-account-token` Secret each came back
as a size and nothing else: `ca.crt` at 1,107 bytes, `namespace` at 7 bytes, and
the JWT key at 892 bytes. No `eyJ`-shaped run reached stdout.

### What the test was fed

`src/k8s_tests.rs`, `a_secrets_values_are_replaced_by_their_sizes_and_are_nowhere_in_what_is_printed`
asserts `!printed.contains(<the base64>)`. Its input document has no
`metadata` entries at all.

---

## 2. What a newline inside a value came out as

### On the fixture cluster

```
$ kubectl get cm coredns -n kube-system -o yaml | wc -l
33
$ ./target/debug/k8rs --yaml --kind configmap --object kube-system/coredns 2>/dev/null | wc -l
20
```

The `Corefile` key is 20 lines in `kubectl`'s output and **one** line in k8rs's:
every `\n` came back as a single space and the value's own indentation survived
as runs of spaces mid-line.

### On a planted object (`kind-review`)

A ConfigMap was created with three multi-line values and two unprintable
characters. k8rs's `--yaml` output:

```
    esc: a[31mRED[0m b            <- ESC removed, invariant 9 holds
    multi: one two three          <- was "one\ntwo\nthree"
    note: prodreversed            <- U+202E removed, invariant 9 holds
  conf: line1 line2               <- was "line1\nline2\n"
  big: xxxxxxxx…                  <- 60 000 bytes, held and printed whole
```

Checked programmatically over the same output: no `ESC`, no `U+202E`, no
`… (shortened by k8rs)` marker, 121 010 bytes total.

`src/k8s.rs:5737` `fn clean` calls `text(held, usize::MAX)`; `text`
(`src/k8s.rs:278`) turns an `unprintable` character that is `is_whitespace`
into one space.

### What the test was fed

`nothing_unprintable_survives_into_the_document` (`src/k8s_tests.rs`) feeds
U+200D, U+202E and U+200B. No input in the family's tests carries a `\n` inside
a string value.

---

## 3. `managedFields`, in k8rs's output and in `kubectl`'s

```
$ kubectl get cm coredns -n kube-system -o yaml | grep -c "managedFields\|f:Corefile"
0
$ kubectl get cm coredns -n kube-system -o yaml --show-managed-fields | grep -c "managedFields\|f:Corefile"
2
$ ./target/debug/k8rs --yaml --kind configmap --object kube-system/coredns 2>/dev/null | grep -c "managedFields\|f:Corefile"
2
```

On a pod:

```
$ ./target/debug/k8rs --yaml --object default/broken-exit0 2>/dev/null | wc -l
246
$ kubectl get pod broken-exit0 -n default -o yaml | wc -l
151
$ kubectl get pod broken-exit0 -n default -o yaml --show-managed-fields | wc -l
247
```

95 of the 246 lines k8rs prints (39%) are the block `kubectl get -o yaml` has
hidden by default since v1.21.

---

## 4. Event `count` and the timestamps

### Distinct `Event` objects per involved object, fixture cluster, 8 days old

```
$ kubectl get events -A -o json | python3 <group by involvedObject, count>
8 Pod default broken-hostpath
4 Pod default broken-ds-5885d
4 Pod default broken-exit0
4 Pod default broken-podlimit
4 Pod default broken-rollout-764f96ccf7-jz9rm
4 Pod default broken-rollout-764f96ccf7-k8j9v
4 Pod default broken-sts-0
4 Pod default healthy-deploy-7f84bdfb9b-9frpf
total events: 56 max count field: 27639
```

The `EVENTS_KEPT = 500` doc comment records 8 distinct events and `count: 26787`
for this cluster; re-measured today the maximum distinct count is still 8 and the
maximum `count` field reads `27639`.

### The `count` field, one object

```
$ kubectl get events -n default --field-selector involvedObject.name=broken-exit0 \
    -o custom-columns='REASON:.reason,COUNT:.count,FIRST:.firstTimestamp,LAST:.lastTimestamp'
REASON      COUNT   FIRST                  LAST
Pulling     1212    2026-08-26T16:37:23Z   2026-08-31T00:43:00Z
Unhealthy   2383    2026-08-26T16:37:29Z   2026-08-31T00:43:01Z
BackOff     5320    2026-08-26T16:37:32Z   2026-08-31T00:33:45Z
Pulled      1084    2026-08-26T17:03:48Z   2026-08-31T00:37:32Z
```

`kubectl describe pod broken-exit0` renders the second row as
`Unhealthy  3m14s (x2383 over 4d8h)`.

k8rs renders the same row as one line with no count and no first-seen:

```
$ ./target/debug/k8rs --describe --object default/broken-exit0
$ kubectl describe pod broken-exit0 -n default
Pod · running · created 8 days ago

containers:
  batch   waiting, 1461 restarts

events (newest first):
  2 min ago   the health check failed
  2 min ago   the container started pulling its image
  8 min ago   the image finished downloading
  12 min ago  Back-off restarting failed container batch in pod broken-exit0_default(<uid>)
exit=0
```

`src/k8s.rs:5467` `struct Happening` carries `at`, `reason`, `message` and no
`count`.

### `remainingItemCount` under a field selector

```
$ kubectl get --raw '/api/v1/namespaces/default/events?fieldSelector=involvedObject.name%3Dbroken-exit0&limit=1'
metadata: {'continue': '<opaque>'}

$ kubectl get --raw '/api/v1/namespaces/default/events?limit=1'
metadata: {'continue': '<opaque>', 'remainingItemCount': 52}
```

With a field selector `remainingItemCount` is absent from the response and
`continue` is present; without one both are present.

The `continue` token from the first call decodes to a `start` key of
`broken-exit0.18cf6849d7fc25fc` — one object's events are keyed in etcd by event
name, and on the legacy path that name embeds the creation time, so the order the
server returns them in is time-ascending.

### `series.lastObservedTime` is not read

An `events.k8s.io/v1` Event with a `series` was created on `kind-review`
against a running pod, `eventTime` seven days before `series.lastObservedTime`.
Read back through `core/v1`:

```
{'reason': 'Unhealthy', 'count': None, 'firstTimestamp': None, 'lastTimestamp': None,
 'eventTime': '2026-08-24T10:00:00Z'}
 series= {'count': 900, 'lastObservedTime': '2026-08-31T00:55:00Z'}
```

`src/k8s.rs:5619` reads `last_timestamp` → `event_time` →
`metadata.creation_timestamp`. `series.last_observed_time` is not in the chain.

At `now: 2026-08-31T00:53:01Z`:

```
$ ./target/debug/k8rs --describe --object default/probe --context kind-review
events (newest first):
  10s ago     the image finished downloading
  10s ago     Container created
  10s ago     Container started
  13s ago     the container started pulling its image
  14s ago     kubernetes placed this pod on a node
  6 days ago  the health check failed        <- series.lastObservedTime says "now"
```

`kubectl describe` on the same pod sorts that row **first** and prints
`Warning  Unhealthy  <invalid> (x900 over 6d14h)`.

---

## 5. The four translated reason words, beside the messages a cluster sent

Every distinct `(reason, message)` pair on the fixture cluster:

```
$ kubectl get events -A -o json | python3 <distinct reason -> messages>
BackOff          Back-off pulling image "registry.invalid/does-not-exist:v9"
                 Back-off restarting failed container batch in pod broken-exit0_default(<uid>)
Created          Container created
Failed           Error: ImagePullBackOff
                 Error: configmap "this-configmap-does-not-exist" not found
                 Failed to pull image "registry.invalid/does-not-exist:v9": …
FailedCreate     Error creating: pods "…" is forbidden: exceeded quota: deny-all-pods, …
FailedScheduling 0/4 nodes are available: 1 node(s) had untolerated taint(s), …
Pulled           (combined from similar events): Successfully pulled image "busybox" in 1.022s …
                 Successfully pulled image "busybox" in 905ms …
Pulling          Pulling image "busybox"
                 Pulling image "registry.invalid/does-not-exist:v9"
Started          Container started
Unhealthy        Readiness probe failed:
```

### `Pulled` on a cached image

A pod was started on `kind-review` with `--image-pull-policy=IfNotPresent`
against an image already on the node.

```
$ kubectl --context kind-review get events -n default \
    --field-selector involvedObject.name=probe2 -o jsonpath=…
Scheduled | Successfully assigned default/probe2 to review-control-plane
Pulled | Container image "busybox" already present on machine and can be accessed by the pod
Created | Container created
Started | Container started

$ ./target/debug/k8rs --describe --object default/probe2 --context kind-review
events (newest first):
  8s ago  kubernetes placed this pod on a node
  8s ago  the image finished downloading
  8s ago  Container created
  8s ago  Container started
```

Two things in that block: the kubelet said the image was **already present**
and k8rs printed "the image finished downloading"; and the four rows carry one
timestamp each to the second, so the stable sort left them in the server's own
order — which for this object is time-**ascending** — under a heading reading
*newest first*.

### What the other three sentences drop

| reason | the message the cluster sent | what k8rs printed |
|---|---|---|
| `Unhealthy` | `Readiness probe failed: …` | the health check failed |
| `Scheduled` | `Successfully assigned default/probe2 to review-control-plane` | kubernetes placed this pod on a node |
| `Pulling` | `Pulling image "registry.invalid/does-not-exist:v9"` | the container started pulling its image |

`src/k8s.rs:5520` `Happening::plainly`.

---

## 6. `--describe` against `kubectl describe`, on the fixture cluster's broken pods

```
$ ./target/debug/k8rs --describe --object default/broken-neverback
$ kubectl describe pod broken-neverback -n default
Pod · failed · created 8 days ago

containers:
  broke    done
  done     done
  keeper   done
k8rs: Kubernetes only keeps events for a while, and this pod has run long enough that none are left.
exit=0
```

The three containers, from the API:

```
$ kubectl get pod broken-neverback -n default -o jsonpath=…
broke exit=1 reason=Error
done exit=0 reason=Completed
keeper exit=255 reason=Unknown
```

The "no events" sentence was checked and is true here:

```
$ kubectl get events -n default --field-selector involvedObject.name=broken-neverback
No resources found in default namespace.
```

Same for `broken-evicted` and `broken-succeeded`.

Other pods:

```
broken-image      Pod · pending   nope waiting     (kubectl: State Waiting, Reason ImagePullBackOff)
broken-config     Pod · pending   app waiting      (kubectl: Reason CreateContainerConfigError)
broken-exit0      Pod · running   batch waiting, 1461 restarts
                                  (kubectl get pods STATUS column: CrashLoopBackOff)
broken-succeeded  Pod · succeeded migrate done, 3 restarts
```

`kubectl describe pod broken-exit0` additionally carries, and k8rs does not:
`Node`, `IP`, `Image`, `Command`, `Last State` (reason + exit code + finished
time), `Restart Count`, `Readiness` probe spec, `QoS Class`, `Conditions`,
`Node-Selectors`, `Tolerations`, `Volumes`, `Controlled By`.

---

## 7. `--kind`, the same word, on both verbs

```
$ ./target/debug/k8rs --yaml   --object default/probe2 --kind pods  --context kind-review
$ kubectl get pod probe2 -n default -o yaml
$ ./target/debug/k8rs --yaml   --object default/probe2 --kind Pod   --context kind-review
$ kubectl get pod probe2 -n default -o yaml
$ ./target/debug/k8rs --yaml   --object default/probe2 --kind POD   --context kind-review
$ kubectl get pod probe2 -n default -o yaml

$ ./target/debug/k8rs --describe --object default/probe2 --kind pods --context kind-review
k8rs: --describe only knows how to read a pod right now — containers and events don't mean the
same thing on a Secret. --kind pod is the only value it accepts
$ ./target/debug/k8rs --describe --object default/probe2 --kind Pod  --context kind-review
(same refusal)
$ ./target/debug/k8rs --describe --object default/probe2 --kind POD  --context kind-review
(same refusal)
$ ./target/debug/k8rs --describe --object default/probe2 --kind pod  --context kind-review
$ kubectl describe pod probe2 -n default
```

`src/main.rs:3898` compares `kind != POD` on the raw word; `src/k8s.rs:5800`
`kind_named` lowercases and matches plural **or** kind.

---

## 8. The kubectl lines, pasted as a reader would

All checked against `kubectl` on the same cluster; every one below is a command
that runs and returns the same object.

```
$ ./target/debug/k8rs --yaml --kind node --object review-control-plane --context kind-review
$ kubectl get node review-control-plane -o yaml                      <- no -n, correct
$ ./target/debug/k8rs --yaml --kind deployment --object kube-system/coredns --context kind-review
$ kubectl get deployment.apps coredns -n kube-system -o yaml         <- group-qualified, runs
$ ./target/debug/k8rs --yaml --kind 'events.' --object default/<name> --context kind-review
$ kubectl get event <name> -n default -o yaml
$ ./target/debug/k8rs --yaml --kind 'events.events.k8s.io' --object default/<name> --context kind-review
$ kubectl get event.events.k8s.io <name> -n default -o yaml
```

Ambiguity and spelling:

```
$ ./target/debug/k8rs --yaml --kind events --object default/x --context kind-review
k8rs: --kind events matches two things this cluster serves — the original one, and the one
events.k8s.io adds. Say which: --kind 'events.' for the original one, or
--kind 'events.events.k8s.io' for the other
exit=2

$ ./target/debug/k8rs --yaml --kind widget --object default/x --context kind-review
k8rs: this cluster does not serve a kind named widget — check the spelling
exit=2
```

Both suggested spellings were then run and both resolved (above).

Missing objects:

```
$ ./target/debug/k8rs --yaml --kind secret --object default/ghost --context kind-review
$ kubectl get secret ghost -n default -o yaml
k8rs: there is no secret named ghost in default — check the name and the namespace
exit=2

$ ./target/debug/k8rs --yaml --kind node --object ghost --context kind-review
$ kubectl get node ghost -o yaml
k8rs: there is no node named ghost — check the name
exit=2

$ ./target/debug/k8rs --describe --object default/ghost --context kind-review
k8rs: there is no pod named ghost in default — check the name and the namespace
exit=2
```

`--yaml` prints its `kubectl` line **before** the read, so a 404 still teaches
the command. `--describe` and `--logs` print theirs **after** the pod read
succeeds, so a 404 teaches nothing.

Neither verb puts `--context` on the `kubectl` line it prints, and both were run
with `--context kind-review` above.

---

## 9. RBAC

`docs/security.md`'s read-only ClusterRole was extracted verbatim, bound to a
ServiceAccount on `kind-review`, and used through a kubeconfig holding that
ServiceAccount's token.

```
$ ./target/debug/k8rs --describe --object default/probe2
$ kubectl describe pod probe2 -n default
Pod · running · created 59s ago

containers:
  probe2   running

events (newest first):
  59s ago  kubernetes placed this pod on a node
  59s ago  the image finished downloading
  59s ago  Container created
  59s ago  Container started
exit=0

$ ./target/debug/k8rs --yaml --object default/probe2
exit=0

$ ./target/debug/k8rs --yaml --kind secret --object default/db-credentials
$ kubectl get secret db-credentials -n default -o yaml
k8rs: the role this kubeconfig uses needs to get the secret db-credentials in default
exit=2

$ ./target/debug/k8rs --yaml --kind configmap --object kube-system/coredns
$ kubectl get configmap coredns -n kube-system -o yaml
k8rs: the role this kubeconfig uses needs to get the configmap coredns in kube-system
exit=2
```

Under a second role granting `pods` only:

```
$ ./target/debug/k8rs --describe --object default/probe2
$ kubectl describe pod probe2 -n default
Pod · running · created 1 min ago

containers:
  probe2   running
k8rs: the role this kubeconfig uses needs to list events in default
exit=2
```

No crash, no retry loop, one degraded feature each, and the verb + resource
named every time. Reproduces the author's own measurement.

A malformed kubeconfig on the same path:

```
k8rs: no cluster to watch — the kubeconfig itself could not be read — it is missing,
unreadable, or not valid YAML
exit=2
```

A context that does not exist:

```
$ ./target/debug/k8rs --describe --object default/ghost --context kind-nope
k8rs: no cluster to watch — this kubeconfig has no such context — check the `--context` you
gave, or the `current-context` line in the file
```

---

## 10. `--yaml` on a Pod and `docs/security.md:354`

A pod was created on `kind-review` with `--env=DB_PASSWORD=<redacted, 8 chars>`.

```
$ ./target/debug/k8rs --yaml --object default/envpod --context kind-review | grep -A3 '^    env:'
```

Four lines came back: the `env` key, the entry naming `DB_PASSWORD`, the
entry's own value field carrying the 8-character literal **verbatim and
unmasked**, and the container's `resources` key.

`docs/security.md:354` reads "Environment variable **values are never
displayed**." `screens/detail.md` § The yaml tab argues the exception.

---

## 11. Argument handling

```
$ ./target/debug/k8rs --describe --yaml --object default/probe2
k8rs: --describe and --yaml each print a different thing about the same object, so k8rs will
not do more than one of them in a run — pick one   + usage

$ ./target/debug/k8rs --logs --describe --object default/probe2
k8rs: --logs and --describe each print a different thing …

$ ./target/debug/k8rs --yaml --object default/probe2 --kind
k8rs: --kind needs the name of a kind   + usage

$ ./target/debug/k8rs --describe
k8rs: --describe and --object go together — --describe says what to print and --object says
which object to print it for   + usage

$ ./target/debug/k8rs --object default/probe2
k8rs: --logs and --object go together — …   (falls back to --logs when no verb is on the line)

$ ./target/debug/k8rs --yaml --once --object default/probe2
$ kubectl get pod probe2 -n default -o yaml
(--once is silently ignored; --logs --once behaves the same way)
```

---

## 12. Cost of one run

Median of five, against `kind-review` (one node) from this machine:

```
k8rs --describe    : 0.041 s
k8rs --yaml        : 0.045 s
kubectl describe   : 0.085 s
kubectl get -o yaml: 0.077 s
```

`connect()` builds the five watch streams (`src/k8s.rs`, `fn watches`) but a
`watcher()` stream is lazy and neither `describe_run` nor `yaml_run` polls
`session.watches`, so no watch is opened. `coverage()`'s pod probe is
`.limit(1)`. The events fetch is one LIST with a field selector and a
server-side `limit`, not a poll.

---

## 13. Bounds

- 60 000-byte ConfigMap value: held and printed whole, no `shortened by k8rs`
  marker, 121 010 bytes on stdout. `clean` passes `usize::MAX`, per
  `screens/detail.md` § A very large object.
- 500-event bound: not reachable from `--describe` on either cluster — the
  busiest object had 8 distinct `Event` objects.
- `decoded_bytes` was checked against two real values: a 7-character base64 run
  reported 5 bytes and an 11-character run reported 8 bytes, both matching
  `base64 -d | wc -c`.

---

## 14. Teardown

```
$ docker rm -f review-control-plane
review-control-plane
$ kind get clusters
k8rs
$ kubectl config get-contexts -o name
kind-k8rs
$ kubectl config current-context
kind-k8rs
$ kubectl get secrets -A
No resources found
$ kubectl get pods -n default --no-headers | wc -l
25
```

The fixture cluster holds no Secret and the same 25 pods it started with.
