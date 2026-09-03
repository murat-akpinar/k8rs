# The five typed report lists — what was measured for the operator review (2026-08-29)

Subject: the uncommitted family in `src/k8s.rs` § WHAT A REPORT ASKS FOR —
`whole_list`, the six fetches routed through it, `ReportLists`, `report_lists`,
`Store::reports_fetched`, and the `if analysis` block in `src/main.rs`
§ WATCHING A CLUSTER. Consumers read read-only: `analysis.rs` `waste`,
`drain_safety`, `replica_sets_parked_at_zero`, `services_reaching_nothing`,
`disks_nobody_mounts`.

**No cluster was brought up.** Everything below is a local source read, a
committed-fixture measurement, or an upstream document. What needs a cluster is
listed in § 7 and is marked as unmeasured in the review.

---

## 1. The lists are read once, at connect, and there is no second call site

```
$ grep -rn "reports_fetched\|report_lists(" src/main.rs src/k8s.rs
src/main.rs:1612:            k8s::report_lists(&session.client, k8s::REPORT_FETCH),
src/main.rs:1615:        store.reports_fetched(reports);
src/k8s.rs:1474:    /// **Waste's and Drain safety's inputs, filed by [`Store::reports_fetched`]** — the same
src/k8s.rs:1521:    pub(crate) fn reports_fetched(&mut self, lists: ReportLists) {
src/k8s.rs:1742:            // these is watched (invariant 6); [`Store::reports_fetched`] fills them from
src/k8s.rs:1963:/// what [`Store::reports_fetched`] files.
src/k8s.rs:2002:pub(crate) async fn report_lists(client: &Client, deadline: std::time::Duration) -> ReportLists {
```

One producing call site, on the startup path, before the first watch is polled.
`Store::snapshot` then clones the five fields on every call, and `main.rs`'s
`drive_watching` closure calls `snapshot` once per watch event.

Field values the finding turns on: `ReportLists` is `#[derive(Clone, Default)]`;
`Store::reports` is a plain field with one setter; `drain_row` reads
`budgets` (frozen at connect) beside `snapshot.pods` and `snapshot.nodes`
(streamed, live).

## 2. `ListParams::default()` sends no `limit` **and** no `resourceVersion`

Read off the crate on disk, `kube-core-4.2.0/src/params.rs`:

```
$ sed -n '20,21p' ~/.cargo/registry/src/*/kube-core-4.2.0/src/params.rs
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListParams {
```

`populate_qp` (`:94-122`) appends `limit` only when `self.limit` is `Some`, and
appends `resourceVersion` only when `self.resource_version` is `Some`. With the
derived `Default` both are `None`, so the request carries an empty query — which
`k8s_tests.rs`'s own path assertion records independently, e.g.
`/api/v1/services?`.

Consequence measured only as a fact about the request, not about a server: no
`resourceVersion` parameter is the *quorum read* form of a LIST, not the
watch-cache form. § THE INITIAL LIST one region away states the same distinction
for the watches (`ListSemantic::MostRecent` vs `Any`) and chose the quorum read
there deliberately.

## 3. `kubectl get` does **not** read it "exactly this way"

The `whole_list` doc argues the no-`limit` decision with *"`kubectl get` reads it
exactly this way"*. Upstream reference,
<https://kubernetes.io/docs/reference/kubectl/generated/kubectl_get/>:

```
--chunk-size int     Default: 500
Return large lists in chunks rather than all at once. Pass 0 to disable.
```

So `kubectl get rs -A` pages at 500 by default and follows the continue token to
the end; `whole_list` sends one unpaged request. The two differ in the request
they make, and agree only in the answer they end up with.

## 4. What the default `view` ClusterRole grants — the five vs the CSR list

Upstream `plugin/pkg/auth/authorizer/rbac/bootstrappolicy/policy.go`,
`func viewRules()`, quoted verbatim (only the lines that bear on the five):

```
rbacv1helpers.NewRule(Read...).Groups(legacyGroup).Resources("pods", "replicationcontrollers", "replicationcontrollers/scale", "serviceaccounts", "services", "services/status", "endpoints", "persistentvolumeclaims", "persistentvolumeclaims/status", "configmaps").RuleOrDie(),
rbacv1helpers.NewRule(Read...).Groups(discoveryGroup).Resources("endpointslices").RuleOrDie(),
rbacv1helpers.NewRule(Read...).Groups(appsGroup).Resources("controllerrevisions", "statefulsets", "statefulsets/status", "statefulsets/scale", "daemonsets", "daemonsets/status", "deployments", "deployments/status", "deployments/scale", "replicasets", "replicasets/status", "replicasets/scale").RuleOrDie(),
rbacv1helpers.NewRule(Read...).Groups(policyGroup).Resources("poddisruptionbudgets", "poddisruptionbudgets/status").RuleOrDie(),
```

