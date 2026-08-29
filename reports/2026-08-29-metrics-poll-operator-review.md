# The metrics poll, read from the operator's chair — what a stopped poll costs, what an empty answer draws, and what `using` sits next to

`k8s-admin`, 2026-08-29. Operator review (cycle step 6) of the uncommitted
`// --- WHAT A NODE IS USING ---` region in `src/k8s.rs` and its `src/main.rs`
call site, on top of `e2550e0`.

Cluster `kind-k8rs`, **read-only throughout**: no `apply`, no `delete`, no
`cluster.sh`, nothing created, scaled or torn down. The only non-`get` verb used
was `--as` impersonation on a read, which the apiserver refused before it reached
an object. metrics-server was left running and the cluster is in the state it was
handed over in.

**Tree state.** `HEAD` = `e2550e0`; `src/k8s.rs`, `src/k8s_tests.rs`,
`src/main.rs`, `NOTES.md`, `todo.md` and `backlog.md` dirty in the working tree —
the code under review. `src/rules.rs` and `src/analysis.rs` clean at `HEAD`
(frozen, judged not changed). Code is cited by **region and symbol**, not by line
number, because the file moved under an earlier report today and will move again.

Every number below was produced by the command printed above it, on this machine,
on 2026-08-29. Anything not measured is labelled **unmeasured** in the line that
makes the claim.

## 0. What was measured against

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1 | jq -c '[.resources[] | {name, namespaced, kind, verbs}]'
[{"name":"nodes","namespaced":false,"kind":"NodeMetrics","verbs":["get","list"]},
 {"name":"pods","namespaced":true,"kind":"PodMetrics","verbs":["get","list"]}]

$ kubectl --context kind-k8rs get nodes --no-headers | awk '{print $1, $2, $3}'
k8rs-control-plane Ready control-plane
k8rs-worker Ready,SchedulingDisabled <none>
k8rs-worker2 Ready <none>
k8rs-worker3 Ready <none>
```

Four nodes, server v1.36.1, metrics-server v0.8.0 with `--metric-resolution=15s`
(§ 0 of `reports/2026-08-29-metrics-server-units.md`, same cluster, same day).

**`verbs: ["get","list"]` and no `watch`** — re-read here, and it is the premise
`node_usage_poll` rests on. The region's own text says whether a `watch` verb is
served "was not read off a cluster in this turn". It is read now, and the premise
holds: this group cannot be watched, so a poll is the only shape available.

## 1. What the binary actually prints against a live metrics-server

The build under review, run against the cluster for 95 seconds:

```
$ cargo build --quiet
$ timeout 95 ./target/debug/k8rs --live --analysis --context kind-k8rs
```

The capacity block of the first report it printed (the rest of the report is
elided; one finding message on this cluster carries a node IP and is not
reproduced):

```
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
41 pods · 4 nodes
   … 16 findings elided …

[capacity]
  What each node promised, and what it has
    k8rs-control-plane   0.95 of 12 cpu · 290Mi of 23.1Gi
      using 0.077 cpu and 1Gi
    k8rs-worker   0.47 of 12 cpu · 378Mi of 23.1Gi
      using 0.027 cpu and 534.6Mi
    k8rs-worker2   0.1 of 12 cpu · 50Mi of 23.1Gi
      using 0.011 cpu and 202.9Mi
    k8rs-worker3   0.22 of 12 cpu · 282Mi of 23.1Gi
      using 0.025 cpu and 488.4Mi
    19 workloads have no memory or CPU limit
      Nothing stops one taking a whole node.
```

End to end on a real metrics-server: four `using` paragraphs under four node
rows, keyed correctly, no missing node, no unparsed quantity. This is the
`todo.md` box's own done-when output and it is first-hand.

**Eight whole reports were printed in 95 s.** Diffing consecutive ones shows what
drove each reprint:

```
$ diff report7 report8            # reports at lines 1516 and 1768 of the run log
87c87
<       using 0.096 cpu and 1Gi
>       using 0.089 cpu and 1Gi
89c89
<       using 0.03 cpu and 534.4Mi
>       using 0.03 cpu and 534.3Mi
91c91
<       using 0.014 cpu and 198.5Mi
>       using 0.013 cpu and 198Mi
93c93
<       using 0.025 cpu and 490.2Mi
>       using 0.027 cpu and 491.3Mi
   (a trailing `250,251d249` hunk is elided: report 8 was cut off mid-print by
    the 95 s `timeout`, and is an artifact of how the run was ended)

