# `fieldValidation=Strict` on the mutation contract — what was measured

`k8s-admin`, 2026-09-04, step 6 over `todo.md:3679` on top of `f851bf0` plus the
uncommitted `src/ops.rs` / `src/ops_tests.rs` diff.

**No cluster was brought up.** The four-node `k8rs` fixture cluster was running
throughout and belongs to the PM. Everything below is either a wire capture
against a local recording HTTP server, a run of the repo's own tests in a copy of
the tree, or a read of upstream source at the tag the fixture cluster runs
(`release-1.36`, `tests/fixtures/K8S_VERSION`). Where a claim could not be
measured without a cluster it says so.

## Environment note — `/tmp` was 100% full for the whole session

```
$ df -h /tmp
Filesystem      Size  Used Avail Use% Mounted on
tmpfs            12G   12G  128K 100% /tmp
```

12G of it is `/tmp/claude-1000/…` agent scratch trees (7.9G + 2.1G + 1.7G in
three session directories). `scripts/mutants.sh` is unaffected — it names
`${XDG_CACHE_HOME:-$HOME/.cache}/k8rs-mutants` and exports `TMPDIR` to it
(`scripts/mutants.sh:53`, `:440`) — so the box's `just mutants-diff` result is
not a D133 case. Everything below therefore ran with `TMPDIR` and
`CARGO_TARGET_DIR` under `$HOME`.

## 1 — what `Pass::patch()` and `Pass::delete()` put on the wire

Repo copied to `$HOME/.cache/…/tree`, own `CARGO_TARGET_DIR`, own `TMPDIR`.

```
$ cargo test --offline ops::tests -- --nocapture --test-threads=1
…
PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale?&dryRun=All&fieldValidation=Strict
PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale?&fieldValidation=Strict
DELETE /apis/apps/v1/namespaces/payments/deployments/web? · body {"dryRun":["All"]}
DELETE /apis/apps/v1/namespaces/payments/deployments/web? · body {}

test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 890 filtered out
```

Matches the diff's claim exactly.

`kube-core-4.2.0/src/params.rs`, read for the same claim:

- `PatchParams::populate_qp` appends `fieldValidation` for **any** patch, with no
  branch on `Patch::Apply` — `:705-707`. Its own tests at `:909-926` assert
  `some/resource?&fieldValidation=Ignore|Warn|Strict`.
- `PatchParams::validate` (`:675-692`) refuses `force` on a non-apply and says
  nothing about `field_validation`.
- `DeleteParams` (`:763-791`) has four fields — `dry_run`, `grace_period_seconds`,
  `propagation_policy`, `preconditions` — and no `field_validation`.
- `PostParams` (`:534-540`) has two — `dry_run`, `field_manager` — and no
  `field_validation`.
- `kube::api::ValidationDirective`'s doc (`:272-281`) says *"fail the request with
  a BadRequest error"*; the merge-patch path returns `NewInvalid` (422). See §3.

## 2 — kube discards the `Warning` header

```
$ grep -rli "warning" kube-client-4.2.0/src/ kube-core-4.2.0/src/
kube-core-4.2.0/src/admission.rs
kube-core-4.2.0/src/params.rs
$ grep -rin '"warning"\|Warning:' kube-client-4.2.0/src/ kube-core-4.2.0/src/
(no output)
```

`admission.rs` is the webhook *response* type; `params.rs` is
`ValidationDirective::Warn`'s doc. No HTTP `Warning` header is read anywhere in
`kube-client` or `kube-core`, at any layer — not only in the `Api` methods.

## 3 — the apiserver path, at `release-1.36`

Sources fetched from `raw.githubusercontent.com/kubernetes/kubernetes/release-1.36`.

`staging/src/k8s.io/apiserver/pkg/endpoints/handlers/patch.go`:

- `:161-171` — with `Strict` or `Warn` the decode serializer becomes
  `s.StrictSerializer`, and the codec's encoder half stays the ordinary one.
- `:323-334` — `jsonPatcher.applyPatchToCurrentObject` starts
  `currentObjJS, err := runtime.Encode(p.codec, currentObject)`.
