# `connect()` on a real cluster — the idle proof, the refused watch, and the read-only role

Operator review measurements for the Phase 5 `connect()` box (`src/k8s.rs`
§ CONNECTING, `src/main.rs` § WATCHING A CLUSTER).

Ephemeral cluster, `K8RS_CLUSTER=review`, one control-plane and one worker,
`kindest/node:v1.36.1`, API on port 6444 so it does not collide with the
fixture cluster's 6443. Nodes are `review-control-plane` / `review-worker`,
which `scripts/sanitize.jq` refuses, so nothing here can become a fixture
([D94](../NOTES.md#d94--the-first-review-cluster-was-named-k8rs-review-and-a-guard-the-obvious-wrong-name-walks-straight-past-is-not-a-guard-2026-08-15)).
Binary: `cargo build` (debug), tree as reviewed, uncommitted on `development`.

Host condition at run time: 23 GiB RAM, 16 GiB available, `/tmp` 34% of 12 GiB,
`$HOME` 5% of 954 GB. The PM's `k8rs` fixture cluster was up beside this one and
**no capture was in flight** — `tests/fixtures/` last written 2026-08-23.

```
$ kind get clusters
k8rs
review
$ kubectl --context kind-review get nodes
NAME                   STATUS   ROLES           AGE   VERSION
review-control-plane   Ready    control-plane   30s   v1.36.1
review-worker          Ready    <none>          15s   v1.36.1
```

---

## 1. The idle proof — a severed socket, and nobody touching the keyboard

`PRIOR-ART § B3`. Run left alone in a terminal, stdout and stderr merged and
timestamped by `awk`; the control plane stopped, left down 3m42s, started, and
not touched again.

```
$ ./target/debug/k8rs --live --context kind-review 2>&1 \
    | stdbuf -oL awk '{ printf "%s | %s\n", strftime("%H:%M:%S"), $0; fflush() }'
```

### Bootstrap

```
START 13:15:53
13:15:53 | k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
13:15:53 | 11 pods · 2 nodes
13:15:53 |
13:15:53 | ○ nothing is broken
```

First report block **under one second** after launch.

### The outage

```
$ docker stop review-control-plane        # 13:16:48
review-control-plane
```

All five watch-health lines appeared inside the same second, one kind at a
time, as five growing blocks. Final block during the outage:

```
13:16:48 | ▲ k8rs cannot read pods from this cluster right now — what is shown about them may be out of date
13:16:48 | ▲ k8rs cannot read nodes from this cluster right now — what is shown about them may be out of date
13:16:48 | ▲ k8rs cannot read Deployments from this cluster right now — what is shown about them may be out of date
13:16:48 | ▲ k8rs cannot read StatefulSets from this cluster right now — what is shown about them may be out of date
13:16:48 | ▲ k8rs cannot read DaemonSets from this cluster right now — what is shown about them may be out of date
13:16:48 |
13:16:48 | 11 pods · 2 nodes
13:16:48 |
13:16:48 | ○ nothing is broken
```

Nothing further printed for the remaining 3m30s.

### Alive, and what it cost

```
$ cat /proc/3112419/stat | awk '{print $14+$15}'   # utime+stime, CLK_TCK=100
# 60-second window with the control plane down:
window: 60s   ticks: 10 -> 13   delta=3 ticks = .03s CPU

$ date +%H:%M:%S; ps -o pid,%cpu,etime,stat --no-headers -p 3112419
13:20:23
3112419  0.0       04:30 S<l
total ticks: 15
output lines: 46      # unchanged since 13:16:48
```

Reference figure to compare against (brief): 0.20s CPU over 20s.
Measured here: **0.03s CPU over 60s**, process `S` (sleeping), output frozen.

### The recovery, with nobody touching it

```
$ docker start review-control-plane       # 13:20:30
review-control-plane
```

```
13:21:25 | (DaemonSets line gone)      +55s after start
13:21:52 | (nodes line gone)           +82s
13:21:58 | (pods line gone)            +88s
13:21:58 | (Deployments line gone)     +88s
13:22:11 | (StatefulSets line gone)   +101s
13:22:11 | 11 pods · 2 nodes
13:22:11 |
13:22:11 | ○ nothing is broken
13:22:11 |
```

Last block carries no `▲` at all. The string
`k8rs: every watch has stopped, so nothing is being read any more` never
printed; the process never exited.

Idle again after recovery:

```
post-recovery idle 30s: ticks 30 -> 30, delta=0 = 0s CPU
3112419  0.0       11:49 25840          # RSS KiB, 11-pod cluster
```

---

## 2. The banner, checked against `kubectl` for the same cluster

```
$ kubectl --context kind-review version -o json | ... serverVersion.gitVersion
serverGitVersion: v1.36.1

$ kubectl --context kind-review api-resources --verbs=list --no-headers | wc -l
60
$ kubectl --context kind-review api-resources --no-headers | wc -l
67
$ comm -13 <listable> <all>
bindings
localsubjectaccessreviews
selfsubjectaccessreviews
selfsubjectreviews
selfsubjectrulesreviews
subjectaccessreviews
tokenreviews
```

Banner says `server v1.36.1 · 60 kinds`. Both match; the seven dropped are
exactly the non-listable review/binding endpoints `browsable`'s `LIST` filter
removes.

Capability set on a cluster exactly as `kind create cluster` left it:
`{DisruptionBudgets}` — not `Some(∅)`.

---

## 3. Aggregated vs legacy discovery, with a registered APIService whose backend is down

A `v1beta1.metrics.k8s.io` APIService pointing at a Service that does not exist
(the shape a crashlooping metrics-server has):

```
$ kubectl --context kind-review get apiservice v1beta1.metrics.k8s.io \
    -o jsonpath='{...conditions...}'
Available=False reason=ServiceNotFound

$ kubectl --context kind-review get --raw /apis/metrics.k8s.io/v1beta1
Error from server (ServiceUnavailable): the server is currently unable to handle the request
```

The legacy `/apis` list still **names** the group:

```
$ KUBECONFIG=<read-only sa> kubectl get --raw /apis
... {"name":"metrics.k8s.io","versions":[{"groupVersion":"metrics.k8s.io/v1beta1", ...
```

k8rs, same instant:

```
$ ./target/debug/k8rs --live --context kind-review
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
11 pods · 2 nodes

○ nothing is broken
```

Kind count unchanged at 60; `Metrics` **absent** from the capability set; the
sidebar survives. Byte-identical to the same banner before the APIService
existed — no field in the answer distinguishes *registered and down* from
*not installed*. `grep -n "list_api_groups_aggregated" src/k8s.rs` matches only
prose (lines 2007, 2258); `served()` calls `Discovery::new(...).run_aggregated()`.

The apiserver counted the aggregation failures:

```
$ kubectl --context kind-review get --raw /metrics | grep apiserver_request_terminations
apiserver_request_terminations_total{code="503",component="aggregator",...,subresource="/apis",verb="GET",...} 10
```

---

## 4. A watch the cluster refuses — the retry rate, measured

ServiceAccount granted `get,list,watch` on `pods` only, plus
`nonResourceURLs: ["/version","/api","/api/*","/apis","/apis/*"]`.

```
$ KUBECONFIG=<probe> kubectl auth can-i list pods
yes
$ KUBECONFIG=<probe> kubectl auth can-i list nodes
no
$ KUBECONFIG=<probe> kubectl get nodes
Error from server (Forbidden): nodes is forbidden: User "system:serviceaccount:default:probe" cannot list resource "nodes" in API group "" at the cluster scope
```

Counter used: `authorization_attempts_total{result="no-opinion"}` on the
apiserver's `/metrics`. Its baseline drift with nothing running:

```
t0=13:31:11 no-opinion=6
t1=13:33:11 no-opinion=6   delta over 120s idle = 0
```

k8rs run against that kubeconfig, four of five watches refused:

```
COUNTER_BEFORE=6 at 13:33:23
t=13:39:30  counter now=1233  delta=1227 over 360s

second window: 120s   1330 -> 1728   delta=398
rate=3.31/s across 4 refused kinds
per kind: .82/s   =>  interval 1.20s
extrapolated per kind per hour: 2985
```

Process cost over the same period:

```
3152741  0.9       06:07 S<l
ticks: 344      # 3.44s CPU over 367s
ticks: 483      # at 8m30s
```

Steady state, not a ramp: the second window's rate equals the first's.

`apiserver_request_total` does not count these — authorization denials
short-circuit before it:

```
$ kubectl --context kind-review get --raw /metrics | grep '^apiserver_request_total' | grep 'code="403"'
(no output)
```

### The same rate with an invalid credential

`exec` credential plugin that succeeds and returns a token the server rejects:

```
$ KUBECONFIG=<exec-ok> kubectl get pods
error: You must be logged in to the server (Unauthorized)
```

```
authn errors before: 470
after 90s: 853  delta=383
rate all five: 4.25/s   per watch: .85/s  interval 1.17s
per watch per hour: 3064   all five per hour: 15320
```

### The mechanism, off kube 4.2.0's own source

```
$ sed -n '9,14p' kube-runtime-4.2.0/src/utils/stream_backoff.rs
/// Applies a [`Backoff`] policy to a [`Stream`]
///
/// After any [`Err`] is emitted, the stream is paused for [`Backoff::next_backoff`]. The
/// [`Backoff`] is [`reset`](`Backoff::reset`) on any [`Ok`] value.
```

```
$ sed -n '86,91p' kube-runtime-4.2.0/src/utils/stream_backoff.rs
            Poll::Ready(_) => {
                tracing::trace!("Non-error received, resetting backoff");
                this.backoff.reset();
            }
```

```
$ sed -n '521,525p' kube-runtime-4.2.0/src/watcher.rs
        State::Empty => match wc.initial_list_strategy {
            InitialListStrategy::ListWatch => (Some(Ok(Event::Init)), State::InitPage {
$ sed -n '578,585p' kube-runtime-4.2.0/src/watcher.rs
                Err(err) => {
                    ...
                    (Some(Err(Error::InitialListFailed(err))), State::Empty)
```

```
$ sed -n '51,55p' kube-runtime-4.2.0/src/utils/backoff_reset_timer.rs
impl<B: Backoff> Backoff for ResetTimerBackoff<B> {
    fn reset(&mut self) {
        self.backoff.reset();
    }
}
```

`DefaultBackoff` is `ResetTimerBackoff<ExponentialBackoff>`, and its `reset()`
forwards unconditionally — the 120s timer in `next()` is not consulted.

Emitted sequence on a standing list refusal: `Ok(Init)`, `Err`, `Ok(Init)`,
`Err`, … — one `Ok` between every pair of `Err`s. Measured interval 1.17–1.20s
against `DefaultBackoff`'s first step of 800ms plus a jitter that only adds.

### What the two tests in the tree measure

`src/k8s_tests.rs:5494` `kubes_default_backoff_never_gives_up_and_costs_under_130_requests_an_hour`
drives `watcher::DefaultBackoff::default()` with `policy.next()` in a loop and
never calls `reset()`.

`src/k8s_tests.rs:5614` `a_refused_watch_of_every_kind_waits_before_it_asks_again`
takes `watch.take(4)` per stream — `Init`, `Err`, `Init`, `Err` — and asserts
`waited >= 500ms`.

---

## 5. One refused kind, and what the reader is shown

`docs/security.md`'s `k8rs-readonly` with `"nodes"` removed from the core rule
and everything else intact:

```
$ KUBECONFIG=<ro> kubectl auth can-i list nodes
no
$ KUBECONFIG=<ro> kubectl auth can-i list pods
yes
$ KUBECONFIG=<ro> timeout 40 ./target/debug/k8rs --live
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
▲ k8rs cannot read nodes from this cluster right now — what is shown about them may be out of date

```

Nothing else printed in 40 seconds.

With `pods` only (four kinds refused), a pod the reader *is* allowed to see was
created and put into `Error` mid-run:

```
$ kubectl --context kind-review run canary --image=busybox --restart=Never --command -- sh -c 'exit 1'
pod/canary created
$ KUBECONFIG=<probe> kubectl get pod canary
NAME     READY   STATUS   RESTARTS   AGE
canary   0/1     Error    0          25s
```

k8rs output after that pod appeared: no new lines. Last output was the connect
banner block at 13:33:23; nine minutes of silence followed.

`src/k8s.rs:1091` `Store::snapshot` returns `None` unless `listed()` — which is
`still_listing().is_empty()` over all five watches.

---

## 6. The documented read-only role on a cluster with the default discovery bindings gone

The three default bindings auto-reconcile: deleted at 13:41 they were back
within minutes, so they were pinned off instead —

```
$ kubectl annotate clusterrolebinding system:discovery \
    rbac.authorization.kubernetes.io/autoupdate=false --overwrite
$ kubectl patch clusterrolebinding system:discovery --type=json -p '[{"op":"remove","path":"/subjects"}]'
   (same for system:basic-user and system:public-info-viewer)

$ kubectl get clusterrolebinding system:discovery system:basic-user system:public-info-viewer \
    -o jsonpath='{range .items[*]}{.metadata.name}{" subjects="}{.subjects}{" autoupdate="}...'
system:discovery subjects= autoupdate=false
system:basic-user subjects= autoupdate=false
system:public-info-viewer subjects= autoupdate=false
```

`docs/security.md`'s `k8rs-readonly` applied verbatim (lines 91–136), bound to
one ServiceAccount, nothing else.

**A. the same role with the `nonResourceURLs` rule deleted**

```
$ KUBECONFIG=<ro> kubectl get --raw /apis
Error from server (Forbidden): forbidden: User "system:serviceaccount:default:ro" cannot get path "/apis"

$ KUBECONFIG=<ro> timeout 25 ./target/debug/k8rs --live
k8rs: watching — the server would not say which version it is · the cluster would not say which kinds it serves
12 pods · 2 nodes

▲ kube-system/kube-apiserver-review-control-plane · 10 min ago
  The last run on record failed — exit 137 (killed with SIGKILL — a stop the program cannot refuse, and the code does not say what sent it)
  container kube-apiserver · ran for 24 min
  → check the liveness and startup probes, whether it stops when asked to, and the memory limit: ...

1 warning
```

**B. the documented role verbatim**

```
$ KUBECONFIG=<ro> kubectl get --raw /apis
{"kind":"APIGroupList","apiVersion":"v1","groups":[{"name":"apiregi ...

$ KUBECONFIG=<ro> timeout 25 ./target/debug/k8rs --live
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
12 pods · 2 nodes

▲ kube-system/kube-apiserver-review-control-plane · 11 min ago
  ...
1 warning
```

All five watches complete under the documented role; no `▲` watch-health line
in either run.

---

## 7. Round trips on a healthy run

`apiserver_request_total` for the five watched kinds, before and after a
45-second run under the read-only role:

```
                                        before  after
group="",     resource="pods",   LIST        1      2
group="",     resource="nodes",  LIST        2      3
group="apps", resource="deployments",  LIST  1      2
group="apps", resource="statefulsets", LIST  1      2
group="apps", resource="daemonsets",   LIST  1      2
```

Exactly one LIST per watched kind for the whole run. (`WATCH` deltas are larger
because the cluster's own controllers watch the same kinds.)

`src/k8s.rs:1473` `const INITIAL_LIST_PAGE: u32 = 500;`

---

## 8. Token hygiene — every stream, every path

Credential planted in each kubeconfig; `grep -c -F` over every byte of stdout
and stderr, separately captured.

| path | exit | token found | JWT-shaped string found |
|---|---|---|---|
| happy run, read-only SA (bearer token, 917 bytes) | 124 (timeout) | 0 | none |
| `--context=nope` | 2 | 0 | none |
| server port rewritten to `:1` | 124 (timeout) | 0 | none |
| `exec` plugin exits 3, prints canary to stdout **and** stderr | 2 | 0 | none |
| `exec` plugin succeeds, hands a canary token the server rejects | 124 (timeout) | 0 | none |

```
$ grep -oE 'eyJ[A-Za-z0-9_-]{8}' o1.out o1.err o2.out o2.err o3.out o3.err
(no output)
$ grep -c -E "CANARY" o4.out o4.err o5.out o5.err
0 0 0 0
```

For contrast, `kubectl` with the same failing plugin does print the plugin's
stderr:

```
$ KUBECONFIG=<exec> kubectl get pods
CANARYSTDERR-...-LEAKME
Unable to connect to the server: getting credentials: exec: executable ... failed with exit code 3
```

k8rs printed only:

```
k8rs: no cluster to watch — the kubeconfig was read and no client could be built from what is in it
```

That sentence is the `NotConnected::Client` arm, which `src/k8s.rs:3319` marks
`#[expect(dead_code, ...)]` and documents as reachable only through broken TLS
material.

---

## 9. The connection-failure shapes, as they arrive today

```
$ KUBECONFIG=/nonexistent/path ./target/debug/k8rs --live
k8rs: no cluster to watch — the kubeconfig could not be read, or names no such context
$ KUBECONFIG=<empty file> ./target/debug/k8rs --live
k8rs: no cluster to watch — the kubeconfig could not be read, or names no such context
$ KUBECONFIG=<invalid yaml> ./target/debug/k8rs --live
k8rs: no cluster to watch — the kubeconfig could not be read, or names no such context
$ KUBECONFIG=<no current-context> ./target/debug/k8rs --live
k8rs: no cluster to watch — the kubeconfig could not be read, or names no such context
$ ./target/debug/k8rs --live --context=nope
k8rs: no cluster to watch — the kubeconfig could not be read, or names no such context
```

Five shapes, one sentence, all `NotConnected::Kubeconfig`. The sixth — a
failing `exec` plugin — is `NotConnected::Client` and is in the table above.

A server that is reachable but answers nothing (port rewritten to `:1`):

```
k8rs: watching — the server would not say which version it is · the cluster would not say which kinds it serves
▲ k8rs cannot read pods from this cluster right now — what is shown about them may be out of date
...
```

---

## 10. Flags

```
$ ./target/debug/k8rs --live --contxt=kind-review
k8rs: --contxt=kind-review is not a flag k8rs has
usage: k8rs [--analysis] <file.json>...   |   k8rs --live [--context <name>]
Each file holds Kubernetes objects as JSON: one object, or a list of them.
Without --live this build reads files only — it cannot reach a cluster.

$ ./target/debug/k8rs --live --context --live
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
12 pods · 2 nodes
...
$ kubectl config current-context
kind-review
```

The second form printed no message and watched the kubeconfig's current
context. `src/main.rs:688-692`:

```rust
if arg == CONTEXT {
    return Some(
        rest.next()
            .map(String::as_str)
            .filter(|value| !value.starts_with(FLAG)),
    );
}
```

---

## Teardown

```
$ K8RS_CLUSTER=review bash scripts/cluster.sh down
```

---

# Re-measurement, 2026-08-27 15:01–15:46 — the idle proof re-run against `StandingBackoff`

Section 1's proof was taken against the backoff that reset on every `Ok(Event::Init)`
(section 4 has the mechanism). That policy was replaced by `StandingBackoff`
(`src/k8s.rs:3552`), so the proof does not carry over and was re-run whole.

Same discipline: ephemeral cluster, `K8RS_CLUSTER=review`, one control-plane and one
worker, `kindest/node:v1.36.1`, API on port 6444. Nodes `review-control-plane` /
`review-worker`, refused by `scripts/sanitize.jq`. The PM's `k8rs` fixture cluster was
up beside it; `tests/fixtures/` last written 2026-08-23, no capture in flight. Host: 23
GiB RAM, 18 GiB available, `/tmp` 34% of 12 GiB, `$HOME` 5% of 954 GB.

## 11. The binary under test is the fixed one

```
$ cargo build
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s

$ grep -a -c -F "from this cluster right now" target/debug/k8rs      # the old sentence
0
$ grep -a -c -F "keeps asking, and until" target/debug/k8rs          # the new one
1
$ grep -a -c -F "needs the name of a context" target/debug/k8rs
1
$ nm -C target/debug/k8rs | grep -c StandingBackoff
293
```

`StreamBackoff<…, k8rs::k8s::StandingBackoff>` is monomorphised for all five watched
kinds (`Pod`, `Node`, `Deployment`, `StatefulSet`, `DaemonSet`) in the symbol table.

## 12. What the retry was counted with

`strace`, `tcpdump`, `bpftrace` and `perf` are all absent on this host; `ss` and `nstat`
are present. A refused connect to `127.0.0.1:6444` increments `Tcp: AttemptFails` in
`/proc/net/snmp`, and the kind nodes run in their own network namespaces, so that
counter is host-side traffic only. Its noise floor with nothing running:

```
$ nstat -a | grep -E 'TcpActiveOpens|TcpAttemptFails'
TcpActiveOpens                  3192               0.0
TcpAttemptFails                 238                0.0
   (30 s later)
(no delta on either counter)
```

Zero over 30 s. Sampled every 0.5 s alongside `utime+stime` from `/proc/<pid>/stat`.

## 13. The long outage — 5 m 43 s down, nobody at the keyboard

```
$ ./target/debug/k8rs --live --context kind-review 2>&1 \
    | stdbuf -oL awk '{ printf "%s | %s\n", strftime("%H:%M:%S"), $0; fflush() }'

START 15:03:22
15:03:22 | k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
15:03:22 | 11 pods · 2 nodes
15:03:22 |
15:03:22 | ○ nothing is broken
```

First report block under one second, unchanged.

```
$ docker stop review-control-plane      # returned 15:04:41.73
```

All five health lines up inside the same second, one kind at a time:

```
15:04:41 | ▲ k8rs is not getting pods from this cluster — it keeps asking, and until that works nothing here about them can be trusted
15:04:41 | ▲ k8rs is not getting nodes from this cluster — it keeps asking, and until that works nothing here about them can be trusted
15:04:41 | ▲ k8rs is not getting Deployments from this cluster — it keeps asking, and until that works nothing here about them can be trusted
15:04:41 | ▲ k8rs is not getting StatefulSets from this cluster — it keeps asking, and until that works nothing here about them can be trusted
15:04:41 | ▲ k8rs is not getting DaemonSets from this cluster — it keeps asking, and until that works nothing here about them can be trusted
15:04:41 |
15:04:41 | 11 pods · 2 nodes
15:04:41 |
15:04:41 | ○ nothing is broken
```

Nothing further printed for the remaining 5 m 30 s.

### The retry interval, as it grew

Every change in `Tcp: AttemptFails`. Each row is one refused connect; the five watches
are interleaved, and they arrive in clean generations of exactly five.

```
15:04:43.12  AF=243 (+5)     <- generation 1, all five
15:04:45.21  AF=244 (+1)
15:04:45.73  AF=248 (+4)     <- generation 2 complete
15:04:49.38  AF=249 (+1)
15:04:49.90  AF=251 (+2)
15:04:51.47  AF=252 (+1)
15:04:51.99  AF=253 (+1)     <- generation 3 complete
15:04:56.18  AF=254 (+1)
15:04:56.70  AF=255 (+1)
15:05:02.43  AF=256 (+1)
15:05:03.47  AF=257 (+1)
15:05:04.52  AF=258 (+1)     <- generation 4 complete
15:05:12.87  AF=259 (+1)
15:05:19.14  AF=260 (+1)
15:05:20.71  AF=261 (+1)
15:05:21.23  AF=262 (+1)
15:05:24.89  AF=263 (+1)     <- generation 5 complete
15:05:38.99  AF=264 (+1)
15:05:55.19  AF=265 (+1)
15:05:55.71  AF=266 (+1)
15:06:01.46  AF=267 (+1)
15:06:05.64  AF=268 (+1)     <- generation 6 complete
15:06:19.71  AF=269 (+1)
15:06:37.48  AF=270 (+1)
15:06:50.03  AF=271 (+1)
15:06:53.68  AF=272 (+1)
15:07:04.12  AF=273 (+1)     <- generation 7 complete
15:07:11.45  AF=274 (+1)
15:07:15.09  AF=275 (+1)
15:07:42.22  AF=276 (+1)
15:07:51.63  AF=277 (+1)
15:07:53.19  AF=278 (+1)     <- generation 8 complete
15:08:02.06  AF=279 (+1)
15:08:06.23  AF=280 (+1)
15:08:31.32  AF=281 (+1)
15:08:42.29  AF=282 (+1)
15:08:42.81  AF=283 (+1)     <- generation 9 complete
15:08:50.12  AF=284 (+1)
15:09:05.27  AF=285 (+1)
15:09:18.31  AF=286 (+1)
15:09:21.96  AF=287 (+1)
```

Median gap per generation — i.e. the per-watch retry interval:

| generation | 1→2 | 2→3 | 3→4 | 4→5 | 5→6 | 6→7 | 7→8 | 8→9 |
|---|---|---|---|---|---|---|---|---|
| seconds | 2.6 | 4.2 | 12.5 | 18.3 | 35.0 | 54.3 | 52.2 | 49.1 |

53 refused connects across five watches over the 343 s outage. The same outage under
the policy section 4 measured (1.20 s flat) would have been 5 × 343 / 1.20 ≈ **1430**.

### Why the plateau is ~50 s and not the 30 s the constant names

```
$ sed -n '981,988p' kube-runtime-4.2.0/src/watcher.rs
impl Default for DefaultBackoff {
    fn default() -> Self {
        Self(ResetTimerBackoff::new(
            ExponentialBackoff::new(Duration::from_millis(800), Duration::from_secs(30), 2.0, true),
            Duration::from_secs(120),
        ))

$ sed -n '232,235p' backon-1.6.0/src/backoff/exponential.rs
        // If jitter is enabled, add random jitter based on min delay.
        if self.jitter {
            tmp_cur = tmp_cur.saturating_add(tmp_cur.mul_f32(self.rng.f32()));
        }
```

`with_jitter` **multiplies** by `1 + U(0,1)`, it does not add a small offset, and it is
applied after the 30 s cap. The delay handed out at the plateau is therefore uniform on
**[30 s, 60 s)**, mean 45 s — which is what the generation table measures.

### CPU

`utime+stime` from `/proc/<pid>/stat`, `CLK_TCK=100`.

```
15:04:42.60 AF=238 AO=3226 ticks=6   vctx=78     <- outage begins
15:05:42.66 AF=264 AO=3252 ticks=13  vctx=150    delta 7 ticks = .07 s over 60 s
15:06:42.70 AF=270 AO=3258 ticks=14  vctx=165    delta 1 tick  = .01 s over 60 s
15:09:12.57 AF=285 AO=3299 ticks=18  vctx=207
15:10:03.73 AF=291 AO=3306 ticks=19  vctx=224
```

Section 1's figure to beat: **0.03 s over 60 s**. Minute one costs 0.07 s — the ramp is
still short and five report blocks are rendered. From minute two on it is **0.01 s over
60 s**. Whole outage: 14 ticks (6 -> 20) = 0.14 s over 343 s.

### Recovery, with nobody touching it

```
$ docker start review-control-plane      # 15:10:24.47
$ ./apiready.sh                          # kubectl --request-timeout=2s get --raw /readyz, every 0.5 s
API /readyz OK at 15:10:37.43            # +12.96 s
```

```
15:11:33 | (nodes line gone)          +68.5 s after docker start, +55.6 s after /readyz
15:11:37 | (pods line gone)           +72.5 s
15:11:41 | (DaemonSets line gone)     +76.5 s
15:11:50 | (Deployments line gone)    +85.5 s
15:11:52 | (StatefulSets line gone)   +87.5 s
15:11:52 | 11 pods · 2 nodes
15:11:52 |
15:11:52 | ○ nothing is broken
```

Last block carries no `▲`. `k8rs: every watch has stopped, so nothing is being read any
more` never printed; the process never exited.

Section 1's figure, against the reset-honouring backoff and a 3 m 42 s outage:
**+101 s**. Here, against a 5 m 43 s outage with the ramp at its ceiling: **+87.5 s**.

Post-recovery idle:

```
t0: 15:33:09.08 AF=316 AO=3391 ticks=62 vctx=668
t1: 15:34:11.65 AF=316 AO=3393 ticks=62 vctx=673
3535719  0.0       30:49 25664 SNl        # %cpu, etime, RSS KiB, state
```

0 ticks over 62 s.

## 14. The control — the same outage, 15 seconds long

Run against the same process 12 minutes later, so the ramp starts from the bottom.
This separates the backoff's share of the recovery from the apiserver's own warm-up.

```
$ docker stop review-control-plane   # returned 15:24:18.22   (five ▲ at 15:24:18)
$ docker start review-control-plane  # 15:24:33.32
API /readyz OK at 15:24:37.80        # +4.5 s
```

```
nodes         back 15:24:51   +17.7 s after docker start
DaemonSets    back 15:24:57   +23.7 s
pods          back 15:24:58   +24.7 s
Deployments   back 15:25:46   +72.7 s
StatefulSets  back 15:25:49   +75.7 s
```

| | first kind back | all five back |
|---|---|---|
| 15 s outage, ramp near the bottom | +17.7 s | +75.7 s |
| 5 m 43 s outage, ramp at the ceiling | +68.5 s | +87.5 s |
| section 1, reset-honouring backoff, 3 m 42 s outage | +55 s | +101 s |

### The 120-second reset of the policy, observed

`AttemptFails` over the first 14 s of the control outage, with the previous outage
having driven every watch to the 30–60 s plateau:

```
15:24:19.41  AF=296 (+2)
15:24:19.94  AF=299 (+3)     <- all five, first failure
15:24:20.97  AF=300 (+1)
15:24:21.49  AF=302 (+2)
15:24:22.02  AF=303 (+1)
15:24:22.54  AF=304 (+1)     <- all five again, ~2 s later
15:24:24.12  AF=305 (+1)
15:24:26.21  AF=306 (+1)
15:24:27.76  AF=309 (+3)     <- all five again, ~4 s later
15:24:32.46  AF=310 (+1)
```

16 refused connects in 13 s across five watches — the 0.8 / 1.6 / 3.2 / 6.4 s ladder
starting over. At the plateau the same window would have produced 0 or 1.
`ResetTimerBackoff`'s wall-clock reset inside `next()` is reached on a real cluster.

## 15. The standing 403 — the case that runs forever

ServiceAccount granted `get,list,watch` on `pods` only, plus the `nonResourceURLs`
grant. Four of five watches refused. Counter: `authorization_attempts_total{result="no-opinion"}`.
Baseline drift with nothing running: **0 over 90 s**.

```
BEFORE=30  at 15:38:19        (k8rs started)
15:38:33 no-opinion=46
15:39:03 no-opinion=54
15:39:33 no-opinion=54
15:40:03 no-opinion=58
15:40:33 no-opinion=62
15:41:03 no-opinion=63
15:41:33 no-opinion=66
15:42:03 no-opinion=70
15:42:34 no-opinion=72
15:43:04 no-opinion=74
15:43:34 no-opinion=78
15:44:04 no-opinion=80
15:44:34 no-opinion=84
15:45:04 no-opinion=87
15:45:24 no-opinion=88
```

Steady-state window 15:41:03 → 15:45:04: 24 attempts over 241 s across 4 refused kinds
= 6 per kind, **interval 40.2 s, 89.6 requests per watch per hour**.

```
ticks(utime+stime)=18
3673106  0.0       07:05 24996 SNsl
```

0.18 s CPU over 425 s.

| | section 4 (reset honoured) | here (reset silenced) |
|---|---|---|
| per-watch interval | 1.20 s | 40.2 s |
| per watch per hour | 2985 | 89.6 |
| CPU | 3.44 s / 367 s | 0.18 s / 425 s |

Seven minutes of run printed four report blocks, all during the first second, and
nothing after.

## 16. The two reworded health lines, against both states

Standing 403, four kinds refused:

```
$ KUBECONFIG=<pods-only sa> ./target/debug/k8rs --live
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}
▲ k8rs is not getting nodes from this cluster — it keeps asking, and until that works nothing here about them can be trusted
```

Outage, same sentence, section 13 above.

| clause | under a 403 | under an outage |
|---|---|---|
| `is not getting {kind}` | true — the LIST is refused | true — the connect is refused |
| `it keeps asking` | true — 89.6/hour measured | true — 53 attempts in 343 s measured |
| `nothing here about them can be trusted` | true — the list is empty, no count is printed at all | true — the last-known counts are stale by an unknown amount |

The `●` line could not be produced against a cluster: `ended` needs a `watcher()` stream
to finish, and it did not in 45 minutes of outage, refusal and recovery. Its wording is
judged against the state it names, not measured.

## 17. `--context` followed by a flag

```
$ ./target/debug/k8rs --live --context --live
k8rs: --context needs the name of a context, and --live is a flag
usage: k8rs [--analysis] <file.json>...   |   k8rs --live [--context <name>]
Each file holds Kubernetes objects as JSON: one object, or a list of them.
Without --live this build reads files only — it cannot reach a cluster.
(exit 2)

$ ./target/debug/k8rs --live --context --analysis
k8rs: --context needs the name of a context, and --analysis is a flag
(exit 2)

$ ./target/debug/k8rs --live --context kind-review
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}

$ ./target/debug/k8rs --live --context=kind-review
k8rs: watching — server v1.36.1 · 60 kinds · {DisruptionBudgets}

$ ./target/debug/k8rs --live --context=--weird
k8rs: no cluster to watch — the kubeconfig could not be read, or names no such context
(exit 2)
```

The `--context=NAME` form is not read by the pair check, so a context whose name begins
with `--` is still reachable.

## Teardown

```
$ K8RS_CLUSTER=review bash scripts/cluster.sh down
Deleting cluster "review" ...
Deleted nodes: ["review-worker" "review-control-plane"]
$ kind get clusters
k8rs
```
