# metrics-server, read off the object — units, the `"0"` shape, `window`, and the four `Metrics` states

`k8s-admin`, 2026-08-29. Cluster `kind-k8rs`, read-only throughout: no `apply`,
no `delete`, no `cluster.sh`, nothing created or torn down.

**Tree state while this ran.** `HEAD` was `e2550e0`; `src/rules.rs` and
`src/analysis.rs` — the two files every code claim below reads — were clean at
`HEAD`, and `src/k8s.rs` and `src/main.rs` were dirty in the working tree,
another writer's in-flight work. `src/k8s.rs` is therefore cited **by the
sentence, not by a line number**: its numbers moved by ~317 lines during this
run.

Every number below was produced by the command printed above it, on this
machine, on 2026-08-29. Numbers move between reads — a later read of the same
command will not reprint them, and that is itself measurement 3.

## 0. What was measured against

```
$ kubectl config get-contexts -o name
kind-k8rs

$ kubectl --context kind-k8rs version -o json      # serverVersion, trimmed to two fields
  "gitVersion": "v1.36.1"
  "platform": "linux/amd64"

$ kubectl --context kind-k8rs get nodes -o custom-columns=NAME:.metadata.name,VERSION:.status.nodeInfo.kubeletVersion
NAME                 VERSION
k8rs-control-plane   v1.36.1
k8rs-worker          v1.36.1
k8rs-worker2         v1.36.1
k8rs-worker3         v1.36.1

$ kubectl --context kind-k8rs -n kube-system get deployment metrics-server \
    -o jsonpath='{.spec.template.spec.containers[0].image}'
registry.k8s.io/metrics-server/metrics-server:v0.8.0

$ kubectl --context kind-k8rs -n kube-system get deployment metrics-server \
    -o jsonpath='{.spec.template.spec.containers[0].args}'
["--cert-dir=/tmp","--secure-port=10250","--kubelet-preferred-address-types=InternalIP,ExternalIP,Hostname","--kubelet-use-node-status-port","--metric-resolution=15s","--kubelet-insecure-tls"]
```

`--kubelet-insecure-tls` and the APIService's `insecureSkipTLSVerify` below are
settings of the *test cluster's* metrics-server, inside the cluster, on the
metrics-server → kubelet and apiserver → metrics-server hops. Neither is on any
path k8rs opens.

```
$ kubectl --context kind-k8rs get apiservice v1beta1.metrics.k8s.io \
    -o jsonpath='service={.spec.service.namespace}/{.spec.service.name}:{.spec.service.port} insecureSkipTLSVerify={.spec.insecureSkipTLSVerify} group={.spec.group} version={.spec.version}{"\n"}'
service=kube-system/metrics-server:443 insecureSkipTLSVerify=true group=metrics.k8s.io version=v1beta1

$ kubectl --context kind-k8rs get apiservice v1beta1.metrics.k8s.io \
    -o jsonpath='{range .status.conditions[*]}{.type}={.status} reason={.reason}{"\n"}{end}'
Available=True reason=Passed
```

## 1. The units, first-hand

The object's key shape, keys only:

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
    | jq -r '.items[0] | {top_level_keys: keys, metadata_keys: (.metadata|keys), usage_keys: (.usage|keys)}'
{
  "top_level_keys": [ "metadata", "timestamp", "usage", "window" ],
  "metadata_keys": [ "creationTimestamp", "labels", "name" ],
  "usage_keys": [ "cpu", "memory" ]
}
```

`metadata.labels` is present on every `NodeMetrics` item — it carries the node's
own labels, seven distinct keys on this cluster, values not reproduced here. A
prune that keeps `metadata` whole keeps them.

The values, projected to the four fields:

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
    | jq -r '.items[] | "\(.metadata.name)\tcpu=\(.usage.cpu)\tmemory=\(.usage.memory)\twindow=\(.window)\ttimestamp_present=\(.timestamp|type)"'
k8rs-control-plane	cpu=90756662n	memory=1112552Ki	window=10.019s	timestamp_present=string
k8rs-worker	cpu=30670278n	memory=571132Ki	window=20.026s	timestamp_present=string
k8rs-worker2	cpu=14503896n	memory=206272Ki	window=10.01s	timestamp_present=string
k8rs-worker3	cpu=23416983n	memory=485520Ki	window=20.02s	timestamp_present=string
```