$ diff report5 report6
160c160
<       This run started 47 min ago.
>       This run started 48 min ago.
```

So a reprint of the **whole ~250-line report** is caused by the metrics poll
alone, with nothing else in the cluster having changed, and the delta that caused
it is one milli-core and one tenth of a mebibyte.

Gate checks on the same binary:

```
$ timeout 20 ./target/debug/k8rs --analysis tests/fixtures/pods.json | grep -c "using "
0

$ timeout 25 ./target/debug/k8rs --live --context kind-k8rs | grep -c "^\[capacity\]"
0

$ timeout 20 ./target/debug/k8rs --live --analysis --context no-such-context
k8rs: no cluster to watch — this kubeconfig has no such context — check the `--context` you gave, or the `current-context` line in the file
```

No cluster, no `--analysis`, and a context that does not exist: all three
degrade to a named cause and none of them polls.

## 2. What the two stopped states cost, and what the state that keeps polling costs

Round-trip time on this apiserver, six samples each, read off `kubectl -v=6`'s own
`milliseconds=` field so kubectl start-up is excluded:

```
$ t() { kubectl --context kind-k8rs "$@" -v=6 2>&1 >/dev/null | grep -oE 'status="[^"]+" milliseconds=[0-9]+'; }

$ for i in 1 2 3 4 5 6; do t get --raw /apis/metrics.k8s.io/v1beta1/nodes; done
status="200 OK" milliseconds=15
status="200 OK" milliseconds=11
status="200 OK" milliseconds=36
status="200 OK" milliseconds=26
status="200 OK" milliseconds=34
status="200 OK" milliseconds=13

$ for i in 1 2 3 4 5 6; do t get --raw /apis/nosuch.k8s.io/v1beta1/nodes; done
status="404 Not Found" milliseconds=16
status="404 Not Found" milliseconds=34
status="404 Not Found" milliseconds=29
status="404 Not Found" milliseconds=16
status="404 Not Found" milliseconds=31
status="404 Not Found" milliseconds=36

$ for i in 1 2 3 4 5 6; do t --as=nobody-at-all get --raw /apis/metrics.k8s.io/v1beta1/nodes; done
status="403 Forbidden" milliseconds=12
status="403 Forbidden" milliseconds=20
status="403 Forbidden" milliseconds=35
status="403 Forbidden" milliseconds=21
status="403 Forbidden" milliseconds=15
status="403 Forbidden" milliseconds=13
```

**404 and 403 — the two answers that end the poll — cost 11–36 ms.** They are the
two cheapest answers the metrics path can give, indistinguishable in cost from
the 200.

The answer that does **not** end the poll, timed by the repo's own test against a
local stub with a 300 ms deadline:

```
$ cargo test --quiet the_deadline_and_not_a_status_is_what_ends_a_throttled_metrics_api -- --nocapture
`429 Too Many Requests` -> Silent in 301.523674ms (kube retries it: true)
`503 Service Unavailable` -> Silent in 301.274261ms (kube retries it: true)
`504 Gateway Timeout` -> Silent in 301.61354ms (kube retries it: true)
`500 Internal Server Error` -> Silent in 1.311559ms (kube retries it: false)
`502 Bad Gateway` -> Silent in 2.471748ms (kube retries it: false)
test result: ok. 1 passed
```

In production the deadline is `REPORT_FETCH` = 10 s, so a 503 costs **10 s per
poll**, and with `MissedTickBehavior::Delay` the effective period on such a
cluster is 30 s + 10 s = **40 s**, not 30 s.

**How many requests go out inside that 10 s is unmeasured**, and the arithmetic
only brackets it. D148's read constants are `RetryPolicy::new(5ms, 1000s, 15, true)`
with bases `5ms × 2^i` and tower's `uniform(0, 2·base)` jitter, so the jitter's
*mean* is the base and its *floor* is zero. Summing the bases,
5+10+20+40+80+160+320+640+1280+2560 = 5115 ms of sleep after ten retries and
10 235 ms after eleven — so **around eleven or twelve requests in a 10 s deadline
if every jitter draw lands on its mean**, more if they land low, and there is no
useful lower bound because the floor is zero. That is arithmetic over constants
somebody else read, not a count: nothing in this run counted a request.
`stub_list` already returns `Arc<Mutex<Vec<String>>>` of every path asked and the
throttle test discards it as `_`, so pinning the real number costs one assertion
and no new machinery.

Ratio, measured against measured: the state the code keeps re-asking costs
**~10 s** per poll; each of the two states it refuses to re-ask costs **~20 ms**.

## 3. What an empty answer looks like on the wire, and what the pane makes of it

This metrics-server's empty-list body, taken from the namespaced sibling of the
polled path (`kube-public` has no pods on this cluster):

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/namespaces/kube-public/pods
{"kind":"PodMetricsList","apiVersion":"metrics.k8s.io/v1beta1","metadata":{},"items":[]}

$ kubectl --context kind-k8rs top pods -n kube-public; echo "exit=$?"
No resources found in kube-public namespace.
exit=0
```

