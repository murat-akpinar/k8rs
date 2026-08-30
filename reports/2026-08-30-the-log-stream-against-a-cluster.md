# The log stream against a cluster — `--logs`, the container picker, the bounds

`k8s-admin`, 2026-08-30, operator review of commit `4b82198` (Phase 6, family 1).

**Cluster:** ephemeral kind, `K8RS_CLUSTER=review`, one control-plane node,
`kindest/node:v1.36.1`, on its own API port so it never touched the PM's fixture
cluster. Created and deleted inside this run; `kind get clusters` afterwards
lists only the PM's. **Deviation to declare:** the PM's long-lived `k8rs` fixture
cluster was up (idle, four days old, no capture running) and a `just
mutants-diff` was in flight for the whole session, so this is two clusters at
once against CLAUDE.md's *one cluster at a time*. Nothing here is a timing
measurement; the one resident-set number is a peak high-water mark, which load
does not move.

**Binary:** `target/debug/k8rs`, built 21:17 from sources last written 21:00 on a
clean tree at `4b82198`. Client `kubectl v1.36.3`, server `v1.36.1`.

---

## 1. Which container is the default

```
$ kubectl get pod two-containers -o jsonpath='{.spec.containers[*].name}'
zeta alpha
$ kubectl get pod two-containers -o jsonpath='{.status.containerStatuses[*].name}'
alpha zeta
```

The kubelet sorts `status.containerStatuses` by name; `spec.containers` keeps the
order the author wrote.

```
$ kubectl logs two-containers -n default
Defaulted container "zeta" out of: zeta, alpha
ZETA-LOG-LINE

$ k8rs --logs --object default/two-containers
k8rs: this pod has 2 containers — alpha (running), zeta (running)
k8rs: reading alpha. Name another with `--container <name>`.
$ kubectl logs two-containers -n default -c alpha
ALPHA-LOG-LINE
```

With the default-container annotation set to `zeta` (pod `annotated2`, same two
containers, same order):

```
$ kubectl logs annotated2 -n default
ZETA-LOG-LINE

$ k8rs --logs --object default/annotated2
k8rs: reading alpha. Name another with `--container <name>`.
$ kubectl logs annotated2 -n default -c alpha
ALPHA-LOG-LINE
```

Sidecar pod — init `migrate`, native sidecar `envoy`, regular `web`:

```
$ k8rs --logs --object default/sidecar-pod
k8rs: this pod has 3 containers — migrate (done), envoy (running), web (running)
k8rs: reading web. Name another with `--container <name>`.
```

## 2. A Pending multi-container pod

Two containers, unsatisfiable `nodeSelector`, so `status.containerStatuses` is
absent:

```
$ kubectl get pod pending-multi -o jsonpath='{.status.phase} statuses={.status.containerStatuses}'
Pending statuses=

$ kubectl get --raw '/api/v1/namespaces/default/pods/pending-multi/log'
Error from server (BadRequest): a container name must be specified for pod pending-multi, choose one of: [zeta alpha]

$ kubectl logs pending-multi -n default   ; echo "exit $?"
Defaulted container "zeta" out of: zeta, alpha
exit 0

$ k8rs --logs --object default/pending-multi   ; echo "exit $?"
$ kubectl logs pending-multi -n default
k8rs: nothing usable came back when k8rs tried to get pods/log in default
exit 2
```

Same shape with `--previous`, and the printed line then carries `--previous`:

```
$ k8rs --logs --object default/pending-multi --previous
$ kubectl logs pending-multi -n default --previous
k8rs: nothing usable came back when k8rs tried to get pods/log in default
```

A **single**-container Pending pod is a state and reads correctly:

```
$ k8rs --logs --object default/pending-one   ; echo "exit $?"
$ kubectl logs pending-one -n default
k8rs: nothing has been written to this container's log yet
exit 0
```

## 3. Pod deleted mid-follow

`ticker` writes one line a second. `kubectl delete pod ticker --wait=false` four
seconds into the follow; the object was sampled once a second afterwards.

Grace period 1 s:

```
t+1: deletionTimestamp 2026-08-30T18:54:50Z
t+2: deletionTimestamp 2026-08-30T18:54:50Z
t+3: Error from server (NotFound): pods "ticker" not found
```

Grace period 30 s (the default for `kubectl delete`):

```
t+1 .. t+6: deletionTimestamp 2026-08-30T18:55:31Z   (still present)
```

k8rs, both runs:

```
$ kubectl logs ticker -n default -c app -f
tick-33
tick-34
EXIT=0
```

No `--- stream ended: pod deleted ---` in either run. The re-read after the
stream ends finds the object still there — terminating, with a
`deletionTimestamp` — so the fault is not `Gone`.