Observed: cpu carries the suffix `n`, memory carries `Ki`, on all four nodes.
Both are quantity strings, not numbers — `usage.cpu` and `usage.memory` are JSON
strings, `window` is a Go duration string, `timestamp` is an RFC3339 string.

`n` and `Ki` are the metrics API's own spelling and are **not** the alphabet a
pod's `resources.requests` is written in. The same cluster's requests, read off
the same objects the rules already parse:

```
$ kubectl --context kind-k8rs get pods -A \
    -o jsonpath='{range .items[*].spec.containers[*]}{.resources.requests.cpu} {.resources.requests.memory}{"\n"}{end}' \
    | sort -u
100m 
100m 100Mi
100m 200Mi
100m 50Mi
100m 64Mi
100m 70Mi
10m 
10m 16Mi
200m 
250m 
```

So a value spelled `100m` and a value spelled `90756662n` are the two ends of
one comparison. Four of those ten rows name cpu and omit memory — the trailing
space is an absent `requests.memory`.

Note also `window=10.01s` above: the duration is not fixed-width and not always
three decimals. A parser that slices a fixed number of characters is wrong.

## 2. `cpu: "0"` arrives with no suffix at all — 12 of 28 containers

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/pods \
    | jq -r '{items: (.items|length), containers: ([.items[].containers[]]|length)}'
{
  "items": 27,
  "containers": 28
}
```

27 `PodMetrics` items, 28 container entries — one pod on this cluster has two
containers, so a reader that assumes one container per item under-counts.

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/pods \
    | jq -r '[.items[].containers[].usage.cpu] | group_by(sub("^[0-9.]+";"")) | map({suffix: (.[0]|sub("^[0-9.]+";"")), count: length})'
[ { "suffix": "", "count": 12 }, { "suffix": "n", "count": 16 } ]

$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/pods \
    | jq -r '[.items[].containers[].usage.memory] | group_by(sub("^[0-9.]+";"")) | map({suffix: (.[0]|sub("^[0-9.]+";"")), count: length})'
[ { "suffix": "Ki", "count": 28 } ]
```

**12 of 28 container cpu values carry no suffix at all**; all 28 memory values
carry `Ki`. Every bare value is the single character `0`:

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/pods \
    | jq -r '[.items[].containers[].usage.cpu | select(test("^[0-9.]+$"))] | group_by(.) | map({value: .[0], count: length})'
[ { "value": "0", "count": 12 } ]

$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/pods \
    | jq -r '[.items[].containers[].usage.memory | select(test("^[0-9.]+$"))] | length'
0
```

The pods carrying it, container by container (namespace `default`, the fixture
manifest's idle pods):

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/pods \
    | jq -r '.items[] | select([.containers[].usage.cpu] | any(test("^[0-9.]+$"))) | "\(.metadata.namespace)/\(.metadata.name) " + ([.containers[] | "\(.name):cpu=\(.usage.cpu),mem=\(.usage.memory)"]|join(" "))'
default/broken-hostpath shipper:cpu=0,mem=412Ki nosy:cpu=0,mem=392Ki
default/broken-overhead app:cpu=0,mem=388Ki
default/broken-podlimit app:cpu=0,mem=388Ki
default/broken-restarts10serving flaky:cpu=0,mem=624Ki
default/broken-sts-0 app:cpu=0,mem=380Ki
default/broken-unstarted app:cpu=0,mem=384Ki
default/healthy-disk app:cpu=0,mem=388Ki
   (11 pods in total; the four with generated ReplicaSet suffixes are elided)
```

Where the bare `"0"` was **not** observed: on any of the four nodes, in any read
taken today. Node cpu was `n` in every sample of § 3. A node-level `"0"` is
therefore **unmeasured** on this cluster, not shown to be impossible.