`kind`, `apiVersion` and `items` are all present, so a body of this shape passes
`NodeMetricsList`'s strict decode and becomes `Metrics::Read(∅)`.

What the pane does with it, read off the two frozen functions in `analysis.rs`:
`live_usage_row` returns `None` for `Some(Metrics::Read(_))` **without looking
inside the map**, and `using` returns `None` for every node because
`nodes.get(node)` misses. So `Read(∅)` draws **no `using` paragraph and no
explanatory row** — byte-identical output to a healthy answer.

No test feeds it:

```
$ grep -rn "Metrics::Read(BTreeMap::new())\|Metrics::Read(Default::default())\|metrics_saying(&\[\])" src/analysis_tests/ src/rules_tests/
   (no output)
```

**Unmeasured:** that a freshly-started metrics-server serves an empty
`NodeMetricsList` before its first scrape completes. Producing it needs a restart
of metrics-server, which is out of bounds for this review. The *body shape* that
reaches `Read(∅)` is measured above; the *cluster condition* that produces it at
node level is not.

## 4. Which requests `kubectl top nodes` actually makes

```
$ kubectl --context kind-k8rs top nodes -v=6 2>&1 >/dev/null | grep -oE 'verb="[A-Z]+" url="[^"]+" status="[^"]+"' | sed 's|https://[^/]*||'
verb="GET" url="/api" status="200 OK"
verb="GET" url="/apis" status="200 OK"
verb="GET" url="/apis/metrics.k8s.io/v1beta1/nodes" status="200 OK"
verb="GET" url="/api/v1/nodes" status="200 OK"

$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes -v=6 2>&1 >/dev/null | grep -oE 'verb="[A-Z]+" url="[^"]+" status="[^"]+"' | sed 's|https://[^/]*||'
verb="GET" url="/apis/metrics.k8s.io/v1beta1/nodes" status="200 OK"
```

Four requests against one. `node_usage` sends the third of those four; the
one-for-one equivalent an operator would type is
`kubectl get --raw /apis/metrics.k8s.io/v1beta1/nodes`.

## 5. Who is allowed to read node metrics

```
$ kubectl --context kind-k8rs get clusterrole view -o jsonpath='{range .rules[*]}{.apiGroups}{" "}{.resources}{"\n"}{end}' | grep -i metrics
["metrics.k8s.io"] ["pods","nodes"]

$ kubectl --context kind-k8rs get clusterrole system:aggregated-metrics-reader \
    -o jsonpath='rules={range .rules[*]}{.apiGroups}{.resources}{.verbs}{end} labels={.metadata.labels}{"\n"}'
rules=["metrics.k8s.io"]["pods","nodes"]["get","list","watch"] labels={"k8s-app":"metrics-server","rbac.authorization.k8s.io/aggregate-to-admin":"true","rbac.authorization.k8s.io/aggregate-to-edit":"true","rbac.authorization.k8s.io/aggregate-to-view":"true"}

$ kubectl --context kind-k8rs auth can-i list nodes.metrics.k8s.io --as=some-random-dev
no
$ kubectl --context kind-k8rs auth can-i list nodes --as=some-random-dev
no
```

