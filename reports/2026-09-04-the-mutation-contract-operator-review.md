# The mutation contract — what was read and measured

`k8s-admin`, 2026-09-04, step 6 over the uncommitted `src/ops.rs` (264 lines) and
`src/ops_tests.rs` (524 lines) on `development`. No cluster was brought up this
run; everything below is read off the tree, off the vendored crate sources, or
off the upstream document already in `tmp/`.

## 1. The two disposals, side by side

`src/ops.rs:218` `clean()` filters and collects. `src/k8s.rs:281` `text()`
substitutes one space for an unprintable that is whitespace. Both were extracted
verbatim into a standalone file and run against the two consequence strings
`screens/dialogs.md` draws.

```
$ rustc -O -o dispose dispose.rs && ./dispose
input      "This starts 1 more copy of your app.\nRight now: 2 copies.  After: 3 copies."
ops::clean "This starts 1 more copy of your app.Right now: 2 copies.  After: 3 copies."
k8s::text  "This starts 1 more copy of your app. Right now: 2 copies.  After: 3 copies."
input      "This removes the pod. Its Deployment will start a\nreplacement immediately — the app keeps running."
ops::clean "This removes the pod. Its Deployment will start areplacement immediately — the app keeps running."
k8s::text  "This removes the pod. Its Deployment will start a replacement immediately — the app keeps running."
```

`clean()` reaches the screen at `src/ops.rs:168` → `ask(consequence)` at
`src/ops.rs:183`, and the audit line at `src/ops.rs:224-238`.

## 2. The audit verdict strings, and what selects them

`src/ops.rs:175-199`. Four constants, each selected by which branch fired:

| branch | verdict string written to the log |
|---|---|
| `call(DRY_RUN)` returned `Err`, any error at all | `the server refused the dry-run, so the change was never sent` |
| `ask` returned `false`, any reason at all | `nobody confirmed it, so nothing was changed` |
| `call(FOR_REAL)` returned `Err` | `the dry-run passed and the call itself failed` |
| `call(FOR_REAL)` returned `Ok` | `the change was made` |

The first string is written for every `Fault` the classifier can produce. The
set it covers, counted off the enum:

```
$ awk '/^pub enum Fault \{/,/^\}/' src/k8s.rs | grep -E '^    [A-Z][A-Za-z]+,$'
    Kubeconfig,
    NoContext,
    BadEntry,
    NoCredential,
    Rejected,
    Expired,
    Refused,
    Gone,
    Unfinished,
    Unanswered,
```

`result_line` (`src/ops.rs:243-255`) writes `result · {verdict}` with no
timestamp, no object and no context; the attempt line above it carries all
three. `write_line` (`src/ops.rs:259-262`) is one `write_all` plus a flush per
line.

## 3. `answer()`'s arms

`src/k8s.rs:1100-1115`: `match status.code` has `400 401 403 404`; the fallback
reads `status.reason` against `BAD_REQUEST UNAUTHORIZED FORBIDDEN NOT_FOUND` and
otherwise yields `Fault::Unanswered`. A `409 Conflict` and a `422 Invalid` match
neither list.

The constants that exist upstream but are not read:

```
$ grep -n "pub const" ~/.cargo/registry/src/*/kube-core-4.2.0/src/response.rs
283:    pub const CONFLICT: &str = "Conflict";
301:    pub const INVALID: &str = "Invalid";
```

## 4. What the kube API offers the later operation boxes

```
$ grep -rn "pub async fn restart" ~/.cargo/registry/src/*/kube-client-4.2.0/src/api/
api/util/mod.rs:19:    pub async fn restart(&self, name: &str) -> Result<K> {
```

`Api::restart`, `Api::cordon` and `Api::uncordon` take no params argument, so no
`dryRun` query parameter can be attached through them. `PatchParams` and
`DeleteParams` both carry `dry_run` (`kube-core-4.2.0/src/params.rs:537, 666,
730, 827`), and `PatchParams::populate_qp` appends `fieldValidation` for any
patch, not only an apply (`params.rs:705-707`, and its own test at `:908-927`).

## 5. Dry-run and admission webhooks — upstream, already in `tmp/`

`tmp/k8s/api-concepts.txt:743-745`:

> If the non-dry-run version of a request would trigger an admission controller
> that has side effects, the request will be failed rather than risk an unwanted
> side effect.

and `:759-761`:

> Authorization for dry-run and non-dry-run requests is identical. Thus, to make
> a dry-run request, you must be authorized to make the non-dry-run request.

**Not measured here.** The command that would settle it on a cluster, for
whoever takes it:

```
kubectl apply -f - <<'YAML'
apiVersion: admissionregistration.k8s.io/v1
kind: ValidatingWebhookConfiguration
metadata: { name: sideeffects-probe }
webhooks:
- name: probe.example.com
  sideEffects: Some
  admissionReviewVersions: ["v1"]
  clientConfig: { url: "https://127.0.0.1:1/nope" }
  failurePolicy: Ignore
  rules: [{ operations: ["UPDATE"], apiGroups: ["apps"], apiVersions: ["v1"], resources: ["deployments/scale"] }]
YAML
kubectl scale deployment/web --replicas=3 -n payments --dry-run=server
```

## 6. Dependency surface available to a redesign

`Cargo.toml:80-101` — tokio features are `rt-multi-thread`, `macros`, `net`,
`time`. There is no `sync`. `futures-util` is `default-features = false` with
`std` only (`Cargo.toml:110-112`), so no `channel` either. Any signature that
wants a one-shot channel between the dialog and the dry-run needs a feature
added first.

## 7. What was read and not measured

`src/ops.rs` whole · `src/ops_tests.rs` whole · `src/k8s.rs` § THE INGEST GUARD
(163-330) and § WHAT WENT WRONG (744-1250) · `src/main.rs:329-331` ·
`screens/dialogs.md` whole · `NOTES.md` § Operations, § The safety model, D8,
D20, D21, D22, D23 · `todo.md:3624-3739` and `:4034-4048` ·
`PRIOR-ART.md` § C1, § G1-G3 · `CLAUDE.md` invariants 1, 2, 4, 9, 14 ·
`reports/README.md`.

Nothing was built: `cargo` was not run, because `tester` held the shared target
directory for the same diff this turn.