Against the code:

- `quantity_milli` (`src/rules.rs:7095`) has a `"" => (1, 1)` arm, and the bare
  `"0"` is row 1 of its own table test:

  ```
  $ cargo test --quiet a_quantity_becomes_a_number_and_a_number_becomes_a_size_a_human_reads -- --nocapture
                                             0 -> Some(0)
                                            1n -> Some(1)
                                            0n -> Some(0)
                                           1Ki -> Some(1024000)
                                            .5 -> Some(500)
           67108864000 -> 64Mi
        24860065792000 -> 23.1Gi
               1024000 -> 1Ki
         1610612736000 -> 1.5Gi
                512000 -> 512
  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 668 filtered out
  ```

  (The run prints 46 quantity rows and 5 size rows; the ten above are the ones
  this report turns on.)

- The analysis layer's plant `metrics_saying`
  (`src/analysis_tests/capacity.rs:139`) has two call sites, and neither feeds a
  bare-suffix value:

  ```
  $ grep -n 'metrics_saying' src/analysis_tests/capacity.rs
  139:pub(super) fn metrics_saying(usage: &[(&str, &str, &str)]) -> Metrics {
  1052:        metrics_saying(&[("k8rs-worker", "137669270n", "1035316Ki")]),
  1092:            metrics_saying(&[("k8rs-worker", usage.0, usage.1)]),
  ```

  Line 1092 is the unparseable-value loop; its three pairs are
  `("137669270n", "not a quantity")`, `("not a quantity", "1035316Ki")`,
  `("not a quantity", "also not one")`.

## 3. `window` is not constant, and it is not `--metric-resolution`

Six back-to-back reads return the identical window on every node:

```
$ for i in 1 2 3 4 5 6; do kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
    | jq -r --arg i "$i" '.items[] | "read\($i) \(.metadata.name) window=\(.window)"'; done
read1 k8rs-control-plane window=10.016s
read1 k8rs-worker window=20.026s
read1 k8rs-worker2 window=10.006s
read1 k8rs-worker3 window=20.023s
   … reads 2 through 6 reprint those four lines unchanged …
```

Spread over time it moves. Twelve samples, twelve seconds apart:

```
$ for i in $(seq 1 12); do echo "== sample $i $(date -u +%H:%M:%S)"; \
    kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
      | jq -r '.items[] | "  \(.metadata.name) window=\(.window) ts=\(.timestamp)"'; sleep 12; done
== sample 1 04:12:17
  k8rs-control-plane window=20.025s ts=2026-08-29T04:12:01Z
  k8rs-worker window=10.016s ts=2026-08-29T04:11:59Z
  k8rs-worker2 window=20.018s ts=2026-08-29T04:12:04Z
  k8rs-worker3 window=10.015s ts=2026-08-29T04:12:00Z
== sample 2 04:12:29
  k8rs-control-plane window=10.015s ts=2026-08-29T04:12:11Z
  k8rs-worker window=20.024s ts=2026-08-29T04:12:19Z
  k8rs-worker2 window=10.006s ts=2026-08-29T04:12:14Z
  k8rs-worker3 window=20.025s ts=2026-08-29T04:12:20Z
== sample 4 04:12:54
  k8rs-control-plane window=10.013s ts=2026-08-29T04:12:41Z
== sample 5 04:13:06
  k8rs-control-plane window=10.013s ts=2026-08-29T04:12:41Z
   (sample 5 reprints sample 4 exactly — same scrape, all four nodes)
   … samples 3 and 6-10 in the same alternating pattern …
```

Tally over the ten samples:

```
$ F=<the sample file above>
$ echo "samples: $(grep -c '^== sample' $F)"; echo "node-readings: $(grep -c 'window=' $F)"
$ grep -oE 'window=[0-9.]+s' $F | sort -u | wc -l
$ grep -oE 'window=[0-9.]+s' $F | sort | uniq -c | sort -rn
samples: 12
node-readings: 48
17
      6 window=20.025s      3 window=10.016s      1 window=20.018s
      6 window=20.024s      3 window=10.006s      1 window=10.034s
      5 window=20.011s      2 window=20.026s      1 window=10.019s
      4 window=20.022s      2 window=10.041s      1 window=10.017s
      4 window=10.015s      2 window=10.038s      1 window=10.014s
      4 window=10.013s      2 window=10.007s
```

