# `--once` measured against a live cluster and six injected faults (2026-08-30)

Operator review of the uncommitted `--once` diff (`src/main.rs`, `src/main_tests.rs`)
at HEAD `da37c61`. Everything below was run on the dev machine against the PM's
existing four-node kind cluster (context `kind-k8rs`, server v1.36.1, 41 pods,
nodes `k8rs-control-plane` and `k8rs-worker[1-3]`). **Nothing was created,
written or deleted in the cluster**; the restricted identities are *impersonation
headers* on a copied kubeconfig, and the fault injection is a local relay in
front of `kubectl proxy`. Node count and pod count were re-checked after the run
and are unchanged (4 / 41).

The binary is `target/release/k8rs` built from the working tree.

## 1 — The healthy run

```
$ ./target/release/k8rs --once ; echo "EXIT=$?"
EXIT=0
```

stdout ends `13 critical, 3 warnings`; stderr is one line:

```
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
```

Wall clock, three consecutive runs:

```
run 1: exit=0 elapsed=.073037463
run 2: exit=0 elapsed=.065245686
run 3: exit=0 elapsed=.067747262
```

Forty consecutive runs, counting report headers on stdout per run
(`grep -c '^41 pods · 4 nodes$'`): `1` forty times out of forty.

## 2 — Pods refused (zero-RBAC identity)

Impersonating a ServiceAccount that holds no RBAC at all
(`kubectl auth can-i list pods -A` → `no`):

```
$ KUBECONFIG=<impersonating kubeconfig> ./target/release/k8rs --once ; echo "EXIT=$?"
EXIT=2
stdout: 0 bytes
stderr:
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
k8rs: the role this kubeconfig uses needs to `list` pods across the whole cluster — and this kubeconfig names no namespace, so k8rs tried default and was refused there too. Pass --namespace <name> to say which namespace you work in
k8rs: this cluster did not show k8rs its pods, and every finding starts there, so there is nothing to report — the role this kubeconfig uses needs to `list` and `watch` pods
```

Same identity, with an explicit namespace:

```
$ KUBECONFIG=<same> ./target/release/k8rs --once --namespace kube-system ; echo "EXIT=$?"
EXIT=2
stdout: 0 bytes
stderr:
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
k8rs: this cluster did not show k8rs its pods, and every finding starts there, so there is nothing to report — the role this kubeconfig uses needs to `list` and `watch` pods
```

The namespace that was refused is named in the first shape and not in the second.

## 3 — Nodes and workloads refused, pods readable

Impersonating `system:serviceaccount:kube-system:replicaset-controller`, whose
permissions measure as:

```
pods: list=yes watch=yes  nodes: list=no watch=no  deployments: list=no watch=no
statefulsets: list=no watch=no  daemonsets: list=no watch=no  replicasets: list=yes watch=yes
```

```
$ KUBECONFIG=<impersonating kubeconfig> ./target/release/k8rs --once ; echo "EXIT=$?"
EXIT=0
```

stdout, first five lines and last line:

```
▲ k8rs is not getting nodes from this cluster: the role this kubeconfig uses needs to `list` and `watch` nodes. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting Deployments from this cluster: the role this kubeconfig uses needs to `list` and `watch` deployments. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting StatefulSets from this cluster: the role this kubeconfig uses needs to `list` and `watch` statefulsets. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting DaemonSets from this cluster: the role this kubeconfig uses needs to `list` and `watch` daemonsets. It keeps asking, and until that works nothing here about them can be trusted

41 pods
...
12 critical, 2 warnings
```

The tally counts 2 warnings while four `▲` glyphs are on the same screen. The
full-permission run over the same cluster prints `41 pods · 4 nodes` and
`13 critical, 3 warnings`.

## 4 — The fault-injection harness

`kubectl proxy --port=8001` (read-only, plain HTTP) with a local Python relay on
`127.0.0.1:8999` between it and k8rs. The relay parses **every** request on a
keep-alive connection and applies one fault to a chosen endpoint; a kubeconfig
naming `http://127.0.0.1:8999` is what k8rs is pointed at. Proxy and relay are
killed in a `trap … EXIT` and not on a last line (NOTES § D185); `ss -ltn` after
the run shows no listener on 8001, 8998 or 8999.

### 4a — pod initial LIST accepted and never answered

```
MODE=hang EXIT=2 elapsed=30.028621073s
stdout: 0 bytes
stderr:
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
k8rs: this cluster has not finished answering after 30 seconds, so there is nothing to report — still reading pods (0 read so far, the last one 30s ago). Run it again: counts that have moved mean it is slow, counts that have not mean nothing is coming
```

Relay log for that run:

```
[conn 1] GET /apis HTTP/1.1
[conn 1] GET /api HTTP/1.1
[conn 1] GET /api/v1/pods?&limit=500 HTTP/1.1   [TARGET]
[conn 1]    ... swallowed, never forwarded
[conn 2] GET /api/v1/nodes?&limit=500 HTTP/1.1
[conn 3] GET /apis/apps/v1/deployments?&limit=500 HTTP/1.1
[conn 4] GET /apis/apps/v1/statefulsets?&limit=500 HTTP/1.1
[conn 5] GET /apis/apps/v1/daemonsets?&limit=500 HTTP/1.1
```

`so_far` is `0` and the sentence still says *the last one 30s ago*.

### 4b — pod initial LIST answered `429 Too Many Requests` with `Retry-After: 1`

```
MODE=429 EXIT=2 elapsed=30.031228137s
stdout: 0 bytes
stderr:
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
k8rs: this cluster has not finished answering after 30 seconds, so there is nothing to report — still reading pods (0 read so far, the last one 30s ago). Run it again: counts that have moved mean it is slow, counts that have not mean nothing is coming
```

The relay answered **12** pod LISTs with `429` inside the 30 s window; kube was
still retrying when the deadline fired. `k8s.rs § WHAT A THROTTLE LOOKS LIKE`
puts the fifteen-retry sum at 164 s at the floor and ~491 s at the jitter
ceiling.

### 4c — only the `nodes` initial LIST hangs; pods answer normally

```
MODE=hang TARGET=/api/v1/nodes EXIT=2 elapsed=30.031762604s
stdout: 0 bytes
stderr:
k8rs: watching — server v1.36.1 · 62 kinds · {Metrics, DisruptionBudgets}
k8rs: this cluster has not finished answering after 30 seconds, so there is nothing to report — still reading nodes (0 read so far, the last one 30s ago). Run it again: counts that have moved mean it is slow, counts that have not mean nothing is coming
```

The same kind **refused** (§ 3) produces a full report and exit `0`.

### 4d — metrics endpoint slower than the bootstrap gate

`--analysis`, `/apis/metrics.k8s.io` delayed 3 s, everything else untouched.

```
--once --analysis  : EXIT=0 elapsed=.105670112s
```

Capacity pane in that run:

```
[capacity]
  What each node promised, and what it has
    k8rs-control-plane   0.95 of 12 cpu · 290Mi of 23.1Gi
    k8rs-worker   0.47 of 12 cpu · 378Mi of 23.1Gi
    k8rs-worker2   0.1 of 12 cpu · 50Mi of 23.1Gi
    k8rs-worker3   0.22 of 12 cpu · 282Mi of 23.1Gi
  What each node is actually using is not shown. That number comes from metrics-server, and k8rs does not read it.
  Nothing to ask for — the numbers above are complete without it.
```

`--live --analysis` over the same relay, killed after 8 s, printed three reports.
The first carries the same two sentences; the third carries:

```
    k8rs-control-plane   0.95 of 12 cpu · 290Mi of 23.1Gi
      using 0.136 cpu and 1.1Gi
    k8rs-worker   0.47 of 12 cpu · 378Mi of 23.1Gi
      using 0.039 cpu and 605.6Mi
```

Same cluster, same 3 s delay. Without the delay, `--once --analysis` finishes in
0.075 s and does print the `using …` rows, so this is a race and not a
capability difference. The greeting on stderr says `{Metrics, DisruptionBudgets}`
in every one of these runs.

## 5 — Unreachable clusters

Connection refused (`http://127.0.0.1:1`):

```
EXIT=2 elapsed=30.014411561  stdout: 0 bytes
k8rs: watching — could not read the server version (nothing usable came back when k8rs tried to `get /version`) · could not list what this cluster serves, so k8rs cannot show you what is in it or tell which add-ons it has (nothing usable came back when k8rs tried to `get /apis`)
k8rs: this cluster has not finished answering after 30 seconds, so there is nothing to report — still reading pods (0 read so far, the last one 12s ago), nodes (0 read so far, the last one 12s ago), Deployments (0 read so far, the last one 10s ago), StatefulSets (0 read so far, the last one 12s ago), DaemonSets (0 read so far, the last one 10s ago). Run it again: counts that have moved mean it is slow, counts that have not mean nothing is coming
```

Packets dropped (an https endpoint on a TEST-NET-1 address, `timeout 400`):

```
UNROUTABLE EXIT=2 elapsed=140.021628824s   stdout: 0 bytes
```

Nothing on either stream for the first ~110 s; then the same two lines as above,
with all five ages reading `30s ago`.

Accepts the TCP connection and answers nothing (`timeout 75`):

