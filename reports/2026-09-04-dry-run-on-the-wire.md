# Server-side dry-run — what kubectl sends, and what kube sends

`k8s-admin`, 2026-09-04, step 6 over the uncommitted `src/ops.rs` (+79/−5) and
`src/ops_tests.rs` (+185/−15) on `development`, HEAD `891fc8d`.

**No Kubernetes cluster was brought up.** The reference client's wire format was
captured against a throwaway HTTP server on `127.0.0.1` that answers discovery
and prints every non-GET request it receives; k8rs's own wire format came from
the repo's tests and from a scratch probe over the vendored crate. Everything
below is a request *k8rs or kubectl built*, never an object from a cluster.

## 0. The rig

```
$ kubectl version --client
Client Version: v1.36.3
Kustomize Version: v5.8.1
```

A ~70-line `http.server` subclass serving `/api`, `/apis`, `/api/v1`,
`/apis/apps/v1`, `/apis/policy/v1` and canned objects for
`deployment/web`, `pod/web-5d9f-abcde` and `node/n1` in namespace `payments` —
the same names `src/ops_tests.rs` already uses. It prints `WIRE <method> <path>`
and the body for everything that is not a GET. A kubeconfig pointing one context
at `http://127.0.0.1:18443`. Both live in the run's scratchpad and are not
committed.

## 1. What kubectl puts on the wire with `--dry-run=server`

```
$ KUBECONFIG=<scratch> kubectl scale deployment/web --replicas=3 -n payments --dry-run=server
deployment.apps/web scaled (server dry run)

$ KUBECONFIG=<scratch> kubectl delete deployment web -n payments --dry-run=server
deployment.apps "web" deleted from payments namespace (server dry run)

$ KUBECONFIG=<scratch> kubectl delete deployment web -n payments        # no dry-run
(killed after the request went out; the fake server never reports the object gone)

$ KUBECONFIG=<scratch> kubectl drain n1 --ignore-daemonsets --force --dry-run=server
node/n1 cordoned (server dry run)
Warning: deleting Pods that declare no controller: payments/web-5d9f-abcde
evicting pod payments/web-5d9f-abcde (server dry run)
node/n1 drained (server dry run)

$ KUBECONFIG=<scratch> kubectl cordon n1 --dry-run=server
node/n1 cordoned (server dry run)

$ KUBECONFIG=<scratch> kubectl rollout restart deployment/web -n payments --dry-run=server
error: unknown flag: --dry-run
```

The server side of those runs, verbatim:

```
WIRE PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale?dryRun=All
     body {"spec":{"replicas":3}}

WIRE DELETE /apis/apps/v1/namespaces/payments/deployments/web
     body {"propagationPolicy":"Background","dryRun":["All"]}

WIRE DELETE /apis/apps/v1/namespaces/payments/deployments/web
     body {"propagationPolicy":"Background"}

WIRE PATCH /api/v1/nodes/n1?dryRun=All
     body {"spec":{"unschedulable":true}}

WIRE POST /api/v1/namespaces/payments/pods/web-5d9f-abcde/eviction
     body {"kind":"Eviction","apiVersion":"policy/v1","metadata":{"name":"web-5d9f-abcde","namespace":"payments"},"deleteOptions":{"dryRun":["All"]}}
```

`kubectl rollout restart` has no `--dry-run` flag in v1.36.3, so its own call was
captured without one, and separately the same call was issued through
`kubectl patch --dry-run=server` to see whether the verb accepts the marker:

```
$ KUBECONFIG=<scratch> kubectl rollout restart deployment/web -n payments
deployment.apps/web restarted

WIRE PATCH /apis/apps/v1/namespaces/payments/deployments/web?fieldManager=kubectl-rollout
     body {"spec":{"template":{"metadata":{"annotations":{"kubectl.kubernetes.io/restartedAt":"2026-09-04T03:15:03+03:00"}}}}}

$ KUBECONFIG=<scratch> kubectl patch deployment web -n payments --dry-run=server -p '<the same body>'
deployment.apps/web patched (no change)

WIRE PATCH /apis/apps/v1/namespaces/payments/deployments/web?dryRun=All&fieldManager=kubectl-patch
```

The annotation key kubectl writes: `kubectl.kubernetes.io/restartedAt`.

Field values this run turns on:

| operation | dry-run marker | where it rides |
|---|---|---|
| `scale` | `dryRun=All` | query string, on the `scale` subresource |
| `delete` | `["All"]` | **request body**, `DeleteOptions.dryRun` — no query string at all |
| `cordon` | `dryRun=All` | query string, `PATCH /api/v1/nodes/<name>` |
| `restart` | `dryRun=All` | query string, `PATCH …/deployments/<name>` |
| eviction | `["All"]` | request body, `Eviction.deleteOptions.dryRun` |

`kubectl delete` also sends `propagationPolicy: "Background"` on both passes.

## 2. What k8rs puts on the wire

```
$ CARGO_TARGET_DIR=<scratch> cargo test --quiet ops::tests::the_ -- --nocapture --test-threads=1
PATCH  /apis/apps/v1/namespaces/payments/deployments/web/scale?&dryRun=All
PATCH  /apis/apps/v1/namespaces/payments/deployments/web/scale?
DELETE /apis/apps/v1/namespaces/payments/deployments/web? · body {"dryRun":["All"]}
DELETE /apis/apps/v1/namespaces/payments/deployments/web? · body {}
POST   /api/v1/namespaces/payments/pods/web-5d9f-abcde/eviction?&dryRun=All
POST   /api/v1/namespaces/payments/pods/web-5d9f-abcde/eviction?
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 909 filtered out
```