## 4. The bounds

Line count, 6 000 written:

```
$ kubectl logs noisy -n default -c app | wc -l
6000
$ k8rs --logs --object default/noisy > out
$ wc -l < out
5001
$ head -2 out
1000 lines were dropped from the top to keep this pane bounded.
line-1000
$ tail -1 out
line-5999
```

Retained bytes, 1 000 lines of 4 002 bytes each:

```
$ kubectl logs fat -n default | wc -l ; kubectl logs fat -n default | wc -c
1000
4003890
$ k8rs --logs --object default/fat > fat.out
$ head -1 fat.out
477 lines were dropped from the top to keep this pane bounded.
$ wc -l < fat.out ; tail -n +2 fat.out | wc -c
524
2094092
```

2 094 092 retained against the 2 097 152 ceiling; 523 lines, not 5 000.

One line of 10 000 bytes:

```
$ kubectl logs longline -n default -c app | awk '{print NR": chars="length($0)}'
1: chars=10000
2: chars=15
$ k8rs --logs --object default/longline | awk '{print NR": chars="length($0)}'
1: chars=4117
2: chars=15
$ ... | head -1 | tail -c 40
AAAAAAAAAAAAAAAAM-bM-^@M-& (shortened by k8rs)
```

4 117 characters = 4 096 bytes cut plus the 21-character marker; the line after
the cut one survives.

One unterminated line of 800 000 bytes, followed by a normal line:

```
$ kubectl logs endless -n default | head -1 | wc -c
800001
$ k8rs --logs --object default/endless   (peak VmHWM sampled from /proc)
peak VmHWM: 9184 kB
1: chars=4117
2: chars=18
```

## 5. A multi-byte character across the read ceiling

Container writes 4 098 `ESC` bytes, then U+2014 (3 bytes, so it spans byte 4 100),
then `TAIL`:

```
$ kubectl logs straddle -n default | head -1 | wc -c
4106
$ kubectl logs straddle -n default | head -1 | tail -c 12 | xxd
00000000: 1b1b 1b1b e280 9454 4149 4c0a            .......TAIL.

$ k8rs --logs --object default/straddle | xxd
00000000: efbf bde2 80a6 2028 7368 6f72 7465 6e65  ...... (shortene
00000010: 6420 6279 206b 3872 7329 0a              d by k8rs).
```

`efbfbd` is U+FFFD. The container wrote no such character.

## 6. Control characters

```
$ kubectl logs nasty -n default | xxd
00000000: 6265 666f 7265 1b5b 324a 6166 7465 720a  before.[2Jafter.
00000010: 7072 6f64 e280 ae72 6576 6572 7365 640a  prod...reversed.
00000020: 6100 620a                                a.b.

$ k8rs --logs --object default/nasty | xxd
00000000: 6265 666f 7265 5b32 4a61 6674 6572 0a70  before[2Jafter.p
00000010: 726f 6472 6576 6572 7365 640a 6162 0a    rodreversed.ab.
```

`ESC` (1b), U+202E (e2 80 ae) and `NUL` (00) removed. kubectl passes all three
through.

## 7. The printed kubectl line, pasted and diffed

Each row: run k8rs, take the line it printed on stderr, run that line verbatim,
diff its stdout against k8rs's stdout.

```
k8rs invocation                    | printed kubectl line                                       | pasted vs k8rs stdout
default container                  | kubectl logs two-containers -n default -c alpha            | IDENTICAL
chosen container                   | kubectl logs two-containers -n default -c zeta             | IDENTICAL
annotation pod                     | kubectl logs annotated2 -n default -c alpha                | IDENTICAL
--previous (restarted)             | kubectl logs slowcrash -n default -c app --previous        | IDENTICAL
--previous (never)                 | kubectl logs quiet -n default -c worker                    | IDENTICAL
namespace via --object             | kubectl logs othereum -n other -c othereum                 | IDENTICAL
namespace via flag                 | kubectl logs othereum -n other -c othereum                 | IDENTICAL
6000-line log                      | kubectl logs noisy -n default -c app                       | DIFFERS (5001 vs 6000 lines)
10000-byte line                    | kubectl logs longline -n default -c app                    | DIFFERS (2 vs 2 lines)
control characters                 | kubectl logs nasty -n default -c app                       | DIFFERS (3 vs 3 lines)
pending multi-cont.                | kubectl logs pending-multi -n default                      | IDENTICAL
```

The last row's stdout matches because both are empty; the *outcome* does not —
k8rs exits 2 with a fault sentence, the pasted line exits 0.