Two readings. First, on a cluster where metrics-server was installed from its own
manifest, `system:aggregated-metrics-reader` aggregates into `view`/`edit`/`admin`
— so the ordinary read-only user already has the grant and this call adds no
requirement for them. Second, that grant arrives **with the metrics-server
install**, not with Kubernetes: a cluster whose metrics API is served by something
else, or whose `view` predates the install, does not have it, and there this call
is a new permission requirement (PRIOR-ART § B4's review question).

Note also that the upstream role grants `watch` on a group that serves no `watch`
verb (§ 0) — upstream over-grants; nothing here depends on it.

Against product code, `pods` has no reader:

```
$ grep -rn "PodMetrics\|v1beta1/pods\|metrics.k8s.io" src/*.rs | grep -v "_tests" | wc -l
14
$ grep -rn "PodMetrics\|v1beta1/pods\|metrics.k8s.io" src/*.rs | grep -v "_tests" | grep -v "^src/k8s.rs:[0-9]*://\|^src/k8s.rs:[0-9]*: *///\|^src/k8s.rs:[0-9]*: *//"
src/k8s.rs:2255:const METRICS_NODES: &str = "/apis/metrics.k8s.io/v1beta1/nodes";
src/k8s.rs:2264:const METRICS_VERSION: &str = "metrics.k8s.io/v1beta1";
src/k8s.rs:3734:                    ("metrics.k8s.io", _) => Some(Capability::Metrics),
```

Fourteen hits, and outside comments there are three: the polled path, the version
string it checks, and the capability probe's arm. **No `PodMetricsList` type
exists and nothing requests `/apis/metrics.k8s.io/v1beta1/pods`.**
`docs/security.md:128-130` grants `resources: ["pods", "nodes"]`.

## 6. Response size, for the load question

```
$ B=$(kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes | wc -c)
$ N=$(kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes | jq '.items|length')
$ echo "bytes=$B nodes=$N per_node=$((B/N))"
bytes=1624 nodes=4 per_node=406

$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
    | jq -r '.items[0] | {item_bytes: (tostring|length), labels_bytes: (.metadata.labels|tostring|length), label_keys: (.metadata.labels|keys|length)}'
{ "item_bytes": 471, "labels_bytes": 273, "label_keys": 7 }

$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
    | jq -r '.items[0] | {name: .metadata.name, cpu: .usage.cpu, memory: .usage.memory} | tostring | length'
68
```

406 bytes per node on the wire; 68 bytes per node survive the decode. **58 % of
the body is `metadata.labels`, which the hand-rolled struct never names and serde
therefore never builds.** Label values are not reproduced here.

The list is one entry per **node**. The pod endpoint is one entry per pod — 27
items on this 41-pod cluster (`reports/2026-08-29-metrics-server-units.md` § 2).

**Unmeasured:** the body size on a cluster with cloud-provider node labels
(topology, instance type, nodegroup), which is where the per-node figure would
grow. On this kind cluster a node carries 7 labels.

## 7. Sampling arithmetic

From `reports/2026-08-29-metrics-server-units.md` § 3, same cluster, same day, not
re-run here: `window` alternates 10 s / 20 s per node against a configured 15 s
resolution, `timestamp` lags the read, and two reads inside one scrape return
byte-identical values.

Composed with `METRICS_POLL` = 30 s: the value on screen is an average over a
10–20 s window that ended at `timestamp`, which may be up to 30 s before the poll
that fetched it, plus up to 30 s before the next poll replaces it. **A drawn
number can describe an interval that ended ~50 s earlier**, and neither `window`
nor `timestamp` is decoded, so nothing on screen carries that fact. A 30 s poll
against a 15 s scrape reads every second scrape at best; the intervening scrape is
never seen by anything.

## Cluster state at the end

Nothing was created, modified, scaled or deleted. metrics-server is running and
`v1beta1.metrics.k8s.io` still reports `Available=True`.
