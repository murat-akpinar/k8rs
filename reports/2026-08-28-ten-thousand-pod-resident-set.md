# Resident set at 11, 111, 1 011 and 10 011 pods, with the three workload watches (2026-08-28)

Ephemeral measurement by `k8s-admin` for the Phase 5 box *Measure resident
memory against 10 000 pods plus the three workload watches*
([NOTES § D25](../NOTES.md#d25--what-this-review-did-not-decide)).

Cluster: a throwaway `kind` cluster created and deleted inside this run under
`K8RS_CLUSTER=review` ([NOTES § D92](../NOTES.md#d92--who-may-touch-a-cluster-split-by-the-artifact-and-not-by-the-agent-2026-08-15)) —
one control plane, one worker, `kindest/node:v1.36.1`, its own API port so the
PM's fixture cluster `k8rs` was never touched and stayed up beside it. No
artifact of this cluster was committed; the generated objects are described
below and are not a fixture.

Binary under test: `target/release/k8rs` at `c66b311`, tree clean, rebuilt
before the run (`cargo build --release` → `Finished` in 0.11s, already current).

Host: 12 cores, 23 GiB, the `k8rs` fixture cluster running throughout.

## How each number was taken

`/usr/bin/time` is **not installed on this machine** —

```
$ /usr/bin/time -v timeout -s INT 60 ./target/release/k8rs --live --context kind-review
(eval):1: no such file or directory: /usr/bin/time
```

— so peak comes from the kernel's own high-water mark, `VmHWM` in
`/proc/<pid>/status`, and steady state from `VmRSS` in the same file. Both are
KiB, as `/proc` reports them.

The sampler starts `k8rs --live --context kind-review`, reads both fields every
100 ms (500 ms for readings 1 and 4), notes when the first report reaches
stdout, and kills the process at the end of the window. The two variants differ
only in that interval:

```sh
./target/release/k8rs --live --context kind-review >"$LABEL.out" 2>"$LABEL.err" &
PID=$!
for _ in $(seq $((SECS*10))); do
  awk '/^VmRSS:/{print $2}' /proc/$PID/status
  awk '/^VmHWM:/{print $2}' /proc/$PID/status
  sleep 0.1
done
```

**"Settled" is defined here as: the store has published a snapshot** — which
`Store::snapshot` only does once all five initial LISTs have landed, so the
first line on stdout is the proof — **and `VmRSS` has stopped changing.** In
every reading below `VmRSS` reached its final value within 2.3 s and then did
not move again for the rest of the window (60–202 s), so the steady-state figure
is the last sample and every sample after the second is identical to it.

## The readings

| # | pods | nodes | Deployments | StatefulSets | DaemonSets | peak `VmHWM` | steady `VmRSS` | first report |
|---|---|---|---|---|---|---|---|---|
| 1 baseline | 11 | 2 | 2 | 0 | 2 | 11 244 KiB | 11 244 KiB | 0.52 s |
| 2 control | 111 | 2 | 2 | 0 | 2 | 19 492 KiB | 19 492 KiB | 0.12 s |
| 3 attribution | 111 | 2 | 1 002 | 200 | 32 | 51 216 KiB | 51 216 KiB | 0.23 s |
| 4 **~1 000 pods** | 1 011 | 2 | 102 | 20 | 12 | 58 752 KiB | 58 752 KiB | 0.53 s |
| 5 **~10 000 pods** | 10 011 | 2 | 1 002 | 200 | 32 | 128 844 KiB | 125 704 KiB | 1.53 s |
| 5' 10 000, repeat | 10 011 | 2 | 1 002 | 200 | 32 | 129 000 KiB | 119 800 KiB | 1.42 s |

In MB (decimal, the unit `REQUIREMENTS.md` states its budget in): reading 4 is
**60.2 MB peak and steady**; reading 5 is **131.9 MB peak, 128.7 MB steady**;
the baseline is **11.5 MB**.

`REQUIREMENTS.md:198` states **`Memory: < 50MB RSS at ~1000 pods`**. The measured
value at 1 011 pods is 58 752 KiB = 57.4 MiB = 60.2 MB.

Readings 2 and 3 were taken to make reading 5 attributable: 3 differs from 5
only in pod count, and 2 differs from 3 only in the generated workloads.
Reading 1 is the bare cluster, before anything was generated.

**Run order was 1, 4, 5, 5', 3, 2** — the table is in size order, the run was
not. Readings 3 and 2 were taken by deleting down from the 10 000-pod cluster,
so they are readings on an API server that had just held 10 000 pods. Each
`k8rs` run is a fresh process, so nothing carried over on the client side.

**The `first report` column has two different resolutions.** Readings 1 and 4
were sampled at 2 Hz, so their figures are upper bounds to the nearest 0.5 s and
the true value may be anything below; readings 2, 3, 5 and 5' were sampled at
10 Hz.

**The `k8rs` cluster's own numbers are not in this table** — it was not measured,
only left alone.

### 1 — baseline, nothing generated

```
$ for k in pods nodes deployments statefulsets daemonsets; do printf "%s=%s\n" "$k" \
    "$(kubectl --context kind-review get $k -A --no-headers | wc -l)"; done
pods=11
nodes=2
deployments=2
statefulsets=0
daemonsets=2
$ ./measure.sh baseline 60
label=baseline first_report_after=.524986877s peak_VmHWM_kB=11244
60.374933684 11244 11244
60.889953751 11244 11244
61.404191649 11244 11244
```

Curve (elapsed s, `VmRSS`, `VmHWM`), first samples:

```
.008102154 8120 8908
.524986877 11232 11232
1.046177398 11232 11232
```

`VmHWM` ends 12 KiB above `VmRSS`; nothing else moves for 60 s. Printed report:

```
11 pods · 2 nodes

○ nothing is broken
```

**A bare `kind` cluster has no StatefulSet at all**, so the StatefulSet watch is
empty in this one reading. The brief asks for all three workload watches to be
non-empty "at each size"; that is true of readings 3, 4, 5 and 5', and cannot be
true of a baseline defined as *nothing generated*. Reading 2 is the same shape
with more pods and is the control for reading 3.

### 4 — ~1 000 pods

```
$ for k in pods deployments statefulsets daemonsets; do printf "%s=%s\n" "$k" \
    "$(kubectl --context kind-review get $k -A --no-headers | wc -l)"; done
pods=1011
deployments=102
statefulsets=20
daemonsets=12
$ ./measure.sh pods1k 90
label=pods1k first_report_after=.526569094s peak_VmHWM_kB=58752
91.298554775 58752 58752
91.814393613 58752 58752
92.326057300 58752 58752
```

First samples — the whole rise happens before the first snapshot is published:

```
.011062927 7952 8136
.526569094 58752 58752
1.047384920 58752 58752
```

and `58752` is then every one of the remaining 178 samples. Printed report:

```
1011 pods · 2 nodes

○ nothing is broken
```

### 5 — ~10 000 pods

```
$ for k in pods nodes deployments statefulsets daemonsets; do printf "%s=%s\n" "$k" \
    "$(kubectl --context kind-review get $k -A --no-headers | wc -l)"; done
pods=10011
nodes=2
deployments=1002
statefulsets=200
daemonsets=32
$ kubectl --context kind-review -n gen get pods --no-headers | awk '{print $2" "$3}' | sort | uniq -c
  10000 1/1 Running
$ ./measure10hz.sh pods10k 180
label=pods10k first_report_after=1.525826037s peak_VmHWM_kB=128844
202.317243298 125704 128844
202.436056860 125704 128844
202.554356191 125704 128844
```

The rise and the settle, at 10 Hz (elapsed s, `VmRSS`, `VmHWM`):

```
0.0 7988 8116
0.6 81996 81996
1.1 102944 102944
1.6 120076 128844
2.2 125704 128844
2.8 125704 128844
```

and `125704 128844` for all 1 790 samples after that, out to 202 s:

```
22.3 125704 128844
112.1 125704 128844
202.6 125704 128844
```

Peak is reached at ~1.6 s, during the initial LIST; `VmRSS` then falls 3 140 KiB
(2.4 %) below it and does not move again. The repeat run gave `peak 129000`,
steady `119800` — peak within 0.12 %, steady 4.7 % lower.

Header line, stderr first and then the report's own first line. **This is the
one run whose report is not `nothing is broken`** — three cards, all about the
cluster's own CNI, in the observations below:

```
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
10011 pods · 2 nodes
```

### 2 and 3 — the attribution pair

```
$ python3 gen.py delete 0 9900
errors=0
python3 gen.py delete 0 9900  3,52s user 0,63s system 29% cpu 14,118 total
$ kubectl --context kind-review -n gen get pods --no-headers | awk '{print $3}' | sort | uniq -c
    100 Running
$ ./measure10hz.sh pods100 60           # 111 pods, 1 002 + 200 + 32 workloads
label=pods100 first_report_after=.231548281s peak_VmHWM_kB=51216
69.133329906 51216 51216
$ kubectl --context kind-review -n gen delete deployments,statefulsets,daemonsets --all --wait=false
$ ./measure10hz.sh pods100-noworkloads 45   # 111 pods, 2 + 0 + 2 workloads
label=pods100-noworkloads first_report_after=.118751750s peak_VmHWM_kB=19492
51.874242776 19492 19492
```

Arithmetic on the table, and nothing more than arithmetic:

- **3 → 5**, pods alone changing: (125 704 − 51 216) / 9 900 = **7.52 KiB per
  pod**, the slope between 111 and 10 011 pods at a fixed workload set.
- **4** sits 764 KiB above what that slope predicts for 1 011 pods, with 1 100
  *fewer* workload objects than reading 3.
- **2 → 3**, workloads alone changing: (51 216 − 19 492) / 1 230 = **25.8 KiB per
  workload object** between 4 and 1 234 of them.
- **1 → 2**, pods alone changing: (19 492 − 11 244) / 100 = **82.5 KiB per pod**
  between 11 and 111 pods.

The three per-object figures — 7.52, 25.8 and 82.5 KiB — are slopes between
different pairs of readings and do not agree with each other: resident set here
is not a constant per object, and the marginal falls as the count rises. No
model is fitted in this file.

## What the generated objects were, and what was verified about them

The pod is the committed capture `tests/fixtures/healthy.json` with a
`schedulerName` nothing answers to, so no scheduler and no kubelet ever touches
it. Four things about that shape were checked against the API server rather than
assumed, and one more was measured off the object it returned:

**Status is wiped on CREATE.** `POST` of a pod carrying a full `status` came back
`201` with:

```
create status code: 201
status after CREATE: {"phase": "Pending", "qosClass": "Burstable"}
```

so every pod needs a second call, `PUT .../pods/<name>/status`.

**The sanitized capture cannot be replayed as-is.** The first `PUT` came back
`422 Invalid`, `status.podIPs[0]` and `status.hostIPs[0]` — the sanitizer had
replaced both addresses with a placeholder the API server rejects as not an
address. The generator substitutes made-up ones.

**With the status restored the pod reads as healthy and stays that way**:

```
$ kubectl --context kind-review -n gen get pods
NAME          READY   STATUS    RESTARTS   AGE
gen-0000000   1/1     Running   0          3s
```

and at 10 000 of them, still `10000 1/1 Running` when checked immediately before
the reading. **No rule fires on them** — the 1 011-pod run printed
`○ nothing is broken`, and the 10 000-pod run's three cards are all about the
cluster's own CNI (below), never a generated pod.

**No controller created a pod behind the generator.** Pod totals were exactly
`generated + 11` (the cluster's own) at every size: 1 011 pods with 100 generated
Deployments, 10 011 with 1 000. Deployments and StatefulSets carry `replicas: 0`, which
`short_of_pods` gates out at `desired == 0`; DaemonSets carry a `nodeSelector`
no node matches, so the DaemonSet controller wants zero pods.

**Object size on the wire, for the prune line.** One generated pod as the API
server serves it:

```
bytes as served (compact JSON): 7451
managedFields entries: 2
managedFields bytes: 2853
```

so `managedFields` is **38.3 %** of that object and what survives the prune's
first step is 4 598 bytes. `src/k8s.rs:1868` cites a median of 3 708 bytes over
the committed captures, which are sanitized and therefore already stripped; this
capture is 5 188 bytes compact in the repo.

## Two observations the run produced and this file does not act on

**Time to first report** (`--live` prints when the store first publishes, which
is after all five initial LISTs): 0.52 s at 11 pods and 0.53 s at 1 011 — both
upper bounds at 0.5 s sampling resolution — then 1.53 s and 1.42 s at 10 011,
measured at 10 Hz. This is the driver's stdout, not a TUI first paint.

**At 10 000 pods the cluster's own CNI DaemonSet ran out of memory, and k8rs
reported it.** `kindnet-cni` has a 50Mi limit and watches every pod; both
replicas were `OOMKilled` with `exit 137` during the 10 000-pod run, and the
10 000-pod report is therefore not `nothing is broken` but two CRITICAL cards
(rule 2) and one WARN (rule 5), naming `kube-system/kindnet-*`, `limit 50Mi`,
`exit 137`, 2 and 3 restarts. Three cards, not 10 000 — the generated pods stayed
silent throughout.

## What was not measured

- **Any point between 1 011 and 10 011 pods**, and any point between 11 and 111.
  The curve above is four points.
- **A cluster where the pods are real.** Every generated pod is unscheduled and
  has no kubelet writing to it, so there is no watch traffic from them after the
  initial LIST: this measures the LIST and the resting store, not a cluster under
  churn. A 10 000-pod cluster with pods actually starting and stopping would send
  events these did not.
- **Where the resident set goes.** No allocator instrumentation, no heap profile;
  `VmRSS` is the only instrument here.
- **The effect of `INITIAL_LIST_PAGE`.** It was 500 for every reading; no other
  page size was run.
- **Whether readings 2 and 3 reproduce on a cluster that never held 10 000
  pods.** Both were taken by deleting down from the 10 000-pod cluster.
- **Anything on a machine that is not this one**, and anything about the `k8rs`
  fixture cluster.

## The generator, verbatim

Throwaway, not committed, run from a scratch directory against a
`kubectl --context kind-review proxy --port 8011`. One literal is replaced by a
placeholder below under [reports/README.md](README.md)'s rule: `HOST_IP` held a
syntactically valid IPv4 address in the node network of the throwaway cluster,
and any valid address does — the API server validates only the form.

```python
#!/usr/bin/env python3
"""Fill a kind cluster with realistic-but-inert objects, through `kubectl proxy`.

  kubectl --context kind-review proxy --port 8011 &
  ./gen.py pods 0 1000        # create pods gen-0000000..gen-0000999
  ./gen.py workloads 100 20 10
  ./gen.py delete 0 9900

Pods are the committed healthy-pod capture with a scheduler name nothing
answers to, created and then given their status back through /status.
Deployments and StatefulSets carry replicas: 0 and DaemonSets a nodeSelector
no node matches, so no controller creates a pod behind our back.
"""
import json, sys, http.client, threading, queue

PROXY = ("127.0.0.1", 8011)
NS = "gen"
FIX = "/home/shyuuhei/GIT/k8rs/tests/fixtures/healthy.json"
HOST_IP = "<any syntactically valid IPv4 address>"


def call(conn, method, path, body=None, ctype="application/json"):
    conn.request(method, path, json.dumps(body) if body is not None else None,
                 {"Content-Type": ctype, "Accept": "application/json"})
    r = conn.getresponse()
    return r.status, r.read()


def pod_template():
    d = json.load(open(FIX))
    m = d["metadata"]
    for k in ("uid", "resourceVersion", "creationTimestamp", "generation"):
        m.pop(k, None)
    m["namespace"] = NS
    s = d["spec"]
    s.pop("nodeName", None)
    # Nothing answers to this name, so no scheduler and no kubelet ever
    # touches the pod; spec.nodeName would invite the pod GC instead.
    s["schedulerName"] = "nobody-answers-to-this"
    return d


def workload(kind, name, pod_spec):
    meta = {"name": name, "namespace": NS, "labels": {"app": name}}
    tmpl = {"metadata": {"labels": {"app": name}}, "spec": pod_spec}
    api = "apps/v1"
    if kind == "DaemonSet":
        # No node carries this label, so the DaemonSet controller wants zero pods.
        tmpl["spec"] = dict(pod_spec, nodeSelector={"k8rs.io/nowhere": "true"})
        spec = {"selector": {"matchLabels": {"app": name}}, "template": tmpl}
    else:
        spec = {"replicas": 0, "selector": {"matchLabels": {"app": name}},
                "template": tmpl}
        if kind == "StatefulSet":
            spec["serviceName"] = name
    return {"apiVersion": api, "kind": kind, "metadata": meta, "spec": spec}


def worker(q, out):
    conn = http.client.HTTPConnection(*PROXY)
    while True:
        job = q.get()
        if job is None:
            q.task_done(); conn.close(); return
        try:
            for method, path, body in job:
                st, data = call(conn, method, path, body)
                if st not in (200, 201):
                    out.append((st, data[:200])); break
        except Exception as e:                      # noqa: BLE001
            out.append((0, repr(e)[:200]))
            conn = http.client.HTTPConnection(*PROXY)
        q.task_done()


def run(jobs, threads=24):
    q, out = queue.Queue(maxsize=2000), []
    ts = [threading.Thread(target=worker, args=(q, out), daemon=True) for _ in range(threads)]
    for t in ts:
        t.start()
    for j in jobs:
        q.put(j)
    for _ in ts:
        q.put(None)
    q.join()
    return out


def pod_jobs(lo, hi):
    tmpl = pod_template()
    base = f"/api/v1/namespaces/{NS}/pods"
    for i in range(lo, hi):
        d = json.loads(json.dumps(tmpl))
        name = f"gen-{i:07d}"
        d["metadata"]["name"] = name
        status = d.pop("status")
        for cs in status.get("containerStatuses", []) + status.get("initContainerStatuses", []):
            cs.pop("containerID", None)
        # The committed capture is sanitized: its addresses are placeholders,
        # which the API server refuses as invalid. Made up, in kind's own ranges.
        pod_ip = f"10.244.{i // 250 % 250}.{i % 250}"
        status["podIP"], status["podIPs"] = pod_ip, [{"ip": pod_ip}]
        status["hostIP"], status["hostIPs"] = HOST_IP, [{"ip": HOST_IP}]
        yield [("POST", base, d),
               ("PUT", f"{base}/{name}/status", {"apiVersion": "v1", "kind": "Pod",
                                                 "metadata": {"name": name, "namespace": NS},
                                                 "status": status})]


def workload_jobs(nd, ns_, nds):
    spec = pod_template()["spec"]
    for kind, n, plural in (("Deployment", nd, "deployments"),
                            ("StatefulSet", ns_, "statefulsets"),
                            ("DaemonSet", nds, "daemonsets")):
        for i in range(n):
            name = f"{plural[:-1]}-{i:05d}"
            yield [("POST", f"/apis/apps/v1/namespaces/{NS}/{plural}",
                    workload(kind, name, spec))]


def delete_jobs(lo, hi):
    # A pod that was never scheduled has no kubelet to confirm its deletion, so
    # the API server removes it at once rather than leaving it Terminating.
    for i in range(lo, hi):
        yield [("DELETE", f"/api/v1/namespaces/{NS}/pods/gen-{i:07d}", None)]


if __name__ == "__main__":
    what = sys.argv[1]
    if what == "delete":
        errs = run(list(delete_jobs(int(sys.argv[2]), int(sys.argv[3]))))
    elif what == "pods":
        errs = run(list(pod_jobs(int(sys.argv[2]), int(sys.argv[3]))))
    else:
        errs = run(list(workload_jobs(*[int(a) for a in sys.argv[2:5]])), threads=8)
    print(f"errors={len(errs)}")
    for e in errs[:3]:
        print(e)
```

Generation cost, for anyone repeating this: 10 000 pods is 20 000 HTTP calls
over 24 connections and took **15.3 s** of wall clock summed over the five
batched runs it was split into — batched because a single Bash call here is
capped at ten minutes, not because the API server needed it; 1 234 workload
objects took **1.7 s** over two runs; deleting 9 900 pods took **14 s**.

```
$ python3 gen.py pods 1 500
errors=0
python3 gen.py pods 1 500  0,43s user 0,09s system 67% cpu 0,770 total
$ python3 gen.py pods 1000 4000
errors=0
python3 gen.py pods 1000 4000  2,43s user 0,50s system 62% cpu 4,692 total
```

The 130 errors reported by the second `workloads` run are `409 AlreadyExists` for
the 130 objects the first run had created; the generator numbers from zero.

## Teardown

```
$ K8RS_CLUSTER=review scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: [the two nodes of the throwaway cluster]
$ kind get clusters
k8rs
```

`kind create cluster` sets the current kubectl context, so this run left it
unset on delete; it was put back to `kind-k8rs` afterwards. The fixture cluster
answered with its 4 nodes after the teardown.