Seventeen distinct values across 48 node-readings, range **10.006s – 10.041s and
20.011s – 20.026s**. Nothing near 15s was observed, though
`--metric-resolution=15s` is what metrics-server is configured with (§ 0).

`window` equals the elapsed time since that node's previous distinct sample.
Measured, after collapsing reads that landed inside one scrape:

```
$ python3 …  # for each node, gap between consecutive distinct timestamps vs the later sample's window
k8rs-control-plane
   ts gap    10s   window on the later sample  10.015s   match=yes
   ts gap    20s   window on the later sample  20.024s   match=yes
   ts gap    10s   window on the later sample  10.013s   match=yes
   ts gap    21s   window on the later sample  20.026s   match=yes
   … the other five pairs on this node and the 27 on the other three …
pairs=36 matches=36 misses=0
```

Per node the gap alternates 10s, 20s, 10s, 20s — averaging the configured 15s
without ever being it. **Why it alternates is not measured here**: the plausible
reading is that metrics-server scrapes on its own 15s tick while the kubelet
refreshes its stats on a 10s one, so each scrape picks up whichever kubelet
sample is current and the gaps snap to multiples of 10. That is an inference and
no command in this report tests it.

Pod windows are far wider spread than node windows, in a single read:

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/pods \
    | jq -r '[.items[].window] | {n: length, distinct: (unique|length), min: (min), max: (max)}'
{
  "n": 27,
  "distinct": 26,
  "min": "10.112s",
  "max": "28.216s"
}
```

26 distinct windows across 27 pods in **one** read: 10.112s to 28.216s. The
`window` is per `PodMetrics` item, not per container — `.containers[]` carries
only `name` and `usage`:

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/pods \
    | jq -r '.items[0] | {top: keys, metadata_keys: (.metadata|keys), container_keys: (.containers[0]|keys)}'
{
  "top": [ "containers", "metadata", "timestamp", "window" ],
  "metadata_keys": [ "creationTimestamp", "labels", "name", "namespace" ],
  "container_keys": [ "name", "usage" ]
}
```

What this measures for a pane that prints a usage line: the number is an average
over an interval that varies read to read, differs between two rows drawn in the
same frame, and is stated by the object in a field the screen does not currently
draw. Also measured: two reads inside one scrape return byte-identical values
(samples 4 and 5 above, and the `RAW-AGAIN` block in § 4), so polling faster
than the scrape returns the same numbers again.

## 4. `kubectl top` against the raw numbers, and the arithmetic between them

A paired read — raw, then `top`, then raw again to prove both hit one scrape:

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
    | jq -r '.items[] | "RAW \(.metadata.name) cpu=\(.usage.cpu) memory=\(.usage.memory) ts=\(.timestamp)"'
$ kubectl --context kind-k8rs top nodes --no-headers
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
    | jq -r '.items[] | "RAW-AGAIN \(.metadata.name) cpu=\(.usage.cpu) ts=\(.timestamp)"'