The author's three lines reproduce exactly. Two spellings to note, both kube's
own and neither k8rs's: the check pass emits `?&dryRun=All` (an empty leading
query pair) and the real pass emits a bare trailing `?`.

## 3. The eviction body kube builds

Scratch probe, `kube-core = "=4.2.0"`, `Request::evict` with `Pass::post()`'s
`PostParams` and nothing else changed:

```
DRY_RUN  POST …/pods/web-5d9f-abcde/eviction?&dryRun=All
         body {"delete_options":null,"metadata":{"name":"web-5d9f-abcde"}}
FOR_REAL POST …/pods/web-5d9f-abcde/eviction?
         body {"delete_options":null,"metadata":{"name":"web-5d9f-abcde"}}
GRACE    POST …/pods/web-5d9f-abcde/eviction?&dryRun=All
         body {"delete_options":{"gracePeriodSeconds":30},"metadata":{"name":"web-5d9f-abcde"}}
```

Against §1's `deleteOptions`, the field values that differ: the key is
`delete_options` rather than `deleteOptions`, and `kind` / `apiVersion` /
`metadata.namespace` are absent. `GRACE` is the third case: a grace period set
through `EvictParams::delete_options` is serialised under the same key.

Source of the spelling, `kube-core-4.2.0/src/subresource.rs:119-121`:

```rust
let data = serde_json::to_vec(&serde_json::json!({
    "delete_options": ep.delete_options,
    "metadata": { "name": name }
}))
```

## 4. Field values read off the vendored crate

`kube-core-4.2.0/src/params.rs`:

| item | line | value |
|---|---|---|
| `PostParams` fields | 535-541 | `dry_run`, `field_manager` — **no `field_validation`** |
| `PatchParams` fields | 664-674 | `dry_run`, `force`, `field_manager`, `field_validation` |
| `DeleteParams` fields | 763-791 | `dry_run`, `grace_period_seconds`, `propagation_policy`, `preconditions` — **no `field_validation`** |
| `DeleteParams::dry_run` serde | 765-769 | `serialize_with = "dry_run_all_ser"`, emits `["All"]`, skipped when false |

`kube-core-4.2.0/src/request.rs:107-116` — `Request::delete` builds the query
with no pairs appended and serialises `DeleteParams` into the body
unconditionally. `Request::delete_collection` (`:127`) is the one that sends an
empty body when the params are default; `Request::delete` is not.

## 4b. The two kube helpers `Mutation::checkable: false` names

`kube-core-4.2.0/src/util.rs:21-37` — `Request::restart` builds its own
`PatchParams::default()` and a merge patch that sets one annotation on
`spec.template.metadata`, keyed `kube.kubernetes.io/restartedAt` (line 28) to
`Timestamp::now().to_string()`. §1 measured kubectl writing the same value under
`kubectl.kubernetes.io/restartedAt`. The two keys are the field this run turns
on; the values are timestamps and are not otherwise interesting.

`kube-core-4.2.0/src/util.rs:43-60` — `cordon` / `uncordon` call
`set_unschedulable`, which sends `Patch::Strategic({"spec":{"unschedulable":
<bool>}})` with `PatchParams::default()`.

Neither helper takes a params argument, so neither can carry `dryRun=All`. The
paths they build are the same paths §1 shows kubectl sending `dryRun=All` on:
`/apis/apps/v1/namespaces/<ns>/deployments/<name>` and `/api/v1/nodes/<name>`.

## 4c. The fault a PDB-blocked eviction would carry

`src/k8s.rs:1128-1145` — `answer()` has arms for `400 401 403 404 409 422` and a
reason fallback of `BadRequest / Unauthorized / Forbidden / NotFound / Conflict /
Invalid`. A `429` with `reason: "TooManyRequests"` — what the eviction subresource
answers when a PodDisruptionBudget will not allow the eviction — matches none of
them and falls to `Fault::Unanswered`, whose sentence at `src/ops.rs:642` is
*"k8rs could not reach the cluster"*.

## 5. Upstream text

`tmp/k8s/api-concepts.txt:730-760` — "When you use HTTP verbs that can modify
resources (POST, PUT, PATCH, and DELETE), you can submit your request in a dry
run mode"; "Dry-run is triggered by setting the dryRun query parameter";
"Authorization for dry-run and non-dry-run requests is identical".

`tmp/k8s/api-concepts.txt:684` names `dryRun` alongside `gracePeriodSeconds`,
`orphanDependents`, `preconditions` and `propagationPolicy` as *fields* of a
delete request — the `DeleteOptions` body.

`tmp/k8s/api-concepts.txt:719` — "The field validation level is set by the
fieldValidation query parameter."

## 6. Cleanup owed

The fake server was stopped (`pgrep -af fake-apiserver.py` returns nothing). Two
scratch cargo target directories could not be removed by this run — the sandbox
refused the `rm` — and are the user's to delete:
`~/.cache/k8rs-review-target` and `~/.cache/k8rs-review-target-evict`.
Nothing was written inside the repository except this file.