`--object other/othereum --namespace default` printed `-n other` and read
`other`: the more specific half wins and the line says so.

## 8. `--previous`

Never restarted:

```
$ k8rs --logs --object default/quiet --previous
k8rs: worker hasn't restarted, so there's no previous run to show. Showing the current run instead.
$ kubectl logs quiet -n default -c worker
k8rs: nothing has been written to this container's log yet
```

Restarted once, previous log still on disk — k8rs and the pasted line agree
byte for byte:

```
$ k8rs --logs --object default/slowcrash --previous
$ kubectl logs slowcrash -n default -c app --previous
RUN-AT-1788116343
dying
```

Restarted, previous log already collected. The API answers **200** with a
sentence in the body, and both tools print it as content:

```
$ kubectl logs looper -n default -c app --previous   ; echo "exit $?"
unable to retrieve container logs for containerd://<64-hex id>exit 0

$ k8rs --logs --object default/looper --previous   ; echo "exit $?"
$ kubectl logs looper -n default -c app --previous
unable to retrieve container logs for containerd://<64-hex id>
exit 0
```

A crashlooping single-container pod with no flags reads correctly:

```
$ k8rs --logs --object default/looper
$ kubectl logs looper -n default -c app
FATAL: cannot connect to postgres
```

## 9. RBAC

Three ServiceAccounts, each with a ClusterRole of exactly the rules named.

`get,list,watch` on `pods`, nothing on `pods/log`:

```
$ kubectl logs quiet -n default
Error from server (Forbidden): ... cannot get resource "pods/log" in API group "" in the namespace "default"

$ k8rs --logs --object default/quiet   ; echo "exit $?"
$ kubectl logs quiet -n default -c worker
k8rs: the role this kubeconfig uses needs to get pods/log in default
exit 2
```

`get` on `pods/log`, nothing on `pods` — kubectl needs the pod too, with and
without `-c`:

```
$ kubectl logs quiet -n default -c worker
Error from server (Forbidden): ... cannot get resource "pods" in API group "" in the namespace "default"

$ k8rs --logs --object default/quiet --container worker   ; echo "exit $?"
k8rs: the role this kubeconfig uses needs to get the pod quiet in default
exit 2
```

The `k8rs-readonly` ClusterRole from `docs/security.md`, extracted from the file
and applied verbatim, bound to a ServiceAccount:

```
$ k8rs --logs --object default/two-containers --container zeta   ; echo "exit $?"
$ kubectl logs two-containers -n default -c zeta
ZETA-LOG-LINE
exit 0
```

## 10. What one `--logs` run costs the API server

`apiserver_request_total` sampled before and after one
`k8rs --logs --object default/quiet`, with an idle window of the same length
measured first to identify background traffic. Deltas attributable to the run:

```
+1  subresource="api",  verb="GET"
+1  subresource="apis", verb="GET"
+1  resource="pods",   scope="cluster",  verb="LIST"
+1  resource="pods",   scope="resource", verb="GET"
+1  resource="pods",   subresource="log", verb="GET"
```

No `WATCH` on any resource. The cluster-scope LIST is the coverage probe and
carries `limit=1`.

## 11. Error and refusal paths

```
$ k8rs --logs --object default/no-such-pod                 exit 2
k8rs: there is no pod named no-such-pod in default — check the name and the namespace

$ k8rs --logs --object default/quiet --context nope        exit 2
k8rs: no cluster to watch — this kubeconfig has no such context — check the `--context` you gave, or the `current-context` line in the file

$ k8rs --logs --object 'default/../../secrets'             exit 2
k8rs: --object names one pod, written as `<namespace>/<name>` or just `<name>`, and ../../secrets is not one — a name is letters, digits, dashes and dots, up to 253 characters

$ k8rs --logs --object default/quiet --container '../../x' exit 2
k8rs: --container needs the name of a container, and ../../x is not one

$ k8rs --logs --object '../ns/quiet'                       exit 2
k8rs: the namespace in --object needs the name of a namespace, and .. is not one — a namespace is lowercase letters, digits and dashes, up to 63 characters

$ k8rs --logs --object $'default/we\033[2Jb'               exit 2
k8rs: --object names one pod, written as `<namespace>/<name>` or just `<name>`, and we[2Jb is not one — ...

$ k8rs --logs --object default/quiet   (server port with nothing listening)   exit 2
k8rs: nothing usable came back when k8rs tried to get the pod quiet in default
```

`k8rs --logs --object default/ticker --follow | head -3` printed two log lines
and terminated.

## 12. Teardown

```
$ kind delete cluster --name review
Deleting cluster "review" ...
Deleted nodes: ["review-control-plane"]
$ kind get clusters
k8rs
```