- `:417-427` — for `types.MergePatchType` the strict pass over the patch bytes is
  `kjson.UnmarshalStrict(p.patchBytes, &map[string]interface{}{})`, i.e.
  duplicate keys only; unknown fields are caught later, by the strict decode of
  the merged object.
- `:338-364` — three `field.Invalid(field.NewPath("patch"), string(patchedObjJS), …)`
  sites: `:346` (non-strict decode failure), `:353` (strict decode failure — the
  unknown-field case), `:362` (duplicate keys with a clean decode). All three pass
  **`patchedObjJS`**, the whole patched object, as the BadValue. All three call
  `errors.NewInvalid(schema.GroupKind{}, "", …)` — empty kind, empty name.
- `:770-786` — the **strategic**-merge path (`smpPatcher`) passes
  `fmt.Sprintf("%+v", patchMap)` instead: the patch map, not the object.
- `:509-524` — the apply path returns `errors.NewBadRequest(…)`, which is where
  kube's doc wording ("BadRequest") comes from.

`staging/src/k8s.io/apimachinery/pkg/api/errors/errors.go:285-311` — `NewInvalid`
builds `Details.Causes` **and** sets
`err.ErrStatus.Message = fmt.Sprintf("%s %q is invalid: %v", qualifiedKind.String(), name, aggregatedErrs)`
at `:309`.

`staging/src/k8s.io/apimachinery/pkg/util/validation/field/errors.go:99-154` —
`Error()` is `"{Field}: {ErrorBody()}"`; `ErrorBody()` for `ErrorTypeInvalid` with a
`string` BadValue is `fmt.Sprintf("%s: %q", e.Type, t)` followed by `": " + Detail`.
**No length branch anywhere in the function.**

`pkg/registry/apps/{deployment,statefulset,replicaset}/storage/storage.go` —
`scaleFrom*` at `:370-393`, `:265-282` and `:271-288` respectively. All three build
`autoscaling.Scale` with an `ObjectMeta` of exactly `Name`, `Namespace`, `UID`,
`ResourceVersion`, `CreationTimestamp`, a `ScaleSpec{Replicas}` and a
`ScaleStatus{Replicas, Selector}`.

`staging/src/k8s.io/apimachinery/pkg/apis/meta/v1/types.go`, `ObjectMeta` JSON tag
order:

```
1 name  2 generateName  3 namespace  4 selfLink  5 uid  6 resourceVersion
7 generation  8 creationTimestamp  9 deletionTimestamp 10 deletionGracePeriodSeconds
11 labels 12 annotations 13 ownerReferences 14 finalizers 15 clusterName 16 managedFields
```

`staging/src/k8s.io/apiserver/pkg/endpoints/handlers/create.go:259-283` —
`managerOrUserAgent(manager, userAgent)` returns `manager` when non-empty,
otherwise `prefixFromUserAgent(userAgent)`, which is `strings.Split(u, "/")[0]`
with unprintables dropped.

## 4 — the sentence a Strict 422 produces

Assembled from the three format strings above (`errors.go:309`,
`field/errors.go:99`, `field/errors.go:109-154`) over a **synthetic** Scale in the
shape `scaleFromDeployment` builds — no cluster object was used, the `uid` is
`<uid>`:

```
len(Status.message) = 412 bytes   (k8s::FREE_TEXT cap = 4096)

 "" is invalid: patch: Invalid value: "{\"kind\":\"Scale\",\"apiVersion\":\"autoscaling/v1\",
\"metadata\":{\"name\":\"web\",\"namespace\":\"payments\",\"uid\":\"<uid>\",\"resourceVersion\":
\"88213\",\"creationTimestamp\":\"2026-09-01T00:00:00Z\"},\"spec\":{\"replicas\":9},\"status\":
{\"replicas\":3,\"selector\":\"app=web\"}}": strict decoding error: unknown field "spec.replicaz"
```

Wrapped to the 50 columns `screens/dialogs.md`'s refusal box draws, that is **10
lines**, of which the last one and a half are the diagnosis. The `%q` on the
object roughly doubles its length, because every `"` becomes `\"`.