RAW k8rs-control-plane cpu=70393388n memory=1109200Ki ts=2026-08-29T04:16:42Z
RAW k8rs-worker cpu=34469662n memory=563676Ki ts=2026-08-29T04:16:50Z
RAW k8rs-worker2 cpu=10997201n memory=208984Ki ts=2026-08-29T04:16:44Z
RAW k8rs-worker3 cpu=26950604n memory=483852Ki ts=2026-08-29T04:16:50Z
k8rs-control-plane   71m   0%    1083Mi   4%
k8rs-worker          35m   0%    550Mi    2%
k8rs-worker2         11m   0%    204Mi    0%
k8rs-worker3         27m   0%    472Mi    1%
RAW-AGAIN k8rs-control-plane cpu=70393388n ts=2026-08-29T04:16:42Z
RAW-AGAIN k8rs-worker cpu=34469662n ts=2026-08-29T04:16:50Z
RAW-AGAIN k8rs-worker2 cpu=10997201n ts=2026-08-29T04:16:44Z
RAW-AGAIN k8rs-worker3 cpu=26950604n ts=2026-08-29T04:16:50Z
```

The conversions, and the direction each one rounds:

| raw | ÷1e9 cores, ×1000 | `top` | nearest would be |
|---|---|---|---|
| `70393388n` | 70.393388 milli | **71m** | 70m |
| `34469662n` | 34.469662 milli | **35m** | 34m |
| `10997201n` | 10.997201 milli | **11m** | 11m |
| `26950604n` | 26.950604 milli | **27m** | 27m |

| raw | ×1024 bytes, ÷1048576 | `top` | nearest would be |
|---|---|---|---|
| `1109200Ki` | 1 135 820 800 B = 1083.203 Mi | **1083Mi** | 1083Mi |
| `563676Ki` | 577 204 224 B = 550.457 Mi | **550Mi** | 550Mi |
| `208984Ki` | 213 999 616 B = 204.086 Mi | **204Mi** | 204Mi |
| `483852Ki` | 495 464 448 B = 472.512 Mi | **472Mi** | **473Mi** |

Rows 1 and 2 of the cpu table and row 4 of the memory table are the ones that
discriminate: **`kubectl top` rounds cpu up and truncates memory down.**

An earlier paired read, taken the same way, gives four more discriminating cases:

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1/nodes \
    | jq -r '.items[] | "RAW \(.metadata.name) cpu=\(.usage.cpu) memory=\(.usage.memory)"'
$ kubectl --context kind-k8rs top nodes

RAW k8rs-control-plane cpu=81887169n memory=1111616Ki
RAW k8rs-worker cpu=29089342n memory=582700Ki
RAW k8rs-worker2 cpu=10774935n memory=207452Ki
RAW k8rs-worker3 cpu=26130287n memory=486780Ki
NAME                 CPU(cores)   CPU(%)   MEMORY(bytes)   MEMORY(%)
k8rs-control-plane   82m          0%       1085Mi          4%
k8rs-worker          30m          0%       569Mi           2%
k8rs-worker2         11m          0%       202Mi           0%
k8rs-worker3         27m          0%       475Mi           2%
```

`29089342n` = 29.089 milli → **30m** (nearest 29m). `26130287n` = 26.130 milli
→ **27m** (nearest 26m). `207452Ki` = 202.590Mi → **202Mi** (nearest 203Mi).
`486780Ki` = 475.371Mi → **475Mi**. Same two directions.

`quantity_milli` (`src/rules.rs:7095`) does the same on both counts: it charges
the whole milli it cannot subdivide
(`i64::try_from(numerator.checked_add(denominator - 1)? / denominator)`), and
`bytes` (`src/rules.rs:7174`) truncates (`value / scale`, one decimal place, no
rounding). Applying that arithmetic to the paired read above:

| node | `quantity_milli(cpu)` | `top` cpu | `quantity_milli(memory)` | `top` memory |
|---|---|---|---|---|
| k8rs-control-plane | 71 milli | 71m | 1 135 820 800 B | 1083Mi |
| k8rs-worker | 35 milli | 35m | 577 204 224 B | 550Mi |
| k8rs-worker2 | 11 milli | 11m | 213 999 616 B | 204Mi |
| k8rs-worker3 | 27 milli | 27m | 495 464 448 B | 472Mi |

Every shape measured today parses through `quantity_milli`, including the bare
`"0"`. The suffix arms the metrics API uses are `""`, `"n"` and `"Ki"`, and all
three are proven by the real function's own run in § 2 (`0 -> Some(0)`,
`1n -> Some(1)`, `0n -> Some(0)`, `1Ki -> Some(1024000)`).