```
EXIT=124 elapsed=75.004873258s   stdout: 0 bytes   stderr: 0 bytes
```

No kubeconfig at all:

```
EXIT=2 elapsed=.006834080
k8rs: no cluster to watch — the kubeconfig itself could not be read — it is missing, unreadable, or not valid YAML
```

## 6 — Flag surface

```
$ k8rs --once pod.json      EXIT=2  k8rs: --once and --live read a cluster, so k8rs cannot also read pod.json — run it with the flag, or with the file, not both
$ k8rs --once --read-only   EXIT=2  k8rs: --read-only is not a flag k8rs has
$ k8rs --once -o json       EXIT=2  k8rs: --once and --live read a cluster, so k8rs cannot also read json — run it with the flag, or with the file, not both
$ k8rs --once --live        EXIT=0  (runs once)
$ k8rs --once --context     EXIT=0  (connects to the current context, no message)
$ k8rs --once --context ""  EXIT=2  k8rs: no cluster to watch — this kubeconfig has no such context — …
$ k8rs --once --namespace   EXIT=2  k8rs: --namespace needs the name of a namespace
$ k8rs --once -nkube-system EXIT=2  k8rs: the namespace has to be separate from -n — write it as `-n <name>` or `-n=<name>`
$ k8rs --once --anaylsis    EXIT=2  k8rs: --anaylsis is not a flag k8rs has
```

Namespace that does not exist:

```
$ k8rs --once --namespace no-such-namespace-here ; echo "EXIT=$?"
ns: no-such-namespace-here · 0 pods · 4 nodes

○ nothing is broken in no-such-namespace-here

One node check is off: spotting a node someone started emptying and did not finish needs every pod in the cluster.

EXIT=0
```

## 7 — Write-failure paths on stdout

```
$ k8rs --once 2>/dev/null | head -1     → report line printed, no error
$ k8rs --once > /dev/full               → EXIT=2  k8rs: the report could not be written — No space left on device (os error 28)
$ bash -c 'exec 2>/tmp/e; exec 1>&-; exec k8rs --once'  → EXIT=0, no report
```

The third is Rust's own `sanitize_standard_fds` reopening a closed fd 1 onto the
null device before `main` runs; the write therefore succeeds.

## 8 — Which function owns which doc comment

```
$ cargo doc --no-deps --document-private-items
$ # docblock extracted from the generated HTML
fn.unreadable.html -> (NO DOCBLOCK)          docblock length: 0
fn.plain_kind.html -> "What this tool is not being given, one plain line each — empty when all
                       five are delivering. The reconnect proof reads off this and not off
                       silence (NOTES § D161). A watch that die…"
                                             docblock length: 4266
```

`src/main.rs:1952-2014` is one contiguous `///` block; `fn plain_kind` is at
`:2015` and `fn unreadable` at `:2026` with nothing between it and the closing
brace of `plain_kind`.

## 9 — Where the abort is checked

`futures-util-0.3.31/src/abortable.rs:64-68`, on `Abortable::is_aborted`:

```
/// Checks whether the task has been aborted. Note that all this
/// method indicates is whether [`AbortHandle::abort`] was *called*.
/// This means that it will return `true` even if:
/// * `abort` was called after the task had completed.
/// * `abort` was called while the task was being polled - the task may still be running and
///   will not be stopped until `poll` returns.
```

## 10 — What `--live` says about the same unreachable endpoint

`--live` against `http://127.0.0.1:1` (connection refused), killed after 25 s.
stderr is the same greeting as § 5; stdout, last block of five:

```
▲ k8rs is not getting pods from this cluster: nothing usable came back when k8rs tried to `list` and `watch` pods. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting nodes from this cluster: nothing usable came back when k8rs tried to `list` and `watch` nodes. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting Deployments from this cluster: nothing usable came back when k8rs tried to `list` and `watch` deployments. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting StatefulSets from this cluster: nothing usable came back when k8rs tried to `list` and `watch` statefulsets. It keeps asking, and until that works nothing here about them can be trusted
▲ k8rs is not getting DaemonSets from this cluster: nothing usable came back when k8rs tried to `list` and `watch` daemonsets. It keeps asking, and until that works nothing here about them can be trusted
```

The first of these appeared within the first second of the run. `--once` over
the same endpoint (§ 5) printed none of them and ended on the *has not finished
answering* sentence instead.

## 11 — Concurrency note

`src/main.rs` and `src/main_tests.rs` were last written at 13:25 and the measured
binary was built at 13:39, so every measurement above is against one stable
subject. `tests/binary.rs` changed at 14:17, during this run — a second writer
holds `tests/`. It cannot affect the product binary, and it was not reviewed.