`k8s::text` collapses whitespace to single spaces and never inserts a break, so
the message reaches the dialog and the audit log as one unbroken line.

## 5 — what real kubectl v1.36.3 sends, recorded

A local Python `HTTPServer` on `127.0.0.1` serving minimal discovery plus a
synthetic Deployment and Scale; kubeconfig pointing at it; recorder and kubectl
both under a script whose `kill` is in a `trap … EXIT` (D185).

```
$ kubectl scale deployment/web --replicas=5 -n payments --dry-run=server
PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale?dryRun=All
    content-type: application/merge-patch+json
    body: {"spec":{"replicas":5}}

$ kubectl scale deployment/web --replicas=5 -n payments
PATCH /apis/apps/v1/namespaces/payments/deployments/web/scale
    content-type: application/merge-patch+json
    body: {"spec":{"replicas":5}}

$ kubectl rollout restart deployment/web -n payments
PATCH /apis/apps/v1/namespaces/payments/deployments/web?fieldManager=kubectl-rollout
    content-type: application/strategic-merge-patch+json
    body: {"spec":{"template":{"metadata":{"annotations":{"kubectl.kubernetes.io/restartedAt":"2026-09-04T04:21:33+03:00"}}}}}

$ kubectl rollout restart deployment/web -n payments --dry-run=server
error: unknown flag: --dry-run
   exit=1

$ kubectl patch deployment web -n payments --type=merge --dry-run=server -p '{"spec":{"replicas":9}}'
PATCH /apis/apps/v1/namespaces/payments/deployments/web?dryRun=All&fieldManager=kubectl-patch
    content-type: application/merge-patch+json
    body: {"spec":{"replicas":9}}

$ kubectl delete deployment web -n payments --dry-run=server
DELETE /apis/apps/v1/namespaces/payments/deployments/web
    content-type: application/json
    body: {"propagationPolicy":"Background","dryRun":["All"]}
```

Four facts off that capture:

- `kubectl scale` sends the **same verb, path, media type and body** k8rs builds.
- **No kubectl patch of any kind sends `fieldValidation`.** Not `scale`, not
  `rollout restart`, not `patch --type=merge`, on either pass.
- `kubectl rollout restart` uses **strategic** merge patch, sends
  `fieldManager=kubectl-rollout`, writes the restart annotation with a
  local-offset RFC3339 stamp (`+03:00`, not `Z`), and **has no `--dry-run` flag
  at all** in v1.36.3.
- Every kubectl patch sends an explicit `fieldManager`.

## 6 — k8rs sends no `User-Agent`

The built binary from the copied tree, against the same recorder:

```
$ k8rs --yaml --kind deployment --object web -n payments
GET /version  UA='curl/8.21.0'      ← the readiness probe in the harness
GET /version  UA=None               ← k8rs
GET /apis     UA=None
GET /api      UA=None
GET /api/v1   UA=None
```

```
$ grep -rn "USER_AGENT\|user-agent" kube-client-4.2.0/ kube-core-4.2.0/src/
(no output)
```

`PatchParams::default()` leaves `field_manager: None`, so by `create.go:259-283`
the `managedFields` manager for a k8rs write is `prefixFromUserAgent("")`.
**Not measured against a cluster:** what that empty prefix produces in a stored
`managedFields` entry. One `kubectl get deploy -o yaml --show-managed-fields`
after a k8rs scale on kind settles it.

## 7 — reachability of the whole-object echo, checked

`patch.go:346` passes the same `patchedObjJS` BadValue on the **non-strict**
decode-failure branch, which a merge patch with a type mismatch
(`{"spec":{"replicas":"three"}}`) reaches with no `fieldValidation` sent at all.
So the whole-object echo is reachable before this box; `Strict` widens the set of
patch bodies that reach it.

`grep -rn "Patch::" src/` returns one line, in `src/ops_tests.rs`. No product code
builds a patch body yet.

## Teardown

The recorder was a background process under a `trap … EXIT INT TERM`; no cluster
was created; the scratch tree, target directory and downloaded sources are under
`$HOME/.cache/k8rs-review-k8s-admin` and `…/scratchpad/k8s-admin`.