**Caveat on how the four-row table above and the `bytes()` column below were
produced.** `quantity_milli`, `bytes` and `cpu_text` are `pub(crate)` and no
`lib.rs` exists (D50), so their values for *these specific strings* were computed
by a transcription of the three functions' arithmetic, not by the functions
themselves — `k8s-admin` may not write `src/` and so cannot add a case to the
table test. What is first-hand: the run in § 2, which fixes every suffix arm and
every size arm the transcription uses (`0`, `1n`, `0n`, `1Ki`, and the five
`bytes()` rows), `kubectl top`, which is an independent oracle and agrees with
all eight cells above, and the end-to-end run below on an `n`/`Ki` pair.

Where the two differ is the **printing**, not the parse. `bytes` switches to the
largest unit that leaves a number above 1 and keeps one decimal:

```
value in bytes   bytes() prints   kubectl top prints
1 135 820 800    1Gi              1083Mi
  577 204 224    550.4Mi          550Mi
  213 999 616    204Mi            204Mi
  495 464 448    472.5Mi          472Mi
```

So the line `screens/analysis.md` § Capacity draws for the control-plane node in
that read would be `using 0.071 cpu and 1Gi` — 0.071 cpu is exactly `top`'s
`71m`, and `1Gi` is 1083Mi at the Gi boundary: `bytes` computes its one decimal
as `(value % scale) * 10 / scale`, which for 1 135 820 800 bytes is
`64 552 960 * 10 / 1 073 741 824` = 0, and `trimmed` then drops the `.0`.
The real pipeline confirms the same two spellings on an `n`/`Ki` pair:

```
$ cargo test --quiet a_node_using_far_less_than_it_asked_for_says_two_different_numbers -- --nocapture
  k8rs-worker   0.45 of 12 cpu · 234Mi of 23.1Gi
      using 0.138 cpu and 1011Mi
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 668 filtered out
```

At the pod level, `top` collapses everything under a mebibyte:

```
$ kubectl --context kind-k8rs top pods -A --no-headers
default              broken-hostpath                              0m    0Mi
default              broken-overhead                              0m    0Mi
kube-system          coredns-…                                    1m    17Mi
kube-system          etcd-k8rs-control-plane                      12m   71Mi
kube-system          kube-apiserver-k8rs-control-plane            22m   338Mi
kube-system          kube-controller-manager-k8rs-control-plane   10m   140Mi
kube-system          kube-scheduler-k8rs-control-plane            6m    73Mi
kube-system          metrics-server-…                             3m    24Mi
local-path-storage   local-path-provisioner-…                     1m    51Mi
   (9 of 27 rows. Elided: 9 more `default` pods, the second coredns, 4 kindnet
    and 4 kube-proxy. Generated ReplicaSet/DaemonSet suffixes cut to `…`.)
```

`broken-hostpath`'s two containers were `412Ki` and `392Ki` in the same read
(§ 2), and `top` prints `0Mi` for their sum.

## 5. `k8s-openapi` ships no `metrics.k8s.io` types

```
$ grep -n 'k8s-openapi' Cargo.toml
29:k8s-openapi = { version = "0.28.0", features = ["v1_36"] }

$ ls ~/.cargo/registry/src/*/k8s-openapi-0.28.0/src/v1_36/api/
admissionregistration  authorization  coordination  events      node    resource       storage
apiserverinternal      autoscaling    core          flowcontrol policy  scheduling     storagemigration
apps                   batch          discovery     mod.rs      rbac
authentication         certificates

$ grep -rl "metrics.k8s.io" ~/.cargo/registry/src/*/k8s-openapi-0.28.0
   (no output)

$ grep -rn "struct NodeMetrics\|struct PodMetrics\|ContainerMetrics" ~/.cargo/registry/src/*/k8s-openapi-0.28.0/src
   (no output)
```

Confirmed independently: no `metrics` directory in the v1_36 API tree, no file
in the crate mentions the string `metrics.k8s.io`, and none of `NodeMetrics`,
`PodMetrics` or `ContainerMetrics` is defined anywhere in it. The
`autoscaling/v1` and `autoscaling/v2` trees are present and are a different
group.