All five kinds the family fetches are in `view`. `certificatesigningrequests`
appears in neither `viewRules()` nor `editRules()`.

Second reading, `docs/security.md` § RBAC, the documented `k8rs-readonly`
ClusterRole: it grants `services` and `persistentvolumeclaims` (core),
`replicasets` (apps), `poddisruptionbudgets` (policy), `endpointslices`
(discovery) and `certificatesigningrequests` (certificates) — the six the family
reads, all at cluster scope.

## 5. Per-object size of the five kinds, over the committed corpus

The corpus is a kind cluster, sanitized. `scripts/sanitize.jq:220` deletes
`managedFields` and every annotation, and its header (`:6-7`) records that the
last-applied annotation is "a full copy of the spec".

```
$ cd tests/fixtures && for f in healthy-replicasets services endpointslices persistentvolumeclaims poddisruptionbudgets; do \
    printf '%-28s items=%-3s mean_item_bytes=%s\n' "$f.json" "$(jq '.items|length' $f.json)" \
      "$(jq -c '.items[]' $f.json | awk '{s+=length($0)} END{printf "%d", s/NR}')"; done
healthy-replicasets.json     items=1   mean_item_bytes=1289
services.json                items=4   mean_item_bytes=608
endpointslices.json          items=4   mean_item_bytes=901
persistentvolumeclaims.json  items=2   mean_item_bytes=568
poddisruptionbudgets.json    items=2   mean_item_bytes=629
```

```
$ jq -r '.items[0].spec | keys | join(",")' tests/fixtures/healthy-replicasets.json
replicas,selector,template
```

A ReplicaSet carries the whole pod template. These numbers are a **floor**: they
are a minimal kind workload with `managedFields` and all annotations already
removed. The comparable pod figure this repo has already measured is in
`src/k8s.rs` § THE INITIAL LIST — median 3708 bytes, largest 5662, over 55 pod
objects, "with `managedFields` already stripped by the sanitizer, so a live
object is larger by an amount only a cluster can say".

Count side, from upstream defaults rather than a cluster:
`Deployment.spec.revisionHistoryLimit` defaults to **10**, so a cluster keeps up
to ten `spec.replicas: 0` ReplicaSets per Deployment as rollback history.
`--max-endpoints-per-slice` defaults to **100**, so EndpointSlice count grows
with endpoints/100 per Service.

## 6. The retry layer the 10s deadline sits on top of

Already measured in this repo and re-read, not re-measured:
`src/k8s.rs` § WHAT A THROTTLE LOOKS LIKE (NOTES § D148) records that kube's
`RetryPolicy::server_retry()` retries 429, 503 and 504 fifteen times, backoff
base `5 ms x 2^i` with tower jitter of `base .. 3 x base`, and that nothing in
this process can observe it.

Cumulative floor of the first eleven waits, arithmetic over those constants:
5+10+20+40+80+160+320+640+1280+2560+5120 = **10235 ms**. `REPORT_FETCH` is
10 000 ms and wraps the whole retry stack, so a run of roughly nine to eleven
consecutive 429/503/504 answers is cut off by the deadline and returns the same
`None` a 403 returns.

## 7. What was not measured, and the command that would measure it

No cluster was used. These need one:

- Wall time of an unpaged `list` of every ReplicaSet, Service, EndpointSlice,
  PVC and PDB on a cluster of ~10 000 pods, against `REPORT_FETCH`'s 10 s:
  `time kubectl get rs -A --chunk-size=0 -o json | wc -c` beside
  `time kubectl get rs -A -o json | wc -c` (the second is kubectl's paged
  default), and the API server's own `apiserver_request_duration_seconds` for
  that LIST.
- Resident set of the k8rs process immediately after the six fetches complete,
  against `REQUIREMENTS.md`'s `< 50MB RSS at ~1000 pods`.
- Whether the six concurrent fetches share one HTTP/2 connection, which
  `report_lists`'s doc asserts ("one task and one connection").
- Whether API Priority and Fairness de-prioritises six concurrent unpaged
  cluster-wide LISTs at connect, and what its `Retry-After` says.