For the fetch this means there is no `k8s_openapi::api::…::NodeMetrics` to hand
`Api::<T>::all(client)`, so the request is either a `DynamicObject` against an
`ApiResource` built from the group/version/kind in § 6, or local `serde` structs
for the four fields in § 1. Which of those the fetch uses is not settled here.

## 6. The discovery entry

```
$ kubectl --context kind-k8rs get --raw /apis \
    | jq -c '.groups[] | select(.name=="metrics.k8s.io")'
{"name":"metrics.k8s.io","versions":[{"groupVersion":"metrics.k8s.io/v1beta1","version":"v1beta1"}],"preferredVersion":{"groupVersion":"metrics.k8s.io/v1beta1","version":"v1beta1"}}
```

Byte-identical to the entry in the brief. The group's resources, which is what
`Discovery::run_aggregated` turns into pairs:

```
$ kubectl --context kind-k8rs get --raw /apis/metrics.k8s.io/v1beta1 \
    | jq -c '{groupVersion, resources: [.resources[] | {name, namespaced, kind, verbs}]}'
{"groupVersion":"metrics.k8s.io/v1beta1","resources":[{"name":"nodes","namespaced":false,"kind":"NodeMetrics","verbs":["get","list"]},{"name":"pods","namespaced":true,"kind":"PodMetrics","verbs":["get","list"]}]}
```

Two resources, `verbs: ["get","list"]` only — no `watch` on either. `nodes` is
cluster-scoped, `pods` is namespaced.

## 7. The four `Metrics` states against this cluster

`src/rules.rs:1211` defines `Read` / `NotInstalled` / `Silent` / `Denied`.

### `Read` — measured

Every command in §§ 1–4 is this state. The group is in `/apis` (§ 6), the
APIService reports `Available=True reason=Passed` (§ 0), and the request returns
four node items and 27 pod items.

### `Denied` — measured, via impersonation, no cluster change

```
$ kubectl --context kind-k8rs --as=nobody-at-all get --raw /apis/metrics.k8s.io/v1beta1/nodes
Error from server (Forbidden): nodes.metrics.k8s.io is forbidden: User "nobody-at-all" cannot list resource "nodes" in API group "metrics.k8s.io" at the cluster scope
exit=1
```

The wire body, fields the formatter would read:

```
$ kubectl --context kind-k8rs --as=nobody-at-all get --raw /apis/metrics.k8s.io/v1beta1/nodes -v=8 2>&1 \
    | sed -n '/Response Body/,/>$/p' | grep -v "Response Body" | tr -d '\t\n ' | sed 's/>$//' \
    | jq -c '{kind,status,reason,code,details}'
{"kind":"Status","status":"Failure","reason":"Forbidden","code":403,"details":{"group":"metrics.k8s.io","kind":"nodes"}}
```

`details.group` and `details.kind` are **both populated** here. That is the
opposite of the `nonResourceURL` refusal measured on 2026-08-26
(`reports/2026-08-26-capability-probe-group-strings.md`), where `details` came
back empty — so a `Denied` sentence built from `details.group`/`details.kind`
has real values to put in it on this path.

Note `details.kind` is `nodes`, the plural resource name, not the `NodeMetrics`
kind from § 6.

### `NotInstalled` — the discovery signal is measured; the request-path shape is measured on a stand-in

The signal `Capability::Metrics` keys on is the group's presence in `/apis`
(§ 6). Its absence could not be produced here without uninstalling
metrics-server, which is out of bounds. What the *request* returns when a group
is not served was measured against a group no cluster has:

```
$ kubectl --context kind-k8rs get --raw /apis/nosuch.k8s.io/v1beta1/nodes
Error from server (NotFound): the server could not find the requested resource
exit=1
```

The wire body is **not** a `Status`:

```
$ kubectl --context kind-k8rs get --raw /apis/nosuch.k8s.io/v1beta1/nodes -v=8   # Response Body
404 page not found
```

Three neighbouring paths, for the shapes a fetch can actually hit:

```
$ for p in /apis/metrics.k8s.io/v1beta2/nodes /apis/metrics.k8s.io/v1beta1/nosuchresource \
           /apis/metrics.k8s.io/v1beta1/nodes/no-such-node; do
    echo "=== $p ==="
    kubectl --context kind-k8rs get --raw "$p" -v=8 2>&1 \
      | sed -n '/Response Body/,/>$/p' | grep -v "Response Body" | tr -d '\t\n ' | sed 's/>$//'
  done
   (spaces inside the message strings are stripped by the `tr` above)

/apis/metrics.k8s.io/v1beta2/nodes            404 page not found
/apis/metrics.k8s.io/v1beta1/nosuchresource   {"kind":"Status",…,"reason":"NotFound","details":{},"code":404}
/apis/metrics.k8s.io/v1beta1/nodes/no-such-node
                                              {"kind":"Status",…,"reason":"NotFound","details":{"name":"no-such-node","kind":"node"},"code":404}
```

So an absent **group** and an absent **version of a served group** both return
the bare text; an absent resource inside a served version returns a real
`Status` with empty `details`; an absent object returns a `Status` with
`details.name`/`details.kind`. This confirms on the metrics path what `src/k8s.rs` already
records for the browser, in `not_acceptable`'s doc comment (*"the literal body
`404 page not found` — not a `Status` at all"*): kube wraps the bare body
as `Status::failure(text, "Failed to parse error data").with_code(404)`, so
`.code` is 404 and `.reason` is that phrase and not `NotFound`.

### `Silent` — unmeasured on this cluster

`Metrics::Silent` is documented as *the API is registered and the request failed
or timed out*. Producing it needs the metrics-server backend to stop answering
while its APIService stays registered — a delete, a scale-to-zero or a network
break, all of which are out of bounds here. **Not measured.**

The passage the brief points at was read — the `served`/capability-probe
comment block in `src/k8s.rs`, the paragraph beginning *"Nothing in this file
draws that distinction, and no version of it has"*. It records a
`v1beta1.metrics.k8s.io` APIService
whose Service does not exist producing a capability banner byte-identical to a
cluster with no metrics-server at all. That is the **same cluster condition** as
`Silent` but a **different signal**: it measures what *discovery* returns
(`Discovery::run_aggregated` drops `freshness`, so a group whose `resources`
array came back empty contributes no pair), whereas `Metrics::Silent` is what
the *metrics request itself* returns. Neither that measurement nor this one has
read the request-path error for a registered-but-dead backend. **What status
code and what body the aggregator returns in that case is unmeasured** — nothing
in this report or that one produced one, and no claim about it is made here. It
stays owed.

## Numbers in the brief, re-run

| brief's claim | re-measured | verdict |
|---|---|---|
| cpu carries `n`, memory carries `Ki` | § 1 | holds |
| `cpu: "0"` with no suffix, common | 12 of 28 containers, § 2 | holds, and counted |
| window ≠ `--metric-resolution=15s`, spread 10–20s | 17 distinct values, 10.006s–20.026s over 48 node-readings; pods 10.112s–28.216s in one read, § 3 | holds, and wider than shown |
| `/apis/metrics.k8s.io/v1beta1/pods` — 27 items | 27 items, **28 containers** | holds; the container count is one higher |
| `broken-hostpath/shipper cpu="0" memory="412Ki"` | `default/broken-hostpath`, container `shipper`, cpu `0`, memory `412Ki` | holds; the namespace is `default`, and `broken-hostpath` is the pod, not a namespace |
| discovery entry for `metrics.k8s.io` | § 6 | byte-identical |
| `k8s-openapi` ships no `metrics.k8s.io` types | § 5 | holds |
| metrics-server v0.8.0, `--metric-resolution=15s`, `--kubelet-insecure-tls` | § 0 | holds |
| four nodes, v1.36.1 | § 0 | holds; client is v1.36.3, server v1.36.1 |

No number in the brief was found wrong. The individual usage values differ from
the brief's because they are a different scrape.

## Cluster state at the end

Nothing was created, modified or deleted. The only non-`get` verbs used were
`--as` impersonation on a read, which the apiserver refused before it reached
any object.
